//! Create Branch as the window keeps it (`docs/prd/staging-and-commit.md` R11.3; the user's
//! decisions, 2026-10-09): Fork's dialog, opened by "New Branch…" on any commit row.
//!
//! - **The name** is the window's text; each change asks the engine whether git takes it and
//!   whether a branch has it (`Request::CheckBranchName`, the branch-name lane, the latest ask
//!   superseding the one before), and the button is enabled only for the answer to the text
//!   shown. A create that fails leaves its name for the next opening, as Fork remembers it.
//! - **"Check out after create"** is sticky for the session (decision 1); kept across restarts
//!   once Cairn has a settings store (a later roadmap item). "Local changes:" is drawn while it
//!   is ticked and the working tree has staged, unstaged or conflicted changes; each opening
//!   starts on "Don't change", so "Discard" is never the remembered choice.
//! - **The writes**: unticked, `LocalWrite::CreateBranch`; ticked with "Don't change",
//!   `LocalWrite::CreateBranchAndCheckout`; ticked with "Discard", the engine first counts what
//!   would be lost (`Request::CheckoutConsequence`), the dialog closes, and the confirmation
//!   opens on that `Consequence`, its token the write's (`LocalWrite::CreateBranchDiscarding`).
//!   An engine refusal before any prompt is said in the dialog, which stays open.
//! - **A failure** — git refusing, or a discard whose state moved since its confirmation — opens
//!   Fork's Git Error dialog with its words.

use std::rc::Rc;

use cairn_model::{BranchName, Consequence, Oid, StatusEntry, WorkingTreeStatus};
use cairn_ui::{CreateBranchDialog, GitErrorDialog, LocalChoice};
use freya::prelude::*;

use crate::confirming::Confirming;
use crate::local_writes;
use crate::window::View;
use crate::worker::{LocalWrite, OperationId, Request, WriteEnding};

/// The confirmation's title: a discard's, as Local Changes' are.
pub const DISCARD_BEFORE_CHECKOUT_TITLE: &str = crate::local_changes_actions::DISCARD_TITLE;

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
    /// The name of a create that failed, offered to the next opening.
    kept_name: Option<String>,
    /// The write asked by the last create, and its name.
    asked: Option<(OperationId, String)>,
    /// A failure to draw in the Git Error dialog.
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
    /// A discard's count asked and not yet answered.
    counting: Option<OperationId>,
    /// Why the engine refused a discard before any prompt.
    refused: Option<String>,
    /// The discard's count answered: what the confirmation opens on, taken by the dialog.
    arrived: Option<Consequence>,
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

    fn serial(&mut self) -> u64 {
        self.next_serial += 1;
        self.next_serial
    }
}

/// "New Branch…" chosen on the row of `at`, whose subject is `subject`: the dialog opens, its
/// name the one a failed create left, or empty.
pub fn open(view: View, at: Oid, subject: String, submit: Option<&dyn Fn(Request)>) {
    let (mut state, mut name) = (view.branch.state, view.branch.name);
    let kept = {
        let mut state = state.write();
        let serial = state.serial();
        state.open = Some(Opened {
            serial,
            at,
            subject,
            local: LocalChoice::Keep,
            checked: None,
            counting: None,
            refused: None,
            arrived: None,
        });
        state.kept_name.take().unwrap_or_default()
    };
    name.set(kept.clone());
    ask_name(&kept, submit);
}

/// The name's text changed: what the engine said of the old text no longer stands, and the new
/// one is asked about.
pub fn name_changed(view: View, submit: Option<&dyn Fn(Request)>) {
    let typed = view.branch.name.peek().clone();
    let mut state = view.branch.state;
    if let Some(opened) = state.write().open.as_mut() {
        opened.refused = None;
    }
    ask_name(&typed, submit);
}

fn ask_name(name: &str, submit: Option<&dyn Fn(Request)>) {
    if let (false, Some(submit)) = (name.is_empty(), submit) {
        submit(Request::CheckBranchName {
            name: name.to_owned(),
        });
    }
}

/// The engine's answer for `name`: kept while the dialog is open, for the text it answers.
pub fn name_checked(view: View, name: String, outcome: Result<BranchName, String>) {
    let mut state = view.branch.state;
    if let Some(opened) = state.write().open.as_mut() {
        opened.checked = Some((name, outcome));
    }
}

/// Whether the dialog may create `typed`: the engine said git takes it and no branch has it.
fn ready(opened: &Opened, typed: &str) -> bool {
    !typed.is_empty()
        && opened.counting.is_none()
        && matches!(&opened.checked, Some((name, Ok(BranchName::Free))) if name == typed)
}

/// Why the dialog cannot create `typed`, said beside its buttons.
fn refusal(opened: &Opened, typed: &str) -> Option<String> {
    match &opened.checked {
        Some((name, Ok(answer))) if name == typed => answer.refusal(typed),
        Some((name, Err(failed))) if name == typed => Some(failed.clone()),
        Some(_) | None => None,
    }
    .or_else(|| opened.refused.clone())
}

/// Whether the working tree has a staged, an unstaged or a conflicted change: `git status`
/// lists those before any untracked path, so its first entry says.
fn has_changes(status: Option<&WorkingTreeStatus>) -> bool {
    match status {
        Some(WorkingTreeStatus::Listed(entries)) => matches!(
            entries.first(),
            Some(StatusEntry::Changed(_) | StatusEntry::Conflicted(_))
        ),
        Some(WorkingTreeStatus::IndexUnreadable(_) | WorkingTreeStatus::NoWorkingTree) | None => {
            false
        }
    }
}

/// The dialog's button, or Return in its field.
pub fn create(view: View, submit: Option<&dyn Fn(Request)>) {
    let Some(submit) = submit else {
        return;
    };
    let typed = view.branch.name.peek().clone();
    let shows_local = has_changes(view.refreshed.peek().status());
    let mut state = view.branch.state;
    let (at, checkout, local) = {
        let state = state.peek();
        let Some(opened) = state.open.as_ref().filter(|opened| ready(opened, &typed)) else {
            return;
        };
        (opened.at, state.checkout, opened.local)
    };
    let write = match (checkout, shows_local, local) {
        (false, _, _) => LocalWrite::CreateBranch {
            name: typed.clone(),
            at,
        },
        (true, true, LocalChoice::Discard) => {
            let asked = OperationId::next();
            if let Some(opened) = state.write().open.as_mut() {
                opened.counting = Some(asked);
                opened.refused = None;
            }
            submit(Request::CheckoutConsequence {
                asked,
                name: typed,
                at,
            });
            return;
        }
        (true, _, LocalChoice::Keep | LocalChoice::Discard) => {
            LocalWrite::CreateBranchAndCheckout {
                name: typed.clone(),
                at,
            }
        }
    };
    let mut writes = view.writes;
    let id = local_writes::ask(&mut writes.write(), submit, write);
    let mut state = state.write();
    state.open = None;
    state.asked = Some((id, typed));
}

/// What Create Branch's discard would lose, answering the count asked under `asked`: kept for
/// the dialog to open the confirmation on, or the engine's refusal said in the dialog.
pub fn consequence_arrived(view: View, asked: OperationId, outcome: Result<Consequence, String>) {
    let mut state = view.branch.state;
    let mut state = state.write();
    let Some(opened) = state
        .open
        .as_mut()
        .filter(|opened| opened.counting == Some(asked))
    else {
        return;
    };
    opened.counting = None;
    match outcome {
        Ok(consequence) => opened.arrived = Some(consequence),
        Err(refused) => opened.refused = Some(refused),
    }
}

/// The count answered: the dialog closes, its name kept should the confirmation be cancelled,
/// and the confirmation opens on what would be lost, its token the write's.
fn confirm_arrived(view: View, submit: Option<Rc<dyn Fn(Request)>>) {
    let mut state = view.branch.state;
    let Some(consequence) = state
        .write()
        .open
        .as_mut()
        .and_then(|opened| opened.arrived.take())
    else {
        return;
    };
    let typed = view.branch.name.peek().clone();
    {
        let mut state = state.write();
        state.open = None;
        state.kept_name = Some(typed.clone());
    }
    let mut confirming = view.confirming;
    confirming.set(Some(Confirming::new(
        DISCARD_BEFORE_CHECKOUT_TITLE,
        consequence,
        move |token| {
            let Some(submit) = submit.as_deref() else {
                return;
            };
            let mut writes = view.writes;
            let id = local_writes::ask(
                &mut writes.write(),
                submit,
                LocalWrite::CreateBranchDiscarding(token),
            );
            let mut state = view.branch.state;
            state.write().asked = Some((id, typed.clone()));
        },
    )));
}

/// "Check out after create" ticked or not: sticky for the session.
pub fn checkout_toggled(view: View, to: bool) {
    let mut state = view.branch.state;
    state.write().checkout = to;
}

/// A choice of "Local changes:" for this opening.
pub fn local_chosen(view: View, choice: LocalChoice) {
    let mut state = view.branch.state;
    if let Some(opened) = state.write().open.as_mut() {
        opened.local = choice;
        opened.refused = None;
    }
}

/// Cancel, Escape or a press outside: the dialog closes, its name not kept.
pub fn cancel(view: View) {
    let mut state = view.branch.state;
    state.write().open = None;
}

/// A local write ended: when it is the dialog's, a success lets the name go, and anything else
/// keeps it and opens the Git Error dialog with its words.
pub fn write_ended(view: View, id: OperationId, ending: &WriteEnding) {
    let mut state = view.branch.state;
    let name = match state.peek().asked.as_ref() {
        Some((asked, name)) if *asked == id => name.clone(),
        Some(_) | None => return,
    };
    let mut state = state.write();
    state.asked = None;
    let (command, lines) = match ending {
        WriteEnding::Done(_) => {
            state.kept_name = None;
            return;
        }
        WriteEnding::Failed {
            message,
            command,
            output,
            ..
        } => {
            let lines: Vec<String> = output
                .lines()
                .map(crate::commit_box_state::strip_ansi)
                .collect();
            let lines = if lines.is_empty() {
                vec![message.clone()]
            } else {
                lines
            };
            (command.clone().unwrap_or_default(), lines)
        }
        WriteEnding::Stale { message, .. }
        | WriteEnding::Refused { message }
        | WriteEnding::MayHaveTakenEffect { message, .. }
        | WriteEnding::Incomplete { message, .. }
        | WriteEnding::NotRun { message } => (String::new(), vec![message.clone()]),
    };
    state.kept_name = Some(name);
    let serial = state.serial();
    state.error = Some(BranchError {
        serial,
        command,
        lines: Rc::new(lines),
    });
}

/// The dialog, while it is open, and the Git Error dialog, while a create's failure is shown.
pub fn dialogs(view: View, submit: Option<Rc<dyn Fn(Request)>>) -> Option<Element> {
    if let Some(error) = view.branch.state.read().error().cloned() {
        return Some(
            GitErrorDialog::new(error.serial, error.command, error.lines)
                .key(DiffKey::U64(error.serial))
                .on_close(move |()| {
                    let mut state = view.branch.state;
                    state.write().error = None;
                })
                .into(),
        );
    }
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
        // Each change of the name asks the engine about it; the first run, as the dialog opens,
        // is the opening's own ask.
        let mut first = use_state(|| true);
        use_side_effect(move || {
            let _typed = view.branch.name.read();
            if *first.peek() {
                first.set(false);
                return;
            }
            name_changed(view, asking.as_deref());
        });
        // A discard's count answered: the confirmation opens in the dialog's place.
        let confirming = self.submit.clone();
        use_side_effect(move || {
            let arrived = view
                .branch
                .state
                .read()
                .open
                .as_ref()
                .is_some_and(|opened| opened.arrived.is_some());
            if arrived {
                confirm_arrived(view, confirming.clone());
            }
        });
        let typed = view.branch.name.read().clone();
        let shows_local = has_changes(view.refreshed.read().status());
        let state = view.branch.state.read();
        let Some(opened) = state.open.as_ref() else {
            return rect().into_element();
        };
        let checkout = state.checkout;
        let creating = self.submit.clone();
        CreateBranchDialog::new(
            opened.serial,
            opened.at,
            opened.subject.clone(),
            view.branch.name,
        )
        .ready(ready(opened, &typed))
        .refusal(refusal(opened, &typed))
        .checkout(checkout)
        .local_changes((checkout && shows_local).then_some(opened.local))
        .on_checkout(move |to: bool| checkout_toggled(view, to))
        .on_local(move |choice: LocalChoice| local_chosen(view, choice))
        .on_create(move |()| create(view, creating.as_deref()))
        .on_cancel(move |()| cancel(view))
        .into_element()
    }
}
