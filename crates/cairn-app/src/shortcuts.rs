//! What each accelerator does in the window (PRD R8). The chords are the table's
//! (`cairn_ui::accelerators`), heard by the window for [`Scope::Window`] and by the detail
//! pane for [`Scope::Detail`]; this is where an [`Action`] meets the view state, one arm per
//! action, so an action added to the table does not compile until it is placed here.
//!
//! [`Scope::Window`]: cairn_ui::accelerators::Scope::Window
//! [`Scope::Detail`]: cairn_ui::accelerators::Scope::Detail

use cairn_ui::accelerators::Action;
use cairn_ui::{DetailTab, HeaderAction};
use freya::prelude::*;

use crate::diff_actions;
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

/// Does `action` to `view`, asking through `submit` what it must — nothing while a
/// credential prompt is up, which owns the keys until it is answered (Q3). The second commit
/// of a comparison is a press, not a key: the history list resolves it against the keys the
/// window heard held (`HeldKeys`) and selects it (`selection::extend`).
pub fn act(action: Action, view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut detail_tab,
        mut pane_collapsed,
        prompt,
        ..
    } = view;
    if prompt.peek().is_some() {
        return;
    }
    match action {
        Action::ShowCommitTab => show(DetailTab::Commit, &mut detail_tab, &mut pane_collapsed),
        Action::ShowChangesTab => show(DetailTab::Changes, &mut detail_tab, &mut pane_collapsed),
        Action::PreviousChange => diff_actions::step(view, false),
        Action::NextChange => diff_actions::step(view, true),
        Action::ToggleIgnoreWhitespace => diff_actions::change_settings(view, submit, |s| {
            s.toggle_ignore_whitespace();
            true
        }),
        Action::MoreLines => diff_actions::change_settings(view, submit, |s| s.more_lines()),
        Action::FewerLines => diff_actions::change_settings(view, submit, |s| s.fewer_lines()),
        Action::EntireFile => diff_actions::change_settings(view, submit, |s| {
            s.toggle_entire_file();
            true
        }),
        Action::ToggleSideBySide => diff_actions::toggle_side_by_side(view),
        // A press's chord, never a key's: resolved where the press lands.
        Action::ExtendSelection => {}
    }
}

/// A tab asked for by its chord is shown, on a pane opened for it if it was collapsed.
fn show(tab: DetailTab, detail_tab: &mut State<DetailTab>, collapsed: &mut State<bool>) {
    detail_tab.set(tab);
    collapsed.set(false);
}
