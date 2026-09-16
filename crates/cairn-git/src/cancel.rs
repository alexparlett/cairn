//! Telling a query in progress to stop.
//!
//! The engine knows nothing about threads: it polls a signal the caller owns,
//! and the caller decides what sets it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Something a running query asks, often, whether it should stop.
///
/// A trait rather than a concrete flag so a test can count the polls and stop
/// the walk at a chosen commit rather than by racing it.
pub trait Cancel {
    /// Called once per commit visited. Returning `true` abandons the query.
    fn is_cancelled(&self) -> bool;
}

/// A flag one thread sets and another polls. Cloning shares it, so the worker
/// keeps one handle and hands another to whoever may abandon the work.
#[derive(Debug, Clone, Default)]
pub struct CancelSignal(Arc<AtomicBool>);

impl CancelSignal {
    /// A signal that is not yet set; a query given one that never fires runs to
    /// its limit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask every query holding this signal to stop. Idempotent, and it cannot
    /// be unset: un-abandoning a query is a race, not a feature.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}

impl Cancel for CancelSignal {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
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
