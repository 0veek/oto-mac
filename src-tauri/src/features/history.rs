use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{OtoError, OtoResult};

static HISTORY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct HistoryEntry {
    pub id: String,
    pub created_at_ms: u64,
    pub raw_text: String,
    pub final_text: String,
    /// `dictation` or `command`.
    pub mode: String,
    pub language: Option<String>,
    /// A WAV for this entry is retained on disk and can be replayed.
    pub has_audio: bool,
    /// Recording length in milliseconds; zero when unknown.
    pub duration_ms: u64,
}

impl Default for HistoryEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            created_at_ms: 0,
            raw_text: String::new(),
            final_text: String::new(),
            mode: "dictation".into(),
            language: None,
            has_audio: false,
            duration_ms: 0,
        }
    }
}

/// Directory holding retained dictation audio.
pub fn audio_dir() -> OtoResult<PathBuf> {
    let base = directories::ProjectDirs::from("dev", "Oto", "oto")
        .ok_or_else(|| OtoError::Message("could not resolve data dir".into()))?;
    Ok(base.data_local_dir().join("audio"))
}

/// Path of the WAV retained for `id`.
pub fn audio_path(id: &str) -> OtoResult<PathBuf> {
    // Ids are generated internally, but a caller could still pass anything, and
    // a traversal here would let a crafted id read or delete arbitrary files.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(OtoError::Message("invalid history id".into()));
    }
    Ok(audio_dir()?.join(format!("{id}.wav")))
}

/// Store the recording for `id`, best effort.
pub fn save_audio(id: &str, wav: &[u8]) -> OtoResult<()> {
    let path = audio_path(id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, wav)?;
    Ok(())
}

/// Remove audio for entries that are no longer in history.
///
/// Called after every append so a trimmed or deleted entry cannot leave its
/// recording behind — orphaned audio would be an invisible, growing archive of
/// the user's voice.
pub fn prune_audio(entries: &[HistoryEntry]) -> OtoResult<()> {
    let dir = audio_dir()?;
    if !dir.exists() {
        return Ok(());
    }
    let keep: std::collections::HashSet<&str> = entries
        .iter()
        .filter(|entry| entry.has_audio)
        .map(|entry| entry.id.as_str())
        .collect();
    for item in fs::read_dir(&dir)? {
        let Ok(item) = item else { continue };
        let path = item.path();
        if path.extension().and_then(|e| e.to_str()) != Some("wav") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !keep.contains(stem) {
            let _ = fs::remove_file(&path);
        }
    }
    Ok(())
}

fn history_path() -> OtoResult<PathBuf> {
    let base = directories::ProjectDirs::from("dev", "Oto", "oto")
        .ok_or_else(|| OtoError::Message("could not resolve data dir".into()))?;
    Ok(base.data_local_dir().join("history.json"))
}

fn load_from(path: &Path) -> OtoResult<Vec<HistoryEntry>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(vec![]);
    }
    Ok(serde_json::from_str(&raw)?)
}

fn save_to(path: &Path, entries: &[HistoryEntry]) -> OtoResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, serde_json::to_vec_pretty(entries)?)?;
    fs::rename(temp, path)?;
    Ok(())
}

pub fn list() -> OtoResult<Vec<HistoryEntry>> {
    let _guard = HISTORY_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| OtoError::Message("history lock poisoned".into()))?;
    load_from(&history_path()?)
}

#[allow(clippy::too_many_arguments)]
pub fn append(
    raw_text: String,
    final_text: String,
    mode: &str,
    language: Option<String>,
    limit: usize,
    audio: Option<&[u8]>,
    duration_ms: u64,
) -> OtoResult<HistoryEntry> {
    let _guard = HISTORY_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| OtoError::Message("history lock poisoned".into()))?;
    let path = history_path()?;
    let mut entries = load_from(&path)?;
    let created_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    // Include a short unique suffix so two saves in the same millisecond never collide.
    let id = format!(
        "{created_at_ms}-{:x}",
        std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    );
    let mut has_audio = false;
    if let Some(wav) = audio {
        match save_audio(&id, wav) {
            Ok(()) => has_audio = true,
            // Losing the recording must not lose the transcript.
            Err(error) => eprintln!("oto history: could not retain audio: {error}"),
        }
    }
    let entry = HistoryEntry {
        id,
        created_at_ms,
        raw_text,
        final_text,
        mode: mode.into(),
        language,
        has_audio,
        duration_ms,
    };
    entries.insert(0, entry.clone());
    entries.truncate(limit.clamp(1, 1000));
    save_to(&path, &entries)?;
    if let Err(error) = prune_audio(&entries) {
        eprintln!("oto history: could not prune retained audio: {error}");
    }
    Ok(entry)
}

/// One entry by id.
pub fn get(id: &str) -> OtoResult<Option<HistoryEntry>> {
    Ok(list()?.into_iter().find(|entry| entry.id == id))
}

pub fn delete(id: &str) -> OtoResult<()> {
    let _guard = HISTORY_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| OtoError::Message("history lock poisoned".into()))?;
    let path = history_path()?;
    let mut entries = load_from(&path)?;
    entries.retain(|entry| entry.id != id);
    save_to(&path, &entries)?;
    if let Err(error) = prune_audio(&entries) {
        eprintln!("oto history: could not prune retained audio: {error}");
    }
    Ok(())
}

pub fn clear() -> OtoResult<()> {
    let _guard = HISTORY_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| OtoError::Message("history lock poisoned".into()))?;
    let path = history_path()?;
    // Always write an empty list first so readers never see a half-deleted file.
    save_to(&path, &[])?;
    // Best-effort remove: if this fails, list() still returns [] from the empty JSON.
    let _ = fs::remove_file(&path);
    if let Err(error) = prune_audio(&[]) {
        eprintln!("oto history: could not prune retained audio: {error}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_json_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let entries = vec![HistoryEntry {
            id: "one".into(),
            created_at_ms: 1,
            raw_text: "raw".into(),
            final_text: "final".into(),
            mode: "dictation".into(),
            language: Some("en".into()),
            has_audio: false,
            duration_ms: 0,
        }];
        save_to(&path, &entries).unwrap();
        assert_eq!(load_from(&path).unwrap(), entries);
    }

    #[test]
    fn entries_written_before_audio_retention_still_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        fs::write(
            &path,
            r#"[{"id":"old","created_at_ms":1,"raw_text":"r","final_text":"f","mode":"dictation","language":null}]"#,
        )
        .unwrap();
        let entries = load_from(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].has_audio);
        assert_eq!(entries[0].duration_ms, 0);
    }

    #[test]
    fn audio_paths_reject_ids_that_could_escape_the_directory() {
        for bad in ["../../etc/passwd", "a/b", "", "id with space", "id.wav"] {
            assert!(audio_path(bad).is_err(), "{bad} should be rejected");
        }
        assert!(audio_path("1800000000000-abc123").is_ok());
    }

    #[test]
    fn clear_empties_history_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let entries = vec![HistoryEntry {
            id: "one".into(),
            created_at_ms: 1,
            raw_text: "raw".into(),
            final_text: "final".into(),
            mode: "dictation".into(),
            language: None,
            has_audio: false,
            duration_ms: 0,
        }];
        save_to(&path, &entries).unwrap();
        assert_eq!(load_from(&path).unwrap().len(), 1);

        // Mirror clear(): write empty, then remove.
        save_to(&path, &[]).unwrap();
        assert_eq!(load_from(&path).unwrap(), vec![]);
        let _ = fs::remove_file(&path);
        assert_eq!(load_from(&path).unwrap(), vec![]);
    }
}
