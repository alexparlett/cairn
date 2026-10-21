//! What the sidebar's menus do: the gear's Copy Repository Path, and a ref's New Branch… and
//! Copy Branch Name or Copy Tag Name, each read from the snapshot the rows were laid out from —
//! the row the menu was opened on, by its place in it.

use std::path::Path;

use cairn_model::{RefsSnapshot, RowContent, SidebarRow};
use cairn_ui::{GearChoice, RefChoice};

use crate::window::View;

/// The text Copy Repository Path puts on the clipboard: the repository's root as the worker
/// opened it, absolute.
pub fn repository_path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The text Copy Branch Name or Copy Tag Name puts on the clipboard for `row`: the ref's name
/// past its namespace — `feature/login`, `origin/main`, `v0.2` — or nothing for a row that is no
/// ref.
pub fn ref_name_text(row: SidebarRow, refs: &RefsSnapshot) -> Option<String> {
    match row {
        SidebarRow::Ref { index, .. } => refs
            .refs
            .get(index as usize)
            .map(|listed| listed.name.shorthand().to_owned()),
        SidebarRow::Section { .. }
        | SidebarRow::Folder { .. }
        | SidebarRow::DetachedHead
        | SidebarRow::Stash { .. } => None,
    }
}

/// The gear's menu chosen.
pub fn gear(choice: GearChoice, view: View) {
    match choice {
        GearChoice::CopyRepositoryPath => {
            let Some(text) = view
                .repository_path
                .peek()
                .as_deref()
                .map(repository_path_text)
            else {
                return;
            };
            copy(text);
        }
    }
}

/// A ref's menu chosen on `row`: New Branch… opens Create Branch at the ref's commit, its
/// subject the loaded row's — found among the rows refs label, never every row — or none where
/// it is not loaded; Copy puts its name on the clipboard.
pub fn chosen(row: SidebarRow, choice: RefChoice, view: View) {
    let Some(refs) = view
        .sidebar
        .state
        .peek()
        .shown()
        .map(|shown| shown.refs.clone())
    else {
        return;
    };
    match choice {
        RefChoice::CopyName => {
            if let Some(text) = ref_name_text(row, &refs) {
                copy(text);
            }
        }
        RefChoice::NewBranch => {
            let SidebarRow::Ref { index, .. } = row else {
                return;
            };
            let Some(at) = refs
                .refs
                .get(index as usize)
                .and_then(cairn_model::Ref::commit_id)
            else {
                return;
            };
            let subject = {
                let rows = view.rows.peek();
                rows.labelled_position(at)
                    .and_then(|index| rows.row(index))
                    // No wildcard arm: a ref's row is a commit's, but every kind is named.
                    .map(|row| match row.content() {
                        RowContent::Commit(commit) => commit.summary,
                        RowContent::Stash(stash) => stash.message,
                    })
                    .unwrap_or_default()
            };
            crate::operations::create_branch::open(view, at, subject);
        }
    }
}

fn copy(text: String) {
    crate::clipboard::copy(text);
}

#[cfg(test)]
mod tests {
    use cairn_model::{HeadState, Oid, Ref, RefKind, RefName, RefTarget};

    use super::*;

    fn listed(name: &str, kind: RefKind) -> Ref {
        Ref {
            name: RefName::new(name),
            kind,
            target: RefTarget::Commit(Oid::from_bytes(&[1; 20]).unwrap()),
            symbolic: None,
            upstream: None,
            commit_time: None,
        }
    }

    /// A ref's name is copied past its namespace — a branch in a folder whole, a remote's with
    /// its remote, a tag by its name — and a row that is no ref copies nothing. Caught by: the
    /// leaf name copied where the row draws it, the full ref name, or a folder's text copied.
    #[test]
    fn a_refs_name_is_copied_past_its_namespace() {
        let refs = RefsSnapshot {
            refs: vec![
                listed("refs/heads/feature/login", RefKind::LocalBranch),
                listed("refs/remotes/origin/main", RefKind::RemoteTracking),
                listed("refs/tags/v0.2", RefKind::Tag),
            ],
            head: HeadState::Branch(RefName::new("refs/heads/feature/login")),
            stashes: Vec::new(),
            unreadable: 0,
            substitutions: cairn_model::Substitutions::default(),
        };
        let name = |index| ref_name_text(SidebarRow::Ref { index, depth: 1 }, &refs);
        assert_eq!(name(0).as_deref(), Some("feature/login"));
        assert_eq!(name(1).as_deref(), Some("origin/main"));
        assert_eq!(name(2).as_deref(), Some("v0.2"));
        assert_eq!(
            ref_name_text(
                SidebarRow::Folder {
                    first: 0,
                    depth: 0,
                    open: true
                },
                &refs
            ),
            None
        );
        assert_eq!(ref_name_text(SidebarRow::DetachedHead, &refs), None);
    }

    #[test]
    fn the_repository_path_is_copied_whole() {
        assert_eq!(
            repository_path_text(Path::new("/srv/work/cairn")),
            "/srv/work/cairn"
        );
    }
}
