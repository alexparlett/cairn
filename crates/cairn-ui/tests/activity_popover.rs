//! Headless tests for the activity popover (staging-and-commit R12.1, C29): its operations and
//! the selected one's lines are each drawn through a virtualizing view of fixed rows, so a
//! session of many operations and a `git` that said a hundred thousand lines build one viewport
//! of rows each; each row is Fork's — its name, its result line, its time, ⚠ or the cancel's
//! shape, no status word — and the right pane its word, time and duration, its one sentence and
//! the prompt it confirmed whole; a press on an operation, its cancel and the way back each
//! report, and no entry offers `Remove index.lock…` (phase 19: the leftover lock is the
//! repository's banner); and it hangs from the status box inside the window (the user's decision
//! A, 2026-10-09).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use cairn_model::Oid;
use cairn_ui::{
    ACTIVITY_TITLE, ActivityEntry, ActivityLine, ActivityPopover, ActivityStatus,
    CANCEL_OPERATION_CAPTION, CANCELLED_GLYPH, CONFIRMED_CAPTION, EARLIER_NOT_KEPT,
    ENTRY_ROW_HEIGHT, FAILED_GLYPH, LINE_ROW_HEIGHT, REMOVE_LOCK_CAPTION,
    SHOW_IN_LOST_COMMITS_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 1200.;
const HEIGHT: f32 = 800.;

fn settle(test: &mut TestingRunner) {
    for _ in 0..4 {
        test.sync_and_update();
    }
}

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

fn click(test: &mut TestingRunner, caption: &str) {
    let at = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == caption)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("nothing reads {caption:?}: {:?}", labels(test)));
    test.click_cursor((f64::from(at.x), f64::from(at.y)));
    settle(test);
}

type Lines = Arc<VecDeque<ActivityLine>>;

fn entry(n: usize, lines: Lines) -> ActivityEntry {
    ActivityEntry {
        id: n as u64,
        name: format!("Operation {n}"),
        result: None,
        status: ActivityStatus::Succeeded,
        started: "12:00:00 UTC".to_owned(),
        took: Some("12 ms".to_owned()),
        sentence: None,
        prompt: None,
        replaced: None,
        cancellable: false,
        lines,
        earlier_not_kept: false,
    }
}

fn none() -> Lines {
    Arc::new(VecDeque::new())
}

#[derive(Clone, Default)]
struct Heard {
    selected: Rc<RefCell<Vec<usize>>>,
    cancelled: Rc<RefCell<Vec<usize>>>,
    replaced: Rc<RefCell<Vec<Oid>>>,
    closed: Rc<RefCell<usize>>,
}

fn launch(entries: Rc<Vec<ActivityEntry>>) -> (TestingRunner, Heard) {
    launch_hung(entries, None, WIDTH)
}

/// The popover over `entries`, hung from a status box laid out at `anchor`, in a window `width`
/// wide.
fn launch_hung(
    entries: Rc<Vec<ActivityEntry>>,
    anchor: Option<(f32, f32)>,
    width: f32,
) -> (TestingRunner, Heard) {
    let heard = Heard::default();
    let (mut test, _) = TestingRunner::new(
        {
            let heard = heard.clone();
            move || {
                let Heard {
                    selected,
                    cancelled,
                    replaced,
                    closed,
                } = heard.clone();
                rect().expanded().child(
                    ActivityPopover::new(entries.clone(), 0)
                        .anchor(anchor)
                        .on_select(move |at| selected.borrow_mut().push(at))
                        .on_cancel(move |at| cancelled.borrow_mut().push(at))
                        .on_replaced(move |commit| replaced.borrow_mut().push(commit))
                        .on_close(move |()| *closed.borrow_mut() += 1),
                )
            }
        },
        (width, HEIGHT).into(),
        |_| {},
        1.,
    );
    settle(&mut test);
    (test, heard)
}

/// R12.1, the lists virtualized: 200 operations and a selected one of 100,000 lines build one
/// viewport of rows each, never every operation or every line. Caught by: either list laid out
/// whole.
#[test]
fn the_popover_builds_one_viewport_of_operations_and_of_lines() {
    let lines: Lines = Arc::new(
        std::iter::once(ActivityLine::Ran("$ git commit -q -F -".to_owned()))
            .chain((0..100_000).map(|n| ActivityLine::Output(format!("hook line {n}"))))
            .collect(),
    );
    let entries: Rc<Vec<ActivityEntry>> =
        Rc::new((0..200).map(|n| entry(n, Arc::clone(&lines))).collect());
    let (test, _) = launch(entries);
    let shown = labels(&test);
    assert!(
        shown.iter().any(|label| label == ACTIVITY_TITLE),
        "{shown:?}"
    );
    let operations = shown
        .iter()
        .filter(|label| label.starts_with("Operation "))
        .count();
    // The selected one's name is drawn on the right too.
    assert!(operations > 1, "no operation drawn: {shown:?}");
    assert!(
        operations <= (460. / ENTRY_ROW_HEIGHT) as usize + 4,
        "{operations} operations built for one viewport"
    );
    let built = shown
        .iter()
        .filter(|label| label.starts_with("hook line "))
        .count();
    assert!(built > 0, "no line drawn");
    assert!(
        built <= (460. / LINE_ROW_HEIGHT) as usize + 4,
        "{built} lines built for one viewport"
    );
    assert!(shown.iter().any(|label| label == "$ git commit -q -F -"));
}

/// Each control reports what it is for: a press on an operation its place, a running one's
/// cancel its place, the way back the replaced commit; a press outside closes it, the way back
/// is drawn only where the entry has one, and `Remove index.lock…` nowhere (phase 19). Caught by: a control wired to another's handler, or drawn whatever the entry says.
#[test]
fn each_control_reports_what_it_is_for() {
    let replaced = Oid::from_bytes(&[0xcd; 20]).unwrap_or_else(|e| panic!("{e}"));
    let entries = Rc::new(vec![
        ActivityEntry {
            replaced: Some(replaced),
            prompt: Some("Amend cdcdcdc?".to_owned()),
            ..entry(0, none())
        },
        ActivityEntry {
            cancellable: true,
            status: ActivityStatus::Running,
            ..entry(1, none())
        },
    ]);
    let (mut test, heard) = launch(entries);
    assert!(labels(&test).iter().any(|label| label == "Amend cdcdcdc?"));
    click(&mut test, SHOW_IN_LOST_COMMITS_CAPTION);
    assert_eq!(*heard.replaced.borrow(), [replaced]);
    assert!(
        !labels(&test)
            .iter()
            .any(|label| label == REMOVE_LOCK_CAPTION),
        "the popover offers the lock's removal"
    );
    click(&mut test, "Operation 1");
    assert_eq!(*heard.selected.borrow(), [1]);
    click(&mut test, CANCEL_OPERATION_CAPTION);
    assert_eq!(*heard.cancelled.borrow(), [1]);
    test.click_cursor((5., f64::from(HEIGHT) - 5.));
    settle(&mut test);
    assert_eq!(*heard.closed.borrow(), 1);

    let (test, _) = launch(Rc::new(vec![entry(0, none())]));
    let shown = labels(&test);
    assert!(
        !shown
            .iter()
            .any(|label| label == SHOW_IN_LOST_COMMITS_CAPTION)
    );
    assert!(!shown.iter().any(|label| label == REMOVE_LOCK_CAPTION));
    assert!(!shown.iter().any(|label| label == CANCEL_OPERATION_CAPTION));
}

/// Where `text`'s label was laid out.
fn area(test: &TestingRunner, text: &str) -> freya::prelude::Area {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|_| node.layout().area)
    })
    .unwrap_or_else(|| panic!("nothing reads {text:?}: {:?}", labels(test)))
}

/// The user's decision A (2026-10-09): the popover hangs from the status box — its left edge
/// under the box's and just below it — and stays inside the window when the box is near its
/// right edge or the window is narrower than the panel. Caught by: a centred popover, or one
/// hung off the window's edge.
#[test]
fn the_popover_hangs_from_the_status_box_inside_the_window() {
    let entries = || Rc::new(vec![entry(0, none())]);
    // Room to hang under the box.
    let (test, _) = launch_hung(entries(), Some((300., 40.)), WIDTH);
    let title = area(&test, ACTIVITY_TITLE);
    assert!(
        (300. ..=330.).contains(&title.min_x()),
        "the title is at {}",
        title.min_x()
    );
    assert!(
        (40. ..=70.).contains(&title.min_y()),
        "the title is at {}",
        title.min_y()
    );
    // The box near the right edge: moved left to fit, its right edge inside the window.
    let (test, _) = launch_hung(entries(), Some((1_100., 40.)), WIDTH);
    let title = area(&test, ACTIVITY_TITLE);
    assert!(
        title.min_x() < 1_100. && title.min_x() > 300.,
        "{}",
        title.min_x()
    );
    let rightmost = test
        .find_many(|node, _| Some(node.layout().area.max_x()))
        .into_iter()
        .fold(0f32, f32::max);
    assert!(
        rightmost <= WIDTH,
        "drawn to {rightmost} in a window {WIDTH} wide"
    );
    // A window narrower than the panel: the panel narrows to it.
    let (test, _) = launch_hung(entries(), Some((40., 40.)), 600.);
    let rightmost = test
        .find_many(|node, _| Some(node.layout().area.max_x()))
        .into_iter()
        .fold(0f32, f32::max);
    assert!(
        rightmost <= 600.,
        "drawn to {rightmost} in a window 600 wide"
    );
}

/// C29's row (observed `am1`): its name, Fork's result line under it, its time, ⚠ on a failure
/// and a shape of its own on a cancel — no status word; the selected one's header its word with
/// its time and how long it took. Caught by: a status word drawn in a row, a glyph on a success,
/// or the duration left out.
#[test]
fn each_row_is_forks_name_result_time_and_glyph() {
    let entries = Rc::new(vec![
        ActivityEntry {
            result: Some("stage failed".to_owned()),
            status: ActivityStatus::Failed,
            sentence: Some("An unexpected error occurred.".to_owned()),
            ..entry(0, none())
        },
        ActivityEntry {
            result: Some("staged".to_owned()),
            ..entry(1, none())
        },
        ActivityEntry {
            status: ActivityStatus::Cancelled,
            ..entry(2, none())
        },
    ]);
    let (test, _) = launch(entries);
    let shown = labels(&test);
    for text in ["stage failed", "staged", FAILED_GLYPH, CANCELLED_GLYPH] {
        assert!(
            shown.iter().any(|label| label == text),
            "{text:?}: {shown:?}"
        );
    }
    assert_eq!(
        shown.iter().filter(|label| *label == FAILED_GLYPH).count(),
        1,
        "{shown:?}"
    );
    assert_eq!(
        shown
            .iter()
            .filter(|label| *label == "12:00:00 UTC")
            .count(),
        3,
        "{shown:?}"
    );
    for word in ["running", "succeeded", "failed", "cancelled"] {
        assert!(!shown.iter().any(|label| label == word), "{shown:?}");
    }
    assert!(
        shown
            .iter()
            .any(|label| label == "failed 12:00:00 UTC · took 12 ms"),
        "{shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|label| label == "An unexpected error occurred.")
    );
}

/// C29 and F3: the sentence and the confirmed prompt are drawn whole and wrapped above the lines
/// — never cut, with no Show All — and the lines keep one fixed row height beneath them; an
/// operation whose output passed its bound starts with "Earlier output not kept.". Caught by: a
/// prompt cut, lines drawn over it, or the cut unsaid.
#[test]
fn the_prompt_is_whole_above_fixed_lines_and_a_cut_is_said() {
    let prompt = (0..8)
        .map(|n| format!("Sentence {n} of what the confirmation said about the loss."))
        .collect::<Vec<_>>()
        .join(" ");
    let lines: Lines = Arc::new(
        std::iter::once(ActivityLine::Ran("$ git commit -q -F -".to_owned()))
            .chain((0..40).map(|n| ActivityLine::Output(format!("hook line {n}"))))
            .collect(),
    );
    let (test, _) = launch(Rc::new(vec![ActivityEntry {
        prompt: Some(prompt.clone()),
        earlier_not_kept: true,
        ..entry(0, lines)
    }]));
    let whole = area(&test, &prompt);
    assert!(whole.height() > 3. * 14., "cut to {} px", whole.height());
    assert!(labels(&test).iter().any(|label| label == CONFIRMED_CAPTION));
    let earlier = area(&test, EARLIER_NOT_KEPT);
    assert!(
        earlier.min_y() >= whole.max_y(),
        "the lines drawn over the prompt"
    );
    let ran = area(&test, "$ git commit -q -F -");
    assert!(
        ((ran.min_y() - earlier.min_y()) - LINE_ROW_HEIGHT).abs() < 1.,
        "rows not of one fixed height"
    );
    assert!(!labels(&test).iter().any(|label| label == "Show All"));
}

/// Phase 20's QA item 1, as Fork does it (observed first-hand 2026-10-11,
/// `docs/research/staging-and-commit/fork-activity-result-lines-2026-10-11.md`): the selected
/// entry's lines open, for a running operation, at the end of its output so far — its newest
/// line built — and, for a finished one, at its top — its `$ git …` line built — over the window's
/// one tail's worth of lines and fifty more. Caught by: every entry opened at its top (the
/// newest of a running hook's flood a scroll away) or at its end.
#[test]
fn a_running_entry_opens_at_its_end_and_a_finished_one_at_its_top() {
    // `crate::output_tail::OUTPUT_LINES` in the window, and fifty more.
    let count = 10_000 + 50;
    let lines: Lines = Arc::new(
        std::iter::once(ActivityLine::Ran("$ git commit -q -F -".to_owned()))
            .chain((0..count).map(|n| ActivityLine::Output(format!("hook line {n}"))))
            .collect(),
    );
    let newest = format!("hook line {}", count - 1);
    let (test, _) = launch(Rc::new(vec![ActivityEntry {
        status: ActivityStatus::Running,
        took: None,
        cancellable: true,
        ..entry(0, Arc::clone(&lines))
    }]));
    let shown = labels(&test);
    assert!(shown.contains(&newest), "{shown:?}");
    assert!(!shown.iter().any(|label| label == "$ git commit -q -F -"));
    let (test, _) = launch(Rc::new(vec![entry(0, lines)]));
    let shown = labels(&test);
    assert!(
        shown.iter().any(|label| label == "$ git commit -q -F -"),
        "{shown:?}"
    );
    assert!(!shown.contains(&newest));
}
