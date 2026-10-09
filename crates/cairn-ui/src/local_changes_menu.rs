//! What Local Changes offers a selection, and its context menu (staging-and-commit R8.2, R8.4,
//! R8.7, R8.8): Stage or Unstage, Discard Changes…, Stage All or Unstage All, and Copy Path —
//! Fork's menu, less what this packet does not build (open, external diff, blame, history,
//! ignore, stash, save as patch).
//!
//! **What is never offered.** No discard on the staged side, anywhere (R3.6): the Staged menu
//! has no Discard item at all. A conflicted path stages whole — `git add`, which marks it
//! resolved — and is never discarded (R8.7, L25); a submodule's changes are never discarded,
//! since `git restore` leaves its commit where it was and no prompt can count what is dirty
//! inside it (R8.8, L24). A selection holding either offers its Discard disabled, with the
//! reason beside it ([`NoDiscard`]), and the discard chord says the same rather than asking.

use cairn_model::{ChangeKind, ChangeList, LocalChanges, RepoPath};
use freya::prelude::*;

use crate::accelerators::Action;

/// The menu's words.
pub const STAGE_CAPTION: &str = "Stage";
pub const UNSTAGE_CAPTION: &str = "Unstage";
pub const STAGE_ALL_CAPTION: &str = "Stage All";
pub const UNSTAGE_ALL_CAPTION: &str = "Unstage All";
pub const DISCARD_CAPTION: &str = "Discard Changes…";
pub const COPY_PATH_CAPTION: &str = "Copy Path";

/// Why a selection offers no discard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoDiscard {
    /// The Staged list: staged changes are never discarded (R3.6).
    Staged,
    /// A submodule among the paths (R8.8).
    Submodule(RepoPath),
    /// A conflicted path among them (R8.7).
    Conflicted(RepoPath),
}

impl NoDiscard {
    /// The reason, as the menu and the view say it.
    pub fn text(&self) -> String {
        match self {
            Self::Staged => "Staged changes can't be discarded: unstage them first.".to_owned(),
            Self::Submodule(path) => format!(
                "{} is a submodule: its changes can't be discarded, since what they would \
                 lose inside it can't be counted.",
                path.display()
            ),
            Self::Conflicted(path) => format!(
                "{} has a conflict: it can't be discarded. Stage it to mark it resolved.",
                path.display()
            ),
        }
    }
}

/// Why discarding `paths` of `list` is not offered, if it is not: the Staged list, else the
/// first submodule or conflicted path among them, in the order given. A path `list` does not
/// list is left out, as acting on the selection leaves it out. A binary search per path.
pub fn no_discard<'a>(
    changes: &LocalChanges,
    list: ChangeList,
    paths: impl IntoIterator<Item = &'a RepoPath>,
) -> Option<NoDiscard> {
    if list == ChangeList::Staged {
        return Some(NoDiscard::Staged);
    }
    paths.into_iter().find_map(|path| {
        let change = changes
            .row_of(list, path)
            .and_then(|row| changes.get(list, row))?;
        match change.kind {
            ChangeKind::Submodule => Some(NoDiscard::Submodule(path.clone())),
            ChangeKind::Conflicted => Some(NoDiscard::Conflicted(path.clone())),
            ChangeKind::Modified
            | ChangeKind::Added
            | ChangeKind::Deleted
            | ChangeKind::Renamed
            | ChangeKind::Copied
            | ChangeKind::TypeChanged => None,
        }
    })
}

/// One item of the menu: what it does when chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuChoice {
    /// An action of the table on the selection, or on every row of the list.
    Act(Action),
    /// The selected paths, one per line, to the clipboard.
    CopyPaths,
}

/// The menu for a selection in `list`: Stage or Unstage, Discard Changes… on the unstaged side
/// — disabled with its reason when `refusal` says why — Stage All or Unstage All, Copy Path.
/// Each item closes the menu as it is chosen (Freya closes it on no choice of its own).
pub fn menu(
    list: ChangeList,
    refusal: Option<&NoDiscard>,
    chosen: EventHandler<MenuChoice>,
) -> Menu {
    let item = |caption: &'static str, choice: MenuChoice| {
        let chosen = chosen.clone();
        MenuButton::new()
            .on_press(move |_: Event<PressEventData>| {
                ContextMenu::close();
                chosen.call(choice);
            })
            .child(caption)
    };
    let (one, all) = match list {
        ChangeList::Unstaged => (STAGE_CAPTION, STAGE_ALL_CAPTION),
        ChangeList::Staged => (UNSTAGE_CAPTION, UNSTAGE_ALL_CAPTION),
    };
    let discard: Vec<MenuButton> = match (list, refusal) {
        (ChangeList::Staged, _) | (ChangeList::Unstaged, Some(NoDiscard::Staged)) => Vec::new(),
        (ChangeList::Unstaged, None) => {
            vec![item(DISCARD_CAPTION, MenuChoice::Act(Action::Discard))]
        }
        (ChangeList::Unstaged, Some(why)) => vec![
            MenuButton::new().enabled(false).child(DISCARD_CAPTION),
            MenuButton::new().enabled(false).child(
                label()
                    .text(why.text())
                    .max_lines(2)
                    .width(Size::px(300.))
                    .font_size(12.),
            ),
        ],
    };
    Menu::new()
        .child(item(one, MenuChoice::Act(Action::StageOrUnstage)))
        .children(discard)
        .child(item(all, MenuChoice::Act(Action::StageOrUnstageAll)))
        .child(item(COPY_PATH_CAPTION, MenuChoice::CopyPaths))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{
        ChangedEntry, ConflictKind, ConflictedEntry, StatusEntry, SubmoduleState, UnstagedChange,
        WorkingTreeStatus,
    };

    fn lists() -> LocalChanges {
        LocalChanges::new(WorkingTreeStatus::Listed(vec![
            StatusEntry::Changed(ChangedEntry {
                path: RepoPath::from("a.rs"),
                staged: None,
                unstaged: Some(UnstagedChange::Modified),
                submodule: None,
            }),
            StatusEntry::Changed(ChangedEntry {
                path: RepoPath::from("sub"),
                staged: None,
                unstaged: Some(UnstagedChange::Modified),
                submodule: Some(SubmoduleState {
                    new_commits: true,
                    modified_content: false,
                    untracked_content: false,
                }),
            }),
            StatusEntry::Conflicted(ConflictedEntry {
                path: RepoPath::from("clash.rs"),
                kind: ConflictKind::BothModified,
                submodule: None,
            }),
            StatusEntry::Untracked(RepoPath::from("new.rs")),
        ]))
    }

    /// R3.6, R8.7, R8.8: the Staged list offers no discard whatever it holds; on the unstaged
    /// side a submodule or a conflicted path among the paths refuses, naming it; modified and
    /// untracked paths, and a path the list does not hold, do not. Caught by: a discard offered
    /// on Staged, a submodule or conflict let through, or a refusal for an ordinary path.
    #[test]
    fn a_discard_is_refused_for_staged_changes_submodules_and_conflicts_only() {
        let changes = lists();
        let path = |name: &str| RepoPath::from(name);
        assert_eq!(
            no_discard(&changes, ChangeList::Staged, &[path("a.rs")]),
            Some(NoDiscard::Staged)
        );
        assert_eq!(
            no_discard(
                &changes,
                ChangeList::Unstaged,
                &[path("a.rs"), path("new.rs"), path("gone.rs")]
            ),
            None
        );
        assert_eq!(
            no_discard(&changes, ChangeList::Unstaged, &[path("a.rs"), path("sub")]),
            Some(NoDiscard::Submodule(path("sub")))
        );
        assert_eq!(
            no_discard(&changes, ChangeList::Unstaged, &[path("clash.rs")]),
            Some(NoDiscard::Conflicted(path("clash.rs")))
        );
        assert!(
            NoDiscard::Submodule(path("sub"))
                .text()
                .contains("sub is a submodule")
        );
    }
}
