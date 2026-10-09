//! Show Lost Commits as the window keeps it (`docs/prd/staging-and-commit.md` R11): a toggle
//! of the session, which reopens the history — a reopen like any other, off the UI thread
//! (R11.4) — walking from every reflog entry of `HEAD` and each local branch too, or no
//! longer.

use cairn_ui::accelerators::Action;
use freya::prelude::*;

use crate::window::View;
use crate::worker::Request;

/// What a chord of the history's own scope does (R7.3): Show Lost Commits toggles it.
pub fn history_action(action: Action, view: View, submit: Option<&dyn Fn(Request)>) {
    // Every other action is heard in a scope of its own, never the history's.
    if action == Action::ShowLostCommits {
        toggle(view, submit);
    }
}

/// Turns Show Lost Commits on or off, and reopens the history to walk as it now says. With no
/// repository open, nothing is asked and nothing changes.
pub fn toggle(view: View, submit: Option<&dyn Fn(Request)>) {
    let Some(submit) = submit else {
        return;
    };
    let mut showing = view.show_lost;
    let lost = !*showing.peek();
    showing.set(lost);
    crate::session::reopen_history(view.rows, view.progress, lost, submit);
    // A find looking in the history it replaced looks in the new one.
    crate::ref_find::reopened(view, Some(submit));
}
