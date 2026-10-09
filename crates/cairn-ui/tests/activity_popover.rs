//! Headless tests for the activity popover (staging-and-commit R12.1): its operations and the
//! selected one's lines are each drawn through a virtualizing view, so a session of many
//! operations and a `git` that said a hundred thousand lines build one viewport of rows each;
//! a press on an operation, its cancel, the way back and `Remove index.lock…` each report; it
//! hangs from the status box inside the window (the user's decision A, 2026-10-09); the lock's
//! removal stays drawn while it cannot be pressed, saying why (H); and a long confirmed prompt is
//! cut to four lines, Show All making the whole of it reachable (K).

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::Oid;
use cairn_ui::{
    ACTIVITY_TITLE, ActivityEntry, ActivityLine, ActivityPopover, CANCEL_OPERATION_CAPTION,
    ENTRY_ROW_HEIGHT, LINE_ROW_HEIGHT, LockOffer, REMOVE_LOCK_CAPTION, SHOW_ALL_CAPTION,
    SHOW_LESS_CAPTION, SHOW_REPLACED_CAPTION,
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

fn entry(n: usize, lines: Rc<Vec<ActivityLine>>) -> ActivityEntry {
    ActivityEntry {
        name: format!("Operation {n}"),
        status: "succeeded".to_owned(),
        started: "12:00:00 UTC".to_owned(),
        took: Some("12 ms".to_owned()),
        message: None,
        prompt: None,
        replaced: None,
        lock: None,
        cancellable: false,
        lines,
    }
}

#[derive(Clone, Default)]
struct Heard {
    selected: Rc<RefCell<Vec<usize>>>,
    cancelled: Rc<RefCell<Vec<usize>>>,
    replaced: Rc<RefCell<Vec<Oid>>>,
    removed: Rc<RefCell<Vec<usize>>>,
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
                    removed,
                    closed,
                } = heard.clone();
                rect().expanded().child(
                    ActivityPopover::new(entries.clone(), 0)
                        .anchor(anchor)
                        .on_select(move |at| selected.borrow_mut().push(at))
                        .on_cancel(move |at| cancelled.borrow_mut().push(at))
                        .on_replaced(move |commit| replaced.borrow_mut().push(commit))
                        .on_remove_lock(move |at| removed.borrow_mut().push(at))
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
    let lines: Rc<Vec<ActivityLine>> = Rc::new(
        std::iter::once(ActivityLine::Ran("$ git commit -q -F -".to_owned()))
            .chain((0..100_000).map(|n| ActivityLine::Output(format!("hook line {n}"))))
            .collect(),
    );
    let entries: Rc<Vec<ActivityEntry>> =
        Rc::new((0..200).map(|n| entry(n, Rc::clone(&lines))).collect());
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
/// cancel its place, the way back the replaced commit, `Remove index.lock…` its place;
/// a press outside closes it, and neither the way back nor the removal is drawn where the entry
/// has none. Caught by: a control wired to another's handler, or drawn whatever the entry says.
#[test]
fn each_control_reports_what_it_is_for() {
    let replaced = Oid::from_bytes(&[0xcd; 20]).unwrap_or_else(|e| panic!("{e}"));
    let none = Rc::new(Vec::new());
    let entries = Rc::new(vec![
        ActivityEntry {
            replaced: Some(replaced),
            lock: Some(LockOffer::Ready { note: None }),
            prompt: Some("Amend cdcdcdc?".to_owned()),
            ..entry(0, Rc::clone(&none))
        },
        ActivityEntry {
            cancellable: true,
            status: "running".to_owned(),
            ..entry(1, Rc::clone(&none))
        },
    ]);
    let (mut test, heard) = launch(entries);
    assert!(labels(&test).iter().any(|label| label == "Amend cdcdcdc?"));
    click(&mut test, SHOW_REPLACED_CAPTION);
    assert_eq!(*heard.replaced.borrow(), [replaced]);
    click(&mut test, REMOVE_LOCK_CAPTION);
    assert_eq!(*heard.removed.borrow(), [0]);
    click(&mut test, "Operation 1");
    assert_eq!(*heard.selected.borrow(), [1]);
    click(&mut test, CANCEL_OPERATION_CAPTION);
    assert_eq!(*heard.cancelled.borrow(), [1]);
    test.click_cursor((5., f64::from(HEIGHT) - 5.));
    settle(&mut test);
    assert_eq!(*heard.closed.borrow(), 1);

    let (test, _) = launch(Rc::new(vec![entry(0, none)]));
    let shown = labels(&test);
    assert!(!shown.iter().any(|label| label == SHOW_REPLACED_CAPTION));
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
    let entries = || Rc::new(vec![entry(0, Rc::new(Vec::new()))]);
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

/// The user's decision H (2026-10-09): `Remove index.lock…` stays drawn while a `git` of
/// Cairn's runs, saying why, and its press reports nothing; offered, a refusal of the last press
/// is said beside it and it can be pressed again. Caught by: an offer that vanishes while a git
/// runs, or one pressable when it cannot go ahead.
#[test]
fn the_lock_removal_says_why_it_cannot_be_pressed() {
    let none = Rc::new(Vec::new());
    let (mut test, heard) = launch(Rc::new(vec![ActivityEntry {
        lock: Some(LockOffer::Blocked("Cairn is running git".to_owned())),
        ..entry(0, Rc::clone(&none))
    }]));
    assert!(
        labels(&test)
            .iter()
            .any(|label| label == "Cairn is running git")
    );
    click(&mut test, REMOVE_LOCK_CAPTION);
    assert!(heard.removed.borrow().is_empty(), "pressed while blocked");

    let (mut test, heard) = launch(Rc::new(vec![ActivityEntry {
        lock: Some(LockOffer::Ready {
            note: Some("index.lock is no longer there".to_owned()),
        }),
        ..entry(0, none)
    }]));
    assert!(
        labels(&test)
            .iter()
            .any(|label| label == "index.lock is no longer there")
    );
    click(&mut test, REMOVE_LOCK_CAPTION);
    assert_eq!(*heard.removed.borrow(), [0]);
}

/// The user's decision K (2026-10-09): a confirmed prompt longer than four lines is cut to them,
/// with Show All; pressed, the whole prompt is laid out above the lines, every line of it
/// reachable by scrolling the pane down to the last of git's lines; Show Less cuts it again. A
/// short prompt offers neither. Caught by: a cut prompt with no way to read the rest, a prompt
/// shown whole but clipped by its row, or lines drawn over it.
#[test]
fn show_all_makes_the_whole_confirmed_prompt_reachable() {
    let prompt = (0..14)
        .map(|n| format!("Sentence {n} of what the confirmation said about the loss."))
        .collect::<Vec<_>>()
        .join(" ");
    let lines: Rc<Vec<ActivityLine>> = Rc::new(
        std::iter::once(ActivityLine::Ran("$ git commit -q -F -".to_owned()))
            .chain((0..40).map(|n| ActivityLine::Output(format!("hook line {n}"))))
            .collect(),
    );
    let (mut test, _) = launch(Rc::new(vec![ActivityEntry {
        prompt: Some(prompt.clone()),
        ..entry(0, lines)
    }]));
    let cut = area(&test, &prompt);
    assert!(cut.height() <= 4. * 18., "cut to {} px", cut.height());
    assert!(!labels(&test).iter().any(|label| label == SHOW_LESS_CAPTION));
    click(&mut test, SHOW_ALL_CAPTION);
    settle(&mut test);
    let whole = area(&test, &prompt);
    assert!(
        whole.height() > cut.height() * 2.,
        "shown whole at {} px, cut at {}",
        whole.height(),
        cut.height()
    );
    // Its row holds it whole: the first of git's lines starts below it.
    let ran = area(&test, "$ git commit -q -F -");
    assert!(
        ran.min_y() >= whole.max_y(),
        "a line at {} drawn over the prompt ending at {}",
        ran.min_y(),
        whole.max_y()
    );
    // It scrolls with the pane: the last of git's lines is reachable below it.
    let at = (f64::from(ran.center().x), f64::from(ran.min_y()) - 20.);
    test.scroll(at, (0., -1e6));
    settle(&mut test);
    assert!(
        labels(&test).iter().any(|label| label == "hook line 39"),
        "{:?}",
        labels(&test)
    );
    test.scroll(at, (0., 1e6));
    settle(&mut test);
    click(&mut test, SHOW_LESS_CAPTION);
    let again = area(&test, &prompt);
    assert!(
        (again.height() - cut.height()).abs() < 1.,
        "{}",
        again.height()
    );
    assert!(labels(&test).iter().any(|label| label == SHOW_ALL_CAPTION));

    // A prompt taller than the pane: shown whole, it is scrolled past to git's last line.
    let tall = prompt.repeat(5);
    let lines: Rc<Vec<ActivityLine>> = Rc::new(
        (0..40)
            .map(|n| ActivityLine::Output(format!("hook line {n}")))
            .collect(),
    );
    let (mut test, _) = launch(Rc::new(vec![ActivityEntry {
        prompt: Some(tall.clone()),
        ..entry(0, lines)
    }]));
    let cut = area(&test, &tall);
    click(&mut test, SHOW_ALL_CAPTION);
    let whole = area(&test, &tall);
    assert!(
        whole.height() > 400.,
        "shown whole at {} px",
        whole.height()
    );
    assert!(!labels(&test).iter().any(|label| label == "hook line 0"));
    let at = (f64::from(cut.center().x), f64::from(cut.min_y()) + 40.);
    test.scroll(at, (0., -1e6));
    settle(&mut test);
    assert!(
        labels(&test).iter().any(|label| label == "hook line 39"),
        "{:?}",
        labels(&test)
    );

    let (test, _) = launch(Rc::new(vec![ActivityEntry {
        prompt: Some("Amend cdcdcdc?".to_owned()),
        ..entry(0, Rc::new(Vec::new()))
    }]));
    assert!(!labels(&test).iter().any(|label| label == SHOW_ALL_CAPTION));
}
