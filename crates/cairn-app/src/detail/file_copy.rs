//! What a commit's file's menu does, from the Commit tab or the Changes tab: Copy Path and Copy
//! Full Path, read from the change set answered for the selection drawn.

use cairn_ui::FileChoice;

use crate::diff::state::answered_changes;
use crate::window::View;

/// The text `choice` copies for the file at `index` of the change set answered: its path as
/// git writes it — the path it has after the change, a rename's new one — or that path joined to
/// the repository's root; `None` when no such file is answered, or no root is known for a full
/// path.
pub fn text(view: View, index: usize, choice: FileChoice) -> Option<String> {
    let diff = view.diff.peek();
    let path = answered_changes(&diff).files.get(index)?.new_path.clone();
    match choice {
        FileChoice::CopyPath => Some(path.display().into_owned()),
        FileChoice::CopyFullPath => {
            let root = view.repository_path.peek().clone()?;
            Some(crate::clipboard::full_path(&root, &path))
        }
    }
}

/// The file's menu item `choice` chosen on the file at `index`.
pub fn chosen(view: View, index: usize, choice: FileChoice) {
    if let Some(text) = text(view, index, choice) {
        crate::clipboard::copy(text);
    }
}
