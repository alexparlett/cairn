//! The activity popover in the window, headless: the status box opens it over the session's
//! operations; each draws its name, its `git` and what that said with no token in it; an amend's
//! way back finds the replaced commit in Show Lost Commits. And the leftover lock: its banner
//! exactly while the stat finds the lock and no git of Cairn's runs or waits, `Remove index.lock…`
//! in a write's Git Error where git could not create it, both opening "Remove Stale Lock" over the
//! consequence read at the press. Updates are applied through `session::apply`, as the worker's
//! stream applies them.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use cairn_model::{
    CommandExit, CommandRecord, Confirmed, Consequence, Oid, Publication, Reflog, ScrubbedLines,
    UnstagedChange,
};
use cairn_ui::{
    ACTIVITY_GLYPH_NAME, ACTIVITY_TITLE, CANCEL_CAPTION, CANCELLED_GLYPH, CLOSE_CAPTION,
    EARLIER_NOT_KEPT, FAILED_GLYPH, GIT_ERROR_TITLE, LOCK_BANNER_TEXT, MainView, NO_ACTIVITY,
    REMOVE_LOCK_CAPTION, SHOW_IN_LOST_COMMITS_CAPTION,
};

use crate::operations::leftover_lock::REMOVE_LOCK_TITLE;
use freya::prelude::*;
use freya_testing::TestingRunner;

use crate::local_changes::tests::{
    Submitted, apply, changed, labels, launch, launch_counted, open_local_changes, press_row,
    status,
};
use crate::operations::local_writes::Asked;
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

/// Presses the status box's list glyph: the one place on the box that opens the popover.
fn press_glyph(test: &mut TestingRunner) {
    let at = test
        .find(|node, element| {
            Rect::try_downcast(element)
                .filter(|rect| rect.accessibility.builder.value() == Some(ACTIVITY_GLYPH_NAME))
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("no status box glyph"));
    test.click_cursor((f64::from(at.x), f64::from(at.y)));
    settle(test);
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
    WriteEnding::Failed(crate::worker::Failure {
        said: cairn_ui::GIT_ERROR_TEXT.to_owned(),
        command: Some("git add".to_owned()),
        output: ScrubbedLines::scrubbing("fatal: Unable to create index.lock"),
        locks: vec![PathBuf::from("/home/ada/engine/.git/index.lock")],
        index_lock: true,
    })
}

/// Opens the popover by pressing the status box's list glyph — once every ending's dialog over
/// the window is closed, as the one modal state asks.
fn open(test: &mut TestingRunner) {
    while drawn(test, GIT_ERROR_TITLE) || labels(test).iter().any(|l| l.starts_with("Couldn't")) {
        click_lowest(test, CLOSE_CAPTION);
    }
    press_glyph(test);
}

/// The status box's glyph opens the popover, and a press on the box's centre opens nothing, as
/// Fork's; before anything has run it says so; a write that ran draws its
/// name, its status, its `$ git` and what git said — as the engine hands them on, git's text in
/// `ScrubbedLines` and the arguments scrubbed as they were recorded, so no token anywhere — and
/// Escape closes it. Caught by: a status box that opens nothing, or opens from its centre, a
/// popover drawing an operation without its `git`, or git's text drawn other than through the
/// type.
#[test]
fn the_status_box_opens_the_operations_with_their_git_and_no_token() {
    let (mut test, view, submitted) = launch();
    click(&mut test, "engine");
    assert!(
        !drawn(&test, ACTIVITY_TITLE),
        "the box's centre opened the popover"
    );
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
        },
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::Failed(crate::worker::Failure {
                said: cairn_ui::GIT_ERROR_TEXT.to_owned(),
                command: Some("git add".to_owned()),
                output: ScrubbedLines::scrubbing("https://ada:ghp_TOKEN@example.com/r refused"),
                locks: Vec::new(),
                index_lock: false,
            }),
            read_again: crate::worker::ReadAgain::Status,
            unspent: crate::worker::Unspent::none(),
        },
    );
    // Its Git Error first: the popover opens once it is closed — one modal state.
    assert!(drawn(&test, GIT_ERROR_TITLE), "{:?}", labels(&test));
    assert!(!drawn(&test, REMOVE_LOCK_CAPTION), "offered with no lock");
    click_lowest(&mut test, CLOSE_CAPTION);
    open(&mut test);
    for text in [
        "Stage 1 File",
        "$ git --literal-pathspecs add https://example.com/r",
        "hint: https://example.com/r refused",
        "exit code 1",
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

/// The press of `Remove index.lock…` at `at`: the consequence asked of the local lane, under the
/// number the request carries.
fn press_remove_lock(test: &mut TestingRunner, submitted: &Submitted) -> OperationId {
    submitted.borrow_mut().clear();
    click_lowest(test, REMOVE_LOCK_CAPTION);
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

/// The Git Error's route, as Fork takes it: a write git refused for the lock opens its Git Error
/// with `Remove index.lock…` beside Close; pressed, it closes the dialog and asks the local lane
/// what the removal would cost NOW; only that press's answer opens "Remove Stale Lock" over its
/// prompt, whose button asks the removal with the token built from it — never the failed write
/// again; a refusal at the press is "Couldn't Remove index.lock" with its sentence. Caught by: the
/// removal offered on a failure git did not have for the lock, asked without its confirmation, an
/// older press's answer opening it, the write run again, or a refusal that says nothing.
#[test]
fn a_write_refused_for_the_lock_offers_its_removal_through_the_confirmation() {
    let (mut test, view, submitted) = launch();
    let write = LocalWrite::StageFiles {
        paths: vec!["a.rs".into()],
    };
    let stranded = started(&mut test, view, &submitted, &write);
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: stranded,
            ending: failed_on_the_lock(),
            read_again: crate::worker::ReadAgain::Status,
            unspent: crate::worker::Unspent::none(),
        },
    );
    assert!(drawn(&test, GIT_ERROR_TITLE), "{:?}", labels(&test));
    assert!(drawn(&test, "fatal: Unable to create index.lock"));

    // Refused at the press: "Couldn't Remove index.lock", no confirmation.
    let asked = press_remove_lock(&mut test, &submitted);
    assert!(!drawn(&test, GIT_ERROR_TITLE), "the Git Error stayed");
    apply(
        &mut test,
        view,
        &submitted,
        Update::LockConsequence {
            asked,
            outcome: Err("There is no index.lock any more.".to_owned()),
        },
    );
    assert!(
        drawn(&test, "Couldn't Remove index.lock"),
        "{:?}",
        labels(&test)
    );
    assert!(drawn(
        &test,
        "There is no index.lock any more. Nothing was changed."
    ));
    assert!(!drawn(&test, REMOVE_LOCK_TITLE));
    click_lowest(&mut test, CLOSE_CAPTION);

    // The banner's route, the lock old and nothing running: read at the press, the confirmation
    // opens over that answer alone, and only its button asks.
    apply(
        &mut test,
        view,
        &submitted,
        Update::IndexLock {
            changed: Some(SystemTime::now() - Duration::from_secs(3_600)),
        },
    );
    assert!(drawn(&test, LOCK_BANNER_TEXT), "{:?}", labels(&test));
    let asked = press_remove_lock(&mut test, &submitted);
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
        !submitted.borrow().iter().any(|request| matches!(
            request,
            Request::Write {
                write: LocalWrite::StageFiles { .. },
                ..
            }
        )),
        "the failed write was asked again"
    );
    assert!(
        !drawn(&test, CANCEL_CAPTION),
        "the confirmation stayed open"
    );
}

/// The leftover lock's banner: drawn exactly while the stat finds `<gitdir>/index.lock`, no git of
/// Cairn's runs or waits and the lock is older than about ten seconds; gone when a stat finds it
/// gone, whoever removed it. Caught by: a banner over Cairn's own write, over a fresh lock, or one
/// left after the file went.
#[test]
fn the_leftover_lock_banner_shows_exactly_while_it_should() {
    let (mut test, view, submitted) = launch();
    let old = SystemTime::now() - Duration::from_secs(60);
    assert!(!drawn(&test, LOCK_BANNER_TEXT), "a banner with no stat");
    apply(
        &mut test,
        view,
        &submitted,
        Update::IndexLock {
            changed: Some(SystemTime::now()),
        },
    );
    assert!(!drawn(&test, LOCK_BANNER_TEXT), "a lock seconds old");
    apply(
        &mut test,
        view,
        &submitted,
        Update::IndexLock { changed: Some(old) },
    );
    assert!(drawn(&test, LOCK_BANNER_TEXT), "{:?}", labels(&test));

    // A write of Cairn's running or queued: no banner, its lock may be that git's.
    let write = LocalWrite::StageFiles {
        paths: vec!["a.rs".into()],
    };
    let running = started(&mut test, view, &submitted, &write);
    assert!(
        !drawn(&test, LOCK_BANNER_TEXT),
        "drawn while Cairn's git ran"
    );
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
                replaced: None,
                cost_after: None,
            }),
            read_again: crate::worker::ReadAgain::Status,
            unspent: crate::worker::Unspent::none(),
        },
    );
    assert!(drawn(&test, LOCK_BANNER_TEXT), "{:?}", labels(&test));
    let queued = OperationId::next();
    let mut writes = view.writes;
    test.run_in(|| writes.write().asked(queued, &write));
    settle(&mut test);
    assert!(
        !drawn(&test, LOCK_BANNER_TEXT),
        "drawn while a write waited"
    );
    test.run_in(|| writes.write().ended(queued));
    settle(&mut test);

    // Removed by anyone: the next stat takes it down.
    apply(
        &mut test,
        view,
        &submitted,
        Update::IndexLock { changed: None },
    );
    assert!(!drawn(&test, LOCK_BANNER_TEXT), "left after the file went");
}

/// A write refused before it started keeps its own name. Caught by: "A write" drawn for a write the
/// window asked by name.
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
                said: "The repository was closing.".to_owned(),
            },
            read_again: crate::worker::ReadAgain::Status,
            unspent: crate::worker::Unspent::none(),
        },
    );
    // Closing is a refusal found while it waited — "Couldn't <its name>".
    assert!(
        drawn(&test, "Couldn't Stage 2 Files"),
        "{:?}",
        labels(&test)
    );
    assert!(drawn(
        &test,
        "The repository was closing. Nothing was changed."
    ));
    open(&mut test);
    assert!(drawn(&test, "Stage 2 Files"), "{:?}", labels(&test));
    assert!(!drawn(&test, "A write"), "{:?}", labels(&test));
}

/// A destructive write quotes the prompt the person accepted — copied as it was asked, since the
/// verb spent the token — whole, only once it ran: an amend cancelled while git ran does, a removal
/// never run does not; and Create Branch's Discard quotes none, its entry saying once it succeeded
/// what it did, under Fork's "Switched to branch 'topic'", or git's failure where it failed. Caught
/// by: "Confirmed:" over an operation that never happened, or the forced checkout's question
/// quoted.
#[test]
fn a_destructive_write_quotes_its_prompt_only_once_it_ran() {
    let (mut test, view, submitted) = launch();
    let amend = Consequence::Amend {
        commit: Oid::from_bytes(&[0xab; 20]).unwrap(),
        published: Publication::Unpublished,
        reflog: Reflog::NotWritten,
    };
    let discarding = || Consequence::CheckoutDiscarding {
        branch: "topic".to_owned(),
        at: Oid::from_bytes(&[0xcd; 20]).unwrap(),
        head: Some(Oid::from_bytes(&[1; 20]).unwrap()),
    };
    let lock = index_lock();
    let (amend_prompt, discard_prompt, lock_prompt) =
        (amend.prompt(), discarding().prompt(), lock.prompt());

    // An amend cancelled while git ran.
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
                locks: Vec::new(),
                ran: true,
            },
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        },
    );
    // Create Branch's discard, which git failed.
    let failed = started(
        &mut test,
        view,
        &submitted,
        &LocalWrite::CreateBranchDiscarding(Confirmed::by_user(discarding())),
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: failed,
            ending: WriteEnding::Failed(crate::worker::Failure {
                said: "git checkout failed: fatal: a branch named 'topic' already exists"
                    .to_owned(),
                command: Some("git checkout -q -f -b topic".to_owned()),
                output: ScrubbedLines::scrubbing("fatal: a branch named 'topic' already exists"),
                locks: Vec::new(),
                index_lock: false,
            }),
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
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
                said: "Not run: the repository was closing.".to_owned(),
            },
            read_again: crate::worker::ReadAgain::Status,
            unspent: crate::worker::Unspent::none(),
        },
    );
    // Create Branch's discard, which succeeded.
    let checked_out = started(
        &mut test,
        view,
        &submitted,
        &LocalWrite::CreateBranchDiscarding(Confirmed::by_user(discarding())),
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: checked_out,
            ending: WriteEnding::Done(Done {
                description: "checked out".to_owned(),
                acknowledged: Some(discard_prompt.clone()),
                locks_before: Vec::new(),
                locks_after: Vec::new(),
                replaced: None,
                cost_after: None,
            }),
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        },
    );

    // Newest first: the checkout that succeeded, the removal, the checkout that failed, the
    // amend — each selected in turn.
    open(&mut test);
    let names: Vec<String> = test.run_in(|| {
        view.activity
            .log
            .peek()
            .newest_first()
            .map(|entry| entry.name.clone())
            .collect()
    });
    assert_eq!(names.len(), 4, "{names:?}");
    assert!(
        drawn(&test, "Switched to branch 'topic'"),
        "{:?}",
        labels(&test)
    );
    assert!(drawn(
        &test,
        "Discarded local changes and any untracked files in the way, then checked out 'topic' \
         at cdcdcdc."
    ));
    assert!(
        !drawn(&test, &discard_prompt),
        "the forced checkout's prompt was quoted"
    );
    assert!(!drawn(&test, cairn_ui::CONFIRMED_CAPTION));
    click_lowest(&mut test, &names[1]);
    assert!(
        !drawn(&test, &lock_prompt),
        "a removal never run quoted its prompt"
    );
    assert!(!drawn(&test, cairn_ui::CONFIRMED_CAPTION));
    click_lowest(&mut test, &names[2]);
    assert!(
        !drawn(&test, &discard_prompt),
        "the forced checkout's prompt was quoted"
    );
    assert!(drawn(
        &test,
        "git checkout failed: fatal: a branch named 'topic' already exists"
    ));
    click_lowest(&mut test, &names[3]);
    assert!(drawn(&test, &amend_prompt), "{:?}", labels(&test));
    assert!(drawn(&test, cairn_ui::CONFIRMED_CAPTION));
    assert!(drawn(&test, crate::operations::activity::WRITE_CANCELLED));
}

/// An amend's way back: an amend that succeeded draws the prompt it confirmed and offers "Show in
/// Lost Commits", whose press switches to All Commits, turns Show Lost Commits on and looks for the
/// commit in the walk that draws it. Caught by: a way back that changes nothing, or one offered for
/// an amend that did not run.
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
                files: 1,
            },
        );
    });
    // An amend's asked state names no commit: its way back is what its run answered.
    apply(&mut test, view, &submitted, Update::WriteStarted { id });
    let mut activity = view.activity.log;
    test.run_in(|| {
        activity.write().write_started(&Asked {
            id,
            name: "Amend".to_owned(),
            cancellable: true,
            prompt: Some("Replaces cdcdcdc 'old'.".to_owned()),
            happened: None,
            results: crate::worker::ResultLines::default(),
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
                replaced: Some(replaced),
                cost_after: None,
            }),
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        },
    );
    open(&mut test);
    assert!(
        drawn(&test, "Replaces cdcdcdc 'old'."),
        "{:?}",
        labels(&test)
    );
    submitted.borrow_mut().clear();
    click(&mut test, SHOW_IN_LOST_COMMITS_CAPTION);
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

/// Where the status box's list glyph is laid out: at the box's left edge, its bottom near the
/// box's.
fn glyph_area(test: &TestingRunner) -> Area {
    test.find(|node, element| {
        Rect::try_downcast(element)
            .filter(|rect| rect.accessibility.builder.value() == Some(ACTIVITY_GLYPH_NAME))
            .map(|_| node.layout().area)
    })
    .unwrap_or_else(|| panic!("no status box glyph"))
}

/// The popover hangs from the status box as the window lays it out — its title just under the box,
/// never right of the box's left edge, where its glyph is. Caught by: a popover placed without
/// the box's laid-out position.
#[test]
fn the_popover_hangs_under_the_status_box() {
    let (mut test, _, _) = launch();
    let status_box = glyph_area(&test);
    open(&mut test);
    let title = area(&test, ACTIVITY_TITLE);
    assert!(
        title.min_y() > status_box.max_y() && title.min_y() < status_box.max_y() + 40.,
        "the title at {}, the box ending at {}",
        title.min_y(),
        status_box.max_y()
    );
    // The panel is wider than the room right of the box in the middle of the toolbar, so it
    // moves left to fit the window, its left edge never right of the box's.
    assert!(
        title.min_x() < status_box.min_x() + 40.,
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

/// While the popover is open the window's chords and the focused list's keys do nothing, as under a
/// dialog — Enter stages nothing, F5 refreshes nothing — and Escape closes it, after which they act
/// again. Caught by: a chord heard through the popover, or a list acting on a key pressed while it
/// is open.
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
    press_glyph(&mut test);
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

/// One read of a running write's output, as its lane wakes the window for it.
fn output(
    test: &mut TestingRunner,
    view: View,
    submitted: &Submitted,
    id: OperationId,
    text: &str,
) {
    apply(
        test,
        view,
        submitted,
        Update::WriteOutput {
            id,
            tap: crate::worker::OutputTap::holding(&ScrubbedLines::scrubbing(text)),
        },
    );
}

/// Fork's Activity Manager as the window draws it: each row its name, Fork's result line under it
/// ("staged", "stage failed"), its start time as `HH:MM:SS` in the window's zone, ⚠ on a failure and a shape of its
/// own on a cancel, and no status word; the selected one's header its name, one of four words with
/// its time, and how long it took; a failure's one sentence; and an operation whose output passed
/// its bound headed "Earlier output not kept." over its latest lines. Caught by: a status word in a
/// row, a result line or glyph missing, or the latest output dropped instead of the oldest.
#[test]
fn the_activity_manager_draws_forks_rows_and_header() {
    let (mut test, view, submitted) = launch();
    let stage = |path: &str| LocalWrite::StageFiles {
        paths: vec![path.into()],
    };
    let staged = started(&mut test, view, &submitted, &stage("a.rs"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: staged,
            ending: WriteEnding::Done(Done {
                description: "staged".to_owned(),
                acknowledged: None,
                locks_before: Vec::new(),
                locks_after: Vec::new(),
                replaced: None,
                cost_after: None,
            }),
            read_again: crate::worker::ReadAgain::Status,
            unspent: crate::worker::Unspent::none(),
        },
    );
    let refused = started(&mut test, view, &submitted, &stage("b.rs"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: refused,
            ending: failed_on_the_lock(),
            read_again: crate::worker::ReadAgain::Status,
            unspent: crate::worker::Unspent::none(),
        },
    );
    let commit = LocalWrite::Commit {
        message: "m".to_owned(),
        skip_hooks: false,
        files: 1,
    };
    let cancelled = started(&mut test, view, &submitted, &commit);
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: cancelled,
            ending: WriteEnding::Cancelled {
                locks: Vec::new(),
                ran: true,
            },
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        },
    );
    // A hook that wrote past the one tail's bound, still running.
    let flooding = started(&mut test, view, &submitted, &commit);
    let lines: Vec<String> = (0..crate::operations::output_tail::OUTPUT_LINES + 50)
        .map(|n| format!("hook line {n}"))
        .collect();
    output(&mut test, view, &submitted, flooding, &lines.join("\n"));

    open(&mut test);
    let shown = labels(&test);
    for text in [
        "staged",
        "stage failed",
        "cancelled",
        FAILED_GLYPH,
        CANCELLED_GLYPH,
    ] {
        assert!(
            shown.iter().any(|label| label == text),
            "{text:?}: {shown:?}"
        );
    }
    // The running entry, selected as the popover opens, opens at the end of its output so far,
    // as Fork's does: its newest line built.
    let newest = format!(
        "hook line {}",
        crate::operations::output_tail::OUTPUT_LINES + 49
    );
    assert!(drawn(&test, &newest), "{shown:?}");
    assert!(
        !drawn(&test, EARLIER_NOT_KEPT),
        "a running entry opened at its top"
    );
    let is_time = |text: &str| {
        let bytes = text.as_bytes();
        text.len() == 8
            && bytes[2] == b':'
            && bytes[5] == b':'
            && text.chars().filter(char::is_ascii_digit).count() == 6
    };
    let times = shown.iter().filter(|label| is_time(label)).count();
    assert_eq!(times, 4, "every row its start time: {shown:?}");
    // No status word in a row: the four words are the header's; a cancel's row reads Fork's
    // result line, "cancelled", once.
    for word in ["running", "succeeded", "failed"] {
        assert!(
            !shown.iter().any(|label| label == word),
            "a status word in a row: {shown:?}"
        );
    }
    assert_eq!(
        shown.iter().filter(|label| *label == "cancelled").count(),
        1,
        "{shown:?}"
    );

    assert!(
        shown
            .iter()
            .any(|label| label.strip_prefix("running ").is_some_and(is_time)),
        "the header's word and time: {shown:?}"
    );
    // The failed stage, selected: its word, time and duration, and its one sentence.
    click_lowest(&mut test, "stage failed");
    let shown = labels(&test);
    assert!(
        shown.iter().any(|label| label
            .strip_prefix("failed ")
            .and_then(|rest| rest.split_once(" · took "))
            .is_some_and(|(time, _)| is_time(time))),
        "{shown:?}"
    );
    assert!(drawn(&test, cairn_ui::GIT_ERROR_TEXT), "{shown:?}");
    // The flooding commit, finished and selected afresh, opens at its top: "Earlier output not
    // kept." first, its newest line far below.
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: flooding,
            ending: WriteEnding::Done(Done {
                description: "committed".to_owned(),
                acknowledged: None,
                locks_before: Vec::new(),
                locks_after: Vec::new(),
                replaced: None,
                cost_after: None,
            }),
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        },
    );
    let ended: Vec<String> = test.run_in(|| {
        view.activity
            .log
            .peek()
            .newest_first()
            .map(|entry| entry.name.clone())
            .collect()
    });
    assert_eq!(ended.first().map(String::as_str), Some("Commit 1 file"));
    let mut log = view.activity.log;
    test.run_in(|| {
        let key = log.peek().key_at(0);
        if let Some(key) = key {
            log.write().choose(key);
        }
    });
    settle(&mut test);
    assert!(drawn(&test, EARLIER_NOT_KEPT), "{:?}", labels(&test));
    assert!(!drawn(&test, &newest), "a finished entry opened at its end");
}

/// An amend run at the press that git made while its replaced commit was pushed says so in its
/// entry, once — and opens no dialog. Caught by: the engine's answer never drawn, or a dialog
/// opened for it.
#[test]
fn an_amend_pushed_while_it_ran_says_so_in_its_entry() {
    let (mut test, view, submitted) = launch();
    let id = started(
        &mut test,
        view,
        &submitted,
        &LocalWrite::AmendAtPress {
            message: "m".to_owned(),
            skip_hooks: false,
        },
    );
    let commit = Oid::from_bytes(&[0x3f; 20]).unwrap();
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::Done(Done {
                description: "amended".to_owned(),
                acknowledged: None,
                locks_before: Vec::new(),
                locks_after: Vec::new(),
                replaced: Some(commit),
                cost_after: Some(Box::new(Consequence::Amend {
                    commit,
                    published: Publication::Upstream(cairn_model::RefName::new(
                        "refs/remotes/origin/main",
                    )),
                    reflog: Reflog::Written,
                })),
            }),
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        },
    );
    assert!(!drawn(&test, GIT_ERROR_TITLE), "a dialog opened for it");
    assert!(view.confirming.peek().is_none(), "a dialog opened for it");
    open(&mut test);
    let said = "Amended 3f3f3f3, which was pushed to origin/main while the amend ran. Sharing \
                the amended commit needs a force push.";
    assert_eq!(
        labels(&test).iter().filter(|label| *label == said).count(),
        1,
        "{:?}",
        labels(&test)
    );
    assert!(drawn(&test, SHOW_IN_LOST_COMMITS_CAPTION));
}

/// The window's root reads whether the popover is open, never the log, and the commit box's
/// streamed output is kept apart from the state the root reads, so the commit box's own commit
/// streaming its hook's lines draws nothing again while the popover is closed — and, while it is
/// open, only the popover, which shows them. Caught by: the root subscribed to the log (`is_open`
/// read from it, or the fetch's entry read with `read`), or the commit's output written into the
/// box's state.
#[test]
fn a_streamed_line_never_draws_the_window_again() {
    let (mut test, view, submitted, drawn_root) = launch_counted();
    // The commit asked through the commit box, as its button asks it, so the box keeps its
    // output for a Git Error.
    let mut subject = view.local.commit.subject;
    test.run_in(|| subject.set("A commit past a running hook".to_owned()));
    let submit: std::rc::Rc<dyn Fn(Request)> = {
        let submitted = std::rc::Rc::clone(&submitted);
        std::rc::Rc::new(move |request| submitted.borrow_mut().push(request))
    };
    test.run_in(|| crate::commit_box::pane::pressed(view, Some(submit)));
    let id = submitted
        .borrow()
        .iter()
        .find_map(|request| match request {
            Request::Write { id, .. } => Some(*id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no commit asked: {:?}", submitted.borrow()));
    apply(&mut test, view, &submitted, Update::WriteStarted { id });
    assert!(
        view.local
            .commit
            .state
            .peek()
            .asked()
            .is_some_and(|asked| asked.id == id),
        "the commit box did not ask the commit"
    );
    let before = drawn_root.get();
    for n in 0..20 {
        output(&mut test, view, &submitted, id, &format!("hook line {n}"));
    }
    assert_eq!(
        drawn_root.get(),
        before,
        "a streamed line drew the window again with the popover closed"
    );
    open(&mut test);
    assert!(drawn(&test, "hook line 19"), "{:?}", labels(&test));
    let open_at = drawn_root.get();
    output(&mut test, view, &submitted, id, "hook line 20");
    assert!(
        drawn(&test, "hook line 20"),
        "the open popover missed a line"
    );
    assert_eq!(
        drawn_root.get(),
        open_at,
        "a streamed line drew the window again with the popover open"
    );
}

/// As Fork does it, a running entry opens at the end of its output so far and does not follow the
/// lines that arrive after. Caught by: a pane that opens at its top, or one that scrolls to each
/// new line.
#[test]
fn a_running_entry_opens_at_its_end_and_does_not_follow() {
    let (mut test, view, submitted) = launch();
    let id = started(
        &mut test,
        view,
        &submitted,
        &LocalWrite::Commit {
            message: "m".to_owned(),
            skip_hooks: false,
            files: 1,
        },
    );
    let lines = |range: std::ops::Range<usize>| {
        range
            .map(|n| format!("hook line {n}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    output(&mut test, view, &submitted, id, &lines(0..100));
    open(&mut test);
    assert!(drawn(&test, "hook line 99"), "{:?}", labels(&test));
    assert!(
        !drawn(&test, "hook line 0"),
        "a running entry opened at its top"
    );
    output(&mut test, view, &submitted, id, &lines(100..150));
    assert!(
        drawn(&test, "hook line 99"),
        "the pane moved as lines arrived: {:?}",
        labels(&test)
    );
    assert!(
        !drawn(&test, "hook line 149"),
        "the pane followed the lines that arrived"
    );
}
