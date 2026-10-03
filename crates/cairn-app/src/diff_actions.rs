//! What the diff view's controls do to the view state (PRD R6.2, R6.3, R6.7, R8): choosing
//! a file, the settings every diff view shares, and previous and next change. On the UI
//! thread; asking is a [`Request`] handed to the caller's submit, never a wait.
//!
//! A setting is part of what a file's diff is asked with — the context git is asked at, and
//! whether the whitespace-ignoring ranges are read — so a change of setting asks again,
//! through [`DiffState`](crate::diff_state::DiffState) and the file-diff lane, superseding
//! what is in flight; the answer kept is then the one naming the new options, and no other.

use cairn_model::Context;
use cairn_ui::{DiffSettings, step_change};
use freya::prelude::*;

use crate::window::View;
use crate::worker::{DiffOptions, FileQuery, FileTarget, Request};

/// What a file's diff is asked with, from the shared settings.
pub fn options(settings: DiffSettings) -> DiffOptions {
    DiffOptions {
        context: settings.context(),
        ignore_whitespace: settings.ignore_whitespace(),
        load_anyway: false,
    }
}

fn submit_all(requests: Vec<Request>, submit: Option<&dyn Fn(Request)>) {
    if let Some(submit) = submit {
        for request in requests {
            submit(request);
        }
    }
}

/// The view starts at the top of a diff it has not drawn before, with no change moved to.
fn reset_view(view: View) {
    let View {
        mut diff_scroll,
        mut change_cursor,
        ..
    } = view;
    diff_scroll.scroll_to_x(0);
    diff_scroll.scroll_to_y(0);
    change_cursor.set(None);
}

/// The file at `index` of the change set the selected commit answered becomes the file
/// shown, asked at the session's settings. Choosing the file already shown asks nothing,
/// unless its answer failed, when choosing it again is how to retry.
pub fn choose_file(index: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut diff,
        diff_settings,
        ..
    } = view;
    let query = {
        let state = diff.peek();
        let Some((of, _)) = state.changes() else {
            return;
        };
        let Some(file) = crate::diff_state::answered_changes(&state).files.get(index) else {
            return;
        };
        FileQuery {
            target: FileTarget::Committed {
                of,
                file: file.clone(),
            },
            options: options(*diff_settings.peek()),
        }
    };
    let asked = diff.peek().file().is_some_and(|(selected, answer)| {
        *selected == query && !matches!(answer, crate::diff_state::Answer::Failed(_))
    });
    if asked {
        return;
    }
    let requests = diff.write().select_file(query);
    reset_view(view);
    submit_all(requests, submit);
}

/// Asks the file shown again at the settings as they are now, keeping where the view is.
fn ask_again(view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut diff,
        diff_settings,
        mut change_cursor,
        ..
    } = view;
    let Some(mut query) = diff.peek().file().map(|(query, _)| query.clone()) else {
        return;
    };
    let asked = options(*diff_settings.peek());
    if query.options == asked {
        return;
    }
    query.options = asked;
    let requests = diff.write().select_file(query);
    change_cursor.set(None);
    submit_all(requests, submit);
}

/// Changes the shared settings with `change` and, when it changed them, asks the file shown
/// again.
pub fn change_settings(
    view: View,
    submit: Option<&dyn Fn(Request)>,
    change: impl FnOnce(&mut DiffSettings) -> bool,
) {
    let mut settings = view.diff_settings;
    let mut changed = *settings.peek();
    if !change(&mut changed) {
        return;
    }
    settings.set(changed);
    ask_again(view, submit);
}

/// The user's `diff.context` arrived: the session starts from it, unless the context was
/// already moved, and a file shown at the old default is asked again.
pub fn configured(context: Context, view: View, submit: &dyn Fn(Request)) {
    change_settings(view, Some(submit), |settings| settings.configured(context));
}

/// Previous or next change (R6.2): one change from where the view is, or from the change
/// last moved to while the view has not moved since; nothing when no diff is drawn — the
/// Changes tab hidden, the pane collapsed — or there is no change that way.
pub fn step(view: View, forward: bool) {
    let View {
        diff,
        detail_tab,
        pane_collapsed,
        mut diff_scroll,
        mut change_cursor,
        ..
    } = view;
    if *detail_tab.peek() != cairn_ui::DetailTab::Changes || *pane_collapsed.peek() {
        return;
    }
    let (_, scrolled_y): (i32, i32) = diff_scroll.into();
    let moved = {
        let state = diff.peek();
        let Some(layout) = state.shown_file().and_then(|shown| shown.layout()) else {
            return;
        };
        step_change(layout, *change_cursor.peek(), scrolled_y, forward)
    };
    if let Some(moved) = moved {
        diff_scroll.scroll_to_y(moved.scrolled_y);
        change_cursor.set(Some(moved));
    }
}
