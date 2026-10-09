//! Local Changes, read only (refs-and-status R9): in the main region in place of the history
//! when Local Changes is pressed in the sidebar — the filter and the two lists on the left,
//! Unstaged above Staged, and the chosen path's diff on the right under the diff view's bar, as
//! Fork lays the view out (`fork-refs-and-status-ui.md` section 6). A component of its own,
//! mounted only while it is shown, so the work it watches for — a filter's text, a path to
//! choose, a diff to ask again — is done only for a view someone is looking at.
//!
//! **What it asks, and where that runs.** Typing in the filter asks a worker which rows match
//! (`Request::FilterLocalChanges`). Choosing a path asks its diff through the working-tree query
//! in the file-diff lane — its staged diff from Staged, its unstaged or untracked diff from
//! Unstaged (R9.3) — superseding the last; a conflicted path asks nothing and draws its notice
//! (R9.4). With nothing chosen, the first path the lists show is, as the view opens (R9.3).
//! When other lists are drawn — a refresh's status — the path chosen is looked for in them by a
//! search: still listed, it is asked again, its diff drawn until the new one comes; gone from
//! the status, the first path shown is chosen in its place, so no diff is drawn for a path the
//! status no longer lists. The filter follows the Changes tab's rule: a path it hides stays
//! chosen, its diff drawn and no row highlighted, and is asked again on each refresh.
//!
//! **What it draws.** The lists only for the status they are laid out over; the diff only for
//! the answer naming the path chosen, its side and the settings now — always the exact diff,
//! ignore whitespace drawn off and disabled whatever the shared setting says
//! (staging-and-commit R8.5) — and, under the lists, what the writes asked from here are doing
//! (R8.6).
//!
//! **What it does** (staging-and-commit R8): the lists' selection, Fork's routes to stage and
//! unstage, and the discard through its confirmation are `local_changes_actions`'; this view
//! hands them what the lists and the diff report, and opens the confirmation once the engine
//! has said what the discard would lose. The diff hears the stage and discard chords
//! (`Scope::LocalChanges`) and acts on the file it shows, whole.

use std::rc::Rc;

use cairn_model::{
    ChangeList, ChangeStatus, ChangedFile, LocalChanges, ShownDiff, UnreadableIndex,
    WorkingTreeStatus,
};
use cairn_ui::accelerators::{self, Scope};
use cairn_ui::{
    DiffHeader, DiffNotice, DiffNoticeView, DiffSettings, DiffView, ListIntent, LocalChangesList,
    ShownFiles,
};
use freya::prelude::*;

use crate::detail_pane::notice;
use crate::diff_state::{WorkingChoice, WorkingShown, answered_working};
use crate::local_changes_state::{drawn_changes, shown_paths, shown_rows};
use crate::window::View;
use crate::worker::{FileQuery, Refreshed, Request};
use crate::{diff_actions, local_changes_actions, shortcuts};

/// Said in place of the lists before the first status is read.
pub const READING_STATUS: &str = "Reading the working tree's status…";
/// Said beside the lists while no path is chosen.
pub const NO_PATH_CHOSEN: &str = "Choose a path to see its diff.";
/// Said beside the lists when status lists nothing.
pub const NO_LOCAL_CHANGES: &str = "No local changes.";
/// Said in place of the lists for a bare repository.
pub const NO_WORKING_TREE: &str = "This repository has no working tree.";
/// Said in place of the lists when the git in use cannot read a sparse index (R3.7).
pub const SPARSE_INDEX: &str =
    "The index is a sparse index, which the git in use cannot read (git 2.32 and later can).";
/// Said while the chosen path's diff is on its way.
pub const READING_DIFF: &str = crate::changes_tab::READING_DIFF;

/// Why the status could not be read, as said over the lists.
pub fn status_failure(message: &str) -> String {
    format!("The working tree's status could not be read: {message}")
}

/// The least a drag, or a narrowing window, leaves either side: the Changes tab's.
const LIST_MIN_PIXELS: f32 = 200.0;
const DIFF_MIN_PIXELS: f32 = 240.0;

pub struct LocalChangesPane {
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
}

impl LocalChangesPane {
    pub fn new(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Self {
        Self { view, submit }
    }
}

// By the handles it reads: the submitter is the same repository's however often it is built.
impl PartialEq for LocalChangesPane {
    fn eq(&self, other: &Self) -> bool {
        let (one, two) = (&self.view, &other.view);
        one.local == two.local
            && one.diff == two.diff
            && one.diff_settings == two.diff_settings
            && one.refreshed == two.refreshed
            && self.submit.is_some() == other.submit.is_some()
    }
}

fn submit_all(requests: Vec<Request>, submit: Option<&dyn Fn(Request)>) {
    if let Some(submit) = submit {
        for request in requests {
            submit(request);
        }
    }
}

/// The first row the lists show — Unstaged's, else Staged's — by its list and its row there.
fn first_shown(
    lists: &LocalChanges,
    unstaged: &ShownFiles,
    staged: &ShownFiles,
) -> Option<(ChangeList, usize)> {
    [
        (ChangeList::Unstaged, unstaged),
        (ChangeList::Staged, staged),
    ]
    .into_iter()
    .find_map(|(list, shown)| {
        (shown.len(lists.len(list)) > 0)
            .then(|| shown.file_at(0))
            .flatten()
            .map(|row| (list, row))
    })
}

/// What following the lists does to the path chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Follow {
    /// Chosen in these lists already: nothing is written.
    Keep,
    /// Still listed: asked again, as it is listed now — `None` for a conflict, which asks
    /// nothing.
    ReAsk(Option<FileQuery>),
    /// Gone, or never chosen: the first path the lists show.
    ChooseFirst(ChangeList, usize),
    /// Gone, and no path shown: let go of.
    LetGo,
    /// None chosen and none shown: nothing is written. A decision that wrote here would wake
    /// the effect that asks it again, for good.
    Nothing,
}

/// What the path chosen is to be in the lists drawn, numbered `serial`, through the filter's
/// rows: found again in them, it is asked again; gone, or none chosen, the first path shown is
/// chosen; with none shown, a path chosen is let go of and none chosen stays so. Only a path
/// gone from the status is let go of: one the filter hides stays chosen and drawn, its row
/// unhighlighted, as the Changes tab keeps a file its filter hides.
fn follow(
    lists: &LocalChanges,
    serial: u64,
    shown: [&ShownFiles; 2],
    chosen: Option<&WorkingChoice>,
    settings: DiffSettings,
) -> Follow {
    if let Some(choice) = chosen {
        if choice.lists == serial {
            return Follow::Keep;
        }
        let found = lists
            .row_of(choice.list, &choice.path)
            .and_then(|row| lists.get(choice.list, row));
        if let Some(change) = found {
            return Follow::ReAsk(diff_actions::working_query(choice.list, &change, settings));
        }
    }
    match first_shown(lists, shown[0], shown[1]) {
        Some((list, row)) => Follow::ChooseFirst(list, row),
        None if chosen.is_some() => Follow::LetGo,
        None => Follow::Nothing,
    }
}

/// Does what [`follow`] decides for the lists drawn now. Run as the view is shown and whenever
/// other lists are drawn.
fn follow_the_lists(view: View, submit: Option<&dyn Fn(Request)>) {
    let decided = {
        let local = view.local.state.peek();
        if !local.is_settled() {
            return;
        }
        let diff = view.diff.peek();
        (
            local.serial(),
            follow(
                drawn_changes(&local),
                local.serial(),
                [
                    shown_rows(&local, ChangeList::Unstaged),
                    shown_rows(&local, ChangeList::Staged),
                ],
                diff.working_choice(),
                *view.diff_settings.peek(),
            ),
        )
    };
    let mut diff = view.diff;
    match decided {
        (_, Follow::Keep | Follow::Nothing) => {}
        (serial, Follow::ReAsk(query)) => {
            let requests = diff.write().refresh_working(serial, query);
            submit_all(requests, submit);
        }
        (_, Follow::ChooseFirst(list, row)) => {
            diff_actions::choose_working(list, row, view, submit);
        }
        // Written only when there is a path to let go of: a write wakes the effect again, so a
        // decision gone wrong here must cost one more look, never a loop.
        (_, Follow::LetGo) if diff.peek().working_choice().is_some() => {
            let requests = diff.write().let_go_of_working();
            submit_all(requests, submit);
        }
        (_, Follow::LetGo) => {}
    }
}

impl Component for LocalChangesPane {
    fn render(&self) -> impl IntoElement {
        let view = self.view;
        // The split's width as last laid out, which turns the list's dragged width into its
        // share. Only peeked: a measurement redraws nothing.
        let mut split_width = use_state(|| 0f32);
        // The filter's text, as typed: asked of a worker when it changes.
        let filtering = self.submit.clone();
        use_side_effect(move || {
            let text = view.local.filter_text.read().clone();
            let mut local = view.local.state;
            if local.peek().text() == text {
                return;
            }
            let requests = local.write().filter(&text);
            submit_all(requests, filtering.as_deref());
        });
        // The path chosen follows the lists drawn: subscribed to the lists' state and the
        // selection, acting only when the lists moved under it or nothing is chosen.
        let following = self.submit.clone();
        use_side_effect(move || {
            let settled = {
                let local = view.local.state.read();
                let serial = local.serial();
                let chosen = view.diff.read().working_choice().map(|choice| choice.lists);
                local.is_settled() && chosen != Some(serial)
            };
            if settled {
                follow_the_lists(view, following.as_deref());
            }
        });
        // What a discard would lose has arrived: the confirmation opens over the window, its
        // token asking the discard (R8.4).
        let confirming = self.submit.clone();
        use_side_effect(move || {
            if view.local.acting.read().has_arrived() {
                local_changes_actions::confirm_arrived(view, confirming.clone());
            }
        });
        // A discard's count nobody will now confirm stops holding the local lane as the view
        // goes (phase 07's QA item 1): the view moved on, so the count is let go of and ended.
        let stopping = self.submit.clone();
        use_drop(move || {
            let mut acting = view.local.acting;
            if acting.peek().is_reading() {
                acting.write().forget_discard();
                if let Some(submit) = stopping.as_deref() {
                    submit(Request::StopCounting);
                }
            }
        });
        // A path whose request lost the file-diff lane to the commit's file or the files opened
        // in place is asked again as the view is shown.
        let reasking = self.submit.clone();
        use_side_effect(move || {
            if !view.diff.read().working_needs_asking() {
                return;
            }
            let mut diff = view.diff;
            let asked = diff.write().reask_working();
            if let (Some(request), Some(submit)) = (asked, reasking.as_deref()) {
                submit(request);
            }
        });

        let local = view.local.state.read();
        let failure = view
            .refreshed
            .read()
            .failure(Refreshed::Status)
            .map(status_failure);
        if !local.has_lists() {
            return match failure {
                Some(failure) => notice(failure, true),
                None => notice(READING_STATUS, false),
            };
        }
        match drawn_changes(&local).status() {
            WorkingTreeStatus::Listed(_) => {}
            WorkingTreeStatus::NoWorkingTree => return notice(NO_WORKING_TREE, false),
            WorkingTreeStatus::IndexUnreadable(UnreadableIndex::Sparse) => {
                return notice(SPARSE_INDEX, true);
            }
        }
        let chosen = view.diff.read().working_choice().and_then(|choice| {
            drawn_changes(&local)
                .row_of(choice.list, &choice.path)
                .map(|row| (choice.list, row))
        });
        let empty = drawn_changes(&local).is_empty();
        let shown_paths = shown_paths(&local);
        drop(local);

        let choosing = self.submit.clone();
        let intending = self.submit.clone();
        let readable = view.local.state.into_readable();
        let list = LocalChangesList::new(
            readable.map(drawn_changes, |_| true),
            readable.map(|state| shown_rows(state, ChangeList::Unstaged), |_| true),
            readable.map(|state| shown_rows(state, ChangeList::Staged), |_| true),
            view.local.filter_text,
        )
        .shown_paths(shown_paths)
        .split(view.local.lists_split)
        .chosen(chosen)
        .selection(view.local.selection)
        .held(view.held_keys)
        .on_choose(move |(list, row): (ChangeList, usize)| {
            local_changes_actions::choose(list, row, view, choosing.as_deref());
        })
        .on_intent(move |intent: ListIntent| {
            local_changes_actions::intent(intent, view, intending.clone());
        });
        let said =
            local_changes_actions::acting_line(&view.local.acting.read(), &view.writes.read());
        let list = rect()
            .expanded()
            .content(Content::Flex)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(list),
            )
            .maybe_child(said.map(|(line, failed)| acting(line, failed)));

        let mut width = view.local.list_width;
        // Peeked: the share only matters when the split is laid out anew.
        let list_share = *width.peek();
        let hearing = self.submit.clone();
        rect()
            .expanded()
            .content(Content::Flex)
            // The diff's own chords (previous and next change), heard from inside the view, as
            // the detail pane hears them from inside it.
            .on_key_down(move |e: Event<KeyboardEventData>| {
                if let Some(action) = accelerators::resolve_key(&e, Scope::Detail) {
                    e.stop_propagation();
                    shortcuts::act(action, view, hearing.as_deref());
                }
            })
            .maybe_child(failure.map(banner))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .on_sized(move |e: Event<SizedEventData>| {
                        split_width.set_if_modified(e.area.width())
                    })
                    .child(
                        ResizableContainer::new()
                            .direction(Direction::Horizontal)
                            .panel(
                                ResizablePanel::new(PanelSize::percent(list_share))
                                    .min_pixels(LIST_MIN_PIXELS)
                                    .on_resized(move |dragged: f32| {
                                        if let Some(share) = crate::changes_tab::share_of(
                                            dragged,
                                            *split_width.peek(),
                                        ) {
                                            width.set(share);
                                        }
                                    })
                                    .child(list),
                            )
                            .panel(
                                ResizablePanel::new(PanelSize::percent(100. - list_share))
                                    .min_pixels(DIFF_MIN_PIXELS)
                                    .child(diff_side(view, self.submit.clone(), empty)),
                            ),
                    ),
            )
            .into()
    }
}

/// One line under the lists: what the writes asked from here are doing, or why the last did
/// not do what was asked (R8.6).
fn acting(line: String, failed: bool) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    rect()
        .width(Size::fill())
        .padding(Gaps::new(4., 8., 4., 8.))
        .background(colours.surface_tertiary)
        .child(
            label()
                .text(line)
                .max_lines(3)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(12.)
                .color(if failed {
                    colours.error
                } else {
                    colours.text_secondary
                }),
        )
        .into()
}

/// One line over the lists: why the last status read failed, the lists before it still drawn.
fn banner(message: String) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    rect()
        .width(Size::fill())
        .padding(Gaps::new(4., 8., 4., 8.))
        .background(colours.surface_tertiary)
        .child(
            label()
                .text(message)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(12.)
                .color(colours.error),
        )
        .into()
}

/// The file the bar names: the answer's own when it is here — a rename's source and all — else
/// the path chosen.
fn header_file(shown: Option<&ShownDiff>, path: &cairn_model::RepoPath) -> ChangedFile {
    shown.map_or_else(
        || ChangedFile {
            status: ChangeStatus::Modified,
            old_path: path.clone(),
            new_path: path.clone(),
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        },
        |shown| shown.diff().file.clone(),
    )
}

/// The diff side: the chosen path's bar and its diff, or its notice.
fn diff_side(view: View, submit: Option<Rc<dyn Fn(Request)>>, empty: bool) -> Element {
    let state = view.diff.read();
    let (Some(choice), Some(shown)) = (state.working_choice(), state.working_shown()) else {
        return notice(
            if empty {
                NO_LOCAL_CHANGES
            } else {
                NO_PATH_CHOSEN
            },
            false,
        );
    };
    let settings = *view.diff_settings.read();
    let drawn = state.shown_working();
    let acting = submit.clone();
    let hearing = submit.clone();
    // The exact diff, always (R8.5): what is staged from here is what is drawn.
    let header = DiffHeader::new(header_file(drawn, &choice.path), settings)
        .exact(true)
        .on_action(move |pressed| {
            shortcuts::act(shortcuts::of_header(pressed), view, acting.as_deref());
        });
    let body = match shown {
        WorkingShown::Conflicted => DiffNoticeView::new(DiffNotice::Conflicted).into(),
        WorkingShown::Waiting => notice(READING_DIFF, false),
        WorkingShown::Failed(message) => notice(message.to_owned(), true),
        WorkingShown::Nothing => DiffNoticeView::new(DiffNotice::NothingShown {
            hides_changes: false,
        })
        .into(),
        WorkingShown::Diff(shown) => diff_body(shown, view, submit, settings.side_by_side()),
    };
    rect()
        .expanded()
        .content(Content::Flex)
        // The stage and discard chords heard on the diff act on the file it shows (R7.3): the
        // gesture's lines are phase 08's.
        .on_key_down(move |e: Event<KeyboardEventData>| {
            if let Some(action) = accelerators::resolve_key(&e, Scope::LocalChanges) {
                e.stop_propagation();
                local_changes_actions::on_the_diff(action, view, hearing.as_deref());
            }
        })
        .child(header)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.))
                .child(body),
        )
        .into()
}

/// The chosen path's diff: its rows, or the notice that stands in their place (R6.8).
fn diff_body(
    shown: &ShownDiff,
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
    side_by_side: bool,
) -> Element {
    if let Some(stands_in) = DiffNotice::of(shown) {
        return DiffNoticeView::new(stands_in)
            .on_load(move |()| diff_actions::load_working_anyway(view, submit.as_deref()))
            .into();
    }
    let current = view.local.cursor.read().and_then(|cursor| {
        shown
            .stops(side_by_side)
            .and_then(|stops| stops.rows(cursor.change))
    });
    DiffView::new(
        view.diff.into_readable().map(answered_working, |_| true),
        view.local.scroll,
    )
    .side_by_side(side_by_side)
    .current(current)
    .into()
}

#[cfg(test)]
mod tests {
    use cairn_model::{RepoPath, StatusEntry, WorkingTreeStatus};

    use super::*;
    use crate::diff_state::DiffState;

    fn lists(paths: &[&str]) -> LocalChanges {
        LocalChanges::new(WorkingTreeStatus::Listed(
            paths
                .iter()
                .map(|path| StatusEntry::Untracked(RepoPath::from(*path)))
                .collect(),
        ))
    }

    fn chosen(path: &str, lists: u64) -> WorkingChoice {
        WorkingChoice {
            list: ChangeList::Unstaged,
            path: RepoPath::from(path),
            lists,
        }
    }

    const ALL: [&ShownFiles; 2] = [&ShownFiles::All, &ShownFiles::All];

    /// Phase 09 QA's TC1: an empty status with nothing chosen decides nothing — a decision that
    /// wrote there woke the effect again, for good, which a test could only see as a hang — and
    /// with a path chosen lets it go once, then decides nothing. Caught by: a let-go decided
    /// with nothing chosen (`None if chosen.is_some() || true`).
    #[test]
    fn an_empty_status_lets_a_path_go_once_and_then_decides_nothing() {
        let empty = lists(&[]);
        let settings = DiffSettings::default();
        assert_eq!(follow(&empty, 2, ALL, None, settings), Follow::Nothing);
        assert_eq!(
            follow(&empty, 2, ALL, Some(&chosen("a.rs", 1)), settings),
            Follow::LetGo
        );
        // The effect, run as the view runs it: each decision applied, and asked again after
        // each write, until it writes nothing.
        let mut state = DiffState::default();
        let _ = state.choose_working(chosen("a.rs", 1), None);
        let mut writes = 0;
        loop {
            match follow(&empty, 2, ALL, state.working_choice(), settings) {
                Follow::Keep | Follow::Nothing => break,
                Follow::LetGo => {
                    let _ = state.let_go_of_working();
                }
                other => panic!("an empty status decided {other:?}"),
            }
            writes += 1;
            assert!(
                writes <= 1,
                "following an empty status writes again and again"
            );
        }
        assert_eq!(writes, 1);
    }

    /// The decision for each case: a path chosen in these lists kept; one still listed asked
    /// again as it is listed now; one gone, or none, the first path shown; a filter hiding the
    /// path chosen keeps it (the Changes tab's rule), and a filter leaving no row lets none be
    /// chosen. Caught by: a path re-asked against the lists it was chosen in, a path the filter
    /// hides let go of, or a hidden first row chosen.
    #[test]
    fn the_path_chosen_follows_the_lists_as_decided() {
        let settings = DiffSettings::default();
        let drawn = lists(&["a.rs", "b.rs"]);
        assert_eq!(
            follow(&drawn, 1, ALL, Some(&chosen("b.rs", 1)), settings),
            Follow::Keep
        );
        match follow(&drawn, 2, ALL, Some(&chosen("b.rs", 1)), settings) {
            Follow::ReAsk(Some(query)) => {
                assert_eq!(diff_state_path(&query), "b.rs");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            follow(&drawn, 2, ALL, Some(&chosen("gone.rs", 1)), settings),
            Follow::ChooseFirst(ChangeList::Unstaged, 0)
        );
        assert_eq!(
            follow(&drawn, 2, ALL, None, settings),
            Follow::ChooseFirst(ChangeList::Unstaged, 0)
        );
        // The filter shows b.rs alone: a.rs, chosen and hidden, stays chosen and is asked
        // again; with none chosen, the first row shown is b.rs.
        let filtered = [
            &ShownFiles::Filtered(vec![1]),
            &ShownFiles::Filtered(Vec::new()),
        ];
        assert!(matches!(
            follow(&drawn, 2, filtered, Some(&chosen("a.rs", 1)), settings),
            Follow::ReAsk(Some(_))
        ));
        assert_eq!(
            follow(&drawn, 2, filtered, None, settings),
            Follow::ChooseFirst(ChangeList::Unstaged, 1)
        );
        let none = [
            &ShownFiles::Filtered(Vec::new()),
            &ShownFiles::Filtered(Vec::new()),
        ];
        assert_eq!(follow(&drawn, 2, none, None, settings), Follow::Nothing);
    }

    fn diff_state_path(query: &FileQuery) -> String {
        match &query.target {
            crate::worker::FileTarget::WorkingTree { path, .. } => path.display().into_owned(),
            crate::worker::FileTarget::Committed { file, .. } => {
                file.new_path.display().into_owned()
            }
        }
    }
}
