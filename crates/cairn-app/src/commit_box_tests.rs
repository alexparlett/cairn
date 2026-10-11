//! The commit box in the window, headless (staging-and-commit R10; criteria C13, C14 and C24's
//! view halves): the draft kept through Amend's toggling, a failed hook and its skip, the
//! amend's button and its token naming one `HEAD`, the dialog asked first exactly when a remote
//! has it or git keeps no reflog, a commit running drawn with its Cancel, a merge's message and the operations
//! that disable the box, amend's staged list drawn and unstaged from, and the recent messages.
//! Updates are applied through `session::apply`, as the worker's stream applies them, and
//! requests read back from the window's submit.

use std::sync::Arc;

use cairn_model::{
    ChangeStatus, ChangedFile, Consequence, FileMode, HeadState, LocalChanges, Oid,
    OperationInProgress, Publication, RefName, Reflog, RefsSnapshot, RepoPath, ScrubbedLines,
    StagedChange, StatusEntry, UnstagedChange, WorkingTreeStatus,
};
use cairn_ui::{
    AMEND_BUTTON_CAPTION, AMEND_CAPTION, CANCEL_COMMIT_CAPTION, GIT_ERROR_TITLE,
    SKIP_HOOKS_CAPTION, UNSTAGE_CAPTION,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

use crate::local_changes_tests::{
    Submitted, apply, asked, changed, labels, launch, open_local_changes, press_row, status,
};
use crate::window::View;
use crate::worker::{
    AmendRead, CommitReads, Done, LocalWrite, OperationId, Request, UnstageTarget, Update,
    WorkingSide, WriteEnding,
};

fn settle(test: &mut TestingRunner) {
    for _ in 0..4 {
        test.sync_and_update();
    }
}

fn oid(byte: u8) -> Oid {
    Oid::from_bytes(&[byte; 20]).unwrap_or_else(|e| panic!("{e}"))
}

/// `HEAD` on `main`, or on an unborn `main`.
fn refs(born: bool) -> Update {
    let main = RefName::new("refs/heads/main");
    Update::Refs {
        snapshot: Arc::new(RefsSnapshot {
            refs: Vec::new(),
            head: if born {
                HeadState::Branch(main)
            } else {
                HeadState::Unborn(main)
            },
            stashes: Vec::new(),
            unreadable: 0,
        }),
        reopen: false,
    }
}

/// Unstaged: a.rs; Staged: s.rs.
fn entries() -> Vec<StatusEntry> {
    vec![
        changed("a.rs", None, Some(UnstagedChange::Modified)),
        changed("s.rs", Some(StagedChange::Modified), None),
    ]
}

/// What the box reads: the operation in progress, the message git prepared for it (cleaned),
/// and the recent messages.
fn reads(operation: Option<OperationInProgress>, message: Option<&str>, recent: &[&str]) -> Update {
    Update::CommitReads(Box::new(CommitReads {
        operation,
        message: Ok(message.map(str::to_owned)),
        recent: Ok(recent.iter().map(|message| (*message).to_owned()).collect()),
    }))
}

/// Local Changes opened over `entries`, `HEAD` born or not, and the box's reads answered.
fn opened_with(entries: Vec<StatusEntry>, born: bool) -> (TestingRunner, View, Submitted) {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(entries));
    apply(&mut test, view, &submitted, refs(born));
    open_local_changes(&mut test);
    apply(&mut test, view, &submitted, reads(None, None, &[]));
    settle(&mut test);
    (test, view, submitted)
}

fn opened() -> (TestingRunner, View, Submitted) {
    opened_with(entries(), true)
}

/// Where the label reading `text` is drawn: right of the lists — in the box — first, else
/// anywhere (a dialog over the window).
fn at(test: &TestingRunner, text: &str) -> (f64, f64) {
    let reading = |outside_lists: bool| -> Option<(f32, f32)> {
        test.find(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == text)
                .filter(|_| {
                    !outside_lists
                        || !crate::local_changes_tests::in_lists(node.layout().area.min_x())
                })
                .map(|_| {
                    let area = node.layout().area;
                    (area.center().x, area.center().y)
                })
        })
    };
    let (x, y) = reading(true)
        .or_else(|| reading(false))
        .unwrap_or_else(|| panic!("nothing reads {text:?}: {:?}", labels(test)));
    (f64::from(x), f64::from(y))
}

fn click(test: &mut TestingRunner, text: &str) {
    let at = at(test, text);
    test.click_cursor(at);
    settle(test);
}

/// Clicks the last label reading `text`: a dialog's button, drawn over the window after the box
/// whose Amend toggle reads the same.
fn click_last(test: &mut TestingRunner, text: &str) {
    let centres = test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|_| node.layout().area.center())
    });
    let Some(centre) = centres.last() else {
        panic!("nothing reads {text:?}: {:?}", labels(test));
    };
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    settle(test);
}

fn drawn(test: &TestingRunner, text: &str) -> bool {
    labels(test).iter().any(|label| label == text)
}

fn set(test: &mut TestingRunner, mut state: State<String>, text: &str) {
    test.run_in(|| state.set(text.to_owned()));
    settle(test);
}

fn draft(view: View) -> (String, String) {
    (
        view.local.commit.subject.peek().clone(),
        view.local.commit.description.peek().clone(),
    )
}

/// The writes asked, oldest first.
fn writes(submitted: &Submitted) -> Vec<(OperationId, String)> {
    submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::Write { id, write } => Some((*id, describe(write))),
            _ => None,
        })
        .collect()
}

fn describe(write: &LocalWrite) -> String {
    match write {
        LocalWrite::Commit {
            message,
            skip_hooks,
        } => format!("commit {message:?} skip={skip_hooks}"),
        LocalWrite::Amend {
            confirmed,
            message,
            skip_hooks,
        } => format!(
            "amend {message:?} skip={skip_hooks} confirming {:?}",
            confirmed.prompt()
        ),
        LocalWrite::AmendAtPress {
            message,
            skip_hooks,
        } => format!("amend at the press {message:?} skip={skip_hooks}"),
        LocalWrite::UnstageFiles { paths, to } => format!(
            "unstage {:?} to {to:?}",
            paths.iter().map(ToString::to_string).collect::<Vec<_>>()
        ),
        other => format!("{other:?}"),
    }
}

fn count(submitted: &Submitted, wanted: impl Fn(&Request) -> bool) -> usize {
    submitted.borrow().iter().filter(|r| wanted(r)).count()
}

fn amend_consequence(published: Publication) -> Consequence {
    amend_consequence_of(0xab, published, Reflog::Written)
}

/// An amend of the commit whose id is `commit` repeated.
fn amend_consequence_of(commit: u8, published: Publication, reflog: Reflog) -> Consequence {
    Consequence::Amend {
        commit: oid(commit),
        published,
        reflog,
    }
}

/// What the worker answers for amend's read over the status the window asked it for last:
/// `HEAD`'s message, and amend's lists over `entries` — s.rs and h.rs, a file `HEAD` added, in
/// Staged — against `parent`.
fn amend_answer(submitted: &Submitted, parent: Option<Oid>) -> Update {
    amend_read(submitted, Ok(parent))
}

/// [`amend_answer`], or amend's staged list unread for `why`.
fn amend_read(submitted: &Submitted, parent: Result<Option<Oid>, &str>) -> Update {
    let file = |status, path: &str| ChangedFile {
        status,
        old_path: RepoPath::from(path),
        new_path: RepoPath::from(path),
        old_mode: Some(FileMode::Regular),
        new_mode: Some(FileMode::Regular),
        old_id: None,
        new_id: None,
    };
    let status = submitted
        .borrow()
        .iter()
        .rev()
        .find_map(|request| match request {
            Request::Amending { status } => Some(Arc::clone(status)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("amend's lists were never asked"));
    let lists = parent
        .map(|parent| {
            Arc::new(LocalChanges::amending(
                WorkingTreeStatus::Listed(entries()),
                vec![
                    file(ChangeStatus::Added, "h.rs"),
                    file(ChangeStatus::Modified, "s.rs"),
                ],
                parent,
            ))
        })
        .map_err(str::to_owned);
    Update::Amending {
        status,
        read: Box::new(AmendRead {
            message: Ok("Fix the parser\n\nIt read past the end.\n".to_owned()),
            lists,
        }),
    }
}

/// Where the field showing `text` (its value, or its placeholder while empty) is drawn.
fn field_at(test: &TestingRunner, text: &str) -> (f64, f64) {
    let found: Option<(f32, f32)> = test.find(|node, element| {
        Paragraph::try_downcast(element)
            .filter(|paragraph| {
                paragraph
                    .spans
                    .iter()
                    .map(|span| span.text.as_ref())
                    .collect::<String>()
                    == text
            })
            .map(|_| {
                let area = node.layout().area;
                (area.min_x() + 4., area.center().y)
            })
    });
    let (x, y) = found.unwrap_or_else(|| panic!("no field shows {text:?}"));
    (f64::from(x), f64::from(y))
}

fn type_into(test: &mut TestingRunner, showing: &str, typed: &str) {
    let at = field_at(test, showing);
    test.click_cursor(at);
    settle(test);
    test.write_text(typed);
    settle(test);
}

/// R10.3, R10.7, C7 and the QA brief's first case: a draft typed in both fields, Amend ticked
/// and unticked, is back exactly as typed — the amend's read, arriving meanwhile, never writing
/// over it, even emptied, since the person touched it; an empty, untouched draft is filled with
/// `HEAD`'s message, which unticking takes away again; and a fill the person edited and cleared
/// stays cleared through the next read. Each tick asks amend's lists over the status drawn, and
/// each untick draws the status's own lists again. Caught by: the draft lost to `HEAD`'s
/// message, a draft not restored, a cleared fill filled again, or amend's lists left drawn.
#[test]
fn a_draft_typed_then_amend_ticked_and_unticked_is_back_exactly() {
    let (mut test, view, submitted) = opened();
    type_into(&mut test, cairn_ui::SUBJECT_PLACEHOLDER, "WIP: the parser");
    // Tab moves from the subject to the description, as Fork's does.
    test.press_key(Key::Named(NamedKey::Tab));
    settle(&mut test);
    test.write_text("half done");
    settle(&mut test);
    let typed = draft(view);
    assert_eq!(
        typed,
        ("WIP: the parser".to_owned(), "half done".to_owned())
    );
    assert!(
        view.local.commit.state.peek().is_touched(),
        "typing left the draft untouched"
    );

    click(&mut test, AMEND_CAPTION);
    assert_eq!(
        count(&submitted, |r| matches!(r, Request::Amending { .. })),
        1,
        "ticking Amend asked nothing"
    );
    apply(
        &mut test,
        view,
        &submitted,
        amend_answer(&submitted, Some(oid(0x0c))),
    );
    assert_eq!(draft(view), typed, "the amend's read wrote over the draft");
    assert!(
        crate::local_changes_tests::in_lists_reads(&test, "h.rs"),
        "amend's staged list is not drawn"
    );
    click(&mut test, AMEND_CAPTION);
    assert_eq!(draft(view), typed);
    assert!(
        !crate::local_changes_tests::in_lists_reads(&test, "h.rs"),
        "amend's lists outlived Amend"
    );

    // Typed, then emptied: the person's empty draft, never filled.
    set(&mut test, view.local.commit.subject, "");
    set(&mut test, view.local.commit.description, "");
    click(&mut test, AMEND_CAPTION);
    apply(
        &mut test,
        view,
        &submitted,
        amend_answer(&submitted, Some(oid(0x0c))),
    );
    assert_eq!(
        draft(view),
        (String::new(), String::new()),
        "a touched draft was filled"
    );

    // An untouched empty draft: `HEAD`'s message fills it, and unticking empties it again.
    let (mut test, view, submitted) = opened();
    click(&mut test, AMEND_CAPTION);
    apply(
        &mut test,
        view,
        &submitted,
        amend_answer(&submitted, Some(oid(0x0c))),
    );
    let filled = (
        "Fix the parser".to_owned(),
        "It read past the end.\n".to_owned(),
    );
    assert_eq!(draft(view), filled);
    click(&mut test, AMEND_CAPTION);
    assert_eq!(draft(view), (String::new(), String::new()));

    // Filled, then edited and cleared: the next read leaves it cleared.
    click(&mut test, AMEND_CAPTION);
    apply(
        &mut test,
        view,
        &submitted,
        amend_answer(&submitted, Some(oid(0x0c))),
    );
    type_into(&mut test, "Fix the parser", "!");
    set(&mut test, view.local.commit.subject, "");
    set(&mut test, view.local.commit.description, "");
    apply(&mut test, view, &submitted, status(entries()));
    apply(
        &mut test,
        view,
        &submitted,
        amend_answer(&submitted, Some(oid(0x0c))),
    );
    assert_eq!(
        draft(view),
        (String::new(), String::new()),
        "a cleared fill was filled"
    );
}

/// R10.6, C1, C6 and C14's view half, and the QA brief's third case: Amend ticked, the one
/// button reads Fork's "Amend Last Commit", with no line under it, and is ready with a subject
/// even with nothing new staged; its press, or the commit chord in either field, asks the amend at
/// the press once — no token, no dialog; drawn amending while it runs; and made, the draft it
/// took is cleared and Amend unticks itself. Caught by: a token built in the box, a dialog for a
/// recoverable amend, the chord doing other than the button, or two amends for two presses.
#[test]
fn amend_last_commit_asks_the_amend_at_the_press_by_button_and_chord() {
    for by_chord in [false, true] {
        let (mut test, view, submitted) = opened_with(
            vec![changed("a.rs", None, Some(UnstagedChange::Modified))],
            true,
        );
        set(&mut test, view.local.commit.subject, "Fix the parser");
        click(&mut test, AMEND_CAPTION);
        assert!(drawn(&test, AMEND_BUTTON_CAPTION), "{:?}", labels(&test));
        if by_chord {
            press_commit_chord(&mut test, "Fix the parser");
            press_commit_chord(&mut test, "Fix the parser");
        } else {
            click(&mut test, AMEND_BUTTON_CAPTION);
            assert!(
                !drawn(&test, AMEND_BUTTON_CAPTION),
                "the button outlived its press"
            );
        }
        let asked = writes(&submitted);
        assert_eq!(
            asked.iter().map(|(_, w)| w.as_str()).collect::<Vec<_>>(),
            ["amend at the press \"Fix the parser\" skip=false"],
            "by chord: {by_chord}"
        );
        assert!(
            view.confirming.peek().is_none(),
            "a recoverable amend asked a dialog"
        );
        let id = asked[0].0;
        apply(&mut test, view, &submitted, Update::WriteStarted { id });
        assert!(
            labels(&test).iter().any(|l| l.starts_with("Amending 0:0")),
            "{:?}",
            labels(&test)
        );
        apply(&mut test, view, &submitted, done(id));
        assert_eq!(draft(view), (String::new(), String::new()));
        assert!(
            !view.local.commit.state.peek().is_amending(),
            "Amend stayed ticked"
        );
    }
}

/// R10.6 and C14: an amend at the press that must be confirmed — a remote has `HEAD`, or git
/// keeps no reflog — ends asking, and the confirmation dialog "Amend Commit" opens in the fixed
/// sentences; Cancel returns to the box unchanged — the draft, Amend ticked, the button ready —
/// and the dialog's Amend asks the confirmed amend of the pressed message under its token.
/// Caught by: a dialog for neither reason, a Cancel that loses the draft or amends, or a token
/// recording other words.
#[test]
fn an_amend_that_must_be_confirmed_opens_the_dialog_and_cancel_returns_unchanged() {
    for consequence in [
        amend_consequence(Publication::Upstream(RefName::new(
            "refs/remotes/origin/main",
        ))),
        amend_consequence_of(0xab, Publication::Unpublished, Reflog::NotWritten),
    ] {
        let (mut test, view, submitted) = opened();
        set(&mut test, view.local.commit.subject, "Fix the parser");
        click(&mut test, AMEND_CAPTION);
        let needs = |id| Update::WriteEnded {
            id,
            ending: WriteEnding::NeedsConfirming {
                consequence: consequence.clone(),
            },
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        };
        click(&mut test, AMEND_BUTTON_CAPTION);
        let first = writes(&submitted)[0].0;
        apply(&mut test, view, &submitted, needs(first));
        let prompt = consequence.prompt();
        assert!(drawn(&test, cairn_ui::AMEND_TITLE), "{:?}", labels(&test));
        assert!(
            drawn(&test, &prompt),
            "the dialog's words: {:?}",
            labels(&test)
        );
        click_last(&mut test, "Cancel");
        assert!(view.confirming.peek().is_none());
        assert_eq!(draft(view).0, "Fix the parser");
        assert!(view.local.commit.state.peek().is_amending());
        assert_eq!(writes(&submitted).len(), 1, "Cancel asked a write");

        // Asked again, by the chord this time, and confirmed.
        press_commit_chord(&mut test, "Fix the parser");
        let second = writes(&submitted)[1].0;
        apply(&mut test, view, &submitted, needs(second));
        click_last(&mut test, "Amend");
        let asked = writes(&submitted);
        assert_eq!(asked.len(), 3);
        assert_eq!(
            asked[2].1,
            format!("amend \"Fix the parser\" skip=false confirming {prompt:?}")
        );
    }
}

/// Focuses the field showing `showing` and presses the commit chord there.
fn press_commit_chord(test: &mut TestingRunner, showing: &str) {
    let at = field_at(test, showing);
    test.click_cursor(at);
    settle(test);
    crate::window::tests::press_chord(test, cairn_ui::accelerators::Action::Commit);
    settle(test);
}

/// QA item 5: the commit chord does nothing the button could not — not during a rebase, not
/// with an empty subject, amending or not: no write and no dialog. Caught by: a chord that
/// ignores the box's readiness.
#[test]
fn the_commit_chord_asks_nothing_while_the_box_is_not_ready() {
    // A rebase in progress.
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Something");
    apply(
        &mut test,
        view,
        &submitted,
        reads(Some(OperationInProgress::Rebase), None, &[]),
    );
    test.click_cursor(field_at(&test, "Something"));
    settle(&mut test);
    crate::window::tests::press_chord(&mut test, cairn_ui::accelerators::Action::Commit);
    settle(&mut test);
    assert!(writes(&submitted).is_empty(), "a commit during a rebase");

    // An empty subject, committing and amending.
    for amending in [false, true] {
        let (mut test, view, submitted) = opened();
        if amending {
            click(&mut test, AMEND_CAPTION);
        }
        set(&mut test, view.local.commit.description, "a body alone");
        press_commit_chord(&mut test, "a body alone");
        assert!(
            writes(&submitted).is_empty(),
            "amending {amending}: no subject"
        );
        assert!(view.confirming.peek().is_none());
    }
}

/// R10.8 and C8: on a detached `HEAD` one line says the commit will be on no branch, and the
/// commit is asked with no dialog. Caught by: a detached commit asking, or the line missing.
#[test]
fn a_detached_head_commits_under_one_line() {
    let (mut test, view, submitted) = opened();
    apply(
        &mut test,
        view,
        &submitted,
        Update::Refs {
            snapshot: Arc::new(RefsSnapshot {
                refs: Vec::new(),
                head: HeadState::Detached(oid(0xab)),
                stashes: Vec::new(),
                unreadable: 0,
            }),
            reopen: false,
        },
    );
    assert!(drawn(&test, cairn_ui::DETACHED_LINE), "{:?}", labels(&test));
    set(&mut test, view.local.commit.subject, "Detached");
    click(&mut test, "Commit 1 File");
    assert_eq!(writes(&submitted).len(), 1);
    assert!(view.confirming.peek().is_none());
}

/// A commit's failure as the local lane hands it on: git's output as the engine kept it, and the
/// message the engine's text of the error (`Error::shown`), both scrubbed there.
fn failed(id: OperationId, command: &str, output: &str) -> Update {
    let output = ScrubbedLines::scrubbing(output);
    Update::WriteEnded {
        id,
        ending: WriteEnding::Failed {
            message: format!("{command} failed (exit status: 1): {output}"),
            locks: Vec::new(),
            command: Some(command.to_owned()),
            output,
        },
        read_again: crate::worker::ReadAgain::Everything,
        unspent: crate::worker::Unspent::none(),
    }
}

fn done(id: OperationId) -> Update {
    Update::WriteEnded {
        id,
        ending: WriteEnding::Done(Done {
            description: "commit".to_owned(),
            acknowledged: None,
            locks_before: Vec::new(),
            locks_after: Vec::new(),
            replaced: None,
            cost_after: None,
        }),
        read_again: crate::worker::ReadAgain::Everything,
        unspent: crate::worker::Unspent::none(),
    }
}

/// R10.5, R10.7, phase 05's QA item 8 and the QA brief's second case: a commit a `pre-commit`
/// hook fails opens the Git Error dialog — the command, and the hook's output as it streamed,
/// ANSI stripped — the draft kept; the skip commits the same message with `--no-verify` for
/// that one commit; and the commit after it runs its hooks again. Caught by: the skip kept for
/// later commits, a message other than the draft's, or the draft lost to the failure.
#[test]
fn a_failed_hooks_skip_commits_once_without_hooks_and_the_next_runs_them() {
    let (mut test, view, submitted) = opened();
    apply(&mut test, view, &submitted, reads(None, None, &[]));
    set(&mut test, view.local.commit.subject, "Fix the parser");
    set(&mut test, view.local.commit.description, "Twice.");
    click(&mut test, "Commit 1 File");
    let asked = writes(&submitted);
    assert_eq!(
        asked.iter().map(|(_, w)| w.as_str()).collect::<Vec<_>>(),
        ["commit \"Fix the parser\\n\\nTwice.\" skip=false"]
    );
    let id = asked[0].0;
    apply(&mut test, view, &submitted, Update::WriteStarted { id });
    for line in ["\u{1b}[31mlint:\u{1b}[0m a.rs:3 unused", "hook failed"] {
        apply(
            &mut test,
            view,
            &submitted,
            Update::WriteOutput {
                id,
                lines: ScrubbedLines::scrubbing(line),
                receipt: crate::worker::OutputReceipt::none(),
            },
        );
    }
    apply(
        &mut test,
        view,
        &submitted,
        failed(id, "git commit -q -F -", "hook failed"),
    );
    for text in [
        GIT_ERROR_TITLE,
        "git commit -q -F -",
        "lint: a.rs:3 unused",
        "hook failed",
        SKIP_HOOKS_CAPTION,
    ] {
        assert!(drawn(&test, text), "{text:?} missing: {:?}", labels(&test));
    }
    assert_eq!(
        draft(view),
        ("Fix the parser".to_owned(), "Twice.".to_owned()),
        "the failure took the draft"
    );
    click(&mut test, SKIP_HOOKS_CAPTION);
    assert!(!drawn(&test, GIT_ERROR_TITLE));
    let asked = writes(&submitted);
    assert_eq!(asked.len(), 2);
    assert_eq!(
        asked[1].1,
        "commit \"Fix the parser\\n\\nTwice.\" skip=true"
    );
    let skipped = asked[1].0;
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteStarted { id: skipped },
    );
    apply(&mut test, view, &submitted, done(skipped));
    assert_eq!(
        draft(view),
        (String::new(), String::new()),
        "a commit made kept its draft"
    );

    set(&mut test, view.local.commit.subject, "Next");
    click(&mut test, "Commit 1 File");
    let asked = writes(&submitted);
    assert_eq!(asked.len(), 3);
    assert_eq!(
        asked[2].1, "commit \"Next\" skip=false",
        "the skip outlived its commit"
    );
}

/// R12.2 and phase 09's QA item 11, at the dialog: a hook that prints a remote URL with a token
/// — in the reads streamed and in the output the engine kept, each as the engine hands it on,
/// scrubbed as its runner split it (R4.10; a URL split over two reads is the engine's to join,
/// `process::registry`'s `git_output_is_read_once_as_whole_scrubbed_lines_on_both_streams`) —
/// opens the Git Error dialog with the URLs drawn and no token anywhere in it. Caught by: the
/// dialog drawing other than what the type holds.
#[test]
fn the_git_error_draws_no_token_a_hook_printed() {
    for streamed in [true, false] {
        let (mut test, view, submitted) = opened();
        set(&mut test, view.local.commit.subject, "Push it");
        click(&mut test, "Commit 1 File");
        let id = writes(&submitted)[0].0;
        apply(&mut test, view, &submitted, Update::WriteStarted { id });
        let kept = "fatal: https://deploy:ghp_SECRET1@example.com/r.git denied\n\
                    retry at https://deploy:ghp_SECRET2@example.com/again";
        if streamed {
            for read in [
                ScrubbedLines::scrubbing(
                    "fatal: https://deploy:ghp_SECRET1@example.com/r.git denied",
                ),
                ScrubbedLines::scrubbing("retry at https://deploy:ghp_SECRET2@example.com/again"),
            ] {
                apply(
                    &mut test,
                    view,
                    &submitted,
                    Update::WriteOutput {
                        id,
                        lines: read,
                        receipt: crate::worker::OutputReceipt::none(),
                    },
                );
            }
        }
        apply(
            &mut test,
            view,
            &submitted,
            failed(id, "git commit -q -F -", kept),
        );
        assert!(drawn(&test, GIT_ERROR_TITLE), "{:?}", labels(&test));
        assert!(
            drawn(&test, "fatal: https://example.com/r.git denied"),
            "streamed={streamed}: {:?}",
            labels(&test)
        );
        let texts = labels(&test);
        assert!(
            texts.iter().all(|text| !text.contains("SECRET")
                && !text.contains("SEC")
                && !text.contains("RET2")),
            "streamed={streamed}: a token was drawn: {texts:?}"
        );
    }
}

/// R10.5 as C4 amended it: the skip is offered on every failed commit — Cairn keeps no hook
/// model, and where no hook ran it fails the same way again, harmlessly — even one no hook
/// caused, git's own error drawn; Escape closes the dialog, the draft kept. Caught by: a skip
/// withheld for want of a hook Cairn found.
#[test]
fn every_failed_commit_offers_the_skip() {
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "No identity");
    click(&mut test, "Commit 1 File");
    let id = writes(&submitted)[0].0;
    apply(
        &mut test,
        view,
        &submitted,
        failed(id, "git commit -q -F -", "Author identity unknown"),
    );
    assert!(drawn(&test, GIT_ERROR_TITLE));
    assert!(
        drawn(&test, "Author identity unknown"),
        "git's own error: {:?}",
        labels(&test)
    );
    assert!(drawn(&test, SKIP_HOOKS_CAPTION), "{:?}", labels(&test));
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    assert!(!drawn(&test, GIT_ERROR_TITLE));
    assert_eq!(draft(view).0, "No identity");
}

/// R10.4, C5 and R4.3: a commit asked behind a write that runs waits, its button's place saying
/// what it waits for in the one form every queued write uses, with no Cancel; once it runs it
/// reads "Committing 0:00" with its Cancel, which cancels it by its id and nothing else, and then
/// reads "Cancelling…" until it ends — which opens no dialog and keeps the draft. Caught by: a
/// Cancel offered for a queued commit, one naming another write, or a cancel that opens a dialog.
#[test]
fn cancel_reaches_only_the_running_commit() {
    let (mut test, view, submitted) = opened();
    // A stage asked first, and running.
    press_row(&mut test, "a.rs", 0);
    crate::window::tests::press_chord(&mut test, cairn_ui::accelerators::Action::StageOrUnstage);
    settle(&mut test);
    let stage = writes(&submitted)[0].0;
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteStarted { id: stage },
    );
    set(&mut test, view.local.commit.subject, "After the stage");
    click(&mut test, "Commit 1 File");
    let commit = writes(&submitted)[1].0;
    assert!(
        drawn(&test, "Waiting for staging to finish…"),
        "{:?}",
        labels(&test)
    );
    assert!(
        !drawn(&test, CANCEL_COMMIT_CAPTION),
        "a queued commit offered Cancel"
    );
    apply(&mut test, view, &submitted, done(stage));
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteStarted { id: commit },
    );
    assert!(
        labels(&test).iter().any(|l| l == "Committing 0:00"),
        "{:?}",
        labels(&test)
    );
    click(&mut test, CANCEL_COMMIT_CAPTION);
    assert!(drawn(&test, cairn_ui::CANCELLING), "{:?}", labels(&test));
    assert!(!drawn(&test, CANCEL_COMMIT_CAPTION), "Cancel offered twice");
    let cancels: Vec<OperationId> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::CancelWrite { id } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(cancels, [commit]);
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: commit,
            ending: WriteEnding::Cancelled {
                message: "cancelled".to_owned(),
                locks: Vec::new(),
            },
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        },
    );
    assert!(!drawn(&test, cairn_ui::CANCELLING));
    assert!(!drawn(&test, GIT_ERROR_TITLE), "a cancel opened a dialog");
    assert!(view.confirming.peek().is_none());
    assert_eq!(
        draft(view).0,
        "After the stage",
        "the cancel took the draft"
    );
    assert!(drawn(&test, "Commit 1 File"), "{:?}", labels(&test));
}

/// R10.8, C24 and C32's view half: with a merge in progress the box fills an empty draft with
/// the message git prepared, cleaned of its comment lines as git's editor cleans it (R6.10),
/// under the one line saying so; the commit is asked with that text, whatever is staged; and
/// Amend is greyed, its reason on that line. Caught by: a merge commit refused for nothing
/// staged, or an amend offered during a merge.
#[test]
fn a_merge_fills_the_draft_with_the_cleaned_message_and_disables_amend() {
    let cleaned = "Merge branch 'feature'\n\nWith its body.\n";
    let (mut test, view, submitted) = opened_with(
        vec![changed("a.rs", None, Some(UnstagedChange::Modified))],
        true,
    );
    apply(
        &mut test,
        view,
        &submitted,
        reads(Some(OperationInProgress::Merge), Some(cleaned), &[]),
    );
    let (subject, description) = draft(view);
    assert_eq!(subject, "Merge branch 'feature'");
    assert_eq!(description, "With its body.\n");
    assert!(
        drawn(
            &test,
            "Merging. Committing concludes the merge; Amend is unavailable."
        ),
        "{:?}",
        labels(&test)
    );
    assert_eq!(
        crate::commit_box_state::compose_message(&subject, &description),
        cleaned,
        "the prefill is not the cleaned message"
    );
    click(&mut test, AMEND_CAPTION);
    assert_eq!(
        count(&submitted, |r| matches!(r, Request::Amending { .. })),
        0,
        "Amend ticked during a merge"
    );
    click(&mut test, "Commit");
    assert_eq!(
        writes(&submitted)
            .iter()
            .map(|(_, w)| w.clone())
            .collect::<Vec<_>>(),
        [format!("commit {cleaned:?} skip=false")]
    );
}

/// R10.8, C24 and C33's view half: during a rebase, `git am` or a sequence of picks or reverts
/// the box is disabled and names the operation; nothing it offers asks a write. A single
/// cherry-pick fills the draft with its cleaned message and is committed. Caught by: a commit
/// asked during a refusing operation, or a single pick refused.
#[test]
fn during_a_rebase_am_or_sequence_the_box_is_disabled_and_names_it() {
    for (operation, named) in [
        (
            OperationInProgress::Rebase,
            "A rebase is in progress. Continue or abort it with git rebase --continue or --abort.",
        ),
        (
            OperationInProgress::ApplyingPatches,
            "git am is in progress. Continue or abort it with git am --continue or --abort.",
        ),
        (
            OperationInProgress::CherryPickSequence,
            "A cherry-pick is in progress. Continue or abort it with git cherry-pick --continue \
             or --abort.",
        ),
        (
            OperationInProgress::RevertSequence,
            "A revert is in progress. Continue or abort it with git revert --continue or --abort.",
        ),
    ] {
        let (mut test, view, submitted) = opened();
        set(&mut test, view.local.commit.subject, "Something");
        apply(
            &mut test,
            view,
            &submitted,
            reads(Some(operation), None, &[]),
        );
        assert!(drawn(&test, named), "{named:?}: {:?}", labels(&test));
        click(&mut test, "Commit 1 File");
        click(&mut test, AMEND_CAPTION);
        assert!(writes(&submitted).is_empty(), "{named}: a commit was asked");
        assert_eq!(
            count(&submitted, |r| matches!(r, Request::Amending { .. })),
            0,
            "{named}: an amend was asked"
        );
    }
    let (mut test, view, submitted) = opened();
    apply(
        &mut test,
        view,
        &submitted,
        reads(
            Some(OperationInProgress::CherryPick {
                picked: Some(oid(0xab)),
            }),
            Some("The picked change\n"),
            &[],
        ),
    );
    assert_eq!(draft(view), ("The picked change".to_owned(), String::new()));
    assert!(
        drawn(
            &test,
            "Cherry-picking abababa. Committing concludes it; Amend is unavailable."
        ),
        "{:?}",
        labels(&test)
    );
    click(&mut test, "Commit 1 File");
    assert_eq!(
        writes(&submitted)
            .iter()
            .map(|(_, w)| w.clone())
            .collect::<Vec<_>>(),
        ["commit \"The picked change\" skip=false"]
    );
}

/// R10.1, R6.3, C13 and rule 4: on an unborn branch Amend is greyed, its reason on the box's
/// line — ticking it asks nothing — and a first commit is offered. Caught by: an amend read
/// asked with no `HEAD` to amend, or Amend greyed without its reason.
#[test]
fn amend_is_disabled_on_an_unborn_branch() {
    let (mut test, view, submitted) = opened_with(
        vec![changed("s.rs", Some(StagedChange::Added), None)],
        false,
    );
    click(&mut test, AMEND_CAPTION);
    assert_eq!(
        count(&submitted, |r| matches!(r, Request::Amending { .. })),
        0
    );
    assert!(!view.local.commit.state.peek().is_amending());
    assert!(
        drawn(&test, cairn_ui::UNBORN_LINE),
        "Amend greyed with no reason: {:?}",
        labels(&test)
    );
    set(&mut test, view.local.commit.subject, "First");
    click(&mut test, "Commit 1 File");
    assert_eq!(writes(&submitted).len(), 1);
}

/// R6.3 and R10.3 (phase 05's carry): while amending, Staged lists amend's staged list — a file
/// `HEAD` added among it — whose diff is asked against `HEAD`'s parent, and unstaging a file of
/// it puts the parent's entry back; a root commit's amend unstages to nothing. Caught by: the
/// staged diff asked against `HEAD`, or an unstage out of an amend that resets to `HEAD`
/// (the file would stay in the amended commit).
#[test]
fn amends_staged_list_is_drawn_diffed_against_heads_parent_and_unstaged_to_it() {
    for (parent, to) in [
        (Some(oid(0x0c)), UnstageTarget::Commit(oid(0x0c))),
        (None, UnstageTarget::Nothing),
    ] {
        let (mut test, view, submitted) = opened();
        click(&mut test, AMEND_CAPTION);
        apply(
            &mut test,
            view,
            &submitted,
            amend_answer(&submitted, parent),
        );
        press_row(&mut test, "h.rs", 0);
        let last = asked(&submitted).pop();
        assert!(
            matches!(
                last.as_ref().map(|query| &query.target),
                Some(crate::worker::FileTarget::WorkingTree {
                    side: WorkingSide::Amending,
                    ..
                })
            ),
            "{last:?}"
        );
        crate::local_changes_tests::click_heading(&mut test, UNSTAGE_CAPTION);
        let unstaged: Vec<String> = writes(&submitted).into_iter().map(|(_, w)| w).collect();
        assert_eq!(
            unstaged,
            [format!("unstage [\"h.rs\"] to {to:?}")],
            "{:?}",
            labels(&test)
        );
    }
}

/// Phase 05's QA item 6 under rule 6: when amend's staged list cannot be read (a partial
/// clone's missing blob on git 2.44 and later), the box's one line says so and why, the lists
/// stay the status's against `HEAD`, and the amend itself is still offered. Caught by: a failure
/// dropped silently, or amend's lists half-drawn.
#[test]
fn amends_staged_list_unread_is_said_and_the_status_lists_stay() {
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Fix");
    click(&mut test, AMEND_CAPTION);
    apply(
        &mut test,
        view,
        &submitted,
        amend_read(&submitted, Err("git diff-index failed: missing blob")),
    );
    let said = labels(&test)
        .into_iter()
        .find(|label| label.starts_with("Amend's staged list couldn't be read"));
    assert!(
        said.as_deref()
            .is_some_and(|said| said.contains("missing blob")),
        "{:?}",
        labels(&test)
    );
    assert!(crate::local_changes_tests::in_lists_reads(&test, "s.rs"));
    assert!(!crate::local_changes_tests::in_lists_reads(&test, "h.rs"));
    assert!(drawn(&test, AMEND_BUTTON_CAPTION));
}

/// R10.2: the `≡` lists the recent messages' subjects and a choice fills both fields; ↑ in an
/// empty subject recalls the newest, ↑ again the next older, ↓ back, and ↓ from the newest
/// empties the draft; in a subject the person typed, ↑ is the editor's. Caught by: a recall over
/// a typed subject, or a choice filling the subject alone.
#[test]
fn recent_messages_fill_both_fields_from_the_menu_and_the_arrows() {
    let (mut test, view, submitted) = opened();
    apply(
        &mut test,
        view,
        &submitted,
        reads(None, None, &["Fix A\n\nBody A\n", "Fix B\n"]),
    );
    click(&mut test, "≡");
    click(&mut test, "Fix B");
    assert_eq!(draft(view), ("Fix B".to_owned(), String::new()));
    set(&mut test, view.local.commit.subject, "");
    set(&mut test, view.local.commit.description, "");
    let at = field_at(&test, cairn_ui::SUBJECT_PLACEHOLDER);
    test.click_cursor(at);
    settle(&mut test);
    let arrow = |test: &mut TestingRunner, key| {
        test.press_key(Key::Named(key));
        settle(test);
    };
    arrow(&mut test, NamedKey::ArrowUp);
    assert_eq!(draft(view), ("Fix A".to_owned(), "Body A\n".to_owned()));
    arrow(&mut test, NamedKey::ArrowUp);
    assert_eq!(draft(view), ("Fix B".to_owned(), String::new()));
    arrow(&mut test, NamedKey::ArrowDown);
    assert_eq!(draft(view).0, "Fix A");
    arrow(&mut test, NamedKey::ArrowDown);
    assert_eq!(draft(view), (String::new(), String::new()));
    test.write_text("typed");
    settle(&mut test);
    arrow(&mut test, NamedKey::ArrowUp);
    assert_eq!(draft(view).0, "typed", "a typed subject was recalled over");
}

/// R10.5, C4 and R1.1's option (a), the user's answer of 2026-10-10, and the QA brief's second
/// case: a confirmed amend a hook fails hands its token back on the ending; the Git Error dialog
/// draws no prompt, and its skip — in the footer — asks the amend with `--no-verify` once, under
/// that very token, no dialog asked again. An amend at the press that a hook fails is skipped at
/// the press again. Caught by: the skip asking the dialog again, committing rather than amending,
/// a token other than the one handed back, or a prompt drawn in the Git Error dialog.
#[test]
fn a_failed_amends_skip_amends_once_without_asking_again() {
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Fix the parser");
    click(&mut test, AMEND_CAPTION);
    click(&mut test, AMEND_BUTTON_CAPTION);
    let pressed = writes(&submitted)[0].0;
    let consequence = amend_consequence(Publication::Upstream(RefName::new(
        "refs/remotes/origin/main",
    )));
    let prompt = consequence.prompt();
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id: pressed,
            ending: WriteEnding::NeedsConfirming {
                consequence: consequence.clone(),
            },
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        },
    );
    click_last(&mut test, "Amend");
    let confirmed = writes(&submitted)[1].0;
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteStarted { id: confirmed },
    );
    // The token handed back is told from any the window could build afresh: it names another
    // commit, so its prompt is its own (phase 18's QA item 7).
    let handed_back = amend_consequence_of(
        0xcd,
        Publication::Upstream(RefName::new("refs/remotes/origin/main")),
        Reflog::Written,
    );
    let handed_back_prompt = handed_back.prompt();
    assert_ne!(handed_back_prompt, prompt);
    let mut failure = failed(confirmed, "git commit -q --amend -F -", "lint failed");
    if let Update::WriteEnded { unspent, .. } = &mut failure {
        *unspent = crate::worker::Unspent::holding(cairn_model::Confirmed::by_user(handed_back));
    }
    apply(&mut test, view, &submitted, failure);
    assert!(drawn(&test, GIT_ERROR_TITLE), "{:?}", labels(&test));
    assert!(!drawn(&test, &prompt), "a prompt in the Git Error dialog");
    click(&mut test, SKIP_HOOKS_CAPTION);
    let asked: Vec<String> = writes(&submitted).into_iter().map(|(_, w)| w).collect();
    assert_eq!(
        asked,
        [
            "amend at the press \"Fix the parser\" skip=false".to_owned(),
            format!("amend \"Fix the parser\" skip=false confirming {prompt:?}"),
            format!("amend \"Fix the parser\" skip=true confirming {handed_back_prompt:?}"),
        ],
        "the skip spent another token than the one handed back"
    );
    assert!(
        view.confirming.peek().is_none(),
        "the skip asked the dialog again"
    );
    assert!(!drawn(&test, GIT_ERROR_TITLE));

    // An amend at the press a hook failed: skipped at the press again.
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Fix");
    click(&mut test, AMEND_CAPTION);
    click(&mut test, AMEND_BUTTON_CAPTION);
    let id = writes(&submitted)[0].0;
    apply(
        &mut test,
        view,
        &submitted,
        failed(id, "git commit -q --amend -F -", "lint failed"),
    );
    click(&mut test, SKIP_HOOKS_CAPTION);
    let asked: Vec<String> = writes(&submitted).into_iter().map(|(_, w)| w).collect();
    assert_eq!(
        asked,
        [
            "amend at the press \"Fix\" skip=false",
            "amend at the press \"Fix\" skip=true",
        ]
    );
}

/// QA item 1: a draft typed while a commit runs is the person's next message, and the commit's
/// ending keeps it — only the draft the commit was asked with is cleared. Caught by: every
/// draft wiped when a commit is made.
#[test]
fn a_draft_typed_while_a_commit_runs_outlives_its_ending() {
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "First");
    click(&mut test, "Commit 1 File");
    let id = writes(&submitted)[0].0;
    apply(&mut test, view, &submitted, Update::WriteStarted { id });
    set(
        &mut test,
        view.local.commit.subject,
        "Second, typed meanwhile",
    );
    apply(&mut test, view, &submitted, done(id));
    assert_eq!(
        draft(view),
        ("Second, typed meanwhile".to_owned(), String::new()),
        "the draft typed during the commit was wiped"
    );
    // The draft the commit took is cleared.
    set(&mut test, view.local.commit.subject, "Third");
    click(&mut test, "Commit 1 File");
    let id = writes(&submitted)[1].0;
    apply(&mut test, view, &submitted, done(id));
    assert_eq!(draft(view), (String::new(), String::new()));
}

/// QA item 7: Cancel with the commit still queued behind a running stage asks no cancel — the
/// write running is the stage's. Caught by: a cancel sent for whatever runs, or for the queued
/// commit.
#[test]
fn a_cancel_while_the_commit_is_queued_asks_nothing() {
    let (mut test, view, submitted) = opened();
    press_row(&mut test, "a.rs", 0);
    crate::window::tests::press_chord(&mut test, cairn_ui::accelerators::Action::StageOrUnstage);
    settle(&mut test);
    let stage = writes(&submitted)[0].0;
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteStarted { id: stage },
    );
    set(&mut test, view.local.commit.subject, "Queued");
    click(&mut test, "Commit 1 File");
    let submit = {
        let submitted = std::rc::Rc::clone(&submitted);
        move |request| submitted.borrow_mut().push(request)
    };
    test.run_in(|| crate::commit_box_pane::cancel(view, Some(&submit)));
    assert_eq!(
        count(&submitted, |r| matches!(r, Request::CancelWrite { .. })),
        0
    );
}

/// Phase 18's QA item 1: an amend at the press that ends asking to be confirmed never replaces a
/// confirmation already open — a discard's, opened while the amend's cost was read — which stays
/// drawn, the amend's waiting; once that one is answered, "Amend Commit" opens. Caught by: a
/// dialog swapped under the person's pointer, its acknowledgement then another prompt's.
#[test]
fn the_amend_dialog_waits_for_a_confirmation_already_open() {
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Fix the parser");
    click(&mut test, AMEND_CAPTION);
    click(&mut test, AMEND_BUTTON_CAPTION);
    let id = writes(&submitted)[0].0;
    let mut confirming = view.confirming;
    test.run_in(|| {
        confirming.set(Some(crate::confirming::Confirming::new(
            "Discard Changes",
            amend_consequence_of(0xcd, Publication::SomeRemote, Reflog::Written),
            |_| {},
        )));
    });
    settle(&mut test);
    apply(
        &mut test,
        view,
        &submitted,
        Update::WriteEnded {
            id,
            ending: WriteEnding::NeedsConfirming {
                consequence: amend_consequence(Publication::SomeRemote),
            },
            read_again: crate::worker::ReadAgain::Everything,
            unspent: crate::worker::Unspent::none(),
        },
    );
    assert!(drawn(&test, "Discard Changes"), "{:?}", labels(&test));
    assert!(
        !drawn(&test, cairn_ui::AMEND_TITLE),
        "the open confirmation was replaced"
    );
    assert!(view.local.commit.state.peek().is_confirming());
    click_last(&mut test, "Cancel");
    assert!(drawn(&test, cairn_ui::AMEND_TITLE), "{:?}", labels(&test));
    assert!(!view.local.commit.state.peek().is_confirming());
}

/// Phase 18's QA item 6: a commit asked while the lane is busy with no write — a read — is drawn
/// without a Cancel nothing would hear, and "Waiting for" names only a write ahead of it, never
/// one asked after it. Caught by: a dead Cancel, or a queued stage behind the commit named as
/// what it waits for.
#[test]
fn a_queued_commit_offers_no_dead_cancel_and_waits_only_on_writes_ahead() {
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Queued");
    click(&mut test, "Commit 1 File");
    assert!(
        !drawn(&test, CANCEL_COMMIT_CAPTION),
        "a Cancel offered before the commit ran: {:?}",
        labels(&test)
    );
    // A stage asked after the commit, queued behind it.
    press_row(&mut test, "a.rs", 0);
    crate::window::tests::press_chord(&mut test, cairn_ui::accelerators::Action::StageOrUnstage);
    settle(&mut test);
    assert_eq!(writes(&submitted).len(), 2, "{:?}", writes(&submitted));
    assert!(
        !labels(&test).iter().any(|l| l.starts_with("Waiting for")),
        "a write behind the commit named as what it waits for: {:?}",
        labels(&test)
    );
    assert!(!drawn(&test, CANCEL_COMMIT_CAPTION));
}

/// The coordinator's ruling under rule 4 (phase 18's QA item 8): on a detached `HEAD` the
/// detached line wins over amend's unread one, since it says what the press will do. Caught by:
/// the unread line drawn in its place.
#[test]
fn the_detached_line_wins_over_amends_unread_one() {
    let (mut test, view, submitted) = opened();
    apply(
        &mut test,
        view,
        &submitted,
        Update::Refs {
            snapshot: Arc::new(RefsSnapshot {
                refs: Vec::new(),
                head: HeadState::Detached(oid(0xab)),
                stashes: Vec::new(),
                unreadable: 0,
            }),
            reopen: false,
        },
    );
    click(&mut test, AMEND_CAPTION);
    apply(
        &mut test,
        view,
        &submitted,
        amend_read(&submitted, Err("git diff-index failed: missing blob")),
    );
    assert!(drawn(&test, cairn_ui::DETACHED_LINE), "{:?}", labels(&test));
    assert!(
        !labels(&test)
            .iter()
            .any(|l| l.starts_with("Amend's staged list couldn't be read")),
        "{:?}",
        labels(&test)
    );
}
