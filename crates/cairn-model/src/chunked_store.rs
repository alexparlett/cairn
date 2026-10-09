//! Stores that grow in fixed chunks, never by doubling: a history's rows, its text, its
//! lane changes and its lane snapshots (PRD R4.7).
//!
//! A chunk is allocated once, at its full size, and never grows or moves, so what a store
//! holds is its chunks' capacity and a page appended costs at most a new chunk — never a
//! copy of everything before it, and never a doubling's slack of up to half the store. A
//! run of items is never split across chunks, so it reads back as one slice: a run that
//! does not fit what is left of the chunk being filled starts the next, and a run at least
//! a chunk long gets a chunk of exactly its own length, leaving the chunk being filled to
//! the runs after it.

use std::ops::Range;

/// Where a run sits in a [`Runs`] store: its chunk and offset, packed into `at` as
/// `chunk << SHIFT | offset`, and its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Span {
    at: u32,
    len: u32,
}

impl Span {
    pub(crate) fn len(self) -> usize {
        usize::try_from(self.len).unwrap_or(usize::MAX)
    }
}

/// A store is full: one more chunk would not be addressable in 32 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Full;

/// What a chunk holds: a `Vec` of items, or a `String` of text.
pub(crate) trait Chunk {
    type Run: ?Sized + RunOfNothing + 'static;

    fn with_capacity(capacity: usize) -> Self;
    fn len(&self) -> usize;
    fn run_len(run: &Self::Run) -> usize;
    /// Appends `run`, which the caller has checked fits the capacity left.
    fn append(&mut self, run: &Self::Run);
    fn run(&self, range: Range<usize>) -> Option<&Self::Run>;
    /// Bytes the chunk holds, by capacity.
    fn bytes(&self) -> usize;
}

impl<T: Copy + 'static> Chunk for Vec<T> {
    type Run = [T];

    fn with_capacity(capacity: usize) -> Self {
        Vec::with_capacity(capacity)
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn run_len(run: &[T]) -> usize {
        run.len()
    }

    fn append(&mut self, run: &[T]) {
        self.extend_from_slice(run);
    }

    fn run(&self, range: Range<usize>) -> Option<&[T]> {
        self.get(range)
    }

    fn bytes(&self) -> usize {
        Vec::capacity(self) * size_of::<T>()
    }
}

impl Chunk for String {
    type Run = str;

    fn with_capacity(capacity: usize) -> Self {
        String::with_capacity(capacity)
    }

    fn len(&self) -> usize {
        String::len(self)
    }

    fn run_len(run: &str) -> usize {
        run.len()
    }

    fn append(&mut self, run: &str) {
        self.push_str(run);
    }

    fn run(&self, range: Range<usize>) -> Option<&str> {
        // Only checks that both ends fall on a character's boundary.
        self.get(range)
    }

    fn bytes(&self) -> usize {
        String::capacity(self)
    }
}

/// Runs of items in chunks of `1 << SHIFT` items, each run read back whole by its [`Span`].
#[derive(Debug)]
pub(crate) struct Runs<C, const SHIFT: u32> {
    chunks: Vec<C>,
    /// The chunk runs are appended to, while one has room.
    filling: Option<usize>,
}

impl<C: Chunk, const SHIFT: u32> Runs<C, SHIFT> {
    const CHUNK: usize = 1 << SHIFT;
    /// The most chunks a 32-bit `at` can name.
    const MAX_CHUNKS: usize = 1 << (32 - SHIFT);

    pub(crate) fn new() -> Self {
        Self {
            chunks: Vec::new(),
            filling: None,
        }
    }

    pub(crate) fn push(&mut self, run: &C::Run) -> Result<Span, Full> {
        let len = C::run_len(run);
        let Ok(stored_len) = u32::try_from(len) else {
            return Err(Full);
        };
        if len == 0 {
            return Ok(Span::default());
        }
        // Within the chunk's own size, whatever capacity the allocator handed back: an
        // offset past it would not fit beside the chunk's number in `at`.
        let room = |chunk: &C| Self::CHUNK.saturating_sub(chunk.len()) >= len;
        let (chunk, offset) = match self.filling {
            Some(index) if self.chunks.get(index).is_some_and(room) => {
                let offset = self.chunks.get(index).map_or(0, Chunk::len);
                (index, offset)
            }
            _ => {
                let index = self.chunks.len();
                if index >= Self::MAX_CHUNKS {
                    return Err(Full);
                }
                // The list of chunks grows by one, not by doubling: a few hundred chunks
                // hold all of rust-lang/rust, and moving their headers is nothing beside
                // filling one.
                self.chunks.reserve_exact(1);
                // A run at least a chunk long has one of exactly its length, and the chunk
                // being filled stays the one to fill: a run of exactly a chunk would fill a
                // fresh chunk to the brim anyway, so making it the one to fill would only
                // abandon the room left in the last.
                if len >= Self::CHUNK {
                    self.chunks.push(C::with_capacity(len));
                } else {
                    self.chunks.push(C::with_capacity(Self::CHUNK));
                    self.filling = Some(index);
                }
                (index, 0)
            }
        };
        let Some(target) = self.chunks.get_mut(chunk) else {
            return Err(Full);
        };
        target.append(run);
        let (Ok(chunk), Ok(offset)) = (u32::try_from(chunk), u32::try_from(offset)) else {
            return Err(Full);
        };
        Ok(Span {
            at: (chunk << SHIFT) | offset,
            len: stored_len,
        })
    }

    pub(crate) fn get(&self, span: Span) -> Option<&C::Run> {
        if span.len == 0 {
            return Some(C::Run::nothing());
        }
        let chunk = usize::try_from(span.at >> SHIFT).ok()?;
        let offset = usize::try_from(span.at & ((1 << SHIFT) - 1)).ok()?;
        let end = offset.checked_add(span.len())?;
        self.chunks.get(chunk)?.run(offset..end)
    }

    /// Bytes held, by capacity: every chunk, and the vector that lists them.
    pub(crate) fn bytes(&self) -> usize {
        self.chunks.iter().map(Chunk::bytes).sum::<usize>()
            + self.chunks.capacity() * size_of::<C>()
    }

    /// Names every chunk a 32-bit address can, each empty, so the next run that holds
    /// anything finds the store full: how a test reaches a history's limit without
    /// gigabytes of text.
    #[cfg(test)]
    pub(crate) fn fill_every_address(&mut self) {
        while self.chunks.len() < Self::MAX_CHUNKS {
            self.chunks.push(C::with_capacity(0));
        }
        self.filling = None;
    }

    #[cfg(test)]
    pub(crate) fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

/// The run an empty span reads back as.
pub(crate) trait RunOfNothing {
    fn nothing() -> &'static Self;
}

impl<T> RunOfNothing for [T] {
    fn nothing() -> &'static Self {
        &[]
    }
}

impl RunOfNothing for str {
    fn nothing() -> &'static Self {
        ""
    }
}

/// Items numbered from zero, in chunks of `1 << SHIFT`: item `n` is in chunk `n >> SHIFT`.
#[derive(Debug)]
pub(crate) struct Chunks<T, const SHIFT: u32> {
    runs: Runs<Vec<T>, SHIFT>,
    len: usize,
}

impl<T: Copy + 'static, const SHIFT: u32> Chunks<T, SHIFT> {
    pub(crate) fn new() -> Self {
        Self {
            runs: Runs::new(),
            len: 0,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Appends `item`, returning its number. One item never overflows a chunk, so every
    /// chunk but the last is full and item `n` is at offset `n` of the whole.
    pub(crate) fn push(&mut self, item: T) -> Result<u32, Full> {
        let number = u32::try_from(self.len).map_err(|_| Full)?;
        self.runs.push(std::slice::from_ref(&item))?;
        self.len += 1;
        Ok(number)
    }

    pub(crate) fn get(&self, number: usize) -> Option<T> {
        let at = u32::try_from(number).ok()?;
        self.runs
            .get(Span { at, len: 1 })
            .and_then(|run| run.first().copied())
    }

    /// Item `number`, to change in place: its chunk never moves, so nothing else does.
    pub(crate) fn get_mut(&mut self, number: usize) -> Option<&mut T> {
        let chunk = number >> SHIFT;
        let offset = number & ((1 << SHIFT) - 1);
        if number >= self.len {
            return None;
        }
        self.runs.chunks.get_mut(chunk)?.get_mut(offset)
    }

    pub(crate) fn bytes(&self) -> usize {
        self.runs.bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: a run split across two chunks, or a chunk grown past its size.
    #[test]
    fn a_run_that_does_not_fit_starts_the_next_chunk_and_reads_back_whole() {
        let mut runs: Runs<Vec<u16>, 3> = Runs::new();
        let first = runs.push(&[1, 2, 3, 4, 5]).unwrap();
        let second = runs.push(&[6, 7, 8, 9]).unwrap();
        let third = runs.push(&[10, 11, 12]).unwrap();
        assert_eq!(runs.get(first), Some(&[1, 2, 3, 4, 5][..]));
        assert_eq!(runs.get(second), Some(&[6, 7, 8, 9][..]));
        assert_eq!(runs.get(third), Some(&[10, 11, 12][..]));
        assert_eq!(runs.chunk_count(), 2, "the third run fits after the second");
        for chunk in &runs.chunks {
            assert_eq!(chunk.capacity(), 8, "a chunk grew past its fixed size");
        }
    }

    /// Caught by: a long run spread over chunks, or the chunk being filled abandoned for it.
    #[test]
    fn a_run_longer_than_a_chunk_gets_a_chunk_of_its_own_length() {
        let mut runs: Runs<String, 4> = Runs::new();
        let short = runs.push("abc").unwrap();
        let long = "x".repeat(40);
        let long_span = runs.push(&long).unwrap();
        let after = runs.push("def").unwrap();
        assert_eq!(runs.get(short), Some("abc"));
        assert_eq!(runs.get(long_span), Some(long.as_str()));
        assert_eq!(runs.get(after), Some("def"));
        assert_eq!(
            runs.chunk_count(),
            2,
            "the short runs share the first chunk"
        );
        assert_eq!(
            runs.bytes(),
            16 + 40 + runs.chunks.capacity() * size_of::<String>()
        );
    }

    #[test]
    fn text_past_ascii_and_an_empty_run_read_back_exactly() {
        let mut runs: Runs<String, 4> = Runs::new();
        let empty = runs.push("").unwrap();
        let accented = runs.push("Zoë Ångström").unwrap();
        assert_eq!(runs.get(empty), Some(""));
        assert_eq!(runs.get(accented), Some("Zoë Ångström"));
    }

    /// Caught by: numbering items by anything but their order.
    #[test]
    fn items_are_numbered_in_order_across_chunks() {
        let mut chunks: Chunks<u64, 2> = Chunks::new();
        for n in 0..11u64 {
            assert_eq!(chunks.push(n * 10), Ok(n as u32));
        }
        assert_eq!(chunks.len(), 11);
        for n in 0..11usize {
            assert_eq!(chunks.get(n), Some(n as u64 * 10));
        }
        assert_eq!(chunks.get(11), None);
        assert_eq!(chunks.runs.chunk_count(), 3);
    }

    /// The boundary: a run of exactly a chunk takes a chunk of its own and leaves the one
    /// being filled to the runs after it. Caught by: `len > CHUNK`, which fills a fresh chunk
    /// with it and abandons the first chunk's room.
    #[test]
    fn a_run_exactly_a_chunk_long_leaves_the_chunk_being_filled_alone() {
        let mut runs: Runs<String, 4> = Runs::new();
        let before = runs.push("abc").unwrap();
        let whole = "y".repeat(16);
        let whole_span = runs.push(&whole).unwrap();
        let after = runs.push("def").unwrap();
        assert_eq!(runs.get(before), Some("abc"));
        assert_eq!(runs.get(whole_span), Some(whole.as_str()));
        assert_eq!(runs.get(after), Some("def"));
        assert_eq!(
            runs.chunk_count(),
            2,
            "the run after a whole chunk's did not go back to the chunk being filled"
        );
        assert_eq!(runs.chunks[1].capacity(), 16);
    }

    /// Caught by: an item changed in place reading back under another number, a change
    /// reaching past the chunk boundary, or a number past the end handed out.
    #[test]
    fn an_item_changed_in_place_is_that_item_alone() {
        let mut items: Chunks<u32, 2> = Chunks::new();
        for n in 0..10 {
            items.push(n).unwrap();
        }
        for number in [0, 3, 4, 9] {
            if let Some(item) = items.get_mut(number) {
                *item += 100;
            }
        }
        let read: Vec<u32> = (0..10).filter_map(|n| items.get(n)).collect();
        assert_eq!(read, [100, 1, 2, 103, 104, 5, 6, 7, 8, 109]);
        assert!(items.get_mut(10).is_none(), "a number past the end");
        assert!(items.get_mut(usize::MAX).is_none());
    }

    /// Caught by: an address past 32 bits wrapping onto chunk zero.
    #[test]
    fn a_store_that_cannot_name_another_chunk_says_it_is_full() {
        let mut runs: Runs<Vec<u8>, 30> = Runs::new();
        for _ in 0..4 {
            runs.chunks.push(Vec::new());
        }
        assert_eq!(runs.push(&[1]), Err(Full));
    }
}
