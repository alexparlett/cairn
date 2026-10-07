//! Where a row sits in the history, looked for once per history and row and never again:
//! what the Commit tab's REFS row reads the selected row's chips from (refs-and-status R6.1).
//!
//! Rows only append to one history, and a reopen builds another (`History::serial`), so a row
//! found stays where it was found, and a row not found yet is looked for only among the rows
//! appended since the last look: the selected row's place costs one pass over the loaded rows
//! per history and selection, spread over the pages as they arrive, never a pass per frame.

use cairn_model::{History, RowId};

#[derive(Debug, Default)]
pub struct RowFinder {
    /// The history and the row the rest describe.
    looked_for: Option<(u64, RowId)>,
    found: Option<usize>,
    /// Rows `0..scanned` of that history do not hold the row.
    scanned: usize,
}

impl RowFinder {
    /// Where `id` sits in `rows`, if it is loaded.
    pub fn find(&mut self, rows: &History, id: RowId) -> Option<usize> {
        let asked = (rows.serial(), id);
        if self.looked_for != Some(asked) {
            *self = Self {
                looked_for: Some(asked),
                found: None,
                scanned: 0,
            };
        }
        if self.found.is_some() {
            return self.found;
        }
        let found = (self.scanned..rows.len()).find(|&at| rows.id(at) == Some(id));
        self.scanned = rows.len();
        self.found = found;
        found
    }

    /// How many rows the last look read: a test's measure of the work.
    #[cfg(test)]
    fn scanned(&self) -> usize {
        self.scanned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{GraphRow, Lane, Oid, PagedCommit, RowsPage};

    fn oid(n: usize) -> Oid {
        let mut bytes = [0u8; 20];
        bytes[12..20].copy_from_slice(&(n as u64).to_be_bytes());
        Oid::from_bytes(&bytes).unwrap()
    }

    fn page(range: std::ops::Range<usize>) -> RowsPage {
        let mut page = RowsPage::new();
        for n in range {
            page.push(
                GraphRow::new(oid(n), Lane::new(0), Vec::new()),
                PagedCommit {
                    parents: 1,
                    subject: "s",
                    author: "A",
                    author_time: 0,
                },
            );
        }
        page
    }

    /// A row found is found again without a look; one not loaded yet is looked for only among
    /// the rows that arrived since; another history is looked through afresh. Caught by:
    /// trusting a place found in a history since replaced, rescanning every loaded row on
    /// every look, or never looking past the rows of the first look.
    #[test]
    fn a_row_is_looked_for_once_per_history_and_found_where_it_arrives() {
        let mut finder = RowFinder::default();
        let mut history = History::new();
        history.append(page(0..100)).unwrap();
        let wanted = RowId::Commit(oid(150));
        assert_eq!(finder.find(&history, wanted), None);
        assert_eq!(finder.scanned(), 100);

        history.append(page(100..200)).unwrap();
        assert_eq!(finder.find(&history, wanted), Some(150));
        assert_eq!(finder.find(&history, wanted), Some(150));

        // A reopen: the same row now sits elsewhere, among rows the old history had scanned.
        let mut reopened = History::new();
        reopened.append(page(140..160)).unwrap();
        assert_eq!(finder.find(&reopened, wanted), Some(10));
        assert_eq!(finder.find(&reopened, RowId::Commit(oid(141))), Some(1));
        assert_eq!(finder.find(&reopened, RowId::Stash(oid(141))), None);
    }
}
