//! macOS permission probes (Accessibility, Microphone, Input Monitoring).

use std::process::Command;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use objc2::msg_send;
use objc2::runtime::AnyClass;
use objc2_foundation::NSString;
use serde::Serialize;

use crate::error::{OtoError, OtoResult};
use crate::injection::{
    accessibility_status_detail, display_name, executable_path, is_bundled_app, is_process_trusted,
};

/// One permission row for the Settings UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionItem {
    pub id: String,
    pub name: String,
    pub required: bool,
    /// granted | denied | not_determined | restricted | unknown | recommended
    pub status: String,
    pub detail: String,
    pub can_open_settings: bool,
}

/// Full permission report.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsReport {
    pub all_required_granted: bool,
    pub display_name: String,
    pub executable_path: String,
    pub bundled: bool,
    pub checked_at_ms: u128,
    pub items: Vec<PermissionItem>,
    pub summary: String,
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// AVAuthorizationStatus values.
const AV_AUTH_NOT_DETERMINED: i64 = 0;
const AV_AUTH_RESTRICTED: i64 = 1;
const AV_AUTH_DENIED: i64 = 2;
const AV_AUTH_AUTHORIZED: i64 = 3;

fn microphone_status_code() -> Option<i64> {
    // AVCaptureDevice.authorizationStatus(for: .audio)
    // AVMediaTypeAudio is the string "soun"
    let cls = AnyClass::get(c"AVCaptureDevice")?;
    let media = NSString::from_str("soun");
    let status: i64 = unsafe { msg_send![cls, authorizationStatusForMediaType: &*media] };
    Some(status)
}

fn microphone_item() -> PermissionItem {
    let (status, detail) = match microphone_status_code() {
        Some(AV_AUTH_AUTHORIZED) => (
            "granted".into(),
            "Microphone access is granted for dictation.".into(),
        ),
        Some(AV_AUTH_DENIED) => (
            "denied".into(),
            "Microphone is denied. Enable Oto under System Settings → Privacy & Security → Microphone."
                .into(),
        ),
        Some(AV_AUTH_RESTRICTED) => (
            "restricted".into(),
            "Microphone is restricted by system policy (Screen Time / MDM).".into(),
        ),
        Some(AV_AUTH_NOT_DETERMINED) => (
            "not_determined".into(),
            "Microphone has not been requested yet. Click “Request microphone” or use Test microphone."
                .into(),
        ),
        Some(other) => (
            "unknown".into(),
            format!("Unexpected microphone status code: {other}"),
        ),
        None => (
            "unknown".into(),
            "Could not query AVCaptureDevice (AVFoundation unavailable).".into(),
        ),
    };
    PermissionItem {
        id: "microphone".into(),
        name: "Microphone".into(),
        required: true,
        status,
        detail,
        can_open_settings: true,
    }
}

fn accessibility_item() -> PermissionItem {
    let ax = accessibility_status_detail();
    let (status, detail) = if ax.trusted {
        (
            "granted".into(),
            "Accessibility is granted — Oto can paste (⌘V) and insert text.".into(),
        )
    } else {
        (
            "denied".into(),
            format!(
                "Accessibility is not granted. Unlock Accessibility, click +, add “{}”, enable the toggle, then restart Oto.\nPath: {}",
                ax.display_name, ax.executable_path
            ),
        )
    };
    PermissionItem {
        id: "accessibility".into(),
        name: "Accessibility".into(),
        required: true,
        status,
        detail,
        can_open_settings: true,
    }
}

/// Input Monitoring (listen to keyboard events) — optional.
fn input_monitoring_item() -> PermissionItem {
    let granted = unsafe { CGPreflightListenEventAccess() };
    let (status, detail) = if granted {
        (
            "granted".into(),
            "Input Monitoring is granted (optional; helps some global-shortcut backends)."
                .into(),
        )
    } else {
        (
            "not_determined".into(),
            "Input Monitoring is not granted. Usually optional; enable it if the hotkey never fires."
                .into(),
        )
    };
    PermissionItem {
        id: "input_monitoring".into(),
        name: "Input Monitoring".into(),
        required: false,
        status,
        detail,
        can_open_settings: true,
    }
}

fn bundle_item() -> PermissionItem {
    let bundled = is_bundled_app();
    if bundled {
        PermissionItem {
            id: "app_bundle".into(),
            name: "App bundle".into(),
            required: false,
            status: "granted".into(),
            detail: "Running as Oto.app — best for stable Accessibility entries.".into(),
            can_open_settings: false,
        }
    } else {
        PermissionItem {
            id: "app_bundle".into(),
            name: "App bundle".into(),
            required: false,
            status: "recommended".into(),
            detail: format!(
                "Running a development binary ({}). Prefer Oto.app so macOS lists permissions under “Oto”.",
                executable_path().unwrap_or_else(|| "unknown".into())
            ),
            can_open_settings: false,
        }
    }
}

/// Collect every permission Oto cares about.
pub fn check_all_permissions() -> PermissionsReport {
    let items = vec![
        accessibility_item(),
        microphone_item(),
        input_monitoring_item(),
        bundle_item(),
    ];
    let all_required_granted = items
        .iter()
        .filter(|i| i.required)
        .all(|i| i.status == "granted");
    let missing: Vec<&str> = items
        .iter()
        .filter(|i| i.required && i.status != "granted")
        .map(|i| i.name.as_str())
        .collect();
    let summary = if all_required_granted {
        "All required permissions are granted.".into()
    } else {
        format!(
            "Missing required: {}. Open each item’s settings and enable Oto, then re-check.",
            missing.join(", ")
        )
    };
    PermissionsReport {
        all_required_granted,
        display_name: display_name(),
        executable_path: executable_path().unwrap_or_default(),
        bundled: is_bundled_app(),
        checked_at_ms: now_ms(),
        items,
        summary,
    }
}

fn open_url(url: &str) -> OtoResult<()> {
    let status = Command::new("open")
        .arg(url)
        .status()
        .map_err(|e| OtoError::Message(format!("open failed: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(OtoError::Message(format!("open exited with {status}")))
    }
}

/// Open the System Settings pane for a permission id.
pub fn open_permission_settings(id: &str) -> OtoResult<()> {
    let urls: &[&str] = match id {
        "accessibility" => &[
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Accessibility",
        ],
        "microphone" => &[
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
            "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Microphone",
        ],
        "input_monitoring" => &[
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_ListenEvent",
        ],
        _ => {
            return Err(OtoError::Message(format!(
                "no settings pane for permission id: {id}"
            )));
        }
    };
    let mut last = None;
    for url in urls {
        match open_url(url) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e.to_string()),
        }
    }
    Err(OtoError::Message(
        last.unwrap_or_else(|| "open failed".into()),
    ))
}

/// Soft-request microphone access by briefly opening the default input device
/// (triggers the TCC dialog when status is not determined).
pub fn request_microphone_access() -> OtoResult<PermissionsReport> {
    match microphone_status_code() {
        Some(AV_AUTH_AUTHORIZED) => return Ok(check_all_permissions()),
        Some(AV_AUTH_DENIED) | Some(AV_AUTH_RESTRICTED) => {
            let _ = open_permission_settings("microphone");
            return Ok(check_all_permissions());
        }
        _ => {}
    }

    // Opening a short input stream triggers the system microphone prompt.
    let host = cpal::default_host();
    if let Some(dev) = host.default_input_device() {
        if let Ok(supported) = dev.default_input_config() {
            let stream_config: cpal::StreamConfig = supported.into();
            let stream = dev.build_input_stream(
                stream_config,
                |_data: &[f32], _| {},
                |_err| {},
                None,
            );
            if let Ok(stream) = stream {
                let _ = stream.play();
                std::thread::sleep(std::time::Duration::from_millis(500));
                drop(stream);
            }
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(250));
    Ok(check_all_permissions())
}

/// Request Accessibility trust dialog (if not already trusted).
pub fn request_accessibility_access() -> PermissionsReport {
    let _ = is_process_trusted(true);
    if !is_process_trusted(false) {
        let _ = open_permission_settings("accessibility");
    }
    check_all_permissions()
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
}

// Ensure AVFoundation is linked for AVCaptureDevice.
#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}
