//! Headless tests for the activity popover (staging-and-commit R12.1): its operations and the
//! selected one's lines are each drawn through a virtualizing view, so a session of many
//! operations and a `git` that said a hundred thousand lines build one viewport of rows each;
//! a press on an operation, its cancel, the way back and `Remove index.lock…` each report.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use cairn_model::{Consequence, Oid};
use cairn_ui::{
    ACTIVITY_TITLE, ActivityEntry, ActivityLine, ActivityPopover, CANCEL_OPERATION_CAPTION,
    ENTRY_ROW_HEIGHT, LINE_ROW_HEIGHT, REMOVE_LOCK_CAPTION, SHOW_REPLACED_CAPTION,
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
    removed: Rc<RefCell<usize>>,
    closed: Rc<RefCell<usize>>,
}

fn launch(entries: Rc<Vec<ActivityEntry>>) -> (TestingRunner, Heard) {
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
                        .on_select(move |at| selected.borrow_mut().push(at))
                        .on_cancel(move |at| cancelled.borrow_mut().push(at))
                        .on_replaced(move |commit| replaced.borrow_mut().push(commit))
                        .on_remove_lock(move |_: Rc<Consequence>| *removed.borrow_mut() += 1)
                        .on_close(move |()| *closed.borrow_mut() += 1),
                )
            }
        },
        (WIDTH, HEIGHT).into(),
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
/// cancel its place, the way back the replaced commit, `Remove index.lock…` its consequence;
/// a press outside closes it, and neither the way back nor the removal is drawn where the entry
/// has none. Caught by: a control wired to another's handler, or drawn whatever the entry says.
#[test]
fn each_control_reports_what_it_is_for() {
    let replaced = Oid::from_bytes(&[0xcd; 20]).unwrap_or_else(|e| panic!("{e}"));
    let lock = Consequence::RemoveLock {
        path: PathBuf::from("/r/.git/index.lock"),
        modified: SystemTime::UNIX_EPOCH,
        read_at: SystemTime::UNIX_EPOCH + Duration::from_secs(60),
        bytes: 0,
        device: 1,
        inode: 2,
    };
    let none = Rc::new(Vec::new());
    let entries = Rc::new(vec![
        ActivityEntry {
            replaced: Some(replaced),
            lock: Some(Rc::new(lock)),
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
    assert_eq!(*heard.removed.borrow(), 1);
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
