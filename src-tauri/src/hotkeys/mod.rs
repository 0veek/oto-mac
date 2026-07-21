//! Global push-to-talk hotkeys for macOS via tauri-plugin-global-shortcut.

use crate::error::{OtoError, OtoResult};
use crate::state::AppState;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Shared hotkey state. The event gate preserves press/release ordering even
/// though recording work runs asynchronously.
pub struct HotkeyManager {
    pressed: AtomicBool,
    event_gate: Arc<tokio::sync::Mutex<()>>,
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self {
            pressed: AtomicBool::new(false),
            event_gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

/// Normalize user input like `cmd + shift + space` → `Super+Shift+Space`.
pub fn normalize_hotkey(s: &str) -> String {
    s.split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|part| -> String {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => "Ctrl".into(),
                "alt" | "option" => "Alt".into(),
                "shift" => "Shift".into(),
                "super" | "meta" | "win" | "cmd" | "command" => "Super".into(),
                "space" => "Space".into(),
                "enter" | "return" => "Enter".into(),
                "tab" => "Tab".into(),
                "escape" | "esc" => "Escape".into(),
                other if other.len() == 1 => other.to_ascii_uppercase(),
                other => {
                    let mut c = other.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Parse a human-readable hotkey string like `Ctrl+Shift+Space` into a [`Shortcut`].
pub fn parse_hotkey(s: &str) -> OtoResult<Shortcut> {
    let mut mods = Modifiers::empty();
    let mut key: Option<Code> = None;

    for part in s.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" | "option" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            "super" | "meta" | "win" | "cmd" | "command" => mods |= Modifiers::SUPER,
            "space" => set_hotkey_key(&mut key, Code::Space, s)?,
            "enter" | "return" => set_hotkey_key(&mut key, Code::Enter, s)?,
            "tab" => set_hotkey_key(&mut key, Code::Tab, s)?,
            "escape" | "esc" => set_hotkey_key(&mut key, Code::Escape, s)?,
            other if other.len() == 1 => {
                let c = other.chars().next().unwrap();
                let code = match c {
                    'a' => Code::KeyA,
                    'b' => Code::KeyB,
                    'c' => Code::KeyC,
                    'd' => Code::KeyD,
                    'e' => Code::KeyE,
                    'f' => Code::KeyF,
                    'g' => Code::KeyG,
                    'h' => Code::KeyH,
                    'i' => Code::KeyI,
                    'j' => Code::KeyJ,
                    'k' => Code::KeyK,
                    'l' => Code::KeyL,
                    'm' => Code::KeyM,
                    'n' => Code::KeyN,
                    'o' => Code::KeyO,
                    'p' => Code::KeyP,
                    'q' => Code::KeyQ,
                    'r' => Code::KeyR,
                    's' => Code::KeyS,
                    't' => Code::KeyT,
                    'u' => Code::KeyU,
                    'v' => Code::KeyV,
                    'w' => Code::KeyW,
                    'x' => Code::KeyX,
                    'y' => Code::KeyY,
                    'z' => Code::KeyZ,
                    _ => {
                        return Err(OtoError::Message(format!(
                            "unsupported key in hotkey: {part}"
                        )));
                    }
                };
                set_hotkey_key(&mut key, code, s)?;
            }
            other => {
                return Err(OtoError::Message(format!(
                    "unsupported hotkey token: {other}"
                )));
            }
        }
    }

    let key = key.ok_or_else(|| OtoError::Message(format!("no key in hotkey: {s}")))?;
    Ok(Shortcut::new(Some(mods), key))
}

fn set_hotkey_key(slot: &mut Option<Code>, key: Code, hotkey: &str) -> OtoResult<()> {
    if slot.replace(key).is_some() {
        return Err(OtoError::Message(format!(
            "hotkey must contain exactly one non-modifier key: {hotkey}"
        )));
    }
    Ok(())
}

/// Unregister all global shortcuts (no-op if none are registered).
pub fn unregister_all_hotkeys<R: Runtime>(app: &AppHandle<R>) -> OtoResult<()> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| OtoError::Message(e.to_string()))
}

fn dispatch_ptt_event<R: Runtime>(app: &AppHandle<R>, event: ShortcutState) {
    let Some(hotkey_state) = app.try_state::<HotkeyManager>() else {
        eprintln!("oto hotkey: HotkeyManager missing");
        return;
    };

    // Native backends can repeat Pressed while a key is held. Only accept real
    // up/down transitions so repeat events cannot immediately stop recording.
    let should_dispatch = match event {
        ShortcutState::Pressed => !hotkey_state.pressed.swap(true, Ordering::SeqCst),
        ShortcutState::Released => hotkey_state.pressed.swap(false, Ordering::SeqCst),
    };
    if !should_dispatch {
        return;
    }

    let Some(state) = app.try_state::<AppState>() else {
        eprintln!("oto hotkey: AppState missing");
        return;
    };
    let pipeline = state.pipeline.clone();
    let gate = hotkey_state.event_gate.clone();
    tauri::async_runtime::spawn(async move {
        let _guard = gate.lock().await;
        let result = match event {
            ShortcutState::Pressed => {
                eprintln!("oto hotkey: Pressed → ptt_down");
                pipeline.ptt_down().await
            }
            ShortcutState::Released => {
                eprintln!("oto hotkey: Released → ptt_up");
                pipeline.ptt_up().await
            }
        };
        if let Err(error) = result {
            eprintln!("oto hotkey pipeline event failed: {error}");
        }
    });
}

/// Register the push-to-talk hotkey via the native macOS global-shortcut backend.
pub async fn register_ptt<R: Runtime>(app: &AppHandle<R>, hotkey: &str) -> OtoResult<()> {
    let normalized = normalize_hotkey(hotkey);
    parse_hotkey(&normalized)?;
    register_native_ptt(app, &normalized)
}

fn register_native_ptt<R: Runtime>(app: &AppHandle<R>, normalized: &str) -> OtoResult<()> {
    if let Some(state) = app.try_state::<HotkeyManager>() {
        state.pressed.store(false, Ordering::SeqCst);
    }
    let shortcut = parse_hotkey(normalized)?;
    let shortcut_for_check = parse_hotkey(normalized)?;

    // Best-effort clear so changing the binding does not leave stale shortcuts.
    let _ = unregister_all_hotkeys(app);

    app.global_shortcut()
        .on_shortcut(shortcut, |app, sc, event| {
            eprintln!(
                "oto hotkey event: {:?} state={:?} id={}",
                sc,
                event.state(),
                sc.id()
            );
            dispatch_ptt_event(app, event.state());
        })
        .map_err(|e| OtoError::Message(format!("failed to register hotkey '{normalized}': {e}")))?;

    if app.global_shortcut().is_registered(shortcut_for_check) {
        eprintln!("hotkey registered and active: {normalized}");
    } else {
        eprintln!(
            "hotkey register returned OK but is_registered=false for {normalized} \
             (macOS may require Accessibility / Input Monitoring — use tray Start/Stop)"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_hotkey() {
        let sc = parse_hotkey("Ctrl+Shift+Space").unwrap();
        assert_eq!(sc.key, Code::Space);
        assert!(sc.mods.contains(Modifiers::CONTROL));
        assert!(sc.mods.contains(Modifiers::SHIFT));
    }

    #[test]
    fn parses_cmd_as_super() {
        let sc = parse_hotkey("Cmd+Shift+D").unwrap();
        assert_eq!(sc.key, Code::KeyD);
        assert!(sc.mods.contains(Modifiers::SUPER));
        assert!(sc.mods.contains(Modifiers::SHIFT));
    }

    #[test]
    fn normalize_hotkey_formats() {
        assert_eq!(normalize_hotkey("cmd + shift + space"), "Super+Shift+Space");
        assert_eq!(normalize_hotkey("CTRL+ALT+SPACE"), "Ctrl+Alt+Space");
        assert_eq!(normalize_hotkey("option+d"), "Alt+D");
    }

    #[test]
    fn parses_letter_with_modifiers() {
        let sc = parse_hotkey("Alt+Shift+A").unwrap();
        assert_eq!(sc.key, Code::KeyA);
        assert!(sc.mods.contains(Modifiers::ALT));
        assert!(sc.mods.contains(Modifiers::SHIFT));
    }

    #[test]
    fn rejects_empty_and_unknown() {
        assert!(parse_hotkey("").is_err());
        assert!(parse_hotkey("Ctrl+F1").is_err());
        assert!(parse_hotkey("Ctrl").is_err());
        assert!(parse_hotkey("Ctrl+A+B").is_err());
    }
}
