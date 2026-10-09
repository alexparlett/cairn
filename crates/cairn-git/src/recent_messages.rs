//! The commit box's Recent Commit Messages (`docs/prd/staging-and-commit.md` R6.7): the
//! messages of the latest commits on the current branch, newest first, as Fork lists them —
//! what `git log -n 10 --format=%B HEAD` prints, read from the history walk on a worker and
//! never stored.

use crate::history::{HistoryOrder, walk};
use crate::{Cancel, Error, Repository};

/// How many messages the menu holds, as Fork's does.
pub const RECENT_MESSAGES: usize = 10;

impl Repository {
    /// The messages of the [`RECENT_MESSAGES`] latest commits `HEAD` reaches, in `git log`'s
    /// order, each exactly as written — in the characters git shows, through the encoding
    /// the commit names, as the Commit tab reads it. None on an unborn branch. `cancel` is
    /// polled between commits; a cancelled read is [`Error::Cancelled`].
    pub fn recent_messages(&self, cancel: &impl Cancel) -> Result<Vec<String>, Error> {
        let walk_error = |source: Box<dyn std::error::Error + Send + Sync>| Error::Walk { source };
        let mut head = self
            .inner()
            .head()
            .map_err(|source| walk_error(Box::new(source)))?;
        let Some(tip) = head
            .try_peel_to_id()
            .map_err(|source| walk_error(Box::new(source)))?
        else {
            return Ok(Vec::new());
        };
        let cancelled = |walked: usize| Error::Cancelled { walked };
        let Some(walk) = walk::open(
            self.inner(),
            &[tip.detach()],
            HistoryOrder::CommitTime,
            cancel,
        )?
        else {
            return Err(cancelled(0));
        };
        let mut messages = Vec::with_capacity(RECENT_MESSAGES);
        for commit in walk.take(RECENT_MESSAGES) {
            if cancel.is_cancelled() {
                return Err(cancelled(messages.len()));
            }
            let commit = commit.map_err(|source| walk_error(Box::new(source)))?;
            let id = crate::object_id::model_id(&commit.id)?;
            messages.push(self.commit_details(&id)?.message);
        }
        Ok(messages)
    }
}
