//! Amend's staged list (`docs/prd/staging-and-commit.md` R6.3): what an amend will commit
//! that `HEAD`'s parent does not have — the index against `HEAD^`, or against the empty tree
//! for a root commit — as the user's `git diff --cached HEAD^` lists it.
//!
//! Through the plumbing the working-tree query's staged side runs, `git diff-index
//! --cached`, over the whole index (`crate::reads::staged_since`), a rename paired under the
//! user's `diff.renames` and `diff.renameLimit`. `HEAD`'s parent is the one git shows: a
//! shallow clone's boundary commit has none (`crate::commit::details_of`), and is compared
//! with the empty tree as a root commit is. A merge commit's is its first parent.
//!
//! A read never lazily fetches (`crate::reads`): in a partial clone, a staged inexact rename
//! whose old blob only the promisor holds fails the whole list on git 2.44 and later, and is
//! fetched by git before 2.44, which ignores `GIT_NO_LAZY_FETCH`
//! (`in_a_partial_clone_amends_staged_list_fails_rather_than_fetching`).

use cairn_model::ChangedFile;

use crate::object_id::model_id;
use crate::ops::GitBinary;
use crate::reads::staged_since;
use crate::{Cancel, Error, Repository};

use super::renames::Configured;

impl Repository {
    /// Amend's staged list (module docs), in git's order. An unborn branch, which has
    /// nothing to amend, is [`Error::UnbornHead`]; a superseded read
    /// [`Error::ContentCancelled`].
    pub fn amend_staged(
        &self,
        git: &GitBinary,
        cancel: &impl Cancel,
    ) -> Result<Vec<ChangedFile>, Error> {
        let inner = self.inner();
        let walk_error = |source: Box<dyn std::error::Error + Send + Sync>| Error::Walk { source };
        let mut head = inner
            .head()
            .map_err(|source| walk_error(Box::new(source)))?;
        let Some(head) = head
            .try_peel_to_id()
            .map_err(|source| walk_error(Box::new(source)))?
        else {
            return Err(Error::UnbornHead {
                path: self.git_dir().to_owned(),
            });
        };
        let details = self.commit_details(&model_id(&head)?)?;
        let base = match details.parents.first() {
            Some(parent) => *parent,
            None => model_id(&gix::ObjectId::empty_tree(inner.object_hash()))?,
        };
        let detection = Configured::read(inner)?.search(git.version()).detection();
        staged_since(git, self, &base, detection, cancel)
    }
}
