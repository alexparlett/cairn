//! The commit box under Local Changes' diff, and what its presses do (staging-and-commit R10):
//! `cairn_ui::CommitBox` drawn from `commit_box_state`, each press asking the local lane — a
//! commit, an amend at the press, an amend under the dialog's confirmation, a cancel of the commit
//! running — and each update the worker sends for the box applied here (`session::apply` hands
//! them on).
//!
//! **What is asked, and where.** As the box is shown, and as each refresh's refs arrive while it
//! is, the box's reads are asked (`Request::CommitReads`: the operation in progress, the message
//! git prepared for the commit that concludes it, cleaned, and the recent messages), on the
//! local lane after the writes asked before them. While Amend is ticked, Local Changes' lists ask
//! amend's staged list over each status (`LocalChangesState`'s overlay, `Request::Amending`),
//! which brings `HEAD`'s message with it; what an amend costs is never read before the press
//! (rule 3 of the redesign, C14). Nothing here waits: each is a submit.
//!
//! **One button, one press** (R10.6, C1, C6): `Commit N Files`, or "Amend Last Commit" while
//! Amend is ticked; the commit chord does exactly what the button does. Amend asks
//! `LocalWrite::AmendAtPress`, whose job reads the cost and runs an amend git logs and no remote
//! has at once, with no token; any other ends `WriteEnding::NeedsConfirming`, and the
//! confirmation dialog, "Amend Commit", opens on it — its Cancel returning to the box unchanged,
//! its Amend asking `LocalWrite::Amend` with the token the dialog built, the one confirmation
//! surface for it. A failed commit or amend opens the Git Error dialog, whose skip asks the same
//! message again with `--no-verify`, once: a confirmed amend's with the token its failure handed
//! back (`GitError::take_unspent`, R1.1's option (a)), so nothing asks twice.

use std::rc::Rc;

use cairn_model::{ChangeList, HeadState, ScrubbedLines};
use cairn_ui::accelerators::RecallStep;
use cairn_ui::{AMEND_TITLE, Busy, CommitBox, CommitButton, GitErrorDialog, MainView};
use freya::prelude::*;

use crate::commit_box_state::{AskedCommit, compose_message, split_message};
use crate::confirming::Confirming;
use crate::local_changes_state::drawn_changes;
use crate::local_writes;
use crate::window::View;
use crate::worker::{
    AmendRead, CommitReads, LocalWrite, OperationId, Request, Unspent, WriteEnding,
};

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

/// What a commit asked is doing, drawn in the button's place (R10.4, C5): queued behind a write
/// ahead of it — the one running, or one asked before it — in the one form every queued write
/// uses; running; or cancelled until git is reaped. Asked with no write ahead and not yet
/// started (the lane busy with a read), it is drawn by its verb alone, no Cancel offered until
/// git runs (phase 18's QA item 6).
fn busy_of(asked: &AskedCommit, state: &crate::commit_box_state::CommitBox, view: View) -> Busy {
    if state.is_cancelling() {
        return Busy::Cancelling;
    }
    let verb = if asked.amend {
        "Amending"
    } else {
        "Committing"
    };
    let writes = view.writes.read();
    let running = writes.running();
    if running.is_some_and(|running| running.id == asked.id) {
        return Busy::Running {
            verb,
            started: state.started(),
        };
    }
    let ahead = running.or_else(|| {
        writes
            .queued()
            .take_while(|queued| queued.id != asked.id)
            .next()
    });
    match ahead {
        Some(ahead) => Busy::Waiting(format!("Waiting for {}…", ahead.name)),
        None => Busy::Running {
            verb,
            started: None,
        },
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
        use_amend_confirmation(view, self.submit.clone());

        let state = commit.state.read();
        let refreshed = view.refreshed.read();
        let local = view.local.state.read();
        let operation = state.operation().copied();
        let stopped = operation.is_some_and(|operation| operation.refuses_commit());
        let concluding = operation.is_some_and(|operation| operation.concluded_by_commit());
        let head = refreshed.refs().map(|refs| &refs.head);
        let born = head.is_some_and(|head| !matches!(head, HeadState::Unborn(_)));
        let detached = head.is_some_and(|head| matches!(head, HeadState::Detached(_)));
        let amend_enabled = born && operation.is_none();
        // One line at most (R10.8, rule 6): the operation in progress; or, while Amend is ticked
        // and its staged list could not be read, that the lists show the status's own (R10.3);
        // or an unborn branch, where Amend is greyed; or a detached `HEAD`.
        let line = match (operation, local.amend_unread()) {
            // The detached line wins: it says what the press will do (the coordinator's ruling
            // under rule 4, phase 18's QA item 8).
            (None, Some(why)) if state.is_amending() && !detached => Some(format!(
                "Amend's staged list couldn't be read, so the lists show what is staged against \
                 HEAD: {why}"
            )),
            // Amend greyed on an unborn branch, its reason beside it (rule 4).
            (None, _) if head.is_some() && !born => Some(cairn_ui::UNBORN_LINE.to_owned()),
            (operation, _) => cairn_ui::operation_line(operation.as_ref(), detached),
        };

        let busy = state.asked().map(|asked| busy_of(asked, &state, view));
        let (button, enabled) = if state.is_amending() {
            (CommitButton::Amend, true)
        } else {
            let files = drawn_changes(&local).len(ChangeList::Staged);
            (CommitButton::Commit { files }, files > 0 || concluding)
        };
        let subject_given = !commit.subject.read().trim().is_empty();
        let ready = enabled && subject_given && state.asked().is_none() && !stopped;
        let recent = state.recent().clone();
        let amending = state.is_amending();
        drop((state, refreshed, local));

        let submit = self.submit.clone();
        let toggling = submit.clone();
        let pressing = submit.clone();
        let cancelling = submit.clone();
        CommitBox::new(commit.subject, commit.description)
            .amend(amending, amend_enabled)
            .button(button, ready)
            .busy(busy)
            .stopped(stopped)
            .line(line)
            .recent(recent)
            .on_amend(move |on: bool| toggle_amend(on, view, toggling.as_deref()))
            .on_commit(move |()| pressed(view, pressing.clone()))
            .on_cancel(move |()| cancel(view, cancelling.as_deref()))
            .on_edit(move |()| {
                let mut state = view.local.commit.state;
                if !state.peek().is_touched() || state.peek().recalled().is_some() {
                    state.write().touch();
                }
            })
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

/// Amend ticked or unticked (R10.3): the draft set aside or put back exactly, and amend's lists
/// asked over the newest status, or the status's own drawn again.
pub fn toggle_amend(on: bool, view: View, submit: Option<&dyn Fn(Request)>) {
    let commit = view.local.commit;
    let mut state = commit.state;
    let latest = view.refreshed.peek().local_changes().cloned();
    let mut local = view.local.state;
    if on {
        if state.peek().is_amending() {
            return;
        }
        let (subject, description) = (
            commit.subject.peek().clone(),
            commit.description.peek().clone(),
        );
        state.write().tick_amend(&subject, &description);
        let requests = local.write().amend_ticked(latest);
        submit_all(requests, submit);
        return;
    }
    let Some((subject, description)) = state.write().untick_amend() else {
        return;
    };
    let (mut typed, mut body) = (commit.subject, commit.description);
    typed.set(subject);
    body.set(description);
    let requests = local.write().amend_unticked(latest);
    submit_all(requests, submit);
    crate::diff_actions::show_primary(view, submit, false);
}

/// The button, or the commit chord heard in a field (R10.1, R10.6, R7.3): commits what is
/// staged, or — Amend ticked — asks the amend at the press, whose job reads what it costs and
/// runs it, or ends asking for the dialog.
pub fn pressed(view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    let commit = view.local.commit;
    let amending = {
        let state = commit.state.peek();
        if state.asked().is_some() {
            return;
        }
        state.is_amending()
    };
    let message = compose_message(&commit.subject.peek(), &commit.description.peek());
    if let Some(submit) = submit.as_deref() {
        let write = if amending {
            LocalWrite::AmendAtPress {
                message: message.clone(),
                skip_hooks: false,
            }
        } else {
            LocalWrite::Commit {
                message: message.clone(),
                skip_hooks: false,
                files: staged_files(view),
            }
        };
        ask(view, submit, write, amending, message, false);
    }
}

/// How many files a commit asked now commits — what is staged, as the button counts it — for
/// its name ("Commit 2 files", Fork's row).
fn staged_files(view: View) -> usize {
    drawn_changes(&view.local.state.peek()).len(ChangeList::Staged)
}

/// `write` asked, and drawn waiting until it starts.
fn ask(
    view: View,
    submit: &dyn Fn(Request),
    write: LocalWrite,
    amend: bool,
    message: String,
    skip_hooks: bool,
) {
    let mut writes = view.writes;
    let id = local_writes::ask(&mut writes.write(), submit, write);
    let mut state = view.local.commit.state;
    state.write().commit_asked(AskedCommit {
        id,
        amend,
        message,
        skip_hooks,
    });
    let mut output = view.local.commit.output;
    output.write().asked(id);
}

/// The confirmation dialog over the window for an amend at the press that must be confirmed — a
/// remote has `HEAD`, or git keeps no reflog (R10.6, C1) — opened as the box renders, which holds
/// the submitter its answer needs: titled "Amend Commit", in the consequence's fixed sentences;
/// its Amend asks the amend of the pressed message under the token it built, its Cancel leaves
/// the box as it was.
fn use_amend_confirmation(view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    use_side_effect(move || {
        // Read so the effect runs again once a confirmation already open is answered: one open
        // is never replaced, its acknowledgement then another prompt's (phase 18's QA item 1).
        let open = view.confirming.read().is_some();
        if open || !view.local.commit.state.read().is_confirming() {
            return;
        }
        let mut state = view.local.commit.state;
        let Some((consequence, asked)) = state.write().take_confirming() else {
            return;
        };
        let Some(submit) = submit.clone() else {
            return;
        };
        let mut confirming = view.confirming;
        confirming.set(Some(Confirming::new(
            AMEND_TITLE,
            consequence,
            move |token| {
                let write = LocalWrite::Amend {
                    confirmed: token,
                    message: asked.message.clone(),
                    skip_hooks: asked.skip_hooks,
                };
                ask(
                    view,
                    &*submit,
                    write,
                    true,
                    asked.message.clone(),
                    asked.skip_hooks,
                );
            },
        )));
    });
}

/// Cancel (R10.4, R4.3): the running commit, never one still queued; "Cancelling…" until git is
/// reaped.
pub fn cancel(view: View, submit: Option<&dyn Fn(Request)>) {
    let asked = view.local.commit.state.peek().asked().map(|asked| asked.id);
    let running = view.writes.peek().running().map(|running| running.id);
    if let (Some(asked), Some(submit)) = (asked.filter(|id| Some(*id) == running), submit) {
        let mut state = view.local.commit.state;
        state.write().cancel_asked();
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

/// What the box reads has arrived (R10.2, R10.8): kept, and the message git prepared, cleaned,
/// filling an empty, untouched draft while a merge, a single cherry-pick or a single revert is
/// in progress (C7).
pub fn reads_arrived(reads: CommitReads, view: View) {
    let empty = draft_empty(view);
    let mut state = view.local.commit.state;
    let merge = state.write().reads_arrived(reads, empty);
    if let Some(message) = merge {
        fill(view, &message);
    }
}

/// Amend's lists arrived for `status` (R10.3): drawn by Local Changes' lists while Amend is
/// ticked — or the status's own, where amend's staged list could not be read — and `HEAD`'s
/// message filling an empty, untouched draft (C7); handed to a worker to free otherwise.
pub fn amend_arrived(
    status: std::sync::Arc<cairn_model::LocalChanges>,
    read: AmendRead,
    view: View,
    submit: &dyn Fn(Request),
) {
    let AmendRead { message, lists } = read;
    if let Ok(message) = message {
        let empty = draft_empty(view);
        let filling = view
            .local
            .commit
            .state
            .peek()
            .amend_message_arrived(message, empty);
        if let Some(message) = filling {
            fill(view, &message);
        }
    }
    let mut local = view.local.state;
    let requests = local.write().amend_arrived(status, lists);
    for request in requests {
        submit(request);
    }
    crate::diff_actions::show_primary(view, Some(submit), false);
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

/// Lines of a write's output: kept when they are this box's commit's, in a state no render
/// reads (`CommitBoxView::output`), so they draw nothing again.
pub fn write_output(id: OperationId, lines: &ScrubbedLines, view: View) {
    let mut output = view.local.commit.output;
    output.write().arrived(id, lines);
}

/// A write ended: when it is this box's commit — made, the draft it took cleared and Amend
/// unticked (R10.3); failed by git, the Git Error dialog opened (R10.5), with the token a
/// confirmed amend's failure handed back; an amend at the press that must be confirmed, the
/// dialog opened on its consequence (R10.6); anything else, the draft kept (R10.7) — a cancel
/// opens nothing (rule 4).
pub fn write_ended(
    id: OperationId,
    ending: &WriteEnding,
    unspent: &mut Unspent,
    view: View,
    submit: &dyn Fn(Request),
) -> bool {
    let mut state = view.local.commit.state;
    let Some(asked) = state.write().ended(id) else {
        return false;
    };
    match ending {
        WriteEnding::Done(_) => {
            let was_amending = state.peek().is_amending();
            // Only the draft the commit took: one typed while it ran is the next message.
            let commit = view.local.commit;
            let taken = compose_message(&commit.subject.peek(), &commit.description.peek())
                == asked.message;
            state.write().made(taken);
            if taken {
                fill(view, "");
            }
            if was_amending {
                let latest = view.refreshed.peek().local_changes().cloned();
                let mut local = view.local.state;
                let requests = local.write().amend_unticked(latest);
                for request in requests {
                    submit(request);
                }
            }
            false
        }
        // git's failure: the box's own Git Error, whose skip runs this commit again (R10.5).
        WriteEnding::Failed(failure) if failure.command.is_some() => {
            let mut output = view.local.commit.output;
            let streamed = output.write().take();
            state
                .write()
                .failed(asked, failure, unspent.take(), streamed);
            true
        }
        WriteEnding::NeedsConfirming { consequence } => {
            state.write().needs_confirming(consequence.clone(), asked);
            true
        }
        // A refusal, a stale amend, a failure beneath git: "Couldn't <name>", the window's
        // endings' (R14.3); a cancel nothing (R14.5).
        WriteEnding::Failed(_)
        | WriteEnding::Stale { .. }
        | WriteEnding::Refused { .. }
        | WriteEnding::Cancelled { .. }
        | WriteEnding::Incomplete { .. }
        | WriteEnding::NotRun { .. } => false,
    }
}

/// The Git Error dialog, while one is open (R10.5): its skip asks the failed commit or amend
/// again with its hooks skipped; Close lets it go, and with it any token it kept.
pub fn git_error(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Option<Element> {
    let (serial, command, lines, skip, sentence, index_lock) = {
        let state = view.local.commit.state.read();
        let error = state.error()?;
        (
            error.serial,
            error.command.clone(),
            error.lines.clone(),
            error.skip,
            error.sentence.clone(),
            error.index_lock,
        )
    };
    let unlocking = submit.clone();
    let dialog = GitErrorDialog::new(serial, command, lines)
        .sentence(sentence)
        .key(DiffKey::U64(serial))
        .skip(skip)
        .remove_lock(index_lock)
        .on_close(move |()| {
            let mut state = view.local.commit.state;
            let _ = state.write().close_error();
        })
        .on_remove_lock(move |()| {
            let mut state = view.local.commit.state;
            let _ = state.write().close_error();
            crate::leftover_lock::ask(view, unlocking.as_deref());
        })
        .on_skip(move |()| skip_hooks(view, submit.clone()));
    Some(dialog.into())
}

/// The Git Error dialog's skip (R10.5, C4): the same message asked again with `--no-verify` for
/// that one commit or amend — a confirmed amend's under the token its failure handed back,
/// asking nothing again; an amend at the press's at the press again, which reads its cost anew.
pub fn skip_hooks(view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    let mut state = view.local.commit.state;
    let Some(mut error) = state.write().close_error() else {
        return;
    };
    let Some(submit) = submit.as_deref() else {
        return;
    };
    let failed = error.failed.clone();
    let write = match (failed.amend, error.take_unspent()) {
        (false, _) => LocalWrite::Commit {
            message: failed.message.clone(),
            skip_hooks: true,
            files: staged_files(view),
        },
        (true, Some(confirmed)) => LocalWrite::Amend {
            confirmed,
            message: failed.message.clone(),
            skip_hooks: true,
        },
        (true, None) => LocalWrite::AmendAtPress {
            message: failed.message.clone(),
            skip_hooks: true,
        },
    };
    ask(view, submit, write, failed.amend, failed.message, true);
}
