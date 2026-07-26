//! Usage statistics derived from local history.
//!
//! Everything here is computed from entries already on disk — no separate
//! tracking store, and nothing to keep in sync. Turning history off turns stats
//! off with it, which is the honest behaviour.

use serde::{Deserialize, Serialize};

use super::history::HistoryEntry;

const MS_PER_DAY: u64 = 24 * 60 * 60 * 1_000;

/// Words per minute a competent typist sustains on prose.
const TYPING_WPM: f64 = 40.0;
/// Words per minute of comfortable speech. The gap between these two is the
/// entire basis of the time-saved figure, so both are stated rather than hidden
/// inside a magic constant.
const SPEAKING_WPM: f64 = 150.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DailyCount {
    /// Days before today; 0 is today.
    pub days_ago: u32,
    pub sessions: usize,
    pub words: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct UsageStats {
    pub total_sessions: usize,
    pub total_words: usize,
    pub words_today: usize,
    pub average_words_per_session: f64,
    /// Minutes saved versus typing the same words.
    pub estimated_minutes_saved: f64,
    /// Consecutive days ending today or yesterday with at least one dictation.
    pub current_streak_days: u32,
    pub best_streak_days: u32,
    /// Newest first, one entry per day for the last 30 days.
    pub daily: Vec<DailyCount>,
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().filter(|w| !w.is_empty()).count()
}

/// Whole days between `then` and `now`, in local terms close enough for a streak.
fn days_ago(created_at_ms: u64, now_ms: u64) -> Option<u32> {
    if created_at_ms > now_ms {
        // Clock changes and imported history can produce future timestamps;
        // count them as today rather than underflowing.
        return Some(0);
    }
    u32::try_from((now_ms - created_at_ms) / MS_PER_DAY).ok()
}

/// Longest and current runs of consecutive active days.
fn streaks(active_days: &[u32]) -> (u32, u32) {
    if active_days.is_empty() {
        return (0, 0);
    }
    let mut sorted: Vec<u32> = active_days.to_vec();
    sorted.sort_unstable();
    sorted.dedup();

    let mut best = 1u32;
    let mut run = 1u32;
    for pair in sorted.windows(2) {
        if pair[1] == pair[0] + 1 {
            run += 1;
            best = best.max(run);
        } else {
            run = 1;
        }
    }

    // A streak counts as live if it reaches today or yesterday — finishing the
    // day's work before midnight should not be required to keep it.
    let current = if sorted[0] <= 1 {
        let mut length = 1u32;
        let mut expected = sorted[0] + 1;
        for &day in sorted.iter().skip(1) {
            if day == expected {
                length += 1;
                expected += 1;
            } else {
                break;
            }
        }
        length
    } else {
        0
    };

    (current, best)
}

/// Compute statistics over `entries`. `now_ms` is passed in so this stays pure.
pub fn compute(entries: &[HistoryEntry], now_ms: u64) -> UsageStats {
    let mut total_words = 0usize;
    let mut words_today = 0usize;
    let mut active_days: Vec<u32> = Vec::new();
    let mut daily = vec![
        DailyCount {
            days_ago: 0,
            sessions: 0,
            words: 0
        };
        30
    ];
    for (index, day) in daily.iter_mut().enumerate() {
        day.days_ago = index as u32;
    }

    for entry in entries {
        let words = word_count(&entry.final_text);
        total_words += words;
        let Some(age) = days_ago(entry.created_at_ms, now_ms) else {
            continue;
        };
        active_days.push(age);
        if age == 0 {
            words_today += words;
        }
        if let Some(bucket) = daily.get_mut(age as usize) {
            bucket.sessions += 1;
            bucket.words += words;
        }
    }

    let total_sessions = entries.len();
    let (current_streak_days, best_streak_days) = streaks(&active_days);
    let minutes_saved =
        total_words as f64 * (1.0 / TYPING_WPM - 1.0 / SPEAKING_WPM);

    UsageStats {
        total_sessions,
        total_words,
        words_today,
        average_words_per_session: if total_sessions == 0 {
            0.0
        } else {
            total_words as f64 / total_sessions as f64
        },
        estimated_minutes_saved: minutes_saved.max(0.0),
        current_streak_days,
        best_streak_days,
        daily,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str, days: u64, now_ms: u64) -> HistoryEntry {
        HistoryEntry {
            id: format!("{days}-{text}"),
            created_at_ms: now_ms - days * MS_PER_DAY,
            raw_text: text.into(),
            final_text: text.into(),
            mode: "dictation".into(),
            language: None,
            has_audio: false,
            duration_ms: 0,
        }
    }

    const NOW: u64 = 1_800_000_000_000;

    #[test]
    fn empty_history_reports_zeroes_not_nan() {
        let stats = compute(&[], NOW);
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.average_words_per_session, 0.0);
        assert_eq!(stats.estimated_minutes_saved, 0.0);
        assert_eq!(stats.current_streak_days, 0);
        assert_eq!(stats.daily.len(), 30);
    }

    #[test]
    fn words_and_sessions_are_counted_from_the_final_text() {
        let entries = vec![entry("one two three", 0, NOW), entry("four five", 0, NOW)];
        let stats = compute(&entries, NOW);
        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.total_words, 5);
        assert_eq!(stats.words_today, 5);
        assert_eq!(stats.average_words_per_session, 2.5);
    }

    #[test]
    fn time_saved_is_the_gap_between_speaking_and_typing() {
        // 600 words: 15 min typing at 40 wpm, 4 min speaking at 150 wpm.
        let text = "word ".repeat(600);
        let stats = compute(&[entry(text.trim(), 0, NOW)], NOW);
        assert!((stats.estimated_minutes_saved - 11.0).abs() < 0.01);
    }

    #[test]
    fn a_streak_runs_from_today_backwards() {
        let entries = vec![
            entry("a", 0, NOW),
            entry("b", 1, NOW),
            entry("c", 2, NOW),
            // Gap at day 3.
            entry("d", 4, NOW),
        ];
        let stats = compute(&entries, NOW);
        assert_eq!(stats.current_streak_days, 3);
        assert_eq!(stats.best_streak_days, 3);
    }

    #[test]
    fn yesterday_still_counts_as_a_live_streak() {
        // Otherwise the streak would break every morning before first use.
        let entries = vec![entry("a", 1, NOW), entry("b", 2, NOW)];
        assert_eq!(compute(&entries, NOW).current_streak_days, 2);
    }

    #[test]
    fn a_stale_streak_is_not_current_but_is_still_the_best() {
        let entries = vec![entry("a", 5, NOW), entry("b", 6, NOW), entry("c", 7, NOW)];
        let stats = compute(&entries, NOW);
        assert_eq!(stats.current_streak_days, 0);
        assert_eq!(stats.best_streak_days, 3);
    }

    #[test]
    fn several_sessions_on_one_day_do_not_inflate_the_streak() {
        let entries = vec![entry("a", 0, NOW), entry("b", 0, NOW), entry("c", 0, NOW)];
        assert_eq!(compute(&entries, NOW).current_streak_days, 1);
    }

    #[test]
    fn the_daily_series_buckets_by_age() {
        let entries = vec![entry("one two", 0, NOW), entry("three", 3, NOW)];
        let stats = compute(&entries, NOW);
        assert_eq!(stats.daily[0].words, 2);
        assert_eq!(stats.daily[0].sessions, 1);
        assert_eq!(stats.daily[3].words, 1);
        assert_eq!(stats.daily[1].sessions, 0);
    }

    #[test]
    fn entries_older_than_the_window_still_count_in_totals() {
        let entries = vec![entry("old words here", 200, NOW)];
        let stats = compute(&entries, NOW);
        assert_eq!(stats.total_words, 3);
        // …but do not appear in the 30-day series.
        assert!(stats.daily.iter().all(|day| day.words == 0));
    }

    #[test]
    fn a_future_timestamp_is_treated_as_today_rather_than_underflowing() {
        let mut future = entry("clock skew", 0, NOW);
        future.created_at_ms = NOW + MS_PER_DAY;
        let stats = compute(&[future], NOW);
        assert_eq!(stats.words_today, 2);
        assert_eq!(stats.current_streak_days, 1);
    }
}
