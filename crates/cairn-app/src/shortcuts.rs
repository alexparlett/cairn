//! What each accelerator does in the window. The chords are the table's
//! (`cairn_ui::accelerators`), heard by the window for [`Scope::Window`] and by the detail
//! pane for [`Scope::Detail`]; this is where an [`Action`] meets the view state, one arm per
//! action, so an action added to the table does not compile until it is placed here.
//!
//! [`Scope::Window`]: cairn_ui::accelerators::Scope::Window
//! [`Scope::Detail`]: cairn_ui::accelerators::Scope::Detail

use cairn_ui::accelerators::Action;
use cairn_ui::{DetailTab, HeaderAction, MainView};
use freya::prelude::*;

use crate::diff;
use crate::window::View;
use crate::worker::Request;

/// The action a press in the diff's bar is: the bar's buttons and the chords are one set of
/// actions, done in one place.
pub fn of_header(pressed: HeaderAction) -> Action {
    match pressed {
        HeaderAction::PreviousChange => Action::PreviousChange,
        HeaderAction::NextChange => Action::NextChange,
        HeaderAction::IgnoreWhitespace => Action::ToggleIgnoreWhitespace,
        HeaderAction::FewerLines => Action::FewerLines,
        HeaderAction::MoreLines => Action::MoreLines,
        HeaderAction::EntireFile => Action::EntireFile,
        HeaderAction::SideBySide => Action::ToggleSideBySide,
    }
}

/// Whether the window's chords and the views' keys do nothing now: while any dialog — a
/// credential prompt, a confirmation, a Git Error or "Couldn't <name>", Create Branch — or the
/// activity popover, which Escape alone closes, is open: the one modal state
/// decides it (`crate::modal`).
pub fn keys_inert(view: View) -> bool {
    crate::modal::modal(view).is_some()
}

/// Does `action` to `view`, asking through `submit` what it must — nothing while a
/// credential prompt is up, which owns the keys until it is answered, nor while a destructive
/// operation's confirmation is open: the window hears every key before any dialog can, so the
/// dialog cannot make the chords inert itself. The second commit of a comparison is a press, not
/// a key: the history list resolves it against the keys the window heard held (`HeldKeys`) and
/// selects it (`selection::extend`).
pub fn act(action: Action, view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut detail_tab,
        mut pane_collapsed,
        ..
    } = view;
    if keys_inert(view) {
        return;
    }
    match action {
        Action::ShowCommitTab => show(DetailTab::Commit, &mut detail_tab, &mut pane_collapsed),
        Action::ShowChangesTab => show(DetailTab::Changes, &mut detail_tab, &mut pane_collapsed),
        Action::PreviousChange => diff::actions::step(view, false),
        Action::NextChange => diff::actions::step(view, true),
        Action::ToggleIgnoreWhitespace => diff::actions::change_settings(view, submit, |s| {
            s.toggle_ignore_whitespace();
            true
        }),
        Action::MoreLines => diff::actions::change_settings(view, submit, |s| s.more_lines()),
        Action::FewerLines => diff::actions::change_settings(view, submit, |s| s.fewer_lines()),
        Action::EntireFile => diff::actions::change_settings(view, submit, |s| {
            s.toggle_entire_file();
            true
        }),
        Action::ToggleSideBySide => diff::actions::toggle_side_by_side(view),
        // A press's chord, never a key's: resolved where the press lands. The range press and
        // Shift+↑/↓ are Local Changes' lists' own, resolved by the list.
        Action::ExtendSelection
        | Action::SelectRange
        | Action::ExtendSelectionUp
        | Action::ExtendSelectionDown => {}
        // Heard only in their own scopes — Local Changes' lists and diff, the commit box — by
        // the view that has focus there, never by the window.
        Action::StageOrUnstage | Action::StageOrUnstageAll | Action::Discard | Action::Commit => {}
        // Fork's New Branch, at `HEAD`, wherever focus is.
        Action::NewBranch => crate::operations::create_branch::open_at_head(view),
        // Read again on the worker; the window only asks.
        Action::Refresh => {
            if let Some(submit) = submit {
                submit(Request::Refresh);
            }
        }
        Action::ShowLostCommits => crate::history::lost_commits::toggle(view, submit),
        Action::ShowLocalChanges => {
            crate::history::selection::show_main(MainView::LocalChanges, view, submit);
        }
        Action::ShowAllCommits => {
            crate::history::selection::show_main(MainView::AllCommits, view, submit);
        }
        Action::ShowHead => crate::history::head::show(view, submit),
        // The pane collapses to its tab strip, or opens again at the height it had.
        Action::ToggleDetails => pane_collapsed.toggle(),
        Action::Fetch => crate::operations::fetch::start(view, submit),
        Action::ZoomIn => crate::frame::zoom(view, true),
        Action::ZoomOut => crate::frame::zoom(view, false),
    }
}

/// A tab asked for by its chord is shown, on a pane opened for it if it was collapsed.
fn show(tab: DetailTab, detail_tab: &mut State<DetailTab>, collapsed: &mut State<bool>) {
    detail_tab.set(tab);
    collapsed.set(false);
}
