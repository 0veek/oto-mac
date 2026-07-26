use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderPreset {
    OpenAi,
    Groq,
    OpenRouter,
    Deepgram,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InjectionMode {
    Auto,
    DirectType,
    ClipboardPaste,
    ClipboardOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IdleBehavior {
    Hide,
    Minimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SttBackend {
    #[default]
    Cloud,
    LocalWhisper,
}

/// How the hotkey starts and stops a dictation session.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActivationMode {
    /// Press to start, release to stop. The 0.1.0 behaviour.
    #[default]
    Hold,
    /// Press to start, press again to stop. Hands-free.
    Toggle,
    /// A quick tap toggles; holding past the tap threshold behaves like `Hold`.
    Hybrid,
}

/// Microphone selection and input conditioning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AudioConfig {
    /// cpal device name. `None` follows the system default input.
    pub input_device: Option<String>,
    /// Linear multiplier applied before the sample is stored.
    pub input_gain: f32,
    /// Attenuate frames that sit at or below the measured noise floor.
    pub noise_gate: bool,
    /// Gate opens this far above the adaptive noise floor (0..=1 of full scale).
    pub noise_gate_threshold: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            input_device: None,
            input_gain: 1.0,
            noise_gate: false,
            noise_gate_threshold: 0.02,
        }
    }
}

impl AudioConfig {
    /// Gain clamped to a range that cannot destroy a signal or blow it out.
    pub fn effective_gain(&self) -> f32 {
        if self.input_gain.is_finite() {
            self.input_gain.clamp(0.25, 4.0)
        } else {
            1.0
        }
    }

    pub fn effective_gate_threshold(&self) -> f32 {
        if self.noise_gate_threshold.is_finite() {
            self.noise_gate_threshold.clamp(0.0, 0.5)
        } else {
            0.02
        }
    }
}

/// Voice-activity detection used to end hands-free sessions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VadConfig {
    /// Stop a hands-free session after `silence_ms` of trailing silence.
    /// Never applies to a held hotkey — the release governs there.
    pub auto_stop: bool,
    pub silence_ms: u32,
    /// Speech required before auto-stop may fire, so a slow start is not cut off.
    pub min_speech_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            auto_stop: true,
            silence_ms: 1_500,
            min_speech_ms: 400,
        }
    }
}

impl VadConfig {
    pub fn effective_silence_ms(&self) -> u32 {
        self.silence_ms.clamp(400, 10_000)
    }

    pub fn effective_min_speech_ms(&self) -> u32 {
        self.min_speech_ms.clamp(0, 5_000)
    }
}

/// Short synthesized cues marking session transitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SoundConfig {
    pub enabled: bool,
    pub volume: f32,
    pub on_start: bool,
    pub on_stop: bool,
    pub on_done: bool,
    pub on_error: bool,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            // Off by default: an unexpected beep is worse than a missing one.
            // Onboarding offers to turn cues on.
            enabled: false,
            volume: 0.4,
            on_start: true,
            on_stop: true,
            on_done: false,
            on_error: true,
        }
    }
}

impl SoundConfig {
    pub fn effective_volume(&self) -> f32 {
        if self.volume.is_finite() {
            self.volume.clamp(0.0, 1.0)
        } else {
            0.4
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreset {
    System,
    #[default]
    Midnight,
    Light,
    HighContrast,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snippet {
    pub id: String,
    pub trigger: String,
    pub expansion: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A deterministic find-and-replace applied after transcription.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ReplacementRule {
    pub id: String,
    pub from: String,
    pub to: String,
    /// Only match when the text is not glued to surrounding word characters.
    pub whole_word: bool,
    pub case_sensitive: bool,
    pub enabled: bool,
}

impl Default for ReplacementRule {
    fn default() -> Self {
        Self {
            id: String::new(),
            from: String::new(),
            to: String::new(),
            whole_word: true,
            case_sensitive: false,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StylePreset {
    pub id: String,
    pub name: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub stt_model: String,
    pub polish_model: String,
}

/// How much is told to the polish model about where the text is going.
///
/// Ordered by how much leaves the machine, and every step past `App` is opt-in.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ContextLevel {
    /// Send nothing about the target application.
    None,
    /// Application name or bundle id only, e.g. `Slack`.
    #[default]
    App,
    /// Application identity and window title.
    Window,
    /// Adds the current selection read through the macOS Accessibility API.
    Selection,
}

/// Applications whose contents are never described to a remote model.
///
/// Matched as substrings against the lowercased app name *and* bundle id, so
/// `com.apple.keychainaccess` and `Keychain Access` both hit. Users can extend
/// this, but never shorten it.
pub const CONTEXT_BLOCKLIST: &[&str] = &[
    "keychain",
    "apple.passwords",
    "securityagent",
    "loginwindow",
    "keepass",
    "bitwarden",
    "1password",
    "1passwd",
    // 1Password's own bundle ids spell it out and reverse it.
    "onepassword",
    "agilebits",
    "lastpass",
    "dashlane",
    "enpass",
    "nordpass",
    "protonpass",
    "proton pass",
    "strongbox",
    "secretive",
    "gpg",
    "authy",
    "authenticator",
];

/// True when an application's contents must not be described to a remote model.
pub fn context_is_blocked(app_class: &str, extra: &[String]) -> bool {
    let class = app_class.trim().to_ascii_lowercase();
    if class.is_empty() {
        return false;
    }
    if CONTEXT_BLOCKLIST.iter().any(|term| class.contains(term)) {
        return true;
    }
    extra.iter().any(|term| {
        let term = term.trim().to_ascii_lowercase();
        !term.is_empty() && class.contains(&term)
    })
}

/// Which windows a [`Mode`] applies to.
///
/// An empty rule never matches automatically — such a mode is reachable only
/// through its own hotkey. Matching everything would silently shadow the
/// default configuration, which is never what someone means by "no rule".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct ModeMatch {
    /// Application names or bundle ids, compared case-insensitively.
    /// Empty means "any application".
    pub app_classes: Vec<String>,
    /// Case-insensitive substring of the window title. Empty means "any title".
    pub title_contains: String,
}

impl ModeMatch {
    fn is_empty(&self) -> bool {
        self.app_classes.iter().all(|c| c.trim().is_empty()) && self.title_contains.trim().is_empty()
    }

    pub fn matches(&self, app_class: Option<&str>, window_title: Option<&str>) -> bool {
        if self.is_empty() {
            return false;
        }

        let classes: Vec<String> = self
            .app_classes
            .iter()
            .map(|c| c.trim().to_ascii_lowercase())
            .filter(|c| !c.is_empty())
            .collect();
        if !classes.is_empty() {
            let Some(actual) = app_class.map(|c| c.trim().to_ascii_lowercase()) else {
                return false;
            };
            // Substring both ways: users type "code" for "Visual Studio Code",
            // and macOS reports reverse-DNS bundle ids like "com.tinyspeck.slackmacgap".
            if !classes
                .iter()
                .any(|wanted| actual.contains(wanted) || wanted.contains(&actual))
            {
                return false;
            }
        }

        let needle = self.title_contains.trim().to_ascii_lowercase();
        if !needle.is_empty() {
            let Some(title) = window_title.map(|t| t.to_ascii_lowercase()) else {
                return false;
            };
            if !title.contains(&needle) {
                return false;
            }
        }

        true
    }
}

/// A per-application configuration override.
///
/// Every field is optional and `None` inherits the global value, so a mode only
/// states what it changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Mode {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    #[serde(rename = "match")]
    pub match_rule: ModeMatch,
    /// Dedicated global shortcut. Empty means the mode has no binding of its own.
    pub hotkey: String,

    pub stt_backend: Option<SttBackend>,
    pub provider_preset: Option<ProviderPreset>,
    pub stt_model: Option<String>,
    pub language: Option<String>,

    pub polish_enabled: Option<bool>,
    pub polish_model: Option<String>,
    pub active_style_id: Option<String>,
    pub tone_hint: Option<String>,

    /// Extra vocabulary, added to the global dictionary rather than replacing it.
    pub dictionary: Vec<String>,
    pub injection_mode: Option<InjectionMode>,
    pub context_level: Option<ContextLevel>,
}

impl Mode {
    /// A mode with sensible defaults and no overrides set.
    ///
    /// The settings UI builds modes in TypeScript, so this currently has no
    /// production caller; it stays because it is part of the config API and is
    /// what the tests construct against.
    #[allow(dead_code)]
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            enabled: true,
            ..Default::default()
        }
    }

    pub fn matches(&self, app_class: Option<&str>, window_title: Option<&str>) -> bool {
        self.enabled && self.match_rule.matches(app_class, window_title)
    }

    /// Fold this mode's overrides into a copy of the global configuration.
    pub fn apply(&self, cfg: &mut AppConfig) {
        if let Some(backend) = self.stt_backend.clone() {
            cfg.stt_backend = backend;
        }
        if let Some(preset) = self.provider_preset.clone() {
            // The base URL belongs to the preset; carrying the old one over
            // would point a Deepgram key at an OpenAI host.
            cfg.base_url = crate::providers::presets::base_url_for(&preset).to_string();
            cfg.provider_preset = preset;
        }
        if let Some(model) = self.stt_model.as_ref().filter(|m| !m.trim().is_empty()) {
            cfg.stt_model = model.clone();
        }
        if let Some(language) = self.language.as_ref() {
            // An explicitly empty language means "auto-detect", which is a real
            // choice and distinct from inheriting the global setting.
            cfg.language = Some(language.clone()).filter(|l| !l.trim().is_empty());
        }
        if let Some(enabled) = self.polish_enabled {
            cfg.polish_enabled = enabled;
        }
        if let Some(model) = self.polish_model.as_ref().filter(|m| !m.trim().is_empty()) {
            cfg.polish_model = model.clone();
        }
        if let Some(style_id) = self.active_style_id.as_ref() {
            cfg.active_style_id = Some(style_id.clone()).filter(|s| !s.trim().is_empty());
        }
        if let Some(tone) = self.tone_hint.as_ref() {
            cfg.tone_hint = tone.clone();
        }
        if let Some(mode) = self.injection_mode.clone() {
            cfg.injection_mode = mode;
        }
        if let Some(level) = self.context_level {
            cfg.context_level = level;
        }
        for term in &self.dictionary {
            let term = term.trim();
            if !term.is_empty() && !cfg.dictionary.iter().any(|existing| existing == term) {
                cfg.dictionary.push(term.to_string());
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct SyncConfig {
    pub enabled: bool,
    /// User-controlled HTTP(S) document URL. Credentials are stored in keyring.
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub provider_preset: ProviderPreset,
    pub base_url: String,
    pub stt_model: String,
    pub polish_model: String,
    pub polish_enabled: bool,
    pub temperature: f32,
    pub tone_hint: String,
    pub hotkey: String,
    pub language: Option<String>,
    pub dictionary: Vec<String>,
    pub injection_mode: InjectionMode,
    pub idle_behavior: IdleBehavior,
    pub overlay_x: Option<i32>,
    pub overlay_y: Option<i32>,
    pub stt_backend: SttBackend,
    pub local_whisper_model_path: String,
    pub vocabulary_boost: bool,
    pub snippets: Vec<Snippet>,
    pub styles: Vec<StylePreset>,
    pub active_style_id: Option<String>,
    pub history_enabled: bool,
    pub history_limit: usize,
    pub streaming_enabled: bool,
    pub theme: ThemePreset,
    pub reduce_motion: bool,
    pub font_scale: f32,
    pub custom_providers: Vec<ProviderProfile>,
    pub active_custom_provider_id: Option<String>,
    pub sync: SyncConfig,
    pub activation_mode: ActivationMode,
    /// Below this hold duration a `Hybrid` press counts as a tap (toggle).
    pub hybrid_tap_threshold_ms: u32,
    pub audio: AudioConfig,
    pub vad: VadConfig,
    pub sounds: SoundConfig,
    /// Per-application overrides, evaluated in order; the first match wins.
    pub modes: Vec<Mode>,
    /// How much the polish model is told about the target application.
    pub context_level: ContextLevel,
    /// User additions to the never-describe list. The built-in entries always apply.
    pub context_blocklist: Vec<String>,
    /// Honour spoken edits such as "scratch that" and "new paragraph".
    pub voice_edits_enabled: bool,
    /// Deterministic find-and-replace applied after transcription.
    pub replacements: Vec<ReplacementRule>,
    /// Keep each dictation's audio so history can replay and re-transcribe it.
    pub keep_history_audio: bool,
    /// First-run setup has been completed or dismissed.
    ///
    /// Defaults to `true` so an existing `config.json` — which predates this
    /// field — is never sent back through onboarding. Only a genuinely absent
    /// config file starts at `false`; see `store::read_config_from`.
    pub onboarding_complete: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            provider_preset: ProviderPreset::Groq,
            base_url: "https://api.groq.com/openai/v1".into(),
            stt_model: "whisper-large-v3".into(),
            polish_model: "llama-3.1-8b-instant".into(),
            polish_enabled: true,
            temperature: 0.2,
            tone_hint: String::new(),
            hotkey: "Ctrl+Shift+Space".into(),
            language: None,
            dictionary: vec![],
            injection_mode: InjectionMode::Auto,
            idle_behavior: IdleBehavior::Hide,
            overlay_x: None,
            overlay_y: None,
            stt_backend: SttBackend::Cloud,
            local_whisper_model_path: String::new(),
            vocabulary_boost: true,
            snippets: vec![],
            styles: default_styles(),
            active_style_id: None,
            history_enabled: true,
            history_limit: 100,
            streaming_enabled: false,
            theme: ThemePreset::Midnight,
            reduce_motion: false,
            font_scale: 1.0,
            custom_providers: vec![],
            active_custom_provider_id: None,
            sync: SyncConfig::default(),
            activation_mode: ActivationMode::default(),
            hybrid_tap_threshold_ms: 350,
            audio: AudioConfig::default(),
            vad: VadConfig::default(),
            sounds: SoundConfig::default(),
            modes: vec![],
            context_level: ContextLevel::default(),
            context_blocklist: vec![],
            voice_edits_enabled: true,
            replacements: vec![],
            // Off by default: audio is the most sensitive thing Oto handles, and
            // keeping it is a choice the user should make deliberately.
            keep_history_audio: false,
            onboarding_complete: true,
        }
    }
}

fn default_true() -> bool {
    true
}

pub fn default_styles() -> Vec<StylePreset> {
    vec![
        StylePreset {
            id: "professional".into(),
            name: "Professional".into(),
            prompt: "Professional, clear, and concise. Avoid filler and unnecessary flourish."
                .into(),
        },
        StylePreset {
            id: "casual".into(),
            name: "Casual".into(),
            prompt: "Natural and friendly while preserving the speaker's personality.".into(),
        },
        StylePreset {
            id: "email".into(),
            name: "Email".into(),
            prompt: "Polished email prose with sensible paragraphs and a courteous tone.".into(),
        },
        StylePreset {
            id: "code_comment".into(),
            name: "Code comment".into(),
            prompt: "Concise technical language suitable for code comments and documentation."
                .into(),
        },
    ]
}

impl AppConfig {
    pub fn active_style_prompt(&self) -> String {
        let preset = self
            .active_style_id
            .as_deref()
            .and_then(|id| self.styles.iter().find(|style| style.id == id))
            .map(|style| style.prompt.trim())
            .filter(|prompt| !prompt.is_empty());
        match (preset, self.tone_hint.trim()) {
            (Some(preset), "") => preset.to_string(),
            (Some(preset), custom) => format!("{preset} {custom}"),
            (None, custom) => custom.to_string(),
        }
    }

    /// Hold duration under which a `Hybrid` press is treated as a tap.
    pub fn effective_tap_threshold_ms(&self) -> u64 {
        u64::from(self.hybrid_tap_threshold_ms.clamp(120, 2_000))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_mvp_config_gets_phase_two_defaults() {
        let old = serde_json::json!({
            "provider_preset": "groq",
            "base_url": "https://api.groq.com/openai/v1",
            "stt_model": "whisper-large-v3",
            "polish_model": "llama-3.1-8b-instant",
            "polish_enabled": true,
            "temperature": 0.2,
            "tone_hint": "",
            "hotkey": "Ctrl+Shift+Space",
            "language": null,
            "dictionary": [],
            "injection_mode": "auto",
            "idle_behavior": "hide",
            "overlay_x": null,
            "overlay_y": null
        });
        let config: AppConfig = serde_json::from_value(old).unwrap();
        assert_eq!(config.stt_backend, SttBackend::Cloud);
        assert!(config.history_enabled);
        assert!(!config.styles.is_empty());
        // A 0.1.0 config must keep behaving exactly as it did: push-to-talk,
        // unity gain, no gate, no beeps.
        assert_eq!(config.activation_mode, ActivationMode::Hold);
        assert_eq!(config.audio, AudioConfig::default());
        assert!((config.audio.effective_gain() - 1.0).abs() < f32::EPSILON);
        assert!(!config.audio.noise_gate);
        assert!(!config.sounds.enabled);
        // An upgrade must never drop the user back into first-run setup.
        assert!(config.onboarding_complete);
    }

    #[test]
    fn partial_audio_section_fills_the_rest_from_defaults() {
        // Hand-edited config.json files are a supported reality.
        let partial = serde_json::json!({ "audio": { "input_gain": 2.0 } });
        let config: AppConfig = serde_json::from_value(partial).unwrap();
        assert!((config.audio.input_gain - 2.0).abs() < f32::EPSILON);
        assert!(config.audio.input_device.is_none());
        assert!(!config.audio.noise_gate);
    }

    #[test]
    fn hostile_numeric_values_are_clamped_not_trusted() {
        let cfg = AppConfig {
            audio: AudioConfig {
                input_gain: -50.0,
                ..Default::default()
            },
            vad: VadConfig {
                silence_ms: 0,
                min_speech_ms: 999_999,
                ..Default::default()
            },
            sounds: SoundConfig {
                volume: 12.0,
                ..Default::default()
            },
            hybrid_tap_threshold_ms: 1,
            ..AppConfig::default()
        };
        assert!((cfg.audio.effective_gain() - 0.25).abs() < f32::EPSILON);
        assert_eq!(cfg.vad.effective_silence_ms(), 400);
        assert_eq!(cfg.vad.effective_min_speech_ms(), 5_000);
        assert!((cfg.sounds.effective_volume() - 1.0).abs() < f32::EPSILON);
        assert_eq!(cfg.effective_tap_threshold_ms(), 120);
    }

    #[test]
    fn class_matching_is_case_insensitive_and_partial_both_ways() {
        let rule = ModeMatch {
            app_classes: vec!["slack".into()],
            title_contains: String::new(),
        };
        assert!(rule.matches(Some("Slack"), None));
        // macOS reports reverse-DNS bundle ids.
        assert!(rule.matches(Some("com.tinyspeck.slackmacgap"), None));
        // Users type a short prefix for a long application name.
        let short = ModeMatch {
            app_classes: vec!["code".into()],
            title_contains: String::new(),
        };
        assert!(short.matches(Some("Visual Studio Code"), None));
        assert!(!rule.matches(Some("Firefox"), None));
        assert!(!rule.matches(None, None));
    }

    #[test]
    fn title_narrows_a_class_match() {
        let rule = ModeMatch {
            app_classes: vec!["slack".into()],
            title_contains: "#eng".into(),
        };
        assert!(rule.matches(Some("Slack"), Some("Slack | #eng-general")));
        assert!(!rule.matches(Some("Slack"), Some("Slack | #random")));
        // A title requirement cannot be satisfied by a window with no title.
        assert!(!rule.matches(Some("Slack"), None));
    }

    #[test]
    fn a_title_only_rule_matches_any_application() {
        let rule = ModeMatch {
            app_classes: vec![],
            title_contains: "jira".into(),
        };
        assert!(rule.matches(Some("Safari"), Some("OTO-14 · Jira")));
        assert!(rule.matches(Some("Google Chrome"), Some("jira board")));
        assert!(!rule.matches(Some("Safari"), Some("Hacker News")));
    }

    #[test]
    fn an_empty_rule_matches_nothing() {
        let rule = ModeMatch::default();
        assert!(!rule.matches(Some("anything"), Some("any title")));
        assert!(!rule.matches(None, None));
        // Blank strings count as empty, not as a wildcard.
        let blank = ModeMatch {
            app_classes: vec!["   ".into()],
            title_contains: "  ".into(),
        };
        assert!(!blank.matches(Some("Safari"), Some("title")));
    }

    #[test]
    fn password_managers_are_blocked_from_context_by_default() {
        for class in [
            "com.apple.keychainaccess",
            "Keychain Access",
            "com.apple.Passwords",
            "Bitwarden",
            "1Password",
            // The bundle id spells the name out; matching only "1password"
            // would let the real application through.
            "com.agilebits.onepassword7",
            "com.1password.1password",
            "org.keepassxc.KeePassXC",
            "com.authy.authy",
        ] {
            assert!(
                context_is_blocked(class, &[]),
                "{class} should never be described to a remote model"
            );
        }
        assert!(!context_is_blocked("Safari", &[]));
        assert!(!context_is_blocked("", &[]));
    }

    #[test]
    fn the_user_blocklist_extends_but_cannot_shorten_the_builtin_one() {
        let extra = vec!["my-notes".into()];
        assert!(context_is_blocked("my-notes", &extra));
        assert!(context_is_blocked("My-Notes-Pro", &extra));
        // Adding entries never re-enables a built-in block.
        assert!(context_is_blocked("Keychain Access", &extra));
        // Blank user entries must not match everything.
        assert!(!context_is_blocked("Safari", &["".into(), "   ".into()]));
    }

    #[test]
    fn context_levels_order_by_how_much_they_disclose() {
        assert!(ContextLevel::None < ContextLevel::App);
        assert!(ContextLevel::App < ContextLevel::Window);
        assert!(ContextLevel::Window < ContextLevel::Selection);
        assert_eq!(ContextLevel::default(), ContextLevel::App);
    }

    #[test]
    fn modes_round_trip_with_match_renamed() {
        let cfg = AppConfig {
            modes: vec![Mode {
                match_rule: ModeMatch {
                    app_classes: vec!["slack".into()],
                    title_contains: String::new(),
                },
                polish_enabled: Some(false),
                ..Mode::new("chat".into(), "Chat".into())
            }],
            ..AppConfig::default()
        };
        let raw = serde_json::to_string(&cfg).unwrap();
        assert!(raw.contains("\"match\":"));
        let back: AppConfig = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.modes, cfg.modes);
    }

    #[test]
    fn activation_mode_round_trips_as_snake_case() {
        let cfg = AppConfig {
            activation_mode: ActivationMode::Hybrid,
            ..AppConfig::default()
        };
        let raw = serde_json::to_string(&cfg).unwrap();
        assert!(raw.contains("\"activation_mode\":\"hybrid\""));
        let back: AppConfig = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.activation_mode, ActivationMode::Hybrid);
    }

    #[test]
    fn active_style_and_custom_hint_are_combined() {
        let config = AppConfig {
            active_style_id: Some("professional".into()),
            tone_hint: "Use short paragraphs.".into(),
            ..AppConfig::default()
        };
        let prompt = config.active_style_prompt();
        assert!(prompt.contains("Professional"));
        assert!(prompt.contains("short paragraphs"));
    }
}
