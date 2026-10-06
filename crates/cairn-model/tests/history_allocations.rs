//! C16's third half, counted: no kept row owns a heap allocation. A row is `Copy` (the
//! compile-time half, in `history.rs`); here the allocator says so too — appending ten
//! thousand rows, labelled and stashes among them, allocates per chunk of the history's
//! stores, never per row, and reading a row's identity, lane, lane changes and labels
//! allocates nothing at all.

use cairn_model::{
    GraphRow, History, Label, LaneAssigner, Oid, PagedCommit, PagedStash, RefKind, RowsPage,
};

fn oid(n: usize) -> Oid {
    let mut bytes = [0u8; 20];
    bytes[12..20].copy_from_slice(&(n as u64).to_be_bytes());
    Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
}

/// A braided walk: every row a lane change or two, merges every fifth row, so the lane
/// change and snapshot stores fill as well as the row and text stores.
fn graphs(len: usize) -> Vec<GraphRow> {
    let walk = (0..len).map(|n| {
        let parents = match n {
            n if n + 1 == len => Vec::new(),
            n if n % 5 == 0 && n + 3 < len => vec![oid(n + 1), oid(n + 3)],
            n => vec![oid(n + 1)],
        };
        (oid(n), parents)
    });
    LaneAssigner::new().assign_each(walk)
}

fn page(graphs: &[GraphRow], subjects: &[String]) -> RowsPage {
    let authors = ["Ada Lovelace", "Grace Hopper", "Margaret Hamilton"];
    let labels = [
        Label {
            name: "refs/heads/topic",
            kind: RefKind::LocalBranch,
            current: false,
        },
        Label {
            name: "refs/tags/v1",
            kind: RefKind::Tag,
            current: false,
        },
    ];
    let mut page = RowsPage::new();
    for (n, graph) in graphs.iter().enumerate() {
        let commit = PagedCommit {
            parents: 1 + n % 2,
            subject: &subjects[n],
            author: authors[n % authors.len()],
            author_time: n as i64,
        };
        // A stash every thirteenth row and two labels every seventh, as rows of a walk
        // from every ref carry them.
        if n % 13 == 12 {
            let stash = PagedStash {
                index: n,
                base: graph.id,
                message: commit.subject,
                author: commit.author,
                author_time: commit.author_time,
            };
            page.push_stash(graph.clone(), stash);
        } else {
            let labelled = if n % 7 == 0 { &labels[..] } else { &[] };
            page.push_labelled(graph.clone(), commit, n % 50 == 0, labelled);
        }
    }
    page
}

/// Caught by: a row that boxes its subject, its author, its lane changes or its labels
/// again, or a store that grows by reallocating as it fills rather than by adding a chunk.
#[test]
fn appending_ten_thousand_rows_allocates_per_chunk_never_per_row() {
    let rows = 10_000;
    let graphs = graphs(rows);
    let subjects: Vec<String> = (0..rows)
        .map(|n| format!("commit number {n} of the walk"))
        .collect();
    let first = page(&graphs[..64], &subjects[..64]);
    let rest = page(&graphs[64..], &subjects[64..]);

    let mut history = History::new();
    history.append(first).unwrap();
    let appended = allocation_counter::measure(|| {
        history.append(rest).unwrap();
    });
    assert_eq!(history.len(), rows);
    assert_eq!(history.author_count(), 3);
    // Ten thousand rows fill about ten chunks of rows, three or four of text, three of lane
    // changes and one or two of each snapshot store; each new chunk is one allocation, and
    // the list of chunks moves once as it grows by one.
    assert!(
        appended.count_total < 100,
        "appending {} rows allocated {} times; a kept row allocates nothing of its own",
        rows - 64,
        appended.count_total,
    );
}

/// Caught by: reading a row through anything that copies its stores.
#[test]
fn reading_a_rows_identity_lane_and_changes_allocates_nothing() {
    let rows = 3_000;
    let graphs = graphs(rows);
    let subjects: Vec<String> = (0..rows).map(|n| format!("subject {n}")).collect();
    let mut history = History::new();
    for (graphs, subjects) in graphs.chunks(64).zip(subjects.chunks(64)) {
        history.append(page(graphs, subjects)).unwrap();
    }
    let mut changes = 0usize;
    let mut labels = 0usize;
    for index in [0, 1, 63, 64, 1_500, rows - 1] {
        let read = allocation_counter::measure(|| {
            let row = history.row(index).unwrap();
            assert!(history.id(index).is_some());
            assert_eq!(row.lane(), graphs[index].lane);
            changes += row.changes().len();
            labels += row.labels().iter().count();
        });
        assert_eq!(
            read.count_total, 0,
            "reading row {index} allocated {} times",
            read.count_total
        );
    }
    assert!(
        changes > 0,
        "no row read had a lane change, so nothing was read"
    );
    assert!(labels > 0, "no row read had a label, so none was read");
}
