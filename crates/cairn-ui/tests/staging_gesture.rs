//! Headless tests for the diff's staging gesture (staging-and-commit R9, criterion C19): a
//! hovered chunk's actions, a drag-selection across rows the virtual list unmounts, a
//! side-by-side selection kept to its column, no gesture where none is handed, and one
//! viewport of rows built with the gesture drawn.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{
    ChangeStatus, ChangedFile, ChangedRange, Context, DiffContent, DiffLine, DisplayOverlay,
    FileDiff, LineNumber, LineSpan, RepoPath, Selection, ShownDiff, TextDiff,
};
use cairn_ui::{
    DIFF_ROW_HEIGHT, DISCARD_CHUNK_CAPTION, DiffView, Expansion, Gesture, GestureAct, GestureSide,
    GestureVerb, LineDrag, Opened, STAGE_CHUNK_CAPTION, StackedDiff, UNSTAGE_CHUNK_CAPTION,
    lines_caption,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 400.;
const ROW: f64 = DIFF_ROW_HEIGHT as f64;

/// `lines` lines, `line {n}`, with line `first` and every `every`-th after it replaced by
/// `LINE {n}`.
fn edited(lines: u32, every: u32, first: u32, context: Context) -> ShownDiff {
    let is_edited = |n: u32| n >= first && (n - first).is_multiple_of(every);
    let old: Vec<DiffLine> = (0..lines)
        .map(|n| DiffLine::terminated(format!("line {n}")))
        .collect();
    let new: Vec<DiffLine> = (0..lines)
        .map(|n| {
            DiffLine::terminated(if is_edited(n) {
                format!("LINE {n}")
            } else {
                format!("line {n}")
            })
        })
        .collect();
    let changes = (first..lines)
        .step_by(every as usize)
        .map(|at| ChangedRange::new(LineSpan::at(at, 1), LineSpan::at(at, 1)))
        .collect();
    ShownDiff::new(
        FileDiff {
            file: ChangedFile {
                status: ChangeStatus::Modified,
                old_path: RepoPath::from("src/lib.rs"),
                new_path: RepoPath::from("src/lib.rs"),
                old_mode: None,
                new_mode: None,
                old_id: None,
                new_id: None,
            },
            content: DiffContent::Text {
                text: TextDiff::new(old, new, changes),
                overlay: DisplayOverlay::none(),
            },
        },
        context,
    )
}

#[derive(Clone)]
struct Fixture {
    shown: State<ShownDiff>,
    lines: State<LineDrag>,
    drawn: State<u64>,
    acts: Rc<RefCell<Vec<GestureAct>>>,
    side: Option<GestureSide>,
    side_by_side: bool,
}

struct Launched {
    test: TestingRunner,
    fixture: Fixture,
}

fn launch(shown: ShownDiff, side: Option<GestureSide>, side_by_side: bool) -> Launched {
    let acts = Rc::new(RefCell::new(Vec::new()));
    let kept: Rc<RefCell<Option<Fixture>>> = Rc::new(RefCell::new(None));
    let keeping = kept.clone();
    let recording = acts.clone();
    let (mut test, _) = TestingRunner::new(
        move || {
            let fixture = use_consume::<Fixture>();
            let scroll = use_scroll_controller(ScrollConfig::default);
            let acts = fixture.acts.clone();
            let gesture = fixture.side.map(|side| {
                Gesture::new(
                    side,
                    *fixture.drawn.read(),
                    fixture.lines,
                    move |act: GestureAct| acts.borrow_mut().push(act),
                )
            });
            rect()
                .expanded()
                .child(
                    DiffView::new(fixture.shown, scroll)
                        .side_by_side(fixture.side_by_side)
                        .gesture(gesture),
                )
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || {
                let fixture = Fixture {
                    shown: State::create(shown),
                    lines: State::create(LineDrag::default()),
                    drawn: State::create(1),
                    acts: recording,
                    side,
                    side_by_side,
                };
                *keeping.borrow_mut() = Some(fixture.clone());
                fixture
            })
        },
        1.,
    );
    test.sync_and_update();
    let fixture = kept
        .borrow()
        .clone()
        .unwrap_or_else(|| panic!("the fixture was not provided"));
    Launched { test, fixture }
}

/// Every label's text and its centre.
fn labels(test: &TestingRunner) -> Vec<(String, f32, f32)> {
    test.find_many(|node, element| {
        Label::try_downcast(element).map(|label| {
            let area = node.layout().area;
            (label.text.to_string(), area.center().x, area.center().y)
        })
    })
}

fn captions(test: &TestingRunner) -> Vec<String> {
    labels(test).into_iter().map(|(text, ..)| text).collect()
}

fn press_caption(test: &mut TestingRunner, caption: &str) {
    let (_, x, y) = labels(test)
        .into_iter()
        .find(|(text, ..)| text == caption)
        .unwrap_or_else(|| panic!("no {caption} drawn: {:?}", captions(test)));
    test.click_cursor((f64::from(x), f64::from(y)));
    test.sync_and_update();
}

/// Rows built: each row draws one paragraph.
fn built_rows(test: &TestingRunner) -> usize {
    test.find_many(|_, element| Paragraph::try_downcast(element).map(|_| ()))
        .len()
}

/// The centre of row `row` of the view, scrolled to the top.
fn row_y(row: usize) -> f64 {
    (row as f64 + 0.5) * ROW
}

fn hover(test: &mut TestingRunner, x: f64, y: f64) {
    test.move_cursor((x, y));
    test.sync_and_update();
}

fn numbers(selection: &Selection) -> (Vec<u32>, Vec<u32>) {
    (
        selection.removed().map(LineNumber::index).collect(),
        selection.added().map(LineNumber::index).collect(),
    )
}

/// R9.1, the QA brief's first risk: hovering a chunk drawn at context ten — one outline over
/// three of git's changes — floats Stage and Discard Changes…, and Stage takes every changed
/// line the outline holds and none outside it. Over the staged diff the one action is
/// Unstage. Caught by: a chunk's action mapped to the change under the pointer alone, or to
/// the whole file.
#[test]
fn a_hovered_chunks_actions_take_exactly_its_changes() {
    // Edits every eight lines from line 4: at context ten the first hunks merge.
    let shown = edited(200, 8, 4, Context::lines(3));
    let mut wide = launch(
        edited(60, 8, 4, Context::lines(10)),
        Some(GestureSide::Unstaged),
        false,
    );
    assert!(
        captions(&wide.test)
            .iter()
            .all(|c| c != STAGE_CHUNK_CAPTION)
    );
    hover(&mut wide.test, 300., row_y(3));
    let drawn = captions(&wide.test);
    assert!(drawn.iter().any(|c| c == STAGE_CHUNK_CAPTION), "{drawn:?}");
    assert!(
        drawn.iter().any(|c| c == DISCARD_CHUNK_CAPTION),
        "{drawn:?}"
    );
    press_caption(&mut wide.test, STAGE_CHUNK_CAPTION);
    let acts = wide.fixture.acts.borrow().clone();
    assert_eq!(acts.len(), 1, "{acts:?}");
    assert_eq!(acts[0].verb, GestureVerb::Stage);
    assert_eq!(acts[0].file, 0);
    // Sixty lines edited every eight from line 4: 4, 12, ..., 52 — one hunk at context ten.
    let every: Vec<u32> = (0..7).map(|k| k * 8 + 4).collect();
    assert_eq!(numbers(&acts[0].selection), (every.clone(), every));

    let mut staged = launch(shown, Some(GestureSide::Staged), false);
    hover(&mut staged.test, 300., row_y(2));
    let drawn = captions(&staged.test);
    assert!(
        drawn.iter().any(|c| c == UNSTAGE_CHUNK_CAPTION),
        "{drawn:?}"
    );
    assert!(
        !drawn
            .iter()
            .any(|c| c == DISCARD_CHUNK_CAPTION || c == STAGE_CHUNK_CAPTION),
        "the staged diff offered a discard or a stage: {drawn:?}"
    );
    press_caption(&mut staged.test, UNSTAGE_CHUNK_CAPTION);
    let acts = staged.fixture.acts.borrow().clone();
    assert_eq!(acts[0].verb, GestureVerb::Unstage);
    // At context three the first chunk holds the edit at line 4 alone.
    assert_eq!(numbers(&acts[0].selection), (vec![4], vec![4]));
}

/// R9.5: a diff view handed no gesture — the Commit and Changes tabs' — draws no action
/// however it is hovered or dragged. Caught by: a gesture drawn by every diff view.
#[test]
fn a_view_handed_no_gesture_draws_no_action() {
    let mut plain = launch(edited(60, 8, 4, Context::lines(3)), None, false);
    hover(&mut plain.test, 300., row_y(2));
    plain.test.press_cursor((300., row_y(1)));
    plain.test.move_cursor((300., row_y(6)));
    plain.test.release_cursor((300., row_y(6)));
    plain.test.sync_and_update();
    hover(&mut plain.test, 300., row_y(6));
    let drawn = captions(&plain.test);
    for caption in [
        STAGE_CHUNK_CAPTION,
        DISCARD_CHUNK_CAPTION,
        UNSTAGE_CHUNK_CAPTION,
    ] {
        assert!(!drawn.iter().any(|c| c == caption), "{drawn:?}");
    }
    assert!(!drawn.iter().any(|c| c.contains(" Line")), "{drawn:?}");
}

/// R9.2: a drag across lines narrows the actions to the changed lines it covers — counted in
/// the caption — and a press without a drag clears the selection. Caught by: the actions still
/// taking the chunk, or a click selecting a line.
#[test]
fn a_drag_narrows_the_actions_to_its_lines() {
    let mut view = launch(
        edited(60, 4, 2, Context::lines(3)),
        Some(GestureSide::Unstaged),
        false,
    );
    // Rows: 0 the header, 1-2 context, 3 removed line 2, 4 added line 2, 5-7 context,
    // 8 removed line 6, 9 added line 6 — one hunk at context three.
    view.test.press_cursor((300., row_y(4)));
    view.test.move_cursor((300., row_y(6)));
    view.test.move_cursor((300., row_y(8)));
    view.test.release_cursor((300., row_y(8)));
    view.test.sync_and_update();
    let caption = lines_caption(GestureVerb::Stage, 2);
    assert!(
        captions(&view.test).contains(&caption),
        "{:?}",
        captions(&view.test)
    );
    let selected = view
        .fixture
        .lines
        .peek()
        .selected(1)
        .map(|(file, s)| (file, numbers(s)));
    assert_eq!(selected, Some((0, (vec![6], vec![2]))));
    press_caption(&mut view.test, &caption);
    let acts = view.fixture.acts.borrow().clone();
    assert_eq!(numbers(&acts[0].selection), (vec![6], vec![2]));
    assert_eq!(
        view.fixture.lines.peek().selected(1),
        None,
        "the selection outlived its act"
    );

    // A press without a drag selects nothing, and lets a selection go.
    view.test.press_cursor((300., row_y(3)));
    view.test.move_cursor((300., row_y(4)));
    view.test.release_cursor((300., row_y(4)));
    view.test.sync_and_update();
    assert!(view.fixture.lines.peek().selected(1).is_some());
    view.test.click_cursor((300., row_y(9)));
    view.test.sync_and_update();
    assert_eq!(view.fixture.lines.peek().selected(1), None);
}

/// The QA brief's second risk: a drag from row 10 to row 5,000 of a 10,000-line diff, the list
/// scrolled under the held pointer so every row between is unmounted on the way, selects every
/// changed line between its ends — counted from the diff, not from the rows built — and the
/// list builds one viewport of rows with the gesture drawn, at the top and deep (C19's
/// viewport twin, `the_gesture_builds_one_viewport_over_a_10000_line_diff`'s companion). Caught
/// by: a drag tracked by a row (it ends as its row unmounts), or a selection read from the
/// rows built.
#[test]
fn a_drag_across_unmounted_rows_selects_every_line_between_its_ends() {
    let shown = edited(10_000, 50, 25, Context::EntireFile);
    let layout = shown.layout().expect("text");
    let mut view = launch(shown.clone(), Some(GestureSide::Unstaged), false);
    let viewport_rows = (HEIGHT / DIFF_ROW_HEIGHT).ceil() as usize;
    view.test.press_cursor((300., row_y(10)));
    view.test.move_cursor((300., row_y(14)));
    view.test.sync_and_update();
    // Scroll the list under the held pointer until row 5,000 is under it.
    let target = 5_000usize;
    let scrolled = (target - 14) as f64 * ROW;
    view.test.scroll((300., row_y(14)), (0., -scrolled));
    view.test.sync_and_update();
    view.test.move_cursor((301., row_y(14)));
    view.test.sync_and_update();
    let built = built_rows(&view.test);
    assert!(
        built <= viewport_rows + 2,
        "{built} rows built mid-drag for a {viewport_rows}-row viewport"
    );
    view.test.release_cursor((301., row_y(14)));
    view.test.sync_and_update();

    let (text, _) = shown.text().expect("text");
    let expected = layout.selection_in(text, 10..target + 1).expect("exact");
    assert!(
        expected.len() > 150,
        "the span holds {} lines",
        expected.len()
    );
    let selected = view
        .fixture
        .lines
        .peek()
        .selected(1)
        .map(|(_, selection)| selection.clone());
    assert_eq!(selected.as_ref().map(Selection::len), Some(expected.len()));
    assert_eq!(selected, Some(expected));
}

/// R9.3: side by side, a drag begun in the old column selects removed lines alone, and one in
/// the new column added lines alone, whatever column the pointer crosses into. Caught by: a
/// side-by-side drag taking both halves of a replaced row.
#[test]
fn side_by_side_a_drag_keeps_to_its_column() {
    for (x, removed) in [(WIDTH as f64 * 0.25, true), (WIDTH as f64 * 0.75, false)] {
        let mut view = launch(
            edited(60, 4, 2, Context::lines(3)),
            Some(GestureSide::Unstaged),
            true,
        );
        // Side by side: 0 the header, 1-2 context, 3 line 2 replaced, 4-6 context, 7 line 6.
        view.test.press_cursor((x, row_y(3)));
        view.test.move_cursor((WIDTH as f64 - x, row_y(5)));
        view.test.move_cursor((WIDTH as f64 - x, row_y(7)));
        view.test.release_cursor((WIDTH as f64 - x, row_y(7)));
        view.test.sync_and_update();
        let selected = view
            .fixture
            .lines
            .peek()
            .selected(1)
            .map(|(_, s)| numbers(s));
        let expected = if removed {
            (vec![2, 6], vec![])
        } else {
            (vec![], vec![2, 6])
        };
        assert_eq!(selected, Some(expected), "begun at x {x}");
    }
}

/// The QA brief's third risk, and Fork's Tracker #480: after Stage, with the pointer still,
/// the actions are gone while the old rows are drawn; once the diff is drawn again they follow
/// whatever is now under the pointer. A selection made under one answer is nothing under the
/// next. Caught by: actions left on rows already acted on, or hidden for good.
#[test]
fn after_an_action_the_actions_follow_the_new_rows_or_none() {
    let mut view = launch(
        edited(60, 8, 4, Context::lines(3)),
        Some(GestureSide::Unstaged),
        false,
    );
    hover(&mut view.test, 300., row_y(2));
    press_caption(&mut view.test, STAGE_CHUNK_CAPTION);
    assert!(
        !captions(&view.test)
            .iter()
            .any(|c| c == STAGE_CHUNK_CAPTION),
        "the actions stayed over the chunk just staged"
    );
    // The diff drawn again: the first edit is gone, so the chunk under the pointer is the
    // next one's.
    let mut shown = view.fixture.shown;
    shown.set(edited(60, 8, 12, Context::lines(3)));
    let mut drawn = view.fixture.drawn;
    drawn.set(2);
    view.test.sync_and_update();
    hover(&mut view.test, 300., row_y(2));
    assert!(
        captions(&view.test)
            .iter()
            .any(|c| c == STAGE_CHUNK_CAPTION)
    );
    press_caption(&mut view.test, STAGE_CHUNK_CAPTION);
    let acts = view.fixture.acts.borrow().clone();
    assert_eq!(numbers(&acts[1].selection), (vec![12], vec![12]));

    // A selection under answer 2 is nothing once answer 3 is drawn.
    view.test.press_cursor((300., row_y(3)));
    view.test.move_cursor((300., row_y(4)));
    view.test.release_cursor((300., row_y(4)));
    view.test.sync_and_update();
    assert!(view.fixture.lines.peek().selected(2).is_some());
    drawn.set(3);
    view.test.sync_and_update();
    assert!(view.fixture.lines.peek().selected(3).is_none());
}

/// C19's viewport twin: a 10,000-line diff with the gesture drawn — hovered, a selection made
/// — builds one viewport of rows at the top, deep and at the end, and the layer over it draws
/// at most three actions. Caught by: a gesture that builds a row per line, or a layer whose
/// elements grow with the selection or the diff.
#[test]
fn the_gesture_builds_one_viewport_over_a_10000_line_diff() {
    let viewport_rows = (HEIGHT / DIFF_ROW_HEIGHT).ceil() as usize;
    let shown = edited(10_000, 50, 25, Context::EntireFile);
    let rows = shown.rows(false);
    let mut view = launch(shown, Some(GestureSide::Unstaged), false);
    let within = |test: &TestingRunner, place: &str| {
        let built = built_rows(test);
        assert!(
            built <= viewport_rows + 2 + cairn_ui::with_end_room(1, DIFF_ROW_HEIGHT),
            "{built} rows built for a {viewport_rows}-row viewport {place}"
        );
        let actions = captions(test)
            .into_iter()
            .filter(|c| c.starts_with("Stage") || c.starts_with("Discard"))
            .count();
        assert!(actions <= 2, "{actions} actions drawn {place}");
    };
    hover(&mut view.test, 300., row_y(5));
    within(&view.test, "hovered at the top");
    view.test.press_cursor((300., row_y(2)));
    view.test.move_cursor((300., row_y(20)));
    view.test.release_cursor((300., row_y(20)));
    view.test.sync_and_update();
    within(&view.test, "with a selection at the top");
    view.test
        .scroll((300., 100.), (0., -(rows as f64 / 2.) * ROW));
    view.test.sync_and_update();
    hover(&mut view.test, 300., 120.);
    within(&view.test, "deep");
    view.test
        .scroll((300., 100.), (0., -(rows as f64) * ROW * 2.));
    view.test.sync_and_update();
    hover(&mut view.test, 300., 120.);
    within(&view.test, "at the end");
}

#[derive(Clone)]
struct StackedFixture {
    files: State<Expansion>,
    paths: State<Vec<RepoPath>>,
    lines: State<LineDrag>,
    acts: Rc<RefCell<Vec<GestureAct>>>,
}

/// Several files' diffs drawn together, each answered.
fn launch_stacked(diffs: Vec<ShownDiff>) -> (TestingRunner, StackedFixture) {
    let acts = Rc::new(RefCell::new(Vec::new()));
    let kept: Rc<RefCell<Option<StackedFixture>>> = Rc::new(RefCell::new(None));
    let keeping = kept.clone();
    let (mut test, _) = TestingRunner::new(
        move || {
            let fixture = use_consume::<StackedFixture>();
            let scroll = use_scroll_controller(ScrollConfig::default);
            let acts = fixture.acts.clone();
            rect()
                .expanded()
                .child(
                    StackedDiff::new(fixture.files, fixture.paths, scroll).gesture(Some(
                        Gesture::new(
                            GestureSide::Unstaged,
                            1,
                            fixture.lines,
                            move |act: GestureAct| acts.borrow_mut().push(act),
                        ),
                    )),
                )
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || {
                let paths: Vec<RepoPath> = (0..diffs.len())
                    .map(|n| RepoPath::from(format!("file{n}.rs").as_str()))
                    .collect();
                let mut files = Expansion::new();
                files.set(
                    diffs
                        .into_iter()
                        .enumerate()
                        .map(|(n, shown)| (n, Opened::Shown(Box::new(shown)))),
                );
                let fixture = StackedFixture {
                    files: State::create(files),
                    paths: State::create(paths),
                    lines: State::create(LineDrag::default()),
                    acts,
                };
                *keeping.borrow_mut() = Some(fixture.clone());
                fixture
            })
        },
        1.,
    );
    test.sync_and_update();
    let fixture = kept
        .borrow()
        .clone()
        .unwrap_or_else(|| panic!("the fixture was not provided"));
    (test, fixture)
}

/// R8.1 (the user's decision) with R9: files drawn together are each under their own row, and a
/// hovered chunk of the second file stages its lines named as that file; a drag begun in the
/// first file stays inside it, however far into the second it is carried. Caught by: a chunk
/// read from the wrong file's rows, or a drag running across files.
#[test]
fn files_drawn_together_each_take_their_own_gesture() {
    // Two files of twenty lines, edits at lines 4 and 12, at context three: each file is its
    // own row, then a header and rows 1-3 of context before the first edit.
    let (mut test, fixture) = launch_stacked(vec![
        edited(20, 8, 4, Context::lines(3)),
        edited(20, 8, 4, Context::lines(3)),
    ]);
    let drawn = captions(&test);
    assert!(drawn.iter().any(|c| c == "file0.rs"), "{drawn:?}");
    assert!(drawn.iter().any(|c| c == "file1.rs"), "{drawn:?}");
    let first_rows = fixture
        .files
        .peek()
        .get(0)
        .map_or(0, |opened| match opened {
            Opened::Shown(shown) => shown.rows(false),
            Opened::Reading | Opened::Failed(_) => 0,
        });
    // The second file's own row is after the first file's row and rows.
    let second_file_row = 1 + first_rows;
    hover(&mut test, 300., row_y(second_file_row + 2));
    press_caption(&mut test, STAGE_CHUNK_CAPTION);
    let acts = fixture.acts.borrow().clone();
    assert_eq!(acts.len(), 1, "{acts:?}");
    assert_eq!(acts[0].file, 1, "the act named another file");
    assert_eq!(numbers(&acts[0].selection), (vec![4], vec![4]));

    // A drag from the first file's first edit carried into the second file: the first file's
    // rows alone, to its last.
    test.press_cursor((300., row_y(1 + 4)));
    test.move_cursor((300., row_y(1 + 8)));
    test.move_cursor((300., row_y(second_file_row + 2)));
    test.release_cursor((300., row_y(second_file_row + 2)));
    test.sync_and_update();
    let selected = fixture
        .lines
        .peek()
        .selected(1)
        .map(|(file, selection)| (file, numbers(selection)));
    assert_eq!(selected, Some((0, (vec![4, 12], vec![4, 12]))));
}

/// C19's viewport twin for files drawn together: fifty files of 10,000 lines each, drawn
/// whole, with the gesture over them, build one viewport of rows at the top, deep and at the
/// end. Caught by: a list of files that builds every file's rows.
#[test]
fn files_drawn_together_build_one_viewport() {
    let viewport_rows = (HEIGHT / DIFF_ROW_HEIGHT).ceil() as usize;
    let diffs: Vec<ShownDiff> = (0..50)
        .map(|_| edited(10_000, 50, 25, Context::EntireFile))
        .collect();
    let per_file = diffs[0].rows(false);
    let (mut test, _fixture) = launch_stacked(diffs);
    let total = 50 * (1 + per_file);
    for (place, offset) in [
        ("at the top", 0.0),
        ("deep", -(total as f64 / 2.) * ROW),
        ("at the end", -(total as f64) * ROW * 2.),
    ] {
        test.scroll((300., 100.), (0., offset));
        test.sync_and_update();
        hover(&mut test, 300., 150.);
        let built = built_rows(&test);
        assert!(
            built <= viewport_rows + 2 + cairn_ui::with_end_room(1, DIFF_ROW_HEIGHT),
            "{built} rows built for a {viewport_rows}-row viewport {place}"
        );
    }
}
