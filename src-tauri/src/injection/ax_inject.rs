//! macOS Accessibility (AXUIElement) text insert / selection for focused fields.
//!
//! Chromium / Electron / many web views often accept `AXSelectedText` / `AXValue`
//! writes with `kAXErrorSuccess` without actually changing the field. Only trust
//! AX insert for real native text roles, and only after a best-effort verify.

use std::ffi::c_void;
use std::ptr;
use std::time::{Duration, Instant};

use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};

use crate::error::{OtoError, OtoResult};
use crate::injection::paste::is_process_trusted;

type AXUIElementRef = *mut c_void;
type AXError = i32;

const AX_ERROR_SUCCESS: AXError = 0;

/// Roles where settable text is usually real (not a web content shell).
const INSERTABLE_ROLES: &[&str] = &[
    "AXTextField",
    "AXTextArea",
    "AXComboBox",
    "AXSearchField",
    "AXSecureTextField",
];

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout_in_seconds: f32) -> AXError;
    fn CFRelease(cf: CFTypeRef);
}

fn attr(name: &str) -> CFString {
    CFString::new(name)
}

fn copy_attribute(element: AXUIElementRef, name: &str) -> Option<CFTypeRef> {
    if element.is_null() {
        return None;
    }
    let mut value: CFTypeRef = ptr::null();
    let status = unsafe {
        AXUIElementCopyAttributeValue(element, attr(name).as_concrete_TypeRef(), &mut value)
    };
    if status != AX_ERROR_SUCCESS || value.is_null() {
        return None;
    }
    Some(value)
}

fn cfstring_to_string(value: CFTypeRef) -> Option<String> {
    if value.is_null() {
        return None;
    }
    let s = unsafe { CFString::wrap_under_get_rule(value as CFStringRef) };
    Some(s.to_string())
}

fn attribute_string(element: AXUIElementRef, name: &str) -> Option<String> {
    let value = copy_attribute(element, name)?;
    let text = cfstring_to_string(value);
    unsafe {
        CFRelease(value);
    }
    text
}

fn focused_element() -> Option<AXUIElementRef> {
    let system = unsafe { AXUIElementCreateSystemWide() };
    if system.is_null() {
        return None;
    }
    // Fail fast on hung apps (Chromium AX can stall for seconds).
    unsafe {
        let _ = AXUIElementSetMessagingTimeout(system, 0.35);
    }
    let focused = copy_attribute(system, "AXFocusedUIElement");
    unsafe {
        CFRelease(system as CFTypeRef);
    }
    let element = focused.map(|v| v as AXUIElementRef)?;
    unsafe {
        let _ = AXUIElementSetMessagingTimeout(element, 0.35);
    }
    Some(element)
}

fn role_is_insertable(role: &str) -> bool {
    INSERTABLE_ROLES
        .iter()
        .any(|allowed| role.eq_ignore_ascii_case(allowed))
}

/// Try to insert `text` into a *native* focused text field.
/// Returns `Ok(true)` only when we believe text was actually applied.
pub fn try_ax_insert(text: &str) -> OtoResult<bool> {
    if text.is_empty() || !is_process_trusted(false) {
        return Ok(false);
    }

    let started = Instant::now();
    let Some(element) = focused_element() else {
        return Ok(false);
    };

    let role = attribute_string(element, "AXRole").unwrap_or_default();
    if !role_is_insertable(&role) {
        unsafe {
            CFRelease(element as CFTypeRef);
        }
        // Web areas / groups / unknown roles: let clipboard+paste handle it.
        return Ok(false);
    }

    let before = attribute_string(element, "AXValue").unwrap_or_default();
    let selected_before = attribute_string(element, "AXSelectedText").unwrap_or_default();

    // Prefer replacing the selection / inserting at caret via AXSelectedText.
    let selected_attr = attr("AXSelectedText");
    let set_selected = unsafe {
        AXUIElementSetAttributeValue(
            element,
            selected_attr.as_concrete_TypeRef(),
            CFString::new(text).as_CFTypeRef(),
        )
    };

    let mut applied = false;
    if set_selected == AX_ERROR_SUCCESS {
        // Verify the value actually changed when we can read it.
        let after = attribute_string(element, "AXValue").unwrap_or_default();
        let selected_after = attribute_string(element, "AXSelectedText").unwrap_or_default();
        applied = after != before
            || after.contains(text)
            || (!selected_before.is_empty() && selected_after != selected_before)
            || selected_after == text;
        // Some fields report success but keep AXValue empty until commit — if
        // selection was empty and value unchanged, treat as failure and paste.
        if !applied && selected_before.is_empty() && after == before {
            applied = false;
        } else if set_selected == AX_ERROR_SUCCESS && after.contains(text) {
            applied = true;
        }
    }

    if !applied {
        let value_attr = attr("AXValue");
        // Only overwrite AXValue when the field was empty or fully selected —
        // never clobber a long document with just the transcript.
        let fully_selected = !before.is_empty() && selected_before == before;
        if before.is_empty() || fully_selected {
            let set_value = unsafe {
                AXUIElementSetAttributeValue(
                    element,
                    value_attr.as_concrete_TypeRef(),
                    CFString::new(text).as_CFTypeRef(),
                )
            };
            if set_value == AX_ERROR_SUCCESS {
                let after = attribute_string(element, "AXValue").unwrap_or_default();
                applied = after == text || after.contains(text);
            }
        }
    }

    unsafe {
        CFRelease(element as CFTypeRef);
    }

    if started.elapsed() > Duration::from_millis(800) {
        eprintln!(
            "oto injection: AX insert slow ({:?}) role={role} applied={applied}",
            started.elapsed()
        );
    }

    Ok(applied)
}

/// Title of the focused window of the application owning `pid`.
///
/// Modes match on it and the context builder may disclose it, so a failure has
/// to be a quiet `None` rather than an error: a missing title must never stop a
/// dictation.
pub fn focused_window_title(pid: i32) -> Option<String> {
    if !is_process_trusted(false) {
        return None;
    }
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return None;
    }
    unsafe {
        let _ = AXUIElementSetMessagingTimeout(app, 0.35);
    }
    // AXFocusedWindow is the typing target; AXMainWindow is the fallback for
    // applications that never mark a window focused.
    let window = copy_attribute(app, "AXFocusedWindow")
        .or_else(|| copy_attribute(app, "AXMainWindow"))
        .map(|v| v as AXUIElementRef);
    unsafe {
        CFRelease(app as CFTypeRef);
    }
    let window = window?;
    unsafe {
        let _ = AXUIElementSetMessagingTimeout(window, 0.35);
    }
    let title = attribute_string(window, "AXTitle");
    unsafe {
        CFRelease(window as CFTypeRef);
    }
    title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// Try to read the focused field's selected text via Accessibility.
pub fn try_ax_selection() -> OtoResult<Option<String>> {
    if !is_process_trusted(false) {
        return Ok(None);
    }
    let Some(element) = focused_element() else {
        return Ok(None);
    };
    let selected = attribute_string(element, "AXSelectedText");
    unsafe {
        CFRelease(element as CFTypeRef);
    }
    Ok(selected.filter(|s| !s.is_empty()))
}

#[allow(dead_code)]
pub fn ax_unavailable_message() -> OtoError {
    OtoError::Message(
        "macOS Accessibility is unavailable — enable Oto in System Settings → Privacy & Security → Accessibility".into(),
    )
}
