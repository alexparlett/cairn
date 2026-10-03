//! Rows a view draws, reached one at a time.
//!
//! Both projections of R1.4 live here, over one addressing scheme. Neither builds a row
//! until it is asked for one (R1.5): what is built up front is an index of the diff's
//! *changes*, which is as long as the file has separate changes and never as long as the
//! file has lines. A row borrows its line from the diff — the row types are `Copy`, so
//! they cannot own one — which is what makes asking for a row cost nothing to allocate.
//!
//! The unified rows are what `git diff` prints (phase 06's parity rule): a hunk's header,
//! its lines with a context line read from the NEW side — the side git prints it from,
//! which differs from the old one under `-w` — and git's `\ No newline at end of file`
//! after a line that did not end. A view drawing them with whitespace ignored groups the
//! whitespace-ignoring ranges ([`UnifiedLayout::shown`]), as `git diff -w` does.

use std::ops::Range;

use crate::{
    ChangedRange, Context, DiffLine, DisplayOverlay, HunkHeader, Hunks, LineNumber, TextDiff,
};

/// A stretch of rows of one kind. The index holds one per context run and one per change,
/// plus one per hunk header and per end-of-file marker — never one per row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Piece {
    Header {
        hunk: u32,
    },
    Context {
        old: u32,
        new: u32,
        len: u32,
    },
    Change {
        change: u32,
    },
    /// git's `\ No newline at end of file` after a context run that ends the file.
    NoNewline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PieceAt {
    first_row: usize,
    piece: Piece,
}

/// How a change's lines are laid out: what tells the two projections apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// Every removed line, then every added one, each side's last line followed by git's
    /// end-of-file marker when it did not end.
    Unified,
    /// The removed and added lines paired, the shorter side padded. Its end-of-file marker
    /// is phase 07's, which builds side-by-side.
    SideBySide,
}

/// Where each piece starts, so a row number becomes a piece and an offset by search.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RowIndex {
    pieces: Vec<PieceAt>,
    rows: usize,
    /// The first row of each change drawn, in order: what previous and next change move
    /// between, found by search.
    change_rows: Vec<Range<usize>>,
}

impl RowIndex {
    fn build(text: &TextDiff, changes: &[ChangedRange], hunks: &Hunks, shape: Shape) -> Self {
        let mut index = Self {
            pieces: Vec::new(),
            rows: 0,
            change_rows: Vec::new(),
        };
        let new_len = u32::try_from(text.new_lines().len()).unwrap_or(u32::MAX);

        for hunk_index in 0..hunks.len() {
            let Some(hunk) = hunks.get(hunk_index) else {
                // Unreachable: `hunk_index` is below the length just read.
                break;
            };
            index.push(
                Piece::Header {
                    hunk: u32::try_from(hunk_index).unwrap_or(u32::MAX),
                },
                1,
            );

            let mut old = hunk.old.start().index();
            let mut new = hunk.new.start().index();
            for change_index in hunk.changes.clone() {
                let Some(change) = changes.get(change_index as usize) else {
                    // Unreachable for a hunk of these ranges; a hunk of others stops here.
                    break;
                };
                let run = change
                    .removed
                    .start()
                    .index()
                    .saturating_sub(old)
                    .min(change.added.start().index().saturating_sub(new));
                index.push(Piece::Context { old, new, len: run }, run as usize);
                let rows = rows_of_change(text, *change, shape);
                if rows > 0 {
                    index.change_rows.push(index.rows..index.rows + rows);
                }
                index.push(
                    Piece::Change {
                        change: change_index,
                    },
                    rows,
                );
                old = change.removed.end().index();
                new = change.added.end().index();
            }

            let trailing = hunk
                .old
                .end()
                .index()
                .saturating_sub(old)
                .min(hunk.new.end().index().saturating_sub(new));
            index.push(
                Piece::Context {
                    old,
                    new,
                    len: trailing,
                },
                trailing as usize,
            );
            // A context run that ends the file ends both sides together, and git prints its
            // last line from the new side, so the new side says whether it ended.
            if shape == Shape::Unified
                && trailing > 0
                && new.saturating_add(trailing) == new_len
                && unterminated_last(text.new_lines())
            {
                index.push(Piece::NoNewline, 1);
            }
        }
        index
    }

    /// Records a stretch of rows, unless it is empty: an index entry for nothing to draw
    /// would make a row number ambiguous.
    fn push(&mut self, piece: Piece, count: usize) {
        if count == 0 {
            return;
        }
        self.pieces.push(PieceAt {
            first_row: self.rows,
            piece,
        });
        self.rows += count;
    }

    /// The piece a row falls in, and how far into it — found by search, so the cost is in
    /// the number of pieces and not in the row number.
    fn locate(&self, row: usize) -> Option<(Piece, usize)> {
        if row >= self.rows {
            return None;
        }
        let after = self.pieces.partition_point(|piece| piece.first_row <= row);
        let at = self.pieces.get(after.checked_sub(1)?)?;
        Some((at.piece, row - at.first_row))
    }

    /// How many pieces the index holds. This is the projection's whole footprint, and it
    /// counts changes rather than rows.
    fn piece_count(&self) -> usize {
        self.pieces.len()
    }
}

/// Whether a side's last line did not end in a newline, which git marks.
fn unterminated_last(lines: &[DiffLine]) -> bool {
    lines.last().is_some_and(|line| !line.ends_with_newline())
}

/// Whether `span` of a side runs to that side's last line and that line did not end.
fn ends_unterminated(lines: &[DiffLine], span: crate::LineSpan) -> bool {
    !span.is_empty() && span.end().index() as usize == lines.len() && unterminated_last(lines)
}

/// Where git's end-of-file markers fall inside a unified change: after its removed lines,
/// and after its added lines.
fn change_markers(text: &TextDiff, change: ChangedRange) -> (bool, bool) {
    (
        ends_unterminated(text.old_lines(), change.removed),
        ends_unterminated(text.new_lines(), change.added),
    )
}

fn rows_of_change(text: &TextDiff, change: ChangedRange, shape: Shape) -> usize {
    let (removed, added) = (change.removed.len() as usize, change.added.len() as usize);
    match shape {
        Shape::Unified => {
            let (after_removed, after_added) = change_markers(text, change);
            removed + usize::from(after_removed) + added + usize::from(after_added)
        }
        Shape::SideBySide => removed.max(added),
    }
}

/// One row of a unified diff (R6.4): a line of `git diff`'s output. `Copy`, so it owns no
/// line: it borrows the diff's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnifiedRow<'a> {
    Header(HunkHeader),
    /// A line both sides hold. `line` is the new side's, which is the line git prints: the
    /// same bytes as the old side's, except under `-w`, where they may differ in whitespace.
    Context {
        old: LineNumber,
        new: LineNumber,
        line: &'a DiffLine,
    },
    Removed {
        old: LineNumber,
        line: &'a DiffLine,
    },
    Added {
        new: LineNumber,
        line: &'a DiffLine,
    },
    /// git's `\ No newline at end of file`, after the row whose line did not end.
    NoNewlineAtEnd,
}

/// One row of a side-by-side diff (R1.4, R6.4). `Removed` leaves the right side filler and
/// `Added` leaves the left side filler, which is how the shorter side is padded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideBySideRow<'a> {
    Header(HunkHeader),
    Context {
        old: LineNumber,
        new: LineNumber,
        line: &'a DiffLine,
    },
    Replaced {
        old: LineNumber,
        old_line: &'a DiffLine,
        new: LineNumber,
        new_line: &'a DiffLine,
    },
    Removed {
        old: LineNumber,
        line: &'a DiffLine,
    },
    Added {
        new: LineNumber,
        line: &'a DiffLine,
    },
}

/// Which ranges a unified layout groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawnRanges {
    /// The exact changes: `git diff`.
    Exact,
    /// The whitespace-ignoring ones the overlay holds: `git diff -w`.
    IgnoringWhitespace,
}

/// The unified rows of one diff at one context, indexed and owning nothing of the diff — so
/// a view builds it once per answer and keeps it across frames, asking it for rows against
/// the diff it was built from. Built in time proportional to the diff's changes, never its
/// lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedLayout {
    hunks: Hunks,
    index: RowIndex,
    ranges: DrawnRanges,
}

impl UnifiedLayout {
    /// The exact changes at `context`, grouped as `git diff -U<n>` groups them with no
    /// inter-hunk context.
    pub fn exact(text: &TextDiff, context: Context) -> Self {
        Self::over(text, text.changes(), context, 0, DrawnRanges::Exact)
    }

    /// What a view shows of `text` at `context`: `git diff -w -U<n>`'s rows when the overlay
    /// holds the whitespace-ignoring ranges (asked for because whitespace is ignored), and
    /// `git diff -U<n>`'s otherwise — each grouped with the inter-hunk context git's
    /// function context was read with, so every hunk starts where git's does.
    pub fn shown(text: &TextDiff, overlay: &DisplayOverlay, context: Context) -> Self {
        let inter_hunk = overlay.function_context().inter_hunk_context();
        match overlay.changes_ignoring_whitespace() {
            Some(changes) => Self::over(
                text,
                changes,
                context,
                inter_hunk,
                DrawnRanges::IgnoringWhitespace,
            ),
            None => Self::over(
                text,
                text.changes(),
                context,
                inter_hunk,
                DrawnRanges::Exact,
            ),
        }
    }

    fn over(
        text: &TextDiff,
        changes: &[ChangedRange],
        context: Context,
        inter_hunk_context: u32,
        ranges: DrawnRanges,
    ) -> Self {
        let hunks = Hunks::of_ranges(text, changes, context, inter_hunk_context);
        let index = RowIndex::build(text, changes, &hunks, Shape::Unified);
        Self {
            hunks,
            index,
            ranges,
        }
    }

    pub fn len(&self) -> usize {
        self.index.rows
    }

    pub fn is_empty(&self) -> bool {
        self.index.rows == 0
    }

    pub fn hunks(&self) -> &Hunks {
        &self.hunks
    }

    /// Which ranges these rows draw.
    pub fn ranges(&self) -> DrawnRanges {
        self.ranges
    }

    /// What the index costs, in entries. Proportional to the diff's changes, never to its
    /// rows: a hundred thousand unchanged lines add one entry, not a hundred thousand.
    pub fn index_size(&self) -> usize {
        self.index.piece_count()
    }

    /// How many changes the rows draw: what previous and next change move between.
    pub fn change_count(&self) -> usize {
        self.index.change_rows.len()
    }

    /// The first row of the `change`-th change drawn.
    pub fn change_row(&self, change: usize) -> Option<usize> {
        self.index.change_rows.get(change).map(|rows| rows.start)
    }

    /// The rows the `change`-th change draws: its removed and added lines, and git's
    /// end-of-file markers among them.
    pub fn change_rows(&self, change: usize) -> Option<Range<usize>> {
        self.index.change_rows.get(change).cloned()
    }

    /// The first change that starts at `row` or below it. A search, never a scan.
    pub fn first_change_from(&self, row: usize) -> Option<usize> {
        let next = self
            .index
            .change_rows
            .partition_point(|rows| rows.start < row);
        (next < self.index.change_rows.len()).then_some(next)
    }

    /// The first change that starts below `row`. A search, never a scan.
    pub fn next_change_after(&self, row: usize) -> Option<usize> {
        self.first_change_from(row.saturating_add(1))
    }

    /// The last change that starts above `row`. A search, never a scan.
    pub fn previous_change_before(&self, row: usize) -> Option<usize> {
        self.index
            .change_rows
            .partition_point(|rows| rows.start < row)
            .checked_sub(1)
    }

    /// Row `row`, its line borrowed from `text` — the diff this layout was built from, with
    /// `overlay` its overlay. Another diff answers rows of no meaning, or none; it never
    /// panics.
    pub fn row<'a>(
        &self,
        text: &'a TextDiff,
        overlay: &'a DisplayOverlay,
        row: usize,
    ) -> Option<UnifiedRow<'a>> {
        let changes = match self.ranges {
            DrawnRanges::Exact => text.changes(),
            DrawnRanges::IgnoringWhitespace => overlay.changes_ignoring_whitespace()?,
        };
        self.row_over(text, changes, row)
    }

    fn row_over<'a>(
        &self,
        text: &'a TextDiff,
        changes: &[ChangedRange],
        row: usize,
    ) -> Option<UnifiedRow<'a>> {
        let (piece, offset) = self.index.locate(row)?;
        match piece {
            Piece::Header { hunk } => {
                Some(UnifiedRow::Header(self.hunks.get(hunk as usize)?.header()))
            }
            Piece::Context { old, new, .. } => {
                let step = u32::try_from(offset).unwrap_or(u32::MAX);
                let old = LineNumber::from_index(old.saturating_add(step));
                let new = LineNumber::from_index(new.saturating_add(step));
                Some(UnifiedRow::Context {
                    old,
                    new,
                    line: text.new_line(new)?,
                })
            }
            Piece::NoNewline => Some(UnifiedRow::NoNewlineAtEnd),
            Piece::Change { change } => {
                let change = *changes.get(change as usize)?;
                let (after_removed, after_added) = change_markers(text, change);
                let removed = change.removed.len() as usize;
                if offset < removed {
                    let step = u32::try_from(offset).unwrap_or(u32::MAX);
                    let old =
                        LineNumber::from_index(change.removed.start().index().saturating_add(step));
                    return Some(UnifiedRow::Removed {
                        old,
                        line: text.old_line(old)?,
                    });
                }
                let offset = offset - removed;
                if after_removed && offset == 0 {
                    return Some(UnifiedRow::NoNewlineAtEnd);
                }
                let offset = offset - usize::from(after_removed);
                if offset < change.added.len() as usize {
                    let step = u32::try_from(offset).unwrap_or(u32::MAX);
                    let new =
                        LineNumber::from_index(change.added.start().index().saturating_add(step));
                    return Some(UnifiedRow::Added {
                        new,
                        line: text.new_line(new)?,
                    });
                }
                after_added.then_some(UnifiedRow::NoNewlineAtEnd)
            }
        }
    }
}

/// The unified rows of one diff at one context, addressed by row number: a
/// [`UnifiedLayout`] beside the diff it borrows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedRows<'a> {
    text: &'a TextDiff,
    changes: &'a [ChangedRange],
    layout: UnifiedLayout,
}

impl<'a> UnifiedRows<'a> {
    /// The exact changes, as `git diff -U<n>` prints them with no inter-hunk context.
    pub fn new(text: &'a TextDiff, context: Context) -> Self {
        Self {
            text,
            changes: text.changes(),
            layout: UnifiedLayout::exact(text, context),
        }
    }

    /// What a view shows: [`UnifiedLayout::shown`].
    pub fn shown(text: &'a TextDiff, overlay: &'a DisplayOverlay, context: Context) -> Self {
        let layout = UnifiedLayout::shown(text, overlay, context);
        let changes = match layout.ranges() {
            DrawnRanges::Exact => text.changes(),
            DrawnRanges::IgnoringWhitespace => {
                overlay.changes_ignoring_whitespace().unwrap_or_default()
            }
        };
        Self {
            text,
            changes,
            layout,
        }
    }

    pub fn len(&self) -> usize {
        self.layout.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layout.is_empty()
    }

    pub fn hunks(&self) -> &Hunks {
        self.layout.hunks()
    }

    pub fn layout(&self) -> &UnifiedLayout {
        &self.layout
    }

    /// What the index costs, in entries: [`UnifiedLayout::index_size`].
    pub fn index_size(&self) -> usize {
        self.layout.index_size()
    }

    pub fn row(&self, row: usize) -> Option<UnifiedRow<'a>> {
        self.layout.row_over(self.text, self.changes, row)
    }
}

/// The side-by-side rows of one diff at one context, addressed by row number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideBySideRows<'a> {
    text: &'a TextDiff,
    hunks: Hunks,
    index: RowIndex,
}

impl<'a> SideBySideRows<'a> {
    pub fn new(text: &'a TextDiff, context: Context) -> Self {
        let hunks = Hunks::of(text, context);
        let index = RowIndex::build(text, text.changes(), &hunks, Shape::SideBySide);
        Self { text, hunks, index }
    }

    pub fn len(&self) -> usize {
        self.index.rows
    }

    pub fn is_empty(&self) -> bool {
        self.index.rows == 0
    }

    pub fn hunks(&self) -> &Hunks {
        &self.hunks
    }

    /// What the index costs, in entries; see [`UnifiedRows::index_size`].
    pub fn index_size(&self) -> usize {
        self.index.piece_count()
    }

    pub fn row(&self, row: usize) -> Option<SideBySideRow<'a>> {
        let (piece, offset) = self.index.locate(row)?;
        match piece {
            Piece::Header { hunk } => Some(SideBySideRow::Header(
                self.hunks.get(hunk as usize)?.header(),
            )),
            // Side-by-side builds no end-of-file marker yet (phase 07); none is indexed.
            Piece::NoNewline => None,
            Piece::Context { old, new, .. } => {
                let step = u32::try_from(offset).unwrap_or(u32::MAX);
                let old = LineNumber::from_index(old.saturating_add(step));
                let new = LineNumber::from_index(new.saturating_add(step));
                Some(SideBySideRow::Context {
                    old,
                    new,
                    line: self.text.old_line(old)?,
                })
            }
            Piece::Change { change } => {
                let change = *self.text.changes().get(change as usize)?;
                let removed = change.removed.len() as usize;
                let added = change.added.len() as usize;
                let step = u32::try_from(offset).unwrap_or(u32::MAX);
                let old =
                    LineNumber::from_index(change.removed.start().index().saturating_add(step));
                let new = LineNumber::from_index(change.added.start().index().saturating_add(step));
                if offset < removed.min(added) {
                    Some(SideBySideRow::Replaced {
                        old,
                        old_line: self.text.old_line(old)?,
                        new,
                        new_line: self.text.new_line(new)?,
                    })
                } else if offset < removed {
                    Some(SideBySideRow::Removed {
                        old,
                        line: self.text.old_line(old)?,
                    })
                } else {
                    Some(SideBySideRow::Added {
                        new,
                        line: self.text.new_line(new)?,
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LineSpan, split_lines};

    fn change(removed: (u32, u32), added: (u32, u32)) -> ChangedRange {
        ChangedRange::new(
            LineSpan::at(removed.0, removed.1),
            LineSpan::at(added.0, added.1),
        )
    }

    /// `a b c d e f g h` with `d` replaced by `D` and `E`.
    fn replaced() -> TextDiff {
        TextDiff::new(
            split_lines(b"a\nb\nc\nd\ne\nf\ng\nh\n"),
            split_lines(b"a\nb\nc\nD\nE\ne\nf\ng\nh\n"),
            vec![change((3, 1), (3, 2))],
        )
    }

    fn unified_picture(rows: &UnifiedRows<'_>) -> Vec<String> {
        (0..rows.len())
            .map(|n| match rows.row(n) {
                Some(UnifiedRow::Header(header)) => header.to_string(),
                Some(UnifiedRow::Context { old, new, line }) => {
                    format!("{} {} | {}", old.one_based(), new.one_based(), line.text())
                }
                Some(UnifiedRow::Removed { old, line }) => {
                    format!("{}   |-{}", old.one_based(), line.text())
                }
                Some(UnifiedRow::Added { new, line }) => {
                    format!("  {} |+{}", new.one_based(), line.text())
                }
                Some(UnifiedRow::NoNewlineAtEnd) => "\\ No newline at end of file".to_string(),
                None => "<missing>".to_string(),
            })
            .collect()
    }

    fn side_picture(rows: &SideBySideRows<'_>) -> Vec<String> {
        (0..rows.len())
            .map(|n| match rows.row(n) {
                Some(SideBySideRow::Header(header)) => header.to_string(),
                Some(SideBySideRow::Context { old, new, line }) => format!(
                    "{} {} | {} | {}",
                    old.one_based(),
                    new.one_based(),
                    line.text(),
                    line.text()
                ),
                Some(SideBySideRow::Replaced {
                    old,
                    old_line,
                    new,
                    new_line,
                }) => format!(
                    "{} {} | {} | {}",
                    old.one_based(),
                    new.one_based(),
                    old_line.text(),
                    new_line.text()
                ),
                Some(SideBySideRow::Removed { old, line }) => {
                    format!("{} - | {} |", old.one_based(), line.text())
                }
                Some(SideBySideRow::Added { new, line }) => {
                    format!("- {} | | {}", new.one_based(), line.text())
                }
                None => "<missing>".to_string(),
            })
            .collect()
    }

    /// A row that owned its line would allocate on every frame of a scroll. `Copy` is the
    /// proof: a type holding a `Vec` cannot be `Copy`, so this stops compiling the day a
    /// row starts carrying its own bytes.
    #[test]
    fn a_row_owns_nothing_it_would_have_to_allocate() {
        fn owns_nothing<T: Copy>(_row: T) {}
        let text = replaced();
        owns_nothing(
            UnifiedRows::new(&text, Context::lines(3))
                .row(0)
                .expect("a row"),
        );
        owns_nothing(
            SideBySideRows::new(&text, Context::lines(3))
                .row(0)
                .expect("a row"),
        );
    }

    /// Caught by: copying the line into the row, which the `Copy` bound alone would not
    /// see if the row ever held an inline buffer.
    #[test]
    fn a_row_points_at_the_diffs_own_line() {
        let text = replaced();
        let rows = UnifiedRows::new(&text, Context::lines(3));
        let Some(UnifiedRow::Removed { old, line }) = rows.row(4) else {
            panic!("row 4 of the unified picture is the removed line");
        };
        assert!(
            std::ptr::eq(line, text.old_line(old).expect("the old line")),
            "the row handed out a copy of the line rather than the diff's own"
        );
    }

    #[test]
    fn a_unified_hunk_reads_header_context_removed_then_added() {
        let text = replaced();
        let rows = UnifiedRows::new(&text, Context::lines(3));
        assert_eq!(
            unified_picture(&rows),
            vec![
                "@@ -1,7 +1,8 @@",
                "1 1 | a",
                "2 2 | b",
                "3 3 | c",
                "4   |-d",
                "  4 |+D",
                "  5 |+E",
                "5 6 | e",
                "6 7 | f",
                "7 8 | g",
            ]
        );
        assert_eq!(rows.len(), 10);
        assert!(rows.row(10).is_none(), "a row past the end answered");
        assert!(!rows.is_empty());
    }

    #[test]
    fn a_side_by_side_hunk_pairs_the_changed_lines_and_fills_the_shorter_side() {
        let text = replaced();
        let rows = SideBySideRows::new(&text, Context::lines(3));
        assert_eq!(
            side_picture(&rows),
            vec![
                "@@ -1,7 +1,8 @@",
                "1 1 | a | a",
                "2 2 | b | b",
                "3 3 | c | c",
                "4 4 | d | D",
                "- 5 | | E",
                "5 6 | e | e",
                "6 7 | f | f",
                "7 8 | g | g",
            ]
        );
        assert_eq!(
            rows.len(),
            9,
            "a side-by-side view built a row per changed line rather than per pair"
        );
    }

    /// The other direction: more removed than added leaves filler on the right.
    #[test]
    fn side_by_side_fills_the_right_when_more_lines_went_than_came() {
        let text = TextDiff::new(
            split_lines(b"a\nb\nc\nd\n"),
            split_lines(b"a\nD\n"),
            vec![change((1, 3), (1, 1))],
        );
        let rows = SideBySideRows::new(&text, Context::lines(3));
        assert_eq!(
            side_picture(&rows),
            vec![
                "@@ -1,4 +1,2 @@",
                "1 1 | a | a",
                "2 2 | b | D",
                "3 - | c |",
                "4 - | d |",
            ]
        );
    }

    /// Caught by: numbering rows from the hunk rather than from the file, which makes a
    /// selection made in one view name different lines in another.
    #[test]
    fn a_line_keeps_its_number_whatever_the_context_is() {
        let text = TextDiff::new(
            split_lines(b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n"),
            split_lines(b"a\nb\nc\nd\ne\nF\ng\nh\ni\nj\n"),
            vec![change((5, 1), (5, 1))],
        );
        for context in [
            Context::lines(1),
            Context::lines(3),
            Context::lines(9),
            Context::EntireFile,
        ] {
            let rows = UnifiedRows::new(&text, context);
            let removed: Vec<u32> = (0..rows.len())
                .filter_map(|n| match rows.row(n) {
                    Some(UnifiedRow::Removed { old, .. }) => Some(old.index()),
                    Some(UnifiedRow::Header(_))
                    | Some(UnifiedRow::Context { .. })
                    | Some(UnifiedRow::Added { .. })
                    | Some(UnifiedRow::NoNewlineAtEnd)
                    | None => None,
                })
                .collect();
            assert_eq!(
                removed,
                vec![5],
                "at {context:?} the removed line answered to another number"
            );
        }
    }

    /// The whole point of the index: one row of a very long file costs what one row of a
    /// short one costs, because the index counts changes and not lines.
    #[test]
    fn one_row_of_a_hundred_thousand_lines_is_reached_through_four_index_entries() {
        let mut old: Vec<DiffLine> = (0..100_000)
            .map(|n| DiffLine::terminated(format!("line {n}")))
            .collect();
        let mut new = old.clone();
        old[50_000] = DiffLine::terminated("before");
        new[50_000] = DiffLine::terminated("after");
        let text = TextDiff::new(old, new, vec![change((50_000, 1), (50_000, 1))]);

        let rows = UnifiedRows::new(&text, Context::EntireFile);
        assert_eq!(
            rows.len(),
            100_002,
            "header, every line, and the extra side"
        );
        assert_eq!(
            rows.index_size(),
            4,
            "the index grew with the file's lines rather than with its changes"
        );

        let Some(UnifiedRow::Context { old, new, line }) = rows.row(100_001) else {
            panic!("the last row of an entire-file view is a context line");
        };
        assert_eq!(old.index(), 99_999);
        assert_eq!(new.index(), 99_999);
        assert_eq!(line.text(), "line 99999");

        let Some(UnifiedRow::Removed { old, line }) = rows.row(50_001) else {
            panic!("the changed line sits after the header and fifty thousand context rows");
        };
        assert_eq!(old.index(), 50_000);
        assert_eq!(line.text(), "before");
    }

    /// Fifty thousand separate changes: the index is proportional to those, not to the
    /// quarter of a million rows they make.
    #[test]
    fn many_changes_index_by_change_and_not_by_row() {
        let old: Vec<DiffLine> = (0..100_000)
            .map(|n| DiffLine::terminated(format!("l{n}")))
            .collect();
        let new: Vec<DiffLine> = (0..100_000)
            .map(|n| {
                if n % 2 == 0 {
                    DiffLine::terminated(format!("L{n}"))
                } else {
                    DiffLine::terminated(format!("l{n}"))
                }
            })
            .collect();
        let count = 50_000u32;
        let changes: Vec<ChangedRange> =
            (0..count).map(|n| change((n * 2, 1), (n * 2, 1))).collect();
        let text = TextDiff::new(old, new, changes);

        let rows = UnifiedRows::new(&text, Context::lines(3));
        assert!(
            rows.len() > 2 * count as usize,
            "a change every other line makes more rows than there are changes"
        );
        // Both bounds matter. The second alone would pass an index holding one entry per
        // row, because a change every other line makes fewer rows than four per change.
        assert!(
            rows.index_size() < rows.len(),
            "the index held an entry per row: {} entries for {} rows",
            rows.index_size(),
            rows.len()
        );
        assert!(
            rows.index_size() <= 2 * count as usize + 2,
            "the index grew past two entries per change plus the hunk's own: {} entries for \
             {count} changes",
            rows.index_size()
        );

        let Some(UnifiedRow::Removed { old, line }) = rows.row(1) else {
            panic!("the row after the header is the first change's removed line");
        };
        assert_eq!(old.index(), 0);
        assert_eq!(line.text(), "l0");

        let Some(UnifiedRow::Context { old, .. }) = rows.row(rows.len() - 1) else {
            panic!("the last row is the trailing context line");
        };
        assert_eq!(
            old.index(),
            99_999,
            "the last row was reached at the wrong line"
        );
    }

    #[test]
    fn a_diff_with_no_changes_has_no_rows_at_a_line_context() {
        let text = TextDiff::new(split_lines(b"a\nb\n"), split_lines(b"a\nb\n"), Vec::new());
        let rows = UnifiedRows::new(&text, Context::lines(3));
        assert!(rows.is_empty());
        assert_eq!(rows.len(), 0);
        assert!(rows.row(0).is_none());
        assert!(SideBySideRows::new(&text, Context::lines(3)).is_empty());
    }

    /// Git parity: `git diff -U<as long as the file>` prints nothing for a file with no
    /// change, so the entire file of one is no row (phase 06; phase 01 drew it whole).
    #[test]
    fn entire_file_draws_nothing_for_a_file_nothing_changed_in() {
        let text = TextDiff::new(split_lines(b"a\nb\n"), split_lines(b"a\nb\n"), Vec::new());
        let rows = UnifiedRows::new(&text, Context::EntireFile);
        assert!(rows.is_empty(), "{:?}", unified_picture(&rows));
        assert_eq!(rows.hunks().len(), 0);
    }

    /// A file that gained its first content: no old line exists to draw as context.
    #[test]
    fn an_added_file_is_every_line_added() {
        let text = TextDiff::new(
            Vec::new(),
            split_lines(b"x\ny\n"),
            vec![change((0, 0), (0, 2))],
        );
        let rows = UnifiedRows::new(&text, Context::lines(3));
        assert_eq!(
            unified_picture(&rows),
            vec!["@@ -0,0 +1,2 @@", "  1 |+x", "  2 |+y"]
        );

        let side = SideBySideRows::new(&text, Context::lines(3));
        assert_eq!(
            side_picture(&side),
            vec!["@@ -0,0 +1,2 @@", "- 1 | | x", "- 2 | | y"]
        );
    }

    /// A deleted file: every line goes and nothing comes.
    #[test]
    fn a_deleted_file_is_every_line_removed() {
        let text = TextDiff::new(
            split_lines(b"x\ny\n"),
            Vec::new(),
            vec![change((0, 2), (0, 0))],
        );
        let rows = UnifiedRows::new(&text, Context::lines(3));
        assert_eq!(
            unified_picture(&rows),
            vec!["@@ -1,2 +0,0 @@", "1   |-x", "2   |-y"]
        );
    }

    /// Two hunks at a context of one, one hunk at three: the rows follow the grouping and
    /// the numbers do not move.
    #[test]
    fn separate_hunks_each_get_their_own_header() {
        let text = TextDiff::new(
            split_lines(b"a\nb\nc\nd\ne\nf\ng\nh\n"),
            split_lines(b"A\nb\nc\nd\ne\nf\ng\nH\n"),
            vec![change((0, 1), (0, 1)), change((7, 1), (7, 1))],
        );
        let rows = UnifiedRows::new(&text, Context::lines(1));
        assert_eq!(
            unified_picture(&rows),
            vec![
                "@@ -1,2 +1,2 @@",
                "1   |-a",
                "  1 |+A",
                "2 2 | b",
                "@@ -7,2 +7,2 @@",
                "7 7 | g",
                "8   |-h",
                "  8 |+H",
            ]
        );
        assert_eq!(rows.hunks().len(), 2);
    }

    fn overlay_ignoring(changes: Vec<ChangedRange>) -> DisplayOverlay {
        DisplayOverlay::new(Some(changes), Vec::new())
    }

    /// `git diff -w` prints a context line from the new side (`xdl_emit_diff` emits
    /// pre- and post-context from `xdf2`), and under `-w` the two sides of a context line
    /// may differ in whitespace. Measured: `printf 'a\n  b\nc\n' | ...` against
    /// `a\nb\nC\n` under `git diff -w` prints ` b` (the new side), not `   b`. Caught by:
    /// reading a context line from the old side.
    #[test]
    fn a_context_line_is_the_new_sides_as_git_prints_it() {
        let text = TextDiff::new(
            split_lines(b"a\n  b\nc\n"),
            split_lines(b"a\nb\nC\n"),
            vec![change((1, 2), (1, 2))],
        );
        let overlay = overlay_ignoring(vec![change((2, 1), (2, 1))]);
        let rows = UnifiedRows::shown(&text, &overlay, Context::lines(3));
        assert_eq!(
            unified_picture(&rows),
            vec![
                "@@ -1,3 +1,3 @@",
                "1 1 | a",
                "2 2 | b",
                "3   |-c",
                "  3 |+C"
            ]
        );
        assert_eq!(rows.layout().ranges(), DrawnRanges::IgnoringWhitespace);
    }

    /// Without whitespace-ignoring ranges the view draws the exact ones, grouped with the
    /// inter-hunk context the overlay's function context was read with. Caught by: drawing
    /// `-w` ranges that were never asked for, or grouping without the inter-hunk context.
    #[test]
    fn the_view_draws_the_exact_ranges_grouped_as_git_was_asked() {
        let text = TextDiff::new(
            (0..20)
                .map(|n| DiffLine::terminated(format!("l{n}")))
                .collect(),
            (0..20)
                .map(|n| {
                    DiffLine::terminated(if n == 2 || n == 7 {
                        format!("L{n}")
                    } else {
                        format!("l{n}")
                    })
                })
                .collect(),
            vec![change((2, 1), (2, 1)), change((7, 1), (7, 1))],
        );
        let apart = DisplayOverlay::none();
        assert_eq!(
            UnifiedRows::shown(&text, &apart, Context::lines(1))
                .hunks()
                .len(),
            2
        );
        let overlay = DisplayOverlay::none().with_function_context(
            crate::FunctionContext::read_at(Context::lines(1), Vec::new())
                .with_inter_hunk_context(2),
        );
        let rows = UnifiedRows::shown(&text, &overlay, Context::lines(1));
        assert_eq!(rows.hunks().len(), 1, "{:?}", unified_picture(&rows));
        assert_eq!(rows.layout().ranges(), DrawnRanges::Exact);
    }

    /// git prints `\ No newline at end of file` after a removed, an added or a context line
    /// that did not end — the row the patch shows. Caught by: dropping the marker, putting
    /// it after the wrong side, or reading a context line's end from the old side.
    #[test]
    fn a_line_that_did_not_end_is_followed_by_gits_marker() {
        let both_sides = TextDiff::new(
            split_lines(b"a\nb"),
            split_lines(b"a\nB"),
            vec![change((1, 1), (1, 1))],
        );
        assert_eq!(
            unified_picture(&UnifiedRows::new(&both_sides, Context::lines(3))),
            vec![
                "@@ -1,2 +1,2 @@",
                "1 1 | a",
                "2   |-b",
                "\\ No newline at end of file",
                "  2 |+B",
                "\\ No newline at end of file",
            ]
        );

        let gained_newline = TextDiff::new(
            split_lines(b"a\nb"),
            split_lines(b"a\nb\n"),
            vec![change((1, 1), (1, 1))],
        );
        assert_eq!(
            unified_picture(&UnifiedRows::new(&gained_newline, Context::lines(3))),
            vec![
                "@@ -1,2 +1,2 @@",
                "1 1 | a",
                "2   |-b",
                "\\ No newline at end of file",
                "  2 |+b",
            ]
        );

        let context_at_end = TextDiff::new(
            split_lines(b"A\nb"),
            split_lines(b"a\nb"),
            vec![change((0, 1), (0, 1))],
        );
        let rows = UnifiedRows::new(&context_at_end, Context::lines(3));
        assert_eq!(
            unified_picture(&rows),
            vec![
                "@@ -1,2 +1,2 @@",
                "1   |-A",
                "  1 |+a",
                "2 2 | b",
                "\\ No newline at end of file",
            ]
        );
        assert_eq!(rows.len(), 5, "the marker is a row the count holds");

        // Under -w the new side decides: the old side's last line did not end, the new
        // side's did, and git prints the new one, with no marker.
        let ignoring = TextDiff::new(
            split_lines(b"A\nb"),
            split_lines(b"a\nb\n"),
            vec![change((0, 2), (0, 2))],
        );
        let overlay = overlay_ignoring(vec![change((0, 1), (0, 1))]);
        assert_eq!(
            unified_picture(&UnifiedRows::shown(&ignoring, &overlay, Context::lines(3))),
            vec!["@@ -1,2 +1,2 @@", "1   |-A", "  1 |+a", "2 2 | b"]
        );
    }

    /// With whitespace ignored and nothing left to show, there is no row — git prints no
    /// hunk — at any context, the entire file included.
    #[test]
    fn whitespace_only_changes_ignored_leave_no_row() {
        let text = TextDiff::new(
            split_lines(b"a\n b\n"),
            split_lines(b"a\nb\n"),
            vec![change((1, 1), (1, 1))],
        );
        let overlay = overlay_ignoring(Vec::new());
        for context in [Context::lines(1), Context::lines(3), Context::EntireFile] {
            assert!(
                UnifiedRows::shown(&text, &overlay, context).is_empty(),
                "{context:?}"
            );
        }
    }

    /// Previous and next change move between the first rows of the changes drawn, found by
    /// search; a layout kept apart from its diff answers the rows the borrowing view does.
    /// Caught by: counting a header or a context run as a change, or an off-by-one at the
    /// row a change starts on.
    #[test]
    fn changes_are_found_by_the_row_they_start_on() {
        let text = TextDiff::new(
            split_lines(b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\n"),
            split_lines(b"A\nb\nc\nd\ne\nf\nG\nh\ni\nj\nk\nL\n"),
            vec![
                change((0, 1), (0, 1)),
                change((6, 1), (6, 1)),
                change((11, 1), (11, 1)),
            ],
        );
        let layout = UnifiedLayout::exact(&text, Context::lines(1));
        let starts: Vec<usize> = (0..layout.change_count())
            .filter_map(|k| layout.change_row(k))
            .collect();
        let rows = UnifiedRows::new(&text, Context::lines(1));
        for start in &starts {
            assert!(
                matches!(rows.row(*start), Some(UnifiedRow::Removed { .. })),
                "row {start} does not start a change: {:?}",
                unified_picture(&rows)
            );
        }
        assert_eq!(starts.len(), 3);
        assert_eq!(layout.first_change_from(starts[1]), Some(1));
        assert_eq!(layout.first_change_from(starts[1] + 1), Some(2));
        assert_eq!(layout.first_change_from(rows.len()), None);
        assert_eq!(
            layout.change_rows(0),
            Some(starts[0]..starts[0] + 2),
            "a one-line edit is a removed and an added row"
        );
        assert_eq!(layout.change_rows(3), None);
        assert_eq!(layout.next_change_after(0), Some(0));
        assert_eq!(layout.next_change_after(starts[0]), Some(1));
        assert_eq!(layout.next_change_after(starts[2]), None);
        assert_eq!(layout.previous_change_before(starts[1]), Some(0));
        assert_eq!(layout.previous_change_before(starts[0]), None);
        assert_eq!(layout.previous_change_before(rows.len()), Some(2));

        let none = DisplayOverlay::none();
        for n in 0..rows.len() {
            assert_eq!(layout.row(&text, &none, n), rows.row(n), "row {n}");
        }
    }
}
