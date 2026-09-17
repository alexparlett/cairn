//! Rows a view draws, reached one at a time.
//!
//! Both projections of R1.4 live here, over one addressing scheme. Neither builds a row
//! until it is asked for one (R1.5): what is built up front is an index of the diff's
//! *changes*, which is as long as the file has separate changes and never as long as the
//! file has lines. A row borrows its line from the diff — the row types are `Copy`, so
//! they cannot own one — which is what makes asking for a row cost nothing to allocate.

use crate::{ChangedRange, Context, DiffLine, HunkHeader, Hunks, LineNumber, TextDiff};

/// A stretch of rows of one kind. The index holds one per context run and one per change,
/// plus one per hunk header — never one per row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Piece {
    Header { hunk: u32 },
    Context { old: u32, new: u32, len: u32 },
    Change { change: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PieceAt {
    first_row: usize,
    piece: Piece,
}

/// Where each piece starts, so a row number becomes a piece and an offset by search.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RowIndex {
    pieces: Vec<PieceAt>,
    rows: usize,
}

impl RowIndex {
    /// `rows_of_change` is what tells the two projections apart: a unified view draws
    /// every removed line and then every added one, a side-by-side view draws them paired.
    fn build(text: &TextDiff, hunks: &Hunks, rows_of_change: fn(ChangedRange) -> usize) -> Self {
        let mut pieces: Vec<PieceAt> = Vec::new();
        let mut rows = 0usize;

        for index in 0..hunks.len() {
            let Some(hunk) = hunks.get(index) else {
                // Unreachable: `index` is below the length just read.
                break;
            };
            push(
                &mut pieces,
                &mut rows,
                Piece::Header {
                    hunk: u32::try_from(index).unwrap_or(u32::MAX),
                },
                1,
            );

            let mut old = hunk.old.start().index();
            let mut new = hunk.new.start().index();
            for change_index in hunk.changes.clone() {
                let Some(change) = text.changes().get(change_index as usize) else {
                    // Unreachable for a hunk of this diff; a hunk from another one stops here.
                    break;
                };
                let run = change
                    .removed
                    .start()
                    .index()
                    .saturating_sub(old)
                    .min(change.added.start().index().saturating_sub(new));
                push(
                    &mut pieces,
                    &mut rows,
                    Piece::Context { old, new, len: run },
                    run as usize,
                );
                push(
                    &mut pieces,
                    &mut rows,
                    Piece::Change {
                        change: change_index,
                    },
                    rows_of_change(*change),
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
            push(
                &mut pieces,
                &mut rows,
                Piece::Context {
                    old,
                    new,
                    len: trailing,
                },
                trailing as usize,
            );
        }

        Self { pieces, rows }
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

/// Records a stretch of rows, unless it is empty: an index entry for nothing to draw would
/// make a row number ambiguous.
fn push(pieces: &mut Vec<PieceAt>, rows: &mut usize, piece: Piece, count: usize) {
    if count == 0 {
        return;
    }
    pieces.push(PieceAt {
        first_row: *rows,
        piece,
    });
    *rows += count;
}

fn unified_rows_of(change: ChangedRange) -> usize {
    change.removed.len() as usize + change.added.len() as usize
}

fn side_by_side_rows_of(change: ChangedRange) -> usize {
    change.removed.len().max(change.added.len()) as usize
}

/// One row of a unified diff (R6.4). `Copy`, so it owns no line: it borrows the diff's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnifiedRow<'a> {
    Header(HunkHeader),
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

/// The unified rows of one diff at one context, addressed by row number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedRows<'a> {
    text: &'a TextDiff,
    hunks: Hunks,
    index: RowIndex,
}

impl<'a> UnifiedRows<'a> {
    pub fn new(text: &'a TextDiff, context: Context) -> Self {
        let hunks = Hunks::of(text, context);
        let index = RowIndex::build(text, &hunks, unified_rows_of);
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

    /// What the index costs, in entries. Proportional to the diff's changes, never to its
    /// rows: a hundred thousand unchanged lines add one entry, not a hundred thousand.
    pub fn index_size(&self) -> usize {
        self.index.piece_count()
    }

    pub fn row(&self, row: usize) -> Option<UnifiedRow<'a>> {
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
                    line: self.text.old_line(old)?,
                })
            }
            Piece::Change { change } => {
                let change = *self.text.changes().get(change as usize)?;
                let removed = change.removed.len() as usize;
                if offset < removed {
                    let step = u32::try_from(offset).unwrap_or(u32::MAX);
                    let old =
                        LineNumber::from_index(change.removed.start().index().saturating_add(step));
                    Some(UnifiedRow::Removed {
                        old,
                        line: self.text.old_line(old)?,
                    })
                } else {
                    let step = u32::try_from(offset - removed).unwrap_or(u32::MAX);
                    let new =
                        LineNumber::from_index(change.added.start().index().saturating_add(step));
                    Some(UnifiedRow::Added {
                        new,
                        line: self.text.new_line(new)?,
                    })
                }
            }
        }
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
        let index = RowIndex::build(text, &hunks, side_by_side_rows_of);
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

    /// Entire-file mode shows an unchanged file whole; the hunks it reads come with it.
    #[test]
    fn entire_file_draws_a_file_nothing_changed_in() {
        let text = TextDiff::new(split_lines(b"a\nb\n"), split_lines(b"a\nb\n"), Vec::new());
        let rows = UnifiedRows::new(&text, Context::EntireFile);
        assert_eq!(
            unified_picture(&rows),
            vec!["@@ -1,2 +1,2 @@", "1 1 | a", "2 2 | b"]
        );
        assert_eq!(rows.hunks().len(), 1);
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
}
