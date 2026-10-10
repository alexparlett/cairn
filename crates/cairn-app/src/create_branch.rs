//! Create Branch as the window keeps it (`docs/prd/staging-and-commit.md` R11.3; the user's
//! decisions of 2026-10-09 and 2026-10-10): Fork's dialog, opened by "New Branch…" on any
//! commit row, or at `HEAD` by Fork's New Branch chord (`Action::NewBranch`).
//!
//! - **The name** is the window's text; each change asks the engine whether git takes it and
//!   whether a branch has it (`Request::CheckBranchName`, the branch-name lane, the latest ask
//!   superseding the one before), and the button is enabled only for the answer to the text
//!   shown. The engine's answer is typed, and the dialog words it (`cairn_ui::name_refusal`).
//! - **"Check out after create"** is sticky for the session (decision 1); kept across restarts
//!   once Cairn has a settings store (a later roadmap item). "Local changes:" is drawn while it
//!   is ticked and a tracked path has a change (`WorkingTreeStatus::has_tracked_changes`); each
//!   opening starts on "Don't change", so "Discard" is never the remembered choice.
//! - **The writes**: unticked, `LocalWrite::CreateBranch`; ticked with "Don't change",
//!   `LocalWrite::CreateBranchAndCheckout`; ticked with "Discard", the dialog's press is the
//!   confirmation (B2): while Discard is chosen the engine is asked what it would be confirmed
//!   as for the name shown (`Request::CheckoutConsequence`, again as the name changes), the
//!   button is enabled once it has answered, and the press builds the token in the dialog from
//!   that `Consequence` — `LocalWrite::CreateBranchDiscarding`, no second dialog. An operation in
//!   progress is said in the dialog's refusal row (`cairn_ui::discard_refusal`), the button
//!   disabled.
//! - **The dialog stays open** while its write is asked, and closes when it is done. A failure
//!   — git refusing, or a discard whose `HEAD`, commit or name moved since the press — opens
//!   Fork's Git Error dialog over it with its words, the dialog beneath kept as it was left, so
//!   Close returns to it. While either is up the window's keys are inert.

use std::rc::Rc;

use cairn_model::{BranchName, Confirmed, Consequence, HeadState, Oid, Ref, RowContent};
use cairn_ui::{CreateBranchDialog, GitErrorDialog, LocalChoice};
use freya::prelude::*;

use crate::local_writes::{self, LocalWrites};
use crate::window::View;
use crate::worker::{CheckoutRefused, LocalWrite, OperationId, Request, WriteEnding};

/// The dialog's handles, made once in the window's root.
#[derive(Clone, Copy)]
pub struct CreateBranchView {
    pub state: State<CreateBranchState>,
    /// The name field's text.
    pub name: State<String>,
}

impl CreateBranchView {
    /// Each handle made by its hook, in the window's root.
    pub fn used() -> Self {
        Self {
            state: use_state(CreateBranchState::default),
            name: use_state(String::new),
        }
    }

    /// Each handle made outside a component, for a test's view.
    #[cfg(test)]
    pub fn created() -> Self {
        Self {
            state: State::create(CreateBranchState::default()),
            name: State::create(String::new()),
        }
    }
}

/// Create Branch's state for the session.
#[derive(Debug, Default)]
pub struct CreateBranchState {
    open: Option<Opened>,
    /// "Check out after create", sticky for the session.
    checkout: bool,
    /// The write asked by the last press, while it has not ended.
    asked: Option<OperationId>,
    /// A failure to draw in the Git Error dialog, over the dialog when it is still open.
    error: Option<BranchError>,
    next_serial: u64,
}

/// The dialog open.
#[derive(Debug)]
struct Opened {
    serial: u64,
    at: Oid,
    subject: String,
    local: LocalChoice,
    /// The engine's answer for a name, and the name it answered.
    checked: Option<(String, Result<BranchName, String>)>,
    /// Discard's consequence asked and not yet answered: the ask, and the name it is for.
    discard_asked: Option<(OperationId, String)>,
    /// Discard's answer, and the name it is for: what the press confirms, or why it cannot.
    discarding: Option<(String, Result<Consequence, CheckoutRefused>)>,
    /// The write this opening's press asked, while it has not ended: the button is disabled.
    writing: Option<OperationId>,
}

/// What the Git Error dialog draws for a create that failed.
#[derive(Debug, Clone)]
pub struct BranchError {
    pub serial: u64,
    pub command: String,
    pub lines: Rc<Vec<String>>,
}

impl CreateBranchState {
    /// The failure the Git Error dialog draws.
    pub fn error(&self) -> Option<&BranchError> {
        self.error.as_ref()
    }

    /// Whether the dialog, or a create's Git Error dialog, is up: the window's keys are inert
    /// (R11.3, C28).
    pub fn is_up(&self) -> bool {
        self.open.is_some() || self.error.is_some()
    }

    fn serial(&mut self) -> u64 {
        self.next_serial += 1;
        self.next_serial
    }
}

/// "New Branch…" chosen on the row of `at`, whose subject is `subject`: the dialog opens, its
/// name empty and "Don't change" chosen.
pub fn open(view: View, at: Oid, subject: String) {
    let (mut state, mut name) = (view.branch.state, view.branch.name);
    {
        let mut state = state.write();
        let serial = state.serial();
        state.open = Some(Opened {
            serial,
            at,
            subject,
            local: LocalChoice::Keep,
            checked: None,
            discard_asked: None,
            discarding: None,
            writing: None,
        });
    }
    name.set(String::new());
}

/// Fork's New Branch chord: the dialog opens at `HEAD`'s commit as the last refresh read it —
/// a branch's tip or a detached `HEAD` — its subject the loaded row's, found among the rows
/// refs label (`History::labelled_position`, never every row), or none where it is not loaded.
/// Nothing opens before the refs are read, on an unborn `HEAD`, or while the dialog or a
/// create's Git Error dialog is up: a second press would reset the name typed.
pub fn open_at_head(view: View) {
    if view.branch.state.peek().is_up() {
        return;
    }
    let at = {
        let refreshed = view.refreshed.peek();
        let Some(refs) = refreshed.refs() else {
            return;
        };
        match &refs.head {
            HeadState::Branch(name) => refs.find(name).and_then(Ref::commit_id),
            HeadState::Detached(at) => Some(*at),
            HeadState::Unborn(_) => None,
        }
    };
    let Some(at) = at else {
        return;
    };
    let subject = {
        let rows = view.rows.peek();
        rows.labelled_position(at)
            .and_then(|index| rows.row(index))
            // No wildcard arm: `HEAD`'s row is a commit's, but every kind is named.
            .map(|row| match row.content() {
                RowContent::Commit(commit) => commit.summary,
                RowContent::Stash(stash) => stash.message,
            })
            .unwrap_or_default()
    };
    open(view, at, subject);
}

/// The name's text changed: it is asked about, and, while Discard is chosen, what Discard of it
/// would be confirmed as.
pub fn name_changed(view: View, submit: Option<&dyn Fn(Request)>) {
    let typed = view.branch.name.peek().clone();
    if let (false, Some(submit)) = (typed.is_empty(), submit) {
        submit(Request::CheckBranchName {
            name: typed.clone(),
        });
    }
    ask_discard(view, submit);
}

/// Whether "Local changes:" is drawn: the box ticked and a tracked path changed.
fn shows_local(view: View, checkout: bool) -> bool {
    checkout
        && view
            .refreshed
            .peek()
            .status()
            .is_some_and(|status| status.has_tracked_changes())
}

/// Whether the press would discard: "Local changes:" drawn with Discard chosen.
fn discards(view: View, state: &CreateBranchState, opened: &Opened) -> bool {
    opened.local == LocalChoice::Discard && shows_local(view, state.checkout)
}

/// While Discard is chosen, asks what it would be confirmed as for the name shown — the newer
/// ask superseding the older in its lane — so the press has the engine's `Consequence` to build
/// its token from, and an operation in progress is said before anything is pressed.
fn ask_discard(view: View, submit: Option<&dyn Fn(Request)>) {
    let Some(submit) = submit else {
        return;
    };
    let typed = view.branch.name.peek().clone();
    let mut state = view.branch.state;
    let at = {
        let state = state.peek();
        let Some(opened) = state.open.as_ref() else {
            return;
        };
        if typed.is_empty() || !discards(view, &state, opened) {
            return;
        }
        opened.at
    };
    let asked = OperationId::next();
    if let Some(opened) = state.write().open.as_mut() {
        opened.discard_asked = Some((asked, typed.clone()));
    }
    submit(Request::CheckoutConsequence {
        asked,
        name: typed,
        at,
    });
}

/// The engine's answer for `name`: kept while the dialog is open, for the text it answers.
pub fn name_checked(view: View, name: String, outcome: Result<BranchName, String>) {
    let mut state = view.branch.state;
    if let Some(opened) = state.write().open.as_mut() {
        opened.checked = Some((name, outcome));
    }
}

/// What Discard would be confirmed as, answering the ask `asked`: kept for the name it was
/// asked for, while it is the latest ask.
pub fn consequence_arrived(
    view: View,
    asked: OperationId,
    outcome: Result<Consequence, CheckoutRefused>,
) {
    let mut state = view.branch.state;
    let mut state = state.write();
    let Some(opened) = state.open.as_mut().filter(|opened| {
        opened
            .discard_asked
            .as_ref()
            .is_some_and(|(latest, _)| *latest == asked)
    }) else {
        return;
    };
    if let Some((_, name)) = opened.discard_asked.take() {
        opened.discarding = Some((name, outcome));
    }
}

/// Discard's answer for `typed`, when one is in.
fn discarding_for<'a>(
    opened: &'a Opened,
    typed: &str,
) -> Option<&'a Result<Consequence, CheckoutRefused>> {
    opened
        .discarding
        .as_ref()
        .filter(|(name, _)| name == typed)
        .map(|(_, answer)| answer)
}

/// Whether the dialog may create `typed`: the engine said git takes it and no branch has it,
/// no write of this opening's is running, and — when the press discards — the engine has said
/// what it would be confirmed as.
fn ready(opened: &Opened, typed: &str, discarding: bool) -> bool {
    !typed.is_empty()
        && opened.writing.is_none()
        && matches!(&opened.checked, Some((name, Ok(BranchName::Free))) if name == typed)
        && (!discarding || matches!(discarding_for(opened, typed), Some(Ok(_))))
}

/// The consequence Discard's press confirms for `typed`.
fn consequence_for(opened: &Opened, typed: &str) -> Option<Consequence> {
    match discarding_for(opened, typed) {
        Some(Ok(consequence)) => Some(consequence.clone()),
        Some(Err(_)) | None => None,
    }
}

/// Why the dialog cannot create `typed`, said beside its buttons, worded by the dialog: the
/// name's refusal, or, with Discard chosen, why Discard cannot be confirmed.
fn refusal(opened: &Opened, typed: &str, discarding: bool) -> Option<String> {
    let name = match &opened.checked {
        Some((name, Ok(answer))) if name == typed => answer
            .refusal()
            .map(|why| cairn_ui::name_refusal(typed, why)),
        Some((name, Err(failed))) if name == typed => Some(failed.clone()),
        Some(_) | None => None,
    };
    name.or_else(|| match (discarding, discarding_for(opened, typed)) {
        (true, Some(Err(CheckoutRefused::InProgress(operation)))) => {
            Some(cairn_ui::discard_refusal(operation))
        }
        (true, Some(Err(CheckoutRefused::Failed(failed)))) => Some(failed.clone()),
        (true, Some(Ok(_)) | None) | (false, _) => None,
    })
}

/// What the check of `typed` waits behind, said beside the buttons (the user's decision D,
/// 2026-10-09): the local lane runs one thing at a time, so a name asked and not yet answered
/// waits for the write running, or the first queued. `None` once the answer for `typed` is in,
/// for an empty name, or while no write is asked.
fn waiting(opened: &Opened, typed: &str, writes: &LocalWrites) -> Option<String> {
    let answered = matches!(&opened.checked, Some((name, _)) if name == typed);
    if typed.is_empty() || answered {
        return None;
    }
    writes
        .running()
        .or_else(|| writes.queued().next())
        .map(|asked| format!("Waiting for {}…", asked.awaited))
}

/// The dialog's button, or Return in its field, with Discard not chosen: the create asked, the
/// dialog kept open until it ends.
pub fn create(view: View, submit: Option<&dyn Fn(Request)>) {
    let Some(submit) = submit else {
        return;
    };
    let typed = view.branch.name.peek().clone();
    let write = {
        let state = view.branch.state.peek();
        let Some(opened) = state.open.as_ref() else {
            return;
        };
        let discarding = discards(view, &state, opened);
        if discarding || !ready(opened, &typed, false) {
            return;
        }
        if state.checkout {
            LocalWrite::CreateBranchAndCheckout {
                name: typed,
                at: opened.at,
            }
        } else {
            LocalWrite::CreateBranch {
                name: typed,
                at: opened.at,
            }
        }
    };
    asked(view, submit, write);
}

/// The dialog's button, or Return in its field, with Discard chosen: the press built `token`
/// from the consequence the dialog was handed, and the discarding write is asked with it.
pub fn discard_confirmed(view: View, token: Confirmed, submit: Option<&dyn Fn(Request)>) {
    let Some(submit) = submit else {
        return;
    };
    if view
        .branch
        .state
        .peek()
        .open
        .as_ref()
        .is_none_or(|opened| opened.writing.is_some())
    {
        return;
    }
    asked(view, submit, LocalWrite::CreateBranchDiscarding(token));
}

/// `write` asked for the open dialog, which stays open until it ends.
fn asked(view: View, submit: &dyn Fn(Request), write: LocalWrite) {
    let mut writes = view.writes;
    let id = local_writes::ask(&mut writes.write(), submit, write);
    let mut state = view.branch.state;
    let mut state = state.write();
    state.asked = Some(id);
    if let Some(opened) = state.open.as_mut() {
        opened.writing = Some(id);
    }
}

/// "Check out after create" ticked or not: sticky for the session.
pub fn checkout_toggled(view: View, to: bool, submit: Option<&dyn Fn(Request)>) {
    let mut state = view.branch.state;
    state.write().checkout = to;
    ask_discard(view, submit);
}

/// A choice of "Local changes:" for this opening.
pub fn local_chosen(view: View, choice: LocalChoice, submit: Option<&dyn Fn(Request)>) {
    let mut state = view.branch.state;
    if let Some(opened) = state.write().open.as_mut() {
        opened.local = choice;
    }
    ask_discard(view, submit);
}

/// Cancel, Escape or a press outside: the dialog closes — but not while a create's Git Error
/// dialog is over it, which the press or the key belongs to.
pub fn cancel(view: View) {
    let mut state = view.branch.state;
    if state.peek().error.is_some() {
        return;
    }
    state.write().open = None;
}

/// A local write ended: when it is the dialog's, a success closes the dialog, and anything else
/// opens the Git Error dialog over it with its words, the dialog left as it was.
pub fn write_ended(view: View, id: OperationId, ending: &WriteEnding) {
    let mut state = view.branch.state;
    if state.peek().asked != Some(id) {
        return;
    }
    let mut state = state.write();
    state.asked = None;
    let ours = state
        .open
        .as_ref()
        .is_some_and(|opened| opened.writing == Some(id));
    if let Some(opened) = state.open.as_mut().filter(|_| ours) {
        opened.writing = None;
    }
    let (command, lines) = match ending {
        WriteEnding::Done(_) => {
            if ours {
                state.open = None;
            }
            return;
        }
        WriteEnding::Failed {
            message,
            command,
            output,
            ..
        } => {
            let lines = crate::shown_output::shown_lines(output);
            let lines = if lines.is_empty() {
                vec![message.clone()]
            } else {
                lines
            };
            (command.clone().unwrap_or_default(), lines)
        }
        WriteEnding::Stale { message, .. }
        | WriteEnding::Refused { message }
        | WriteEnding::Cancelled { message, .. }
        | WriteEnding::Incomplete { message, .. }
        | WriteEnding::NotRun { message } => (String::new(), vec![message.clone()]),
        WriteEnding::NeedsConfirming { consequence } => (String::new(), vec![consequence.prompt()]),
    };
    let serial = state.serial();
    state.error = Some(BranchError {
        serial,
        command,
        lines: Rc::new(lines),
    });
}

/// The dialog, while it is open.
pub fn dialog(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Option<Element> {
    let state = view.branch.state.read();
    let opened = state.open.as_ref()?;
    Some(
        CreateBranchPane {
            view,
            submit,
            serial: opened.serial,
        }
        .into(),
    )
}

/// A create's Git Error dialog, while its failure is shown — over the dialog when that is still
/// open, which Close returns to as it was left.
pub fn git_error(view: View) -> Option<Element> {
    let error = view.branch.state.read().error().cloned()?;
    Some(
        GitErrorDialog::new(error.serial, error.command, error.lines)
            .key(DiffKey::U64(error.serial))
            .on_close(move |()| {
                let mut state = view.branch.state;
                state.write().error = None;
            })
            .into(),
    )
}

/// The dialog drawn from the window's state, asking the engine about each name typed.
struct CreateBranchPane {
    view: View,
    submit: Option<Rc<dyn Fn(Request)>>,
    serial: u64,
}

// By the handles it reads: the submitter is the same repository's however often it is built.
impl PartialEq for CreateBranchPane {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
            && self.view.branch.state == other.view.branch.state
            && self.view.branch.name == other.view.branch.name
    }
}

impl Component for CreateBranchPane {
    fn render(&self) -> impl IntoElement {
        let view = self.view;
        let asking = self.submit.clone();
        // Each change of the name asks the engine about it, the first run as the dialog opens
        // included (an empty name asks nothing).
        use_side_effect(move || {
            let _typed = view.branch.name.read();
            name_changed(view, asking.as_deref());
        });
        let typed = view.branch.name.read().clone();
        // Read so a status arriving draws "Local changes:" anew.
        let _status = view.refreshed.read();
        let state = view.branch.state.read();
        let Some(opened) = state.open.as_ref() else {
            return rect().into_element();
        };
        let checkout = state.checkout;
        let local_shown = shows_local(view, checkout);
        let discarding = discards(view, &state, opened);
        let (creating, confirming, ticking, choosing) = (
            self.submit.clone(),
            self.submit.clone(),
            self.submit.clone(),
            self.submit.clone(),
        );
        CreateBranchDialog::new(
            opened.serial,
            opened.at,
            opened.subject.clone(),
            view.branch.name,
        )
        .ready(ready(opened, &typed, discarding))
        .refusal(refusal(opened, &typed, discarding))
        .waiting(waiting(opened, &typed, &view.writes.read()))
        .checkout(checkout)
        .local_changes(local_shown.then_some(opened.local))
        .discarding(
            discarding
                .then(|| consequence_for(opened, &typed))
                .flatten(),
        )
        .on_checkout(move |to: bool| checkout_toggled(view, to, ticking.as_deref()))
        .on_local(move |choice: LocalChoice| local_chosen(view, choice, choosing.as_deref()))
        .on_create(move |()| create(view, creating.as_deref()))
        .on_confirmed(move |token: Confirmed| discard_confirmed(view, token, confirming.as_deref()))
        .on_cancel(move |()| cancel(view))
        .into_element()
    }
}
