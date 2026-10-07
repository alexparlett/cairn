//! What the last refresh answered, as the window keeps it (refs-and-status R10): the refs
//! snapshot, each branch's ahead/behind and the working tree's status, each kept until the
//! next answer replaces it, and each read's failure beside it. The history's chips and the
//! title bar read the refs, the title bar the current branch's counts and whether status
//! listed a change; the sidebar and Local Changes read the rest, from phases 08-09.
//!
//! An answer replaced is handed back, never dropped here: the window sends it to a worker to
//! free (`Request::Retire`), since a snapshot of tens of thousands of refs, or a status of
//! tens of thousands of paths, is too much to free on the UI thread (R11.3).

use std::sync::Arc;

use cairn_model::{AheadBehind, LocalChanges, RefName, RefsSnapshot, WorkingTreeStatus};
use cairn_ui::BranchCounts;

use crate::worker::{Refreshed, Retired};

/// One read's answer: the last that arrived, and the failure of a read since, if any.
#[derive(Debug)]
struct Kept<T> {
    last: Option<T>,
    failure: Option<String>,
}

impl<T> Default for Kept<T> {
    fn default() -> Self {
        Self {
            last: None,
            failure: None,
        }
    }
}

impl<T> Kept<T> {
    /// `answer` is kept; the one it replaces is handed back.
    fn arrived(&mut self, answer: T) -> Option<T> {
        self.failure = None;
        self.last.replace(answer)
    }
}

#[derive(Debug, Default)]
pub struct RefreshState {
    refs: Kept<Arc<RefsSnapshot>>,
    ahead_behind: Kept<BranchCounts>,
    status: Kept<Arc<LocalChanges>>,
}

impl RefreshState {
    /// The refs snapshot the window has, if one has arrived.
    pub fn refs(&self) -> Option<&Arc<RefsSnapshot>> {
        self.refs.last.as_ref()
    }

    /// Each local branch's distance from its upstream, if counted: shared with the sidebar,
    /// which draws each branch's.
    pub fn ahead_behind(&self) -> Option<&BranchCounts> {
        self.ahead_behind.last.as_ref()
    }

    /// `branch`'s distance from its upstream, if it was counted: a binary search, since the
    /// counts come in the snapshot's order, by name.
    pub fn ahead_behind_of(&self, branch: &RefName) -> Option<AheadBehind> {
        let counts = self.ahead_behind.last.as_deref()?.as_slice();
        counts
            .binary_search_by(|(name, _)| name.as_str().as_bytes().cmp(branch.as_str().as_bytes()))
            .ok()
            .and_then(|at| counts.get(at))
            .map(|(_, counts)| *counts)
    }

    /// The working tree's status, if read.
    pub fn status(&self) -> Option<&WorkingTreeStatus> {
        self.status.last.as_deref().map(LocalChanges::status)
    }

    /// The working tree's status laid out as Local Changes' two lists, if read: shared with a
    /// filter of them asked of a worker.
    pub fn local_changes(&self) -> Option<&Arc<LocalChanges>> {
        self.status.last.as_ref()
    }

    /// Why the last read of `what` failed, while no answer has arrived since: the sidebar says
    /// the refs'.
    pub fn failure(&self, what: Refreshed) -> Option<&str> {
        match what {
            Refreshed::Refs => self.refs.failure.as_deref(),
            Refreshed::AheadBehind => self.ahead_behind.failure.as_deref(),
            Refreshed::Status => self.status.failure.as_deref(),
        }
    }

    /// Keeps `snapshot`, handing back the one it replaces to be freed on a worker.
    pub fn refs_arrived(&mut self, snapshot: Arc<RefsSnapshot>) -> Option<Retired> {
        self.refs.arrived(snapshot).map(Retired::refs)
    }

    /// Keeps `counts`, handing back those they replace to be freed on a worker.
    pub fn ahead_behind_arrived(&mut self, counts: Vec<(RefName, AheadBehind)>) -> Option<Retired> {
        self.ahead_behind
            .arrived(Arc::new(counts))
            .and_then(Retired::ahead_behind)
    }

    /// Keeps `status`, handing back the one it replaces to be freed on a worker.
    pub fn status_arrived(&mut self, status: Arc<LocalChanges>) -> Option<Retired> {
        self.status.arrived(status).map(Retired::status)
    }

    /// The read of `what` failed: said beside the answer kept, which stays.
    pub fn failed(&mut self, what: Refreshed, message: String) {
        let failure = match what {
            Refreshed::Refs => &mut self.refs.failure,
            Refreshed::AheadBehind => &mut self.ahead_behind.failure,
            Refreshed::Status => &mut self.status.failure,
        };
        *failure = Some(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A branch's counts are found by its name among every branch counted, and a branch not
    /// counted (no upstream, a gone one) has none. Caught by: a neighbour's counts answered,
    /// or the search reading an order the counts are not in.
    #[test]
    fn a_branchs_counts_are_found_by_its_name() {
        let mut state = RefreshState::default();
        assert_eq!(state.ahead_behind_of(&RefName::new("refs/heads/a")), None);
        let counted = |ahead, behind| AheadBehind { ahead, behind };
        let _ = state.ahead_behind_arrived(vec![
            (RefName::new("refs/heads/a"), counted(1, 0)),
            (RefName::new("refs/heads/a/b"), counted(2, 3)),
            (RefName::new("refs/heads/main"), counted(0, 7)),
            (RefName::new("refs/heads/z"), counted(4, 4)),
        ]);
        for (name, wanted) in [
            ("refs/heads/a", Some(counted(1, 0))),
            ("refs/heads/a/b", Some(counted(2, 3))),
            ("refs/heads/main", Some(counted(0, 7))),
            ("refs/heads/z", Some(counted(4, 4))),
            ("refs/heads/gone", None),
            ("refs/heads/mai", None),
        ] {
            assert_eq!(state.ahead_behind_of(&RefName::new(name)), wanted, "{name}");
        }
    }
}
