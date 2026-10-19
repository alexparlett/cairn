//! The refs the history walks from, shared by the two threads that touch them: the repository
//! thread, which reads them for a refresh, and the history thread, which opens the walk from
//! them.
//!
//! A refresh reads the refs in the refs lane on the repository thread and compares what they
//! would draw with the snapshot the open walk began from (`RefsSnapshot::walks_as`): the
//! answer, [`Update::Refs`], says whether the history must be reopened, and the window — which
//! alone numbers the history lane — asks the history thread for the reopen, whose number
//! supersedes a walk still opening there. A refresh never touches the walk itself, and never
//! waits on it: an opening pass that reads the whole history runs on the history thread.
//!
//! The history thread takes the snapshot the last refresh read as it opens a walk, and names it
//! the one the walk began from in the same breath, so a refresh while the walk is still opening
//! compares with what that walk will draw and leaves it alone when nothing moved.
//!
//! The lock is held for a clone or an assignment of a shared pointer — and by a refresh across
//! its comparison of two snapshots, so an open between the comparison and the keeping cannot
//! answer it wrongly — never across a read of the refs, and only by those two worker threads.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};

use cairn_git::{Error, Repository};
use cairn_model::RefsSnapshot;

use crate::worker::epoch::Epoch;
use crate::worker::history::lane::Answering;
use crate::worker::refresh::lane::RefreshJob;
use crate::worker::request::{Refreshed, Update};

/// The two snapshots, shared by cloning.
#[derive(Debug, Clone, Default)]
pub(crate) struct WalkedRefs(Arc<Mutex<Snapshots>>);

#[derive(Debug, Default)]
struct Snapshots {
    /// The snapshot the last refresh read: what the next open walks from.
    latest: Option<Arc<RefsSnapshot>>,
    /// The snapshot the walk open — or opening — began from: what a refresh compares with.
    walked_from: Option<Arc<RefsSnapshot>>,
}

impl WalkedRefs {
    fn held(&self) -> std::sync::MutexGuard<'_, Snapshots> {
        // A panic on the other thread while it held this lock left two pointers, each whole.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What the next open walks from, now named the snapshot the walk began from; `None` when
    /// no refresh has read one, or the last open failed.
    pub(crate) fn opening(&self) -> Option<Arc<RefsSnapshot>> {
        let mut held = self.held();
        let latest = held.latest.clone();
        held.walked_from.clone_from(&latest);
        latest
    }

    /// Refs an open read itself, or read again: the latest, and what the walk begins from.
    pub(crate) fn read_for_open(&self, snapshot: &Arc<RefsSnapshot>) {
        let mut held = self.held();
        held.latest = Some(Arc::clone(snapshot));
        held.walked_from = Some(Arc::clone(snapshot));
    }

    /// The walk failed before any page of it: both forgotten, so the next open reads the refs
    /// afresh and the next refresh reopens.
    pub(crate) fn forget(&self) {
        let mut held = self.held();
        held.latest = None;
        held.walked_from = None;
    }

    /// A refresh's read, kept as the latest — the one held when the two are equal, so the
    /// read is freed here — and whether the history must be reopened: when what it draws
    /// differs from the snapshot the walk began from, or no walk has begun. Compared and kept
    /// under one hold of the lock, so an open taking the latest meanwhile is compared with as
    /// the walk it begins: the history thread waits that comparison out, never the UI thread.
    fn refreshed(&self, read: RefsSnapshot) -> (Arc<RefsSnapshot>, bool) {
        let mut held = self.held();
        let snapshot = match &held.latest {
            Some(kept) if **kept == read => Arc::clone(kept),
            _ => Arc::new(read),
        };
        let reopen = !held
            .walked_from
            .as_ref()
            .is_some_and(|walked| walked.walks_as(&snapshot));
        held.latest = Some(Arc::clone(&snapshot));
        drop(held);
        (snapshot, reopen)
    }

    /// Holding `latest` as a refresh had read it, for a test of an open from it.
    #[cfg(test)]
    pub(crate) fn holding(latest: RefsSnapshot) -> Self {
        let refs = Self::default();
        refs.held().latest = Some(Arc::new(latest));
        refs
    }

    /// The snapshot the walk began from, for a test.
    #[cfg(test)]
    pub(crate) fn walked_from(&self) -> Option<Arc<RefsSnapshot>> {
        self.held().walked_from.clone()
    }
}

/// A refresh's first read, on the repository thread: the refs as they stand, under the refs
/// lane's `epoch`, which also cancels it. Answered as [`Update::Refs`] — `reopen` when what the
/// history draws differs from the snapshot its walk began from, or no walk has begun — and
/// handed on to the refresh thread for ahead/behind, under `ahead_behind`.
pub(crate) fn refresh(
    repo: &Repository,
    walked: &WalkedRefs,
    (epoch, ahead_behind): (Epoch, Epoch),
    refresh: &Sender<RefreshJob>,
    answering: &Answering<'_>,
) {
    let read = match repo.refs(&answering.epochs.watch(epoch)) {
        Ok(read) => read,
        // Superseded by the next refresh: not a failure.
        Err(Error::RefsCancelled) => return,
        Err(error) => {
            answering.outbox.send(
                Some(epoch),
                Update::RefreshFailed {
                    what: Refreshed::Refs,
                    message: error.to_string(),
                },
            );
            return;
        }
    };
    let (snapshot, reopen) = walked.refreshed(read.snapshot);
    // A failed send means the refresh thread has gone with the repository.
    let _ = refresh.send(RefreshJob::AheadBehind {
        epoch: ahead_behind,
        snapshot: Arc::clone(&snapshot),
    });
    answering
        .outbox
        .send(Some(epoch), Update::Refs { snapshot, reopen });
}

#[cfg(test)]
mod tests {
    use cairn_model::{HeadState, Oid, Ref, RefKind, RefName, RefTarget};

    use super::*;

    fn naming(commit: u8) -> RefsSnapshot {
        RefsSnapshot {
            refs: vec![Ref {
                name: RefName::new("refs/heads/main"),
                kind: RefKind::LocalBranch,
                target: RefTarget::Commit(
                    Oid::from_bytes(&[commit; 20]).unwrap_or_else(|error| panic!("{error}")),
                ),
                symbolic: None,
                upstream: None,
            }],
            head: HeadState::Branch(RefName::new("refs/heads/main")),
            stashes: Vec::new(),
            unreadable: 0,
            substitutions: cairn_model::Substitutions::default(),
        }
    }

    /// A refresh while a walk is still opening compares with the snapshot that walk takes as it
    /// opens, never with none: the same refs reopen nothing, moved refs reopen, and the walk
    /// takes the latest. With no walk begun, any refs reopen; a failed open forgets both. Caught
    /// by: the snapshot the walk began from set only once its first page is out (a refresh
    /// during the opening pass reopens, superseding the pass for nothing), or a read equal to
    /// the one held kept as a second copy.
    #[test]
    fn a_refresh_during_an_open_compares_with_the_refs_the_open_took() {
        let refs = WalkedRefs::default();
        let (first, reopen) = refs.refreshed(naming(1));
        assert!(reopen, "no walk has begun, yet nothing is reopened");

        let taken = refs.opening();
        assert_eq!(taken.as_deref(), Some(&*first));
        let (same, reopen) = refs.refreshed(naming(1));
        assert!(!reopen, "the refs the walk is opening from reopened it");
        assert!(Arc::ptr_eq(&same, &first), "an equal read kept twice");

        let (moved, reopen) = refs.refreshed(naming(2));
        assert!(reopen, "moved refs did not reopen");
        assert_eq!(refs.opening().as_deref(), Some(&*moved));

        refs.forget();
        assert_eq!(refs.opening(), None);
        assert!(
            refs.refreshed(naming(2)).1,
            "a failed open's refs still compared"
        );
    }
}
