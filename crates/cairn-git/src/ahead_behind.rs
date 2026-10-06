//! How far each local branch and its upstream have gone apart: what
//! `git rev-list --left-right --count <branch>...<upstream>` answers (PRD R2).
//!
//! gix has no API for it, so it is two walks with the other side hidden — the commits the
//! branch reaches that its upstream does not, and the reverse — over the commits git's walk
//! sees (a shallow clone's boundary grafted, `history::walk`). It is a query of its own,
//! asked after the refs snapshot it belongs to and over that snapshot's ids, so a long
//! divergence never holds up the refs; and it is cancellable between branches and within
//! a walk, the frontier gix paints before a hiding walk's first commit included
//! (`history::walk::hiding`).

use std::cell::Cell;
use std::time::{Duration, Instant};

use cairn_model::{AheadBehind, Oid, RefKind, RefName, RefsSnapshot, Upstream};

use crate::history::walk;
use crate::object_id::object_id;
use crate::{Cancel, Error, Repository};

/// Each counted branch's answer, and what counting cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AheadBehindRead {
    /// One entry per local branch whose upstream exists and identifies a commit, in the
    /// snapshot's order. A branch with no upstream, or a gone one, has none.
    pub counts: Vec<(RefName, AheadBehind)>,
    /// Commit objects read, by both walks of every branch.
    pub commits_read: usize,
    pub elapsed: Duration,
}

impl Repository {
    /// Counts every local branch of `snapshot` against its upstream. `cancel` is polled
    /// before each branch, after each commit counted and before each object read; a
    /// cancelled query is [`Error::AheadBehindCancelled`], and what it had counted is not
    /// answered.
    pub fn ahead_behind(
        &self,
        snapshot: &RefsSnapshot,
        cancel: &impl Cancel,
    ) -> Result<AheadBehindRead, Error> {
        let started = Instant::now();
        let reads = Cell::new(0);
        let mut counts = Vec::new();
        for branch in snapshot.of_kind(RefKind::LocalBranch) {
            let (
                Some(tip),
                Some(Upstream::Exists {
                    commit: Some(upstream),
                    ..
                }),
            ) = (branch.commit_id(), &branch.upstream)
            else {
                continue;
            };
            let cancelled = || Error::AheadBehindCancelled {
                branches: counts.len(),
                commits_read: reads.get(),
            };
            if cancel.is_cancelled() {
                return Err(cancelled());
            }
            let answer = if tip == *upstream {
                AheadBehind::default()
            } else {
                let count = |from: &Oid, hiding: &Oid| {
                    reachable_only_from(self, from, hiding, cancel, &reads)
                        .map_err(|error| error.unwrap_or_else(cancelled))
                };
                AheadBehind {
                    ahead: count(&tip, upstream)?,
                    behind: count(upstream, &tip)?,
                }
            };
            counts.push((branch.name.clone(), answer));
        }
        Ok(AheadBehindRead {
            counts,
            commits_read: reads.get(),
            elapsed: started.elapsed(),
        })
    }
}

/// How many commits `from` reaches that `hiding` does not. `Err(None)` when cancelled; an
/// error converts into `Err(Some(..))` by `Option`'s own `From`.
fn reachable_only_from(
    repo: &Repository,
    from: &Oid,
    hiding: &Oid,
    cancel: &impl Cancel,
    reads: &Cell<usize>,
) -> Result<usize, Option<Error>> {
    let walk = walk::hiding(
        repo.inner(),
        object_id(from)?,
        object_id(hiding)?,
        cancel,
        reads,
    )?;
    let mut counted = 0;
    for commit in walk {
        if cancel.is_cancelled() {
            return Err(None);
        }
        commit.map_err(|source| {
            // A read refused because the walk was cancelled surfaces as the walk's error.
            if cancel.is_cancelled() {
                None
            } else {
                Some(Error::Walk {
                    source: Box::new(source),
                })
            }
        })?;
        counted += 1;
    }
    Ok(counted)
}
