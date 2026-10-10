//! The activity popover in the window, headless (staging-and-commit R12, C20's popover half):
//! the status box opens it over the session's operations; each draws its name, its `git` and
//! what that said with no token in it; `Remove index.lock…` appears exactly where the lane
//! offered it and asks the removal through the confirmation; an amend's way back finds the
//! replaced commit in Show Lost Commits. Updates are applied through `session::apply`, as the
//! worker's stream applies them.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use cairn_model::{
    CommandExit, CommandRecord, Confirmed, Consequence, Oid, Publication, Reflog, ScrubbedLines,
    UnstagedChange,
};
use cairn_ui::{
    ACTIVITY_TITLE, CANCEL_CAPTION, MainView, NO_ACTIVITY, REMOVE_LOCK_CAPTION,
    SHOW_REPLACED_CAPTION,
};

use crate::activity::{GIT_RUNNING_NOTE, LOCKS_AT_OPEN_NAME, REMOVE_LOCK_TITLE};
use freya::prelude::*;
use freya_testing::TestingRunner;

use crate::local_changes_tests::{
    Submitted, apply, changed, labels, launch, open_local_changes, press_row, status,
};
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
        stderr: ScrubbedLines::scrubbing(stderr),
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
        output: ScrubbedLines::scrubbing("fatal: Unable to create index.lock"),
    }
}

/// Opens the popover by pressing the status box: the repository's name in the title bar.
fn open(test: &mut TestingRunner) {
    click(test, "engine");
}

/// C20's popover half and R12.2: the status box opens the popover; before anything has run it
/// says so; a write that ran draws its name, its status, its `$ git` and what git said — as the
/// engine hands them on, git's text in `ScrubbedLines` and the arguments scrubbed as they were
/// recorded (R4.10), so no token anywhere — and Escape closes it. Caught by: a status box that
/// opens nothing, a popover drawing an operation without its `git`, or git's text drawn other
/// than through the type.
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
                &["--literal-pathspecs", "add", "https://example.com/r"],
                "hint: https://ada:ghp_TOKEN@example.com/r refused",
                1,
            )],
            lock_named: false,
        },
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::Failed {
                message: "git add failed: https://example.com/r refused".to_owned(),
                locks: Vec::new(),
                command: Some("git add".to_owned()),
                output: ScrubbedLines::scrubbing("https://ada:ghp_TOKEN@example.com/r refused"),
            },
            read_again: crate::worker::ReadAgain::Status,
        },
    );
    open(&mut test);
    for text in [
        "Stage 1 file",
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

/// The press of `Remove index.lock…` on `test`'s newest entry: the consequence asked of the local
/// lane, under the number the request carries.
fn press_remove_lock(test: &mut TestingRunner, submitted: &Submitted) -> OperationId {
    submitted.borrow_mut().clear();
    click(test, REMOVE_LOCK_CAPTION);
    let asked: Vec<OperationId> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::LockConsequence { asked } => Some(*asked),
            _ => None,
        })
        .collect();
    assert_eq!(asked.len(), 1, "{:?}", submitted.borrow());
    asked[0]
}

fn removals(submitted: &Submitted) -> Vec<String> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write {
                write: LocalWrite::RemoveLock(confirmed),
                ..
            } => Some(confirmed.prompt().to_owned()),
            _ => None,
        })
        .collect()
}

/// R12.4 and C20, with the user's decisions G and H of 2026-10-09: `Remove index.lock…` is
/// offered on an ending that names the lock; while a `git` of Cairn's runs it is drawn
/// unpressable, saying why, rather than vanishing; pressed, it asks the local lane what the
/// removal would cost NOW, and only that answer opens the "Remove Stale Lock" confirmation, whose
/// button asks the removal with the token built from it; a refusal at the press is said beside
/// the button, which stays. Caught by: an offer lost because a `git` ran as the write ended, a
/// consequence read once at the ending (a stale age), the removal asked without its
/// confirmation, or a refusal that says nothing.
#[test]
fn remove_index_lock_asks_its_consequence_at_the_press_and_asks_through_its_confirmation() {
    let (mut test, view, submitted) = launch();
    let write = LocalWrite::StageFiles {
        paths: vec!["a.rs".into()],
    };
    let stranded = started(&mut test, view, &submitted, &write);
    for update in [
        Update::OperationRan {
            by: RanBy::Write(stranded),
            commands: vec![record(&["add"], "fatal: Unable to create index.lock", 128)],
            lock_named: true,
        },
        Update::WriteEnded {
            id: stranded,
            ending: failed_on_the_lock(),
            read_again: crate::worker::ReadAgain::Status,
        },
    ] {
        apply(&mut test, view, &submitted, update);
    }

    // A git of Cairn's running: drawn, saying why, and its press asks nothing.
    let running = started(&mut test, view, &submitted, &write);
    open(&mut test);
    click_lowest(&mut test, "Stage 1 file");
    assert!(drawn(&test, REMOVE_LOCK_CAPTION), "{:?}", labels(&test));
    assert!(drawn(&test, GIT_RUNNING_NOTE), "{:?}", labels(&test));
    submitted.borrow_mut().clear();
    click(&mut test, REMOVE_LOCK_CAPTION);
    assert!(
        !submitted
            .borrow()
            .iter()
            .any(|request| matches!(request, Request::LockConsequence { .. })),
        "asked while a git ran: {:?}",
        submitted.borrow()
    );
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: running,
            ending: WriteEnding::Done(Done {
                description: "staged".to_owned(),
                acknowledged: None,
                locks_before: Vec::new(),
                locks_after: Vec::new(),
            }),
            read_again: crate::worker::ReadAgain::Status,
        },
    );

    // Nothing running: pressable. The older entry, selected, offers nothing.
    open(&mut test);
    click_lowest(&mut test, "Stage 1 file");
    assert!(drawn(&test, REMOVE_LOCK_CAPTION), "{:?}", labels(&test));
    assert!(!drawn(&test, GIT_RUNNING_NOTE), "{:?}", labels(&test));
    click(&mut test, "Stage 1 file");
    assert!(
        !drawn(&test, REMOVE_LOCK_CAPTION),
        "the newer entry offered it"
    );
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);

    // Refused at the press: said beside the button, which stays; no confirmation.
    open(&mut test);
    click_lowest(&mut test, "Stage 1 file");
    let asked = press_remove_lock(&mut test, &submitted);
    let refusal = "index.lock is no longer there".to_owned();
    apply(
        &mut test,
        view,
        &submitted,
        Update::LockConsequence {
            asked,
            outcome: Err(refusal.clone()),
        },
    );
    assert!(drawn(&test, &refusal), "{:?}", labels(&test));
    assert!(drawn(&test, REMOVE_LOCK_CAPTION));
    assert!(!drawn(&test, REMOVE_LOCK_TITLE), "{:?}", labels(&test));

    // Read at the press: the confirmation opens over that answer, and only its button asks.
    let asked = press_remove_lock(&mut test, &submitted);
    assert!(!drawn(&test, &refusal), "the old refusal stayed");
    // An answer to an older press is let go of.
    apply(
        &mut test,
        view,
        &submitted,
        Update::LockConsequence {
            asked: OperationId::next(),
            outcome: Ok(index_lock()),
        },
    );
    assert!(
        !drawn(&test, REMOVE_LOCK_TITLE),
        "an older answer opened it"
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::LockConsequence {
            asked,
            outcome: Ok(index_lock()),
        },
    );
    let prompt = index_lock().prompt();
    assert!(drawn(&test, REMOVE_LOCK_TITLE), "{:?}", labels(&test));
    assert!(drawn(&test, &prompt), "{:?}", labels(&test));
    assert!(
        removals(&submitted).is_empty(),
        "asked before it was confirmed"
    );
    click_lowest(&mut test, &index_lock().action());
    assert_eq!(removals(&submitted), [prompt]);
    assert!(
        !drawn(&test, CANCEL_CAPTION),
        "the confirmation stayed open"
    );
}

/// The user's decision H of 2026-10-09: a lock found as the repository opened is an entry of its
/// own, naming each lock, with `Remove index.lock…` where the repository's own index lock is
/// among them; a lock file that is not the index's is named and offers nothing. Caught by: a lock
/// at open said only under the lists, or an offer for a lock Cairn would not remove.
#[test]
fn a_lock_found_at_open_is_an_entry_offering_its_removal() {
    let (mut test, view, submitted) = launch();
    apply(
        &mut test,
        view,
        &submitted,
        Update::LocksAtOpen {
            locks: vec![PathBuf::from("/home/ada/engine/.git/HEAD.lock")],
            index_lock: false,
        },
    );
    open(&mut test);
    assert!(drawn(&test, LOCKS_AT_OPEN_NAME), "{:?}", labels(&test));
    assert!(drawn(&test, "/home/ada/engine/.git/HEAD.lock"));
    assert!(!drawn(&test, REMOVE_LOCK_CAPTION), "offered for HEAD.lock");
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);

    apply(
        &mut test,
        view,
        &submitted,
        Update::LocksAtOpen {
            locks: vec![PathBuf::from("/home/ada/engine/.git/index.lock")],
            index_lock: true,
        },
    );
    open(&mut test);
    assert!(drawn(&test, "/home/ada/engine/.git/index.lock"));
    assert!(drawn(&test, REMOVE_LOCK_CAPTION), "{:?}", labels(&test));
    let asked = press_remove_lock(&mut test, &submitted);
    apply(
        &mut test,
        view,
        &submitted,
        Update::LockConsequence {
            asked,
            outcome: Ok(index_lock()),
        },
    );
    assert!(drawn(&test, REMOVE_LOCK_TITLE), "{:?}", labels(&test));
}

/// The user's decision N of 2026-10-09: a write refused before it started keeps its own name.
/// Caught by: "A write" drawn for a write the window asked by name.
#[test]
fn a_write_refused_before_it_started_keeps_its_name() {
    let (mut test, view, submitted) = launch();
    let id = OperationId::next();
    let mut writes = view.writes;
    test.run_in(|| {
        writes.write().asked(
            id,
            &LocalWrite::StageFiles {
                paths: vec!["a.rs".into(), "b.rs".into()],
            },
        )
    });
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::NotRun {
                message: "Not run: the repository was closing.".to_owned(),
            },
            read_again: crate::worker::ReadAgain::Status,
        },
    );
    open(&mut test);
    assert!(drawn(&test, "Stage 2 files"), "{:?}", labels(&test));
    assert!(!drawn(&test, "A write"), "{:?}", labels(&test));
}

/// R12.1, the merge bar's W2: a destructive write that git failed, or that was cancelled, still
/// quotes the prompt the user accepted — copied as it was asked,
/// since the verb spent the token and only an ending that ran carries it back — and so does one
/// that never ran. Caught by: the prompt drawn only for a write that succeeded.
#[test]
fn a_destructive_write_that_did_not_succeed_still_quotes_its_prompt() {
    let (mut test, view, submitted) = launch();
    let amend = Consequence::Amend {
        commit: Oid::from_bytes(&[0xab; 20]).unwrap(),
        published: Publication::Unpublished,
        reflog: Reflog::NotWritten,
    };
    let discarding = Consequence::CheckoutDiscarding {
        branch: "topic".to_owned(),
        at: Oid::from_bytes(&[0xcd; 20]).unwrap(),
        head: Some(Oid::from_bytes(&[1; 20]).unwrap()),
    };
    let lock = index_lock();
    let prompts = [amend.prompt(), discarding.prompt(), lock.prompt()];
    assert!(
        prompts.iter().all(|prompt| !prompt.is_empty()),
        "{prompts:?}"
    );

    // An amend cancelled before git made it.
    let killed = started(
        &mut test,
        view,
        &submitted,
        &LocalWrite::Amend {
            confirmed: Confirmed::by_user(amend),
            message: "Fix the parser properly\n".to_owned(),
            skip_hooks: false,
        },
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: killed,
            ending: WriteEnding::Cancelled {
                message: "git commit -q --amend -F - was cancelled".to_owned(),
                locks: Vec::new(),
            },
            read_again: crate::worker::ReadAgain::Everything,
        },
    );
    // Create Branch's discard, which git failed.
    let failed = started(
        &mut test,
        view,
        &submitted,
        &LocalWrite::CreateBranchDiscarding(Confirmed::by_user(discarding)),
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: failed,
            ending: WriteEnding::Failed {
                message: "git checkout failed: fatal: a branch named 'topic' already exists"
                    .to_owned(),
                locks: Vec::new(),
                command: Some("git checkout -q -f -b topic".to_owned()),
                output: ScrubbedLines::scrubbing("fatal: a branch named 'topic' already exists"),
            },
            read_again: crate::worker::ReadAgain::Everything,
        },
    );
    // A removal asked and never run: the repository was closing.
    let id = OperationId::next();
    let mut writes = view.writes;
    test.run_in(|| {
        writes
            .write()
            .asked(id, &LocalWrite::RemoveLock(Confirmed::by_user(lock)))
    });
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::NotRun {
                message: "Not run: the repository was closing.".to_owned(),
            },
            read_again: crate::worker::ReadAgain::Status,
        },
    );

    // Newest first: the removal, the checkout, the amend — each selected in turn.
    open(&mut test);
    let names: Vec<String> = test.run_in(|| {
        view.activity
            .peek()
            .newest_first()
            .map(|entry| entry.name.clone())
            .collect()
    });
    assert_eq!(names.len(), 3, "{names:?}");
    for (name, prompt) in names.iter().zip(prompts.iter().rev()) {
        click_lowest(&mut test, name);
        assert!(
            drawn(&test, prompt),
            "{name:?} did not quote {prompt:?}: {:?}",
            labels(&test)
        );
    }
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
            name: "Amend".to_owned(),
            awaited: "the amend to finish",
            replaces: Some(replaced),
            cancellable: true,
            prompt: None,
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

fn area(test: &TestingRunner, text: &str) -> Area {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|_| node.layout().area)
    })
    .unwrap_or_else(|| panic!("nothing reads {text:?}: {:?}", labels(test)))
}

/// The user's decision A of 2026-10-09: the popover hangs from the status box as the window
/// lays it out — its title just under the box, at the box's left edge — never centred. Caught
/// by: a popover placed without the box's laid-out position.
#[test]
fn the_popover_hangs_under_the_status_box() {
    let (mut test, _, _) = launch();
    let status_box = area(&test, "engine");
    open(&mut test);
    let title = area(&test, ACTIVITY_TITLE);
    assert!(
        title.min_y() > status_box.max_y() && title.min_y() < status_box.max_y() + 40.,
        "the title at {}, the box ending at {}",
        title.min_y(),
        status_box.max_y()
    );
    assert!(
        (title.min_x() - status_box.min_x()).abs() < 40.,
        "the title at {}, the box at {}",
        title.min_x(),
        status_box.min_x()
    );
}

/// Whether a write — or a refresh — was asked.
fn acted(submitted: &Submitted) -> bool {
    submitted
        .borrow()
        .iter()
        .any(|request| matches!(request, Request::Write { .. } | Request::Refresh))
}

/// The user's decision I of 2026-10-09: while the popover is open the window's chords and the
/// focused list's keys do nothing, as under a dialog — Enter stages nothing, F5 refreshes
/// nothing — and Escape closes it, after which they act again. Caught by: a chord heard through
/// the popover, or a list acting on a key pressed while it is open.
#[test]
fn while_the_popover_is_open_the_keys_do_nothing_but_close_it() {
    let (mut test, view, submitted) = launch();
    open_local_changes(&mut test);
    apply(
        &mut test,
        view,
        &submitted,
        status(vec![changed("a.rs", None, Some(UnstagedChange::Modified))]),
    );
    press_row(&mut test, "a.rs", 0);
    // The repository's name, starred: status lists a change.
    click(&mut test, "engine*");
    submitted.borrow_mut().clear();
    for key in [
        NamedKey::Enter,
        NamedKey::F5,
        NamedKey::ArrowDown,
        NamedKey::Delete,
    ] {
        test.press_key(Key::Named(key));
        settle(&mut test);
    }
    assert!(!acted(&submitted), "{:?}", submitted.borrow());
    assert!(view.confirming.peek().is_none(), "a discard was opened");
    assert!(drawn(&test, ACTIVITY_TITLE), "a key closed it");
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    assert!(!drawn(&test, ACTIVITY_TITLE));
    // Closed, they act again.
    test.press_key(Key::Named(NamedKey::F5));
    settle(&mut test);
    press_row(&mut test, "a.rs", 0);
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    let asked = submitted.borrow();
    assert!(
        asked
            .iter()
            .any(|request| matches!(request, Request::Refresh)),
        "{asked:?}"
    );
    assert!(
        asked.iter().any(|request| matches!(
            request,
            Request::Write {
                write: LocalWrite::StageFiles { .. },
                ..
            }
        )),
        "{asked:?}"
    );
}
