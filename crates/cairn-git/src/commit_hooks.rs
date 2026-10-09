//! Which of `pre-commit` and `commit-msg` git would run for a commit here
//! (`docs/prd/staging-and-commit.md` R6.6): the two hooks `--no-verify` skips, so the commit
//! box offers the skip only where one of them exists.
//!
//! Where hooks live is git's answer, `git rev-parse --git-path hooks`
//! ([`crate::reads::hooks_path`]), so `core.hooksPath` and a linked worktree count as git
//! counts them. A hook counts when git would run it: `find_hook` in git's `run-command.c`
//! takes the file when `access(path, X_OK)` allows it, following a link. The check here is
//! the permission bits as `access` reads them for this process — the owner's bit when the
//! effective uid owns the file, any bit for root — with one residual: for a file owned by
//! someone else, the group's bit counts whether or not this user is in its group, since
//! the process's groups are not read.

use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;

use cairn_model::CommitHooks;

use crate::ops::GitBinary;
use crate::ownership::Identity;
use crate::{Cancel, Error, Repository};

impl Repository {
    /// The commit hooks git would run here (module docs): one read of where hooks live,
    /// cancelled through `cancel` as any read is, and two looks in that directory.
    pub fn commit_hooks(
        &self,
        git: &GitBinary,
        cancel: &impl Cancel,
    ) -> Result<CommitHooks, Error> {
        let directory = crate::reads::hooks_path(git, self, cancel)?;
        let euid = Identity::of_this_process().euid;
        Ok(CommitHooks {
            pre_commit: runs(&directory.join("pre-commit"), euid),
            commit_msg: runs(&directory.join("commit-msg"), euid),
        })
    }
}

/// Whether `access(path, X_OK)` would let git run `path` for `euid` (module docs).
fn runs(path: &Path, euid: Option<u32>) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let mode = meta.permissions().mode();
    match euid {
        Some(0) => mode & 0o111 != 0,
        Some(uid) if uid == meta.uid() => mode & 0o100 != 0,
        Some(_) | None => mode & 0o011 != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a hook counted whose owner may not run it, one counted that does not
    /// exist, or a link not followed.
    #[test]
    fn a_hook_counts_where_access_would_let_its_owner_run_it() {
        let root = std::env::temp_dir().join(format!("cairn-commit-hooks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let file = |name: &str, mode: u32| {
            let path = root.join(name);
            std::fs::write(&path, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            path
        };
        let euid = Identity::of_this_process().euid;
        let mine = euid.filter(|uid| *uid != 0);
        let runnable = file("runnable", 0o700);
        let others_only = file("others-only", 0o611);
        let plain = file("plain", 0o644);
        std::os::unix::fs::symlink(&runnable, root.join("link")).unwrap();
        assert!(runs(&runnable, mine));
        assert!(!runs(&plain, mine));
        assert!(
            runs(&root.join("link"), mine),
            "a link to a hook was not followed"
        );
        assert!(!runs(&root.join("missing"), mine));
        if mine.is_some() {
            assert!(
                !runs(&others_only, mine),
                "the owner may not run a file whose owner bit is clear"
            );
        }
        assert!(runs(&others_only, Some(0)), "root runs any executable file");
        assert!(runs(&others_only, Some(u32::MAX)), "another user's bit");
        let _ = std::fs::remove_dir_all(&root);
    }
}
