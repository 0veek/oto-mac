//! Keyboard simulation for paste/copy/type on macOS.
//!
//! Strategy (in order) for ⌘V:
//! 1. CGEvent posted **to the target PID** (most reliable when focus is known)
//! 2. CGEvent posted to the session event stream
//! 3. AppleScript / System Events `keystroke "v" using command down`
//!
//! Clipboard is already set by the caller before `simulate_paste`.

use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode, KeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use serde::Serialize;

use crate::error::{OtoError, OtoResult};

const KEY_C: CGKeyCode = 8;
const KEY_V: CGKeyCode = 9;

static PROMPTED_THIS_SESSION: AtomicBool = AtomicBool::new(false);

fn event_source() -> OtoResult<CGEventSource> {
    // Private sources are preferred for synthetic events.
    CGEventSource::new(CGEventSourceStateID::Private)
        .or_else(|_| CGEventSource::new(CGEventSourceStateID::CombinedSessionState))
        .or_else(|_| CGEventSource::new(CGEventSourceStateID::HIDSystemState))
        .map_err(|_| OtoError::Message("could not create CGEventSource".into()))
}

fn make_key(source: &CGEventSource, keycode: CGKeyCode, key_down: bool, flags: CGEventFlags) -> OtoResult<CGEvent> {
    let event = CGEvent::new_keyboard_event(source.clone(), keycode, key_down)
        .map_err(|_| OtoError::Message("could not create keyboard event".into()))?;
    event.set_flags(flags);
    Ok(event)
}

/// Post a single key event exactly once (global session stream).
fn post_key_global(keycode: CGKeyCode, key_down: bool, flags: CGEventFlags) -> OtoResult<()> {
    let source = event_source()?;
    let event = make_key(&source, keycode, key_down, flags)?;
    // One tap location only — posting to HID + Session + AnnotatedSession
    // caused the same ⌘V to fire 3 times in the focused app.
    event.post(CGEventTapLocation::HID);
    Ok(())
}

/// Post a single key event exactly once to a process (no global fan-out).
fn post_key_to_pid(pid: i32, keycode: CGKeyCode, key_down: bool, flags: CGEventFlags) -> OtoResult<()> {
    let source = event_source()?;
    let event = make_key(&source, keycode, key_down, flags)?;
    event.post_to_pid(pid);
    Ok(())
}

fn command_chord_global(keycode: CGKeyCode) -> OtoResult<()> {
    let cmd = KeyCode::COMMAND;
    post_key_global(cmd, true, CGEventFlags::CGEventFlagCommand)?;
    thread::sleep(Duration::from_millis(20));
    post_key_global(keycode, true, CGEventFlags::CGEventFlagCommand)?;
    thread::sleep(Duration::from_millis(30));
    post_key_global(keycode, false, CGEventFlags::CGEventFlagCommand)?;
    thread::sleep(Duration::from_millis(20));
    post_key_global(cmd, false, CGEventFlags::empty())?;
    Ok(())
}

fn command_chord_to_pid(pid: i32, keycode: CGKeyCode) -> OtoResult<()> {
    let cmd = KeyCode::COMMAND;
    post_key_to_pid(pid, cmd, true, CGEventFlags::CGEventFlagCommand)?;
    thread::sleep(Duration::from_millis(20));
    post_key_to_pid(pid, keycode, true, CGEventFlags::CGEventFlagCommand)?;
    thread::sleep(Duration::from_millis(30));
    post_key_to_pid(pid, keycode, false, CGEventFlags::CGEventFlagCommand)?;
    thread::sleep(Duration::from_millis(20));
    post_key_to_pid(pid, cmd, false, CGEventFlags::empty())?;
    Ok(())
}

/// AppleScript paste — often works when CGEvent is filtered but Accessibility is granted.
fn paste_via_osascript() -> OtoResult<()> {
    let script = r#"
tell application "System Events"
    keystroke "v" using {command down}
end tell
"#;
    let output = Command::new("osascript")
        .args(["-e", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| OtoError::Message(format!("osascript paste failed to start: {e}")))?;
    if output.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(OtoError::Message(format!(
        "osascript paste failed: {}",
        if err.is_empty() {
            output.status.to_string()
        } else {
            err
        }
    )))
}

fn copy_via_osascript() -> OtoResult<()> {
    let script = r#"
tell application "System Events"
    keystroke "c" using {command down}
end tell
"#;
    let output = Command::new("osascript")
        .args(["-e", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| OtoError::Message(format!("osascript copy failed to start: {e}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(OtoError::Message(format!(
            "osascript copy failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

/// Release leftover modifiers from the push-to-talk chord (Ctrl/Shift/etc.).
fn release_modifiers(pid: Option<i32>) {
    let keys = [
        KeyCode::COMMAND,
        KeyCode::CONTROL,
        KeyCode::SHIFT,
        KeyCode::OPTION,
        KeyCode::RIGHT_COMMAND,
        KeyCode::RIGHT_CONTROL,
        KeyCode::RIGHT_SHIFT,
        KeyCode::RIGHT_OPTION,
    ];
    for key in keys {
        // One path only — dual pid+global releases also duplicated key-ups.
        if let Some(pid) = pid.filter(|p| *p > 0) {
            let _ = post_key_to_pid(pid, key, false, CGEventFlags::empty());
        } else {
            let _ = post_key_global(key, false, CGEventFlags::empty());
        }
    }
    thread::sleep(Duration::from_millis(40));
}

fn not_trusted_message(action: &str) -> OtoError {
    let path = executable_path().unwrap_or_else(|| "(unknown path)".into());
    let name = display_name();
    OtoError::Message(format!(
        "Accessibility permission required for {action}. \
Open System Settings → Privacy & Security → Accessibility, unlock, click +, add “{name}” \
({path}), enable the toggle, then quit and reopen Oto."
    ))
}

/// Simulate ⌘V. Prefer events targeted at `target_pid` when known.
pub fn simulate_paste() -> OtoResult<()> {
    simulate_paste_to(None)
}

/// Simulate ⌘V into a specific process when possible.
pub fn simulate_paste_to(target_pid: Option<i32>) -> OtoResult<()> {
    if !is_process_trusted(false) {
        return Err(not_trusted_message("paste simulation (⌘V)"));
    }

    release_modifiers(target_pid);
    thread::sleep(Duration::from_millis(50));

    let mut errors = Vec::new();

    if let Some(pid) = target_pid.filter(|p| *p > 0) {
        match command_chord_to_pid(pid, KEY_V) {
            Ok(()) => {
                eprintln!("oto injection: paste via CGEventPostToPid({pid})");
                thread::sleep(Duration::from_millis(40));
                return Ok(());
            }
            Err(e) => errors.push(format!("pid-post: {e}")),
        }
    }

    match command_chord_global(KEY_V) {
        Ok(()) => {
            eprintln!("oto injection: paste via global CGEvent");
            thread::sleep(Duration::from_millis(40));
            return Ok(());
        }
        Err(e) => errors.push(format!("global: {e}")),
    }

    match paste_via_osascript() {
        Ok(()) => {
            eprintln!("oto injection: paste via osascript/System Events");
            thread::sleep(Duration::from_millis(40));
            return Ok(());
        }
        Err(e) => errors.push(format!("osascript: {e}")),
    }

    Err(OtoError::Message(format!(
        "all paste methods failed: {}",
        errors.join("; ")
    )))
}

/// Simulate ⌘C.
pub fn simulate_copy() -> OtoResult<()> {
    simulate_copy_to(None)
}

pub fn simulate_copy_to(target_pid: Option<i32>) -> OtoResult<()> {
    if !is_process_trusted(false) {
        return Err(not_trusted_message("copy simulation (⌘C)"));
    }
    release_modifiers(target_pid);
    thread::sleep(Duration::from_millis(40));

    if let Some(pid) = target_pid.filter(|p| *p > 0) {
        if command_chord_to_pid(pid, KEY_C).is_ok() {
            return Ok(());
        }
    }
    if command_chord_global(KEY_C).is_ok() {
        return Ok(());
    }
    copy_via_osascript()
}

/// Type `text` as unicode key events.
pub fn simulate_type(text: &str) -> OtoResult<()> {
    simulate_type_to(text, None)
}

pub fn simulate_type_to(text: &str, target_pid: Option<i32>) -> OtoResult<()> {
    if text.is_empty() {
        return Err(OtoError::Message("cannot type empty text".into()));
    }
    if !is_process_trusted(false) {
        return Err(not_trusted_message("typing"));
    }
    release_modifiers(target_pid);
    thread::sleep(Duration::from_millis(40));

    let source = event_source()?;
    for ch in text.chars() {
        if ch == '\r' {
            continue;
        }
        let event = CGEvent::new_keyboard_event(source.clone(), 0, true)
            .map_err(|_| OtoError::Message("could not create type event".into()))?;
        event.set_string(ch.to_string().as_str());
        event.set_flags(CGEventFlags::empty());
        if let Some(pid) = target_pid.filter(|p| *p > 0) {
            event.post_to_pid(pid);
        } else {
            event.post(CGEventTapLocation::HID);
        }

        let up = CGEvent::new_keyboard_event(source.clone(), 0, false)
            .map_err(|_| OtoError::Message("could not create type key-up".into()))?;
        up.set_flags(CGEventFlags::empty());
        if let Some(pid) = target_pid.filter(|p| *p > 0) {
            up.post_to_pid(pid);
        } else {
            up.post(CGEventTapLocation::HID);
        }
        thread::sleep(Duration::from_millis(6));
    }
    Ok(())
}

pub fn is_process_trusted(prompt: bool) -> bool {
    unsafe {
        if prompt {
            let key = CFString::new("AXTrustedCheckOptionPrompt");
            let value = CFBoolean::true_value();
            let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
        } else {
            AXIsProcessTrusted()
        }
    }
}

pub fn executable_path() -> Option<String> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok().or(Some(p)))
        .map(|p| p.display().to_string())
}

pub fn is_bundled_app() -> bool {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

pub fn display_name() -> String {
    if is_bundled_app() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(name) = app_bundle_name(&exe) {
                return name;
            }
        }
        "Oto".into()
    } else {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "oto".into())
    }
}

fn app_bundle_name(exe: &std::path::Path) -> Option<String> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    let file = app.file_name()?.to_string_lossy();
    Some(file.trim_end_matches(".app").to_string())
}

pub fn open_accessibility_settings() -> OtoResult<()> {
    let urls = [
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Accessibility",
    ];
    let mut last_err = None;
    for url in urls {
        match Command::new("open").arg(url).status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => last_err = Some(format!("open {url} exited with {status}")),
            Err(e) => last_err = Some(e.to_string()),
        }
    }
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security")
        .status();
    Err(OtoError::Message(format!(
        "could not open Accessibility settings: {}",
        last_err.unwrap_or_else(|| "unknown error".into())
    )))
}

pub fn ensure_accessibility_prompt() -> bool {
    if is_process_trusted(false) {
        return true;
    }
    if PROMPTED_THIS_SESSION.swap(true, Ordering::SeqCst) {
        return false;
    }
    let _ = open_accessibility_settings();
    is_process_trusted(true)
}

pub fn reveal_executable_in_finder() -> OtoResult<()> {
    let path = executable_path()
        .ok_or_else(|| OtoError::Message("could not resolve Oto executable path".into()))?;
    let status = Command::new("open")
        .args(["-R", &path])
        .status()
        .map_err(|e| OtoError::Message(format!("open Finder failed: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(OtoError::Message(format!(
            "open Finder failed with status {status}"
        )))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityStatus {
    pub trusted: bool,
    pub bundled: bool,
    pub display_name: String,
    pub executable_path: String,
    pub bundle_id: String,
    pub guidance: String,
}

pub fn accessibility_status_detail() -> AccessibilityStatus {
    let trusted = is_process_trusted(false);
    let bundled = is_bundled_app();
    let display_name = display_name();
    let executable_path = executable_path().unwrap_or_default();
    let guidance = if trusted {
        "Accessibility is granted. Oto will paste with ⌘V into the focused app.".into()
    } else if bundled {
        format!(
            "Accessibility is required for insertion. Unlock Accessibility settings, click +, \
add “{display_name}.app”, enable the toggle, then quit and reopen Oto."
        )
    } else {
        format!(
            "Dev binary — add this path with + in Accessibility:\n{executable_path}"
        )
    };
    AccessibilityStatus {
        trusted,
        bundled,
        display_name,
        executable_path,
        bundle_id: "dev.oto.mac".into(),
        guidance,
    }
}

pub fn accessibility_status() -> &'static str {
    if is_process_trusted(false) {
        "trusted"
    } else {
        "not-trusted"
    }
}

#[allow(dead_code)]
pub fn path_to_authorize() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut p = exe.as_path();
    while let Some(parent) = p.parent() {
        if parent
            .file_name()
            .is_some_and(|n| n.to_string_lossy().ends_with(".app"))
        {
            return Some(parent.to_path_buf());
        }
        p = parent;
    }
    Some(exe)
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessibility_status_is_known() {
        let s = accessibility_status();
        assert!(s == "trusted" || s == "not-trusted");
    }
}
