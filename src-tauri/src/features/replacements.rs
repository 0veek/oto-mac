//! Literal find-and-replace applied to a finished transcript.
//!
//! Distinct from the dictionary, which only nudges the recogniser toward a
//! spelling. Replacements are deterministic: they fix the words a model gets
//! wrong every single time — a colleague's name, a product spelled unusually,
//! an expansion the speaker never wants to say in full.

use crate::config::ReplacementRule;

/// True when `start..end` in `text` is not glued to surrounding word characters.
fn is_word_boundary(text: &str, start: usize, end: usize) -> bool {
    let before_ok = text[..start]
        .chars()
        .next_back()
        .is_none_or(|c| !c.is_alphanumeric() && c != '_');
    let after_ok = text[end..]
        .chars()
        .next()
        .is_none_or(|c| !c.is_alphanumeric() && c != '_');
    before_ok && after_ok
}

/// Apply one rule everywhere it matches.
fn apply_rule(text: &str, rule: &ReplacementRule) -> String {
    let needle = rule.from.trim();
    if needle.is_empty() {
        return text.to_string();
    }

    // Case-insensitive search runs against lowered copies. Doing that only when
    // the lengths agree keeps the byte offsets valid; scripts where lowercasing
    // changes length fall back to an exact match rather than slicing wrongly.
    let (haystack, pattern) = if rule.case_sensitive {
        (text.to_string(), needle.to_string())
    } else {
        let lowered = text.to_lowercase();
        if lowered.len() == text.len() {
            (lowered, needle.to_lowercase())
        } else {
            (text.to_string(), needle.to_string())
        }
    };

    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    while let Some(offset) = haystack[cursor..].find(&pattern) {
        let start = cursor + offset;
        let end = start + pattern.len();
        if rule.whole_word && !is_word_boundary(&haystack, start, end) {
            out.push_str(&text[cursor..end]);
            cursor = end;
            continue;
        }
        out.push_str(&text[cursor..start]);
        out.push_str(&rule.to);
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    out
}

/// Apply every enabled rule, in the user's order.
pub fn apply_replacements(text: &str, rules: &[ReplacementRule]) -> String {
    rules
        .iter()
        .filter(|rule| rule.enabled)
        .fold(text.to_string(), |acc, rule| apply_rule(&acc, rule))
}

/// A correction Oto noticed and could turn into a permanent rule.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplacementSuggestion {
    pub from: String,
    pub to: String,
}

/// Shortest word worth learning a rule for. Below this, a "correction" is
/// almost always a filler word or an article the user rephrased.
const MIN_LEARNABLE_LEN: usize = 3;
/// Suggesting a dozen rules from one edit would be noise, not help.
const MAX_SUGGESTIONS: usize = 6;

/// Strip surrounding punctuation so "kubernetis," and "kubernetis" compare equal.
fn core(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Length of the longest common subsequence table's alignment, as index pairs.
///
/// A word-level diff rather than a character one: the unit a replacement rule
/// operates on is a word, so aligning anything finer would produce rules nobody
/// could read.
fn align(before: &[&str], after: &[&str]) -> Vec<(usize, usize)> {
    let (n, m) = (before.len(), after.len());
    let mut table = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if core(before[i]).eq_ignore_ascii_case(core(after[j])) {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }

    let mut pairs = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if core(before[i]).eq_ignore_ascii_case(core(after[j])) {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// Infer replacement rules from a transcript the user corrected by hand.
///
/// Only one-for-one word substitutions are learned. Insertions, deletions, and
/// multi-word rewrites are real edits but they are not the kind of consistent
/// mis-hearing a literal rule can fix, and turning them into rules would corrupt
/// later transcripts.
pub fn suggest_replacements(raw: &str, corrected: &str) -> Vec<ReplacementSuggestion> {
    let before: Vec<&str> = raw.split_whitespace().collect();
    let after: Vec<&str> = corrected.split_whitespace().collect();
    if before.is_empty() || after.is_empty() {
        return Vec::new();
    }

    let anchors = align(&before, &after);
    let mut suggestions: Vec<ReplacementSuggestion> = Vec::new();

    // Walk the gaps between aligned words; a gap of exactly one word on each
    // side is a substitution.
    let mut previous = (0usize, 0usize);
    let boundaries = anchors
        .iter()
        .copied()
        .chain(std::iter::once((before.len(), after.len())));

    for (i, j) in boundaries {
        if i == previous.0 + 1 && j == previous.1 + 1 {
            let from = core(before[previous.0]);
            let to = core(after[previous.1]);
            let learnable = from.len() >= MIN_LEARNABLE_LEN
                && !to.is_empty()
                // A pure case change is what cleanup already does; a rule would
                // fight it every time.
                && !from.eq_ignore_ascii_case(to)
                && !suggestions.iter().any(|s| s.from.eq_ignore_ascii_case(from));
            if learnable {
                suggestions.push(ReplacementSuggestion {
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
        }
        previous = (i + 1, j + 1);
    }

    suggestions.truncate(MAX_SUGGESTIONS);
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(from: &str, to: &str) -> ReplacementRule {
        ReplacementRule {
            id: "r".into(),
            from: from.into(),
            to: to.into(),
            whole_word: false,
            case_sensitive: false,
            enabled: true,
        }
    }

    #[test]
    fn a_simple_replacement_applies_everywhere() {
        let rules = vec![rule("kubernetis", "Kubernetes")];
        assert_eq!(
            apply_replacements("kubernetis and kubernetis again", &rules),
            "Kubernetes and Kubernetes again"
        );
    }

    #[test]
    fn matching_is_case_insensitive_by_default_but_output_is_verbatim() {
        let rules = vec![rule("oto", "Oto")];
        assert_eq!(apply_replacements("OTO and oto and Oto", &rules), "Oto and Oto and Oto");
    }

    #[test]
    fn case_sensitive_rules_only_match_exactly() {
        let rules = vec![ReplacementRule {
            case_sensitive: true,
            ..rule("IT", "information technology")
        }];
        assert_eq!(
            apply_replacements("IT department did it", &rules),
            "information technology department did it"
        );
    }

    #[test]
    fn whole_word_rules_do_not_match_inside_words() {
        let rules = vec![ReplacementRule {
            whole_word: true,
            ..rule("cat", "dog")
        }];
        assert_eq!(
            apply_replacements("the cat in the catalogue", &rules),
            "the dog in the catalogue"
        );
    }

    #[test]
    fn substring_rules_do_match_inside_words() {
        let rules = vec![rule("cat", "dog")];
        assert_eq!(apply_replacements("catalogue", &rules), "dogalogue");
    }

    #[test]
    fn rules_apply_in_order_and_can_chain() {
        let rules = vec![rule("a", "b"), rule("b", "c")];
        assert_eq!(apply_replacements("a", &rules), "c");
    }

    #[test]
    fn disabled_and_blank_rules_are_skipped() {
        let rules = vec![
            ReplacementRule {
                enabled: false,
                ..rule("keep", "changed")
            },
            rule("   ", "noise"),
        ];
        assert_eq!(apply_replacements("keep this", &rules), "keep this");
    }

    #[test]
    fn a_rule_whose_output_contains_its_input_does_not_loop() {
        // Naive repeated replacement would expand forever here.
        let rules = vec![rule("bug", "bug (tracked)")];
        assert_eq!(
            apply_replacements("one bug", &rules),
            "one bug (tracked)"
        );
    }

    #[test]
    fn multibyte_text_is_replaced_without_panicking() {
        let rules = vec![rule("café", "cafe")];
        assert_eq!(apply_replacements("le café noir", &rules), "le cafe noir");
        // Turkish dotted capital I lowercases to two code points; the rule must
        // fall back to an exact match rather than slice at a shifted offset.
        let tricky = vec![rule("İ", "I")];
        assert_eq!(apply_replacements("İstanbul", &tricky), "Istanbul");
    }

    #[test]
    fn empty_text_and_no_rules_are_safe() {
        assert_eq!(apply_replacements("", &[]), "");
        assert_eq!(apply_replacements("unchanged", &[]), "unchanged");
    }

    fn suggestion(from: &str, to: &str) -> ReplacementSuggestion {
        ReplacementSuggestion {
            from: from.into(),
            to: to.into(),
        }
    }

    #[test]
    fn a_single_word_correction_becomes_a_rule() {
        assert_eq!(
            suggest_replacements("we deployed to kubernetis today", "we deployed to Kubernetes today"),
            vec![suggestion("kubernetis", "Kubernetes")]
        );
    }

    #[test]
    fn several_corrections_in_one_edit_are_all_learned() {
        let found = suggest_replacements(
            "ask Aveek about the grafana dashbored",
            "ask Aveek about the Grafana dashboard",
        );
        assert_eq!(found, vec![suggestion("dashbored", "dashboard")]);
    }

    #[test]
    fn punctuation_around_a_word_does_not_block_learning() {
        assert_eq!(
            suggest_replacements("deploy to kubernetis, then wait", "deploy to Kubernetes, then wait"),
            vec![suggestion("kubernetis", "Kubernetes")]
        );
    }

    #[test]
    fn a_pure_case_change_is_not_worth_a_rule() {
        // Cleanup already handles capitalization; a rule would fight it.
        assert!(suggest_replacements("send it to bob", "send it to Bob").is_empty());
    }

    #[test]
    fn short_words_are_not_learned() {
        // "a" → "the" is rephrasing, not a consistent mis-hearing.
        assert!(suggest_replacements("open a window", "open the window").is_empty());
    }

    #[test]
    fn insertions_and_deletions_are_not_turned_into_rules() {
        // Learning "" for a deleted word would erase it from every transcript.
        assert!(suggest_replacements("please review this now", "please review this").is_empty());
        assert!(suggest_replacements("review this", "please review this now").is_empty());
    }

    #[test]
    fn a_full_rewrite_produces_nothing_rather_than_nonsense() {
        let found = suggest_replacements(
            "the quick brown fox jumps",
            "an entirely different sentence altogether",
        );
        assert!(found.is_empty(), "got {found:?}");
    }

    #[test]
    fn an_unchanged_transcript_suggests_nothing() {
        let text = "nothing at all was corrected here";
        assert!(suggest_replacements(text, text).is_empty());
    }

    #[test]
    fn empty_input_is_safe() {
        assert!(suggest_replacements("", "anything").is_empty());
        assert!(suggest_replacements("anything", "").is_empty());
    }

    #[test]
    fn the_same_word_is_only_suggested_once() {
        let found = suggest_replacements(
            "kubernetis and more kubernetis",
            "Kubernetes and more Kubernetes",
        );
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn suggestions_are_capped() {
        let raw = (0..20).map(|i| format!("wordx{i}")).collect::<Vec<_>>().join(" ");
        let fixed = (0..20).map(|i| format!("wordy{i}")).collect::<Vec<_>>().join(" ");
        assert!(suggest_replacements(&raw, &fixed).len() <= MAX_SUGGESTIONS);
    }

    #[test]
    fn a_learned_rule_actually_fixes_the_transcript() {
        // End to end: the suggestion has to be usable as a rule.
        let found = suggest_replacements("ship it to kubernetis", "ship it to Kubernetes");
        let rules: Vec<ReplacementRule> = found
            .iter()
            .map(|s| ReplacementRule {
                id: "learned".into(),
                from: s.from.clone(),
                to: s.to.clone(),
                whole_word: true,
                case_sensitive: false,
                enabled: true,
            })
            .collect();
        assert_eq!(
            apply_replacements("deploying kubernetis again", &rules),
            "deploying Kubernetes again"
        );
    }
}
