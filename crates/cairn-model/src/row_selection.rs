//! What the diff's staging gesture selects of the rows a view draws (staging-and-commit R9.1-
//! R9.3): a hovered chunk's changed lines, and the changed lines a drag across rows covers.
//!
//! **A chunk is a drawn hunk.** At any context a hunk groups every exact change whose context
//! touches the next one's, so at context ten one outlined chunk can hold several of git's
//! changes; its actions take every changed line drawn inside the outline and nothing outside
//! it — exactly what the person sees selected ([`UnifiedLayout::hunk_selection`]). Only rows of
//! the exact changes are selected from: rows drawn ignoring whitespace group ranges that are
//! not git's lines, so they select nothing (L6: Local Changes always draws the exact diff).
//!
//! **A drag selects whole lines** (Fork: a partial selection acts on whole lines). The rows it
//! spans are read as the view reads them — row by row through the layout — so a selection is
//! the lines drawn in those rows and no others; a context row, a hunk's header and git's
//! end-of-file marker select nothing. Only the changes the rows overlap are read, so the cost
//! is the changed lines selected, never the rows spanned. In side by side a drag stays in the
//! column it began in ([`SideColumn`], R9.3): the old column selects removed lines, the new
//! one added lines.

use std::ops::Range;

use crate::{
    ChangeStops, DrawnRanges, Selection, SideBySideLayout, SideBySideRow, TextDiff, UnifiedLayout,
    UnifiedRow,
};

/// The column of a side-by-side view a drag began in, and stays in (R9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SideColumn {
    /// The left column: the old side, its removed lines.
    Old,
    /// The right column: the new side, its added lines.
    New,
}

/// The changes whose rows overlap `rows`, by their index among the drawn changes: found by
/// search, so the rows spanned between them are never walked.
fn overlapping(stops: &ChangeStops, rows: &Range<usize>) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    if rows.is_empty() {
        return found;
    }
    // The last change starting at or above the first row may reach into it.
    let mut change = stops
        .last_before(rows.start.saturating_add(1))
        .or_else(|| stops.first_from(rows.start));
    while let Some(at) = change {
        let Some(drawn) = stops.rows(at) else {
            break;
        };
        if drawn.start >= rows.end {
            break;
        }
        let overlap = drawn.start.max(rows.start)..drawn.end.min(rows.end);
        if !overlap.is_empty() {
            found.push(overlap);
        }
        change = Some(at + 1).filter(|next| *next < stops.len());
    }
    found
}

impl UnifiedLayout {
    /// Every changed line hunk `hunk` draws: every change it groups, both sides — what its
    /// floating actions take. `None` for a hunk that is not there, or for rows drawn ignoring
    /// whitespace, whose ranges are not git's lines.
    pub fn hunk_selection(&self, text: &TextDiff, hunk: usize) -> Option<Selection> {
        if self.ranges() != DrawnRanges::Exact {
            return None;
        }
        let hunk = self.hunks().get(hunk)?;
        let mut selection = Selection::empty();
        for change in hunk.changes.clone() {
            selection.select_change(text.changes().get(change as usize)?);
        }
        Some(selection)
    }

    /// The changed lines drawn in `rows` — a removed row's old line, an added row's new line,
    /// nothing for a context row, a header or git's end-of-file marker. `None` for rows drawn
    /// ignoring whitespace. Costs the changed rows in `rows`, never the rows themselves.
    pub fn selection_in(&self, text: &TextDiff, rows: Range<usize>) -> Option<Selection> {
        if self.ranges() != DrawnRanges::Exact {
            return None;
        }
        let rows = rows.start..rows.end.min(self.len());
        let mut selection = Selection::empty();
        for overlap in overlapping(self.stops(), &rows) {
            for row in overlap {
                match self.row_over(text, text.changes(), row) {
                    Some(UnifiedRow::Removed { old, .. }) => selection.select_removed(old),
                    Some(UnifiedRow::Added { new, .. }) => selection.select_added(new),
                    Some(
                        UnifiedRow::Header(_)
                        | UnifiedRow::Context { .. }
                        | UnifiedRow::NoNewlineAtEnd,
                    )
                    | None => {}
                }
            }
        }
        Some(selection)
    }
}

impl SideBySideLayout {
    /// Every changed line hunk `hunk` draws, both columns: see
    /// [`UnifiedLayout::hunk_selection`].
    pub fn hunk_selection(&self, text: &TextDiff, hunk: usize) -> Option<Selection> {
        if self.ranges() != DrawnRanges::Exact {
            return None;
        }
        let hunk = self.hunks().get(hunk)?;
        let mut selection = Selection::empty();
        for change in hunk.changes.clone() {
            selection.select_change(text.changes().get(change as usize)?);
        }
        Some(selection)
    }

    /// The changed lines drawn in `rows` of `column` alone (R9.3): the old column's removed
    /// lines, or the new column's added ones — a replaced row's other half is left out. `None`
    /// for rows drawn ignoring whitespace.
    pub fn selection_in(
        &self,
        text: &TextDiff,
        rows: Range<usize>,
        column: SideColumn,
    ) -> Option<Selection> {
        if self.ranges() != DrawnRanges::Exact {
            return None;
        }
        let rows = rows.start..rows.end.min(self.len());
        let mut selection = Selection::empty();
        for overlap in overlapping(self.stops(), &rows) {
            for row in overlap {
                let (old, new) = match self.row_over(text, text.changes(), row) {
                    Some(SideBySideRow::Replaced { old, new, .. }) => (Some(old), Some(new)),
                    Some(SideBySideRow::Removed { old, .. }) => (Some(old), None),
                    Some(SideBySideRow::Added { new, .. }) => (None, Some(new)),
                    Some(
                        SideBySideRow::Header(_)
                        | SideBySideRow::Context { .. }
                        | SideBySideRow::NoNewlineAtEnd { .. },
                    )
                    | None => (None, None),
                };
                match (column, old, new) {
                    (SideColumn::Old, Some(old), _) => selection.select_removed(old),
                    (SideColumn::New, _, Some(new)) => selection.select_added(new),
                    (SideColumn::Old, None, _) | (SideColumn::New, _, None) => {}
                }
            }
        }
        Some(selection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangedRange, Context, DisplayOverlay, LineNumber, LineSpan, split_lines};

    /// Sixty lines, `l0` to `l59`, with three edits: line 10 replaced, lines 14-15 removed and
    /// a line added after line 40.
    fn three_edits() -> TextDiff {
        let old: Vec<u8> = (0..60)
            .flat_map(|n| format!("l{n}\n").into_bytes())
            .collect();
        let new: Vec<u8> = (0..60)
            .flat_map(|n| match n {
                10 => b"L10\n".to_vec(),
                14 | 15 => Vec::new(),
                40 => b"l40\nnew\n".to_vec(),
                _ => format!("l{n}\n").into_bytes(),
            })
            .collect();
        TextDiff::new(
            split_lines(&old),
            split_lines(&new),
            vec![
                ChangedRange::new(LineSpan::at(10, 1), LineSpan::at(10, 1)),
                ChangedRange::new(LineSpan::at(14, 2), LineSpan::at(14, 0)),
                ChangedRange::new(LineSpan::at(41, 0), LineSpan::at(39, 1)),
            ],
        )
    }

    fn lines(selection: &Selection) -> (Vec<u32>, Vec<u32>) {
        (
            selection.removed().map(LineNumber::index).collect(),
            selection.added().map(LineNumber::index).collect(),
        )
    }

    /// The QA brief's first risk: at context ten one drawn chunk groups the first two edits,
    /// and its actions take exactly their lines — every changed line inside the outline, none
    /// outside it — and the same chunk read as rows says the same. At context one the two
    /// are chunks of their own. Every row of a hunk names that hunk, and no row names another.
    /// Caught by: a chunk mapped to its first change alone, or to every change of the file.
    #[test]
    fn a_chunk_at_context_ten_takes_every_change_it_draws_and_no_other() {
        let text = three_edits();
        let wide = UnifiedLayout::exact(&text, Context::lines(10));
        assert_eq!(
            wide.hunks().len(),
            2,
            "context ten merges the first two edits"
        );
        let first = wide.hunk_selection(&text, 0).expect("a chunk");
        assert_eq!(lines(&first), (vec![10, 14, 15], vec![10]));
        let rows = wide.hunk_rows(0).expect("its rows");
        assert_eq!(wide.selection_in(&text, rows.clone()), Some(first));
        for row in rows.clone() {
            assert_eq!(wide.hunk_at(row), Some(0), "row {row}");
        }
        assert_eq!(wide.hunk_at(rows.end), Some(1));
        let last = wide.hunk_selection(&text, 1).expect("the last chunk");
        assert_eq!(lines(&last), (vec![], vec![39]));
        assert_eq!(wide.hunk_at(wide.len()), None);
        assert_eq!(wide.hunk_selection(&text, 2), None);

        let narrow = UnifiedLayout::exact(&text, Context::lines(1));
        assert_eq!(narrow.hunks().len(), 3);
        assert_eq!(
            lines(&narrow.hunk_selection(&text, 0).expect("a chunk")),
            (vec![10], vec![10])
        );
        assert_eq!(
            lines(&narrow.hunk_selection(&text, 1).expect("a chunk")),
            (vec![14, 15], vec![])
        );

        // Side by side groups alike.
        let side = SideBySideLayout::exact(&text, Context::lines(10));
        assert_eq!(side.hunks().len(), 2);
        assert_eq!(
            side.hunk_selection(&text, 0).map(|s| lines(&s)),
            Some((vec![10, 14, 15], vec![10]))
        );
        let rows = side.hunk_rows(0).expect("rows");
        assert!(rows.clone().all(|row| side.hunk_at(row) == Some(0)));
    }

    /// A drag selects the changed lines in the rows it spans, and only those: one removed row
    /// alone, a replaced line's two rows, a span starting and ending on context. Caught by: a
    /// selection taken from the changes the rows touch (whole changes), or context rows read as
    /// lines.
    #[test]
    fn a_drag_selects_the_changed_lines_in_its_rows_alone() {
        let text = three_edits();
        let layout = UnifiedLayout::exact(&text, Context::lines(10));
        let (first_change, second_change) = (
            layout.change_rows(0).expect("first"),
            layout.change_rows(1).expect("second"),
        );
        // The first change's removed row alone.
        let removed_row = first_change.start..first_change.start + 1;
        assert_eq!(
            layout.selection_in(&text, removed_row).map(|s| lines(&s)),
            Some((vec![10], vec![]))
        );
        // The second change's second removed line alone.
        let second_line = second_change.start + 1..second_change.start + 2;
        assert_eq!(
            layout.selection_in(&text, second_line).map(|s| lines(&s)),
            Some((vec![15], vec![]))
        );
        // From the context above the first change to the context between the two: the first
        // change, both sides.
        let span = first_change.start - 2..first_change.end + 1;
        assert_eq!(
            layout.selection_in(&text, span).map(|s| lines(&s)),
            Some((vec![10], vec![10]))
        );
        // Context and the header alone select nothing.
        let header = layout.hunk_rows(0).expect("rows").start;
        assert_eq!(
            layout.selection_in(&text, header..first_change.start),
            Some(Selection::empty())
        );
        // Past the end is cut at the end.
        assert_eq!(
            layout.selection_in(&text, 0..usize::MAX).map(|s| s.len()),
            Some(5)
        );
    }

    /// R9.3: side by side, a drag stays in its column — the old column selects the removed
    /// lines of the rows it spans, the new column the added ones, a replaced row giving each
    /// column its own half. Caught by: a column ignored (both halves selected).
    #[test]
    fn side_by_side_a_drag_selects_its_own_column() {
        let text = three_edits();
        let layout = SideBySideLayout::exact(&text, Context::lines(10));
        let every = 0..layout.len();
        assert_eq!(
            layout
                .selection_in(&text, every.clone(), SideColumn::Old)
                .map(|s| lines(&s)),
            Some((vec![10, 14, 15], vec![]))
        );
        assert_eq!(
            layout
                .selection_in(&text, every, SideColumn::New)
                .map(|s| lines(&s)),
            Some((vec![], vec![10, 39]))
        );
        let replaced = layout.stops().rows(0).expect("the replaced row");
        assert_eq!(
            layout
                .selection_in(&text, replaced.clone(), SideColumn::New)
                .map(|s| lines(&s)),
            Some((vec![], vec![10]))
        );
        assert_eq!(
            layout
                .selection_in(&text, replaced, SideColumn::Old)
                .map(|s| lines(&s)),
            Some((vec![10], vec![]))
        );
    }

    /// L6: rows drawn ignoring whitespace select nothing, chunk or drag. Caught by: a
    /// whitespace-ignoring range read as git's lines (staging lines the person never saw).
    #[test]
    fn rows_drawn_ignoring_whitespace_select_nothing() {
        let text = three_edits();
        let overlay = DisplayOverlay::new(
            Some(vec![ChangedRange::new(
                LineSpan::at(10, 1),
                LineSpan::at(10, 1),
            )]),
            Vec::new(),
        );
        let layout = UnifiedLayout::shown(&text, &overlay, Context::lines(3));
        assert_eq!(layout.ranges(), DrawnRanges::IgnoringWhitespace);
        assert_eq!(layout.hunk_selection(&text, 0), None);
        assert_eq!(layout.selection_in(&text, 0..layout.len()), None);
        let side = SideBySideLayout::shown(&text, &overlay, Context::lines(3));
        assert_eq!(side.hunk_selection(&text, 0), None);
        assert_eq!(
            side.selection_in(&text, 0..side.len(), SideColumn::New),
            None
        );
    }

    /// A drag over a hundred thousand rows of one long diff reads only the changes it
    /// overlaps: the selection is the changed lines, found from the change stops. Caught by: an
    /// off-by-one at either end of the span (a change at its edge lost or gained).
    #[test]
    fn a_long_span_selects_exactly_the_changes_between_its_ends() {
        let count = 100_000u32;
        let old: Vec<u8> = (0..count)
            .flat_map(|n| format!("{n}\n").into_bytes())
            .collect();
        let new: Vec<u8> = (0..count)
            .flat_map(|n| {
                if n % 1000 == 500 {
                    format!("x{n}\n").into_bytes()
                } else {
                    format!("{n}\n").into_bytes()
                }
            })
            .collect();
        let changes = (0..count / 1000)
            .map(|k| {
                ChangedRange::new(
                    LineSpan::at(k * 1000 + 500, 1),
                    LineSpan::at(k * 1000 + 500, 1),
                )
            })
            .collect();
        let text = TextDiff::new(split_lines(&old), split_lines(&new), changes);
        let layout = UnifiedLayout::exact(&text, Context::EntireFile);
        // From the added row of change 3 to the removed row of change 90, inclusive.
        let from = layout.change_rows(3).expect("change 3").start + 1;
        let to = layout.change_rows(90).expect("change 90").start + 1;
        let selection = layout.selection_in(&text, from..to).expect("exact");
        let (removed, added) = lines(&selection);
        assert_eq!(removed.first(), Some(&4500));
        assert_eq!(removed.last(), Some(&90500));
        assert_eq!(added.first(), Some(&3500));
        assert_eq!(added.last(), Some(&89500));
        assert_eq!(removed.len() + added.len(), 2 * 87);
    }
}
