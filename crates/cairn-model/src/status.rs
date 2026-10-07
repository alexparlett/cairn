//! The working tree's status as `git status` reports it: per path, what is staged, what
//! is not, a submodule's state, a conflict's kind, or that git does not track it — and the
//! states in which git gives no list at all (PRD R3.2, R3.7).
//!
//! Plain data, read from `git status --porcelain=v2 -z` by the engine. Every path is the
//! bytes git printed, so a name with a newline, a leading space or bytes that are not
//! UTF-8 is held as it is.

use crate::{RepoPath, Similarity};

/// What a status read answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkingTreeStatus {
    /// What git listed, in the order it listed it: changed tracked paths, then conflicts,
    /// then untracked paths. Empty for a clean working tree.
    Listed(Vec<StatusEntry>),
    /// git could not read the index, so it listed nothing — not "nothing changed" (R3.7).
    IndexUnreadable(UnreadableIndex),
    /// The repository is bare: there is no working tree to have a status, and no `git`
    /// was asked.
    NoWorkingTree,
}

/// Why the git in use could not read the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnreadableIndex {
    /// The index is a sparse index (it carries the `sdir` extension), which git reads only
    /// from 2.32; the git in use is older.
    Sparse,
}

/// One path `git status` listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusEntry {
    /// A tracked path with a staged change, an unstaged change, or both (porcelain v2's
    /// `1` and `2` records).
    Changed(ChangedEntry),
    /// A path with unmerged index entries (a `u` record).
    Conflicted(ConflictedEntry),
    /// A path git does not track and does not ignore (a `?` record). One per file, except
    /// a nested repository, which git lists as its directory, with a trailing `/`.
    Untracked(RepoPath),
}

impl StatusEntry {
    /// The path the entry is about: for a rename or a copy, its destination.
    pub fn path(&self) -> &RepoPath {
        match self {
            Self::Changed(entry) => &entry.path,
            Self::Conflicted(entry) => &entry.path,
            Self::Untracked(path) => path,
        }
    }
}

/// A tracked path's changes: `HEAD` against the index (`staged`) and the index against
/// the working tree (`unstaged`). At least one is a change, unless git listed the path for
/// its submodule state alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedEntry {
    pub path: RepoPath,
    pub staged: Option<StagedChange>,
    pub unstaged: Option<UnstagedChange>,
    /// `Some` when the path is a submodule on any side.
    pub submodule: Option<SubmoduleState>,
}

/// What changed between `HEAD` and the index: `git diff --cached`, with the rename and
/// copy detection the user's configuration gives `git status` (R3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedChange {
    Added,
    Modified,
    Deleted,
    /// A file became a symlink or a submodule, or the reverse.
    TypeChanged,
    /// Renamed from `from`, `similarity` alike.
    Renamed {
        from: RepoPath,
        similarity: Similarity,
    },
    /// Copied from `from`, which is still there, `similarity` alike.
    Copied {
        from: RepoPath,
        similarity: Similarity,
    },
}

/// What changed between the index and the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnstagedChange {
    Modified,
    Deleted,
    TypeChanged,
    /// The path is an intent-to-add entry (`git add -N`): in the index with no content yet.
    IntentToAdd,
    /// An intent-to-add entry git paired with `from`, a tracked path gone from the working
    /// tree whose content it carries — the only rename git's status finds between the
    /// index and the working tree. `from` is listed nowhere else.
    Renamed {
        from: RepoPath,
        similarity: Similarity,
    },
    /// An intent-to-add entry git found to be a copy of `from`, under copy detection.
    Copied {
        from: RepoPath,
        similarity: Similarity,
    },
}

/// A submodule's state, as git's status reports it under the user's
/// `submodule.<name>.ignore` and `diff.ignoreSubmodules` (R3.6): each is `false` where the
/// setting has git not look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubmoduleState {
    /// Its checked-out commit differs from the one the index records.
    pub new_commits: bool,
    /// Its tracked files have changes.
    pub modified_content: bool,
    /// It has untracked files.
    pub untracked_content: bool,
}

/// A path with unmerged index entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictedEntry {
    pub path: RepoPath,
    pub kind: ConflictKind,
    /// `Some` when the path is a submodule on any side.
    pub submodule: Option<SubmoduleState>,
}

/// Which of git's seven unmerged states a path is in, decided by which of the three index
/// stages — 1 the common ancestor, 2 ours, 3 theirs — it has entries in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictKind {
    /// Stage 1 only: `DD`.
    BothDeleted,
    /// Stage 2 only: `AU`.
    AddedByUs,
    /// Stages 1 and 2: `UD`.
    DeletedByThem,
    /// Stage 3 only: `UA`.
    AddedByThem,
    /// Stages 1 and 3: `DU`.
    DeletedByUs,
    /// Stages 2 and 3: `AA`.
    BothAdded,
    /// All three stages: `UU`.
    BothModified,
}

impl ConflictKind {
    /// Every kind, in the order of git's stage mask (1 to 7).
    pub const ALL: [Self; 7] = [
        Self::BothDeleted,
        Self::AddedByUs,
        Self::DeletedByThem,
        Self::AddedByThem,
        Self::DeletedByUs,
        Self::BothAdded,
        Self::BothModified,
    ];

    /// The two letters `git status` prints for the kind (`wt-status.c`'s stage-mask table).
    pub fn code(self) -> &'static [u8; 2] {
        match self {
            Self::BothDeleted => b"DD",
            Self::AddedByUs => b"AU",
            Self::DeletedByThem => b"UD",
            Self::AddedByThem => b"UA",
            Self::DeletedByUs => b"DU",
            Self::BothAdded => b"AA",
            Self::BothModified => b"UU",
        }
    }

    /// The kind git prints as `code`; `None` for any two bytes that are not one of the
    /// seven.
    pub fn from_code(code: &[u8]) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.code() == code)
    }

    /// The kind of a path with entries in exactly the stages `mask` names — bit 0 stage 1,
    /// bit 1 stage 2, bit 2 stage 3 — as git computes it; `None` for no stage at all or a
    /// bit past stage 3.
    pub fn from_stages(mask: u8) -> Option<Self> {
        match mask {
            1..=7 => Self::ALL.get(usize::from(mask) - 1).copied(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: two kinds swapped in the table (`UD` read as deleted by us), a code
    /// that two kinds share, or a code outside the seven accepted.
    #[test]
    fn each_conflict_code_is_its_own_kind_and_reads_back() {
        let codes: Vec<&[u8; 2]> = ConflictKind::ALL.iter().map(|kind| kind.code()).collect();
        assert_eq!(
            codes,
            [b"DD", b"AU", b"UD", b"UA", b"DU", b"AA", b"UU"],
            "git's table, stage mask 1 to 7"
        );
        for kind in ConflictKind::ALL {
            assert_eq!(ConflictKind::from_code(kind.code()), Some(kind));
        }
        for refused in [&b"UU "[..], b"U", b"MM", b"..", b"du", b""] {
            assert_eq!(ConflictKind::from_code(refused), None, "{refused:?}");
        }
    }

    /// The stage sets of `git ls-files -u` name the same kinds as the letters do — the
    /// independent reading the engine's tests check git's letters against. Caught by: a
    /// mask off by one, or a mask past stage 3 read as a kind.
    #[test]
    fn each_set_of_stages_is_the_kind_git_names_for_it() {
        let expected = [
            (0b001, ConflictKind::BothDeleted),
            (0b010, ConflictKind::AddedByUs),
            (0b011, ConflictKind::DeletedByThem),
            (0b100, ConflictKind::AddedByThem),
            (0b101, ConflictKind::DeletedByUs),
            (0b110, ConflictKind::BothAdded),
            (0b111, ConflictKind::BothModified),
        ];
        for (mask, kind) in expected {
            assert_eq!(
                ConflictKind::from_stages(mask),
                Some(kind),
                "mask {mask:#05b}"
            );
        }
        assert_eq!(ConflictKind::from_stages(0), None);
        assert_eq!(ConflictKind::from_stages(8), None);
        assert_eq!(ConflictKind::from_stages(0xff), None);
    }

    /// Caught by: a rename's entry answering its source, or a path rewritten on the way.
    #[test]
    fn an_entry_names_its_own_path_and_a_rename_its_destination() {
        let renamed = StatusEntry::Changed(ChangedEntry {
            path: RepoPath::from("new name"),
            staged: Some(StagedChange::Renamed {
                from: RepoPath::from("old name"),
                similarity: Similarity::from_percent(90),
            }),
            unstaged: None,
            submodule: None,
        });
        assert_eq!(renamed.path(), &RepoPath::from("new name"));
        let conflicted = StatusEntry::Conflicted(ConflictedEntry {
            path: RepoPath::new(b"bad\xffname".to_vec()),
            kind: ConflictKind::BothAdded,
            submodule: None,
        });
        assert_eq!(conflicted.path().as_bytes(), b"bad\xffname");
        let untracked = StatusEntry::Untracked(RepoPath::new(b"new\nline".to_vec()));
        assert_eq!(untracked.path().as_bytes(), b"new\nline");
    }
}
