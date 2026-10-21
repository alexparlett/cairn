//! The menu a commit's changed file opens, in the Commit tab and the Changes tab alike: Fork's
//! Copy Path and Copy Full Path — the items of Fork's menu that work, the rest (open, reset,
//! blame, history, save as) not offered. The Changes tab's shows each beside its chord, which
//! its list hears (FS1 §8, uv23); the Commit tab's shows none and hears none, as Fork's
//! (`docs/research/ui-review/fork-copy-and-dialogs-2026-10-20.md` §4, cp52-cp59).

use freya::prelude::*;

use crate::accelerators::Action;
use crate::local_changes::menu::{COPY_FULL_PATH_CAPTION, COPY_PATH_CAPTION};
use crate::menu::{Groups, Item, context_menu_over};

/// What a file's menu item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChoice {
    /// The file's path as git writes it from the repository's root, to the clipboard.
    CopyPath,
    /// The file's path joined to the working tree's, to the clipboard.
    CopyFullPath,
}

/// Which tab a file's menu is opened in: its chords are the Changes tab's alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMenuIn {
    ChangesTab,
    CommitTab,
}

/// The items of a commit's file's menu: Copy Path and Copy Full Path, one group, with the copy
/// chords in the Changes tab, whose list's chords choose from these (`crate::menu::chosen_by`).
pub fn file_menu(shown_in: FileMenuIn) -> Groups<FileChoice> {
    let path = Item::new(COPY_PATH_CAPTION, FileChoice::CopyPath);
    let full = Item::new(COPY_FULL_PATH_CAPTION, FileChoice::CopyFullPath);
    vec![match shown_in {
        FileMenuIn::ChangesTab => vec![path.chord(Action::Copy), full.chord(Action::CopyFull)],
        FileMenuIn::CommitTab => vec![path, full],
    }]
}

/// The context menu over [`file_menu`] for the file at `index` of the change set, opened over
/// the list `owner` names; a choice is reported with the file.
pub(crate) fn open_file_menu(
    shown_in: FileMenuIn,
    owner: AccessibilityId,
    index: usize,
    on_choose: EventHandler<(usize, FileChoice)>,
) {
    ContextMenu::open_from_down(context_menu_over(
        file_menu(shown_in),
        owner,
        move |choice: FileChoice| on_choose.call((index, choice)),
    ));
}
