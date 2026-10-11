//! A running operation's output on its way to the window: one shared bounded tail the lane
//! fills and the window drains on a wake, the latest kept (staging-and-commit phase 20;
//! `review-code-app-ui.md` H3 (c), phase 12's QA #15 and #16).
//!
//! A hook can write far faster than the window applies what it is sent, and the outbox is
//! unbounded. So a lane never sends lines: it puts each read's lines — whole, scrubbed by the
//! engine's runner as it split them (R4.10, R12.2) — into its tap, a [`crate::output_tail`]
//! bounded by the one pair of bounds, and wakes the window with one update only when no wake is
//! already on its way. The window drains everything waiting when it applies the wake. However
//! far behind the window falls, at most one wake per tap waits in the outbox and the tap holds
//! the latest lines; what was let go of is older than what the window's own tails keep, and the
//! tap says so ([`crate::output_tail::OutputTail::cut`]). Nothing waits for git's next read: a
//! wake is sent with the first lines after each drain.
//!
//! Each write the local lane runs has a tap of its own, made as it starts and named by every
//! wake it sends with the write's id: a wake drains only its own write's lines, however late
//! the window applies it — one shared tap would let a wake still on its way for one write
//! drain the next write's lines under the first write's id. The network lane keeps one for its
//! fetch, the only operation it runs, and a fetch's progress has no id to misfile.
//!
//! The tap's mutex is taken by the lane for an append — a push of a read already copied, and,
//! past a bound, popping whole oldest reads — and by the window's drain, on the UI thread, for a
//! swap
//! (`OutputTap::drain`, a `worker/` function the UI thread calls; the root `CLAUDE.md` names
//! it among the residuals).

use std::sync::{Arc, Mutex, PoisonError};

use cairn_model::ScrubbedLines;

use crate::output_tail::OutputTail;

/// What waits for the window, and whether a wake for it is on its way.
#[derive(Debug, Default)]
struct Waiting {
    lines: OutputTail<ScrubbedLines>,
    woken: bool,
}

/// One lane's output on its way to the window. A clone is the same tap; two taps are equal
/// when they are the same tap, so an update carrying one compares by which tap it names.
#[derive(Clone, Default)]
pub struct OutputTap(Arc<Mutex<Waiting>>);

impl OutputTap {
    /// A tap holding nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// A tap already holding `lines`, as a lane would have left them: for a test's update.
    #[cfg(test)]
    pub fn holding(lines: &ScrubbedLines) -> Self {
        let tap = Self::new();
        tap.hold(lines);
        tap
    }

    /// One read's lines, kept after what waits: `true` when the window must be woken for them,
    /// since no wake is on its way.
    pub(super) fn hold(&self, lines: &ScrubbedLines) -> bool {
        if lines.is_empty() {
            return false;
        }
        // Copied before the lock, so the window's drain never waits behind the copy.
        let lines = lines.clone();
        let mut waiting = self.lock();
        waiting.lines.push(lines);
        !std::mem::replace(&mut waiting.woken, true)
    }

    /// Everything waiting, taken: the next read wakes the window again. On the UI thread.
    pub fn drain(&self) -> OutputTail<ScrubbedLines> {
        let mut waiting = self.lock();
        waiting.woken = false;
        waiting.lines.take()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Waiting> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl PartialEq for OutputTap {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for OutputTap {}

impl std::fmt::Debug for OutputTap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let waiting = self.lock();
        write!(
            f,
            "OutputTap({} lines waiting, woken: {})",
            waiting.lines.lines(),
            waiting.woken
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output_tail::{OUTPUT_BYTES, OUTPUT_LINES};

    fn read(lines: &[&str]) -> ScrubbedLines {
        ScrubbedLines::scrubbing(&lines.join("\n"))
    }

    /// The tap wakes the window once for whatever waits, however many reads arrive before it
    /// drains; holds the latest lines, bounded, while the window is behind, saying older ones
    /// went; and wakes it again with the first read after a drain — so the latest lines are
    /// never left waiting for git's next read. Caught by: a wake a read (the unbounded queue),
    /// an unbounded hold, the newest let go of, or a read after a drain that wakes nothing.
    #[test]
    fn one_wake_for_whatever_waits_and_the_latest_kept() {
        let tap = OutputTap::new();
        assert!(!tap.hold(&ScrubbedLines::new()), "nothing woke the window");
        assert!(tap.hold(&read(&["a", "b"])), "the first read woke nothing");
        let mut wakes = 1;
        for n in 0..OUTPUT_LINES * 3 {
            let line = format!("held {n}");
            wakes += usize::from(tap.hold(&read(&[&line])));
        }
        assert_eq!(wakes, 1, "a wake sent while one was on its way");
        let drained = tap.drain();
        assert!(drained.lines() <= OUTPUT_LINES && drained.bytes() <= OUTPUT_BYTES);
        assert!(drained.cut(), "the lines let go of were not said");
        assert_eq!(
            drained.iter().next_back().map(ScrubbedLines::text),
            Some(format!("held {}", OUTPUT_LINES * 3 - 1).as_str()),
            "the newest line was let go of"
        );
        assert!(tap.drain().is_empty(), "drained twice");
        assert!(
            tap.hold(&read(&["after"])),
            "a read after the drain woke nothing"
        );
        let after = tap.drain();
        assert_eq!(
            after.iter().map(ScrubbedLines::text).collect::<Vec<_>>(),
            ["after"]
        );
        assert!(!after.cut());

        // Wide lines: held by bytes, not count.
        let wide = |n: usize| format!("{n:06}{}", "w".repeat(64 * 1024));
        for n in 0..40 {
            tap.hold(&read(&[&wide(n)]));
        }
        let drained = tap.drain();
        assert!(drained.bytes() <= OUTPUT_BYTES, "{}", drained.bytes());
        assert_eq!(
            drained.iter().next_back().map(ScrubbedLines::text),
            Some(wide(39).as_str())
        );
        // Exactly as many reads as fit, the oldest of them first (phase 20's QA item 6).
        let fit = OUTPUT_BYTES / wide(0).len();
        assert_eq!(drained.iter().count(), fit);
        assert_eq!(
            drained.iter().next().map(ScrubbedLines::text),
            Some(wide(40 - fit).as_str())
        );

        // A clone is the same tap, and only it equals it.
        assert_eq!(tap.clone(), tap);
        assert_ne!(OutputTap::new(), tap);
    }
}
