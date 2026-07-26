use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use crate::config::load_config;
use crate::error::OtoError;
use crate::features::history::{self, HistoryEntry};
use crate::features::stats::{self, UsageStats};
use crate::injection::{capture_focus_target, inject_text_to, set_clipboard_text};
use crate::state::AppState;

#[tauri::command]
pub fn get_history() -> Result<Vec<HistoryEntry>, OtoError> {
    history::list()
}

#[tauri::command]
pub fn delete_history_entry(id: String) -> Result<(), OtoError> {
    history::delete(&id)
}

#[tauri::command]
pub fn clear_history() -> Result<(), OtoError> {
    history::clear()
}

#[tauri::command]
pub fn copy_history_text(text: String) -> Result<(), OtoError> {
    set_clipboard_text(&text)
}

/// Usage statistics derived from local history.
#[tauri::command]
pub fn get_usage_stats() -> Result<UsageStats, OtoError> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    Ok(stats::compute(&history::list()?, now_ms))
}

/// Retained recording for an entry, as a base64 data URL the webview can play.
///
/// Returned inline rather than as a path because the webview's asset protocol
/// does not reach Oto's data directory, and widening that scope for playback
/// would expose far more than these few WAVs.
#[tauri::command]
pub fn get_history_audio(id: String) -> Result<String, OtoError> {
    let entry = history::get(&id)?
        .ok_or_else(|| OtoError::Message("that history entry no longer exists".into()))?;
    if !entry.has_audio {
        return Err(OtoError::Message("no audio was kept for this entry".into()));
    }
    let path = history::audio_path(&id)?;
    let bytes = std::fs::read(&path).map_err(|error| {
        OtoError::Message(format!("could not read the retained recording: {error}"))
    })?;
    Ok(format!("data:audio/wav;base64,{}", base64_encode(&bytes)))
}

/// Transcribe an entry's retained audio again with the current settings.
///
/// Useful after changing model, language, or vocabulary: the same recording can
/// be run through the new configuration without speaking it again.
#[tauri::command]
pub async fn retranscribe_history(id: String) -> Result<String, OtoError> {
    let entry = history::get(&id)?
        .ok_or_else(|| OtoError::Message("that history entry no longer exists".into()))?;
    if !entry.has_audio {
        return Err(OtoError::Message(
            "no audio was kept for this entry — enable \"Keep dictation audio\" under Privacy to re-transcribe future dictations".into(),
        ));
    }
    let path = history::audio_path(&id)?;
    let wav = std::fs::read(&path).map_err(|error| {
        OtoError::Message(format!("could not read the retained recording: {error}"))
    })?;
    crate::pipeline::orchestrator::transcribe_wav(&wav).await
}

/// Insert a past transcript into the focused application.
#[tauri::command]
pub async fn reinsert_history(
    state: State<'_, AppState>,
    text: String,
    focus_delay_ms: Option<u64>,
) -> Result<(), OtoError> {
    if text.trim().is_empty() {
        return Err(OtoError::Message("nothing to insert".into()));
    }
    // Reject while a dictation is running so two writers cannot race for the
    // same text field.
    if !state.pipeline.is_idle() {
        return Err(OtoError::Message(
            "Finish or cancel the current dictation first".into(),
        ));
    }
    // Give the user time to click back into the target window.
    let delay = focus_delay_ms.unwrap_or(1_200).min(5_000);
    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

    let cfg = load_config()?;
    let target = capture_focus_target();
    inject_text_to(&text, &cfg.injection_mode, Some(&target)).await?;
    Ok(())
}

/// Minimal base64 encoder — avoids a dependency for one call site.
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) as usize & 63] as char);
        out.push(TABLE[(triple >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(triple >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[triple as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_rfc_test_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_handles_high_bytes_a_wav_header_contains() {
        assert_eq!(base64_encode(&[0xFF, 0xFE, 0xFD]), "//79");
        assert_eq!(base64_encode(&[0x00, 0x00, 0x00]), "AAAA");
    }
}
