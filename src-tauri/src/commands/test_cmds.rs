use tauri::{AppHandle, Emitter, Manager, State};
use tokio::time::{sleep, Duration};

use crate::audio::{AudioRecorder, CaptureTuning};
use crate::config::{load_config, IdleBehavior};
use crate::error::OtoError;
use crate::injection::{
    capture_focus_target, inject_text_to, paste_tooling_summary, InjectResult,
};
use crate::pipeline::events::{PipelineEvent, PipelineState};
use crate::pipeline::orchestrator::position_overlay;
use crate::state::AppState;

fn show_overlay_for_test(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        position_overlay(&window);
        let _ = window.set_always_on_top(true);
        let _ = window.set_skip_taskbar(true);
        let _ = window.show();
        let _ = window.unminimize();
    }
}

fn hide_overlay_after_test(app: &AppHandle) {
    let keep = load_config()
        .map(|config| config.idle_behavior == IdleBehavior::Minimal)
        .unwrap_or(false);
    if keep {
        return;
    }
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
}

/// Transcribe the last PTT capture with the configured STT provider.
/// Returns an error if no audio has been recorded yet.
#[tauri::command]
pub async fn test_transcription(state: State<'_, AppState>) -> Result<String, OtoError> {
    state.pipeline.transcribe_last().await
}

/// Inject a fixed sample string using the configured injection mode.
/// After calling, focus a target app during the short delay below.
#[tauri::command]
pub async fn test_injection() -> Result<String, OtoError> {
    let cfg = load_config()?;
    let sample = "Oto injection test";
    // Clicking the settings button focuses Oto. Give the user a moment to
    // return focus to the target app before simulating ⌘V.
    sleep(Duration::from_millis(1500)).await;
    // Capture whichever app the user focused during the delay — not Oto Settings.
    let focus = capture_focus_target();
    let result = inject_text_to(sample, &cfg.injection_mode, Some(&focus)).await?;
    let tooling = paste_tooling_summary();
    let target = focus
        .name
        .clone()
        .or(focus.class.clone())
        .unwrap_or_else(|| "unknown-app".into());
    let msg = match result {
        InjectResult::Accessibility => {
            format!("Injected via Accessibility into {target} ({tooling})")
        }
        InjectResult::DirectTyped => {
            format!("Typed through synthetic key events into {target} ({tooling})")
        }
        InjectResult::Pasted => {
            format!("Pasted via clipboard + ⌘V into {target} ({tooling})")
        }
        InjectResult::ClipboardOnly => {
            format!("Copied — press ⌘V ({tooling})")
        }
    };
    Ok(msg)
}

/// Capture ~2s of microphone audio and stream level events (no STT).
#[tauri::command]
pub async fn test_microphone(app: AppHandle, state: State<'_, AppState>) -> Result<(), OtoError> {
    if !state.pipeline.is_idle() {
        return Err(OtoError::Message(
            "Finish or cancel the current dictation before testing the microphone".into(),
        ));
    }

    show_overlay_for_test(&app);
    let _ = app.emit(
        "pipeline://event",
        PipelineEvent::state(PipelineState::Listening, Some("Mic test".into())),
    );

    // Exercise the user's real device, gain, and gate settings — a mic test that
    // bypassed them would pass while dictation stayed silent.
    let tuning = load_config()
        .map(|cfg| CaptureTuning::from_config(&cfg))
        .unwrap_or_default();

    let recorder = match AudioRecorder::start(app.clone(), tuning) {
        Ok(r) => r,
        Err(e) => {
            let _ = app.emit(
                "pipeline://event",
                PipelineEvent::Error {
                    message: e.to_string(),
                },
            );
            show_overlay_for_test(&app);
            // Keep the error visible briefly, then return to idle appearance.
            sleep(Duration::from_secs(3)).await;
            let _ = app.emit(
                "pipeline://event",
                PipelineEvent::state(PipelineState::Idle, None),
            );
            hide_overlay_after_test(&app);
            return Err(e);
        }
    };

    sleep(Duration::from_secs(2)).await;

    // Drop stream (levels already streamed during capture).
    let _ = recorder.stop();

    let _ = app.emit(
        "pipeline://event",
        PipelineEvent::state(PipelineState::Idle, None),
    );
    hide_overlay_after_test(&app);
    Ok(())
}
