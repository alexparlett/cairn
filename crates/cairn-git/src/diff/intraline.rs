//! Where a removed line and the added line it was paired with differ inside themselves
//! (R2.7).
//!
//! Always on, by decision L4, and display-only: the ranges land in
//! [`cairn_model::DisplayOverlay`], which the patch emitter cannot reach. The pairing is
//! the i-th removed line of a changed range with its i-th added line, the same pairing a
//! side-by-side view draws.

use std::ops::Range;

use cairn_model::{ByteRange, IntraLineHighlight, TextDiff};
use gix::diff::blob::{Algorithm, Diff, InternedInput};

pub(super) fn highlights<'a>(text: &'a TextDiff, max_line_bytes: u32) -> Vec<IntraLineHighlight> {
    let mut input: InternedInput<&'a [u8]> = InternedInput::default();
    let mut diff = Diff::default();
    let mut out = Vec::new();

    for change in text.changes() {
        for (removed, added) in change.removed.numbers().zip(change.added.numbers()) {
            let (Some(old), Some(new)) = (text.old_line(removed), text.new_line(added)) else {
                continue;
            };
            // A pair over the long-line limit is skipped: the word diff of two lines that
            // long costs more than the row it decorates is worth (R2.7).
            if too_long(old.bytes(), max_line_bytes) || too_long(new.bytes(), max_line_bytes) {
                continue;
            }
            let (on_removed, on_added) = ranges(old.bytes(), new.bytes(), &mut input, &mut diff);
            if on_removed.is_empty() && on_added.is_empty() {
                continue;
            }
            out.push(IntraLineHighlight {
                removed_line: removed,
                added_line: added,
                on_removed,
                on_added,
            });
        }
    }
    out
}

fn too_long(line: &[u8], max_line_bytes: u32) -> bool {
    u64::try_from(line.len()).unwrap_or(u64::MAX) > u64::from(max_line_bytes)
}

/// The `input` and `diff` are handed in so a file of ten thousand changed pairs reuses two
/// allocations instead of making twenty thousand.
fn ranges<'a>(
    old: &'a [u8],
    new: &'a [u8],
    input: &mut InternedInput<&'a [u8]>,
    diff: &mut Diff,
) -> (Vec<ByteRange>, Vec<ByteRange>) {
    let old_tokens = tokenize(old);
    let new_tokens = tokenize(new);
    input.clear();
    input.update_before(old_tokens.iter().map(|range| &old[range.clone()]));
    input.update_after(new_tokens.iter().map(|range| &new[range.clone()]));

    let num_tokens = input.interner.num_tokens();
    diff.compute_with(Algorithm::Myers, &input.before, &input.after, num_tokens);
    // Myers, and no indent heuristic: the heuristic is about where to place a line-level
    // slider, and imara's own documentation says a histogram diff reads badly over
    // characters.
    diff.postprocess_no_heuristic(input);

    let mut on_removed = Vec::new();
    let mut on_added = Vec::new();
    for hunk in diff.hunks() {
        if let Some(range) = span(&old_tokens, hunk.before) {
            on_removed.push(range);
        }
        if let Some(range) = span(&new_tokens, hunk.after) {
            on_added.push(range);
        }
    }
    (on_removed, on_added)
}

/// A run of tokens as the bytes it covers; `None` for an empty run, which marks nothing.
fn span(tokens: &[Range<usize>], run: Range<u32>) -> Option<ByteRange> {
    let first = tokens.get(usize::try_from(run.start).ok()?)?;
    let last = tokens.get(usize::try_from(run.end).ok()?.checked_sub(1)?)?;
    Some(ByteRange::new(
        u32::try_from(first.start).unwrap_or(u32::MAX),
        u32::try_from(last.end).unwrap_or(u32::MAX),
    ))
}

/// A word, a run of spacing, or one other byte — git's own word granularity, over bytes
/// rather than characters so a line that is not UTF-8 still tokenises. A byte at or above
/// `0x80` counts as part of a word, which keeps a multi-byte character whole.
fn tokenize(line: &[u8]) -> Vec<Range<usize>> {
    let mut tokens = Vec::new();
    let mut at = 0usize;
    while at < line.len() {
        let byte = line[at];
        let end = if is_word(byte) {
            run_from(line, at, is_word)
        } else if byte.is_ascii_whitespace() {
            run_from(line, at, |b| b.is_ascii_whitespace())
        } else {
            at + 1
        };
        tokens.push(at..end);
        at = end;
    }
    tokens
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

fn run_from(line: &[u8], from: usize, mut belongs: impl FnMut(u8) -> bool) -> usize {
    let mut end = from;
    while end < line.len() && belongs(line[end]) {
        end += 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{ChangedRange, DiffLine, LineNumber, LineSpan};

    fn words(line: &[u8]) -> Vec<&[u8]> {
        tokenize(line).into_iter().map(|at| &line[at]).collect()
    }

    #[test]
    fn a_line_splits_into_words_spacing_and_single_punctuation() {
        assert_eq!(
            words(b"let x_1 = f(2);"),
            vec![
                &b"let"[..],
                b" ",
                b"x_1",
                b" ",
                b"=",
                b" ",
                b"f",
                b"(",
                b"2",
                b")",
                b";"
            ]
        );
        assert_eq!(words(b""), Vec::<&[u8]>::new());
        assert_eq!(words(b"   "), vec![&b"   "[..]], "spacing runs together");
        assert_eq!(
            words("héllo".as_bytes()),
            vec!["héllo".as_bytes()],
            "a multi-byte character stays inside its word"
        );
        assert_eq!(
            words(&[0xff, b'a']),
            vec![&[0xff, b'a'][..]],
            "a byte that is not UTF-8 at all still tokenises"
        );
    }

    fn highlight_of(old: &str, new: &str) -> Option<IntraLineHighlight> {
        let text = TextDiff::new(
            vec![DiffLine::terminated(old)],
            vec![DiffLine::terminated(new)],
            vec![ChangedRange::new(LineSpan::at(0, 1), LineSpan::at(0, 1))],
        );
        highlights(&text, 2048).into_iter().next()
    }

    /// Caught by: marking the whole line, which is what a view would show if the byte
    /// ranges were the line's own bounds rather than the words that moved.
    #[test]
    fn only_the_words_that_changed_are_marked() {
        let found = highlight_of("let x = 1;", "let x = 2;").expect("a pair differs");
        assert_eq!(found.removed_line, LineNumber::from_index(0));
        assert_eq!(found.added_line, LineNumber::from_index(0));
        assert_eq!(found.on_removed, vec![ByteRange::new(8, 9)]);
        assert_eq!(found.on_added, vec![ByteRange::new(8, 9)]);
    }

    #[test]
    fn a_pair_that_only_gained_text_marks_nothing_on_the_side_that_lost_none() {
        let found = highlight_of("value", "value = 1").expect("a pair differs");
        assert!(
            found.on_removed.is_empty(),
            "nothing was removed, yet {:?} is marked",
            found.on_removed
        );
        assert_eq!(found.on_added, vec![ByteRange::new(5, 9)]);
    }

    #[test]
    fn identical_lines_are_not_paired_at_all() {
        assert!(highlight_of("same", "same").is_none());
    }

    /// R2.7's skip. Caught by: running the word diff anyway, which is the cost the limit
    /// exists to refuse.
    #[test]
    fn a_pair_over_the_long_line_limit_is_skipped() {
        let long = "x".repeat(64);
        let other = format!("{long}y");
        let text = TextDiff::new(
            vec![DiffLine::terminated(long.as_bytes())],
            vec![DiffLine::terminated(other.as_bytes())],
            vec![ChangedRange::new(LineSpan::at(0, 1), LineSpan::at(0, 1))],
        );
        assert!(
            highlights(&text, 32).is_empty(),
            "a pair past the limit was highlighted"
        );
        assert!(
            !highlights(&text, 2048).is_empty(),
            "the same pair under the limit must be highlighted, or the test above is vacuous"
        );
    }

    /// Side-by-side pairs the i-th removed line with the i-th added one; a change with
    /// more lines on one side leaves the leftovers unpaired and unmarked.
    #[test]
    fn a_change_of_uneven_sides_pairs_as_far_as_both_sides_go() {
        let text = TextDiff::new(
            vec![DiffLine::terminated("one"), DiffLine::terminated("two")],
            vec![
                DiffLine::terminated("ONE"),
                DiffLine::terminated("TWO"),
                DiffLine::terminated("three"),
            ],
            vec![ChangedRange::new(LineSpan::at(0, 2), LineSpan::at(0, 3))],
        );
        let found = highlights(&text, 2048);
        assert_eq!(found.len(), 2, "the third added line has no partner");
        assert_eq!(found[1].removed_line, LineNumber::from_index(1));
        assert_eq!(found[1].added_line, LineNumber::from_index(1));
    }
}
