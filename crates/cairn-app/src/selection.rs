//! What the detail pane describes: the row chosen in the history, or a parent reached from
//! the Commit tab. On the UI thread; asking is a [`Request`] handed to the caller's submit,
//! never a wait.

use cairn_model::{HistoryRow, Oid, RowId};
use freya::prelude::*;

use crate::diff_state::Answer;
use crate::window::View;
use crate::worker::{Comparison, Request};

/// What a row's changes are asked against. Named variant by variant: a row that is not a
/// commit — the working tree's, when it lands — must say here what it compares, or the
/// window does not compile.
pub fn comparison_of(id: RowId) -> Comparison {
    match id {
        RowId::Commit(oid) => Comparison::Commit(oid),
    }
}

/// Selects `id` and asks what it changed — through [`crate::diff_state::DiffState`], so an
/// answer for any other selection is never kept. Choosing what is already chosen asks
/// nothing new, unless its answer failed, when choosing it again is how to retry.
pub fn choose(id: RowId, view: View, submit: Option<&dyn Fn(Request)>) {
    let View {
        mut selected,
        mut diff,
        ..
    } = view;
    selected.set(Some(id));
    let of = comparison_of(id);
    let asked = diff
        .peek()
        .changes()
        .is_some_and(|(asked, answer)| asked == of && !matches!(answer, Answer::Failed(_)));
    if asked {
        return;
    }
    // The query, then the answers it replaces, which are freed on a worker.
    let requests = diff.write().select_changes(of);
    if let Some(submit) = submit {
        for request in requests {
            submit(request);
        }
    }
}

/// Where `parent` is in the loaded history, if it is loaded. A scan of what is loaded, run
/// once per press of a parent link and never per frame.
pub fn loaded_row(rows: &[HistoryRow], parent: Oid) -> Option<usize> {
    let wanted = RowId::Commit(parent);
    rows.iter().position(|row| row.id() == wanted)
}

#[cfg(test)]
mod tests {
    use cairn_model::{CommitSummary, EdgeSegment, GraphRow, Lane, RowContent};

    use super::*;

    fn row(n: u8) -> HistoryRow {
        let id = Oid::from_bytes(&[n; 20]).unwrap();
        HistoryRow {
            content: RowContent::Commit(CommitSummary {
                id,
                parents: Vec::new(),
                summary: format!("commit {n}"),
                author_name: "Ada".to_owned(),
                author_email: "ada@example.com".to_owned(),
                author_time: 0,
            }),
            graph: GraphRow {
                id,
                lane: Lane::new(0),
                edges: vec![EdgeSegment::passing(Lane::new(0))],
            },
        }
    }

    #[test]
    fn a_parent_is_found_where_it_is_loaded_and_nowhere_else() {
        let rows: Vec<HistoryRow> = (1..=5).map(row).collect();
        assert_eq!(
            loaded_row(&rows, Oid::from_bytes(&[4; 20]).unwrap()),
            Some(3)
        );
        assert_eq!(loaded_row(&rows, Oid::from_bytes(&[9; 20]).unwrap()), None);
        assert_eq!(loaded_row(&[], Oid::from_bytes(&[1; 20]).unwrap()), None);
    }
}
