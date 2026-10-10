//! Local Changes' selection (staging-and-commit R8.1, the redesign's rule 8 and L1): the paths
//! selected in one of the two lists, the primary among them — the file whose diff is shown — and
//! the path a range extends from. One value; `None` (no selection at all) is the only way to
//! have nothing selected, so there is no empty selection still "in" a list.
//!
//! **Paths, not rows.** A selection names paths, kept in the lists' own order
//! (`cairn_model::path_order`), so a refresh that moves rows under it keeps it, a row is drawn
//! selected by a binary search as it is built, the first selected path is the first one held,
//! and nothing walks the lists to draw one. One list at a time, as Fork's lists are two:
//! selecting in the other list starts a selection there.
//!
//! **The primary is the file clicked first** (observed in Fork for Windows 2.21.1, `lc2`): a
//! plain press makes its row the selection, primary and anchor; a ⌘- or Ctrl-press toggles a
//! row in — where ranges extend from next, the primary staying the file clicked first — or out,
//! the primary toggled out moving to the nearest path still selected; a Shift-press or
//! Shift+↑/↓ spans from the anchor to the row, the row primary. Where the selection goes after
//! an action or a refresh is the application's one rule (`next_selection`), built on
//! [`LocalSelection::kept`] and [`nearest_remaining`].

use std::cmp::Ordering;

use cairn_model::{ChangeList, RepoPath, path_order};

/// The paths selected in one list — never none — the primary among them and the anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSelection {
    list: ChangeList,
    /// In the lists' order (`path_order`), each once; never empty.
    paths: Vec<RepoPath>,
    /// One of `paths`: the file whose diff is shown, where ↑ and ↓ move from.
    primary: RepoPath,
    /// One of `paths`: where a range press or Shift+↑/↓ extends from.
    anchor: RepoPath,
}

/// Where `path` is, or would go, among `paths`, which are in the lists' order.
fn place(paths: &[RepoPath], path: &RepoPath) -> Result<usize, usize> {
    paths.binary_search_by(|held| path_order(held.as_bytes(), path.as_bytes()))
}

/// The path of `paths` nearest `gone`, which is not among them: the one after it in the lists'
/// order — the one that takes its place — else the one before. `None` when `paths` is empty.
fn nearest(paths: &[RepoPath], gone: &RepoPath) -> Option<RepoPath> {
    let at = paths
        .partition_point(|held| path_order(held.as_bytes(), gone.as_bytes()) == Ordering::Less);
    paths
        .get(at)
        .or_else(|| at.checked_sub(1).and_then(|before| paths.get(before)))
        .cloned()
}

impl LocalSelection {
    /// A plain press: `path` alone, in `list`, primary and anchor.
    pub fn of(list: ChangeList, path: RepoPath) -> Self {
        Self {
            list,
            paths: vec![path.clone()],
            primary: path.clone(),
            anchor: path,
        }
    }

    /// A range press or Shift+↑/↓: every one of `paths` in `list` — `anchor` and `row` among
    /// them — the range extending from `anchor`, `row` the primary.
    pub fn spanning(
        list: ChangeList,
        anchor: RepoPath,
        row: RepoPath,
        paths: impl IntoIterator<Item = RepoPath>,
    ) -> Self {
        let mut paths: Vec<RepoPath> = paths
            .into_iter()
            .chain([anchor.clone(), row.clone()])
            .collect();
        paths.sort_unstable_by(|one, other| path_order(one.as_bytes(), other.as_bytes()));
        paths.dedup();
        Self {
            list,
            paths,
            primary: row,
            anchor,
        }
    }

    /// A ⌘- or Ctrl-press on `path` of `list`: toggled into `selection` — where a range extends
    /// from next, the primary the file clicked first — or out of it, the primary or the anchor
    /// toggled out moving to the nearest path still selected; the last path toggled out leaves
    /// nothing selected. A path of the other list, or with nothing selected, starts a selection
    /// there.
    pub fn toggled(selection: Option<&Self>, list: ChangeList, path: RepoPath) -> Option<Self> {
        let Some(selection) = selection.filter(|selection| selection.list == list) else {
            return Some(Self::of(list, path));
        };
        let mut toggled = selection.clone();
        match place(&toggled.paths, &path) {
            Err(at) => {
                toggled.paths.insert(at, path.clone());
                toggled.anchor = path;
            }
            Ok(at) => {
                toggled.paths.remove(at);
                if toggled.primary == path {
                    toggled.primary = nearest(&toggled.paths, &path)?;
                }
                if toggled.anchor == path {
                    toggled.anchor = toggled.primary.clone();
                }
            }
        }
        Some(toggled)
    }

    /// The selection's paths that `keep` keeps, in its list — the primary kept or moved to the
    /// nearest path kept, the anchor kept or moved to the primary — or `None` when it keeps none.
    /// Costs the selection, never the lists `keep` asks.
    pub fn kept(&self, keep: impl Fn(&RepoPath) -> bool) -> Option<Self> {
        let paths: Vec<RepoPath> = self
            .paths
            .iter()
            .filter(|path| keep(path))
            .cloned()
            .collect();
        if paths.len() == self.paths.len() {
            return Some(self.clone());
        }
        let primary = if place(&paths, &self.primary).is_ok() {
            self.primary.clone()
        } else {
            nearest(&paths, &self.primary)?
        };
        let anchor = if place(&paths, &self.anchor).is_ok() {
            self.anchor.clone()
        } else {
            primary.clone()
        };
        Some(Self {
            list: self.list,
            paths,
            primary,
            anchor,
        })
    }

    /// The list the paths are selected in.
    pub fn list(&self) -> ChangeList {
        self.list
    }

    /// The paths selected, in the lists' order; never none.
    pub fn paths(&self) -> &[RepoPath] {
        &self.paths
    }

    /// The first path selected in the lists' order: whose place a selection that leaves whole
    /// gives to the row that takes it (R8.3).
    pub fn first(&self) -> &RepoPath {
        // Never empty: every constructor and every change leaves a path, or no selection.
        self.paths.first().unwrap_or(&self.primary)
    }

    /// The file whose diff is shown: the file clicked first.
    pub fn primary(&self) -> &RepoPath {
        &self.primary
    }

    /// Where a range extends from.
    pub fn anchor(&self) -> &RepoPath {
        &self.anchor
    }

    /// Whether `path` is selected in `list`: a binary search.
    pub fn holds(&self, list: ChangeList, path: &RepoPath) -> bool {
        self.list == list && place(&self.paths, path).is_ok()
    }
}

/// The row a list moves to once the rows `acted` says leave it, a list of `len` rows, measured
/// from `first`, the first row's place (R8.3): the first row at or after it that stays — the
/// row that slides into its place — else the nearest above it; `None` when no row stays. Rows
/// past `len` are never answered.
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

    fn names(selection: &LocalSelection) -> Vec<&str> {
        selection
            .paths()
            .iter()
            .map(|path| std::str::from_utf8(path.as_bytes()).unwrap_or("?"))
            .collect()
    }

    const U: ChangeList = ChangeList::Unstaged;
    const S: ChangeList = ChangeList::Staged;

    /// R8.1, observed `lc2`: a plain press selects one path, primary and anchor; a toggle adds
    /// a path without making it primary — the primary is the file clicked first — and moves
    /// the anchor to it; a toggle in the other list, or with nothing selected, starts a
    /// selection there; the paths are kept in the lists' order. Caught by: the toggled path made
    /// primary (Cairn's old brief, superseded by Fork's observed behaviour), a toggle that keeps
    /// the other list's paths, or paths in byte order (`b10` before `b9`).
    #[test]
    fn a_toggle_adds_a_path_and_the_file_clicked_first_stays_primary() {
        let one = LocalSelection::of(U, path("b9"));
        assert_eq!(one.primary(), &path("b9"));
        assert_eq!(one.anchor(), &path("b9"));
        assert!(one.holds(U, &path("b9")));
        assert!(!one.holds(S, &path("b9")));
        let two = LocalSelection::toggled(Some(&one), U, path("b10")).unwrap();
        assert_eq!(names(&two), ["b9", "b10"]);
        assert_eq!(two.primary(), &path("b9"), "the file clicked first");
        assert_eq!(two.anchor(), &path("b10"));
        assert_eq!(two.first(), &path("b9"));
        let three = LocalSelection::toggled(Some(&two), U, path("a")).unwrap();
        assert_eq!(names(&three), ["a", "b9", "b10"]);
        assert_eq!(three.primary(), &path("b9"));
        assert_eq!(three.first(), &path("a"));
        let other = LocalSelection::toggled(Some(&three), S, path("c")).unwrap();
        assert_eq!(other, LocalSelection::of(S, path("c")));
        assert_eq!(
            LocalSelection::toggled(None, S, path("c")),
            Some(LocalSelection::of(S, path("c")))
        );
    }

    /// R8.1: the primary toggled out moves to the nearest path still selected — the one after
    /// it, else the one before — the anchor toggled out to the primary, and the last path
    /// toggled out leaves nothing selected. Caught by: a primary left on a path no longer
    /// selected (the diff showing an unselected file), the first path chosen rather than the
    /// nearest, or an empty selection still held.
    #[test]
    fn the_primary_toggled_out_moves_to_the_nearest_path_and_the_last_leaves_none() {
        let mut selection = LocalSelection::of(U, path("c"));
        for name in ["a", "e", "g"] {
            selection = LocalSelection::toggled(Some(&selection), U, path(name)).unwrap();
        }
        assert_eq!(selection.anchor(), &path("g"));
        let without_c = LocalSelection::toggled(Some(&selection), U, path("c")).unwrap();
        assert_eq!(without_c.primary(), &path("e"), "the path after it");
        let without_g = LocalSelection::toggled(Some(&without_c), U, path("g")).unwrap();
        assert_eq!(
            without_g.anchor(),
            &path("e"),
            "the anchor moved to the primary"
        );
        let without_e = LocalSelection::toggled(Some(&without_g), U, path("e")).unwrap();
        assert_eq!(
            without_e.primary(),
            &path("a"),
            "with none after, the path before"
        );
        assert_eq!(
            LocalSelection::toggled(Some(&without_e), U, path("a")),
            None
        );
    }

    /// R8.1: a range holds every path it was given once each, the anchor and the row among
    /// them, the row primary and the anchor kept. Caught by: the anchor made primary, or a path
    /// held twice.
    #[test]
    fn a_range_spans_from_the_anchor_and_its_row_is_primary() {
        let range = LocalSelection::spanning(
            S,
            path("c"),
            path("e"),
            [path("e"), path("d"), path("c"), path("d")],
        );
        assert_eq!(names(&range), ["c", "d", "e"]);
        assert_eq!(range.primary(), &path("e"));
        assert_eq!(range.anchor(), &path("c"));
        assert_eq!(range.list(), S);
    }

    /// L5: what a selection keeps — all of it, unchanged; some, the primary kept or moved to
    /// the nearest kept path and the anchor kept or moved to the primary; none, nothing.
    /// Caught by: a kept selection whose primary or anchor moved though both stayed, or a
    /// primary left on a path not kept.
    #[test]
    fn a_selection_keeps_its_primary_and_anchor_while_they_stay() {
        let range =
            LocalSelection::spanning(U, path("a"), path("c"), [path("a"), path("b"), path("c")]);
        assert_eq!(range.kept(|_| true), Some(range.clone()));
        let no_b = range.kept(|p| p != &path("b")).unwrap();
        assert_eq!(names(&no_b), ["a", "c"]);
        assert_eq!((no_b.primary(), no_b.anchor()), (&path("c"), &path("a")));
        let no_c = range.kept(|p| p != &path("c")).unwrap();
        assert_eq!(no_c.primary(), &path("b"), "the nearest kept, before it");
        let only_c = range.kept(|p| p == &path("c")).unwrap();
        assert_eq!(
            only_c.anchor(),
            &path("c"),
            "the anchor moved to the primary"
        );
        assert_eq!(range.kept(|_| false), None);
    }

    /// R8.3: the row that slides into the first acted row's place, else the nearest row above
    /// it, and none when every row went. Caught by: the row after the last acted on chosen for
    /// a selection with a gap, the top of the list chosen, or an acted row chosen.
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
