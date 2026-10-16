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
//! whenever they are replaced (`diff::actions::show_primary`).
//!
//! **Amend is an overlay over the status** (staging-and-commit R10.3, `review-code-app-ui.md`
//! M3): every status lands here. While Amend is ticked, the lists drawn are amend's laid out
//! over the newest status — Staged as amend's staged list against `HEAD`'s parent — read for it
//! on the local lane (`Request::Amending`, which reads that list and `HEAD`'s message, never
//! what the amend costs); until they arrive, the lists drawn before stay drawn, so the selection
//! never moves through the status's own Staged list on the way. Where amend's staged list cannot
//! be read, the status's own lists are drawn and the box says so.
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

use crate::local_changes::next_selection::{Leaving, Next, Rows, next_selection};
use crate::worker::{Request, Retired};

/// The list side's share of the view until its splitter is dragged, in percent: the Changes
/// tab's.
pub const LIST_WIDTH: f32 = crate::detail::changes_tab::LIST_WIDTH;

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
    pub acting: State<crate::local_changes::actions::Acting>,
    /// A drag across the diff's lines, and the lines it selected (staging-and-commit R9.2):
    /// written by the diff view, read by the chords heard on the diff.
    pub lines: State<cairn_ui::LineDrag>,
    /// The commit box under the diff: the draft, kept for the window's life (R10.7), and the
    /// rest of its state.
    pub commit: crate::commit_box::state::CommitBoxView,
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
            acting: use_state(crate::local_changes::actions::Acting::default),
            lines: use_state(cairn_ui::LineDrag::default),
            commit: crate::commit_box::state::CommitBoxView::used(),
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
            acting: State::create(crate::local_changes::actions::Acting::default()),
            lines: State::create(cairn_ui::LineDrag::default()),
            commit: crate::commit_box::state::CommitBoxView::created(),
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
    /// While Amend is ticked: amend's overlay (module docs).
    amend: Option<AmendOverlay>,
}

/// Amend's overlay while Amend is ticked: the status amend's lists are being read over, and why
/// the last read of amend's staged list failed, if it did.
#[derive(Debug, Default)]
struct AmendOverlay {
    awaiting: Option<Arc<LocalChanges>>,
    unread: Option<String>,
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

    /// A status arrived: drawn at once with no filter on; with one, its rows asked first; while
    /// Amend is ticked, amend's lists asked over it instead, the lists drawn kept meanwhile.
    /// Returns what to submit: the filter's or the amend's ask, and what is let go of, for a
    /// worker to free.
    pub fn status_arrived(&mut self, changes: Arc<LocalChanges>) -> Vec<Request> {
        if let Some(overlay) = &mut self.amend {
            let mut requests = vec![Request::Amending {
                status: Arc::clone(&changes),
            }];
            requests.extend(overlay.awaiting.replace(changes).map(retire));
            return requests;
        }
        self.lay_out(changes)
    }

    /// Amend ticked: amend's lists asked over `latest`, the newest status. Returns what to submit.
    pub fn amend_ticked(&mut self, latest: Option<Arc<LocalChanges>>) -> Vec<Request> {
        if self.amend.is_some() {
            return Vec::new();
        }
        self.amend = Some(AmendOverlay::default());
        latest
            .map(|latest| self.status_arrived(latest))
            .unwrap_or_default()
    }

    /// Amend unticked, or ended by a commit made: the overlay let go of, amend's read in flight
    /// ended (`Request::StopAmending`), and `latest`'s own lists drawn wherever they are not the
    /// newest drawn already — amend's were, or a status arrived while amend's lists for it were
    /// on their way. Returns what to submit.
    pub fn amend_unticked(&mut self, latest: Option<Arc<LocalChanges>>) -> Vec<Request> {
        let Some(overlay) = self.amend.take() else {
            return Vec::new();
        };
        let mut requests = vec![Request::StopAmending];
        requests.extend(overlay.awaiting.map(retire));
        let drawn = self
            .newest()
            .zip(latest.as_ref())
            .is_some_and(|(newest, latest)| Arc::ptr_eq(newest, latest));
        if let (false, Some(latest)) = (drawn, latest) {
            requests.extend(self.lay_out(latest));
        }
        requests
    }

    /// Amend's lists arrived for `status`: drawn when Amend is still ticked and `status` is the
    /// newest asked — or, where amend's staged list could not be read, `status`'s own, the reason
    /// kept for the box. Returns what to submit: what is let go of, the answer's holds among it.
    pub fn amend_arrived(
        &mut self,
        status: Arc<LocalChanges>,
        lists: Result<Arc<LocalChanges>, String>,
    ) -> Vec<Request> {
        let wanted = self.amend.as_ref().is_some_and(|overlay| {
            overlay
                .awaiting
                .as_ref()
                .is_some_and(|awaiting| Arc::ptr_eq(awaiting, &status))
        });
        if !wanted {
            return vec![Request::Retire(Retired::amending(status, lists.ok()))];
        }
        let awaiting = self
            .amend
            .as_mut()
            .and_then(|overlay| overlay.awaiting.take());
        let mut requests: Vec<Request> = awaiting.map(retire).into_iter().collect();
        let unread = match lists {
            Ok(lists) => {
                requests.push(retire(status));
                requests.extend(self.lay_out(lists));
                None
            }
            Err(why) => {
                requests.extend(self.lay_out(status));
                Some(why)
            }
        };
        if let Some(overlay) = &mut self.amend {
            overlay.unread = unread;
        }
        requests
    }

    /// Why amend's staged list could not be read, while Amend is ticked and the status's own
    /// lists are drawn in its place (R10.3: a partial clone's missing blob).
    pub fn amend_unread(&self) -> Option<&str> {
        self.amend.as_ref()?.unread.as_deref()
    }

    /// `changes` laid out: drawn at once with no filter on; with one, its rows asked first.
    fn lay_out(&mut self, changes: Arc<LocalChanges>) -> Vec<Request> {
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

    /// Phase 18's QA items 4 and 5: unticking Amend ends amend's read in flight
    /// (`Request::StopAmending`), and draws the newest status even when amend's lists never
    /// arrived — a status parked while Amend was ticked is drawn, not left behind the pre-tick
    /// lists, and the parked hold is let go of on a worker. Caught by: the read left running on
    /// the serial lane, or the newer status undrawn until the next refresh.
    #[test]
    fn unticking_amend_ends_its_read_and_draws_the_newest_status() {
        let mut state = LocalChangesState::default();
        let first = lists(&["a"]);
        let _ = state.status_arrived(Arc::clone(&first));
        let asked = state.amend_ticked(Some(Arc::clone(&first)));
        assert!(
            asked.iter().any(|r| matches!(r, Request::Amending { .. })),
            "{asked:?}"
        );
        let second = lists(&["a", "b"]);
        let parked = state.status_arrived(Arc::clone(&second));
        assert!(
            Arc::ptr_eq(state.drawn().unwrap(), &first),
            "a status drawn before amend's lists for it"
        );
        let requests = state.amend_unticked(Some(Arc::clone(&second)));
        assert!(
            requests.iter().any(|r| matches!(r, Request::StopAmending)),
            "the amend read was not ended: {requests:?}"
        );
        assert!(
            Arc::ptr_eq(state.drawn().unwrap(), &second),
            "the newest status was not drawn"
        );
        // The parked hold and the pre-tick lists, each freed on a worker.
        assert_eq!(retired(&requests), 2, "{requests:?}");
        drop(parked);
        // With the newest already drawn, an untick redraws nothing.
        let mut state = LocalChangesState::default();
        let _ = state.status_arrived(Arc::clone(&second));
        let _ = state.amend_ticked(None);
        let serial = state.serial();
        let requests = state.amend_unticked(Some(Arc::clone(&second)));
        assert_eq!(state.serial(), serial, "the drawn status was drawn again");
        assert_eq!(retired(&requests), 0, "{requests:?}");
    }
}
