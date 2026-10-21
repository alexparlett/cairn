//! The menu bar's menus, built from the window's state: Fork's File, View, Repository, Window
//! and Help in its order and groups (`docs/research/ui-review/fork-window-survey-2026-10-16.md`,
//! the menu bar), holding only what Cairn does — each an action of the accelerator table, so a
//! menu item and its chord are one thing done in one place (`crate::shortcuts`), or Exit and
//! About Cairn. An item that cannot act now is greyed with its reason; Hide Commit Details and
//! Show Lost Commits are ticked while on, their labels unchanged, as Fork's
//! (`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`).

use cairn_ui::accelerators::Action;
use cairn_ui::menu::{BarMenu, Item};

use crate::window::View;
use crate::worker::Request;

/// What a menu item does when chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarChoice {
    /// The accelerator table's action, done as its chord does it.
    Act(Action),
    /// Closes the window, through the close hook as the desktop's close does.
    Exit,
    /// Opens About Cairn.
    About,
}

/// Why an item that needs a repository is greyed while none is open.
const NO_REPOSITORY: &str = "No repository is open.";

/// The menus as the window's state says them now; `open` is whether a repository is open.
pub fn menus(view: View, open: bool) -> Vec<BarMenu<BarChoice>> {
    let act = |label: &str, action: Action| Item::new(label, BarChoice::Act(action)).chord(action);
    let needs_repository = || (!open).then(|| NO_REPOSITORY.to_owned());
    // Read, not peeked: the menus are built on the window's render, which must draw them again
    // when the refs arrive.
    let head = {
        let _subscribed = view.refreshed.read();
        crate::history::head::head_commit(view)
            .err()
            .map(str::to_owned)
    };
    let fetch_label = match crate::operations::fetch::default_remote(view) {
        Some(remote) => format!("Fetch '{remote}'"),
        None => "Fetch".to_owned(),
    };
    vec![
        BarMenu::new("File", vec![vec![Item::new("Exit", BarChoice::Exit)]]),
        BarMenu::new(
            "View",
            vec![
                vec![
                    act("Show Uncommitted Changes", Action::ShowLocalChanges),
                    act("Show All Commits", Action::ShowAllCommits),
                ],
                vec![
                    act("Hide Commit Details", Action::ToggleDetails)
                        .ticked(*view.pane_collapsed.read()),
                ],
                vec![
                    act("Show HEAD", Action::ShowHead).greyed(needs_repository().or(head.clone())),
                ],
                vec![
                    act("Show Lost Commits (Reflog)", Action::ShowLostCommits)
                        .ticked(*view.show_lost.read())
                        .greyed(needs_repository()),
                ],
            ],
        ),
        BarMenu::new(
            "Repository",
            vec![
                vec![act("Refresh", Action::Refresh).greyed(needs_repository())],
                vec![
                    act(&fetch_label, Action::Fetch)
                        .greyed(crate::operations::fetch::refusal(view, open).map(str::to_owned)),
                ],
                vec![act("New Branch…", Action::NewBranch).greyed(needs_repository().or(head))],
            ],
        ),
        BarMenu::new(
            "Window",
            vec![vec![
                act("Zoom In", Action::ZoomIn).greyed(crate::frame::zoom_refusal(view, true)),
                act("Zoom Out", Action::ZoomOut).greyed(crate::frame::zoom_refusal(view, false)),
            ]],
        ),
        BarMenu::new(
            "Help",
            vec![vec![Item::new("About Cairn", BarChoice::About)]],
        ),
    ]
}

/// Does what `choice` names.
pub fn choose(choice: BarChoice, view: View, submit: Option<&dyn Fn(Request)>) {
    match choice {
        BarChoice::Act(action) => crate::shortcuts::act(action, view, submit),
        BarChoice::Exit => crate::frame::close(),
        BarChoice::About => {
            if !crate::shortcuts::keys_inert(view) {
                crate::frame::open_about(view);
            }
        }
    }
}

#[cfg(test)]
mod tests;
