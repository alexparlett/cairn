//! Create Branch in the window, headless (staging-and-commit R11.3; the user's decisions of
//! 2026-10-09 and 2026-10-10): "New Branch…" on a commit row opens Fork's dialog, each name
//! typed is asked of the engine and the button follows its answer, "Check out after create" is
//! sticky for the session, "Local changes:" is drawn only over changes and starts on "Don't
//! change", Discard's press is its confirmation — the token built from the engine's consequence,
//! no second dialog — the dialog stays open under a failure's Git Error dialog, and the
//! window's keys are inert while it is up. Updates are applied through `session::apply`, as the
//! worker's stream applies them, and requests read back from the window's submit.

use std::sync::Arc;

use cairn_model::{
    BranchName, Consequence, GraphRow, HeadState, Lane, NameRefusal, Oid, OperationInProgress,
    PagedCommit, Ref, RefKind, RefName, RefTarget, RefsSnapshot, RepoPath, RowsPage, StagedChange,
    UnstagedChange,
};
use cairn_ui::accelerators::Action;
use cairn_ui::{
    CHECK_OUT_AFTER_CREATE, CREATE_AND_CHECKOUT_CAPTION, CREATE_BRANCH_TITLE, CREATE_CAPTION,
    DISCARD_LOCAL_CAPTION, DONT_CHANGE_CAPTION, GIT_ERROR_TITLE, LOCAL_CHANGES_LABEL,
    NEW_BRANCH_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;
use freya_testing::prelude::{MouseEventName, PlatformEvent};

use crate::local_changes_tests::{Submitted, apply, changed, labels, launch, status, untracked};
use crate::window::View;
use crate::window::tests::press_chord;
use crate::worker::{CheckoutRefused, Done, LocalWrite, OperationId, Request, Update, WriteEnding};

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
/// for the text shown; a taken name is refused in Fork's words; a free one is created, the
/// dialog staying open, its button disabled, until the write is done. Caught by: a create
/// before the engine answered, an answer for another text trusted, the write naming another
/// commit or name, a second press asking a second write, or the dialog left open once done.
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
            outcome: Ok(BranchName::Refused(NameRefusal::Taken)),
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
    assert!(
        drawn(&test, CREATE_BRANCH_TITLE),
        "closed before the write ended"
    );
    click(&mut test, CREATE_CAPTION);
    assert_eq!(
        writes(&submitted).len(),
        1,
        "a second write asked while the first ran"
    );
    let (id, _) = writes(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    apply(&mut test, view, &submitted, done(id));
    assert!(!drawn(&test, CREATE_BRANCH_TITLE), "the dialog stayed open");
}

/// The write `id` ended done.
fn done(id: OperationId) -> Update {
    Update::WriteEnded {
        id,
        ending: WriteEnding::Done(Done {
            description: "created".to_owned(),
            acknowledged: None,
            locks_before: Vec::new(),
            locks_after: Vec::new(),
        }),
        read_again: crate::worker::ReadAgain::Everything,
    }
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
/// is listed before its changed one (phase 10's QA, item 17): the status is asked by name
/// whether a tracked path has a change (`has_tracked_changes`), never read from its first
/// entry. Caught by: the choices drawn over untracked files alone (a checkout keeps them), or
/// a change missed behind an untracked entry listed first.
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
            untracked("a.txt"),
            changed("b.rs", None, Some(UnstagedChange::Modified)),
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

/// What Create Branch's Discard of `branch` at `0xab` is confirmed as.
fn discarding(branch: &str) -> Consequence {
    Consequence::CheckoutDiscarding {
        branch: branch.to_owned(),
        at: oid(0xab),
        head: Some(oid(1)),
    }
}

/// The asks of what Discard would be confirmed as, in order: the id and the name.
fn discard_asks(submitted: &Submitted) -> Vec<(OperationId, String)> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::CheckoutConsequence { asked, name, at } => {
                assert_eq!(*at, oid(0xab));
                Some((*asked, name.clone()))
            }
            _ => None,
        })
        .collect()
}

/// The discarding writes asked, each with the consequence its token carries.
fn discards_asked(submitted: &Submitted) -> Vec<Consequence> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write {
                write: LocalWrite::CreateBranchDiscarding(token),
                ..
            } => Some(token.consequence().clone()),
            _ => None,
        })
        .collect()
}

/// Opens the dialog over a staged change, ticks the box, chooses Discard and types `name`,
/// which the engine answers free.
fn discard_chosen(test: &mut TestingRunner, view: View, submitted: &Submitted, name: &str) {
    apply(
        test,
        view,
        submitted,
        status(vec![changed("a.rs", Some(StagedChange::Modified), None)]),
    );
    new_branch(test);
    click(test, CHECK_OUT_AFTER_CREATE);
    click(test, DISCARD_LOCAL_CAPTION);
    type_name(test, view, name);
    apply(
        test,
        view,
        submitted,
        Update::BranchName {
            name: name.to_owned(),
            outcome: Ok(BranchName::Free),
        },
    );
}

/// C34, B2 and the user's decision of 2026-10-10: with Discard chosen, what it would be
/// confirmed as is asked as the name changes; the button waits for the engine's answer to the
/// latest ask, for the name shown; then the press — or Return — IS the confirmation: the dialog builds the token
/// from that consequence and the discarding write is asked with it, no second dialog opening.
/// A press with "Don't change" builds none. Caught by: a token built before the engine answered
/// or from an older ask's answer, a confirmation dialog opened, Return not pressing, a token
/// built for "Don't change", or the token's consequence not the engine's.
#[test]
fn discards_press_is_its_confirmation_and_builds_the_token_from_the_engines_consequence() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    discard_chosen(&mut test, view, &submitted, "rescue");
    let asks = discard_asks(&submitted);
    let Some((asked, name)) = asks.last().cloned() else {
        panic!("what Discard would be confirmed as was not asked: {asks:?}");
    };
    assert_eq!(name, "rescue");
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    assert!(
        discards_asked(&submitted).is_empty() && writes(&submitted).is_empty(),
        "written before the engine answered"
    );
    // The name changes: what was asked for the old one is not this one's, even arriving late.
    type_name(&mut test, view, "rescue2");
    apply(
        &mut test,
        view,
        &submitted,
        Update::BranchName {
            name: "rescue2".to_owned(),
            outcome: Ok(BranchName::Free),
        },
    );
    let (newer, name) = discard_asks(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("the new name was not asked"));
    assert_eq!(name, "rescue2");
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Ok(discarding("rescue")),
        },
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    assert!(
        discards_asked(&submitted).is_empty(),
        "an older name's answer was trusted"
    );
    let asked = newer;
    let consequence = discarding("rescue2");
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Ok(consequence.clone()),
        },
    );
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    assert_eq!(
        discards_asked(&submitted),
        std::slice::from_ref(&consequence),
        "Return did not confirm"
    );
    assert!(view.confirming.peek().is_none(), "a second dialog opened");
    assert!(
        drawn(&test, CREATE_BRANCH_TITLE),
        "closed before the write ended"
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    assert_eq!(
        discards_asked(&submitted).len(),
        1,
        "a second token built while it ran"
    );

    // The button, in a fresh opening; and "Don't change" builds none.
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    discard_chosen(&mut test, view, &submitted, "rescue");
    let (asked, _) = discard_asks(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("not asked"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Ok(discarding("rescue")),
        },
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    assert_eq!(discards_asked(&submitted), [discarding("rescue")]);
    assert!(view.confirming.peek().is_none(), "a second dialog opened");

    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    discard_chosen(&mut test, view, &submitted, "kept");
    click(&mut test, DONT_CHANGE_CAPTION);
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    assert!(
        discards_asked(&submitted).is_empty(),
        "Don't change built a token"
    );
    assert!(
        matches!(
            writes(&submitted).as_slice(),
            [(_, what)] if what.contains("kept")
        ),
        "{:?}",
        writes(&submitted)
    );
}

/// The user's decision of 2026-10-10: an operation in progress refuses Discard in the dialog's
/// refusal row, "<Operation> is in progress. Finish or abort it first.", Create and Checkout
/// disabled while Discard is chosen — and enabled again for "Don't change". Caught by: the
/// refusal unsaid, a discard asked over a merge, or "Don't change" held back too.
#[test]
fn an_operation_in_progress_refuses_discard_in_the_dialog() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    discard_chosen(&mut test, view, &submitted, "rescue");
    let (asked, _) = discard_asks(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("not asked"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Err(CheckoutRefused::InProgress(OperationInProgress::Merge)),
        },
    );
    assert!(drawn(
        &test,
        "A merge is in progress. Finish or abort it first."
    ));
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    assert!(
        writes(&submitted).is_empty(),
        "a discard asked over a merge"
    );
    click(&mut test, DONT_CHANGE_CAPTION);
    assert!(!drawn(
        &test,
        "A merge is in progress. Finish or abort it first."
    ));
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    assert_eq!(writes(&submitted).len(), 1, "Don't change held back");
}

/// The write `id` ended in git's refusal to check out over a local change.
fn failed(id: OperationId) -> Update {
    Update::WriteEnded {
        id,
        ending: WriteEnding::Failed {
            message: "git failed".to_owned(),
            locks: Vec::new(),
            command: Some("git --literal-pathspecs checkout -q -b topic abab --".to_owned()),
            output: cairn_model::ScrubbedLines::scrubbing(
                "error: Your local changes to the following files would be overwritten by \
                 checkout:\n",
            ),
        },
        read_again: crate::worker::ReadAgain::Everything,
    }
}

/// The window's submit, for a test calling the dialog's handlers directly.
fn submitter(submitted: &Submitted) -> impl Fn(Request) + use<> {
    let submitted = submitted.clone();
    move |request| submitted.borrow_mut().push(request)
}

/// Phase 14's QA (TC#1): what Discard would be confirmed as is kept for the name it was asked
/// for, and is never the token for another name shown. Discard is answered for "rescue"; the
/// name becomes "rescue2", which the name check answers free before Discard's answer for it
/// arrives: the button is not ready and neither a press nor Return builds a token. Once
/// "rescue2"'s answer is in, the token names "rescue2". Caught by: the answer kept unkeyed (the
/// filter on its name dropped), a token for a branch the dialog no longer shows.
#[test]
fn an_older_names_discard_answer_never_builds_the_token() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    discard_chosen(&mut test, view, &submitted, "rescue");
    let (asked, _) = discard_asks(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("not asked"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Ok(discarding("rescue")),
        },
    );
    type_name(&mut test, view, "rescue2");
    apply(
        &mut test,
        view,
        &submitted,
        Update::BranchName {
            name: "rescue2".to_owned(),
            outcome: Ok(BranchName::Free),
        },
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    test.press_key(Key::Named(NamedKey::Enter));
    settle(&mut test);
    assert!(
        discards_asked(&submitted).is_empty(),
        "a token built from another name's answer: {:?}",
        discards_asked(&submitted)
    );
    let (newer, name) = discard_asks(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("the new name was not asked"));
    assert_eq!(name, "rescue2");
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked: newer,
            outcome: Ok(discarding("rescue2")),
        },
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    assert_eq!(discards_asked(&submitted), [discarding("rescue2")]);
}

/// Phase 14's QA (DO#3): the press's handler takes the token only while Discard is still chosen
/// and the token's consequence is the one asked for the name shown — a token built at the last
/// render, for another name or before the radio moved, is let go of. Caught by: a token spent
/// for a state the dialog no longer shows.
#[test]
fn a_token_for_another_name_or_choice_is_let_go_of() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    discard_chosen(&mut test, view, &submitted, "rescue");
    let (asked, _) = discard_asks(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("not asked"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Ok(discarding("rescue")),
        },
    );
    let submit = submitter(&submitted);
    // A token for a name not shown.
    test.run_in(|| {
        crate::create_branch::discard_confirmed(
            view,
            cairn_model::Confirmed::by_user(discarding("other")),
            Some(&submit),
        )
    });
    settle(&mut test);
    assert!(
        discards_asked(&submitted).is_empty(),
        "another name's token spent"
    );
    // "Don't change" chosen since the token was built.
    click(&mut test, DONT_CHANGE_CAPTION);
    test.run_in(|| {
        crate::create_branch::discard_confirmed(
            view,
            cairn_model::Confirmed::by_user(discarding("rescue")),
            Some(&submit),
        )
    });
    settle(&mut test);
    assert!(
        discards_asked(&submitted).is_empty() && writes(&submitted).is_empty(),
        "a token spent with Don't change chosen"
    );
    // The one for the state shown is taken.
    click(&mut test, DISCARD_LOCAL_CAPTION);
    let (asked, _) = discard_asks(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("not asked again"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Ok(discarding("rescue")),
        },
    );
    test.run_in(|| {
        crate::create_branch::discard_confirmed(
            view,
            cairn_model::Confirmed::by_user(discarding("rescue")),
            Some(&submit),
        )
    });
    settle(&mut test);
    assert_eq!(discards_asked(&submitted), [discarding("rescue")]);
}

/// Phase 14's QA (RS#1): while a create's Git Error is up over the dialog, nothing more is
/// asked — neither the create nor a discard — though the dialog beneath is ready again. Caught
/// by: a second forced checkout run under the Git Error, kept from it only by where focus sits.
#[test]
fn nothing_is_asked_while_a_creates_git_error_is_up() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    discard_chosen(&mut test, view, &submitted, "rescue");
    let (asked, _) = discard_asks(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("not asked"));
    apply(
        &mut test,
        view,
        &submitted,
        Update::CheckoutConsequence {
            asked,
            outcome: Ok(discarding("rescue")),
        },
    );
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    let (id, _) = writes(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    apply(&mut test, view, &submitted, failed(id));
    assert!(drawn(&test, GIT_ERROR_TITLE));
    let submit = submitter(&submitted);
    test.run_in(|| {
        crate::create_branch::discard_confirmed(
            view,
            cairn_model::Confirmed::by_user(discarding("rescue")),
            Some(&submit),
        )
    });
    settle(&mut test);
    assert_eq!(
        writes(&submitted).len(),
        1,
        "a discard asked under the Git Error"
    );
    click(&mut test, DONT_CHANGE_CAPTION);
    test.run_in(|| crate::create_branch::create(view, Some(&submit)));
    settle(&mut test);
    assert_eq!(
        writes(&submitted).len(),
        1,
        "a create asked under the Git Error"
    );
}

/// Phase 14's QA (TC#3): a press outside, or Escape, heard by the dialog beneath while a
/// create's Git Error is over it does not close it: Close returns to it as it was left. Caught
/// by: `cancel` taking the dialog away under the Git Error, its name and choices lost.
#[test]
fn a_cancel_under_the_git_error_leaves_the_dialog_as_it_was() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    discard_chosen(&mut test, view, &submitted, "rescue");
    click(&mut test, DONT_CHANGE_CAPTION);
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    let (id, _) = writes(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    apply(&mut test, view, &submitted, failed(id));
    test.run_in(|| crate::create_branch::cancel(view));
    settle(&mut test);
    click(&mut test, cairn_ui::CLOSE_CAPTION);
    assert!(
        drawn(&test, CREATE_BRANCH_TITLE),
        "the dialog went under the Git Error"
    );
    assert_eq!(view.branch.name.peek().as_str(), "rescue");
    assert!(drawn(&test, LOCAL_CHANGES_LABEL), "its choices were lost");
}

/// R11.3 and C34: a create that fails opens Fork's Git Error dialog over git's words, OVER the
/// dialog, which stays open beneath it as it was left — its name, its box and its choice — so
/// Close returns to it; Escape closes the Git Error alone. Caught by: a failure shown nowhere,
/// the dialog closed under it, its state reset, or Escape closing both.
#[test]
fn a_failed_create_opens_the_git_error_over_the_dialog_left_as_it_was() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    apply(
        &mut test,
        view,
        &submitted,
        status(vec![changed("a.rs", None, Some(UnstagedChange::Modified))]),
    );
    new_branch(&mut test);
    click(&mut test, CHECK_OUT_AFTER_CREATE);
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
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    let (id, _) = writes(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked"));
    apply(&mut test, view, &submitted, failed(id));
    assert!(drawn(&test, GIT_ERROR_TITLE));
    assert!(drawn(
        &test,
        "error: Your local changes to the following files would be overwritten by checkout:"
    ));
    assert!(
        drawn(&test, CREATE_BRANCH_TITLE),
        "the dialog closed under the error"
    );
    click(&mut test, cairn_ui::CLOSE_CAPTION);
    assert!(!drawn(&test, GIT_ERROR_TITLE));
    assert!(
        drawn(&test, CREATE_BRANCH_TITLE),
        "Close did not return to the dialog"
    );
    assert_eq!(
        view.branch.name.peek().as_str(),
        "topic",
        "the name was reset"
    );
    assert!(
        drawn(&test, CREATE_AND_CHECKOUT_CAPTION),
        "the box was reset"
    );
    assert!(drawn(&test, LOCAL_CHANGES_LABEL));

    // Again, closed by Escape: the Git Error alone goes.
    click(&mut test, CREATE_AND_CHECKOUT_CAPTION);
    let (id, _) = writes(&submitted)
        .pop()
        .unwrap_or_else(|| panic!("nothing asked again"));
    apply(&mut test, view, &submitted, failed(id));
    assert!(drawn(&test, GIT_ERROR_TITLE));
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    assert!(!drawn(&test, GIT_ERROR_TITLE), "Escape left the Git Error");
    assert!(
        drawn(&test, CREATE_BRANCH_TITLE),
        "Escape closed the dialog beneath"
    );
}

/// R11.3, C28 and the review's M5: while Create Branch is open — and while a create's Git Error
/// is up, the dialog gone — the window's keys are inert: a window chord does nothing. Caught by:
/// the dialog, or its Git Error, left out of `keys_inert`.
#[test]
fn the_windows_keys_are_inert_while_create_branch_is_up() {
    let (mut test, view, submitted) = launch();
    with_a_commit(&mut test, view, &submitted);
    let refreshes = |submitted: &Submitted| {
        submitted
            .borrow()
            .iter()
            .filter(|request| matches!(request, Request::Refresh))
            .count()
    };
    press_chord(&mut test, Action::Refresh);
    settle(&mut test);
    let heard = refreshes(&submitted);
    assert!(heard > 0, "the chord is not heard at all");
    new_branch(&mut test);
    assert!(crate::shortcuts::keys_inert(view));
    press_chord(&mut test, Action::Refresh);
    settle(&mut test);
    assert_eq!(
        refreshes(&submitted),
        heard,
        "a chord acted under the dialog"
    );

    // A create's Git Error alone (phase 14's QA, TC#2): the dialog cancelled while its write ran,
    // the write then failing.
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
    test.run_in(|| crate::create_branch::cancel(view));
    settle(&mut test);
    assert!(!drawn(&test, CREATE_BRANCH_TITLE));
    apply(&mut test, view, &submitted, failed(id));
    assert!(drawn(&test, GIT_ERROR_TITLE));
    assert!(crate::shortcuts::keys_inert(view));
    // The write's ending asks its own read again; counted from here.
    let heard = refreshes(&submitted);
    press_chord(&mut test, Action::Refresh);
    settle(&mut test);
    assert_eq!(
        refreshes(&submitted),
        heard,
        "a chord acted under a create's Git Error"
    );
}
