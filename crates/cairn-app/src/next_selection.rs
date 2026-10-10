//! Where Local Changes' selection goes (staging-and-commit R8.3, the redesign's L3 and L5,
//! C25): one pure rule, run after an action and as each status or filter's rows arrive — never
//! from a render effect — whoever moved the files: a stage, a commit made in a terminal, a
//! filter.
//!
//! **The rule.** The selected paths the lists still show, and the action did not take, stay
//! selected, the primary kept or moved to the nearest of them and the anchor kept or moved to
//! the primary. When none stays, the row that took the first selected path's place is selected
//! — the first row shown at or after it in the lists' order, past any the action took — else
//! the nearest row above; with no row left in the list, nothing (Fork leaves the diff blank
//! once the last file is staged, Tracker #1663). With nothing selected, the first row shown —
//! Unstaged's, else Staged's — is selected only when the lists before showed none: as the view
//! opens and when files appear (Fork, Tracker #2333), never after an action emptied the list.
//!
//! **In the lists' order, not by rows.** Both lists are in `cairn_model::path_order`, before a
//! refresh and after it, so a path's place is found in the new lists by a binary search
//! (`LocalChanges::row_from`, `ShownFiles::row_from`) — the row that took a vanished path's place
//! is the first after it, however many rows above it went too. So the rule reads the lists
//! before only to know whether they showed a row, and costs the selection and the paths the
//! action took, each a binary search, never the lists.
//!
//! **Through the filter** (L3): a path is kept only while the filter shows it, so the filter
//! prunes the selection to the rows it shows and nothing hidden is ever acted on.

use cairn_model::{ChangeList, LocalChanges, RepoPath, path_order};
use cairn_ui::{LocalSelection, ShownFiles, nearest_remaining};

/// Local Changes' lists as one side of the rule reads them: a status's lists and the rows the
/// filter shows of each.
#[derive(Debug, Clone, Copy)]
pub struct Rows<'a> {
    pub changes: &'a LocalChanges,
    /// Unstaged's rows shown, then Staged's.
    pub shown: [&'a ShownFiles; 2],
}

impl<'a> Rows<'a> {
    fn shown(&self, list: ChangeList) -> &'a ShownFiles {
        match list {
            ChangeList::Unstaged => self.shown[0],
            ChangeList::Staged => self.shown[1],
        }
    }

    /// How many rows `list` shows.
    fn len(&self, list: ChangeList) -> usize {
        self.shown(list).len(self.changes.len(list))
    }

    /// Whether `list` shows `path`: two binary searches.
    pub fn shows(&self, list: ChangeList, path: &RepoPath) -> bool {
        self.changes
            .row_of(list, path)
            .is_some_and(|row| self.shown(list).row_of(row).is_some())
    }

    /// The path on the `drawn`th row `list` shows.
    fn path_at(&self, list: ChangeList, drawn: usize) -> Option<&'a RepoPath> {
        let row = self.shown(list).file_at(drawn)?;
        self.changes.get(list, row).map(|change| change.path)
    }

    /// Where `path` is, or would be, among the rows `list` shows.
    fn place(&self, list: ChangeList, path: &RepoPath) -> usize {
        self.shown(list).row_from(self.changes.row_from(list, path))
    }

    /// Whether neither list shows a row.
    fn shows_none(&self) -> bool {
        ChangeList::ALL.into_iter().all(|list| self.len(list) == 0)
    }

    /// The first row shown, Unstaged's before Staged's.
    fn first_shown(&self) -> Option<LocalSelection> {
        ChangeList::ALL.into_iter().find_map(|list| {
            self.path_at(list, 0)
                .map(|path| LocalSelection::of(list, path.clone()))
        })
    }
}

/// What an action takes out of a list: leaving it, a path is not kept, and the row that takes
/// the selection's place is found past it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaving(Leaves);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Leaves {
    /// These paths, in the lists' order.
    Paths(Vec<RepoPath>),
    /// Every row a list shows: Stage All or Unstage All.
    Every(ChangeList),
}

impl Leaving {
    /// No path leaves: a refresh's or a filter's turn of the rule.
    pub fn none() -> Self {
        Self(Leaves::Paths(Vec::new()))
    }

    /// `paths` leave the selection's list.
    pub fn of(paths: impl IntoIterator<Item = RepoPath>) -> Self {
        let mut paths: Vec<RepoPath> = paths.into_iter().collect();
        paths.sort_unstable_by(|one, other| path_order(one.as_bytes(), other.as_bytes()));
        paths.dedup();
        Self(Leaves::Paths(paths))
    }

    /// Every row `list` shows leaves it — named without walking them, which the local lane
    /// gathers.
    pub fn every(list: ChangeList) -> Self {
        Self(Leaves::Every(list))
    }

    /// Whether every row of `list` leaves.
    fn empties(&self, list: ChangeList) -> bool {
        self.0 == Leaves::Every(list)
    }

    /// Whether `path` of `list` leaves.
    fn holds(&self, list: ChangeList, path: &RepoPath) -> bool {
        match &self.0 {
            Leaves::Paths(paths) => paths
                .binary_search_by(|held| path_order(held.as_bytes(), path.as_bytes()))
                .is_ok(),
            Leaves::Every(every) => *every == list,
        }
    }
}

/// What the rule makes of a selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// It stays exactly as it is — every path kept, or nothing selected and nothing to select:
    /// the usual refresh, which copies and writes nothing.
    Unchanged,
    /// It becomes this.
    To(Option<LocalSelection>),
}

/// Where `selection` goes once the lists show `after` — `before` the lists it was made in —
/// and the paths `leaving` names leave its list: the module's rule. A selection that stays
/// whole is answered [`Next::Unchanged`], with nothing copied.
pub fn next_selection(
    selection: Option<&LocalSelection>,
    before: Rows<'_>,
    after: Rows<'_>,
    leaving: &Leaving,
) -> Next {
    let Some(selection) = selection else {
        return match before.shows_none().then(|| after.first_shown()).flatten() {
            Some(first) => Next::To(Some(first)),
            None => Next::Unchanged,
        };
    };
    let list = selection.list();
    // Every row the list shows leaves: no row is left to take the selection's place.
    if leaving.empties(list) {
        return Next::To(None);
    }
    let stays = |path: &RepoPath| after.shows(list, path) && !leaving.holds(list, path);
    if selection.paths().iter().all(stays) {
        return Next::Unchanged;
    }
    if let Some(kept) = selection.kept(stays) {
        return Next::To(Some(kept));
    }
    // None stays: the row that took the first selected path's place, past any leaving.
    let place = after.place(list, selection.first());
    let row = nearest_remaining(after.len(list), place, |drawn| {
        after
            .path_at(list, drawn)
            .is_some_and(|path| leaving.holds(list, path))
    });
    Next::To(row.and_then(|row| {
        after
            .path_at(list, row)
            .map(|path| LocalSelection::of(list, path.clone()))
    }))
}

#[cfg(test)]
mod tests {
    use cairn_model::{StagedChange, StatusEntry, WorkingTreeStatus};

    use super::*;

    /// The rule's answer as the selection it leaves.
    fn moved(
        selection: Option<&LocalSelection>,
        before: Rows<'_>,
        after: Rows<'_>,
        leaving: &Leaving,
    ) -> Option<LocalSelection> {
        match next_selection(selection, before, after, leaving) {
            Next::Unchanged => selection.cloned(),
            Next::To(next) => next,
        }
    }

    const U: ChangeList = ChangeList::Unstaged;
    const S: ChangeList = ChangeList::Staged;
    const ALL: ShownFiles = ShownFiles::All;

    fn path(name: &str) -> RepoPath {
        RepoPath::from(name)
    }

    /// Unstaged lists each of `unstaged` (untracked); Staged each of `staged` (added).
    fn lists(unstaged: &[&str], staged: &[&str]) -> LocalChanges {
        LocalChanges::new(WorkingTreeStatus::Listed(
            unstaged
                .iter()
                .map(|name| StatusEntry::Untracked(path(name)))
                .chain(staged.iter().map(|name| {
                    StatusEntry::Changed(cairn_model::ChangedEntry {
                        path: path(name),
                        staged: Some(StagedChange::Added),
                        unstaged: None,
                        submodule: None,
                    })
                }))
                .collect(),
        ))
    }

    fn rows<'a>(changes: &'a LocalChanges, shown: [&'a ShownFiles; 2]) -> Rows<'a> {
        Rows { changes, shown }
    }

    /// A selection of `paths` in `list`, its primary and its anchor the first named.
    fn selecting(list: ChangeList, paths: &[&str]) -> LocalSelection {
        let mut selection = LocalSelection::of(list, path(paths[0]));
        for name in &paths[1..] {
            selection = LocalSelection::toggled(Some(&selection), list, path(name))
                .unwrap_or_else(|| unreachable!("a toggle in adds"));
        }
        selection
    }

    fn names(selection: Option<&LocalSelection>) -> Vec<String> {
        selection.map_or_else(Vec::new, |selection| {
            selection
                .paths()
                .iter()
                .map(|path| path.display().into_owned())
                .collect()
        })
    }

    /// C25 after an action (observed `lc4`): the selection staged leaves, so the row that takes
    /// the first one's place is selected — f3 after f2 and f4 are staged out of f1..f5, the
    /// gap's row, not the row after the last one — and with nothing below, the nearest above;
    /// every row taken, nothing. Caught by: the row after the last acted row chosen for a
    /// selection with a gap, the list's first row chosen, or a leaving row kept.
    #[test]
    fn after_an_action_the_row_in_the_first_acted_rows_place_is_selected() {
        let drawn = lists(&["f1", "f2", "f3", "f4", "f5"], &[]);
        let at = rows(&drawn, [&ALL, &ALL]);
        let staged = selecting(U, &["f4", "f2"]);
        let next = moved(
            Some(&staged),
            at,
            at,
            &Leaving::of([path("f2"), path("f4")]),
        );
        assert_eq!(next, Some(LocalSelection::of(U, path("f3"))));
        let last = selecting(U, &["f4", "f5"]);
        let next = moved(Some(&last), at, at, &Leaving::of([path("f4"), path("f5")]));
        assert_eq!(
            next,
            Some(LocalSelection::of(U, path("f3"))),
            "the nearest above"
        );
        let every = selecting(U, &["f1", "f2", "f3", "f4", "f5"]);
        let all = Leaving::of(every.paths().to_vec());
        assert_eq!(moved(Some(&every), at, at, &all), None);
        // Stage All: a selection in the list goes; one in the other list stays as it was.
        let some = selecting(U, &["f2"]);
        assert_eq!(moved(Some(&some), at, at, &Leaving::every(U)), None);
        let staged = lists(&["f1"], &["s1", "s2"]);
        let both = rows(&staged, [&ALL, &ALL]);
        let other = selecting(S, &["s2", "s1"]);
        assert_eq!(
            moved(Some(&other), both, both, &Leaving::every(U)).as_ref(),
            Some(&other)
        );
    }

    /// C25, the QA brief: a refresh with nothing changed — or one that only adds rows — leaves
    /// the selection, its primary and its anchor exactly as they were; one that takes a
    /// selected path away keeps the rest, the primary moved to the nearest kept path. Caught
    /// by: a refresh that resets the selection to its first row or moves its primary or anchor
    /// though both are listed, or a vanished path kept selected unseen.
    #[test]
    fn after_a_refresh_the_paths_still_listed_stay_with_their_primary_and_anchor() {
        let before = lists(&["a", "b", "c", "d"], &[]);
        let selection =
            LocalSelection::spanning(U, path("b"), path("d"), [path("b"), path("c"), path("d")]);
        let same = lists(&["a", "b", "c", "d"], &[]);
        let next = moved(
            Some(&selection),
            rows(&before, [&ALL, &ALL]),
            rows(&same, [&ALL, &ALL]),
            &Leaving::none(),
        );
        assert_eq!(next.as_ref(), Some(&selection));
        assert_eq!(
            next_selection(
                Some(&selection),
                rows(&before, [&ALL, &ALL]),
                rows(&same, [&ALL, &ALL]),
                &Leaving::none(),
            ),
            Next::Unchanged,
            "a refresh that keeps every path copied the selection"
        );
        let more = lists(&["0", "a", "b", "bb", "c", "d"], &["s"]);
        let next = moved(
            Some(&selection),
            rows(&before, [&ALL, &ALL]),
            rows(&more, [&ALL, &ALL]),
            &Leaving::none(),
        );
        assert_eq!(next.as_ref(), Some(&selection), "rows added elsewhere");
        let without_d = lists(&["a", "b", "c"], &[]);
        let next = moved(
            Some(&selection),
            rows(&before, [&ALL, &ALL]),
            rows(&without_d, [&ALL, &ALL]),
            &Leaving::none(),
        );
        assert_eq!(names(next.as_ref()), ["b", "c"]);
        let next = next.unwrap_or_else(|| unreachable!("two kept"));
        assert_eq!(next.primary(), &path("c"), "the nearest kept path");
        assert_eq!(next.anchor(), &path("b"), "the anchor kept");
    }

    /// C25, observed `lc5`: every selected path vanished — committed in a terminal — selects
    /// the row that took the first one's place: f6 for f5, and still f6 when rows above went
    /// too (a place found in the lists' order, not counted in rows); with every path after it
    /// gone, the nearest above; with the list empty, nothing — even with rows in the other
    /// list. Caught by: the place counted by rows (f7 chosen when a row above went too), the
    /// first row of the list chosen, or the other list's row chosen.
    #[test]
    fn a_vanished_selection_gives_its_place_to_the_row_that_took_it() {
        let before = lists(&["f1", "f2", "f3", "f4", "f5", "f6", "f7"], &[]);
        let selection = selecting(U, &["f5"]);
        let place = |after: &LocalChanges| {
            moved(
                Some(&selection),
                rows(&before, [&ALL, &ALL]),
                rows(after, [&ALL, &ALL]),
                &Leaving::none(),
            )
        };
        let committed = lists(&["f1", "f2", "f3", "f4", "f6", "f7"], &[]);
        assert_eq!(place(&committed), Some(LocalSelection::of(U, path("f6"))));
        let and_above = lists(&["f2", "f4", "f6", "f7"], &[]);
        assert_eq!(place(&and_above), Some(LocalSelection::of(U, path("f6"))));
        let tail_gone = lists(&["f1", "f2", "f3"], &["f5"]);
        assert_eq!(place(&tail_gone), Some(LocalSelection::of(U, path("f3"))));
        let empty = lists(&[], &["f5", "z"]);
        assert_eq!(
            place(&empty),
            None,
            "Staged's rows are not Unstaged's place"
        );
    }

    /// C25, L3: the filter prunes the selection to the rows it shows, keeping the primary where
    /// it is shown; a filter hiding every selected path selects the shown row in the first
    /// one's place; with a filter on, an action's next row is found among the rows shown, never
    /// a hidden one. Caught by: a hidden path kept selected (acted on unseen), or a hidden row
    /// chosen to move to.
    #[test]
    fn a_filter_prunes_the_selection_to_the_rows_it_shows() {
        let drawn = lists(&["a", "bx", "c", "dx", "ex", "f"], &[]);
        // Rows 1, 3 and 4 shown: bx, dx, ex.
        let filtered = ShownFiles::Filtered(vec![1, 3, 4]);
        let unfiltered = rows(&drawn, [&ALL, &ALL]);
        let shown = rows(&drawn, [&filtered, &ALL]);
        let selection = selecting(U, &["c", "a", "dx"]);
        let next = moved(Some(&selection), unfiltered, shown, &Leaving::none());
        assert_eq!(names(next.as_ref()), ["dx"]);
        assert_eq!(
            next.as_ref().map(LocalSelection::primary),
            Some(&path("dx"))
        );
        let hidden = selecting(U, &["c"]);
        let next = moved(Some(&hidden), unfiltered, shown, &Leaving::none());
        assert_eq!(
            next,
            Some(LocalSelection::of(U, path("dx"))),
            "c's place, shown"
        );
        let staged = selecting(U, &["bx"]);
        let next = moved(Some(&staged), shown, shown, &Leaving::of([path("bx")]));
        assert_eq!(
            next,
            Some(LocalSelection::of(U, path("dx"))),
            "never c, hidden"
        );
        let nothing = ShownFiles::Filtered(Vec::new());
        let none_shown = rows(&drawn, [&nothing, &ALL]);
        assert_eq!(
            moved(Some(&hidden), unfiltered, none_shown, &Leaving::none()),
            None
        );
    }

    /// R9.3, Fork's Tracker #2333 and #1663: with nothing selected, the first row shown —
    /// Unstaged's, else Staged's — is selected when the lists before showed none (the view's
    /// first status, files appearing); after an action emptied a list, nothing stays nothing.
    /// Caught by: a selection forced onto Staged's first row once Unstaged's last was staged,
    /// or none chosen as the lists first appear.
    #[test]
    fn nothing_selected_becomes_the_first_row_only_as_files_appear() {
        let empty = lists(&[], &[]);
        let some = lists(&["b", "a"], &["s"]);
        let staged_only = lists(&[], &["s", "t"]);
        let none = Leaving::none();
        let first = moved(
            None,
            rows(&empty, [&ALL, &ALL]),
            rows(&some, [&ALL, &ALL]),
            &none,
        );
        assert_eq!(first, Some(LocalSelection::of(U, path("a"))));
        let first = moved(
            None,
            rows(&empty, [&ALL, &ALL]),
            rows(&staged_only, [&ALL, &ALL]),
            &none,
        );
        assert_eq!(first, Some(LocalSelection::of(S, path("s"))));
        assert_eq!(
            moved(
                None,
                rows(&some, [&ALL, &ALL]),
                rows(&staged_only, [&ALL, &ALL]),
                &none
            ),
            None
        );
    }
}
