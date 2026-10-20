//! Every context menu's items, as data, against Fork's (FS1 §8; the copy items and the Commit
//! tab's menu, `docs/research/ui-review/fork-copy-and-dialogs-2026-10-20.md`): each row's
//! working items in Fork's groups and order, each beside the chord Fork shows, and no item that
//! does nothing.

use cairn_model::{
    ChangeList, ChangedEntry, HeadState, LocalChanges, Oid, Ref, RefKind, RefName, RefTarget,
    RefsSnapshot, RepoPath, SidebarRow, StagedChange, StashEntry, StatusEntry, UnstagedChange,
    WorkingTreeStatus,
};
use cairn_ui::accelerators::Action;
use cairn_ui::menu::Groups;
use cairn_ui::{FileMenuIn, file_menu, gear_menu, menu_groups, ref_menu, row_menu};

/// A menu as its rows read: each group's items, each its label, the action whose chord it
/// shows, and whether it can be chosen.
fn shape<T>(groups: Groups<T>) -> Vec<Vec<(String, Option<Action>, bool)>> {
    groups
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .map(|item| {
                    let enabled = item.is_enabled();
                    (item.label, item.chord, enabled)
                })
                .collect()
        })
        .collect()
}

fn item(label: &str, chord: Option<Action>) -> (String, Option<Action>, bool) {
    (label.to_owned(), chord, true)
}

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap_or_else(|error| panic!("{error}"))
}

fn listed(name: &str, kind: RefKind) -> Ref {
    Ref {
        name: RefName::new(name),
        kind,
        target: RefTarget::Commit(oid(1)),
        symbolic: None,
        upstream: None,
        commit_time: None,
    }
}

fn refs() -> RefsSnapshot {
    RefsSnapshot {
        refs: vec![
            listed("refs/heads/main", RefKind::LocalBranch),
            listed("refs/heads/topic", RefKind::LocalBranch),
            listed("refs/remotes/origin/main", RefKind::RemoteTracking),
            listed("refs/tags/v0.2", RefKind::Tag),
        ],
        head: HeadState::Branch(RefName::new("refs/heads/main")),
        stashes: vec![StashEntry {
            index: 0,
            message: "On main: WIP".to_owned(),
            commit: oid(9),
            base: oid(1),
        }],
        unreadable: 0,
        substitutions: cairn_model::Substitutions::default(),
    }
}

/// Each menu holds Fork's working items for its row, in Fork's groups and order, each beside
/// Fork's chord:
/// - a commit row: New Branch… (Ctrl+Shift+B) — Copy Commit SHA (Ctrl+C), Copy Commit Info
///   (Ctrl+Shift+C); two rows compared: the copies alone; a stash: none;
/// - a local branch (current or not) and a remote branch: New Branch… — Copy Branch Name
///   (Ctrl+C); a tag: New Branch… — Copy Tag Name (Ctrl+C); a stash in the sidebar: none;
/// - an unstaged file: Stage (Enter), Discard Changes… (Delete) — Stage All
///   (Ctrl+Shift+Alt+S) — Copy Path (Ctrl+C), Copy Full Path (Ctrl+Shift+C);
/// - a staged file: Unstage (Enter) — Unstage All (Ctrl+Shift+Alt+S) — the two copies, and no
///   discard;
/// - a commit's file in the Changes tab: Copy Path (Ctrl+C), Copy Full Path (Ctrl+Shift+C); in
///   the Commit tab the same two without chords, as Fork's shows none;
/// - the sidebar's gear: Copy Repository Path.
///
/// Caught by: an item that does nothing added to a menu, Fork's groups run together, a chord
/// missing beside an item or shown where Fork shows none, a discard offered on the staged side,
/// or a menu over a stash.
#[test]
fn every_context_menu_holds_its_rows_working_items() {
    // A commit row, and two compared.
    let a = (oid(1), "Add note 5".to_owned());
    let b = (oid(2), "Add note 4".to_owned());
    assert_eq!(
        shape(row_menu(std::slice::from_ref(&a)).unwrap()),
        vec![
            vec![item("New Branch…", Some(Action::NewBranch))],
            vec![
                item("Copy Commit SHA", Some(Action::Copy)),
                item("Copy Commit Info", Some(Action::CopyFull)),
            ],
        ]
    );
    assert_eq!(
        shape(row_menu(&[a, b]).unwrap()),
        vec![vec![
            item("Copy Commit SHA", Some(Action::Copy)),
            item("Copy Commit Info", Some(Action::CopyFull)),
        ]]
    );
    assert!(row_menu(&[]).is_none(), "a stash's row offers a menu");

    // The sidebar's refs, by their place in the snapshot.
    let refs = refs();
    let branch_menu = vec![
        vec![item("New Branch…", Some(Action::NewBranch))],
        vec![item("Copy Branch Name", Some(Action::Copy))],
    ];
    for index in [0, 1, 2] {
        assert_eq!(
            shape(ref_menu(SidebarRow::Ref { index, depth: 0 }, &refs).unwrap()),
            branch_menu,
            "{:?}",
            refs.refs[index as usize].name
        );
    }
    assert_eq!(
        shape(ref_menu(SidebarRow::Ref { index: 3, depth: 0 }, &refs).unwrap()),
        vec![
            vec![item("New Branch…", Some(Action::NewBranch))],
            vec![item("Copy Tag Name", Some(Action::Copy))],
        ]
    );
    assert!(ref_menu(SidebarRow::Stash { index: 0 }, &refs).is_none());
    assert_eq!(
        shape(gear_menu()),
        vec![vec![item("Copy Repository Path", None)]]
    );

    // Local Changes' lists.
    assert_eq!(
        shape(menu_groups(ChangeList::Unstaged, None)),
        vec![
            vec![
                item("Stage", Some(Action::StageOrUnstage)),
                item("Discard Changes…", Some(Action::Discard)),
            ],
            vec![item("Stage All", Some(Action::StageOrUnstageAll))],
            vec![
                item("Copy Path", Some(Action::Copy)),
                item("Copy Full Path", Some(Action::CopyFull)),
            ],
        ]
    );
    assert_eq!(
        shape(menu_groups(ChangeList::Staged, None)),
        vec![
            vec![item("Unstage", Some(Action::StageOrUnstage))],
            vec![item("Unstage All", Some(Action::StageOrUnstageAll))],
            vec![
                item("Copy Path", Some(Action::Copy)),
                item("Copy Full Path", Some(Action::CopyFull)),
            ],
        ]
    );

    // A commit's files.
    assert_eq!(
        shape(file_menu(FileMenuIn::ChangesTab)),
        vec![vec![
            item("Copy Path", Some(Action::Copy)),
            item("Copy Full Path", Some(Action::CopyFull)),
        ]]
    );
    assert_eq!(
        shape(file_menu(FileMenuIn::CommitTab)),
        vec![vec![item("Copy Path", None), item("Copy Full Path", None)]]
    );
}

/// A discard the selection cannot make greys Discard Changes… with its reason, the copies still
/// offered. Caught by: a refusal that hides the item, or one that greys the rest.
#[test]
fn a_refused_discard_is_greyed_with_its_reason_and_the_rest_stays() {
    let changes = LocalChanges::new(WorkingTreeStatus::Listed(vec![StatusEntry::Changed(
        ChangedEntry {
            path: RepoPath::from("sub"),
            staged: None::<StagedChange>,
            unstaged: Some(UnstagedChange::Modified),
            submodule: Some(cairn_model::SubmoduleState {
                new_commits: true,
                modified_content: false,
                untracked_content: false,
            }),
        },
    )]));
    let refusal = cairn_ui::no_discard(&changes, ChangeList::Unstaged, [&RepoPath::from("sub")])
        .unwrap_or_else(|| panic!("a submodule's discard was offered"));
    let groups = menu_groups(ChangeList::Unstaged, Some(&refusal));
    let discard = &groups[0][1];
    assert_eq!(discard.reason.as_deref(), Some(refusal.text().as_str()));
    assert!(groups[2].iter().all(|item| item.is_enabled()));
}
