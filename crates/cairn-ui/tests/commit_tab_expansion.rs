//! Headless tests for a file's diff opened in place in the Commit tab (phase 08, PRD R5.3,
//! criterion C10's expansion; R5.5's one viewport of rows however many files are open).

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{
    ChangeSet, ChangeStatus, ChangedFile, ChangedRange, CommitDetails, Context, DiffContent,
    DiffLine, DisplayOverlay, FileDiff, FileMode, LineSpan, Oid, RenameDetection, RepoPath,
    ShownDiff, Signature, SizeLimit, TextDiff, Timestamp,
};
use cairn_ui::{
    COLLAPSE_ALL_CAPTION, CommitTab, DETAIL_ROW_HEIGHT, EXPAND_ALL_CAPTION, Expansion,
    LOAD_DIFF_CAPTION, Opened, READING_DIFF, TOO_LARGE_TO_DISPLAY, budget_notice,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 720.;

fn oid(n: u64) -> Oid {
    let mut bytes = [0u8; 20];
    bytes[0] = 0xcd;
    bytes[12..20].copy_from_slice(&n.to_be_bytes());
    Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
}

fn path(n: usize) -> String {
    format!("src/file-{n:06}.rs")
}

fn file(n: usize) -> ChangedFile {
    let path = RepoPath::from(path(n).as_str());
    ChangedFile {
        status: ChangeStatus::Modified,
        old_path: path.clone(),
        new_path: path,
        old_mode: Some(FileMode::Regular),
        new_mode: Some(FileMode::Regular),
        old_id: Some(oid(1)),
        new_id: Some(oid(2)),
    }
}

fn change_set(files: usize) -> ChangeSet {
    let signature = Signature {
        name: "Ada".to_owned(),
        email: "ada@example.com".to_owned(),
        time: Timestamp::new(1_700_000_000, 0),
    };
    ChangeSet {
        files: (0..files).map(file).collect(),
        details: Some(CommitDetails {
            id: oid(77),
            parents: vec![oid(10)],
            author: signature.clone(),
            committer: signature,
            message: "Change many files\n\nWith a body.\n".to_owned(),
        }),
        renames: RenameDetection::default(),
    }
}

/// File `n`'s diff, drawn whole: `lines` lines, `n:{line}`, every fiftieth from the third
/// changed.
fn long_diff(n: usize, lines: u32) -> ShownDiff {
    let old: Vec<DiffLine> = (0..lines)
        .map(|line| DiffLine::terminated(format!("{n}:{line}")))
        .collect();
    let new: Vec<DiffLine> = (0..lines)
        .map(|line| {
            DiffLine::terminated(if line % 50 == 2 {
                format!("{n}:{line} changed")
            } else {
                format!("{n}:{line}")
            })
        })
        .collect();
    let changes = (0..lines.div_ceil(50))
        .filter(|k| k * 50 + 2 < lines)
        .map(|k| ChangedRange::new(LineSpan::at(k * 50 + 2, 1), LineSpan::at(k * 50 + 2, 1)))
        .collect();
    ShownDiff::new(
        FileDiff {
            file: file(n),
            content: DiffContent::Text {
                text: TextDiff::new(old, new, changes),
                overlay: DisplayOverlay::none(),
            },
        },
        Context::EntireFile,
    )
}

#[derive(Clone)]
struct Fixture {
    changes: State<ChangeSet>,
    expansion: State<Expansion>,
}

#[derive(Default)]
struct Reported {
    pressed: Vec<usize>,
    expand_all: Vec<bool>,
    loads: Vec<usize>,
}

type Reports = Rc<RefCell<Reported>>;

fn launch(changes: ChangeSet, side_by_side: bool) -> (TestingRunner, Fixture, Reports) {
    let reports = Reports::default();
    let app = {
        let reports = reports.clone();
        move || {
            let fixture = use_consume::<Fixture>();
            let (pressed, expand_all, loads) = (reports.clone(), reports.clone(), reports.clone());
            rect()
                .expanded()
                .child(
                    CommitTab::new(fixture.changes)
                        .expansion(fixture.expansion)
                        .side_by_side(side_by_side)
                        .on_file_pressed(move |index: usize| {
                            pressed.borrow_mut().pressed.push(index);
                        })
                        .on_expand_all(move |all: bool| {
                            expand_all.borrow_mut().expand_all.push(all)
                        })
                        .on_load(move |index: usize| loads.borrow_mut().loads.push(index)),
                )
                .into_element()
        }
    };
    let (mut test, fixture) = TestingRunner::new(
        app,
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(|| Fixture {
                changes: State::create(changes),
                expansion: State::create(Expansion::default()),
            })
        },
        1.,
    );
    test.sync_and_update();
    test.sync_and_update();
    (test, fixture, reports)
}

fn open(test: &mut TestingRunner, fixture: &Fixture, files: Vec<(usize, Opened)>) {
    let mut expansion = fixture.expansion;
    test.run_in(|| {
        expansion.write().set(files);
    });
    test.sync_and_update();
    test.sync_and_update();
}

/// Every piece of text built, as `(text, top, visible)`.
fn built(test: &TestingRunner) -> Vec<(String, f32, bool)> {
    test.find_many(|node, element| {
        let area = node.layout().area;
        if let Some(label) = Label::try_downcast(element) {
            return Some((label.text.to_string(), area.min_y(), node.is_visible()));
        }
        Paragraph::try_downcast(element).map(|paragraph| {
            let text: String = paragraph
                .spans
                .iter()
                .map(|span| span.text.as_ref())
                .collect();
            (text, area.min_y(), node.is_visible())
        })
    })
}

/// How many rows of the list were built: the distinct row slots any text was drawn in.
fn rows_built(test: &TestingRunner) -> usize {
    let mut slots: Vec<i64> = built(test)
        .iter()
        .map(|(_, top, _)| (top / DETAIL_ROW_HEIGHT).floor() as i64)
        .collect();
    slots.sort_unstable();
    slots.dedup();
    slots.len()
}

fn top_of(test: &TestingRunner, text: &str) -> Option<f32> {
    built(test)
        .into_iter()
        .find(|(drawn, _, visible)| *visible && drawn == text)
        .map(|(_, top, _)| top)
}

fn press(test: &mut TestingRunner, text: &str) {
    let centre = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("nothing reads {text:?}"));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.sync_and_update();
}

/// C10, R5.3 (Fork, Finding 4): a file starts collapsed, a pressed file is reported for its
/// caller to open, and once open its diff is drawn under its own row and above the next
/// file's — rows of the same list, pushing the files below it down. Caught by: a press that
/// reports another file, the diff drawn anywhere but under its row, or the files after it
/// left where they were (drawn over it).
#[test]
fn a_pressed_file_is_reported_and_its_diff_opens_under_its_row() {
    let (mut test, fixture, reports) = launch(change_set(8), false);
    assert!(
        top_of(&test, "2:0").is_none(),
        "a file's diff was drawn before it was opened"
    );
    let next_before = top_of(&test, &path(3)).unwrap_or_else(|| panic!("no file 3"));
    press(&mut test, &path(2));
    assert_eq!(reports.borrow().pressed, [2]);

    open(
        &mut test,
        &fixture,
        vec![(2, Opened::Shown(Box::new(long_diff(2, 6))))],
    );
    let own = top_of(&test, &path(2)).unwrap_or_else(|| panic!("no file 2"));
    let first = top_of(&test, "2:0")
        .unwrap_or_else(|| panic!("file 2 diff is not drawn: {:?}", built(&test)));
    let last = top_of(&test, "2:5").unwrap_or_else(|| panic!("file 2's last line is not drawn"));
    let next = top_of(&test, &path(3)).unwrap_or_else(|| panic!("no file 3"));
    assert!(
        own < first && first < last && last < next,
        "file 2 at {own}, its diff from {first} to {last}, file 3 at {next}"
    );
    // A header row, five lines and the changed one's two: eight rows between file 2 and 3.
    assert_eq!(next - next_before, 8. * DETAIL_ROW_HEIGHT);
}

/// R5.3's states of an opened file, each a row of the list: being read, failed — that file's
/// alone — and a notice standing in place of rows, whose Load Diff reports which file it is
/// under. Caught by: a failure drawn under another file, a notice missing from the list, or
/// Load Diff reporting nothing or another file.
#[test]
fn an_opened_file_says_it_is_being_read_or_failed_or_why_it_has_no_rows() {
    let (mut test, fixture, reports) = launch(change_set(6), false);
    let too_large = ShownDiff::new(
        FileDiff {
            file: file(4),
            content: DiffContent::TooLarge {
                crossed: SizeLimit::Bytes {
                    limit: 1_048_576,
                    measured: 2_000_000,
                },
                loadable: true,
            },
        },
        Context::lines(3),
    );
    open(
        &mut test,
        &fixture,
        vec![
            (0, Opened::Reading),
            (2, Opened::Failed("git could not read it".to_owned())),
            (4, Opened::Shown(Box::new(too_large))),
        ],
    );
    let at = |text: &str| top_of(&test, text).unwrap_or_else(|| panic!("no {text:?}"));
    assert!(at(&path(0)) < at(READING_DIFF) && at(READING_DIFF) < at(&path(1)));
    let failed = at("git could not read it");
    assert!(at(&path(2)) < failed && failed < at(&path(3)));
    let notice = at(TOO_LARGE_TO_DISPLAY);
    assert!(at(&path(4)) < notice && notice < at(&path(5)));

    press(&mut test, LOAD_DIFF_CAPTION);
    assert_eq!(reports.borrow().loads, [4]);
}

/// C10, R5.3: Expand All above the files reports itself; while any file is open it is
/// Collapse All; and when Expand All stopped at its budget, the tab says how many files it
/// left collapsed — the files not open, counted. Caught by: the button never turning, the
/// notice shown when Expand All did not stop at the budget, or counting the open files.
#[test]
fn expand_all_turns_into_collapse_all_and_says_what_its_budget_left_collapsed() {
    let (mut test, fixture, reports) = launch(change_set(10), false);
    press(&mut test, EXPAND_ALL_CAPTION);
    assert_eq!(reports.borrow().expand_all, [true]);
    let notice = budget_notice(7);
    assert!(top_of(&test, &notice).is_none());

    open(
        &mut test,
        &fixture,
        (0..3)
            .map(|n| (n, Opened::Shown(Box::new(long_diff(n, 4)))))
            .collect(),
    );
    assert!(
        top_of(&test, &notice).is_none(),
        "said before Expand All stopped"
    );
    let mut expansion = fixture.expansion;
    test.run_in(|| expansion.write().set_stopped_at_budget(true));
    test.sync_and_update();
    assert!(
        top_of(&test, &notice).is_some(),
        "the budget's notice is not drawn: {:?}",
        built(&test)
    );
    assert_eq!(
        notice,
        "Expand All stopped at its line budget: 7 files left collapsed."
    );
    press(&mut test, COLLAPSE_ALL_CAPTION);
    assert_eq!(reports.borrow().expand_all, [true, false]);
}

/// R5.5 with files open, the twin of `only_a_viewport_of_rows_is_built_however_long_the_history`
/// (QA brief: expansion makes this the longest list in the application): 55,184 files with
/// three of them open, each 10,000 lines drawn whole, build one viewport of rows at the top,
/// deep inside the middle file's diff and at the very end — the same number at each, unified
/// and side by side — and at the end the last file's row sits at the bottom of the view, so
/// the list is exactly as long as its header, its files and every opened file's rows. Caught
/// by: rows built per opened line, a length that forgets the opened rows (the last file cut
/// off) or counts them twice (empty space after it), or a deep row placed in the wrong file
/// or a row off its place, against row counts worked out by hand.
#[test]
fn only_a_viewport_of_rows_is_built_however_many_files_are_open() {
    const FILES: usize = 55_184;
    const LINES: u32 = 10_000;
    let viewport_rows = (HEIGHT / DETAIL_ROW_HEIGHT).ceil() as usize;
    let opened = [10usize, 27_592, 55_000];
    let mut built_counts = Vec::new();
    for side_by_side in [false, true] {
        let (mut test, fixture, _) = launch(change_set(FILES), side_by_side);
        open(
            &mut test,
            &fixture,
            opened
                .iter()
                .map(|n| (*n, Opened::Shown(Box::new(long_diff(*n, LINES)))))
                .collect(),
        );
        let within = |test: &TestingRunner, place: &str| {
            let count = rows_built(test);
            assert!(
                count >= viewport_rows - 1 && count <= viewport_rows + 2,
                "{count} rows were built for a {viewport_rows}-row viewport {place} \
                 (side by side: {side_by_side})"
            );
            count
        };
        built_counts.push(within(&test, "at the top"));

        // Deep inside the middle file's diff: past the header, the files before it and the
        // rows the file opened above it adds, its own row, then its rows: a hunk header, and
        // each line — a changed one twice unified, once side by side.
        let first_top = top_of(&test, &path(0)).unwrap_or_else(|| panic!("no first file"));
        let header = (first_top / DETAIL_ROW_HEIGHT).floor();
        // Where a file's label sits inside its row, centred.
        let inset = first_top - header * DETAIL_ROW_HEIGHT;
        // Counted by hand, not by the function under test (phase 08 QA's U5: a row too many
        // per opened file, there or in the table, passed when this was read from it): one hunk
        // header and 10,000 lines, and unified the 200 changed lines a second row each.
        let opened_rows = if side_by_side { 10_001 } else { 10_201 };
        assert_eq!(long_diff(10, LINES).rows(side_by_side), opened_rows);
        let opened_above = opened_rows as f64;
        let target_line = 6_000u32;
        let changed_before = (0..target_line).filter(|line| line % 50 == 2).count() as f64;
        assert_eq!(changed_before, 120.);
        let under = 1. + f64::from(target_line) + if side_by_side { 0. } else { changed_before };
        let row = f64::from(header) + 27_592. + opened_above + 1. + under;
        test.scroll((100., 100.), (0., -(row * f64::from(DETAIL_ROW_HEIGHT))));
        built_counts.push(within(&test, "deep in the middle file"));
        let line = format!("27592:{target_line}");
        // Placed exactly: scrolled `row` rows down, the line is the row at the top of the view.
        let slot = built(&test)
            .into_iter()
            .find(|(text, _, visible)| *visible && *text == line)
            .map(|(_, top, _)| (top / DETAIL_ROW_HEIGHT).floor())
            .unwrap_or_else(|| {
                panic!(
                    "scrolling to line {target_line} of file 27,592 did not draw it (side by \
                     side: {side_by_side}): {:?}",
                    built(&test)
                )
            });
        assert_eq!(
            slot, 0.,
            "line {target_line} of file 27,592 is {slot} rows from where its place puts it (side \
             by side: {side_by_side})"
        );

        test.scroll((100., 100.), (0., -1e9));
        built_counts.push(within(&test, "at the end"));
        let last = path(FILES - 1);
        let bottom = built(&test)
            .into_iter()
            .find(|(text, _, visible)| *visible && *text == last)
            .map(|(_, top, _)| top - inset + DETAIL_ROW_HEIGHT)
            .unwrap_or_else(|| panic!("the last file is not at the end"));
        assert!(
            (bottom - HEIGHT).abs() < 1.,
            "the last file's row ends at {bottom}, not at the bottom of the {HEIGHT} px view: \
             the list is not exactly as long as its rows (side by side: {side_by_side})"
        );
    }
    // A row partly scrolled off the top is counted with the row above it: two either way.
    let fewest = built_counts.iter().min().copied().unwrap_or(0);
    let most = built_counts.iter().max().copied().unwrap_or(0);
    assert!(
        most - fewest <= 2,
        "the rows built grow with where the list is scrolled: {built_counts:?}"
    );
}
