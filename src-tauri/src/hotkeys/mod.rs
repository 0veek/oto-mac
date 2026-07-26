//! Global push-to-talk hotkeys for macOS via tauri-plugin-global-shortcut.

pub mod binding;

use binding::{desired_bindings, Binding, PRIMARY_ID};

use crate::config::load_config;
use crate::error::{OtoError, OtoResult};
use crate::state::AppState;
use std::collections::HashSet;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Shared hotkey state. The event gate preserves press/release ordering even
/// though recording work runs asynchronously.
pub struct HotkeyManager {
    /// Shortcut ids currently held down. Per-id rather than one flag so a Mode
    /// chord and the primary chord cannot clobber each other's latch.
    pressed: std::sync::Mutex<HashSet<String>>,
    event_gate: Arc<tokio::sync::Mutex<()>>,
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self {
            pressed: std::sync::Mutex::new(HashSet::new()),
            event_gate: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

impl HotkeyManager {
    /// Set or clear the latch for `id`, returning its previous value.
    fn swap_pressed(&self, id: &str, value: bool) -> bool {
        let Ok(mut held) = self.pressed.lock() else {
            return false;
        };
        if value {
            !held.insert(id.to_string())
        } else {
            held.remove(id)
        }
    }

    fn clear_pressed(&self) {
        if let Ok(mut held) = self.pressed.lock() {
            held.clear();
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
    dispatch_shortcut_event(app, event, PRIMARY_ID, None);
}

/// Route one press/release for `shortcut_id` into the pipeline.
///
/// `mode_id` is `Some` when the chord belongs to a Mode, which starts dictation
/// under that Mode regardless of which application is focused.
fn dispatch_shortcut_event<R: Runtime>(
    app: &AppHandle<R>,
    event: ShortcutState,
    shortcut_id: &str,
    mode_id: Option<String>,
) {
    let Some(hotkey_state) = app.try_state::<HotkeyManager>() else {
        eprintln!("oto hotkey: HotkeyManager missing");
        return;
    };

    let Some(state) = app.try_state::<AppState>() else {
        eprintln!("oto hotkey: AppState missing");
        return;
    };
    let pipeline = state.pipeline.clone();
    let idle = pipeline.is_idle();
    let listening = pipeline.is_listening();

    // Native backends can repeat Pressed while a key is held. Only accept real
    // up/down transitions so repeat events cannot immediately stop recording.
    let should_dispatch = match event {
        ShortcutState::Pressed => {
            let was_pressed = hotkey_state.swap_pressed(shortcut_id, true);
            // Accept a press even when the latch is already set if nothing is
            // running: that means the release never arrived (macOS dropped it, or
            // the take was ended from the tray), and the stale latch would
            // otherwise swallow every later press for the rest of the session.
            //
            // A hands-free session is also stopped by a press, and its release
            // already cleared the latch, so `listening` has to open the gate too.
            !was_pressed || idle || listening
        }
        ShortcutState::Released => hotkey_state.swap_pressed(shortcut_id, false),
    };
    if !should_dispatch {
        return;
    }

    // A press that arrives while a previous take is still *transcribing* must be
    // dropped here, not queued on the gate: the gate is held for the whole of
    // `hotkey_up`, so a queued press would start recording *after* processing
    // ends — with its own release already consumed — producing a phantom
    // capture. The matching release is harmless (`hotkey_up` no-ops unless
    // Listening).
    //
    // Listening is deliberately excluded: in Toggle and Hybrid modes that press
    // is how the user ends the session.
    if event == ShortcutState::Pressed && !idle && !listening {
        eprintln!("oto hotkey: Pressed ignored — previous dictation still running");
        return;
    }

    let gate = hotkey_state.event_gate.clone();
    tauri::async_runtime::spawn(async move {
        let _guard = gate.lock().await;
        let result = match event {
            ShortcutState::Pressed => {
                eprintln!("oto hotkey: Pressed → hotkey_down (mode={mode_id:?})");
                pipeline.hotkey_down_for(mode_id).await
            }
            ShortcutState::Released => {
                eprintln!("oto hotkey: Released → hotkey_up");
                pipeline.hotkey_up().await
            }
        };
        if let Err(error) = result {
            eprintln!("oto hotkey pipeline event failed: {error}");
        }
    });
}

/// Register the push-to-talk hotkey, plus one chord per Mode that asks for one.
pub async fn register_ptt<R: Runtime>(app: &AppHandle<R>, hotkey: &str) -> OtoResult<()> {
    let normalized = normalize_hotkey(hotkey);
    // Validate the primary chord first so an unusable hotkey is rejected without
    // disturbing a working registration.
    parse_hotkey(&normalized)?;

    // Mode chords come from the stored config; the primary chord is whatever the
    // caller is trying to save, which may not be on disk yet.
    let mut bindings = match load_config() {
        Ok(mut cfg) => {
            cfg.hotkey = normalized.clone();
            desired_bindings(&cfg, |h| normalize_hotkey(h))
        }
        Err(_) => vec![Binding::primary(normalized.clone())],
    };
    // An unparseable Mode chord is dropped rather than failing the save: it must
    // never cost the user their working dictation key.
    bindings.retain(|b| {
        if parse_hotkey(&b.hotkey).is_ok() {
            return true;
        }
        eprintln!(
            "oto hotkey: ignoring unparseable chord '{}' for {}",
            b.hotkey, b.id
        );
        false
    });

    register_native_bindings(app, &bindings)
}

fn register_native_bindings<R: Runtime>(app: &AppHandle<R>, bindings: &[Binding]) -> OtoResult<()> {
    if let Some(state) = app.try_state::<HotkeyManager>() {
        state.clear_pressed();
    }
    let Some(primary) = bindings.iter().find(|b| b.id == PRIMARY_ID) else {
        return Err(OtoError::Message("no primary hotkey to register".into()));
    };

    // Best-effort clear so changing a binding does not leave stale shortcuts.
    // If the new bind fails, re-apply the previously saved hotkey so dictation
    // is not left completely unbound for the rest of the session.
    let previous_hotkey = load_config()
        .ok()
        .map(|cfg| normalize_hotkey(&cfg.hotkey))
        .filter(|h| h != &primary.hotkey);
    let _ = unregister_all_hotkeys(app);

    for entry in bindings {
        let shortcut = parse_hotkey(&entry.hotkey)?;
        let shortcut_id = entry.id.clone();
        let mode_id = entry.mode_id.clone();
        let result = app
            .global_shortcut()
            .on_shortcut(shortcut, move |app, sc, event| {
                eprintln!(
                    "oto hotkey event: {:?} state={:?} id={} shortcut={shortcut_id}",
                    sc,
                    event.state(),
                    sc.id()
                );
                dispatch_shortcut_event(app, event.state(), &shortcut_id, mode_id.clone());
            });

        if let Err(error) = result {
            // A Mode chord that will not bind is reported but never fatal —
            // ordinary dictation keeps working.
            if entry.mode_id.is_some() {
                eprintln!(
                    "oto hotkey: could not bind '{}' for {}: {error}",
                    entry.hotkey, entry.id
                );
                continue;
            }
            if let Some(previous) = previous_hotkey.as_deref() {
                if let Ok(prev_sc) = parse_hotkey(previous) {
                    let _ = app
                        .global_shortcut()
                        .on_shortcut(prev_sc, |app, _sc, event| {
                            dispatch_ptt_event(app, event.state());
                        });
                    eprintln!(
                        "oto hotkey: re-registered previous hotkey {previous} after failed bind of {}",
                        entry.hotkey
                    );
                }
            }
            return Err(OtoError::Message(format!(
                "failed to register hotkey '{}': {error}",
                entry.hotkey
            )));
        }

        match parse_hotkey(&entry.hotkey) {
            Ok(check) if app.global_shortcut().is_registered(check) => {
                eprintln!(
                    "hotkey registered and active: {} ({})",
                    entry.hotkey, entry.id
                );
            }
            _ => {
                eprintln!(
                    "hotkey register returned OK but is_registered=false for {} \
                     (macOS may require Accessibility / Input Monitoring — use tray Start/Stop)",
                    entry.hotkey
                );
            }
        }
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
