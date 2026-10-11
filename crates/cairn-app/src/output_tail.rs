//! The one bounded tail of git's output (staging-and-commit phase 20; `review-code-app-ui.md`
//! H3 (b)): the latest [`OUTPUT_LINES`] lines and [`OUTPUT_BYTES`] bytes of what an operation
//! wrote, its oldest whole items let go of first. One type, one pair of bounds, for every holder
//! of a running operation's output — what the local and network lanes hold for the window
//! (`worker::OutputTap`), the commit box's output for its Git Error dialog
//! (`commit_box_state`), and each activity entry's lines (`crate::activity`).
//!
//! It keeps its items in an `Arc`, so a holder hands a snapshot out by a reference count
//! ([`OutputTail::snapshot`]) and the next item pushed copies them only while a snapshot is
//! still held (`Arc::make_mut`): a closed popover holds none, so a line costs what it carries.
//! Letting go pops whole items from the front, never moving what is kept.

use std::collections::VecDeque;
use std::sync::Arc;

use cairn_model::ScrubbedLines;

/// The most lines one tail keeps: the latest.
pub const OUTPUT_LINES: usize = 10_000;
/// The most bytes one tail keeps, whatever its count of lines.
pub const OUTPUT_BYTES: usize = 1024 * 1024;

/// What a tail counts of an item: its lines and its bytes.
pub trait Measured {
    fn lines(&self) -> usize;
    fn bytes(&self) -> usize;
}

impl Measured for String {
    fn lines(&self) -> usize {
        1
    }

    fn bytes(&self) -> usize {
        self.len()
    }
}

/// One read of a pipe: the lines it completed.
impl Measured for ScrubbedLines {
    fn lines(&self) -> usize {
        self.line_count()
    }

    fn bytes(&self) -> usize {
        ScrubbedLines::bytes(self)
    }
}

/// The latest items of an operation's output, bounded by [`OUTPUT_LINES`] and [`OUTPUT_BYTES`].
#[derive(Debug, Clone)]
pub struct OutputTail<T> {
    items: Arc<VecDeque<T>>,
    lines: usize,
    bytes: usize,
    /// Older output was let go of — here, or by a holder before it reached here.
    cut: bool,
}

impl<T> Default for OutputTail<T> {
    fn default() -> Self {
        Self {
            items: Arc::new(VecDeque::new()),
            lines: 0,
            bytes: 0,
            cut: false,
        }
    }
}

impl<T: Measured + Clone> OutputTail<T> {
    /// One item more, after the rest.
    pub fn push(&mut self, item: T) {
        self.extend(std::iter::once(item));
    }

    /// `items` after the rest; then the oldest let go of, whole, while either bound is passed —
    /// never the newest, which alone may pass them.
    pub fn extend(&mut self, items: impl IntoIterator<Item = T>) {
        let mut items = items.into_iter().peekable();
        if items.peek().is_none() {
            return;
        }
        let kept = Arc::make_mut(&mut self.items);
        for item in items {
            self.lines += item.lines();
            self.bytes += item.bytes();
            kept.push_back(item);
        }
        while (self.lines > OUTPUT_LINES || self.bytes > OUTPUT_BYTES) && kept.len() > 1 {
            let Some(oldest) = kept.pop_front() else {
                break;
            };
            self.lines -= oldest.lines();
            self.bytes -= oldest.bytes();
            self.cut = true;
        }
    }

    /// Older output was let go of before it reached here: said as this tail's own cut.
    pub fn mark_cut(&mut self) {
        self.cut = true;
    }

    /// Whether older output was let go of to keep it bounded.
    pub fn cut(&self) -> bool {
        self.cut
    }

    /// The items kept, oldest first, shared: a reference count, never a copy.
    pub fn snapshot(&self) -> Arc<VecDeque<T>> {
        Arc::clone(&self.items)
    }

    /// The items kept, oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> {
        self.items.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The lines it holds.
    pub fn lines(&self) -> usize {
        self.lines
    }

    /// The bytes it holds.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Everything it holds, taken, leaving it empty and uncut.
    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one tail's bounds: past either, the oldest whole items go and it says so; the
    /// newest alone is kept whatever it holds; and a snapshot held is not changed by what is
    /// pushed after it, while one let go of costs no copy. Caught by: a tail that grows without
    /// end, lets go of the newest, cuts silently, or changes a snapshot under its reader.
    #[test]
    fn the_tail_keeps_the_latest_whole_items_under_both_bounds() {
        let mut tail = OutputTail::default();
        tail.extend((0..OUTPUT_LINES + 5).map(|n| format!("line {n}")));
        assert_eq!(tail.lines(), OUTPUT_LINES);
        assert!(tail.cut());
        assert_eq!(
            tail.iter().next_back().map(String::as_str),
            Some(format!("line {}", OUTPUT_LINES + 4).as_str())
        );
        assert_eq!(
            tail.iter().next().map(String::as_str),
            Some("line 5"),
            "not the oldest let go of"
        );
        assert_eq!(tail.bytes(), tail.iter().map(String::len).sum::<usize>());

        // By bytes, whatever the count.
        let mut wide = OutputTail::default();
        let line = |n: usize| format!("{n:04}{}", "w".repeat(OUTPUT_BYTES / 3));
        for n in 0..7 {
            wide.push(line(n));
        }
        assert!(wide.bytes() <= OUTPUT_BYTES, "{}", wide.bytes());
        assert_eq!(wide.iter().next_back(), Some(&line(6)));

        // The newest alone over the bound is kept whole.
        let mut alone: OutputTail<ScrubbedLines> = OutputTail::default();
        let read = ScrubbedLines::scrubbing(&vec!["x"; OUTPUT_LINES + 1].join("\n"));
        alone.push(ScrubbedLines::scrubbing("older"));
        alone.push(read.clone());
        assert_eq!(alone.iter().collect::<Vec<_>>(), [&read]);
        assert!(alone.cut());

        // A snapshot is the reader's: pushed after, it is unchanged; dropped, nothing is copied.
        let mut tail = OutputTail::default();
        tail.push("a".to_owned());
        let held = tail.snapshot();
        tail.push("b".to_owned());
        assert_eq!(held.len(), 1, "a snapshot changed under its reader");
        drop(held);
        let before = Arc::as_ptr(&tail.snapshot());
        tail.push("c".to_owned());
        assert_eq!(
            Arc::as_ptr(&tail.snapshot()),
            before,
            "copied with no reader"
        );

        // Taken, it is empty and uncut; marked, it says so.
        let taken = tail.take();
        assert_eq!(taken.lines(), 3);
        assert!(tail.is_empty() && !tail.cut() && tail.bytes() == 0);
        tail.mark_cut();
        assert!(tail.cut());
    }
}
