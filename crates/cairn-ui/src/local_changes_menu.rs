//! What Local Changes offers a selection, and its context menu (staging-and-commit R8.2, R8.4,
//! R8.7, R8.8): Stage or Unstage, Discard Changes…, Stage All or Unstage All, and Copy Path —
//! Fork's menu, less what this packet does not build (open, external diff, blame, history,
//! ignore, stash, save as patch).
//!
//! **What is never discarded.** Nothing on the staged side, anywhere (R3.6). A conflicted path
//! stages whole — `git add`, which marks it resolved — and is never discarded (R8.7, L25); a
//! submodule's changes are never discarded, since `git restore` leaves its commit where it was
//! and no prompt can count what is dirty inside it (R8.8, L24). A selection mixing either with
//! paths that can be discarded discards those, and the confirmation says in one line what is
//! left ([`left_as_they_are`]; the redesign's D1, where Fork asks twice). A selection with
//! nothing that can be discarded offers its Discard greyed, the reason wrapped beneath it and
//! never cut ([`NoDiscard`]; R14.4), and the discard chord over it does nothing, as Fork's
//! does (rule 4: there is no line under the lists to say it in).

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

/// Why a selection offers no discard: nothing in it can be discarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoDiscard {
    /// The Staged list: staged changes are never discarded (R3.6).
    Staged,
    /// Every path a submodule or a conflicted one, the first a submodule (R8.8).
    Submodule(RepoPath),
    /// Every path a submodule or a conflicted one, the first conflicted (R8.7).
    Conflicted(RepoPath),
}

impl NoDiscard {
    /// The reason, as the menu says it beneath its greyed Discard.
    pub fn text(&self) -> String {
        match self {
            Self::Staged => "Staged changes can't be discarded. Unstage them first.".to_owned(),
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

/// What a discard does to a row of `kind`: whether it can take it at all (R8.7, R8.8).
pub fn discards(kind: ChangeKind) -> bool {
    match kind {
        ChangeKind::Submodule | ChangeKind::Conflicted => false,
        ChangeKind::Modified
        | ChangeKind::Added
        | ChangeKind::Deleted
        | ChangeKind::Renamed
        | ChangeKind::Copied
        | ChangeKind::TypeChanged => true,
    }
}

/// Why discarding `paths` of `list` is not offered, if it is not: the Staged list, else every
/// path `list` lists a submodule or a conflicted one, the first of them named. A selection with
/// one path that can be discarded is offered, whatever else it holds (the redesign's D1). A
/// path `list` does not list is left out, as acting on the selection leaves it out, and a
/// selection of none of its paths offers nothing to refuse. A binary search per path, stopping
/// at the first path that can be discarded.
pub fn no_discard<'a>(
    changes: &LocalChanges,
    list: ChangeList,
    paths: impl IntoIterator<Item = &'a RepoPath>,
) -> Option<NoDiscard> {
    if list == ChangeList::Staged {
        return Some(NoDiscard::Staged);
    }
    let mut first = None;
    for path in paths {
        let Some(change) = changes
            .row_of(list, path)
            .and_then(|row| changes.get(list, row))
        else {
            continue;
        };
        if discards(change.kind) {
            return None;
        }
        if first.is_none() {
            first = Some(match change.kind {
                ChangeKind::Submodule => NoDiscard::Submodule(path.clone()),
                ChangeKind::Conflicted
                | ChangeKind::Modified
                | ChangeKind::Added
                | ChangeKind::Deleted
                | ChangeKind::Renamed
                | ChangeKind::Copied
                | ChangeKind::TypeChanged => NoDiscard::Conflicted(path.clone()),
            });
        }
    }
    first
}

/// The one line a discard's confirmation adds under its prompt when the selection held rows it
/// leaves (the redesign's D1): "1 submodule and 1 conflicted file are left as they are.", "2
/// submodules are left as they are.", "1 conflicted file is left as it is." `None` when it
/// leaves none.
pub fn left_as_they_are(submodules: usize, conflicted: usize) -> Option<String> {
    let counted =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let (what, rows) = match (submodules, conflicted) {
        (0, 0) => return None,
        (n, 0) => (counted(n, "submodule", "submodules"), n),
        (0, n) => (counted(n, "conflicted file", "conflicted files"), n),
        (s, c) => (
            format!(
                "{} and {}",
                counted(s, "submodule", "submodules"),
                counted(c, "conflicted file", "conflicted files")
            ),
            s + c,
        ),
    };
    let verb = if rows == 1 {
        "is left as it is"
    } else {
        "are left as they are"
    };
    Some(format!("{what} {verb}."))
}

/// One item of the menu: what it does when chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuChoice {
    /// An action of the table on the selection, or on every row of the list.
    Act(Action),
    /// The selected paths, one per line, to the clipboard.
    CopyPaths,
}

/// The menu for a selection in `list`: Stage or Unstage, Discard Changes… — greyed with its
/// reason wrapped beneath it when `refusal` says why, as it always does on the staged side
/// (R8.4, R14.4) — Stage All or Unstage All, Copy Path. Each item closes the menu as it is
/// chosen (Freya closes it on no choice of its own).
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
    // Never offered on the staged side, whatever `refusal` says.
    let refusal = match list {
        ChangeList::Staged => Some(&NoDiscard::Staged),
        ChangeList::Unstaged => refusal,
    };
    let discard: Vec<MenuButton> = match refusal {
        None => vec![item(DISCARD_CAPTION, MenuChoice::Act(Action::Discard))],
        // The reason wraps and is never cut (rule 6).
        Some(why) => vec![
            MenuButton::new().enabled(false).child(DISCARD_CAPTION),
            MenuButton::new().enabled(false).child(
                label()
                    .text(why.text())
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

    /// R3.6, R8.7, R8.8 and the redesign's D1: the Staged list offers no discard whatever it
    /// holds; on the unstaged side a selection is refused only when nothing in it can be
    /// discarded — every path a submodule or a conflicted one — and a selection mixing them
    /// with paths that can be is offered; modified and untracked paths, and a path the list
    /// does not hold, refuse nothing. Caught by: a discard offered on Staged, a mixed selection
    /// refused whole (the first offender's refusal), a submodule or conflict alone let through,
    /// or a refusal for an ordinary path.
    #[test]
    fn a_discard_is_refused_only_where_nothing_can_be_discarded() {
        let changes = lists();
        let path = |name: &str| RepoPath::from(name);
        assert_eq!(
            no_discard(&changes, ChangeList::Staged, &[path("a.rs")]),
            Some(NoDiscard::Staged)
        );
        assert_eq!(
            NoDiscard::Staged.text(),
            "Staged changes can't be discarded. Unstage them first."
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
            no_discard(
                &changes,
                ChangeList::Unstaged,
                &[path("sub"), path("clash.rs"), path("a.rs")]
            ),
            None,
            "a mixed selection discards what it can"
        );
        assert_eq!(
            no_discard(
                &changes,
                ChangeList::Unstaged,
                &[path("sub"), path("clash.rs")]
            ),
            Some(NoDiscard::Submodule(path("sub")))
        );
        assert_eq!(
            no_discard(&changes, ChangeList::Unstaged, &[path("clash.rs")]),
            Some(NoDiscard::Conflicted(path("clash.rs")))
        );
        assert_eq!(
            no_discard(&changes, ChangeList::Unstaged, &[path("gone.rs")]),
            None
        );
    }

    /// The redesign's D1: the one line under a mixed discard's prompt counts what is left by
    /// kind, in the user's approved words, singular and plural. Caught by: a kind left out, a
    /// count or a verb that does not agree, or a line where nothing is left.
    #[test]
    fn what_a_mixed_discard_leaves_is_said_in_one_line() {
        assert_eq!(left_as_they_are(0, 0), None);
        assert_eq!(
            left_as_they_are(1, 1).as_deref(),
            Some("1 submodule and 1 conflicted file are left as they are.")
        );
        assert_eq!(
            left_as_they_are(2, 0).as_deref(),
            Some("2 submodules are left as they are.")
        );
        assert_eq!(
            left_as_they_are(1, 0).as_deref(),
            Some("1 submodule is left as it is.")
        );
        assert_eq!(
            left_as_they_are(0, 1).as_deref(),
            Some("1 conflicted file is left as it is.")
        );
        assert_eq!(
            left_as_they_are(0, 3).as_deref(),
            Some("3 conflicted files are left as they are.")
        );
        assert_eq!(
            left_as_they_are(3, 2).as_deref(),
            Some("3 submodules and 2 conflicted files are left as they are.")
        );
    }
}
