//! The commit box in the window, headless (staging-and-commit R10; criteria C13, C14 and C24's
//! view halves): the draft kept through Amend's toggling, a failed hook and its skip, the
//! amend's button, its line and its token naming one `HEAD`, the dialog asked first exactly when
//! a remote has it, a commit running drawn with its Cancel, a merge's message and the operations
//! that disable the box, amend's staged list drawn and unstaged from, and the recent messages.
//! Updates are applied through `session::apply`, as the worker's stream applies them, and
//! requests read back from the window's submit.

use std::sync::Arc;

use cairn_model::{
    ChangeStatus, ChangedFile, CommitHooks, Consequence, FileMode, HeadState, LocalChanges, Oid,
    OperationInProgress, Publication, RefName, Reflog, RefsSnapshot, RepoPath, StagedChange,
    StatusEntry, UnstagedChange, WorkingTreeStatus,
};
use cairn_ui::{
    AMEND_CAPTION, CANCEL_COMMIT_CAPTION, GIT_ERROR_TITLE, SKIP_HOOKS_CAPTION, UNSTAGE_CAPTION,
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

fn reads(operation: Option<OperationInProgress>, hooks: CommitHooks, recent: &[&str]) -> Update {
    Update::CommitReads(Box::new(CommitReads {
        operation,
        hooks: Ok(hooks),
        recent: Ok(recent.iter().map(|message| (*message).to_owned()).collect()),
    }))
}

const NO_HOOKS: CommitHooks = CommitHooks {
    pre_commit: false,
    commit_msg: false,
};

/// Local Changes opened over `entries`, `HEAD` born or not, and the box's reads answered.
fn opened_with(entries: Vec<StatusEntry>, born: bool) -> (TestingRunner, View, Submitted) {
    let (mut test, view, submitted) = launch();
    apply(&mut test, view, &submitted, status(entries));
    apply(&mut test, view, &submitted, refs(born));
    open_local_changes(&mut test);
    apply(&mut test, view, &submitted, reads(None, NO_HOOKS, &[]));
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
    Consequence::Amend {
        commit: oid(0xab),
        subject: "Fix the parser".to_owned(),
        published,
        reflog: Reflog::Written,
    }
}

/// What the worker answers for an amend: `published` or not, `HEAD`'s message, and amend's
/// lists over `entries` — s.rs and h.rs, a file `HEAD` added, in Staged — against `parent`.
fn amend_answer(published: Publication, parent: Option<Oid>) -> Update {
    let file = |status, path: &str| ChangedFile {
        status,
        old_path: RepoPath::from(path),
        new_path: RepoPath::from(path),
        old_mode: Some(FileMode::Regular),
        new_mode: Some(FileMode::Regular),
        old_id: None,
        new_id: None,
    };
    let status = Arc::new(LocalChanges::new(WorkingTreeStatus::Listed(entries())));
    let lists = LocalChanges::amending(
        WorkingTreeStatus::Listed(entries()),
        vec![
            file(ChangeStatus::Added, "h.rs"),
            file(ChangeStatus::Modified, "s.rs"),
        ],
        parent,
    );
    Update::Amending {
        status,
        read: Box::new(AmendRead {
            consequence: Ok(amend_consequence(published)),
            message: Ok("Fix the parser\n\nIt read past the end.\n".to_owned()),
            lists: Ok(Arc::new(lists)),
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

/// R10.3, R10.7 and the QA brief's first case: a draft typed in both fields, Amend ticked and
/// unticked, is back exactly as typed — the amend's read, arriving meanwhile, never writing over
/// it; an empty draft is filled with `HEAD`'s message, which unticking takes away again. Each
/// tick asks what an amend would replace over the status drawn, and each untick stops that read
/// and draws the status's own lists again. Caught by: the draft lost to `HEAD`'s message, a
/// draft not restored, or amend's lists left drawn after Amend is unticked.
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
        amend_answer(Publication::Unpublished, Some(oid(0x0c))),
    );
    assert_eq!(draft(view), typed, "the amend's read wrote over the draft");
    assert!(
        crate::local_changes_tests::in_lists_reads(&test, "h.rs"),
        "amend's staged list is not drawn"
    );
    click(&mut test, AMEND_CAPTION);
    assert_eq!(draft(view), typed);
    assert_eq!(count(&submitted, |r| matches!(r, Request::StopAmending)), 1);
    assert!(
        !crate::local_changes_tests::in_lists_reads(&test, "h.rs"),
        "amend's lists outlived Amend"
    );

    // An empty draft: `HEAD`'s message fills it, and unticking empties it again.
    set(&mut test, view.local.commit.subject, "");
    set(&mut test, view.local.commit.description, "");
    click(&mut test, AMEND_CAPTION);
    apply(
        &mut test,
        view,
        &submitted,
        amend_answer(Publication::Unpublished, Some(oid(0x0c))),
    );
    assert_eq!(
        draft(view),
        (
            "Fix the parser".to_owned(),
            "It read past the end.\n".to_owned()
        )
    );
    click(&mut test, AMEND_CAPTION);
    assert_eq!(draft(view), (String::new(), String::new()));
}

/// R10.6, C14's view half and the QA brief's third case: an amend no remote has is confirmed by
/// its own button, which reads `Amend <short id>` above the line its `Consequence` renders; the
/// token it builds records exactly that line and names the same `HEAD`; one press is one
/// amend, and the box waits while it runs. Caught by: a button or line typed apart from the
/// consequence, a token recording other words, or a second press building a second token.
#[test]
fn the_amend_button_its_line_and_its_token_name_one_head() {
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Fix the parser");
    click(&mut test, AMEND_CAPTION);
    assert!(
        drawn(&test, cairn_ui::AMEND_CAPTION) && !drawn(&test, "Amend abababa"),
        "an amend button before what it replaces was read"
    );
    apply(
        &mut test,
        view,
        &submitted,
        amend_answer(Publication::Unpublished, Some(oid(0x0c))),
    );
    let consequence = amend_consequence(Publication::Unpublished);
    let line = consequence.replaces().unwrap_or_default();
    assert!(drawn(&test, "Amend abababa"), "{:?}", labels(&test));
    assert!(
        drawn(&test, &line),
        "the line under the button: {:?}",
        labels(&test)
    );
    click(&mut test, "Amend abababa");
    click(&mut test, "Amend abababa");
    let asked = writes(&submitted);
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(
        asked[0].1,
        format!("amend \"Fix the parser\" skip=false confirming {line:?}")
    );
    let token_head = submitted.borrow().iter().find_map(|request| match request {
        Request::Write {
            write: LocalWrite::Amend { confirmed, .. },
            ..
        } => confirmed.consequence().amended(),
        _ => None,
    });
    assert_eq!(token_head, Some(oid(0xab)));
    assert!(
        view.confirming.peek().is_none(),
        "an unpublished amend asked a dialog"
    );
}

/// R10.6, L12 and C14: an amend a remote has asks the confirmation dialog first — its words the
/// whole prompt, force push and all — and only the dialog's button builds the token; the commit
/// chord heard in the subject while amending asks the dialog too, never building a token by
/// itself. Caught by: a published amend confirmed by its button alone, or a chord that amends.
#[test]
fn a_published_amend_and_the_commit_chord_ask_the_dialog_first() {
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Fix the parser");
    click(&mut test, AMEND_CAPTION);
    apply(
        &mut test,
        view,
        &submitted,
        amend_answer(
            Publication::Upstream(RefName::new("refs/remotes/origin/main")),
            Some(oid(0x0c)),
        ),
    );
    click(&mut test, "Amend abababa…");
    assert!(writes(&submitted).is_empty(), "amended before the dialog");
    let prompt = amend_consequence(Publication::Upstream(RefName::new(
        "refs/remotes/origin/main",
    )))
    .prompt();
    assert!(
        drawn(&test, &prompt),
        "the dialog's words: {:?}",
        labels(&test)
    );
    click(&mut test, "Amend abababa");
    let asked = writes(&submitted);
    assert_eq!(asked.len(), 1);
    assert!(asked[0].1.contains(&format!("{prompt:?}")), "{asked:?}");

    // Unpublished, the chord in a field asks the dialog rather than amending.
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Fix the parser");
    click(&mut test, AMEND_CAPTION);
    apply(
        &mut test,
        view,
        &submitted,
        amend_answer(Publication::Unpublished, Some(oid(0x0c))),
    );
    let at = field_at(&test, "Fix the parser");
    test.click_cursor(at);
    settle(&mut test);
    crate::window::tests::press_chord(&mut test, cairn_ui::accelerators::Action::Commit);
    settle(&mut test);
    assert!(writes(&submitted).is_empty(), "the chord amended by itself");
    assert!(
        view.confirming.peek().is_some(),
        "the chord asked no dialog"
    );
}

fn failed(id: OperationId, command: &str, output: &str) -> Update {
    Update::WriteEnded {
        id,
        ending: WriteEnding::Failed {
            message: format!("{command} failed (exit status: 1): {output}"),
            locks: Vec::new(),
            command: Some(command.to_owned()),
            output: output.to_owned(),
        },
        read_again: crate::worker::ReadAgain::Everything,
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
        }),
        read_again: crate::worker::ReadAgain::Everything,
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
    apply(
        &mut test,
        view,
        &submitted,
        reads(
            None,
            CommitHooks {
                pre_commit: true,
                commit_msg: false,
            },
            &[],
        ),
    );
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
                line: line.to_owned(),
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

/// R10.5: the skip is offered only where git would run a hook it skips; with none, the dialog
/// offers Close alone, and Escape closes it. Caught by: the skip offered whatever the hooks.
#[test]
fn with_no_hook_the_git_error_offers_no_skip() {
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
    assert!(!drawn(&test, SKIP_HOOKS_CAPTION));
    test.press_key(Key::Named(NamedKey::Escape));
    settle(&mut test);
    assert!(!drawn(&test, GIT_ERROR_TITLE));
    assert_eq!(draft(view).0, "No identity");
}

/// R10.4 and R4.3: a commit asked behind a write that runs waits, drawn waiting with no
/// Cancel; once it runs it is drawn busy with its Cancel, which cancels it by its id and
/// nothing else. Caught by: a Cancel offered for a queued commit, or one naming another write.
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
    assert!(drawn(&test, "Committing (waiting)…"), "{:?}", labels(&test));
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
    assert!(drawn(&test, "Committing…"), "{:?}", labels(&test));
    click(&mut test, CANCEL_COMMIT_CAPTION);
    let cancels: Vec<OperationId> = submitted
        .borrow()
        .iter()
        .filter_map(|request| match request {
            Request::CancelWrite { id } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(cancels, [commit]);
}

/// R10.8, C24 and the user's decision E: with a merge in progress the box fills an empty draft
/// with `MERGE_MSG` exactly as git wrote it, its `# Conflicts:` lines visible; the commit is
/// asked with that text, whatever is staged; and Amend is disabled. Caught by: comment lines
/// stripped, a merge commit refused for nothing staged, or an amend offered during a merge.
#[test]
fn a_merge_fills_the_draft_with_merge_msg_as_git_wrote_it_and_disables_amend() {
    let merge_msg = "Merge branch 'feature'\n\n# Conflicts:\n#\ta.rs\n";
    let (mut test, view, submitted) = opened_with(
        vec![changed("a.rs", None, Some(UnstagedChange::Modified))],
        true,
    );
    apply(
        &mut test,
        view,
        &submitted,
        reads(
            Some(OperationInProgress::Merge {
                message: Some(merge_msg.to_owned()),
            }),
            NO_HOOKS,
            &[],
        ),
    );
    let (subject, description) = draft(view);
    assert_eq!(subject, "Merge branch 'feature'");
    assert_eq!(description, "# Conflicts:\n#\ta.rs\n");
    assert_eq!(
        crate::commit_box_state::compose_message(&subject, &description),
        merge_msg,
        "the prefill is not MERGE_MSG's text"
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
        [format!("commit {merge_msg:?} skip=false")]
    );
}

/// R10.8 and C24: during a rebase, `git am`, a cherry-pick or a revert the box is disabled and
/// names the operation; nothing it offers asks a write. Caught by: a commit asked during one.
#[test]
fn during_a_rebase_am_cherry_pick_or_revert_the_box_is_disabled_and_names_it() {
    for (operation, named) in [
        (OperationInProgress::Rebase, "a rebase"),
        (OperationInProgress::ApplyingPatches, "git am"),
        (OperationInProgress::CherryPick, "a cherry-pick"),
        (OperationInProgress::Revert, "a revert"),
    ] {
        let (mut test, view, submitted) = opened();
        set(&mut test, view.local.commit.subject, "Something");
        apply(
            &mut test,
            view,
            &submitted,
            reads(Some(operation), NO_HOOKS, &[]),
        );
        let said = format!("Committing is unavailable while {named} is in progress.");
        assert!(drawn(&test, &said), "{said:?}: {:?}", labels(&test));
        click(&mut test, "Commit 1 File");
        click(&mut test, AMEND_CAPTION);
        assert!(writes(&submitted).is_empty(), "{named}: a commit was asked");
        assert_eq!(
            count(&submitted, |r| matches!(r, Request::Amending { .. })),
            0,
            "{named}: an amend was asked"
        );
    }
}

/// R10.1, R6.3 and C13: on an unborn branch Amend is disabled — ticking it asks nothing — and a
/// first commit is offered. Caught by: an amend read asked with no `HEAD` to amend.
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
            amend_answer(Publication::Unpublished, parent),
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

/// Phase 05's QA item 6: when amend's staged list cannot be read (a partial clone's missing blob
/// on git 2.44 and later), the box says so and why, the lists stay the status's against `HEAD`,
/// and the amend itself is still offered — the list is the box's to show, not the amend's to
/// need. Caught by: a failure dropped silently, or amend's lists half-drawn.
#[test]
fn amends_staged_list_unread_is_said_and_the_status_lists_stay() {
    let (mut test, view, submitted) = opened();
    set(&mut test, view.local.commit.subject, "Fix");
    click(&mut test, AMEND_CAPTION);
    let status = Arc::new(LocalChanges::new(WorkingTreeStatus::Listed(entries())));
    apply(
        &mut test,
        view,
        &submitted,
        Update::Amending {
            status,
            read: Box::new(AmendRead {
                consequence: Ok(amend_consequence(Publication::Unpublished)),
                message: Ok("Fix the parser\n".to_owned()),
                lists: Err("git diff-index failed: missing blob".to_owned()),
            }),
        },
    );
    let said = labels(&test)
        .into_iter()
        .find(|label| label.starts_with("Amend's staged list could not be read"));
    assert!(
        said.as_deref()
            .is_some_and(|said| said.contains("missing blob")),
        "{:?}",
        labels(&test)
    );
    assert!(!crate::local_changes_tests::in_lists_reads(&test, "h.rs"));
    assert!(drawn(&test, "Amend abababa"));
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
        reads(None, NO_HOOKS, &["Fix A\n\nBody A\n", "Fix B\n"]),
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
