//! Closing the window closes its repository (PRD R6.3).
//!
//! The window's close hook runs on the UI thread, so it asks and never
//! waits: the first request sends [`Request::Close`] and keeps the window
//! open, the worker ends every `git` in the repository and waits for their
//! reaps on its own thread, and the update stream ending is what then closes
//! the window — so no `git` and no askpass socket outlive it. A window whose
//! repository never opened, or whose worker has already gone, closes at once.
//! A second request after [`CLOSE_PATIENCE`] closes it anyway, so a worker
//! that has stopped answering cannot keep the window open for good.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use freya::prelude::CloseDecision;

use crate::worker::{CLOSE_PATIENCE, RepositoryHandle, Request};

/// Shared by the window's close hook and the task that drives the worker's
/// updates; cloning shares it.
#[derive(Clone, Default)]
pub struct Closing(Rc<RefCell<Stage>>);

#[derive(Default)]
enum Stage {
    /// No repository was opened, or not yet.
    #[default]
    Unopened,
    Open(RepositoryHandle),
    /// The worker was asked to close at `since`, and has not finished.
    Closing {
        since: Instant,
    },
    /// The worker's stream has ended.
    Gone,
}

impl Closing {
    /// The repository the window shows is open, and `handle` reaches it.
    pub fn opened(&self, handle: RepositoryHandle) {
        let mut stage = self.0.borrow_mut();
        if matches!(*stage, Stage::Unopened) {
            *stage = Stage::Open(handle);
        }
    }

    /// The window's close hook. Never waits: it sends at most one request.
    pub fn requested(&self) -> CloseDecision {
        let mut stage = self.0.borrow_mut();
        match &*stage {
            Stage::Unopened | Stage::Gone => CloseDecision::Close,
            Stage::Open(handle) => {
                handle.submit(Request::Close);
                *stage = Stage::Closing {
                    since: Instant::now(),
                };
                CloseDecision::KeepOpen
            }
            Stage::Closing { since } if since.elapsed() >= CLOSE_PATIENCE => CloseDecision::Close,
            Stage::Closing { .. } => CloseDecision::KeepOpen,
        }
    }

    /// The worker's update stream has ended; `true` when the window asked to
    /// close and should close now.
    pub fn worker_gone(&self) -> bool {
        let mut stage = self.0.borrow_mut();
        let asked = matches!(*stage, Stage::Closing { .. });
        *stage = Stage::Gone;
        asked
    }

    /// Back-dates the close request, for a test of the patience.
    #[cfg(test)]
    fn asked_long_ago(&self) {
        if let Stage::Closing { since } = &mut *self.0.borrow_mut()
            && let Some(earlier) = since.checked_sub(CLOSE_PATIENCE)
        {
            *since = earlier;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a window with no repository that will not close, which would wait
    /// for a stream that is not coming.
    #[test]
    fn a_window_with_no_repository_closes_at_once() {
        let closing = Closing::default();
        assert_eq!(closing.requested(), CloseDecision::Close);
        assert!(!closing.worker_gone(), "nothing was asked to close");
    }

    /// Caught by: a worker that died before the window closed leaving it unclosable,
    /// and a stream that ends by itself closing a window nobody asked to close.
    #[test]
    fn a_window_whose_worker_has_gone_closes_at_once_and_only_when_asked() {
        let (handle, _updates) = crate::worker::idle_handle();
        let closing = Closing::default();
        closing.opened(handle);
        assert!(
            !closing.worker_gone(),
            "a stream that ended without a close request closed the window"
        );
        assert_eq!(closing.requested(), CloseDecision::Close);
    }

    /// PRD R6.3 on the UI thread: the first request asks the worker to close and keeps
    /// the window open; the stream's end is what closes it; a second request before the
    /// patience runs out changes nothing, and one after it closes. Caught by: closing at
    /// once (the git and the socket outlive the window), not asking the worker, asking it
    /// twice, or a window that can never be closed if the worker stops answering.
    #[test]
    fn the_first_request_asks_the_worker_and_its_stream_ending_closes_the_window() {
        let (handle, asked) = crate::worker::idle_handle();
        let closing = Closing::default();
        closing.opened(handle);

        assert_eq!(closing.requested(), CloseDecision::KeepOpen);
        assert_eq!(
            asked(),
            vec![Request::Close],
            "the worker was not asked to close"
        );
        assert_eq!(closing.requested(), CloseDecision::KeepOpen);
        assert!(asked().is_empty(), "the close was asked for twice");

        closing.asked_long_ago();
        assert_eq!(
            closing.requested(),
            CloseDecision::Close,
            "a worker that stopped answering kept the window open"
        );

        let (handle, _asked) = crate::worker::idle_handle();
        let closing = Closing::default();
        closing.opened(handle);
        assert_eq!(closing.requested(), CloseDecision::KeepOpen);
        assert!(
            closing.worker_gone(),
            "the stream ending did not close the window"
        );
        assert_eq!(closing.requested(), CloseDecision::Close);
    }
}
