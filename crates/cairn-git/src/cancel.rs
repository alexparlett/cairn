//! Telling a query in progress to stop.
//!
//! A repository is somebody's ten-year monorepo, so a query that is no longer
//! wanted has to stop walking rather than run to completion and have its answer
//! thrown away. The engine knows nothing about threads: it polls a signal the
//! caller owns, and the caller decides what sets it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Something a running query asks, often, whether it should stop.
///
/// This is a trait rather than a concrete flag so that a test can count the
/// polls and stop the walk at a chosen commit: a cancellation that cannot be
/// observed stopping a walk is a cancellation nobody has tested.
pub trait Cancel {
    /// Called once per commit visited. Returning `true` abandons the query.
    fn is_cancelled(&self) -> bool;
}

/// A flag one thread sets and another polls.
///
/// Cloning shares the flag, which is the point: the worker keeps one handle and
/// hands another to whoever may need to abandon the work.
#[derive(Debug, Clone, Default)]
pub struct CancelSignal(Arc<AtomicBool>);

impl CancelSignal {
    /// A signal that is not yet set. A query given one that is never set runs
    /// to its limit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask every query holding this signal to stop. Setting it twice is
    /// harmless; it cannot be unset, because a query that was abandoned and
    /// then un-abandoned is a race, not a feature.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}

impl Cancel for CancelSignal {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl<T: Cancel + ?Sized> Cancel for &T {
    fn is_cancelled(&self) -> bool {
        (**self).is_cancelled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_shares_the_flag() {
        let signal = CancelSignal::new();
        let other = signal.clone();
        assert!(!signal.is_cancelled());
        other.cancel();
        assert!(signal.is_cancelled(), "cancelling a clone must be visible");
    }

    #[test]
    fn a_fresh_signal_is_not_cancelled() {
        assert!(!CancelSignal::default().is_cancelled());
    }
}
