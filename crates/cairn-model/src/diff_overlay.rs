//! What a view draws over a text diff, and a patch must never see.
//!
//! Two display-only things live here, both by decision L4: the second set of changed
//! ranges that comparing lines with their whitespace removed produces, and the byte
//! ranges inside a pair of lines that differ. Neither is part of [`crate::TextDiff`], so
//! neither is reachable from the patch emitter, which takes a `TextDiff` and nothing else
//! (R1.7).

use crate::{ChangedRange, LineNumber, LineSpan, TextDiff};

/// A half-open run of bytes inside one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}

impl ByteRange {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.end <= self.start
    }
}

/// Where a removed line and the added line it was paired with differ inside themselves
/// (R2.7). The engine pairs the i-th removed line of a change with its i-th added line,
/// which is the same pairing a side-by-side view draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntraLineHighlight {
    pub removed_line: LineNumber,
    pub added_line: LineNumber,
    pub on_removed: Vec<ByteRange>,
    pub on_added: Vec<ByteRange>,
}

/// The display-only companion of a [`TextDiff`].
///
/// Held beside the exact answer rather than inside it, so that what a view may read and
/// what a patch may read are two different types.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DisplayOverlay {
    ignoring_whitespace: Option<Vec<ChangedRange>>,
    highlights: Vec<IntraLineHighlight>,
    /// Indices into `highlights`, ordered by the added line, so a lookup from either side
    /// is a search rather than a scan however many pairs there are.
    by_added: Vec<u32>,
}

impl DisplayOverlay {
    /// No second set of ranges and no highlights: what a diff that was not asked for
    /// either carries.
    pub fn none() -> Self {
        Self::default()
    }

    pub fn new(
        ignoring_whitespace: Option<Vec<ChangedRange>>,
        mut highlights: Vec<IntraLineHighlight>,
    ) -> Self {
        highlights.sort_by_key(|pair| pair.removed_line);
        let mut by_added: Vec<u32> = (0..highlights.len())
            .map(|index| u32::try_from(index).unwrap_or(u32::MAX))
            .collect();
        by_added.sort_by_key(|index| {
            highlights
                .get(*index as usize)
                .map(|pair| pair.added_line)
                .unwrap_or(LineNumber::from_index(u32::MAX))
        });
        Self {
            ignoring_whitespace,
            highlights,
            by_added,
        }
    }

    /// The ranges a whitespace-ignoring view draws instead of the exact ones, when the
    /// caller asked for them.
    pub fn changes_ignoring_whitespace(&self) -> Option<&[ChangedRange]> {
        self.ignoring_whitespace.as_deref()
    }

    /// The ranges to draw: the whitespace-ignoring ones when they were computed, and the
    /// exact ones otherwise.
    pub fn changes_to_draw<'a>(&'a self, exact: &'a TextDiff) -> &'a [ChangedRange] {
        self.changes_ignoring_whitespace()
            .unwrap_or_else(|| exact.changes())
    }

    /// Whether ignoring whitespace hides a change that is really there — which is what a
    /// view has to say out loud (R6.7). False when no whitespace-ignoring ranges exist.
    pub fn hides_a_change(&self, exact: &TextDiff) -> bool {
        let Some(shown) = self.changes_ignoring_whitespace() else {
            return false;
        };
        !covers_every_span(
            exact.changes().iter().map(|change| change.removed),
            shown.iter().map(|change| change.removed),
        ) || !covers_every_span(
            exact.changes().iter().map(|change| change.added),
            shown.iter().map(|change| change.added),
        )
    }

    pub fn highlights(&self) -> &[IntraLineHighlight] {
        &self.highlights
    }

    /// What to tint inside a removed line; empty when the line was not paired.
    pub fn on_removed_line(&self, line: LineNumber) -> &[ByteRange] {
        let found = self
            .highlights
            .binary_search_by_key(&line, |pair| pair.removed_line);
        match found {
            Ok(index) => self
                .highlights
                .get(index)
                .map(|pair| pair.on_removed.as_slice())
                .unwrap_or_default(),
            Err(_) => &[],
        }
    }

    /// What to tint inside an added line; empty when the line was not paired.
    pub fn on_added_line(&self, line: LineNumber) -> &[ByteRange] {
        let found = self.by_added.binary_search_by_key(&line, |index| {
            self.highlights
                .get(*index as usize)
                .map(|pair| pair.added_line)
                .unwrap_or(LineNumber::from_index(u32::MAX))
        });
        let Ok(slot) = found else {
            return &[];
        };
        self.by_added
            .get(slot)
            .and_then(|index| self.highlights.get(*index as usize))
            .map(|pair| pair.on_added.as_slice())
            .unwrap_or_default()
    }
}

/// Whether every line of every span in `wanted` falls inside some span of `covering`.
/// Both sequences are expected in increasing order and not overlapping, which is the order
/// changed ranges come in.
fn covers_every_span(
    wanted: impl Iterator<Item = LineSpan>,
    covering: impl Iterator<Item = LineSpan>,
) -> bool {
    let covering: Vec<LineSpan> = covering.filter(|span| !span.is_empty()).collect();
    let mut next = 0usize;
    for span in wanted.filter(|span| !span.is_empty()) {
        let mut reached = span.start().index();
        let end = span.end().index();
        while reached < end {
            while covering
                .get(next)
                .is_some_and(|candidate| candidate.end().index() <= reached)
            {
                next += 1;
            }
            let Some(candidate) = covering.get(next) else {
                return false;
            };
            if candidate.start().index() > reached {
                return false;
            }
            reached = candidate.end().index();
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiffLine, TextDiff};

    fn change(removed: (u32, u32), added: (u32, u32)) -> ChangedRange {
        ChangedRange::new(
            LineSpan::at(removed.0, removed.1),
            LineSpan::at(added.0, added.1),
        )
    }

    fn text_with(changes: Vec<ChangedRange>) -> TextDiff {
        let lines: Vec<DiffLine> = (0..32)
            .map(|n| DiffLine::terminated(format!("l{n}")))
            .collect();
        TextDiff::new(lines.clone(), lines, changes)
    }

    #[test]
    fn a_byte_range_measures_what_it_covers() {
        assert_eq!(ByteRange::new(2, 7).len(), 5);
        assert!(!ByteRange::new(2, 7).is_empty());
        assert!(ByteRange::new(4, 4).is_empty());
        assert_eq!(ByteRange::new(7, 2).len(), 0, "a backwards range measured");
    }

    /// Caught by: an overlay with nothing in it claiming a view is hiding something.
    #[test]
    fn an_overlay_with_no_second_set_of_ranges_hides_nothing() {
        let exact = text_with(vec![change((1, 1), (1, 1))]);
        let overlay = DisplayOverlay::none();
        assert_eq!(overlay.changes_ignoring_whitespace(), None);
        assert!(!overlay.hides_a_change(&exact));
        assert_eq!(
            overlay.changes_to_draw(&exact),
            exact.changes(),
            "with nothing to draw instead, the exact ranges were not drawn"
        );
    }

    /// Caught by: comparing the two sets by length, which agrees whenever one range is
    /// swapped for another of the same count.
    #[test]
    fn a_change_that_survives_ignoring_whitespace_is_not_reported_as_hidden() {
        let exact = text_with(vec![change((1, 1), (1, 1)), change((5, 2), (5, 2))]);
        let same = DisplayOverlay::new(
            Some(vec![change((1, 1), (1, 1)), change((5, 2), (5, 2))]),
            Vec::new(),
        );
        assert!(
            !same.hides_a_change(&exact),
            "an unchanged set read as hiding"
        );

        let dropped = DisplayOverlay::new(Some(vec![change((1, 1), (1, 1))]), Vec::new());
        assert!(
            dropped.hides_a_change(&exact),
            "a change that ignoring whitespace swallowed went unannounced"
        );
    }

    /// The whitespace-ignoring set may group differently; only the lines decide.
    #[test]
    fn a_regrouped_set_that_still_covers_every_line_hides_nothing() {
        let exact = text_with(vec![change((2, 1), (2, 1)), change((3, 1), (3, 1))]);
        let merged = DisplayOverlay::new(Some(vec![change((2, 2), (2, 2))]), Vec::new());
        assert!(
            !merged.hides_a_change(&exact),
            "two changes covered by one wider range read as hidden"
        );

        let short = DisplayOverlay::new(Some(vec![change((2, 1), (2, 2))]), Vec::new());
        assert!(
            short.hides_a_change(&exact),
            "a range covering the added lines but not the removed one read as covering both"
        );
    }

    /// Caught by: indexing highlights by position, so a pair arriving out of order answers
    /// for the wrong line.
    #[test]
    fn a_highlight_is_found_from_either_side_whatever_order_it_arrived_in() {
        let overlay = DisplayOverlay::new(
            None,
            vec![
                IntraLineHighlight {
                    removed_line: LineNumber::from_index(9),
                    added_line: LineNumber::from_index(11),
                    on_removed: vec![ByteRange::new(3, 5)],
                    on_added: vec![ByteRange::new(3, 6)],
                },
                IntraLineHighlight {
                    removed_line: LineNumber::from_index(2),
                    added_line: LineNumber::from_index(2),
                    on_removed: vec![ByteRange::new(0, 1)],
                    on_added: vec![ByteRange::new(0, 2)],
                },
            ],
        );

        assert_eq!(
            overlay.on_removed_line(LineNumber::from_index(9)),
            &[ByteRange::new(3, 5)]
        );
        assert_eq!(
            overlay.on_added_line(LineNumber::from_index(11)),
            &[ByteRange::new(3, 6)]
        );
        assert_eq!(
            overlay.on_removed_line(LineNumber::from_index(2)),
            &[ByteRange::new(0, 1)]
        );
        assert_eq!(
            overlay.on_added_line(LineNumber::from_index(2)),
            &[ByteRange::new(0, 2)]
        );
        assert!(
            overlay
                .on_removed_line(LineNumber::from_index(11))
                .is_empty(),
            "the added line's number found a highlight on the removed side"
        );
        assert!(
            overlay.on_added_line(LineNumber::from_index(9)).is_empty(),
            "the removed line's number found a highlight on the added side"
        );
        assert_eq!(overlay.highlights().len(), 2);
    }

    #[test]
    fn a_line_nobody_paired_is_tinted_nowhere() {
        let overlay = DisplayOverlay::none();
        assert!(
            overlay
                .on_removed_line(LineNumber::from_index(0))
                .is_empty()
        );
        assert!(overlay.on_added_line(LineNumber::from_index(0)).is_empty());
    }
}
