//! History query tests. Expectations are read back out of the `git` binary.

mod fixtures;

use std::cell::Cell;
use std::collections::HashMap;

use cairn_git::{
    Cancel, CancelSignal, Error, HistoryOrder, HistoryPage, HistoryRequest, Repository,
};
use cairn_model::{CommitSummary, EdgeSegment, HistoryRow, RowContent, RowId};

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

/// No wildcard arm: a new row kind must fail to compile here.
fn hex_id(row: &HistoryRow) -> String {
    match row.id() {
        RowId::Commit(id) => id.to_string(),
    }
}

fn commit_of(row: &HistoryRow) -> &CommitSummary {
    match &row.content {
        RowContent::Commit(commit) => commit,
    }
}

fn ids(page: &HistoryPage) -> Vec<String> {
    page.rows.iter().map(hex_id).collect()
}

fn ids_of(rows: &[HistoryRow]) -> Vec<String> {
    rows.iter().map(hex_id).collect()
}

fn lanes(rows: &[HistoryRow]) -> Vec<(String, usize)> {
    rows.iter()
        .map(|row| (hex_id(row), row.graph.lane.index()))
        .collect()
}

fn read(repo: &Repository, request: &HistoryRequest) -> HistoryPage {
    ok(
        repo.history(request, &CancelSignal::new()),
        "reading history",
    )
}

/// Fixture dates rise along every parent link, so any divergence from `git` is real.
#[test]
fn every_row_matches_what_git_reports() {
    let fixture = fixtures::braided(30);
    let repo = open(&fixture);
    let expected = fixture.rev_list();
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
        let id = hex_id(row);
        let commit = commit_of(row);
        let mine: Vec<String> = commit.parents.iter().map(ToString::to_string).collect();
        assert_eq!(Some(&mine), parents_of.get(&id), "parents of {id}");

        let expected_text = match text_of.get(&id) {
            Some(fields) => fields,
            None => panic!("git did not report {id} at all"),
        };
        assert_eq!(commit.summary, expected_text[1], "summary of {id}");
        assert_eq!(commit.author_name, expected_text[2], "author of {id}");
        assert_eq!(commit.author_email, expected_text[3], "email of {id}");
        assert_eq!(
            commit.author_time.to_string(),
            expected_text[4],
            "author time of {id}"
        );
        assert_eq!(
            row.id(),
            RowId::Commit(row.graph.id),
            "the two halves named different commits"
        );
    }
}

#[test]
fn a_limit_bounds_the_page_and_a_cursor_continues_it() {
    let fixture = fixtures::braided(30);
    let repo = open(&fixture);
    let expected = fixture.rev_list();

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

/// Caught by: resolving a cursor's starting points again when it is resumed.
#[test]
fn moving_a_ref_between_cold_pages_does_not_move_the_walk() {
    let fixture = fixtures::braided(20);
    let repo = open(&fixture);
    let expected = fixture.rev_list();

    let first = read(&repo, &HistoryRequest::from_head(5));
    let Some(cursor) = first.cursor.clone() else {
        panic!("the first page ended the history; raise the fixture size");
    };

    fixture.git(&["commit", "--quiet", "--allow-empty", "-m", "moved on"]);
    fixture.git(&["branch", "--force", "side", "HEAD~3"]);
    assert_ne!(
        fixture.rev_list(),
        expected,
        "the ref did not move, so this decides nothing"
    );

    let rest = read(
        &repo,
        &HistoryRequest::resume(cursor.clone(), expected.len()),
    );
    assert_eq!(
        [ids(&first), ids(&rest)].concat(),
        expected,
        "the second page walked from where the refs are now, not where the walk began"
    );

    let (rows, _) = drain_session(&repo, &HistoryRequest::resume(cursor, expected.len()), 4);
    assert_eq!(
        [ids(&first), ids_of(&rows)].concat(),
        expected,
        "a session resumed from the cursor walked from where the refs are now"
    );
}

/// Lane indices identical; an edge may differ only where the split run has not yet seen its repaint.
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
            apart.content, together.content,
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

    // Page two walks past everything page one covered and reads none of it.
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

/// The braided fixture cannot put a backward line's ends in different pages.
#[test]
fn a_backward_line_across_a_page_boundary_is_a_gained_segment() {
    let fixture = fixtures::skewed();
    let repo = open(&fixture);
    let expected = fixture.rev_list();

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

/// Stops after a fixed number of polls, not after a time.
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

#[test]
fn a_cancelled_query_stops_walking_and_says_where() {
    let fixture = fixtures::braided(40);
    let repo = open(&fixture);
    let total = fixture.rev_list().len();
    let stop_at = 6;
    assert!(
        total > stop_at * 3,
        "the fixture must outlast the cancellation"
    );

    // The control that makes "it stopped early" mean something.
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

/// Caught by: the window dropping rows rather than making them final.
#[test]
fn a_narrow_window_returns_every_commit() {
    let fixture = fixtures::braided(30);
    let repo = open(&fixture);
    let expected = fixture.rev_list();

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

/// Caught by: repainting a row the window has already made final.
#[test]
fn a_row_that_left_the_window_is_never_repainted() {
    let fixture = fixtures::skewed();
    let repo = open(&fixture);
    let expected = fixture.rev_list();
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

#[test]
fn a_sha256_repository_reads_like_any_other() {
    let fixture = fixtures::braided_in("sha256", 20);
    assert_eq!(
        fixture.git(&["rev-parse", "--show-object-format"]).trim(),
        "sha256",
        "the fixture is not a SHA-256 repository"
    );
    let expected = fixture.rev_list();
    assert!(expected.iter().all(|id| id.len() == 64));
    let repo = open(&fixture);

    let whole = read(&repo, &HistoryRequest::from_head(expected.len() + 1));
    assert_eq!(ids(&whole), expected, "the walk diverged from git's");

    let (rows, _) = drain_session(&repo, &HistoryRequest::from_head(expected.len()), 7);
    assert_eq!(ids_of(&rows), expected, "the session diverged from git's");

    let first = read(&repo, &HistoryRequest::from_head(5));
    let Some(cursor) = first.cursor.clone() else {
        panic!("a partial page left no cursor");
    };
    let rest = read(&repo, &HistoryRequest::resume(cursor, expected.len()));
    assert_eq!(
        [ids(&first), ids(&rest)].concat(),
        expected,
        "a SHA-256 cursor did not continue the walk"
    );

    // Engine to model and back: a 32-byte id named as a starting point.
    let tip = ok(cairn_model::Oid::parse(&expected[0]), "parsing HEAD");
    let from_tip = read(&repo, &HistoryRequest::from_commits([tip], 10));
    assert_eq!(ids(&from_tip), expected[..10]);
    assert_eq!(commit_of(&from_tip.rows[0]).id, tip);

    let narrower = ok(
        cairn_model::Oid::parse("0123456789abcdef0123456789abcdef01234567"),
        "parsing a well-formed SHA-1 id",
    );
    match repo.history_session(&HistoryRequest::from_commits([narrower], 5)) {
        Err(Error::Walk { .. }) => {}
        other => {
            panic!("expected a walk failure for a SHA-1 id in a SHA-256 repository, got {other:?}")
        }
    }
}

/// Caught by: the `BreadthFirst` arm reaching a different set of commits.
#[test]
fn graph_order_reaches_the_same_commits_as_commit_time_order() {
    let fixture = fixtures::braided(30);
    let repo = open(&fixture);
    let expected = fixture.rev_list();

    let by_time = read(
        &repo,
        &HistoryRequest::from_head(expected.len()).with_order(HistoryOrder::CommitTime),
    );
    let by_graph = read(
        &repo,
        &HistoryRequest::from_head(expected.len()).with_order(HistoryOrder::GraphOrder),
    );

    let mut from_git = expected.clone();
    from_git.sort();
    let mut theirs = ids(&by_graph);
    theirs.sort();
    assert_eq!(
        theirs, from_git,
        "graph order reached a different commit set"
    );
    assert_eq!(
        theirs.len(),
        by_graph.rows.len(),
        "graph order repeated a commit"
    );
    assert_eq!(
        ids(&by_time),
        expected,
        "commit-time order stopped matching git"
    );
    assert!(
        by_graph.cursor.is_none(),
        "the whole history left a cursor behind"
    );
}

#[test]
fn resuming_ignores_an_order_or_window_the_cursor_did_not_come_from() {
    let fixture = fixtures::braided(30);
    let repo = open(&fixture);

    let first = read(
        &repo,
        &HistoryRequest::from_head(6)
            .with_order(HistoryOrder::CommitTime)
            .with_window(4),
    );
    let cursor = match first.cursor.clone() {
        Some(cursor) => cursor,
        None => panic!("six commits ended the history"),
    };

    let plain = read(&repo, &HistoryRequest::resume(cursor.clone(), 6));
    let meddled = read(
        &repo,
        &HistoryRequest::resume(cursor, 6)
            .with_order(HistoryOrder::GraphOrder)
            .with_window(1024),
    );
    assert_eq!(
        (ids(&meddled), lanes(&meddled.rows)),
        (ids(&plain), lanes(&plain.rows)),
        "resuming honoured an order or window the cursor did not carry"
    );
}

/// Caught by: a zero limit ending the history or swallowing the cursor.
#[test]
fn a_limit_of_zero_reads_nothing_and_keeps_the_cursor() {
    let fixture = fixtures::braided(20);
    let repo = open(&fixture);

    let first = read(&repo, &HistoryRequest::from_head(4));
    let cursor = match first.cursor.clone() {
        Some(cursor) => cursor,
        None => panic!("four commits ended the history"),
    };

    let nothing = read(&repo, &HistoryRequest::resume(cursor.clone(), 0));
    assert!(nothing.rows.is_empty(), "a zero limit returned rows");
    assert_eq!(nothing.walked, 0, "a zero limit walked the repository");
    assert_eq!(
        nothing.cursor.as_ref(),
        Some(&cursor),
        "a zero limit lost the caller's place in the history"
    );

    let resumed = read(&repo, &HistoryRequest::resume(cursor, 4));
    let again = match nothing.cursor {
        Some(cursor) => read(&repo, &HistoryRequest::resume(cursor, 4)),
        None => panic!("already asserted"),
    };
    assert_eq!(
        ids(&again),
        ids(&resumed),
        "the returned cursor is not the one given"
    );
}

#[test]
fn bad_starting_points_are_errors_rather_than_empty_pages() {
    let fixture = fixtures::braided(10);
    let repo = open(&fixture);

    let absent = ok(
        cairn_model::Oid::parse("0123456789abcdef0123456789abcdef01234567"),
        "parsing a well-formed id that is not in the repository",
    );
    match repo.history(
        &HistoryRequest::from_commits([absent], 5),
        &CancelSignal::new(),
    ) {
        Err(Error::Walk { .. }) => {}
        other => panic!("expected a walk failure for an unknown commit, got {other:?}"),
    }

    let wider = ok(
        cairn_model::Oid::parse(&"0123456789abcdef".repeat(4)),
        "parsing a well-formed SHA-256 id",
    );
    match repo.history(
        &HistoryRequest::from_commits([wider], 5),
        &CancelSignal::new(),
    ) {
        Err(Error::Walk { .. }) => {}
        other => {
            panic!("expected a walk failure for a SHA-256 id in a SHA-1 repository, got {other:?}")
        }
    }

    let empty = read(&repo, &HistoryRequest::from_commits([], 5));
    assert!(empty.rows.is_empty(), "no starting point produced rows");
    assert!(
        empty.cursor.is_none(),
        "no starting point produced a cursor"
    );
}

/// The replayed commits' objects are deleted, so any call site that decodes them fails.
#[test]
fn a_replayed_prefix_is_walked_but_never_decoded() {
    let fixture = fixtures::braided(30);
    fixtures::write_commit_graph(&fixture);
    let expected = fixture.rev_list();
    let page_size = 6;

    // Page one, while every object is still readable, to get a cursor.
    let first = read(&open(&fixture), &HistoryRequest::from_head(page_size));
    assert_eq!(first.rows.len(), page_size);
    let cursor = match first.cursor.clone() {
        Some(cursor) => cursor,
        None => panic!("six commits ended the history"),
    };

    // All of page one but the tip, which gitoxide reads from the object database regardless.
    let unreadable = &expected[1..page_size];
    fixtures::delete_objects(&fixture, unreadable);

    // A second handle: the first one's object cache would serve the deleted commits.
    let repo = open(&fixture);
    let second = read(&repo, &HistoryRequest::resume(cursor, page_size));
    assert_eq!(second.walked, page_size * 2, "the prefix was not replayed");
    assert_eq!(ids(&second), &expected[page_size..page_size * 2]);
    assert_eq!(second.decoded, second.rows.len());

    // Proves those objects really are unreadable.
    match repo.history(&HistoryRequest::from_head(page_size), &CancelSignal::new()) {
        Err(Error::ReadCommit { id, .. }) => {
            assert!(unreadable.contains(&id), "failed on the wrong commit")
        }
        other => panic!("expected the deleted objects to be unreadable, got {other:?}"),
    }
}

/// Caught by: deleting the poll in the one-commit look-ahead.
#[test]
fn cancelling_exactly_at_the_page_boundary_is_still_a_cancellation() {
    let fixture = fixtures::braided(20);
    let repo = open(&fixture);
    let limit = 5;
    assert!(
        fixture.rev_list().len() > limit + 1,
        "the fixture is too short"
    );

    // The limit-th poll is the last inside the loop; the next is the look-ahead.
    let signal = StopAfter {
        limit,
        polls: Cell::new(0),
    };
    match repo.history(&HistoryRequest::from_head(limit), &signal) {
        Err(Error::Cancelled { walked }) => assert_eq!(walked, limit),
        other => panic!("expected a cancellation at the boundary, got {other:?}"),
    }
    assert_eq!(
        signal.polls.get(),
        limit + 1,
        "the look-ahead was not polled"
    );
}

// ── The live walk session ──

/// Pages a session to the end, returning every row and, per page, `(walked, returned)`.
fn drain_session(
    repo: &Repository,
    request: &HistoryRequest,
    page_size: usize,
) -> (Vec<HistoryRow>, Vec<(usize, usize)>) {
    let mut session = ok(repo.history_session(request), "starting a session");
    let mut rows = Vec::new();
    let mut cost = Vec::new();
    loop {
        let page = ok(session.next_page(page_size, &CancelSignal::new()), "paging");
        cost.push((page.walked, page.rows.len()));
        let done = page.cursor.is_none();
        rows.extend(page.rows);
        if done {
            break;
        }
    }
    (rows, cost)
}

#[test]
fn a_session_returns_what_the_cursor_path_returns_row_for_row() {
    let fixture = fixtures::braided(40);
    let repo = open(&fixture);
    let expected = fixture.rev_list();
    let request = HistoryRequest::from_head(expected.len()).with_window(8);

    let one_shot = read(&repo, &request);
    let (rows, _) = drain_session(&repo, &request, 5);

    assert_eq!(
        ids_of(&rows),
        expected,
        "the session missed or added commits"
    );
    assert_eq!(
        lanes(&rows),
        lanes(&one_shot.rows),
        "the same commits landed in different lanes"
    );
}

#[test]
fn paging_a_session_costs_the_page_and_not_the_pages_before_it() {
    let fixture = fixtures::braided(60);
    let repo = open(&fixture);
    let window = 4;
    let page_size = 5;
    let request = HistoryRequest::from_head(usize::MAX).with_window(window);

    let (rows, cost) = drain_session(&repo, &request, page_size);
    assert!(
        cost.len() >= 4,
        "the fixture gave only {} pages to compare",
        cost.len()
    );

    // No page after the first walks more than it returned. Page one also primes the window.
    for (n, (walked, returned)) in cost.iter().enumerate().skip(1) {
        assert!(
            *walked <= page_size,
            "page {n} walked {walked} commits for {returned} rows: {cost:?}"
        );
    }
    assert!(
        cost[0].0 <= page_size + window + 1,
        "priming cost {} commits for a window of {window}",
        cost[0].0
    );

    let total: usize = cost.iter().map(|(walked, _)| walked).sum();
    assert_eq!(rows.len(), fixture.rev_list().len());
    assert_eq!(
        total,
        rows.len(),
        "the session walked {total} commits to return {} rows",
        rows.len()
    );

    // The negative: the cursor path grows with the page index.
    let mut cursor = read(&repo, &HistoryRequest::from_head(page_size)).cursor;
    let mut replayed = Vec::new();
    for _ in 0..3 {
        let Some(at) = cursor else { break };
        let page = read(&repo, &HistoryRequest::resume(at, page_size));
        replayed.push(page.walked);
        cursor = page.cursor;
    }
    assert!(
        replayed.windows(2).all(|pair| pair[1] > pair[0]),
        "the cursor path was expected to grow with the page index, got {replayed:?}"
    );
}

#[test]
fn cancelling_a_session_stops_the_walk_and_keeps_its_progress() {
    let fixture = fixtures::braided(40);
    let repo = open(&fixture);
    let request = HistoryRequest::from_head(usize::MAX).with_window(2);
    let mut session = ok(repo.history_session(&request), "starting a session");

    let stop_at = 7;
    let signal = StopAfter {
        limit: stop_at,
        polls: Cell::new(0),
    };
    match session.next_page(1_000, &signal) {
        Err(Error::Cancelled { walked }) => assert_eq!(
            walked, stop_at,
            "the walk ran past the cancellation instead of stopping at it"
        ),
        other => panic!("expected a cancellation, got {other:?}"),
    }
    assert_eq!(
        signal.polls.get(),
        stop_at + 1,
        "the signal was not polled once per commit"
    );

    // Progress stayed: the next page walks fewer commits than it returns rows.
    let resumed = ok(session.next_page(4, &CancelSignal::new()), "resuming");
    assert_eq!(resumed.rows.len(), 4);
    assert!(
        resumed.walked < 4,
        "resuming re-walked {} commits for 4 rows; the cancelled work was lost",
        resumed.walked
    );

    // And the rows are still the right ones.
    let expected = fixture.rev_list();
    assert_eq!(ids_of(&resumed.rows), &expected[..4]);
}

#[test]
fn a_cursor_taken_from_a_session_restarts_it_where_it_stopped() {
    let fixture = fixtures::braided(30);
    let repo = open(&fixture);
    let expected = fixture.rev_list();
    let request = HistoryRequest::from_head(usize::MAX).with_window(4);

    let mut warm = ok(repo.history_session(&request), "starting a session");
    let first = ok(warm.next_page(6, &CancelSignal::new()), "paging");
    let cursor = match first.cursor.clone() {
        Some(cursor) => cursor,
        None => panic!("six rows ended a thirty-commit history"),
    };
    assert_eq!(cursor.rows_behind(), 6);
    drop(warm);

    let mut cold = ok(
        repo.history_session(&HistoryRequest::resume(cursor, 6)),
        "restarting from the cursor",
    );
    let second = ok(cold.next_page(6, &CancelSignal::new()), "paging");

    assert_eq!(ids_of(&first.rows), &expected[..6]);
    assert_eq!(ids_of(&second.rows), &expected[6..12]);
    assert!(
        second.walked > second.decoded,
        "the replayed prefix should cost walk steps, and it cost none"
    );
}

#[test]
fn a_session_over_a_skewed_history_returns_every_commit() {
    let fixture = fixtures::skewed();
    let repo = open(&fixture);
    let expected = fixture.rev_list();
    let request = HistoryRequest::from_head(usize::MAX).with_window(2);

    let (rows, _) = drain_session(&repo, &request, 2);
    assert_eq!(ids_of(&rows), expected);
}

#[test]
fn priming_the_window_walks_but_never_decodes() {
    let fixture = fixtures::braided(60);
    let repo = open(&fixture);
    let window = 20;
    let page_size = 5;
    let mut session = ok(
        repo.history_session(&HistoryRequest::from_head(usize::MAX).with_window(window)),
        "starting a session",
    );

    let first = ok(session.next_page(page_size, &CancelSignal::new()), "paging");
    assert_eq!(first.rows.len(), page_size);
    assert_eq!(
        first.decoded, page_size,
        "read {} commit objects to return {page_size} rows",
        first.decoded
    );
    // Without this the assertion above passes on a session that primed nothing.
    assert!(
        first.walked >= page_size + window,
        "walked only {} commits, so the window of {window} was never primed",
        first.walked
    );
}

/// Caught by: skipping an unreadable commit instead of naming it.
#[test]
fn a_session_names_the_commit_it_cannot_read() {
    let fixture = fixtures::braided(20);
    // With a commit-graph the walk reads parent ids without the object.
    fixtures::write_commit_graph(&fixture);
    let expected = fixture.rev_list();
    let missing = expected[3].clone();
    fixtures::delete_objects(&fixture, std::slice::from_ref(&missing));

    let repo = open(&fixture);
    let mut session = ok(
        repo.history_session(&HistoryRequest::from_head(usize::MAX).with_window(2)),
        "starting a session",
    );
    match session.next_page(8, &CancelSignal::new()) {
        Err(Error::ReadCommit { id, .. }) => assert_eq!(id, missing, "failed on the wrong commit"),
        other => panic!("expected the missing object to be reported, got {other:?}"),
    }

    // The control: with the object present the same page reads cleanly.
    // The `let` binding is needed: a session borrows its repository.
    let whole = fixtures::braided(20);
    fixtures::write_commit_graph(&whole);
    let intact = open(&whole);
    let mut fine = ok(
        intact.history_session(&HistoryRequest::from_head(usize::MAX).with_window(2)),
        "starting a session",
    );
    assert_eq!(
        ok(fine.next_page(8, &CancelSignal::new()), "paging")
            .rows
            .len(),
        8
    );
}
