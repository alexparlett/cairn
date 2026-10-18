//! The objects git reads in place of others: the repository's replace refs.
//!
//! `git replace` writes `refs/replace/<id>`, and from then on git reads the object the ref
//! names wherever `<id>` is read, a commit's parents included, so the history git shows
//! follows the replacement (`do_lookup_replace_object` in git's `replace-object.c`, at
//! v2.30.9 and v2.56.0 alike: replaced again, up to five times). git reads them unless
//! `core.useReplaceRefs` is false. gitoxide 0.87.1 reads that setting the wrong way round
//! (`replacement_objects_refs_prefix` in its `open/repository.rs` takes an unset or true
//! setting as off, and false as on), so its object database is told to read none
//! (`walk::Grafted`) and the replacements are read here, as git reads them: every ref under
//! `refs/replace/` whose name ends, after its last `/`, in an id of the repository's hash
//! (`register_replace_ref`), the first of two for one object kept. `GIT_NO_REPLACE_OBJECTS`
//! and `GIT_REPLACE_REF_BASE` in Cairn's own environment are not honoured, as no other
//! variable naming a repository's state is.

use std::collections::HashMap;

/// How many times git looks for a replacement before it gives up (`MAXREPLACEDEPTH`).
const DEPTH: usize = 5;

/// The repository's replace refs as they were read once, for one query.
pub(crate) struct Replacements {
    /// Each replaced object and the object its ref names.
    by: HashMap<gix::ObjectId, gix::ObjectId>,
}

/// What a replacement chain longer than git follows fails with, as git dies on it.
#[derive(Debug, thiserror::Error)]
#[error("replace depth too high for object {0}")]
pub(crate) struct TooDeep(gix::ObjectId);

impl Replacements {
    pub(crate) fn read(
        repo: &gix::Repository,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut by = HashMap::new();
        if repo
            .config_snapshot()
            .try_boolean("core.useReplaceRefs")?
            .unwrap_or(true)
        {
            let hex = repo.object_hash().len_in_hex();
            let platform = repo.references()?;
            for reference in platform.prefixed("refs/replace/")? {
                let Ok(reference) = reference else {
                    continue;
                };
                let name = reference.name().as_bstr().to_vec();
                let tail = name.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
                let original = match tail.get(..hex) {
                    Some(id) if tail.len() == hex => gix::ObjectId::from_hex(id).ok(),
                    _ => None,
                };
                // The object the ref names, as git reads it (`ref->oid`), never peeled.
                let target = reference.target();
                let (Some(original), Some(replacement)) = (original, target.try_id()) else {
                    continue;
                };
                by.entry(original).or_insert(replacement.to_owned());
            }
        }
        Ok(Self { by })
    }

    /// Whether git replaces no object: one of the conditions under which git reads the
    /// commit-graph (`commit_graph_compatible`).
    pub(crate) fn is_empty(&self) -> bool {
        self.by.is_empty()
    }

    /// The object git reads for `id`: its replacement's, replaced again as git replaces it —
    /// git looks for a replacement at most [`DEPTH`] times, and gives up on the last even
    /// where it finds none.
    pub(crate) fn of(&self, id: &gix::oid) -> Result<gix::ObjectId, TooDeep> {
        let mut current = id.to_owned();
        if self.by.is_empty() {
            return Ok(current);
        }
        for _ in 0..DEPTH {
            match self.by.get(&current) {
                Some(replacement) => current = *replacement,
                None => return Ok(current),
            }
        }
        Err(TooDeep(id.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> gix::ObjectId {
        gix::ObjectId::from_bytes_or_panic(&[n; 20])
    }

    /// Caught by: a replacement not followed when itself replaced, or a chain of four
    /// replacements refused, or of five accepted — git's loop gives up after its fifth look.
    #[test]
    fn a_replacement_is_followed_as_far_as_git_follows_it() {
        let chain = |links: u8| Replacements {
            by: (0..links).map(|n| (id(n), id(n + 1))).collect(),
        };
        assert_eq!(chain(0).of(&id(0)).ok(), Some(id(0)));
        assert_eq!(chain(1).of(&id(0)).ok(), Some(id(1)));
        assert_eq!(chain(4).of(&id(0)).ok(), Some(id(4)));
        assert!(chain(5).of(&id(0)).is_err());
        assert_eq!(chain(5).of(&id(1)).ok(), Some(id(5)));
    }
}
