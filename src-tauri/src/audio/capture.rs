//! Microphone capture via cpal. Public API is consumed by the pipeline orchestrator.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use tauri::{AppHandle, Emitter};

use crate::audio::devices::open_input_device;
use crate::audio::vad::{rms, VadSnapshot, VadTracker};
use crate::audio::wav::write_wav_i16_mono;
use crate::config::AppConfig;
use crate::error::{OtoError, OtoResult};
use crate::pipeline::events::PipelineEvent;

/// Input conditioning for one capture session, resolved from config up front so
/// the realtime callback never touches the config store.
#[derive(Debug, Clone)]
pub struct CaptureTuning {
    pub device_name: Option<String>,
    pub gain: f32,
    pub noise_gate: bool,
    pub gate_threshold: f32,
}

impl Default for CaptureTuning {
    fn default() -> Self {
        Self {
            device_name: None,
            gain: 1.0,
            noise_gate: false,
            gate_threshold: 0.02,
        }
    }
}

impl CaptureTuning {
    pub fn from_config(cfg: &AppConfig) -> Self {
        Self {
            device_name: cfg.audio.input_device.clone(),
            gain: cfg.audio.effective_gain(),
            noise_gate: cfg.audio.noise_gate,
            gate_threshold: cfg.audio.effective_gate_threshold(),
        }
    }
}

/// Level a gated chunk is attenuated to. Not zero: hard-muting at chunk
/// boundaries introduces steps that both click and confuse the STT front end.
const GATE_FLOOR: f32 = 0.2;

/// Everything the realtime callbacks share. Bundled so the per-format callbacks
/// take one argument instead of six.
struct CaptureShared {
    samples: Mutex<Vec<i16>>,
    meter: Mutex<LevelMeter>,
    vad: Mutex<VadTracker>,
    gain: f32,
    noise_gate: bool,
    gate_threshold: f32,
    app: AppHandle,
}

impl CaptureShared {
    /// Apply gain, update the VAD, gate, meter, and store — in that order.
    ///
    /// The VAD sees the pre-gate signal so its noise floor learns the actual
    /// room; gating first would starve it and it would never re-arm.
    fn ingest(&self, chunk: &mut Vec<i16>) {
        if chunk.is_empty() {
            return;
        }

        if (self.gain - 1.0).abs() > f32::EPSILON {
            for sample in chunk.iter_mut() {
                let scaled = f32::from(*sample) * self.gain;
                *sample = scaled.clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
            }
        }

        let level = rms(chunk);
        let gate_level = match self.vad.lock() {
            Ok(mut vad) => {
                vad.push(chunk);
                vad.gate_level(self.gate_threshold)
            }
            Err(_) => self.gate_threshold,
        };

        if self.noise_gate && level < gate_level {
            for sample in chunk.iter_mut() {
                *sample = (f32::from(*sample) * GATE_FLOOR) as i16;
            }
        }

        emit_level(&self.app, &self.meter, chunk);
        if let Ok(mut buf) = self.samples.lock() {
            buf.extend_from_slice(chunk);
        }
    }
}

/// Live microphone capture session. Dropping or calling [`Self::stop`] ends the stream.
#[allow(dead_code)] // constructed by pipeline (Task 9+)
pub struct AudioRecorder {
    stream: Option<Stream>,
    shared: Arc<CaptureShared>,
    sample_rate: u32,
    /// True when a configured device was missing and the default was used.
    device_fell_back: bool,
}

impl AudioRecorder {
    /// Open the configured (or default) input device and start recording.
    ///
    /// Prefers mono when the device accepts a 1-channel stream; otherwise records
    /// multi-channel and downmixes to mono in the callback. Uses the device default
    /// sample rate (written into the WAV header; Whisper accepts common rates).
    pub fn start(app: AppHandle, tuning: CaptureTuning) -> OtoResult<Self> {
        let (device, device_fell_back) = open_input_device(tuning.device_name.as_deref())?;

        let supported = device
            .default_input_config()
            .map_err(|e| OtoError::Message(format!("default input config: {e}")))?;

        let sample_format = supported.sample_format();
        let sample_rate = supported.sample_rate(); // cpal 0.18: SampleRate = u32
        let native_channels = supported.channels();

        let mut last_err = OtoError::Message("failed to open input stream".into());

        // Prefer mono; fall back to native channel count if mono fails to build.
        let attempts: Vec<(u16, StreamConfig)> = if native_channels == 1 {
            vec![(
                1,
                StreamConfig {
                    channels: 1,
                    sample_rate,
                    buffer_size: cpal::BufferSize::Default,
                },
            )]
        } else {
            vec![
                (
                    1,
                    StreamConfig {
                        channels: 1,
                        sample_rate,
                        buffer_size: cpal::BufferSize::Default,
                    },
                ),
                (
                    native_channels,
                    StreamConfig {
                        channels: native_channels,
                        sample_rate,
                        buffer_size: cpal::BufferSize::Default,
                    },
                ),
            ]
        };

        for (channels, config) in attempts {
            match Self::try_build_stream(
                &device,
                config,
                sample_format,
                channels,
                app.clone(),
                &tuning,
                sample_rate,
            ) {
                Ok((stream, shared)) => {
                    stream
                        .play()
                        .map_err(|e| OtoError::Message(format!("stream play: {e}")))?;
                    return Ok(Self {
                        stream: Some(stream),
                        shared,
                        sample_rate,
                        device_fell_back,
                    });
                }
                Err(e) => last_err = e,
            }
        }

        Err(last_err)
    }

    #[allow(clippy::too_many_arguments)]
    fn try_build_stream(
        device: &cpal::Device,
        config: StreamConfig,
        sample_format: SampleFormat,
        channels: u16,
        app: AppHandle,
        tuning: &CaptureTuning,
        sample_rate: u32,
    ) -> OtoResult<(Stream, Arc<CaptureShared>)> {
        let shared = Arc::new(CaptureShared {
            samples: Mutex::new(Vec::<i16>::new()),
            // One meter per stream: throttles level events without losing peaks.
            meter: Mutex::new(LevelMeter::new()),
            vad: Mutex::new(VadTracker::new(sample_rate)),
            gain: tuning.gain,
            noise_gate: tuning.noise_gate,
            gate_threshold: tuning.gate_threshold,
            app,
        });
        let err_fn = |err| eprintln!("audio input stream error: {err}");

        // Carry incomplete multi-channel frames across callbacks so we never
        // silently drop `data.len() % channels` trailing samples.
        let remainder_f32 = Arc::new(Mutex::new(Vec::<f32>::new()));
        let remainder_i16 = Arc::new(Mutex::new(Vec::<i16>::new()));
        let remainder_u16 = Arc::new(Mutex::new(Vec::<u16>::new()));

        let stream = match sample_format {
            SampleFormat::F32 => {
                let shared_cb = Arc::clone(&shared);
                let remainder = Arc::clone(&remainder_f32);
                device
                    .build_input_stream(
                        config,
                        move |data: &[f32], _| {
                            process_f32(data, channels, &shared_cb, &remainder);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| OtoError::Message(format!("build input stream (f32): {e}")))?
            }
            SampleFormat::I16 => {
                let shared_cb = Arc::clone(&shared);
                let remainder = Arc::clone(&remainder_i16);
                device
                    .build_input_stream(
                        config,
                        move |data: &[i16], _| {
                            process_i16(data, channels, &shared_cb, &remainder);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| OtoError::Message(format!("build input stream (i16): {e}")))?
            }
            SampleFormat::U16 => {
                let shared_cb = Arc::clone(&shared);
                let remainder = Arc::clone(&remainder_u16);
                device
                    .build_input_stream(
                        config,
                        move |data: &[u16], _| {
                            process_u16(data, channels, &shared_cb, &remainder);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| OtoError::Message(format!("build input stream (u16): {e}")))?
            }
            other => {
                return Err(OtoError::Message(format!(
                    "unsupported sample format: {other:?}"
                )));
            }
        };

        Ok((stream, shared))
    }

    /// Stop capture, encode mono PCM as WAV, and return `(wav_bytes, sample_rate)`.
    pub fn stop(mut self) -> OtoResult<(Vec<u8>, u32)> {
        // Drop the stream to stop the callback before reading samples.
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
            drop(stream);
        }

        let mono = self
            .shared
            .samples
            .lock()
            .map_err(|_| OtoError::Message("sample buffer poisoned".into()))?
            .clone();

        let wav = write_wav_i16_mono(&mono, self.sample_rate)?;
        Ok((wav, self.sample_rate))
    }

    /// Encode a point-in-time copy without stopping capture. Used for local
    /// Whisper previews while the user is still speaking.
    pub fn snapshot_wav(&self) -> OtoResult<Option<Vec<u8>>> {
        let mono = self
            .shared
            .samples
            .lock()
            .map_err(|_| OtoError::Message("sample buffer poisoned".into()))?
            .clone();
        // Avoid expensive/local low-quality inference on less than one second.
        if mono.len() < self.sample_rate as usize {
            return Ok(None);
        }
        Ok(Some(write_wav_i16_mono(&mono, self.sample_rate)?))
    }

    /// Copy samples recorded since `cursor`, advancing it.
    ///
    /// This is how streaming STT reads the tap without the capture callback
    /// needing to know a session exists.
    pub fn drain_from(&self, cursor: &mut usize) -> Vec<i16> {
        let Ok(buf) = self.shared.samples.lock() else {
            return Vec::new();
        };
        if *cursor >= buf.len() {
            // A shorter buffer than the cursor would mean the buffer was reset;
            // resynchronize rather than panicking on the slice.
            *cursor = buf.len();
            return Vec::new();
        }
        let chunk = buf[*cursor..].to_vec();
        *cursor = buf.len();
        chunk
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// True when the configured device was unavailable and the default was used.
    pub fn device_fell_back(&self) -> bool {
        self.device_fell_back
    }

    /// Current voice-activity verdict.
    pub fn vad_snapshot(&self) -> VadSnapshot {
        self.shared
            .vad
            .lock()
            .map(|vad| vad.snapshot())
            .unwrap_or_default()
    }

    /// Samples captured so far. Used to reject empty hands-free sessions.
    pub fn captured_samples(&self) -> usize {
        self.shared.samples.lock().map(|b| b.len()).unwrap_or(0)
    }
}

/// Minimum spacing between waveform level events.
///
/// A capture callback fires every few milliseconds; forwarding each one meant
/// ~50 IPC messages per second to every webview, each triggering a canvas
/// redraw, for a meter that only shows seven bars.
const LEVEL_EMIT_INTERVAL: Duration = Duration::from_millis(50);

/// Accumulates RMS energy between emits so throttling cannot swallow loud peaks.
#[derive(Debug)]
struct LevelMeter {
    sum_sq: f64,
    count: usize,
    last_emit: Option<Instant>,
}

impl LevelMeter {
    fn new() -> Self {
        Self {
            sum_sq: 0.0,
            count: 0,
            last_emit: None,
        }
    }

    /// Fold in a chunk and return a level when the emit interval has elapsed.
    fn push(&mut self, samples: &[i16], now: Instant) -> Option<f32> {
        for &sample in samples {
            let value = f64::from(sample);
            self.sum_sq += value * value;
        }
        self.count += samples.len();
        if self.count == 0 {
            return None;
        }
        let due = match self.last_emit {
            Some(last) => now.duration_since(last) >= LEVEL_EMIT_INTERVAL,
            None => true,
        };
        if !due {
            return None;
        }
        let rms = (self.sum_sq / self.count as f64).sqrt();
        self.sum_sq = 0.0;
        self.count = 0;
        self.last_emit = Some(now);
        Some((rms / 32768.0).clamp(0.0, 1.0) as f32)
    }
}

fn emit_level(app: &AppHandle, meter: &Mutex<LevelMeter>, samples: &[i16]) {
    if samples.is_empty() {
        return;
    }
    let level = match meter.lock() {
        Ok(mut meter) => meter.push(samples, Instant::now()),
        Err(_) => return,
    };
    if let Some(level) = level {
        let _ = app.emit("pipeline://event", PipelineEvent::Level { level });
    }
}

/// Convert interleaved multi-channel frames to mono i16, then hand to the shared
/// conditioning path.
fn process_f32(
    data: &[f32],
    channels: u16,
    shared: &Arc<CaptureShared>,
    remainder: &Arc<Mutex<Vec<f32>>>,
) {
    let ch = channels.max(1) as usize;
    let mut pending = match remainder.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    pending.extend_from_slice(data);
    let usable = (pending.len() / ch) * ch;
    let frames = &pending[..usable];
    let mut chunk = Vec::with_capacity(usable / ch);
    if ch == 1 {
        for &s in frames {
            chunk.push(f32_to_i16(s));
        }
    } else {
        for frame in frames.chunks_exact(ch) {
            let sum: f32 = frame.iter().sum();
            chunk.push(f32_to_i16(sum / ch as f32));
        }
    }
    pending.drain(..usable);
    drop(pending);
    shared.ingest(&mut chunk);
}

fn process_i16(
    data: &[i16],
    channels: u16,
    shared: &Arc<CaptureShared>,
    remainder: &Arc<Mutex<Vec<i16>>>,
) {
    let ch = channels.max(1) as usize;
    let mut pending = match remainder.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    pending.extend_from_slice(data);
    let usable = (pending.len() / ch) * ch;
    let frames = &pending[..usable];
    let mut chunk = Vec::with_capacity(usable / ch);
    if ch == 1 {
        chunk.extend_from_slice(frames);
    } else {
        for frame in frames.chunks_exact(ch) {
            let sum: i32 = frame.iter().map(|&s| s as i32).sum();
            chunk.push((sum / ch as i32) as i16);
        }
    }
    pending.drain(..usable);
    drop(pending);
    shared.ingest(&mut chunk);
}

fn process_u16(
    data: &[u16],
    channels: u16,
    shared: &Arc<CaptureShared>,
    remainder: &Arc<Mutex<Vec<u16>>>,
) {
    let ch = channels.max(1) as usize;
    let mut pending = match remainder.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    pending.extend_from_slice(data);
    let usable = (pending.len() / ch) * ch;
    let frames = &pending[..usable];
    let mut chunk = Vec::with_capacity(usable / ch);
    if ch == 1 {
        for &s in frames {
            chunk.push(u16_to_i16(s));
        }
    } else {
        for frame in frames.chunks_exact(ch) {
            let sum: i32 = frame.iter().map(|&s| u16_to_i16(s) as i32).sum();
            chunk.push((sum / ch as i32) as i16);
        }
    }
    pending.drain(..usable);
    drop(pending);
    shared.ingest(&mut chunk);
}

#[inline]
fn f32_to_i16(s: f32) -> i16 {
    let clamped = s.clamp(-1.0, 1.0);
    (clamped * i16::MAX as f32) as i16
}

#[inline]
fn u16_to_i16(s: u16) -> i16 {
    // Center u16 around zero into i16 range.
    (s as i32 - 32768) as i16
}

/// Apply gain the same way the capture path does. Extracted for testing, since
/// [`CaptureShared`] needs a live `AppHandle`.
#[cfg(test)]
fn apply_gain(chunk: &mut [i16], gain: f32) {
    for sample in chunk.iter_mut() {
        let scaled = f32::from(*sample) * gain;
        *sample = scaled.clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_emits_first_chunk_then_throttles() {
        let mut meter = LevelMeter::new();
        let start = Instant::now();
        let loud = vec![16_384i16; 480];
        assert!(meter.push(&loud, start).is_some(), "first chunk sets the baseline");
        assert!(
            meter.push(&loud, start + Duration::from_millis(10)).is_none(),
            "callbacks inside the interval must not emit"
        );
        assert!(meter
            .push(&loud, start + LEVEL_EMIT_INTERVAL)
            .is_some());
    }

    #[test]
    fn throttled_chunks_still_contribute_to_the_level() {
        let mut meter = LevelMeter::new();
        let start = Instant::now();
        // Consume the initial emit so the next one covers the accumulated window.
        meter.push(&[0i16; 480], start);
        // Silence, then a loud burst that lands inside the throttle window.
        meter.push(&[0i16; 480], start + Duration::from_millis(10));
        meter.push(&vec![32_000i16; 480], start + Duration::from_millis(20));
        let level = meter
            .push(&[0i16; 480], start + LEVEL_EMIT_INTERVAL)
            .expect("interval elapsed");
        assert!(level > 0.3, "peak must survive throttling, got {level}");
        assert!(level <= 1.0);
    }

    #[test]
    fn meter_level_stays_in_unit_range() {
        let mut meter = LevelMeter::new();
        let level = meter
            .push(&[i16::MIN, i16::MAX], Instant::now())
            .expect("first push emits");
        assert!((0.0..=1.0).contains(&level));
    }

    #[test]
    fn gain_saturates_instead_of_wrapping() {
        // Naive `as i16` on an overflowing f32 wraps a loud sample to a loud
        // sample of the opposite sign, which sounds like a hard click.
        let mut chunk = [20_000i16, -20_000, 100];
        apply_gain(&mut chunk, 4.0);
        assert_eq!(chunk[0], i16::MAX);
        assert_eq!(chunk[1], i16::MIN);
        assert_eq!(chunk[2], 400);
    }

    #[test]
    fn gain_below_one_attenuates() {
        let mut chunk = [10_000i16, -10_000];
        apply_gain(&mut chunk, 0.5);
        assert_eq!(chunk, [5_000, -5_000]);
    }

    #[test]
    fn tuning_clamps_hostile_config_values() {
        let cfg = AppConfig {
            audio: crate::config::AudioConfig {
                input_gain: 1_000.0,
                noise_gate_threshold: f32::NAN,
                ..Default::default()
            },
            ..AppConfig::default()
        };
        let tuning = CaptureTuning::from_config(&cfg);
        assert_eq!(tuning.gain, 4.0);
        assert!((tuning.gate_threshold - 0.02).abs() < f32::EPSILON);
    }
}
