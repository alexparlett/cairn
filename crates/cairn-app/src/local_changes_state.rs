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
//! lists drawn are numbered ([`LocalChangesState::serial`]), so the path chosen in them is found
//! again — or another chosen — whenever they are replaced (`local_changes_pane`).

use std::sync::{Arc, LazyLock};

use cairn_model::{ChangeList, LocalChanges, MatchedRows, WorkingTreeStatus};
use cairn_ui::{ChangeCursor, ShownFiles};
use freya::prelude::*;

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
        }
    }
}

/// The lists drawn: a status's, and the rows of each the filter left, or all of them.
#[derive(Debug)]
struct Drawn {
    changes: Arc<LocalChanges>,
    shown: [ShownFiles; 2],
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

    /// The lists drawn, shared.
    #[cfg(test)]
    pub fn drawn(&self) -> Option<&Arc<LocalChanges>> {
        self.drawn.as_ref().map(|drawn| &drawn.changes)
    }

    /// Moves each time other lists are drawn: what a path chosen in them was chosen against.
    pub fn serial(&self) -> u64 {
        self.serial
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether what is drawn is the newest status, filtered by the text as it is.
    pub fn is_settled(&self) -> bool {
        self.pending.is_none() && self.drawn.is_some() && self.answered == self.text
    }

    /// The newest lists: the ones waiting for their rows, or else the ones drawn.
    fn newest(&self) -> Option<&Arc<LocalChanges>> {
        self.pending
            .as_ref()
            .or(self.drawn.as_ref().map(|drawn| &drawn.changes))
    }

    /// `changes` drawn with `shown` rows; what it replaces handed back to be freed on a worker.
    fn draw(&mut self, changes: Arc<LocalChanges>, shown: [ShownFiles; 2]) -> Vec<Request> {
        let replaced = self.drawn.replace(Drawn { changes, shown });
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
            requests.extend(self.draw(changes, [ShownFiles::All, ShownFiles::All]));
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
                requests.extend(self.draw(pending, [ShownFiles::Waiting, ShownFiles::Waiting]));
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
                Some(pending) => self.draw(pending, [ShownFiles::All, ShownFiles::All]),
                None => {
                    if let Some(drawn) = &mut self.drawn {
                        drawn.shown = [ShownFiles::All, ShownFiles::All];
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
        let MatchedRows { unstaged, staged } = rows;
        let shown = [ShownFiles::Filtered(unstaged), ShownFiles::Filtered(staged)];
        self.answered = self.text.clone();
        let mut requests = Vec::new();
        match self.pending.take() {
            Some(pending) => requests.extend(self.draw(pending, shown)),
            None => {
                if let Some(drawn) = &mut self.drawn {
                    drawn.shown = shown;
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
    /// the serial left (a chosen path never looked for again).
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
