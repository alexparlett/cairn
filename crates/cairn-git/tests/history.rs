//! Acceptance tests for the bounded history query (`docs/prd/history-graph.md`,
//! R2). A4 is `every_row_matches_what_git_reports` together with
//! `a_limit_bounds_the_page_and_a_cursor_continues_it`; A5 is
//! `a_cancelled_query_stops_walking_and_says_where`.
//!
//! Every expectation is read back out of the `git` binary. A fixture that only
//! proved gitoxide agrees with itself would decide nothing: the question is
//! whether Cairn shows what `git` would show.
//!
//! `unwrap` is unavailable here — `clippy.toml`'s carve-out only reaches
//! `#[cfg(test)]` code, and an integration test crate is not that.

mod fixtures;

use std::cell::Cell;
use std::collections::HashMap;

use cairn_git::{Cancel, CancelSignal, Error, HistoryPage, HistoryRequest, Repository};
use cairn_model::{EdgeSegment, HistoryRow};

use fixtures::Fixture;

fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

fn open(fixture: &Fixture) -> Repository {
    ok(Repository::discover(fixture.path()), "opening the fixture")
}

fn ids(page: &HistoryPage) -> Vec<String> {
    page.rows.iter().map(|row| row.id().to_string()).collect()
}

fn ids_of(rows: &[HistoryRow]) -> Vec<String> {
    rows.iter().map(|row| row.id().to_string()).collect()
}

fn lanes(rows: &[HistoryRow]) -> Vec<(String, usize)> {
    rows.iter()
        .map(|row| (row.id().to_string(), row.graph.lane.index()))
        .collect()
}

fn read(repo: &Repository, request: &HistoryRequest) -> HistoryPage {
    ok(
        repo.history(request, &CancelSignal::new()),
        "reading history",
    )
}

/// A4. Same commits, same order, same parents, same text as `git`.
///
/// The fixture's committer dates rise along every parent link, so `git`'s
/// reverse-chronological order and a pure commit-time walk are the same
/// sequence — which is what makes comparing them a real check rather than a
/// disagreement about what "newest first" means.
#[test]
fn every_row_matches_what_git_reports() {
    let fixture = fixtures::braided(30);
    let repo = open(&fixture);
    let expected = fixture.rev_list(&[]);
    assert!(
        expected.len() > 20,
        "the fixture is too small to decide much"
    );

    let page = read(&repo, &HistoryRequest::from_head(expected.len() + 10));
    assert_eq!(ids(&page), expected, "the walk diverged from git's");
    assert!(
        page.cursor.is_none(),
        "the whole history left a cursor behind"
    );
    assert_eq!(
        page.decoded,
        page.rows.len(),
        "a commit was read for nothing"
    );

    let mut parents_of: HashMap<String, Vec<String>> = HashMap::new();
    for line in fixture.git(&["rev-list", "--parents", "HEAD"]).lines() {
        let mut names = line.split_whitespace().map(str::to_owned);
        let Some(child) = names.next() else { continue };
        parents_of.insert(child, names.collect());
    }

    let mut text_of: HashMap<String, Vec<String>> = HashMap::new();
    let format = "--format=%H%x1f%s%x1f%an%x1f%ae%x1f%at";
    for line in fixture.git(&["log", format, "HEAD"]).lines() {
        let fields: Vec<String> = line.split('\u{1f}').map(str::to_owned).collect();
        if fields.len() == 5 {
            text_of.insert(fields[0].clone(), fields);
        }
    }

    for row in &page.rows {
        let id = row.id().to_string();
        let mine: Vec<String> = row.commit.parents.iter().map(ToString::to_string).collect();
        assert_eq!(Some(&mine), parents_of.get(&id), "parents of {id}");

        let expected_text = match text_of.get(&id) {
            Some(fields) => fields,
            None => panic!("git did not report {id} at all"),
        };
        assert_eq!(row.commit.summary, expected_text[1], "summary of {id}");
        assert_eq!(row.commit.author_name, expected_text[2], "author of {id}");
        assert_eq!(row.commit.author_email, expected_text[3], "email of {id}");
        assert_eq!(
            row.commit.author_time.to_string(),
            expected_text[4],
            "author time of {id}"
        );
        assert_eq!(
            row.id(),
            &row.graph.id,
            "the two halves named different commits"
        );
    }
}

/// A4's second half: the limit is a limit, and the cursor gets you the rest.
#[test]
fn a_limit_bounds_the_page_and_a_cursor_continues_it() {
    let fixture = fixtures::braided(30);
    let repo = open(&fixture);
    let expected = fixture.rev_list(&[]);

    let page = read(&repo, &HistoryRequest::from_head(5));
    assert_eq!(page.rows.len(), 5, "the limit was not honoured");
    assert_eq!(
        ids(&page),
        &expected[..5],
        "the first page is not the newest 5"
    );

    let mut seen = ids(&page);
    let mut cursor = page.cursor;
    let mut pages = 1;
    while let Some(next) = cursor {
        assert_eq!(next.rows_behind(), seen.len(), "the cursor lost its place");
        let page = read(&repo, &HistoryRequest::resume(next, 5));
        assert!(
            page.rows.len() <= 5,
            "the limit was not honoured on page {pages}"
        );
        assert!(
            !page.rows.is_empty(),
            "a cursor was handed out for an empty page"
        );
        seen.extend(ids(&page));
        cursor = page.cursor;
        pages += 1;
        assert!(pages < 100, "paging did not terminate");
    }
    assert_eq!(seen, expected, "paging did not reproduce the whole history");
}

/// The cursor's real test: splitting a page must not change the picture. Lane
/// indices are identical; an edge may differ only by the split run not yet
/// having seen the commit that repaints it, which is exactly R1.2's licence.
#[test]
fn two_pages_of_n_match_one_page_of_2n_including_lanes() {
    let fixture = fixtures::braided(40);
    let repo = open(&fixture);
    let n = 9;

    let whole = read(&repo, &HistoryRequest::from_head(n * 2));
    let first = read(&repo, &HistoryRequest::from_head(n));
    let cursor = match first.cursor.clone() {
        Some(cursor) => cursor,
        None => panic!("the first page ended the history; raise the fixture size"),
    };
    let second = read(&repo, &HistoryRequest::resume(cursor, n));

    let mut split = first.rows.clone();
    split.extend(second.rows.clone());
    assert_eq!(split.len(), whole.rows.len(), "a different number of rows");
    assert_eq!(
        lanes(&split),
        lanes(&whole.rows),
        "lanes moved when the page split"
    );

    for (index, (apart, together)) in split.iter().zip(&whole.rows).enumerate() {
        assert_eq!(
            apart.commit, together.commit,
            "row {index} is a different commit"
        );
        assert!(
            together.graph.edges.starts_with(&apart.graph.edges),
            "row {index} lost segments when the page split:\n  split {:?}\n  whole {:?}",
            apart.graph.edges,
            together.graph.edges
        );
        for gained in &together.graph.edges[apart.graph.edges.len()..] {
            assert!(
                gained.out_of_order,
                "row {index} gained {gained:?} unflagged, so the difference is not a repaint"
            );
        }
    }

    // The replay is what costs: the second page walks past everything the
    // first one covered, and reads not one of those commits.
    assert_eq!(
        second.walked,
        n * 2,
        "the second page did not replay the first"
    );
    assert_eq!(
        second.decoded,
        second.rows.len(),
        "the replayed prefix was decoded"
    );
    assert_eq!(first.walked, n, "the first page walked past its limit");
}

/// The case the braided fixture cannot produce: a backward line whose two ends
/// fall in different pages. The row that the line *leaves* is finished and
/// returned before the row it arrives at has been read, so that segment is
/// missing from the split run and present in the whole one — a gained segment,
/// flagged, which is precisely what R1.2 allows and what phase 04 must expect
/// when it re-renders a row it already has.
#[test]
fn a_backward_line_across_a_page_boundary_is_a_gained_segment() {
    let fixture = fixtures::skewed();
    let repo = open(&fixture);
    let expected = fixture.rev_list(&[]);

    let whole = read(&repo, &HistoryRequest::from_head(expected.len()));
    let first = read(&repo, &HistoryRequest::from_head(3));
    let cursor = match first.cursor.clone() {
        Some(cursor) => cursor,
        None => panic!("three of five commits ended the history"),
    };
    let second = read(&repo, &HistoryRequest::resume(cursor, expected.len()));

    let mut split = first.rows.clone();
    split.extend(second.rows.clone());
    assert_eq!(
        ids_of(&split),
        ids_of(&whole.rows),
        "the split lost a commit"
    );
    assert_eq!(
        lanes(&split),
        lanes(&whole.rows),
        "lanes moved when the page split"
    );

    assert!(
        repaints(&whole.rows) > 0,
        "the fixture delivered no parent early, so this decides nothing"
    );
    assert!(
        repaints(&split) < repaints(&whole.rows),
        "the boundary-crossing line was expected to be missing from the split run"
    );
    for (index, (apart, together)) in split.iter().zip(&whole.rows).enumerate() {
        assert!(
            together.graph.edges.starts_with(&apart.graph.edges),
            "row {index} differs by more than a gained segment"
        );
        for gained in &together.graph.edges[apart.graph.edges.len()..] {
            assert!(
                gained.out_of_order,
                "row {index} gained {gained:?} unflagged"
            );
        }
    }
}

/// Counts how often it is asked, and says stop once it has been asked enough.
///
/// A cancellation test built on a background thread would pass or fail on
/// timing. This one makes the walk stop at a chosen commit, so the assertion
/// is about where it stopped rather than about whether the call returned.
#[derive(Debug, Default)]
struct StopAfter {
    limit: usize,
    polls: Cell<usize>,
}

impl Cancel for StopAfter {
    fn is_cancelled(&self) -> bool {
        let polls = self.polls.get() + 1;
        self.polls.set(polls);
        polls > self.limit
    }
}

/// A5. A cancelled query stops walking — observable, because the walk is asked
/// for a fixed number of commits and reports how many it laid out.
#[test]
fn a_cancelled_query_stops_walking_and_says_where() {
    let fixture = fixtures::braided(40);
    let repo = open(&fixture);
    let total = fixture.rev_list(&[]).len();
    let stop_at = 6;
    assert!(
        total > stop_at * 3,
        "the fixture must outlast the cancellation"
    );

    // What the same request does when nothing cancels it: the control that
    // makes "it stopped early" mean something.
    let uncancelled = read(&repo, &HistoryRequest::from_head(total));
    assert_eq!(uncancelled.rows.len(), total);

    let signal = StopAfter {
        limit: stop_at,
        polls: Cell::new(0),
    };
    let error = repo
        .history(&HistoryRequest::from_head(total), &signal)
        .err();
    match error {
        Some(Error::Cancelled { walked }) => assert_eq!(
            walked, stop_at,
            "the walk carried on for {walked} commits after being told to stop"
        ),
        other => panic!("expected a cancellation, got {other:?}"),
    }
    assert_eq!(
        signal.polls.get(),
        stop_at + 1,
        "the walk kept polling after it had been cancelled"
    );

    // And a signal already set before the call walks nothing at all.
    let already = CancelSignal::new();
    already.cancel();
    match repo.history(&HistoryRequest::from_head(total), &already) {
        Err(Error::Cancelled { walked }) => assert_eq!(walked, 0, "walked before checking"),
        other => panic!("expected a cancellation, got {other:?}"),
    }
}

/// The window makes rows final; it must never make them disappear. A window of
/// one keeps a single row, so every row is handed over the moment the next one
/// arrives — and the page still has all of them, in order, with the same lanes.
#[test]
fn a_narrow_window_returns_every_commit() {
    let fixture = fixtures::braided(30);
    let repo = open(&fixture);
    let expected = fixture.rev_list(&[]);

    let generous = read(&repo, &HistoryRequest::from_head(expected.len()));
    let narrow = read(
        &repo,
        &HistoryRequest::from_head(expected.len()).with_window(1),
    );

    assert_eq!(ids(&narrow), expected, "a narrow window lost a commit");
    assert_eq!(
        lanes(&narrow.rows),
        lanes(&generous.rows),
        "a narrow window moved a lane"
    );
}

fn repaints(rows: &[HistoryRow]) -> usize {
    rows.iter()
        .flat_map(|row| &row.graph.edges)
        .filter(|edge: &&EdgeSegment| edge.out_of_order)
        .count()
}

/// R1.2's finality clause, end to end through the query and against a history
/// real `git` produced: a line back to a parent the walk delivered early is
/// drawn while that parent's row is still held, and is simply absent once the
/// window has made it final. The commit itself is never lost either way.
#[test]
fn a_row_that_left_the_window_is_never_repainted() {
    let fixture = fixtures::skewed();
    let repo = open(&fixture);
    let expected = fixture.rev_list(&[]);
    assert_eq!(expected.len(), 5, "the skew fixture changed shape");

    let held = read(
        &repo,
        &HistoryRequest::from_head(expected.len()).with_window(8),
    );
    let dropped = read(
        &repo,
        &HistoryRequest::from_head(expected.len()).with_window(1),
    );

    let mut from_git = expected.clone();
    from_git.sort();
    for page in [&held, &dropped] {
        let mut mine = ids(page);
        mine.sort();
        assert_eq!(mine, from_git, "a commit went missing");
    }
    assert_eq!(
        ids(&held),
        ids(&dropped),
        "the window changed the walk order"
    );

    assert!(
        repaints(&held.rows) > 0,
        "the fixture never delivered a parent early, so this decides nothing:\n{:?}",
        held.rows
    );
    assert_eq!(
        repaints(&dropped.rows),
        0,
        "a row was repainted after the window made it final:\n{:?}",
        dropped.rows
    );
}

/// R5.2 territory, but the engine's half of it: a repository with no commits
/// has an empty history, and saying so is not the same as panicking.
#[test]
fn a_repository_with_no_commits_is_reported_rather_than_panicking() {
    let fixture = fixtures::unborn();
    let repo = open(&fixture);
    match repo.history(&HistoryRequest::from_head(10), &CancelSignal::new()) {
        Err(Error::UnbornHead { path }) => {
            assert!(path.exists(), "the error named a path that is not there")
        }
        other => panic!("expected an unborn head, got {other:?}"),
    }
}

/// Starting from named commits rather than `HEAD`, which is how a view that
/// shows every branch will ask.
#[test]
fn a_walk_can_start_from_named_commits() {
    let fixture = fixtures::braided(20);
    let repo = open(&fixture);
    let head = fixture.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let tip = ok(cairn_model::Oid::parse(&head), "parsing HEAD");

    let from_head = read(&repo, &HistoryRequest::from_head(10));
    let from_tip = read(&repo, &HistoryRequest::from_commits([tip], 10));
    assert_eq!(ids(&from_tip), ids(&from_head));
}
