//! Whether a name can be a new branch's (`docs/prd/staging-and-commit.md` R11.3; the user's
//! decision, 2026-10-09): Create Branch refuses a name inline, before git runs — one git does
//! not take as a branch's, by git's own rules (`crate::reads::branch_name`), one a local
//! branch has already, one holding `@{` (refused before git is asked), or one git cannot lock
//! beside a branch: a branch at a directory on the way to it (`baz` for `baz/qux`), or one
//! under it (`foo/bar` for `foo`). Each refusal is typed ([`NameRefusal`]) and worded by the
//! view (the review's M3, 2026-10-10). `git check-ref-format --branch` is the oracle `git
//! branch` and `git checkout -b` agree with: both verbs resolve `@{-N}` as it does, where
//! `check-ref-format refs/heads/<name>` would take `-x` and `HEAD`, which they refuse — pinned
//! against real git by `a_name_is_taken_or_refused_as_gits_verbs_take_it`.

use cairn_model::{BranchName, NameRefusal};

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

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
        // The user's decision F (2026-10-09): refused before git is asked, since git reads `@{`
        // as the start of a reflog selector and would take `@{-1}` as another branch's name.
        if name.contains("@{") {
            return Ok(BranchName::Refused(NameRefusal::AtBrace));
        }
        let taken_as = match crate::reads::branch_name(git, self, name, cancel)? {
            Ok(taken_as) => taken_as,
            Err(reason) => return Ok(BranchName::Refused(NameRefusal::Invalid { reason })),
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
            return Ok(BranchName::Refused(NameRefusal::Taken));
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
                return Ok(BranchName::Refused(NameRefusal::InsideABranch {
                    branch: above,
                }));
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
            Some(below) => BranchName::Refused(NameRefusal::HoldsABranch { branch: below }),
            None => BranchName::Free,
        })
    }
}
