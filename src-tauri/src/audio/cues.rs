//! Short synthesized audio cues for session transitions.
//!
//! Rendered rather than shipped as assets: a sine with a raised-cosine envelope
//! is a few lines, stays crisp at any output rate, and keeps tuning (pitch,
//! length, volume) in code instead of in binary files.

use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::config::SoundConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Start,
    Stop,
    Done,
    Error,
}

/// A single sine segment of the cue.
struct Segment {
    freq_start: f32,
    freq_end: f32,
    ms: u32,
    gain: f32,
}

fn segments(cue: Cue) -> Vec<Segment> {
    match cue {
        // Rising: "we're open".
        Cue::Start => vec![Segment { freq_start: 620.0, freq_end: 880.0, ms: 90, gain: 1.0 }],
        // Falling: the mirror of Start.
        Cue::Stop => vec![Segment { freq_start: 880.0, freq_end: 620.0, ms: 90, gain: 0.9 }],
        // Two quick blips, a major third apart.
        Cue::Done => vec![
            Segment { freq_start: 880.0, freq_end: 880.0, ms: 55, gain: 0.8 },
            Segment { freq_start: 0.0, freq_end: 0.0, ms: 30, gain: 0.0 },
            Segment { freq_start: 1_108.0, freq_end: 1_108.0, ms: 70, gain: 0.8 },
        ],
        // Low and slightly longer, so it reads as "wrong" without being harsh.
        Cue::Error => vec![
            Segment { freq_start: 340.0, freq_end: 300.0, ms: 160, gain: 1.0 },
            Segment { freq_start: 0.0, freq_end: 0.0, ms: 40, gain: 0.0 },
            Segment { freq_start: 300.0, freq_end: 260.0, ms: 160, gain: 1.0 },
        ],
    }
}

/// Fade applied to each end of a segment. Anything shorter clicks audibly.
const FADE_MS: f32 = 7.0;

/// Render a cue to mono f32 samples in -1.0..=1.0.
pub fn render(cue: Cue, sample_rate: u32, volume: f32) -> Vec<f32> {
    let rate = sample_rate.max(1) as f32;
    let volume = volume.clamp(0.0, 1.0);
    let mut out = Vec::new();
    // Carried across segments so a frequency sweep has no phase discontinuity.
    let mut phase = 0.0f32;

    for segment in segments(cue) {
        let frames = ((segment.ms as f32 / 1_000.0) * rate).round().max(1.0) as usize;
        let fade = ((FADE_MS / 1_000.0) * rate).max(1.0);
        for i in 0..frames {
            let t = i as f32 / frames as f32;
            let freq = segment.freq_start + (segment.freq_end - segment.freq_start) * t;
            phase += TAU * freq / rate;
            if phase > TAU {
                phase -= TAU;
            }
            // Raised-cosine in and out, flat in the middle.
            let pos = i as f32;
            let remaining = (frames - i) as f32;
            let envelope = (pos / fade).min(remaining / fade).clamp(0.0, 1.0);
            let envelope = 0.5 - 0.5 * (envelope * std::f32::consts::PI).cos();
            out.push(phase.sin() * envelope * segment.gain * volume);
        }
    }
    out
}

/// Play a cue without blocking the caller. Failures are silent by design — a
/// missing sound card must never interrupt a dictation.
pub fn play(cue: Cue, cfg: &SoundConfig) {
    if !cfg.enabled {
        return;
    }
    let wanted = match cue {
        Cue::Start => cfg.on_start,
        Cue::Stop => cfg.on_stop,
        Cue::Done => cfg.on_done,
        Cue::Error => cfg.on_error,
    };
    if !wanted {
        return;
    }
    let volume = cfg.effective_volume();
    if volume <= 0.0 {
        return;
    }

    // cpal's Stream is not Send, so it must be built, played, and dropped on one
    // thread. A detached thread also keeps the ~200 ms playback off the pipeline.
    std::thread::spawn(move || {
        if let Err(error) = play_blocking(cue, volume) {
            eprintln!("oto audio: cue playback skipped: {error}");
        }
    });
}

fn play_blocking(cue: Cue, volume: f32) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no default output device".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("default output config: {e}"))?;

    let sample_rate = supported.sample_rate();
    let channels = supported.channels().max(1) as usize;
    let samples = render(cue, sample_rate, volume);
    let total_frames = samples.len();
    if total_frames == 0 {
        return Ok(());
    }

    let cursor = Arc::new(std::sync::Mutex::new(0usize));
    let finished = Arc::new(AtomicBool::new(false));
    let config: cpal::StreamConfig = supported.clone().into();
    let err_fn = |err| eprintln!("oto audio: cue stream error: {err}");

    // Interleave the mono cue across every output channel.
    macro_rules! writer {
        ($t:ty, $conv:expr) => {{
            let samples = samples.clone();
            let cursor = Arc::clone(&cursor);
            let finished = Arc::clone(&finished);
            move |data: &mut [$t], _: &cpal::OutputCallbackInfo| {
                let mut at = match cursor.lock() {
                    Ok(guard) => guard,
                    Err(_) => return,
                };
                for frame in data.chunks_mut(channels) {
                    let value = samples.get(*at).copied().unwrap_or(0.0);
                    if *at < samples.len() {
                        *at += 1;
                    } else {
                        finished.store(true, Ordering::Relaxed);
                    }
                    let converted = $conv(value);
                    for slot in frame.iter_mut() {
                        *slot = converted;
                    }
                }
            }
        }};
    }

    let stream = match supported.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            config,
            writer!(f32, |v: f32| v),
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            config,
            writer!(i16, |v: f32| (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16),
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_output_stream(
            config,
            writer!(u16, |v: f32| ((v.clamp(-1.0, 1.0) * i16::MAX as f32) as i32 + 32_768)
                as u16),
            err_fn,
            None,
        ),
        other => return Err(format!("unsupported output format: {other:?}")),
    }
    .map_err(|e| format!("build output stream: {e}"))?;

    stream.play().map_err(|e| format!("stream play: {e}"))?;

    // Wait out the cue plus a short tail so the device drains before the stream
    // is dropped, then hard-cap in case the callback never runs.
    let duration_ms = (total_frames as u64 * 1_000) / u64::from(sample_rate.max(1));
    let deadline = std::time::Instant::now() + Duration::from_millis(duration_ms + 400);
    while !finished.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(40));
    drop(stream);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_cue_has_the_requested_length() {
        // Start is a single 90 ms segment.
        let samples = render(Cue::Start, 48_000, 1.0);
        let expected = (0.090 * 48_000.0) as usize;
        assert!((samples.len() as i64 - expected as i64).abs() <= 2);
    }

    #[test]
    fn samples_stay_inside_the_output_range() {
        for cue in [Cue::Start, Cue::Stop, Cue::Done, Cue::Error] {
            for sample in render(cue, 44_100, 1.0) {
                assert!(
                    (-1.0..=1.0).contains(&sample),
                    "{cue:?} produced {sample} outside -1..=1"
                );
            }
        }
    }

    #[test]
    fn envelope_starts_and_ends_near_silence() {
        // A cue that begins at full amplitude clicks through the speakers.
        let samples = render(Cue::Start, 48_000, 1.0);
        assert!(samples[0].abs() < 0.05, "attack should fade in");
        assert!(
            samples[samples.len() - 1].abs() < 0.05,
            "release should fade out"
        );
        let peak = samples.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(peak > 0.5, "the body of the cue should be audible");
    }

    #[test]
    fn volume_scales_the_output() {
        let loud = render(Cue::Done, 48_000, 1.0);
        let quiet = render(Cue::Done, 48_000, 0.25);
        let peak = |v: &[f32]| v.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(peak(&quiet) < peak(&loud) * 0.5);
    }

    #[test]
    fn silent_gap_segments_produce_silence() {
        // Done is blip / gap / blip — the middle must actually be quiet.
        let samples = render(Cue::Done, 48_000, 1.0);
        let gap_start = (0.058 * 48_000.0) as usize;
        let gap_end = (0.082 * 48_000.0) as usize;
        let gap_peak = samples[gap_start..gap_end]
            .iter()
            .fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(gap_peak < 0.01, "gap should be silent, peaked at {gap_peak}");
    }
}
