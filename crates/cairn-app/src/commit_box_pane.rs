//! The commit box under Local Changes' diff, and what its presses do (staging-and-commit R10):
//! `cairn_ui::CommitBox` drawn from `commit_box_state`, each press asking the local lane — a
//! commit, an amend under its confirmation, a cancel of the commit running — and each update the
//! worker sends for the box applied here (`session::apply` hands them on).
//!
//! **What is asked, and where.** As the box is shown, and as each refresh's refs arrive while it
//! is, the box's reads are asked (`Request::CommitReads`: the operation in progress, the hooks
//! git would run, the recent messages), on the local lane after the writes asked before them.
//! While Amend is ticked, each status that arrives asks what an amend would replace and amend's
//! lists over that status (`Request::Amending`), in a lane of its own, so the next supersedes
//! the last; the lists that answer are drawn as a status's are, and unticking Amend draws the
//! status's own lists again. Nothing here waits: each is a submit.
//!
//! **The seal.** An amend is destructive (R1.5): its token is built only by a confirmation
//! surface (R1.1), never here. An amend no remote has is confirmed by the box's amend button or
//! the commit chord — the commit box itself, `cairn_ui::AmendButton`, which draws the line it
//! confirms and hands its token to [`amend_confirmed`]; an amend a remote has opens the
//! confirmation dialog (`Confirming`) over the window first, by button or chord; and the skip of
//! a hook that failed an amend is `cairn_ui::AmendSkip` in the Git Error dialog, which draws the
//! prompt the amend was confirmed with and builds its token from that consequence — one press,
//! no second dialog (the user's decision, 2026-10-09). The engine re-checks every token's
//! consequence before git runs.

use std::rc::Rc;

use cairn_model::{ChangeList, Confirmed, Consequence, OperationInProgress, StagedAgainst};
use cairn_ui::accelerators::RecallStep;
use cairn_ui::{Busy, CommitBox, CommitButton, GitErrorDialog, MainView};
use freya::prelude::*;

use crate::commit_box_state::{AskedCommit, compose_message, split_message};
use crate::confirming::Confirming;
use crate::local_changes_state::drawn_changes;
use crate::local_writes;
use crate::window::View;
use crate::worker::{
    AmendRead, CommitReads, LocalWrite, OperationId, Request, Retired, WriteEnding,
};

/// The confirmation dialog's title for an amend.
pub const AMEND_TITLE: &str = "Amend commit";

pub struct CommitBoxPane {
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
}

impl CommitBoxPane {
    pub fn new(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Self {
        Self { view, submit }
    }
}

// By the handles it reads: the submitter is the same repository's however often it is built.
impl PartialEq for CommitBoxPane {
    fn eq(&self, other: &Self) -> bool {
        self.view.local == other.view.local
            && self.view.writes == other.view.writes
            && self.view.refreshed == other.view.refreshed
            && self.submit.is_some() == other.submit.is_some()
    }
}

impl Component for CommitBoxPane {
    fn render(&self) -> impl IntoElement {
        let view = self.view;
        let commit = view.local.commit;
        // What the box reads is asked as it is shown (and again as each refresh's refs arrive,
        // `session::apply`).
        let asking = self.submit.clone();
        use_hook(move || {
            if let Some(submit) = asking.as_deref() {
                submit(Request::CommitReads);
            }
        });

        let state = commit.state.read();
        let writes = view.writes.read();
        let refreshed = view.refreshed.read();
        let operation = state.operation().cloned();
        let stopped = operation
            .as_ref()
            .filter(|operation| operation.refuses_commit())
            .map(|operation| {
                format!(
                    "Committing is unavailable while {} is in progress.",
                    operation.name()
                )
            });
        let merging = matches!(operation, Some(OperationInProgress::Merge { .. }));
        let born = refreshed
            .refs()
            .is_some_and(|refs| !matches!(refs.head, cairn_model::HeadState::Unborn(_)));
        let amend_enabled = born && operation.is_none();

        let busy = state.asked().map(|asked| {
            let running = writes
                .running()
                .is_some_and(|running| running.id == asked.id);
            let verb = if asked.amend {
                "Amending"
            } else {
                "Committing"
            };
            Busy {
                what: if running {
                    verb.to_owned()
                } else {
                    format!("{verb} (waiting)")
                },
                started: state.started().filter(|_| running),
                cancellable: running,
            }
        });

        let (button, amend_ok, note) = if state.is_amending() {
            match state.amendable() {
                None => (CommitButton::ReadingAmend, false, None),
                Some(amendable) => {
                    let note = amendable.lists_failed.as_ref().map(|why| {
                        (
                            format!(
                                "Amend's staged list could not be read, so the lists show what \
                                 is staged against HEAD: {why}"
                            ),
                            true,
                        )
                    });
                    match &amendable.consequence {
                        Ok(consequence) if consequence.needs_force_push() => (
                            CommitButton::AmendAsking {
                                serial: amendable.serial,
                                consequence: consequence.clone(),
                            },
                            true,
                            note,
                        ),
                        Ok(consequence) => (
                            CommitButton::AmendInPlace {
                                serial: amendable.serial,
                                consequence: consequence.clone(),
                            },
                            true,
                            note,
                        ),
                        Err(why) => (
                            CommitButton::AmendUnreadable(why.clone()),
                            false,
                            Some((
                                format!("What Amend would replace could not be read: {why}"),
                                true,
                            )),
                        ),
                    }
                }
            }
        } else {
            let files = drawn_changes(&view.local.state.read()).len(ChangeList::Staged);
            (CommitButton::Commit { files }, files > 0 || merging, None)
        };
        let subject_given = !commit.subject.read().trim().is_empty();
        // While a newer amend read is on its way the last one's consequence stays drawn, and the
        // button waits for the new one: a press confirms only what was read last.
        let ready = amend_ok
            && subject_given
            && state.asked().is_none()
            && !(state.is_amending() && state.is_reading_amend());
        let recent = state.recent().clone();
        let amending = state.is_amending();
        drop((state, writes, refreshed));

        let submit = self.submit.clone();
        let toggling = submit.clone();
        let pressing = submit.clone();
        let confirming = submit.clone();
        let cancelling = submit.clone();
        CommitBox::new(commit.subject, commit.description)
            .amend(amending, amend_enabled)
            .button(button, ready)
            .busy(busy)
            .stopped(stopped)
            .note(note)
            .recent(recent)
            .on_amend(move |on: bool| toggle_amend(on, view, toggling.as_deref()))
            .on_commit(move |()| pressed(view, pressing.clone()))
            .on_confirmed(move |token: Confirmed| {
                amend_confirmed(token, view, confirming.as_deref());
            })
            .on_cancel(move |()| cancel(view, cancelling.as_deref()))
            .on_recall(move |at: usize| recall(at, view))
            .recall(Callback::new(move |step: RecallStep| {
                recall_step(step, view)
            }))
            .into_element()
    }
}

fn submit_all(requests: Vec<Request>, submit: Option<&dyn Fn(Request)>) {
    if let Some(submit) = submit {
        for request in requests {
            submit(request);
        }
    }
}

/// Amend ticked or unticked (R10.3): the draft set aside or put back exactly, what an amend
/// would replace asked over the latest status, or the status's own lists drawn again.
pub fn toggle_amend(on: bool, view: View, submit: Option<&dyn Fn(Request)>) {
    let commit = view.local.commit;
    let mut state = commit.state;
    let latest = view.refreshed.peek().local_changes().cloned();
    if on {
        if state.peek().is_amending() {
            return;
        }
        let (subject, description) = (
            commit.subject.peek().clone(),
            commit.description.peek().clone(),
        );
        state.write().tick_amend(&subject, &description);
        if let (Some(status), Some(submit)) = (latest, submit) {
            submit(Request::Amending { status });
        }
        return;
    }
    let Some((subject, description)) = state.write().untick_amend() else {
        return;
    };
    let (mut typed, mut body) = (commit.subject, commit.description);
    typed.set(subject);
    body.set(description);
    if let Some(submit) = submit {
        submit(Request::StopAmending);
    }
    // The lists while amending are let go of: the status's own are drawn again.
    let drawn_amend =
        drawn_changes(&view.local.state.peek()).staged_against() != StagedAgainst::Head;
    if let (true, Some(status)) = (drawn_amend, latest) {
        let mut local = view.local.state;
        let requests = local.write().status_arrived(status);
        submit_all(requests, submit);
    }
}

/// The commit button, or the commit chord heard in a field (R10.1, R7.3): commits what is
/// staged; amending a commit a remote has, opens the confirmation dialog. An amend no remote has
/// is confirmed by the box itself and comes through [`amend_confirmed`].
pub fn pressed(view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    let commit = view.local.commit;
    let (amending, consequence) = {
        let state = commit.state.peek();
        if state.asked().is_some() {
            return;
        }
        (
            state.is_amending(),
            state
                .amendable()
                .and_then(|amendable| amendable.consequence.as_ref().ok().cloned()),
        )
    };
    let message = compose_message(&commit.subject.peek(), &commit.description.peek());
    if amending {
        if let Some(consequence) = consequence.filter(|c| c.needs_force_push()) {
            open_amend_dialog(view, submit, &consequence, message, false);
        }
        return;
    }
    if let Some(submit) = submit.as_deref() {
        ask_commit(view, submit, message, false);
    }
}

fn ask_commit(view: View, submit: &dyn Fn(Request), message: String, skip_hooks: bool) {
    let mut writes = view.writes;
    let id = local_writes::ask(
        &mut writes.write(),
        submit,
        LocalWrite::Commit {
            message: message.clone(),
            skip_hooks,
        },
    );
    let mut state = view.local.commit.state;
    state.write().commit_asked(AskedCommit {
        id,
        amend: false,
        message,
        skip_hooks,
        confirmed_with: None,
    });
}

/// The amend button confirmed in place (R10.6): the amend asked with the token the
/// confirmation surface built and the draft as it stands.
pub fn amend_confirmed(token: Confirmed, view: View, submit: Option<&dyn Fn(Request)>) {
    let commit = view.local.commit;
    if commit.state.peek().asked().is_some() {
        return;
    }
    let message = compose_message(&commit.subject.peek(), &commit.description.peek());
    if let Some(submit) = submit {
        ask_amend(view, submit, token, message, false);
    }
}

fn ask_amend(
    view: View,
    submit: &dyn Fn(Request),
    confirmed: Confirmed,
    message: String,
    skip_hooks: bool,
) -> OperationId {
    let confirmed_with = Some(Rc::new(confirmed.consequence().clone()));
    let mut writes = view.writes;
    let id = local_writes::ask(
        &mut writes.write(),
        submit,
        LocalWrite::Amend {
            confirmed,
            message: message.clone(),
            skip_hooks,
        },
    );
    let mut state = view.local.commit.state;
    state.write().commit_asked(AskedCommit {
        id,
        amend: true,
        message,
        skip_hooks,
        confirmed_with,
    });
    id
}

/// The confirmation dialog over the window for an amend: its words and its button the
/// consequence's, its token asking the amend of `message`.
fn open_amend_dialog(
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
    consequence: &Consequence,
    message: String,
    skip_hooks: bool,
) {
    let mut confirming = view.confirming;
    confirming.set(Some(Confirming::new(
        AMEND_TITLE,
        consequence.clone(),
        move |token| {
            if let Some(submit) = submit.as_deref() {
                ask_amend(view, submit, token, message.clone(), skip_hooks);
            }
        },
    )));
}

/// Cancel (R10.4, R4.3): the running commit, never one still queued.
pub fn cancel(view: View, submit: Option<&dyn Fn(Request)>) {
    let asked = view.local.commit.state.peek().asked().map(|asked| asked.id);
    let running = view.writes.peek().running().map(|running| running.id);
    if let (Some(asked), Some(submit)) = (asked.filter(|id| Some(*id) == running), submit) {
        submit(Request::CancelWrite { id: asked });
    }
}

/// The recent message `at`, from the menu, fills both fields (R10.2).
pub fn recall(at: usize, view: View) {
    let commit = view.local.commit;
    let mut state = commit.state;
    let message = state.write().recall(at);
    if let Some(message) = message {
        fill(view, &message);
    }
}

/// A bare ↑ or ↓ in the subject (R10.2): taken only while the subject is empty or holds the
/// message last recalled — ↑ to the next older, ↓ to the next newer, and ↓ from the newest back
/// to an empty draft. Anything else is the editor's.
pub fn recall_step(step: RecallStep, view: View) -> bool {
    let commit = view.local.commit;
    let (recalled, len, holds_recalled) = {
        let state = commit.state.peek();
        let subject = commit.subject.peek();
        let recalled = state.recalled();
        let holds = recalled
            .and_then(|at| state.recent().get(at))
            .is_some_and(|message| split_message(message).0 == *subject);
        (recalled.filter(|_| holds), state.recent().len(), holds)
    };
    let empty = commit.subject.peek().is_empty() && commit.description.peek().is_empty();
    if !(holds_recalled || (empty && recalled.is_none())) {
        return false;
    }
    let next = match (step, recalled) {
        (RecallStep::Older, None) => 0,
        (RecallStep::Older, Some(at)) => at + 1,
        (RecallStep::Newer, None) => return false,
        (RecallStep::Newer, Some(0)) => {
            let mut state = commit.state;
            state.write().forget_recall();
            fill(view, "");
            return true;
        }
        (RecallStep::Newer, Some(at)) => at - 1,
    };
    if next >= len {
        // At the oldest: the step is taken, and nothing moves.
        return recalled.is_some();
    }
    recall(next, view);
    true
}

/// `message` split into the two fields.
fn fill(view: View, message: &str) {
    let (subject, description) = split_message(message);
    let (mut typed, mut body) = (view.local.commit.subject, view.local.commit.description);
    typed.set(subject);
    body.set(description);
}

fn draft_empty(view: View) -> bool {
    let commit = view.local.commit;
    commit.subject.peek().is_empty() && commit.description.peek().is_empty()
}

/// What the box reads has arrived (R10.2, R10.8): kept, and git's `MERGE_MSG` filling an empty
/// draft as a merge begins.
pub fn reads_arrived(reads: CommitReads, view: View) {
    let empty = draft_empty(view);
    let mut state = view.local.commit.state;
    let merge = state.write().reads_arrived(reads, empty);
    if let Some(message) = merge {
        fill(view, &message);
    }
}

/// What amending would replace has arrived for `status` (R10.3, R10.6): kept only while Amend is
/// ticked — `HEAD`'s message filling an empty draft once, the amend's lists drawn, or the
/// status's own when amend's staged list could not be read — and handed to a worker to free
/// otherwise.
pub fn amend_arrived(
    status: std::sync::Arc<cairn_model::LocalChanges>,
    read: AmendRead,
    view: View,
    submit: &dyn Fn(Request),
) {
    let AmendRead {
        consequence,
        message,
        lists,
    } = read;
    if !view.local.commit.state.peek().is_amending() {
        submit(Request::Retire(Retired::amending(status, lists.ok())));
        return;
    }
    let (lists, failed) = match lists {
        Ok(lists) => (Some(lists), None),
        Err(why) => (None, Some(why)),
    };
    let empty = draft_empty(view);
    let mut state = view.local.commit.state;
    let filling = state
        .write()
        .amend_arrived(consequence, message, failed, empty);
    if let Some(message) = filling {
        fill(view, &message);
    }
    let mut local = view.local.state;
    let requests = match lists {
        Some(lists) => {
            let mut requests = local.write().status_arrived(lists);
            requests.push(Request::Retire(Retired::status(status)));
            requests
        }
        // The status's own lists, if the amend's were drawn.
        None if drawn_changes(&local.peek()).staged_against() != StagedAgainst::Head => {
            local.write().status_arrived(status)
        }
        None => vec![Request::Retire(Retired::status(status))],
    };
    for request in requests {
        submit(request);
    }
}

/// A status arrived while Amend is ticked: what an amend would replace is asked over it, rather
/// than the status drawn as it is. `false` when Amend is not ticked.
pub fn status_arrived_amending(
    status: &std::sync::Arc<cairn_model::LocalChanges>,
    view: View,
    submit: &dyn Fn(Request),
) -> bool {
    let mut state = view.local.commit.state;
    if !state.peek().is_amending() {
        return false;
    }
    state.write().amend_asked();
    submit(Request::Amending {
        status: std::sync::Arc::clone(status),
    });
    true
}

/// A refresh's refs arrived: what the box reads is asked again while Local Changes is shown.
pub fn refs_arrived(view: View, submit: &dyn Fn(Request)) {
    if *view.sidebar.main.peek() == MainView::LocalChanges {
        submit(Request::CommitReads);
    }
}

/// A write started: this box's commit's elapsed time starts.
pub fn write_started(id: OperationId, view: View) {
    let mut state = view.local.commit.state;
    state.write().started_now(id);
}

/// A line of a write's output: kept when it is this box's commit's.
pub fn write_output(id: OperationId, line: &str, view: View) {
    let mut state = view.local.commit.state;
    if state.peek().asked().is_some_and(|asked| asked.id == id) {
        state.write().output_arrived(id, line);
    }
}

/// A write ended: when it is this box's commit — made, the draft cleared and Amend unticked
/// (R10.3); failed by git, the Git Error dialog opened (R10.5); anything else said under the
/// lists, the draft kept (R10.7).
pub fn write_ended(id: OperationId, ending: &WriteEnding, view: View, submit: &dyn Fn(Request)) {
    let mut state = view.local.commit.state;
    let Some(asked) = state.write().ended(id) else {
        return;
    };
    match ending {
        WriteEnding::Done(_) => {
            let was_amending = state.peek().is_amending();
            state.write().made();
            // Only the draft the commit took: one typed while it ran is the next message.
            let commit = view.local.commit;
            let taken = compose_message(&commit.subject.peek(), &commit.description.peek())
                == asked.message;
            if taken {
                fill(view, "");
            }
            if was_amending {
                submit(Request::StopAmending);
            }
        }
        WriteEnding::Failed {
            command: Some(command),
            output,
            ..
        } => {
            let command = command.clone();
            state.write().failed(asked, command, output);
        }
        WriteEnding::Failed { command: None, .. }
        | WriteEnding::Stale { .. }
        | WriteEnding::Refused { .. }
        | WriteEnding::MayHaveTakenEffect { .. }
        | WriteEnding::Incomplete { .. }
        | WriteEnding::NotRun { .. } => {}
    }
}

/// The Git Error dialog, while one is open (R10.5): a failed amend's skip is the commit box's
/// `AmendSkip`, confirming the amend's own consequence again; a commit's asks the same message
/// with its hooks skipped.
pub fn git_error(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Option<Element> {
    let error = view.local.commit.state.read().error().cloned()?;
    let dialog = GitErrorDialog::new(error.serial, error.command.clone(), error.lines.clone())
        .key(DiffKey::U64(error.serial))
        .skip(error.skip)
        .on_close(move |()| {
            let mut state = view.local.commit.state;
            let _ = state.write().close_error();
        });
    let dialog = match error.failed.confirmed_with.clone().filter(|_| error.skip) {
        Some(consequence) => {
            let message = error.failed.message.clone();
            dialog.skip_amend(consequence, move |token: Confirmed| {
                skip_amend_hooks(token, message.clone(), view, submit.as_deref());
            })
        }
        None => dialog.on_skip(move |()| skip_hooks(view, submit.clone())),
    };
    Some(dialog.into())
}

/// A failed commit's skip (R10.5): the same message asked again with its hooks skipped —
/// `--no-verify` for that one commit alone.
pub fn skip_hooks(view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    let mut state = view.local.commit.state;
    let Some(error) = state.write().close_error() else {
        return;
    };
    if let (false, Some(submit)) = (error.failed.amend, submit.as_deref()) {
        ask_commit(view, submit, error.failed.message, true);
    }
}

/// A failed amend's skip (the user's decision, 2026-10-09): one press amends at once without
/// hooks, under the token `AmendSkip` built from the consequence the amend was confirmed with.
fn skip_amend_hooks(
    token: Confirmed,
    message: String,
    view: View,
    submit: Option<&dyn Fn(Request)>,
) {
    let mut state = view.local.commit.state;
    let _ = state.write().close_error();
    if let Some(submit) = submit {
        ask_amend(view, submit, token, message, true);
    }
}
