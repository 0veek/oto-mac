//! Which chords Oto wants macOS to deliver.
//!
//! Oto binds one chord for ordinary dictation plus, optionally, one per Mode.
//! Keeping the desired set separate from the registration backend means the
//! tricky part — deciding what to bind without two chords fighting over the same
//! keys — is ordinary testable code.

use crate::config::AppConfig;

/// Shortcut id used for the primary dictation binding.
pub const PRIMARY_ID: &str = "dictation";

/// One chord Oto wants delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Stable identifier: `dictation`, or `mode:<mode id>`.
    pub id: String,
    /// Normalized chord, e.g. `Ctrl+Shift+Space`.
    pub hotkey: String,
    /// `None` for the primary binding; otherwise the Mode to start under.
    pub mode_id: Option<String>,
    /// Human-readable description, used in logs and error messages.
    pub label: String,
}

impl Binding {
    pub fn primary(hotkey: String) -> Self {
        Self {
            id: PRIMARY_ID.to_string(),
            hotkey,
            mode_id: None,
            label: "Start or stop Oto dictation".to_string(),
        }
    }
}

/// Every chord Oto should hold, primary first.
///
/// Modes without a chord, and modes whose chord collides with one already in the
/// list, are skipped: two bindings for the same keys would make which Mode runs
/// depend on registration order.
pub fn desired_bindings(cfg: &AppConfig, normalize: impl Fn(&str) -> String) -> Vec<Binding> {
    let primary = normalize(&cfg.hotkey);
    let mut bindings = vec![Binding::primary(primary.clone())];
    let mut claimed = vec![primary];

    for mode in &cfg.modes {
        if !mode.enabled {
            continue;
        }
        let chord = normalize(&mode.hotkey);
        if chord.is_empty() {
            continue;
        }
        if claimed.iter().any(|existing| existing == &chord) {
            eprintln!(
                "oto hotkey: mode '{}' wants {chord}, which is already bound — skipping",
                mode.name
            );
            continue;
        }
        claimed.push(chord.clone());
        bindings.push(Binding {
            id: format!("mode:{}", mode.id),
            hotkey: chord,
            mode_id: Some(mode.id.clone()),
            label: format!("Oto dictation — {}", mode.name),
        });
    }
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Mode, ModeMatch};

    #[test]
    fn desired_bindings_start_with_the_primary_hotkey() {
        let cfg = AppConfig {
            hotkey: "Ctrl+Shift+Space".into(),
            ..AppConfig::default()
        };
        let bindings = desired_bindings(&cfg, |h| h.to_string());
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].id, PRIMARY_ID);
        assert!(bindings[0].mode_id.is_none());
    }

    #[test]
    fn each_enabled_mode_with_a_chord_gets_a_binding() {
        let cfg = AppConfig {
            hotkey: "Ctrl+Shift+Space".into(),
            modes: vec![
                Mode {
                    hotkey: "Ctrl+Shift+J".into(),
                    ..Mode::new("code".into(), "Code".into())
                },
                // No chord — matched by window only.
                Mode {
                    match_rule: ModeMatch {
                        app_classes: vec!["slack".into()],
                        title_contains: String::new(),
                    },
                    ..Mode::new("chat".into(), "Chat".into())
                },
                Mode {
                    enabled: false,
                    hotkey: "Ctrl+Shift+K".into(),
                    ..Mode::new("off".into(), "Off".into())
                },
            ],
            ..AppConfig::default()
        };
        let bindings = desired_bindings(&cfg, |h| h.to_string());
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[1].id, "mode:code");
        assert_eq!(bindings[1].mode_id.as_deref(), Some("code"));
        assert!(bindings[1].label.contains("Code"));
    }

    #[test]
    fn a_mode_reusing_the_primary_chord_is_skipped() {
        // Two bindings on one chord would make the winner depend on
        // registration order, which is not something a user can reason about.
        let cfg = AppConfig {
            hotkey: "Ctrl+Shift+Space".into(),
            modes: vec![Mode {
                hotkey: "Ctrl+Shift+Space".into(),
                ..Mode::new("dupe".into(), "Dupe".into())
            }],
            ..AppConfig::default()
        };
        assert_eq!(desired_bindings(&cfg, |h| h.to_string()).len(), 1);
    }

    #[test]
    fn a_blank_mode_chord_is_not_a_binding() {
        let cfg = AppConfig {
            modes: vec![Mode {
                hotkey: "   ".into(),
                ..Mode::new("blank".into(), "Blank".into())
            }],
            ..AppConfig::default()
        };
        // `normalize_hotkey` drops empty segments, so whitespace collapses to
        // nothing and the mode contributes no binding at all.
        let bindings = desired_bindings(&cfg, |h| h.trim().to_string());
        assert_eq!(bindings.len(), 1);
    }
}
