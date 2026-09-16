//! Headless component tests for `CommitRow` and `HistoryHeader`.

use cairn_model::{CommitSummary, EdgeSegment, GraphRow, Lane, Oid};
use cairn_ui::{AUTHOR_WIDTH, CommitRow, DATE_WIDTH, HistoryHeader, ID_WIDTH};
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

fn app() -> impl IntoElement {
    let (commit, graph) = commit();
    rect()
        .width(Size::fill())
        .child(HistoryHeader::new())
        .child(CommitRow::new(commit, graph, 3))
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

fn launch() -> TestingRunner {
    let (mut test, ()) = TestingRunner::new(app, (900., 200.).into(), |_| {}, 1.);
    test.sync_and_update();
    test
}

#[test]
fn each_value_is_drawn_in_its_own_column_at_its_own_width() {
    let test = launch();

    let subject = column(&test, SUBJECT);
    let author = column(&test, AUTHOR);
    let id = column(&test, &HEX[..7]);
    let date = column(&test, "2024-03-09 16:05");

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
        ("Date (UTC)", "2024-03-09 16:05"),
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
