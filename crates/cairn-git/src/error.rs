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
