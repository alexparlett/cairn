//! git's `prio_queue`: the greatest key first, and among equal keys the one put first.
//!
//! `prio-queue.c`'s `compare` breaks a tie of the queue's own compare by the insertion
//! counter each element is stamped with as it is put, the earlier first (v2.30.9
//! `prio-queue.c:4-11`, `cmp = ctr_i - ctr_j`; v2.56.0 `prio-queue.c:4-12`, the same as a
//! three-way compare), and `prio_queue_put` stamps `insertion_ctr++` (v2.30.9
//! `prio-queue.c:42`, v2.56.0 `prio-queue.c:73`, `:81`). Every key is then unique, so any
//! correct priority queue pops the same sequence; this one is a binary heap keyed by
//! `(key, Reverse(counter))`. gitoxide's own queue (`gix_revwalk::PriorityQueue`) is keyed by
//! the key alone and pops equal keys in no particular order, which is why it is not used.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// A max-queue whose ties pop in the order they were put.
#[derive(Debug)]
pub(super) struct Queue<K: Ord, T> {
    heap: BinaryHeap<Entry<K, T>>,
    /// git's `insertion_ctr`: one per put, never reset.
    put: u64,
}

#[derive(Debug)]
struct Entry<K, T> {
    key: K,
    order: Reverse<u64>,
    value: T,
}

impl<K: Ord, T> PartialEq for Entry<K, T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<K: Ord, T> Eq for Entry<K, T> {}

impl<K: Ord, T> PartialOrd for Entry<K, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Ord, T> Ord for Entry<K, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key
            .cmp(&other.key)
            .then_with(|| self.order.cmp(&other.order))
    }
}

impl<K: Ord, T> Queue<K, T> {
    pub(super) fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            put: 0,
        }
    }

    pub(super) fn put(&mut self, key: K, value: T) {
        let order = Reverse(self.put);
        self.put += 1;
        self.heap.push(Entry { key, order, value });
    }

    /// Takes the next place in the order of puts without putting anything: the place an
    /// element decided later is put at ([`Queue::put_at`]), as if it had been put now.
    pub(super) fn reserve(&mut self) -> u64 {
        let place = self.put;
        self.put += 1;
        place
    }

    /// Puts `value` at a place [`Queue::reserve`] took.
    pub(super) fn put_at(&mut self, key: K, place: u64, value: T) {
        self.heap.push(Entry {
            key,
            order: Reverse(place),
            value,
        });
    }

    /// The greatest key, the earliest put among equals.
    pub(super) fn pop(&mut self) -> Option<(K, T)> {
        self.heap.pop().map(|entry| (entry.key, entry.value))
    }

    /// What [`Queue::pop`] would answer, left in place.
    pub(super) fn peek(&self) -> Option<(&K, &T)> {
        self.heap.peek().map(|entry| (&entry.key, &entry.value))
    }

    /// The key and the place of what [`Queue::pop`] would answer.
    pub(super) fn peek_place(&self) -> Option<(&K, u64)> {
        self.heap.peek().map(|entry| (&entry.key, entry.order.0))
    }

    pub(super) fn len(&self) -> usize {
        self.heap.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: ties broken by the heap's own arrangement, or the latest put first.
    #[test]
    fn ties_pop_in_the_order_they_were_put() {
        let mut queue = Queue::new();
        // Enough equal keys that a heap left to itself reorders them.
        for value in 0..64 {
            queue.put(7u64, value);
        }
        queue.put(9, 100);
        queue.put(3, 101);
        queue.put(9, 102);
        let mut popped = Vec::new();
        while let Some((key, value)) = queue.pop() {
            popped.push((key, value));
        }
        let mut expected = vec![(9, 100), (9, 102)];
        expected.extend((0..64).map(|value| (7, value)));
        expected.push((3, 101));
        assert_eq!(popped, expected);
    }

    /// The counter is never reset: a value put after others were popped still pops after
    /// every equal key put before it.
    #[test]
    fn the_counter_runs_on_across_pops() {
        let mut queue = Queue::new();
        queue.put(1u64, "first");
        queue.put(1, "second");
        assert_eq!(queue.pop(), Some((1, "first")));
        queue.put(1, "third");
        assert_eq!(queue.peek(), Some((&1, &"second")));
        assert_eq!(queue.pop(), Some((1, "second")));
        assert_eq!(queue.pop(), Some((1, "third")));
        assert_eq!(queue.len(), 0);
    }

    /// An element put at a reserved place pops where it would have, had it been put when the
    /// place was taken. Caught by: a reserved place that takes no number, or a late put
    /// stamped with the counter as it is then.
    #[test]
    fn an_element_put_at_a_reserved_place_pops_from_it() {
        let mut queue = Queue::new();
        queue.put(5u64, "before");
        let place = queue.reserve();
        queue.put(5, "after");
        queue.put(4, "older");
        assert_eq!(queue.peek_place(), Some((&5, 0)));
        assert_eq!(queue.pop(), Some((5, "before")));
        queue.put_at(5, place, "reserved");
        assert_eq!(queue.pop(), Some((5, "reserved")));
        assert_eq!(queue.pop(), Some((5, "after")));
        assert_eq!(queue.pop(), Some((4, "older")));
    }
}
