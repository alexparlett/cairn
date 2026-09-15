use std::path::{Path, PathBuf};

use crate::Error;

/// An opened repository.
///
/// Holds the gitoxide handle privately. Borrow it inside this crate with
/// [`Repository::inner`]; it is not part of the public surface.
pub struct Repository {
    inner: gix::Repository,
    workdir: Option<PathBuf>,
}

impl std::fmt::Debug for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repository")
            .field("git_dir", &self.inner.git_dir())
            .field("workdir", &self.workdir)
            .finish()
    }
}

impl Repository {
    /// Open the repository containing `path`, walking upwards like `git` does.
    ///
    /// Opening installs a small object cache. Walking by committer date looks
    /// each commit up twice without one: measured over 50k commits of a
    /// repository with no commit-graph file, 178 ms became 116 ms, and a cache
    /// larger than [`Self::OBJECT_CACHE_BYTES`] bought nothing further
    /// (`docs/work/history-graph/progress.md`, open question O2).
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let mut inner = gix::discover(path).map_err(|source| match source {
            gix::discover::Error::Discover(_) => Error::NotARepository {
                path: path.to_owned(),
            },
            other => Error::Open {
                path: path.to_owned(),
                source: Box::new(other),
            },
        })?;
        inner.object_cache_size_if_unset(Self::OBJECT_CACHE_BYTES);
        let workdir = inner.workdir().map(Path::to_owned);
        Ok(Self { inner, workdir })
    }

    /// How much memory one open repository spends on caching decoded objects.
    /// Measured, not guessed: see [`Self::discover`].
    pub const OBJECT_CACHE_BYTES: usize = 4 * 1024 * 1024;

    /// The `.git` directory backing this repository.
    pub fn git_dir(&self) -> &Path {
        self.inner.git_dir()
    }

    /// The working tree root, or `None` for a bare repository.
    pub fn workdir(&self) -> Option<&Path> {
        self.workdir.as_deref()
    }

    pub(crate) fn inner(&self) -> &gix::Repository {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_this_repository_from_a_nested_path() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        // Not `ends_with(".git")`: a linked worktree — which is how this
        // repository asks parallel work to be checked out — is backed by
        // `.git/worktrees/<name>`, so the thing to assert is that the path is
        // a git directory, not what it happens to be called.
        assert!(
            repo.git_dir().join("HEAD").is_file(),
            "{} is not a git directory",
            repo.git_dir().display()
        );
        let workdir = repo.workdir().unwrap();
        assert!(workdir.join("crates/cairn-git/Cargo.toml").is_file());
    }

    #[test]
    fn reports_a_non_repository_as_such() {
        let err = Repository::discover("/").unwrap_err();
        assert!(matches!(err, Error::NotARepository { .. }), "got {err:?}");
    }
}
