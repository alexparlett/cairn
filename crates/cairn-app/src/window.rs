//! The window's contents, drawn from the view state.

use std::rc::Rc;

use cairn_model::{HistoryRow, RemoteSummary, RowContent, RowId, Secret};
use cairn_ui::accelerators;
use cairn_ui::{
    CommitRow, CredentialPrompt, DETAIL_STRIP_HEIGHT, DetailTab, HistoryHeader, HistoryList,
    ROW_HEIGHT, RowRender,
};
use freya::prelude::*;

use crate::detail_pane::DetailPane;
use crate::diff_state::DiffState;
use crate::fetch_state::{FetchRefusal, FetchStatus, PromptView};
use crate::history_state::{Progress, Status};
use crate::worker::{Replier, Reply, Request};
use crate::{PAGE_ROWS, selection, shortcuts, status_text};

/// The detail pane's height until the splitter is dragged.
pub const PANE_HEIGHT: f32 = 260.0;
/// The least a drag leaves the pane; pulled past it, the pane collapses (Fork's gesture).
const PANE_MIN_HEIGHT: f32 = 90.0;
/// The least the commit list keeps when the window is squeezed.
const LIST_MIN_HEIGHT: f32 = 80.0;

/// The view state the window is drawn from. Handles, not values: the window
/// subscribes to what it reads.
#[derive(Clone, Copy)]
pub struct View {
    pub rows: State<Vec<HistoryRow>>,
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
    /// The detail pane's tab, kept for the session (R5.2).
    pub detail_tab: State<DetailTab>,
    pub pane_collapsed: State<bool>,
    /// The pane's height as last dragged; read when the pane is laid out again.
    pub pane_height: State<f32>,
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View")
            .field("fetch", &*self.fetch.read())
            .field("prompt", &*self.prompt.read())
            .field("refused", &*self.refused.read())
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
    let refused = view.refused.read().clone();

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
        // Every shortcut resolves through the accelerator table, wherever focus is (R8).
        .on_global_key_down(move |e: Event<KeyboardEventData>| {
            if let Some(action) = accelerators::resolve_key(&e) {
                shortcuts::act(action, view);
            }
        })
        .child(title_bar(
            opened,
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
        .child(split(list, DetailPane::new(view, submit).into(), view))
        .maybe_child(prompt.map(|prompt| dialog(prompt, &fetch, view.prompt, answer)))
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
    fetch: &FetchStatus,
    mut showing: State<Option<PromptView>>,
    answer: Option<Replier>,
) -> Element {
    // A prompt is only shown while a fetch is in flight (`session::apply` refuses one
    // otherwise), so the fallback names what asked when that holds no remote.
    let remote = fetch.remote_in_flight().unwrap_or("git").to_owned();
    let id = prompt.id;
    let on_submit = answer.clone();
    let on_cancel = answer;
    CredentialPrompt::new(remote, prompt.text)
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
    HistoryList::new(view.rows, move |render: RowRender| {
        // No wildcard arm: a new row kind must fail to compile here.
        match render.row.content {
            RowContent::Commit(commit) => CommitRow::new(commit, render.row.graph, render.lanes)
                .selected(render.selected)
                .into(),
        }
    })
    .lanes(lanes)
    .selected(*view.selected.read())
    .controller(view.history_scroll)
    // Choosing a row asks what it changed; the pane draws the answer for that row alone.
    .on_select(move |id: RowId| selection::choose(id, view, choosing.as_deref()))
    .on_reach_end(move |()| {
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

fn title_bar(
    path: &str,
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
            label()
                .text(path.to_owned())
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .width(Size::flex(1.))
                .font_size(13.)
                .color(get_theme_or_default().read().colors().text_secondary),
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
mod tests {
    use std::cell::RefCell;

    use cairn_model::{
        ChangeSet, ChangeStatus, ChangedFile, CommitDetails, CommitSummary, EdgeSegment, FileMode,
        GraphRow, Lane, Oid, RenameDetection, RepoPath, Signature, Timestamp,
    };
    use cairn_ui::accelerators::Action;
    use cairn_ui::{COLLAPSE_CAPTION, EXPAND_CAPTION};
    use freya_testing::TestingRunner;
    use freya_testing::prelude::{KeyboardEventName, PlatformEvent};

    use crate::detail_pane::{CHANGES_NOT_BUILT, NOTHING_SELECTED, READING};
    use crate::worker::Comparison;

    use crate::fetch_state::{FetchStatus, PromptView};

    use cairn_ui::{COLUMN_GAP, ROW_PADDING, graph_width};

    use super::*;
    use crate::history_state;

    const HEIGHT: f32 = 600.;
    const PATH: &str = "/home/ada/engine";

    fn row(n: usize) -> HistoryRow {
        row_in_lane(n, 0)
    }

    /// Row `n`'s id; its first byte differs between rows, so its short id does too.
    fn oid(n: usize) -> Oid {
        let mut bytes = [0u8; 20];
        bytes[0] = 0x10 + (n % 200) as u8;
        bytes[12..20].copy_from_slice(&(n as u64).to_be_bytes());
        Oid::from_bytes(&bytes).unwrap()
    }

    fn row_in_lane(n: usize, lane: usize) -> HistoryRow {
        let id = oid(n);
        HistoryRow {
            content: RowContent::Commit(CommitSummary {
                id,
                parents: Vec::new(),
                summary: format!("commit {n}"),
                author_name: "Ada".to_owned(),
                author_email: "ada@example.com".to_owned(),
                author_time: 0,
            }),
            graph: GraphRow {
                id,
                lane: Lane::new(lane),
                edges: vec![EdgeSegment::passing(Lane::new(lane))],
            },
        }
    }

    type Submitted = Rc<RefCell<Vec<Request>>>;

    /// What was answered: the prompt, and the secret's length (never its bytes) or `None`
    /// for a refusal.
    type Answered = Rc<RefCell<Vec<(crate::worker::PromptId, Option<usize>)>>>;

    fn launch(initial: Vec<HistoryRow>, progress: Progress) -> (TestingRunner, View, Submitted) {
        let (test, view, submitted, _) = launch_with(initial, progress, FetchStatus::Idle, None);
        (test, view, submitted)
    }

    fn launch_with(
        initial: Vec<HistoryRow>,
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
            (800., HEIGHT).into(),
            move |runner| {
                runner.provide_root_context(|| View {
                    rows: State::create(initial),
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
                    detail_tab: State::create(DetailTab::default()),
                    pane_collapsed: State::create(false),
                    pane_height: State::create(PANE_HEIGHT),
                })
            },
            1.,
        );
        for _ in 0..3 {
            test.sync_and_update();
        }
        (test, view, submitted, answered)
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
            Vec<HistoryRow>,
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

    #[test]
    fn the_graph_column_is_as_wide_as_the_widest_lane_seen() {
        let page: Vec<HistoryRow> = (0..3).map(|n| row_in_lane(n, 4)).collect();
        let mut progress = Progress::opening();
        progress.received(history_state::widest_lane(&page), true, page.len());
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
            ROW_PADDING + graph_width(5) + COLUMN_GAP,
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

        test.click_cursor((100., second as f64 + 5.));
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
            test.scroll((100., 200.), (0., -(rows as f64 * ROW_HEIGHT as f64)));
        };

        test.sync_and_update();
        assert!(
            submitted.borrow().is_empty(),
            "the top of the list asked for more"
        );

        scroll_to_end(&mut test, first);
        assert_eq!(submitted.borrow().as_slice(), std::slice::from_ref(&more));

        test.scroll((100., 200.), (0., first as f64 * ROW_HEIGHT as f64));
        scroll_to_end(&mut test, first);
        assert_eq!(
            submitted.borrow().len(),
            1,
            "a second page was asked for before the first arrived"
        );

        let (mut rows, mut progress) = (view.rows, view.progress);
        rows.write().extend((first..first * 2).map(row));
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
            test.scroll((100., 200.), (0., -(rows * ROW_HEIGHT as f64)));
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

        rows.write().extend((first..first * 2).map(row));
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

    /// The dialog's fallback when the view state holds a prompt with no fetch in flight
    /// (which `session::apply` prevents): it still names what asked.
    #[test]
    fn a_prompt_with_no_fetch_in_flight_is_attributed_to_git() {
        let (test, _, _, _) = launch_with(
            Vec::new(),
            received(0, true),
            FetchStatus::Idle,
            Some(PromptView {
                id: crate::worker::PromptId::for_tests(2),
                text: "Username for 'https://git.example.com/x': ".to_owned(),
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

    fn press_chord(test: &mut TestingRunner, action: Action) {
        let chord = accelerators::chord(action, accelerators::Os::current());
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
            pane(&test).iter().any(|t| t == CHANGES_NOT_BUILT),
            "{:?}",
            pane(&test)
        );

        click_row(&mut test, 1);
        click_row(&mut test, 5);
        assert_eq!(*view.detail_tab.read(), DetailTab::Changes);
        assert!(
            pane(&test).iter().any(|t| t == CHANGES_NOT_BUILT),
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
        assert!(!texts(&test).iter().any(|t| t == CHANGES_NOT_BUILT));

        click_label(&mut test, EXPAND_CAPTION);
        assert_eq!(*view.detail_tab.read(), DetailTab::Changes);
        assert!(
            pane(&test).iter().any(|t| t == CHANGES_NOT_BUILT),
            "{:?}",
            pane(&test)
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
        test.press_cursor((300., handle));
        test.move_cursor((300., handle - 80.));
        test.sync_and_update();
        test.move_cursor((300., handle - 120.));
        test.sync_and_update();
        test.release_cursor((300., handle - 120.));
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
}
