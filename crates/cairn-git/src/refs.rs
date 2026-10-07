//! The refs snapshot: every local branch, remote-tracking ref and tag, the stash list and
//! `HEAD`, read through gitoxide as `git for-each-ref`, `git stash list`,
//! `git symbolic-ref` and `git rev-parse` answer them (PRD R1).
//!
//! gix enumerates the refs — loose and packed merged, bytewise by full name, an invalid
//! name skipped as git skips it — and five rules make its answer git's
//! (`docs/research/refs-and-status/gix-refs-and-status-api.md`):
//!
//! 1. **A symbolic ref is never peeled into its target's name.** The refs are read
//!    unpeeled, and a symbolic one is listed as itself, naming its target; gix's own
//!    peeled iteration replaces `refs/remotes/origin/HEAD` with a second
//!    `refs/remotes/origin/main`.
//! 2. **A dangling symbolic ref is hidden**, as `git for-each-ref` hides it, rather than
//!    counted as unreadable.
//! 3. **The stash list is the `refs/stash` reflog read oldest first, then reversed**
//!    (`stash`): gix's newest-first reader stops at the first line over 4 KiB.
//! 4. **`remote = .` is resolved by hand** (`upstream`): gix answers no tracking ref for a
//!    local upstream, where git's is the `merge` ref itself.
//! 5. **Every other upstream is resolved by hand too** (`upstream`): gix takes the last
//!    `merge` where git takes the first, maps a short `merge` git maps to nothing, and of
//!    two refspecs mapping the merge does not take the first.
//!
//! A ref that cannot be read — content that is not a ref, a symbolic chain past git's
//! depth — is skipped and counted, never a failure of the whole snapshot: git skips it too,
//! warning `ignoring broken ref`. So is a ref naming an object that is not there, which is
//! a deliberate divergence (the user's decision): git's `for-each-ref` refuses to list
//! anything at all, `fatal: missing object`.
//!
//! `GIT_NAMESPACE` is not honoured: the repository is opened without it
//! (`SharedRepository::discover_for`), so these are the refs the `git` Cairn runs sees.

mod stash;
mod upstream;

use std::time::{Duration, Instant};

use cairn_model::{HeadState, Ref, RefKind, RefName, RefTarget, RefsSnapshot};

use crate::object_id::model_id;
use crate::{Cancel, Error, Repository};

/// git's limit on resolving a ref (`SYMREF_MAXDEPTH` in `refs.c`): at most this many refs
/// are read, the ref itself included, so a chain of more than four symbolic hops does not
/// resolve and the ref is skipped — git hides it silently, as it hides a dangling one; here
/// it is counted as unreadable.
const SYMREF_MAX_DEPTH: usize = 5;

/// A refs snapshot and what reading it cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefsRead {
    pub snapshot: RefsSnapshot,
    pub cost: RefsCost,
}

/// What a refs query paid (R1.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RefsCost {
    /// Ref entries read from the store: every listed, hidden or skipped ref, each level of
    /// a symbolic chain followed, and each upstream looked up.
    pub refs_read: usize,
    /// Objects looked up, to tell what a ref names and to peel its tags, and stash
    /// commits read for the commit they were made on.
    pub objects_read: usize,
    /// Lines of the `refs/stash` reflog read.
    pub reflog_lines_read: usize,
    pub elapsed: Duration,
}

impl Repository {
    /// The repository's refs as they stand now. `cancel` is polled once per ref and per
    /// stash entry; a cancelled query is [`Error::RefsCancelled`]. Fails only when the
    /// store cannot be listed or `HEAD` cannot be read; a ref that cannot be read is
    /// skipped and counted in [`RefsSnapshot::unreadable`].
    pub fn refs(&self, cancel: &impl Cancel) -> Result<RefsRead, Error> {
        let started = Instant::now();
        let mut reading = Reading {
            repo: self.inner(),
            cost: RefsCost::default(),
            unreadable: 0,
        };
        let head = reading.head()?;
        let mut refs = reading.listed(cancel)?;
        for reference in &mut refs {
            if reference.kind == RefKind::LocalBranch {
                if cancel.is_cancelled() {
                    return Err(Error::RefsCancelled);
                }
                reference.upstream = upstream::of(&mut reading, &reference.name);
            }
        }
        let stashes = stash::list(&mut reading, cancel)?;
        let mut cost = reading.cost;
        cost.elapsed = started.elapsed();
        Ok(RefsRead {
            snapshot: RefsSnapshot {
                refs,
                head,
                stashes,
                unreadable: reading.unreadable,
            },
            cost,
        })
    }
}

/// One query's state: the repository, what it has cost so far and what it skipped.
pub(crate) struct Reading<'repo> {
    repo: &'repo gix::Repository,
    cost: RefsCost,
    unreadable: usize,
}

/// What one ref resolved to.
enum Resolution {
    Listed {
        target: RefTarget,
        symbolic: Option<RefName>,
    },
    /// A symbolic ref whose chain ends at a ref that does not exist: hidden, as git hides it.
    Dangling,
    /// Skipped and counted.
    Unreadable,
}

impl Reading<'_> {
    /// `HEAD` as git answers it: detached at the object it names directly, or on the branch
    /// at the END of its symbolic chain — what `git symbolic-ref HEAD` prints, following
    /// every level — which is unborn when that chain ends at a ref that does not exist (or
    /// does not resolve within git's depth), as `git rev-parse HEAD` then fails. gix's
    /// `head()` follows one level only, and would call `HEAD` → `x` → (nothing) a branch.
    fn head(&mut self) -> Result<HeadState, Error> {
        let head_error = |source: Box<dyn std::error::Error + Send + Sync>| Error::Refs { source };
        self.cost.refs_read += 1;
        let head = self
            .repo
            .find_reference("HEAD")
            .map_err(|source| head_error(Box::new(source)))?
            .detach();
        let mut name = match head.target {
            gix::refs::Target::Object(id) => return Ok(HeadState::Detached(model_id(&id)?)),
            gix::refs::Target::Symbolic(name) => name,
        };
        // `HEAD` itself is the first of git's reads.
        for _ in 1..SYMREF_MAX_DEPTH {
            self.cost.refs_read += 1;
            match self.repo.try_find_reference(name.as_ref()) {
                Ok(Some(found)) => match found.detach().target {
                    gix::refs::Target::Object(_) => return Ok(HeadState::Branch(ref_name(&name))),
                    gix::refs::Target::Symbolic(next) => name = next,
                },
                Ok(None) | Err(_) => return Ok(HeadState::Unborn(ref_name(&name))),
            }
        }
        Ok(HeadState::Unborn(ref_name(&name)))
    }

    /// The local branches, then the remote-tracking refs, then the tags, each read in
    /// `git for-each-ref`'s order — which, `refs/heads/` sorting before `refs/remotes/`
    /// before `refs/tags/`, is that order overall.
    fn listed(&mut self, cancel: &impl Cancel) -> Result<Vec<Ref>, Error> {
        let store_error = |source: Box<dyn std::error::Error + Send + Sync>| Error::Refs { source };
        let platform = self
            .repo
            .references()
            .map_err(|source| store_error(Box::new(source)))?;
        let mut refs = Vec::new();
        for kind in [RefKind::LocalBranch, RefKind::RemoteTracking, RefKind::Tag] {
            let iter = match kind {
                RefKind::LocalBranch => platform.local_branches(),
                RefKind::RemoteTracking => platform.remote_branches(),
                RefKind::Tag => platform.tags(),
            }
            .map_err(|source| store_error(Box::new(source)))?;
            for item in iter {
                if cancel.is_cancelled() {
                    return Err(Error::RefsCancelled);
                }
                self.cost.refs_read += 1;
                let Ok(reference) = item else {
                    self.unreadable += 1;
                    continue;
                };
                let reference = reference.detach();
                match self.resolve(&reference) {
                    Resolution::Listed { target, symbolic } => refs.push(Ref {
                        name: ref_name(&reference.name),
                        kind,
                        target,
                        symbolic,
                        upstream: None,
                    }),
                    Resolution::Dangling => {}
                    Resolution::Unreadable => self.unreadable += 1,
                }
            }
        }
        Ok(refs)
    }

    /// What `reference` names: a symbolic chain followed to the object at its end, within
    /// git's depth, naming the ref at that end — `%(symref)`, which is the end of the chain,
    /// not its first hop — and that object told apart and peeled.
    fn resolve(&mut self, reference: &gix::refs::Reference) -> Resolution {
        let mut symbolic = None;
        let mut target = reference.target.clone();
        let mut peeled = reference.peeled;
        // `reference` itself is the first of git's reads.
        let mut read = 1;
        loop {
            match target {
                gix::refs::Target::Object(id) => {
                    return match self.object(id, peeled) {
                        Some(target) => Resolution::Listed { target, symbolic },
                        None => Resolution::Unreadable,
                    };
                }
                gix::refs::Target::Symbolic(name) => {
                    read += 1;
                    if read > SYMREF_MAX_DEPTH {
                        return Resolution::Unreadable;
                    }
                    self.cost.refs_read += 1;
                    match self.repo.try_find_reference(name.as_ref()) {
                        Ok(Some(next)) => {
                            let next = next.detach();
                            symbolic = Some(ref_name(&next.name));
                            target = next.target;
                            peeled = next.peeled;
                        }
                        Ok(None) => return Resolution::Dangling,
                        Err(_) => return Resolution::Unreadable,
                    }
                }
            }
        }
    }

    /// The object a ref names, told apart: a commit, a tree or blob, or a tag peeled to the
    /// end of its chain — through `packed`, a packed ref's `^` line, where it has one, as
    /// git reads it. `None` when an object is missing or unreadable.
    fn object(&mut self, id: gix::ObjectId, packed: Option<gix::ObjectId>) -> Option<RefTarget> {
        let named = model_id(&id).ok()?;
        match self.kind(id)? {
            gix::objs::Kind::Commit => Some(RefTarget::Commit(named)),
            gix::objs::Kind::Tree | gix::objs::Kind::Blob => Some(RefTarget::Other(named)),
            gix::objs::Kind::Tag => {
                let (end, kind) = match packed {
                    Some(end) => (end, self.kind(end)?),
                    None => self.peel_tags(id)?,
                };
                let commit = match kind {
                    gix::objs::Kind::Commit => Some(model_id(&end).ok()?),
                    gix::objs::Kind::Tree | gix::objs::Kind::Blob | gix::objs::Kind::Tag => None,
                };
                Some(RefTarget::Tag {
                    object: named,
                    commit,
                })
            }
        }
    }

    fn kind(&mut self, id: gix::ObjectId) -> Option<gix::objs::Kind> {
        self.cost.objects_read += 1;
        match self.repo.try_find_header(id) {
            Ok(Some(header)) => Some(header.kind()),
            Ok(None) | Err(_) => None,
        }
    }

    /// Follows tag objects from `tag` to the first object that is not one, as git's
    /// `peel_object` does: the id and kind it ends at. A tag cannot name itself, even
    /// through others (each id is the hash of content naming the next), so the chain ends.
    fn peel_tags(&mut self, tag: gix::ObjectId) -> Option<(gix::ObjectId, gix::objs::Kind)> {
        let mut id = tag;
        loop {
            self.cost.objects_read += 1;
            let object = self.repo.try_find_object(id).ok()??;
            match object.kind {
                gix::objs::Kind::Tag => id = object.try_to_tag_ref_iter()?.target_id().ok()?,
                kind
                @ (gix::objs::Kind::Commit | gix::objs::Kind::Tree | gix::objs::Kind::Blob) => {
                    return Some((id, kind));
                }
            }
        }
    }
}

/// A ref's full name as the model holds one. A name that is not UTF-8 is held lossily.
fn ref_name(name: &gix::refs::FullName) -> RefName {
    RefName::new(name.as_bstr().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CancelSignal;

    /// Against whatever refs this checkout has — a CI checkout is detached with no local
    /// branch, only remote-tracking refs — so nothing here names a branch. Parity with git
    /// is `tests/refs.rs`, over fixtures built with real git.
    #[test]
    fn this_checkout_has_refs_and_two_reads_agree() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let read = repo.refs(&CancelSignal::new()).unwrap();
        assert!(
            !read.snapshot.refs.is_empty(),
            "a checkout with no refs at all"
        );
        assert!(
            read.snapshot
                .refs
                .windows(2)
                .all(|pair| pair[0].name.as_str().as_bytes() < pair[1].name.as_str().as_bytes()),
            "the refs are not in for-each-ref's order"
        );
        assert!(read.cost.refs_read >= read.snapshot.refs.len());
        assert_eq!(
            repo.refs(&CancelSignal::new()).unwrap().snapshot,
            read.snapshot,
            "two reads differ"
        );
    }

    /// Caught by: a refs query that polls nothing, which a superseding refresh cannot stop.
    #[test]
    fn a_cancelled_refs_query_answers_nothing() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let cancel = CancelSignal::new();
        cancel.cancel();
        assert!(matches!(repo.refs(&cancel), Err(Error::RefsCancelled)));
    }
}
