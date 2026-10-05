//! The files opened in place in the Commit tab (PRD R5.3), and where each one's rows fall in
//! the tab's one list.
//!
//! The Commit tab is one virtualised list of rows of one height (R5.5): its header, then a
//! row per changed file — and, under a file opened in place, that file's own rows: its diff,
//! unified or side by side, or the notice that stands in place of rows, or a line saying it is
//! being read or could not be. Opened files make this the longest list in the application,
//! so a row is found from its index without walking the files: the opened files are kept in
//! index order with the rows each adds and the rows the opened files before it add, in both
//! layouts, and an index is placed by a binary search of them — `O(log opened)` per row built,
//! however many files the commit touched or how deep the list is scrolled. The table is built
//! again from the first file that opened, closed or was answered — never per frame — and each
//! file's row counts are worked out once, as it is set, so a page of Expand All appended after
//! the files already open costs the page, not every file open before it.

use std::collections::BTreeMap;

use cairn_model::ShownDiff;

use crate::diff_notice::{DiffNotice, notice_rows};

/// What one file opened in place draws under its row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// Asked for, and not answered yet.
    Reading,
    /// It could not be read: why, as display text. That file's alone (phase 04 QA's R1).
    Failed(String),
    /// Its diff, prepared on the worker that read it.
    Shown(Box<ShownDiff>),
}

impl Opened {
    /// How many rows it adds under its file's row, side by side or unified.
    fn rows(&self, side_by_side: bool) -> usize {
        match self {
            Self::Reading | Self::Failed(_) => 1,
            Self::Shown(shown) => match DiffNotice::of(shown) {
                Some(notice) => notice_rows(&notice).len(),
                // Ignoring whitespace says it is hiding something (R6.7): a row of its own
                // above the diff, since an opened file has no bar to say it in.
                None => shown.rows(side_by_side) + usize::from(shown.hides_changes()),
            },
        }
    }
}

/// One opened file's place: its index in the change set, the rows it adds, and the rows the
/// opened files before it add — unified first, then side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placed {
    file: usize,
    rows: [usize; 2],
    before: [usize; 2],
}

/// The files of one change set opened in place, by their index in it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Expansion {
    /// What each file draws, and the rows that adds, unified then side by side.
    opened: BTreeMap<usize, (Opened, [usize; 2])>,
    placed: Vec<Placed>,
    /// Expand All stopped with its line budget spent, leaving the rest collapsed.
    stopped_at_budget: bool,
}

/// What an item of the files' part of the tab's list is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Item {
    /// The row of the change set's file at this index.
    File(usize),
    /// Row `row` of what the opened file `file` draws under its row.
    Under { file: usize, row: usize },
}

fn layout(side_by_side: bool) -> usize {
    usize::from(side_by_side)
}

impl Expansion {
    /// Nothing open: what a commit's files start as (R5.3, Fork's default).
    pub const fn new() -> Self {
        Self {
            opened: BTreeMap::new(),
            placed: Vec::new(),
            stopped_at_budget: false,
        }
    }

    /// How many files are open.
    pub fn len(&self) -> usize {
        self.opened.len()
    }

    pub fn is_empty(&self) -> bool {
        self.opened.is_empty()
    }

    /// What the file at `index` draws under its row, if it is open.
    pub fn get(&self, index: usize) -> Option<&Opened> {
        self.opened.get(&index).map(|(opened, _)| opened)
    }

    pub fn is_open(&self, index: usize) -> bool {
        self.opened.contains_key(&index)
    }

    /// The files open, in index order, with what each draws.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &Opened)> {
        self.opened
            .iter()
            .map(|(index, (opened, _))| (*index, opened))
    }

    /// Sets what each file draws, opening any that was not open; returns what each replaced,
    /// for the caller to let go of off the UI thread. The table is built again from the first
    /// file set on.
    pub fn set(&mut self, files: impl IntoIterator<Item = (usize, Opened)>) -> Vec<Opened> {
        let mut first = None::<usize>;
        let mut replaced = Vec::new();
        for (index, opened) in files {
            first = Some(first.map_or(index, |at| at.min(index)));
            let rows = [opened.rows(false), opened.rows(true)];
            if let Some((was, _)) = self.opened.insert(index, (opened, rows)) {
                replaced.push(was);
            }
        }
        if let Some(first) = first {
            self.rebuild_from(first);
        }
        replaced
    }

    /// Closes the files named; returns what each drew.
    pub fn close(&mut self, files: impl IntoIterator<Item = usize>) -> Vec<Opened> {
        let mut first = None::<usize>;
        let mut closed = Vec::new();
        for index in files {
            if let Some((opened, _)) = self.opened.remove(&index) {
                first = Some(first.map_or(index, |at| at.min(index)));
                closed.push(opened);
            }
        }
        if let Some(first) = first {
            self.rebuild_from(first);
        }
        closed
    }

    /// Closes every file; returns what each drew.
    pub fn close_all(&mut self) -> Vec<Opened> {
        self.placed.clear();
        self.stopped_at_budget = false;
        std::mem::take(&mut self.opened)
            .into_values()
            .map(|(opened, _)| opened)
            .collect()
    }

    /// Whether Expand All stopped with its budget spent: what the tab says beside it.
    pub fn stopped_at_budget(&self) -> bool {
        self.stopped_at_budget
    }

    pub fn set_stopped_at_budget(&mut self, stopped: bool) {
        self.stopped_at_budget = stopped;
    }

    /// The table rebuilt from the file at `first` on: the places before it stand.
    fn rebuild_from(&mut self, first: usize) {
        let kept = self.placed.partition_point(|placed| placed.file < first);
        self.placed.truncate(kept);
        let mut before = self.placed.last().map_or([0, 0], |last| {
            [last.before[0] + last.rows[0], last.before[1] + last.rows[1]]
        });
        for (index, (_, rows)) in self.opened.range(first..) {
            self.placed.push(Placed {
                file: *index,
                rows: *rows,
                before,
            });
            before = [before[0] + rows[0], before[1] + rows[1]];
        }
    }

    /// How many rows the opened files add, in the layout `side_by_side` names.
    pub(crate) fn added_rows(&self, side_by_side: bool) -> usize {
        let at = layout(side_by_side);
        self.placed
            .last()
            .map_or(0, |last| last.before[at] + last.rows[at])
    }

    /// What item `at` of the files' part of the list is: `O(log opened)`.
    pub(crate) fn item(&self, at: usize, side_by_side: bool) -> Item {
        let which = layout(side_by_side);
        // The first item of an opened file is its own row, at its index plus the rows the
        // opened files before it add.
        let starts = |placed: &Placed| placed.file + placed.before[which];
        let following = self.placed.partition_point(|placed| starts(placed) <= at);
        let Some(placed) = following.checked_sub(1).and_then(|k| self.placed.get(k)) else {
            return Item::File(at);
        };
        let start = starts(placed);
        let past = at - start;
        if past == 0 {
            Item::File(placed.file)
        } else if past <= placed.rows[which] {
            Item::Under {
                file: placed.file,
                row: past - 1,
            }
        } else {
            Item::File(placed.file + (past - placed.rows[which]))
        }
    }

    /// Where the file at `index`'s own row is in the files' part of the list: `O(log opened)`.
    pub(crate) fn position(&self, index: usize, side_by_side: bool) -> usize {
        let which = layout(side_by_side);
        let opened_before = self.placed.partition_point(|placed| placed.file < index);
        let added = opened_before
            .checked_sub(1)
            .and_then(|k| self.placed.get(k))
            .map_or(0, |placed| placed.before[which] + placed.rows[which]);
        index + added
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Placing an item is the inverse of laying the list out by hand: every item of a list of
    /// eight files, three of them open with one, two and three rows under them, read back as
    /// the file or the row under it that a walk from the top finds there, in both layouts —
    /// and every file's own row found at its position. Caught by: an off-by-one at an opened
    /// file's first or last row, the rows of the opened files before it not counted, or the
    /// layouts' counts swapped.
    #[test]
    fn every_item_is_the_file_or_row_a_walk_from_the_top_finds() {
        let mut expansion = Expansion::default();
        expansion.set([
            (0, Opened::Reading),
            (3, Opened::Failed("no".to_owned())),
            (7, Opened::Reading),
        ]);
        // Give the second a different count per layout by hand, and place the table again
        // from it.
        if let Some((_, rows)) = expansion.opened.get_mut(&3) {
            *rows = [2, 3];
        }
        expansion.rebuild_from(3);
        assert_eq!(expansion.placed[2].before, [3, 4]);
        for side_by_side in [false, true] {
            let which = layout(side_by_side);
            let mut walked = Vec::new();
            for file in 0..8 {
                walked.push(Item::File(file));
                if let Some(placed) = expansion.placed.iter().find(|p| p.file == file) {
                    for row in 0..placed.rows[which] {
                        walked.push(Item::Under { file, row });
                    }
                }
            }
            assert_eq!(
                walked.len(),
                8 + expansion.added_rows(side_by_side),
                "side by side: {side_by_side}"
            );
            for (at, item) in walked.iter().enumerate() {
                assert_eq!(expansion.item(at, side_by_side), *item, "item {at}");
                if let Item::File(file) = item {
                    assert_eq!(expansion.position(*file, side_by_side), at, "file {file}");
                }
            }
        }
    }

    /// The table built a piece at a time — files appended after those open, one set before
    /// them, one closed between — is the table built whole. Caught by: a rebuild that starts
    /// past the first file changed (a stale place after it), or one that drops the places
    /// before it.
    #[test]
    fn a_table_built_a_piece_at_a_time_is_the_table_built_whole() {
        let mut pieces = Expansion::default();
        pieces.set([(4, Opened::Reading), (9, Opened::Reading)]);
        pieces.set([(12, Opened::Failed("x".to_owned())), (15, Opened::Reading)]);
        pieces.set([(1, Opened::Reading)]);
        pieces.close([9]);
        pieces.set([(9, Opened::Reading), (20, Opened::Reading)]);
        let mut whole = Expansion::default();
        whole.set(
            [1, 4, 9, 12, 15, 20]
                .into_iter()
                .map(|index| (index, pieces.get(index).cloned().unwrap_or(Opened::Reading))),
        );
        assert_eq!(pieces.placed, whole.placed);
        for at in 0..40 {
            assert_eq!(pieces.item(at, false), whole.item(at, false), "item {at}");
        }
    }

    /// Nothing open: every item is its file, and closing every file says what each drew.
    #[test]
    fn with_nothing_open_every_item_is_its_file() {
        let mut expansion = Expansion::default();
        assert_eq!(expansion.item(41, false), Item::File(41));
        assert_eq!(expansion.position(41, true), 41);
        expansion.set([(2, Opened::Reading)]);
        expansion.set_stopped_at_budget(true);
        assert_eq!(expansion.close_all(), [Opened::Reading]);
        assert!(expansion.is_empty() && !expansion.stopped_at_budget());
        assert_eq!(expansion.item(3, false), Item::File(3));
    }
}
