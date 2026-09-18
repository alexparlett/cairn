//! R1.5, decided by counting: asking a row projection for one row allocates nothing,
//! however long the diff is.
//!
//! The unit tests next to the projections pin what the index STORED —
//! `one_row_of_a_hundred_thousand_lines_is_reached_through_four_index_entries` and
//! `many_changes_index_by_change_and_not_by_row`. Nothing there measures what a LOOKUP
//! builds, and the gap is walkable: renaming `row` to `row_uncached` and giving `row` the
//! body
//!
//! ```text
//! (0..self.len()).map(|n| self.row_uncached(n)).collect::<Vec<_>>().get(row).copied().flatten()
//! ```
//!
//! leaves every one of those assertions green while making a single row cost a hundred
//! thousand builds and a hundred thousand rows of heap. That mutation was applied and this
//! file watched go red on it before it was committed; it is the reason the file exists.
//!
//! Why a counter and not a clock: a timing ratio is flaky on a shared machine, and a count
//! kept inside `TextDiff` would put test state in a seam type. `allocation-counter` carries
//! its own `#[global_allocator]`, so linking it replaces the allocator of THIS test binary
//! and of nothing else, and its counters are thread-local — `cargo test`'s parallel threads
//! cannot leak into each other's totals.

use cairn_model::{
    ChangedRange, Context, DiffLine, LineSpan, SideBySideRows, TextDiff, UnifiedRows,
};

/// A hundred thousand lines with one line replaced in the middle: at entire-file context
/// that is a hundred thousand and two rows over four index entries, so every row number
/// below reaches through a piece that is a long way from where it started.
fn a_hundred_thousand_lines() -> TextDiff {
    let mut old: Vec<DiffLine> = (0..100_000)
        .map(|n| DiffLine::terminated(format!("line {n}")))
        .collect();
    let mut new = old.clone();
    old[50_000] = DiffLine::terminated("before");
    new[50_000] = DiffLine::terminated("after");
    TextDiff::new(
        old,
        new,
        vec![ChangedRange::new(
            LineSpan::at(50_000, 1),
            LineSpan::at(50_000, 1),
        )],
    )
}

/// The row numbers worth asking for: the header, the row after it, the two sides of the
/// change deep in the file, and the last row of all. A lookup that allocated only for one
/// kind of piece would still be caught.
fn probes(len: usize) -> Vec<usize> {
    vec![0, 1, 25_000, 50_000, 50_001, 50_002, 75_000, len - 1]
}

/// Warms whatever the measuring machinery itself initialises on this thread, so the first
/// measured lookup is measured on the same footing as the rest.
fn warm_up(f: impl FnOnce()) {
    let _ = allocation_counter::measure(f);
}

#[test]
fn a_unified_row_costs_no_allocation_however_long_the_diff_is() {
    let text = a_hundred_thousand_lines();
    let rows = UnifiedRows::new(&text, Context::EntireFile);
    assert_eq!(
        rows.len(),
        100_002,
        "header, every line, and the extra side"
    );
    warm_up(|| {
        rows.row(0);
    });

    for row in probes(rows.len()) {
        let mut found = None;
        let info = allocation_counter::measure(|| {
            found = rows.row(row);
        });
        assert!(
            found.is_some(),
            "unified row {row} of {} answered nothing, so nothing was measured",
            rows.len()
        );
        assert_eq!(
            info.count_total,
            0,
            "asking for unified row {row} of {} allocated {} times and {} bytes; a row \
             lookup builds the one row it was asked for, never a collection of them",
            rows.len(),
            info.count_total,
            info.bytes_total
        );
    }
}

#[test]
fn a_side_by_side_row_costs_no_allocation_however_long_the_diff_is() {
    let text = a_hundred_thousand_lines();
    let rows = SideBySideRows::new(&text, Context::EntireFile);
    assert_eq!(
        rows.len(),
        100_001,
        "header and every line, the changed pair drawn side by side"
    );
    warm_up(|| {
        rows.row(0);
    });

    for row in probes(rows.len()) {
        let mut found = None;
        let info = allocation_counter::measure(|| {
            found = rows.row(row);
        });
        assert!(
            found.is_some(),
            "side-by-side row {row} of {} answered nothing, so nothing was measured",
            rows.len()
        );
        assert_eq!(
            info.count_total,
            0,
            "asking for side-by-side row {row} of {} allocated {} times and {} bytes; a row \
             lookup builds the one row it was asked for, never a collection of them",
            rows.len(),
            info.count_total,
            info.bytes_total
        );
    }
}

/// The counter is not measuring an empty closure. Without this, an
/// `allocation_counter::measure` that had stopped counting — a feature gate lost, the
/// global allocator displaced by another dependency — would leave both tests above green
/// and the pin dead.
#[test]
fn the_counter_sees_an_allocation_when_there_is_one() {
    warm_up(|| {});
    let info = allocation_counter::measure(|| {
        let line = DiffLine::terminated("a row that owned its bytes would look like this");
        assert!(!line.bytes().is_empty());
    });
    assert!(
        info.count_total > 0,
        "the allocation counter reported {} allocations for a heap-allocating closure, so \
         the zero it reports for a row lookup decides nothing",
        info.count_total
    );
}
