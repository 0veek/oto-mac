export type PipelineState =
  | "idle"
  | "listening"
  | "processing"
  | "done"
  | "error";

export type PipelineEvent =
  | { type: "state"; state: PipelineState; detail?: string | null }
  | { type: "level"; level: number }
  | { type: "phase"; phase: string }
  | { type: "partial"; text: string }
  | { type: "error"; message: string };

export type ProviderPreset = "open_ai" | "groq" | "open_router" | "deepgram" | "custom";
export type InjectionMode = "auto" | "direct_type" | "clipboard_paste" | "clipboard_only";
export type IdleBehavior = "hide" | "minimal";
export type SttBackend = "cloud" | "local_whisper";
export type ThemePreset = "system" | "midnight" | "light" | "high_contrast";
export type ActivationMode = "hold" | "toggle" | "hybrid";
export type ContextLevel = "none" | "app" | "window" | "selection";

export interface ModeMatch {
  /** Application names or bundle ids, compared case-insensitively. Empty means "any application". */
  app_classes: string[];
  /** Case-insensitive substring of the window title. Empty means "any title". */
  title_contains: string;
}

/**
 * A per-application override. Every nullable field inherits the global value,
 * so a mode only states what it changes.
 */
export interface Mode {
  id: string;
  name: string;
  enabled: boolean;
  match: ModeMatch;
  /** Dedicated global shortcut; empty means no binding of its own. */
  hotkey: string;
  stt_backend: SttBackend | null;
  provider_preset: ProviderPreset | null;
  stt_model: string | null;
  language: string | null;
  polish_enabled: boolean | null;
  polish_model: string | null;
  active_style_id: string | null;
  tone_hint: string | null;
  /** Added to the global dictionary rather than replacing it. */
  dictionary: string[];
  injection_mode: InjectionMode | null;
  context_level: ContextLevel | null;
}

/** A selectable microphone, from `list_audio_inputs`. */
export interface InputDevice {
  name: string;
  is_default: boolean;
}

export interface AudioConfig {
  /** `null` follows the system default input. */
  input_device: string | null;
  input_gain: number;
  noise_gate: boolean;
  noise_gate_threshold: number;
}

export interface VadConfig {
  auto_stop: boolean;
  silence_ms: number;
  min_speech_ms: number;
}

export interface SoundConfig {
  enabled: boolean;
  volume: number;
  on_start: boolean;
  on_stop: boolean;
  on_done: boolean;
  on_error: boolean;
}

export interface Snippet {
  id: string;
  trigger: string;
  expansion: string;
  enabled: boolean;
}

export interface StylePreset {
  id: string;
  name: string;
  prompt: string;
}

export interface ProviderProfile {
  id: string;
  name: string;
  base_url: string;
  stt_model: string;
  polish_model: string;
}

export interface SyncConfig {
  enabled: boolean;
  endpoint: string;
}

export interface HistoryEntry {
  id: string;
  created_at_ms: number;
  raw_text: string;
  final_text: string;
  mode: "dictation" | "command" | "file";
  language: string | null;
  /** A recording is retained on disk and can be replayed or re-transcribed. */
  has_audio: boolean;
  duration_ms: number;
}

/** A deterministic find-and-replace applied after transcription. */
export interface ReplacementRule {
  id: string;
  from: string;
  to: string;
  whole_word: boolean;
  case_sensitive: boolean;
  enabled: boolean;
}

/** A correction Oto could turn into a permanent replacement rule. */
export interface ReplacementSuggestion {
  from: string;
  to: string;
}

export interface DailyCount {
  /** Days before today; 0 is today. */
  days_ago: number;
  sessions: number;
  words: number;
}

export interface UsageStats {
  total_sessions: number;
  total_words: number;
  words_today: number;
  average_words_per_session: number;
  estimated_minutes_saved: number;
  current_streak_days: number;
  best_streak_days: number;
  daily: DailyCount[];
}

export interface AppConfig {
  provider_preset: ProviderPreset;
  base_url: string;
  stt_model: string;
  polish_model: string;
  polish_enabled: boolean;
  temperature: number;
  tone_hint: string;
  hotkey: string;
  language: string | null;
  dictionary: string[];
  injection_mode: InjectionMode;
  idle_behavior: IdleBehavior;
  overlay_x: number | null;
  overlay_y: number | null;
  stt_backend: SttBackend;
  local_whisper_model_path: string;
  vocabulary_boost: boolean;
  snippets: Snippet[];
  styles: StylePreset[];
  active_style_id: string | null;
  history_enabled: boolean;
  history_limit: number;
  streaming_enabled: boolean;
  theme: ThemePreset;
  reduce_motion: boolean;
  font_scale: number;
  custom_providers: ProviderProfile[];
  active_custom_provider_id: string | null;
  sync: SyncConfig;
  activation_mode: ActivationMode;
  /** Below this hold duration a hybrid press counts as a tap. */
  hybrid_tap_threshold_ms: number;
  audio: AudioConfig;
  vad: VadConfig;
  sounds: SoundConfig;
  /** Evaluated in order; the first match wins. */
  modes: Mode[];
  /** Honour spoken edits such as "scratch that" and "new paragraph". */
  voice_edits_enabled: boolean;
  replacements: ReplacementRule[];
  /** Keep each dictation's audio so history can replay and re-transcribe it. */
  keep_history_audio: boolean;
  /** First-run setup has been completed or dismissed. */
  onboarding_complete: boolean;
  context_level: ContextLevel;
  /** User additions to the never-describe list; built-ins always apply. */
  context_blocklist: string[];
}
