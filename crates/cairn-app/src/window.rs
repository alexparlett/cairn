//! The window's contents, drawn from the view state.

use std::rc::Rc;

use cairn_model::{HistoryRow, RowContent, RowId};
use cairn_ui::{CommitRow, HistoryHeader, HistoryList, ROW_HEIGHT, RowRender};
use freya::prelude::*;

use crate::history_state::{Progress, Status};
use crate::worker::Request;
use crate::{PAGE_ROWS, status_text};

/// `submit` is `None` when no repository could be opened.
pub fn window(
    opened: &str,
    rows: State<Vec<HistoryRow>>,
    progress: State<Progress>,
    selected: State<Option<RowId>>,
    submit: Option<Rc<dyn Fn(Request)>>,
) -> Element {
    let status = progress.read().status().clone();
    let lanes = progress.read().lanes();
    let has_rows = progress.read().has_rows();
    let counted = status_text::loaded_count(&progress.read());

    rect()
        .expanded()
        .theme_background()
        .child(title_bar(opened, &counted))
        .child(HistoryHeader::new())
        .child(match status_text::placeholder(&status, has_rows) {
            Some(message) => notice(message, opened),
            None => history(rows, lanes, selected, progress, submit),
        })
        .maybe(has_rows, |el| match &status {
            Status::Failed(message) => el.child(banner(message.clone())),
            _ => el,
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

fn title_bar(path: &str, counted: &str) -> Element {
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
        .into()
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

fn banner(message: String) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::px(ROW_HEIGHT))
        .cross_align(Alignment::center())
        .padding(Gaps::new(0., 12., 0., 12.))
        .background(get_theme_or_default().read().colors().surface_tertiary)
        .child(
            label()
                .text(message)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(13.)
                .color(get_theme_or_default().read().colors().error),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use cairn_model::{CommitSummary, EdgeSegment, GraphRow, Lane, Oid};
    use freya_testing::TestingRunner;

    use super::*;

    const HEIGHT: f32 = 600.;
    const PATH: &str = "/home/ada/engine";

    fn row(n: usize) -> HistoryRow {
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
                lane: Lane::new(0),
                edges: vec![EdgeSegment::passing(Lane::new(0))],
            },
        }
    }

    #[derive(Clone)]
    struct Fixture {
        rows: State<Vec<HistoryRow>>,
        progress: State<Progress>,
    }

    type Submitted = Rc<RefCell<Vec<Request>>>;

    fn launch(initial: Vec<HistoryRow>, progress: Progress) -> (TestingRunner, Fixture, Submitted) {
        let submitted = Submitted::default();
        let app = {
            let submitted = submitted.clone();
            move || {
                let fixture = use_consume::<Fixture>();
                let selected = use_state(|| None::<RowId>);
                let submitted = submitted.clone();
                let submit: Rc<dyn Fn(Request)> =
                    Rc::new(move |request| submitted.borrow_mut().push(request));
                window(PATH, fixture.rows, fixture.progress, selected, Some(submit))
            }
        };
        let (mut test, fixture) = TestingRunner::new(
            app,
            (800., HEIGHT).into(),
            move |runner| {
                runner.provide_root_context(|| Fixture {
                    rows: State::create(initial),
                    progress: State::create(progress),
                })
            },
            1.,
        );
        test.sync_and_update();
        (test, fixture, submitted)
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
        let cases: [(&str, Vec<HistoryRow>, Progress, &[&str]); 5] = [
            (
                "loading",
                Vec::new(),
                Progress::opening(),
                &["Reading history…", PATH],
            ),
            (
                "empty",
                Vec::new(),
                received(0, true),
                &["No commits yet.", "0 commits"],
            ),
            (
                "failed before any rows",
                Vec::new(),
                failed(0, "no git repository at /home/ada/engine"),
                &["no git repository at /home/ada/engine", PATH],
            ),
            (
                "failed after rows",
                (0..3).map(row).collect(),
                failed(3, "failed to read commit abc"),
                &["failed to read commit abc", "commit 0", "3 commits"],
            ),
            (
                "ready",
                (0..3).map(row).collect(),
                received(3, false),
                &["commit 0", "commit 2", "3 commits…"],
            ),
        ];

        for (state, rows, progress, sentences) in cases {
            let (test, _, _) = launch(rows, progress);
            let shown = texts(&test);
            for sentence in sentences {
                assert!(
                    shown.iter().any(|text| text == sentence),
                    "the {state} window does not show {sentence:?}; it shows {shown:?}"
                );
            }
        }
    }

    #[test]
    fn a_placeholder_replaces_the_list_and_rows_replace_the_placeholder() {
        let (loading, _, _) = launch(Vec::new(), Progress::opening());
        assert!(
            !texts(&loading)
                .iter()
                .any(|text| text.starts_with("commit ")),
            "a loading window drew rows"
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
    fn reaching_the_end_asks_for_one_page_until_that_page_arrives() {
        let first = 200;
        let (mut test, fixture, submitted) =
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
        assert_eq!(submitted.borrow().as_slice(), &[more.clone()]);

        // Clicks select rows, which re-renders every one in view.
        for n in 0..5 {
            test.click_cursor((100., 200. + n as f64 * ROW_HEIGHT as f64));
        }
        scroll_to_end(&mut test, first);
        assert_eq!(
            submitted.borrow().len(),
            1,
            "a second page was asked for before the first arrived"
        );

        let (mut rows, mut progress) = (fixture.rows, fixture.progress);
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
}
