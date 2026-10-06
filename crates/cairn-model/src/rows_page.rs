//! A page of history rows as the worker hands it over: its rows, its own text and the
//! authors it names, its lane changes and its snapshots — flat vectors, so a page is a
//! handful of allocations whatever its length, and appending it to a [`crate::History`]
//! copies it into the history's stores (PRD R4.7).

use crate::{GraphRow, Lane, LaneChange, LaneSnapshot, Oid};

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
}

impl RowsPage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends the row `graph` lays out, for the commit `commit` describes.
    pub fn push(&mut self, graph: GraphRow, commit: PagedCommit<'_>) {
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
        });
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The commit of each row, in order.
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
