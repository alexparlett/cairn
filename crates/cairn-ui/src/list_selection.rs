//! Local Changes' multi-selection (staging-and-commit R8.1, R8.3): the paths selected in one of
//! the two lists, and the path a range extends from.
//!
//! **Paths, not rows.** A selection names paths, kept sorted by their bytes, so a refresh that
//! moves rows under it keeps it, a row is drawn selected by a binary search as it is built, and
//! nothing walks the lists to draw one. One list at a time, as Fork's lists are two: selecting
//! in the other list starts a selection there. What the selection names that the lists no
//! longer list is left out where it is acted on, never pruned here, so a refresh costs the
//! selection nothing.
//!
//! **After a stage or unstage** the selection moves to the nearest path left in the list it
//! left ([`nearest_remaining`], R8.3, Fork's Tracker #514).

use cairn_model::{ChangeList, RepoPath};

/// The paths selected in one list, and the path a range extends from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListSelection {
    /// The list the paths are selected in; `None` while nothing is.
    list: Option<ChangeList>,
    /// Sorted by their bytes, each once.
    paths: Vec<RepoPath>,
    /// Where a range press or Shift+↑/↓ extends from: the path last pressed plainly or toggled.
    anchor: Option<RepoPath>,
}

impl ListSelection {
    /// `path` alone, in `list`, the anchor of any range from it.
    pub fn of(list: ChangeList, path: RepoPath) -> Self {
        Self {
            list: Some(list),
            paths: vec![path.clone()],
            anchor: Some(path),
        }
    }

    /// Every one of `paths`, in `list`, ranges extending from `anchor`.
    pub fn spanning(
        list: ChangeList,
        anchor: RepoPath,
        paths: impl IntoIterator<Item = RepoPath>,
    ) -> Self {
        let mut paths: Vec<RepoPath> = paths.into_iter().collect();
        paths.sort_unstable();
        paths.dedup();
        Self {
            list: Some(list),
            paths,
            anchor: Some(anchor),
        }
    }

    /// This selection with `path` of `list` toggled in or out (a ⌘- or Ctrl-press); a path of
    /// the other list starts a selection there. The path toggled is where a range extends
    /// from next. The last path toggled out leaves the list's selection empty, and still the
    /// list's: nothing is selected there, so nothing is acted on and nothing drawn selected.
    pub fn toggled(&self, list: ChangeList, path: RepoPath) -> Self {
        if self.list != Some(list) {
            return Self::of(list, path);
        }
        let mut paths = self.paths.clone();
        match paths.binary_search(&path) {
            Ok(at) => {
                paths.remove(at);
            }
            Err(at) => paths.insert(at, path.clone()),
        }
        Self {
            list: Some(list),
            paths,
            anchor: Some(path),
        }
    }

    /// The list the selection is in: `None` until something is selected, and the list still
    /// once a toggle has emptied it.
    pub fn list(&self) -> Option<ChangeList> {
        self.list
    }

    /// The paths selected, sorted by their bytes.
    pub fn paths(&self) -> &[RepoPath] {
        &self.paths
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Whether `path` is selected in `list`: a binary search.
    pub fn holds(&self, list: ChangeList, path: &RepoPath) -> bool {
        self.list == Some(list) && self.paths.binary_search(path).is_ok()
    }

    /// Where a range extends from, in the selection's list.
    pub fn anchor(&self) -> Option<&RepoPath> {
        self.anchor.as_ref()
    }
}

/// The row the selection moves to once the rows acted on leave a list of `len` rows (R8.3): the
/// nearest row left, measured from `first`, the first row acted on — the row that slides into
/// its place, else the nearest above it; `None` when every row was acted on. `acted` says
/// whether a row was; rows past `len` are never answered.
pub fn nearest_remaining(len: usize, first: usize, acted: impl Fn(usize) -> bool) -> Option<usize> {
    let first = first.min(len);
    (first..len)
        .find(|row| !acted(*row))
        .or_else(|| (0..first).rev().find(|row| !acted(*row)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> RepoPath {
        RepoPath::from(name)
    }

    /// R8.1: a plain press selects one path, a toggle adds and removes paths in its list and
    /// starts again in the other, a range holds what it was given once each; a path is held
    /// only in its own list. Caught by: a toggle that keeps the other list's paths, a removal
    /// that leaves the path, or a selection that holds a path in both lists.
    #[test]
    fn a_selection_toggles_spans_and_holds_paths_of_one_list() {
        let one = ListSelection::of(ChangeList::Unstaged, path("b"));
        assert!(one.holds(ChangeList::Unstaged, &path("b")));
        assert!(!one.holds(ChangeList::Staged, &path("b")));
        let two = one.toggled(ChangeList::Unstaged, path("a"));
        assert_eq!(two.paths(), [path("a"), path("b")]);
        assert_eq!(two.anchor(), Some(&path("a")));
        let back = two.toggled(ChangeList::Unstaged, path("b"));
        assert_eq!(back.paths(), [path("a")]);
        let empty = back.toggled(ChangeList::Unstaged, path("a"));
        assert!(empty.is_empty());
        assert_eq!(
            empty.list(),
            Some(ChangeList::Unstaged),
            "an emptied selection is still its list's: nothing selected there"
        );
        let other = two.toggled(ChangeList::Staged, path("c"));
        assert_eq!(other.paths(), [path("c")]);
        assert_eq!(other.list(), Some(ChangeList::Staged));
        let range = ListSelection::spanning(
            ChangeList::Staged,
            path("c"),
            [path("e"), path("c"), path("d"), path("c")],
        );
        assert_eq!(range.paths(), [path("c"), path("d"), path("e")]);
        assert_eq!(range.len(), 3);
    }

    /// R8.3: the selection moves to the row that slides into the first acted row's place, else
    /// the nearest row above it, and to none when every row went. Caught by: the row after the
    /// last acted on chosen for a selection with a gap, the top of the list chosen, or an acted
    /// row chosen.
    #[test]
    fn the_nearest_row_left_is_the_one_that_takes_the_first_acted_rows_place() {
        let acted = |rows: &'static [usize]| move |row: usize| rows.contains(&row);
        assert_eq!(nearest_remaining(5, 1, acted(&[1, 2])), Some(3));
        assert_eq!(nearest_remaining(5, 1, acted(&[1, 3])), Some(2));
        assert_eq!(nearest_remaining(5, 3, acted(&[3, 4])), Some(2));
        assert_eq!(nearest_remaining(3, 0, acted(&[0, 1, 2])), None);
        assert_eq!(nearest_remaining(0, 0, acted(&[])), None);
        assert_eq!(nearest_remaining(3, 9, acted(&[])), Some(2));
    }
}
