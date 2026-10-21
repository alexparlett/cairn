//! A commit row's context menu, Fork's items that work (FS1 §8, uv22; the copy items'
//! texts observed, `docs/research/ui-review/fork-copy-and-dialogs-2026-10-20.md` §1-2): New
//! Branch… beside its chord, then Copy Commit SHA and Copy Commit Info beside the copy chords.
//! With two rows selected — a comparison — Fork's menu keeps the copies alone, each copying
//! every selected commit, one per line in the list's order, the upper row first. A stash's row
//! offers nothing.

use cairn_model::{History, Oid, RowContent};

use crate::accelerators::Action;
use crate::history::list::NEW_BRANCH_CAPTION;
use crate::menu::{Groups, Item};

/// The copy items' words, Fork's.
pub const COPY_COMMIT_SHA_CAPTION: &str = "Copy Commit SHA";
pub const COPY_COMMIT_INFO_CAPTION: &str = "Copy Commit Info";

/// What a commit row's menu item does.
#[derive(Debug, Clone, PartialEq)]
pub enum RowChoice {
    /// Create Branch at this commit, its subject shown.
    NewBranch(Oid, String),
    /// This text to the clipboard.
    Copy(String),
}

/// One commit as the menu reads it: its id and subject.
pub type MenuCommit = (Oid, String);

/// The full ids, one per line: Copy Commit SHA's text.
pub fn commit_sha_text(commits: &[MenuCommit]) -> String {
    commits
        .iter()
        .map(|(id, _)| id.hex().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Each commit's short id, " - " and its subject, one per line: Copy Commit Info's text, as
/// Fork's reads `635ee8e - Add note 5`.
pub fn commit_info_text(commits: &[MenuCommit]) -> String {
    commits
        .iter()
        .map(|(id, subject)| format!("{} - {subject}", id.short()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The menu over `commits`, in the list's order: one commit's New Branch… and its copies, or
/// two's copies alone; none for no commit.
pub fn row_menu(commits: &[MenuCommit]) -> Option<Groups<RowChoice>> {
    let copies = vec![
        Item::new(
            COPY_COMMIT_SHA_CAPTION,
            RowChoice::Copy(commit_sha_text(commits)),
        )
        .chord(Action::Copy),
        Item::new(
            COPY_COMMIT_INFO_CAPTION,
            RowChoice::Copy(commit_info_text(commits)),
        )
        .chord(Action::CopyFull),
    ];
    match commits {
        [] => None,
        [(id, subject)] => Some(vec![
            vec![
                Item::new(
                    NEW_BRANCH_CAPTION,
                    RowChoice::NewBranch(*id, subject.clone()),
                )
                .chord(Action::NewBranch),
            ],
            copies,
        ]),
        [_, _, ..] => Some(vec![copies]),
    }
}

/// The commits of the rows at `indices` in `rows`, in the list's order, stashes left out.
pub fn menu_commits(rows: &History, indices: &[usize]) -> Vec<MenuCommit> {
    let mut indices = indices.to_vec();
    indices.sort_unstable();
    indices.dedup();
    indices
        .into_iter()
        .filter_map(|index| {
            let row = rows.row(index)?;
            // No wildcard arm: a stash's row offers nothing.
            match row.content() {
                RowContent::Commit(commit) => Some((commit.id, commit.summary)),
                RowContent::Stash(_) => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(hex: &str) -> Oid {
        Oid::parse(hex).unwrap()
    }

    /// Fork's texts: the full id for Copy Commit SHA, the seven-character id, " - " and the
    /// subject for Copy Commit Info, one line per commit, nothing after the last. Caught by: a
    /// short id copied as the SHA, the info's separator or order changed, or a trailing newline.
    #[test]
    fn the_copy_texts_are_forks() {
        let one = [(
            oid("635ee8e2f83b6f15e4a464f8763894d67fa658a1"),
            "Add note 5".to_owned(),
        )];
        assert_eq!(
            commit_sha_text(&one),
            "635ee8e2f83b6f15e4a464f8763894d67fa658a1"
        );
        assert_eq!(commit_info_text(&one), "635ee8e - Add note 5");
        let two = [
            one[0].clone(),
            (
                oid("e3a0d1e9d5c29b4cc7b5fdafb06c3e104563fb25"),
                "Add note 3".to_owned(),
            ),
        ];
        assert_eq!(
            commit_info_text(&two),
            "635ee8e - Add note 5\ne3a0d1e - Add note 3"
        );
    }

    /// The commits a menu reads come in the list's order, the upper row first, whatever order
    /// the rows were named in — a right press on the lower row of a pair names it first — and
    /// a row named twice counts once. Caught by: the copy text in press order.
    #[test]
    fn a_menus_commits_are_in_the_lists_order() {
        use cairn_model::{GraphRow, Lane, PagedCommit, RowsPage};
        let ids = [
            "635ee8e2f83b6f15e4a464f8763894d67fa658a1",
            "e3a0d1e9d5c29b4cc7b5fdafb06c3e104563fb25",
            "2f83b6f15e4a464f8763894d67fa658a1904d4c1",
        ];
        let mut page = RowsPage::new();
        for (n, id) in ids.iter().enumerate() {
            let subject = format!("Add note {}", 5 - n);
            page.push(
                GraphRow::new(oid(id), Lane::new(0), Vec::new()),
                PagedCommit {
                    parents: 1,
                    subject: &subject,
                    author: "Ada",
                    author_time: 0,
                },
            );
        }
        let mut rows = History::new();
        rows.append(page).unwrap();
        assert_eq!(
            menu_commits(&rows, &[2, 0, 2]),
            vec![
                (oid(ids[0]), "Add note 5".to_owned()),
                (oid(ids[2]), "Add note 3".to_owned()),
            ]
        );
    }

    /// One commit's menu is New Branch… then the two copies, each beside its chord; two
    /// commits' is the copies alone, copying both; no commit, no menu. Caught by: New Branch…
    /// offered over a comparison, a copy without its chord, or a menu over a stash.
    #[test]
    fn a_rows_menu_is_forks_for_one_commit_and_for_two() {
        let a = (
            oid("635ee8e2f83b6f15e4a464f8763894d67fa658a1"),
            "A".to_owned(),
        );
        let b = (
            oid("e3a0d1e9d5c29b4cc7b5fdafb06c3e104563fb25"),
            "B".to_owned(),
        );
        let shape = |groups: Groups<RowChoice>| -> Vec<Vec<(String, Option<Action>)>> {
            groups
                .into_iter()
                .map(|group| {
                    group
                        .into_iter()
                        .map(|item| (item.label, item.chord))
                        .collect()
                })
                .collect()
        };
        let one = row_menu(std::slice::from_ref(&a)).unwrap();
        assert_eq!(
            shape(one),
            vec![
                vec![(NEW_BRANCH_CAPTION.to_owned(), Some(Action::NewBranch))],
                vec![
                    (COPY_COMMIT_SHA_CAPTION.to_owned(), Some(Action::Copy)),
                    (COPY_COMMIT_INFO_CAPTION.to_owned(), Some(Action::CopyFull)),
                ],
            ]
        );
        let two = row_menu(&[a.clone(), b.clone()]).unwrap();
        assert_eq!(
            crate::menu::chosen_by(&two, Action::CopyFull),
            Some(RowChoice::Copy("635ee8e - A\ne3a0d1e - B".to_owned()))
        );
        assert_eq!(shape(two).len(), 1);
        assert_eq!(row_menu(&[]), None);
    }
}
