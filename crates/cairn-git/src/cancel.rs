//! Query cancellation.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub trait Cancel {
    /// Called once per commit visited. Returning `true` abandons the query.
    fn is_cancelled(&self) -> bool;
}

/// A flag one thread sets and another polls; cloning shares it.
#[derive(Debug, Clone, Default)]
pub struct CancelSignal(Arc<AtomicBool>);

impl CancelSignal {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops every query holding this signal. It cannot be unset.
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
