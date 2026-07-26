//! PTT lifecycle: record → STT → optional polish → inject → events.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};
use tokio::time::{sleep, Duration};

use crate::audio::cues::{self, Cue};
use crate::audio::{AudioRecorder, CaptureTuning};
use crate::config::{
    load_config, ActivationMode, AppConfig, AppContext, ContextLevel, IdleBehavior, ProviderPreset,
    ResolvedConfig, SttBackend,
};
use crate::error::{OtoError, OtoResult};
use crate::features::{history, snippets::expand_snippet};
use crate::injection::{
    capture_focus_target, capture_selected_text, inject_text_to, FocusTarget, InjectResult,
};
use crate::pipeline::context;
use crate::pipeline::events::{PipelineEvent, PipelineState};
use crate::providers::{
    client_from_config, DeepgramStream, LocalWhisperClient, OpenAiCompatClient, PolishContext,
    SpeechToText, SttStream, TextPolisher, TranscriptionContext,
};
use crate::state::AppState;

async fn client_from_config_async(cfg: &crate::config::AppConfig) -> OtoResult<OpenAiCompatClient> {
    // keyring's Secret Service backend is synchronous and internally blocks on
    // Tokio. Run it outside the async worker to avoid nested-runtime panics.
    let cfg = cfg.clone();
    tauri::async_runtime::spawn_blocking(move || client_from_config(&cfg))
        .await
        .map_err(|error| OtoError::Message(format!("credential lookup task failed: {error}")))?
}

async fn deepgram_from_config_async(
    cfg: &crate::config::AppConfig,
) -> OtoResult<crate::providers::DeepgramClient> {
    let cfg = cfg.clone();
    tauri::async_runtime::spawn_blocking(move || crate::providers::deepgram::client_from_config(&cfg))
        .await
        .map_err(|error| OtoError::Message(format!("credential lookup task failed: {error}")))?
}

fn is_deepgram(cfg: &AppConfig) -> bool {
    cfg.provider_preset == ProviderPreset::Deepgram
}

fn command_mode_client_error(error: &OtoError) -> String {
    let message = error.to_string();
    if message.contains("API key not set") {
        "Command Mode needs an LLM API key to rewrite the selection. Local Whisper and Deepgram only handle speech-to-text — add a key under Providers for an OpenAI-compatible LLM, or use a Custom provider pointed at a local OpenAI-compatible server (localhost needs no key).".into()
    } else {
        message
    }
}

fn transcription_context(cfg: &AppConfig) -> TranscriptionContext {
    let keyterms = if cfg.vocabulary_boost {
        cfg.dictionary
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    };
    TranscriptionContext {
        language: cfg
            .language
            .as_deref()
            .and_then(crate::providers::openai_compat::normalize_stt_language),
        vocabulary_prompt: if cfg.vocabulary_boost && !cfg.dictionary.is_empty() {
            Some(format!(
                "Preferred names, spellings, and domain terms: {}",
                cfg.dictionary.join(", ")
            ))
        } else {
            None
        },
        keyterms,
    }
}

async fn transcribe_from_config(cfg: &AppConfig, wav: &[u8]) -> OtoResult<String> {
    let context = transcription_context(cfg);
    match cfg.stt_backend {
        SttBackend::Cloud if is_deepgram(cfg) => {
            let client = deepgram_from_config_async(cfg).await?;
            client.transcribe(wav, &context).await
        }
        SttBackend::Cloud => {
            let client = client_from_config_async(cfg).await?;
            client.transcribe(wav, &context).await
        }
        SttBackend::LocalWhisper => {
            let client = LocalWhisperClient::new(cfg.local_whisper_model_path.clone())?;
            client.transcribe(wav, &context).await
        }
    }
}

/// Transcribe a WAV with the current global settings.
///
/// Used by history re-transcription and audio-file import, which have no
/// focused window to resolve a Mode against.
pub async fn transcribe_wav(wav: &[u8]) -> OtoResult<String> {
    let cfg = load_config()?;
    transcribe_from_config(&cfg, wav).await
}

/// Shortest capture worth sending to a provider (about a syllable).
const MIN_CAPTURE_MS: u64 = 250;
/// How often new audio is handed to a live streaming session. Deepgram accepts
/// anything from ~20 ms up; this trades a little latency for far fewer frames.
const STREAM_PUMP_INTERVAL: Duration = Duration::from_millis(120);
/// How often a hands-free session checks whether the speaker has stopped.
const VAD_POLL_INTERVAL: Duration = Duration::from_millis(150);
/// How long an insertion stays undoable. Undo deletes backwards from the caret,
/// which is only correct while nothing else has happened in that field.
const UNDO_WINDOW: Duration = Duration::from_secs(45);
/// Longest insertion undo will delete. Beyond this the backspace storm takes
/// long enough that the user would be typing over it.
const UNDO_MAX_CHARS: usize = 2_000;

/// The last text Oto put into an application, and the conditions under which
/// removing it again is still the right thing to do.
#[derive(Debug, Clone)]
struct LastInsertion {
    char_count: usize,
    at: Instant,
    /// Application the text went into; undo refuses if focus has moved on.
    app_class: Option<String>,
    /// Process to aim the deletions at, as injection does.
    pid: Option<i32>,
}

/// What a hotkey press should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PressAction {
    Start,
    Stop,
}

/// What a hotkey release should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReleaseAction {
    Stop,
    /// Leave the session running — it is hands-free from here.
    Ignore,
}

/// Press semantics for each activation mode.
fn press_action(mode: ActivationMode, listening: bool) -> PressAction {
    match mode {
        // A held key cannot be pressed again mid-session; `ptt_down` no-ops if
        // the state machine ever disagrees.
        ActivationMode::Hold => PressAction::Start,
        ActivationMode::Toggle | ActivationMode::Hybrid => {
            if listening {
                PressAction::Stop
            } else {
                PressAction::Start
            }
        }
    }
}

/// Release semantics for each activation mode.
fn release_action(
    mode: ActivationMode,
    listening: bool,
    held_ms: u64,
    tap_threshold_ms: u64,
) -> ReleaseAction {
    match mode {
        ActivationMode::Hold => ReleaseAction::Stop,
        ActivationMode::Toggle => ReleaseAction::Ignore,
        ActivationMode::Hybrid => {
            // The press that ended a toggled session also produces a release.
            // Reading that as a hold would try to stop an already-stopped run.
            if !listening {
                ReleaseAction::Ignore
            } else if held_ms >= tap_threshold_ms {
                ReleaseAction::Stop
            } else {
                ReleaseAction::Ignore
            }
        }
    }
}

/// Exclusive pipeline phase — only one session may run at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Listening,
    Processing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionMode {
    Dictation,
    Command,
}

impl SessionMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dictation => "dictation",
            Self::Command => "command",
        }
    }
}

struct Inner {
    recorder: Option<AudioRecorder>,
    phase: Phase,
    /// Bumped on new sessions / cancel so delayed work doesn't clobber later sessions.
    epoch: u64,
    /// Set on cancel; checked after awaits during processing.
    cancel_flag: bool,
    /// Last captured WAV bytes (for STT / test_transcription).
    last_wav: Option<Vec<u8>>,
    /// Sample rate of `last_wav`, needed to report its duration.
    last_sample_rate: u32,
    mode: SessionMode,
    selected_text: Option<String>,
    /// Window that should receive injected text (captured at PTT press).
    focus_target: Option<FocusTarget>,
    /// Configuration resolved once at session start.
    ///
    /// Read again at release time instead of reloading, so a settings change
    /// mid-dictation cannot make the second half of a session disagree with the
    /// first about the provider, the model, or where the text goes.
    resolved: Option<ResolvedConfig>,
    /// True while the activation hotkey is physically down.
    ///
    /// This, rather than the activation mode, is what tells VAD auto-stop
    /// whether the session is hands-free: a held key always ends on release.
    key_held: bool,
    /// When the current press began, for the hybrid tap/hold decision.
    press_started_at: Option<Instant>,
    /// Most recent successful insertion, for undo.
    last_insertion: Option<LastInsertion>,
}

pub struct Pipeline {
    app: AppHandle,
    inner: Mutex<Inner>,
    /// Live streaming session, when the provider supports one.
    ///
    /// Separate from `inner` because feeding it is async and a `std::sync`
    /// guard cannot be held across an await.
    stream: tokio::sync::Mutex<Option<Box<dyn SttStream>>>,
    /// How far into the recorder's buffer the stream has been fed.
    stream_cursor: AtomicUsize,
}

impl Pipeline {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            inner: Mutex::new(Inner {
                recorder: None,
                phase: Phase::Idle,
                epoch: 0,
                cancel_flag: false,
                last_wav: None,
                last_sample_rate: 0,
                mode: SessionMode::Dictation,
                selected_text: None,
                focus_target: None,
                resolved: None,
                key_held: false,
                press_started_at: None,
                last_insertion: None,
            }),
            stream: tokio::sync::Mutex::new(None),
            stream_cursor: AtomicUsize::new(0),
        }
    }

    /// Configuration for the session in flight, falling back to the stored
    /// config when there is no session (diagnostics, tray actions).
    fn session_config(&self) -> OtoResult<ResolvedConfig> {
        if let Ok(inner) = self.lock_inner() {
            if let Some(resolved) = inner.resolved.clone() {
                return Ok(resolved);
            }
        }
        Ok(ResolvedConfig::global(load_config()?))
    }

    fn play_cue(&self, cue: Cue) {
        let sounds = self
            .lock_inner()
            .ok()
            .and_then(|inner| inner.resolved.as_ref().map(|r| r.cfg.sounds.clone()))
            .or_else(|| load_config().ok().map(|cfg| cfg.sounds));
        if let Some(sounds) = sounds {
            cues::play(cue, &sounds);
        }
    }

    fn emit(&self, event: PipelineEvent) {
        let _ = self.app.emit("pipeline://event", event);
    }

    fn emit_state(&self, state: PipelineState) {
        self.emit(PipelineEvent::state(state, None));
    }

    /// Position overlay from config or bottom-center of the current monitor, then show.
    fn show_overlay(&self) {
        if let Some(w) = self.app.get_webview_window("overlay") {
            position_overlay(&w);
            let _ = w.set_always_on_top(true);
            let _ = w.set_skip_taskbar(true);
            // Never accept keyboard focus — synthetic typing must hit the dictation target.
            let _ = w.set_focusable(false);
            // Do not steal keyboard focus from the app the user is dictating into.
            if let Err(e) = w.show() {
                eprintln!("oto: overlay.show failed: {e}");
            } else {
                eprintln!("oto: overlay shown");
            }
            let _ = w.unminimize();
        } else {
            eprintln!("oto: overlay window missing");
        }
    }

    /// Hide overlay unless appearance is set to minimal dormant pill.
    fn hide_overlay(&self) {
        let keep = load_config()
            .map(|c| c.idle_behavior == IdleBehavior::Minimal)
            .unwrap_or(false);
        if keep {
            return;
        }
        if let Some(w) = self.app.get_webview_window("overlay") {
            let _ = w.hide();
        }
    }

    fn bump_epoch(&self) -> OtoResult<u64> {
        let mut inner = self.lock_inner()?;
        inner.epoch = inner.epoch.wrapping_add(1);
        Ok(inner.epoch)
    }

    /// True when phase is Idle (safe for appearance changes / new PTT).
    pub fn is_idle(&self) -> bool {
        self.lock_inner()
            .map(|g| g.phase == Phase::Idle)
            .unwrap_or(true)
    }

    /// True while actively capturing audio (between ptt_down and ptt_up).
    pub fn is_listening(&self) -> bool {
        self.lock_inner()
            .map(|g| g.phase == Phase::Listening)
            .unwrap_or(false)
    }

    fn listening_snapshot(&self, session_epoch: u64) -> OtoResult<Option<Vec<u8>>> {
        let inner = self.lock_inner()?;
        if inner.phase != Phase::Listening || inner.epoch != session_epoch {
            return Ok(None);
        }
        inner
            .recorder
            .as_ref()
            .map(AudioRecorder::snapshot_wav)
            .transpose()
            .map(Option::flatten)
    }

    /// True if this processing session was cancelled or superseded.
    fn session_aborted(&self, session_epoch: u64) -> bool {
        self.lock_inner()
            .map(|g| g.epoch != session_epoch || g.cancel_flag)
            .unwrap_or(true)
    }

    /// Mark phase Idle (best-effort) without bumping epoch.
    fn set_phase_idle(&self) {
        if let Ok(mut inner) = self.lock_inner() {
            inner.phase = Phase::Idle;
            inner.recorder = None;
            inner.selected_text = None;
            inner.resolved = None;
        }
        self.stream_cursor.store(0, Ordering::Relaxed);
    }

    /// Drop any live streaming session without waiting for its final transcript.
    async fn discard_stream(&self) {
        let mut guard = self.stream.lock().await;
        *guard = None;
        self.stream_cursor.store(0, Ordering::Relaxed);
    }

    /// True while this exact session is still capturing audio.
    fn listening_with_epoch(&self, session_epoch: u64) -> bool {
        self.lock_inner()
            .map(|g| g.phase == Phase::Listening && g.epoch == session_epoch)
            .unwrap_or(false)
    }

    /// Error state stays ~4s (or until cancel/dismiss), then idle.
    async fn finish_error(&self, message: String) {
        self.play_cue(Cue::Error);
        // Allow a new PTT immediately; error flash is non-exclusive.
        self.set_phase_idle();
        let epoch = self.bump_epoch().unwrap_or(0);
        self.emit(PipelineEvent::Error {
            message: message.clone(),
        });
        // Ensure overlay is visible for the error flash.
        self.show_overlay();
        sleep(Duration::from_secs(4)).await;
        // Skip if user dismissed or a new session started.
        let still = self
            .lock_inner()
            .map(|g| g.epoch == epoch && g.phase == Phase::Idle)
            .unwrap_or(false);
        if still {
            self.emit_state(PipelineState::Idle);
            self.hide_overlay();
        }
    }

    fn lock_inner(&self) -> OtoResult<std::sync::MutexGuard<'_, Inner>> {
        self.inner
            .lock()
            .map_err(|_| OtoError::Message("pipeline lock poisoned".into()))
    }

    /// Sample rate of the last captured WAV.
    fn last_sample_rate(&self) -> u32 {
        self.lock_inner().map(|g| g.last_sample_rate).unwrap_or(0)
    }

    /// Clone of the last captured WAV, if any.
    pub fn last_wav(&self) -> OtoResult<Option<Vec<u8>>> {
        let inner = self.lock_inner()?;
        Ok(inner.last_wav.clone())
    }

    /// Run STT on the last recorded buffer (settings "Test transcription").
    pub async fn transcribe_last(&self) -> OtoResult<String> {
        let wav = self
            .last_wav()?
            .ok_or_else(|| OtoError::Message("No audio yet — dictate first".into()))?;
        let cfg = load_config()?;
        transcribe_from_config(&cfg, &wav).await
    }

    /// Hotkey pressed. Dispatches on the configured activation mode.
    ///
    /// The hotkey backend always reports press and release; interpreting them is
    /// this layer's job. `mode_id` is set when the chord belongs to a Mode, which
    /// then applies regardless of which application has focus.
    pub async fn hotkey_down_for(&self, mode_id: Option<String>) -> OtoResult<()> {
        let activation = load_config()
            .map(|cfg| cfg.activation_mode)
            .unwrap_or_default();

        if let Ok(mut inner) = self.lock_inner() {
            inner.key_held = true;
            inner.press_started_at = Some(Instant::now());
        }

        match press_action(activation, self.is_listening()) {
            PressAction::Start => match mode_id {
                Some(mode_id) => self.ptt_down_for_mode(mode_id).await,
                None => self.ptt_down().await,
            },
            PressAction::Stop => self.ptt_up().await,
        }
    }

    /// Hotkey released.
    pub async fn hotkey_up(&self) -> OtoResult<()> {
        let cfg = load_config().unwrap_or_default();

        let held_ms = {
            let mut inner = match self.lock_inner() {
                Ok(inner) => inner,
                Err(_) => return Ok(()),
            };
            inner.key_held = false;
            inner
                .press_started_at
                .take()
                .map(|at| at.elapsed().as_millis() as u64)
                .unwrap_or(0)
        };

        match release_action(
            cfg.activation_mode,
            self.is_listening(),
            held_ms,
            cfg.effective_tap_threshold_ms(),
        ) {
            ReleaseAction::Stop => self.ptt_up().await,
            ReleaseAction::Ignore => Ok(()),
        }
    }

    pub async fn ptt_down(&self) -> OtoResult<()> {
        self.start_listening(SessionMode::Dictation, None).await
    }

    /// Start dictation under a specific Mode, bypassing window matching.
    pub async fn ptt_down_for_mode(&self, mode_id: String) -> OtoResult<()> {
        self.start_listening_with_mode(SessionMode::Dictation, None, Some(mode_id))
            .await
    }

    /// Assemble the polish context, including whatever the user's disclosure
    /// level permits about the target application.
    async fn polish_context(&self, cfg: &AppConfig) -> PolishContext {
        let target = self
            .lock_inner()
            .ok()
            .and_then(|inner| inner.focus_target.clone())
            .unwrap_or_default();

        // Only read the screen when the user asked for that level. The
        // Accessibility API is used rather than a simulated ⌘C so the clipboard
        // is never touched.
        let selection = if cfg.context_level >= ContextLevel::Selection {
            crate::injection::read_ax_selection()
        } else {
            None
        };

        let dictation_context = context::build(
            cfg.context_level,
            &target,
            &cfg.context_blocklist,
            selection.as_deref(),
        );
        if dictation_context.redacted {
            eprintln!("oto context: target application is on the never-describe list");
        }

        PolishContext {
            language: cfg.language.clone(),
            dictionary: cfg.dictionary.clone(),
            tone_hint: cfg.active_style_prompt(),
            app_context: dictation_context.prompt_line(),
        }
    }

    /// Start Command Mode after capturing the selected text in the focused app.
    /// Settings uses a short delay so the user can restore focus; tray uses zero.
    pub async fn command_down(&self, focus_delay_ms: u64) -> OtoResult<()> {
        if focus_delay_ms > 0 {
            sleep(Duration::from_millis(focus_delay_ms.min(5000))).await;
        }
        let selected = capture_selected_text().await?;
        self.start_listening(SessionMode::Command, Some(selected))
            .await
    }

    async fn start_listening(
        &self,
        mode: SessionMode,
        selected_text: Option<String>,
    ) -> OtoResult<()> {
        self.start_listening_with_mode(mode, selected_text, None)
            .await
    }

    /// `forced_mode_id` overrides window matching — used when a Mode is invoked
    /// through its own dedicated hotkey rather than by the focused application.
    async fn start_listening_with_mode(
        &self,
        mode: SessionMode,
        selected_text: Option<String>,
        forced_mode_id: Option<String>,
    ) -> OtoResult<()> {
        // Capture focus *before* showing the overlay so injection can restore it
        // after multi-second STT, even if Settings/overlay steals keyboard focus.
        let focus_target = capture_focus_target();
        eprintln!(
            "oto focus: captured name={:?} bundle={:?} pid={:?}",
            focus_target.name, focus_target.bundle_id, focus_target.pid
        );

        // Resolve configuration once, against the application we are dictating into.
        let context = AppContext::new(focus_target.class.clone(), focus_target.title.clone())
            .with_bundle_id(focus_target.bundle_id.clone());
        let resolved = match forced_mode_id.as_deref() {
            Some(mode_id) => load_config()?.resolve_mode(mode_id),
            None => load_config()?.resolve(Some(&context)),
        };
        let cfg = resolved.cfg.clone();
        if let Some(mode_id) = resolved.mode_id.as_deref() {
            eprintln!("oto mode: {} ({mode_id})", resolved.mode_name);
        }

        // A previous session's socket must never receive this session's audio.
        self.discard_stream().await;

        let session_epoch = {
            let mut inner = self.lock_inner()?;
            // Only start a new listen from Idle — reject if already Listening or Processing.
            if inner.phase != Phase::Idle {
                return Ok(());
            }
            // Invalidate any pending error timeout / leftover cancel from a previous take.
            inner.epoch = inner.epoch.wrapping_add(1);
            inner.cancel_flag = false;
            inner.phase = Phase::Listening;
            inner.mode = mode;
            inner.selected_text = selected_text;
            inner.focus_target = Some(focus_target);
            inner.resolved = Some(resolved);
            inner.epoch
        };

        cues::play(Cue::Start, &cfg.sounds);

        // Set the visible state at the press boundary. The overlay is prewarmed,
        // so emitting before show prevents a stale Processing frame on map.
        self.emit(PipelineEvent::state(
            PipelineState::Listening,
            (mode == SessionMode::Command).then(|| "Command mode".into()),
        ));
        self.show_overlay();

        // A first-run webview may still attach its listener after show. Retry only
        // while this session is listening so a quick release cannot be overwritten
        // by a late Listening event.
        {
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move {
                sleep(Duration::from_millis(80)).await;
                let still_listening = app
                    .try_state::<AppState>()
                    .map(|state| state.pipeline.is_listening())
                    .unwrap_or(false);
                if still_listening {
                    let _ = app.emit(
                        "pipeline://event",
                        PipelineEvent::state(
                            PipelineState::Listening,
                            (mode == SessionMode::Command).then(|| "Command mode".into()),
                        ),
                    );
                }
            });
        }

        match AudioRecorder::start(self.app.clone(), CaptureTuning::from_config(&cfg)) {
            Ok(recorder) => {
                if recorder.device_fell_back() {
                    self.emit(PipelineEvent::state(
                        PipelineState::Listening,
                        Some("Selected microphone unavailable — using the default".into()),
                    ));
                }
                let sample_rate = recorder.sample_rate();
                let mut inner = self.lock_inner()?;
                // Cancel or supersede may have happened while starting the device.
                if inner.phase != Phase::Listening || inner.cancel_flag {
                    return Ok(());
                }
                inner.recorder = Some(recorder);
                drop(inner);
                self.spawn_partial_loop(session_epoch);
                self.spawn_stream_session(session_epoch, cfg.clone(), sample_rate);
                self.spawn_vad_watchdog(session_epoch, cfg);
                Ok(())
            }
            Err(e) => {
                self.set_phase_idle();
                self.finish_error(e.to_string()).await;
                Err(e)
            }
        }
    }

    /// True when the pipeline should try a live streaming transcription.
    fn streaming_supported(cfg: &AppConfig) -> bool {
        cfg.streaming_enabled && cfg.stt_backend == SttBackend::Cloud && is_deepgram(cfg)
    }

    /// Open a streaming session and pump captured audio into it.
    ///
    /// Everything here is best-effort. The recorder keeps buffering regardless,
    /// so any failure simply leaves the batch upload in `ptt_up` to do the work.
    fn spawn_stream_session(&self, session_epoch: u64, cfg: AppConfig, sample_rate: u32) {
        if !Self::streaming_supported(&cfg) {
            return;
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let Some(pipeline) = app
                .try_state::<AppState>()
                .map(|state| state.pipeline.clone())
            else {
                return;
            };

            let ctx = transcription_context(&cfg);
            let client = match deepgram_from_config_async(&cfg).await {
                Ok(client) => client,
                Err(error) => {
                    eprintln!("oto stream: no Deepgram credentials, using batch upload: {error}");
                    return;
                }
            };
            let stream = match DeepgramStream::connect(&client, &ctx, sample_rate).await {
                Ok(stream) => stream,
                Err(error) => {
                    eprintln!("oto stream: connect failed, using batch upload: {error}");
                    return;
                }
            };

            // The press may already be over by the time the socket is up.
            if !pipeline.listening_with_epoch(session_epoch) {
                return;
            }
            {
                let mut guard = pipeline.stream.lock().await;
                *guard = Some(Box::new(stream));
            }
            pipeline.stream_cursor.store(0, Ordering::Relaxed);

            loop {
                sleep(STREAM_PUMP_INTERVAL).await;
                if !pipeline.listening_with_epoch(session_epoch) {
                    break;
                }
                if !pipeline.pump_stream_once().await {
                    break;
                }
            }
        });
    }

    /// Feed one batch of new audio to the live stream and publish any partial.
    /// Returns false when the stream is gone or has failed.
    async fn pump_stream_once(&self) -> bool {
        let mut guard = self.stream.lock().await;
        let Some(stream) = guard.as_mut() else {
            return false;
        };
        if stream.failed() {
            *guard = None;
            eprintln!("oto stream: transport failed — falling back to batch upload");
            return false;
        }

        let chunk = {
            let Ok(inner) = self.lock_inner() else {
                return false;
            };
            let Some(recorder) = inner.recorder.as_ref() else {
                return false;
            };
            let mut cursor = self.stream_cursor.load(Ordering::Relaxed);
            let chunk = recorder.drain_from(&mut cursor);
            self.stream_cursor.store(cursor, Ordering::Relaxed);
            chunk
        };

        if !chunk.is_empty() && stream.feed(&chunk).await.is_err() {
            *guard = None;
            return false;
        }
        if let Some(text) = stream.take_partial() {
            self.emit(PipelineEvent::Partial { text });
        }
        true
    }

    /// End a hands-free session once the speaker stops.
    ///
    /// Only fires while the hotkey is *not* held: a held key always ends on its
    /// own release, so applying VAD there would cut people off mid-sentence.
    fn spawn_vad_watchdog(&self, session_epoch: u64, cfg: AppConfig) {
        if !cfg.vad.auto_stop || cfg.activation_mode == ActivationMode::Hold {
            return;
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                sleep(VAD_POLL_INTERVAL).await;
                let Some(pipeline) = app
                    .try_state::<AppState>()
                    .map(|state| state.pipeline.clone())
                else {
                    return;
                };
                if !pipeline.listening_with_epoch(session_epoch) {
                    return;
                }

                let (key_held, snapshot) = {
                    let Ok(inner) = pipeline.lock_inner() else {
                        return;
                    };
                    let snapshot = inner
                        .recorder
                        .as_ref()
                        .map(AudioRecorder::vad_snapshot)
                        .unwrap_or_default();
                    (inner.key_held, snapshot)
                };
                if key_held {
                    continue;
                }

                // Prefer the provider's own endpointing when a stream is live —
                // it hears the audio the model actually transcribed, so it ends
                // on a real end-of-utterance rather than on room energy. Still
                // require the local minimum-speech guard so a stray noise burst
                // cannot finish an empty session.
                let provider_endpointed = pipeline
                    .stream
                    .lock()
                    .await
                    .as_ref()
                    .is_some_and(|stream| stream.endpointed());
                let enough_speech =
                    snapshot.speech_ms >= u64::from(cfg.vad.effective_min_speech_ms());

                if (provider_endpointed && enough_speech) || snapshot.should_auto_stop(&cfg.vad) {
                    eprintln!(
                        "oto vad: finishing hands-free session ({} ms trailing silence, provider endpointed: {provider_endpointed})",
                        snapshot.trailing_silence_ms
                    );
                    if let Err(error) = pipeline.ptt_up().await {
                        eprintln!("oto vad: auto-stop failed: {error}");
                    }
                    return;
                }
            }
        });
    }

    fn spawn_partial_loop(&self, session_epoch: u64) {
        let Ok(config) = load_config() else {
            return;
        };
        if !config.streaming_enabled || config.stt_backend != SttBackend::LocalWhisper {
            return;
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let mut previous = String::new();
            loop {
                sleep(Duration::from_millis(1800)).await;
                let Some(pipeline) = app
                    .try_state::<AppState>()
                    .map(|state| state.pipeline.clone())
                else {
                    break;
                };
                let wav = match pipeline.listening_snapshot(session_epoch) {
                    Ok(Some(wav)) => wav,
                    Ok(None) => break,
                    Err(error) => {
                        eprintln!("oto: live preview snapshot failed: {error}");
                        break;
                    }
                };
                match transcribe_from_config(&config, &wav).await {
                    Ok(text) if !text.trim().is_empty() && text != previous => {
                        if pipeline
                            .listening_snapshot(session_epoch)
                            .ok()
                            .flatten()
                            .is_none()
                        {
                            break;
                        }
                        previous = text.clone();
                        let _ = app.emit("pipeline://event", PipelineEvent::Partial { text });
                    }
                    Ok(_) => {}
                    Err(error) => {
                        // Preview failure must never abort the actual dictation.
                        eprintln!("oto: live local preview failed: {error}");
                        break;
                    }
                }
            }
        });
    }

    pub async fn ptt_up(&self) -> OtoResult<()> {
        let (recorder, session_epoch, mode, selected_text) = {
            let mut inner = self.lock_inner()?;
            if inner.phase != Phase::Listening {
                return Ok(());
            }
            // Capture epoch for this session; further work aborts if cancel bumps it.
            let session_epoch = inner.epoch;
            inner.phase = Phase::Processing;
            (
                inner.recorder.take(),
                session_epoch,
                inner.mode,
                inner.selected_text.take(),
            )
        };

        // Switch the overlay as soon as the hotkey is released. Finalizing the
        // recorder and network processing happen after the UI leaves Listening.
        self.emit_state(PipelineState::Processing);
        self.play_cue(Cue::Stop);

        // Hand the streaming session the tail of the audio before the recorder
        // is torn down, so the last word is not lost to the pump interval.
        if let Some(rec) = recorder.as_ref() {
            let mut cursor = self.stream_cursor.load(Ordering::Relaxed);
            let leftover = rec.drain_from(&mut cursor);
            self.stream_cursor.store(cursor, Ordering::Relaxed);
            if !leftover.is_empty() {
                let mut guard = self.stream.lock().await;
                if let Some(stream) = guard.as_mut() {
                    let _ = stream.feed(&leftover).await;
                }
            }
        }

        let wav = if let Some(rec) = recorder {
            match rec.stop() {
                Ok((wav, sample_rate)) => {
                    // Tiny captures are almost always accidental taps (no speech).
                    // Surface a clearer error than a failed remote STT call. Measure
                    // real duration: a byte threshold means very different lengths at
                    // 16 kHz and 48 kHz.
                    if wav_duration_ms(wav.len(), sample_rate) < MIN_CAPTURE_MS {
                        self.finish_error(
                            "Recording was too short — keep the hotkey active while speaking".into(),
                        )
                        .await;
                        return Ok(());
                    }
                    let mut inner = self.lock_inner()?;
                    inner.last_wav = Some(wav.clone());
                    inner.last_sample_rate = sample_rate;
                    wav
                }
                Err(e) => {
                    self.finish_error(e.to_string()).await;
                    return Err(e);
                }
            }
        } else {
            self.finish_error("No audio captured".into()).await;
            return Ok(());
        };

        if self.session_aborted(session_epoch) {
            self.set_phase_idle();
            return Ok(());
        }

        // Use the configuration this session started with, not whatever the
        // settings window holds now.
        let cfg = match self.session_config() {
            Ok(resolved) => resolved.cfg,
            Err(e) => {
                self.finish_error(e.to_string()).await;
                return Err(e);
            }
        };

        self.emit(PipelineEvent::Phase {
            phase: "transcribing".into(),
        });

        let mut text = match self.finalize_transcript(&cfg, &wav).await {
            Ok(t) => t,
            Err(e) => {
                if self.session_aborted(session_epoch) {
                    self.set_phase_idle();
                    return Ok(());
                }
                self.finish_error(e.to_string()).await;
                return Err(e);
            }
        };

        if self.session_aborted(session_epoch) {
            self.set_phase_idle();
            return Ok(());
        }

        if text.trim().is_empty() {
            self.finish_error("No speech detected".into()).await;
            return Ok(());
        }

        let raw_text = text.clone();
        if cfg.streaming_enabled {
            self.emit(PipelineEvent::Partial { text: text.clone() });
        }

        // Spoken edits run before anything else so a retraction is honoured
        // rather than paraphrased, and before snippet matching so "scratch
        // that" cannot end up inside a trigger comparison.
        if mode == SessionMode::Dictation && cfg.voice_edits_enabled {
            let edited = crate::features::voice_edits::apply_voice_edits(&text);
            if edited != text {
                eprintln!("oto: applied spoken edits");
                text = edited;
            }
            if text.trim().is_empty() {
                // The user retracted everything they said.
                self.finish_error("Nothing left after \"scratch that\"".into())
                    .await;
                return Ok(());
            }
        }

        let snippet_expanded = if mode == SessionMode::Dictation {
            if let Some(expansion) = expand_snippet(&text, &cfg.snippets).map(str::to_owned) {
                text = expansion;
                true
            } else {
                false
            }
        } else {
            false
        };

        if mode == SessionMode::Command {
            self.emit(PipelineEvent::Phase {
                phase: "rewriting selection".into(),
            });
            if is_deepgram(&cfg) {
                let message = "Command Mode needs an OpenAI-compatible LLM to rewrite the selection. Deepgram only handles speech-to-text — switch provider under Providers, or disable Command Mode.".into();
                let error = OtoError::Message(message);
                self.finish_error(error.to_string()).await;
                return Err(error);
            }
            let selected = selected_text
                .as_deref()
                .ok_or_else(|| OtoError::Message("Command Mode lost the selected text".into()))?;
            let client = match client_from_config_async(&cfg).await {
                Ok(client) => client,
                Err(error) => {
                    // Command Mode rewrites via the LLM provider; Local Whisper / Deepgram
                    // only cover STT. Surface a clearer message when the polish key is missing.
                    let message = command_mode_client_error(&error);
                    self.finish_error(message).await;
                    return Err(error);
                }
            };
            let ctx = self.polish_context(&cfg).await;
            text = match client.rewrite(selected, &text, &ctx).await {
                Ok(rewritten) => rewritten,
                Err(error) => {
                    self.finish_error(error.to_string()).await;
                    return Err(error);
                }
            };
        } else if cfg.polish_enabled && !snippet_expanded && is_deepgram(&cfg) {
            // Deepgram smart_format already punctuates; polish requires a chat LLM.
            self.emit(PipelineEvent::state(
                PipelineState::Processing,
                Some(
                    "Polish skipped: Deepgram is STT-only (smart_format is already applied). Use an OpenAI-compatible provider for LLM polish."
                        .into(),
                ),
            ));
        } else if cfg.polish_enabled && !snippet_expanded {
            self.emit(PipelineEvent::Phase {
                phase: "polishing".into(),
            });
            let client = match client_from_config_async(&cfg).await {
                Ok(client) => client,
                Err(error) => {
                    self.emit(PipelineEvent::state(
                        PipelineState::Processing,
                        Some(format!("Polish unavailable, using raw: {error}")),
                    ));
                    // Continue with raw transcription, matching polish-failure behavior.
                    return self
                        .finish_with_text(text, raw_text, mode, &cfg, session_epoch)
                        .await;
                }
            };
            let ctx = self.polish_context(&cfg).await;
            match client.polish(&text, &ctx).await {
                Ok(polished) => {
                    if self.session_aborted(session_epoch) {
                        self.set_phase_idle();
                        return Ok(());
                    }
                    text = polished;
                }
                Err(e) => {
                    if self.session_aborted(session_epoch) {
                        self.set_phase_idle();
                        return Ok(());
                    }
                    // Spec: fall back to raw + toast (do not abort pipeline).
                    self.emit(PipelineEvent::state(
                        PipelineState::Processing,
                        Some(format!("Polish failed, using raw: {e}")),
                    ));
                }
            }
        }

        if self.session_aborted(session_epoch) {
            self.set_phase_idle();
            return Ok(());
        }

        self.finish_with_text(text, raw_text, mode, &cfg, session_epoch)
            .await
    }

    /// Final transcript for a session: the live stream's result when one ran and
    /// produced text, otherwise a batch upload of the captured WAV.
    ///
    /// Streaming is treated as an optimisation, never a dependency. A dropped
    /// socket, an empty result, or a provider error all land on the same batch
    /// path that 0.1.0 used, so the words are never lost to a network hiccup.
    async fn finalize_transcript(&self, cfg: &AppConfig, wav: &[u8]) -> OtoResult<String> {
        let stream = self.stream.lock().await.take();
        self.stream_cursor.store(0, Ordering::Relaxed);

        if let Some(stream) = stream {
            match stream.finish().await {
                Ok(text) if !text.trim().is_empty() => return Ok(text),
                Ok(_) => {
                    eprintln!("oto stream: empty streaming result — retrying as batch upload");
                }
                Err(error) => {
                    eprintln!("oto stream: {error} — retrying as batch upload");
                }
            }
        }

        transcribe_from_config(cfg, wav).await
    }

    async fn finish_with_text(
        &self,
        text: String,
        raw_text: String,
        mode: SessionMode,
        cfg: &AppConfig,
        session_epoch: u64,
    ) -> OtoResult<()> {
        // Replacements are last: they are the user's final say on spelling, and
        // running them before cleanup would let the model undo them.
        let text = crate::features::replacements::apply_replacements(&text, &cfg.replacements);

        if cfg.history_enabled {
            let (audio, duration_ms) = if cfg.keep_history_audio {
                match self.last_wav() {
                    Ok(Some(wav)) => {
                        let duration = wav_duration_ms(wav.len(), self.last_sample_rate());
                        (Some(wav), duration)
                    }
                    _ => (None, 0),
                }
            } else {
                (None, 0)
            };
            if let Err(error) = history::append(
                raw_text,
                text.clone(),
                mode.as_str(),
                cfg.language.clone(),
                cfg.history_limit,
                audio.as_deref(),
                duration_ms,
            ) {
                eprintln!("oto: could not save history: {error}");
            }
        }

        self.emit(PipelineEvent::Phase {
            phase: "injecting".into(),
        });

        // PTT release can leave Ctrl/Shift held. Wait for the physical chord to
        // settle; inject also synthesizes key-ups before ⌘V.
        sleep(Duration::from_millis(500)).await;

        // Hide Settings so it cannot remain the key window and swallow ⌘V.
        // Keep the overlay non-focusable visual-only.
        if let Some(settings) = self.app.get_webview_window("settings") {
            let _ = settings.hide();
        }
        if let Some(overlay) = self.app.get_webview_window("overlay") {
            let _ = overlay.set_focusable(false);
        }
        // Yield so AppKit applies hide/activate ordering before we keystroke.
        sleep(Duration::from_millis(80)).await;

        let focus_target = {
            let mut inner = self.lock_inner()?;
            inner.focus_target.take()
        };

        let done_detail = match inject_text_to(
            &text,
            &cfg.injection_mode,
            focus_target.as_ref(),
        )
        .await
        {
            Ok(InjectResult::ClipboardOnly) => {
                if self.session_aborted(session_epoch) {
                    self.set_phase_idle();
                    return Ok(());
                }
                // Text is on clipboard; user pastes manually.
                "Copied — press ⌘V".to_string()
            }
            Ok(InjectResult::Pasted | InjectResult::Accessibility | InjectResult::DirectTyped) => {
                if self.session_aborted(session_epoch) {
                    self.set_phase_idle();
                    return Ok(());
                }
                // Remember what went in, so it can be taken back out.
                if let Ok(mut inner) = self.lock_inner() {
                    inner.last_insertion = Some(LastInsertion {
                        char_count: text.chars().count(),
                        at: Instant::now(),
                        app_class: focus_target.as_ref().and_then(|t| t.class.clone()),
                        pid: focus_target.as_ref().and_then(|t| t.pid),
                    });
                }
                // Surface the injected text (truncate long transcripts for overlay).
                if text.chars().count() > 120 {
                    let short: String = text.chars().take(117).collect();
                    format!("{short}…")
                } else {
                    text
                }
            }
            Err(e) => {
                if self.session_aborted(session_epoch) {
                    self.set_phase_idle();
                    return Ok(());
                }
                self.finish_error(format!("Injection failed: {e}")).await;
                return Err(e);
            }
        };

        if self.session_aborted(session_epoch) {
            self.set_phase_idle();
            return Ok(());
        }

        cues::play(Cue::Done, &cfg.sounds);
        self.emit(PipelineEvent::state(PipelineState::Done, Some(done_detail)));
        // Done flash ~700ms then idle.
        sleep(Duration::from_millis(700)).await;

        if self.session_aborted(session_epoch) {
            self.set_phase_idle();
            return Ok(());
        }

        {
            let mut inner = self.lock_inner()?;
            inner.phase = Phase::Idle;
        }
        self.emit_state(PipelineState::Idle);
        self.hide_overlay();
        Ok(())
    }

    /// Remove the text Oto last inserted.
    ///
    /// Deliberately narrow. Deleting backwards from the caret is only correct
    /// while the caret is still where the insertion left it, so this refuses
    /// once the insertion is stale, once focus has moved to another
    /// application, or when the text is long enough that the user has certainly
    /// carried on working.
    pub async fn undo_last_insertion(&self) -> OtoResult<String> {
        if !self.is_idle() {
            return Err(OtoError::Message(
                "Finish or cancel the current dictation first".into(),
            ));
        }

        let last = {
            let inner = self.lock_inner()?;
            inner.last_insertion.clone()
        }
        .ok_or_else(|| OtoError::Message("Nothing to undo yet".into()))?;

        if last.at.elapsed() > UNDO_WINDOW {
            return Err(OtoError::Message(
                "That insertion is too old to undo safely — the cursor has probably moved".into(),
            ));
        }
        if last.char_count == 0 {
            return Err(OtoError::Message("Nothing to undo yet".into()));
        }
        if last.char_count > UNDO_MAX_CHARS {
            return Err(OtoError::Message(format!(
                "That insertion is {} characters — too long to remove safely. Use the application's own undo.",
                last.char_count
            )));
        }

        let current = capture_focus_target();
        if !same_application(last.app_class.as_deref(), current.class.as_deref()) {
            return Err(OtoError::Message(
                "Undo only works in the application the text went into — focus it and try again"
                    .into(),
            ));
        }

        crate::injection::simulate_backspace(last.char_count, last.pid)?;
        // One insertion, one undo: a second press must not eat what came before.
        if let Ok(mut inner) = self.lock_inner() {
            inner.last_insertion = None;
        }
        Ok(format!("Removed {} characters", last.char_count))
    }

    pub async fn cancel(&self) -> OtoResult<()> {
        {
            let mut inner = self.lock_inner()?;
            inner.recorder = None;
            inner.phase = Phase::Idle;
            inner.selected_text = None;
            inner.focus_target = None;
            inner.resolved = None;
            inner.press_started_at = None;
            inner.cancel_flag = true;
            // Invalidate pending error auto-dismiss and in-flight processing.
            inner.epoch = inner.epoch.wrapping_add(1);
        }
        // Abandon the socket rather than waiting for a transcript nobody wants.
        self.discard_stream().await;
        self.emit_state(PipelineState::Idle);
        self.hide_overlay();
        Ok(())
    }
}

/// Whether two focus snapshots describe the same application.
///
/// An unknown application on either side is treated as a match: without
/// Accessibility permission macOS reports nothing, and refusing undo there would
/// be worse than trusting a user who just asked for it.
fn same_application(recorded: Option<&str>, current: Option<&str>) -> bool {
    match (recorded, current) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => true,
    }
}

/// Playback length of a 16-bit mono WAV, ignoring the 44-byte canonical header.
fn wav_duration_ms(byte_len: usize, sample_rate: u32) -> u64 {
    const HEADER: usize = 44;
    const BYTES_PER_SAMPLE: usize = 2;
    if sample_rate == 0 || byte_len <= HEADER {
        return 0;
    }
    let samples = (byte_len - HEADER) / BYTES_PER_SAMPLE;
    (samples as u64 * 1_000) / sample_rate as u64
}

/// Apply saved overlay position, or place bottom-center on the current monitor.
pub fn position_overlay(w: &tauri::WebviewWindow) {
    let cfg = load_config().ok();
    // Treat (0, 0) as unset — Moved events often fire with that before layout.
    if let Some(cfg) = cfg.as_ref() {
        if let (Some(x), Some(y)) = (cfg.overlay_x, cfg.overlay_y) {
            if !(x == 0 && y == 0) {
                let _ = w.set_position(PhysicalPosition::new(x, y));
                return;
            }
        }
    }

    // Best-effort bottom-center of the monitor the window is on (or primary).
    let monitor = w
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| w.primary_monitor().ok().flatten());

    if let Some(monitor) = monitor {
        let screen = monitor.size();
        let origin = monitor.position();
        let win = w.outer_size().unwrap_or(tauri::PhysicalSize::new(320, 72));
        let margin_bottom = 96i32;
        let x = origin.x + (screen.width as i32 - win.width as i32) / 2;
        let y = origin.y + screen.height as i32 - win.height as i32 - margin_bottom;
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_duration_ignores_header_and_scales_with_rate() {
        // 44-byte header + 16000 samples of 16-bit mono at 16 kHz == 1s.
        assert_eq!(wav_duration_ms(44 + 16_000 * 2, 16_000), 1_000);
        assert_eq!(wav_duration_ms(44 + 48_000 * 2, 48_000), 1_000);
        // Header-only and degenerate inputs must not panic or divide by zero.
        assert_eq!(wav_duration_ms(44, 48_000), 0);
        assert_eq!(wav_duration_ms(10, 48_000), 0);
        assert_eq!(wav_duration_ms(44 + 1_000, 0), 0);
    }

    #[test]
    fn short_tap_is_rejected_but_a_held_key_is_not() {
        // The old byte-count guard (1024 bytes) accepted a 10 ms tap at 48 kHz.
        let tap = 44 + (48_000 * 10 / 1_000) * 2;
        assert!(wav_duration_ms(tap, 48_000) < MIN_CAPTURE_MS);
        let spoken = 44 + (48_000 * 600 / 1_000) * 2;
        assert!(wav_duration_ms(spoken, 48_000) >= MIN_CAPTURE_MS);
    }

    #[test]
    fn undo_matches_applications_case_insensitively() {
        assert!(same_application(Some("Safari"), Some("safari")));
        assert!(!same_application(Some("Safari"), Some("Terminal")));
    }

    #[test]
    fn undo_allows_an_unknown_application_rather_than_refusing_everywhere() {
        // Without Accessibility permission macOS reports no frontmost app.
        assert!(same_application(None, Some("Safari")));
        assert!(same_application(Some("Safari"), None));
        assert!(same_application(None, None));
    }

    #[test]
    fn hold_mode_is_unchanged_press_starts_release_stops() {
        assert_eq!(press_action(ActivationMode::Hold, false), PressAction::Start);
        assert_eq!(
            release_action(ActivationMode::Hold, true, 5_000, 350),
            ReleaseAction::Stop
        );
        // Even a very short hold still ends on release — no tap threshold here.
        assert_eq!(
            release_action(ActivationMode::Hold, true, 10, 350),
            ReleaseAction::Stop
        );
    }

    #[test]
    fn toggle_mode_starts_and_stops_on_presses_only() {
        assert_eq!(
            press_action(ActivationMode::Toggle, false),
            PressAction::Start
        );
        assert_eq!(press_action(ActivationMode::Toggle, true), PressAction::Stop);
        // Releasing must never end a toggled session, however long it was held.
        assert_eq!(
            release_action(ActivationMode::Toggle, true, 10, 350),
            ReleaseAction::Ignore
        );
        assert_eq!(
            release_action(ActivationMode::Toggle, true, 9_000, 350),
            ReleaseAction::Ignore
        );
    }

    #[test]
    fn hybrid_tap_leaves_the_session_running_but_a_hold_ends_it() {
        assert_eq!(
            press_action(ActivationMode::Hybrid, false),
            PressAction::Start
        );
        // Quick tap: keep listening hands-free.
        assert_eq!(
            release_action(ActivationMode::Hybrid, true, 120, 350),
            ReleaseAction::Ignore
        );
        // Held past the threshold: behave like push-to-talk.
        assert_eq!(
            release_action(ActivationMode::Hybrid, true, 351, 350),
            ReleaseAction::Stop
        );
        assert_eq!(
            release_action(ActivationMode::Hybrid, true, 2_000, 350),
            ReleaseAction::Stop
        );
    }

    #[test]
    fn hybrid_second_tap_stops_and_its_release_is_not_a_second_stop() {
        // Tap two ends the session …
        assert_eq!(press_action(ActivationMode::Hybrid, true), PressAction::Stop);
        // … and the release that follows it must be inert, because the pipeline
        // has already left Listening. Without the `listening` guard this reads
        // as a hold and tries to stop a session that is mid-transcription.
        assert_eq!(
            release_action(ActivationMode::Hybrid, false, 400, 350),
            ReleaseAction::Ignore
        );
    }
}
