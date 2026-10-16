//! One file's diff as the diff views show it: the answer, both row projections built once at
//! the context it was asked at, and what drawing it needs measured once — whether ignoring
//! whitespace hides a change, the widest line as drawn, the gutter's digits.
//!
//! Built where the answer is computed (the application's diff thread), never on the thread
//! that draws: both indexes are proportional to the diff's changes, and the widest line is a
//! pass over every drawn byte — about 39 ms for a 64 MiB file loaded past the limits. A view
//! then reads rows from it one at a time ([`UnifiedLayout::row`], [`SideBySideLayout::row`]).
//!
//! **A line past the long-line limit is drawn cut**. Only a diff loaded past the default ceilings
//! ([`DiffLimits`]) holds one — the engine refuses any other with a line longer than
//! [`DiffLimits::MAX_LINE_BYTES`] — and drawn whole it would cost its whole length per row and a
//! horizontal extent of its whole width. [`drawn_bytes`] is the cut every view draws to, and the
//! widest line is measured to it.
//!
//! **What it was drawn from**: a diff keyed with what it was asked as ([`ShownDiff::keyed`])
//! carries a hash of that and of the whole answer — its paths, modes and ids, every line of both
//! sides and the ranges that differ, the context it is drawn at — computed where it is built. Two
//! answers with the same [`ShownDiff::content`] draw the same rows over the same text, so a line
//! selection made over one names the same lines of the other: what lets a refresh that re-reads a
//! file unchanged keep a selection, and one that reads another byte drop it.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use crate::{
    ChangeStops, Context, DiffContent, DiffLimits, DiffLine, DisplayOverlay, FileDiff,
    SideBySideLayout, TextDiff, UnifiedLayout,
};

/// How many of a line's bytes a view draws at most: the long-line limit.
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
/// context it was asked at. The answer is shared ([`Self::shared_diff`]), so what the staging
/// gesture hands a write is the answer drawn, never a copy of it made on the thread that draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownDiff {
    diff: Arc<FileDiff>,
    context: Context,
    /// `None` for a file that is not text.
    unified: Option<UnifiedLayout>,
    side_by_side: Option<SideBySideLayout>,
    hides_changes: bool,
    widest_columns: usize,
    has_cut_line: bool,
    number_digits: usize,
    /// What it was drawn from, once keyed ([`Self::keyed`]); 0 before.
    content: u64,
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
            diff: Arc::new(diff),
            context,
            unified,
            side_by_side,
            hides_changes,
            widest_columns,
            has_cut_line,
            number_digits: digits,
            content: 0,
        }
    }

    /// The same diff keyed with what it was asked as — `asked`, the caller's question: for a
    /// working-tree diff its path, its side and its options. Hashes `asked`, the context and the
    /// whole answer (the patch's text, not the view's highlights), so it costs a pass over the
    /// answer's bytes: done where the diff is built, never on the thread that draws.
    pub fn keyed(mut self, asked: &impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        asked.hash(&mut hasher);
        self.context.hash(&mut hasher);
        answer_hash(&self.diff, &mut hasher);
        self.content = hasher.finish();
        self
    }

    /// What the diff was drawn from ([`Self::keyed`]): equal for two answers to the same
    /// question whose text is the same in every byte, and — but for a 64-bit collision —
    /// different for any other. 0 for a diff never keyed.
    pub fn content(&self) -> u64 {
        self.content
    }

    pub fn diff(&self) -> &FileDiff {
        &self.diff
    }

    /// The answer drawn, shared: a handle, not a copy, whatever the file's size.
    pub fn shared_diff(&self) -> Arc<FileDiff> {
        Arc::clone(&self.diff)
    }

    /// The answer itself, for a caller letting go of it: moved out when nothing else shares
    /// it, copied otherwise.
    pub fn into_diff(self) -> FileDiff {
        Arc::unwrap_or_clone(self.diff)
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

    /// Whether ignoring whitespace hides a change that is really there — false when
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

/// Every part of `diff` a patch or a drawn row is made from, into `hasher`: the file — paths,
/// status, modes, ids — and its content, each state by name. Of a text diff, both sides' lines,
/// the exact ranges and the whitespace-ignoring ones; the intra-line highlights and git's
/// function context are drawn from those and add nothing.
fn answer_hash(diff: &FileDiff, hasher: &mut DefaultHasher) {
    diff.file.hash(hasher);
    match &diff.content {
        DiffContent::Text { text, overlay } => {
            0u8.hash(hasher);
            text.hash(hasher);
            overlay.changes_ignoring_whitespace().hash(hasher);
        }
        DiffContent::Binary { old_size, new_size } => (1u8, old_size, new_size).hash(hasher),
        DiffContent::TooLarge { crossed, loadable } => (2u8, crossed, loadable).hash(hasher),
        DiffContent::LfsPointer { old, new } => (3u8, old, new).hash(hasher),
        DiffContent::Submodule {
            old_target,
            new_target,
            dirty,
        } => (4u8, old_target, new_target, dirty).hash(hasher),
        DiffContent::ModeChangeOnly => 5u8.hash(hasher),
        DiffContent::Conflicted => 6u8.hash(hasher),
        DiffContent::Unsupported { reason } => (7u8, reason).hash(hasher),
    }
}

fn digits(count: usize) -> usize {
    count.max(1).to_string().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangeStatus, ChangedFile, ChangedRange, DiffContent, LineSpan, RepoPath};

    /// A line longer than the long-line limit is drawn to the limit, never past it,
    /// ending on a character — a two-, three- or four-byte character straddling the cut is
    /// left out whole; at the limit exactly nothing is cut, and one byte past it is cut to
    /// it. Caught by: drawing the whole line, cutting inside a character (backing off fewer
    /// than three continuation bytes), or the cut drifting from the limit's 2,048 bytes.
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

        // The limit's number, written out, so the cut cannot drift from it unseen.
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

    /// The answer drawn is shared, never copied: a handle to it is the same allocation, and
    /// letting go of the view's last hold moves it out whole. Caught by: a `diff()` copied into
    /// each handle (the staging gesture copying a whole file per action on the UI thread).
    #[test]
    fn the_answer_drawn_is_shared_not_copied() {
        let file = ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("f"),
            new_path: RepoPath::from("f"),
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        };
        let diff = FileDiff {
            file,
            content: DiffContent::ModeChangeOnly,
        };
        let shown = ShownDiff::new(diff.clone(), Context::lines(3));
        let handle = shown.shared_diff();
        assert!(std::ptr::eq(&*handle, shown.diff()), "the handle is a copy");
        assert_eq!(Arc::strong_count(&handle), 2);
        drop(handle);
        assert_eq!(shown.into_diff(), diff);
    }

    /// A text diff of `new` against three lines, its one change at line 2.
    fn edited(path: &str, new: &[u8]) -> FileDiff {
        FileDiff {
            file: ChangedFile {
                status: ChangeStatus::Modified,
                old_path: RepoPath::from(path),
                new_path: RepoPath::from(path),
                old_mode: None,
                new_mode: None,
                old_id: None,
                new_id: None,
            },
            content: DiffContent::Text {
                text: TextDiff::new(
                    crate::split_lines(b"a\nb\nc\n"),
                    crate::split_lines(new),
                    vec![ChangedRange::new(LineSpan::at(1, 1), LineSpan::at(1, 1))],
                ),
                overlay: DisplayOverlay::none(),
            },
        }
    }

    /// Two answers to one question whose text is the same in every byte are
    /// one content, built apart; one byte of another line, another path, another question (the
    /// side or the options a caller keys with) or another context is another; a diff never
    /// keyed is 0. Caught by: a content of the drawn rows alone (a byte changed outside the
    /// context missed), of the text alone (the staged diff's selection kept over the unstaged
    /// one), or the context left out (a selection's rows kept across a wider context).
    #[test]
    fn a_diffs_content_is_its_question_and_every_byte_of_its_answer() {
        let at = Context::lines(3);
        let content = |diff: FileDiff, asked: &str, context: Context| {
            ShownDiff::new(diff, context).keyed(&asked).content()
        };
        let one = content(edited("f", b"a\nB\nc\n"), "unstaged", at);
        assert_eq!(one, content(edited("f", b"a\nB\nc\n"), "unstaged", at));
        assert_ne!(one, 0);
        for (other, why) in [
            (
                content(edited("f", b"a\nB\nC\n"), "unstaged", at),
                "a byte of another line",
            ),
            (
                content(edited("f", b"a\nb!\nc\n"), "unstaged", at),
                "a byte of the changed line",
            ),
            (
                content(edited("g", b"a\nB\nc\n"), "unstaged", at),
                "another path",
            ),
            (
                content(edited("f", b"a\nB\nc\n"), "staged", at),
                "another question",
            ),
            (
                content(edited("f", b"a\nB\nc\n"), "unstaged", Context::lines(4)),
                "another context",
            ),
        ] {
            assert_ne!(one, other, "{why} kept the content");
        }
        assert_eq!(ShownDiff::new(edited("f", b"a\nB\nc\n"), at).content(), 0);
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
