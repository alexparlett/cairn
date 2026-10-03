//! git's function context: the text `git diff` prints after a hunk header's closing `@@`.
//!
//! git finds it by searching the OLD side backwards from the line above where the hunk
//! starts, for the nearest line its `xfuncname` (or its default rule) matches, and prints
//! that line's match — so the text a hunk carries depends on where the hunk starts and on
//! nothing else, and a hunk starting somewhere else, at another context, carries another.
//! Cairn implements none of that: the engine takes the text from git's own header, at the
//! context the view asked for, and this type keeps it beside the exact answer, by the line
//! each of git's hunks starts at.
//!
//! It is display-only. It lives in [`crate::DisplayOverlay`], which the patch emitter never
//! sees, so a patch is the same bytes whatever git printed here (R1.6, R1.7).

use crate::{Context, HunkHeader, LineNumber};

/// git's function context for each hunk it printed, keyed by where the hunk starts on the
/// old side.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FunctionContext {
    /// The context git was asked at; `None` when nothing was read.
    context: Option<Context>,
    /// Sorted by start, one entry per start. An empty text is git printing none.
    by_start: Vec<(LineNumber, Vec<u8>)>,
}

impl FunctionContext {
    /// Nothing read: every lookup answers `None`.
    pub fn none() -> Self {
        Self::default()
    }

    /// What git printed at `context`: for each hunk, the line its old side starts at (the
    /// line before an empty side, as a header names it, counted from zero) and the text
    /// after its header's closing `@@`, empty where git printed none. Order does not
    /// matter; a start given twice keeps the last text given for it.
    pub fn read_at(context: Context, hunks: Vec<(LineNumber, Vec<u8>)>) -> Self {
        let mut by_start = hunks;
        // Stable, so of two entries for one start the later stays later, and is kept.
        by_start.sort_by_key(|(start, _)| *start);
        let mut kept: Vec<(LineNumber, Vec<u8>)> = Vec::with_capacity(by_start.len());
        for entry in by_start {
            match kept.last_mut() {
                Some(last) if last.0 == entry.0 => *last = entry,
                _ => kept.push(entry),
            }
        }
        Self {
            context: Some(normalised(context)),
            by_start: kept,
        }
    }

    /// The context these were read at, `None` when nothing was.
    pub fn context(&self) -> Option<Context> {
        self.context
    }

    /// The text git prints after the header of the hunk `header` heads: `Some` of it —
    /// empty where git prints none — when git printed a hunk starting where this one does,
    /// and `None` when it did not, which is what a hunk grouped at another context than the
    /// one these were read at mostly gets. A search, not a scan.
    pub fn of(&self, header: HunkHeader) -> Option<&[u8]> {
        let start = header.old.start();
        self.by_start
            .binary_search_by_key(&start, |(at, _)| *at)
            .ok()
            .and_then(|index| self.by_start.get(index))
            .map(|(_, text)| text.as_slice())
    }

    /// How many hunk starts are known.
    pub fn len(&self) -> usize {
        self.by_start.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_start.is_empty()
    }
}

/// `Lines(0)` means one line, as [`Context::line_count`] reads it.
fn normalised(context: Context) -> Context {
    match context.line_count() {
        Some(count) => Context::Lines(count),
        None => Context::EntireFile,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineSpan;

    fn header(old_start: u32, old_len: u32) -> HunkHeader {
        HunkHeader {
            old: LineSpan::at(old_start, old_len),
            new: LineSpan::at(old_start, old_len),
        }
    }

    fn at(start: u32, text: &str) -> (LineNumber, Vec<u8>) {
        (LineNumber::from_index(start), text.as_bytes().to_vec())
    }

    /// Caught by: keying the lookup on the new side, or on a hunk's position in the list.
    #[test]
    fn a_hunk_finds_the_text_git_printed_for_where_it_starts() {
        let context = FunctionContext::read_at(
            Context::lines(3),
            vec![at(40, "fn later()"), at(4, "fn first()"), at(20, "")],
        );
        assert_eq!(context.of(header(4, 7)), Some(&b"fn first()"[..]));
        assert_eq!(context.of(header(40, 3)), Some(&b"fn later()"[..]));
        assert_eq!(
            context.of(header(20, 6)),
            Some(&b""[..]),
            "git printing nothing is known, and is not the same as not knowing"
        );
        assert_eq!(
            context.of(header(5, 7)),
            None,
            "a hunk starting where git printed none has no answer here"
        );
        let other_new_side = HunkHeader {
            old: LineSpan::at(4, 7),
            new: LineSpan::at(9, 7),
        };
        assert_eq!(
            context.of(other_new_side),
            Some(&b"fn first()"[..]),
            "the text belongs to the old side's start, whatever the new side says"
        );
        assert_eq!(context.len(), 3);
    }

    #[test]
    fn nothing_read_answers_nothing() {
        let none = FunctionContext::none();
        assert_eq!(none.context(), None);
        assert!(none.is_empty());
        assert_eq!(none.of(header(0, 1)), None);
    }

    /// The context is kept as the view will compare it: zero lines is one line.
    #[test]
    fn the_context_is_kept_the_way_a_view_reads_it() {
        assert_eq!(
            FunctionContext::read_at(Context::Lines(0), Vec::new()).context(),
            Some(Context::Lines(1))
        );
        assert_eq!(
            FunctionContext::read_at(Context::EntireFile, Vec::new()).context(),
            Some(Context::EntireFile)
        );
    }

    /// Caught by: keeping the first of two entries for one start, or both, which a search
    /// then answers either of.
    #[test]
    fn a_start_given_twice_keeps_the_last_text() {
        let context = FunctionContext::read_at(
            Context::lines(3),
            vec![at(7, "first"), at(2, "x"), at(7, "second")],
        );
        assert_eq!(context.len(), 2);
        assert_eq!(context.of(header(7, 3)), Some(&b"second"[..]));
    }

    /// Many hunks, found by search: every one of a thousand answers its own text.
    #[test]
    fn every_one_of_many_hunks_answers_its_own_text() {
        let hunks: Vec<(LineNumber, Vec<u8>)> = (0..1000u32)
            .rev()
            .map(|n| at(n * 10, &format!("fn f{n}()")))
            .collect();
        let context = FunctionContext::read_at(Context::lines(3), hunks);
        for n in 0..1000u32 {
            assert_eq!(
                context.of(header(n * 10, 4)),
                Some(format!("fn f{n}()").as_bytes()),
                "hunk {n}"
            );
            assert_eq!(context.of(header(n * 10 + 1, 4)), None);
        }
    }
}
