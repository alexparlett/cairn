//! Which changed lines a patch is to carry.

use std::collections::BTreeSet;

use crate::{ChangedRange, LineNumber, TextDiff};

/// A set of changed lines, each named by its line number on its own side (R1.3).
///
/// A removed line is named by its old number and an added line by its new one, so a
/// selection says nothing about hunks, context or which view was on screen when it was
/// made. That is why it survives every projection of the same diff unchanged (C4), and
/// why the patch built from it does too.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    removed: BTreeSet<LineNumber>,
    added: BTreeSet<LineNumber>,
}

impl Selection {
    /// Nothing selected: the patch this makes is empty.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Every changed line of a diff — what staging a whole file selects. Costs what the
    /// file has changed lines; see [`Self::holds_every_change`].
    pub fn with_every_change(text: &TextDiff) -> Self {
        let mut selection = Self::empty();
        for change in text.changes() {
            selection.select_change(change);
        }
        selection
    }

    /// Every changed line of one change — what staging a hunk selects.
    pub fn select_change(&mut self, change: &ChangedRange) {
        self.removed.extend(change.removed.numbers());
        self.added.extend(change.added.numbers());
    }

    pub fn select_removed(&mut self, line: LineNumber) {
        self.removed.insert(line);
    }

    pub fn select_added(&mut self, line: LineNumber) {
        self.added.insert(line);
    }

    pub fn unselect_removed(&mut self, line: LineNumber) {
        self.removed.remove(&line);
    }

    pub fn unselect_added(&mut self, line: LineNumber) {
        self.added.remove(&line);
    }

    pub fn holds_removed(&self, line: LineNumber) -> bool {
        self.removed.contains(&line)
    }

    pub fn holds_added(&self, line: LineNumber) -> bool {
        self.added.contains(&line)
    }

    pub fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.added.is_empty()
    }

    /// How many changed lines are selected, both sides together.
    pub fn len(&self) -> usize {
        self.removed.len() + self.added.len()
    }

    pub fn removed(&self) -> impl Iterator<Item = LineNumber> + '_ {
        self.removed.iter().copied()
    }

    pub fn added(&self) -> impl Iterator<Item = LineNumber> + '_ {
        self.added.iter().copied()
    }

    /// Whether every changed line of `text` is selected — the difference between staging
    /// a whole file and staging part of it, which decides whether a deletion stays a
    /// deletion.
    ///
    /// Reads every changed LINE, not every change, so a file with tens of thousands of them
    /// costs milliseconds. [`crate::emit_patch`] asks it once, off the UI thread. A
    /// whole-file control that wants the same answer per frame should keep it and update it
    /// as the selection changes, not recompute it; the per-row [`Self::holds_removed`] and
    /// [`Self::holds_added`] are the cheap ones.
    pub fn holds_every_change(&self, text: &TextDiff) -> bool {
        text.changes().iter().all(|change| {
            change
                .removed
                .numbers()
                .all(|line| self.holds_removed(line))
                && change.added.numbers().all(|line| self.holds_added(line))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiffLine, LineSpan, split_lines};

    fn diff() -> TextDiff {
        TextDiff::new(
            split_lines(b"a\nb\nc\nd\n"),
            split_lines(b"a\nB\nc\nd\ne\n"),
            vec![
                ChangedRange::new(LineSpan::at(1, 1), LineSpan::at(1, 1)),
                ChangedRange::new(LineSpan::at(4, 0), LineSpan::at(4, 1)),
            ],
        )
    }

    /// Caught by: one set for both sides, which would make removed line 1 and added
    /// line 1 the same selection.
    #[test]
    fn a_removed_line_and_an_added_line_of_the_same_number_are_two_selections() {
        let mut selection = Selection::empty();
        selection.select_removed(LineNumber::from_index(1));
        assert!(selection.holds_removed(LineNumber::from_index(1)));
        assert!(
            !selection.holds_added(LineNumber::from_index(1)),
            "selecting a removed line selected the added line of the same number"
        );
        assert_eq!(selection.len(), 1);

        selection.select_added(LineNumber::from_index(1));
        assert_eq!(selection.len(), 2);
        selection.unselect_removed(LineNumber::from_index(1));
        assert!(!selection.holds_removed(LineNumber::from_index(1)));
        assert!(
            selection.holds_added(LineNumber::from_index(1)),
            "unselecting one side unselected the other"
        );
    }

    #[test]
    fn nothing_is_selected_until_something_is() {
        let selection = Selection::empty();
        assert!(selection.is_empty());
        assert_eq!(selection.len(), 0);
        assert_eq!(selection.removed().count(), 0);
        assert_eq!(selection.added().count(), 0);
        assert!(
            selection.holds_every_change(&TextDiff::new(
                vec![DiffLine::terminated("a")],
                vec![DiffLine::terminated("a")],
                Vec::new()
            )),
            "a diff with no changes was not entirely selected by the empty selection"
        );
    }

    /// Caught by: selecting the changed ranges' bounds rather than their lines, which
    /// leaves the middle of a multi-line change unselected.
    #[test]
    fn selecting_every_change_selects_every_line_of_every_change() {
        let text = diff();
        let selection = Selection::with_every_change(&text);
        assert!(selection.holds_removed(LineNumber::from_index(1)));
        assert!(selection.holds_added(LineNumber::from_index(1)));
        assert!(selection.holds_added(LineNumber::from_index(4)));
        assert_eq!(
            selection.len(),
            3,
            "a line outside the changed ranges was selected"
        );
        assert!(selection.holds_every_change(&text));
    }

    /// The question a deletion's headers turn on: partial means the file survives.
    #[test]
    fn one_line_left_out_is_not_every_change() {
        let text = diff();
        let mut selection = Selection::with_every_change(&text);
        selection.unselect_added(LineNumber::from_index(4));
        assert!(
            !selection.holds_every_change(&text),
            "a selection missing an added line passed as complete"
        );

        let mut selection = Selection::with_every_change(&text);
        selection.unselect_removed(LineNumber::from_index(1));
        assert!(
            !selection.holds_every_change(&text),
            "a selection missing a removed line passed as complete"
        );
    }

    #[test]
    fn selecting_a_change_selects_both_of_its_sides() {
        let mut selection = Selection::empty();
        selection.select_change(&ChangedRange::new(LineSpan::at(2, 2), LineSpan::at(7, 3)));
        let removed: Vec<u32> = selection.removed().map(LineNumber::index).collect();
        let added: Vec<u32> = selection.added().map(LineNumber::index).collect();
        assert_eq!(removed, vec![2, 3]);
        assert_eq!(added, vec![7, 8, 9]);
    }
}
