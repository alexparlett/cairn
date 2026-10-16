//! `Remove index.lock…`: the one mutation Cairn makes without `git`, because git has no verb
//! that removes a lock.
//!
//! A `git` that is killed past its grace (`SIGKILL`, a crash, a power cut) leaves
//! `<gitdir>/index.lock` behind, and every later write to the index fails on it until someone
//! removes it — git's own advice is to remove it by hand. Cairn offers that removal only where
//! a write's outcome named the lock and Cairn runs no `git` in the repository
//! ([`crate::Repository::running_invocations`]), confirmed like every destructive operation:
//! its [`Consequence::RemoveLock`] names the lock's path and age and carries what identifies
//! the file — its time, size, device and inode — so the removal re-checks each of them first
//! and refuses a lock removed and made again since the prompt, which may be a running git's.
//! It removes exactly that one path, with `std::fs::remove_file`, and nothing else:
//! not another lock, not a directory, not through a symbolic link.
//!
//! What a check cannot close: another program — a `git` in a terminal, an editor's git
//! integration — may take a lock between the re-check and the removal, and nothing in `std`
//! removes a file only if it is still the inode it was. Nor does the registry count every
//! process Cairn started: a hook's child left running in the background, its pipes closed, is
//! no longer counted once its `git` is reaped, and a `git` it starts can hold the lock. Only an
//! invocation Cairn is still driving is refused; the rest is the window the prompt warns of.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use cairn_model::{Confirmed, Consequence};

use super::{Invalidated, Performed};
use crate::{Error, LockRefusal, Repository};

/// The lock file this operation is for, in `repo`'s git directory — a linked worktree's own,
/// where its index lives.
fn index_lock(repo: &Repository) -> PathBuf {
    repo.git_dir().join("index.lock")
}

/// What one look at a lock file found, as the consequence carries it.
struct Seen {
    modified: SystemTime,
    bytes: u64,
    device: u64,
    inode: u64,
}

/// Reads `path` without following a link: `None` when nothing is there, an error when what is
/// there is not a plain file.
fn look(path: &Path) -> Result<Option<Seen>, Error> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        // Unreadable — permission denied, an I/O error: said as that, never as "not a file".
        Err(error) => {
            return Err(Error::LockRefused {
                why: LockRefusal::Unreadable(error.kind()),
            });
        }
    };
    if !metadata.file_type().is_file() {
        return Err(Error::LockRefused {
            why: LockRefusal::NotAFile,
        });
    }
    Ok(Some(Seen {
        modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        bytes: metadata.len(),
        device: metadata.dev(),
        inode: metadata.ino(),
    }))
}

/// Refused while Cairn runs any `git` in the repository: the lock may be its.
fn no_git_running(repo: &Repository) -> Result<(), Error> {
    match repo.running_invocations() {
        0 => Ok(()),
        running => Err(Error::LockRefused {
            why: LockRefusal::GitRunning(running),
        }),
    }
}

/// When `repo`'s `<gitdir>/index.lock` was last changed, if one is there as a plain file: the
/// leftover lock as a state of the repository, read by one `stat` on every refresh, which
/// writes nothing. `None` when there is none — or what is there is not
/// a plain file, or cannot be looked at: nothing the removal would offer to remove.
pub fn index_lock_changed(repo: &Repository) -> Option<SystemTime> {
    look(&index_lock(repo))
        .ok()
        .flatten()
        .map(|seen| seen.modified)
}

/// What tells one `<gitdir>/index.lock` from another: its time, size, device and inode, as the
/// removal's re-check compares them. Equal identities are the same file, untouched between.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockIdentity {
    modified: SystemTime,
    bytes: u64,
    device: u64,
    inode: u64,
}

/// `repo`'s `<gitdir>/index.lock`'s identity, if one is there as a plain file: what the local
/// lane compares before and after a write's attempt, to run again only a write git refused for a
/// lock that stood, the same file, throughout it. Writes nothing.
pub fn index_lock_identity(repo: &Repository) -> Option<LockIdentity> {
    look(&index_lock(repo))
        .ok()
        .flatten()
        .map(|seen| LockIdentity {
            modified: seen.modified,
            bytes: seen.bytes,
            device: seen.device,
            inode: seen.inode,
        })
}

/// What removing `repo`'s `index.lock` would destroy, for its confirmation: the lock's path,
/// its age as of now, and what identifies it. Refused — [`Error::LockRefused`] — when there is
/// no lock, when it is not a plain file, or while Cairn runs a `git` in the repository, so the
/// removal is offered only then.
pub fn remove_lock_consequence(repo: &Repository) -> Result<Consequence, Error> {
    no_git_running(repo)?;
    let path = index_lock(repo);
    let read_at = SystemTime::now();
    let Some(seen) = look(&path)? else {
        return Err(Error::LockRefused {
            why: LockRefusal::NoLock,
        });
    };
    Ok(Consequence::RemoveLock {
        path,
        modified: seen.modified,
        read_at,
        bytes: seen.bytes,
        device: seen.device,
        inode: seen.inode,
    })
}

/// Removes `repo`'s `index.lock`, as `confirmed` names it: refused while Cairn runs a `git`
/// there, or when the confirmation is not of this repository's lock; refused, removing
/// nothing, when the lock is gone or is not the file confirmed — another time, size, device
/// or inode ([`Error::LockChangedSinceConfirmed`]); otherwise exactly that path is removed.
pub fn remove_index_lock(repo: &Repository, confirmed: Confirmed) -> Result<Performed, Error> {
    let (path, modified, bytes, device, inode) = match confirmed.consequence() {
        Consequence::RemoveLock {
            path,
            modified,
            read_at: _,
            bytes,
            device,
            inode,
        } => (path.clone(), *modified, *bytes, *device, *inode),
        Consequence::DiscardLines { .. }
        | Consequence::DiscardFiles { .. }
        | Consequence::Amend { .. }
        | Consequence::CheckoutDiscarding { .. } => {
            return Err(Error::LockRefused {
                why: LockRefusal::NotWhatWasConfirmed,
            });
        }
    };
    if path != index_lock(repo) {
        return Err(Error::LockRefused {
            why: LockRefusal::NotWhatWasConfirmed,
        });
    }
    no_git_running(repo)?;
    let changed = || Error::LockChangedSinceConfirmed {
        path: path.display().to_string(),
    };
    let seen = match look(&path) {
        Ok(Some(seen)) => seen,
        Ok(None) | Err(_) => return Err(changed()),
    };
    if (seen.modified, seen.bytes, seen.device, seen.inode) != (modified, bytes, device, inode) {
        return Err(changed());
    }
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(changed()),
        Err(source) => {
            return Err(Error::LockNotRemoved {
                path: path.display().to_string(),
                source,
            });
        }
    }
    Ok(Performed::destructive(
        "removed index.lock",
        confirmed,
        Invalidated {
            index: true,
            ..Invalidated::NOTHING
        },
    ))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::SharedRepository;

    /// A repository of its own under the temporary directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch() -> Scratch {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("cairn-remove-lock-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&dir)
            .status()
            .unwrap();
        assert!(status.success());
        Scratch(dir)
    }

    fn open(dir: &Scratch) -> Repository {
        SharedRepository::discover(dir.path()).unwrap().to_worker()
    }

    /// The leftover lock is read as its time while `<gitdir>/index.lock` is a plain file, and as
    /// nothing once it is gone, is a directory, or stands only in the work tree. The look
    /// removes nothing. Caught by: a look at the work tree's file, a directory read as a lock,
    /// or the lock's time not the file's.
    #[test]
    fn the_leftover_lock_is_its_time_while_it_is_a_file_and_nothing_otherwise() {
        let dir = scratch();
        let repo = open(&dir);
        let lock = dir.path().join(".git/index.lock");
        std::fs::write(dir.path().join("index.lock"), b"a file in the work tree").unwrap();
        assert_eq!(index_lock_changed(&repo), None);
        std::fs::write(&lock, b"").unwrap();
        let old = SystemTime::now() - Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(&lock)
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert_eq!(index_lock_changed(&repo), Some(old));
        assert!(lock.exists(), "the look removed the lock");
        std::fs::remove_file(&lock).unwrap();
        std::fs::create_dir(&lock).unwrap();
        assert_eq!(
            index_lock_changed(&repo),
            None,
            "a directory read as a lock"
        );
    }

    /// The lock's identity is its time, size, device and inode — a lock swapped for another of
    /// the same time but another size, or another inode, reads as another lock, as the removal's
    /// own re-check reads it; one left alone reads the same. Caught by: an identity of the time
    /// alone.
    #[test]
    fn a_lock_swapped_at_the_same_time_reads_as_another() {
        let dir = scratch();
        let repo = open(&dir);
        let lock = dir.path().join(".git/index.lock");
        assert_eq!(index_lock_identity(&repo), None);
        std::fs::write(&lock, b"one").unwrap();
        let when = SystemTime::now() - Duration::from_secs(60);
        let stamp = |path: &Path| {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(when)
                .unwrap();
        };
        stamp(&lock);
        let first = index_lock_identity(&repo);
        assert!(first.is_some());
        assert_eq!(
            index_lock_identity(&repo),
            first,
            "the same lock read as another"
        );
        // Rewritten in place, another size, the same time.
        std::fs::write(&lock, b"longer").unwrap();
        stamp(&lock);
        assert_ne!(
            index_lock_identity(&repo),
            first,
            "another size read as the same lock"
        );
        // Made again: another inode, the same size and time.
        let second = index_lock_identity(&repo);
        let held = dir.path().join(".git/held");
        std::fs::rename(&lock, &held).unwrap();
        std::fs::write(&lock, b"longer").unwrap();
        stamp(&lock);
        assert_ne!(
            index_lock_identity(&repo),
            second,
            "another inode read as the same lock"
        );
    }

    /// The lock confirmed is removed — exactly `<gitdir>/index.lock`, nothing beside it — and
    /// the record quotes the prompt. Caught by: a removal of anything else, or the prompt not
    /// recorded.
    #[test]
    fn the_confirmed_lock_and_nothing_else_is_removed() {
        let dir = scratch();
        let git_dir = dir.path().join(".git");
        std::fs::write(git_dir.join("index.lock"), b"partial").unwrap();
        std::fs::write(git_dir.join("HEAD.lock"), b"x").unwrap();
        std::fs::write(dir.path().join("index.lock"), b"a file in the work tree").unwrap();
        let repo = open(&dir);
        let consequence = remove_lock_consequence(&repo).unwrap();
        let prompt = consequence.prompt();
        assert!(prompt.contains("index.lock"), "{prompt}");
        let performed = remove_index_lock(&repo, Confirmed::by_user(consequence)).unwrap();
        assert_eq!(performed.acknowledged(), Some(prompt.as_str()));
        assert!(!git_dir.join("index.lock").exists());
        assert!(
            git_dir.join("HEAD.lock").exists(),
            "another lock was removed"
        );
        assert!(
            dir.path().join("index.lock").exists(),
            "a work-tree file was removed"
        );
    }

    /// A lock removed and made again since the prompt — even at the same size and time — is
    /// refused and left; so is one rewritten in place, and one gone. Caught by: a re-check by
    /// path alone, or none.
    #[test]
    fn a_lock_removed_and_made_again_since_the_prompt_is_refused() {
        let dir = scratch();
        let lock = dir.path().join(".git/index.lock");
        std::fs::write(&lock, b"one").unwrap();
        let repo = open(&dir);
        let consequence = remove_lock_consequence(&repo).unwrap();
        // Made again: a new inode, its time set back to the old one's.
        let old = std::fs::metadata(&lock).unwrap().modified().unwrap();
        let held = dir.path().join(".git/held");
        std::fs::rename(&lock, &held).unwrap();
        std::fs::write(&lock, b"one").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&lock)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let refused = remove_index_lock(&repo, Confirmed::by_user(consequence)).unwrap_err();
        assert!(
            matches!(refused, Error::LockChangedSinceConfirmed { .. }),
            "{refused:?}"
        );
        assert!(lock.exists(), "the new lock was removed");

        // Written since: another size.
        let consequence = remove_lock_consequence(&repo).unwrap();
        std::fs::write(&lock, b"longer now").unwrap();
        let refused = remove_index_lock(&repo, Confirmed::by_user(consequence)).unwrap_err();
        assert!(matches!(refused, Error::LockChangedSinceConfirmed { .. }));
        assert!(lock.exists());

        // Rewritten in place: the same inode and size, another time.
        let consequence = remove_lock_consequence(&repo).unwrap();
        let written = std::fs::metadata(&lock).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&lock)
            .unwrap()
            .set_modified(written.modified().unwrap() - std::time::Duration::from_secs(90))
            .unwrap();
        {
            use std::os::unix::fs::MetadataExt as _;
            let now = std::fs::metadata(&lock).unwrap();
            assert_eq!((now.ino(), now.len()), (written.ino(), written.len()));
        }
        let refused = remove_index_lock(&repo, Confirmed::by_user(consequence)).unwrap_err();
        assert!(
            matches!(refused, Error::LockChangedSinceConfirmed { .. }),
            "{refused:?}"
        );
        assert!(lock.exists(), "a lock with another time was removed");

        // A directory in its place since.
        let consequence = remove_lock_consequence(&repo).unwrap();
        std::fs::remove_file(&lock).unwrap();
        std::fs::create_dir(&lock).unwrap();
        let refused = remove_index_lock(&repo, Confirmed::by_user(consequence)).unwrap_err();
        assert!(
            matches!(refused, Error::LockChangedSinceConfirmed { .. }),
            "{refused:?}"
        );
        assert!(lock.is_dir(), "the directory was removed");
        std::fs::remove_dir(&lock).unwrap();

        // Gone since.
        std::fs::write(&lock, b"one").unwrap();
        let consequence = remove_lock_consequence(&repo).unwrap();
        std::fs::remove_file(&lock).unwrap();
        let refused = remove_index_lock(&repo, Confirmed::by_user(consequence)).unwrap_err();
        assert!(matches!(refused, Error::LockChangedSinceConfirmed { .. }));
    }

    /// A lock that cannot be read — its directory's search permission gone — is refused as
    /// unreadable, never as "not a file". Skipped as root, who reads it anyway.
    #[test]
    fn an_unreadable_lock_is_said_to_be_unreadable() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch();
        let git_dir = dir.path().join(".git");
        std::fs::write(git_dir.join("index.lock"), b"").unwrap();
        let repo = open(&dir);
        std::fs::set_permissions(&git_dir, std::fs::Permissions::from_mode(0o600)).unwrap();
        let readable = std::fs::symlink_metadata(git_dir.join("index.lock")).is_ok();
        let answer = remove_lock_consequence(&repo);
        std::fs::set_permissions(&git_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        if readable {
            eprintln!("SKIPPED: this user reads a directory without search permission (root)");
            return;
        }
        assert!(
            matches!(
                answer,
                Err(Error::LockRefused {
                    why: LockRefusal::Unreadable(std::io::ErrorKind::PermissionDenied)
                })
            ),
            "{answer:?}"
        );
    }

    /// Nothing to offer with no lock, or a lock that is a directory or a link; and a
    /// confirmation of another repository's lock is refused, leaving both.
    #[test]
    fn no_lock_a_lock_that_is_no_file_and_another_repositorys_lock_are_refused() {
        let dir = scratch();
        let repo = open(&dir);
        assert!(matches!(
            remove_lock_consequence(&repo),
            Err(Error::LockRefused {
                why: LockRefusal::NoLock
            })
        ));
        let lock = dir.path().join(".git/index.lock");
        std::fs::create_dir(&lock).unwrap();
        assert!(matches!(
            remove_lock_consequence(&repo),
            Err(Error::LockRefused {
                why: LockRefusal::NotAFile
            })
        ));
        std::fs::remove_dir(&lock).unwrap();
        std::os::unix::fs::symlink(dir.path().join(".git/index"), &lock).unwrap();
        assert!(matches!(
            remove_lock_consequence(&repo),
            Err(Error::LockRefused {
                why: LockRefusal::NotAFile
            })
        ));
        std::fs::remove_file(&lock).unwrap();

        let other = scratch();
        std::fs::write(other.path().join(".git/index.lock"), b"").unwrap();
        std::fs::write(&lock, b"").unwrap();
        let theirs = remove_lock_consequence(&open(&other)).unwrap();
        assert!(matches!(
            remove_index_lock(&repo, Confirmed::by_user(theirs)),
            Err(Error::LockRefused {
                why: LockRefusal::NotWhatWasConfirmed
            })
        ));
        assert!(lock.exists() && other.path().join(".git/index.lock").exists());
    }

    /// While a `git` Cairn started holds the lock and runs, the removal is not offered — and a
    /// confirmation made before it started is refused, the lock left; once it is over, the
    /// removal is offered. Caught by: offering on the lock alone, or a registry
    /// that cannot tell its own running git from none.
    #[test]
    fn no_removal_is_offered_or_made_while_cairns_git_holds_the_lock() {
        use crate::process::stub_git::{StubGit, discover_retrying};
        let dir = scratch();
        let lock = dir.path().join(".git/index.lock");
        let repo = open(&dir);
        std::fs::write(&lock, b"").unwrap();
        let before = remove_lock_consequence(&repo).unwrap();
        let stub = StubGit::with_git(&format!(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
             PATH=/usr/bin:/bin; : > '{}'; echo holding >&2; sleep 30 & wait",
            lock.display()
        ));
        let git = discover_retrying(stub.environment()).unwrap();
        let holding = git
            .read_invocation()
            .in_repository(&repo)
            .arg("hold")
            .start()
            .unwrap();
        assert_eq!(repo.running_invocations(), 1);
        assert!(matches!(
            remove_lock_consequence(&repo),
            Err(Error::LockRefused {
                why: LockRefusal::GitRunning(1)
            })
        ));
        assert!(matches!(
            remove_index_lock(&repo, Confirmed::by_user(before)),
            Err(Error::LockRefused {
                why: LockRefusal::GitRunning(1)
            })
        ));
        assert!(lock.exists());
        drop(holding);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while repo.running_invocations() > 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let after = remove_lock_consequence(&repo).unwrap();
        remove_index_lock(&repo, Confirmed::by_user(after)).unwrap();
        assert!(!lock.exists());
    }

    /// The prompt names the lock's age from the times read, never a clock of the renderer's.
    #[test]
    fn the_consequence_carries_the_locks_age() {
        let dir = scratch();
        let lock = dir.path().join(".git/index.lock");
        std::fs::write(&lock, b"").unwrap();
        let an_hour_ago = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&lock)
            .unwrap()
            .set_modified(an_hour_ago)
            .unwrap();
        let prompt = remove_lock_consequence(&open(&dir)).unwrap().prompt();
        assert!(prompt.contains("1 hour"), "{prompt}");
    }
}
