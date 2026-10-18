//! How often the opening pass says how far it has read: at most once per interval of wall
//! time, the first once a whole interval has passed since it began.

use std::time::{Duration, Instant};

/// The count reaches whoever listens at most this often.
pub(crate) const REPORT_EVERY: Duration = Duration::from_millis(100);

/// When the next count is due.
#[derive(Debug)]
pub(super) struct Pace {
    every: Duration,
    last: Instant,
}

impl Pace {
    pub(super) fn starting(at: Instant, every: Duration) -> Self {
        Self { every, last: at }
    }

    /// Whether a count is due at `now`; when it is, the next is due an interval later.
    pub(super) fn due(&mut self, now: Instant) -> bool {
        if now.saturating_duration_since(self.last) < self.every {
            return false;
        }
        self.last = now;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a count before the first interval has passed, two inside one interval, or
    /// the next interval measured from the start rather than from the last count.
    #[test]
    fn a_count_is_due_at_most_once_per_interval() {
        let start = Instant::now();
        let at = |millis: u64| start + Duration::from_millis(millis);
        let mut pace = Pace::starting(start, REPORT_EVERY);
        let due: Vec<u64> = [0, 50, 99, 100, 150, 199, 230, 329, 330, 1000, 1001]
            .into_iter()
            .filter(|&millis| pace.due(at(millis)))
            .collect();
        assert_eq!(due, vec![100, 230, 330, 1000]);
    }
}
