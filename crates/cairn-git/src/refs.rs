//! The tips of every ref, for telling whether an operation moved one.

use std::collections::BTreeMap;

use cairn_model::{Oid, RefName};

use crate::{Error, Repository};

impl Repository {
    /// Every ref and what it points at, read through gitoxide as it stands
    /// now (a subprocess's writes included). Symbolic refs resolve to their
    /// target's id; one that resolves to nothing is left out. What the worker
    /// compares before and after a fetch to decide whether the history on
    /// screen is stale.
    pub fn ref_tips(&self) -> Result<BTreeMap<RefName, Oid>, Error> {
        let refs = self.inner().references().map_err(|source| Error::Refs {
            source: Box::new(source),
        })?;
        let all = refs.all().map_err(|source| Error::Refs {
            source: Box::new(source),
        })?;
        let mut tips = BTreeMap::new();
        for reference in all {
            let mut reference = reference.map_err(|source| Error::Refs { source })?;
            let name = RefName::new(reference.name().as_bstr().to_string());
            if let Ok(id) = reference.peel_to_id()
                && let Ok(oid) = Oid::from_bytes(id.as_bytes())
            {
                tips.insert(name, oid);
            }
        }
        Ok(tips)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against whatever refs this checkout has — a CI checkout is detached with no
    /// local branch, only remote-tracking refs — so nothing here names a branch. That
    /// the tips follow the refs git writes is `ref_tips_follow_the_refs_git_writes` in
    /// `tests/fetch.rs`, over a fixture built with real git.
    #[test]
    fn this_checkout_has_ref_tips_and_two_reads_agree() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let tips = repo.ref_tips().unwrap();
        assert!(!tips.is_empty(), "a checkout with no refs at all");
        assert!(
            tips.keys().all(|name| name.as_str().starts_with("refs/")),
            "a tip outside refs/: {:?}",
            tips.keys().collect::<Vec<_>>()
        );
        let again = repo.ref_tips().unwrap();
        assert_eq!(tips, again, "two reads of the same refs differ");
    }
}
