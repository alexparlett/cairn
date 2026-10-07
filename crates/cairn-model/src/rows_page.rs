//! A page of history rows as the worker hands it over: its rows, its own text and the
//! authors it names, its lane changes and its snapshots, its rows' labels and its stashes —
//! flat vectors, so a page is a handful of allocations whatever its length, and appending
//! it to a [`crate::History`] copies it into the history's stores (PRD R4.3, R4.7).

use crate::{GraphRow, Label, Lane, LaneChange, LaneSnapshot, Oid, RefKind};

/// What a page row says about its commit beyond its lane: what the list draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagedCommit<'a> {
    /// How many parents the commit names. Only whether it has more than one is drawn (a
    /// merge's ring); its parents themselves are the details query's.
    pub parents: usize,
    pub subject: &'a str,
    pub author: &'a str,
    /// Seconds since the Unix epoch.
    pub author_time: i64,
}

/// What a stash's row says beyond its lane (PRD R4.2): the stash commit is the row's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagedStash<'a> {
    /// `0` is the newest: `stash@{0}`.
    pub index: usize,
    /// The commit the stash was made on: its first parent.
    pub base: Oid,
    /// The stash list's message, the row's subject.
    pub message: &'a str,
    pub author: &'a str,
    /// Seconds since the Unix epoch.
    pub author_time: i64,
}

/// A label of a page row, its name a range into the page's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageLabel {
    pub(crate) name: (usize, usize),
    pub(crate) kind: RefKind,
    pub(crate) current: bool,
}

/// What a page row of a stash keeps beyond a commit's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageStash {
    pub(crate) index: usize,
    pub(crate) base: Oid,
}

/// One row of a page: its text and changes as ranges into the page's own vectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageRow {
    pub(crate) id: Oid,
    pub(crate) parents: usize,
    pub(crate) subject: (usize, usize),
    /// Into the page's `authors`.
    pub(crate) author: usize,
    pub(crate) author_time: i64,
    pub(crate) lane: Lane,
    pub(crate) changes: (usize, usize),
    /// Into the page's `snapshots`.
    pub(crate) snapshot: Option<usize>,
    /// Into the page's `labels`.
    pub(crate) labels: (usize, usize),
    /// The row's commit is `HEAD`'s.
    pub(crate) head: bool,
    /// The row is a stash's, not a commit's.
    pub(crate) stash: Option<PageStash>,
}

/// Rows laid out and read for one page, in walk order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowsPage {
    pub(crate) rows: Vec<PageRow>,
    /// Every subject and author name of the page, end to end.
    pub(crate) text: String,
    /// The page's authors, each named once, as ranges into `text`.
    pub(crate) authors: Vec<(usize, usize)>,
    pub(crate) changes: Vec<LaneChange>,
    pub(crate) snapshots: Vec<LaneSnapshot>,
    pub(crate) labels: Vec<PageLabel>,
}

impl RowsPage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends the row `graph` lays out, for the commit `commit` describes, which no ref
    /// points at and which is not `HEAD`'s.
    pub fn push(&mut self, graph: GraphRow, commit: PagedCommit<'_>) {
        self.push_labelled(graph, commit, false, &[]);
    }

    /// Appends the row `graph` lays out, for the commit `commit` describes, carrying the
    /// refs that point at it and whether it is `HEAD`'s (PRD R4.3). The labels are kept
    /// bytewise by full name — a snapshot's own order, which a walk hands them in — so that
    /// [`crate::RowLabels::find`] can search them; any other order is sorted into that one.
    pub fn push_labelled(
        &mut self,
        graph: GraphRow,
        commit: PagedCommit<'_>,
        head: bool,
        labels: &[Label<'_>],
    ) {
        let first_label = self.labels.len();
        let sorted;
        let labels = if labels.is_sorted_by_key(|label| label.name.as_bytes()) {
            labels
        } else {
            let mut owned = labels.to_vec();
            owned.sort_by_key(|label| label.name.as_bytes());
            sorted = owned;
            &sorted[..]
        };
        for label in labels {
            let name = self.text_of(label.name);
            self.labels.push(PageLabel {
                name,
                kind: label.kind,
                current: label.current,
            });
        }
        let labels = (first_label, self.labels.len());
        self.push_row(graph, commit, labels, head, None);
    }

    /// Appends the row `graph` lays out for a stash (PRD R4.2): its id is the stash commit,
    /// its subject the stash's message.
    pub fn push_stash(&mut self, graph: GraphRow, stash: PagedStash<'_>) {
        let commit = PagedCommit {
            parents: 1,
            subject: stash.message,
            author: stash.author,
            author_time: stash.author_time,
        };
        let at = self.labels.len();
        let kept = PageStash {
            index: stash.index,
            base: stash.base,
        };
        self.push_row(graph, commit, (at, at), false, Some(kept));
    }

    fn push_row(
        &mut self,
        graph: GraphRow,
        commit: PagedCommit<'_>,
        labels: (usize, usize),
        head: bool,
        stash: Option<PageStash>,
    ) {
        let (id, lane, changes, snapshot) = graph.into_parts();
        let subject = self.text_of(commit.subject);
        let author = self.author_of(commit.author);
        let first_change = self.changes.len();
        self.changes.extend_from_slice(&changes);
        let snapshot = snapshot.map(|snapshot| {
            self.snapshots.push(*snapshot);
            self.snapshots.len() - 1
        });
        self.rows.push(PageRow {
            id,
            parents: commit.parents,
            subject,
            author,
            author_time: commit.author_time,
            lane,
            changes: (first_change, self.changes.len()),
            snapshot,
            labels,
            head,
            stash,
        });
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The commit of each row, in order: a stash's row names the stash commit.
    pub fn ids(&self) -> impl Iterator<Item = Oid> + '_ {
        self.rows.iter().map(|row| row.id)
    }

    /// The widest the page's rows draw: one more than the highest lane any of them names —
    /// its node, its changes and, on a row with a snapshot, every line crossing into it.
    /// Over every page from the first, every lane a line crosses is named. One for a page
    /// with no rows: the graph column is never narrower than a lane.
    pub fn lanes_named(&self) -> usize {
        self.rows
            .iter()
            .map(|row| {
                let changes = self
                    .changes
                    .get(row.changes.0..row.changes.1)
                    .unwrap_or_default();
                let crossing = row
                    .snapshot
                    .and_then(|at| self.snapshots.get(at))
                    .and_then(LaneSnapshot::highest_lane);
                crate::graph::lanes_named(row.lane, changes, crossing)
            })
            .max()
            .unwrap_or(1)
    }

    pub(crate) fn text(&self, range: (usize, usize)) -> &str {
        self.text.get(range.0..range.1).unwrap_or_default()
    }

    fn text_of(&mut self, text: &str) -> (usize, usize) {
        let start = self.text.len();
        self.text.push_str(text);
        (start, self.text.len())
    }

    /// The page's number for the author `name`, naming each author once: a page is a
    /// screenful of rows, so a scan of the authors it already names is short.
    fn author_of(&mut self, name: &str) -> usize {
        if let Some(known) = self
            .authors
            .iter()
            .position(|&range| self.text.get(range.0..range.1) == Some(name))
        {
            return known;
        }
        let range = self.text_of(name);
        self.authors.push(range);
        self.authors.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(n: u8) -> GraphRow {
        GraphRow::new(Oid::from_bytes(&[n; 20]).unwrap(), Lane::new(0), Vec::new())
    }

    fn commit<'a>(subject: &'a str, author: &'a str) -> PagedCommit<'a> {
        PagedCommit {
            parents: 1,
            subject,
            author,
            author_time: 0,
        }
    }

    /// Caught by: an author named again for every row that names them.
    #[test]
    fn a_page_names_each_of_its_authors_once() {
        let mut page = RowsPage::new();
        page.push(graph(1), commit("one", "Ada"));
        page.push(graph(2), commit("two", "Grace"));
        page.push(graph(3), commit("three", "Ada"));
        assert_eq!(page.authors.len(), 2);
        let names: Vec<&str> = page
            .rows
            .iter()
            .map(|row| page.text(page.authors[row.author]))
            .collect();
        assert_eq!(names, ["Ada", "Grace", "Ada"]);
        assert_eq!(page.text(page.rows[2].subject), "three");
        assert_eq!(page.len(), 3);
        assert_eq!(page.ids().count(), 3);
    }

    #[test]
    fn an_empty_page_still_names_one_lane() {
        assert_eq!(RowsPage::new().lanes_named(), 1);
    }
}
