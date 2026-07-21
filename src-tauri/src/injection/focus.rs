//! Save/restore the frontmost application so injection hits the dictation target.

use objc2::rc::Retained;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

/// Snapshot of the application that should receive injected text.
#[derive(Debug, Clone, Default)]
pub struct FocusTarget {
    pub pid: Option<i32>,
    pub name: Option<String>,
    pub class: Option<String>,
}

fn frontmost_app() -> Option<Retained<NSRunningApplication>> {
    let workspace = NSWorkspace::sharedWorkspace();
    workspace.frontmostApplication()
}

fn is_oto_app(app: &NSRunningApplication) -> bool {
    let name = app.localizedName().map(|s| s.to_string()).unwrap_or_default();
    let bundle = app
        .bundleIdentifier()
        .map(|s| s.to_string())
        .unwrap_or_default();
    name.starts_with("Oto")
        || bundle.eq_ignore_ascii_case("dev.oto.mac")
        || bundle.eq_ignore_ascii_case("dev.oto.app")
        || bundle.to_ascii_lowercase().contains("oto")
}

/// Capture the frontmost non-Oto application.
pub fn capture_focus_target() -> FocusTarget {
    let Some(app) = frontmost_app() else {
        return FocusTarget::default();
    };
    if is_oto_app(&app) {
        if let Some(previous) = previous_non_oto_app() {
            return target_from_app(&previous);
        }
        return FocusTarget::default();
    }
    target_from_app(&app)
}

fn previous_non_oto_app() -> Option<Retained<NSRunningApplication>> {
    let workspace = NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();
    for app in apps.iter() {
        if app.isActive() {
            continue;
        }
        if is_oto_app(&app) {
            continue;
        }
        if app.activationPolicy() != objc2_app_kit::NSApplicationActivationPolicy::Regular {
            continue;
        }
        return Some(app);
    }
    None
}

fn target_from_app(app: &NSRunningApplication) -> FocusTarget {
    let name = app.localizedName().map(|s| s.to_string());
    let pid = Some(app.processIdentifier());
    FocusTarget {
        pid,
        name: name.clone(),
        class: name,
    }
}

/// Restore focus to a previously captured target. Returns true if activation ran.
pub fn restore_focus_target(target: &FocusTarget) -> bool {
    let Some(pid) = target.pid else {
        return false;
    };
    let workspace = NSWorkspace::sharedWorkspace();
    let apps = workspace.runningApplications();
    for app in apps.iter() {
        if app.processIdentifier() != pid {
            continue;
        }
        // Unhide if needed, then activate as the active app.
        if app.isHidden() {
            let _ = app.unhide();
        }
        // macOS 14+ deprecates ignoringOtherApps but it still helps activate
        // a background app as the key application for keystrokes.
        #[allow(deprecated)]
        let activated = app.activateWithOptions(
            NSApplicationActivationOptions::ActivateIgnoringOtherApps,
        );
        return activated || app.isActive();
    }
    false
}

/// Log-friendly summary of the currently frontmost app.
pub fn active_focus_summary() -> String {
    let Some(app) = frontmost_app() else {
        return "unknown".into();
    };
    let name = app
        .localizedName()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "?".into());
    let bundle = app
        .bundleIdentifier()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "?".into());
    let pid = app.processIdentifier();
    format!("{name} | {bundle} | pid={pid}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_focus_summary_nonempty() {
        let s = active_focus_summary();
        assert!(!s.is_empty());
    }
}
