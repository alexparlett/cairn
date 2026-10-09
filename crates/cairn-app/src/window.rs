//! The window's contents, drawn from the view state.

use std::rc::Rc;

use cairn_model::{History, RemoteSummary, RowContent, RowId, Secret, Upstream, WorkingTreeStatus};
use cairn_ui::accelerators::{self, HeldKeys, Scope};
use cairn_ui::{
    ChangeCursor, CommitRow, ConfirmDialog, CredentialPrompt, DETAIL_STRIP_HEIGHT, DetailTab,
    DiffSettings, HistoryHeader, HistoryList, MainView, ROW_HEIGHT, RowRender, StatusBox, Tracking,
    current_branch,
};
use freya::prelude::*;

use crate::confirming::Confirming;
use crate::detail_pane::DetailPane;
use crate::diff_state::DiffState;
use crate::fetch_state::{FetchRefusal, FetchStatus, PromptView};
use crate::history_state::{Progress, Status};
use crate::local_changes_pane::LocalChangesPane;
use crate::local_changes_state::LocalChangesView;
use crate::local_writes::LocalWrites;
use crate::refresh_state::RefreshState;
use crate::selection::Pair;
use crate::sidebar_pane::SidebarPane;
use crate::sidebar_state::{SIDEBAR_MIN_WIDTH, SidebarView};
use crate::worker::{Replier, Reply, Request};
use crate::{PAGE_ROWS, ref_find, selection, shortcuts, status_text};

/// The detail pane's height until the splitter is dragged.
pub const PANE_HEIGHT: f32 = 260.0;
/// The least a drag leaves the pane; pulled past it, the pane collapses (Fork's gesture).
const PANE_MIN_HEIGHT: f32 = 90.0;
/// The least the commit list keeps when the window is squeezed.
const LIST_MIN_HEIGHT: f32 = 80.0;
/// The least the main region keeps beside the sidebar when the window is squeezed.
const MAIN_MIN_WIDTH: f32 = 240.0;

/// The view state the window is drawn from. Handles, not values: the window
/// subscribes to what it reads.
#[derive(Clone, Copy)]
pub struct View {
    pub rows: State<History>,
    pub progress: State<Progress>,
    pub selected: State<Option<RowId>>,
    pub fetch: State<FetchStatus>,
    pub prompt: State<Option<PromptView>>,
    pub remotes: State<Vec<RemoteSummary>>,
    /// A fetch the worker refused, until another is asked for.
    pub refused: State<Option<FetchRefusal>>,
    /// The diff selection and the answers kept for it.
    pub diff: State<DiffState>,
    /// The commit list's scroll, shared so a parent link can bring its row into view.
    pub history_scroll: ScrollController,
    /// Where the commit list's selection sits, shared so a row chosen outside the list — a
    /// pressed ref's, a parent link's — is told to it and the next arrow key starts there
    /// rather than searching the loaded rows.
    pub history_cursor: State<usize>,
    /// The detail pane's tab, kept for the session (R5.2).
    pub detail_tab: State<DetailTab>,
    pub pane_collapsed: State<bool>,
    /// The pane's height as last dragged; read when the pane is laid out again.
    pub pane_height: State<f32>,
    /// The settings every diff view shares, kept for the session (R6.1, R6.3).
    pub diff_settings: State<DiffSettings>,
    /// The diff view's scroll, shared so previous and next change can move it.
    pub diff_scroll: ScrollController,
    /// The change previous or next change last moved to.
    pub change_cursor: State<Option<ChangeCursor>>,
    /// The Changes tab's filter, as typed; kept for the session.
    pub filter_text: State<String>,
    /// The Changes tab's file list's share of the pane as last dragged, in percent.
    pub changes_list_width: State<f32>,
    /// Two commits selected to compare (R7), `selected` the one pressed plainly; `None` while
    /// one is selected.
    pub pair: State<Option<Pair>>,
    /// The keys the window hears held, which a press on a row is resolved against (a pointer
    /// press carries no modifiers in this toolkit build): the accelerator table's.
    pub held_keys: State<HeldKeys>,
    /// What the last refresh answered: the refs, ahead/behind and the working tree's status.
    pub refreshed: State<RefreshState>,
    /// What the title bar calls the repository, once the worker has opened it
    /// (`Update::Opened`).
    pub repository: State<Option<String>>,
    /// The sidebar: its rows, the filter, what the main region shows, a press's find.
    pub sidebar: SidebarView,
    /// Local Changes: its lists, its filter and its diff's scroll (refs-and-status R9).
    pub local: LocalChangesView,
    /// The local writes: queued, running and ended, the lock files last listed, and whether
    /// the window waits on one to close (staging-and-commit R4).
    pub writes: State<LocalWrites>,
    /// A destructive operation's confirmation, while one is open: drawn over everything, the
    /// window's chords inert until it is answered (staging-and-commit R7.4).
    pub confirming: State<Option<Confirming>>,
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View")
            .field("fetch", &*self.fetch.read())
            .field("prompt", &*self.prompt.read())
            .field("refused", &*self.refused.read())
            .field("confirming", &*self.confirming.read())
            .finish_non_exhaustive()
    }
}

/// `submit` and `answer` are `None` when no repository could be opened.
pub fn window(
    opened: &str,
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
    answer: Option<Replier>,
) -> Element {
    let status = view.progress.read().status().clone();
    let lanes = view.progress.read().lanes();
    let has_rows = view.progress.read().has_rows();
    let counted = status_text::loaded_count(&view.progress.read());
    let fetch = view.fetch.read().clone();
    let prompt = view.prompt.read().clone();
    let confirming = view.confirming.read().clone();
    // What the window says about the local writes: which it waits on to close (R4.9), and the
    // lock files last listed (R3.8, R4.9).
    let (closing_on, locks) = {
        let writes = view.writes.read();
        (
            status_text::closing_line(&writes),
            status_text::locks_line(writes.locks()),
        )
    };
    let refused = view.refused.read().clone();
    let hearing = submit.clone();
    let erring = submit.clone();
    // The keys held are let go of when the window loses focus: a release made while another
    // window has it is never heard here, and a press after coming back must not be read as a
    // chord still held (the user's decision, 2026-10-04).
    let mut held_keys = view.held_keys;
    use_side_effect(move || {
        if !*Platform::get().is_app_focused.read() {
            held_keys.set(HeldKeys::default());
        }
    });
    // Coming back to the window reads the refs and the working tree again (R10.1).
    crate::refresh::on_focus_gained(submit.clone());
    // A scroll of the list supersedes a find in the sidebar (R8.5): subscribed to the list's
    // scroll alone, and acting only while a find looks.
    let stopping = submit.clone();
    use_side_effect(move || {
        let (_, y): (i32, i32) = view.history_scroll.into();
        let away = view
            .sidebar
            .finding
            .peek()
            .as_ref()
            .is_some_and(|find| ref_find::scrolled_away(find, y));
        if away {
            ref_find::superseded(view, stopping.as_deref());
        }
    });

    let list = rect()
        .width(Size::fill())
        .height(Size::fill())
        .child(HistoryHeader::new())
        .child(match status_text::placeholder(&status, has_rows) {
            Some(message) => notice(message, opened),
            None => history(view, lanes, submit.clone()),
        })
        .maybe(has_rows, |el| match &status {
            Status::Failed(message) => el.child(banner(message.clone(), true)),
            _ => el,
        });

    rect()
        .expanded()
        .theme_background()
        // Every shortcut resolves through the accelerator table (R8): here the ones heard
        // wherever focus is; the detail pane hears its own (`detail_pane`).
        .on_global_key_down(move |e: Event<KeyboardEventData>| {
            let mut held = view.held_keys;
            held.write().heard(&e, true);
            if let Some(action) = accelerators::resolve_key(&e, Scope::Window) {
                shortcuts::act(action, view, hearing.as_deref());
            }
        })
        .on_global_key_up(move |e: Event<KeyboardEventData>| {
            let mut held = view.held_keys;
            held.write().heard(&e, false);
        })
        .child(title_bar(
            status_box(view.repository.read().clone(), &view.refreshed.read()),
            &counted,
            &fetch,
            view.fetch,
            view.refused,
            &view.remotes.read(),
            submit.clone(),
        ))
        .maybe_child(status_text::fetch_line(&fetch).map(|line| {
            let failed = matches!(fetch, FetchStatus::Failed { .. });
            banner(line, failed)
        }))
        .maybe_child(
            refused
                .as_ref()
                .map(|refusal| banner(status_text::refusal_line(refusal), false)),
        )
        .maybe_child(closing_on.map(|line| banner(line, false)))
        .maybe_child(locks.map(|line| banner(line, false)))
        .child(beside(
            SidebarPane::new(view, submit.clone()).into(),
            match *view.sidebar.main.read() {
                MainView::AllCommits => split(list, DetailPane::new(view, submit).into(), view),
                MainView::LocalChanges => LocalChangesPane::new(view, submit).into(),
            },
            view,
        ))
        // A credential prompt stacks over an open confirmation by setting it aside: a write
        // running behind the dialog asked for a secret and waits on it, and the confirmation's
        // modal would keep every key from the prompt. The confirmation stays kept, unanswered,
        // and is drawn again — focus on Cancel — once the prompt is answered or refused
        // (phase 06's QA item 5).
        // A commit's Git Error dialog (staging-and-commit R10.5): its skip may open the amend's
        // confirmation in its place.
        .maybe_child(crate::commit_box_pane::git_error(view, erring).filter(|_| prompt.is_none()))
        .maybe_child(
            confirming
                .filter(|_| prompt.is_none())
                .map(|asking| confirmation(asking, view.confirming)),
        )
        .maybe_child(prompt.map(|prompt| dialog(prompt, view.prompt, answer)))
        // Every context menu opens here, so none panics for want of a host (R7.5).
        .child(ContextMenuViewer::new())
        .into()
}

/// The confirmation a destructive operation waits on (R7.4): its token handed where the
/// asking view said, and the dialog let go of on either answer.
fn confirmation(asking: Confirming, mut showing: State<Option<Confirming>>) -> Element {
    let serial = asking.serial();
    let title = asking.title().to_owned();
    let consequence = asking.consequence().clone();
    // Keyed by its serial: another confirmation is another dialog, never this one's handlers
    // and answered state under new words.
    ConfirmDialog::new(serial, title, consequence)
        .key(DiffKey::U64(serial))
        .on_confirm(move |token| {
            showing.set(None);
            asking.confirmed(token);
        })
        .on_cancel(move |()| showing.set(None))
        .into()
}

/// The sidebar left of the main region, behind a draggable splitter (refs-and-status R8.1).
fn beside(sidebar: Element, main: Element, view: View) -> Element {
    let mut width = view.sidebar.width;
    // Peeked: the width only matters when the split is laid out anew, and reading it would
    // redraw the window on every step of a drag.
    let at = *width.peek();
    ResizableContainer::new()
        .direction(Direction::Horizontal)
        .panel(
            ResizablePanel::new(PanelSize::px(at))
                .min_size(SIDEBAR_MIN_WIDTH)
                .on_resized(move |dragged: f32| width.set(dragged))
                .child(sidebar),
        )
        .panel(
            ResizablePanel::new(PanelSize::percent(100.))
                .min_pixels(MAIN_MIN_WIDTH)
                .child(main),
        )
        .into()
}

/// The commit list over the detail pane, behind a draggable splitter (R5.1); a collapsed
/// pane keeps only its strip, under a list that takes the rest.
fn split(list: Rect, pane: Element, view: View) -> Element {
    let View {
        mut pane_collapsed,
        mut pane_height,
        ..
    } = view;
    if *pane_collapsed.read() {
        return rect()
            .expanded()
            .content(Content::Flex)
            .child(list.height(Size::flex(1.)))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(DETAIL_STRIP_HEIGHT))
                    .child(pane),
            )
            .into();
    }
    // Peeked: the height only matters when the pane is laid out anew, and reading it would
    // redraw the window on every step of a drag.
    let height = *pane_height.peek();
    ResizableContainer::new()
        .direction(Direction::Vertical)
        .panel(
            ResizablePanel::new(PanelSize::percent(100.))
                .min_pixels(LIST_MIN_HEIGHT)
                .child(list),
        )
        .panel(
            ResizablePanel::new(PanelSize::px(height))
                .min_size(PANE_MIN_HEIGHT)
                .on_resized(move |dragged: f32| pane_height.set(dragged))
                .on_collapse(move |()| pane_collapsed.set(true))
                .child(pane),
        )
        .into()
}

/// The credential dialog for `prompt`, answering through `answer` exactly once.
fn dialog(
    prompt: PromptView,
    mut showing: State<Option<PromptView>>,
    answer: Option<Replier>,
) -> Element {
    // Titled by the operation whose token the helper presented — the fetch's remote, or the
    // local write ("Commit is asking for a credential") — whatever else runs beside it, and
    // by git when the token named none.
    let asking = prompt.asking.clone().unwrap_or_else(|| "git".to_owned());
    let id = prompt.id;
    let on_submit = answer.clone();
    let on_cancel = answer;
    CredentialPrompt::new(asking, prompt.text)
        .on_submit(move |typed: String| {
            // The first Cairn-owned type the characters reach; the buffer moves, no copy.
            let secret = Secret::from_string(typed);
            if let Some(answer) = &on_submit {
                answer(Reply::Provide { prompt: id, secret });
            }
            showing.set(None);
        })
        .on_cancel(move |()| {
            if let Some(answer) = &on_cancel {
                answer(Reply::Refuse { prompt: id });
            }
            showing.set(None);
        })
        .into()
}

fn history(view: View, lanes: usize, submit: Option<Rc<dyn Fn(Request)>>) -> Element {
    let mut progress = view.progress;
    let choosing = submit.clone();
    let extending = submit.clone();
    let second = view
        .pair
        .read()
        .as_ref()
        .and_then(|pair| pair.other(*view.selected.read()));
    let refs = view.refreshed.read().refs().cloned();
    HistoryList::new(view.rows, move |render: RowRender| {
        // No wildcard arm: a new row kind must fail to compile here.
        match render.content {
            RowContent::Commit(commit) => CommitRow::new(commit, render.graph, render.lanes)
                .chips(render.chips)
                .head(render.head)
                .selected(render.selected)
                .into(),
            // A stash's row: its `stash@{n}` chip and its message as the subject (R5.4).
            RowContent::Stash(stash) => {
                CommitRow::new(stash.as_commit(), render.graph, render.lanes)
                    .chips(render.chips)
                    .selected(render.selected)
                    .into()
            }
        }
    })
    .refs(refs)
    .lanes(lanes)
    .selected(*view.selected.read())
    .also_selected(second)
    .held(view.held_keys)
    .controller(view.history_scroll)
    .cursor(view.history_cursor)
    // Choosing a row asks what it changed; the pane draws the answer for that row alone. It
    // supersedes a find in the sidebar, and the entry pressed there is let go of.
    .on_select(move |id: RowId| {
        ref_find::row_chosen(view, choosing.as_deref());
        selection::choose(id, view, choosing.as_deref());
    })
    // A row pressed with the table's extending chord is the second commit of a comparison.
    .on_extend(move |(id, index): (RowId, usize)| {
        ref_find::row_chosen(view, extending.as_deref());
        selection::extend(id, index, view, extending.as_deref());
    })
    .on_reach_end(move |()| {
        // A find pages the walk itself; a page asked here would supersede it.
        if view.sidebar.finding.peek().is_some() {
            return;
        }
        // `wants_more` debounces: every `submit` supersedes. `peek`, not `read`: reading here
        // subscribes the window to the progress it writes, and loops.
        if !progress.peek().wants_more() {
            return;
        }
        if let Some(submit) = &submit {
            submit(Request::MoreHistory { rows: PAGE_ROWS });
            progress.write().asked();
        }
    })
    .into()
}

/// The status box for the repository called `name` (R7.1) — none until it is open — `*` while the last status
/// listed a change, the current branch and its distance from its upstream, from what the last
/// refresh answered — each as old as the answer it is read from (a status may be one refresh
/// behind, R10.3 as amended).
fn status_box(name: Option<String>, refreshed: &RefreshState) -> StatusBox {
    let refs = refreshed.refs();
    let head = refs.map(|refs| refs.head.clone());
    let tracking = refs
        .and_then(|refs| {
            let branch = current_branch(&refs.head)?;
            Some(match refs.find(branch)?.upstream.as_ref()? {
                Upstream::Gone { .. } => Tracking::Gone,
                Upstream::Exists { .. } => refreshed
                    .ahead_behind_of(branch)
                    .map_or(Tracking::Untold, Tracking::Counts),
            })
        })
        .unwrap_or(Tracking::Untold);
    let dirty = match refreshed.status() {
        Some(WorkingTreeStatus::Listed(entries)) => !entries.is_empty(),
        Some(WorkingTreeStatus::IndexUnreadable(_) | WorkingTreeStatus::NoWorkingTree) | None => {
            false
        }
    };
    StatusBox::new(name, head).dirty(dirty).tracking(tracking)
}

fn title_bar(
    status: StatusBox,
    counted: &str,
    fetch: &FetchStatus,
    fetch_state: State<FetchStatus>,
    refused: State<Option<FetchRefusal>>,
    remotes: &[RemoteSummary],
    submit: Option<Rc<dyn Fn(Request)>>,
) -> Element {
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .cross_align(Alignment::center())
        .spacing(10.)
        .padding(Gaps::new(8., 12., 8., 12.))
        .child(label().text("Cairn").theme_color().font_size(16.))
        .child(
            rect()
                .width(Size::flex(1.))
                .overflow(Overflow::Clip)
                .child(status),
        )
        .child(
            label()
                .text(counted.to_owned())
                .max_lines(1)
                .font_size(13.)
                .color(get_theme_or_default().read().colors().text_placeholder),
        )
        .maybe_child(fetch_button(fetch, fetch_state, refused, remotes, submit))
        .into()
}

/// "Fetch <remote>" for the default remote while nothing is in flight;
/// "Cancel" while a fetch is; nothing when the repository has no remote to
/// fetch. The press itself marks the fetch as starting, so a second press
/// before the worker answers has no button to land on, and takes down an
/// earlier refusal, which was about a press before this one.
fn fetch_button(
    fetch: &FetchStatus,
    mut fetch_state: State<FetchStatus>,
    mut refused: State<Option<FetchRefusal>>,
    remotes: &[RemoteSummary],
    submit: Option<Rc<dyn Fn(Request)>>,
) -> Option<Element> {
    let submit = submit?;
    if fetch.can_be_cancelled() {
        return Some(
            Button::new()
                .compact()
                .on_press(move |_| {
                    // Said at once: the worker's answer is up to a grace period away.
                    fetch_state.write().cancelling();
                    submit(Request::CancelFetch);
                })
                .child("Cancel")
                .into(),
        );
    }
    if fetch.is_in_flight() {
        // Cancelling: nothing to press until the worker says the process is gone.
        return None;
    }
    let remote = remotes.first()?.name.clone();
    let caption = format!("Fetch {remote}");
    Some(
        Button::new()
            .compact()
            .on_press(move |_| {
                refused.set(None);
                fetch_state.write().starting(remote.clone());
                submit(Request::Fetch {
                    remote: remote.clone(),
                });
            })
            .child(caption)
            .into(),
    )
}

fn notice(message: impl Into<String>, path: &str) -> Element {
    let message = message.into();
    rect()
        .expanded()
        .center()
        .spacing(6.)
        .child(label().text(message).theme_color().font_size(14.))
        .child(
            label()
                .text(path.to_owned())
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(12.)
                .color(get_theme_or_default().read().colors().text_placeholder),
        )
        .into()
}

/// One line across the window; `alarming` draws it in the error colour.
fn banner(message: String, alarming: bool) -> Element {
    let colours = get_theme_or_default();
    let colour = if alarming {
        colours.read().colors().error
    } else {
        colours.read().colors().text_secondary
    };
    rect()
        .width(Size::fill())
        .height(Size::px(ROW_HEIGHT))
        .cross_align(Alignment::center())
        .padding(Gaps::new(0., 12., 0., 12.))
        .background(colours.read().colors().surface_tertiary)
        .child(
            label()
                .text(message)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(13.)
                .color(colour),
        )
        .into()
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use cairn_model::{
        ChangeSet, ChangeStatus, ChangedFile, CommitDetails, Context, FileMode, GraphRow, Lane,
        Oid, PagedCommit, RenameDetection, RepoPath, RowsPage, Signature, Timestamp,
    };
    use cairn_ui::accelerators::Action;
    use cairn_ui::{COLLAPSE_CAPTION, EXPAND_CAPTION};
    use freya_testing::TestingRunner;
    use freya_testing::prelude::{KeyboardEventName, PlatformEvent};

    use crate::detail_pane::{NOTHING_SELECTED, READING};
    use crate::worker::Comparison;

    use crate::fetch_state::{FetchStatus, PromptView};

    use cairn_ui::{COLUMN_GAP, ROW_PADDING, graph_width};

    use super::*;
    use crate::history_state;

    const HEIGHT: f32 = 600.;
    /// Where the main region begins: right of the sidebar at its opening width and the
    /// splitter's handle. The window is as wide again as the main region's 800 px.
    const LEFT: f32 = crate::sidebar_state::SIDEBAR_WIDTH + ResizableContext::HANDLE_SIZE;
    const LEFT_X: f64 = LEFT as f64;
    const WINDOW_WIDTH: f32 = LEFT + 800.;
    const PATH: &str = "/home/ada/engine";

    fn row(n: usize) -> TestRow {
        row_in_lane(n, 0)
    }

    /// Row `n`'s id; its first byte differs between rows, so its short id does too.
    fn oid(n: usize) -> Oid {
        let mut bytes = [0u8; 20];
        bytes[0] = 0x10 + (n % 200) as u8;
        bytes[12..20].copy_from_slice(&(n as u64).to_be_bytes());
        Oid::from_bytes(&bytes).unwrap()
    }

    /// A row as a test writes it: its layout and its subject, `commit n`.
    struct TestRow {
        graph: GraphRow,
        subject: String,
    }

    fn row_in_lane(n: usize, lane: usize) -> TestRow {
        TestRow {
            graph: GraphRow::new(oid(n), Lane::new(lane), Vec::new()),
            subject: format!("commit {n}"),
        }
    }

    /// `rows` as one page from the worker, authored by Ada.
    fn page_of(rows: Vec<TestRow>) -> RowsPage {
        let mut page = RowsPage::new();
        for TestRow { graph, subject } in rows {
            page.push(
                graph,
                PagedCommit {
                    parents: 1,
                    subject: &subject,
                    author: "Ada",
                    author_time: 0,
                },
            );
        }
        page
    }

    /// `rows` appended to `history` as one page.
    fn hold(history: &mut History, rows: Vec<TestRow>) {
        history
            .append(page_of(rows))
            .unwrap_or_else(|full| panic!("{full}"));
    }

    fn held(rows: Vec<TestRow>) -> History {
        let mut history = History::new();
        hold(&mut history, rows);
        history
    }

    type Submitted = Rc<RefCell<Vec<Request>>>;

    /// What was answered: the prompt, and the secret's length (never its bytes) or `None`
    /// for a refusal.
    type Answered = Rc<RefCell<Vec<(crate::worker::PromptId, Option<usize>)>>>;

    fn launch(initial: Vec<TestRow>, progress: Progress) -> (TestingRunner, View, Submitted) {
        let (test, view, submitted, _) = launch_with(initial, progress, FetchStatus::Idle, None);
        (test, view, submitted)
    }

    fn launch_with(
        initial: Vec<TestRow>,
        progress: Progress,
        fetch: FetchStatus,
        prompt: Option<PromptView>,
    ) -> (TestingRunner, View, Submitted, Answered) {
        let submitted = Submitted::default();
        let answered = Answered::default();
        let app = {
            let submitted = submitted.clone();
            let answered = answered.clone();
            move || {
                let view = use_consume::<View>();
                let submitted = submitted.clone();
                let answered = answered.clone();
                let submit: Rc<dyn Fn(Request)> =
                    Rc::new(move |request| submitted.borrow_mut().push(request));
                let answer: Replier = Rc::new(move |answer| {
                    answered.borrow_mut().push(match answer {
                        Reply::Provide { prompt, secret } => (prompt, Some(secret.len())),
                        Reply::Refuse { prompt } => (prompt, None),
                    });
                });
                window(PATH, view, Some(submit), Some(answer))
            }
        };
        let (mut test, view) = TestingRunner::new(
            app,
            (WINDOW_WIDTH, HEIGHT).into(),
            move |runner| {
                runner.provide_root_context(|| test_view(initial, progress, fetch, prompt))
            },
            1.,
        );
        for _ in 0..3 {
            test.sync_and_update();
        }
        (test, view, submitted, answered)
    }

    /// The view state a test window is drawn from: `initial` rows loaded, everything else as
    /// a window opens.
    fn test_view(
        initial: Vec<TestRow>,
        progress: Progress,
        fetch: FetchStatus,
        prompt: Option<PromptView>,
    ) -> View {
        View {
            rows: State::create(held(initial)),
            progress: State::create(progress),
            selected: State::create(None),
            fetch: State::create(fetch),
            prompt: State::create(prompt),
            remotes: State::create(vec![RemoteSummary {
                name: "origin".to_owned(),
                url: Some("https://git.example.com/ada/engine".to_owned()),
            }]),
            refused: State::create(None),
            diff: State::create(DiffState::default()),
            history_scroll: ScrollController::new(0, 0, Vec::new()),
            history_cursor: State::create(0),
            detail_tab: State::create(DetailTab::default()),
            pane_collapsed: State::create(false),
            pane_height: State::create(PANE_HEIGHT),
            diff_settings: State::create(DiffSettings::default()),
            diff_scroll: ScrollController::new(0, 0, Vec::new()),
            change_cursor: State::create(None),
            filter_text: State::create(String::new()),
            changes_list_width: State::create(crate::changes_tab::LIST_WIDTH),
            pair: State::create(None),
            held_keys: State::create(HeldKeys::default()),
            refreshed: State::create(RefreshState::default()),
            repository: State::create(Some("engine".to_owned())),
            sidebar: SidebarView::created(),
            local: crate::local_changes_state::LocalChangesView::created(),
            writes: State::create(crate::local_writes::LocalWrites::default()),
            confirming: State::create(None),
        }
    }

    fn click_label(test: &mut TestingRunner, caption: &str) {
        let centre = test
            .find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == caption)
                    .map(|_| node.layout().area.center())
            })
            .unwrap_or_else(|| panic!("no label reads {caption:?}; labels: {:?}", texts(test)));
        test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
        test.sync_and_update();
    }

    /// Presses the button assistive technology reads as `name`: the diff bar's buttons are
    /// glyphs, named rather than captioned.
    fn click_named(test: &mut TestingRunner, name: &str) {
        let centre = test
            .find(|node, element| {
                Rect::try_downcast(element)
                    .filter(|rect| rect.accessibility.builder.value() == Some(name))
                    .map(|_| node.layout().area.center())
            })
            .unwrap_or_else(|| panic!("no button is named {name:?}"));
        test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
        test.sync_and_update();
    }

    fn texts(test: &TestingRunner) -> Vec<String> {
        test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
    }

    /// The labels below the column headings: what fills the list's place.
    fn body(test: &TestingRunner) -> Vec<String> {
        let headings_end = test
            .find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == "Author")
                    .map(|_| node.layout().area.max_y())
            })
            .unwrap();
        test.find_many(|node, element| {
            Label::try_downcast(element)
                .filter(|_| node.layout().area.min_y() >= headings_end)
                .map(|label| label.text.to_string())
        })
    }

    fn received(loaded: usize, complete: bool) -> Progress {
        let mut progress = Progress::opening();
        progress.received(1, complete, loaded);
        progress
    }

    fn failed(loaded: usize, message: &str) -> Progress {
        let mut progress = received(loaded, false);
        progress.failed(message.to_owned());
        progress
    }

    #[test]
    fn loading_and_an_empty_repository_draw_different_windows() {
        let (loading, _, _) = launch(Vec::new(), Progress::opening());
        let (empty, _, _) = launch(Vec::new(), received(0, true));

        assert!(!body(&loading).is_empty(), "a loading window says nothing");
        assert_ne!(
            body(&loading),
            body(&empty),
            "a loading window and an empty repository's window say the same thing"
        );
    }

    #[test]
    fn every_view_state_puts_its_own_sentence_in_the_window() {
        type Case = (
            &'static str,
            Vec<TestRow>,
            Progress,
            &'static [&'static str],
            &'static [&'static str],
        );
        // (state, rows, progress, under the headings, anywhere in the window)
        let cases: [Case; 5] = [
            (
                "loading",
                Vec::new(),
                Progress::opening(),
                &["Reading history…", PATH],
                &[],
            ),
            (
                "empty",
                Vec::new(),
                received(0, true),
                &["No commits yet.", PATH],
                &["0 commits"],
            ),
            (
                "failed before any rows",
                Vec::new(),
                failed(0, "no git repository at /home/ada/engine"),
                &["no git repository at /home/ada/engine", PATH],
                &[],
            ),
            (
                "failed after rows",
                (0..3).map(row).collect(),
                failed(3, "failed to read commit abc"),
                &["failed to read commit abc", "commit 0"],
                &["3 commits"],
            ),
            (
                "ready",
                (0..3).map(row).collect(),
                received(3, false),
                &["commit 0", "commit 2"],
                &["3 commits…"],
            ),
        ];

        for (state, rows, progress, below, anywhere) in cases {
            let (test, _, _) = launch(rows, progress);
            let (below_headings, everywhere) = (body(&test), texts(&test));
            for sentence in below {
                assert!(
                    below_headings.iter().any(|text| text == sentence),
                    "the {state} window does not show {sentence:?} in the list's place; \
                     it shows {below_headings:?}"
                );
            }
            for sentence in anywhere {
                assert!(
                    everywhere.iter().any(|text| text == sentence),
                    "the {state} window does not show {sentence:?}; it shows {everywhere:?}"
                );
            }
        }
    }

    #[test]
    fn a_failure_with_nothing_loaded_is_said_once() {
        let message = "no git repository at /home/ada/engine";
        let (test, _, _) = launch(Vec::new(), failed(0, message));
        assert_eq!(
            texts(&test).iter().filter(|text| *text == message).count(),
            1,
            "the failure was shown as both the placeholder and a banner"
        );
    }

    #[test]
    fn a_placeholder_replaces_the_list_and_rows_replace_the_placeholder() {
        let (loading, _, _) = launch((0..3).map(row).collect(), Progress::opening());
        assert!(
            !texts(&loading)
                .iter()
                .any(|text| text.starts_with("commit ")),
            "a loading window drew the rows it was holding"
        );

        let (ready, _, _) = launch((0..3).map(row).collect(), received(3, true));
        let shown = texts(&ready);
        for placeholder in ["Reading history…", "No commits yet."] {
            assert!(
                !shown.iter().any(|text| text == placeholder),
                "a ready window still says {placeholder:?}"
            );
        }
    }

    /// `main` when rows were slimmed, and what each of its commits' rows drew in the window
    /// before: subject, author, short id, date, and whether its node is a merge's ring.
    /// Committed with the slimming (`1b0ca9f`); its lines were read by this test run
    /// against a build of `4205d5d`, while every row still carried its full parents and its
    /// author's address, and they agree with `CAIRN_BEFORE` in
    /// `crates/cairn-git/tests/slim_rows.rs`, committed at `4205d5d`
    /// (`the_windows_table_agrees_with_the_engines_table_captured_before`).
    const MAIN_DRAWN_BEFORE: &[(&str, &str, &str, &str, &str, bool)] = &[
        (
            "0cfd746b5a913529bbd1a9684f8e92f1cb364c84",
            "docs(prd): plan the refs-and-status packet, from evidence and a Fork study (#60)",
            "Alexander Parlett",
            "0cfd746",
            "2026-10-06 05:13",
            false,
        ),
        (
            "0a7aeb78b918a341bc21109059a240bc64924644",
            "fix(app): let the worker tests search a short checkout history (#61)",
            "Alexander Parlett",
            "0a7aeb7",
            "2026-10-05 19:36",
            false,
        ),
        (
            "9b6b7d0faf648d82c3da02297afc3777e969b827",
            "diff-engine: show what a commit changed as git answers it, drawn as Fork draws it (#59)",
            "Alexander Parlett",
            "9b6b7d0",
            "2026-10-05 13:41",
            false,
        ),
        (
            "cfe33151ce42afd96a6edfe5e8ee22a6489cde48",
            "process-manager: one place that starts git, and a runner every verb can share (#50)",
            "Alexander Parlett",
            "cfe3315",
            "2026-10-03 05:37",
            false,
        ),
        (
            "4fe7165b6f155b883e3fd960b63d27a91f573a02",
            "docs(prd): plan the process-manager packet, from evidence (#40)",
            "Alexander Parlett",
            "4fe7165",
            "2026-10-02 17:02",
            false,
        ),
        (
            "2987a53d1408bf625c6ae730f81ec4a312040614",
            "docs(docs): keep point-in-time packet state out of design/ (#39)",
            "Alexander Parlett",
            "2987a53",
            "2026-09-23 05:45",
            false,
        ),
        (
            "7d4d9ad7a2e366ca3a3d6277079d493d44c4560f",
            "docs(prd): plan the diff-engine packet, from evidence and a Fork study (#38)",
            "Alexander Parlett",
            "7d4d9ad",
            "2026-09-17 19:03",
            false,
        ),
        (
            "bf93a4eed43422a8f8f7cfbcda29c852d694ad44",
            "credential-prompts: authenticated fetch without Cairn holding a credential (#28)",
            "Alexander Parlett",
            "bf93a4e",
            "2026-09-17 13:57",
            false,
        ),
        (
            "f4f1d0512492d07fa38eb36eb8105e884d79090d",
            "history-graph: draw the commit graph of a real repository (#14)",
            "Alexander Parlett",
            "f4f1d05",
            "2026-09-16 20:58",
            false,
        ),
        (
            "9c93114bfba6a280b4965c42ac56a9079396e7f3",
            "Merge pull request #15 from alexparlett/docs/ui-design",
            "Alexander Parlett",
            "9c93114",
            "2026-09-16 18:42",
            true,
        ),
        (
            "e480fddafc07897e52b1cd9b9a69dd1c9f9fddb1",
            "docs(design): UI design modelled on Fork, with five annotated mockups",
            "Alex Parlett",
            "e480fdd",
            "2026-09-16 18:40",
            false,
        ),
        (
            "7f9c64852879d27ff41ecc2b247141fc8b6258b9",
            "docs(design): stamp the out-of-scope list as reviewed against real usage",
            "Alex Parlett",
            "7f9c648",
            "2026-09-14 20:31",
            false,
        ),
        (
            "b8f51524295a5709d685b0ee4a1dd217b241d457",
            "docs(design): forge links are in scope (D9); the old line conflated two things",
            "Alex Parlett",
            "b8f5152",
            "2026-09-14 20:26",
            false,
        ),
        (
            "1391fb127b2d84dc9aa9f0125e3ca63d066083b7",
            "docs(design): inventory the feature surface, plan the daily-loop program",
            "Alex Parlett",
            "1391fb1",
            "2026-09-14 20:21",
            false,
        ),
        (
            "3c1f0b34a7f47d68a7e689331eb3de1f8d4b4cc2",
            "docs(design): order the packets, and close two gaps in the graph plan",
            "Alex Parlett",
            "3c1f0b3",
            "2026-09-14 20:13",
            false,
        ),
        (
            "43602a3042ed2c6ec38a6039eab5f927cf0d0cc9",
            "docs(design): close four open questions, fix two I framed wrongly",
            "Alex Parlett",
            "43602a3",
            "2026-09-14 19:44",
            false,
        ),
        (
            "d23b4ff446097e9930ca30c16364b279b906bd0e",
            "docs(design): lock decisions D1-D5 and file two packets",
            "Alex Parlett",
            "d23b4ff",
            "2026-09-14 18:25",
            false,
        ),
        (
            "382d6980fe973f4b6e92fbd94651bead5b4f88cd",
            "chore(repo): adopt the agentic harness and scaffold the Cairn workspace",
            "Alex Parlett",
            "382d698",
            "2026-09-14 18:06",
            false,
        ),
    ];

    /// C16, headless, over the Cairn checkout: its rows, read by the worker, applied as the
    /// window applies every page and drawn by the window's own rows, draw what they drew
    /// before rows were slimmed. Caught by: a row reading another row's text or author, a
    /// parent count that is not its commit's, or a page's authors misnumbered in the
    /// history.
    /// The window's table and the engine's, captured at `4205d5d` and committed there: the
    /// same commits, in the same order, with the same subjects and authors, and a ring
    /// exactly where the engine counted more than one parent. Caught by: a table edited
    /// after the rows were slimmed to match what they draw now.
    #[test]
    fn the_windows_table_agrees_with_the_engines_table_captured_before() {
        let engine = include_str!("../../cairn-git/tests/slim_rows.rs");
        let table = engine
            .split_once("const CAIRN_BEFORE: &[Row] = &[")
            .and_then(|(_, rest)| rest.split_once("\n];"))
            .map(|(table, _)| table)
            .unwrap_or_else(|| panic!("no CAIRN_BEFORE table in slim_rows.rs"));
        // Each row: `("id", "subject", "author", time, parents)`; no subject holds a quote.
        let rows: Vec<(String, String, String, usize)> = table
            .split("),")
            .filter(|row| row.contains('"'))
            .map(|row| {
                let quoted: Vec<&str> = row.split('"').skip(1).step_by(2).collect();
                let parents = row
                    .rsplit(',')
                    .find_map(|field| field.trim().trim_end_matches(')').parse().ok())
                    .unwrap_or_else(|| panic!("no parent count in {row}"));
                (
                    quoted[0].to_owned(),
                    quoted[1].to_owned(),
                    quoted[2].to_owned(),
                    parents,
                )
            })
            .collect();
        assert_eq!(rows.len(), MAIN_DRAWN_BEFORE.len());
        for ((id, subject, author, parents), drawn) in rows.iter().zip(MAIN_DRAWN_BEFORE) {
            let (hex, drawn_subject, drawn_author, short, _, ring) = *drawn;
            assert_eq!((id.as_str(), subject.as_str()), (hex, drawn_subject));
            assert_eq!(author, drawn_author, "{hex}");
            assert!(hex.starts_with(short), "{hex}");
            assert_eq!(*parents > 1, ring, "{hex}");
        }
    }

    #[test]
    fn the_cairn_checkouts_rows_draw_what_they_drew_before_rows_were_slimmed() {
        use freya::engine::prelude::{Image, ImageInfo, raster_n32_premul};

        let (handle, mut updates) = crate::worker::checkout();
        let (mut test, view, submitted) = launch(Vec::new(), Progress::opening());
        let submit = {
            let submitted = submitted.clone();
            move |request| submitted.borrow_mut().push(request)
        };
        // Page as the window pages, each page applied as the window applies it, until every
        // commit of `main`'s is held or the history ends: however far `HEAD` has moved on.
        let wanted: Vec<RowId> = MAIN_DRAWN_BEFORE
            .iter()
            .map(|&(hex, ..)| RowId::Commit(Oid::parse(hex).unwrap()))
            .collect();
        handle.submit(Request::OpenHistory { rows: PAGE_ROWS });
        loop {
            let (page, complete) = match crate::worker::next_update(&mut updates) {
                update @ crate::worker::Update::Rows { complete, .. } => (update, complete),
                crate::worker::Update::Failed { message } => {
                    panic!("the history failed: {message}")
                }
                _ => continue,
            };
            test.run_in(|| {
                crate::session::apply(
                    page,
                    view,
                    &crate::session::Worker {
                        submit: &submit,
                        refuse: &|_| {},
                        closing: false,
                    },
                );
            });
            let held = {
                let rows = view.rows.peek();
                wanted.iter().all(|id| rows.position(*id).is_some())
            };
            if held || complete {
                break;
            }
            handle.submit(Request::MoreHistory { rows: PAGE_ROWS });
        }
        test.sync_and_update();

        let graph_left = test
            .find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == "Graph and subject")
                    .map(|_| node.layout().area.min_x())
            })
            .unwrap_or_else(|| panic!("no history header"));
        let mut drawn = Vec::new();
        for &(hex, ..) in MAIN_DRAWN_BEFORE {
            let id = Oid::parse(hex).unwrap();
            let (index, lane) = {
                let rows = view.rows.peek();
                let index = rows
                    .position(RowId::Commit(id))
                    .unwrap_or_else(|| panic!("{hex} is not in the checkout's history"));
                (index, rows.row(index).map(|row| row.lane()).unwrap())
            };
            // Back to the top, then down to the row, so it sits at the top of the list.
            test.scroll((LEFT_X + 100., 200.), (0., 1e6));
            test.scroll(
                (LEFT_X + 100., 200.),
                (0., -(index as f64) * f64::from(ROW_HEIGHT)),
            );
            let short = id.short().to_string();
            let centre_y = test
                .find(|node, element| {
                    Label::try_downcast(element)
                        .filter(|label| label.text == short.as_str())
                        .map(|_| node.layout().area.center().y)
                })
                .unwrap_or_else(|| panic!("row {index} ({hex}) was not drawn"));
            // The chips are phase 07's, and name whatever refs the checkout has today: what
            // the row drew before is its text past them.
            let chip_size = Some(freya::prelude::FontSize::from(cairn_ui::CHIP_FONT_SIZE));
            let mut labels: Vec<(f32, String)> = test.find_many(|node, element| {
                let area = node.layout().area;
                Label::try_downcast(element)
                    .filter(|_| (area.center().y - centre_y).abs() < 1.0)
                    // The history's, not the sidebar's beside it.
                    .filter(|_| area.min_x() >= LEFT)
                    .filter(|label| label.text_style_data.font_size != chip_size)
                    .map(|label| (area.min_x(), label.text.to_string()))
            });
            labels.sort_by(|a, b| a.0.total_cmp(&b.0));

            let png = test.render();
            let image = Image::from_encoded(png).unwrap();
            let size = (WINDOW_WIDTH as i32, HEIGHT as i32);
            let mut surface = raster_n32_premul(size).unwrap();
            surface.canvas().draw_image(&image, (0, 0), None);
            let info = ImageInfo::new_n32_premul(size, None);
            let stride = info.min_row_bytes();
            let mut pixels = vec![0u8; stride * size.1 as usize];
            assert!(surface.read_pixels(&info, &mut pixels, stride, (0, 0)));
            let x = (graph_left - cairn_ui::ROW_PADDING
                + cairn_ui::ROW_PADDING
                + cairn_ui::graph_geometry::lane_x(lane))
            .round() as usize;
            let at = centre_y.round() as usize * stride + x * 4;
            // The node's centre: a dot fills it with its lane's colour, a ring leaves it be.
            let colour = cairn_ui::lane_palette::lane_colour(lane);
            let filled = pixels.get(at..at + 3).is_some_and(|bgr| {
                [bgr[2], bgr[1], bgr[0]]
                    .iter()
                    .zip([colour.r(), colour.g(), colour.b()])
                    .all(|(a, b)| a.abs_diff(b) <= 2)
            });

            let mut line: Vec<String> = labels.into_iter().map(|(_, text)| text).collect();
            line.push((!filled).to_string());
            drawn.push(line);
        }
        let before: Vec<Vec<String>> = MAIN_DRAWN_BEFORE
            .iter()
            .map(|&(_, subject, author, short, date, ring)| {
                vec![
                    subject.to_owned(),
                    author.to_owned(),
                    short.to_owned(),
                    date.to_owned(),
                    ring.to_string(),
                ]
            })
            .collect();
        // As sets of texts: a row whose chips fill its column pushes its subject past the
        // author's (R5.3, as in Fork), which a left-to-right reading would then put second.
        // The columns' own order is `commit_row`'s tests'.
        let sorted = |line: &Vec<String>| {
            let mut line = line.clone();
            line.sort();
            line
        };
        for (index, (now, then)) in drawn.iter().zip(&before).enumerate() {
            assert_eq!(
                sorted(now),
                sorted(then),
                "main's row {index} draws something else"
            );
        }
        assert_eq!(drawn.len(), 18, "main's rows were not all drawn");
        assert!(
            before.iter().any(|line| line[4] == "true"),
            "no merge among main's rows, so no ring was decided"
        );
    }

    #[test]
    fn the_graph_column_is_as_wide_as_the_widest_lane_seen() {
        let page: Vec<TestRow> = (0..3).map(|n| row_in_lane(n, 4)).collect();
        let mut progress = Progress::opening();
        progress.received(
            history_state::widest_lane(&page_of((0..3).map(|n| row_in_lane(n, 4)).collect())),
            true,
            page.len(),
        );
        let (test, _, _) = launch(page, progress);

        let subject_left = test
            .find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == "commit 0")
                    .map(|_| node.layout().area.min_x())
            })
            .unwrap();
        assert_eq!(
            subject_left,
            LEFT + ROW_PADDING + graph_width(5) + COLUMN_GAP,
            "the rows were not drawn with the lanes the history needs"
        );
    }

    #[test]
    fn a_clicked_row_is_drawn_as_selected() {
        let (mut test, _, _) = launch((0..10).map(row).collect(), received(10, true));
        let top = |test: &TestingRunner, text: &str| {
            test.find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == text)
                    .map(|_| node.layout().area.min_y())
            })
            .unwrap()
        };
        let backgrounds = |test: &TestingRunner, y: f32| {
            test.find_many(|node, element| {
                let area = node.layout().area;
                Rect::try_downcast(element)
                    .filter(|_| {
                        area.min_y() <= y && y < area.max_y() && area.height() == ROW_HEIGHT
                    })
                    .map(|rect| rect.style.background)
            })
        };

        let (second, third) = (top(&test, "commit 2"), top(&test, "commit 3"));
        assert!(
            !backgrounds(&test, second).is_empty(),
            "no row found under its subject"
        );
        assert_eq!(backgrounds(&test, second), backgrounds(&test, third));

        test.click_cursor((LEFT_X + 100., second as f64 + 5.));
        assert_ne!(
            backgrounds(&test, second),
            backgrounds(&test, third),
            "the clicked row looks like the one below it"
        );
    }

    #[test]
    fn reaching_the_end_asks_for_one_page_until_that_page_arrives() {
        let first = 200;
        let (mut test, view, submitted) =
            launch((0..first).map(row).collect(), received(first, false));
        let more = Request::MoreHistory { rows: PAGE_ROWS };
        let scroll_to_end = |test: &mut TestingRunner, rows: usize| {
            test.scroll(
                (LEFT_X + 100., 200.),
                (0., -(rows as f64 * ROW_HEIGHT as f64)),
            );
        };

        test.sync_and_update();
        assert!(
            submitted.borrow().is_empty(),
            "the top of the list asked for more"
        );

        scroll_to_end(&mut test, first);
        assert_eq!(submitted.borrow().as_slice(), std::slice::from_ref(&more));

        test.scroll(
            (LEFT_X + 100., 200.),
            (0., first as f64 * ROW_HEIGHT as f64),
        );
        scroll_to_end(&mut test, first);
        assert_eq!(
            submitted.borrow().len(),
            1,
            "a second page was asked for before the first arrived"
        );

        let (mut rows, mut progress) = (view.rows, view.progress);
        hold(&mut rows.write(), (first..first * 2).map(row).collect());
        progress.write().received(1, false, first * 2);
        test.sync_and_update();
        scroll_to_end(&mut test, first * 2);
        assert_eq!(
            submitted.borrow().as_slice(),
            &[more.clone(), more],
            "the end of the page that arrived did not ask for the next"
        );
    }

    #[test]
    fn a_failed_page_is_asked_for_again_on_the_next_approach_to_the_end() {
        let first = 200;
        let (mut test, view, submitted) =
            launch((0..first).map(row).collect(), received(first, false));
        let scroll = |test: &mut TestingRunner, rows: f64| {
            test.scroll((LEFT_X + 100., 200.), (0., -(rows * ROW_HEIGHT as f64)));
        };

        scroll(&mut test, first as f64);
        assert_eq!(submitted.borrow().len(), 1);

        let (mut rows, mut progress) = (view.rows, view.progress);
        progress
            .write()
            .failed("failed to read commit abc".to_owned());
        test.sync_and_update();
        assert_eq!(
            submitted.borrow().len(),
            1,
            "a failure asked again by itself, which loops a broken repository"
        );
        assert!(
            texts(&test)
                .iter()
                .any(|text| text == "failed to read commit abc"),
            "the failure was not shown"
        );

        scroll(&mut test, -(first as f64));
        scroll(&mut test, first as f64);
        assert_eq!(
            submitted.borrow().len(),
            2,
            "coming back to the end after a failed page did not ask for it again"
        );

        hold(&mut rows.write(), (first..first * 2).map(row).collect());
        progress.write().received(1, false, first * 2);
        test.sync_and_update();
        scroll(&mut test, first as f64);
        let shown = texts(&test);
        assert!(
            shown
                .iter()
                .any(|text| text == &format!("commit {}", first * 2 - 1)),
            "the retried page's rows were not drawn: {shown:?}"
        );
        assert!(
            !shown.iter().any(|text| text == "failed to read commit abc"),
            "the failure stayed up after the retry worked"
        );
    }

    /// Staging-and-commit R4.9, R5.1 and C11 drawn: a prompt raised while a local write runs is
    /// titled by the write its token names; the first close asked while it runs says which
    /// write the window waits on and that closing again leaves it unfinished (the user's
    /// decision 12); and the lock files a write's ending or the open named are said, by path.
    /// Caught by: a close that waits in silence, a prompt titled "git", or lock files dropped
    /// between the state and the window.
    #[test]
    fn the_window_names_the_write_it_waits_on_its_prompt_and_the_locks_found() {
        let (mut test, view, _, _) = launch_with(
            (0..3).map(row).collect(),
            received(3, true),
            FetchStatus::Idle,
            Some(PromptView {
                id: crate::worker::PromptId::for_tests(2),
                text: "Enter passphrase for key '/k': ".to_owned(),
                asking: Some("Staging 1 file".to_owned()),
            }),
        );
        let id = crate::worker::OperationId::for_tests(11);
        let mut writes = view.writes;
        writes.write().asked(
            id,
            &crate::worker::LocalWrite::StageFiles {
                paths: vec![cairn_model::RepoPath::from("src/a.rs")],
            },
        );
        writes.write().started(id);
        test.sync_and_update();
        let shown = texts(&test);
        assert!(
            shown
                .iter()
                .any(|t| t == "Staging 1 file is asking for a credential"),
            "the dialog does not name the write: {shown:?}"
        );
        assert!(
            !shown.iter().any(|t| t.starts_with("Finishing")),
            "waiting on a close nobody asked for: {shown:?}"
        );

        writes.write().closing();
        writes
            .write()
            .locks_at_open(vec![std::path::PathBuf::from("/r/.git/index.lock")]);
        test.sync_and_update();
        let shown = texts(&test);
        assert!(
            shown
                .iter()
                .any(|t| t == "Finishing staging 1 file… Closing again leaves it unfinished."),
            "the close does not say which write it waits on: {shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|t| t.starts_with("Lock files remain") && t.ends_with("/r/.git/index.lock")),
            "the lock found is not named: {shown:?}"
        );
    }

    /// PRD product rule: a prompt in the view state is drawn as the dialog naming the
    /// remote being fetched and the URL git asked about, and what is typed leaves as a
    /// secret naming that prompt — once — after which the dialog is gone.
    #[test]
    fn a_prompt_draws_the_dialog_and_its_answer_leaves_as_a_secret_for_that_prompt() {
        let id = crate::worker::PromptId::for_tests(7);
        let (mut test, view, _, answered) = launch_with(
            (0..3).map(row).collect(),
            received(3, true),
            FetchStatus::Running {
                remote: "origin".to_owned(),
                line: None,
            },
            Some(PromptView {
                id,
                text: "Password for 'https://git.example.com/ada/engine': ".to_owned(),
                asking: Some("origin".to_owned()),
            }),
        );
        let shown = texts(&test);
        assert!(
            shown
                .iter()
                .any(|t| t == "origin is asking for a credential"),
            "the dialog itself does not name the remote: {shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|t| t.contains("https://git.example.com/ada/engine")),
            "the dialog does not show the URL: {shown:?}"
        );

        let typed = format!("generated-{}", std::process::id());
        test.write_text(&typed);
        test.press_key(Key::Named(NamedKey::Enter));
        assert_eq!(
            answered.borrow().as_slice(),
            [(id, Some(typed.len()))],
            "the answer did not leave as a secret naming the prompt"
        );
        assert_eq!(
            *view.prompt.read(),
            None,
            "the dialog stayed up after answering"
        );
        assert!(
            !texts(&test).iter().any(|t| t.contains(&typed)),
            "the typed secret is drawn somewhere: {:?}",
            texts(&test)
        );
    }

    /// Staging-and-commit R5.1 and phase 04's QA item 13: with a fetch and a commit both in
    /// flight, a prompt is titled by the operation whose token asked — the commit's hook's,
    /// here — never by whichever is in flight. Caught by: a title read from the fetch in
    /// flight (the guess it replaced named "origin").
    #[test]
    fn a_prompt_is_titled_by_its_own_operation_with_a_fetch_and_a_write_in_flight() {
        let (mut test, view, _, _) = launch_with(
            Vec::new(),
            received(0, true),
            FetchStatus::Running {
                remote: "origin".to_owned(),
                line: None,
            },
            Some(PromptView {
                id: crate::worker::PromptId::for_tests(5),
                text: "Enter passphrase for key '/k': ".to_owned(),
                asking: Some("Commit".to_owned()),
            }),
        );
        let id = crate::worker::OperationId::for_tests(12);
        let mut writes = view.writes;
        writes.write().asked(
            id,
            &crate::worker::LocalWrite::StageFiles {
                paths: vec![cairn_model::RepoPath::from("a")],
            },
        );
        writes.write().started(id);
        test.sync_and_update();
        let shown = texts(&test);
        assert!(
            shown
                .iter()
                .any(|t| t == "Commit is asking for a credential"),
            "the prompt is not titled by its own operation: {shown:?}"
        );
        assert!(
            !shown
                .iter()
                .any(|t| t == "origin is asking for a credential"),
            "{shown:?}"
        );
    }

    #[test]
    fn cancelling_the_dialog_refuses_that_prompt() {
        let id = crate::worker::PromptId::for_tests(3);
        let (mut test, view, _, answered) = launch_with(
            Vec::new(),
            received(0, true),
            FetchStatus::Running {
                remote: "origin".to_owned(),
                line: None,
            },
            Some(PromptView {
                id,
                text: "Username for 'https://git.example.com/x': ".to_owned(),
                asking: Some("origin".to_owned()),
            }),
        );
        click_label(&mut test, "Cancel");
        assert_eq!(answered.borrow().as_slice(), [(id, None)]);
        assert_eq!(*view.prompt.read(), None);
    }

    #[test]
    fn the_fetch_button_fetches_the_default_remote_and_becomes_cancel_while_running() {
        let (mut test, view, submitted) = launch(Vec::new(), received(0, true));
        click_label(&mut test, "Fetch origin");
        assert_eq!(
            submitted.borrow().as_slice(),
            [Request::Fetch {
                remote: "origin".to_owned()
            }]
        );

        // The press itself takes the button away, before the worker answers.
        assert_eq!(
            *view.fetch.read(),
            FetchStatus::Starting {
                remote: "origin".to_owned()
            }
        );
        assert!(
            !texts(&test).iter().any(|t| t == "Fetch origin"),
            "a second fetch could be asked for before the first was confirmed"
        );

        let mut fetch = view.fetch;
        fetch.write().started("origin".to_owned());
        fetch
            .write()
            .progressed("Receiving objects: 40%".to_owned());
        test.sync_and_update();
        let shown = texts(&test);
        assert!(
            shown
                .iter()
                .any(|t| t == "Fetching origin: Receiving objects: 40%"),
            "the progress is not shown: {shown:?}"
        );
        assert!(
            !shown.iter().any(|t| t == "Fetch origin"),
            "a second fetch was offered while one runs"
        );
        click_label(&mut test, "Cancel");
        assert_eq!(submitted.borrow().last(), Some(&Request::CancelFetch));
        // The press says so at once, and takes the button away: the worker may need the
        // runner's whole grace period before it answers.
        assert_eq!(
            *view.fetch.read(),
            FetchStatus::Cancelling {
                remote: "origin".to_owned()
            }
        );
        test.sync_and_update();
        let shown = texts(&test);
        assert!(
            shown.iter().any(|t| t == "Cancelling fetch of origin…"),
            "the cancel was not said: {shown:?}"
        );
        assert!(
            !shown.iter().any(|t| t == "Cancel" || t == "Fetch origin"),
            "a button was offered while the cancel is in progress: {shown:?}"
        );
    }

    #[test]
    fn a_failed_fetch_is_said_in_the_window_and_the_button_comes_back() {
        let (test, _, _, _) = launch_with(
            (0..3).map(row).collect(),
            received(3, true),
            FetchStatus::Failed {
                remote: "origin".to_owned(),
                message: "could not read Username for 'https://x': terminal prompts disabled"
                    .to_owned(),
            },
            None,
        );
        let shown = texts(&test);
        assert!(
            shown
                .iter()
                .any(|t| t.contains("Fetch of origin failed") && t.contains("could not read")),
            "{shown:?}"
        );
        assert!(shown.iter().any(|t| t == "Fetch origin"), "{shown:?}");
        assert!(
            shown.iter().any(|t| t == "commit 0"),
            "the rows were lost: {shown:?}"
        );
    }

    /// PRD R7.2, G15's window half: a refused fetch is drawn as a reason, beside the fetch
    /// in flight, which goes on showing; and the next press takes it down. Caught by: not
    /// drawing the refusal, drawing it in place of the running fetch, or leaving it up
    /// after the user has asked again.
    #[test]
    fn a_refused_fetch_is_drawn_with_its_reason_until_the_next_press() {
        let (mut test, view, submitted, _) = launch_with(
            (0..3).map(row).collect(),
            received(3, true),
            FetchStatus::Running {
                remote: "origin".to_owned(),
                line: Some("Receiving objects: 40%".to_owned()),
            },
            None,
        );
        let mut refused = view.refused;
        refused.set(Some(FetchRefusal {
            remote: "origin".to_owned(),
            reason: "a fetch of origin is already running".to_owned(),
        }));
        test.sync_and_update();
        let shown = texts(&test);
        assert!(
            shown
                .iter()
                .any(|t| t == "Fetch of origin not started: a fetch of origin is already running"),
            "the refusal is not drawn: {shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|t| t == "Fetching origin: Receiving objects: 40%"),
            "the refusal took the running fetch's place: {shown:?}"
        );

        let mut fetch = view.fetch;
        fetch.set(FetchStatus::Finished {
            remote: "origin".to_owned(),
        });
        test.sync_and_update();
        click_label(&mut test, "Fetch origin");
        assert_eq!(*view.refused.read(), None, "the press left the refusal up");
        assert!(
            !texts(&test).iter().any(|t| t.contains("not started")),
            "{:?}",
            texts(&test)
        );
        assert_eq!(
            submitted.borrow().last(),
            Some(&Request::Fetch {
                remote: "origin".to_owned()
            })
        );
    }

    /// The dialog's fallback for a prompt whose token named no operation: it still names what
    /// asked.
    #[test]
    fn a_prompt_naming_no_operation_is_attributed_to_git() {
        let (test, _, _, _) = launch_with(
            Vec::new(),
            received(0, true),
            FetchStatus::Idle,
            Some(PromptView {
                id: crate::worker::PromptId::for_tests(2),
                text: "Username for 'https://git.example.com/x': ".to_owned(),
                asking: None,
            }),
        );
        assert!(
            texts(&test)
                .iter()
                .any(|t| t == "git is asking for a credential"),
            "{:?}",
            texts(&test)
        );
    }

    /// Row `n`'s change set: its details, with `parents`, and one file named for it.
    fn answer_for(n: usize, parents: Vec<Oid>) -> ChangeSet {
        let signature = |name: &str| Signature {
            name: name.to_owned(),
            email: format!("{}@example.com", name.to_lowercase()),
            time: Timestamp::new(1_700_000_000, 0),
        };
        ChangeSet {
            files: vec![ChangedFile {
                status: ChangeStatus::Modified,
                old_path: RepoPath::from(format!("file-of-{n}.rs").as_str()),
                new_path: RepoPath::from(format!("file-of-{n}.rs").as_str()),
                old_mode: Some(FileMode::Regular),
                new_mode: Some(FileMode::Regular),
                old_id: None,
                new_id: None,
            }],
            details: Some(CommitDetails {
                id: oid(n),
                parents,
                author: signature("Ada"),
                committer: signature("Grace"),
                message: format!("subject of {n}\n"),
            }),
            renames: RenameDetection::default(),
        }
    }

    fn arrives(test: &mut TestingRunner, view: View, n: usize, parents: Vec<Oid>) {
        let mut diff = view.diff;
        let of = Comparison::Commit(oid(n));
        test.run_in(|| diff.write().changes_arrived(of, answer_for(n, parents)));
        test.sync_and_update();
        test.sync_and_update();
    }

    /// The top of the detail pane: where its strip's Changes tab sits (the history's column
    /// headings have a "Commit" of their own).
    fn pane_top(test: &TestingRunner) -> f32 {
        test.find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == DetailTab::Changes.caption())
                .map(|_| node.layout().area.min_y())
        })
        .unwrap()
    }

    /// The labels drawn in the detail pane.
    fn pane(test: &TestingRunner) -> Vec<String> {
        let top = pane_top(test);
        test.find_many(|node, element| {
            Label::try_downcast(element)
                .filter(|_| node.layout().area.min_y() > top && node.is_visible())
                .map(|label| label.text.to_string())
        })
    }

    fn click_row(test: &mut TestingRunner, n: usize) {
        click_label(test, &format!("commit {n}"));
        test.sync_and_update();
    }

    /// Presses `action`'s chord on this platform, as a key press reaches the window; for the
    /// other window tests too, so no other test file names a modifier.
    pub(crate) fn press_chord(test: &mut TestingRunner, action: Action) {
        let chord = accelerators::chords(action, accelerators::Os::current())
            .first()
            .unwrap();
        let (key, code, modifiers) = chord.key_press().unwrap();
        test.send_event(PlatformEvent::Keyboard {
            name: KeyboardEventName::KeyDown,
            key,
            code,
            modifiers,
        });
        test.sync_and_update();
        test.sync_and_update();
    }

    /// R4.4 at the place it is drawn, through the window: choosing a row asks what it changed
    /// and the Commit tab draws that row's answer alone — not the last row's while the next
    /// one's is on its way, and not one the diff state holds for a row no longer selected.
    /// Choosing the same row again asks nothing new. Caught by: a pane that draws whatever
    /// change set is kept, or a selection that does not ask.
    #[test]
    fn the_commit_tab_draws_the_answer_for_the_row_selected_and_no_other() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        assert!(
            pane(&test).iter().any(|t| t == NOTHING_SELECTED),
            "{:?}",
            pane(&test)
        );

        click_row(&mut test, 2);
        let two = Comparison::Commit(oid(2));
        assert_eq!(
            submitted.borrow().as_slice(),
            [Request::Changes { of: two }]
        );
        assert!(
            pane(&test).iter().any(|t| t == READING),
            "{:?}",
            pane(&test)
        );

        arrives(&mut test, view, 2, Vec::new());
        assert!(
            pane(&test).iter().any(|t| t == "subject of 2"),
            "{:?}",
            pane(&test)
        );
        assert!(
            pane(&test).iter().any(|t| t == "file-of-2.rs"),
            "{:?}",
            pane(&test)
        );

        click_row(&mut test, 2);
        assert_eq!(
            submitted.borrow().len(),
            1,
            "choosing the same row asked again"
        );

        click_row(&mut test, 3);
        // The query, then the answer it replaces, handed to the worker to free (R2).
        match &submitted.borrow()[1..] {
            [asked, Request::Retire(retired)] => {
                assert_eq!(
                    *asked,
                    Request::Changes {
                        of: Comparison::Commit(oid(3))
                    }
                );
                assert_eq!(retired.changes(), Some(&answer_for(2, Vec::new())));
            }
            other => panic!("expected the query and a retirement, got {other:?}"),
        }
        let shown = pane(&test);
        assert!(shown.iter().any(|t| t == READING), "{shown:?}");
        assert!(
            !shown
                .iter()
                .any(|t| t == "subject of 2" || t == "file-of-2.rs"),
            "the last row's answer is drawn under the next: {shown:?}"
        );

        // The diff state answered for row 3, but the window has row 4 selected: whatever is
        // kept is another selection's, and is not drawn.
        arrives(&mut test, view, 3, Vec::new());
        let mut selected = view.selected;
        selected.set(Some(RowId::Commit(oid(4))));
        test.sync_and_update();
        let shown = pane(&test);
        assert!(
            !shown
                .iter()
                .any(|t| t == "subject of 3" || t == "file-of-3.rs"),
            "row 3's answer is drawn with row 4 selected: {shown:?}"
        );
    }

    /// C10, R5.2: Commit is the default tab, and the tab chosen is kept for the session —
    /// across another row chosen, and across a collapse, which leaves the strip to open the
    /// pane again. Caught by: a selection resetting the tab, or a collapse forgetting it.
    #[test]
    fn the_tab_chosen_is_kept_across_selections_and_a_collapse() {
        let (mut test, view, _) = launch((0..10).map(row).collect(), received(10, true));
        assert_eq!(*view.detail_tab.read(), DetailTab::Commit);

        click_label(&mut test, DetailTab::Changes.caption());
        assert!(
            pane(&test).iter().any(|t| t == NOTHING_SELECTED),
            "{:?}",
            pane(&test)
        );

        click_row(&mut test, 1);
        click_row(&mut test, 5);
        assert_eq!(*view.detail_tab.read(), DetailTab::Changes);
        assert!(
            pane(&test).iter().any(|t| t == READING),
            "{:?}",
            pane(&test)
        );

        let open_top = pane_top(&test);
        click_label(&mut test, COLLAPSE_CAPTION);

        assert!(*view.pane_collapsed.read());
        assert!(
            pane_top(&test) > open_top,
            "the collapsed pane did not give the list its room"
        );
        assert!(!texts(&test).iter().any(|t| t == READING));

        click_label(&mut test, EXPAND_CAPTION);
        assert_eq!(*view.detail_tab.read(), DetailTab::Changes);
        assert!(
            pane(&test).iter().any(|t| t == READING),
            "{:?}",
            pane(&test)
        );
    }

    /// Q3: while a credential prompt is up no accelerator acts — the dialog owns the keys
    /// until it is answered — and once it is gone they act again. Caught by: a window that
    /// acts on a chord typed into the dialog.
    #[test]
    fn no_accelerator_acts_while_a_credential_prompt_is_up() {
        let prompt = PromptView {
            id: crate::worker::PromptId::for_tests(1),
            text: "Password for 'https://ada@git.example.com': ".to_owned(),
            asking: None,
        };
        let (mut test, view, _, _) = launch_with(
            (0..10).map(row).collect(),
            received(10, true),
            FetchStatus::Idle,
            Some(prompt),
        );
        // In the field, which keeps what it is typed; and outside it, in the dialog's text,
        // where nothing else would stop the chord.
        press_chord(&mut test, Action::ShowChangesTab);
        click_label(&mut test, "git is asking for a credential");
        press_chord(&mut test, Action::ShowChangesTab);
        assert_eq!(
            *view.detail_tab.read(),
            DetailTab::Commit,
            "a chord acted under the credential prompt"
        );

        let mut prompt = view.prompt;
        prompt.set(None);
        test.sync_and_update();
        press_chord(&mut test, Action::ShowChangesTab);
        assert_eq!(*view.detail_tab.read(), DetailTab::Changes);
    }

    /// User decision 6, Fork's model through the window: Tab takes focus from the history
    /// into the Commit tab's files, whose ↑ and ↓ then move the current file — bringing it
    /// into view — and not the commit; Shift-Tab gives the history its arrows back. Caught
    /// by: a Commit tab Tab cannot reach, arrows that move the commit while the files have
    /// focus, or a focus that does not come back.
    #[test]
    fn tab_moves_the_arrows_between_the_history_and_the_commits_files() {
        let (mut test, view, _) = launch((0..10).map(row).collect(), received(10, true));
        click_row(&mut test, 2);
        let mut answer = answer_for(2, Vec::new());
        answer.files = (0..30)
            .map(|n| ChangedFile {
                old_path: RepoPath::from(format!("many-{n:02}.rs").as_str()),
                new_path: RepoPath::from(format!("many-{n:02}.rs").as_str()),
                ..answer.files[0].clone()
            })
            .collect();
        let mut diff = view.diff;
        test.run_in(|| {
            diff.write()
                .changes_arrived(Comparison::Commit(oid(2)), answer)
        });
        test.sync_and_update();
        test.sync_and_update();
        assert!(
            !pane(&test).iter().any(|t| t == "many-25.rs"),
            "{:?}",
            pane(&test)
        );

        let tab = |test: &mut TestingRunner, held: Modifiers| {
            test.press_key_with_modifiers(Key::Named(NamedKey::Tab), held);
            test.sync_and_update();
            test.sync_and_update();
        };
        let arrow = |test: &mut TestingRunner| {
            test.press_key(Key::Named(NamedKey::ArrowDown));
            test.sync_and_update();
            test.sync_and_update();
        };
        // The history list, then the strip's Collapse control, then the files.
        tab(&mut test, Modifiers::empty());
        tab(&mut test, Modifiers::empty());
        for _ in 0..26 {
            arrow(&mut test);
        }
        assert_eq!(
            *view.selected.read(),
            Some(RowId::Commit(oid(2))),
            "the arrows moved the commit while the files had focus"
        );
        assert!(
            pane(&test).iter().any(|t| t == "many-25.rs"),
            "the current file was not brought into view: {:?}",
            pane(&test)
        );

        tab(&mut test, Modifiers::SHIFT);
        tab(&mut test, Modifiers::SHIFT);
        arrow(&mut test);
        assert_eq!(
            *view.selected.read(),
            Some(RowId::Commit(oid(3))),
            "Shift-Tab did not give the history its arrows back"
        );
    }

    /// C13: the tab chords resolve through the accelerator table and act in the window, a
    /// collapsed pane opening for the tab asked for. Caught by: a window that resolves no
    /// chord, or acts on one by its literal keys.
    #[test]
    fn the_tab_chords_resolve_through_the_table() {
        let (mut test, view, _) = launch((0..10).map(row).collect(), received(10, true));
        press_chord(&mut test, Action::ShowChangesTab);
        assert_eq!(*view.detail_tab.read(), DetailTab::Changes);

        let mut collapsed = view.pane_collapsed;
        collapsed.set(true);
        test.sync_and_update();
        press_chord(&mut test, Action::ShowCommitTab);
        assert_eq!(*view.detail_tab.read(), DetailTab::Commit);
        assert!(
            !*view.pane_collapsed.read(),
            "the chord left the pane collapsed"
        );
        assert!(
            pane(&test).iter().any(|t| t == NOTHING_SELECTED),
            "{:?}",
            pane(&test)
        );
    }

    /// R5.3: a loaded parent's link selects its row, asks what it changed and brings it into
    /// view; an unloaded parent's does nothing visible (reaching it is issue #3). Caught by:
    /// a link that selects whatever is at some index, or one that asks for a parent the
    /// list cannot show.
    #[test]
    fn a_parent_link_selects_a_loaded_parent_and_ignores_an_unloaded_one() {
        let (mut test, view, submitted) = launch((0..60).map(row).collect(), received(60, true));
        click_row(&mut test, 1);
        let unloaded = Oid::from_bytes(&[0xee; 20]).unwrap();
        arrives(&mut test, view, 1, vec![oid(45), unloaded]);
        let before = (submitted.borrow().len(), *view.selected.read());

        let link = |test: &TestingRunner, short: String| {
            let top = pane_top(test);
            test.find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == short && node.layout().area.min_y() > top)
                    .map(|_| node.layout().area.center())
            })
            .unwrap()
        };
        let centre = link(&test, unloaded.short().as_str().to_owned());
        test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
        test.sync_and_update();
        assert_eq!(
            (submitted.borrow().len(), *view.selected.read()),
            before,
            "an unloaded parent's link changed something"
        );
        assert!(pane(&test).iter().any(|t| t == "subject of 1"));

        let row_45_visible = |test: &TestingRunner| {
            test.find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == "commit 45")
                    .map(|_| node.is_visible() && node.layout().area.max_y() <= pane_top(test))
            })
            .unwrap_or(false)
        };
        assert!(
            !row_45_visible(&test),
            "the parent's row was in view already"
        );

        let centre = link(&test, oid(45).short().as_str().to_owned());
        test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
        test.sync_and_update();
        test.sync_and_update();
        assert_eq!(*view.selected.read(), Some(RowId::Commit(oid(45))));
        // Told to the list, so its next arrow key starts at the parent without a search.
        assert_eq!(*view.history_cursor.read(), 45);
        let asked: Vec<Request> = submitted
            .borrow()
            .iter()
            .filter(|request| !matches!(request, Request::Retire(_)))
            .cloned()
            .collect();
        assert_eq!(
            asked.last(),
            Some(&Request::Changes {
                of: Comparison::Commit(oid(45))
            })
        );
        assert!(
            row_45_visible(&test),
            "the parent's row was not brought into view"
        );
    }

    /// R5.1: the splitter between the list and the pane drags, and the height dragged to is
    /// the one the pane opens at again after a collapse. Caught by: a fixed pane, or one
    /// that forgets its height when it collapses.
    #[test]
    fn the_splitter_drags_and_the_pane_keeps_its_height() {
        let (mut test, view, _) = launch((0..10).map(row).collect(), received(10, true));
        let top = pane_top(&test);
        // The handle: the one full-width rect as thin as Freya's splitter.
        let handle = test
            .find(|node, element| {
                let area = node.layout().area;
                Rect::try_downcast(element)
                    .filter(|_| {
                        area.height() == ResizableContext::HANDLE_SIZE && area.width() > 700.
                    })
                    .map(|_| f64::from(area.center().y))
            })
            .unwrap();
        test.press_cursor((LEFT_X + 300., handle));
        test.move_cursor((LEFT_X + 300., handle - 80.));
        test.sync_and_update();
        test.move_cursor((LEFT_X + 300., handle - 120.));
        test.sync_and_update();
        test.release_cursor((LEFT_X + 300., handle - 120.));
        test.sync_and_update();
        let dragged = pane_top(&test);
        assert!(
            dragged < top - 60.,
            "dragging the splitter up did not grow the pane: {top} then {dragged}"
        );

        click_label(&mut test, COLLAPSE_CAPTION);
        click_label(&mut test, EXPAND_CAPTION);
        test.sync_and_update();
        assert!(
            (pane_top(&test) - dragged).abs() < 2.,
            "the pane opened at {} rather than where it was dragged to, {dragged}",
            pane_top(&test)
        );
        assert!(*view.pane_height.read() > PANE_HEIGHT);
    }

    /// Row `n`'s one file, `file-of-{n}.rs`, as a text diff of `lines` lines with one line in
    /// every ten replaced.
    fn text_answer(n: usize, lines: u32) -> cairn_model::FileDiff {
        use cairn_model::{
            ChangedRange, DiffContent, DiffLine, DisplayOverlay, LineSpan, TextDiff,
        };
        let old: Vec<DiffLine> = (0..lines)
            .map(|k| DiffLine::terminated(format!("line {k}")))
            .collect();
        let new: Vec<DiffLine> = (0..lines)
            .map(|k| {
                DiffLine::terminated(if k % 10 == 5 {
                    format!("LINE {k}")
                } else {
                    format!("line {k}")
                })
            })
            .collect();
        let changes = (0..lines / 10)
            .map(|k| ChangedRange::new(LineSpan::at(k * 10 + 5, 1), LineSpan::at(k * 10 + 5, 1)))
            .collect();
        cairn_model::FileDiff {
            file: answer_for(n, Vec::new()).files.remove(0),
            content: DiffContent::Text {
                text: TextDiff::new(old, new, changes),
                overlay: DisplayOverlay::none(),
            },
        }
    }

    /// The text paragraphs drawn in the pane: the diff's rows.
    fn pane_rows(test: &TestingRunner) -> Vec<String> {
        let top = pane_top(test);
        test.find_many(|node, element| {
            Paragraph::try_downcast(element)
                .filter(|_| node.layout().area.min_y() > top && node.is_visible())
                .map(|paragraph| {
                    paragraph
                        .spans
                        .iter()
                        .map(|span| span.text.as_ref())
                        .collect()
                })
        })
    }

    /// The file query asked last.
    fn last_file_query(submitted: &Submitted) -> crate::worker::FileQuery {
        submitted
            .borrow()
            .iter()
            .rev()
            .find_map(|request| match request {
                Request::FileDiff(query) => Some(query.clone()),
                _ => None,
            })
            .expect("a file's diff was asked")
    }

    /// Row 2 chosen, its change set arrived, and the Changes tab shown, which chooses its
    /// one file as Fork chooses the first (phase 08: a press in the Commit tab opens a file in
    /// place instead, and the Changes tab keeps a file of its own).
    fn choose_the_file(test: &mut TestingRunner, view: View) {
        click_row(test, 2);
        arrives(test, view, 2, Vec::new());
        click_label(test, DetailTab::Changes.caption());
        test.sync_and_update();
    }

    fn answer_file(
        test: &mut TestingRunner,
        view: View,
        query: &crate::worker::FileQuery,
        lines: u32,
    ) {
        let mut diff = view.diff;
        test.run_in(|| {
            diff.write().file_arrived(
                query,
                Some(cairn_model::ShownDiff::new(
                    text_answer(2, lines),
                    query.options.context,
                )),
            )
        });
        test.sync_and_update();
        test.sync_and_update();
    }

    /// Phase 06 wiring, R4.4: the Changes tab's file is asked at the session's settings,
    /// says it is reading, then draws the answer's rows under the bar naming the file — and an
    /// answer naming the file at other options is not drawn. Caught by: a file chosen that
    /// asks nothing, an answer drawn for a query no longer selected, or a diff drawn from
    /// anything but `DiffState`.
    #[test]
    fn the_changes_tabs_file_draws_its_diff_for_that_query_alone() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        choose_the_file(&mut test, view);
        let query = last_file_query(&submitted);
        assert_eq!(
            query.options,
            crate::diff_actions::options(DiffSettings::default())
        );
        assert_eq!(*view.detail_tab.read(), DetailTab::Changes);
        assert!(
            pane(&test)
                .iter()
                .any(|t| t == crate::changes_tab::READING_DIFF),
            "{:?}",
            pane(&test)
        );

        let mut stale = query.clone();
        stale.options.ignore_whitespace = true;
        answer_file(&mut test, view, &stale, 40);
        assert!(
            pane_rows(&test).iter().all(|t| !t.starts_with("@@")),
            "an answer at other options was drawn"
        );

        answer_file(&mut test, view, &query, 40);
        let rows = pane_rows(&test);
        assert!(
            rows.iter().any(|t| t == "file-of-2.rs"),
            "the bar does not name the file: {rows:?}"
        );
        assert!(rows.iter().any(|t| t.starts_with("@@ -3,")), "{rows:?}");
        assert!(rows.iter().any(|t| t == "LINE 5"), "{rows:?}");
    }

    /// The files opened in place the request asked last names, and whether it asked for
    /// Expand All.
    fn last_expansion(submitted: &Submitted) -> crate::worker::ExpandQuery {
        submitted
            .borrow()
            .iter()
            .rev()
            .find_map(|request| match request {
                Request::Expand(asked) => Some(asked.clone()),
                _ => None,
            })
            .expect("files opened in place were asked")
    }

    /// One file of a page of files opened in place, read by Expand All or by name.
    fn opened_file(
        index: usize,
        diff: cairn_model::FileDiff,
        by_all: bool,
    ) -> crate::worker::ExpandedFile {
        crate::worker::ExpandedFile {
            file: crate::worker::OpenedFile {
                index,
                load_anyway: false,
            },
            by_all,
            outcome: Ok(Box::new(cairn_model::ShownDiff::new(
                diff,
                crate::diff_actions::options(DiffSettings::default()).context,
            ))),
        }
    }

    /// C10, R5.3 through the window (Fork, Finding 4): a file pressed in the Commit tab asks
    /// for its diff in place — the Commit tab stays shown — says it is reading under its row,
    /// then draws the answer's rows there; pressed again, it closes, and what it drew is
    /// handed to a worker. Caught by: a press that switches to the Changes tab (phase 06's
    /// wiring), asks nothing, or draws the rows anywhere but the Commit tab.
    #[test]
    fn a_file_pressed_in_the_commit_tab_opens_its_diff_in_place() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        click_row(&mut test, 2);
        arrives(&mut test, view, 2, Vec::new());
        click_label(&mut test, "file-of-2.rs");
        test.sync_and_update();
        assert_eq!(*view.detail_tab.read(), DetailTab::Commit);
        let asked = last_expansion(&submitted);
        assert_eq!(asked.of, Comparison::Commit(oid(2)));
        assert_eq!(
            asked.files,
            [crate::worker::OpenedFile {
                index: 0,
                load_anyway: false
            }]
        );
        assert_eq!(asked.all, None);
        assert!(
            pane(&test).iter().any(|t| t == cairn_ui::READING_DIFF),
            "{:?}",
            pane(&test)
        );

        let mut diff = view.diff;
        test.run_in(|| {
            diff.write()
                .expansion_arrived(vec![opened_file(0, text_answer(2, 40), false)], None)
        });
        test.sync_and_update();
        test.sync_and_update();
        // The pane is short: the hunk's header is what shows under the file's row.
        let rows = pane_rows(&test);
        assert!(rows.iter().any(|t| t.starts_with("@@ -3,")), "{rows:?}");
        assert_eq!(*view.detail_tab.read(), DetailTab::Commit);

        let before = submitted.borrow().len();
        click_label(&mut test, "file-of-2.rs");
        test.sync_and_update();
        assert!(
            !pane_rows(&test).iter().any(|t| t.starts_with("@@")),
            "the file did not close"
        );
        assert!(
            submitted.borrow()[before..]
                .iter()
                .any(|r| matches!(r, Request::Retire(_))),
            "what the closed file drew was not handed to a worker"
        );
    }

    /// C10, R5.3 through the window: Expand All asks from the first file; each page opens its
    /// files under their rows; when Expand All stops with its budget spent the tab says how
    /// many files stay collapsed, and the button is Collapse All, which closes every file and
    /// ends what is in flight. Caught by: Expand All asking nothing, a page not drawn, the
    /// budget's end not said, or Collapse All leaving files open.
    #[test]
    fn expand_all_stops_at_its_budget_and_says_how_many_files_stay_collapsed() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        click_row(&mut test, 2);
        let mut diff = view.diff;
        test.run_in(|| {
            diff.write()
                .changes_arrived(Comparison::Commit(oid(2)), answer_with(2, 6))
        });
        test.sync_and_update();
        test.sync_and_update();
        click_label(&mut test, cairn_ui::EXPAND_ALL_CAPTION);
        let asked = last_expansion(&submitted);
        assert_eq!(asked.all, Some(crate::worker::AllFrom::default()));

        let file = |k: usize| {
            let mut answer = text_answer(2, 12);
            answer.file = answer_with(2, 6).files.remove(k);
            answer
        };
        test.run_in(|| {
            diff.write().expansion_arrived(
                vec![opened_file(0, file(0), true), opened_file(1, file(1), true)],
                Some(crate::worker::AllProgress {
                    at: crate::worker::AllFrom { next: 2, spent: 50 },
                    ended: Some(crate::worker::AllEnded::Budget),
                }),
            )
        });
        test.sync_and_update();
        test.sync_and_update();
        let shown = pane(&test);
        assert!(
            shown.iter().any(|t| *t == cairn_ui::budget_notice(4)),
            "the budget's notice is not drawn: {shown:?}"
        );
        assert!(shown.iter().any(|t| t == cairn_ui::COLLAPSE_ALL_CAPTION));
        assert!(pane_rows(&test).iter().any(|t| t.starts_with("@@")));

        let before = submitted.borrow().len();
        click_label(&mut test, cairn_ui::COLLAPSE_ALL_CAPTION);
        test.sync_and_update();
        assert!(!pane_rows(&test).iter().any(|t| t.starts_with("@@")));
        let superseding = last_expansion(&submitted);
        assert!(
            submitted.borrow().len() > before
                && superseding.files.is_empty()
                && superseding.all.is_none(),
            "Collapse All did not end what was in flight: {superseding:?}"
        );
        assert!(
            pane(&test)
                .iter()
                .any(|t| t == cairn_ui::EXPAND_ALL_CAPTION)
        );
    }

    /// Holds (`down`) or lets go of the keys of the table's extending chord — ⌘ on macOS,
    /// Ctrl elsewhere — through the table, as the window hears them.
    fn hold_extending(test: &mut TestingRunner, down: bool) {
        let chord = accelerators::chords(Action::ExtendSelection, accelerators::Os::current())
            .first()
            .and_then(|chord| chord.press_hold())
            .expect("the extending chord is a press");
        let (key, code, modifiers) = chord;
        test.send_event(PlatformEvent::Keyboard {
            name: if down {
                KeyboardEventName::KeyDown
            } else {
                KeyboardEventName::KeyUp
            },
            key,
            code,
            modifiers: if down { modifiers } else { Default::default() },
        });
        test.sync_and_update();
    }

    /// Row `n` pressed with the extending chord held: ⌘-click or Ctrl-click.
    fn extend_row(test: &mut TestingRunner, n: usize) {
        hold_extending(test, true);
        click_row(test, n);
        hold_extending(test, false);
    }

    /// The changes queries asked since `from`.
    fn changes_asked(submitted: &Submitted, from: usize) -> Vec<Comparison> {
        submitted.borrow()[from..]
            .iter()
            .filter_map(|request| match request {
                Request::Changes { of } => Some(*of),
                _ => None,
            })
            .collect()
    }

    fn between(base: usize, tip: usize) -> Comparison {
        Comparison::Between {
            old: oid(base),
            new: oid(tip),
        }
    }

    /// The two commits selected, as the window keeps them: the one pressed plainly, and the
    /// other of a pair.
    fn selection(view: View) -> (Option<RowId>, Option<RowId>) {
        let selected = *view.selected.peek();
        let other = view
            .pair
            .peek()
            .as_ref()
            .and_then(|pair| pair.other(selected));
        (selected, other)
    }

    /// The title bar's texts: every label above the column headings.
    fn title(test: &TestingRunner) -> Vec<String> {
        let headings = test
            .find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == "Author")
                    .map(|_| node.layout().area.min_y())
            })
            .unwrap_or_else(|| panic!("no column headings"));
        test.find_many(|node, element| {
            Label::try_downcast(element)
                .filter(|_| node.layout().area.max_y() <= headings)
                .map(|label| label.text.to_string())
        })
    }

    /// What a refresh answered, handed to the window as `session::apply` would keep it: refs
    /// with `HEAD` at `head`, one branch `main` whose upstream is `upstream` (gone when it is
    /// not listed), counts for `main`, and a status listing `changed` paths.
    fn refreshed_with(
        test: &mut TestingRunner,
        view: View,
        head: cairn_model::HeadState,
        upstream: Option<(&str, bool)>,
        counts: Option<cairn_model::AheadBehind>,
        changed: usize,
    ) {
        use cairn_model::{Ref, RefKind, RefName, RefTarget, RefsSnapshot, StatusEntry, Upstream};
        let mut refs = vec![Ref {
            name: RefName::new("refs/heads/main"),
            kind: RefKind::LocalBranch,
            target: RefTarget::Commit(oid(0)),
            symbolic: None,
            upstream: upstream.map(|(name, exists)| {
                if exists {
                    Upstream::Exists {
                        name: RefName::new(name),
                        commit: Some(oid(1)),
                    }
                } else {
                    Upstream::Gone {
                        name: RefName::new(name),
                    }
                }
            }),
        }];
        if let Some((name, true)) = upstream {
            refs.push(Ref {
                name: RefName::new(name),
                kind: RefKind::RemoteTracking,
                target: RefTarget::Commit(oid(1)),
                symbolic: None,
                upstream: None,
            });
        }
        let mut refreshed = view.refreshed;
        let mut state = refreshed.write();
        let _ = state.refs_arrived(std::sync::Arc::new(RefsSnapshot {
            refs,
            head,
            stashes: Vec::new(),
            unreadable: 0,
        }));
        if let Some(counts) = counts {
            let _ = state.ahead_behind_arrived(vec![(RefName::new("refs/heads/main"), counts)]);
        }
        let _ = state.status_arrived(std::sync::Arc::new(cairn_model::LocalChanges::new(
            WorkingTreeStatus::Listed(
                (0..changed)
                    .map(|n| {
                        StatusEntry::Untracked(cairn_model::RepoPath::from(
                            format!("new-{n}").as_str(),
                        ))
                    })
                    .collect(),
            ),
        )));
        drop(state);
        test.sync_and_update();
    }

    /// R7.1, the QA brief: the title bar names the repository — `*` while status lists a
    /// change — the current branch and its counts behind then ahead; a gone upstream says so
    /// and draws no counts; a detached `HEAD` its short id; an unborn branch its name and that
    /// it has no commit. Caught by: the star drawn for a clean tree or missed for a dirty one,
    /// another branch's counts, counts drawn for a gone upstream, or a detached or unborn
    /// `HEAD` drawn as a branch.
    #[test]
    fn the_title_bar_names_the_repository_the_branch_and_how_far_it_is_from_its_upstream() {
        use cairn_model::{AheadBehind, HeadState, RefName};
        let main = || HeadState::Branch(RefName::new("refs/heads/main"));

        let (mut test, view, _) = launch((0..2).map(row).collect(), received(2, true));
        assert!(
            title(&test).contains(&"engine".to_owned()),
            "{:?}",
            title(&test)
        );
        refreshed_with(
            &mut test,
            view,
            main(),
            Some(("refs/remotes/origin/main", true)),
            Some(AheadBehind {
                ahead: 1,
                behind: 18,
            }),
            2,
        );
        let shown = title(&test);
        for wanted in ["engine*", "main", "18↓1↑"] {
            assert!(
                shown.contains(&wanted.to_owned()),
                "no {wanted:?} in {shown:?}"
            );
        }

        let (mut test, view, _) = launch((0..2).map(row).collect(), received(2, true));
        refreshed_with(
            &mut test,
            view,
            main(),
            Some(("refs/remotes/origin/main", false)),
            // Counts a refresh read before the upstream went: never drawn beside `gone`.
            Some(AheadBehind {
                ahead: 1,
                behind: 18,
            }),
            0,
        );
        let shown = title(&test);
        for wanted in ["engine", "main", cairn_ui::UPSTREAM_GONE] {
            assert!(
                shown.contains(&wanted.to_owned()),
                "no {wanted:?} in {shown:?}"
            );
        }
        assert!(
            !shown
                .iter()
                .any(|text| text.contains('↓') || text.contains('↑')),
            "{shown:?}"
        );

        let (mut test, view, _) = launch((0..2).map(row).collect(), received(2, true));
        refreshed_with(&mut test, view, HeadState::Detached(oid(1)), None, None, 0);
        let detached = format!("HEAD detached at {}", oid(1).short().as_str());
        assert!(title(&test).contains(&detached), "{:?}", title(&test));

        let (mut test, view, _) = launch(Vec::new(), received(0, true));
        refreshed_with(
            &mut test,
            view,
            HeadState::Unborn(RefName::new("refs/heads/trunk")),
            None,
            None,
            1,
        );
        let shown = title(&test);
        assert!(
            shown.contains(&"trunk (no commits yet)".to_owned()),
            "{shown:?}"
        );
        assert!(shown.contains(&"engine*".to_owned()), "{shown:?}");

        // QC1: the name is the one the worker says it opened, never the path given.
        let mut repository = view.repository;
        repository.set(None);
        test.sync_and_update();
        let shown = title(&test);
        assert!(
            !shown
                .iter()
                .any(|text| text.starts_with("engine") || text == PATH),
            "a repository not yet opened was named: {shown:?}"
        );
        test.run_in(|| {
            crate::session::apply(
                crate::worker::Update::Opened {
                    name: "folder".to_owned(),
                },
                view,
                &crate::session::Worker {
                    submit: &|_| {},
                    refuse: &|_| {},
                    closing: false,
                },
            );
        });
        test.sync_and_update();
        let shown = title(&test);
        assert!(shown.contains(&"folder*".to_owned()), "{shown:?}");
    }

    /// C7 through the window: a row's chips are laid out against the refresh's snapshot — the
    /// current branch first with its check mark and folded with its upstream at that commit,
    /// another remote's ref its own chip — and `HEAD`'s subject is bold; selected, its Commit
    /// tab draws the same chips in a REFS row (R6.1), and a commit no ref points at has none.
    /// Caught by: the window not handing the list the snapshot (no fold, no current first),
    /// the head flag dropped, or the REFS row not given the selected row's chips.
    #[test]
    fn a_rows_chips_and_its_refs_row_are_drawn_against_the_refreshs_snapshot() {
        use cairn_model::{
            HeadState, Label as RefLabel, Ref, RefKind, RefName, RefTarget, RefsSnapshot, Upstream,
        };

        let (mut test, view, _) = launch(Vec::new(), received(2, true));
        let mut page = RowsPage::new();
        let names = [
            "refs/heads/main",
            "refs/remotes/mike/main",
            "refs/remotes/origin/main",
        ];
        let labels: Vec<RefLabel<'_>> = names
            .iter()
            .map(|name| RefLabel {
                name,
                kind: if name.starts_with("refs/heads/") {
                    RefKind::LocalBranch
                } else {
                    RefKind::RemoteTracking
                },
                current: *name == "refs/heads/main",
            })
            .collect();
        page.push_labelled(
            GraphRow::new(oid(0), Lane::new(0), Vec::new()),
            PagedCommit {
                parents: 1,
                subject: "commit 0",
                author: "Ada",
                author_time: 0,
            },
            true,
            &labels,
        );
        page.push(
            GraphRow::new(oid(1), Lane::new(0), Vec::new()),
            PagedCommit {
                parents: 1,
                subject: "commit 1",
                author: "Ada",
                author_time: 0,
            },
        );
        let mut rows = view.rows;
        rows.write()
            .append(page)
            .unwrap_or_else(|full| panic!("{full}"));
        let listed = |name: &str, upstream: Option<&str>| Ref {
            name: RefName::new(name),
            kind: if name.starts_with("refs/heads/") {
                RefKind::LocalBranch
            } else {
                RefKind::RemoteTracking
            },
            target: RefTarget::Commit(oid(0)),
            symbolic: None,
            upstream: upstream.map(|upstream| Upstream::Exists {
                name: RefName::new(upstream),
                commit: Some(oid(0)),
            }),
        };
        let mut refreshed = view.refreshed;
        let _ = refreshed
            .write()
            .refs_arrived(std::sync::Arc::new(RefsSnapshot {
                refs: vec![
                    listed("refs/heads/main", Some("refs/remotes/origin/main")),
                    listed("refs/remotes/mike/main", None),
                    listed("refs/remotes/origin/main", None),
                ],
                head: HeadState::Branch(RefName::new("refs/heads/main")),
                stashes: Vec::new(),
                unreadable: 0,
            }));
        test.sync_and_update();
        test.sync_and_update();

        let row_y = test
            .find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == "commit 0")
                    .map(|_| node.layout().area.center().y)
            })
            .unwrap_or_else(|| panic!("the labelled row was not drawn"));
        let chips: Vec<(f32, String)> = {
            let chip_size = Some(freya::prelude::FontSize::from(cairn_ui::CHIP_FONT_SIZE));
            let mut found = test.find_many(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text_style_data.font_size == chip_size)
                    .filter(|_| (node.layout().area.center().y - row_y).abs() < 1.)
                    .map(|label| (node.layout().area.min_x(), label.text.to_string()))
            });
            found.sort_by(|a, b| a.0.total_cmp(&b.0));
            found
        };
        let texts: Vec<&str> = chips.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(
            texts,
            ["main", "mike/main"],
            "origin/main was not folded into main"
        );
        let weight = |text: &str| {
            test.find(|_, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == text)
                    .map(|label| label.text_style_data.font_weight)
            })
            .flatten()
        };
        assert_eq!(
            weight("commit 0"),
            Some(FontWeight::BOLD),
            "HEAD's subject is not bold"
        );
        assert_ne!(weight("commit 1"), Some(FontWeight::BOLD));

        click_row(&mut test, 0);
        arrives(&mut test, view, 0, vec![oid(1)]);
        let shown = pane(&test);
        let at = |text: &str| shown.iter().position(|shown| shown == text);
        assert!(
            at(cairn_ui::REFS_CAPTION).is_some(),
            "no REFS row: {shown:?}"
        );
        assert!(
            at(cairn_ui::REFS_CAPTION) < at("main")
                && at("main") < at("mike/main")
                && at("mike/main") < at(cairn_ui::ID_CAPTION),
            "REFS does not hold the row's chips above the id: {shown:?}"
        );
        assert_eq!(
            at("origin/main"),
            None,
            "the REFS row did not fold origin/main"
        );

        click_row(&mut test, 1);
        arrives(&mut test, view, 1, vec![oid(2)]);
        let shown = pane(&test);
        assert!(
            shown.iter().any(|text| text == cairn_ui::ID_CAPTION),
            "{shown:?}"
        );
        assert!(
            !shown.iter().any(|text| text == cairn_ui::REFS_CAPTION),
            "a commit no ref points at has a REFS row: {shown:?}"
        );
    }

    /// A stash's row (refs-and-status R4.2, R5.4): its `stash@{n}` chip, then its message as
    /// its subject, on one row; pressed, it is selected by its
    /// own identity and asks what `git stash show` lists of it (R6.2); pressed with the extending
    /// chord beside a commit, the pair holds the stash's row, the lower of the two the base.
    /// Caught by: a stash's row drawing nothing, selected as a commit's, or asking a commit's
    /// comparison of its stash commit rather than `git stash show`'s.
    #[test]
    fn a_stash_row_draws_its_message_and_asks_what_it_changed_on_its_base() {
        let (mut test, view, submitted) = launch((0..3).map(row).collect(), received(6, true));
        let mut rows = view.rows;
        let mut page = RowsPage::new();
        page.push_stash(
            GraphRow::new(oid(90), Lane::new(1), Vec::new()),
            cairn_model::PagedStash {
                index: 0,
                base: oid(3),
                message: "On main: wip",
                author: "Ada",
                author_time: 0,
            },
        );
        rows.write()
            .append(page)
            .unwrap_or_else(|full| panic!("{full}"));
        hold(&mut rows.write(), (3..6).map(row).collect());
        test.sync_and_update();
        test.sync_and_update();

        let place = |text: &str| {
            test.find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == text)
                    .map(|_| (node.layout().area.min_x(), node.layout().area.center().y))
            })
            .unwrap_or_else(|| panic!("nothing reads {text:?}"))
        };
        let (chip, message) = (place("stash@{0}"), place("On main: wip"));
        assert!(
            chip.0 < message.0,
            "the stash's chip is not before its message"
        );
        assert!(
            (chip.1 - message.1).abs() < 1.,
            "the chip is not on the stash's row"
        );

        let from = submitted.borrow().len();
        click_label(&mut test, "On main: wip");
        test.sync_and_update();
        assert_eq!(selection(view), (Some(RowId::Stash(oid(90))), None));
        assert_eq!(
            changes_asked(&submitted, from),
            [Comparison::Stash(oid(90))],
            "a stash's row asks what `git stash show` lists"
        );

        let from = submitted.borrow().len();
        extend_row(&mut test, 1);
        assert_eq!(
            selection(view),
            (Some(RowId::Stash(oid(90))), Some(RowId::Commit(oid(1))))
        );
        assert_eq!(
            changes_asked(&submitted, from),
            [Comparison::Between {
                old: oid(90),
                new: oid(1)
            }],
            "the stash's row, the lower, is the base"
        );
    }

    /// C12, R7.1-R7.3 through the window: a row pressed with the table's extending chord
    /// selects it beside the row selected — exactly two, drawn selected — and asks for the
    /// comparison tip against tip with the LOWER row the base, whichever was pressed first,
    /// far apart and adjacent; the pane shows the Changes tab under a header naming both
    /// commits, the Commit tab unavailable; the swap asks for the comparison the other way
    /// round. Caught by: the press read without the held keys (a plain selection), the base
    /// taken as the first pressed rather than the lower row, a merge-base comparison, a swap
    /// that only relabels, or the Commit tab still reachable.
    #[test]
    fn a_modifier_click_compares_two_commits_tip_against_tip_with_the_lower_row_the_base() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        for (first, second, base, tip) in [(2, 7, 7, 2), (7, 2, 7, 2), (3, 4, 4, 3)] {
            click_row(&mut test, first);
            let from = submitted.borrow().len();
            extend_row(&mut test, second);
            assert_eq!(
                changes_asked(&submitted, from),
                [between(base, tip)],
                "{first} then {second}"
            );
            assert_eq!(
                selection(view),
                (
                    Some(RowId::Commit(oid(first))),
                    Some(RowId::Commit(oid(second)))
                )
            );
        }

        // Far apart again, for the pane, the comparison's answer arrived: no commit's details.
        click_row(&mut test, 2);
        extend_row(&mut test, 7);
        // Both drawn selected, and no other.
        let background = |test: &TestingRunner, subject: &str| {
            let y = test
                .find(|node, element| {
                    Label::try_downcast(element)
                        .filter(|label| label.text == subject)
                        .map(|_| node.layout().area.min_y())
                })
                .unwrap_or_else(|| panic!("no row reads {subject}"));
            test.find_many(|node, element| {
                let area = node.layout().area;
                Rect::try_downcast(element)
                    .filter(|_| {
                        area.min_y() <= y && y < area.max_y() && area.height() == ROW_HEIGHT
                    })
                    .map(|rect| rect.style.background)
            })
        };
        assert_eq!(background(&test, "commit 2"), background(&test, "commit 7"));
        assert_ne!(
            background(&test, "commit 7"),
            background(&test, "commit 5"),
            "the second commit is not drawn selected"
        );
        let mut diff = view.diff;
        let mut compared = answer_with(2, 3);
        compared.details = None;
        test.run_in(|| {
            diff.write()
                .changes_arrived(between(7, 2), compared.clone())
        });
        for _ in 0..3 {
            test.sync_and_update();
        }
        let shown = pane(&test);
        let base_id = oid(7).short().as_str().to_owned();
        let tip_id = oid(2).short().as_str().to_owned();
        let at = |text: &str| shown.iter().position(|t| t == text);
        assert!(
            at(cairn_ui::BASE_CAPTION) < at(&base_id)
                && at(&base_id) < at(cairn_ui::TIP_CAPTION)
                && at(cairn_ui::TIP_CAPTION) < at(&tip_id),
            "the header does not name the base then the tip: {shown:?}"
        );
        assert!(shown.iter().any(|t| t == "commit 7") && shown.iter().any(|t| t == "commit 2"));
        // The Commit tab is unavailable: pressed, the Changes tab stays.
        click_tab(&mut test, DetailTab::Commit);
        assert_eq!(*view.detail_tab.read(), DetailTab::default());
        assert!(
            pane(&test).iter().any(|t| t == cairn_ui::BASE_CAPTION),
            "the Commit tab was shown over two commits"
        );

        let from = submitted.borrow().len();
        click_named(&mut test, cairn_ui::SWAP_LABEL);
        assert_eq!(changes_asked(&submitted, from), [between(2, 7)]);
        test.run_in(|| {
            diff.write()
                .changes_arrived(between(2, 7), compared.clone())
        });
        for _ in 0..3 {
            test.sync_and_update();
        }
        let shown = pane(&test);
        let at = |text: &str| shown.iter().position(|t| t == text);
        assert!(
            at(&tip_id) < at(&base_id),
            "the swap did not reverse the header: {shown:?}"
        );
    }

    /// C12 and the QA brief: a comparison is never left half-selected. A third press with the
    /// chord replaces the second, the row pressed plainly staying — exactly two; a press with
    /// the chord on one of the pair leaves the other selected alone; on the one row selected it
    /// changes nothing; and a plain press while the comparison is on its way returns to one,
    /// the comparison's late answer never drawn. Caught by: a third commit kept, a pair with
    /// one commit, nothing selected, or a late comparison drawn under one commit.
    #[test]
    fn a_comparison_is_never_left_half_selected() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        let id = |n: usize| Some(RowId::Commit(oid(n)));
        click_row(&mut test, 2);
        extend_row(&mut test, 7);
        let from = submitted.borrow().len();
        extend_row(&mut test, 5);
        assert_eq!(selection(view), (id(2), id(5)), "a third press");
        assert_eq!(changes_asked(&submitted, from), [between(5, 2)]);

        let from = submitted.borrow().len();
        extend_row(&mut test, 5);
        assert_eq!(selection(view), (id(2), None), "the second pressed again");
        assert_eq!(
            changes_asked(&submitted, from),
            [Comparison::Commit(oid(2))]
        );

        let from = submitted.borrow().len();
        extend_row(&mut test, 2);
        assert_eq!(
            selection(view),
            (id(2), None),
            "the one selected pressed again"
        );
        assert!(changes_asked(&submitted, from).is_empty());

        extend_row(&mut test, 5);
        extend_row(&mut test, 2);
        assert_eq!(
            selection(view),
            (id(5), None),
            "the first of a pair pressed again"
        );

        // A plain press while the comparison is on its way.
        click_row(&mut test, 1);
        extend_row(&mut test, 8);
        let from = submitted.borrow().len();
        click_row(&mut test, 4);
        assert_eq!(selection(view), (id(4), None));
        assert_eq!(
            changes_asked(&submitted, from),
            [Comparison::Commit(oid(4))]
        );
        let mut diff = view.diff;
        let kept = test.run_in(|| {
            diff.write()
                .changes_arrived(between(8, 1), answer_with(1, 2))
        });
        assert!(
            !kept,
            "the comparison's late answer was kept under one commit"
        );
        test.sync_and_update();
        assert!(
            !pane(&test).iter().any(|t| t == cairn_ui::BASE_CAPTION),
            "the comparison's header is drawn over one commit"
        );
    }

    /// Presses `tab` in the detail pane's strip — not the history's column heading of the
    /// same word.
    fn click_tab(test: &mut TestingRunner, tab: DetailTab) {
        let strip = pane_top(test);
        let centre = test
            .find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| {
                        label.text == tab.caption()
                            && (node.layout().area.min_y() - strip).abs() < 1.
                    })
                    .map(|_| node.layout().area.center())
            })
            .unwrap_or_else(|| panic!("no {tab:?} tab in the pane's strip"));
        test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
        test.sync_and_update();
    }

    /// Phase 08 QA's U1, R7.3 and the session's tab: while two commits are compared the
    /// Commit tab is unavailable — pressed, it changes nothing, not even the tab kept for the
    /// session — and when one commit is selected again, the tab chosen before the comparison
    /// is the one drawn. Caught by: the press reaching the session's tab while it is drawn
    /// disabled (the Commit tab shown once the comparison ends).
    #[test]
    fn the_commit_tab_is_unavailable_while_comparing_and_the_tab_chosen_comes_back() {
        let (mut test, view, _) = launch((0..10).map(row).collect(), received(10, true));
        click_row(&mut test, 2);
        click_tab(&mut test, DetailTab::Changes);
        assert_eq!(*view.detail_tab.read(), DetailTab::Changes);
        extend_row(&mut test, 7);
        click_tab(&mut test, DetailTab::Commit);
        assert_eq!(
            *view.detail_tab.read(),
            DetailTab::Changes,
            "the press on the unavailable Commit tab changed the session's tab"
        );
        assert!(pane(&test).iter().any(|t| t == cairn_ui::BASE_CAPTION));

        click_row(&mut test, 4);
        arrives(&mut test, view, 4, Vec::new());
        let shown = pane(&test);
        assert!(
            shown.iter().any(|t| t == crate::changes_tab::READING_DIFF)
                && !shown.iter().any(|t| t == cairn_ui::EXPAND_ALL_CAPTION),
            "the Changes tab chosen before the comparison is not the one drawn: {shown:?}"
        );
    }

    /// The user's decision (2026-10-04): the comparison's header — base, then tip, and the
    /// swap — is drawn the moment the pair is set, with "Reading the comparison…" under it
    /// while the change set is awaited, and its failure under it if it fails. Caught by: the
    /// header drawn only once the answer arrives (the tab saying only that it reads).
    #[test]
    fn the_comparisons_header_is_drawn_at_once_and_the_answer_awaited_under_it() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        click_row(&mut test, 2);
        extend_row(&mut test, 7);
        test.sync_and_update();
        let shown = pane(&test);
        let (base_id, tip_id) = (
            oid(7).short().as_str().to_owned(),
            oid(2).short().as_str().to_owned(),
        );
        let at = |text: &str| shown.iter().position(|t| t == text);
        assert!(
            at(cairn_ui::BASE_CAPTION) < at(&base_id)
                && at(&base_id) < at(cairn_ui::TIP_CAPTION)
                && at(cairn_ui::TIP_CAPTION) < at(&tip_id)
                && at(&tip_id) < at(crate::changes_tab::READING_COMPARISON),
            "the header is not drawn over the comparison being read: {shown:?}"
        );

        // The swap works before the answer, too.
        let from = submitted.borrow().len();
        click_named(&mut test, cairn_ui::SWAP_LABEL);
        assert_eq!(changes_asked(&submitted, from), [between(2, 7)]);

        let mut diff = view.diff;
        test.run_in(|| {
            diff.write().failed(
                &crate::worker::DiffQuery::Changes(between(2, 7)),
                "no such commit".to_owned(),
            )
        });
        test.sync_and_update();
        test.sync_and_update();
        let shown = pane(&test);
        assert!(
            shown.iter().any(|t| t == cairn_ui::BASE_CAPTION)
                && shown.iter().any(|t| t == "no such commit"),
            "the failure is not drawn under the header: {shown:?}"
        );
    }

    /// The user's decision (2026-10-04): the keys held are let go of when the window loses
    /// focus — a release made in another window is never heard here — so a plain press after
    /// coming back selects one commit and compares nothing. Caught by: the held keys kept
    /// across the focus loss (the plain press read as the extending chord).
    #[test]
    fn the_keys_held_are_let_go_of_when_the_window_loses_focus() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        click_row(&mut test, 2);
        hold_extending(&mut test, true);
        test.run_in(|| Platform::get().is_app_focused.set(false));
        test.sync_and_update();
        test.run_in(|| Platform::get().is_app_focused.set(true));
        test.sync_and_update();

        let from = submitted.borrow().len();
        click_row(&mut test, 5);
        assert_eq!(selection(view), (Some(RowId::Commit(oid(5))), None));
        assert!(view.pair.peek().is_none(), "the plain press compared two");
        assert_eq!(
            changes_asked(&submitted, from),
            [Comparison::Commit(oid(5))]
        );
    }

    /// Phase 08 QA's U6: a row pressed with the extending chord while nothing is selected is
    /// selected alone, and its changes asked — nothing to compare it with. Caught by: a pair
    /// made with nothing, or the press ignored.
    #[test]
    fn a_modifier_click_with_nothing_selected_selects_that_commit_alone() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        let from = submitted.borrow().len();
        extend_row(&mut test, 5);
        assert_eq!(selection(view), (Some(RowId::Commit(oid(5))), None));
        assert!(view.pair.peek().is_none());
        assert_eq!(
            changes_asked(&submitted, from),
            [Comparison::Commit(oid(5))]
        );
    }

    /// One more line of context, as the bar or a shortcut asks it, its requests landing in
    /// `submitted`.
    fn more_lines(test: &mut TestingRunner, view: View, submitted: &Submitted) {
        let pushing = submitted.clone();
        let submit = move |request| pushing.borrow_mut().push(request);
        test.run_in(|| {
            crate::diff_actions::change_settings(view, Some(&submit), |settings| {
                settings.more_lines()
            })
        });
        test.sync_and_update();
        test.sync_and_update();
    }

    fn asks_file(requests: &[Request]) -> bool {
        requests.iter().any(|r| matches!(r, Request::FileDiff(_)))
    }

    fn asks_expansion(requests: &[Request]) -> bool {
        requests.iter().any(|r| matches!(r, Request::Expand(_)))
    }

    /// Phase 08 QA's U3 through the window: a setting changed asks at once only the selection
    /// whose tab is shown — with the Commit tab shown and a file open in place, the files
    /// opened in place and not the Changes tab's file; with the Changes tab shown, its file
    /// and not the files opened in place — and the other is asked, at the new setting, as
    /// its tab is shown. Caught by: the shown tab mapped to the other selection, which asks
    /// both (the second ending the first).
    #[test]
    fn a_setting_asks_the_shown_tabs_selection_at_once_and_the_other_as_its_tab_is_shown() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        choose_the_file(&mut test, view);
        let query = last_file_query(&submitted);
        answer_file(&mut test, view, &query, 40);
        click_tab(&mut test, DetailTab::Commit);
        click_label(&mut test, "file-of-2.rs");
        let mut diff = view.diff;
        test.run_in(|| {
            diff.write()
                .expansion_arrived(vec![opened_file(0, text_answer(2, 40), false)], None)
        });
        test.sync_and_update();
        test.sync_and_update();

        let from = submitted.borrow().len();
        more_lines(&mut test, view, &submitted);
        let asked = requests_since(&submitted, from);
        assert!(asks_expansion(&asked), "{asked:?}");
        assert!(
            !asks_file(&asked),
            "the Changes tab's file was asked with the Commit tab shown: {asked:?}"
        );
        assert_eq!(
            last_expansion(&submitted).options.context,
            Context::Lines(4)
        );

        let from = submitted.borrow().len();
        click_tab(&mut test, DetailTab::Changes);
        test.sync_and_update();
        let asked = requests_since(&submitted, from);
        assert!(asks_file(&asked) && !asks_expansion(&asked), "{asked:?}");
        assert_eq!(
            last_file_query(&submitted).options.context,
            Context::Lines(4)
        );

        let from = submitted.borrow().len();
        more_lines(&mut test, view, &submitted);
        let asked = requests_since(&submitted, from);
        assert!(asks_file(&asked), "{asked:?}");
        assert!(
            !asks_expansion(&asked),
            "the files opened in place were asked with the Changes tab shown: {asked:?}"
        );

        let from = submitted.borrow().len();
        click_tab(&mut test, DetailTab::Commit);
        test.sync_and_update();
        let asked = requests_since(&submitted, from);
        assert!(asks_expansion(&asked) && !asks_file(&asked), "{asked:?}");
        assert_eq!(
            last_expansion(&submitted).options.context,
            Context::Lines(5)
        );
    }

    /// The user's decision (2026-10-04), departing from Fork: Entire File is the Changes
    /// tab's alone. With it on there, a file opened in place in the Commit tab, and Expand
    /// All, are asked at the shared line context and draw hunks; turning it off asks nothing
    /// for the files opened in place; and the context and whitespace, which the two tabs
    /// share, still reach them. Caught by: the Commit tab stuck showing the entire file with
    /// no bar to turn it off (the user's report), or a toggle that re-reads its open files.
    #[test]
    fn entire_file_is_the_changes_tabs_alone_and_never_reaches_a_file_opened_in_place() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        choose_the_file(&mut test, view);
        let query = last_file_query(&submitted);
        answer_file(&mut test, view, &query, 40);
        click_named(&mut test, cairn_ui::ENTIRE_FILE_LABEL);
        assert_eq!(
            last_file_query(&submitted).options.context,
            Context::EntireFile
        );

        click_tab(&mut test, DetailTab::Commit);
        click_label(&mut test, "file-of-2.rs");
        let opened = last_expansion(&submitted);
        assert_eq!(
            opened.options.context,
            Context::Lines(3),
            "a file opened in place was asked for the entire file"
        );
        let mut diff = view.diff;
        test.run_in(|| {
            diff.write()
                .expansion_arrived(vec![opened_file(0, text_answer(2, 40), false)], None)
        });
        test.sync_and_update();
        test.sync_and_update();
        let rows = pane_rows(&test);
        assert!(rows.iter().any(|t| t.starts_with("@@ -3,")), "{rows:?}");

        // Collapse All, then Expand All: asked at the line context too.
        click_label(&mut test, cairn_ui::COLLAPSE_ALL_CAPTION);
        click_label(&mut test, cairn_ui::EXPAND_ALL_CAPTION);
        let all = last_expansion(&submitted);
        assert!(all.all.is_some(), "{all:?}");
        assert_eq!(all.options.context, Context::Lines(3));
        test.run_in(|| {
            diff.write().expansion_arrived(
                vec![opened_file(0, text_answer(2, 40), true)],
                Some(crate::worker::AllProgress {
                    at: crate::worker::AllFrom { next: 1, spent: 40 },
                    ended: Some(crate::worker::AllEnded::Every),
                }),
            )
        });
        test.sync_and_update();
        test.sync_and_update();

        // Entire File off, in the Changes tab: nothing is asked for the files opened in place,
        // then or as the Commit tab is shown again.
        click_tab(&mut test, DetailTab::Changes);
        test.sync_and_update();
        let from = submitted.borrow().len();
        click_named(&mut test, cairn_ui::ENTIRE_FILE_LABEL);
        click_tab(&mut test, DetailTab::Commit);
        test.sync_and_update();
        let asked = requests_since(&submitted, from);
        assert!(
            !asks_expansion(&asked),
            "Entire File re-asked the files opened in place: {asked:?}"
        );
        assert!(pane_rows(&test).iter().any(|t| t.starts_with("@@ -3,")));

        // The context and the whitespace moved in the Changes tab, then Entire File on again:
        // the shared two reach the files opened in place, at their line context.
        click_tab(&mut test, DetailTab::Changes);
        more_lines(&mut test, view, &submitted);
        click_named(&mut test, cairn_ui::IGNORE_WHITESPACE_LABEL);
        click_named(&mut test, cairn_ui::ENTIRE_FILE_LABEL);
        let from = submitted.borrow().len();
        click_tab(&mut test, DetailTab::Commit);
        test.sync_and_update();
        let asked = requests_since(&submitted, from);
        assert!(asks_expansion(&asked), "{asked:?}");
        let reasked = last_expansion(&submitted);
        assert_eq!(reasked.options.context, Context::Lines(4));
        assert!(reasked.options.ignore_whitespace);
    }

    fn requests_since(submitted: &Submitted, from: usize) -> Vec<Request> {
        submitted.borrow()[from..]
            .iter()
            .filter(|request| !matches!(request, Request::Retire(_)))
            .cloned()
            .collect()
    }

    /// R6.2, R6.3, C11 through the window: each line button moves the context by one and asks
    /// the file again at it, never below one line; the entire file and ignoring whitespace
    /// ask again with theirs; every diff view shares the settings, so the next file chosen is
    /// asked at them. Caught by: a step of more than one, a floor of zero, a setting that
    /// changes what is drawn without asking git again, or one the next file forgets.
    #[test]
    fn the_bar_moves_the_context_a_line_at_a_time_and_asks_again_never_below_one() {
        use cairn_model::Context;
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        choose_the_file(&mut test, view);
        let query = last_file_query(&submitted);
        answer_file(&mut test, view, &query, 40);

        let context_after = |test: &mut TestingRunner, name: &str| {
            let before = submitted.borrow().len();
            click_named(test, name);
            test.sync_and_update();
            requests_since(&submitted, before)
        };
        let asked = |requests: Vec<Request>| -> Vec<(Context, bool)> {
            requests
                .into_iter()
                .map(|request| match request {
                    Request::FileDiff(query) => {
                        (query.options.context, query.options.ignore_whitespace)
                    }
                    other => panic!("asked {other:?}"),
                })
                .collect()
        };
        assert_eq!(
            asked(context_after(&mut test, cairn_ui::FEWER_LINES_LABEL)),
            [(Context::Lines(2), false)]
        );
        assert_eq!(
            asked(context_after(&mut test, cairn_ui::FEWER_LINES_LABEL)),
            [(Context::Lines(1), false)]
        );
        assert_eq!(
            asked(context_after(&mut test, cairn_ui::FEWER_LINES_LABEL)),
            [],
            "the context went below one line"
        );
        assert_eq!(
            asked(context_after(&mut test, cairn_ui::MORE_LINES_LABEL)),
            [(Context::Lines(2), false)]
        );
        assert_eq!(
            asked(context_after(&mut test, cairn_ui::IGNORE_WHITESPACE_LABEL)),
            [(Context::Lines(2), true)]
        );
        assert_eq!(
            asked(context_after(&mut test, cairn_ui::ENTIRE_FILE_LABEL)),
            [(Context::EntireFile, true)]
        );
        assert_eq!(
            asked(context_after(&mut test, cairn_ui::ENTIRE_FILE_LABEL)),
            [(Context::Lines(2), true)],
            "leaving the entire file forgot the lines"
        );
        let settings = *view.diff_settings.read();
        assert_eq!(settings.context(), Context::Lines(2));

        // The next file is asked at the shared settings.
        let mut answer = answer_for(2, Vec::new());
        answer.files.push(ChangedFile {
            old_path: RepoPath::from("second.rs"),
            new_path: RepoPath::from("second.rs"),
            ..answer.files[0].clone()
        });
        let mut diff = view.diff;
        test.run_in(|| diff.write().select_changes(Comparison::Commit(oid(2))));
        test.run_in(|| {
            diff.write()
                .changes_arrived(Comparison::Commit(oid(2)), answer)
        });
        test.sync_and_update();
        press_chord(&mut test, Action::ShowCommitTab);
        click_label(&mut test, "second.rs");
        let next = last_file_query(&submitted);
        assert_eq!(next.options.context, Context::Lines(2));
        assert!(next.options.ignore_whitespace);
    }

    fn scrolled_y(view: View) -> i32 {
        let (_, y): (i32, i32) = view.diff_scroll.into();
        y
    }

    /// R6.2, R8 with the user's chords: next change moves the diff to the first change, then
    /// one change at a time, and previous back — heard while focus is inside the detail
    /// pane, and not from the history list. The bar's buttons do the same. Caught by: a
    /// chord heard window-wide, one that does nothing, or buttons and chords that disagree.
    #[test]
    fn previous_and_next_change_move_the_diff_while_the_pane_has_focus() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        choose_the_file(&mut test, view);
        let query = last_file_query(&submitted);
        answer_file(&mut test, view, &query, 200);
        let layout_row = |change: usize| {
            view.diff
                .peek()
                .shown_file()
                .and_then(|shown| shown.layout())
                .and_then(|layout| layout.change_row(change))
                .expect("a change")
        };
        let top_for =
            |change: usize| -(((layout_row(change) - 1) as f32) * cairn_ui::DIFF_ROW_HEIGHT) as i32;

        // Focus in the history list: the pane does not hear the chord.
        click_row(&mut test, 2);
        press_chord(&mut test, Action::NextChange);
        assert_eq!(
            scrolled_y(view),
            0,
            "a chord outside the pane moved the diff"
        );

        // Focus in the diff.
        // Below the strip, the summary and the bar; right of the file list.
        let rows_top = pane_top(&test) + 130.;
        test.click_cursor((LEFT_X + 600., f64::from(rows_top)));
        test.sync_and_update();
        press_chord(&mut test, Action::NextChange);
        assert_eq!(scrolled_y(view), top_for(0));
        press_chord(&mut test, Action::NextChange);
        press_chord(&mut test, Action::NextChange);
        assert_eq!(scrolled_y(view), top_for(2));
        press_chord(&mut test, Action::PreviousChange);
        assert_eq!(scrolled_y(view), top_for(1));
        assert_eq!(
            view.change_cursor
                .read()
                .as_ref()
                .map(|cursor| cursor.change),
            Some(1)
        );

        click_named(&mut test, cairn_ui::NEXT_CHANGE_LABEL);
        assert_eq!(
            scrolled_y(view),
            top_for(2),
            "the bar's next change did not move"
        );
        click_named(&mut test, cairn_ui::PREVIOUS_CHANGE_LABEL);
        assert_eq!(scrolled_y(view), top_for(1));
    }

    /// Row `n`'s change set with `files` files, `file-{k:05}.rs`, in place of its one.
    fn answer_with(n: usize, files: usize) -> ChangeSet {
        let mut answer = answer_for(n, Vec::new());
        let template = answer.files.remove(0);
        answer.files = (0..files)
            .map(|k| {
                let path = format!("file-{k:05}.rs");
                ChangedFile {
                    old_path: RepoPath::from(path.as_str()),
                    new_path: RepoPath::from(path.as_str()),
                    ..template.clone()
                }
            })
            .collect();
        answer
    }

    /// Row 2 chosen, a change set of `files` files arrived for it, and the Changes tab shown.
    fn changes_tab_over(test: &mut TestingRunner, view: View, files: usize) {
        click_row(test, 2);
        let mut diff = view.diff;
        test.run_in(|| {
            diff.write()
                .changes_arrived(Comparison::Commit(oid(2)), answer_with(2, files))
        });
        click_label(test, DetailTab::Changes.caption());
        for _ in 0..3 {
            test.sync_and_update();
        }
    }

    fn file_queries(submitted: &Submitted) -> Vec<crate::worker::FileQuery> {
        submitted
            .borrow()
            .iter()
            .filter_map(|request| match request {
                Request::FileDiff(query) => Some(query.clone()),
                _ => None,
            })
            .collect()
    }

    fn path_of(query: &crate::worker::FileQuery) -> String {
        match &query.target {
            crate::worker::FileTarget::Committed { file, .. } => {
                file.new_path.display().into_owned()
            }
            crate::worker::FileTarget::WorkingTree { path, .. } => path.display().into_owned(),
        }
    }

    /// C10, R5.4 through the window: the Changes tab shows the commit's one-line summary —
    /// author, short id, date, subject — its files in a list, and, with no file chosen, the
    /// first file's diff asked for, as Fork selects the first file (Finding 5); the answer is
    /// drawn beside the list. Caught by: no summary, no list, or a tab that waits for a file
    /// to be chosen elsewhere.
    #[test]
    fn the_changes_tab_shows_the_summary_the_files_and_the_first_files_diff() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        changes_tab_over(&mut test, view, 3);
        let shown = pane(&test);
        let [author, id, date, subject] =
            cairn_ui::summary_parts(&answer_for(2, Vec::new()).details.unwrap());
        for part in [&author, &id, &date, &subject] {
            assert!(
                shown.contains(part),
                "the summary lacks {part:?}: {shown:?}"
            );
        }
        for k in 0..3 {
            let path = format!("file-{k:05}.rs");
            assert!(shown.contains(&path), "the list lacks {path}: {shown:?}");
        }
        let asked = file_queries(&submitted);
        assert_eq!(
            asked.iter().map(path_of).collect::<Vec<_>>(),
            ["file-00000.rs"],
            "the first file was not chosen, or was asked twice"
        );
        let mut diff = view.diff;
        let query = asked[0].clone();
        test.run_in(|| {
            diff.write().file_arrived(
                &query,
                Some(cairn_model::ShownDiff::new(
                    text_answer(2, 40),
                    query.options.context,
                )),
            )
        });
        test.sync_and_update();
        test.sync_and_update();
        assert!(
            pane_rows(&test).iter().any(|t| t == "LINE 5"),
            "{:?}",
            pane_rows(&test)
        );
    }

    /// C10, R5.4 and the QA brief's "a list operation, not a diff operation": typing in the
    /// filter over 55,184 files asks a worker which match — handing it the window's own change
    /// set, shared, not a copy — and the UI thread filters nothing: until the answer the list
    /// says it is filtering and shows no file, then shows the files it names and no others, one
    /// viewport of them, under "Showing N of M files". Caught by: filtering on the UI thread
    /// (files shown before any answer), a copied change set, or rows drawn for files the
    /// answer left out.
    #[test]
    fn typing_in_the_filter_asks_a_worker_and_the_list_draws_its_answer() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        changes_tab_over(&mut test, view, 55_184);
        let list_rows = |test: &TestingRunner| -> Vec<String> {
            pane(test)
                .into_iter()
                .filter(|t| t.starts_with("file-") && t.ends_with(".rs"))
                .collect()
        };
        let before = list_rows(&test);
        assert!(!before.is_empty() && before.len() < 40, "{}", before.len());

        let field = test
            .find(|node, element| {
                Paragraph::try_downcast(element)
                    .filter(|paragraph| {
                        paragraph
                            .spans
                            .iter()
                            .any(|span| span.text.as_ref() == cairn_ui::FILTER_PLACEHOLDER)
                    })
                    // The Changes tab's, not the sidebar's.
                    .filter(|_| node.layout().area.min_x() > LEFT)
                    .map(|_| node.layout().area.center())
            })
            .expect("the filter field");
        test.click_cursor((f64::from(field.x), f64::from(field.y)));
        test.sync_and_update();
        let asked_before = submitted.borrow().len();
        test.write_text("01234");
        for _ in 0..3 {
            test.sync_and_update();
        }
        let filters: Vec<(String, bool)> = submitted.borrow()[asked_before..]
            .iter()
            .filter_map(|request| match request {
                Request::FilterFiles { text, files, .. } => {
                    let shared = match view.diff.peek().changes() {
                        Some((_, crate::diff_state::Answer::Ready(kept))) => {
                            std::sync::Arc::ptr_eq(kept, files)
                        }
                        _ => false,
                    };
                    Some((text.clone(), shared))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            filters.last(),
            Some(&("01234".to_owned(), true)),
            "the filter was not asked of a worker with the window's own change set: {filters:?}"
        );
        // No answer yet: the list says it is filtering and shows no file — it neither keeps
        // every file counted as matched nor filters anything itself.
        assert!(list_rows(&test).is_empty(), "{:?}", list_rows(&test));
        assert!(pane(&test).iter().any(|t| t == cairn_ui::FILTERING));

        let mut diff = view.diff;
        let of = Comparison::Commit(oid(2));
        test.run_in(|| diff.write().filter_arrived(of, "0123", vec![1]));
        test.sync_and_update();
        assert!(
            list_rows(&test).is_empty(),
            "a superseded text's answer was drawn"
        );
        test.run_in(|| diff.write().filter_arrived(of, "01234", vec![1234, 11234]));
        test.sync_and_update();
        test.sync_and_update();
        assert_eq!(list_rows(&test), ["file-01234.rs", "file-11234.rs"]);
        assert!(
            pane(&test).iter().any(|t| t == "Showing 2 of 55,184 files"),
            "{:?}",
            pane(&test)
        );
    }

    /// C10 and the QA brief: a file chosen in the Changes tab's list asks its diff and lets go
    /// of the last one's — the query it replaces is no longer wanted, so its answer, if it
    /// still arrives, is never drawn, and the diff thread's epoch for the lane has moved on
    /// (`worker::diff_tests`' `a_superseded_diff_kills_its_git` shows that kills its `git`).
    /// Caught by: a list that reports nothing, or a choice that leaves the last query wanted.
    #[test]
    fn a_file_chosen_in_the_list_supersedes_the_last_ones_diff() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        changes_tab_over(&mut test, view, 2);
        let first = file_queries(&submitted)[0].clone();
        click_label(&mut test, "file-00001.rs");
        test.sync_and_update();
        let asked = file_queries(&submitted);
        assert_eq!(
            asked.iter().map(path_of).collect::<Vec<_>>(),
            ["file-00000.rs", "file-00001.rs"]
        );
        assert!(
            !view.diff.peek().wants_file(&first),
            "the first file's diff is still wanted"
        );
        assert!(view.diff.peek().wants_file(&asked[1]));
        assert_eq!(view.diff.peek().file_index(), Some(1));
    }

    /// R6.1, the QA brief's "genuinely shared": side-by-side is one setting for every diff
    /// view, kept for the session — it holds across another file and another commit — and
    /// toggling it asks git nothing, since it draws the same answer another way. Caught by: a
    /// per-view setting, one the next file forgets, or a toggle that asks again.
    #[test]
    fn side_by_side_is_one_setting_for_every_diff_and_asks_nothing() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        changes_tab_over(&mut test, view, 2);
        let query = file_queries(&submitted)[0].clone();
        answer_file(&mut test, view, &query, 40);
        let headers = |test: &TestingRunner| {
            pane_rows(test)
                .iter()
                .filter(|t| t.starts_with("@@ -3,"))
                .count()
        };
        assert_eq!(headers(&test), 1, "unified draws one header per hunk");

        let before = submitted.borrow().len();
        click_named(&mut test, cairn_ui::SIDE_BY_SIDE_LABEL);
        test.sync_and_update();
        assert!(view.diff_settings.read().side_by_side());
        assert!(
            requests_since(&submitted, before).is_empty(),
            "toggling side-by-side asked git again"
        );
        assert_eq!(
            headers(&test),
            2,
            "side by side, the header is in each column"
        );

        click_label(&mut test, "file-00001.rs");
        click_row(&mut test, 3);
        assert!(
            view.diff_settings.read().side_by_side(),
            "the setting was not kept"
        );
    }

    /// R6.8, R6.9: Load Diff on a file past the limits asks it again with `load_anyway`, and
    /// while it is shown a setting changed keeps it loaded; another file starts unloaded.
    /// Caught by: Load Diff asking nothing, a context change that drops the load (the notice
    /// would come back), or a load that leaks to the next file.
    #[test]
    fn load_diff_asks_the_file_again_past_the_limits() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        changes_tab_over(&mut test, view, 2);
        let query = file_queries(&submitted)[0].clone();
        let mut diff = view.diff;
        let large = cairn_model::FileDiff {
            file: answer_with(2, 1).files.remove(0),
            content: cairn_model::DiffContent::TooLarge {
                crossed: cairn_model::SizeLimit::Bytes {
                    limit: 1_048_576,
                    measured: 2_532_736,
                },
                loadable: true,
            },
        };
        test.run_in(|| {
            diff.write().file_arrived(
                &query,
                Some(cairn_model::ShownDiff::new(large, query.options.context)),
            )
        });
        test.sync_and_update();
        assert!(
            pane(&test)
                .iter()
                .any(|t| t == cairn_ui::TOO_LARGE_TO_DISPLAY)
        );
        click_label(&mut test, cairn_ui::LOAD_DIFF_CAPTION);
        let loaded = last_file_query(&submitted);
        assert_eq!(path_of(&loaded), path_of(&query));
        assert!(
            loaded.options.load_anyway,
            "Load Diff did not ask past the limits"
        );

        click_named(&mut test, cairn_ui::MORE_LINES_LABEL);
        assert!(
            last_file_query(&submitted).options.load_anyway,
            "a setting dropped the load"
        );

        click_label(&mut test, "file-00001.rs");
        let next = last_file_query(&submitted);
        assert_eq!(path_of(&next), "file-00001.rs");
        assert!(
            !next.options.load_anyway,
            "the load leaked to the next file"
        );
    }

    /// Where the Changes tab's splitter sits: the x of the one handle-thin, pane-tall rect.
    fn list_split(test: &TestingRunner) -> f32 {
        let top = pane_top(test);
        test.find(|node, element| {
            let area = node.layout().area;
            Rect::try_downcast(element)
                .filter(|_| {
                    area.width() == ResizableContext::HANDLE_SIZE
                        && area.height() > 60.
                        && area.min_y() > top
                })
                .map(|_| area.center().x)
        })
        .unwrap_or_else(|| panic!("no splitter in the Changes tab"))
    }

    /// The user's decision (2026-10-03): the Changes tab's file list opens at about 35% of the
    /// pane, a drag never takes it below about 200 px, and the width dragged to is kept for
    /// the session — across another tab shown and back. Caught by: a fixed pixel width, no
    /// floor, or a width forgotten when the tab is shown again.
    #[test]
    fn the_file_list_opens_at_35_percent_of_the_pane_and_keeps_its_dragged_width() {
        let (mut test, view, _) = launch((0..10).map(row).collect(), received(10, true));
        changes_tab_over(&mut test, view, 3);
        let opened = list_split(&test);
        assert!(
            (opened - LEFT - 0.35 * 800.).abs() < 8.,
            "the list opened {opened} px wide in an 800 px pane"
        );

        let y = f64::from(pane_top(&test) + 120.);
        test.press_cursor((f64::from(opened), y));
        test.move_cursor((f64::from(opened) - 40., y));
        test.sync_and_update();
        test.move_cursor((LEFT_X + 20., y));
        test.sync_and_update();
        test.release_cursor((LEFT_X + 20., y));
        test.sync_and_update();
        let narrowest = list_split(&test) - LEFT;
        assert!(
            (190. ..230.).contains(&narrowest),
            "a drag took the list to {narrowest} px, past its 200 px floor"
        );

        click_label(&mut test, DetailTab::Commit.caption());
        click_label(&mut test, DetailTab::Changes.caption());
        for _ in 0..3 {
            test.sync_and_update();
        }
        assert!(
            (list_split(&test) - LEFT - narrowest).abs() < 2.,
            "the width dragged to, {narrowest}, was not kept: {}",
            list_split(&test)
        );
    }

    /// The labels drawn right of the Changes tab's splitter, inside the pane: the diff side.
    fn diff_side_labels(test: &TestingRunner) -> Vec<String> {
        let top = pane_top(test);
        let split = list_split(test);
        test.find_many(|node, element| {
            let area = node.layout().area;
            Label::try_downcast(element)
                .filter(|_| area.min_y() > top && area.min_x() > split && area.width() > 0.)
                .map(|label| label.text.to_string())
        })
    }

    /// The bug the user met ("the diff view disappeared and won't come back"): the file
    /// list's splitter reports the width dragged to in PIXELS, and the list's share is laid
    /// out as a PERCENTAGE — so once dragged, the next time the Changes tab was laid out anew
    /// (another commit chosen) the list was given hundreds of percent and the diff side less
    /// than nothing, for the rest of the session. The width dragged to is kept as a share of
    /// the pane, and the diff side is drawn beside it whatever is chosen next. Caught by: a
    /// share stored in pixels.
    #[test]
    fn a_dragged_file_list_leaves_the_diff_drawn_beside_it_on_the_next_commit() {
        let (mut test, view, _) = launch((0..10).map(row).collect(), received(10, true));
        changes_tab_over(&mut test, view, 3);
        let opened = list_split(&test);
        let reading = crate::changes_tab::READING_DIFF;
        assert!(
            diff_side_labels(&test).iter().any(|t| t == reading),
            "{:?}",
            diff_side_labels(&test)
        );

        // A modest drag right, well inside both floors.
        let y = f64::from(pane_top(&test) + 120.);
        test.press_cursor((f64::from(opened), y));
        test.move_cursor((f64::from(opened) + 40., y));
        test.sync_and_update();
        test.move_cursor((f64::from(opened) + 80., y));
        test.sync_and_update();
        test.release_cursor((f64::from(opened) + 80., y));
        test.sync_and_update();
        let dragged = list_split(&test);
        assert!(
            dragged > opened + 40.,
            "the drag did not move the splitter: {opened} then {dragged}"
        );
        let share = *view.changes_list_width.read();
        assert!(
            (0. ..100.).contains(&share),
            "the list's share is kept as {share}, not as a percentage of the pane"
        );

        // Another commit chosen: the tab is laid out anew, at the width dragged to.
        click_row(&mut test, 3);
        let mut diff = view.diff;
        test.run_in(|| {
            diff.write()
                .changes_arrived(Comparison::Commit(oid(3)), answer_with(3, 3))
        });
        for _ in 0..4 {
            test.sync_and_update();
        }
        let again = list_split(&test);
        assert!(
            (again - dragged).abs() < 4.,
            "the list was laid out at {again} px after being dragged to {dragged}"
        );
        assert!(
            diff_side_labels(&test).iter().any(|t| t == reading),
            "the diff side is not drawn: {:?}",
            diff_side_labels(&test)
        );
    }

    fn toggle_focus(test: &mut TestingRunner, focused: bool) {
        test.run_in(|| Platform::get().is_app_focused.set(focused));
        test.sync_and_update();
    }

    /// R10.1 and the QA brief, headless with focus set: gaining focus asks for a refresh,
    /// and that submit is all the UI thread does — no row cleared, nothing else asked;
    /// losing focus asks nothing; each gain asks once, so focus flapping asks twice and
    /// leaves superseding the first to the worker (`a_refresh_asked_twice_at_once_draws_one_
    /// answer_of_each`). Caught by: no refresh on focus, one on losing it, one per render,
    /// or work besides the submit.
    #[test]
    fn gaining_focus_asks_for_a_refresh_and_nothing_else() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        let from = submitted.borrow().len();
        test.sync_and_update();
        assert_eq!(
            requests_since(&submitted, from),
            [],
            "rendering asked something"
        );

        toggle_focus(&mut test, false);
        assert_eq!(
            requests_since(&submitted, from),
            [],
            "losing focus asked something"
        );
        toggle_focus(&mut test, true);
        assert_eq!(requests_since(&submitted, from), [Request::Refresh]);
        assert_eq!(view.rows.read().len(), 10, "focus touched the rows");
        assert_eq!(*view.progress.read(), received(10, true));

        toggle_focus(&mut test, false);
        toggle_focus(&mut test, true);
        assert_eq!(
            requests_since(&submitted, from),
            [Request::Refresh, Request::Refresh]
        );
    }

    /// R10.1: the Refresh action, through the accelerator table's chord on this platform,
    /// asks for a refresh and nothing else. Caught by: the action placed in the table and
    /// never acted on, or acting by more than a submit.
    #[test]
    fn the_refresh_chord_asks_for_a_refresh() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        let from = submitted.borrow().len();
        press_chord(&mut test, Action::Refresh);
        assert_eq!(requests_since(&submitted, from), [Request::Refresh]);
        assert_eq!(view.rows.read().len(), 10);
    }

    /// Presses the sidebar's filter field, so it holds focus.
    fn focus_sidebar_filter(test: &mut TestingRunner) {
        let field = test
            .find(|node, element| {
                Paragraph::try_downcast(element)
                    .filter(|paragraph| {
                        paragraph
                            .spans
                            .iter()
                            .any(|span| span.text.as_ref() == cairn_ui::SIDEBAR_FILTER_PLACEHOLDER)
                    })
                    .map(|_| node.layout().area.center())
            })
            .unwrap_or_else(|| panic!("no filter field in the sidebar"));
        test.click_cursor((f64::from(field.x), f64::from(field.y)));
        test.sync_and_update();
        test.sync_and_update();
    }

    /// C15, R7.1: a focused text field hands the window's chords and the held modifiers to the
    /// window. With the sidebar's filter focused, the Refresh chord asks for a refresh and types
    /// nothing into the field, and a row pressed with the extending chord held — the hold made
    /// while the field had focus — selects it beside the row selected. Caught by: a field whose
    /// key handler claims every key, which cancels the window's global key event, so F5 is
    /// unheard and `HeldKeys` never sees ⌘ or Ctrl go down (the hole recon found in Freya's
    /// dispatch, `docs/research/staging-and-commit/freya-ui-apis.md` §2).
    #[test]
    fn a_focused_filter_field_hands_the_windows_chords_and_held_keys_to_the_window() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        click_row(&mut test, 2);
        focus_sidebar_filter(&mut test);
        let from = submitted.borrow().len();
        press_chord(&mut test, Action::Refresh);
        assert_eq!(
            requests_since(&submitted, from)
                .into_iter()
                .filter(|request| matches!(request, Request::Refresh))
                .count(),
            1,
            "the Refresh chord went unheard while the filter had focus"
        );
        assert_eq!(
            view.sidebar.filter_text.peek().as_str(),
            "",
            "the chord typed"
        );

        focus_sidebar_filter(&mut test);
        extend_row(&mut test, 7);
        assert_eq!(
            selection(view),
            (Some(RowId::Commit(oid(2))), Some(RowId::Commit(oid(7)))),
            "the extending chord held while the filter had focus was never heard"
        );
    }

    /// What a discard of one modified file would cost, for a confirmation to draw.
    fn one_file_discard() -> cairn_model::Consequence {
        discard_of("src/lib.rs")
    }

    /// What a discard of the one modified file at `path` would cost.
    fn discard_of(path: &str) -> cairn_model::Consequence {
        use cairn_model::{Consequence, DiscardedFile, FileLoss};
        Consequence::DiscardFiles {
            files: vec![DiscardedFile {
                path: RepoPath::from(path),
                loss: FileLoss::Modified {
                    index: oid(1),
                    working_tree: Some(oid(2)),
                    executable: false,
                    lines: Some(3),
                    mode: None,
                },
            }],
        }
    }

    /// Opens a confirmation of [`one_file_discard`] whose token goes to the returned list.
    fn confirm_open(
        test: &mut TestingRunner,
        view: View,
    ) -> Rc<RefCell<Vec<cairn_model::Confirmed>>> {
        let tokens: Rc<RefCell<Vec<cairn_model::Confirmed>>> = Rc::default();
        let kept = tokens.clone();
        let mut confirming = view.confirming;
        confirming.set(Some(Confirming::new(
            "Discard changes",
            one_file_discard(),
            move |token| kept.borrow_mut().push(token),
        )));
        for _ in 0..4 {
            test.sync_and_update();
        }
        tokens
    }

    /// A confirmation of a discard at `path` whose token goes to `kept`.
    fn asking(path: &str, kept: &Rc<RefCell<Vec<cairn_model::Confirmed>>>) -> Confirming {
        let kept = kept.clone();
        Confirming::new("Discard changes", discard_of(path), move |token| {
            kept.borrow_mut().push(token)
        })
    }

    fn settle(test: &mut TestingRunner) {
        for _ in 0..4 {
            test.sync_and_update();
        }
    }

    /// QA P1, R7.4, L3: a confirmation replaced in place by another — B set while A is open —
    /// draws B and hands B's token, built from B's consequence, to B's continuation; A's gets
    /// nothing. Caught by: a dialog whose buttons keep the first confirmation's handlers (the
    /// toolkit keeps a component's old props when they compare equal), which spends a token
    /// naming A's paths under B's words.
    #[test]
    fn a_confirmation_replaced_in_place_hands_its_own_token_to_its_own_continuation() {
        let (mut test, view, _) = launch((0..10).map(row).collect(), received(10, true));
        let (first, second) = (
            Rc::<RefCell<Vec<cairn_model::Confirmed>>>::default(),
            Rc::<RefCell<Vec<cairn_model::Confirmed>>>::default(),
        );
        let mut confirming = view.confirming;
        confirming.set(Some(asking("src/lib.rs", &first)));
        settle(&mut test);
        confirming.set(Some(asking("src/other.rs", &second)));
        settle(&mut test);
        assert!(
            texts(&test)
                .iter()
                .any(|t| t == &discard_of("src/other.rs").prompt()),
            "{:?}",
            texts(&test)
        );
        click_label(&mut test, "Discard Changes in 1 File");
        assert!(
            first.borrow().is_empty(),
            "the replaced confirmation got a token"
        );
        assert_eq!(second.borrow().len(), 1, "the confirmation drawn got none");
        assert_eq!(
            second.borrow()[0].prompt(),
            discard_of("src/other.rs").prompt(),
            "the token names what was drawn"
        );
    }

    /// QA P2, R7.4: a confirmation opened by the previous one's answer — its continuation asks
    /// for the next — answers afresh: Escape dismisses it, and so does Cancel, pressed twice
    /// harmlessly; confirming builds exactly one token for it. Caught by: an answered flag that
    /// outlives the confirmation it guarded, leaving the next one deaf while the window's chords
    /// are inert — the window stuck.
    #[test]
    fn a_confirmation_opened_by_the_last_ones_answer_answers_afresh() {
        let (mut test, view, _) = launch((0..10).map(row).collect(), received(10, true));
        let (first, second) = (
            Rc::<RefCell<Vec<cairn_model::Confirmed>>>::default(),
            Rc::<RefCell<Vec<cairn_model::Confirmed>>>::default(),
        );
        let mut confirming = view.confirming;
        let next = Rc::new(RefCell::new(Some(asking("src/other.rs", &second))));
        let opener = view.confirming;
        let chain = move |first: &Rc<RefCell<Vec<cairn_model::Confirmed>>>,
                          next: &Rc<RefCell<Option<Confirming>>>| {
            let (first, next) = (first.clone(), next.clone());
            Confirming::new("Discard changes", discard_of("src/lib.rs"), move |token| {
                first.borrow_mut().push(token);
                let mut confirming = opener;
                if let Some(next) = next.borrow_mut().take() {
                    confirming.set(Some(next));
                }
            })
        };

        // Escape dismisses the second.
        confirming.set(Some(chain(&first, &next)));
        settle(&mut test);
        click_label(&mut test, "Discard Changes in 1 File");
        settle(&mut test);
        assert_eq!(first.borrow().len(), 1);
        assert!(
            view.confirming.peek().is_some(),
            "the next confirmation did not open"
        );
        test.press_key(Key::Named(NamedKey::Escape));
        settle(&mut test);
        assert!(view.confirming.peek().is_none(), "Escape did nothing");

        // Cancel dismisses the second, and a second press of it is harmless.
        *next.borrow_mut() = Some(asking("src/other.rs", &second));
        confirming.set(Some(chain(&first, &next)));
        settle(&mut test);
        click_label(&mut test, "Discard Changes in 1 File");
        settle(&mut test);
        assert!(view.confirming.peek().is_some());
        let cancel = test
            .find(|node, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == cairn_ui::CANCEL_CAPTION)
                    .map(|_| node.layout().area.center())
            })
            .unwrap_or_else(|| panic!("no Cancel"));
        let at = (f64::from(cancel.x), f64::from(cancel.y));
        test.click_cursor(at);
        settle(&mut test);
        assert!(view.confirming.peek().is_none(), "Cancel did nothing");
        test.click_cursor(at);
        settle(&mut test);

        // Confirming the second builds exactly one token, for the second.
        *next.borrow_mut() = Some(asking("src/other.rs", &second));
        confirming.set(Some(chain(&first, &next)));
        settle(&mut test);
        click_label(&mut test, "Discard Changes in 1 File");
        settle(&mut test);
        click_label(&mut test, "Discard Changes in 1 File");
        settle(&mut test);
        assert!(view.confirming.peek().is_none(), "the second did not close");
        assert_eq!(
            second.borrow().len(),
            1,
            "the second built {} tokens",
            second.borrow().len()
        );
        assert_eq!(
            second.borrow()[0].prompt(),
            discard_of("src/other.rs").prompt()
        );
        assert_eq!(first.borrow().len(), 3);
    }

    /// C17, R7.4 through the window: while a confirmation is open the window's chords do
    /// nothing — Refresh asks for nothing, a tab chord shows no tab — and a press behind it
    /// selects nothing there but cancels; Return, with focus on Cancel, cancels; the button
    /// hands the token, built from the consequence drawn, where the asking view said, and
    /// closes the dialog; once closed the chords act again. Caught by: a window that acts on
    /// a chord under the dialog (its key listener runs before the dialog's), a dialog a press
    /// can reach past, or a token that goes nowhere.
    #[test]
    fn no_chord_acts_under_a_confirmation_and_its_token_goes_where_it_was_asked() {
        let (mut test, view, submitted) = launch((0..10).map(row).collect(), received(10, true));
        click_row(&mut test, 2);
        let tokens = confirm_open(&mut test, view);
        assert!(
            texts(&test)
                .iter()
                .any(|t| t == "Discard Changes in 1 File"),
            "{:?}",
            texts(&test)
        );
        let from = submitted.borrow().len();
        press_chord(&mut test, Action::Refresh);
        press_chord(&mut test, Action::ShowChangesTab);
        for _ in 0..3 {
            test.press_key(Key::Named(NamedKey::Tab));
            test.sync_and_update();
        }
        press_chord(&mut test, Action::Refresh);
        assert_eq!(
            requests_since(&submitted, from),
            [],
            "a chord acted under it"
        );
        assert_eq!(*view.detail_tab.read(), DetailTab::Commit);
        assert!(view.confirming.peek().is_some());

        // A press behind it, on a row: the row is not chosen, and the dialog is cancelled.
        click_row(&mut test, 5);
        assert_eq!(selection(view), (Some(RowId::Commit(oid(2))), None));
        assert!(view.confirming.peek().is_none(), "a press outside kept it");
        assert!(tokens.borrow().is_empty());

        // Return, on Cancel, cancels.
        let tokens = confirm_open(&mut test, view);
        test.press_key(Key::Named(NamedKey::Enter));
        test.sync_and_update();
        assert!(view.confirming.peek().is_none(), "Return did not cancel");
        assert!(tokens.borrow().is_empty(), "Return confirmed");

        // The button builds the token from the consequence drawn and hands it on.
        let tokens = confirm_open(&mut test, view);
        click_label(&mut test, "Discard Changes in 1 File");
        assert_eq!(tokens.borrow().len(), 1);
        assert_eq!(tokens.borrow()[0].prompt(), one_file_discard().prompt());
        assert!(view.confirming.peek().is_none());

        let from = submitted.borrow().len();
        press_chord(&mut test, Action::Refresh);
        assert_eq!(requests_since(&submitted, from), [Request::Refresh]);
    }

    /// R7.5: a context menu opens from anywhere in the window, since the window mounts the
    /// host every menu needs (`ContextMenuViewer`), without which the toolkit panics. Caught
    /// by: no host mounted.
    #[test]
    fn a_context_menu_opens_from_inside_the_window() {
        const MENU_ITEM: &str = "Copy Path";
        const STRIP: f32 = 40.;
        let (mut test, _) = TestingRunner::new(
            || {
                let view = use_consume::<View>();
                rect()
                    .expanded()
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(HEIGHT - STRIP))
                            .child(window(PATH, view, None, None)),
                    )
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::px(STRIP))
                            .on_secondary_down(|_| {
                                ContextMenu::open_from_down(
                                    Menu::new().child(MenuButton::new().child(MENU_ITEM)),
                                );
                            }),
                    )
            },
            (WINDOW_WIDTH, HEIGHT).into(),
            |runner| {
                runner.provide_root_context(|| {
                    test_view(
                        (0..3).map(row).collect(),
                        received(3, true),
                        FetchStatus::Idle,
                        None,
                    )
                })
            },
            1.,
        );
        test.sync_and_update();
        let at = (20., f64::from(HEIGHT - STRIP / 2.));
        test.move_cursor(at);
        for name in [
            freya_testing::prelude::MouseEventName::MouseDown,
            freya_testing::prelude::MouseEventName::MouseUp,
        ] {
            test.send_event(PlatformEvent::Mouse {
                name,
                cursor: at.into(),
                button: Some(MouseButton::Right),
            });
            test.sync_and_update();
        }
        test.sync_and_update();
        assert!(
            texts(&test).iter().any(|t| t == MENU_ITEM),
            "no menu: {:?}",
            texts(&test)
        );
    }

    /// C10 headless with focus set, through the real boundary: the window opens its history
    /// from its first refresh; a row is chosen; a ref moves; the window gains focus, the
    /// refresh finds the move and the history is reopened — the old rows retired to the
    /// worker — and the chosen row, arriving again, is drawn chosen (R10.5); gaining focus
    /// again with nothing moved reopens nothing. Caught by: focus that does not refresh, a
    /// reopen that drops the selection, or one on an unchanged snapshot.
    #[test]
    fn focus_gained_after_a_ref_moved_reopens_the_history_keeping_the_chosen_row() {
        use crate::session::{Worker, apply};
        use crate::worker::{Refreshable, next_update};

        let fixture = Refreshable::new("cairn-window-focus-reopens");
        let (handle, mut updates) = fixture.open();
        let (mut test, view, submitted) = launch(Vec::new(), Progress::opening());
        let mut forwarded = 0;
        // How many times the window asked for the history, and how many refs answers it
        // applied, so far.
        let (mut opens, mut refs) = (0, 0);
        // Hands what the window asked to the worker and applies what comes back, until
        // `done` holds of the window and the two counts.
        let mut pump = |test: &mut TestingRunner, done: &dyn Fn(View, usize, usize) -> bool| {
            loop {
                let asked: Vec<Request> = submitted.borrow()[forwarded..].to_vec();
                forwarded += asked.len();
                for request in asked {
                    if matches!(request, Request::OpenHistory { .. }) {
                        opens += 1;
                    }
                    handle.submit(request);
                }
                if done(view, opens, refs) {
                    return (opens, refs);
                }
                let update = next_update(&mut updates);
                if matches!(update, crate::worker::Update::Refs { .. }) {
                    refs += 1;
                }
                let record = {
                    let submitted = submitted.clone();
                    move |request| submitted.borrow_mut().push(request)
                };
                test.run_in(|| {
                    apply(
                        update,
                        view,
                        &Worker {
                            submit: &record,
                            refuse: &|_| {},
                            closing: false,
                        },
                    );
                });
                test.sync_and_update();
            }
        };
        let loaded = |view: View| {
            let progress = view.progress.peek();
            progress.has_rows() && *progress.status() == Status::Ready
        };

        submitted.borrow_mut().push(Request::Refresh);
        let (opened, _) = pump(&mut test, &|view, opens, _| opens == 1 && loaded(view));
        assert_eq!(opened, 1, "the first refresh did not open");
        let chosen = fixture.commits[1];
        let subject = {
            let rows = view.rows.peek();
            let Some(at) = rows.position(RowId::Commit(chosen)) else {
                panic!("the chosen commit was not drawn");
            };
            match rows.row(at).map(|row| row.content()) {
                Some(cairn_model::RowContent::Commit(commit)) => commit.summary,
                Some(cairn_model::RowContent::Stash(stash)) => stash.message,
                None => unreachable!("the row was found"),
            }
        };
        for _ in 0..3 {
            test.sync_and_update();
        }
        click_label(&mut test, &subject);
        test.sync_and_update();
        assert_eq!(*view.selected.peek(), Some(RowId::Commit(chosen)));

        fixture.point("topic", fixture.commits[3]);
        toggle_focus(&mut test, false);
        toggle_focus(&mut test, true);
        let (reopened, seen) = pump(&mut test, &|view, opens, _| opens == 2 && loaded(view));
        assert_eq!(
            reopened, 2,
            "focus after a moved ref did not reopen the history"
        );
        assert!(
            submitted.borrow().iter().any(|request| matches!(
                request,
                Request::Retire(retired) if retired.replaced_rows().is_some()
            )),
            "the old rows were not handed to the worker"
        );
        assert_eq!(
            *view.selected.peek(),
            Some(RowId::Commit(chosen)),
            "the selection was lost"
        );
        assert!(
            view.rows.peek().position(RowId::Commit(chosen)).is_some(),
            "the chosen commit did not arrive again in the reopened rows"
        );

        // Nothing moved: the refresh's refs arrive, and nothing reopens.
        toggle_focus(&mut test, false);
        toggle_focus(&mut test, true);
        let (quiet, _) = pump(&mut test, &|_, _, refs| refs > seen);
        assert_eq!(quiet, 2, "focus with nothing moved reopened the history");
    }
}
