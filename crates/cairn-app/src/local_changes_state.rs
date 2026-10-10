//! Local Changes' lists as the window keeps them (refs-and-status R9): the status they are laid
//! out over, the rows the view's filter leaves of each, and the session's handles for the view —
//! its filter's text, its list's width, its diff's scroll and the change last moved to.
//!
//! **Which lists are drawn.** A status arrives laid out already (`cairn_model::LocalChanges`,
//! built on the refresh thread). With no filter it is drawn at once. With one, its rows are
//! asked of a worker (`Request::FilterLocalChanges`, numbered in a lane of its own) and the lists
//! drawn before it stay drawn, with their own rows, until the answer comes — so a row is never
//! read from one status by an index into another, and a refresh never empties the lists while a
//! filter is typed. A pass over every path is never run here.
//!
//! **What is freed where.** Every status the window lets go of is handed to a worker
//! (`Request::Retire`): one of tens of thousands of paths costs more than a frame to free. The
//! lists drawn are numbered ([`LocalChangesState::serial`]), so the primary's diff is asked again
//! whenever they are replaced (`diff_actions::show_primary`).
//!
//! **The selection is the state's, and the state's alone** (staging-and-commit R8.1, rule 8):
//! one [`LocalSelection`], or none, moved by the one rule of `next_selection` whenever other
//! lists are drawn or the filter's rows change — as the status and the filter's answers arrive,
//! in `session::apply` — and after an action, with the paths it took leaving
//! ([`LocalChangesState::acted`]). While the filter's first rows are on their way it is left as
//! it was; nothing is acted on meanwhile, since an action takes only the rows shown.

use std::sync::{Arc, LazyLock};

use cairn_model::{ChangeList, LocalChanges, MatchedRows, WorkingTreeStatus};
use cairn_ui::{ChangeCursor, LocalSelection, ShownFiles};
use freya::prelude::*;

use crate::next_selection::{Leaving, Next, Rows, next_selection};
use crate::worker::{Request, Retired};

/// The list side's share of the view until its splitter is dragged, in percent: the Changes
/// tab's.
pub const LIST_WIDTH: f32 = crate::changes_tab::LIST_WIDTH;

/// Local Changes' handles, each a session's.
#[derive(Clone, Copy, PartialEq)]
pub struct LocalChangesView {
    pub state: State<LocalChangesState>,
    pub filter_text: State<String>,
    pub list_width: State<f32>,
    /// Unstaged's share of the two lists' height, as the splitter between them was last
    /// dragged: the session's, never kept beyond it (the user's decision, 2026-10-07).
    pub lists_split: State<f32>,
    /// The diff's scroll: the view's own, so the Changes tab's is where it was when it is
    /// shown again.
    pub scroll: ScrollController,
    /// The change previous or next change last moved to in the view's diff.
    pub cursor: State<Option<ChangeCursor>>,
    /// What the view's actions are waiting on and what it says of them (R8.4, R8.6).
    pub acting: State<crate::local_changes_actions::Acting>,
    /// A drag across the diff's lines, and the lines it selected (staging-and-commit R9.2):
    /// written by the diff view, read by the chords heard on the diff.
    pub lines: State<cairn_ui::LineDrag>,
    /// The commit box under the diff: the draft, kept for the window's life (R10.7), and the
    /// rest of its state.
    pub commit: crate::commit_box_state::CommitBoxView,
}

impl LocalChangesView {
    /// Each handle made by its hook, in the window's root.
    pub fn used() -> Self {
        Self {
            state: use_state(LocalChangesState::default),
            filter_text: use_state(String::new),
            list_width: use_state(|| LIST_WIDTH),
            lists_split: use_state(|| cairn_ui::LISTS_SPLIT),
            scroll: use_scroll_controller(ScrollConfig::default),
            cursor: use_state(|| None),
            acting: use_state(crate::local_changes_actions::Acting::default),
            lines: use_state(cairn_ui::LineDrag::default),
            commit: crate::commit_box_state::CommitBoxView::used(),
        }
    }

    /// Each handle made outside a component, for a test's view.
    #[cfg(test)]
    pub fn created() -> Self {
        Self {
            state: State::create(LocalChangesState::default()),
            filter_text: State::create(String::new()),
            list_width: State::create(LIST_WIDTH),
            lists_split: State::create(cairn_ui::LISTS_SPLIT),
            scroll: ScrollController::new(0, 0, Vec::new()),
            cursor: State::create(None),
            acting: State::create(crate::local_changes_actions::Acting::default()),
            lines: State::create(cairn_ui::LineDrag::default()),
            commit: crate::commit_box_state::CommitBoxView::created(),
        }
    }
}

/// The lists drawn: a status's, and the rows of each the filter left, or all of them, with the
/// distinct paths those rows show as the filter's pass counted them (`None` while every row is
/// shown or the rows are on their way).
#[derive(Debug)]
struct Drawn {
    changes: Arc<LocalChanges>,
    shown: [ShownFiles; 2],
    shown_paths: Option<usize>,
}

impl Drawn {
    fn rows(&self) -> Rows<'_> {
        Rows {
            changes: &self.changes,
            shown: [&self.shown[0], &self.shown[1]],
        }
    }
}

#[derive(Debug, Default)]
pub struct LocalChangesState {
    drawn: Option<Drawn>,
    /// A status that arrived while a filter was on, waiting for its rows before it is drawn.
    pending: Option<Arc<LocalChanges>>,
    /// The filter's text as the view has it.
    text: String,
    /// The text the rows drawn were filtered by; empty when they are every row.
    answered: String,
    /// Moves each time other lists are drawn.
    serial: u64,
    /// What is selected in the lists drawn (R8.1): `None` only when nothing is.
    selection: Option<LocalSelection>,
}

/// What [`drawn_changes`] hands back while nothing is drawn.
static NO_CHANGES: LazyLock<LocalChanges> =
    LazyLock::new(|| LocalChanges::new(WorkingTreeStatus::Listed(Vec::new())));

/// What [`shown_rows`] hands back while nothing is drawn.
static NO_ROWS: ShownFiles = ShownFiles::Waiting;

/// The lists drawn, or empty ones while none are: the view of the state the list is handed,
/// which reads its rows by index.
pub fn drawn_changes(state: &LocalChangesState) -> &LocalChanges {
    state
        .drawn
        .as_ref()
        .map_or(&*NO_CHANGES, |drawn| drawn.changes.as_ref())
}

/// How many distinct paths the filter's rows drawn show, as the filter's pass counted them
/// (`MatchedRows::paths`); `None` while every row is drawn or none is yet.
pub fn shown_paths(state: &LocalChangesState) -> Option<usize> {
    state.drawn.as_ref().and_then(|drawn| drawn.shown_paths)
}

/// The selection, as the list reads it to draw its rows.
pub fn selection_of(state: &LocalChangesState) -> &Option<LocalSelection> {
    &state.selection
}

/// The rows of `list` the filter leaves, as the list reads them.
pub fn shown_rows(state: &LocalChangesState, list: ChangeList) -> &ShownFiles {
    state.drawn.as_ref().map_or(&NO_ROWS, |drawn| match list {
        ChangeList::Unstaged => &drawn.shown[0],
        ChangeList::Staged => &drawn.shown[1],
    })
}

fn retire(changes: Arc<LocalChanges>) -> Request {
    Request::Retire(Retired::status(changes))
}

impl LocalChangesState {
    /// Whether any lists are drawn.
    pub fn has_lists(&self) -> bool {
        self.drawn.is_some()
    }

    /// What is selected, if anything is.
    pub fn selection(&self) -> Option<&LocalSelection> {
        self.selection.as_ref()
    }

    /// `selection` is selected: a press in the lists.
    pub fn set_selection(&mut self, selection: Option<LocalSelection>) {
        self.selection = selection;
    }

    /// The lists drawn and the rows shown of each, as the rule reads them.
    pub fn rows(&self) -> Option<Rows<'_>> {
        self.drawn.as_ref().map(Drawn::rows)
    }

    /// An action took `leaving` out of the selection's list (R8.3, L5): the selection moved by
    /// the rule at once, over the lists drawn — the next file's diff asked before the status
    /// that shows the write arrives, whose turn of the rule then changes nothing.
    pub fn acted(&mut self, leaving: &Leaving) {
        if let Some(rows) = self.rows()
            && let Next::To(next) = next_selection(self.selection.as_ref(), rows, rows, leaving)
        {
            self.selection = next;
        }
    }

    /// The first row shown, selected when nothing is (R9.3: as the view opens).
    pub fn select_first_if_none(&mut self) {
        if self.selection.is_some() {
            return;
        }
        let Some(rows) = self.rows() else {
            return;
        };
        let empty = Rows {
            changes: &NO_CHANGES,
            shown: [&ShownFiles::All, &ShownFiles::All],
        };
        if let Next::To(first) = next_selection(None, empty, rows, &Leaving::none()) {
            self.selection = first;
        }
    }

    /// The selection moved by the rule from the lists drawn to `after` (none leaving) — unless
    /// `after`'s rows are on their way, when it is left as it is; one the rule keeps whole is
    /// neither copied nor written.
    fn follow(&mut self, after: Rows<'_>) {
        if *after.shown[0] == ShownFiles::Waiting {
            return;
        }
        let empty = Rows {
            changes: &NO_CHANGES,
            shown: [&ShownFiles::All, &ShownFiles::All],
        };
        let before = self.rows().unwrap_or(empty);
        if let Next::To(next) =
            next_selection(self.selection.as_ref(), before, after, &Leaving::none())
        {
            self.selection = next;
        }
    }

    /// The lists drawn, shared: what Stage All and Unstage All hand the local lane to gather
    /// their paths from, so the UI thread never walks a list.
    pub fn drawn_shared(&self) -> Option<Arc<LocalChanges>> {
        self.drawn.as_ref().map(|drawn| Arc::clone(&drawn.changes))
    }

    /// The lists drawn, shared.
    #[cfg(test)]
    pub fn drawn(&self) -> Option<&Arc<LocalChanges>> {
        self.drawn.as_ref().map(|drawn| &drawn.changes)
    }

    /// Moves each time other lists are drawn: what the primary's diff was asked against.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether what is drawn is the newest status, filtered by the text as it is.
    #[cfg(test)]
    pub fn is_settled(&self) -> bool {
        self.pending.is_none() && self.drawn.is_some() && self.answered == self.text
    }

    /// The newest lists: the ones waiting for their rows, or else the ones drawn.
    fn newest(&self) -> Option<&Arc<LocalChanges>> {
        self.pending
            .as_ref()
            .or(self.drawn.as_ref().map(|drawn| &drawn.changes))
    }

    /// `changes` drawn with `shown` rows, which show `shown_paths` distinct paths, the selection
    /// moved by the rule; what it replaces handed back to be freed on a worker.
    fn draw(
        &mut self,
        changes: Arc<LocalChanges>,
        shown: [ShownFiles; 2],
        shown_paths: Option<usize>,
    ) -> Vec<Request> {
        self.follow(Rows {
            changes: &changes,
            shown: [&shown[0], &shown[1]],
        });
        let replaced = self.drawn.replace(Drawn {
            changes,
            shown,
            shown_paths,
        });
        self.serial += 1;
        replaced
            .map(|drawn| retire(drawn.changes))
            .into_iter()
            .collect()
    }

    /// A status arrived: drawn at once with no filter on; with one, its rows asked first. Returns
    /// what to submit: the filter's ask, and what is let go of, for a worker to free.
    pub fn status_arrived(&mut self, changes: Arc<LocalChanges>) -> Vec<Request> {
        if self.text.is_empty() {
            let mut requests: Vec<Request> = self.pending.take().map(retire).into_iter().collect();
            self.answered.clear();
            requests.extend(self.draw(changes, [ShownFiles::All, ShownFiles::All], None));
            return requests;
        }
        let asked = Request::FilterLocalChanges {
            changes: Arc::clone(&changes),
            text: self.text.clone(),
        };
        let mut requests = vec![asked];
        requests.extend(self.pending.replace(changes).map(retire));
        if self.drawn.is_none() {
            // Nothing to keep drawing meanwhile: the lists wait for their first rows.
            if let Some(pending) = self.pending.take() {
                requests.extend(self.draw(
                    pending,
                    [ShownFiles::Waiting, ShownFiles::Waiting],
                    None,
                ));
            }
        }
        requests
    }

    /// The filter's text is now `text`. Returns what to submit: the rows asked of a worker for
    /// the newest lists, and what is let go of. An empty text draws every row of the newest
    /// lists at once; the text already asked for asks nothing.
    pub fn filter(&mut self, text: &str) -> Vec<Request> {
        if self.text == text {
            return Vec::new();
        }
        self.text = text.to_owned();
        if self.text.is_empty() {
            self.answered.clear();
            return match self.pending.take() {
                Some(pending) => self.draw(pending, [ShownFiles::All, ShownFiles::All], None),
                None => {
                    if let Some(drawn) = &mut self.drawn {
                        drawn.shown = [ShownFiles::All, ShownFiles::All];
                        drawn.shown_paths = None;
                    }
                    Vec::new()
                }
            };
        }
        let Some(newest) = self.newest().cloned() else {
            return Vec::new();
        };
        // Every row was drawn: until the first answer, the lists say they are filtering
        // rather than counting every row as matched.
        if let Some(drawn) = &mut self.drawn
            && self.answered.is_empty()
        {
            drawn.shown = [ShownFiles::Waiting, ShownFiles::Waiting];
            drawn.shown_paths = None;
        }
        vec![Request::FilterLocalChanges {
            changes: newest,
            text: self.text.clone(),
        }]
    }

    /// A filter's rows arrived for `changes` and `text`: kept only for the newest lists and the
    /// text as it is, drawing those lists if they were waiting. Returns what to submit — what is
    /// let go of, the answer's own hold on its lists among it when it is not kept.
    pub fn filtered(
        &mut self,
        changes: Arc<LocalChanges>,
        text: &str,
        rows: MatchedRows,
    ) -> Vec<Request> {
        let wanted = text == self.text
            && self
                .newest()
                .is_some_and(|newest| Arc::ptr_eq(newest, &changes));
        if !wanted {
            return vec![retire(changes)];
        }
        let MatchedRows {
            unstaged,
            staged,
            paths,
        } = rows;
        let shown = [ShownFiles::Filtered(unstaged), ShownFiles::Filtered(staged)];
        self.answered = self.text.clone();
        let mut requests = Vec::new();
        match self.pending.take() {
            Some(pending) => requests.extend(self.draw(pending, shown, Some(paths))),
            None => {
                if let Some(changes) = self.drawn.as_ref().map(|drawn| Arc::clone(&drawn.changes)) {
                    self.follow(Rows {
                        changes: &changes,
                        shown: [&shown[0], &shown[1]],
                    });
                }
                if let Some(drawn) = &mut self.drawn {
                    drawn.shown = shown;
                    drawn.shown_paths = Some(paths);
                }
            }
        }
        // The answer's hold on the lists is not the last — the state keeps them — so letting it
        // go here frees nothing.
        drop(changes);
        requests
    }
}

#[cfg(test)]
mod tests {
    use cairn_model::{RepoPath, StatusEntry};

    use super::*;

    fn lists(paths: &[&str]) -> Arc<LocalChanges> {
        Arc::new(LocalChanges::new(WorkingTreeStatus::Listed(
            paths
                .iter()
                .map(|path| StatusEntry::Untracked(RepoPath::from(*path)))
                .collect(),
        )))
    }

    fn retired(requests: &[Request]) -> usize {
        requests
            .iter()
            .filter(|request| matches!(request, Request::Retire(_)))
            .count()
    }

    fn asked(requests: &[Request]) -> Option<(&Arc<LocalChanges>, &str)> {
        requests.iter().find_map(|request| match request {
            Request::FilterLocalChanges { changes, text } => Some((changes, text.as_str())),
            _ => None,
        })
    }

    /// With no filter, each status is drawn as it arrives, the one it replaces freed on a
    /// worker, and the serial moves. Caught by: a replaced status dropped on the UI thread, or
    /// the serial left (the primary's diff never asked again).
    #[test]
    fn with_no_filter_a_status_is_drawn_as_it_arrives() {
        let mut state = LocalChangesState::default();
        let first = lists(&["a"]);
        assert!(state.status_arrived(Arc::clone(&first)).is_empty());
        assert_eq!(state.serial(), 1);
        assert!(state.is_settled());
        let second = lists(&["a", "b"]);
        let requests = state.status_arrived(Arc::clone(&second));
        assert_eq!(retired(&requests), 1, "{requests:?}");
        assert_eq!(state.serial(), 2);
        assert!(Arc::ptr_eq(state.drawn().unwrap(), &second));
        assert_eq!(shown_rows(&state, ChangeList::Unstaged), &ShownFiles::All);
    }

    /// With a filter on, a new status is asked for its rows and the lists before it stay drawn
    /// until they come; an answer for the old lists or an old text is not kept, and is freed on
    /// a worker. Caught by: the new lists drawn with the old rows (rows of one status read
    /// against another), or a stale answer kept.
    #[test]
    fn with_a_filter_a_status_is_drawn_once_its_rows_arrive() {
        let mut state = LocalChangesState::default();
        let first = lists(&["a", "b"]);
        let _ = state.status_arrived(Arc::clone(&first));
        let requests = state.filter("b");
        let (changes, text) = asked(&requests).unwrap_or_else(|| panic!("{requests:?}"));
        assert!(Arc::ptr_eq(changes, &first) && text == "b");
        assert!(!state.is_settled());
        assert_eq!(
            shown_rows(&state, ChangeList::Unstaged),
            &ShownFiles::Waiting,
            "every row counted as matched before an answer"
        );
        let _ = state.filtered(
            Arc::clone(&first),
            "b",
            MatchedRows {
                unstaged: vec![1],
                staged: Vec::new(),
                paths: 1,
            },
        );
        assert!(state.is_settled());

        let second = lists(&["a", "b", "bb"]);
        let requests = state.status_arrived(Arc::clone(&second));
        let (changes, _) = asked(&requests).unwrap_or_else(|| panic!("{requests:?}"));
        assert!(Arc::ptr_eq(changes, &second));
        assert!(
            Arc::ptr_eq(state.drawn().unwrap(), &first),
            "the old lists stopped being drawn before the new ones' rows"
        );
        assert_eq!(state.serial(), 1);

        // An answer for the old lists, and one for another text, are not kept.
        let stale = state.filtered(Arc::clone(&first), "b", MatchedRows::default());
        assert_eq!(retired(&stale), 1);
        let other = state.filtered(Arc::clone(&second), "a", MatchedRows::default());
        assert_eq!(retired(&other), 1);
        assert!(Arc::ptr_eq(state.drawn().unwrap(), &first));

        let kept = state.filtered(
            Arc::clone(&second),
            "b",
            MatchedRows {
                unstaged: vec![1, 2],
                staged: Vec::new(),
                paths: 2,
            },
        );
        assert_eq!(
            retired(&kept),
            1,
            "the old lists were not freed on a worker"
        );
        assert!(Arc::ptr_eq(state.drawn().unwrap(), &second));
        assert_eq!(state.serial(), 2);
        assert_eq!(
            shown_rows(&state, ChangeList::Unstaged),
            &ShownFiles::Filtered(vec![1, 2])
        );
        assert_eq!(shown_paths(&state), Some(2));
        // Cleared, every row is drawn and no count of what a filter left is kept.
        let _ = state.filter("");
        assert_eq!(shown_paths(&state), None);
    }

    /// Phase 16's QA #4: a status that keeps every selected path leaves the selection as it was
    /// — the same value, not a copy — while one that takes a path away moves it. Caught by: a
    /// refresh that clones and rewrites the selection whatever it keeps.
    #[test]
    fn a_status_that_keeps_the_selection_leaves_it_uncopied() {
        let mut state = LocalChangesState::default();
        let _ = state.status_arrived(lists(&["a", "b", "c"]));
        assert_eq!(
            state.selection().map(|s| s.primary().clone()),
            Some(RepoPath::from("a")),
            "the first status selects the first row"
        );
        let wide = LocalSelection::spanning(
            ChangeList::Unstaged,
            RepoPath::from("a"),
            RepoPath::from("b"),
            [RepoPath::from("a"), RepoPath::from("b")],
        );
        state.set_selection(Some(wide.clone()));
        let held = state.selection().map(|s| s.paths().as_ptr());
        let _ = state.status_arrived(lists(&["a", "b", "c", "d"]));
        assert_eq!(state.selection(), Some(&wide));
        assert_eq!(
            state.selection().map(|s| s.paths().as_ptr()),
            held,
            "a selection kept whole was copied"
        );
        let _ = state.status_arrived(lists(&["b", "c"]));
        assert_eq!(
            state.selection().map(|s| s.paths().len()),
            Some(1),
            "a path the status took away stayed selected"
        );
    }

    /// Clearing the filter draws every row of the newest lists at once, a status waiting for
    /// its rows among them. Caught by: the waiting status left undrawn.
    #[test]
    fn clearing_the_filter_draws_the_newest_lists_whole() {
        let mut state = LocalChangesState::default();
        let _ = state.status_arrived(lists(&["a"]));
        let _ = state.filter("a");
        let second = lists(&["a", "b"]);
        let _ = state.status_arrived(Arc::clone(&second));
        let requests = state.filter("");
        assert_eq!(retired(&requests), 1);
        assert!(Arc::ptr_eq(state.drawn().unwrap(), &second));
        assert_eq!(shown_rows(&state, ChangeList::Staged), &ShownFiles::All);
        assert!(state.is_settled());
    }
}
