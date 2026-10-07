//! Finding a row by paging the held walk, and the sidebar's rows, through the real boundary
//! (refs-and-status R8.3, R8.5, R8.6, criterion C8).
//!
//! A find is history-lane work: it pages the walk a scroll pages, every page answered as a
//! scroll's is, and stops at the page that holds its row or at the walk's end. What it
//! leaves is what scrolling there would have: the walk's rows, in order, with no gap and no
//! row twice, however it was superseded — by another find, a stop or a scroll. Over this
//! checkout, whose history is at least a handful of rows long.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cairn_model::{
    Disclosure, HeadState, Oid, Ref, RefKind, RefName, RefTarget, RefsSnapshot, SidebarRow,
};

use super::diff_tests::checkout;
use super::epoch::{Epochs, QueryLane};
use super::fetch_tests::collect_until;
use super::pool::{Outbox, RepositoryHandle, Updates, sidebar_rows};
use super::request::{Request, Update};

/// A commit no walk reaches.
fn nowhere() -> Oid {
    Oid::from_bytes(&[0x11; 20]).unwrap_or_else(|error| panic!("{error}"))
}

/// Every row of the walk from every ref, in order: one page as large as any history here.
fn whole_walk(handle: &RepositoryHandle, updates: &mut Updates) -> Vec<Oid> {
    handle.submit(Request::OpenHistory { rows: 1_000_000 });
    let seen = collect_until(updates, |u| matches!(u, Update::Rows { .. }));
    let walk = rows_of(&seen);
    assert!(
        walk.len() >= 6,
        "this checkout's history is too short to find in: {walk:?}"
    );
    walk
}

/// Every row the updates answered, in the order they arrived.
fn rows_of(seen: &[Update]) -> Vec<Oid> {
    seen.iter()
        .filter_map(|update| match update {
            Update::Rows { rows, .. } => Some(rows.ids().collect::<Vec<_>>()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn holds(update: &Update, target: Oid) -> bool {
    matches!(update, Update::Rows { rows, .. } if rows.ids().any(|id| id == target))
}

/// R8.5, R8.6: a row past the loaded pages is found by paging the walk forward until a page
/// holds it, every page answered as a scroll's, and no further — the next page asked starts
/// with the row after the found page's last. The find is asked straight after a reopen, so it
/// also supersedes an open that never started, whose walk it must page from the start.
/// Caught by: a find that pages past the row (more rows kept than scrolling there would
/// keep), a page lost or repeated, pages answered under the query's number (a superseded
/// find's pages dropped), or a find that pages the walk the open was replacing.
#[test]
fn a_find_pages_the_walk_until_a_page_holds_the_row_and_no_further() {
    let (handle, mut updates) = checkout();
    let walk = whole_walk(&handle, &mut updates);
    let at = (walk.len() - 3).min(40);
    let target = walk[at];

    handle.submit(Request::OpenHistory { rows: 1 });
    handle.submit(Request::FindRow { target, rows: 2 });
    let seen = collect_until(&mut updates, |u| holds(u, target));
    let found = rows_of(&seen);
    assert_eq!(
        found,
        walk[..found.len()],
        "the find's pages are not the walk's"
    );
    assert!(
        found.len() > at && found.len() <= at + 2,
        "row {at} found after {} rows, in pages of two",
        found.len()
    );

    handle.submit(Request::MoreHistory { rows: 1 });
    let next = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    assert_eq!(
        rows_of(&next),
        [walk[found.len()]],
        "the walk went on past the page that held the row"
    );
}

/// R8.5: a find asked while another runs supersedes it, and the pages the first had laid out
/// still arrive, so the history has no hole; the second stops at its own row. Caught by:
/// pages answered under the query's number (the first find's pages dropped, the history
/// missing rows), or the second find never asked.
#[test]
fn a_second_find_supersedes_the_first_and_no_page_is_lost() {
    let (handle, mut updates) = checkout();
    let walk = whole_walk(&handle, &mut updates);
    let at = (walk.len() - 3).min(40);
    let target = walk[at];

    handle.submit(Request::OpenHistory { rows: 1 });
    handle.submit(Request::FindRow {
        target: nowhere(),
        rows: 1,
    });
    handle.submit(Request::FindRow { target, rows: 1 });
    let seen = collect_until(&mut updates, |u| holds(u, target));
    // What the window does on finding its row in a page: stops the find, whoever's page it
    // was (the first's may have passed the row before the second began).
    handle.submit(Request::StopFinding);
    handle.submit(Request::MoreHistory { rows: 1 });
    let after = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    let kept: Vec<Oid> = rows_of(&seen).into_iter().chain(rows_of(&after)).collect();
    assert_eq!(kept, walk[..kept.len()], "a page was lost or repeated");
    assert!(
        kept.len() > at + 1,
        "the next page did not follow the find's"
    );
}

/// R8.5: a stop — what a scroll or a row chosen asks during a find — supersedes the find and
/// leaves the walk where it stood, so the scroll's page is the walk's next rows. Caught by: a
/// find that runs on to the walk's end after its stop (the scroll's page empty, every row
/// kept), or a stop that drops the walk.
#[test]
fn a_stopped_find_leaves_the_walk_for_the_next_page() {
    let (handle, mut updates) = checkout();
    let walk = whole_walk(&handle, &mut updates);

    handle.submit(Request::OpenHistory { rows: 1 });
    handle.submit(Request::FindRow {
        target: nowhere(),
        rows: 1,
    });
    handle.submit(Request::StopFinding);
    handle.submit(Request::MoreHistory { rows: 3 });
    let seen = collect_until(
        &mut updates,
        |u| matches!(u, Update::Rows { rows, complete } if rows.len() == 3 || *complete),
    );
    let kept = rows_of(&seen);
    assert_eq!(kept, walk[..kept.len()], "a page was lost or repeated");
    assert!(
        kept.len() < walk.len(),
        "the find ran on to the end of the walk after it was stopped"
    );
}

/// R8.5: the pages a find laid out before it was superseded arrive all the same, read after
/// the supersession: a page is the walk's, answered under the walk's number, not the query's,
/// so a press or a scroll never leaves a hole where the find's last pages were. The find is
/// let run on (a row no page holds) while nothing is read, then stopped, then read. Caught by:
/// pages answered under the query's number, which the stop makes stale — every page laid out
/// before it dropped, and the next page appended after a hole.
#[test]
fn the_pages_a_find_laid_out_before_it_was_stopped_still_arrive() {
    let (handle, mut updates) = checkout();
    let walk = whole_walk(&handle, &mut updates);

    handle.submit(Request::OpenHistory { rows: 1 });
    handle.submit(Request::FindRow {
        target: nowhere(),
        rows: 1,
    });
    // Read until the find has begun, then leave it walking unread.
    let begun = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    std::thread::sleep(Duration::from_millis(50));
    handle.submit(Request::StopFinding);
    handle.submit(Request::MoreHistory { rows: 2 });
    let rest = collect_until(
        &mut updates,
        |u| matches!(u, Update::Rows { rows, complete } if rows.len() == 2 || *complete),
    );
    let kept: Vec<Oid> = rows_of(&begun).into_iter().chain(rows_of(&rest)).collect();
    assert_eq!(
        kept,
        walk[..kept.len()],
        "a page laid out before the stop was lost"
    );
    // A walk that ended was walked by the find, every page of it kept.
    if matches!(rest.last(), Some(Update::Rows { complete: true, .. })) {
        assert_eq!(kept.len(), walk.len(), "the find's pages were dropped");
    }
}

/// R8.5: a find whose row no page holds — a stash whose base no ref reaches, a ref moved since
/// the walk began — pages to the walk's end, and the last page says the walk is complete: what
/// the window reads as the row not being in the graph. Caught by: a find that stops short of
/// the end, or an end the window cannot tell.
#[test]
fn a_find_for_a_row_the_walk_never_reaches_ends_with_the_walk() {
    let (handle, mut updates) = checkout();
    let walk = whole_walk(&handle, &mut updates);

    handle.submit(Request::OpenHistory { rows: 1 });
    handle.submit(Request::FindRow {
        target: nowhere(),
        rows: 5,
    });
    let seen = collect_until(&mut updates, |u| {
        matches!(u, Update::Rows { complete: true, .. })
    });
    assert_eq!(rows_of(&seen), walk, "the find did not page the whole walk");
}

fn many_refs(count: usize) -> Arc<RefsSnapshot> {
    let id = Oid::from_bytes(&[3; 20]).unwrap_or_else(|error| panic!("{error}"));
    Arc::new(RefsSnapshot {
        refs: (0..count)
            .map(|n| Ref {
                name: RefName::new(format!("refs/heads/team-{}/branch-{n:07}", n % 50)),
                kind: RefKind::LocalBranch,
                target: RefTarget::Commit(id),
                symbolic: None,
                upstream: None,
            })
            .collect(),
        head: HeadState::Detached(id),
        stashes: Vec::new(),
        unreadable: 0,
    })
}

fn sorted(mut snapshot: RefsSnapshot) -> RefsSnapshot {
    snapshot.refs.sort_by(|a, b| a.name.cmp(&b.name));
    snapshot
}

/// Phase 06 QA's TC3, R8.3: the sidebar's rows are answered on the repository thread through
/// the real boundary — with the snapshot they index — and of two asked in a row only the
/// second is drawn, though the first was answered before the second was asked. Caught by:
/// rows answered under no number (the first, superseded, drawn too), or answered for another
/// snapshot than the one asked.
#[test]
fn the_sidebars_rows_are_answered_on_a_worker_and_a_newer_ask_supersedes_the_older() {
    let (handle, mut updates) = checkout();
    let refs = Arc::new(sorted(Arc::unwrap_or_clone(many_refs(50_000))));
    let mut open = Disclosure::default();
    open.toggle_folder("refs/heads/team-7");
    handle.submit(Request::FilterRefs {
        refs: Arc::clone(&refs),
        text: String::new(),
        disclosure: Arc::new(Disclosure::default()),
    });
    // Time for the first to be answered, unread: it is then superseded in the channel, where
    // only its number keeps it from being drawn.
    std::thread::sleep(Duration::from_millis(200));
    handle.submit(Request::FilterRefs {
        refs: Arc::clone(&refs),
        text: "branch-000000".to_owned(),
        disclosure: Arc::new(open),
    });
    let seen = collect_until(
        &mut updates,
        |u| matches!(u, Update::FilteredRefs { text, .. } if text == "branch-000000"),
    );
    let answers: Vec<&Update> = seen
        .iter()
        .filter(|u| matches!(u, Update::FilteredRefs { .. }))
        .collect();
    match answers.as_slice() {
        [
            Update::FilteredRefs {
                refs: answered,
                rows,
                ..
            },
        ] => {
            assert!(Arc::ptr_eq(answered, &refs), "rows for another snapshot");
            // Ten of the fifty thousand hold the text, `branch-0000000`..`branch-0000009`,
            // each in its own team's folder, every folder drawn open while filtering.
            let refs_drawn = rows
                .iter()
                .filter(|row| matches!(row, SidebarRow::Ref { .. }))
                .count();
            assert_eq!(refs_drawn, 10, "{rows:?}");
        }
        other => panic!("expected the second ask's rows alone, got {other:?}"),
    }
}

/// Phase 06 QA's TC3, R8.3: an ask superseded while its rows are laid out stops part way,
/// within a few thousand refs, and sends nothing. Driven on the lane's own function with the
/// outbox read raw — beneath the epoch filter, which would hide a superseded answer either
/// way — over a snapshot large enough that laying it out takes far longer than the moment
/// the supersession comes after. Caught by: a pass told to keep going whatever its number
/// (`|| true`), which runs to the end and sends rows no one will draw.
#[test]
fn a_sidebar_ask_superseded_mid_pass_stops_and_sends_nothing() {
    let refs = Arc::new(sorted(Arc::unwrap_or_clone(many_refs(400_000))));
    let disclosure = Disclosure::default();

    // Unsuperseded, for how long the whole pass takes on this machine.
    let epochs = Epochs::new();
    let (outbox, sent) = Outbox::watched();
    let started = Instant::now();
    sidebar_rows(
        Arc::clone(&refs),
        String::new(),
        &disclosure,
        epochs.bump(QueryLane::RefFilter),
        &epochs,
        &outbox,
    );
    let whole = started.elapsed();
    assert_eq!(
        sent.try_iter().count(),
        1,
        "the unsuperseded pass sent nothing"
    );

    let mut stopped = 0;
    for _ in 0..5 {
        let epochs = Epochs::new();
        let (outbox, sent) = Outbox::watched();
        let mine = epochs.bump(QueryLane::RefFilter);
        let superseding = epochs.clone();
        let delay = whole / 10;
        let bump = std::thread::spawn(move || {
            std::thread::sleep(delay);
            let at = Instant::now();
            superseding.bump(QueryLane::RefFilter);
            at
        });
        let began = Instant::now();
        sidebar_rows(
            Arc::clone(&refs),
            String::new(),
            &disclosure,
            mine,
            &epochs,
            &outbox,
        );
        let ended = Instant::now();
        let bumped = bump.join().unwrap_or_else(|_| panic!("the bump panicked"));
        // Only a supersession that landed while the pass ran decides anything.
        if bumped <= began || bumped >= ended {
            continue;
        }
        assert_eq!(
            sent.try_iter().count(),
            0,
            "a pass superseded part way sent its rows"
        );
        assert!(
            ended - began < whole / 2 + Duration::from_millis(5),
            "a pass superseded after {:?} ran {:?} of {whole:?}",
            delay,
            ended - began
        );
        stopped += 1;
    }
    assert!(
        stopped > 0,
        "no supersession landed while a pass ran (a pass takes {whole:?})"
    );
}

/// What one find answered, read as the window reads it: every page appended to one history.
struct Found {
    history: cairn_model::History,
    found: bool,
    elapsed: Duration,
}

/// Opens the walk afresh, reads its first page, then finds `target` from there, appending
/// every page as the window does, until a page holds it or the walk ends.
fn find_through_the_boundary(
    handle: &RepositoryHandle,
    updates: &mut Updates,
    target: Oid,
) -> Found {
    let mut history = cairn_model::History::new();
    handle.submit(Request::OpenHistory {
        rows: crate::PAGE_ROWS,
    });
    let first = collect_until(updates, |u| matches!(u, Update::Rows { .. }));
    for update in first {
        if let Update::Rows { rows, .. } = update {
            history.append(rows).unwrap_or_else(|full| panic!("{full}"));
        }
    }
    let started = Instant::now();
    handle.submit(Request::FindRow {
        target,
        rows: crate::ref_find::FIND_PAGE_ROWS,
    });
    let deadline = Instant::now() + Duration::from_secs(600);
    let found = loop {
        let Some(update) = super::fetch_tests::next_by(updates, deadline, &[]) else {
            panic!("the stream ended during a find");
        };
        if let Update::Rows { rows, complete } = update {
            let found = rows.ids().any(|id| id == target);
            history.append(rows).unwrap_or_else(|full| panic!("{full}"));
            if found || complete {
                break found;
            }
        }
    };
    Found {
        history,
        found,
        elapsed: started.elapsed(),
    }
}

/// Phase 08's measurement (R8.6, C15, C16, the QA brief): a find of a commit deep in a named
/// repository through the real boundary — the repository thread paging the held walk, every
/// page appended to one history as the window appends it — timed (median of seven after a
/// warm-up) and its retained memory read; then a find stopped halfway, as a scroll stops
/// one, with how many rows arrived after the stop and how long after, and the next page asked
/// to show the walk was left where it stopped. The target is `CAIRN_FIND_TARGET` (a commit's
/// hex id), else `CAIRN_FIND_REF`'s commit (a full ref name), else the walk's last row — the
/// repository's oldest commit in the walk's order. Run with `--release` on the bench, which
/// it only reads.
#[test]
#[ignore = "needs a repository named by CAIRN_BENCH_REPO"]
fn measures_a_find_through_the_boundary() {
    let path = std::env::var("CAIRN_BENCH_REPO").unwrap_or_else(|_| panic!("set CAIRN_BENCH_REPO"));
    let (handle, mut updates) = super::diff_tests::opened(std::path::Path::new(&path));

    // The whole walk in one page: the oldest commit, and the snapshot it walks from.
    let started = Instant::now();
    handle.submit(Request::OpenHistory {
        rows: usize::MAX / 2,
    });
    let opened = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    let whole = rows_of(&opened);
    let snapshot = opened.iter().find_map(|u| match u {
        Update::Refs { snapshot, .. } => Some(Arc::clone(snapshot)),
        _ => None,
    });
    eprintln!(
        "WALK rows={} ms={:.0} (warm-up, one page)",
        whole.len(),
        started.elapsed().as_secs_f64() * 1e3
    );
    let target = match (
        std::env::var("CAIRN_FIND_TARGET").ok(),
        std::env::var("CAIRN_FIND_REF").ok(),
    ) {
        (Some(hex), _) => Oid::parse(&hex).unwrap_or_else(|error| panic!("{error}")),
        (None, Some(name)) => snapshot
            .as_deref()
            .and_then(|refs| refs.find(&RefName::new(name.as_str())))
            .and_then(Ref::commit_id)
            .unwrap_or_else(|| panic!("{name} names no commit in the snapshot")),
        (None, None) => *whole.last().unwrap_or_else(|| panic!("an empty walk")),
    };
    let depth = whole.iter().position(|id| *id == target);
    eprintln!("TARGET {target} at row {depth:?}");
    drop(opened);

    let _ = find_through_the_boundary(&handle, &mut updates, target);
    let mut times = Vec::new();
    let mut last = None;
    for _ in 0..7 {
        let found = find_through_the_boundary(&handle, &mut updates, target);
        assert!(found.found, "the find ended without its row");
        times.push(found.elapsed);
        last = Some(found);
    }
    times.sort();
    let found = last.unwrap_or_else(|| panic!("no find ran"));
    let retained = found.history.retained();
    let mib = |bytes: usize| bytes as f64 / (1024.0 * 1024.0);
    eprintln!(
        "FIND median={:.0} ms [{:.0}-{:.0}] rows_kept={} retained={:.2} MiB ({} B/row): rows {:.2}, \
         text {:.2}, lane changes {:.2}, snapshots {:.2}, authors {:.3}, labels {:.3}, stashes {:.3} MiB",
        times[3].as_secs_f64() * 1e3,
        times[0].as_secs_f64() * 1e3,
        times[6].as_secs_f64() * 1e3,
        found.history.len(),
        mib(retained.total()),
        retained.total() / found.history.len().max(1),
        mib(retained.rows),
        mib(retained.text),
        mib(retained.lane_changes),
        mib(retained.snapshots),
        mib(retained.authors),
        mib(retained.labels),
        mib(retained.stashes),
    );
    // What a press looks through on the UI thread for a row not loaded: the labelled rows,
    // the stashes' and `HEAD`'s.
    let mut scans = Vec::new();
    for _ in 0..7 {
        let scanning = Instant::now();
        let place = found.history.labelled_position(nowhere());
        scans.push(scanning.elapsed());
        assert_eq!(place, None);
    }
    scans.sort();
    eprintln!(
        "LOOKUP over the labelled rows of {} loaded rows for a row not among them: median \
         {:.3} ms [{:.3}-{:.3}]",
        found.history.len(),
        scans[3].as_secs_f64() * 1e3,
        scans[0].as_secs_f64() * 1e3,
        scans[6].as_secs_f64() * 1e3,
    );
    let ceiling = std::env::var("CAIRN_C16_MIB").ok().map_or(64.0, |mib| {
        mib.parse::<f64>().unwrap_or_else(|error| panic!("{error}"))
    });
    assert!(
        mib(retained.total()) <= ceiling,
        "{:.1} MiB retained, past C16's {ceiling} MiB",
        mib(retained.total())
    );
    drop(found);

    // Stopped halfway, as a scroll stops it.
    let halfway = times[3] / 2;
    let mut history = cairn_model::History::new();
    handle.submit(Request::OpenHistory {
        rows: crate::PAGE_ROWS,
    });
    for update in collect_until(&mut updates, |u| matches!(u, Update::Rows { .. })) {
        if let Update::Rows { rows, .. } = update {
            history.append(rows).unwrap_or_else(|full| panic!("{full}"));
        }
    }
    let began = Instant::now();
    handle.submit(Request::FindRow {
        target,
        rows: crate::ref_find::FIND_PAGE_ROWS,
    });
    while began.elapsed() < halfway {
        if let Some(Update::Rows { rows, .. }) =
            super::fetch_tests::next_by(&mut updates, began + halfway * 2, &[])
        {
            history.append(rows).unwrap_or_else(|full| panic!("{full}"));
        }
    }
    let stopped = Instant::now();
    let at_stop = history.len();
    handle.submit(Request::StopFinding);
    // Every page laid out before the stop arrives; then nothing, for as long as a find of
    // the whole walk took.
    let mut last_page = None;
    let quiet = times[3];
    loop {
        match poll_for(&mut updates, quiet) {
            Some(Update::Rows { rows, .. }) => {
                assert!(
                    !rows.ids().any(|id| id == target),
                    "the stopped find found its row"
                );
                history.append(rows).unwrap_or_else(|full| panic!("{full}"));
                last_page = Some(stopped.elapsed());
            }
            Some(_) => {}
            None => break,
        }
    }
    let after_stop = history.len() - at_stop;
    handle.submit(Request::MoreHistory {
        rows: crate::PAGE_ROWS,
    });
    let next = collect_until(&mut updates, |u| matches!(u, Update::Rows { .. }));
    let next_rows = rows_of(&next);
    let held = history.len();
    assert_eq!(
        next_rows.first(),
        whole.get(held),
        "the page after the stop did not take the walk up where it stopped"
    );
    eprintln!(
        "CANCEL stopped after {:.0} ms with {at_stop} rows kept; {after_stop} rows arrived after \
         the stop, the last {:?} after it; the next page began at row {held} of {}",
        halfway.as_secs_f64() * 1e3,
        last_page.map(|after| format!("{:.1} ms", after.as_secs_f64() * 1e3)),
        whole.len()
    );
}

/// The next update within `quiet`, or `None` when nothing came.
fn poll_for(updates: &mut Updates, quiet: Duration) -> Option<Update> {
    use std::task::{Context, Poll, Wake, Waker};
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let deadline = Instant::now() + quiet;
    let mut next = std::pin::pin!(updates.next());
    loop {
        if let Poll::Ready(next) = next.as_mut().poll(&mut cx) {
            return next;
        }
        let now = Instant::now();
        if now >= deadline {
            return None;
        }
        std::thread::park_timeout(deadline - now);
    }
}
