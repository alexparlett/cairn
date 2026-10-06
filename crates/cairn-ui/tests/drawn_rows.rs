//! C16, headless: every row of the history list draws the subject, author, short id, date
//! and merge marker it drew before rows were slimmed.
//!
//! `DRAWN_BEFORE` is what the list drew before: it was asserted against the old rows — each
//! carrying its full parents and its author's address — while those rows were still what
//! the list read (commit `4205d5d`), and the slimmed rows, read through a history built
//! page by page, are now held to it. Each row is drawn by the window's own row,
//! `CommitRow`, and read back from the drawn tree: its labels' text, and the pixel at the
//! centre of its node, which a dot fills and a merge's ring leaves empty.

use cairn_model::{GraphRow, History, Lane, Oid, PagedCommit, RowContent, RowsPage};
use cairn_ui::graph_geometry::{lane_x, row_middle};
use cairn_ui::{CommitRow, HistoryList, ROW_HEIGHT, ROW_PADDING, RowRender};
use freya::engine::prelude::{Image, ImageInfo, raster_n32_premul};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 400.;

/// A row as the tests write it: its id's first byte, subject, author, author date and
/// parent count.
type Written = (u8, &'static str, &'static str, i64, usize);

/// Longer than a whole chunk of any text store.
fn long_subject() -> String {
    (0..7_000)
        .map(|n| format!("{n:09} "))
        .collect::<String>()
        .trim_end()
        .to_owned()
}

const LONG: &str = "<long>";

/// The rows, newest first: a lone tip, an octopus, a merge, an empty subject, a subject
/// longer than any chunk, names and subjects past ASCII, a root.
const WRITTEN: &[Written] = &[
    (0x10, "Tip", "Ada Lovelace", 1_710_000_300, 1),
    (
        0x21,
        "first line second line",
        "Margaret Hamilton",
        1_709_990_000,
        3,
    ),
    (0x32, "Merge side work", "Ada Lovelace", 1_700_000_000, 2),
    (0x43, "", "Grace Hopper", 1_600_000_059, 1),
    (0x54, LONG, "Ada Lovelace", 0, 1),
    (0x65, "Café crème", "Zoë Ångström", -1, 1),
    (0x76, "Begin the walk", "Ada Lovelace", 86_400 * 365, 0),
];

/// What each row drew before rows were slimmed: subject, author, short id, date, and whether
/// its node is a merge's ring.
const DRAWN_BEFORE: &[(&str, &str, &str, &str, bool)] = &[
    ("Tip", "Ada Lovelace", "10ababa", "2024-03-09 16:05", false),
    (
        "first line second line",
        "Margaret Hamilton",
        "21ababa",
        "2024-03-09 13:13",
        true,
    ),
    (
        "Merge side work",
        "Ada Lovelace",
        "32ababa",
        "2023-11-14 22:13",
        true,
    ),
    ("", "Grace Hopper", "43ababa", "2020-09-13 12:27", false),
    (LONG, "Ada Lovelace", "54ababa", "1970-01-01 00:00", false),
    (
        "Café crème",
        "Zoë Ångström",
        "65ababa",
        "1969-12-31 23:59",
        false,
    ),
    (
        "Begin the walk",
        "Ada Lovelace",
        "76ababa",
        "1971-01-01 00:00",
        false,
    ),
];

fn oid(first: u8) -> Oid {
    let mut bytes = [0xabu8; 20];
    bytes[0] = first;
    Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
}

fn subject(written: &str) -> String {
    if written == LONG {
        long_subject()
    } else {
        written.to_owned()
    }
}

/// The rows, appended to one history two at a time, as the worker's pages are: the first
/// page brings two authors, later ones bring one new author or none.
fn history() -> History {
    let mut history = History::new();
    for pair in WRITTEN.chunks(2) {
        let mut page = RowsPage::new();
        for &(first, summary, author, author_time, parents) in pair {
            let subject = subject(summary);
            page.push(
                GraphRow::new(oid(first), Lane::new(0), Vec::new()),
                PagedCommit {
                    parents,
                    subject: &subject,
                    author,
                    author_time,
                },
            );
        }
        history.append(page).unwrap_or_else(|full| panic!("{full}"));
    }
    history
}

/// The list as the window draws it: each row a `CommitRow`.
fn app() -> Element {
    let rows = use_consume::<State<History>>();
    HistoryList::new(rows, |render: RowRender| match render.content {
        RowContent::Commit(commit) => CommitRow::new(commit, render.graph, render.lanes).into(),
    })
    .into()
}

/// Every row drawn, top to bottom: its labels' text, left to right, and whether its node's
/// centre is left unpainted.
fn drawn(test: &mut TestingRunner) -> Vec<(Vec<String>, bool)> {
    let mut labels: Vec<(usize, f32, String)> = test.find_many(|node, element| {
        Label::try_downcast(element).map(|label| {
            let area = node.layout().area;
            let row = (area.min_y() / ROW_HEIGHT).floor() as usize;
            (row, area.min_x(), label.text.to_string())
        })
    });
    labels.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));

    let png = test.render();
    let image = Image::from_encoded(png).unwrap_or_else(|| panic!("the frame did not decode"));
    let (width, height) = (WIDTH as i32, HEIGHT as i32);
    let mut surface =
        raster_n32_premul((width, height)).unwrap_or_else(|| panic!("no raster surface"));
    surface.canvas().draw_image(&image, (0, 0), None);
    let info = ImageInfo::new_n32_premul((width, height), None);
    let stride = info.min_row_bytes();
    let mut pixels = vec![0u8; stride * height as usize];
    assert!(surface.read_pixels(&info, &mut pixels, stride, (0, 0)));
    let white_at = |x: f32, y: f32| {
        let at = y.round() as usize * stride + x.round() as usize * 4;
        pixels
            .get(at..at + 3)
            .is_some_and(|rgb| rgb.iter().all(|c| *c == 0xff))
    };

    let rows = labels.iter().map(|(row, _, _)| *row + 1).max().unwrap_or(0);
    (0..rows)
        .map(|row| {
            let texts = labels
                .iter()
                .filter(|(at, _, _)| *at == row)
                .map(|(_, _, text)| text.clone())
                .collect();
            let centre = (
                ROW_PADDING + lane_x(Lane::new(0)),
                row as f32 * ROW_HEIGHT + row_middle(),
            );
            (texts, white_at(centre.0, centre.1))
        })
        .collect()
}

#[test]
fn every_row_draws_what_it_drew_before_rows_were_slimmed() {
    let (mut test, ()) = TestingRunner::new(
        app,
        (WIDTH, HEIGHT).into(),
        |runner| {
            runner.provide_root_context(|| State::create(history()));
        },
        1.,
    );
    test.sync_and_update();

    let drawn = drawn(&mut test);
    let before: Vec<(Vec<String>, bool)> = DRAWN_BEFORE
        .iter()
        .map(|&(subject_text, author, short, date, ring)| {
            (
                vec![
                    subject(subject_text),
                    author.to_owned(),
                    short.to_owned(),
                    date.to_owned(),
                ],
                ring,
            )
        })
        .collect();
    for (index, (now, then)) in drawn.iter().zip(&before).enumerate() {
        assert_eq!(now, then, "row {index} draws something else");
    }
    assert_eq!(drawn.len(), WRITTEN.len(), "a row was not drawn");
    assert_eq!(drawn.len(), before.len());
}
