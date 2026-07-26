//! Energy-based voice-activity detection with an adaptive noise floor.
//!
//! Fed from the capture callback, which already computes per-chunk energy for
//! the waveform meter. Deliberately not a neural VAD: this only has to answer
//! "has the speaker stopped talking", and a model would add a dependency, a
//! model file, and latency for no gain at that job.

use crate::config::VadConfig;

/// Speech must exceed the noise floor by this factor.
const SPEECH_RATIO: f32 = 3.0;
/// Absolute energy below which nothing counts as speech, whatever the floor is.
/// Keeps a perfectly silent input from ratcheting the floor down to zero and
/// then treating its own dither as speech.
const ABSOLUTE_FLOOR: f32 = 0.006;
/// The floor rises slowly (a burst of speech must not be learned as noise) …
const FLOOR_ATTACK: f32 = 0.02;
/// … and falls quickly, so moving somewhere quieter re-arms detection fast.
const FLOOR_DECAY: f32 = 0.25;

/// Point-in-time view of the tracker, cheap to copy out from under a lock.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VadSnapshot {
    /// Cumulative time judged to be speech.
    pub speech_ms: u64,
    /// Silence since the last speech frame. Zero while speaking.
    pub trailing_silence_ms: u64,
    pub ever_spoke: bool,
    pub noise_floor: f32,
}

impl VadSnapshot {
    /// Whether a hands-free session should end now.
    pub fn should_auto_stop(&self, cfg: &VadConfig) -> bool {
        self.ever_spoke
            && self.speech_ms >= u64::from(cfg.effective_min_speech_ms())
            && self.trailing_silence_ms >= u64::from(cfg.effective_silence_ms())
    }
}

#[derive(Debug)]
pub struct VadTracker {
    sample_rate: u32,
    noise_floor: f32,
    speech_ms: u64,
    trailing_silence_ms: u64,
    ever_spoke: bool,
    seeded: bool,
}

impl VadTracker {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate.max(1),
            noise_floor: ABSOLUTE_FLOOR,
            speech_ms: 0,
            trailing_silence_ms: 0,
            ever_spoke: false,
            seeded: false,
        }
    }

    /// Fold in one capture chunk of mono samples.
    pub fn push(&mut self, samples: &[i16]) {
        if samples.is_empty() {
            return;
        }
        let level = rms(samples);
        let duration_ms = (samples.len() as u64 * 1_000) / u64::from(self.sample_rate);
        self.push_level(level, duration_ms);
    }

    /// Energy-only entry point. Split out so the decision logic is testable
    /// without synthesizing waveforms.
    pub fn push_level(&mut self, level: f32, duration_ms: u64) {
        // The first chunk defines the starting floor; without this an opening
        // word would be measured against ABSOLUTE_FLOOR in a noisy room and the
        // floor would take seconds to catch up.
        if !self.seeded {
            self.noise_floor = level.max(ABSOLUTE_FLOOR);
            self.seeded = true;
        }

        let threshold = (self.noise_floor * SPEECH_RATIO).max(ABSOLUTE_FLOOR);
        if level > threshold {
            self.speech_ms = self.speech_ms.saturating_add(duration_ms);
            self.trailing_silence_ms = 0;
            self.ever_spoke = true;
        } else {
            self.trailing_silence_ms = self.trailing_silence_ms.saturating_add(duration_ms);
            // Only adapt the floor on non-speech frames, so sustained speech
            // cannot raise the bar until it silences itself.
            let alpha = if level < self.noise_floor {
                FLOOR_DECAY
            } else {
                FLOOR_ATTACK
            };
            self.noise_floor += alpha * (level - self.noise_floor);
            self.noise_floor = self.noise_floor.max(0.0);
        }
    }

    pub fn snapshot(&self) -> VadSnapshot {
        VadSnapshot {
            speech_ms: self.speech_ms,
            trailing_silence_ms: self.trailing_silence_ms,
            ever_spoke: self.ever_spoke,
            noise_floor: self.noise_floor,
        }
    }

    /// Gate threshold for the noise gate, expressed in full-scale units.
    pub fn gate_level(&self, configured: f32) -> f32 {
        (self.noise_floor * 1.5).max(configured)
    }
}

/// Root-mean-square energy of a chunk, normalized to 0..=1.
pub fn rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    ((sum_sq / samples.len() as f64).sqrt() / 32_768.0).clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(silence_ms: u32, min_speech_ms: u32) -> VadConfig {
        VadConfig {
            auto_stop: true,
            silence_ms,
            min_speech_ms,
        }
    }

    #[test]
    fn rms_of_silence_is_zero_and_full_scale_is_one() {
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(rms(&[0; 128]), 0.0);
        assert!(rms(&[i16::MAX; 128]) > 0.99);
    }

    #[test]
    fn speech_then_silence_triggers_auto_stop() {
        let mut vad = VadTracker::new(16_000);
        // Quiet room seeds the floor.
        for _ in 0..10 {
            vad.push_level(0.004, 20);
        }
        // A second of speech well above the floor.
        for _ in 0..50 {
            vad.push_level(0.20, 20);
        }
        let mid = vad.snapshot();
        assert!(mid.ever_spoke);
        assert_eq!(mid.trailing_silence_ms, 0);
        assert!(!mid.should_auto_stop(&cfg(1_000, 400)));

        // 1.2 s of silence.
        for _ in 0..60 {
            vad.push_level(0.004, 20);
        }
        let end = vad.snapshot();
        assert!(end.trailing_silence_ms >= 1_000);
        assert!(end.should_auto_stop(&cfg(1_000, 400)));
    }

    #[test]
    fn silence_alone_never_auto_stops() {
        let mut vad = VadTracker::new(16_000);
        for _ in 0..500 {
            vad.push_level(0.002, 20);
        }
        let snap = vad.snapshot();
        assert!(!snap.ever_spoke);
        // Without this guard, opening the mic and saying nothing would
        // immediately "finish" an empty session.
        assert!(!snap.should_auto_stop(&cfg(1_000, 400)));
    }

    #[test]
    fn a_brief_blip_does_not_satisfy_min_speech() {
        let mut vad = VadTracker::new(16_000);
        for _ in 0..10 {
            vad.push_level(0.004, 20);
        }
        // 100 ms of sound — a cough or a door, not an utterance.
        for _ in 0..5 {
            vad.push_level(0.3, 20);
        }
        for _ in 0..100 {
            vad.push_level(0.004, 20);
        }
        let snap = vad.snapshot();
        assert!(snap.ever_spoke);
        assert!(snap.speech_ms < 400);
        assert!(!snap.should_auto_stop(&cfg(1_000, 400)));
    }

    #[test]
    fn a_noisy_room_raises_the_floor_instead_of_hearing_speech() {
        let mut vad = VadTracker::new(16_000);
        // Constant fan noise at a level that would be "speech" against silence.
        for _ in 0..200 {
            vad.push_level(0.05, 20);
        }
        let snap = vad.snapshot();
        assert!(
            snap.noise_floor > 0.02,
            "floor should learn the room, got {}",
            snap.noise_floor
        );
        assert!(!snap.should_auto_stop(&cfg(1_000, 400)));
    }

    #[test]
    fn speech_resumes_after_a_pause_and_clears_trailing_silence() {
        let mut vad = VadTracker::new(16_000);
        for _ in 0..10 {
            vad.push_level(0.004, 20);
        }
        for _ in 0..50 {
            vad.push_level(0.2, 20);
        }
        for _ in 0..20 {
            vad.push_level(0.004, 20);
        }
        assert!(vad.snapshot().trailing_silence_ms > 0);
        vad.push_level(0.2, 20);
        assert_eq!(vad.snapshot().trailing_silence_ms, 0);
    }
}
