//! One file's diff as the diff views show it: the answer, both row projections built once at
//! the context it was asked at, and what drawing it needs measured once — whether ignoring
//! whitespace hides a change, the widest line as drawn, the gutter's digits.
//!
//! Built where the answer is computed (the application's diff thread), never on the thread
//! that draws: both indexes are proportional to the diff's changes, and the widest line is a
//! pass over every drawn byte — about 39 ms for a 64 MiB file loaded past the limits. A view
//! then reads rows from it one at a time ([`UnifiedLayout::row`], [`SideBySideLayout::row`]).
//!
//! **A line past the long-line limit is drawn cut** (R6.9). Only a diff loaded past R2.6's
//! ceilings holds one — the engine refuses any other with a line longer than
//! [`DiffLimits::MAX_LINE_BYTES`] — and drawn whole it would cost its whole length per row and
//! a horizontal extent of its whole width. [`drawn_bytes`] is the cut every view draws to, and
//! the widest line is measured to it.

use crate::{
    ChangeStops, Context, DiffLimits, DiffLine, DisplayOverlay, FileDiff, SideBySideLayout,
    TextDiff, UnifiedLayout,
};

/// How many of a line's bytes a view draws at most: the long-line limit (R2.6, R6.9).
pub const LINE_CUT_BYTES: usize = DiffLimits::MAX_LINE_BYTES as usize;

/// The column a tab moves to a multiple of, as `git diff` shows it in a terminal.
pub const TAB_STOP: usize = 8;

/// The bytes of `bytes` a view draws, and whether that is fewer than all of them: at most
/// [`LINE_CUT_BYTES`], ending where a UTF-8 character ends.
pub fn drawn_bytes(bytes: &[u8]) -> (&[u8], bool) {
    if bytes.len() <= LINE_CUT_BYTES {
        return (bytes, false);
    }
    // Back off past continuation bytes (`10xxxxxx`) so the cut never splits a character; at
    // most three, since a UTF-8 character is at most four bytes.
    let mut end = LINE_CUT_BYTES;
    while end > LINE_CUT_BYTES.saturating_sub(3)
        && bytes.get(end).is_some_and(|byte| byte & 0xC0 == 0x80)
    {
        end -= 1;
    }
    (bytes.get(..end).unwrap_or(bytes), true)
}

/// At least as many columns as the widest of `lines` draws, and whether any line is cut: a
/// byte draws at most one column (a two-column character is at least three bytes) and a tab
/// at most [`TAB_STOP`], counted over the drawn bytes only. One pass, done once per answer.
pub fn widest_drawn_columns<'a>(lines: impl Iterator<Item = &'a DiffLine>) -> (usize, bool) {
    lines.fold((0, false), |(widest, any_cut), line| {
        let (bytes, cut) = drawn_bytes(line.bytes());
        let tabs = bytes.iter().filter(|byte| **byte == b'\t').count();
        (
            widest.max(bytes.len() + tabs * (TAB_STOP - 1)),
            any_cut || cut,
        )
    })
}

/// One file's diff as the views draw it: built once when the answer is computed, at the
/// context it was asked at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownDiff {
    diff: FileDiff,
    context: Context,
    /// `None` for a file that is not text.
    unified: Option<UnifiedLayout>,
    side_by_side: Option<SideBySideLayout>,
    hides_changes: bool,
    widest_columns: usize,
    has_cut_line: bool,
    number_digits: usize,
}

impl ShownDiff {
    /// Builds what both views need of `diff`, asked at `context`: proportional to its changes
    /// and one pass over its drawn bytes, never repeated per frame or per toggle.
    pub fn new(diff: FileDiff, context: Context) -> Self {
        let (unified, side_by_side, hides_changes, (widest_columns, has_cut_line), digits) =
            match (diff.text(), diff.overlay()) {
                (Some(text), Some(overlay)) => (
                    Some(UnifiedLayout::shown(text, overlay, context)),
                    Some(SideBySideLayout::shown(text, overlay, context)),
                    overlay.hides_a_change(text),
                    widest_drawn_columns(text.old_lines().iter().chain(text.new_lines())),
                    digits(text.old_lines().len().max(text.new_lines().len())),
                ),
                _ => (None, None, false, (0, false), 1),
            };
        Self {
            diff,
            context,
            unified,
            side_by_side,
            hides_changes,
            widest_columns,
            has_cut_line,
            number_digits: digits,
        }
    }

    pub fn diff(&self) -> &FileDiff {
        &self.diff
    }

    /// The answer itself, for a caller letting go of it.
    pub fn into_diff(self) -> FileDiff {
        self.diff
    }

    pub fn context(&self) -> Context {
        self.context
    }

    /// The unified rows, for a text diff.
    pub fn layout(&self) -> Option<&UnifiedLayout> {
        self.unified.as_ref()
    }

    /// The side-by-side rows, for a text diff.
    pub fn side_by_side_layout(&self) -> Option<&SideBySideLayout> {
        self.side_by_side.as_ref()
    }

    /// How many unified rows there are.
    pub fn row_count(&self) -> usize {
        self.unified.as_ref().map_or(0, UnifiedLayout::len)
    }

    /// How many rows a view draws, side by side or not.
    pub fn rows(&self, side_by_side: bool) -> usize {
        if side_by_side {
            self.side_by_side.as_ref().map_or(0, SideBySideLayout::len)
        } else {
            self.row_count()
        }
    }

    /// The changes previous and next change stop at, in the rows a view draws.
    pub fn stops(&self, side_by_side: bool) -> Option<&ChangeStops> {
        if side_by_side {
            self.side_by_side.as_ref().map(SideBySideLayout::stops)
        } else {
            self.unified.as_ref().map(UnifiedLayout::stops)
        }
    }

    /// Whether ignoring whitespace hides a change that is really there (R6.7) — false when
    /// whitespace is not ignored, and false when ignoring it hides nothing.
    pub fn hides_changes(&self) -> bool {
        self.hides_changes
    }

    /// The widest line's columns as drawn: to its cut, for a line past the limit.
    pub fn widest_columns(&self) -> usize {
        self.widest_columns
    }

    /// Whether a line is drawn cut, so a view leaves room for its marker.
    pub fn has_cut_line(&self) -> bool {
        self.has_cut_line
    }

    /// How many digits the largest line number has.
    pub fn number_digits(&self) -> usize {
        self.number_digits
    }

    /// The text diff and its overlay, for a file that is text.
    pub fn text(&self) -> Option<(&TextDiff, &DisplayOverlay)> {
        self.diff.text().zip(self.diff.overlay())
    }
}

fn digits(count: usize) -> usize {
    count.max(1).to_string().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangeStatus, ChangedFile, ChangedRange, DiffContent, LineSpan, RepoPath};

    /// R6.9: a line longer than the long-line limit is drawn to the limit, never past it,
    /// ending on a character — a two-, three- or four-byte character straddling the cut is
    /// left out whole; at the limit exactly nothing is cut, and one byte past it is cut to
    /// it. Caught by: drawing the whole line, cutting inside a character (backing off fewer
    /// than three continuation bytes), or the cut drifting from R2.6's 2,048 bytes.
    #[test]
    fn a_line_past_the_limit_is_cut_at_the_limit_on_a_character() {
        let long = vec![b'a'; 3 * 1024 * 1024];
        let (drawn, cut) = drawn_bytes(&long);
        assert!(cut);
        assert_eq!(drawn.len(), LINE_CUT_BYTES);

        // Each character starts so that its last byte is the first past the cut: two bytes
        // at the cut less one, three at less two, four at less three.
        for (character, starts_at) in [
            ("é", LINE_CUT_BYTES - 1),
            ("€", LINE_CUT_BYTES - 2),
            ("😀", LINE_CUT_BYTES - 3),
        ] {
            let straddling = format!("{}{character}{}", "a".repeat(starts_at), "b".repeat(10));
            assert!(straddling.is_char_boundary(starts_at));
            assert!(!straddling.is_char_boundary(LINE_CUT_BYTES));
            let (drawn, cut) = drawn_bytes(straddling.as_bytes());
            assert!(cut, "{character}");
            assert_eq!(drawn.len(), starts_at, "{character} was not left out whole");
            assert!(std::str::from_utf8(drawn).is_ok(), "{character}");
        }

        // R2.6's number, written out, so the cut cannot drift from it unseen.
        let (drawn, cut) = drawn_bytes(&[b'a'; 2_049]);
        assert!(cut, "a line one byte past the limit is drawn whole");
        assert_eq!(drawn.len(), 2_048);
        let (drawn, cut) = drawn_bytes(&[b'a'; 2_048]);
        assert!(!cut);
        assert_eq!(drawn.len(), 2_048);

        assert!(!drawn_bytes("a".repeat(LINE_CUT_BYTES).as_bytes()).1);
    }

    /// The widest line is bounded from above by its drawn bytes and tabs, and a line past the
    /// limit counts to its cut, never its length. Caught by: a width bound from the line's
    /// whole length (a 64 MiB line is hundreds of millions of pixels wide).
    #[test]
    fn the_widest_line_is_counted_to_its_cut() {
        let lines = [
            DiffLine::terminated("short"),
            DiffLine::terminated("\t\tx"),
            DiffLine::unterminated("é"),
        ];
        assert_eq!(widest_drawn_columns(lines.iter()), (17, false));
        let long = DiffLine::terminated("a".repeat(3 * 1024 * 1024));
        assert_eq!(
            widest_drawn_columns(std::iter::once(&long)),
            (LINE_CUT_BYTES, true)
        );
        assert_eq!(widest_drawn_columns(std::iter::empty()), (0, false));
    }

    /// Both projections are built once, at the context asked, and a file that is not text
    /// has neither. Caught by: building only the projection drawn now (a toggle would then
    /// build the other on the thread that draws).
    #[test]
    fn both_projections_are_built_with_the_answer() {
        let file = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("f"),
            new_path: RepoPath::from("f"),
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        };
        let text = TextDiff::new(
            crate::split_lines(b"a\nb\nc\n"),
            crate::split_lines(b"a\nB\nC\nD\nc\n"),
            vec![ChangedRange::new(LineSpan::at(1, 1), LineSpan::at(1, 3))],
        );
        let shown = ShownDiff::new(
            FileDiff {
                file: file.clone(),
                content: DiffContent::Text {
                    text,
                    overlay: DisplayOverlay::none(),
                },
            },
            Context::lines(3),
        );
        assert_eq!(shown.rows(false), 1 + 1 + 1 + 3 + 1);
        assert_eq!(shown.rows(true), 1 + 1 + 3 + 1);
        assert_eq!(shown.stops(true).map(ChangeStops::len), Some(1));
        assert_eq!(shown.number_digits(), 1);

        let binary = ShownDiff::new(
            FileDiff {
                file,
                content: DiffContent::Binary {
                    old_size: 1,
                    new_size: 2,
                },
            },
            Context::lines(3),
        );
        assert_eq!((binary.rows(false), binary.rows(true)), (0, 0));
        assert!(binary.stops(false).is_none());
    }
}
