//! What the polish model is told about where the text is going.
//!
//! Knowing the target application lets the model format for it — bullets and
//! short lines for a chat client, prose for an email, no markdown in a terminal.
//! That is genuinely useful, and it is also the part of dictation that leaks the
//! most, so every level past the application name is opt-in and password
//! managers are redacted regardless of the setting.

use crate::config::{context_is_blocked, ContextLevel};
use crate::injection::FocusTarget;

/// Window titles get long and often carry the document's full path.
const MAX_TITLE_CHARS: usize = 120;
/// Enough surrounding text to establish voice and formatting, not the document.
const MAX_SURROUNDING_CHARS: usize = 600;

/// The context assembled for one dictation, ready to describe to a model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DictationContext {
    pub app: Option<String>,
    pub window_title: Option<String>,
    pub surrounding_text: Option<String>,
    /// True when the target was on the never-describe list and everything was
    /// dropped. Surfaced in settings so the redaction is visible, not silent.
    pub redacted: bool,
}

/// Char-safe truncation with an ellipsis.
fn clip(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// Assemble context at `level` for `target`.
///
/// `selection` is the Accessibility selection, already fetched by the caller —
/// this function performs no I/O so it stays testable.
pub fn build(
    level: ContextLevel,
    target: &FocusTarget,
    extra_blocklist: &[String],
    selection: Option<&str>,
) -> DictationContext {
    if level == ContextLevel::None {
        return DictationContext::default();
    }

    let app = target
        .class
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    let bundle_id = target
        .bundle_id
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());

    // A blocked application discloses nothing at all — not even its name, which
    // would itself reveal that the user is in their password manager. Either
    // identifier is enough to block: an app renamed in Finder still ships the
    // same bundle id.
    let blocked = app.is_some_and(|app| context_is_blocked(app, extra_blocklist))
        || bundle_id.is_some_and(|id| context_is_blocked(id, extra_blocklist));
    if blocked {
        return DictationContext {
            redacted: true,
            ..Default::default()
        };
    }

    let mut context = DictationContext {
        // Fall back to the bundle id only when there is no human-readable name;
        // "com.apple.Terminal" is worse guidance to a model than "Terminal".
        app: app.or(bundle_id).map(str::to_string),
        ..Default::default()
    };

    if level >= ContextLevel::Window {
        context.window_title = target
            .title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| clip(t, MAX_TITLE_CHARS));
    }

    if level >= ContextLevel::Selection {
        context.surrounding_text = selection
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| clip(t, MAX_SURROUNDING_CHARS));
    }

    context
}

impl DictationContext {
    pub fn is_empty(&self) -> bool {
        self.app.is_none() && self.window_title.is_none() && self.surrounding_text.is_none()
    }

    /// The sentence appended to the polish system prompt, or `None` when there
    /// is nothing to say.
    pub fn prompt_line(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(app) = self.app.as_deref() {
            parts.push(format!("the application \"{app}\""));
        }
        if let Some(title) = self.window_title.as_deref() {
            parts.push(format!("window titled \"{title}\""));
        }
        let mut line = format!(
            "This text will be typed into {}. Match the formatting conventions of that context.",
            parts.join(", ")
        );
        if let Some(surrounding) = self.surrounding_text.as_deref() {
            line.push_str(&format!(
                " Nearby text for tone and style reference (do not repeat it): \"{surrounding}\""
            ));
        }
        Some(line)
    }

    /// Exactly what would be sent, for the settings preview. The preview has to
    /// render the real payload, or it is just a promise.
    pub fn preview(&self) -> String {
        if self.redacted {
            return "Nothing — this application is on the never-describe list.".to_string();
        }
        self.prompt_line()
            .unwrap_or_else(|| "Nothing — no context available for this window.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(class: &str, title: &str) -> FocusTarget {
        FocusTarget {
            pid: Some(1234),
            name: Some(class.to_string()),
            class: Some(class.to_string()),
            bundle_id: None,
            title: Some(title.to_string()),
        }
    }

    #[test]
    fn level_none_sends_nothing() {
        let ctx = build(
            ContextLevel::None,
            &target("Slack", "#eng-general"),
            &[],
            Some("hello"),
        );
        assert!(ctx.is_empty());
        assert!(ctx.prompt_line().is_none());
    }

    #[test]
    fn level_app_sends_only_the_application() {
        let ctx = build(
            ContextLevel::App,
            &target("Slack", "#eng-general"),
            &[],
            Some("hello"),
        );
        assert_eq!(ctx.app.as_deref(), Some("Slack"));
        assert!(ctx.window_title.is_none());
        assert!(ctx.surrounding_text.is_none());
        let line = ctx.prompt_line().unwrap();
        assert!(line.contains("Slack"));
        assert!(!line.contains("eng-general"));
    }

    #[test]
    fn level_window_adds_the_title_but_not_the_selection() {
        let ctx = build(
            ContextLevel::Window,
            &target("Slack", "#eng-general"),
            &[],
            Some("secret draft"),
        );
        assert_eq!(ctx.window_title.as_deref(), Some("#eng-general"));
        assert!(ctx.surrounding_text.is_none());
        assert!(!ctx.prompt_line().unwrap().contains("secret draft"));
    }

    #[test]
    fn level_selection_adds_surrounding_text() {
        let ctx = build(
            ContextLevel::Selection,
            &target("Slack", "#eng-general"),
            &[],
            Some("the previous message"),
        );
        assert_eq!(ctx.surrounding_text.as_deref(), Some("the previous message"));
        let line = ctx.prompt_line().unwrap();
        assert!(line.contains("the previous message"));
        assert!(line.contains("do not repeat it"));
    }

    #[test]
    fn a_blocked_application_discloses_nothing_at_any_level() {
        // Not even the app name: "you are in Keychain Access" is itself the leak.
        for level in [
            ContextLevel::App,
            ContextLevel::Window,
            ContextLevel::Selection,
        ] {
            let ctx = build(
                level,
                &target("Keychain Access", "Login — Keychain Access"),
                &[],
                Some("hunter2"),
            );
            assert!(ctx.is_empty(), "{level:?} leaked something");
            assert!(ctx.redacted);
            assert!(ctx.preview().contains("never-describe"));
        }
    }

    #[test]
    fn a_blocked_bundle_id_redacts_even_when_the_name_looks_harmless() {
        // Applications can be renamed in Finder; the bundle id cannot.
        let renamed = FocusTarget {
            bundle_id: Some("com.agilebits.onepassword7".into()),
            ..target("Notes", "Shopping list")
        };
        let ctx = build(ContextLevel::Window, &renamed, &[], None);
        assert!(ctx.is_empty());
        assert!(ctx.redacted);
    }

    #[test]
    fn the_bundle_id_names_the_app_only_when_nothing_better_exists() {
        let unnamed = FocusTarget {
            class: None,
            name: None,
            bundle_id: Some("com.example.tool".into()),
            ..target("ignored", "Window")
        };
        let ctx = build(ContextLevel::App, &unnamed, &[], None);
        assert_eq!(ctx.app.as_deref(), Some("com.example.tool"));
    }

    #[test]
    fn a_user_blocklist_entry_is_honoured() {
        let ctx = build(
            ContextLevel::Window,
            &target("my-journal", "2026 diary"),
            &["my-journal".into()],
            None,
        );
        assert!(ctx.is_empty());
        assert!(ctx.redacted);
    }

    #[test]
    fn long_titles_and_selections_are_clipped() {
        let long_title = "x".repeat(400);
        let long_selection = "y".repeat(5_000);
        let ctx = build(
            ContextLevel::Selection,
            &target("Code", &long_title),
            &[],
            Some(&long_selection),
        );
        let title = ctx.window_title.unwrap();
        assert_eq!(
            title.chars().count(),
            MAX_TITLE_CHARS + 1,
            "clipped plus ellipsis"
        );
        assert!(title.ends_with('…'));
        let surrounding = ctx.surrounding_text.unwrap();
        assert_eq!(surrounding.chars().count(), MAX_SURROUNDING_CHARS + 1);
    }

    #[test]
    fn multibyte_text_is_clipped_without_panicking() {
        let emoji_title = "🙂".repeat(300);
        let ctx = build(
            ContextLevel::Window,
            &target("Code", &emoji_title),
            &[],
            None,
        );
        assert!(ctx.window_title.unwrap().ends_with('…'));
    }

    #[test]
    fn a_window_with_no_application_or_title_produces_no_line() {
        let empty = FocusTarget::default();
        let ctx = build(ContextLevel::Selection, &empty, &[], None);
        assert!(ctx.is_empty());
        assert!(!ctx.redacted);
        assert!(ctx.preview().contains("no context available"));
    }
}
