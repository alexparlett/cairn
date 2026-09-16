//! Headless component tests for `CommitRow` and `HistoryHeader`.

use cairn_model::{CommitSummary, EdgeSegment, GraphRow, Lane, Oid};
use cairn_ui::{
    AUTHOR_WIDTH, CommitRow, DATE_WIDTH, HistoryHeader, ID_WIDTH, ROW_FONT_SIZE, ROW_HEIGHT,
    graph_width,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const SUBJECT: &str = "Teach the walker to stop";
const AUTHOR: &str = "Ada Lovelace";
const HEX: &str = "0123456789abcdef0123456789abcdef01234567";
/// 2024-03-09 16:05:00 UTC.
const WHEN: i64 = 1_710_000_300;

fn commit() -> (CommitSummary, GraphRow) {
    let id = Oid::parse(HEX).unwrap_or_else(|_| unreachable!("40 hex digits is a SHA-1"));
    (
        CommitSummary {
            id,
            parents: Vec::new(),
            summary: SUBJECT.to_owned(),
            author_name: AUTHOR.to_owned(),
            author_email: "ada@example.com".to_owned(),
            author_time: WHEN,
        },
        GraphRow {
            id,
            lane: Lane::new(0),
            edges: vec![EdgeSegment::passing(Lane::new(0))],
        },
    )
}

const WIDTH: f32 = 900.;
const DATE: &str = "2024-03-09 16:05";

#[derive(Clone, Copy)]
struct Setup {
    lanes: usize,
}

/// The header, an unselected row, then a selected one.
fn app() -> impl IntoElement {
    let Setup { lanes } = use_consume::<Setup>();
    let (commit, graph) = commit();
    let mut second = commit.clone();
    second.summary = format!("selected {SUBJECT}");
    rect()
        .width(Size::fill())
        .child(HistoryHeader::new())
        .child(CommitRow::new(commit, graph.clone(), lanes))
        .child(CommitRow::new(second, graph, lanes).selected(true))
}

/// How wide `text` is at the rows' font size, laid out on its own.
fn measured(text: &'static str) -> f32 {
    let (mut test, ()) = TestingRunner::new(
        move || label().text(text).font_size(ROW_FONT_SIZE).max_lines(1),
        (WIDTH, 200.).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    column(&test, text).1
}

/// `(left, width)` of the label reading exactly `text`.
fn column(test: &TestingRunner, text: &str) -> (f32, f32) {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|_| {
                let area = node.layout().area;
                (area.min_x(), area.width())
            })
    })
    .unwrap_or_else(|| panic!("no label reads {text:?}"))
}

fn launch_with(lanes: usize) -> TestingRunner {
    let (mut test, ()) = TestingRunner::new(
        app,
        (WIDTH, 200.).into(),
        move |runner| {
            runner.provide_root_context(|| Setup { lanes });
        },
        1.,
    );
    test.sync_and_update();
    test
}

fn launch() -> TestingRunner {
    launch_with(3)
}

#[test]
fn each_value_is_drawn_in_its_own_column_at_its_own_width() {
    let test = launch();

    let subject = column(&test, SUBJECT);
    let author = column(&test, AUTHOR);
    let id = column(&test, &HEX[..7]);
    let date = column(&test, DATE);

    assert_eq!(author.1, AUTHOR_WIDTH, "the author column");
    assert_eq!(id.1, ID_WIDTH, "the commit column");
    assert_eq!(date.1, DATE_WIDTH, "the date column");
    assert!(subject.1 > 0., "the subject column has no width");

    assert!(
        subject.0 < author.0 && author.0 < id.0 && id.0 < date.0,
        "columns out of order: subject {subject:?}, author {author:?}, id {id:?}, date {date:?}"
    );
}

#[test]
fn every_heading_sits_over_its_column() {
    let test = launch();

    for (heading, value) in [
        ("Author", AUTHOR),
        ("Commit", &HEX[..7]),
        ("Date (UTC)", DATE),
    ] {
        assert_eq!(
            column(&test, heading),
            column(&test, value),
            "the {heading:?} heading does not line up with its column"
        );
    }

    let heading = column(&test, "Graph and subject");
    let subject = column(&test, SUBJECT);
    assert_eq!(
        heading.0 + heading.1,
        subject.0 + subject.1,
        "the subject column and its heading end in different places"
    );
}

#[test]
fn the_graph_column_widens_with_the_lanes_it_is_given() {
    let narrow = column(&launch_with(1), SUBJECT);
    let wide = column(&launch_with(5), SUBJECT);
    assert_eq!(
        wide.0 - narrow.0,
        graph_width(5) - graph_width(1),
        "the subject did not move over by the extra lanes"
    );
}

#[test]
fn a_selected_row_is_drawn_differently_from_an_unselected_one() {
    let test = launch();
    let full_rows = test.find_many(|node, element| {
        let area = node.layout().area;
        Rect::try_downcast(element)
            .filter(|_| area.width() == WIDTH && area.height() == ROW_HEIGHT)
            .map(|rect| (area.min_y(), rect.style.background))
    });
    let background_at = |row: f32| {
        full_rows
            .iter()
            .find(|(y, _)| *y == row * ROW_HEIGHT)
            .map(|(_, background)| background.clone())
            .unwrap_or_else(|| panic!("no row at {row}: {full_rows:?}"))
    };

    assert_ne!(
        background_at(1.),
        background_at(2.),
        "the selected row looks like the unselected one"
    );
}

#[test]
fn the_date_and_the_short_id_fit_their_columns() {
    for (text, width, name) in [(DATE, DATE_WIDTH, "date"), (&HEX[..7], ID_WIDTH, "commit")] {
        let needed = measured(text);
        assert!(
            needed > 0.,
            "no font drew {text:?}, so nothing was measured"
        );
        assert!(
            needed <= width,
            "the {name} column is {width}px but {text:?} needs {needed}px"
        );
    }
}
