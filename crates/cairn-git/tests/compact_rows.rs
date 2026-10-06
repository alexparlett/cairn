//! C15 on real repositories: every row a history pages out draws exactly the edges the lane
//! assigner retained for it before rows were compacted — repaints included — with the
//! frozen copy of that assigner as the oracle, run over the very walk the rows came from.
//! The `#[ignore]`d reporter does the same over every ref of a named repository and
//! measures what compact rows cost.

#[path = "../../cairn-model/tests/layout_before_compaction/mod.rs"]
mod layout_before_compaction;

use std::time::{Duration, Instant};

use cairn_git::{CancelSignal, HistoryOrder, HistoryRequest, Repository};
use cairn_model::{
    GraphRow, HistoryRow, LaneAssigner, LaneChange, Oid, RowContent, RowEdges, row_edges,
};
use layout_before_compaction::{AssignerBeforeCompaction, RowBeforeCompaction};

/// The application's page: `PAGE_ROWS` in `crates/cairn-app/src/main.rs`.
const PAGE: usize = 64;

fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

/// The ids and parents the assigner was handed, in walk order. No wildcard arm.
fn walk_of(rows: &[HistoryRow]) -> Vec<(Oid, Vec<Oid>)> {
    rows.iter()
        .map(|row| match &row.content {
            RowContent::Commit(commit) => (commit.id, commit.parents.clone()),
        })
        .collect()
}

/// The rows the assigner retained before compaction, made final as it made them.
fn before_compaction(walk: &[(Oid, Vec<Oid>)], window: usize) -> Vec<RowBeforeCompaction> {
    let mut assigner = AssignerBeforeCompaction::with_window(window);
    let mut rows = Vec::with_capacity(walk.len());
    for (id, parents) in walk {
        rows.extend(assigner.push(*id, parents.clone()));
    }
    rows.extend(assigner.into_rows());
    rows
}

/// Every ref that names a commit, once each, as the walk is seeded from them.
fn every_ref_tip(repo: &Repository) -> Vec<Oid> {
    let read = ok(repo.refs(&CancelSignal::new()), "reading the refs");
    let mut tips: Vec<Oid> = Vec::new();
    for tip in read
        .snapshot
        .refs
        .iter()
        .filter_map(|reference| reference.target.commit_id())
    {
        if !tips.contains(&tip) {
            tips.push(tip);
        }
    }
    tips
}

/// Pages a held session to the end, keeping every row as the window keeps them.
fn page_all(repo: &Repository, request: &HistoryRequest) -> Vec<HistoryRow> {
    let mut session = ok(repo.history_session(request), "opening the session");
    let mut rows = Vec::new();
    loop {
        let page = ok(session.next_page(PAGE, &CancelSignal::new()), "paging");
        rows.extend(page.rows);
        if page.cursor.is_none() {
            return rows;
        }
    }
}

#[derive(Debug, Default)]
struct Checked {
    rows: usize,
    /// Rows carrying a segment the assigner repainted after a late parent arrived.
    repainted: usize,
}

/// Every row's derived edges are the old assigner's, in its order. `rows` is everything
/// the reader holds; `before` is the old assigner's output for the same rows.
fn assert_draws_what_it_drew(
    what: &str,
    rows: &[HistoryRow],
    before: &[RowBeforeCompaction],
    checked: &mut Checked,
) {
    assert_eq!(rows.len(), before.len(), "{what}: a row was lost");
    for (index, (row, old)) in rows.iter().zip(before).enumerate() {
        assert_eq!(
            row.graph.id, old.id,
            "{what}: row {index} is another commit"
        );
        let drawn = row_edges(rows, index);
        let expected = RowEdges {
            lane: old.lane,
            edges: old.edges.clone(),
        };
        if drawn.as_ref() != Some(&expected) {
            panic!(
                "{what}: row {index} ({}) draws\n  {drawn:?}\nwhere the assigner retained\n  \
                 {expected:?}",
                old.id
            );
        }
        checked.rows += 1;
        if old.edges.iter().any(|edge| edge.out_of_order) {
            checked.repainted += 1;
        }
    }
}

fn cairn() -> Repository {
    ok(
        Repository::discover(env!("CARGO_MANIFEST_DIR")),
        "opening the Cairn checkout",
    )
}

#[test]
fn the_cairn_checkouts_rows_draw_what_the_assigner_drew_before_compaction() {
    let repo = cairn();
    let tips = every_ref_tip(&repo);
    assert!(!tips.is_empty(), "no ref names a commit");

    let mut checked = Checked::default();
    let seeds = [
        ("HEAD", HistoryRequest::from_head(PAGE)),
        ("every ref", HistoryRequest::from_commits(tips, PAGE)),
    ];
    for (seed, request) in &seeds {
        for order in [HistoryOrder::CommitTime, HistoryOrder::GraphOrder] {
            // The default window, and one narrow enough that rows leave it mid-walk.
            for window in [LaneAssigner::DEFAULT_WINDOW, 8] {
                let request = request.clone().with_order(order).with_window(window);
                let rows = page_all(&repo, &request);
                let before = before_compaction(&walk_of(&rows), window);
                assert_draws_what_it_drew(
                    &format!("{seed}, {order:?}, window {window}"),
                    &rows,
                    &before,
                    &mut checked,
                );
            }
        }
    }
    eprintln!("checked {checked:?}");
    assert!(
        checked.rows > PAGE,
        "the checkout paged too little to decide anything: {checked:?}"
    );
    // Graph order delivers parents before children throughout.
    assert!(
        checked.repainted > 0,
        "no row was repainted, so the late lines decided nothing: {checked:?}"
    );
}

/// Caught by: a cold page's, or a resumed session's, first row carrying no snapshot, so it
/// cannot be drawn without the rows the replay laid out and dropped.
#[test]
fn each_cold_page_and_resumed_session_draws_on_its_own_what_the_assigner_drew_for_it() {
    let repo = cairn();
    let window = 8;
    let limit = 7;
    let mut checked = Checked::default();
    let mut pages = 0usize;
    for order in [HistoryOrder::CommitTime, HistoryOrder::GraphOrder] {
        let first = HistoryRequest::from_head(limit)
            .with_order(order)
            .with_window(window);
        let walk = walk_of(&page_all(&repo, &first));

        let mut request = first;
        let mut skip = 0usize;
        loop {
            let page = ok(
                repo.history(&request, &CancelSignal::new()),
                "reading a page",
            );
            // A cold page replays the walk to its end and makes the window final there.
            let end = (skip + limit).min(walk.len());
            let before = before_compaction(&walk[..end], window);
            assert_draws_what_it_drew(
                &format!("{order:?}, the page from row {skip}"),
                &page.rows,
                &before[skip..],
                &mut checked,
            );
            pages += 1;
            skip += page.rows.len();
            let Some(cursor) = page.cursor else {
                break;
            };
            // A session resumed from the cursor replays to it and holds the walk from there.
            let resumed = page_all(&repo, &HistoryRequest::resume(cursor.clone(), limit));
            let whole = before_compaction(&walk, window);
            assert_draws_what_it_drew(
                &format!("{order:?}, a session resumed at row {skip}"),
                &resumed,
                &whole[skip..],
                &mut checked,
            );
            request = HistoryRequest::resume(cursor, limit);
            if pages > 40 {
                break;
            }
        }
    }
    eprintln!("checked {checked:?} over {pages} pages");
    assert!(pages > 2, "too few pages to cross a page boundary: {pages}");
    assert!(
        checked.repainted > 0,
        "no paged row was repainted: {checked:?}"
    );
}

// --- The reporter: C15's measured half, and K ---

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// A `/proc/self/status` field, in kB.
fn proc_kb(field: &str) -> usize {
    let status = ok(
        std::fs::read_to_string("/proc/self/status"),
        "reading status",
    );
    status
        .lines()
        .find(|line| line.starts_with(field))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|kb| kb.parse().ok())
        .unwrap_or(0)
}

/// Heap and struct bytes the kept rows hold, by capacity (allocator overhead not counted).
#[derive(Debug, Default)]
struct Retained {
    /// The row vector's capacity, slack included.
    row_structs: usize,
    /// Lane changes and snapshots.
    graph: usize,
    parents: usize,
    text: usize,
    snapshots: usize,
}

impl Retained {
    fn of(rows: &Vec<HistoryRow>) -> Self {
        let mut retained = Self {
            row_structs: rows.capacity() * size_of::<HistoryRow>(),
            ..Self::default()
        };
        for row in rows {
            retained.graph += row.graph.heap_bytes();
            retained.snapshots += usize::from(row.graph.has_snapshot());
            match &row.content {
                RowContent::Commit(commit) => {
                    retained.parents += commit.parents.capacity() * size_of::<Oid>();
                    retained.text += commit.summary.capacity()
                        + commit.author_name.capacity()
                        + commit.author_email.capacity();
                }
            }
        }
        retained
    }

    fn total(&self) -> usize {
        self.row_structs + self.graph + self.parents + self.text
    }
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn request_for(repo: &Repository, seed: &str) -> HistoryRequest {
    match seed {
        "head" => HistoryRequest::from_head(PAGE),
        "refs" => HistoryRequest::from_commits(every_ref_tip(repo), PAGE),
        other => panic!("CAIRN_FIND_SEED={other}: want head or refs"),
    }
}

/// Reporter. Env: `CAIRN_BENCH_REPO`; `CAIRN_C15_MODE` — `find` (time a find of
/// `CAIRN_FIND_TARGET`, or of the last row, paging as the window does; retained bytes),
/// `equivalence` (every row of the whole history against the frozen assigner), or
/// `derive` (the worst-case derivation at each snapshot interval in `CAIRN_C15_KS`);
/// `CAIRN_FIND_SEED` is `head` or `refs`. Run with `--release`, one mode per process.
#[test]
#[ignore = "needs a repository named by CAIRN_BENCH_REPO"]
fn measures_compact_rows_over_a_named_repository() {
    let path = env("CAIRN_BENCH_REPO").unwrap_or_else(|| panic!("set CAIRN_BENCH_REPO"));
    let mode = env("CAIRN_C15_MODE").unwrap_or_else(|| "find".to_owned());
    let seed = env("CAIRN_FIND_SEED").unwrap_or_else(|| "refs".to_owned());
    let repo = ok(Repository::discover(&path), "opening the repository");
    eprintln!(
        "repository {path}, mode {mode}, seed {seed}; HistoryRow {} B, GraphRow {} B, \
         LaneChange {} B",
        size_of::<HistoryRow>(),
        size_of::<GraphRow>(),
        size_of::<LaneChange>(),
    );
    match mode.as_str() {
        "find" => find(&repo, &seed),
        "equivalence" => equivalence(&repo, &seed),
        "derive" => derive(&repo, &seed),
        other => panic!("CAIRN_C15_MODE={other}"),
    }
}

fn find(repo: &Repository, seed: &str) {
    let target = env("CAIRN_FIND_TARGET").map(|hex| ok(Oid::parse(&hex), "CAIRN_FIND_TARGET"));
    let anon_before = proc_kb("RssAnon:");
    let started = Instant::now();
    let request = request_for(repo, seed);
    let mut session = ok(repo.history_session(&request), "opening the session");
    // Every loaded row is kept, as the window keeps them.
    let mut kept: Vec<HistoryRow> = Vec::new();
    let mut found = None;
    loop {
        let page = ok(session.next_page(PAGE, &CancelSignal::new()), "paging");
        let end = page.cursor.is_none();
        let hit = target.and_then(|target| {
            page.rows
                .iter()
                .position(|row| row.graph.id == target)
                .map(|at| kept.len() + at + 1)
        });
        kept.extend(page.rows);
        if hit.is_some() {
            found = hit;
            break;
        }
        if end {
            found = found.or(Some(kept.len()));
            break;
        }
    }
    let elapsed = started.elapsed();
    let anon_after = proc_kb("RssAnon:");
    let retained = Retained::of(&kept);
    eprintln!(
        "FIND seed={seed} found_at_row={} rows_kept={} ms={:.1}",
        found.unwrap_or(0),
        kept.len(),
        elapsed.as_secs_f64() * 1e3
    );
    eprintln!(
        "RETAINED total={:.1} MiB ({} B/row): row structs {:.1} MiB (vec capacity {}), \
         lane changes and snapshots {:.2} MiB ({} snapshots), parents {:.1} MiB, text {:.1} MiB; RssAnon {anon_before} -> {anon_after} kB \
         (+{:.1} MiB)",
        mib(retained.total()),
        retained.total() / kept.len().max(1),
        mib(retained.row_structs),
        kept.capacity(),
        mib(retained.graph),
        retained.snapshots,
        mib(retained.parents),
        mib(retained.text),
        (anon_after.saturating_sub(anon_before)) as f64 / 1024.0,
    );
}

fn equivalence(repo: &Repository, seed: &str) {
    let started = Instant::now();
    let rows = page_all(repo, &request_for(repo, seed));
    let paged = started.elapsed();
    let before = before_compaction(&walk_of(&rows), LaneAssigner::DEFAULT_WINDOW);
    let mut checked = Checked::default();
    assert_draws_what_it_drew(&format!("seed {seed}"), &rows, &before, &mut checked);
    let late_lines: usize = rows
        .iter()
        .flat_map(|row| row.graph.changes())
        .filter(|change| matches!(change, LaneChange::StartsLate { .. }))
        .count();
    eprintln!(
        "EQUIVALENT seed={seed} rows={} repainted_rows={} late_lines={late_lines} \
         paged_in={paged:?} checked_in={:?}",
        checked.rows,
        checked.repainted,
        started.elapsed() - paged,
    );
}

/// Worst case: a row `k - 1` rows below its snapshot, derived alone, as the list derives
/// each row it draws.
fn derive(repo: &Repository, seed: &str) {
    let rows = page_all(repo, &request_for(repo, seed));
    let walk = walk_of(&rows);
    drop(rows);
    let ks: Vec<usize> = env("CAIRN_C15_KS")
        .unwrap_or_else(|| "1,8,16,32,64,128,256,512,1024".to_owned())
        .split(',')
        .filter_map(|k| k.trim().parse().ok())
        .collect();
    for k in ks {
        let graphs = LaneAssigner::new()
            .with_snapshot_every(k)
            .assign_each(walk.iter().cloned());
        let snapshot_bytes: usize = graphs
            .iter()
            .filter(|graph| graph.has_snapshot())
            .map(|graph| graph.heap_bytes() - size_of_val(graph.changes()))
            .sum();
        let mut samples: Vec<Duration> = Vec::new();
        let mut segments = 0usize;
        // Every row `k - 1` below a snapshot, the furthest a row can be from its own.
        let mut index = k - 1;
        while index < graphs.len() {
            let started = Instant::now();
            let drawn = row_edges(&graphs, index);
            samples.push(started.elapsed());
            segments += drawn.map_or(0, |drawn| drawn.edges.len());
            index += k;
        }
        samples.sort_unstable();
        let at = |fraction: f64| {
            samples
                .get(((samples.len().saturating_sub(1)) as f64 * fraction).round() as usize)
                .copied()
                .unwrap_or_default()
        };
        eprintln!(
            "DERIVE k={k} rows_sampled={} mean_segments={:.1} per_row p50={:?} p99={:?} \
             max={:?}; snapshots {:.2} MiB",
            samples.len(),
            segments as f64 / samples.len().max(1) as f64,
            at(0.5),
            at(0.99),
            at(1.0),
            mib(snapshot_bytes),
        );
    }
}
