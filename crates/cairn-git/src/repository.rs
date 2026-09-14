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
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let inner = gix::discover(path).map_err(|source| match source {
            gix::discover::Error::Discover(_) => Error::NotARepository {
                path: path.to_owned(),
            },
            other => Error::Open {
                path: path.to_owned(),
                source: Box::new(other),
            },
        })?;
        let workdir = inner.workdir().map(Path::to_owned);
        Ok(Self { inner, workdir })
    }

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
        assert!(repo.git_dir().ends_with(".git"));
        assert!(repo.workdir().is_some());
    }

    #[test]
    fn reports_a_non_repository_as_such() {
        let err = Repository::discover("/").unwrap_err();
        assert!(matches!(err, Error::NotARepository { .. }), "got {err:?}");
    }
}
