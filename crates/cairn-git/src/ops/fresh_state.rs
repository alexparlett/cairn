//! What a write is checked against, read fresh from the repository the moment it is
//! needed: the index entry at a path, read with gix, and the working-tree file, hashed as
//! its bytes are and, where a patch needs it, in git's form
//! (`docs/prd/staging-and-commit.md` R1.4, R3.7).
//!
//! Two hashes of a working-tree file, for two questions:
//!
//! - **Its bytes as they are on disk** ([`on_disk`]): the file's content hashed as a blob
//!   with no filter — what `git hash-object --no-filters` gives — and a symlink as its
//!   target, which is how git stores one and which `hash-object` cannot give, since it
//!   opens the file a link points to. Hashed here, with gix's hasher, streamed: no process,
//!   and no file held whole in memory. What a destructive operation's re-check compares,
//!   so ANY byte changed after the confirmation refuses it — a line ending under
//!   `core.autocrlf` included, which git's form does not show (phase 01's QA item 25).
//! - **Its git form** ([`git_form`]): the id git's diff gives the working-tree side —
//!   through the clean filter, the line-ending conversion, `ident` — asked of git
//!   (`crate::reads::hash_object`). What a discard of lines' patch was built against, so
//!   what its stale check compares (R3.7).
//!
//! Nothing here caches: every call reads the index file and the working tree again, so a
//! re-check reads the repository, never the copy a `Consequence` carries.

use std::io::Read as _;
use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;

use cairn_model::{FileMode, Oid, RepoPath};
use gix::bstr::ByteSlice as _;

use crate::object_id::model_id;
use crate::ops::GitBinary;
use crate::reads::hash_object;
use crate::{CancelSignal, Error, Refusal, Repository};

/// What the index holds at one path, read from the index file now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum IndexSide {
    /// No entry at any stage.
    Absent,
    /// An entry at a conflict stage: the path is conflicted.
    Conflicted,
    /// The one unconflicted entry.
    Entry {
        id: Oid,
        /// `None` for a mode git does not record for a file (a sparse directory).
        mode: Option<FileMode>,
        /// Added with `git add -N`: its id is the empty blob's, and its diffs treat it as
        /// a file not yet in the index.
        intent_to_add: bool,
    },
}

impl IndexSide {
    /// Whether this is the side a diff names as its old one: the entry with that id and
    /// mode, or — for `None`, a file new to the index — no entry or an intent-to-add one.
    pub(super) fn is_old_side(&self, id: Option<&Oid>, mode: Option<FileMode>) -> bool {
        match (self, id) {
            (Self::Absent, None) => true,
            (
                Self::Entry {
                    intent_to_add: true,
                    ..
                },
                None,
            ) => true,
            (
                Self::Entry {
                    id: entry,
                    mode: entry_mode,
                    intent_to_add: false,
                },
                Some(id),
            ) => entry == id && *entry_mode == mode,
            (Self::Absent | Self::Conflicted | Self::Entry { .. }, _) => false,
        }
    }
}

/// The index entry at `path` now. A repository with no index file has no entries, as git
/// reads it.
pub(super) fn index_side(repo: &Repository, path: &RepoPath) -> Result<IndexSide, Error> {
    let inner = repo.inner();
    let index = match inner.open_index() {
        Ok(index) => index,
        Err(gix::worktree::open_index::Error::IndexFile(gix::index::file::init::Error::Io(
            error,
        ))) if error.kind() == std::io::ErrorKind::NotFound => return Ok(IndexSide::Absent),
        Err(source) => {
            return Err(Error::ReadIndex {
                source: Box::new(source),
            });
        }
    };
    let Some(range) = index.entry_range(path.as_bytes().as_bstr()) else {
        return Ok(IndexSide::Absent);
    };
    let entries = index.entries().get(range).unwrap_or_default();
    if entries
        .iter()
        .any(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
    {
        return Ok(IndexSide::Conflicted);
    }
    let Some(entry) = entries.first() else {
        return Ok(IndexSide::Absent);
    };
    Ok(IndexSide::Entry {
        id: model_id(&entry.id)?,
        mode: FileMode::from_octal(&format!("{:o}", entry.mode.bits())),
        intent_to_add: entry
            .flags
            .contains(gix::index::entry::Flags::INTENT_TO_ADD),
    })
}

/// The working-tree file at one path, as its bytes are now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OnDisk {
    Absent,
    /// A regular file: its bytes hashed as a blob, and their count.
    File {
        id: Oid,
        bytes: u64,
    },
    /// A symlink: its target hashed as a blob, as git stores it, and the target's length.
    Symlink {
        id: Oid,
        bytes: u64,
    },
    /// A directory: what a nested repository or an untracked directory is.
    Directory,
    /// A named pipe, a socket or a device, which git does not store.
    Other,
    /// The file changed while it was read: its length is not what it was when the read
    /// began, so no id can be named for it.
    Changing,
}

impl OnDisk {
    /// The id of a file or a link; `None` for anything that has none.
    pub(super) fn id(&self) -> Option<&Oid> {
        match self {
            Self::File { id, .. } | Self::Symlink { id, .. } => Some(id),
            Self::Absent | Self::Directory | Self::Other | Self::Changing => None,
        }
    }

    /// The bytes a deletion of it takes: the file's, or the link's target's.
    pub(super) fn bytes(&self) -> Option<u64> {
        match self {
            Self::File { bytes, .. } | Self::Symlink { bytes, .. } => Some(*bytes),
            Self::Absent | Self::Directory | Self::Other | Self::Changing => None,
        }
    }
}

/// Where `path` is in the working tree. A bare repository has none, which no caller asks.
fn in_work_tree(repo: &Repository, path: &RepoPath) -> Result<PathBuf, Error> {
    crate::reads::work_tree_relative(path)?;
    let Some(workdir) = repo.workdir() else {
        return Err(Error::Refused {
            path: path.to_string(),
            why: Refusal::NotAFile,
        });
    };
    Ok(workdir.join(std::ffi::OsStr::from_bytes(path.as_bytes())))
}

/// The working-tree file at `path` now, hashed as its bytes are (module docs).
pub(super) fn on_disk(repo: &Repository, path: &RepoPath) -> Result<OnDisk, Error> {
    let file = in_work_tree(repo, path)?;
    let unreadable = |source: std::io::Error| Error::ReadWorkingTree {
        path: path.to_string(),
        source,
    };
    let metadata = match std::fs::symlink_metadata(&file) {
        Ok(metadata) => metadata,
        // A parent that is a file now (`ENOTDIR`) leaves no file at the path either.
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                || error.kind() == std::io::ErrorKind::NotADirectory =>
        {
            return Ok(OnDisk::Absent);
        }
        Err(error) => return Err(unreadable(error)),
    };
    let kind = metadata.file_type();
    let hash_kind = repo.inner().object_hash();
    if kind.is_symlink() {
        let target = std::fs::read_link(&file).map_err(unreadable)?;
        let target = target.as_os_str().as_bytes();
        let id = gix::objs::compute_hash(hash_kind, gix::objs::Kind::Blob, target).map_err(
            |source| Error::ReadWorkingTree {
                path: path.to_string(),
                source: std::io::Error::other(source),
            },
        )?;
        return Ok(OnDisk::Symlink {
            id: model_id(&id)?,
            bytes: target.len() as u64,
        });
    }
    if kind.is_dir() {
        return Ok(OnDisk::Directory);
    }
    if !kind.is_file() {
        return Ok(OnDisk::Other);
    }
    let expected = metadata.len();
    let mut reader = std::fs::File::open(&file).map_err(unreadable)?;
    let mut hasher = gix::hash::hasher(hash_kind);
    hasher.update(&gix::objs::encode::loose_header(
        gix::objs::Kind::Blob,
        expected,
    ));
    let mut buffer = vec![0u8; 64 * 1024];
    let mut read = 0u64;
    loop {
        let n = reader.read(&mut buffer).map_err(unreadable)?;
        if n == 0 {
            break;
        }
        read = read.saturating_add(n as u64);
        if read > expected {
            return Ok(OnDisk::Changing);
        }
        hasher.update(&buffer[..n]);
    }
    if read != expected {
        return Ok(OnDisk::Changing);
    }
    let id = hasher
        .try_finalize()
        .map_err(|source| Error::ReadWorkingTree {
            path: path.to_string(),
            source: std::io::Error::other(source),
        })?;
    Ok(OnDisk::File {
        id: model_id(&id)?,
        bytes: expected,
    })
}

/// The id the working-tree file at `path` has in git's form, given what [`on_disk`] read of
/// it: git's own answer for a file (`git hash-object --path`), and a link's own id, which no
/// filter touches. `None` for anything that has no id.
pub(super) fn git_form(
    git: &GitBinary,
    repo: &Repository,
    path: &RepoPath,
    disk: &OnDisk,
) -> Result<Option<Oid>, Error> {
    match disk {
        OnDisk::File { .. } => {
            // Never cancelled: it runs inside the operation and ends with it (R3.9).
            hash_object(git, repo, path, &CancelSignal::new()).map(Some)
        }
        OnDisk::Symlink { id, .. } => Ok(Some(*id)),
        OnDisk::Absent | OnDisk::Directory | OnDisk::Other | OnDisk::Changing => Ok(None),
    }
}

/// Whether an untracked directory at `path` holds a repository of its own: a `.git` in it,
/// file or directory, which is what `git status` lists as the directory itself in every
/// mode and `git clean` refuses to delete without a second `-f`.
pub(super) fn holds_a_repository(repo: &Repository, path: &RepoPath) -> Result<bool, Error> {
    let directory = in_work_tree(repo, path)?;
    Ok(std::fs::symlink_metadata(directory.join(".git")).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Oid {
        Oid::from_bytes(&[byte; 20]).unwrap()
    }

    /// Caught by: taking an intent-to-add entry for a file's old side, an entry of another
    /// mode for its own, or no entry for a modified file's.
    #[test]
    fn the_old_side_is_the_entry_or_for_a_new_file_none_or_intent_to_add() {
        let entry = IndexSide::Entry {
            id: id(1),
            mode: Some(FileMode::Regular),
            intent_to_add: false,
        };
        assert!(entry.is_old_side(Some(&id(1)), Some(FileMode::Regular)));
        assert!(!entry.is_old_side(Some(&id(2)), Some(FileMode::Regular)));
        assert!(!entry.is_old_side(Some(&id(1)), Some(FileMode::Executable)));
        assert!(!entry.is_old_side(None, None));
        let intent = IndexSide::Entry {
            id: id(1),
            mode: Some(FileMode::Regular),
            intent_to_add: true,
        };
        assert!(intent.is_old_side(None, None));
        assert!(!intent.is_old_side(Some(&id(1)), Some(FileMode::Regular)));
        assert!(IndexSide::Absent.is_old_side(None, None));
        assert!(!IndexSide::Absent.is_old_side(Some(&id(1)), Some(FileMode::Regular)));
        assert!(!IndexSide::Conflicted.is_old_side(None, None));
    }
}
