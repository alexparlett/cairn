//! Local Changes: in the main region in place of the history when Local Changes is pressed in
//! the sidebar — the filter and the two lists on the left, Unstaged above Staged, and the
//! selection's primary's diff on the right under the diff view's bar, as Fork lays the view
//! out. A component of its own, mounted only while it is shown, so the work it watches for — a
//! filter's text, a path to choose, a diff to ask again — is done only for a view someone is
//! looking at.
//!
//! **What it asks, and where that runs.** Typing in the filter asks a worker which rows match
//! (`Request::FilterLocalChanges`). The diff shown is the selection's primary's
//! (`diff::actions::show_primary`), asked through the working-tree query in the file-diff lane —
//! its staged diff from Staged, its unstaged or untracked diff from Unstaged — superseding the
//! last; a conflicted path asks nothing and draws its notice. With nothing selected, the first
//! path the lists show is, as the view opens. Where the selection goes when other lists are
//! drawn or the filter's rows change is one pure rule (`next_selection`), run as the status and
//! the filter's answers arrive (`session::apply`), never by this view: a path still listed and
//! shown stays, its diff asked again and drawn until the new one comes; one gone, or hidden by
//! the filter, gives its place to the row that took it — so no diff is drawn for a path the
//! lists do not show.
//!
//! **What it draws.** The lists only for the status they are laid out over; the diff only for
//! the answer naming the primary, its side and the settings now — always the exact diff,
//! ignore whitespace drawn off and disabled whatever the shared setting says — and nothing
//! under the lists: a write's progress is the status box's, its ending its dialog's.
//!
//! **What it does**: the lists' selection, Fork's routes to stage and unstage, and the discard
//! through its confirmation are `local_changes::actions`'; this view hands them what the lists
//! and the diff report, and opens the confirmation once the engine has said what the discard
//! would lose. The diff draws the staging gesture
//! (`cairn_ui::Gesture`): a hovered chunk's actions, a drag-selection's, and the mode change's
//! own row; it hears the stage and discard chords (`Scope::LocalChanges` — ⌘S or Ctrl+S, never
//! a bare Return, which the lists alone hear) and acts on the lines a drag selected, or with
//! none on the selection, whole.
//!
//! **One file**, as in Fork: with several paths selected the diff is still the primary's alone
//! — the file clicked first — with everything one file's diff has, and its bar the primary's
//! own, counting nothing: the lists' highlighted rows show the selection. With nothing selected
//! it says "No file selected". A refresh that re-reads the primary keeps a finished line
//! selection while the text is the same, and when it is not lets it go, the bar saying so until
//! the next key or press (`session::apply`).

use std::rc::Rc;

use cairn_model::{
    ChangeList, ChangeStatus, ChangedFile, DiffContent, ShownDiff, UnreadableIndex,
    WorkingTreeStatus,
};
use cairn_ui::accelerators::{self, Scope};
use cairn_ui::style::{metrics, tones};
use cairn_ui::{
    DiffHeader, DiffNotice, DiffNoticeView, DiffView, Gesture, GestureAct, GestureSide, LineDrag,
    ListIntent, LocalChangesList, ModeRow, SELECTION_CLEARED,
};
use freya::prelude::*;

use crate::commit_box::pane::CommitBoxPane;
use crate::detail::pane::notice;
use crate::diff::state::{WorkingShown, answered_working};
use crate::local_changes::actions;
use crate::local_changes::state::{drawn_changes, selection_of, shown_paths, shown_rows};
use crate::window::View;
use crate::worker::{Refreshed, Request};
use crate::{diff, shortcuts};

/// Said in place of the lists before the first status is read.
pub const READING_STATUS: &str = "Reading the working tree's status…";
/// Said in place of the diff while nothing is selected.
pub const NO_FILE_SELECTED: &str = "No file selected";
/// Said beside the lists when status lists nothing.
pub const NO_LOCAL_CHANGES: &str = "No local changes.";
/// Said in place of the lists for a bare repository.
pub const NO_WORKING_TREE: &str = "This repository has no working tree.";
/// Said in place of the lists when the git in use cannot read a sparse index.
pub const SPARSE_INDEX: &str =
    "The index is a sparse index, which the git in use cannot read (git 2.32 and later can).";
/// Said while the primary's diff is on its way.
pub const READING_DIFF: &str = crate::detail::changes_tab::READING_DIFF;

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
            // Cleared, every row is drawn at once: the primary's diff, if the lists moved.
            diff::actions::show_primary(view, filtering.as_deref(), false);
        });
        // As the view is shown: the first path selected when none is, and the
        // primary's diff asked if the lists moved while the view was hidden. Peeked only, so
        // it runs once; where the selection goes after that is `session::apply`'s, as the
        // status and the filter's rows arrive.
        let showing = self.submit.clone();
        use_side_effect(move || {
            let mut local = view.local.state;
            if local.peek().selection().is_none() && local.peek().has_lists() {
                local.write().select_first_if_none();
            }
            diff::actions::show_primary(view, showing.as_deref(), false);
        });
        // A discard's count nobody will now confirm stops holding the local lane as the view
        // goes: the view moved on, so the count is let go of and ended.
        let stopping = self.submit.clone();
        use_drop(move || {
            let mut acting = view.local.acting;
            if acting.peek().is_reading() {
                acting.write().forget_discard();
                if let Some(submit) = stopping.as_deref() {
                    submit(Request::StopCounting);
                }
                // A confirmation still counting for it would wait on nothing.
                let mut confirming = view.confirming;
                if confirming
                    .peek()
                    .as_ref()
                    .is_some_and(crate::operations::confirming::Confirming::is_counting)
                {
                    confirming.set(None);
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
        .selection(readable.map(selection_of, |_| true))
        .held(view.held_keys)
        .on_choose(move |(list, row): (ChangeList, usize)| {
            actions::choose(list, row, view, choosing.as_deref());
        })
        .on_intent(move |intent: ListIntent| {
            actions::intent(intent, view, intending.clone());
        });
        // Nothing is drawn under the lists: a write's progress is the status box's, its
        // ending its dialog's.
        let list = rect().expanded().child(list);

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
            // "The file changed — line selection cleared." goes at the next key or press,
            // wherever it lands.
            .on_global_key_down(move |_: Event<KeyboardEventData>| quieten(view))
            .on_global_pointer_down(move |_: Event<PointerEventData>| quieten(view))
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
                                        if let Some(share) = crate::detail::changes_tab::share_of(
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
                                    .child(
                                        // The commit box under the diff.
                                        rect()
                                            .expanded()
                                            .content(Content::Flex)
                                            .child(
                                                rect()
                                                    .width(Size::fill())
                                                    .height(Size::flex(1.))
                                                    .child(diff_side(
                                                        view,
                                                        self.submit.clone(),
                                                        empty,
                                                    )),
                                            )
                                            .child(CommitBoxPane::new(view, self.submit.clone())),
                                    ),
                            ),
                    ),
            )
            .into()
    }
}

/// The bar's "line selection cleared" let go of, if it is said.
fn quieten(view: View) {
    let mut lines = view.local.lines;
    if lines.peek().says_cleared() {
        lines.set(LineDrag::default());
    }
}

/// One line over the lists: why the last status read failed, the lists before it still drawn.
fn banner(message: String) -> Element {
    // Its text keeps the theme's error tone, which Fork Dark's table does not name.
    let error = get_theme_or_default().read().colors().error;
    rect()
        .width(Size::fill())
        .padding(Gaps::new(4., 8., 4., 8.))
        .background(tones::BANNER)
        .child(
            label()
                .text(message)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(metrics::BODY)
                .color(error),
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

/// The diff side: the primary's bar and its diff, or its notice — the primary's alone however
/// many paths are selected, its bar counting nothing.
fn diff_side(view: View, submit: Option<Rc<dyn Fn(Request)>>, empty: bool) -> Element {
    let state = view.diff.read();
    let (Some(choice), Some(shown)) = (state.working_choice(), state.working_shown()) else {
        return notice(
            if empty {
                NO_LOCAL_CHANGES
            } else {
                NO_FILE_SELECTED
            },
            false,
        );
    };
    let settings = *view.diff_settings.read();
    let drawn = state.shown_working();
    let acting = submit.clone();
    let hearing = submit.clone();
    // The exact diff, always: what is staged from here is what is drawn.
    let said = view
        .local
        .lines
        .read()
        .says_cleared()
        .then_some(SELECTION_CLEARED);
    let header = DiffHeader::new(header_file(drawn, &choice.path), settings)
        .exact(true)
        .said(said)
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
        WorkingShown::Diff(shown) => {
            diff_body(shown, choice.list, view, submit, settings.side_by_side())
        }
    };
    rect()
        .expanded()
        .content(Content::Flex)
        // The stage and discard chords heard on the diff act on the lines a drag selected, or
        // with none on the file it shows.
        .on_key_down(move |e: Event<KeyboardEventData>| {
            if let Some(action) = accelerators::resolve_key(&e, Scope::LocalChanges) {
                e.stop_propagation();
                actions::on_the_diff(action, view, hearing.as_ref());
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

/// Which of the gesture's sides a path selected in `list` is drawn under.
fn gesture_side(list: ChangeList) -> GestureSide {
    match list {
        ChangeList::Unstaged => GestureSide::Unstaged,
        ChangeList::Staged => GestureSide::Staged,
    }
}

/// What the gesture asks, done: the gesture's own actions and the mode row's alike.
fn gesture_acts(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> EventHandler<GestureAct> {
    EventHandler::new(move |act: GestureAct| {
        actions::on_gesture(act, view, submit.as_ref());
    })
}

/// The primary's diff: its rows under the staging gesture, or the notice that stands in their
/// place — no gesture over a notice, so a conflicted path, a submodule and every state that is
/// not text draw none — with the mode change's row over either when the file's mode changed.
fn diff_body(
    shown: &ShownDiff,
    list: ChangeList,
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
    side_by_side: bool,
) -> Element {
    let side = gesture_side(list);
    let mode_row = match &shown.diff().content {
        // The mode change's own row: over text, and over a change of the mode alone.
        DiffContent::Text { .. } | DiffContent::ModeChangeOnly => ModeRow::of(
            &shown.diff().file,
            side,
            shown.content(),
            gesture_acts(view, submit.clone()),
        ),
        DiffContent::Binary { .. }
        | DiffContent::TooLarge { .. }
        | DiffContent::LfsPointer { .. }
        | DiffContent::Submodule { .. }
        | DiffContent::Conflicted
        | DiffContent::Unsupported { .. } => None,
    };
    let body: Element = if let Some(stands_in) = DiffNotice::of(shown) {
        let loading = submit.clone();
        DiffNoticeView::new(stands_in)
            .on_load(move |()| diff::actions::load_working_anyway(view, loading.as_deref()))
            .into()
    } else {
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
        .gesture(Some(Gesture::new(
            side,
            shown.content(),
            view.local.lines,
            gesture_acts(view, submit),
        )))
        .into()
    };
    rect()
        .expanded()
        .content(Content::Flex)
        .maybe_child(mode_row)
        .child(
            rect()
                .width(Size::fill())
                .height(Size::flex(1.))
                .child(body),
        )
        .into()
}
