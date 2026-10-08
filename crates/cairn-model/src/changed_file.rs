//! One path in a change set: what happened to it, and what it looked like either side.

use crate::{Oid, RepoPath};

/// A file mode as git records it. git keeps no other value for a blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileMode {
    Regular,
    Executable,
    Symlink,
    Submodule,
}

impl FileMode {
    /// The six octal digits a patch header spells.
    pub fn octal(self) -> &'static str {
        match self {
            Self::Regular => "100644",
            Self::Executable => "100755",
            Self::Symlink => "120000",
            Self::Submodule => "160000",
        }
    }

    pub fn from_octal(digits: &str) -> Option<Self> {
        match digits {
            "100644" => Some(Self::Regular),
            "100755" => Some(Self::Executable),
            "120000" => Some(Self::Symlink),
            "160000" => Some(Self::Submodule),
            _ => None,
        }
    }
}

/// How alike two paths are, as git's `similarity index N%` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Similarity(u8);

impl Similarity {
    /// Anything past a hundred is a hundred: the header spells a percentage.
    pub fn from_percent(percent: u8) -> Self {
        Self(percent.min(100))
    }

    pub fn percent(self) -> u8 {
        self.0
    }
}

/// What happened to a path between two versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeStatus {
    Added,
    Deleted,
    Modified,
    /// The blob became a symlink, a submodule, or stopped being one.
    TypeChanged,
    Renamed(Similarity),
    Copied(Similarity),
}

/// One path's row in a change set (R1.1). It carries no line counts, by decision L10.
///
/// `old_path` and `new_path` name the same path for anything but a rename or a copy —
/// which is what git's own `diff --git` line spells, even for an added or a deleted file.
/// A mode or an id is absent exactly when the file did not exist on that side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub status: ChangeStatus,
    pub old_path: RepoPath,
    pub new_path: RepoPath,
    pub old_mode: Option<FileMode>,
    pub new_mode: Option<FileMode>,
    pub old_id: Option<Oid>,
    pub new_id: Option<Oid>,
}

impl ChangedFile {
    /// True when the path itself moved, which a patch spells as `rename from`/`rename to`.
    pub fn is_rename(&self) -> bool {
        matches!(self.status, ChangeStatus::Renamed(_))
    }

    /// True when one path was left where it was and a second appeared beside it.
    pub fn is_copy(&self) -> bool {
        matches!(self.status, ChangeStatus::Copied(_))
    }

    /// How alike the two paths are, for the statuses that carry it.
    pub fn similarity(&self) -> Option<Similarity> {
        match self.status {
            ChangeStatus::Renamed(similarity) | ChangeStatus::Copied(similarity) => {
                Some(similarity)
            }
            ChangeStatus::Added
            | ChangeStatus::Deleted
            | ChangeStatus::Modified
            | ChangeStatus::TypeChanged => None,
        }
    }

    /// True when both sides exist and their modes differ; a patch spells those as their
    /// own `old mode`/`new mode` lines rather than on the `index` line.
    pub fn mode_changed(&self) -> bool {
        match (self.old_mode, self.new_mode) {
            (Some(old), Some(new)) => old != new,
            _ => false,
        }
    }

    /// The same path's change seen from its other end (R2.1): paths, modes and ids swapped,
    /// an addition a deletion and a deletion an addition, a rename renamed back — so the
    /// forward patch rule over the inverted file and [`crate::TextDiff::inverted`] undoes
    /// the original (L17a).
    ///
    /// A copy is the one change that does not invert into its own kind: `B` copied from `A`
    /// left `A` where it was, so undoing it is `B` deleted — a deletion at the copy's
    /// destination, carrying its mode and id, and nothing of the source. Inverted as a copy
    /// back it would say "copy `B` to `A`", a path that already exists.
    pub fn inverted(&self) -> ChangedFile {
        let status = match self.status {
            ChangeStatus::Added => ChangeStatus::Deleted,
            ChangeStatus::Deleted => ChangeStatus::Added,
            ChangeStatus::Copied(_) => {
                return ChangedFile {
                    status: ChangeStatus::Deleted,
                    old_path: self.new_path.clone(),
                    new_path: self.new_path.clone(),
                    old_mode: self.new_mode,
                    new_mode: None,
                    old_id: self.new_id,
                    new_id: None,
                };
            }
            status @ (ChangeStatus::Modified
            | ChangeStatus::TypeChanged
            | ChangeStatus::Renamed(_)) => status,
        };
        ChangedFile {
            status,
            old_path: self.new_path.clone(),
            new_path: self.old_path.clone(),
            old_mode: self.new_mode,
            new_mode: self.old_mode,
            old_id: self.new_id,
            new_id: self.old_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(path: &str, status: ChangeStatus) -> ChangedFile {
        ChangedFile {
            status,
            old_path: RepoPath::from(path),
            new_path: RepoPath::from(path),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: None,
            new_id: None,
        }
    }

    /// The digits go into a patch header verbatim; a wrong one applies the wrong mode.
    #[test]
    fn every_mode_round_trips_through_its_octal_digits() {
        for mode in [
            FileMode::Regular,
            FileMode::Executable,
            FileMode::Symlink,
            FileMode::Submodule,
        ] {
            assert_eq!(
                FileMode::from_octal(mode.octal()),
                Some(mode),
                "{mode:?} did not read back from {}",
                mode.octal()
            );
        }
        assert_eq!(FileMode::from_octal("100640"), None);
        assert_eq!(FileMode::from_octal(""), None);
    }

    /// Caught by: taking a fraction for a percentage, which writes `similarity index 0%`.
    #[test]
    fn a_similarity_is_a_percentage_and_stops_at_a_hundred() {
        assert_eq!(Similarity::from_percent(87).percent(), 87);
        assert_eq!(Similarity::from_percent(100).percent(), 100);
        assert_eq!(Similarity::from_percent(255).percent(), 100);
    }

    #[test]
    fn only_a_rename_or_a_copy_carries_a_similarity() {
        let similarity = Similarity::from_percent(90);
        assert_eq!(
            at("f", ChangeStatus::Renamed(similarity)).similarity(),
            Some(similarity)
        );
        assert_eq!(
            at("f", ChangeStatus::Copied(similarity)).similarity(),
            Some(similarity)
        );
        for status in [
            ChangeStatus::Added,
            ChangeStatus::Deleted,
            ChangeStatus::Modified,
            ChangeStatus::TypeChanged,
        ] {
            assert_eq!(
                at("f", status).similarity(),
                None,
                "{status:?} answered with a similarity"
            );
        }
        assert!(at("f", ChangeStatus::Renamed(similarity)).is_rename());
        assert!(!at("f", ChangeStatus::Renamed(similarity)).is_copy());
        assert!(at("f", ChangeStatus::Copied(similarity)).is_copy());
    }

    fn id(byte: u8) -> Option<Oid> {
        Oid::from_bytes(&[byte; 20]).ok()
    }

    /// R2.1: everything with two sides swaps them. Caught by: a field left on its side —
    /// a path, a mode or an id — which a patch then writes the wrong way round.
    #[test]
    fn inverting_swaps_paths_modes_and_ids() {
        let similarity = Similarity::from_percent(80);
        let renamed = ChangedFile {
            status: ChangeStatus::Renamed(similarity),
            old_path: RepoPath::from("old.rs"),
            new_path: RepoPath::from("new.rs"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Executable),
            old_id: id(1),
            new_id: id(2),
        };
        assert_eq!(
            renamed.inverted(),
            ChangedFile {
                status: ChangeStatus::Renamed(similarity),
                old_path: RepoPath::from("new.rs"),
                new_path: RepoPath::from("old.rs"),
                old_mode: Some(FileMode::Executable),
                new_mode: Some(FileMode::Regular),
                old_id: id(2),
                new_id: id(1),
            }
        );
        assert_eq!(renamed.inverted().inverted(), renamed);

        for status in [ChangeStatus::Modified, ChangeStatus::TypeChanged] {
            let mut file = at("f", status);
            file.new_mode = Some(FileMode::Symlink);
            file.old_id = id(3);
            file.new_id = id(4);
            let inverted = file.inverted();
            assert_eq!(inverted.status, status, "{status:?} changed kind");
            assert_eq!(inverted.old_mode, Some(FileMode::Symlink));
            assert_eq!(inverted.new_mode, Some(FileMode::Regular));
            assert_eq!((inverted.old_id, inverted.new_id), (id(4), id(3)));
        }
    }

    /// An addition undone is a deletion and the reverse, the absent side moving with it.
    /// Caught by: a status left as it was, which writes `new file mode` over a file being
    /// taken away.
    #[test]
    fn an_addition_inverts_to_a_deletion_and_a_deletion_to_an_addition() {
        let added = ChangedFile {
            status: ChangeStatus::Added,
            old_path: RepoPath::from("n"),
            new_path: RepoPath::from("n"),
            old_mode: None,
            new_mode: Some(FileMode::Executable),
            old_id: None,
            new_id: id(5),
        };
        let deleted = added.inverted();
        assert_eq!(deleted.status, ChangeStatus::Deleted);
        assert_eq!(deleted.old_mode, Some(FileMode::Executable));
        assert_eq!(deleted.new_mode, None);
        assert_eq!((deleted.old_id, deleted.new_id), (id(5), None));
        assert_eq!(deleted.inverted(), added);
    }

    /// A copy undone is its destination deleted, the source left alone. Caught by: a copy
    /// inverted into a copy back onto its source, which already exists.
    #[test]
    fn a_copy_inverts_to_its_destination_deleted() {
        let copied = ChangedFile {
            status: ChangeStatus::Copied(Similarity::from_percent(100)),
            old_path: RepoPath::from("a"),
            new_path: RepoPath::from("b"),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Executable),
            old_id: id(6),
            new_id: id(7),
        };
        assert_eq!(
            copied.inverted(),
            ChangedFile {
                status: ChangeStatus::Deleted,
                old_path: RepoPath::from("b"),
                new_path: RepoPath::from("b"),
                old_mode: Some(FileMode::Executable),
                new_mode: None,
                old_id: id(7),
                new_id: None,
            }
        );
    }

    /// Caught by: reading a mode change off a side that does not exist, which would put
    /// `old mode`/`new mode` on an added or a deleted file.
    #[test]
    fn a_side_that_does_not_exist_is_not_a_mode_change() {
        let mut file = at("f", ChangeStatus::Modified);
        assert!(!file.mode_changed(), "equal modes read as a change");

        file.new_mode = Some(FileMode::Executable);
        assert!(file.mode_changed(), "a real mode change went unseen");

        file.old_mode = None;
        assert!(!file.mode_changed(), "an added file claimed a mode change");

        file.old_mode = Some(FileMode::Regular);
        file.new_mode = None;
        assert!(!file.mode_changed(), "a deleted file claimed a mode change");
    }
}
