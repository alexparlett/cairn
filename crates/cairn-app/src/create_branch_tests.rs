//! Create Branch in the window, headless (staging-and-commit R11.3; the user's decisions,
//! 2026-10-09): "New Branch…" on a commit row opens Fork's dialog, each name typed is asked of
//! the engine and the button follows its answer, "Check out after create" is sticky for the
//! session, "Local changes:" is drawn only over changes and starts on "Don't change", a discard
//! is counted and confirmed before its write, and a failure opens the Git Error dialog with the
//! name kept for the next opening. Updates are applied through `session::apply`, as the
//! worker's stream applies them, and requests read back from the window's submit.

use std::sync::Arc;

use cairn_model::{
    BranchName, ChangeLoss, ChangedKind, Consequence, GraphRow, HeadState, Lane, LostChange, Oid,
    PagedCommit, Ref, RefKind, RefName, RefTarget, RefsSnapshot, RepoPath, RowsPage, StagedChange,
    UnstagedChange,
};
use cairn_ui::accelerators::Action;
use cairn_ui::{
    CHECK_OUT_AFTER_CREATE, CREATE_AND_CHECKOUT_CAPTION, CREATE_BRANCH_TITLE, CREATE_CAPTION,
    DISCARD_LOCAL_CAPTION, GIT_ERROR_TITLE, LOCAL_CHANGES_LABEL, NEW_BRANCH_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;
use freya_testing::prelude::{MouseEventName, PlatformEvent};

use crate::local_changes_tests::{Submitted, apply, changed, labels, launch, status, untracked};
use crate::window::View;
use crate::window::tests::press_chord;
use crate::worker::{LocalWrite, OperationId, Request, Update, WriteEnding};

fn settle(test: &mut TestingRunner) {
    for _ in 0..4 {
        test.sync_and_update();
    }
}

fn oid(byte: u8) -> Oid {
    Oid::from_bytes(&[byte; 20]).unwrap_or_else(|e| panic!("{e}"))
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

fn click(test: &mut TestingRunner, text: &str) {
    let at = centre(test, text);
    test.click_cursor(at);
    settle(test);
}

/// The history holding one commit row, `0xab`'s, "Fix the parser".
fn with_a_commit(test: &mut TestingRunner, view: View, submitted: &Submitted) {
    let mut page = RowsPage::new();
    page.push(
        GraphRow::new(oid(0xab), Lane::new(0), Vec::new()),
        PagedCommit {
            parents: 1,
            subject: "Fix the parser",
            author: "Ada",
            author_time: 0,
        },
    );
    apply(
        test,
        view,
        submitted,
        Update::Rows {
            rows: page,
            complete: true,
        },
    );
}

/// Right-clicks the history's first row and chooses "New Branch…".
fn new_branch(test: &mut TestingRunner) {
    let (x, y) = centre(test, "Fix the parser");
    test.move_cursor((x, y));
    for name in [MouseEventName::MouseDown, MouseEventName::MouseUp] {
        test.send_event(PlatformEvent::Mouse {
            name,
            cursor: (x, y).into(),
            button: Some(MouseButton::Right),
        });
        test.sync_and_update();
    }
    settle(test);
    click(test, NEW_BRANCH_CAPTION);
}

fn type_name(test: &mut TestingRunner, view: View, name: &str) {
    let mut typed = view.branch.name;
    test.run_in(|| typed.set(name.to_owned()));
    settle(test);
}

fn writes(submitted: &Submitted) -> Vec<(OperationId, String)> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write { id, write } => Some((*id, write.what())),
            _ => None,
        })
        .collect()
}

/// The user's decision (2026-10-09): "New Branch…" opens Create Branch at the row's commit; a
/// name typed is asked of the engine, the button disabled until the engine says it is free and
/// for the text shown; a taken name is refused in Fork's words; a free one is created and the
/// dialog closes. Caught by: a create before the engine answered, an answer for another text
/// trusted, or the write naming another commit or name.
#[test]
fn new_branch_opens_the_dialog_and_creates_only_a_name_the_engine_said_is_free() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    new_branch(&mut test);
    assert!(drawn(&test, CREATE_BRANCH_TITLE));
    assert!(drawn(&test, "abababa Fix the parser"));

    type_name(&mut test, view, "test");
    assert!(
        submitted
            .borrow()
            .iter()
            .any(|r| matches!(r, Request::CheckBranchName { name } if name == "test")),
        "the name was not asked about"
    );
    click(&mut test, CREATE_CAPTION);
    assert!(
        writes(&submitted).is_empty(),
        "created before the engine answered"
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::BranchName {
            name: "test".to_owned(),
            outcome: Ok(BranchName::Taken),
        },
    );
    assert!(drawn(&test, "Branch test already exists"));
    click(&mut test, CREATE_CAPTION);
    assert!(writes(&submitted).is_empty(), "a taken name was created");

    type_name(&mut test, view, "topic");
    // An answer for the old text is not the new text's.
    apply(
        &mut test,
        view,
        &submitted,
        Update::BranchName {
            name: "test".to_owned(),
            outcome: Ok(BranchName::Free),
        },
    );
    click(&mut test, CREATE_CAPTION);
    assert!(
        writes(&submitted).is_empty(),
        "another text's answer was trusted"
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::BranchName {
            name: "topic".to_owned(),
            outcome: Ok(BranchName::Free),
        },
    );
    click(&mut test, CREATE_CAPTION);
    let asked: Vec<LocalWrite> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write { write, .. } => Some(write.clone()),
            _ => None,
        })
        .collect();
    assert!(
        matches!(asked.as_slice(), [LocalWrite::CreateBranch { name, at }] if name == "topic" && *at == oid(0xab)),
        "{asked:?}"
    );
    assert!(!drawn(&test, CREATE_BRANCH_TITLE), "the dialog stayed open");
}

/// Decision 1, and Fork's "Local changes:": the box is sticky for the session; the choices are
/// drawn only while it is ticked and the tree has changes, each opening on "Don't change"; the
/// button reads "Create and Checkout" while ticked, and asks the kept checkout. Caught by: the
/// box forgotten at the next opening, the choices drawn over a clean tree, or Discard
/// remembered.
#[test]
fn check_out_after_create_is_sticky_and_local_changes_appear_only_over_changes() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    apply(&mut test, view, &submitted, status(Vec::new()));
    new_branch(&mut test);
    click(&mut test, CHECK_OUT_AFTER_CREATE);
    assert!(drawn(&test, CREATE_AND_CHECKOUT_CAPTION));
    assert!(
        !drawn(&test, LOCAL_CHANGES_LABEL),
        "a clean tree has no local changes"
    );
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);

    apply(
        &mut test,
        view,
        &submitted,
        status(vec![changed("a.rs", None, Some(UnstagedChange::Modified))]),
    );
    new_branch(&mut test);
    assert!(
        drawn(&test, CREATE_AND_CHECKOUT_CAPTION),
        "the box was forgotten"
    );
    assert!(drawn(&test, LOCAL_CHANGES_LABEL));
    click(&mut test, DISCARD_LOCAL_CAPTION);
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    new_branch(&mut test);
    type_name(&mut test, view, "kept");
    apply(
        &mut test,
        view,
        &submitted,
        Update::BranchName {
            name: "kept".to_owned(),
            outcome: Ok(BranchName::Free),
        },
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    let asked: Vec<LocalWrite> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write { write, .. } => Some(write.clone()),
            _ => None,
        })
        .collect();
    assert!(
        matches!(asked.as_slice(), [LocalWrite::CreateBranchAndCheckout { name, .. }] if name == "kept"),
        "Discard was remembered, or the checkout not asked: {asked:?}"
    );
}

/// "Local changes:" over a tree of untracked files alone, and over one whose untracked file
/// sorts before its changed one by name (phase 10's QA, item 17). `git status` lists changed
/// and conflicted entries before untracked ones whatever their names, so the first entry
/// decides. Caught by: the choices drawn over untracked files alone (a checkout keeps them),
/// or the entries searched in path order (the untracked `a.txt` first).
#[test]
fn local_changes_are_offered_over_a_change_and_never_over_untracked_files_alone() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    click_check_out(
        &mut test,
        view,
        &submitted,
        vec![untracked("a.txt"), untracked("z.txt")],
    );
    assert!(drawn(&test, CREATE_AND_CHECKOUT_CAPTION));
    assert!(
        !drawn(&test, LOCAL_CHANGES_LABEL),
        "untracked files alone are not local changes a checkout touches"
    );
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);

    apply(
        &mut test,
        view,
        &submitted,
        status(vec![
            changed("b.rs", None, Some(UnstagedChange::Modified)),
            untracked("a.txt"),
        ]),
    );
    new_branch(&mut test);
    assert!(
        drawn(&test, LOCAL_CHANGES_LABEL),
        "a changed file was missed behind an untracked one that sorts first"
    );
}

/// Applies `entries` as the refreshed status, opens the dialog and ticks "Check out after
/// create".
fn click_check_out(
    test: &mut TestingRunner,
    view: View,
    submitted: &Submitted,
    entries: Vec<cairn_model::StatusEntry>,
) {
    apply(test, view, submitted, status(entries));
    new_branch(test);
    click(test, CHECK_OUT_AFTER_CREATE);
}

/// The last refresh's refs, `refs/heads/main` at `main` and `HEAD` as `head` says.
fn refs_read(test: &mut TestingRunner, view: View, head: HeadState, main: Oid) {
    let mut refreshed = view.refreshed;
    let _ = refreshed.write().refs_arrived(Arc::new(RefsSnapshot {
        refs: vec![Ref {
            name: RefName::new("refs/heads/main"),
            kind: RefKind::LocalBranch,
            target: RefTarget::Commit(main),
            symbolic: None,
            upstream: None,
        }],
        head,
        stashes: Vec::new(),
        unreadable: 0,
    }));
    settle(test);
}

/// Fork's New Branch chord (Ctrl+Shift+B, ⇧⌘B; phase 10's QA, item 19): Create Branch opens at
/// `HEAD`'s commit as the last refresh read it — a branch's or a detached `HEAD`'s — with the
/// loaded row's subject, or none where the row is not loaded; and the chord does nothing before
/// the refs are read, on an unborn `HEAD`, or while the dialog is open. Caught by: the chord
/// unheard, the dialog opened at another commit, or an open dialog reset by the chord (its
/// name lost).
#[test]
fn the_new_branch_chord_opens_create_branch_at_head() {
    let (mut test, view, submitted) = launch();
    // `HEAD`'s row, labelled `main`, as a walk from the refs lays it out.
    let mut page = RowsPage::new();
    page.push_labelled(
        GraphRow::new(oid(0xab), Lane::new(0), Vec::new()),
        PagedCommit {
            parents: 1,
            subject: "Fix the parser",
            author: "Ada",
            author_time: 0,
        },
        true,
        &[cairn_model::Label {
            name: "refs/heads/main",
            kind: RefKind::LocalBranch,
            current: true,
        }],
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::Rows {
            rows: page,
            complete: true,
        },
    );
    press_chord(&mut test, Action::NewBranch);
    settle(&mut test);
    assert!(
        !drawn(&test, CREATE_BRANCH_TITLE),
        "opened before the refs were read"
    );
    refs_read(
        &mut test,
        view,
        HeadState::Unborn(RefName::new("refs/heads/main")),
        oid(0xab),
    );
    press_chord(&mut test, Action::NewBranch);
    settle(&mut test);
    assert!(
        !drawn(&test, CREATE_BRANCH_TITLE),
        "opened on an unborn HEAD"
    );

    refs_read(
        &mut test,
        view,
        HeadState::Branch(RefName::new("refs/heads/main")),
        oid(0xab),
    );
    press_chord(&mut test, Action::NewBranch);
    settle(&mut test);
    assert!(drawn(&test, CREATE_BRANCH_TITLE), "the chord was not heard");
    assert!(
        drawn(&test, "abababa Fix the parser"),
        "not at HEAD's commit: {:?}",
        labels(&test)
    );
    type_name(&mut test, view, "topic");
    press_chord(&mut test, Action::NewBranch);
    settle(&mut test);
    assert_eq!(
        *view.branch.name.peek(),
        "topic",
        "the open dialog was reset"
    );
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);

    refs_read(&mut test, view, HeadState::Detached(oid(0xcd)), oid(0xab));
    press_chord(&mut test, Action::NewBranch);
    settle(&mut test);
    assert!(
        drawn(&test, "cdcdcdc "),
        "not at the detached HEAD: {:?}",
        labels(&test)
    );
}

/// The user's decision D (2026-10-09): while the name's check waits behind a running write on
/// the local lane, the dialog names that write beside its buttons by a plain noun ("Waiting for
/// the commit to finish…"), and says nothing more once
/// the answer for the text shown arrives, or while no write runs. Caught by: the wait left
/// unsaid, another write named, or the line kept after the answer.
#[test]
fn a_name_check_waiting_behind_a_write_says_which_write() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    new_branch(&mut test);
    type_name(&mut test, view, "topic");
    let waiting = |test: &TestingRunner| {
        labels(test)
            .into_iter()
            .filter(|label| label.starts_with("Waiting for"))
            .collect::<Vec<_>>()
    };
    assert_eq!(waiting(&test), Vec::<String>::new(), "no write runs");
    let id = OperationId::next();
    let mut writes = view.writes;
    test.run_in(|| {
        let mut writes = writes.write();
        writes.asked(
            id,
            &LocalWrite::CreateBranch {
                name: "first".to_owned(),
                at: oid(0xab),
            },
        );
        writes.started(id);
    });
    settle(&mut test);
    assert_eq!(
        waiting(&test),
        vec!["Waiting for the branch to be created…".to_owned()]
    );
    // Each write named by a plain noun, as the user's mockup read (decision D).
    for (write, line) in [
        (
            LocalWrite::Commit {
                message: "m".to_owned(),
                skip_hooks: false,
            },
            "Waiting for the commit to finish…",
        ),
        (
            LocalWrite::StageFiles {
                paths: vec![RepoPath::from("a.rs")],
            },
            "Waiting for staging to finish…",
        ),
        (
            LocalWrite::CreateBranchAndCheckout {
                name: "x".to_owned(),
                at: oid(0xab),
            },
            "Waiting for the checkout to finish…",
        ),
    ] {
        let next = OperationId::next();
        test.run_in(|| {
            let mut writes = writes.write();
            writes.asked(next, &write);
            writes.started(next);
        });
        settle(&mut test);
        assert_eq!(waiting(&test), vec![line.to_owned()]);
    }
    apply(
        &mut test,
        view,
        &submitted,
        Update::BranchName {
            name: "topic".to_owned(),
            outcome: Ok(BranchName::Free),
        },
    );
    assert_eq!(
        waiting(&test),
        Vec::<String>::new(),
        "the line outlived the answer"
    );
}

/// What Create Branch's discard would lose at `at` for `branch`.
fn discarding(branch: &str) -> Consequence {
    Consequence::CheckoutDiscarding {
        branch: branch.to_owned(),
        at: oid(0xab),
        head: Some(oid(1)),
        changes: vec![LostChange {
            path: RepoPath::from("a.rs"),
            loss: ChangeLoss::Changed {
                kind: ChangedKind::Modified,
                index: Some(oid(2)),
                working_tree: Some(oid(3)),
                executable: false,
                lines: Some(4),
            },
        }],
        kept_untracked: 0,
    }
}

/// Decision 3: Discard counts what it would lose first, and the engine's refusal is said in
/// the dialog; once counted the dialog closes and the confirmation opens on that consequence,
/// its button asking the discarding write with the token. Caught by: a discard written before
/// its confirmation, a refusal lost, or the token spent on another write.
#[test]
fn discard_is_counted_and_confirmed_before_its_write() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    apply(
        &mut test,
        view,
        &submitted,
        status(vec![changed("a.rs", Some(StagedChange::Modified), None)]),
    );
    new_branch(&mut test);
    click(&mut test, CHECK_OUT_AFTER_CREATE);
    click(&mut test, DISCARD_LOCAL_CAPTION);
    type_name(&mut test, view, "rescue");
    apply(
        &mut test,
        view,
        &submitted,
        Update::BranchName {
            name: "rescue".to_owned(),
            outcome: Ok(BranchName::Free),
        },
    );
    let counted = |submitted: &Submitted| -> Option<OperationId> {
        submitted
            .borrow()
            .iter()
            .rev()
            .find_map(|request| match request {
                Request::CheckoutConsequence { asked, name, at } => {
                    assert_eq!((name.as_str(), *at), ("rescue", oid(0xab)));
                    Some(*asked)
                }
                _ => None,
            })
    };
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    let asked = counted(&submitted).unwrap_or_else(|| panic!("the discard was not counted"));
    assert!(
        writes(&submitted).is_empty(),
        "written before its confirmation"
    );
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Err("a merge is in progress: continue or abort it first".to_owned()),
        },
    );
    assert!(drawn(
        &test,
        "a merge is in progress: continue or abort it first"
    ));
    assert!(
        drawn(&test, CREATE_BRANCH_TITLE),
        "the dialog closed on a refusal"
    );

    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    let asked = counted(&submitted).unwrap_or_else(|| panic!("the discard was not counted again"));
    let consequence = discarding("rescue");
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Ok(consequence.clone()),
        },
    );
    settle(&mut test);
    assert!(
        !drawn(&test, CREATE_BRANCH_TITLE),
        "the dialog stayed over the confirmation"
    );
    assert!(view.confirming.peek().is_some(), "no confirmation opened");
    click(&mut test, &consequence.action());
    let asked: Vec<String> = writes(&submitted)
        .into_iter()
        .map(|(_, what)| what)
        .collect();
    assert_eq!(asked, ["creating a branch, discarding changes"]);
}

/// Fork's Git Error dialog over git's words when a create fails, and the name kept for the
/// next opening (Fork: "Remember branch name if create branch failed"). Caught by: a failure
/// shown nowhere, or the name lost.
#[test]
fn a_failed_create_opens_the_git_error_and_keeps_its_name() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    new_branch(&mut test);
    type_name(&mut test, view, "topic");
    apply(
        &mut test,
        view,
        &submitted,
        Update::BranchName {
            name: "topic".to_owned(),
            outcome: Ok(BranchName::Free),
        },
    );
    click(&mut test, CREATE_CAPTION);
    let (id, _) = writes(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::Failed {
                message: "git failed".to_owned(),
                locks: Vec::new(),
                command: Some("git --literal-pathspecs branch -- topic abab".to_owned()),
                output: cairn_model::ScrubbedLines::scrubbing(
                    "fatal: a branch named 'topic' already exists\n",
                ),
            },
            read_again: crate::worker::ReadAgain::Everything,
        },
    );
    assert!(drawn(&test, GIT_ERROR_TITLE));
    assert!(drawn(&test, "fatal: a branch named 'topic' already exists"));
    click(&mut test, cairn_ui::CLOSE_CAPTION);
    assert!(!drawn(&test, GIT_ERROR_TITLE));
    new_branch(&mut test);
    assert_eq!(
        view.branch.name.peek().as_str(),
        "topic",
        "the name was not kept"
    );
}
