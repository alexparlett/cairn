//! Both versions of one file as lines, and the exact ranges between them that differ.
//!
//! This is the one exact answer of decision L2. Hunks, rows and patches are projections
//! of it and hold no state of their own. It deliberately has no room for the
//! whitespace-ignoring ranges: those are display-only, they live in
//! [`crate::DisplayOverlay`], and keeping them out of this type is what makes them
//! unreachable from the patch emitter by construction rather than by care (R1.7).

use std::borrow::Cow;

/// A line's place on its own side of a diff.
///
/// Stored counting from zero, which is how the lines are indexed; [`LineNumber::one_based`]
/// is the number a gutter and a hunk header show. One type, so the two readings cannot be
/// mixed up silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineNumber(u32);

impl LineNumber {
    pub fn from_index(index: u32) -> Self {
        Self(index)
    }

    pub fn index(self) -> u32 {
        self.0
    }

    /// What a gutter and a hunk header spell, counting from one.
    pub fn one_based(self) -> u32 {
        self.0.saturating_add(1)
    }
}

/// A run of lines on one side, by index and length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineSpan {
    start: LineNumber,
    len: u32,
}

impl LineSpan {
    pub fn new(start: LineNumber, len: u32) -> Self {
        Self { start, len }
    }

    pub fn at(start_index: u32, len: u32) -> Self {
        Self::new(LineNumber::from_index(start_index), len)
    }

    pub fn start(self) -> LineNumber {
        self.start
    }

    pub fn len(self) -> u32 {
        self.len
    }

    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// One past the last line, so an empty span's end is its start.
    pub fn end(self) -> LineNumber {
        LineNumber::from_index(self.start.index().saturating_add(self.len))
    }

    pub fn contains(self, line: LineNumber) -> bool {
        line >= self.start && line < self.end()
    }

    pub fn numbers(self) -> impl Iterator<Item = LineNumber> {
        (self.start.index()..self.end().index()).map(LineNumber::from_index)
    }
}

/// One line of a file: its bytes without the terminator, and whether it had one.
///
/// A file whose last line runs off the end without a newline is a different file from one
/// that ends cleanly, and a patch has to say which it is — so the fact travels with the
/// line rather than being inferred from the content around it. A `\r` of a CRLF ending
/// stays in `bytes`: git's terminator is the `\n` alone.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiffLine {
    bytes: Vec<u8>,
    terminated: bool,
}

impl DiffLine {
    /// A line that ended in a newline.
    pub fn terminated(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            terminated: true,
        }
    }

    /// A last line that ran off the end of the file.
    pub fn unterminated(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            terminated: false,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn ends_with_newline(&self) -> bool {
        self.terminated
    }

    /// Text for a human. Borrowed when the bytes are UTF-8, so a redraw allocates nothing —
    /// but it reads the whole line every call to decide that, and copies it when they are
    /// not. A row that only needs a prefix (R6.9's truncated long line) should slice
    /// [`Self::bytes`] first; over the limit R2.6 allows loading anyway, this is the
    /// difference between microseconds and milliseconds a frame.
    pub fn text(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }

    /// Appends the line as it stands in the file, terminator and all.
    fn write_into(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.bytes);
        if self.terminated {
            out.push(b'\n');
        }
    }
}

/// One contiguous difference: the lines it removed and the lines it added.
///
/// Either side may be empty — an empty `removed` is an insertion, an empty `added` a
/// deletion — and an empty span still says where, so an insertion knows what it follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangedRange {
    pub removed: LineSpan,
    pub added: LineSpan,
}

impl ChangedRange {
    pub fn new(removed: LineSpan, added: LineSpan) -> Self {
        Self { removed, added }
    }

    pub fn is_insertion(self) -> bool {
        self.removed.is_empty()
    }

    pub fn is_removal(self) -> bool {
        self.added.is_empty()
    }
}

/// Both versions of one file and the exact ranges that differ (R1.2).
///
/// A patch is built from this and nothing else. Reaching the whitespace-ignoring ranges
/// from here does not compile, because they are not here:
///
/// ```
/// # use cairn_model::{ChangedRange, DiffLine, LineSpan, TextDiff};
/// let text = TextDiff::new(
///     vec![DiffLine::terminated("a")],
///     vec![DiffLine::terminated("b")],
///     vec![ChangedRange::new(LineSpan::at(0, 1), LineSpan::at(0, 1))],
/// );
/// let _ = text.changes();
/// ```
///
/// The same, asking instead for what only a view may see. Stable rustdoc checks that a
/// `compile_fail` block fails, not why, so the block differs from the one above by its
/// last line alone:
///
/// ```compile_fail
/// # use cairn_model::{ChangedRange, DiffLine, LineSpan, TextDiff};
/// let text = TextDiff::new(
///     vec![DiffLine::terminated("a")],
///     vec![DiffLine::terminated("b")],
///     vec![ChangedRange::new(LineSpan::at(0, 1), LineSpan::at(0, 1))],
/// );
/// let _ = text.changes_ignoring_whitespace();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDiff {
    old: Vec<DiffLine>,
    new: Vec<DiffLine>,
    changes: Vec<ChangedRange>,
}

impl TextDiff {
    /// `changes` is expected in increasing order and not overlapping, which is the order
    /// a diff produces them in. Nothing downstream sorts it: a projection reads it as
    /// given, so a caller that shuffles it gets shuffled rows and a shuffled patch.
    ///
    /// Two further conditions are structure rather than style, and every projection of
    /// this type already depends on them:
    ///
    /// - **The unchanged run between two consecutive changes is the same length on both
    ///   sides**, and so is the run from the start of the file to the first change. Both
    ///   [`crate::emit_patch`] and the row index measure that run as the *smaller* of the
    ///   two gaps, which loses nothing only while they are equal. Unequal gaps drop their
    ///   difference from the body, while a hunk header is recounted from the lines actually
    ///   emitted — so the result is an internally consistent patch that omits content its
    ///   own span claims to cover, which real `git apply` then rejects on a context
    ///   mismatch and nothing in this crate notices.
    /// - **Every range lies inside its own side's lines**: `removed` within
    ///   [`Self::old_lines`], `added` within [`Self::new_lines`]. A line asked for past the
    ///   end answers `None`, and the projections skip such a line rather than fail, which
    ///   shortens the hunk by exactly as many lines as were out of range.
    ///
    /// All three — the order and non-overlap above included — are checked by
    /// `debug_assert!`, so the producer of these values fails loudly in a debug build and
    /// under `cargo test` instead of emitting a quietly wrong patch. A release build
    /// compiles the check away and keeps the crate's existing behaviour: malformed input is
    /// drawn short, never panicked on in front of a user.
    ///
    /// One run is deliberately left out: the trailing one, from the end of the last change
    /// to the end of each side. It has the same shape, but a `TextDiff` is also built as a
    /// container for one side's content alone, and refusing that is a wider precondition
    /// than the projections need.
    pub fn new(old: Vec<DiffLine>, new: Vec<DiffLine>, changes: Vec<ChangedRange>) -> Self {
        let text = Self { old, new, changes };
        #[cfg(debug_assertions)]
        text.debug_assert_changes_are_well_formed();
        text
    }

    /// The structural half of [`Self::new`]'s contract, checked where the producer of a
    /// diff can see it fail. Gated on `debug_assertions` rather than left to the macro
    /// alone so a release build walks no changes at all.
    #[cfg(debug_assertions)]
    fn debug_assert_changes_are_well_formed(&self) {
        let old_len = u32::try_from(self.old.len()).unwrap_or(u32::MAX);
        let new_len = u32::try_from(self.new.len()).unwrap_or(u32::MAX);
        let mut old = 0u32;
        let mut new = 0u32;
        for (index, change) in self.changes.iter().enumerate() {
            let removed_start = change.removed.start().index();
            let added_start = change.added.start().index();
            debug_assert!(
                removed_start >= old && added_start >= new,
                "change {index} starts at old {removed_start} / new {added_start}, behind the \
                 old {old} / new {new} the change before it ended at: changes must be in \
                 increasing order and must not overlap"
            );
            debug_assert_eq!(
                removed_start - old,
                added_start - new,
                "change {index} follows an unchanged run of {} lines on the old side and {} on \
                 the new one; a projection measures that run as the smaller of the two, so the \
                 difference would be dropped from the body of a patch whose header still \
                 claims it",
                removed_start - old,
                added_start - new
            );
            debug_assert!(
                change.removed.end().index() <= old_len,
                "change {index} removes lines up to {} of an old side that has {old_len}",
                change.removed.end().index()
            );
            debug_assert!(
                change.added.end().index() <= new_len,
                "change {index} adds lines up to {} of a new side that has {new_len}",
                change.added.end().index()
            );
            old = change.removed.end().index();
            new = change.added.end().index();
        }
    }

    pub fn old_lines(&self) -> &[DiffLine] {
        &self.old
    }

    pub fn new_lines(&self) -> &[DiffLine] {
        &self.new
    }

    pub fn old_line(&self, line: LineNumber) -> Option<&DiffLine> {
        self.old.get(line.index() as usize)
    }

    pub fn new_line(&self, line: LineNumber) -> Option<&DiffLine> {
        self.new.get(line.index() as usize)
    }

    /// The exact changed ranges, and the only source a patch is allowed to read (R1.7).
    pub fn changes(&self) -> &[ChangedRange] {
        &self.changes
    }

    /// The whole old side as it sits on disk. Materialises the file: the projections
    /// never call it, and a view has no reason to.
    pub fn old_content(&self) -> Vec<u8> {
        Self::content_of(&self.old)
    }

    /// The whole new side as it sits on disk, with the same warning as [`Self::old_content`].
    pub fn new_content(&self) -> Vec<u8> {
        Self::content_of(&self.new)
    }

    fn content_of(lines: &[DiffLine]) -> Vec<u8> {
        let mut out = Vec::new();
        for line in lines {
            line.write_into(&mut out);
        }
        out
    }
}

/// Splits file content into lines the way git's own diff does: a `\n` ends a line and a
/// trailing run with no `\n` is a last line that never ended.
pub fn split_lines(content: &[u8]) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut rest = content;
    while let Some(at) = rest.iter().position(|b| *b == b'\n') {
        lines.push(DiffLine::terminated(&rest[..at]));
        rest = &rest[at + 1..];
    }
    if !rest.is_empty() {
        lines.push(DiffLine::unterminated(rest));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: counting a line number from zero where a header or a gutter shows it.
    #[test]
    fn a_line_number_reads_as_an_index_and_as_what_a_gutter_shows() {
        let first = LineNumber::from_index(0);
        assert_eq!(first.index(), 0);
        assert_eq!(
            first.one_based(),
            1,
            "the first line did not show as line 1"
        );
        assert_eq!(LineNumber::from_index(41).one_based(), 42);
        assert_eq!(
            LineNumber::from_index(u32::MAX).one_based(),
            u32::MAX,
            "the last representable line wrapped instead of saturating"
        );
    }

    /// Pins the half-open reading: an empty span still says where it sits, which is what
    /// an insertion's `-N,0` is built from.
    #[test]
    fn an_empty_span_ends_where_it_starts_and_holds_nothing() {
        let empty = LineSpan::at(5, 0);
        assert!(empty.is_empty());
        assert_eq!(empty.start(), empty.end());
        assert_eq!(
            empty.start().index(),
            5,
            "an empty span forgot where it was"
        );
        assert!(!empty.contains(LineNumber::from_index(5)));
        assert_eq!(empty.numbers().count(), 0);
    }

    #[test]
    fn a_span_holds_its_first_line_and_not_the_one_past_its_last() {
        let span = LineSpan::at(2, 3);
        assert_eq!(span.end().index(), 5);
        assert!(span.contains(LineNumber::from_index(2)));
        assert!(span.contains(LineNumber::from_index(4)));
        assert!(!span.contains(LineNumber::from_index(5)));
        assert!(!span.contains(LineNumber::from_index(1)));
        let numbers: Vec<u32> = span.numbers().map(LineNumber::index).collect();
        assert_eq!(numbers, vec![2, 3, 4]);
    }

    /// Caught by: splitting on `\n` and dropping the fact that the last line had none,
    /// which is the whole reason a patch carries `\ No newline at end of file`.
    #[test]
    fn splitting_and_rejoining_returns_the_same_bytes() {
        for content in [
            &b""[..],
            b"\n",
            b"one\n",
            b"one",
            b"a\nb\nc\n",
            b"a\nb\nc",
            b"\n\n\n",
            b"crlf\r\nkept\r\n",
            b"crlf\r\nkept\r",
        ] {
            let lines = split_lines(content);
            let diff = TextDiff::new(lines.clone(), Vec::new(), Vec::new());
            assert_eq!(
                diff.old_content(),
                content,
                "{content:?} did not survive being split into lines"
            );
        }
    }

    #[test]
    fn a_last_line_with_no_newline_says_so_and_one_with_a_newline_says_so() {
        let ended = split_lines(b"a\nb\n");
        assert_eq!(ended.len(), 2);
        assert!(ended.iter().all(DiffLine::ends_with_newline));

        let ran_off = split_lines(b"a\nb");
        assert_eq!(ran_off.len(), 2);
        assert!(ran_off[0].ends_with_newline());
        assert!(
            !ran_off[1].ends_with_newline(),
            "an unterminated last line claimed a newline"
        );
        assert_eq!(ran_off[1].bytes(), b"b");
    }

    /// The `\r` belongs to the line: git's terminator is the `\n` alone, and a patch that
    /// dropped the `\r` would not apply to a CRLF preimage.
    #[test]
    fn a_carriage_return_stays_in_the_line_it_belongs_to() {
        let lines = split_lines(b"a\r\n");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].bytes(), b"a\r");
        assert!(lines[0].ends_with_newline());
    }

    #[test]
    fn an_empty_file_has_no_lines_at_all() {
        assert!(split_lines(b"").is_empty());
        let diff = TextDiff::new(Vec::new(), Vec::new(), Vec::new());
        assert_eq!(diff.old_content(), b"");
        assert!(diff.old_lines().is_empty());
    }

    #[test]
    fn a_change_says_which_direction_it_goes_in() {
        let insertion = ChangedRange::new(LineSpan::at(3, 0), LineSpan::at(3, 2));
        assert!(insertion.is_insertion());
        assert!(!insertion.is_removal());

        let removal = ChangedRange::new(LineSpan::at(3, 2), LineSpan::at(3, 0));
        assert!(removal.is_removal());
        assert!(!removal.is_insertion());

        let replacement = ChangedRange::new(LineSpan::at(3, 2), LineSpan::at(3, 1));
        assert!(!replacement.is_insertion());
        assert!(!replacement.is_removal());
    }

    #[test]
    fn a_line_is_reached_by_its_number_on_its_own_side() {
        let text = TextDiff::new(
            split_lines(b"a\nb\n"),
            split_lines(b"a\nB\nc\n"),
            vec![ChangedRange::new(LineSpan::at(1, 1), LineSpan::at(1, 2))],
        );
        assert_eq!(
            text.old_line(LineNumber::from_index(1))
                .map(DiffLine::bytes),
            Some(&b"b"[..])
        );
        assert_eq!(
            text.new_line(LineNumber::from_index(2))
                .map(DiffLine::bytes),
            Some(&b"c"[..])
        );
        assert!(
            text.old_line(LineNumber::from_index(2)).is_none(),
            "a line past the end of the old side answered"
        );
        assert_eq!(text.new_content(), b"a\nB\nc\n");
    }

    fn lines(count: u32) -> Vec<DiffLine> {
        (0..count)
            .map(|n| DiffLine::terminated(format!("l{n}")))
            .collect()
    }

    fn change(removed: (u32, u32), added: (u32, u32)) -> ChangedRange {
        ChangedRange::new(
            LineSpan::at(removed.0, removed.1),
            LineSpan::at(added.0, added.1),
        )
    }

    /// The shape every projection is written against: each change follows a run of
    /// unchanged lines that is the same length on both sides, and no range reaches past its
    /// own side. The passing twin of the three refusals below — without it they would stay
    /// green against an assertion that fired on everything.
    #[test]
    fn a_well_formed_diff_with_several_changes_is_accepted() {
        let text = TextDiff::new(
            lines(10),
            lines(11),
            vec![
                change((1, 1), (1, 2)),
                change((4, 2), (5, 2)),
                change((9, 1), (10, 1)),
            ],
        );
        assert_eq!(text.changes().len(), 3);
    }

    /// The precondition the `min` in every projection quietly depends on. A run of three
    /// unchanged lines on one side and two on the other is measured as two, so the third is
    /// dropped from a patch body whose recounted header still claims it — and real
    /// `git apply` rejects the result on a context mismatch.
    ///
    /// `cfg(debug_assertions)`, because that is where the check lives: a release build must
    /// keep drawing a malformed diff short rather than panicking at a user.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "unchanged run of 3 lines on the old side and 2 on")]
    fn an_unchanged_run_of_two_different_lengths_is_refused_in_a_debug_build() {
        let _ = TextDiff::new(
            lines(10),
            lines(10),
            vec![change((1, 1), (1, 1)), change((5, 1), (4, 1))],
        );
    }

    /// The milder instance of the same defect: a line asked for past the end of its side
    /// answers `None`, and every projection skips such a line rather than failing, which
    /// shortens the hunk by exactly as much.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "removes lines up to 5 of an old side that has 2")]
    fn a_change_reaching_past_the_end_of_its_side_is_refused_in_a_debug_build() {
        let _ = TextDiff::new(lines(2), lines(2), vec![change((0, 5), (0, 5))]);
    }

    /// The order the doc comment has always promised, now said out loud where a producer
    /// can hear it.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "behind the old 6 / new 6")]
    fn changes_that_go_backwards_are_refused_in_a_debug_build() {
        let _ = TextDiff::new(
            lines(10),
            lines(10),
            vec![change((5, 1), (5, 1)), change((1, 1), (1, 1))],
        );
    }

    #[test]
    fn a_line_reads_as_text_without_copying_when_it_is_utf8() {
        let line = DiffLine::terminated("fn main() {}");
        assert!(matches!(line.text(), Cow::Borrowed(_)));
        assert_eq!(line.text(), "fn main() {}");
        assert!(
            DiffLine::terminated(vec![0xff]).text().contains('\u{fffd}'),
            "bytes that are not UTF-8 read back without a replacement"
        );
    }
}
