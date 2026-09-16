//! Which request is the one that still matters.
//!
//! R3.2 asks that a stale response be dropped without rendering. That alone is
//! the weak half — it looks identical from the window while the abandoned walk
//! runs to completion and burns a core — so the epoch is not only a tag on the
//! reply, it *is* the cancel signal the engine polls (R2.4), which is also what
//! makes R2.5's live session safe to keep across a supersession.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use cairn_git::Cancel;

/// Which request a value belongs to. Monotonic, and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Epoch(u64);

/// The current epoch, shared by the UI side and the workers.
///
/// Cloning shares it: the UI side bumps it to supersede a request, and the
/// worker reads the same word to discover its work is no longer wanted.
#[derive(Debug, Clone, Default)]
pub struct Epochs {
    current: Arc<AtomicU64>,
    stopping: Arc<AtomicBool>,
}

impl Epochs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Supersede whatever is in flight, and name the request that replaces it.
    pub fn bump(&self) -> Epoch {
        Epoch(self.current.fetch_add(1, Ordering::AcqRel) + 1)
    }

    /// The request that is still wanted.
    pub fn current(&self) -> Epoch {
        Epoch(self.current.load(Ordering::Acquire))
    }

    /// Whether `epoch` is still the one being waited for.
    pub fn is_current(&self, epoch: Epoch) -> bool {
        !self.is_stopping() && self.current() == epoch
    }

    /// Stop everything, for good. Used when the window is closing: without it a
    /// worker part-way through a ten-year monorepo would finish the page first.
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
    }

    pub fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::Acquire)
    }

    /// The cancel signal for one request, in the engine's own vocabulary.
    pub fn watch(&self, mine: Epoch) -> Superseded {
        Superseded {
            mine,
            current: Arc::clone(&self.current),
            stopping: Arc::clone(&self.stopping),
        }
    }
}

/// "Is the request I am serving still the current one?", shaped as the
/// [`Cancel`] the engine polls once per commit.
#[derive(Debug)]
pub struct Superseded {
    mine: Epoch,
    current: Arc<AtomicU64>,
    stopping: Arc<AtomicBool>,
}

impl Cancel for Superseded {
    fn is_cancelled(&self) -> bool {
        self.stopping.load(Ordering::Acquire) || self.current.load(Ordering::Acquire) != self.mine.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epochs_rise_and_are_never_reused() {
        let epochs = Epochs::new();
        let before = epochs.current();
        let first = epochs.bump();
        let second = epochs.bump();
        assert!(
            first > before,
            "the first request did not advance the epoch"
        );
        assert!(second > first, "{second:?} did not follow {first:?}");
        assert_eq!(epochs.current(), second);
        assert!(
            !epochs.is_current(first),
            "the superseded request was still reported current"
        );
    }

    #[test]
    fn a_request_is_cancelled_by_the_one_that_supersedes_it() {
        let epochs = Epochs::new();
        let mine = epochs.bump();
        let cancel = epochs.watch(mine);
        assert!(
            !cancel.is_cancelled(),
            "the current request reported itself abandoned"
        );

        epochs.bump();
        assert!(
            cancel.is_cancelled(),
            "superseding a request left its walk running"
        );
    }

    #[test]
    fn stopping_cancels_the_current_request_too() {
        let epochs = Epochs::new();
        let mine = epochs.bump();
        let cancel = epochs.watch(mine);
        epochs.stop();
        assert!(cancel.is_cancelled(), "shutdown left a walk running");
        assert!(
            !epochs.is_current(mine),
            "a stopping pool has no current work"
        );
    }

    #[test]
    fn the_epoch_is_shared_across_threads() {
        let epochs = Epochs::new();
        let mine = epochs.bump();
        let cancel = epochs.watch(mine);
        let other = epochs.clone();
        std::thread::spawn(move || other.bump()).join().unwrap();
        assert!(
            cancel.is_cancelled(),
            "a supersession from another thread was not visible"
        );
    }
}
