//! Show Lost Commits as the window keeps it: a toggle of the session, which reopens the
//! history — a reopen like any other, off the UI thread — walking from every reflog entry of
//! `HEAD` and each local branch too while it is on.

use freya::prelude::*;

use crate::window::View;
use crate::worker::Request;

/// Turns Show Lost Commits on or off, and reopens the history to walk as it now says. With no
/// repository open, nothing is asked and nothing changes.
pub fn toggle(view: View, submit: Option<&dyn Fn(Request)>) {
    let Some(submit) = submit else {
        return;
    };
    let mut showing = view.show_lost;
    let lost = !*showing.peek();
    showing.set(lost);
    crate::session::reopen_history(view.progress, lost, submit);
    // A find looking in the history it replaced looks in the new one.
    crate::history::ref_find::reopened(view, Some(submit));
}
