//! Spoken editing commands applied to a raw transcript.
//!
//! People self-correct while talking — "the meeting is Tuesday, scratch that,
//! Wednesday" — and expect the correction, not the transcript of making it.
//! These run before cleanup so the model never sees the retracted text and
//! cannot decide to keep it.

/// Phrases that retract the preceding sentence.
const SCRATCH: &[&str] = &[
    "scratch that",
    "delete that",
    "forget that",
    "strike that",
    "ignore that",
];
/// Phrases that insert a paragraph break.
const NEW_PARAGRAPH: &[&str] = &["new paragraph"];
/// Phrases that insert a line break.
const NEW_LINE: &[&str] = &["new line"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Scratch,
    NewParagraph,
    NewLine,
}

/// Earliest command in `haystack` at or after `from`, as `(start, end, command)`.
fn next_command(haystack_lower: &str, from: usize) -> Option<(usize, usize, Command)> {
    let mut best: Option<(usize, usize, Command)> = None;
    let candidates = SCRATCH
        .iter()
        .map(|p| (*p, Command::Scratch))
        .chain(NEW_PARAGRAPH.iter().map(|p| (*p, Command::NewParagraph)))
        .chain(NEW_LINE.iter().map(|p| (*p, Command::NewLine)));

    for (phrase, command) in candidates {
        let mut search = from;
        while let Some(offset) = haystack_lower[search..].find(phrase) {
            let start = search + offset;
            let end = start + phrase.len();
            if is_word_boundary(haystack_lower, start, end) {
                let better = match best {
                    // Longer match wins a tie so "new paragraph" is not read as
                    // a bare word sequence by a shorter competing phrase.
                    Some((best_start, best_end, _)) => {
                        start < best_start || (start == best_start && end > best_end)
                    }
                    None => true,
                };
                if better {
                    best = Some((start, end, command));
                }
                break;
            }
            // Overlapping occurrence inside a longer word; keep looking.
            search = start + 1;
            if search >= haystack_lower.len() {
                break;
            }
        }
    }
    best
}

/// True when `start..end` is not glued to surrounding letters or digits.
fn is_word_boundary(text: &str, start: usize, end: usize) -> bool {
    let before_ok = text[..start]
        .chars()
        .next_back()
        .is_none_or(|c| !c.is_alphanumeric());
    let after_ok = text[end..]
        .chars()
        .next()
        .is_none_or(|c| !c.is_alphanumeric());
    before_ok && after_ok
}

/// Byte index just past the end of the sentence preceding `at`.
///
/// Walks back over the retracted words to the last terminator, so "scratch
/// that" removes the thought it followed rather than the whole utterance.
fn sentence_start_before(text: &str, at: usize) -> usize {
    // People say "…Tuesday, scratch that, Wednesday". The comma immediately
    // before the command closes the clause being *retracted*, so it must not be
    // mistaken for the boundary that clause starts at.
    let head = text[..at].trim_end_matches(|c: char| c.is_whitespace() || c == ',' || c == ';');

    let mut boundary = 0usize;
    for (index, ch) in head.char_indices() {
        if matches!(ch, '.' | '!' | '?' | '\n') {
            boundary = index + ch.len_utf8();
        }
    }
    // A comma bounds a clause too, when it is the closest separator.
    if let Some(comma) = head.rfind([',', ';']) {
        if comma + 1 > boundary {
            boundary = comma + 1;
        }
    }
    boundary
}

/// Separators that belong to the retracted clause rather than to what follows.
fn trim_leading_separators(text: &str) -> &str {
    text.trim_start_matches(|c: char| c.is_whitespace() || c == ',' || c == ';')
}

/// Apply every spoken edit in `text`.
pub fn apply_voice_edits(text: &str) -> String {
    let mut current = text.to_string();
    // Bounded so a pathological input cannot spin: each pass removes or
    // replaces one command, and no pass can add one.
    for _ in 0..64 {
        let lower = current.to_lowercase();
        // Lowercasing can change byte lengths for some scripts; fall back to
        // leaving the text alone rather than slicing at a wrong offset.
        if lower.len() != current.len() {
            break;
        }
        let Some((start, end, command)) = next_command(&lower, 0) else {
            break;
        };

        let mut next = String::new();
        match command {
            Command::Scratch => {
                let keep_to = sentence_start_before(&current, start);
                next.push_str(&current[..keep_to]);
                next.push(' ');
                next.push_str(trim_leading_separators(&current[end..]));
            }
            Command::NewParagraph => {
                next.push_str(current[..start].trim_end());
                next.push_str("\n\n");
                next.push_str(current[end..].trim_start());
            }
            Command::NewLine => {
                next.push_str(current[..start].trim_end());
                next.push('\n');
                next.push_str(current[end..].trim_start());
            }
        }
        current = next;
    }

    tidy(&current)
}

/// Collapse the whitespace and stray punctuation left by a removal.
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    let mut newlines = 0usize;

    for ch in text.chars() {
        match ch {
            '\n' => {
                pending_space = false;
                newlines += 1;
            }
            c if c.is_whitespace() => {
                if newlines == 0 {
                    pending_space = true;
                }
            }
            c => {
                if newlines > 0 {
                    // Cap at a paragraph break; more is never meaningful.
                    for _ in 0..newlines.min(2) {
                        out.push('\n');
                    }
                    newlines = 0;
                } else if pending_space && !out.is_empty() {
                    // Never put a space before closing punctuation.
                    if !matches!(c, ',' | '.' | '!' | '?' | ';' | ':') {
                        out.push(' ');
                    }
                }
                pending_space = false;
                out.push(c);
            }
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_that_removes_the_retracted_clause() {
        assert_eq!(
            apply_voice_edits("The meeting is Tuesday, scratch that, Wednesday."),
            "Wednesday."
        );
    }

    #[test]
    fn scratch_that_retracts_only_back_to_the_previous_sentence() {
        // Sentences the speaker already finished are not collateral damage.
        assert_eq!(
            apply_voice_edits("Ship it today. Actually not today scratch that we ship Friday."),
            "Ship it today. we ship Friday."
        );
    }

    #[test]
    fn every_scratch_synonym_is_recognized() {
        for phrase in ["scratch that", "delete that", "forget that", "strike that"] {
            let input = format!("wrong words {phrase} right words");
            assert_eq!(apply_voice_edits(&input), "right words", "{phrase} failed");
        }
    }

    #[test]
    fn scratch_at_the_start_leaves_the_remainder() {
        assert_eq!(apply_voice_edits("scratch that hello there"), "hello there");
    }

    #[test]
    fn multiple_scratches_are_all_applied() {
        assert_eq!(
            apply_voice_edits("one, scratch that, two, scratch that, three"),
            "three"
        );
    }

    #[test]
    fn new_paragraph_and_new_line_insert_breaks() {
        assert_eq!(
            apply_voice_edits("First thought new paragraph second thought"),
            "First thought\n\nsecond thought"
        );
        assert_eq!(
            apply_voice_edits("milk new line eggs"),
            "milk\neggs"
        );
    }

    #[test]
    fn commands_are_case_insensitive() {
        assert_eq!(
            apply_voice_edits("First. New Paragraph Second."),
            "First.\n\nSecond."
        );
    }

    #[test]
    fn a_command_inside_a_longer_word_is_not_a_command() {
        // "newline" as a spoken word, and "that" inside another phrase.
        let input = "Set the delimiter to a newlines character";
        assert_eq!(apply_voice_edits(input), input);
    }

    #[test]
    fn ordinary_text_is_untouched() {
        let input = "Please review the pull request when you get a chance.";
        assert_eq!(apply_voice_edits(input), input);
    }

    #[test]
    fn empty_input_is_safe() {
        assert_eq!(apply_voice_edits(""), "");
        assert_eq!(apply_voice_edits("   "), "");
    }

    #[test]
    fn scratching_everything_yields_empty_text() {
        // The caller treats an empty result as "no speech", which is right:
        // the user retracted everything they said.
        assert_eq!(apply_voice_edits("hello there scratch that"), "");
    }

    #[test]
    fn multibyte_text_survives() {
        let input = "Le café est ouvert, scratch that, fermé.";
        assert_eq!(apply_voice_edits(input), "fermé.");
        let untouched = "日本語のテキストはそのまま";
        assert_eq!(apply_voice_edits(untouched), untouched);
    }

    #[test]
    fn tidy_does_not_strand_spaces_before_punctuation() {
        assert_eq!(apply_voice_edits("we said this scratch that ."), ".");
        assert_eq!(tidy("hello   ,  world"), "hello, world");
        assert_eq!(tidy("a\n\n\n\nb"), "a\n\nb");
    }
}
