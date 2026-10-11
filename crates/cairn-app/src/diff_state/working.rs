//! The diff of Local Changes' primary (refs-and-status R9.3, R9.4; staging-and-commit R8.1),
//! kept apart from the commit's file so going back to All Commits finds the commit's as it was —
//! but in the diff selection, since the three share the file-diff lane: whichever asks takes
//! it (`DiffState::take_lane`), and the others are asked again when their view is shown.
//!
//! **Derived, never chosen on its own** (rule 8): which path this is, and in which list, is the
//! selection's primary (`LocalChangesState::selection`), and [`WorkingChoice`] is only the key of
//! what was asked for it — written by `diff_actions::show_primary` alone, as the selection or
//! the lists move. A path is asked as its list says: its staged diff from Staged, its unstaged
//! or untracked diff from Unstaged. A conflicted path asks nothing — its notice stands in place
//! of a diff (R9.4). An answer is kept only when it names exactly the query the path is asked
//! as now: its path, its side and its options, so a path in both lists can never draw the
//! other list's diff, and a primary before a refresh can never draw under the primary after it.
//!
//! When other lists are drawn and the primary is still listed, it is asked again — the file may
//! have changed under the same status — and its last diff stays drawn until the new one comes;
//! gone, the selection has moved to another path by its rule, so no diff is drawn under no row.
//! What a line selection over it belongs to is the content of the diff drawn
//! ([`ShownDiff::content`], keyed on the diff thread), never a count of answers: a re-read whose
//! text is the same keeps the selection, and one whose text differs leaves it nothing (R9.6).

use cairn_model::{ChangeList, RepoPath, ShownDiff};

use super::{Answer, DiffState, LaneHolder, retire};
use crate::worker::{DiffOptions, FileQuery, Request};

/// What the diff was asked for: the primary's list and name, and the lists it was asked in or
/// last asked again in — a key derived from the selection, not a choice of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingChoice {
    pub list: ChangeList,
    pub path: RepoPath,
    /// `LocalChangesState::serial` of the lists it was chosen in or last found in.
    pub lists: u64,
}

/// The path chosen and its diff as the view draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Working {
    choice: WorkingChoice,
    /// What its diff is asked as; `None` for a conflicted path, which asks nothing.
    query: Option<FileQuery>,
    /// `None` inside `Ready` where git prints nothing for the path.
    answer: Answer<Option<ShownDiff>>,
    /// The diff drawn for the same path and side before other lists asked it again: drawn
    /// while the new one is on its way, so a refresh does not blank the view.
    previous: Option<ShownDiff>,
}

impl Working {
    fn diffs(self) -> Vec<ShownDiff> {
        let mut diffs: Vec<ShownDiff> = self.previous.into_iter().collect();
        if let Answer::Ready(Some(shown)) = self.answer {
            diffs.push(shown);
        }
        diffs
    }
}

/// What a path chosen in Local Changes is asked as, for the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkingShown<'a> {
    /// A conflicted path: its notice, and no diff (R9.4).
    Conflicted,
    Waiting,
    Failed(&'a str),
    /// Git prints nothing for the path.
    Nothing,
    Diff(&'a ShownDiff),
}

impl DiffState {
    /// `choice` becomes the path chosen, asked as `query` — or, for a conflicted path, nothing.
    /// It takes the file-diff lane from the commit's file and the files opened in place. Returns
    /// what to submit: the query, then what it replaces, for a worker to free.
    pub fn choose_working(
        &mut self,
        choice: WorkingChoice,
        query: Option<FileQuery>,
    ) -> Vec<Request> {
        let replaced = self.working.replace(Working {
            choice,
            query: query.clone(),
            answer: Answer::Waiting,
            previous: None,
        });
        let mut requests = Vec::new();
        match query {
            Some(query) => {
                self.take_lane(LaneHolder::Working);
                requests.push(Request::FileDiff(query));
            }
            None => self.let_go_of_lane(LaneHolder::Working),
        }
        requests.extend(replaced.and_then(|working| retire(working.diffs())));
        requests
    }

    /// The path chosen was found again in other lists (`lists`), to be asked as `query`: asked
    /// again, the diff drawn for it kept drawn until the new one arrives when it is the same
    /// path and side at the same options. Returns what to submit.
    pub fn refresh_working(&mut self, lists: u64, query: Option<FileQuery>) -> Vec<Request> {
        let Some(working) = self.working.take() else {
            return Vec::new();
        };
        let choice = WorkingChoice {
            lists,
            ..working.choice.clone()
        };
        let same = working.query == query;
        let Working {
            answer, previous, ..
        } = working;
        let mut freed: Vec<ShownDiff> = Vec::new();
        let previous = match (same, answer, previous) {
            (true, Answer::Ready(Some(shown)), previous) => {
                freed.extend(previous);
                Some(shown)
            }
            (true, Answer::Waiting, Some(previous)) => Some(previous),
            (_, answer, previous) => {
                freed.extend(previous);
                if let Answer::Ready(Some(shown)) = answer {
                    freed.push(shown);
                }
                None
            }
        };
        let mut requests = self.choose_working(choice, query);
        if let Some(working) = &mut self.working {
            working.previous = previous;
        }
        requests.extend(retire(freed));
        requests
    }

    /// No path chosen — the lists are empty, or the filter leaves none. Returns what was drawn,
    /// for a worker to free.
    pub fn let_go_of_working(&mut self) -> Vec<Request> {
        self.let_go_of_lane(LaneHolder::Working);
        self.working
            .take()
            .and_then(|working| retire(working.diffs()))
            .into_iter()
            .collect()
    }

    pub fn working_choice(&self) -> Option<&WorkingChoice> {
        self.working.as_ref().map(|working| &working.choice)
    }

    pub fn working_query(&self) -> Option<&FileQuery> {
        self.working
            .as_ref()
            .and_then(|working| working.query.as_ref())
    }

    /// What the view draws for the path chosen, if one is.
    pub fn working_shown(&self) -> Option<WorkingShown<'_>> {
        let working = self.working.as_ref()?;
        if working.query.is_none() {
            return Some(WorkingShown::Conflicted);
        }
        Some(match &working.answer {
            Answer::Ready(Some(shown)) => WorkingShown::Diff(shown),
            Answer::Ready(None) => WorkingShown::Nothing,
            Answer::Failed(message) => WorkingShown::Failed(message),
            Answer::Waiting => match &working.previous {
                Some(previous) => WorkingShown::Diff(previous),
                None => WorkingShown::Waiting,
            },
        })
    }

    /// The diff drawn for the path chosen, when there is one.
    pub fn shown_working(&self) -> Option<&ShownDiff> {
        match self.working_shown()? {
            WorkingShown::Diff(shown) => Some(shown),
            WorkingShown::Conflicted
            | WorkingShown::Waiting
            | WorkingShown::Failed(_)
            | WorkingShown::Nothing => None,
        }
    }

    /// Whether the path chosen is awaited and its request lost the lane: what the view checks,
    /// reading only, before it asks again as it is shown.
    pub fn working_needs_asking(&self) -> bool {
        !self.holds_lane(LaneHolder::Working)
            && self.working.as_ref().is_some_and(|working| {
                working.query.is_some() && matches!(working.answer, Answer::Waiting)
            })
    }

    /// The path chosen asked again when its answer is awaited and its request lost the lane.
    pub fn reask_working(&mut self) -> Option<Request> {
        if !self.working_needs_asking() {
            return None;
        }
        let query = self.working_query()?.clone();
        self.take_lane(LaneHolder::Working);
        Some(Request::FileDiff(query))
    }

    /// Whether an answer to `query` is the one the path chosen is asked as now.
    pub(super) fn wants_working(&self, query: &FileQuery) -> bool {
        self.working_query() == Some(query)
    }

    /// A working-tree diff arrived: kept only if it answers exactly the query the path chosen is
    /// asked as now. Returns whether it was, and what it replaces for a worker to free — the
    /// answer itself when it is not kept.
    pub fn working_arrived(
        &mut self,
        query: &FileQuery,
        diff: Option<ShownDiff>,
    ) -> (bool, Option<Request>) {
        let Some(working) = self
            .working
            .as_mut()
            .filter(|working| working.query.as_ref() == Some(query))
        else {
            return (false, retire(diff.into_iter().collect()));
        };
        let mut freed: Vec<ShownDiff> = working.previous.take().into_iter().collect();
        if let Answer::Ready(Some(shown)) =
            std::mem::replace(&mut working.answer, Answer::Ready(diff))
        {
            freed.push(shown);
        }
        (true, retire(freed))
    }

    /// The working-tree query for the path chosen failed: said in place of its diff.
    pub(super) fn working_failed(&mut self, message: String) {
        if let Some(working) = &mut self.working {
            working.answer = Answer::Failed(message);
        }
    }

    /// The path chosen's options moved to `options` (its own `load_anyway` kept): asked again,
    /// now when `ask` — its view is shown — else when it is. Returns the request, if asked, and
    /// what was drawn, for a worker to free.
    pub(super) fn working_settings_changed(
        &mut self,
        options: DiffOptions,
        ask: bool,
    ) -> (Option<Request>, Vec<ShownDiff>) {
        let Some(working) = &mut self.working else {
            return (None, Vec::new());
        };
        let Some(query) = &mut working.query else {
            return (None, Vec::new());
        };
        let wanted = DiffOptions {
            load_anyway: query.options.load_anyway,
            ..options
        };
        if query.options == wanted {
            return (None, Vec::new());
        }
        query.options = wanted;
        let mut freed: Vec<ShownDiff> = working.previous.take().into_iter().collect();
        if let Answer::Ready(Some(shown)) = std::mem::replace(&mut working.answer, Answer::Waiting)
        {
            freed.push(shown);
        }
        self.let_go_of_lane(LaneHolder::Working);
        let asked = if ask { self.reask_working() } else { None };
        (asked, freed)
    }

    /// Load Diff for the path chosen (R6.8): asked again past R2.6's limits. Nothing when no
    /// path is chosen or it was loaded already.
    pub fn load_working_anyway(&mut self) -> Vec<Request> {
        let Some(working) = &self.working else {
            return Vec::new();
        };
        let Some(mut query) = working.query.clone() else {
            return Vec::new();
        };
        if query.options.load_anyway {
            return Vec::new();
        }
        query.options.load_anyway = true;
        let choice = working.choice.clone();
        self.choose_working(choice, Some(query))
    }
}

#[cfg(test)]
mod tests {
    use cairn_model::{
        ChangeStatus, ChangedFile, ChangedRange, Context, DiffContent, DiffLine, DisplayOverlay,
        FileDiff, LineSpan, Oid, TextDiff,
    };

    use super::*;
    use crate::diff_state::Asking;
    use crate::worker::{Comparison, FileTarget, WorkingSide};

    fn query(path: &str, side: WorkingSide) -> FileQuery {
        FileQuery {
            target: FileTarget::WorkingTree {
                path: RepoPath::from(path),
                side,
            },
            options: DiffOptions::default(),
        }
    }

    fn choice(list: ChangeList, path: &str, lists: u64) -> WorkingChoice {
        WorkingChoice {
            list,
            path: RepoPath::from(path),
            lists,
        }
    }

    /// A diff of `path` whose one added line reads `line`.
    fn shown(path: &str, line: &str) -> ShownDiff {
        ShownDiff::new(
            FileDiff {
                file: ChangedFile {
                    status: ChangeStatus::Modified,
                    old_path: RepoPath::from(path),
                    new_path: RepoPath::from(path),
                    old_mode: None,
                    new_mode: None,
                    old_id: None,
                    new_id: None,
                },
                content: DiffContent::Text {
                    text: TextDiff::new(
                        Vec::new(),
                        vec![DiffLine::terminated(line.as_bytes().to_vec())],
                        vec![ChangedRange::new(LineSpan::at(0, 0), LineSpan::at(0, 1))],
                    ),
                    overlay: DisplayOverlay::default(),
                },
            },
            Context::default(),
        )
    }

    fn drawn_line(state: &DiffState) -> Option<Vec<u8>> {
        state
            .shown_working()
            .and_then(|shown| shown.diff().text().map(|text| text.new_content()))
    }

    fn retired(requests: &[Request]) -> usize {
        requests
            .iter()
            .map(|request| match request {
                Request::Retire(retired) => retired.shown().len(),
                _ => 0,
            })
            .sum()
    }

    /// The QA brief: a path both staged and unstaged is chosen in each list as its own query,
    /// so an answer for one list's side is never kept for the other — whichever arrives late.
    /// Caught by: answers compared by path alone (the staged diff drawn under the Unstaged
    /// row), or a late answer for the side chosen before kept.
    #[test]
    fn a_path_in_both_lists_keeps_only_the_answer_for_the_list_chosen() {
        let mut state = DiffState::default();
        let unstaged = query("both.rs", WorkingSide::Unstaged);
        let staged = query("both.rs", WorkingSide::Staged);
        let asked = state.choose_working(
            choice(ChangeList::Unstaged, "both.rs", 1),
            Some(unstaged.clone()),
        );
        assert_eq!(asked, [Request::FileDiff(unstaged.clone())]);
        assert!(!state.wants_file(&staged));
        let (kept, freed) = state.working_arrived(&staged, Some(shown("both.rs", "STAGED")));
        assert!(!kept);
        assert!(matches!(freed, Some(Request::Retire(_))), "{freed:?}");
        assert_eq!(state.working_shown(), Some(WorkingShown::Waiting));
        let (kept, _) = state.working_arrived(&unstaged, Some(shown("both.rs", "UNSTAGED")));
        assert!(kept);
        assert_eq!(drawn_line(&state), Some(b"UNSTAGED\n".to_vec()));

        let asked = state.choose_working(
            choice(ChangeList::Staged, "both.rs", 1),
            Some(staged.clone()),
        );
        assert_eq!(asked.first(), Some(&Request::FileDiff(staged.clone())));
        assert_eq!(
            retired(&asked),
            1,
            "the unstaged diff was not freed on a worker"
        );
        let (kept, _) = state.working_arrived(&unstaged, Some(shown("both.rs", "UNSTAGED")));
        assert!(!kept, "a late answer for the other list was kept");
        assert_eq!(state.working_shown(), Some(WorkingShown::Waiting));
        let (kept, _) = state.working_arrived(&staged, Some(shown("both.rs", "STAGED")));
        assert!(kept);
        assert_eq!(drawn_line(&state), Some(b"STAGED\n".to_vec()));
    }

    /// A path found again in other lists is asked again, its diff drawn until the new one
    /// comes; asked as another side, or let go, nothing of before is drawn. Caught by: a refresh
    /// that blanks the view (previous not kept), one that keeps drawing the old side's diff
    /// under a new side, or a diff left drawn after its path was let go.
    #[test]
    fn a_refresh_asks_the_path_again_and_draws_its_last_diff_meanwhile() {
        let mut state = DiffState::default();
        let unstaged = query("a.rs", WorkingSide::Unstaged);
        state.choose_working(
            choice(ChangeList::Unstaged, "a.rs", 1),
            Some(unstaged.clone()),
        );
        state.working_arrived(&unstaged, Some(shown("a.rs", "OLD")));

        let asked = state.refresh_working(2, Some(unstaged.clone()));
        assert_eq!(asked, [Request::FileDiff(unstaged.clone())]);
        assert_eq!(state.working_choice().map(|choice| choice.lists), Some(2));
        assert_eq!(
            drawn_line(&state),
            Some(b"OLD\n".to_vec()),
            "the view blanked"
        );
        let (kept, freed) = state.working_arrived(&unstaged, Some(shown("a.rs", "NEW")));
        assert!(kept);
        assert!(
            matches!(&freed, Some(Request::Retire(r)) if r.shown().len() == 1),
            "{freed:?}"
        );
        assert_eq!(drawn_line(&state), Some(b"NEW\n".to_vec()));

        // Listed again as untracked (its staged deletion's twin): another side, nothing of the
        // old one drawn.
        let untracked = query("a.rs", WorkingSide::Untracked);
        let asked = state.refresh_working(3, Some(untracked.clone()));
        assert_eq!(asked.first(), Some(&Request::FileDiff(untracked)));
        assert_eq!(retired(&asked), 1);
        assert_eq!(state.working_shown(), Some(WorkingShown::Waiting));

        let freed = state.let_go_of_working();
        assert!(freed.is_empty(), "{freed:?}");
        assert_eq!(state.working_shown(), None);
    }

    /// R9.4: a conflicted path asks nothing and draws its notice; and it gives up the lane, so
    /// nothing of its is asked again. Caught by: a query asked for a conflict, or a conflict
    /// drawn as reading for good.
    #[test]
    fn a_conflicted_path_asks_nothing_and_draws_its_notice() {
        let mut state = DiffState::default();
        let asked = state.choose_working(choice(ChangeList::Unstaged, "clash.rs", 1), None);
        assert!(asked.is_empty(), "{asked:?}");
        assert_eq!(state.working_shown(), Some(WorkingShown::Conflicted));
        assert!(!state.working_needs_asking());
        assert_eq!(state.reask_working(), None);
    }

    /// The three share the file-diff lane: the path chosen takes it from the commit's file and
    /// the files opened in place, which are asked again when their tab is shown, and a commit,
    /// a file or the files opened in place take it back, the path asked again when Local
    /// Changes is shown. Caught by: an answer awaited for good on either side.
    #[test]
    fn the_working_path_and_the_commits_file_take_the_lane_from_each_other() {
        let mut state = DiffState::default();
        let of = Comparison::Commit(Oid::from_bytes(&[1; 20]).unwrap_or_else(|_| unreachable!()));
        state.select_changes(of);
        let file = FileQuery {
            target: FileTarget::Committed {
                of,
                file: ChangedFile {
                    status: ChangeStatus::Added,
                    old_path: RepoPath::from("c.rs"),
                    new_path: RepoPath::from("c.rs"),
                    old_mode: None,
                    new_mode: None,
                    old_id: None,
                    new_id: None,
                },
            },
            options: DiffOptions::default(),
        };
        state.select_file(file.clone());
        let unstaged = query("a.rs", WorkingSide::Unstaged);
        state.choose_working(
            choice(ChangeList::Unstaged, "a.rs", 1),
            Some(unstaged.clone()),
        );
        assert!(
            state.file_needs_asking(),
            "the commit's file kept the lane it lost"
        );
        assert!(!state.working_needs_asking());

        assert_eq!(state.reask_file(), Some(Request::FileDiff(file)));
        assert!(
            state.working_needs_asking(),
            "the path kept the lane it lost"
        );
        assert_eq!(
            state.reask_working(),
            Some(Request::FileDiff(unstaged.clone()))
        );

        // A changes query supersedes the lane, whoever holds it.
        state.select_changes(of);
        assert!(state.working_needs_asking());
    }

    /// The commit's file `c.rs` of `of`, as the Changes tab asks it.
    fn commits_file(of: Comparison) -> FileQuery {
        FileQuery {
            target: FileTarget::Committed {
                of,
                file: ChangedFile {
                    status: ChangeStatus::Added,
                    old_path: RepoPath::from("c.rs"),
                    new_path: RepoPath::from("c.rs"),
                    old_mode: None,
                    new_mode: None,
                    old_id: None,
                    new_id: None,
                },
            },
            options: DiffOptions::default(),
        }
    }

    /// H1, phase 17's QA item 2: only the lane's holder lets go of it. The commit's file in
    /// flight keeps the lane when Local Changes' path is let go of — the lists emptied — or
    /// chosen as a conflict, which asks nothing; and the path in flight keeps it when the
    /// commit's file's options move while the Commit tab is shown. Caught by: a let-go that
    /// clears whoever holds the lane, so a holder still reading is asked again and supersedes
    /// its own read.
    #[test]
    fn only_the_lanes_holder_lets_go_of_it() {
        let of = Comparison::Commit(Oid::from_bytes(&[1; 20]).unwrap_or_else(|_| unreachable!()));
        let mut state = DiffState::default();
        state.select_changes(of);
        let file = commits_file(of);
        state.select_file(file.clone());
        assert!(state.let_go_of_working().is_empty());
        assert!(
            state
                .choose_working(choice(ChangeList::Unstaged, "clash.rs", 1), None)
                .is_empty()
        );
        assert!(!state.file_needs_asking(), "the file lost a lane it held");
        assert_eq!(
            state.reask_file(),
            None,
            "the file was asked again over its own read"
        );

        let unstaged = query("a.rs", WorkingSide::Unstaged);
        state.choose_working(
            choice(ChangeList::Unstaged, "a.rs", 2),
            Some(unstaged.clone()),
        );
        assert!(
            state.file_needs_asking(),
            "the path took no lane from the file"
        );
        let wider = DiffOptions {
            context: Context::Lines(9),
            ..DiffOptions::default()
        };
        // The file's options move, the path's do not; the Commit tab is shown.
        let requests =
            state.settings_changed(wider, wider, DiffOptions::default(), Asking::Expansion);
        assert!(
            !requests.iter().any(|r| matches!(r, Request::FileDiff(_))),
            "{requests:?}"
        );
        assert!(
            !state.working_needs_asking(),
            "the path lost a lane it held"
        );
        assert_eq!(state.reask_working(), None);
    }

    /// A setting moved while Local Changes is shown asks its path again at once, at the new
    /// options, and the commit's file when its tab is shown; moved elsewhere, the path is asked
    /// when Local Changes is shown. Caught by: the path never asked again (a diff at the old
    /// context drawn), or asked while hidden (taking the lane from the view shown).
    #[test]
    fn a_setting_asks_the_working_path_when_local_changes_is_shown() {
        let mut state = DiffState::default();
        let unstaged = query("a.rs", WorkingSide::Unstaged);
        state.choose_working(
            choice(ChangeList::Unstaged, "a.rs", 1),
            Some(unstaged.clone()),
        );
        state.working_arrived(&unstaged, Some(shown("a.rs", "x")));
        let wider = DiffOptions {
            context: Context::Lines(9),
            ..DiffOptions::default()
        };
        let requests = state.settings_changed(wider, wider, wider, Asking::Working);
        match requests.first() {
            Some(Request::FileDiff(asked)) => assert_eq!(asked.options, wider),
            other => panic!("the path was not asked again: {other:?}"),
        }
        assert_eq!(retired(&requests), 1);

        let wider_still = DiffOptions {
            context: Context::Lines(12),
            ..DiffOptions::default()
        };
        let requests = state.settings_changed(wider_still, wider_still, wider_still, Asking::File);
        assert!(
            !requests.iter().any(|r| matches!(r, Request::FileDiff(_))),
            "{requests:?}"
        );
        assert!(state.working_needs_asking());
        match state.reask_working() {
            Some(Request::FileDiff(asked)) => assert_eq!(asked.options, wider_still),
            other => panic!("{other:?}"),
        }
    }
}
