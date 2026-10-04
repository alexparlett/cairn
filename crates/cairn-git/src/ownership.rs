//! Whether a repository found by searching is the user's own, as git decides it.
//!
//! git checks ownership when it DISCOVERS a repository, never when one is named to it
//! (`--git-dir`): `ensure_valid_ownership` in git's `setup.c` (read at v2.56.0) requires
//! the current user to own every one of up to three paths — the `.git` file, when the
//! working tree reaches its git directory through one (a linked worktree, a submodule's
//! checkout); the working tree's top, the directory holding `.git`; and the git directory
//! itself, for a `.git` file the directory it names — and otherwise lets the repository open
//! only when `safe.directory`, in the configuration git protects, names its working tree
//! (or, for a bare repository, its git directory). A bare repository has only the last
//! path to check.
//!
//! Cairn names every repository it opened with full trust to the `git` it runs
//! (`process/cli.rs`), which skips git's own check, so the trust it opens with must be the
//! one git's discovery would reach: full only when every path git checks is owned, the
//! minimum over them ([`Owners::trust`]). `safe.directory` is then applied by gix as it
//! opens, raising a repository trusted less than fully to full trust exactly when that
//! setting names its working tree, or its git directory when bare
//! (`gix::open::Options`, `check_safe_directories` in gix 0.87.1's `open/repository.rs`)
//! — the setting git consults at the same point.
//!
//! The real case needs a second owner, so it is a privileged run rather than a gate step:
//! `a_linked_worktree_whose_git_dir_is_someone_elses_is_refused_as_git_refuses_it`, an
//! `#[ignore]`d test in `crates/cairn-git/tests/diff/ownership.rs`, run as root by whoever
//! reviews a change here.
//!
//! Ownership is decided by `lstat`, as both git (`is_path_owned_by_current_user` in
//! `compat/`) and gix (`gix::sec::identity::is_path_owned_by_current_user`) decide it: a
//! `.git` that is a link is the link's owner's. A path whose owner cannot be read is not
//! owned, as git's check treats it.

use std::path::{Path, PathBuf};

use crate::bare_discovery::Stop;

/// Whether the current user owns each path git checks for one repository: `None` where
/// the repository has no such path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Owners {
    /// The `.git` file naming the git directory, when there is one.
    pub(crate) gitfile: Option<bool>,
    /// The top of the working tree, the directory holding `.git`; `None` when bare.
    pub(crate) work_tree: Option<bool>,
    /// The git directory itself.
    pub(crate) git_dir: bool,
}

impl Owners {
    /// The trust git's discovery reaches before `safe.directory` is consulted: full when
    /// every path it checks is owned, and reduced when any one is not.
    pub(crate) fn trust(self) -> gix::sec::Trust {
        let every_one_owned =
            self.gitfile.unwrap_or(true) && self.work_tree.unwrap_or(true) && self.git_dir;
        if every_one_owned {
            gix::sec::Trust::Full
        } else {
            gix::sec::Trust::Reduced
        }
    }
}

/// The paths git checks for the repository its search stopped at, each answered by
/// `owned`: for a working tree, the `.git` file if `.git` is one, the directory holding
/// `.git`, and the git directory (`.git` itself, or the directory the file names, its path
/// resolved as git resolves it); for a bare repository, the git directory alone. A `.git`
/// file that names nothing readable has no git directory to own, and is not owned.
pub(crate) fn owners(stop: &Stop, owned: &dyn Fn(&Path) -> bool) -> Owners {
    match stop {
        Stop::GitDirectory(git_dir) => Owners {
            gitfile: None,
            work_tree: None,
            git_dir: owned(git_dir),
        },
        Stop::WorkTree(dot_git) => {
            let work_tree = dot_git.parent().map(owned);
            // `read_gitfile_gently` stats `.git`, following a link: a `.git` that is a
            // directory, or a link to one, is the git directory itself.
            if std::fs::metadata(dot_git).is_ok_and(|meta| meta.is_dir()) {
                return Owners {
                    gitfile: None,
                    work_tree,
                    git_dir: owned(dot_git),
                };
            }
            Owners {
                gitfile: Some(owned(dot_git)),
                work_tree,
                git_dir: named_git_dir(dot_git).is_some_and(|git_dir| owned(&git_dir)),
            }
        }
    }
}

/// The git directory a `.git` file names, resolved to a physical path as git's
/// `read_gitfile_gently` resolves it (`real_path`); `None` when it names nothing readable.
fn named_git_dir(gitfile: &Path) -> Option<PathBuf> {
    let named = gix::discover::path::from_gitdir_file(gitfile).ok()?;
    std::fs::canonicalize(named).ok()
}

/// Whether the current user owns `path`, by `lstat`; a path whose owner cannot be read is
/// not owned.
pub(crate) fn is_owned(path: &Path) -> bool {
    gix::sec::identity::is_path_owned_by_current_user(path).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use gix::sec::Trust;

    use super::*;

    /// Every combination of the three answers git distinguishes, spelled out rather than
    /// computed: full trust only when every path that exists is owned. Caught by: trust
    /// taken from the `.git` git's search stopped at and the working tree's top alone —
    /// what Cairn and gix decided before, which let a `.git` file the user owns vouch for a
    /// git directory someone else owns — or any other path's answer ignored.
    #[test]
    fn trust_is_full_only_when_every_path_git_checks_is_owned() {
        let full = Trust::Full;
        let reduced = Trust::Reduced;
        let table = [
            // A bare repository: the git directory alone.
            (None, None, true, full),
            (None, None, false, reduced),
            // A working tree whose `.git` is the git directory.
            (None, Some(true), true, full),
            (None, Some(true), false, reduced),
            (None, Some(false), true, reduced),
            (None, Some(false), false, reduced),
            // A working tree whose `.git` is a file naming the git directory.
            (Some(true), Some(true), true, full),
            (Some(true), Some(true), false, reduced),
            (Some(true), Some(false), true, reduced),
            (Some(true), Some(false), false, reduced),
            (Some(false), Some(true), true, reduced),
            (Some(false), Some(true), false, reduced),
            (Some(false), Some(false), true, reduced),
            (Some(false), Some(false), false, reduced),
        ];
        for (gitfile, work_tree, git_dir, expected) in table {
            let owners = Owners {
                gitfile,
                work_tree,
                git_dir,
            };
            assert_eq!(owners.trust(), expected, "{owners:?}");
        }
    }

    /// Which paths are asked about, for each shape git's search can stop at, recorded
    /// through the `owned` answer so no file has to change owner. Caught by: the git
    /// directory a `.git` file names left unchecked (the gap a linked worktree whose
    /// `worktrees/<name>` belongs to someone else walked through), the working tree's top
    /// left unchecked, or a bare repository given a working tree to check.
    #[test]
    fn each_path_git_checks_is_asked_about_and_no_other() {
        let scratch = std::env::temp_dir().join(format!(
            "cairn-ownership-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        let top = scratch.join("tree");
        let elsewhere = scratch.join("elsewhere.git");
        std::fs::create_dir_all(&top).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        let elsewhere = std::fs::canonicalize(&elsewhere).unwrap();

        let asked = RefCell::new(Vec::new());
        let record = |refuse: Option<PathBuf>| {
            asked.borrow_mut().clear();
            let asked = &asked;
            move |path: &Path| {
                asked.borrow_mut().push(path.to_owned());
                refuse.as_deref() != Some(path)
            }
        };

        // A bare repository: the git directory, and nothing above it.
        let owners = super::owners(&Stop::GitDirectory(elsewhere.clone()), &record(None));
        assert_eq!(*asked.borrow(), std::slice::from_ref(&elsewhere));
        assert_eq!(owners.trust(), Trust::Full);

        // A `.git` directory: the top and `.git`.
        let dot_git = top.join(".git");
        std::fs::create_dir(&dot_git).unwrap();
        let owners = super::owners(&Stop::WorkTree(dot_git.clone()), &record(Some(top.clone())));
        assert_eq!(*asked.borrow(), [top.clone(), dot_git.clone()]);
        assert_eq!(owners.trust(), Trust::Reduced, "the top is someone else's");
        std::fs::remove_dir(&dot_git).unwrap();

        // A `.git` file: the file, the top, and the directory it names — relative, as
        // `git worktree add` writes it on git 2.48 and later with
        // `worktree.useRelativePaths`, resolved against the file's own directory.
        std::fs::write(&dot_git, b"gitdir: ../elsewhere.git\n").unwrap();
        let owners = super::owners(
            &Stop::WorkTree(dot_git.clone()),
            &record(Some(elsewhere.clone())),
        );
        assert_eq!(
            *asked.borrow(),
            [top.clone(), dot_git.clone(), elsewhere.clone()]
        );
        assert_eq!(
            owners,
            Owners {
                gitfile: Some(true),
                work_tree: Some(true),
                git_dir: false,
            },
            "the git directory the file names is someone else's"
        );
        assert_eq!(owners.trust(), Trust::Reduced);

        // A `.git` file naming nothing: no git directory to own.
        std::fs::write(&dot_git, b"gitdir: ../nowhere.git\n").unwrap();
        let owners = super::owners(&Stop::WorkTree(dot_git.clone()), &record(None));
        assert_eq!(*asked.borrow(), [top.clone(), dot_git.clone()]);
        assert_eq!(owners.trust(), Trust::Reduced);

        std::fs::remove_dir_all(&scratch).unwrap();
    }
}
