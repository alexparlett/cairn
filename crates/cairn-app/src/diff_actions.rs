//! What the diff view's controls do to the view state (PRD R6.1-R6.3, R6.7, R6.8, R8):
//! choosing a file, the diff views' settings, Load Diff, and previous and next
//! change. On the UI thread; asking is a [`Request`] handed to the caller's submit, never a
//! wait.
//!
//! A setting is part of what a file's diff is asked with — the context git is asked at, and
//! whether the whitespace-ignoring ranges are read — so a change of setting asks again,
//! through [`DiffState`](crate::diff_state::DiffState) and the file-diff lane, superseding
//! what is in flight; the answer kept is then the one naming the new options, and no other.

use cairn_model::{ChangeList, Context, LocalChange, PathState, StagedAgainst};
use cairn_ui::{DetailTab, DiffSettings, MainView, step_change};
use freya::prelude::*;

use crate::diff_state::{Asking, WorkingChoice};
use crate::window::View;
use crate::worker::{DiffOptions, FileQuery, FileTarget, Request, WorkingSide};

/// What the Changes tab's file is asked with, from the settings.
pub fn options(settings: DiffSettings) -> DiffOptions {
    DiffOptions {
        context: settings.context(),
        ignore_whitespace: settings.ignore_whitespace(),
        load_anyway: false,
    }
}

/// What a file opened in place in the Commit tab is asked with: the shared context and
/// whitespace, never the entire file, which is the Changes tab's and Local Changes' and never a
/// file opened in place (the user's decision, 2026-10-04).
pub fn in_place_options(settings: DiffSettings) -> DiffOptions {
    DiffOptions {
        context: settings.line_context(),
        ..options(settings)
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
    let requests = {
        let mut state = diff.write();
        let requests = state.select_file(query);
        state.chose_file_at(index);
        requests
    };
    reset_view(view);
    submit_all(requests, submit);
}

/// Load Diff (R6.8): the file shown asked again past R2.6's ceilings, up to the load-anyway
/// ceiling, keeping where the view is. Its rows are then drawn with each line past the
/// long-line limit cut (R6.9). Nothing when no file is shown or it was loaded already.
pub fn load_anyway(view: View, submit: Option<&dyn Fn(Request)>) {
    let View { mut diff, .. } = view;
    let Some(mut query) = diff.peek().file().map(|(query, _)| query.clone()) else {
        return;
    };
    if query.options.load_anyway {
        return;
    }
    query.options.load_anyway = true;
    let requests = diff.write().select_file(query);
    submit_all(requests, submit);
}

/// Side-by-side or unified (R6.1): the same answer drawn another way, so nothing is asked
/// again; the change last moved to is let go, in either view, since its rows are another
/// view's.
pub fn toggle_side_by_side(view: View) {
    let View {
        mut diff_settings,
        mut change_cursor,
        ..
    } = view;
    let mut local_cursor = view.local.cursor;
    diff_settings.write().toggle_side_by_side();
    change_cursor.set(None);
    local_cursor.set(None);
}

/// Whether Local Changes is shown in place of the history.
fn local_changes_shown(view: View) -> bool {
    *view.sidebar.main.peek() == MainView::LocalChanges
}

/// What a path of Local Changes is asked with: the Changes tab's options, but always the exact
/// diff — whitespace never ignored, whatever the shared setting says (staging-and-commit R8.5,
/// L6), since what is staged from the view is what it draws.
pub fn working_options(settings: DiffSettings) -> DiffOptions {
    DiffOptions {
        ignore_whitespace: false,
        ..options(settings)
    }
}

/// What a path of Local Changes is asked as (R9.3): its staged diff from Staged, its unstaged
/// or — for a path git does not track — untracked diff from Unstaged, at the view's options
/// ([`working_options`]); `None` for a conflicted path, which has a notice in place of a diff
/// (R9.4).
pub fn working_query(
    list: ChangeList,
    change: &LocalChange<'_>,
    against: StagedAgainst,
    settings: DiffSettings,
) -> Option<FileQuery> {
    let side = match (change.state, list, against) {
        (PathState::Conflicted, _, _) => return None,
        (PathState::Untracked, _, _) => WorkingSide::Untracked,
        (PathState::Tracked, ChangeList::Staged, StagedAgainst::Head) => WorkingSide::Staged,
        // A file of amend's staged list: against `HEAD`'s parent (staging-and-commit R10.3).
        (PathState::Tracked, ChangeList::Staged, StagedAgainst::HeadParent(_)) => {
            WorkingSide::Amending
        }
        (PathState::Tracked, ChangeList::Unstaged, _) => WorkingSide::Unstaged,
    };
    Some(FileQuery {
        target: FileTarget::WorkingTree {
            path: change.path.clone(),
            side,
        },
        options: working_options(settings),
    })
}

/// Local Changes' diff starts at its top, with no change moved to.
fn reset_local_view(view: View) {
    let mut scroll = view.local.scroll;
    let mut cursor = view.local.cursor;
    scroll.scroll_to_x(0);
    scroll.scroll_to_y(0);
    cursor.set(None);
}

/// The path on `row` of `list`, in the lists Local Changes draws, becomes the path chosen and
/// its diff is asked (R9.3). Choosing the path already chosen, asked as it is, asks nothing,
/// unless its answer failed, when choosing it again is how to retry.
pub fn choose_working(list: ChangeList, row: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    let (choice, query) = {
        let local = view.local.state.peek();
        let lists = crate::local_changes_state::drawn_changes(&local);
        let Some(change) = lists.get(list, row) else {
            return;
        };
        (
            WorkingChoice {
                list,
                path: change.path.clone(),
                lists: local.serial(),
            },
            working_query(
                list,
                &change,
                lists.staged_against(),
                *view.diff_settings.peek(),
            ),
        )
    };
    let mut diff = view.diff;
    let asked = {
        let state = diff.peek();
        state.working_choice() == Some(&choice)
            && state.working_query() == query.as_ref()
            && !matches!(
                state.working_shown(),
                Some(crate::diff_state::WorkingShown::Failed(_))
            )
    };
    if asked {
        return;
    }
    let requests = diff.write().choose_working(choice, query);
    reset_local_view(view);
    submit_all(requests, submit);
}

/// Load Diff for the path chosen in Local Changes (R6.8), keeping where its view is.
pub fn load_working_anyway(view: View, submit: Option<&dyn Fn(Request)>) {
    let mut diff = view.diff;
    let requests = diff.write().load_working_anyway();
    submit_all(requests, submit);
}

/// Asks the file shown, and the files opened in place, again at the settings as they are
/// now, keeping where the views are: the one whose tab is shown at once, the other as its tab
/// is shown again — they share the file-diff lane, so asking both would only have the second
/// end the first.
fn ask_again(view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut diff,
        diff_settings,
        mut change_cursor,
        ..
    } = view;
    let mut local_cursor = view.local.cursor;
    let asking = if local_changes_shown(view) {
        Asking::Working
    } else {
        match crate::detail_pane::shown_tab(view) {
            DetailTab::Changes => Asking::File,
            DetailTab::Commit => Asking::Expansion,
        }
    };
    let settings = *diff_settings.peek();
    let requests = diff.write().settings_changed(
        options(settings),
        in_place_options(settings),
        working_options(settings),
        asking,
    );
    if !requests.is_empty() {
        change_cursor.set(None);
        local_cursor.set(None);
    }
    submit_all(requests, submit);
}

/// A file pressed in the Commit tab opens its diff in place under its row, or closes it
/// (R5.3, Fork's Finding 4), at the session's settings but never the entire file.
pub fn toggle_in_place(index: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut diff,
        diff_settings,
        ..
    } = view;
    let requests = diff
        .write()
        .toggle_file(index, in_place_options(*diff_settings.peek()));
    submit_all(requests, submit);
}

/// Expand All (`all`) or Collapse All, above the Commit tab's files.
pub fn expand_all(all: bool, view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut diff,
        diff_settings,
        ..
    } = view;
    let requests = if all {
        diff.write()
            .expand_all(in_place_options(*diff_settings.peek()))
    } else {
        diff.write().collapse_all()
    };
    submit_all(requests, submit);
}

/// Load Diff under a file opened in place (R6.8).
pub fn load_in_place(index: usize, view: View, submit: Option<&dyn Fn(Request)>) {
    let View { mut diff, .. } = view;
    let requests = diff.write().load_in_place(index);
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
/// Changes tab hidden, the pane collapsed — or there is no change that way. With Local Changes
/// shown, in its diff.
pub fn step(view: View, forward: bool) {
    if local_changes_shown(view) {
        // Paths drawn together have no one file's changes to step through.
        if view.diff.peek().together().is_some() {
            return;
        }
        let mut scroll = view.local.scroll;
        let mut cursor = view.local.cursor;
        let side_by_side = view.diff_settings.peek().side_by_side();
        let (_, scrolled_y): (i32, i32) = scroll.into();
        let moved = {
            let state = view.diff.peek();
            let Some(stops) = state
                .shown_working()
                .and_then(|shown| shown.stops(side_by_side))
            else {
                return;
            };
            step_change(stops, *cursor.peek(), scrolled_y, forward)
        };
        if let Some(moved) = moved {
            scroll.scroll_to_y(moved.scrolled_y);
            cursor.set(Some(moved));
        }
        return;
    }
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
    let side_by_side = view.diff_settings.peek().side_by_side();
    let (_, scrolled_y): (i32, i32) = diff_scroll.into();
    let moved = {
        let state = diff.peek();
        let Some(stops) = state
            .shown_file()
            .and_then(|shown| shown.stops(side_by_side))
        else {
            return;
        };
        step_change(stops, *change_cursor.peek(), scrolled_y, forward)
    };
    if let Some(moved) = moved {
        diff_scroll.scroll_to_y(moved.scrolled_y);
        change_cursor.set(Some(moved));
    }
}
