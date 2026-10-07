//! What the last refresh answered, as the window keeps it (refs-and-status R10): the refs
//! snapshot, each branch's ahead/behind and the working tree's status, each kept until the
//! next answer replaces it, and each read's failure beside it. No view draws them yet —
//! the labels and toolbar, the sidebar and Local Changes do, from phases 07-09.
//!
//! An answer replaced is handed back, never dropped here: the window sends it to a worker to
//! free (`Request::Retire`), since a snapshot of tens of thousands of refs, or a status of
//! tens of thousands of paths, is too much to free on the UI thread (R11.3).

use std::sync::Arc;

use cairn_model::{AheadBehind, RefName, RefsSnapshot, WorkingTreeStatus};

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
    ahead_behind: Kept<Vec<(RefName, AheadBehind)>>,
    status: Kept<WorkingTreeStatus>,
}

impl RefreshState {
    /// The refs snapshot the window has, if one has arrived.
    pub fn refs(&self) -> Option<&Arc<RefsSnapshot>> {
        self.refs.last.as_ref()
    }

    /// Each local branch's distance from its upstream, if counted.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the toolbar and sidebar read it from phase 07 on")
    )]
    pub fn ahead_behind(&self) -> Option<&[(RefName, AheadBehind)]> {
        self.ahead_behind.last.as_deref()
    }

    /// The working tree's status, if read.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Local Changes reads it from phase 09 on")
    )]
    pub fn status(&self) -> Option<&WorkingTreeStatus> {
        self.status.last.as_ref()
    }

    /// Why the last read of `what` failed, while no answer has arrived since.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the views that draw each answer say so from phase 07 on"
        )
    )]
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
            .arrived(counts)
            .and_then(Retired::ahead_behind)
    }

    /// Keeps `status`, handing back the one it replaces to be freed on a worker.
    pub fn status_arrived(&mut self, status: WorkingTreeStatus) -> Option<Retired> {
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
