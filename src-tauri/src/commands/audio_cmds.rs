//! Transcribing audio that was not just recorded.

use std::path::Path;

use crate::config::load_config;
use crate::error::{OtoError, OtoResult};
use crate::features::{history, replacements, voice_edits};

/// Upload limit shared by the transcription APIs Oto talks to.
const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// Container formats the supported providers accept directly.
const SUPPORTED: &[&str] = &[
    "wav", "mp3", "m4a", "mp4", "mpeg", "mpga", "webm", "ogg", "oga", "flac", "aac",
];

fn check_extension(path: &Path) -> OtoResult<()> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if SUPPORTED.contains(&extension.as_str()) {
        return Ok(());
    }
    Err(OtoError::Message(format!(
        "unsupported audio format '{extension}' — try one of: {}",
        SUPPORTED.join(", ")
    )))
}

/// Transcribe an audio file from disk using the current settings.
///
/// Runs the same post-processing as dictation (spoken edits, replacements) so a
/// recording and a live dictation produce the same text, then records it in
/// history. It deliberately does not inject: the user is at the settings
/// window, not in the target application.
#[tauri::command]
pub async fn transcribe_audio_file(path: String) -> Result<String, OtoError> {
    let path = Path::new(path.trim());
    if !path.is_file() {
        return Err(OtoError::Message(format!(
            "no such file: {}",
            path.display()
        )));
    }
    check_extension(path)?;

    let bytes = std::fs::read(path)
        .map_err(|error| OtoError::Message(format!("could not read the file: {error}")))?;
    if bytes.is_empty() {
        return Err(OtoError::Message("that file is empty".into()));
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(OtoError::Message(format!(
            "that file is {:.1} MB; providers accept up to {} MB. Split it or transcribe with a local model.",
            bytes.len() as f64 / (1024.0 * 1024.0),
            MAX_UPLOAD_BYTES / (1024 * 1024)
        )));
    }

    let cfg = load_config()?;
    let mut text = crate::pipeline::orchestrator::transcribe_wav(&bytes).await?;
    if text.trim().is_empty() {
        return Err(OtoError::Message("no speech detected in that file".into()));
    }

    let raw = text.clone();
    if cfg.voice_edits_enabled {
        text = voice_edits::apply_voice_edits(&text);
    }
    text = replacements::apply_replacements(&text, &cfg.replacements);

    if cfg.history_enabled {
        if let Err(error) = history::append(
            raw,
            text.clone(),
            "file",
            cfg.language.clone(),
            cfg.history_limit,
            None,
            0,
        ) {
            eprintln!("oto: could not save imported transcript to history: {error}");
        }
    }

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_audio_containers_are_accepted() {
        for name in ["a.wav", "a.MP3", "recording.m4a", "voice.ogg", "x.flac"] {
            assert!(check_extension(Path::new(name)).is_ok(), "{name} rejected");
        }
    }

    #[test]
    fn non_audio_files_are_rejected_with_a_useful_message() {
        let error = check_extension(Path::new("notes.txt")).unwrap_err().to_string();
        assert!(error.contains("unsupported audio format"));
        assert!(error.contains("wav"));
        // An extensionless file must not slip through as the empty extension.
        assert!(check_extension(Path::new("recording")).is_err());
    }
}
