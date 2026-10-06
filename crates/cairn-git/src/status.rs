//! The working tree's status, as `git status` answers it (PRD R3): the engine's public face
//! of `crate::reads`' status read, which says what git is asked and why.

use cairn_model::WorkingTreeStatus;

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

impl Repository {
    /// What `git status` lists for this repository's working tree: every staged and unstaged
    /// change, conflict and untracked file (one per file, unless the user's
    /// `status.showUntrackedFiles` is `no`), never an ignored one — or the state in which git
    /// lists nothing ([`WorkingTreeStatus::IndexUnreadable`], and
    /// [`WorkingTreeStatus::NoWorkingTree`] for a bare repository, which runs nothing).
    ///
    /// It runs `git status --porcelain=v2 -z` as a read, once, or twice where the first answer
    /// collapsed an untracked directory; it writes no index, object, ref or configuration. It
    /// blocks, so it is a worker's call. `cancel` is polled while git runs: a superseded read
    /// ends git and answers [`Error::StatusCancelled`]. A failure is [`Error::GitFailed`], and
    /// an answer this engine cannot read is [`Error::UnexpectedGitOutput`].
    pub fn status(
        &self,
        git: &GitBinary,
        cancel: &impl Cancel,
    ) -> Result<WorkingTreeStatus, Error> {
        crate::reads::status(git, self, cancel)
    }
}
