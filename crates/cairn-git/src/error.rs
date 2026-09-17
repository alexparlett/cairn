use std::path::PathBuf;
use std::process::ExitStatus;

use crate::ops::GitVersion;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no git repository at {path}")]
    NotARepository { path: PathBuf },

    #[error("failed to open the repository at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// `walked` is how many commits were laid out before stopping.
    #[error("the history query was cancelled after {walked} commits")]
    Cancelled { walked: usize },

    /// `HEAD` points at a branch with no commits yet.
    #[error("the repository at {path} has no commits yet")]
    UnbornHead { path: PathBuf },

    /// Usually a corrupt or missing object.
    #[error("failed to walk the history: {source}")]
    Walk {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("failed to read commit {id}: {source}")]
    ReadCommit {
        id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// No executable `git` in any directory of the `PATH` Cairn hands to
    /// subprocesses; `searched` is every directory looked in, in order.
    #[error(
        "git was not found on PATH (searched {}); Cairn needs git {required} or newer",
        Directories(searched)
    )]
    GitNotFound {
        searched: Vec<PathBuf>,
        required: GitVersion,
    },

    /// Raising `required` is a support-policy decision, not an implementation one.
    #[error("git {found} at {path} is too old; Cairn needs git {required} or newer")]
    GitTooOld {
        path: PathBuf,
        found: GitVersion,
        required: GitVersion,
    },

    /// `git --version` ran but printed something that is not a version.
    #[error(
        "could not read the version of git at {path}: `git --version` printed {output:?}; \
         Cairn needs git {required} or newer"
    )]
    GitVersionUnreadable {
        path: PathBuf,
        output: String,
        required: GitVersion,
    },

    /// The process never ran: the file is gone, not executable, or the OS refused.
    #[error("could not start {program}: {source}")]
    GitNotStarted {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// git ran and exited non-zero. `stderr` is git's own diagnostic, for the user.
    #[error("git {arguments} failed ({status}): {stderr}")]
    GitFailed {
        arguments: String,
        status: ExitStatus,
        stderr: String,
    },

    /// The user cancelled the operation and the process was ended. Not a
    /// failure to report as one: the caller asked for this. `stranded_locks`
    /// is every `*.lock` found under the git directory once the process was
    /// gone. A `SIGKILL` that landed mid-write leaves one, and so does an
    /// earlier crash — and so does a git running in a terminal right now,
    /// which the search cannot tell apart; each is what a later write to
    /// that file fails on while it is there. Empty in the common case.
    #[error("git {arguments} was cancelled{}", StrandedLocks(stranded_locks))]
    GitCancelled {
        arguments: String,
        stranded_locks: Vec<PathBuf>,
    },

    /// Reading the refs to see whether an operation moved any failed.
    #[error("failed to read the refs of the repository: {source}")]
    Refs {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The remote's configuration would have a fetch write where Cairn's
    /// never does, so no process was started. `setting` is the entry as
    /// configured (a refspec, or `remote.<name>.mirror`), `write` what it
    /// would have done; the caller shows both, and the user changes the
    /// setting or fetches from a terminal.
    #[error(
        "fetch from {remote} refused: {setting} would {write}; change that setting or fetch \
         from a terminal"
    )]
    FetchRefused {
        remote: String,
        setting: String,
        write: RefusedWrite,
    },

    /// The remote's configuration could not be read, so the fetch could not
    /// be checked and did not run.
    #[error("could not read the configuration of remote {remote}: {source}")]
    RemoteConfig {
        remote: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Why a fetch was refused; see [`Error::FetchRefused`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusedWrite {
    /// A destination under `refs/heads/`: local branches, overwritten with
    /// no reflog in a bare repository.
    LocalBranches,
    /// A destination under `refs/tags/` while pruning is on: local tags,
    /// which have no reflog, deleted. The `setting` beside it names the
    /// refspec and the prune setting that together would do it.
    LocalTags,
    /// `remote.<name>.mirror` is set. git reads it for push, not fetch, so
    /// the setting itself writes nothing on a fetch; it is refused on sight
    /// because it is what `git clone --mirror` leaves beside the
    /// `+refs/*:refs/*` refspec, and a mirror is a repository whose every
    /// ref is the remote's to overwrite (decided on issue #17).
    Mirror,
}

impl std::fmt::Display for RefusedWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::LocalBranches => {
                "write local branches (refs/heads/), which Cairn's fetch never does"
            }
            Self::LocalTags => "delete local tags while pruning, which Cairn's fetch never does",
            Self::Mirror => {
                "declare the remote a mirror, whose fetch Cairn refuses on sight (the setting \
                 itself governs push; it is what a mirror clone carries)"
            }
        })
    }
}

/// Lock files in a message: nothing when there are none, otherwise the
/// sentence a user needs to act on them, naming each path.
struct StrandedLocks<'a>(&'a [PathBuf]);

impl std::fmt::Display for StrandedLocks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }
        f.write_str(
            "; lock files remain under the git directory, which later writes will fail on while \
             they are there (stale if no other git is running here): ",
        )?;
        for (i, path) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", path.display())?;
        }
        Ok(())
    }
}

/// A search path in a message: `/usr/local/bin, /usr/bin`, or `nothing` when
/// `PATH` was unset.
struct Directories<'a>(&'a [PathBuf]);

impl std::fmt::Display for Directories<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return f.write_str("nothing: PATH is unset");
        }
        for (i, directory) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", directory.display())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Issue #19: the message names every stranded lock, and says nothing about locks
    /// when there are none — the common case, which must read as before.
    #[test]
    fn a_cancellation_names_what_it_stranded_and_is_silent_when_nothing_was() {
        let clean = Error::GitCancelled {
            arguments: "fetch origin".to_owned(),
            stranded_locks: Vec::new(),
        };
        assert_eq!(clean.to_string(), "git fetch origin was cancelled");

        let stranded = Error::GitCancelled {
            arguments: "fetch origin".to_owned(),
            stranded_locks: vec![
                PathBuf::from("/r/.git/packed-refs.lock"),
                PathBuf::from("/r/.git/refs/remotes/origin/main.lock"),
            ],
        };
        let text = stranded.to_string();
        assert!(
            text.starts_with("git fetch origin was cancelled; "),
            "{text}"
        );
        assert!(text.contains("lock file"), "{text}");
        assert!(
            text.ends_with("/r/.git/packed-refs.lock, /r/.git/refs/remotes/origin/main.lock"),
            "{text}"
        );
    }
}
