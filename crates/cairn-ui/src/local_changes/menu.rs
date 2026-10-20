//! What Local Changes offers a selection, and its context menu, in Fork's groups and with Fork's
//! chords: Stage (or Unstage) and, on the unstaged side, Discard Changes…; Stage All (or Unstage
//! All); Copy Path and Copy Full Path — Fork's menu, less the items Cairn does not offer (open,
//! external diff, blame, history, ignore, stash, save as patch).
//!
//! **What is never discarded.** Nothing on the staged side, anywhere. A conflicted path stages
//! whole — `git add`, which marks it resolved — and is never discarded; a submodule's changes
//! are never discarded, since `git restore` leaves its commit where it was and no prompt can
//! count what is dirty inside it; and a repository nested in the working tree is never touched,
//! since deleting it would take every commit only it has (Fork deletes one as an untracked
//! folder and loses its history). A selection mixing any of them with paths that can be
//! discarded discards those, and the confirmation says in one line what is left ([`Left`]),
//! where Fork asks twice. A selection with nothing that can be discarded offers its Discard
//! greyed, its reason as its tooltip ([`NoDiscard`]), and the discard chord over it does
//! nothing, as Fork's does: there is no line under the lists to say it in.

use cairn_model::{ChangeKind, ChangeList, LocalChange, LocalChanges, RepoPath};
use freya::prelude::*;

use crate::accelerators::Action;
use crate::menu::{Groups, Item, context_menu_over};

/// The menu's words.
pub const STAGE_CAPTION: &str = "Stage";
pub const UNSTAGE_CAPTION: &str = "Unstage";
pub const STAGE_ALL_CAPTION: &str = "Stage All";
pub const UNSTAGE_ALL_CAPTION: &str = "Unstage All";
pub const DISCARD_CAPTION: &str = "Discard Changes…";
pub const COPY_PATH_CAPTION: &str = "Copy Path";
pub const COPY_FULL_PATH_CAPTION: &str = "Copy Full Path";

/// A row a discard leaves as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    Submodule,
    Conflicted,
    NestedRepository,
}

/// What a discard does to `change`: `None` when it takes it, else why it leaves it as it is.
pub fn kept(change: &LocalChange<'_>) -> Option<Kept> {
    if change.is_nested_repository() {
        return Some(Kept::NestedRepository);
    }
    match change.kind {
        ChangeKind::Submodule => Some(Kept::Submodule),
        ChangeKind::Conflicted => Some(Kept::Conflicted),
        ChangeKind::Modified
        | ChangeKind::Added
        | ChangeKind::Deleted
        | ChangeKind::Renamed
        | ChangeKind::Copied
        | ChangeKind::TypeChanged => None,
    }
}

/// How many rows of each kind a discard leaves as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Left {
    pub submodules: usize,
    pub conflicted: usize,
    pub nested: usize,
}

impl Left {
    /// One more row of `kind` left.
    pub fn count(&mut self, kind: Kept) {
        match kind {
            Kept::Submodule => self.submodules += 1,
            Kept::Conflicted => self.conflicted += 1,
            Kept::NestedRepository => self.nested += 1,
        }
    }

    /// The one line a discard's confirmation adds under its prompt: "1 submodule and 1
    /// conflicted file are left as they are.", "1 nested repository is left as it is.", "2
    /// nested repositories are left as they are.", "1 submodule and 1 nested repository are
    /// left as they are." — each kind in that order, three joined "a, b and c". `None` when it
    /// leaves none.
    pub fn text(&self) -> Option<String> {
        let counted =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let kinds: Vec<String> = [
            (self.submodules, "submodule", "submodules"),
            (self.conflicted, "conflicted file", "conflicted files"),
            (self.nested, "nested repository", "nested repositories"),
        ]
        .into_iter()
        .filter(|(n, ..)| *n > 0)
        .map(|(n, one, many)| counted(n, one, many))
        .collect();
        let what = match kinds.as_slice() {
            [] => return None,
            [only] => only.clone(),
            [before @ .., last] => format!("{} and {last}", before.join(", ")),
        };
        let verb = if self.submodules + self.conflicted + self.nested == 1 {
            "is left as it is"
        } else {
            "are left as they are"
        };
        Some(format!("{what} {verb}."))
    }
}

/// Why a selection offers no discard: nothing in it can be discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoDiscard {
    /// The Staged list: staged changes are never discarded.
    Staged,
    /// Every row one a discard leaves as it is, and which kinds there were.
    Kept {
        submodules: bool,
        conflicted: bool,
        nested: bool,
    },
}

impl NoDiscard {
    /// The reason, as the menu says it beneath its greyed Discard — one template, "<what> can't
    /// be discarded. <way forward>.": one sentence per kind the selection holds, submodules
    /// first.
    pub fn text(&self) -> String {
        match self {
            Self::Staged => "Staged changes can't be discarded. Unstage them first.".to_owned(),
            Self::Kept {
                submodules,
                conflicted,
                nested,
            } => [
                (*submodules, "Submodule changes can't be discarded."),
                (
                    *conflicted,
                    "Conflicted files can't be discarded. Resolve or stage them first.",
                ),
                (*nested, "Nested repositories can't be discarded."),
            ]
            .into_iter()
            .filter_map(|(held, sentence)| held.then_some(sentence))
            .collect::<Vec<_>>()
            .join(" "),
        }
    }
}

/// Why discarding `paths` of `list` is not offered, if it is not: the Staged list, else every
/// path `list` lists one a discard leaves as it is ([`kept`]), with the kinds there were. A
/// selection with one path that can be discarded is offered, whatever else it holds. A path
/// `list` does not list is left out, as acting on the selection leaves it out, and a selection
/// of none of its paths offers nothing to refuse. A binary search per path, stopping at the
/// first path that can be discarded.
pub fn no_discard<'a>(
    changes: &LocalChanges,
    list: ChangeList,
    paths: impl IntoIterator<Item = &'a RepoPath>,
) -> Option<NoDiscard> {
    if list == ChangeList::Staged {
        return Some(NoDiscard::Staged);
    }
    let mut left = Left::default();
    for path in paths {
        let Some(change) = changes
            .row_of(list, path)
            .and_then(|row| changes.get(list, row))
        else {
            continue;
        };
        // One path a discard takes is enough to offer it.
        left.count(kept(&change)?);
    }
    (left != Left::default()).then_some(NoDiscard::Kept {
        submodules: left.submodules > 0,
        conflicted: left.conflicted > 0,
        nested: left.nested > 0,
    })
}

/// One item of the menu: what it does when chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuChoice {
    /// An action of the table on the selection, or on every row of the list.
    Act(Action),
    /// The selected paths, as git writes them from the repository's root, one per line, to the
    /// clipboard.
    CopyPaths,
    /// The selected paths, each the working tree's path joined to it, one per line, to the
    /// clipboard.
    CopyFullPaths,
}

/// The items for a selection in `list`, in Fork's groups (FS1 §8, uv14 and uv15): Stage and
/// Discard Changes… — greyed with its reason when `refusal` says why — then Stage All, then Copy
/// Path and Copy Full Path, each beside its chord; on the staged side Unstage, then Unstage All,
/// then the two copies, and no discard at all, as Fork's staged menu has none. The list's copy
/// chords choose from these items (`crate::menu::chosen_by`).
pub fn menu_groups(list: ChangeList, refusal: Option<&NoDiscard>) -> Groups<MenuChoice> {
    let copies = vec![
        Item::new(COPY_PATH_CAPTION, MenuChoice::CopyPaths).chord(Action::Copy),
        Item::new(COPY_FULL_PATH_CAPTION, MenuChoice::CopyFullPaths).chord(Action::CopyFull),
    ];
    match list {
        ChangeList::Unstaged => vec![
            vec![
                Item::new(STAGE_CAPTION, MenuChoice::Act(Action::StageOrUnstage))
                    .chord(Action::StageOrUnstage),
                Item::new(DISCARD_CAPTION, MenuChoice::Act(Action::Discard))
                    .chord(Action::Discard)
                    .greyed(refusal.map(NoDiscard::text)),
            ],
            vec![
                Item::new(
                    STAGE_ALL_CAPTION,
                    MenuChoice::Act(Action::StageOrUnstageAll),
                )
                .chord(Action::StageOrUnstageAll),
            ],
            copies,
        ],
        ChangeList::Staged => vec![
            vec![
                Item::new(UNSTAGE_CAPTION, MenuChoice::Act(Action::StageOrUnstage))
                    .chord(Action::StageOrUnstage),
            ],
            vec![
                Item::new(
                    UNSTAGE_ALL_CAPTION,
                    MenuChoice::Act(Action::StageOrUnstageAll),
                )
                .chord(Action::StageOrUnstageAll),
            ],
            copies,
        ],
    }
}

/// The context menu over [`menu_groups`], opened over the list `owner` names, which a choice
/// gives focus back to. Each item closes the menu as it is chosen.
pub fn menu(
    list: ChangeList,
    refusal: Option<&NoDiscard>,
    owner: AccessibilityId,
    chosen: EventHandler<MenuChoice>,
) -> Menu {
    context_menu_over(menu_groups(list, refusal), owner, chosen)
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
            StatusEntry::Untracked(RepoPath::from("lib/")),
        ]))
    }

    /// The Staged list offers no discard whatever it holds; on the unstaged side a selection is
    /// refused only when nothing in it can be discarded — every path a submodule, a conflicted
    /// one or a nested repository — saying so per kind, submodules first; a selection mixing
    /// them with paths that can be is offered; modified and untracked paths, and a path the
    /// list does not hold, refuse nothing. Caught by: a discard offered on Staged, a mixed
    /// selection refused whole, a nested repository read as a file, a kind's reason left out,
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
                &[path("sub"), path("clash.rs"), path("lib/"), path("a.rs")]
            ),
            None,
            "a mixed selection discards what it can"
        );
        let refused = |paths: &[RepoPath]| {
            no_discard(&changes, ChangeList::Unstaged, paths).map(|why| why.text())
        };
        assert_eq!(
            refused(&[path("sub")]).as_deref(),
            Some("Submodule changes can't be discarded.")
        );
        assert_eq!(
            refused(&[path("clash.rs")]).as_deref(),
            Some("Conflicted files can't be discarded. Resolve or stage them first.")
        );
        assert_eq!(
            refused(&[path("lib/")]).as_deref(),
            Some("Nested repositories can't be discarded.")
        );
        assert_eq!(
            refused(&[path("clash.rs"), path("sub")]).as_deref(),
            Some(
                "Submodule changes can't be discarded. Conflicted files can't be discarded. \
                 Resolve or stage them first."
            )
        );
        assert_eq!(
            refused(&[path("lib/"), path("clash.rs"), path("sub")]).as_deref(),
            Some(
                "Submodule changes can't be discarded. Conflicted files can't be discarded. \
                 Resolve or stage them first. Nested repositories can't be discarded."
            )
        );
        assert_eq!(refused(&[path("gone.rs")]), None);
    }

    /// The one line under a mixed discard's prompt counts what is left by kind, singular and
    /// plural. Caught by: a kind left out, a count or a verb that does not agree, or a line
    /// where nothing is left.
    #[test]
    fn what_a_mixed_discard_leaves_is_said_in_one_line() {
        let left = |submodules, conflicted, nested| {
            Left {
                submodules,
                conflicted,
                nested,
            }
            .text()
        };
        assert_eq!(left(0, 0, 0), None);
        for (counts, said) in [
            (
                (1, 1, 0),
                "1 submodule and 1 conflicted file are left as they are.",
            ),
            ((2, 0, 0), "2 submodules are left as they are."),
            ((1, 0, 0), "1 submodule is left as it is."),
            ((0, 1, 0), "1 conflicted file is left as it is."),
            ((0, 3, 0), "3 conflicted files are left as they are."),
            ((0, 0, 1), "1 nested repository is left as it is."),
            ((0, 0, 2), "2 nested repositories are left as they are."),
            (
                (1, 0, 1),
                "1 submodule and 1 nested repository are left as they are.",
            ),
            (
                (3, 2, 1),
                "3 submodules, 2 conflicted files and 1 nested repository are left as they are.",
            ),
        ] {
            assert_eq!(left(counts.0, counts.1, counts.2).as_deref(), Some(said));
        }
    }
}
