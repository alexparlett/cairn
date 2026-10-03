//! What each accelerator does in the window (PRD R8). The chords are the table's
//! (`cairn_ui::accelerators`), heard by the window for [`Scope::Window`] and by the detail
//! pane for [`Scope::Detail`]; this is where an [`Action`] meets the view state, one arm per
//! action, so an action added to the table does not compile until it is placed here.
//!
//! [`Scope::Window`]: cairn_ui::accelerators::Scope::Window
//! [`Scope::Detail`]: cairn_ui::accelerators::Scope::Detail

use cairn_ui::DetailTab;
use cairn_ui::accelerators::Action;
use freya::prelude::*;

use crate::window::View;

/// Does `action` to `view` — nothing while a credential prompt is up, which owns the keys
/// until it is answered (Q3). The diff view's actions resolve already and act once the view
/// they move exists: the unified view (phase 06), the Changes tab (phase 07), and the
/// second commit of a comparison (phase 08).
pub fn act(action: Action, view: View) {
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
        Action::PreviousChange
        | Action::NextChange
        | Action::ToggleSideBySide
        | Action::ToggleIgnoreWhitespace
        | Action::MoreLines
        | Action::FewerLines
        | Action::EntireFile
        | Action::ExtendSelection => {}
    }
}

/// A tab asked for by its chord is shown, on a pane opened for it if it was collapsed.
fn show(tab: DetailTab, detail_tab: &mut State<DetailTab>, collapsed: &mut State<bool>) {
    detail_tab.set(tab);
    collapsed.set(false);
}
