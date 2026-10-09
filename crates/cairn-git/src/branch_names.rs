//! Whether a name can be a new branch's (`docs/prd/staging-and-commit.md` R11.3; the user's
//! decision, 2026-10-09): Create Branch refuses a name inline, before git runs — one git does
//! not take as a branch's, by git's own rules (`crate::reads::branch_name`), or one a local
//! branch has already.

use cairn_model::BranchName;

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

impl Repository {
    /// What git says of `name` as a new branch's name, and whether a local branch has it: one
    /// `git check-ref-format --branch` read, then the ref looked up by gix. `cancel` stops the
    /// read.
    pub fn branch_name(
        &self,
        git: &GitBinary,
        name: &str,
        cancel: &impl Cancel,
    ) -> Result<BranchName, Error> {
        let taken_as = match crate::reads::branch_name(git, self, name, cancel)? {
            Ok(taken_as) => taken_as,
            Err(reason) => return Ok(BranchName::Refused { reason }),
        };
        let reference = format!("refs/heads/{taken_as}");
        Ok(match self.inner().try_find_reference(reference.as_str()) {
            Ok(Some(_)) => BranchName::Taken,
            Ok(None) => BranchName::Free,
            Err(source) => {
                return Err(Error::Refs {
                    source: Box::new(source),
                });
            }
        })
    }
}
