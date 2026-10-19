//! The history thread through the real boundary: an opening pass that reads the whole history
//! holds up nothing but the history — a filter and a refresh's refs are answered while it runs,
//! the reopen a refresh leads to supersedes it, a find or its stop never does, and a close ends
//! it rather than waiting for it. Over a line of commits written as one pack for these tests,
//! long enough that its opening pass takes a measurable while (no commit-graph: the pass reads
//! every commit before the first page).

use std::sync::Arc;
use std::time::{Duration, Instant};

use cairn_model::{Disclosure, Oid};

use crate::worker::diff::tests::opened;
use crate::worker::history::reading::Pacing;
use crate::worker::history::tests::{many_refs, poll_for};
use crate::worker::network::tests::{block_on, collect_until, opened_as};
use crate::worker::pool::{RepositoryHandle, Updates, open_paced_with};
use crate::worker::request::{Request, Update};
use crate::worker::startup::Startup;
use crate::worker::written_repository::WrittenRepository;

/// Commits in the line: an opening pass over it reads them all.
const LINE: usize = 200_000;

/// Every count the walk can report told at once: how a test knows a pass is running.
const EVERY_COUNT: Pacing = Pacing {
    after: Duration::ZERO,
    every: Duration::ZERO,
};

/// The line, and the boundary over it with every count of an opening pass told.
fn line(name: &str) -> (WrittenRepository, RepositoryHandle, Updates) {
    let line = WrittenRepository::packed(name, LINE);
    let (handle, updates) = paced(line.path());
    (line, handle, updates)
}

fn paced(path: &std::path::Path) -> (RepositoryHandle, Updates) {
    match open_paced_with(path, Startup::of_this_process(), EVERY_COUNT) {
        Ok((handle, mut updates, _)) => {
            opened_as(&mut updates);
            (handle, updates)
        }
        Err(error) => panic!("starting the worker: {error}"),
    }
}

/// Reads until the walk being opened tells its first count — its pass is running, with nearly
/// all of it still to read — failing if its first page comes first.
fn reading(updates: &mut Updates) {
    let seen = collect_until(updates, |u| {
        matches!(u, Update::Reading { .. } | Update::Rows { .. })
    });
    assert!(
        matches!(seen.last(), Some(Update::Reading { .. })),
        "the pass told no count before its first page"
    );
}

/// Reads until `wanted` or the walk's first page, saying which came first.
fn before_the_first_page(updates: &mut Updates, wanted: impl Fn(&Update) -> bool) -> bool {
    let seen = collect_until(updates, |u| wanted(u) || matches!(u, Update::Rows { .. }));
    seen.last().is_some_and(wanted)
}

fn first_ids(update: &Update) -> Option<Vec<Oid>> {
    match update {
        Update::Rows { rows, .. } => Some(rows.ids().collect()),
        _ => None,
    }
}

/// How long an open of the line takes to its first page, the walk from where `main` points.
fn pass(handle: &RepositoryHandle, updates: &mut Updates) -> Duration {
    let asked = Instant::now();
    handle.submit(Request::OpenHistory {
        rows: 64,
        lost: false,
    });
    collect_until(updates, |u| matches!(u, Update::Rows { .. }));
    asked.elapsed()
}

/// A filter asked while an opening pass runs — once the worker has told its first count — is
/// answered at once, on the repository thread, before the walk's first page, while the history
/// thread reads on; its usual time, with nothing opening, is read first for the report. Caught
/// by: the history lane served on the repository thread again, or a filter that otherwise
/// waits on the open (answered after the first page).
#[test]
fn a_filter_is_answered_while_the_history_opens() {
    let (_line, handle, mut updates) = line("cairn-filter-while-opening");
    let refs = many_refs(500);
    let filter = || Request::FilterRefs {
        refs: Arc::clone(&refs),
        text: "branch-00".to_owned(),
        disclosure: Arc::new(Disclosure::default()),
    };
    let mut usual = Vec::new();
    for _ in 0..5 {
        let asked = Instant::now();
        handle.submit(filter());
        collect_until(&mut updates, |u| matches!(u, Update::FilteredRefs { .. }));
        usual.push(asked.elapsed());
    }
    usual.sort();

    let opened = Instant::now();
    handle.submit(Request::OpenHistory {
        rows: 64,
        lost: false,
    });
    reading(&mut updates);
    let asked = Instant::now();
    handle.submit(filter());
    assert!(
        before_the_first_page(&mut updates, |u| matches!(u, Update::FilteredRefs { .. })),
        "the filter was answered after the walk's first page"
    );
    let answered = asked.elapsed();
    collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    eprintln!(
        "a filter over 500 refs: {:.2} ms alone (median of five), {:.2} ms asked during an \
         opening pass whose first page took {:.0} ms",
        usual[2].as_secs_f64() * 1e3,
        answered.as_secs_f64() * 1e3,
        opened.elapsed().as_secs_f64() * 1e3
    );
}

/// A refresh asked while an opening pass runs is answered while it still runs, before the
/// first page: with nothing moved it says to reopen nothing, the pass having named the refs it
/// opens from the ones its walk began from; with `main` moved it says to reopen, and the reopen
/// the window then asks supersedes the walk still opening, so the first page to arrive is the
/// new walk's, from where `main` now points, and no page of the walk it replaced is drawn.
/// Caught by: a refresh queued behind the open (its refs after the first page), a walk that
/// names the refs it opens from only once it has opened (a refresh during the pass reopens for
/// nothing), or a reopen that does not supersede the walk opening (its first page, from the
/// old `main`, drawn).
#[test]
fn a_refresh_supersedes_an_open_still_reading() {
    let (line, handle, mut updates) = line("cairn-refresh-supersedes-open");
    // As the window opens: a refresh with no walk begun says to reopen.
    handle.submit(Request::Refresh);
    collect_until(&mut updates, |u| {
        matches!(u, Update::Refs { reopen: true, .. })
    });

    handle.submit(Request::OpenHistory {
        rows: 64,
        lost: false,
    });
    reading(&mut updates);
    handle.submit(Request::Refresh);
    assert!(
        before_the_first_page(&mut updates, |u| matches!(
            u,
            Update::Refs { reopen: false, .. }
        )),
        "a refresh with nothing moved was answered after the first page, or said to reopen"
    );
    // `main` moves back five commits while the walk opens, and the window refreshes.
    let main = line.path().join(".git/refs/heads/main");
    std::fs::write(&main, format!("{}\n", line.commits[5]))
        .unwrap_or_else(|error| panic!("moving main: {error}"));
    handle.submit(Request::Refresh);
    assert!(
        before_the_first_page(&mut updates, |u| matches!(
            u,
            Update::Refs { reopen: true, .. }
        )),
        "the refresh was answered after the walk's first page, or said not to reopen"
    );

    handle.submit(Request::OpenHistory {
        rows: 64,
        lost: false,
    });
    let pages = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    let first = pages.iter().find_map(first_ids).unwrap_or_default();
    assert_eq!(
        first.first(),
        line.commits.get(5),
        "the first page drawn is not the new walk's"
    );
}

/// A find, and its stop, asked while a walk opens leave the opening pass running: the walk's
/// first page still arrives, and the next page asked continues it, the pass run once. Through
/// the boundary, the three asked back to back; that the open runs whatever the history lane
/// asked after it is decided with the jobs queued before the history thread starts
/// (`an_open_runs_whatever_the_history_lane_asked_after_it`). Caught by: an open dropped
/// because the history lane moved past it (no first page at all — the window waits on it for
/// good), or a pass cancelled by the history lane rather than the walk's own number (the same).
#[test]
fn a_find_or_its_stop_never_supersedes_a_walk_still_opening() {
    let (line, handle, mut updates) = line("cairn-find-keeps-open");
    handle.submit(Request::OpenHistory {
        rows: 3,
        lost: false,
    });
    handle.submit(Request::FindRow {
        target: Oid::from_bytes(&[0x11; 20]).unwrap_or_else(|error| panic!("{error}")),
        rows: 1,
    });
    handle.submit(Request::StopFinding);
    let mut seen = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    while let Some(update) = poll_for(&mut updates, Duration::from_millis(200)) {
        seen.push(update);
    }
    handle.submit(Request::MoreHistory { rows: 2 });
    seen.extend(collect_until(
        &mut updates,
        |u| matches!(u, Update::Rows { rows, .. } if rows.len() == 2),
    ));
    let kept: Vec<Oid> = seen.iter().filter_map(first_ids).flatten().collect();
    assert_eq!(
        kept,
        line.commits[..kept.len()],
        "a page was lost or repeated"
    );
    assert!(
        kept.len() <= 6,
        "the find paged on after its stop: {} rows",
        kept.len()
    );
}

/// A close asked while an opening pass runs — once the worker has told its first count — ends
/// the pass at its next commit rather than waiting for it: the stream ends in what a close with
/// nothing running takes, plus well under the rest of the pass, both read first on the same
/// line. Decided by time alone: a close stops the epochs as it is asked, so the stream drops
/// any page the pass might still send. Caught by: a pass the close does not cancel (the history
/// thread, joined as the repository closes, runs it to its end first), or a close that never
/// joins the history thread (the stream never ends).
#[test]
fn a_close_during_an_opening_pass_does_not_wait_for_it() {
    let (line, handle, mut updates) = line("cairn-close-while-opening");
    let pass = pass(&handle, &mut updates);
    // What a close costs with nothing running: its reaps and its threads' ends.
    let closing = Instant::now();
    handle.submit(Request::Close);
    while block_on(updates.next()).is_some() {}
    let idle = closing.elapsed();

    let (handle, mut updates) = paced(line.path());
    handle.submit(Request::OpenHistory {
        rows: 64,
        lost: false,
    });
    reading(&mut updates);
    let closing = Instant::now();
    handle.submit(Request::Close);
    while block_on(updates.next()).is_some() {}
    let closed = closing.elapsed();
    eprintln!(
        "an opening pass of {:.0} ms closed as its first count was told: the stream ended {:.1} \
         ms after the close was asked, against {:.1} ms for a close with nothing running",
        pass.as_secs_f64() * 1e3,
        closed.as_secs_f64() * 1e3,
        idle.as_secs_f64() * 1e3
    );
    assert!(
        closed < idle + pass / 2,
        "the close took {closed:?}, against {idle:?} idle, of a {pass:?} pass"
    );
}

/// Median of a sorted run of timings, in milliseconds.
fn median_ms(sorted: &[Duration]) -> f64 {
    sorted
        .get(sorted.len() / 2)
        .map_or(0.0, |median| median.as_secs_f64() * 1e3)
}

/// Measures the history thread over a named repository through the real boundary: the opening
/// — a refresh's refs, then the walk opened from them to its first `PAGE_ROWS` rows, its pass
/// included — seven times after a warm-up; when each open's counts were told; a page after the
/// opening pass; the sidebar's filter over the repository's own refs, alone and asked a quarter
/// of the way into an opening pass; a close asked a quarter of the way into an opening pass,
/// against one with nothing running; and, with `CAIRN_BENCH_ORACLE` naming a file of git's own `git log --date-order
/// --format=%H` for the same tips, every row of the whole history compared with it. Run with
/// `--release` on the bench, which it only reads.
#[test]
#[ignore = "needs a repository named by CAIRN_BENCH_REPO"]
fn measures_the_history_thread_against_a_named_repository() {
    let path = std::env::var("CAIRN_BENCH_REPO").unwrap_or_else(|_| panic!("set CAIRN_BENCH_REPO"));
    let (handle, mut updates) = opened(std::path::Path::new(&path));
    opened_as(&mut updates);
    handle.submit(Request::Refresh);
    let refreshed = collect_until(&mut updates, |u| matches!(u, Update::Refs { .. }));
    let Some(refs) = refreshed.iter().find_map(|u| match u {
        Update::Refs { snapshot, .. } => Some(Arc::clone(snapshot)),
        _ => None,
    }) else {
        panic!("no refs")
    };
    eprintln!("{} refs, {} stashes", refs.refs.len(), refs.stashes.len());
    let rows = crate::PAGE_ROWS;

    // The opening, each time a reopen, and when its counts were told.
    let mut opens = Vec::new();
    let mut told = Vec::new();
    for run in 0..8 {
        let asked = Instant::now();
        handle.submit(Request::OpenHistory { rows, lost: false });
        let mut counts = Vec::new();
        collect_until(&mut updates, |u| {
            if let Update::Reading { commits } = u {
                counts.push((asked.elapsed(), *commits));
            }
            matches!(u, Update::Rows { .. })
        });
        if run > 0 {
            opens.push(asked.elapsed());
            told = counts;
        }
    }
    opens.sort();
    // A quarter of the way into an opening pass, where the filter and the close are asked.
    let into = opens.get(opens.len() / 2).copied().unwrap_or_default() / 4;
    eprintln!(
        "OPEN to the first {rows} rows: median {:.0} ms [{:.0}-{:.0}]",
        median_ms(&opens),
        opens.first().map_or(0.0, |d| d.as_secs_f64() * 1e3),
        opens.last().map_or(0.0, |d| d.as_secs_f64() * 1e3)
    );
    let gaps: Vec<f64> = told
        .windows(2)
        .map(|pair| (pair[1].0 - pair[0].0).as_secs_f64() * 1e3)
        .collect();
    eprintln!(
        "READING told {} times, the first at {:.0} ms ({} commits), the last {} commits; the \
         least gap {:.0} ms",
        told.len(),
        told.first().map_or(0.0, |(at, _)| at.as_secs_f64() * 1e3),
        told.first().map_or(0, |(_, commits)| *commits),
        told.last().map_or(0, |(_, commits)| *commits),
        gaps.iter().copied().fold(f64::INFINITY, f64::min)
    );

    // A page after the opening pass.
    let mut pages = Vec::new();
    for _ in 0..7 {
        let asked = Instant::now();
        handle.submit(Request::MoreHistory { rows });
        collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
        pages.push(asked.elapsed());
    }
    pages.sort();
    eprintln!(
        "PAGE of {rows} after the pass: median {:.2} ms",
        median_ms(&pages)
    );

    // The sidebar's filter, alone and during an opening pass.
    let filter = || Request::FilterRefs {
        refs: Arc::clone(&refs),
        text: "1.".to_owned(),
        disclosure: Arc::new(Disclosure::default()),
    };
    let mut alone = Vec::new();
    for _ in 0..7 {
        let asked = Instant::now();
        handle.submit(filter());
        collect_until(&mut updates, |u| matches!(u, Update::FilteredRefs { .. }));
        alone.push(asked.elapsed());
    }
    alone.sort();
    let mut during = Vec::new();
    for _ in 0..7 {
        handle.submit(Request::OpenHistory { rows, lost: false });
        std::thread::sleep(into);
        let asked = Instant::now();
        handle.submit(filter());
        let seen = collect_until(&mut updates, |u| {
            matches!(u, Update::FilteredRefs { .. } | Update::Rows { .. })
        });
        during.push(asked.elapsed());
        assert!(
            matches!(seen.last(), Some(Update::FilteredRefs { .. })),
            "the filter waited for the open"
        );
        collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    }
    during.sort();
    eprintln!(
        "FILTER over {} refs: median {:.2} ms alone, {:.2} ms asked {:.0} ms into an opening pass \
         [{:.2}-{:.2}]",
        refs.refs.len(),
        median_ms(&alone),
        median_ms(&during),
        into.as_secs_f64() * 1e3,
        during.first().map_or(0.0, |d| d.as_secs_f64() * 1e3),
        during.last().map_or(0.0, |d| d.as_secs_f64() * 1e3)
    );

    // The whole history against git's.
    if let Ok(oracle) = std::env::var("CAIRN_BENCH_ORACLE") {
        let expected: Vec<Oid> = std::fs::read_to_string(&oracle)
            .unwrap_or_else(|error| panic!("reading {oracle}: {error}"))
            .lines()
            .map(|line| Oid::parse(line.trim()).unwrap_or_else(|error| panic!("{error}")))
            .collect();
        handle.submit(Request::OpenHistory {
            rows: usize::MAX / 2,
            lost: false,
        });
        let whole: Vec<Oid> = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }))
            .iter()
            .filter_map(first_ids)
            .flatten()
            .collect();
        let first_difference = whole
            .iter()
            .zip(&expected)
            .position(|(ours, git)| ours != git);
        eprintln!(
            "ORDER {} rows against git's {}: first difference at {first_difference:?}",
            whole.len(),
            expected.len()
        );
        assert_eq!(first_difference, None);
        assert_eq!(whole.len(), expected.len());
    }

    // A close, idle and during an opening pass.
    let closing = Instant::now();
    handle.submit(Request::Close);
    while block_on(updates.next()).is_some() {}
    let idle = closing.elapsed();
    let mut closes = Vec::new();
    for _ in 0..7 {
        let (handle, mut updates) = opened(std::path::Path::new(&path));
        opened_as(&mut updates);
        handle.submit(Request::OpenHistory { rows, lost: false });
        std::thread::sleep(into);
        let closing = Instant::now();
        handle.submit(Request::Close);
        while block_on(updates.next()).is_some() {}
        closes.push(closing.elapsed());
    }
    closes.sort();
    eprintln!(
        "CLOSE {:.1} ms with nothing running; asked {:.0} ms into an opening pass: median {:.1} ms \
         [{:.1}-{:.1}]",
        idle.as_secs_f64() * 1e3,
        into.as_secs_f64() * 1e3,
        median_ms(&closes),
        closes.first().map_or(0.0, |d| d.as_secs_f64() * 1e3),
        closes.last().map_or(0.0, |d| d.as_secs_f64() * 1e3)
    );
}
