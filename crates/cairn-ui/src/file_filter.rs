//! The Changes tab's filter over a change set's files (PRD R5.4): which files a filter's text
//! leaves in the list, and the list's rows as the files they show.
//!
//! **A list operation, run where it may take its time.** Matching is a pass over every path,
//! and a commit can touch 55,184 of them, so `cairn_model::ChangeSet::files_matching` is not
//! run on the UI thread:
//! the application runs it on a worker per filter text, newest first, and hands the window
//! the answer as [`ShownFiles`] — indices into the change set — which the list reads one row
//! at a time. A keystroke therefore costs the UI thread one request, and the list builds
//! only the rows in view whatever it shows.
//!
//! What matches is `ChangeSet::files_matching`'s rule.

/// The files a list shows: every file, or those a filter left, by index into the change set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ShownFiles {
    #[default]
    All,
    Filtered(Vec<u32>),
}

impl ShownFiles {
    /// How many rows the list has, of a change set of `total` files.
    pub fn len(&self, total: usize) -> usize {
        match self {
            Self::All => total,
            Self::Filtered(indices) => indices.len(),
        }
    }

    /// The change set's index of the file on `row`.
    pub fn file_at(&self, row: usize) -> Option<usize> {
        match self {
            Self::All => Some(row),
            Self::Filtered(indices) => indices.get(row).map(|index| *index as usize),
        }
    }

    /// The row that shows the change set's file `index`, if the list shows it. A search: the
    /// indices are in order.
    pub fn row_of(&self, index: usize) -> Option<usize> {
        match self {
            Self::All => Some(index),
            Self::Filtered(indices) => {
                let wanted = u32::try_from(index).ok()?;
                indices.binary_search(&wanted).ok()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_files_map_both_ways() {
        let shown = ShownFiles::Filtered(vec![2, 5, 9]);
        assert_eq!(shown.len(100), 3);
        assert_eq!(shown.file_at(1), Some(5));
        assert_eq!(shown.file_at(3), None);
        assert_eq!(shown.row_of(9), Some(2));
        assert_eq!(shown.row_of(4), None);
        assert_eq!(ShownFiles::All.len(7), 7);
        assert_eq!(ShownFiles::All.row_of(4), Some(4));
    }
}
