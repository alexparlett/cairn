//! The Local Changes view's two lists over a status (refs-and-status R9.1, R9.2): which of the
//! paths `git status` listed go in Unstaged and which in Staged, in what order, and what each
//! row draws — its path, a rename's or a copy's source, and its kind of change.
//!
//! **Laid out once, where it may take its time.** A status can list tens of thousands of paths
//! (11,000 on the bench's scratch clone), so the lists are sorted when the status is read — on
//! the refresh thread ([`LocalChanges::new`]) — and the window only keeps the answer: a row is
//! read by its index, a chosen path is found again by a binary search
//! ([`LocalChanges::row_of`]), and the filter is a pass run on a worker
//! ([`LocalChanges::matching`]). Nothing here is proportional to the status but those two.
//!
//! **Which list.** A tracked path is in Staged when it has a staged change, and in Unstaged when
//! it has an unstaged one — in both when it has both (R9.1) — or when git listed it for a
//! submodule's state alone; a conflicted path and an untracked one are in Unstaged, as Fork
//! puts them (`fork-refs-and-status-ui.md` section 6). Each list is in Fork's natural order of
//! its paths ([`path_order`], the sidebar's `natural_order`: case ignored, numbers read as
//! numbers; the user's decision, 2026-10-07), so tracked and untracked paths are mixed by name,
//! as Fork mixes them (it declined to list tracked paths first).

use std::cmp::Ordering;

use crate::text_filter::{BETWEEN_CHECKS, Folded};
use crate::{
    RepoPath, StagedChange, StatusEntry, UnstagedChange, WorkingTreeStatus, natural_order,
};

/// The order a list's paths are in: Fork's natural order of the paths as text
/// ([`natural_order`]: case ignored, each run of digits read as its number), and, between two
/// paths that read alike — bytes that are not UTF-8 read as the same replacement — their
/// bytes, so the order is total and two paths are equal only when their bytes are.
pub fn path_order(one: &[u8], other: &[u8]) -> Ordering {
    natural_order(
        &String::from_utf8_lossy(one),
        &String::from_utf8_lossy(other),
    )
    .then_with(|| one.cmp(other))
}

/// One of the view's two lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeList {
    /// What the index does not hold yet: unstaged changes, conflicts and untracked paths.
    Unstaged,
    /// What the index holds that `HEAD` does not.
    Staged,
}

impl ChangeList {
    /// Both lists, in the order the view draws them: Unstaged above Staged.
    pub const ALL: [Self; 2] = [Self::Unstaged, Self::Staged];
}

/// The kind of change a row's badge names (R9.1). Each is told apart by its badge's shape, never
/// its colour alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    Modified,
    /// Added to the index, an intent-to-add entry, or a path git does not track: one kind, as
    /// Fork draws them with one badge.
    Added,
    Deleted,
    Renamed,
    Copied,
    /// A file became a symlink or a submodule, or the reverse.
    TypeChanged,
    /// A submodule, whatever changed in it.
    Submodule,
    /// A path with unmerged index entries.
    Conflicted,
}

/// Where a row's path stands, which decides what its diff is asked as: a tracked path's diff
/// is its list's side, an untracked path's is against nothing, and a conflicted path has none
/// (its notice stands in place of one, R9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathState {
    Tracked,
    Untracked,
    Conflicted,
}

/// One row of a list, read from the status it indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalChange<'a> {
    pub path: &'a RepoPath,
    /// A rename's or a copy's source, drawn before its path.
    pub from: Option<&'a RepoPath>,
    pub kind: ChangeKind,
    pub state: PathState,
}

/// Which rows of each list a filter's text leaves, by their place in the list, in order, and
/// how many distinct paths those rows show — a path left in both lists counted once, as
/// [`LocalChanges::paths`] counts the status's (the user's decision, 2026-10-07: "Showing N of M
/// files" counts paths, as the sidebar's count does).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchedRows {
    pub unstaged: Vec<u32>,
    pub staged: Vec<u32>,
    pub paths: usize,
}

impl MatchedRows {
    pub fn of(&self, list: ChangeList) -> &[u32] {
        match list {
            ChangeList::Unstaged => &self.unstaged,
            ChangeList::Staged => &self.staged,
        }
    }
}

/// A status and its two lists, each an index into the status's entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalChanges {
    status: WorkingTreeStatus,
    unstaged: Vec<u32>,
    staged: Vec<u32>,
    /// How many distinct paths the status lists (R9.2).
    paths: usize,
}

impl LocalChanges {
    /// The lists over `status`: a pass over every entry and a sort of each list — run where
    /// the status is read, never on the UI thread. An entry past the four billionth is not
    /// listed (an index is 32 bits wide; no index file holds that many).
    pub fn new(status: WorkingTreeStatus) -> Self {
        let (mut unstaged, mut staged) = (Vec::new(), Vec::new());
        if let WorkingTreeStatus::Listed(entries) = &status {
            for (index, entry) in entries.iter().enumerate() {
                let Ok(index) = u32::try_from(index) else {
                    break;
                };
                let (in_unstaged, in_staged) = lists_of(entry);
                if in_unstaged {
                    unstaged.push(index);
                }
                if in_staged {
                    staged.push(index);
                }
            }
            let by_path =
                |one: &u32, two: &u32| path_order(path_at(entries, *one), path_at(entries, *two));
            unstaged.sort_unstable_by(by_path);
            staged.sort_unstable_by(by_path);
        }
        let paths = match &status {
            WorkingTreeStatus::Listed(entries) => {
                distinct_paths(entries, unstaged.iter().copied(), staged.iter().copied())
            }
            WorkingTreeStatus::IndexUnreadable(_) | WorkingTreeStatus::NoWorkingTree => 0,
        };
        Self {
            status,
            unstaged,
            staged,
            paths,
        }
    }

    /// What the status read answered: a list, or a state git gives none in.
    pub fn status(&self) -> &WorkingTreeStatus {
        &self.status
    }

    /// The status, given up — to be freed where the caller chooses.
    pub fn into_status(self) -> WorkingTreeStatus {
        self.status
    }

    /// How many distinct paths the status lists: a path in both lists counts once, and so does
    /// a path whose staged deletion and untracked copy git lists apart (R9.2).
    pub fn paths(&self) -> usize {
        self.paths
    }

    /// How many rows `list` has.
    pub fn len(&self, list: ChangeList) -> usize {
        self.list(list).len()
    }

    /// Whether neither list has a row.
    pub fn is_empty(&self) -> bool {
        self.unstaged.is_empty() && self.staged.is_empty()
    }

    fn list(&self, list: ChangeList) -> &[u32] {
        match list {
            ChangeList::Unstaged => &self.unstaged,
            ChangeList::Staged => &self.staged,
        }
    }

    fn entries(&self) -> &[StatusEntry] {
        match &self.status {
            WorkingTreeStatus::Listed(entries) => entries,
            WorkingTreeStatus::IndexUnreadable(_) | WorkingTreeStatus::NoWorkingTree => &[],
        }
    }

    /// Row `row` of `list`, as it draws.
    pub fn get(&self, list: ChangeList, row: usize) -> Option<LocalChange<'_>> {
        let index = *self.list(list).get(row)?;
        let entry = self.entries().get(index as usize)?;
        Some(change_of(entry, list))
    }

    /// The row of `list` that lists `path`, if it does: a binary search in [`path_order`], the
    /// order each list is sorted in, since each list lists a path at most once.
    pub fn row_of(&self, list: ChangeList, path: &RepoPath) -> Option<usize> {
        let entries = self.entries();
        self.list(list)
            .binary_search_by(|index| path_order(path_at(entries, *index), path.as_bytes()))
            .ok()
    }

    /// Which rows of each list hold `text` in their path, or in a rename's or a copy's source —
    /// the rule of the Changes tab's filter (`ChangeSet::files_matching`), case ignored. A pass
    /// over every row: run on a worker. `keep_going` is asked every few thousand rows, and a
    /// `false` abandons the pass (`None`): a newer text superseded it. The distinct paths the
    /// rows left show are counted there too, by a merge of the two lists' rows, which are in
    /// one order.
    pub fn matching(
        &self,
        text: &str,
        mut keep_going: impl FnMut() -> bool,
    ) -> Option<MatchedRows> {
        let wanted = Folded::of(text);
        let mut matched = MatchedRows::default();
        let mut seen = 0usize;
        for list in ChangeList::ALL {
            let rows = match list {
                ChangeList::Unstaged => &mut matched.unstaged,
                ChangeList::Staged => &mut matched.staged,
            };
            for row in 0..self.len(list) {
                if seen.is_multiple_of(BETWEEN_CHECKS) && !keep_going() {
                    return None;
                }
                seen += 1;
                let Some(change) = self.get(list, row) else {
                    continue;
                };
                let found = wanted.found_in(change.path.as_bytes())
                    || change
                        .from
                        .is_some_and(|from| wanted.found_in(from.as_bytes()));
                if let (true, Ok(row)) = (found, u32::try_from(row)) {
                    rows.push(row);
                }
            }
        }
        let entry = |list: ChangeList| {
            let indices = self.list(list);
            move |row: &u32| indices.get(*row as usize).copied().unwrap_or(u32::MAX)
        };
        matched.paths = distinct_paths(
            self.entries(),
            matched.unstaged.iter().map(entry(ChangeList::Unstaged)),
            matched.staged.iter().map(entry(ChangeList::Staged)),
        );
        Some(matched)
    }
}

/// Which lists an entry is in: (Unstaged, Staged).
fn lists_of(entry: &StatusEntry) -> (bool, bool) {
    match entry {
        StatusEntry::Changed(changed) => (
            changed.unstaged.is_some() || changed.staged.is_none(),
            changed.staged.is_some(),
        ),
        StatusEntry::Conflicted(_) | StatusEntry::Untracked(_) => (true, false),
    }
}

/// The path of the entry at `index`, as bytes. An index into `entries` names an entry, since
/// the lists are made from them; one that did not would sort as the empty path rather than
/// panic.
fn path_at(entries: &[StatusEntry], index: u32) -> &[u8] {
    entries
        .get(index as usize)
        .map_or(&[], |entry| entry.path().as_bytes())
}

/// The distinct paths of two lists of entries, each in its paths' order and each listing a path
/// at most once: their union, counted by a merge.
fn distinct_paths(
    entries: &[StatusEntry],
    unstaged: impl Iterator<Item = u32>,
    staged: impl Iterator<Item = u32>,
) -> usize {
    let path = |index: u32| path_at(entries, index);
    let (mut one, mut two) = (unstaged.peekable(), staged.peekable());
    let mut count = 0;
    loop {
        match (one.peek(), two.peek()) {
            (Some(a), Some(b)) => {
                match path_order(path(*a), path(*b)) {
                    std::cmp::Ordering::Less => {
                        one.next();
                    }
                    std::cmp::Ordering::Greater => {
                        two.next();
                    }
                    std::cmp::Ordering::Equal => {
                        one.next();
                        two.next();
                    }
                }
                count += 1;
            }
            (Some(_), None) | (None, Some(_)) => {
                return count + one.count() + two.count();
            }
            (None, None) => return count,
        }
    }
}

/// What `entry` draws in `list`.
fn change_of(entry: &StatusEntry, list: ChangeList) -> LocalChange<'_> {
    match entry {
        StatusEntry::Untracked(path) => LocalChange {
            path,
            from: None,
            kind: ChangeKind::Added,
            state: PathState::Untracked,
        },
        StatusEntry::Conflicted(conflicted) => LocalChange {
            path: &conflicted.path,
            from: None,
            kind: ChangeKind::Conflicted,
            state: PathState::Conflicted,
        },
        StatusEntry::Changed(changed) => {
            let (kind, from) = match list {
                ChangeList::Staged => match &changed.staged {
                    Some(StagedChange::Added) => (ChangeKind::Added, None),
                    Some(StagedChange::Modified) | None => (ChangeKind::Modified, None),
                    Some(StagedChange::Deleted) => (ChangeKind::Deleted, None),
                    Some(StagedChange::TypeChanged) => (ChangeKind::TypeChanged, None),
                    Some(StagedChange::Renamed { from, .. }) => (ChangeKind::Renamed, Some(from)),
                    Some(StagedChange::Copied { from, .. }) => (ChangeKind::Copied, Some(from)),
                },
                ChangeList::Unstaged => match &changed.unstaged {
                    Some(UnstagedChange::Modified) | None => (ChangeKind::Modified, None),
                    Some(UnstagedChange::Deleted) => (ChangeKind::Deleted, None),
                    Some(UnstagedChange::TypeChanged) => (ChangeKind::TypeChanged, None),
                    Some(UnstagedChange::IntentToAdd) => (ChangeKind::Added, None),
                    Some(UnstagedChange::Renamed { from, .. }) => (ChangeKind::Renamed, Some(from)),
                    Some(UnstagedChange::Copied { from, .. }) => (ChangeKind::Copied, Some(from)),
                },
            };
            // A submodule is drawn as one, whatever changed in it; its source, if it moved, is
            // still said.
            let kind = if changed.submodule.is_some() {
                ChangeKind::Submodule
            } else {
                kind
            };
            LocalChange {
                path: &changed.path,
                from,
                kind,
                state: PathState::Tracked,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChangedEntry, ConflictKind, ConflictedEntry, Similarity, SubmoduleState};

    fn changed(
        path: &str,
        staged: Option<StagedChange>,
        unstaged: Option<UnstagedChange>,
    ) -> StatusEntry {
        StatusEntry::Changed(ChangedEntry {
            path: RepoPath::from(path),
            staged,
            unstaged,
            submodule: None,
        })
    }

    fn untracked(path: &str) -> StatusEntry {
        StatusEntry::Untracked(RepoPath::from(path))
    }

    fn conflicted(path: &str) -> StatusEntry {
        StatusEntry::Conflicted(ConflictedEntry {
            path: RepoPath::from(path),
            kind: ConflictKind::BothModified,
            submodule: None,
        })
    }

    fn rows(changes: &LocalChanges, list: ChangeList) -> Vec<(String, ChangeKind)> {
        (0..changes.len(list))
            .map(|row| {
                let change = changes
                    .get(list, row)
                    .unwrap_or_else(|| panic!("row {row} of {list:?} is missing"));
                (change.path.display().into_owned(), change.kind)
            })
            .collect()
    }

    /// R9.1: a path with a staged and an unstaged change is in both lists; one with only a
    /// staged change is in Staged alone; conflicted and untracked paths are in Unstaged; and each
    /// list is in its paths' order, untracked mixed with tracked by name — whatever order git
    /// listed them in. Caught by: a path both staged and unstaged in one list only, a conflict or
    /// an untracked path in Staged, or git's order kept (untracked after tracked).
    #[test]
    fn each_path_is_in_the_lists_its_changes_put_it_in_ordered_by_name() {
        let changes = LocalChanges::new(WorkingTreeStatus::Listed(vec![
            changed(
                "both.txt",
                Some(StagedChange::Modified),
                Some(UnstagedChange::Modified),
            ),
            changed("staged.txt", Some(StagedChange::Added), None),
            changed("z-unstaged.txt", None, Some(UnstagedChange::Deleted)),
            conflicted("conflict.txt"),
            untracked("a-new/one.txt"),
            untracked("a-new/two.txt"),
            untracked("m.txt"),
        ]));
        assert_eq!(
            rows(&changes, ChangeList::Unstaged),
            [
                ("a-new/one.txt".to_owned(), ChangeKind::Added),
                ("a-new/two.txt".to_owned(), ChangeKind::Added),
                ("both.txt".to_owned(), ChangeKind::Modified),
                ("conflict.txt".to_owned(), ChangeKind::Conflicted),
                ("m.txt".to_owned(), ChangeKind::Added),
                ("z-unstaged.txt".to_owned(), ChangeKind::Deleted),
            ]
        );
        assert_eq!(
            rows(&changes, ChangeList::Staged),
            [
                ("both.txt".to_owned(), ChangeKind::Modified),
                ("staged.txt".to_owned(), ChangeKind::Added),
            ]
        );
        // Seven paths listed, one of them in both lists: seven distinct.
        assert_eq!(changes.paths(), 7);
        assert_eq!(
            changes
                .get(ChangeList::Unstaged, 0)
                .map(|change| change.state),
            Some(PathState::Untracked)
        );
        assert_eq!(
            changes
                .get(ChangeList::Unstaged, 3)
                .map(|change| change.state),
            Some(PathState::Conflicted)
        );
        assert_eq!(
            changes
                .get(ChangeList::Staged, 0)
                .map(|change| change.state),
            Some(PathState::Tracked)
        );
        assert_eq!(changes.get(ChangeList::Staged, 2), None);
    }

    /// R9.1's badges: every staged and unstaged kind, a rename's and a copy's source, an
    /// intent-to-add entry as added, and a submodule as a submodule whatever changed in it.
    /// Caught by: a kind mapped to another's badge, a source dropped, or a submodule drawn as a
    /// modified file.
    #[test]
    fn each_change_is_drawn_as_its_kind_with_a_sources_path() {
        let similar = Similarity::from_percent(90);
        let changes = LocalChanges::new(WorkingTreeStatus::Listed(vec![
            changed("a", Some(StagedChange::Added), None),
            changed("b", Some(StagedChange::Modified), None),
            changed("c", Some(StagedChange::Deleted), None),
            changed("d", Some(StagedChange::TypeChanged), None),
            changed(
                "e",
                Some(StagedChange::Renamed {
                    from: RepoPath::from("e-old"),
                    similarity: similar,
                }),
                None,
            ),
            changed(
                "f",
                Some(StagedChange::Copied {
                    from: RepoPath::from("f-src"),
                    similarity: similar,
                }),
                None,
            ),
            changed("g", None, Some(UnstagedChange::TypeChanged)),
            changed("h", None, Some(UnstagedChange::IntentToAdd)),
            changed(
                "i",
                None,
                Some(UnstagedChange::Renamed {
                    from: RepoPath::from("i-old"),
                    similarity: similar,
                }),
            ),
            changed(
                "j",
                None,
                Some(UnstagedChange::Copied {
                    from: RepoPath::from("j-src"),
                    similarity: similar,
                }),
            ),
            StatusEntry::Changed(ChangedEntry {
                path: RepoPath::from("k"),
                staged: Some(StagedChange::Modified),
                unstaged: Some(UnstagedChange::Modified),
                submodule: Some(SubmoduleState {
                    new_commits: true,
                    modified_content: false,
                    untracked_content: false,
                }),
            }),
            // Listed for its submodule's state alone: unstaged, as git says it.
            StatusEntry::Changed(ChangedEntry {
                path: RepoPath::from("l"),
                staged: None,
                unstaged: None,
                submodule: Some(SubmoduleState::default()),
            }),
        ]));
        let drawn = |list: ChangeList, path: &str| {
            let row = changes
                .row_of(list, &RepoPath::from(path))
                .unwrap_or_else(|| panic!("{path} is not in {list:?}"));
            let change = changes
                .get(list, row)
                .unwrap_or_else(|| unreachable!("a row found is a row"));
            (
                change.kind,
                change.from.map(|from| from.display().into_owned()),
            )
        };
        use ChangeKind::*;
        use ChangeList::{Staged as S, Unstaged as U};
        for (list, path, kind, from) in [
            (S, "a", Added, None),
            (S, "b", Modified, None),
            (S, "c", Deleted, None),
            (S, "d", TypeChanged, None),
            (S, "e", Renamed, Some("e-old")),
            (S, "f", Copied, Some("f-src")),
            (U, "g", TypeChanged, None),
            (U, "h", Added, None),
            (U, "i", Renamed, Some("i-old")),
            (U, "j", Copied, Some("j-src")),
            (S, "k", Submodule, None),
            (U, "k", Submodule, None),
            (U, "l", Submodule, None),
        ] {
            assert_eq!(
                drawn(list, path),
                (kind, from.map(str::to_owned)),
                "{path} in {list:?}"
            );
        }
        assert_eq!(changes.row_of(U, &RepoPath::from("a")), None);
        assert_eq!(changes.row_of(S, &RepoPath::from("g")), None);
    }

    /// R9.2: the count is of distinct paths — a path in both lists once, and a path git lists
    /// twice (its staged deletion, and an untracked file at the same name) once — and nothing
    /// for a status git gave no list in. Caught by: counting rows, or entries.
    #[test]
    fn the_count_is_of_distinct_paths() {
        let changes = LocalChanges::new(WorkingTreeStatus::Listed(vec![
            changed("gone.txt", Some(StagedChange::Deleted), None),
            changed(
                "both.txt",
                Some(StagedChange::Modified),
                Some(UnstagedChange::Modified),
            ),
            untracked("gone.txt"),
            untracked("new.txt"),
        ]));
        assert_eq!(changes.len(ChangeList::Unstaged), 3);
        assert_eq!(changes.len(ChangeList::Staged), 2);
        assert_eq!(changes.paths(), 3);

        for status in [
            WorkingTreeStatus::NoWorkingTree,
            WorkingTreeStatus::IndexUnreadable(crate::UnreadableIndex::Sparse),
            WorkingTreeStatus::Listed(Vec::new()),
        ] {
            let changes = LocalChanges::new(status.clone());
            assert_eq!(changes.paths(), 0, "{status:?}");
            assert!(changes.is_empty());
            assert_eq!(changes.status(), &status);
        }
    }

    /// A chosen path is found again in a new status by its list and path, by a search over a
    /// list in its paths' order — bytes that are not UTF-8 included — and not found once it has
    /// left the list. Caught by: a search over git's order (a path missed), or a path found in
    /// the other list.
    #[test]
    fn a_path_is_found_by_its_list_and_name() {
        let changes = LocalChanges::new(WorkingTreeStatus::Listed(vec![
            untracked("z"),
            StatusEntry::Untracked(RepoPath::new(b"bad\xffname".to_vec())),
            changed("b", None, Some(UnstagedChange::Modified)),
            changed("a", Some(StagedChange::Added), None),
            untracked("c"),
        ]));
        for (path, row) in [("b", Some(0)), ("c", Some(2)), ("z", Some(3)), ("a", None)] {
            assert_eq!(
                changes.row_of(ChangeList::Unstaged, &RepoPath::from(path)),
                row,
                "{path}"
            );
        }
        assert_eq!(
            changes.row_of(
                ChangeList::Unstaged,
                &RepoPath::new(b"bad\xffname".to_vec())
            ),
            Some(1)
        );
        assert_eq!(
            changes.row_of(ChangeList::Staged, &RepoPath::from("a")),
            Some(0)
        );
    }

    /// The filter's rule: the text in a path or a source, case ignored, each list's rows by
    /// their place in it; and a pass abandoned when told to stop. Caught by: rows counted
    /// across both lists, a source not searched, or a superseded pass answering.
    #[test]
    fn a_filter_leaves_each_lists_rows_that_hold_its_text() {
        let changes = LocalChanges::new(WorkingTreeStatus::Listed(vec![
            changed(
                "src/Lib.rs",
                Some(StagedChange::Modified),
                Some(UnstagedChange::Modified),
            ),
            changed(
                "docs/new.md",
                Some(StagedChange::Renamed {
                    from: RepoPath::from("src/old.md"),
                    similarity: Similarity::from_percent(80),
                }),
                None,
            ),
            untracked("README"),
            untracked("src/main.rs"),
        ]));
        let matched = changes
            .matching("SRC", || true)
            .unwrap_or_else(|| unreachable!("never told to stop"));
        // Unstaged: README, src/Lib.rs, src/main.rs; Staged: docs/new.md, src/Lib.rs.
        assert_eq!(matched.of(ChangeList::Unstaged), [1, 2]);
        assert_eq!(matched.of(ChangeList::Staged), [0, 1]);
        // src/Lib.rs in both lists, src/main.rs and docs/new.md (by its source) once each.
        assert_eq!(matched.paths, 3);
        assert_eq!(changes.matching("src", || false), None);
    }

    /// The user's decision (2026-10-07): what a filter leaves is counted in distinct paths, as
    /// the status is — a path left in both lists once, and a staged deletion and an untracked
    /// file of one name once — and a path left in one list alone once. Caught by: counting the
    /// rows left (4 for "gone" and "both" alike), or counting only one list's rows.
    #[test]
    fn what_a_filter_leaves_is_counted_in_distinct_paths() {
        let changes = LocalChanges::new(WorkingTreeStatus::Listed(vec![
            changed("gone.txt", Some(StagedChange::Deleted), None),
            changed(
                "both.txt",
                Some(StagedChange::Modified),
                Some(UnstagedChange::Modified),
            ),
            untracked("gone.txt"),
            untracked("new.txt"),
        ]));
        let paths = |text: &str| {
            changes
                .matching(text, || true)
                .unwrap_or_else(|| unreachable!("never told to stop"))
                .paths
        };
        assert_eq!(paths("both"), 1);
        assert_eq!(paths("gone"), 1);
        assert_eq!(paths("txt"), 3);
        assert_eq!(paths("new"), 1);
        assert_eq!(paths("nothing"), 0);
        assert_eq!(paths("o"), 2);
    }

    /// The user's decision (2026-10-07): each list is in Fork's natural order — case ignored,
    /// numbers read as numbers — and a path is found by a search in that same order, two paths
    /// that read alike as text (bytes that are not UTF-8) told apart by their bytes. Caught by:
    /// a bytewise sort (`B10` before `a`, `b10` before `b2`), or a search in another order than
    /// the sort's (a path listed and not found).
    #[test]
    fn each_list_is_in_natural_order_and_found_in_it() {
        let paths: Vec<Vec<u8>> = vec![
            b"b10.rs".to_vec(),
            b"B2.rs".to_vec(),
            b"a.rs".to_vec(),
            b"Z/one.rs".to_vec(),
            b"x\xff".to_vec(),
            b"x\xfe".to_vec(),
            b"m 2.rs".to_vec(),
        ];
        let changes = LocalChanges::new(WorkingTreeStatus::Listed(
            paths
                .iter()
                .map(|path| StatusEntry::Untracked(RepoPath::new(path.clone())))
                .collect(),
        ));
        let order: Vec<Vec<u8>> = (0..changes.len(ChangeList::Unstaged))
            .filter_map(|row| changes.get(ChangeList::Unstaged, row))
            .map(|change| change.path.as_bytes().to_vec())
            .collect();
        assert_eq!(
            order,
            [
                b"a.rs".to_vec(),
                b"B2.rs".to_vec(),
                b"b10.rs".to_vec(),
                b"m 2.rs".to_vec(),
                b"x\xfe".to_vec(),
                b"x\xff".to_vec(),
                b"Z/one.rs".to_vec(),
            ]
        );
        for (row, path) in order.iter().enumerate() {
            assert_eq!(
                changes.row_of(ChangeList::Unstaged, &RepoPath::new(path.clone())),
                Some(row),
                "{:?}",
                String::from_utf8_lossy(path)
            );
        }
        assert_eq!(
            changes.row_of(ChangeList::Unstaged, &RepoPath::from("b2.rs")),
            None
        );
        assert_eq!(changes.paths(), 7);
    }

    /// Fifty thousand paths are laid out in their order, each list a row per path. Caught by:
    /// a list cut short or a sort that is not by bytes.
    #[test]
    fn fifty_thousand_paths_are_laid_out_in_order() {
        let entries: Vec<StatusEntry> = (0..50_000)
            .rev()
            .map(|n| untracked(&format!("dir/{n:05}.txt")))
            .collect();
        let changes = LocalChanges::new(WorkingTreeStatus::Listed(entries));
        assert_eq!(changes.len(ChangeList::Unstaged), 50_000);
        assert_eq!(changes.paths(), 50_000);
        for row in [0, 1, 24_999, 49_999] {
            let change = changes
                .get(ChangeList::Unstaged, row)
                .unwrap_or_else(|| panic!("row {row}"));
            assert_eq!(change.path.display(), format!("dir/{row:05}.txt"));
        }
    }
}
