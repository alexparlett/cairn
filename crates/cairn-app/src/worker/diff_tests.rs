//! The query lanes, through the real boundary (PRD R4, criterion C8): a scroll does not
//! cancel a diff and a diff does not cancel a scroll; a changes query supersedes the
//! file-diff lane; a superseded diff's `git` is killed, not left to run out; an answer
//! naming another selection is never drawn (`session`'s
//! `an_answer_naming_another_selection_is_never_drawn`, through these helpers, since a
//! render-side test may name neither the toolkit here nor a wait there); and a fetch still
//! supersedes nothing.
//!
//! Real `git` over this checkout (read only) or over a fixture borrowing its objects, and a
//! stub `git` where a read must hang. No `Command` here — the guards scan this crate's
//! tests — so fixtures are built with `std::fs`.

use std::path::Path;
use std::time::{Duration, Instant};

use cairn_model::{ChangeSet, Oid};

use super::discovery::Discovery;
use super::fetch_tests::{BorrowedRepository, Home, RuntimeDir, WAIT, collect_until, next_by};
use super::lifecycle_tests::StubGit;
#[cfg(target_os = "linux")]
use super::lifecycle_tests::alive_in_group;
use super::pool::{RepositoryHandle, Updates, open_with};
use super::request::{Comparison, DiffOptions, FileQuery, FileTarget, Request, Update};
use super::startup::Startup;

/// The boundary over `repository`, with this process's `git`.
fn opened(repository: &Path) -> (RepositoryHandle, Updates) {
    match open_with(repository, Startup::of_this_process()) {
        Ok((handle, updates, _)) => (handle, updates),
        Err(error) => panic!("starting the worker: {error}"),
    }
}

/// The boundary over this checkout; for the window's tests too, which may not wait.
pub(crate) fn checkout() -> (RepositoryHandle, Updates) {
    opened(Path::new(env!("CARGO_MANIFEST_DIR")))
}

/// The first `count` commits of the history `HEAD` reaches, newest first.
pub(crate) fn commits(handle: &RepositoryHandle, updates: &mut Updates, count: usize) -> Vec<Oid> {
    handle.submit(Request::OpenHistory { rows: count });
    let seen = collect_until(updates, |u| matches!(u, Update::Rows { .. }));
    match seen.last() {
        Some(Update::Rows { rows, .. }) if rows.len() == count => {
            rows.iter().map(|row| row.graph.id).collect()
        }
        other => panic!("expected {count} rows, got {other:?}"),
    }
}

/// `of`'s change set, asked and awaited.
fn change_set(handle: &RepositoryHandle, updates: &mut Updates, of: Comparison) -> ChangeSet {
    handle.submit(Request::Changes { of });
    let seen = collect_until(updates, |u| matches!(u, Update::Changes { .. }));
    match seen.last() {
        Some(Update::Changes {
            of: answered,
            changes,
        }) if *answered == of => changes.clone(),
        other => panic!("expected {of:?}'s change set, got {other:?} after {seen:?}"),
    }
}

fn answers_changes(update: &Update, of: Comparison) -> bool {
    matches!(update, Update::Changes { of: answered, .. } if *answered == of)
}

/// The next answer naming `of`'s change set, as it leaves the boundary — the epoch filter
/// already behind it — for a test of the window's side, which may not wait itself.
pub(crate) fn changes_answer(updates: &mut Updates, of: Comparison) -> Update {
    let seen = collect_until(updates, |u| answers_changes(u, of));
    match seen.last() {
        Some(update) => update.clone(),
        None => unreachable!("collect_until returns what it stopped on"),
    }
}

/// C8, the first half: a scroll asked while a changes query is in flight does not cancel
/// it — the change set still arrives, and so does the page. Mutation that reddens it:
/// numbering every lane on one counter (`QueryLane::index` answering 0 for every lane), so
/// the page supersedes the diff and its answer is dropped on arrival.
#[test]
fn a_scroll_does_not_cancel_a_diff() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 2);
    let of = Comparison::Commit(ids[1]);

    handle.submit(Request::Changes { of });
    handle.submit(Request::OpenHistory { rows: 40 });

    let (mut pages, mut answered) = (0, false);
    let seen = collect_until(&mut updates, |u| {
        pages += usize::from(matches!(u, Update::Rows { .. }));
        answered |= answers_changes(u, of);
        answered && pages == 1
    });
    assert!(
        !seen
            .iter()
            .any(|u| matches!(u, Update::DiffFailed { .. } | Update::Failed { .. })),
        "{seen:?}"
    );
    drop(handle);
}

/// C8, the second half: a changes query and a file diff asked while a page is being
/// walked do not cancel it — the page arrives. Mutation that reddens it: one counter for
/// every lane, under which the diff supersedes the page and the walk is abandoned.
#[test]
fn a_diff_does_not_cancel_a_scroll() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 2);
    let of = Comparison::Commit(ids[0]);
    let changes = change_set(&handle, &mut updates, of);
    let Some(file) = changes.files.first().cloned() else {
        panic!("{of:?} changed no file");
    };

    handle.submit(Request::OpenHistory { rows: 60 });
    handle.submit(Request::Changes {
        of: Comparison::Commit(ids[1]),
    });
    handle.submit(Request::FileDiff(FileQuery {
        target: FileTarget::Committed { of, file },
        options: DiffOptions::default(),
    }));
    let seen = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    match seen.last() {
        Some(Update::Rows { rows, .. }) => assert!(!rows.is_empty(), "an empty page"),
        other => panic!("the page never arrived: {other:?}"),
    }
    drop(handle);
}

/// C8: a changes query supersedes the file diff in flight — its answer is never
/// delivered, whether it was cancelled before it started, killed while git read, or
/// finished and dropped on arrival — and nothing else. The file diff, served before the
/// changes query on the one diff thread, would arrive first if it survived. Mutation that
/// reddens it: `QueryLane::Changes.supersedes()` naming the changes lane alone.
#[test]
fn a_changes_query_supersedes_the_file_diff_in_flight() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 2);
    let (first, second) = (Comparison::Commit(ids[0]), Comparison::Commit(ids[1]));
    let changes = change_set(&handle, &mut updates, first);
    let Some(file) = changes.files.first().cloned() else {
        panic!("{first:?} changed no file");
    };

    handle.submit(Request::FileDiff(FileQuery {
        target: FileTarget::Committed { of: first, file },
        options: DiffOptions::default(),
    }));
    handle.submit(Request::Changes { of: second });
    let seen = collect_until(&mut updates, |u| answers_changes(u, second));
    assert!(
        !seen
            .iter()
            .any(|u| matches!(u, Update::FileDiff { .. } | Update::DiffFailed { .. })),
        "the superseded file diff was delivered: {seen:?}"
    );
    drop(handle);
}

/// C8, the negative of the one above: a file diff does not supersede the changes query in
/// flight, nor one file diff the history — both answers arrive. Mutation that reddens it:
/// a file diff numbered in the changes lane too.
#[test]
fn a_file_diff_does_not_supersede_the_changes_query() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 2);
    let first = Comparison::Commit(ids[0]);
    let changes = change_set(&handle, &mut updates, first);
    let Some(file) = changes.files.first().cloned() else {
        panic!("{first:?} changed no file");
    };
    let second = Comparison::Commit(ids[1]);
    let query = FileQuery {
        target: FileTarget::Committed { of: first, file },
        options: DiffOptions::default(),
    };

    handle.submit(Request::Changes { of: second });
    handle.submit(Request::FileDiff(query.clone()));
    let mut changes_seen = false;
    let mut file_seen = false;
    collect_until(&mut updates, |u| {
        changes_seen |= answers_changes(u, second);
        file_seen |=
            matches!(u, Update::FileDiff { query: answered, diff: Some(_) } if *answered == query);
        changes_seen && file_seen
    });
    drop(handle);
}

/// C8, a fast click through a file list: each file's diff supersedes the one before, and
/// only the last is answered. (Whether the ones before are killed rather than run out is
/// `a_superseded_diff_kills_its_git`; whether one that finished anyway is dropped on
/// arrival is `pool`'s `an_answer_superseded_in_its_lane_is_dropped_on_arrival`.)
#[test]
fn a_click_through_files_answers_the_last_file_only() {
    let (handle, mut updates) = checkout();
    let ids = commits(&handle, &mut updates, 8);
    // A commit that changed at least two files, so there is something to click through.
    let (of, changes) = ids
        .iter()
        .map(|id| {
            let of = Comparison::Commit(*id);
            (of, change_set(&handle, &mut updates, of))
        })
        .find(|(_, changes)| changes.files.len() >= 2)
        .unwrap_or_else(|| panic!("none of {ids:?} changed two files"));
    let queries: Vec<FileQuery> = changes
        .files
        .iter()
        .take(6)
        .map(|file| FileQuery {
            target: FileTarget::Committed {
                of,
                file: file.clone(),
            },
            options: DiffOptions::default(),
        })
        .collect();
    for query in &queries {
        handle.submit(Request::FileDiff(query.clone()));
    }
    let last = queries.last().cloned();
    let seen = collect_until(
        &mut updates,
        |u| matches!(u, Update::FileDiff { query, .. } if Some(query) == last.as_ref()),
    );
    let answered: Vec<_> = seen
        .iter()
        .filter_map(|u| match u {
            Update::FileDiff { query, .. } => Some(query),
            _ => None,
        })
        .collect();
    assert_eq!(answered, [&queries[queries.len() - 1]]);
    drop(handle);
}

/// A diff thread's `git` that leads its group, starts a grandchild holding its pipes and
/// waits on it for ten minutes, appending its pid to `leaders`: what only a group kill
/// ends. `/bin/sleep` by its path, since the stub's `PATH` is its own directory.
const DIFF_TREE_HANGS: &str = "  echo $$ >> \"$DIR/leaders\"\n  /bin/sleep 600 &\n  wait";

/// The pids the stub's `diff-tree` has written, waiting until there are `count`.
fn leaders(stub: &StubGit, count: usize) -> Vec<i32> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let written: Vec<i32> = std::fs::read_to_string(stub.directory.join("leaders"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect();
        if written.len() >= count {
            return written;
        }
        assert!(
            Instant::now() < deadline,
            "the stub's diff-tree ran {} times, not {count}",
            written.len()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// C8 and the design input on fast clicks: a superseded diff's `git` is KILLED — its whole
/// process group, the grandchild holding its pipes included — and the command log records
/// it as cancelled; it is not left to run out with its answer discarded. Mutation that
/// reddens it: handing the engine a cancel that never fires (`CancelSignal::new()` in
/// place of the epoch's `Superseded`), which leaves the first `diff-tree` running for ten
/// minutes.
#[test]
fn a_superseded_diff_kills_its_git() {
    let ids = {
        let (handle, mut updates) = checkout();
        commits(&handle, &mut updates, 2)
    };
    let stub = StubGit::answering("2.45.0", "diff-tree", DIFF_TREE_HANGS);
    let fixture = BorrowedRepository::new(&format!("cairn-diff-killed-{}", std::process::id()));
    fixture.point_main_at(&ids[0].to_string());
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates) = match super::pool::open(
        &fixture.fixture.path,
        &Discovery::new(stub.startup(Some((&home, &runtime)))),
    ) {
        Ok((handle, updates, _)) => (handle, updates),
        Err(error) => panic!("starting the worker: {error}"),
    };

    handle.submit(Request::Changes {
        of: Comparison::Commit(ids[0]),
    });
    let first = leaders(&stub, 1)[0];
    #[cfg(target_os = "linux")]
    assert!(
        alive_in_group(first).len() >= 2,
        "the case needs the first diff-tree and its grandchild running in one group"
    );
    // The next commit selected, as a click on the list does.
    let superseded = Instant::now();
    handle.submit(Request::Changes {
        of: Comparison::Commit(ids[1]),
    });
    leaders(&stub, 2);

    #[cfg(target_os = "linux")]
    {
        let deadline = superseded + Duration::from_secs(10);
        while !alive_in_group(first).is_empty() {
            assert!(
                Instant::now() < deadline,
                "ten seconds after it was superseded, the first diff-tree's group was still \
                 running: {:?}",
                alive_in_group(first)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    // The log holds each invocation once it is over; the first is, as a cancel.
    let deadline = Instant::now() + WAIT;
    let record = loop {
        handle.submit(Request::CommandLog);
        let seen = collect_until(&mut updates, |u| matches!(u, Update::CommandLog { .. }));
        let found = seen.iter().find_map(|u| match u {
            Update::CommandLog { records } => records
                .iter()
                .find(|record| {
                    record.arguments.iter().any(|a| a == "diff-tree")
                        && record
                            .arguments
                            .iter()
                            .any(|a| a.contains(&ids[0].to_string()))
                })
                .cloned(),
            _ => None,
        });
        if let Some(record) = found {
            break record;
        }
        assert!(
            Instant::now() < deadline,
            "the first diff-tree never reached the log"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        record.cancelled,
        "the superseded diff-tree was not ended as a cancel: {record:?}"
    );

    // Neither query answers: one superseded, the other closed with the repository.
    handle.submit(Request::Close);
    let mut seen = Vec::new();
    while let Some(update) = next_by(&mut updates, Instant::now() + WAIT, &seen) {
        seen.push(update);
    }
    assert!(
        !seen
            .iter()
            .any(|u| matches!(u, Update::Changes { .. } | Update::DiffFailed { .. })),
        "a superseded or closed diff answered: {seen:?}"
    );
    drop(handle);
}

/// C8: a fetch still supersedes nothing — a page and a changes query asked before it are
/// both answered. A fixture borrowing this checkout's objects, so the fetch (of a remote
/// that does not exist, which fails at once and writes nothing) runs in a repository of
/// the test's own. Mutation that reddens it: an operation numbered in a lane, such as
/// `Request::Fetch` in the changes lane.
#[test]
fn a_fetch_still_supersedes_nothing() {
    let ids = {
        let (handle, mut updates) = checkout();
        commits(&handle, &mut updates, 1)
    };
    let fixture = BorrowedRepository::new(&format!("cairn-diff-fetch-{}", std::process::id()));
    fixture.point_main_at(&ids[0].to_string());
    let (handle, mut updates) = opened(&fixture.fixture.path);
    let of = Comparison::Commit(ids[0]);

    handle.submit(Request::OpenHistory { rows: 1 });
    handle.submit(Request::Changes { of });
    handle.submit(Request::Fetch {
        remote: "/nonexistent/cairn-remote.git".to_owned(),
    });
    let (mut rows, mut changes, mut fetched) = (false, false, false);
    collect_until(&mut updates, |u| {
        rows |= matches!(u, Update::Rows { .. });
        changes |= answers_changes(u, of);
        fetched |= matches!(
            u,
            Update::FetchFailed { .. }
                | Update::FetchFinished { .. }
                | Update::FetchCancelled { .. }
        );
        rows && changes && fetched
    });
    drop(handle);
}
