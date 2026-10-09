//! The activity popover in the window, headless (staging-and-commit R12, C20's popover half):
//! the status box opens it over the session's operations; each draws its name, its `git` and
//! what that said with no token in it; `Remove index.lock…` appears exactly where the lane
//! offered it and asks the removal through the confirmation; an amend's way back finds the
//! replaced commit in Show Lost Commits. Updates are applied through `session::apply`, as the
//! worker's stream applies them.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use cairn_model::{CommandExit, CommandRecord, Consequence, Oid};
use cairn_ui::{
    ACTIVITY_TITLE, CANCEL_CAPTION, MainView, NO_ACTIVITY, REMOVE_LOCK_CAPTION,
    SHOW_REPLACED_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

use crate::local_changes_tests::{Submitted, apply, labels, launch};
use crate::local_writes::Asked;
use crate::window::View;
use crate::worker::{Done, LocalWrite, OperationId, RanBy, Request, Update, WriteEnding};

fn settle(test: &mut TestingRunner) {
    for _ in 0..4 {
        test.sync_and_update();
    }
}

fn drawn(test: &TestingRunner, text: &str) -> bool {
    labels(test).iter().any(|label| label == text)
}

fn centre(test: &TestingRunner, text: &str) -> (f64, f64) {
    let at = test
        .find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("nothing reads {text:?}: {:?}", labels(test)));
    (f64::from(at.x), f64::from(at.y))
}

/// Clicks the lowest label reading `text`: a dialog's button under its title of the same words.
fn click_lowest(test: &mut TestingRunner, text: &str) {
    let at = test
        .find_many(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .map(|_| node.layout().area.center())
        })
        .into_iter()
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap_or_else(|| panic!("nothing reads {text:?}: {:?}", labels(test)));
    test.click_cursor((f64::from(at.x), f64::from(at.y)));
    settle(test);
}

fn click(test: &mut TestingRunner, text: &str) {
    let at = centre(test, text);
    test.click_cursor(at);
    settle(test);
}

/// A write the window asked, started: kept as `LocalWrites` keeps one.
fn started(
    test: &mut TestingRunner,
    view: View,
    submitted: &Submitted,
    write: &LocalWrite,
) -> OperationId {
    let id = OperationId::next();
    let mut writes = view.writes;
    test.run_in(|| writes.write().asked(id, write));
    apply(test, view, submitted, Update::WriteStarted { id });
    id
}

fn record(arguments: &[&str], stderr: &str, code: i32) -> CommandRecord {
    CommandRecord {
        arguments: arguments
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect(),
        directory: None,
        started: SystemTime::now(),
        duration: Duration::from_millis(12),
        exit: CommandExit::Code(code),
        cancelled: false,
        stderr: stderr.to_owned(),
        stderr_cut: false,
    }
}

fn index_lock() -> Consequence {
    Consequence::RemoveLock {
        path: PathBuf::from("/home/ada/engine/.git/index.lock"),
        modified: SystemTime::UNIX_EPOCH,
        read_at: SystemTime::UNIX_EPOCH + Duration::from_secs(7_200),
        bytes: 0,
        device: 1,
        inode: 2,
    }
}

fn failed_on_the_lock() -> WriteEnding {
    WriteEnding::Failed {
        message: "git add failed: fatal: Unable to create index.lock".to_owned(),
        locks: vec![PathBuf::from("/home/ada/engine/.git/index.lock")],
        command: Some("git add".to_owned()),
        output: "fatal: Unable to create index.lock".to_owned(),
    }
}

/// Opens the popover by pressing the status box: the repository's name in the title bar.
fn open(test: &mut TestingRunner) {
    click(test, "engine");
}

/// C20's popover half and R12.2: the status box opens the popover; before anything has run it
/// says so; a write that ran draws its name, its status, its `$ git` and what git said, with no
/// token anywhere — not in the arguments, not in stderr, not in the line under the lists — and
/// Escape closes it. Caught by: a status box that opens nothing, a popover drawing an operation
/// without its `git`, or any of git's text drawn as git wrote it.
#[test]
fn the_status_box_opens_the_operations_with_their_git_and_no_token() {
    let (mut test, view, submitted) = launch();
    open(&mut test);
    assert!(drawn(&test, ACTIVITY_TITLE), "{:?}", labels(&test));
    assert!(drawn(&test, NO_ACTIVITY));
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    assert!(!drawn(&test, ACTIVITY_TITLE));

    let write = LocalWrite::StageFiles {
        paths: vec!["a.rs".into()],
    };
    let id = started(&mut test, view, &submitted, &write);
    apply(
        &mut test,
        view,
        &submitted,
        Update::OperationRan {
            by: RanBy::Write(id),
            commands: vec![record(
                &[
                    "--literal-pathspecs",
                    "add",
                    "https://ada:ghp_TOKEN@example.com/r",
                ],
                "hint: https://ada:ghp_TOKEN@example.com/r refused",
                1,
            )],
            lock: None,
        },
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::Failed {
                message: "git add failed: https://ada:ghp_TOKEN@example.com/r refused".to_owned(),
                locks: Vec::new(),
                command: Some("git add".to_owned()),
                output: "https://ada:ghp_TOKEN@example.com/r refused".to_owned(),
            },
            read_again: crate::worker::ReadAgain::Status,
        },
    );
    open(&mut test);
    for text in [
        "Staging 1 file",
        "$ git --literal-pathspecs add https://example.com/r",
        "hint: https://example.com/r refused",
        "(exit status 1)",
    ] {
        assert!(drawn(&test, text), "{text:?} missing: {:?}", labels(&test));
    }
    let texts = labels(&test);
    assert!(
        texts.iter().all(|text| !text.contains("TOKEN")),
        "a token was drawn: {texts:?}"
    );
    assert!(!drawn(&test, REMOVE_LOCK_CAPTION), "offered with no lock");
}

/// R12.4 and C20: `Remove index.lock…` appears exactly where the lane offered it — an ending
/// that names the lock with no `git` of Cairn's running — and its press opens the confirmation,
/// whose button asks the local lane for the removal with the token built from that consequence.
/// An ending naming the lock that the lane did not offer (a `git` was running) offers nothing.
/// Caught by: an offer drawn on the lock alone, or the removal asked without its confirmation.
#[test]
fn remove_index_lock_is_offered_where_the_lane_offered_it_and_asks_through_its_confirmation() {
    let (mut test, view, submitted) = launch();
    let write = LocalWrite::StageFiles {
        paths: vec!["a.rs".into()],
    };
    // The lock named, a git running: the lane offers nothing.
    let held = started(&mut test, view, &submitted, &write);
    for update in [
        Update::OperationRan {
            by: RanBy::Write(held),
            commands: vec![record(&["add"], "fatal: Unable to create index.lock", 128)],
            lock: None,
        },
        Update::WriteEnded {
            id: held,
            ending: failed_on_the_lock(),
            read_again: crate::worker::ReadAgain::Status,
        },
    ] {
        apply(&mut test, view, &submitted, update);
    }
    open(&mut test);
    assert!(!drawn(&test, REMOVE_LOCK_CAPTION), "{:?}", labels(&test));
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);

    // The lock named, nothing running: offered.
    let stranded = started(&mut test, view, &submitted, &write);
    for update in [
        Update::OperationRan {
            by: RanBy::Write(stranded),
            commands: vec![record(&["add"], "fatal: Unable to create index.lock", 128)],
            lock: Some(index_lock()),
        },
        Update::WriteEnded {
            id: stranded,
            ending: failed_on_the_lock(),
            read_again: crate::worker::ReadAgain::Status,
        },
    ] {
        apply(&mut test, view, &submitted, update);
    }
    open(&mut test);
    assert!(drawn(&test, REMOVE_LOCK_CAPTION), "{:?}", labels(&test));
    // The older entry, selected, offers nothing; opened again, the newest is drawn.
    click_lowest(&mut test, "Staging 1 file");
    assert!(
        !drawn(&test, REMOVE_LOCK_CAPTION),
        "the older entry offered it"
    );
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    submitted.borrow_mut().clear();
    open(&mut test);
    click(&mut test, REMOVE_LOCK_CAPTION);
    let prompt = index_lock().prompt();
    assert!(drawn(&test, &prompt), "{:?}", labels(&test));
    assert!(
        submitted.borrow().is_empty(),
        "asked before it was confirmed"
    );
    click_lowest(&mut test, &index_lock().action());
    let asked: Vec<_> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write {
                write: LocalWrite::RemoveLock(confirmed),
                ..
            } => Some(confirmed.prompt().to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(asked, [prompt]);
    assert!(
        !drawn(&test, CANCEL_CAPTION),
        "the confirmation stayed open"
    );
}

/// R12.1's way back: an amend that succeeded draws the prompt it confirmed and offers its
/// replaced commit, whose press switches to All Commits, turns Show Lost Commits on and looks
/// for the commit in the walk that draws it. Caught by: a way back that changes nothing, or one
/// offered for an amend that did not run.
#[test]
fn an_amends_way_back_finds_the_replaced_commit_in_show_lost_commits() {
    let (mut test, view, submitted) = launch();
    let replaced = Oid::from_bytes(&[0xcd; 20]).unwrap();
    let mut writes = view.writes;
    let id = OperationId::next();
    test.run_in(|| {
        let mut writes = writes.write();
        writes.asked(
            id,
            &LocalWrite::Commit {
                message: "amend stand-in".to_owned(),
                skip_hooks: false,
            },
        );
    });
    // An amend's asked state, as `local_writes::ask` keeps one: its replaced commit.
    apply(&mut test, view, &submitted, Update::WriteStarted { id });
    let mut activity = view.activity;
    test.run_in(|| {
        activity.write().write_started(&Asked {
            id,
            what: "amend".to_owned(),
            noun: "the amend",
            replaces: Some(replaced),
            cancellable: true,
        });
    });
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::Done(Done {
                description: "amended".to_owned(),
                acknowledged: Some("Replaces cdcdcdc 'old'.".to_owned()),
                locks_before: Vec::new(),
                locks_after: Vec::new(),
            }),
            read_again: crate::worker::ReadAgain::Everything,
        },
    );
    open(&mut test);
    assert!(
        drawn(&test, "Replaces cdcdcdc 'old'."),
        "{:?}",
        labels(&test)
    );
    submitted.borrow_mut().clear();
    click(&mut test, SHOW_REPLACED_CAPTION);
    assert!(!drawn(&test, ACTIVITY_TITLE), "the popover stayed open");
    assert!(*view.show_lost.peek());
    assert_eq!(*view.sidebar.main.peek(), MainView::AllCommits);
    let asked = submitted.borrow();
    assert!(
        asked
            .iter()
            .any(|request| matches!(request, Request::OpenHistory { lost: true, .. })),
        "{asked:?}"
    );
    assert!(
        asked.iter().any(
            |request| matches!(request, Request::FindRow { target, .. } if *target == replaced)
        ),
        "{asked:?}"
    );
}
