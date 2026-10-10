//! A running write's output, sent to the window a read of a pipe at a time, with a bound on
//! what waits for it (staging-and-commit phase 05's QA item 3).
//!
//! A hook can write far faster than the window applies what it is sent, and the outbox is
//! unbounded, so a hook printing gigabytes would queue gigabytes. The lane sends each read's
//! lines as one [`crate::worker::Update::WriteOutput`], counting its bytes into a budget shared
//! with the window; the update carries an [`OutputReceipt`] that gives them back when the window
//! has dropped it. While more than [`IN_FLIGHT_BYTES`] wait, the lane holds the newest lines
//! itself instead — at most [`HELD_BYTES`] and [`HELD_LINES`], the oldest let go — and sends
//! them with the next read that finds the window caught up, or as the write ends. What it lets
//! go of is older than what the window's own tail keeps (`commit_box_state::OutputTail`, the
//! same bounds), so the window ends with the same latest lines either way.
//!
//! Residual: while the window is behind, the held lines wait for git's next read or its end,
//! so a hook that writes a burst the window cannot keep up with and then falls silent shows the
//! end of the burst only when it writes again or ends.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use cairn_model::ScrubbedLines;

/// How many bytes of output may wait for the window before the lane holds the rest itself.
pub const IN_FLIGHT_BYTES: usize = 1024 * 1024;

/// The most the lane holds while the window is behind: the window's own tail's bounds.
pub const HELD_BYTES: usize = 1024 * 1024;
pub const HELD_LINES: usize = 10_000;

/// Gives a [`crate::worker::Update::WriteOutput`]'s bytes back to the budget as it is dropped —
/// on the UI thread, by an atomic subtraction, never a wait. A clone gives back nothing, so the
/// bytes are given back once.
pub struct OutputReceipt {
    budget: Option<Arc<AtomicUsize>>,
    bytes: usize,
}

impl OutputReceipt {
    /// A receipt for nothing, for an update built where no budget counts it.
    pub fn none() -> Self {
        Self {
            budget: None,
            bytes: 0,
        }
    }
}

impl Drop for OutputReceipt {
    fn drop(&mut self) {
        if let Some(budget) = &self.budget {
            budget.fetch_sub(self.bytes, Ordering::AcqRel);
        }
    }
}

impl Clone for OutputReceipt {
    fn clone(&self) -> Self {
        Self::none()
    }
}

impl PartialEq for OutputReceipt {
    /// Receipts are bookkeeping: two updates with the same lines are the same update.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for OutputReceipt {}

impl std::fmt::Debug for OutputReceipt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OutputReceipt({} bytes)", self.bytes)
    }
}

/// The lane's side: what is held while the window is behind, and the budget it counts against.
pub(super) struct OutputFlow {
    budget: Arc<AtomicUsize>,
    held: ScrubbedLines,
}

impl OutputFlow {
    pub(super) fn new(budget: Arc<AtomicUsize>) -> Self {
        Self {
            budget,
            held: ScrubbedLines::new(),
        }
    }

    /// One read's lines, as the engine split and scrubbed them: sent at once with what was held
    /// while the window has room, held otherwise. `send` is handed the lines and their receipt.
    pub(super) fn read(
        &mut self,
        lines: &ScrubbedLines,
        send: &mut dyn FnMut(ScrubbedLines, OutputReceipt),
    ) {
        self.held.append(lines);
        self.held.keep_last_lines(HELD_LINES);
        self.held.keep_last(HELD_BYTES);
        if self.budget.load(Ordering::Acquire) < IN_FLIGHT_BYTES {
            self.flush(send);
        }
    }

    /// Everything held, sent whatever the window is behind by: the write is ending.
    pub(super) fn flush(&mut self, send: &mut dyn FnMut(ScrubbedLines, OutputReceipt)) {
        if self.held.is_empty() {
            return;
        }
        let lines = std::mem::take(&mut self.held);
        let bytes = lines.bytes();
        self.budget.fetch_add(bytes, Ordering::AcqRel);
        send(
            lines,
            OutputReceipt {
                budget: Some(Arc::clone(&self.budget)),
                bytes,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A read is one update while the window keeps up; past the budget the lane holds the
    /// newest lines, bounded, and sends them once the window gives bytes back, or as the write
    /// ends; and every byte sent is given back as its updates are dropped. Caught by: one
    /// update a line, an unbounded queue, the newest lines dropped, or a budget that leaks.
    fn read(lines: &[&str]) -> ScrubbedLines {
        ScrubbedLines::scrubbing(&lines.join("\n"))
    }

    #[test]
    fn output_is_one_update_a_read_and_what_waits_is_bounded() {
        let budget = Arc::new(AtomicUsize::new(0));
        let mut flow = OutputFlow::new(Arc::clone(&budget));
        let mut sent: Vec<(ScrubbedLines, OutputReceipt)> = Vec::new();
        flow.read(&read(&["a", "b", "c"]), &mut |lines, receipt| {
            sent.push((lines, receipt))
        });
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0.lines().collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(budget.load(Ordering::Acquire), "a\nb\nc".len());

        // The window falls behind: a read past the budget is held, not sent.
        let wide = "x".repeat(IN_FLIGHT_BYTES);
        flow.read(&read(&[&wide]), &mut |lines, receipt| {
            sent.push((lines, receipt))
        });
        assert_eq!(sent.len(), 2, "sent while the window had room");
        for n in 0..HELD_LINES + 50 {
            let line = format!("held {n}");
            flow.read(&read(&[&line]), &mut |lines, receipt| {
                sent.push((lines, receipt))
            });
        }
        assert_eq!(sent.len(), 2, "sent while the window was behind");
        assert!(flow.held.line_count() <= HELD_LINES);
        assert_eq!(
            flow.held.lines().next_back(),
            Some(format!("held {}", HELD_LINES + 49).as_str()),
            "the newest line was let go"
        );

        // Wide lines while the window is behind: held by bytes, not count (phase 11's QA, TC8).
        let wide_line = |n: usize| format!("{n:06}{}", "w".repeat(64 * 1024));
        let mut sent_wide = Vec::new();
        let mut wide_flow = OutputFlow::new(Arc::new(AtomicUsize::new(IN_FLIGHT_BYTES)));
        for n in 0..40 {
            let line = wide_line(n);
            wide_flow.read(&read(&[&line]), &mut |lines, receipt| {
                sent_wide.push((lines, receipt))
            });
        }
        assert!(sent_wide.is_empty());
        assert!(
            wide_flow.held.bytes() <= HELD_BYTES,
            "{} bytes held",
            wide_flow.held.bytes()
        );
        assert!(
            wide_flow.held.line_count() < 40,
            "nothing let go of by bytes"
        );
        assert_eq!(
            wide_flow.held.lines().next_back(),
            Some(wide_line(39).as_str()),
            "the newest let go of"
        );

        // The window drops what it was sent: the bytes come back, and the next read sends.
        sent.clear();
        assert_eq!(budget.load(Ordering::Acquire), 0);
        flow.read(&read(&["after"]), &mut |lines, receipt| {
            sent.push((lines, receipt))
        });
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0.line_count(), HELD_LINES);
        assert!(
            sent[0].0.older_dropped(),
            "the lines let go of were not said"
        );
        assert_eq!(sent[0].0.lines().next_back(), Some("after"));

        // Behind again at the end: the flush sends what is held regardless.
        let receipt_held = sent.pop();
        flow.read(&read(&[&wide]), &mut |lines, receipt| {
            sent.push((lines, receipt))
        });
        flow.read(&read(&["tail"]), &mut |lines, receipt| {
            sent.push((lines, receipt))
        });
        let before = sent.len();
        flow.flush(&mut |lines, receipt| sent.push((lines, receipt)));
        assert_eq!(sent.len(), before + 1);
        assert_eq!(
            sent.last().map(|(lines, _)| lines.text().to_owned()),
            Some("tail".to_owned())
        );
        drop(receipt_held);
        let cloned = sent.last().map(|(_, receipt)| receipt.clone());
        drop(cloned);
        sent.clear();
        assert_eq!(budget.load(Ordering::Acquire), 0, "a clone gave bytes back");
    }
}
