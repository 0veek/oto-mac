//! Hybrid text injection for macOS:
//! clipboard + ⌘V (to target PID / global / AppleScript) → AX → type → clipboard-only.

mod ax_inject;
mod clipboard;
mod focus;
mod paste;

pub use clipboard::{get_clipboard_text, set_clipboard_text};
pub use focus::{
    active_focus_summary, capture_focus_target, restore_focus_target, FocusTarget,
};
pub use paste::{
    accessibility_status, accessibility_status_detail, display_name, ensure_accessibility_prompt,
    executable_path, is_bundled_app, is_process_trusted, open_accessibility_settings,
    reveal_executable_in_finder, simulate_backspace, AccessibilityStatus,
};

use crate::config::InjectionMode;
use crate::error::{OtoError, OtoResult};
use ax_inject::{try_ax_insert, try_ax_selection};
use paste::{simulate_copy_to, simulate_paste_to, simulate_type_to};

/// How text was delivered to the target application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectResult {
    Accessibility,
    DirectTyped,
    Pasted,
    ClipboardOnly,
}

pub async fn inject_text(text: &str, mode: &InjectionMode) -> OtoResult<InjectResult> {
    inject_text_to(text, mode, None).await
}

fn append_inject_log(message: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("oto-inject.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{message}");
    }
    eprintln!("oto injection: {message}");
}

fn paste_via_clipboard(text: &str, target_pid: Option<i32>) -> OtoResult<()> {
    set_clipboard_text(text)?;
    // NSPasteboard / Brave need a beat before a single ⌘V.
    std::thread::sleep(std::time::Duration::from_millis(120));
    simulate_paste_to(target_pid)?;
    std::thread::sleep(std::time::Duration::from_millis(40));
    Ok(())
}

/// How long the transcript stays on the clipboard before the previous contents
/// are put back. Long enough for any application to service the synthetic ⌘V,
/// short enough that the user's own clipboard is not gone for long.
const CLIPBOARD_RESTORE_DELAY: std::time::Duration = std::time::Duration::from_millis(900);

/// Put `previous` back on the clipboard once the paste has landed.
///
/// Dictating should not cost the user whatever they had copied. Detached so the
/// pipeline is not held up, and it re-reads first so a clipboard the user
/// changed in the meantime is left alone.
fn restore_clipboard_later(previous: Option<String>, injected: String) {
    let Some(previous) = previous else {
        return;
    };
    if previous == injected {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(CLIPBOARD_RESTORE_DELAY);
        match get_clipboard_text() {
            // Only restore if our transcript is still what is on the clipboard.
            Ok(current) if current == injected => {
                if let Err(error) = set_clipboard_text(&previous) {
                    append_inject_log(&format!("clipboard restore failed: {error}"));
                } else {
                    append_inject_log("clipboard restored");
                }
            }
            _ => append_inject_log("clipboard changed since paste — left as is"),
        }
    });
}

/// The focused field's selection, read through Accessibility.
///
/// Used by the `Selection` context tier. Unlike [`capture_selected_text`] it
/// never touches the clipboard, so reading context cannot disturb what the user
/// has copied.
pub fn read_ax_selection() -> Option<String> {
    try_ax_selection().ok().flatten()
}

fn automatic_injection_failed(
    text: &str,
    paste_error: &OtoError,
    ax_error: Option<&OtoError>,
    type_error: &OtoError,
) -> OtoResult<InjectResult> {
    // Preserve the transcript even when every automatic delivery path fails.
    // This is still an error: returning ClipboardOnly here used to make the
    // pipeline announce success although no text reached the target field.
    set_clipboard_text(text)?;

    let message = if accessibility_status() != "trusted" {
        "Accessibility permission is not granted. The transcript was copied — press ⌘V, then enable Oto in System Settings → Privacy & Security → Accessibility and reopen it."
            .to_string()
    } else {
        let ax_detail = ax_error
            .map(|error| format!("; Accessibility insert: {error}"))
            .unwrap_or_default();
        format!(
            "Could not insert into the focused app; the transcript was copied. Paste: {paste_error}{ax_detail}; direct typing: {type_error}"
        )
    };

    Err(OtoError::Message(message))
}

/// Inject `text`, optionally restoring a previously captured focus target first.
pub async fn inject_text_to(
    text: &str,
    mode: &InjectionMode,
    focus: Option<&FocusTarget>,
) -> OtoResult<InjectResult> {
    inject_text_with_options(text, mode, focus, true).await
}

/// As [`inject_text_to`], with control over whether the clipboard is restored.
pub async fn inject_text_with_options(
    text: &str,
    mode: &InjectionMode,
    focus: Option<&FocusTarget>,
    restore_clipboard: bool,
) -> OtoResult<InjectResult> {
    let target_pid = focus.and_then(|f| f.pid);
    append_inject_log(&format!(
        "inject_text mode={mode:?} chars={} focus_before={} ax={} target_pid={:?}",
        text.chars().count(),
        active_focus_summary(),
        accessibility_status(),
        target_pid
    ));

    if let Some(target) = focus {
        let restored = restore_focus_target(target);
        append_inject_log(&format!(
            "restore_focus ok={restored} target={:?} pid={:?}",
            target.name, target.pid
        ));
        // Brave / Electron need a longer settle after activation.
        let wait_ms = if restored { 280 } else { 120 };
        tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
        // Second activation pass — first often only brings the window forward.
        if restored {
            let _ = restore_focus_target(target);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    append_inject_log(&format!("focus_at_type={}", active_focus_summary()));

    if accessibility_status() != "trusted" {
        let _ = ensure_accessibility_prompt();
        append_inject_log(&format!("ax_after_prompt={}", accessibility_status()));
    }

    // Snapshot before anything writes to the clipboard. Clipboard-only mode is
    // excluded on purpose: there the clipboard *is* the delivery mechanism, so
    // putting the old value back would throw the transcript away.
    let previous_clipboard = if restore_clipboard && *mode != InjectionMode::ClipboardOnly {
        get_clipboard_text().ok()
    } else {
        None
    };

    let result = match mode {
        InjectionMode::ClipboardOnly => {
            set_clipboard_text(text)?;
            Ok(InjectResult::ClipboardOnly)
        }
        InjectionMode::DirectType => {
            let _ = set_clipboard_text(text);
            simulate_type_to(text, target_pid)?;
            Ok(InjectResult::DirectTyped)
        }
        InjectionMode::ClipboardPaste => {
            paste_via_clipboard(text, target_pid)?;
            Ok(InjectResult::Pasted)
        }
        InjectionMode::Auto => match paste_via_clipboard(text, target_pid) {
            Ok(()) => {
                append_inject_log("auto: clipboard+paste ok");
                Ok(InjectResult::Pasted)
            }
            Err(paste_error) => {
                append_inject_log(&format!("clipboard+paste failed: {paste_error}"));
                match try_ax_insert(text) {
                    Ok(true) => {
                        append_inject_log("auto: AX insert ok");
                        Ok(InjectResult::Accessibility)
                    }
                    Ok(false) => {
                        append_inject_log("auto: AX insert skipped/not applied");
                        match simulate_type_to(text, target_pid) {
                            Ok(()) => {
                                append_inject_log("auto: direct type ok");
                                Ok(InjectResult::DirectTyped)
                            }
                            Err(type_error) => {
                                append_inject_log(&format!("direct typing failed: {type_error}"));
                                automatic_injection_failed(text, &paste_error, None, &type_error)
                            }
                        }
                    }
                    Err(ax_error) => {
                        append_inject_log(&format!("AX insert error: {ax_error}"));
                        match simulate_type_to(text, target_pid) {
                            Ok(()) => Ok(InjectResult::DirectTyped),
                            Err(type_error) => automatic_injection_failed(
                                text,
                                &paste_error,
                                Some(&ax_error),
                                &type_error,
                            ),
                        }
                    }
                }
            }
        },
    };
    match &result {
        Ok(kind) => {
            append_inject_log(&format!("result={kind:?}"));
            // A fallback to clipboard-only means the user still has to paste it.
            if *kind != InjectResult::ClipboardOnly {
                restore_clipboard_later(previous_clipboard, text.to_string());
            }
        }
        Err(error) => append_inject_log(&format!("error={error}")),
    }
    result
}

pub async fn capture_selected_text() -> OtoResult<String> {
    if let Some(selected) = try_ax_selection()? {
        return Ok(selected);
    }
    let previous = get_clipboard_text().ok();
    let sentinel = format!(
        "__oto_selection_{}__",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    set_clipboard_text(&sentinel)?;
    let pid = capture_focus_target().pid;
    if let Err(error) = simulate_copy_to(pid) {
        if let Some(previous) = previous {
            let _ = set_clipboard_text(&previous);
        }
        return Err(error);
    }
    tokio::time::sleep(std::time::Duration::from_millis(160)).await;
    let selected = get_clipboard_text()?;
    if selected == sentinel || selected.trim().is_empty() {
        if let Some(previous) = previous {
            let _ = set_clipboard_text(&previous);
        }
        return Err(OtoError::Message(
            "No selected text found — select text in the target app first".into(),
        ));
    }
    if let Some(previous) = previous {
        let _ = set_clipboard_text(&previous);
    }
    Ok(selected)
}

pub fn paste_tooling_summary() -> String {
    format!(
        "platform=macos; accessibility={}",
        accessibility_status()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn clipboard_only_mode() {
        let result = inject_text("oto unit", &InjectionMode::ClipboardOnly).await;
        match result {
            Ok(r) => assert_eq!(r, InjectResult::ClipboardOnly),
            Err(e) => {
                let msg = e.to_string().to_lowercase();
                assert!(
                    msg.contains("clipboard")
                        || msg.contains("pasteboard")
                        || msg.contains("not available"),
                    "unexpected error: {e}"
                );
            }
        }
    }

    #[test]
    fn paste_tooling_summary_nonempty() {
        let s = paste_tooling_summary();
        assert!(s.contains("platform=macos"));
    }
}
