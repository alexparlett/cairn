//! When the window is told how far an opening pass has read: nothing until the pass has run
//! [`READING_AFTER`], then at most once per [`READING_EVERY`]. The worker holds the count back,
//! so the window keeps no timer: what it draws is "Reading history… N commits" from the first
//! count it is sent, and the open's own sentence until then.

use std::time::{Duration, Instant};

/// How long an opening pass runs before the window is told its count.
pub(crate) const READING_AFTER: Duration = Duration::from_millis(250);

/// The least time between two counts the window is told.
pub(crate) const READING_EVERY: Duration = Duration::from_millis(100);

/// The hold-back and the interval a lane tells the window by: [`READING_AFTER`] and
/// [`READING_EVERY`], or another pair for a test that needs a count from a short pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pacing {
    pub(crate) after: Duration,
    pub(crate) every: Duration,
}

impl Default for Pacing {
    fn default() -> Self {
        Self {
            after: READING_AFTER,
            every: READING_EVERY,
        }
    }
}

/// Whether a count the walk reports is told to the window, for one opening pass.
#[derive(Debug)]
pub(crate) struct ReadingPace {
    pacing: Pacing,
    started: Instant,
    /// When the last count was told.
    told: Option<Instant>,
}

impl ReadingPace {
    pub(crate) fn starting(at: Instant, pacing: Pacing) -> Self {
        Self {
            pacing,
            started: at,
            told: None,
        }
    }

    /// Whether a count reported at `now` is told: once the pass has run its hold-back, and an
    /// interval after the last told.
    pub(crate) fn due(&mut self, now: Instant) -> bool {
        if now.saturating_duration_since(self.started) < self.pacing.after {
            return false;
        }
        if self
            .told
            .is_some_and(|told| now.saturating_duration_since(told) < self.pacing.every)
        {
            return false;
        }
        self.told = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a count told before the pass has run 250 ms (a strip that flashes up on every
    /// short reopen), two inside 100 ms, or the interval measured from the start rather than
    /// from the last count told.
    #[test]
    fn the_count_is_told_once_the_pass_has_run_250_ms_then_at_most_every_100_ms() {
        assert_eq!(READING_AFTER, Duration::from_millis(250));
        assert_eq!(READING_EVERY, Duration::from_millis(100));
        let start = Instant::now();
        let at = |millis: u64| start + Duration::from_millis(millis);
        let mut pace = ReadingPace::starting(start, Pacing::default());
        let told: Vec<u64> = [
            0, 100, 200, 249, 250, 300, 349, 350, 351, 449, 450, 1000, 1050,
        ]
        .into_iter()
        .filter(|&millis| pace.due(at(millis)))
        .collect();
        assert_eq!(told, vec![250, 350, 450, 1000]);
    }
}
