//! macOS-specific settings helpers (permissions, Accessibility TCC).

use crate::error::OtoError;
use crate::injection::{
    accessibility_status_detail, ensure_accessibility_prompt, open_accessibility_settings,
    reveal_executable_in_finder, AccessibilityStatus,
};
use crate::permissions::{
    check_all_permissions, open_permission_settings, request_accessibility_access,
    request_microphone_access, PermissionsReport,
};

/// Current Accessibility trust state + how to grant it.
#[tauri::command]
pub fn get_accessibility_status() -> AccessibilityStatus {
    accessibility_status_detail()
}

/// Full macOS permission report (Accessibility, Microphone, Input Monitoring, bundle).
#[tauri::command]
pub fn get_permissions_status() -> PermissionsReport {
    check_all_permissions()
}

/// Open System Settings for a specific permission (`accessibility`, `microphone`, `input_monitoring`).
#[tauri::command]
pub fn open_permission_settings_cmd(id: String) -> Result<(), OtoError> {
    open_permission_settings(&id)
}

/// Open System Settings → Privacy & Security → Accessibility.
#[tauri::command]
pub fn open_accessibility_settings_cmd() -> Result<(), OtoError> {
    open_accessibility_settings()
}

/// Open Accessibility settings and request the system trust dialog once.
#[tauri::command]
pub fn request_accessibility() -> Result<AccessibilityStatus, OtoError> {
    let _ = ensure_accessibility_prompt();
    Ok(accessibility_status_detail())
}

/// Request Accessibility (dialog) and return the full permissions report.
#[tauri::command]
pub fn request_accessibility_permission() -> PermissionsReport {
    request_accessibility_access()
}

/// Soft-request microphone access (triggers TCC prompt when undetermined).
#[tauri::command]
pub fn request_microphone_permission() -> Result<PermissionsReport, OtoError> {
    request_microphone_access()
}

/// Re-run all permission checks and return the report.
#[tauri::command]
pub fn check_permissions() -> PermissionsReport {
    check_all_permissions()
}

/// Reveal the running binary (or app) in Finder for the Accessibility + picker.
#[tauri::command]
pub fn reveal_app_in_finder() -> Result<(), OtoError> {
    reveal_executable_in_finder()
}
