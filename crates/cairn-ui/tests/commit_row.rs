//! Headless component tests for `CommitRow`: Fork's history row.

use cairn_model::{CommitSummary, EdgeSegment, Lane, Oid, RowEdges};

use cairn_ui::style::{metrics, tones};
use cairn_ui::{
    AUTHOR_WIDTH, CommitRow, DATE_WIDTH, ID_WIDTH, LocalZone, ROW_FONT_SIZE, ROW_HEIGHT,
    avatar_colour, graph_width,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const SUBJECT: &str = "Teach the walker to stop";
const AUTHOR: &str = "Ada Lovelace";
const HEX: &str = "0123456789abcdef0123456789abcdef01234567";
/// 2024-03-09 16:05:00 UTC.
const WHEN: i64 = 1_710_000_300;

fn commit() -> (CommitSummary, RowEdges) {
    let id = Oid::parse(HEX).unwrap_or_else(|_| unreachable!("40 hex digits is a SHA-1"));
    (
        CommitSummary {
            id,
            parent_count: 0,
            summary: SUBJECT.to_owned(),
            author_name: AUTHOR.to_owned(),
            author_time: WHEN,
        },
        RowEdges {
            lane: Lane::new(0),
            edges: vec![EdgeSegment::passing(Lane::new(0))],
        },
    )
}

const WIDTH: f32 = 900.;
/// `WHEN` an hour east of UTC, as Fork draws a date: `d Mon yyyy HH:mm`.
const DATE: &str = "9 Mar 2024 17:05";

/// A zone an hour east of UTC, handed to the rows as the window hands them the system's.
fn zone() -> LocalZone {
    LocalZone::read(jiff::tz::TimeZone::fixed(jiff::tz::Offset::constant(1)))
}

#[derive(Clone, Copy)]
struct Setup {
    lanes: usize,
}

/// An unselected row, a selected one, then `HEAD`'s.
fn app() -> impl IntoElement {
    let Setup { lanes } = use_consume::<Setup>();
    let (commit, graph) = commit();
    let mut second = commit.clone();
    second.summary = format!("selected {SUBJECT}");
    let mut head = commit.clone();
    head.summary = format!("head {SUBJECT}");
    head.author_name = "Grace Hopper".to_owned();
    head.author_time = WHEN + 60;
    head.id = Oid::parse(HEAD_HEX).unwrap_or_else(|_| unreachable!("40 hex digits is a SHA-1"));
    rect()
        .width(Size::fill())
        .child(CommitRow::new(commit, graph.clone(), lanes).zone(zone()))
        .child(
            CommitRow::new(second, graph.clone(), lanes)
                .selected(true)
                .zone(zone()),
        )
        .child(CommitRow::new(head, graph, lanes).head(true).zone(zone()))
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

/// The label reading exactly `text`: its weight and colour.
fn style_of(test: &TestingRunner, text: &str) -> (Option<FontWeight>, Option<Fill>) {
    test.find(|_, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|label| {
                (
                    label.text_style_data.font_weight,
                    label.text_style_data.color.clone(),
                )
            })
    })
    .unwrap_or_else(|| panic!("no label reads {text:?}"))
}

/// Fork's history row: 23 px tall; the graph, then the subject, then the author after an
/// avatar of their initials in a rounded square of the name's colour, the 7-character id and
/// the date, local, in Fork's `d Mon yyyy HH:mm`; the author column, the id's and the date's
/// at their widths whatever the subject; and `HEAD`'s row bold throughout. Caught by: the old
/// 26 px row, the date drawn in UTC or as `YYYY-MM-DD`, no avatar or another's initials or
/// colour, the columns out of Fork's order, or `HEAD`'s author, id or date left regular.
#[test]
fn a_history_row_is_forks() {
    let test = launch();

    // Each row's band, the full width and 23 px tall, one under the other.
    let mut tops: Vec<f32> = test.find_many(|node, element| {
        let area = node.layout().area;
        Rect::try_downcast(element)
            .filter(|_| area.width() == WIDTH && area.height() == 23.)
            .map(|_| area.min_y())
    });
    tops.sort_by(f32::total_cmp);
    tops.dedup();
    assert_eq!(tops, [0., 23., 46.], "three rows of 23 px");
    assert_eq!(ROW_HEIGHT, metrics::HISTORY_ROW);

    let subject = column(&test, SUBJECT);
    let initials = column(&test, "AL");
    let author = column(&test, AUTHOR);
    let id = column(&test, &HEX[..7]);
    let date = column(&test, DATE);
    assert!(
        subject.0 < initials.0 && initials.0 < author.0 && author.0 < id.0 && id.0 < date.0,
        "columns out of Fork's order: subject {subject:?}, avatar {initials:?}, author \
         {author:?}, id {id:?}, date {date:?}"
    );
    assert_eq!(id.1, ID_WIDTH, "the commit column");
    assert_eq!(date.1, DATE_WIDTH, "the date column");
    assert!(subject.1 > 0., "the subject column has no width");

    // The avatar: a square of the name's colour, its initials white and bold, the author's
    // column ending where it always does.
    let squares: Vec<(f32, f32, f32, Fill)> = test.find_many(|node, element| {
        let area = node.layout().area;
        Rect::try_downcast(element)
            .filter(|_| area.width() == metrics::AVATAR && area.height() == metrics::AVATAR)
            .map(|rect| {
                (
                    area.min_x(),
                    area.max_x(),
                    area.min_y(),
                    rect.style.background.clone(),
                )
            })
    });
    let first = squares
        .iter()
        .find(|(_, _, top, _)| *top < ROW_HEIGHT)
        .unwrap_or_else(|| panic!("no avatar on the first row: {squares:?}"));
    assert_eq!(
        first.3,
        Fill::from(avatar_colour(AUTHOR)),
        "the avatar's colour"
    );
    assert!(first.0 <= initials.0 && initials.0 + initials.1 <= first.1);
    assert_eq!(
        style_of(&test, "AL"),
        (Some(FontWeight::BOLD), Some(Fill::from(tones::AVATAR_TEXT)))
    );
    assert_eq!(
        author.0 + author.1,
        first.0 + AUTHOR_WIDTH,
        "the author column is {AUTHOR_WIDTH} px from its avatar"
    );
    assert!(column(&test, "GH").0 > subject.0, "HEAD's row's avatar");

    // `HEAD`'s row bold throughout, the others regular.
    for text in [
        format!("head {SUBJECT}"),
        "Grace Hopper".to_owned(),
        "9 Mar 2024 17:06".to_owned(),
        HEAD_HEX[..7].to_owned(),
    ] {
        assert_eq!(style_of(&test, &text).0, Some(FontWeight::BOLD), "{text:?}");
    }
    for text in [SUBJECT, AUTHOR, DATE] {
        assert_ne!(style_of(&test, text).0, Some(FontWeight::BOLD), "{text:?}");
    }
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

/// The widest date Fork's format draws, and the widest short id. Caught by: a column sized
/// for a narrower date than the format can draw.
#[test]
fn the_date_and_the_short_id_fit_their_columns() {
    for (text, width, name) in [
        ("28 May 2026 23:59", DATE_WIDTH, "date"),
        ("30 Sep 2026 00:00", DATE_WIDTH, "date"),
        ("ddddddd", ID_WIDTH, "commit"),
        ("0000000", ID_WIDTH, "commit"),
        (&HEX[..7], ID_WIDTH, "commit"),
    ] {
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

/// `HEAD`'s row's id, its short form unlike the other rows'.
const HEAD_HEX: &str = "aaaabbbbccccddddeeeeffff0000111122223333";
/// A row beyond `HEAD`'s reach: its id, its short form unlike the other rows'.
const BEYOND_HEX: &str = "1111222233334444555566667777888899990000";

const LOST_HEX: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// A lost row beside a reached one: the four texts of each, by the subject they lead.
fn lost_app() -> impl IntoElement {
    let (commit, graph) = commit();
    let mut lost = commit.clone();
    lost.summary = format!("lost {SUBJECT}");
    lost.author_name = "Lost Author".to_owned();
    lost.author_time = WHEN + 86_400;
    lost.id = Oid::parse(LOST_HEX).unwrap_or_else(|_| unreachable!("40 hex digits is a SHA-1"));
    rect()
        .width(Size::fill())
        .child(CommitRow::new(commit, graph.clone(), 3).zone(zone()))
        .child(CommitRow::new(lost, graph, 3).lost(true).zone(zone()))
}

/// As Fork draws a commit `HEAD` cannot reach: its subject, author, short id and date in Fork
/// Dark's grey for such a row, and nothing else dimmed: no element of the row is drawn translucent, so the
/// graph's lanes, edges and node and its chips keep their own colours; a reached row's subject
/// is the body's text. Caught by: the whole row drawn at an opacity, a text left undimmed, or
/// a reached row dimmed.
#[test]
fn a_lost_rows_text_alone_is_dimmed() {
    let (mut test, ()) = TestingRunner::new(lost_app, (WIDTH, 200.).into(), |_| {}, 1.);
    test.sync_and_update();
    let colour = |text: &str| {
        test.find(|_, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .map(|label| label.text_style_data.color)
        })
        .unwrap_or_else(|| panic!("no label reads {text:?}"))
    };
    let placeholder = Some(Fill::from(cairn_ui::style::tones::UNREACHABLE_TEXT));
    for text in [
        &format!("lost {SUBJECT}"),
        "Lost Author",
        &LOST_HEX[..7],
        "10 Mar 2024 17:05",
    ] {
        assert_eq!(colour(text), placeholder, "{text:?} is not dimmed");
    }
    assert_eq!(
        colour(SUBJECT),
        Some(Fill::from(cairn_ui::style::tones::TEXT))
    );
    let translucent = test.find_many(|_, element| {
        Rect::try_downcast(element)
            .and_then(|rect| rect.effect)
            .and_then(|effect| effect.opacity)
    });
    assert!(translucent.is_empty(), "drawn translucent: {translucent:?}");
}

/// A row `HEAD` cannot reach — a commit or a stash's — beside one it reaches: its subject,
/// author, id and date in Fork Dark's grey for such a row, its avatar, graph and chips in
/// their own colours; selected, it takes the selection's tone. Caught by: the mark ignored,
/// the reached row greyed, or the grey kept over the selection's white.
#[test]
fn a_row_head_cannot_reach_is_greyed_and_one_it_reaches_is_not() {
    let (mut test, ()) = TestingRunner::new(
        || -> Element {
            let (commit, graph) = commit();
            let mut beyond = commit.clone();
            beyond.summary = format!("beyond {SUBJECT}");
            beyond.author_name = "Side Author".to_owned();
            beyond.author_time = WHEN + 120;
            beyond.id =
                Oid::parse(BEYOND_HEX).unwrap_or_else(|_| unreachable!("40 hex digits is a SHA-1"));
            let mut chosen = commit.clone();
            chosen.summary = format!("chosen {SUBJECT}");
            rect()
                .width(Size::fill())
                .child(CommitRow::new(commit, graph.clone(), 3).zone(zone()))
                .child(
                    CommitRow::new(beyond, graph.clone(), 3)
                        .head_reaches(false)
                        .zone(zone()),
                )
                .child(
                    CommitRow::new(chosen, graph, 3)
                        .head_reaches(false)
                        .selected(true)
                        .focused(true)
                        .zone(zone()),
                )
                .into()
        },
        (WIDTH, 200.).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    let grey = Some(Fill::from(tones::UNREACHABLE_TEXT));
    for text in [
        &format!("beyond {SUBJECT}"),
        "Side Author",
        &BEYOND_HEX[..7],
        "9 Mar 2024 17:07",
    ] {
        assert_eq!(style_of(&test, text).1, grey, "{text:?}");
    }
    assert_eq!(style_of(&test, SUBJECT).1, Some(Fill::from(tones::TEXT)));
    assert_eq!(style_of(&test, AUTHOR).1, Some(Fill::from(tones::TEXT)));
    assert_eq!(
        style_of(&test, &format!("chosen {SUBJECT}")).1,
        Some(Fill::from(tones::SELECTED_TEXT))
    );
    assert_eq!(
        style_of(&test, "SA").1,
        Some(Fill::from(tones::AVATAR_TEXT)),
        "the avatar keeps its colours"
    );
}
