//! The window's contents, drawn from the view state.

use std::rc::Rc;

use cairn_model::{HistoryRow, RemoteSummary, RowContent, RowId, Secret};
use cairn_ui::{CommitRow, CredentialPrompt, HistoryHeader, HistoryList, ROW_HEIGHT, RowRender};
use freya::prelude::*;

use crate::fetch_state::{FetchStatus, PromptView};
use crate::history_state::{Progress, Status};
use crate::worker::{Replier, Reply, Request};
use crate::{PAGE_ROWS, status_text};

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
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View")
            .field("fetch", &*self.fetch.read())
            .field("prompt", &*self.prompt.read())
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

    rect()
        .expanded()
        .theme_background()
        .child(title_bar(
            opened,
            &counted,
            &fetch,
            view.fetch,
            &view.remotes.read(),
            submit.clone(),
        ))
        .maybe_child(status_text::fetch_line(&fetch).map(|line| {
            let failed = matches!(fetch, FetchStatus::Failed { .. });
            banner(line, failed)
        }))
        .child(HistoryHeader::new())
        .child(match status_text::placeholder(&status, has_rows) {
            Some(message) => notice(message, opened),
            None => history(view.rows, lanes, view.selected, view.progress, submit),
        })
        .maybe(has_rows, |el| match &status {
            Status::Failed(message) => el.child(banner(message.clone(), true)),
            _ => el,
        })
        .maybe_child(prompt.map(|prompt| dialog(prompt, &fetch, view.prompt, answer)))
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

fn history(
    rows: State<Vec<HistoryRow>>,
    lanes: usize,
    mut selected: State<Option<RowId>>,
    mut progress: State<Progress>,
    submit: Option<Rc<dyn Fn(Request)>>,
) -> Element {
    HistoryList::new(rows, move |render: RowRender| {
        // No wildcard arm: a new row kind must fail to compile here.
        match render.row.content {
            RowContent::Commit(commit) => CommitRow::new(commit, render.row.graph, render.lanes)
                .selected(render.selected)
                .into(),
        }
    })
    .lanes(lanes)
    .selected(*selected.read())
    .on_select(move |id: RowId| selected.set(Some(id)))
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
        .maybe_child(fetch_button(fetch, fetch_state, remotes, submit))
        .into()
}

/// "Fetch <remote>" for the default remote while nothing is in flight;
/// "Cancel" while a fetch is; nothing when the repository has no remote to
/// fetch. The press itself marks the fetch as starting, so a second press
/// before the worker answers has no button to land on.
fn fetch_button(
    fetch: &FetchStatus,
    mut fetch_state: State<FetchStatus>,
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

    use cairn_model::{CommitSummary, EdgeSegment, GraphRow, Lane, Oid};
    use freya_testing::TestingRunner;

    use crate::fetch_state::{FetchStatus, PromptView};

    use cairn_ui::{COLUMN_GAP, ROW_PADDING, graph_width};

    use super::*;
    use crate::history_state;

    const HEIGHT: f32 = 600.;
    const PATH: &str = "/home/ada/engine";

    fn row(n: usize) -> HistoryRow {
        row_in_lane(n, 0)
    }

    fn row_in_lane(n: usize, lane: usize) -> HistoryRow {
        let mut bytes = [0u8; 20];
        bytes[12..20].copy_from_slice(&(n as u64).to_be_bytes());
        let id = Oid::from_bytes(&bytes).unwrap();
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
}
