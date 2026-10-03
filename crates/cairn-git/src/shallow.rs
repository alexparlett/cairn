//! A shallow clone's boundary: the commits whose parents the clone does not have.
//!
//! A boundary commit's object still names its parents, and gitoxide hands them over as the
//! object names them — the walk's `parent_ids` and `Commit::parent_ids` alike. git reads
//! the repository's `shallow` file and shows such a commit with NO parents: `git log
//! --format=%P` prints nothing for it, `git log --graph` draws it as a root, and `git
//! show` diffs it against the empty tree. Every read that reports a commit's parents asks
//! here, so Cairn shows what git shows.

use cairn_model::Oid;

use crate::Error;
use crate::object_id::model_id;

/// The repository's shallow file as it was read once, for one query.
pub(crate) struct ShallowBoundary {
    /// Sorted by gix as it reads the file (`gix_shallow::read`); `None` when the
    /// repository is not shallow.
    commits: Option<gix::shallow::Commits>,
}

impl ShallowBoundary {
    pub(crate) fn read(repo: &gix::Repository) -> Result<Self, gix::shallow::read::Error> {
        Ok(Self {
            commits: repo.shallow_commits()?,
        })
    }

    pub(crate) fn is_shallow(&self) -> bool {
        self.commits.is_some()
    }

    /// Whether the clone cut `id`'s parents off.
    pub(crate) fn cuts(&self, id: &gix::oid) -> bool {
        self.commits
            .as_ref()
            .is_some_and(|commits| commits.binary_search(&id.to_owned()).is_ok())
    }

    /// The parents git shows for `id`: the ones its object names, or none at the boundary.
    pub(crate) fn parents_of<'a>(
        &self,
        id: &gix::oid,
        named: impl IntoIterator<Item = &'a gix::oid>,
    ) -> Result<Vec<Oid>, Error> {
        if self.cuts(id) {
            return Ok(Vec::new());
        }
        named.into_iter().map(model_id).collect()
    }
}
