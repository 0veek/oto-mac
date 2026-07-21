//! System clipboard write via arboard (NSPasteboard on macOS).

use std::sync::{Mutex, OnceLock};

use crate::error::{OtoError, OtoResult};

static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();

/// Set the system clipboard to `text`.
pub fn set_clipboard_text(text: &str) -> OtoResult<()> {
    let clipboard = CLIPBOARD.get_or_init(|| Mutex::new(None));
    let mut guard = clipboard
        .lock()
        .map_err(|_| OtoError::Message("clipboard lock poisoned".into()))?;
    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new().map_err(|e| OtoError::Message(e.to_string()))?);
    }
    guard
        .as_mut()
        .expect("clipboard initialized above")
        .set_text(text.to_string())
        .map_err(|e| OtoError::Message(e.to_string()))?;
    Ok(())
}

/// Read text through the same long-lived clipboard owner used for writes.
pub fn get_clipboard_text() -> OtoResult<String> {
    let clipboard = CLIPBOARD.get_or_init(|| Mutex::new(None));
    let mut guard = clipboard
        .lock()
        .map_err(|_| OtoError::Message("clipboard lock poisoned".into()))?;
    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new().map_err(|e| OtoError::Message(e.to_string()))?);
    }
    guard
        .as_mut()
        .expect("clipboard initialized above")
        .get_text()
        .map_err(|e| OtoError::Message(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_clipboard_returns_result() {
        let _ = set_clipboard_text("oto clipboard smoke");
    }
}
