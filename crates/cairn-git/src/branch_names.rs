//! Whether a name can be a new branch's (`docs/prd/staging-and-commit.md` R11.3; the user's
//! decision, 2026-10-09): Create Branch refuses a name inline, before git runs — one git does
//! not take as a branch's, by git's own rules (`crate::reads::branch_name`), one a local
//! branch has already, one holding `@{` (refused before git is asked), or one git cannot lock
//! beside a branch: a branch at a directory on the way to it (`baz` for `baz/qux`), or one
//! under it (`foo/bar` for `foo`).

use cairn_model::BranchName;

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// Why a name holding `@{` is refused, before git is asked (the user's decision F,
/// 2026-10-09): git reads `@{` as the start of a reflog selector, and its own refusal names a
/// ref rather than a branch name.
const AT_BRACE_REFUSAL: &str = "A branch name can't contain '@{'";

impl Repository {
    /// What git says of `name` as a new branch's name, and whether a local branch has it: one
    /// `git check-ref-format --branch` read, then the ref, each proper prefix of it and the
    /// namespace under it looked up by gix. `cancel` stops the read.
    pub fn branch_name(
        &self,
        git: &GitBinary,
        name: &str,
        cancel: &impl Cancel,
    ) -> Result<BranchName, Error> {
        // The user's decision F (2026-10-09): refused in Cairn's words before git is asked.
        if name.contains("@{") {
            return Ok(BranchName::Refused {
                reason: AT_BRACE_REFUSAL.to_owned(),
            });
        }
        let taken_as = match crate::reads::branch_name(git, self, name, cancel)? {
            Ok(taken_as) => taken_as,
            Err(reason) => return Ok(BranchName::Refused { reason }),
        };
        let reference = format!("refs/heads/{taken_as}");
        let find = |name: &str| -> Result<bool, Error> {
            self.inner()
                .try_find_reference(name)
                .map(|found| found.is_some())
                .map_err(|source| Error::Refs {
                    source: Box::new(source),
                })
        };
        if find(&reference)? {
            return Ok(BranchName::Taken);
        }
        // A branch at a directory on the way to it, or one under it: git cannot lock the ref,
        // and says so in these words (phase 10's QA, item 15).
        let parts: Vec<&str> = taken_as.split('/').collect();
        for end in 1..parts.len() {
            let above = format!(
                "refs/heads/{}",
                parts.get(..end).unwrap_or_default().join("/")
            );
            if find(&above)? {
                return Ok(BranchName::Refused {
                    reason: format!("'{above}' exists; cannot create '{reference}'"),
                });
            }
        }
        let under = format!("{reference}/");
        let platform = self.inner().references().map_err(|source| Error::Refs {
            source: Box::new(source),
        })?;
        let below = platform
            .prefixed(under.as_str())
            .map_err(|source| Error::Refs {
                source: Box::new(source),
            })?
            .filter_map(Result::ok)
            .map(|found| found.name().as_bstr().to_string())
            .find(|name| name.starts_with(&under));
        Ok(match below {
            Some(below) => BranchName::Refused {
                reason: format!("'{below}' exists; cannot create '{reference}'"),
            },
            None => BranchName::Free,
        })
    }
}
