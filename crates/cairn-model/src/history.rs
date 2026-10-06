//! The rows a reader keeps of a history, and how each is read (PRD R4.7, L14).
//!
//! A kept row is slim: its commit's id once, a parent count, its subject as a span of a
//! text store the whole history shares, its author as a number in an author table the
//! history shares, its author date, its lane and a span of the history's lane-change store
//! — and, every [`crate::LaneAssigner::snapshot_every`]th row, the number of a lane snapshot
//! the history keeps. It is `Copy`: no row owns a heap allocation. Every store grows in
//! fixed chunks, never by doubling ([`crate::chunked_store`]), and a row is read through the
//! [`History`] that holds them, as a [`HistoryRow`].

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::chunked_store::{Chunks, Full, Runs, Span};
use crate::edge_derivation::{LaidOutRow, LateLine, SnapshotView};
use crate::rows_page::RowsPage;
use crate::{CommitSummary, LaidOutRows, Lane, LaneChange, Oid, RowEdges, row_edges};

/// Not `#[non_exhaustive]`: consumers match every variant, with no wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowContent {
    Commit(CommitSummary),
    /// Test-only second row kind; the shape tests below need one.
    #[cfg(test)]
    NotACommit,
}

/// A row's stable identity: not an index, and not an [`Oid`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowId {
    Commit(Oid),
    /// Test-only, and carries no `Oid`.
    #[cfg(test)]
    NotACommit,
}

/// A history is too large for its stores' 32-bit addresses: some 4 GiB of subjects, or four
/// billion rows or lane changes. Rows appended before it was found are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryFull;

impl std::fmt::Display for HistoryFull {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the history is too large to hold")
    }
}

impl std::error::Error for HistoryFull {}

impl From<Full> for HistoryFull {
    fn from(_: Full) -> Self {
        Self
    }
}

/// No snapshot: the row's edges are derived from one above it.
const NO_SNAPSHOT: u32 = u32::MAX;
/// The end of a chain of authors whose names hash alike.
const NO_AUTHOR: u32 = u32::MAX;

/// One kept row. `Copy`, so it can own nothing on the heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoredRow {
    author_time: i64,
    subject: Span,
    changes: Span,
    author: u32,
    lane: Lane,
    snapshot: u32,
    /// Saturating: only whether a commit has more than one parent is drawn.
    parents: u16,
    id: Oid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoredSnapshot {
    open: Span,
    late: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoredAuthor {
    name: Span,
    /// The next author whose name hashes as this one's does.
    next: u32,
}

// No retained row owns a heap allocation: a `Copy` type cannot hold a `String`, a `Vec` or
// a `Box`, and none of these needs dropping. This stops compiling when one would.
const _: () = {
    const fn copy<T: Copy>() {}
    copy::<StoredRow>();
    copy::<StoredSnapshot>();
    copy::<StoredAuthor>();
    assert!(!std::mem::needs_drop::<StoredRow>());
};

/// Rows a reader keeps, and the stores they are read through.
pub struct History {
    rows: Chunks<StoredRow, 10>,
    /// Subjects and author names.
    text: Runs<String, 16>,
    changes: Runs<Vec<LaneChange>, 12>,
    snapshots: Chunks<StoredSnapshot, 8>,
    open_lanes: Runs<Vec<u64>, 12>,
    late_lines: Runs<Vec<LateLine>, 8>,
    authors: Chunks<StoredAuthor, 10>,
    /// The first author whose name hashes to each key; the rest are chained by `next`.
    author_index: HashMap<u64, u32>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for History {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("History")
            .field("rows", &self.rows.len())
            .field("authors", &self.authors.len())
            .finish_non_exhaustive()
    }
}

/// What a history holds, in bytes, by capacity: every chunk of every store whole, the last
/// one's unused room included, and the vectors that list them. Allocator overhead is not
/// counted. The author index's share is an estimate (`index_bytes`): the standard
/// library's hash table does not report its buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RetainedBytes {
    pub rows: usize,
    /// Subjects and author names.
    pub text: usize,
    pub lane_changes: usize,
    pub snapshots: usize,
    /// The author table, and an estimate of the index that finds a name in it.
    pub authors: usize,
}

impl RetainedBytes {
    pub fn total(&self) -> usize {
        self.rows + self.text + self.lane_changes + self.snapshots + self.authors
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            rows: Chunks::new(),
            text: Runs::new(),
            changes: Runs::new(),
            snapshots: Chunks::new(),
            open_lanes: Runs::new(),
            late_lines: Runs::new(),
            authors: Chunks::new(),
            author_index: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.len() == 0
    }

    /// Row `index`, or `None` past the end.
    pub fn row(&self, index: usize) -> Option<HistoryRow<'_>> {
        self.rows.get(index).map(|row| HistoryRow {
            history: self,
            index,
            row,
        })
    }

    /// Every row, in order.
    pub fn rows(&self) -> impl Iterator<Item = HistoryRow<'_>> + '_ {
        (0..self.len()).filter_map(|index| self.row(index))
    }

    /// Row `index`'s identity, read without its text.
    pub fn id(&self, index: usize) -> Option<RowId> {
        self.rows.get(index).map(|row| RowId::Commit(row.id))
    }

    /// Where the row `id` is: a scan of every row, for a press, never for a frame.
    pub fn position(&self, id: RowId) -> Option<usize> {
        (0..self.len()).find(|&index| self.id(index) == Some(id))
    }

    /// How many authors the history names, each once.
    pub fn author_count(&self) -> usize {
        self.authors.len()
    }

    /// Appends `page`'s rows, copying its text, its new authors, its lane changes and its
    /// snapshots into the history's stores. On [`HistoryFull`] the rows appended before it
    /// stay; the rest of the page does not.
    pub fn append(&mut self, page: RowsPage) -> Result<(), HistoryFull> {
        let mut authors = Vec::with_capacity(page.authors.len());
        for &range in &page.authors {
            authors.push(self.author_of(page.text(range))?);
        }
        for row in &page.rows {
            let subject = self.text.push(page.text(row.subject))?;
            let changes = page
                .changes
                .get(row.changes.0..row.changes.1)
                .unwrap_or_default();
            let changes = self.changes.push(changes)?;
            let snapshot = match row.snapshot.and_then(|at| page.snapshots.get(at)) {
                Some(snapshot) => {
                    let open = self.open_lanes.push(&snapshot.open)?;
                    let late = self.late_lines.push(&snapshot.late)?;
                    self.snapshots.push(StoredSnapshot { open, late })?
                }
                None => NO_SNAPSHOT,
            };
            self.rows.push(StoredRow {
                author_time: row.author_time,
                subject,
                changes,
                author: authors.get(row.author).copied().unwrap_or(NO_AUTHOR),
                lane: row.lane,
                snapshot,
                parents: u16::try_from(row.parents).unwrap_or(u16::MAX),
                id: row.id,
            })?;
        }
        Ok(())
    }

    /// What the history holds, by capacity.
    pub fn retained(&self) -> RetainedBytes {
        RetainedBytes {
            rows: self.rows.bytes(),
            text: self.text.bytes(),
            lane_changes: self.changes.bytes(),
            snapshots: self.snapshots.bytes() + self.open_lanes.bytes() + self.late_lines.bytes(),
            authors: self.authors.bytes() + index_bytes(self.author_index.capacity()),
        }
    }

    /// The number of the author `name`, added to the table if the history does not name
    /// them yet.
    fn author_of(&mut self, name: &str) -> Result<u32, HistoryFull> {
        let key = {
            let mut hasher = DefaultHasher::new();
            name.hash(&mut hasher);
            hasher.finish()
        };
        self.author_under(name, key)
    }

    /// [`Self::author_of`], with the name's index key given: every author filed under a
    /// key is chained from the newest, so names whose keys collide are told apart by name.
    fn author_under(&mut self, name: &str, key: u64) -> Result<u32, HistoryFull> {
        let first = self.author_index.get(&key).copied();
        let mut at = first;
        while let Some(number) = at {
            let Some(author) = self.authors.get(number as usize) else {
                break;
            };
            if self.text.get(author.name) == Some(name) {
                return Ok(number);
            }
            at = (author.next != NO_AUTHOR).then_some(author.next);
        }
        let name = self.text.push(name)?;
        let number = self.authors.push(StoredAuthor {
            name,
            next: first.unwrap_or(NO_AUTHOR),
        })?;
        self.author_index.insert(key, number);
        Ok(number)
    }

    fn text_of(&self, span: Span) -> &str {
        self.text.get(span).unwrap_or_default()
    }

    fn author_name(&self, number: u32) -> &str {
        self.authors
            .get(number as usize)
            .map_or("", |author| self.text_of(author.name))
    }

    fn laid_out_row(&self, row: StoredRow) -> LaidOutRow<'_> {
        let snapshot = (row.snapshot != NO_SNAPSHOT)
            .then(|| self.snapshots.get(row.snapshot as usize))
            .flatten()
            .map(|stored| SnapshotView {
                open: self.open_lanes.get(stored.open).unwrap_or_default(),
                late: self.late_lines.get(stored.late).unwrap_or_default(),
            });
        LaidOutRow {
            lane: row.lane,
            changes: self.changes.get(row.changes).unwrap_or_default(),
            snapshot,
        }
    }
}

/// The bytes a hash table of `capacity` `(u64, u32)` entries holds, as hashbrown — the
/// standard library's table — lays it out: a power of two of buckets, of which it fills
/// seven eighths (all but one under eight), each an entry and a control byte, and one
/// group of control bytes past the end (16, its SSE2 width). An estimate: std reports a
/// capacity, not its buckets, and alignment padding is not counted.
fn index_bytes(capacity: usize) -> usize {
    if capacity == 0 {
        return 0;
    }
    let buckets = if capacity < 8 {
        capacity + 1
    } else {
        capacity / 7 * 8
    };
    buckets.next_power_of_two() * (size_of::<(u64, u32)>() + 1) + 16
}

/// A history is drawn by deriving each row's edges from the rows above it.
impl LaidOutRows for History {
    fn row_count(&self) -> usize {
        self.len()
    }

    fn laid_out(&self, index: usize) -> Option<LaidOutRow<'_>> {
        self.rows.get(index).map(|row| self.laid_out_row(row))
    }
}

/// One row of a [`History`], read through it. Reading its id, lane or changes copies
/// nothing; its content is what the list draws, copied out of the stores.
#[derive(Clone, Copy)]
pub struct HistoryRow<'h> {
    history: &'h History,
    index: usize,
    row: StoredRow,
}

impl std::fmt::Debug for HistoryRow<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryRow")
            .field("index", &self.index)
            .field("id", &self.row.id)
            .finish_non_exhaustive()
    }
}

impl<'h> HistoryRow<'h> {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn id(&self) -> RowId {
        RowId::Commit(self.row.id)
    }

    pub fn lane(&self) -> Lane {
        self.row.lane
    }

    /// The lane changes at this row.
    pub fn changes(&self) -> &'h [LaneChange] {
        self.history
            .changes
            .get(self.row.changes)
            .unwrap_or_default()
    }

    /// Whether this row keeps the lines crossing into it, so its edges derive from it alone.
    pub fn has_snapshot(&self) -> bool {
        self.row.snapshot != NO_SNAPSHOT
    }

    /// One more than the highest lane this row names: [`crate::GraphRow::lanes_named`]'s.
    pub fn lanes_named(&self) -> usize {
        let laid_out = self.history.laid_out_row(self.row);
        crate::graph::lanes_named(
            laid_out.lane,
            laid_out.changes,
            laid_out.snapshot.and_then(SnapshotView::highest_lane),
        )
    }

    /// The edges this row crosses, derived from the nearest snapshot at or above it
    /// ([`row_edges`]).
    pub fn edges(&self) -> Option<RowEdges> {
        row_edges(self.history, self.index)
    }

    /// What the row draws, its text copied out of the history's stores.
    pub fn content(&self) -> RowContent {
        RowContent::Commit(CommitSummary {
            id: self.row.id,
            parent_count: usize::from(self.row.parents),
            summary: self.history.text_of(self.row.subject).to_owned(),
            author_name: self.history.author_name(self.row.author).to_owned(),
            author_time: self.row.author_time,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphRow, LaneAssigner, PagedCommit};

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    fn commit<'a>(subject: &'a str, author: &'a str, parents: usize) -> PagedCommit<'a> {
        PagedCommit {
            parents,
            subject,
            author,
            author_time: i64::from(parents as u8) * 100 - 7,
        }
    }

    fn summary(row: HistoryRow<'_>) -> CommitSummary {
        match row.content() {
            RowContent::Commit(commit) => commit,
            RowContent::NotACommit => unreachable!("a history holds commits"),
        }
    }

    /// The QA brief's two pages: one that brings a new author, one that brings only known
    /// ones. Both draw, and the table names each author once. Caught by: an author added
    /// again for every page that names them, or a row reading the page's number for its
    /// author rather than the history's.
    #[test]
    fn a_page_of_new_authors_and_a_page_of_known_ones_both_draw_and_each_is_named_once() {
        let mut history = History::new();
        let mut first = RowsPage::new();
        first.push(
            GraphRow::new(oid(1), Lane::new(0), Vec::new()),
            commit("one", "Ada", 1),
        );
        first.push(
            GraphRow::new(oid(2), Lane::new(0), Vec::new()),
            commit("two", "Grace", 2),
        );
        history.append(first).unwrap();
        assert_eq!(history.author_count(), 2);

        let mut known = RowsPage::new();
        // Grace first on this page, so the page's author numbers are the reverse of the
        // history's.
        known.push(
            GraphRow::new(oid(3), Lane::new(0), Vec::new()),
            commit("three", "Grace", 0),
        );
        known.push(
            GraphRow::new(oid(4), Lane::new(0), Vec::new()),
            commit("four", "Ada", 3),
        );
        history.append(known).unwrap();
        assert_eq!(history.author_count(), 2, "a known author was added again");

        let mut new = RowsPage::new();
        new.push(
            GraphRow::new(oid(5), Lane::new(0), Vec::new()),
            commit("five", "Ada", 1),
        );
        new.push(
            GraphRow::new(oid(6), Lane::new(0), Vec::new()),
            commit("six", "Margaret", 1),
        );
        history.append(new).unwrap();
        assert_eq!(history.author_count(), 3);

        let drawn: Vec<(String, String, usize, i64)> = history
            .rows()
            .map(|row| {
                let commit = summary(row);
                (
                    commit.summary,
                    commit.author_name,
                    commit.parent_count,
                    commit.author_time,
                )
            })
            .collect();
        let expected = [
            ("one", "Ada", 1, 93),
            ("two", "Grace", 2, 193),
            ("three", "Grace", 0, -7),
            ("four", "Ada", 3, 293),
            ("five", "Ada", 1, 93),
            ("six", "Margaret", 1, 93),
        ];
        let expected: Vec<(String, String, usize, i64)> = expected
            .iter()
            .map(|&(s, a, p, t)| (s.to_owned(), a.to_owned(), p, t))
            .collect();
        assert_eq!(drawn, expected);
    }

    /// Three names filed under one index key, through the path every author takes, each
    /// read back as itself. Caught by: trusting the key without the name, not walking the
    /// chain, and a new author filed under a key without chaining the one already there.
    #[test]
    fn authors_whose_keys_collide_are_told_apart_by_name() {
        let mut history = History::new();
        let key = 7;
        let ada = history.author_under("Ada", key).unwrap();
        let grace = history.author_under("Grace", key).unwrap();
        let margaret = history.author_under("Margaret", key).unwrap();
        assert_eq!(history.author_count(), 3);
        assert_eq!([ada, grace, margaret], [0, 1, 2]);

        for (name, number) in [("Margaret", margaret), ("Grace", grace), ("Ada", ada)] {
            assert_eq!(
                history.author_under(name, key).unwrap(),
                number,
                "{name}, filed under a shared key, was not found as themself"
            );
            assert_eq!(history.author_name(number), name);
        }
        assert_eq!(history.author_count(), 3, "a filed author was added again");
        assert_eq!(
            history.author_of("Ada").unwrap(),
            3,
            "Ada's own key names nobody yet"
        );
    }

    /// The partial-page contract at a history's limit, reached by naming every chunk the
    /// text store's addresses can: a page appends its rows until one cannot be held, says
    /// so, and keeps the rows before it. Caught by: a row pushed before its text was held,
    /// or the error dropped.
    #[test]
    fn a_page_past_the_historys_limit_keeps_the_rows_before_it_and_says_so() {
        let mut history = History::new();
        let mut first = RowsPage::new();
        first.push(
            GraphRow::new(oid(1), Lane::new(0), Vec::new()),
            commit("s", "a", 1),
        );
        history.append(first).unwrap();

        history.text.fill_every_address();
        let mut page = RowsPage::new();
        // An empty subject by a known author needs no new text; the next row's does.
        page.push(
            GraphRow::new(oid(2), Lane::new(0), Vec::new()),
            commit("", "a", 1),
        );
        page.push(
            GraphRow::new(oid(3), Lane::new(0), Vec::new()),
            commit("x", "a", 1),
        );
        page.push(
            GraphRow::new(oid(4), Lane::new(0), Vec::new()),
            commit("", "a", 1),
        );
        let full = history.append(page).unwrap_err();
        assert_eq!(full, HistoryFull);
        assert_eq!(full.to_string(), "the history is too large to hold");
        assert_eq!(
            history.len(),
            2,
            "the rows before the one that could not be held"
        );
        assert_eq!(history.id(1), Some(RowId::Commit(oid(2))));
        assert_eq!(summary(history.row(1).unwrap()).author_name, "a");

        let mut stranger = RowsPage::new();
        stranger.push(
            GraphRow::new(oid(5), Lane::new(0), Vec::new()),
            commit("", "b", 1),
        );
        assert_eq!(
            history.append(stranger),
            Err(HistoryFull),
            "a new author's name could not be held either"
        );
        assert_eq!(history.len(), 2);
    }

    /// The QA brief's subjects: empty, past ASCII and longer than a chunk of the text store.
    #[test]
    fn an_empty_a_non_ascii_and_a_very_long_subject_read_back_exactly() {
        let long: String = (0..20_000)
            .map(|n| char::from(b'a' + (n % 26) as u8))
            .collect();
        let long = format!("{long}{long}{long}{long}");
        assert!(long.len() > 1 << 16, "the subject fits a chunk");
        let subjects = ["", "Café crème \u{fffd} — 漢字", long.as_str(), "after"];
        let mut page = RowsPage::new();
        for (n, subject) in subjects.iter().enumerate() {
            page.push(
                GraphRow::new(oid(n as u8 + 1), Lane::new(0), Vec::new()),
                commit(subject, "Zoë Ångström", 1),
            );
        }
        let mut history = History::new();
        history.append(page).unwrap();
        let read: Vec<String> = history.rows().map(|row| summary(row).summary).collect();
        assert_eq!(read, subjects);
        assert!(
            history
                .rows()
                .all(|row| summary(row).author_name == "Zoë Ångström")
        );
    }

    /// A parent count past what a row keeps saturates, still a merge.
    #[test]
    fn a_parent_count_past_sixteen_bits_still_draws_a_merge() {
        let mut page = RowsPage::new();
        page.push(
            GraphRow::new(oid(1), Lane::new(0), Vec::new()),
            commit("o", "A", 70_000),
        );
        let mut history = History::new();
        history.append(page).unwrap();
        let commit = summary(history.row(0).unwrap());
        assert_eq!(commit.parent_count, usize::from(u16::MAX));
        assert!(commit.parent_count > 1);
    }

    /// Caught by: a history whose rows lost their lane changes or snapshots on the way into
    /// the stores, or kept another row's.
    #[test]
    fn a_history_draws_what_the_rows_it_was_given_draw() {
        let walk: Vec<(Oid, Vec<Oid>)> = (0..200u8)
            .map(|n| {
                let parents = match n {
                    199 => Vec::new(),
                    n if n % 7 == 0 => vec![oid(n + 1), oid(n.saturating_add(9).min(199))],
                    n => vec![oid(n + 1)],
                };
                (oid(n), parents)
            })
            .collect();
        let graphs = LaneAssigner::with_window(8)
            .with_snapshot_every(16)
            .assign_each(walk);
        let mut history = History::new();
        for chunk in graphs.chunks(13) {
            let mut page = RowsPage::new();
            for graph in chunk {
                page.push(graph.clone(), commit("s", "a", 1));
            }
            history.append(page).unwrap();
        }
        assert_eq!(history.len(), graphs.len());
        for (index, graph) in graphs.iter().enumerate() {
            let row = history.row(index).unwrap();
            assert_eq!(row.id(), RowId::Commit(graph.id));
            assert_eq!(row.lane(), graph.lane);
            assert_eq!(row.changes(), graph.changes());
            assert_eq!(row.has_snapshot(), graph.has_snapshot());
            assert_eq!(row.lanes_named(), graph.lanes_named(), "row {index}");
            assert_eq!(row.edges(), row_edges(&graphs, index), "row {index}");
            assert!(row.edges().is_some(), "row {index} drew nothing");
        }
    }

    #[test]
    fn a_row_is_found_by_its_identity_and_nothing_else_is() {
        let mut page = RowsPage::new();
        for n in 1..=5 {
            page.push(
                GraphRow::new(oid(n), Lane::new(0), Vec::new()),
                commit("s", "a", 1),
            );
        }
        let mut history = History::new();
        history.append(page).unwrap();
        assert_eq!(history.position(RowId::Commit(oid(4))), Some(3));
        assert_eq!(history.position(RowId::Commit(oid(9))), None);
        assert_eq!(history.position(RowId::NotACommit), None);
        assert_eq!(history.id(3), Some(RowId::Commit(oid(4))));
        assert_eq!(history.id(5), None);
        assert!(history.row(5).is_none());
        assert_eq!(History::new().position(RowId::Commit(oid(1))), None);
    }

    /// Caught by: counting what the stores hold by length, so a chunk's unused room is not
    /// counted, or any store — a snapshot's open lanes and late lines, the author index —
    /// left out of the total.
    #[test]
    fn what_a_history_retains_counts_every_chunk_whole() {
        use crate::edge_derivation::LaneState;

        let mut history = History::new();
        assert_eq!(
            history.retained().total(),
            0,
            "an empty history holds nothing"
        );
        // A snapshot with an open lane and a late line, so both of its stores hold a run.
        let mut state = LaneState::default();
        state.advance(&[
            LaneChange::Starts(Lane::new(3)),
            LaneChange::StartsLate {
                lane: Lane::new(5),
                rows: 4,
                order: 0,
            },
        ]);
        let graph = GraphRow::laid_out(
            oid(1),
            Lane::new(3),
            vec![
                LaneChange::Ends(Lane::new(3)),
                LaneChange::Starts(Lane::new(3)),
            ]
            .into_boxed_slice(),
            Some(Box::new(state.snapshot())),
        );
        let mut page = RowsPage::new();
        page.push(graph, commit("s", "a", 1));
        history.append(page).unwrap();
        let retained = history.retained();
        assert_eq!(
            retained.rows,
            (1 << 10) * size_of::<StoredRow>() + size_of::<Vec<StoredRow>>()
        );
        assert_eq!(retained.text, (1 << 16) + size_of::<String>());
        assert_eq!(
            retained.lane_changes,
            (1 << 12) * size_of::<LaneChange>() + size_of::<Vec<LaneChange>>()
        );
        assert_eq!(
            retained.snapshots,
            (1 << 8) * size_of::<StoredSnapshot>()
                + size_of::<Vec<StoredSnapshot>>()
                + (1 << 12) * size_of::<u64>()
                + size_of::<Vec<u64>>()
                + (1 << 8) * size_of::<LateLine>()
                + size_of::<Vec<LateLine>>()
        );
        let index = index_bytes(history.author_index.capacity());
        assert!(index > 0, "one author filed and no index counted");
        assert_eq!(
            retained.authors,
            (1 << 10) * size_of::<StoredAuthor>() + size_of::<Vec<StoredAuthor>>() + index
        );
        assert_eq!(
            retained.total(),
            retained.rows
                + retained.text
                + retained.lane_changes
                + retained.snapshots
                + retained.authors
        );
    }

    /// The estimate follows hashbrown's sizing: three entries in four buckets, seven in
    /// eight, fourteen in sixteen.
    #[test]
    fn the_author_index_is_estimated_by_its_buckets() {
        let bucket = size_of::<(u64, u32)>() + 1;
        assert_eq!(index_bytes(0), 0);
        assert_eq!(index_bytes(3), 4 * bucket + 16);
        assert_eq!(index_bytes(7), 8 * bucket + 16);
        assert_eq!(index_bytes(14), 16 * bucket + 16);
        assert_eq!(index_bytes(57_344), 65_536 * bucket + 16);
    }

    /// The slim row's size, which C16's figure is made of. Caught by: a field added to it.
    #[test]
    fn a_kept_row_is_seventy_two_bytes() {
        assert_eq!(size_of::<StoredRow>(), 72);
    }

    /// Pins a shape: stops compiling if `RowContent` becomes a bare commit field.
    #[test]
    fn a_consumer_reads_a_row_by_matching_on_its_content() {
        let mut page = RowsPage::new();
        page.push(
            GraphRow::new(oid(1), Lane::new(0), Vec::new()),
            commit("first", "a", 1),
        );
        let mut history = History::new();
        history.append(page).unwrap();
        let contents = [history.row(0).unwrap().content(), RowContent::NotACommit];
        let drawn: Vec<String> = contents
            .into_iter()
            .map(|content| match content {
                RowContent::Commit(commit) => commit.summary,
                RowContent::NotACommit => "not a commit".to_owned(),
            })
            .collect();
        assert_eq!(drawn, vec!["first".to_owned(), "not a commit".to_owned()]);
    }
}
