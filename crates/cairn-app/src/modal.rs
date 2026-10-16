//! The one modal state: which dialog, or the activity popover, owns the keys and the pointer
//! now. Each keeps its own state — a confirmation its consequence, the commit box its Git Error
//! and token, Create Branch its name — and this decides, from all of them, the one on top: the
//! only one the window draws, and, while there is one, the window's chords and the lists' keys
//! do nothing (`shortcuts::keys_inert`).
//!
//! The order is what must be answered first: a credential prompt (a git waits on it), then a
//! confirmation (the person's own question), then a Git Error or "Couldn't <name>" — the commit
//! box's first, then the others, oldest first — then Create Branch, then the popover. So a
//! commit's Git Error that arrives while Create Branch is up is drawn over it, and alone: its
//! Escape closes it, and Create Branch is drawn again as it was left. A dialog under another is
//! kept, not drawn, so no key reaches it.

use crate::window::View;

/// The dialog, or the popover, on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modal {
    /// A credential prompt a git waits on.
    Prompt,
    /// A destructive operation's confirmation.
    Confirmation,
    /// A failed commit's or amend's Git Error, with its skip.
    CommitError,
    /// Another operation's Git Error or "Couldn't <name>".
    Ending,
    /// Create Branch.
    CreateBranch,
    /// The activity popover.
    Popover,
}

/// The one on top now, read without subscribing: what a key handler asks.
pub fn modal(view: View) -> Option<Modal> {
    if view.prompt.peek().is_some() {
        Some(Modal::Prompt)
    } else if view.confirming.peek().is_some() {
        Some(Modal::Confirmation)
    } else if view.local.commit.state.peek().error().is_some() {
        Some(Modal::CommitError)
    } else if !view.endings.peek().is_empty() {
        Some(Modal::Ending)
    } else if view.branch.state.peek().is_up() {
        Some(Modal::CreateBranch)
    } else if view.activity.is_open() {
        Some(Modal::Popover)
    } else {
        None
    }
}

/// The one on top now, subscribing the window's render to every state it reads: what the
/// window draws.
pub fn drawn(view: View) -> Option<Modal> {
    if view.prompt.read().is_some() {
        Some(Modal::Prompt)
    } else if view.confirming.read().is_some() {
        Some(Modal::Confirmation)
    } else if view.local.commit.state.read().error().is_some() {
        Some(Modal::CommitError)
    } else if !view.endings.read().is_empty() {
        Some(Modal::Ending)
    } else if view.branch.state.read().is_up() {
        Some(Modal::CreateBranch)
    } else if *view.activity.open.read() {
        Some(Modal::Popover)
    } else {
        None
    }
}
