//! History requests, cursors and pages, laid out in lanes.
//!
//! Bounded by construction: a limit and a [`HistoryCursor`], never a call that
//! walks to the root. Parent ids come from the walk, never from a decoded
//! object, which is read only for the rows a page returns;
//! [`HistoryPage::walked`] and [`HistoryPage::decoded`] report the difference.
//! See `docs/systems/history-graph.md`.

mod session;

use cairn_model::{CommitSummary, HistoryRow, LaneAssigner, Oid, RowContent};

pub use session::HistorySession;

use crate::{Cancel, Error, Repository};

/// Neither variant is topological — gitoxide has no `--topo-order` — so either
/// can hand a parent over before its child. The lane assigner is total over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryOrder {
    /// Newest committer date first.
    CommitTime,
    /// Unsorted, as the commit graph mentions them; branches interleave.
    GraphOrder,
}

impl Default for HistoryOrder {
    /// Commit time is the readable order and, with the object cache, the
    /// cheaper one end to end: walking *and laying out* 50k commits took 129 ms
    /// against 196 ms in graph order, which leaves far more lanes open per row.
    /// (Walking alone is the other way round, which is why
    /// [`Repository::discover`] quotes different numbers.)
    fn default() -> Self {
        Self::CommitTime
    }
}

impl HistoryOrder {
    fn sorting(self) -> gix::revision::walk::Sorting {
        match self {
            Self::CommitTime => gix::revision::walk::Sorting::ByCommitTime(
                gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            ),
            Self::GraphOrder => gix::revision::walk::Sorting::BreadthFirst,
        }
    }
}

/// Where a page picks up from. Opaque: hand it back to
/// [`HistoryRequest::resume`] and nothing else. It carries the order and the
/// window as well as the position, since both decide lane numbering (R1.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCursor {
    tips: Vec<Oid>,
    order: HistoryOrder,
    window: usize,
    walked: usize,
}

impl HistoryCursor {
    /// How many commits the pages before this one covered.
    pub fn rows_behind(&self) -> usize {
        self.walked
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Start {
    Head,
    Commits(Vec<Oid>),
    Resume(HistoryCursor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRequest {
    start: Start,
    order: HistoryOrder,
    limit: usize,
    window: usize,
}

impl HistoryRequest {
    /// Starts at whatever `HEAD` resolves to.
    pub fn from_head(limit: usize) -> Self {
        Self::starting(Start::Head, limit)
    }

    /// Starts at the given commits, typically branch tips.
    pub fn from_commits(tips: impl IntoIterator<Item = Oid>, limit: usize) -> Self {
        Self::starting(Start::Commits(tips.into_iter().collect()), limit)
    }

    /// Continues where the page that produced `cursor` stopped.
    pub fn resume(cursor: HistoryCursor, limit: usize) -> Self {
        let (order, window) = (cursor.order, cursor.window);
        Self {
            start: Start::Resume(cursor),
            order,
            limit,
            window,
        }
    }

    fn starting(start: Start, limit: usize) -> Self {
        Self {
            start,
            order: HistoryOrder::default(),
            limit,
            window: LaneAssigner::DEFAULT_WINDOW,
        }
    }

    /// Ignored when resuming: a new order mid-history would renumber lanes the
    /// caller has already drawn.
    pub fn with_order(mut self, order: HistoryOrder) -> Self {
        if !matches!(self.start, Start::Resume(_)) {
            self.order = order;
        }
        self
    }

    /// How many rows the lane assigner holds before making them final; default
    /// [`LaneAssigner::DEFAULT_WINDOW`]. Widen it for skew deeper than that, but
    /// the window is primed before the first row is delivered. Ignored when
    /// resuming, like [`Self::with_order`].
    pub fn with_window(mut self, window: usize) -> Self {
        if !matches!(self.start, Start::Resume(_)) {
            self.window = window;
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPage {
    /// The commits, newest first, with the lane and edges that draw each. Lane
    /// indices are the same however the history was paged; edges are not — a
    /// backward line is drawn by the page reaching the child, so its upper half
    /// is dropped when that lands on an earlier page's row, leaving the lower
    /// half flagged [`cairn_model::EdgeSegment::out_of_order`]. A view must not
    /// assume a flagged segment has a visible other end.
    pub rows: Vec<HistoryRow>,
    /// Where the next page starts, or `None` at the end of the history. A limit
    /// of zero reads nothing and hands back the cursor it was given.
    pub cursor: Option<HistoryCursor>,
    /// Commits laid out for this page, including those before its first row:
    /// larger than `rows.len()` on a resumed page, which replays.
    pub walked: usize,
    /// Commit objects read: one per returned row and no more. A replayed prefix
    /// costs a walk step each, never an object.
    pub decoded: usize,
}

impl Repository {
    /// Reads one page of history, laid out in lanes. `cancel` is polled once
    /// per commit visited; [`Error::Cancelled`] then says how far it got.
    ///
    /// Resuming replays the walk, so two pages of `n` describe the same commits
    /// in the same lanes as one page of `2n`, and page `k` walks `k x n` commits
    /// while decoding only `n`. The cold-restart route, not the scroll route —
    /// see [`Repository::history_session`].
    pub fn history(
        &self,
        request: &HistoryRequest,
        cancel: &impl Cancel,
    ) -> Result<HistoryPage, Error> {
        read_page(self, request, cancel)
    }
}

fn read_page(
    repo: &Repository,
    request: &HistoryRequest,
    cancel: &impl Cancel,
) -> Result<HistoryPage, Error> {
    let Resolved {
        tips,
        order,
        window,
        skip,
    } = starting_points(repo, request)?;
    if request.limit == 0 {
        // `resume` consumed the caller's copy, so `None` would leave no way
        // back.
        return Ok(HistoryPage {
            rows: Vec::new(),
            cursor: Some(HistoryCursor {
                tips,
                order,
                window,
                walked: skip,
            }),
            walked: 0,
            decoded: 0,
        });
    }
    let target = skip.saturating_add(request.limit);

    let mut object_ids = Vec::with_capacity(tips.len());
    for tip in &tips {
        object_ids.push(object_id(tip)?);
    }
    let mut walk = repo
        .inner()
        .rev_walk(object_ids)
        .sorting(order.sorting())
        .all()
        .map_err(|source| Error::Walk {
            source: Box::new(source),
        })?;

    let mut assigner = LaneAssigner::with_window(window);
    let mut page = Page {
        rows: Vec::new(),
        summaries: Vec::new(),
        next_row: 0,
        skip,
    };
    let mut walked = 0usize;
    let mut decoded = 0usize;
    let mut exhausted = false;

    while walked < target {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled { walked });
        }
        let Some(next) = walk.next() else {
            exhausted = true;
            break;
        };
        let info = next.map_err(|source| Error::Walk {
            source: Box::new(source),
        })?;

        let id = model_id(&info.id)?;
        let mut parents = Vec::with_capacity(info.parent_ids.len());
        for parent in info.parent_ids.iter() {
            parents.push(model_id(parent)?);
        }
        if walked >= skip && walked < target {
            page.summaries.push(summary_of(&info, &id, &parents)?);
            decoded += 1;
        }
        walked += 1;

        if let Some(finalised) = assigner.push(id, parents) {
            page.take(finalised);
        }
    }
    for row in assigner.into_rows() {
        page.take(row);
    }

    let cursor = if exhausted {
        None
    } else {
        next_cursor(
            &mut walk,
            HistoryCursor {
                tips,
                order,
                window,
                walked: target,
            },
            cancel,
            walked,
        )?
    };

    Ok(HistoryPage {
        rows: page.rows,
        cursor,
        walked,
        decoded,
    })
}

/// Whether anything remains after `target`. One walk step, no object read.
fn next_cursor(
    walk: &mut gix::revision::Walk<'_>,
    next: HistoryCursor,
    cancel: &impl Cancel,
    so_far: usize,
) -> Result<Option<HistoryCursor>, Error> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled { walked: so_far });
    }
    match walk.next() {
        Some(Ok(_)) => Ok(Some(next)),
        Some(Err(source)) => Err(Error::Walk {
            source: Box::new(source),
        }),
        None => Ok(None),
    }
}

/// The rows this page keeps, and where the assigner's output has got to. Rows
/// leave the assigner in walk order, so `summaries` is indexed by position
/// rather than consumed in order.
struct Page {
    rows: Vec<HistoryRow>,
    summaries: Vec<CommitSummary>,
    next_row: usize,
    skip: usize,
}

impl Page {
    fn take(&mut self, graph: cairn_model::GraphRow) {
        let position = self.next_row;
        self.next_row += 1;
        let Some(offset) = position.checked_sub(self.skip) else {
            return; // Before this page starts.
        };
        let Some(commit) = self.summaries.get(offset) else {
            return; // After it ends: the walk stopped at `target`.
        };
        self.rows.push(HistoryRow {
            content: RowContent::Commit(commit.clone()),
            graph,
        });
    }
}

/// What the request settles before the walk starts.
struct Resolved {
    tips: Vec<Oid>,
    order: HistoryOrder,
    window: usize,
    skip: usize,
}

fn starting_points(repo: &Repository, request: &HistoryRequest) -> Result<Resolved, Error> {
    let tips = match &request.start {
        Start::Resume(cursor) => cursor.tips.clone(),
        Start::Commits(tips) => tips.clone(),
        Start::Head => {
            let mut head = repo.inner().head().map_err(|source| Error::Walk {
                source: Box::new(source),
            })?;
            let peeled = head.try_peel_to_id().map_err(|source| Error::Walk {
                source: Box::new(source),
            })?;
            let Some(id) = peeled else {
                return Err(Error::UnbornHead {
                    path: repo.git_dir().to_owned(),
                });
            };
            vec![model_id(&id)?]
        }
    };
    let skip = match &request.start {
        Start::Resume(cursor) => cursor.walked,
        _ => 0,
    };
    Ok(Resolved {
        tips,
        order: request.order,
        window: request.window,
        skip,
    })
}

fn object_id(oid: &Oid) -> Result<gix::hash::ObjectId, Error> {
    // Both sides hold the digest; no hex round-trip.
    gix::hash::ObjectId::try_from(oid.as_bytes()).map_err(|source| Error::ReadCommit {
        id: oid.to_string(),
        source: Box::new(source),
    })
}

fn model_id(id: &gix::hash::oid) -> Result<Oid, Error> {
    // Hex is built only to name the commit in an error.
    Oid::from_bytes(id.as_bytes()).map_err(|source| Error::ReadCommit {
        id: id.to_hex().to_string(),
        source: Box::new(source),
    })
}

/// The one place a walk step reads a commit object: only the summary and the
/// author need it, the exception R2.3 allows.
fn summary_of(
    info: &gix::revision::walk::Info<'_>,
    id: &Oid,
    parents: &[Oid],
) -> Result<CommitSummary, Error> {
    let commit = info.object().map_err(|source| Error::ReadCommit {
        id: id.to_string(),
        source: Box::new(source),
    })?;
    summary_from(&commit, id, parents)
}

/// The same read from an id: [`HistorySession`] defers it until a row is handed
/// out, by which point the walk has moved on and the `Info` is gone.
fn summary_of_commit(
    repo: &gix::Repository,
    id: &Oid,
    parents: &[Oid],
) -> Result<CommitSummary, Error> {
    let commit = repo
        .find_commit(object_id(id)?)
        .map_err(|source| Error::ReadCommit {
            id: id.to_string(),
            source: Box::new(source),
        })?;
    summary_from(&commit, id, parents)
}

fn summary_from(
    commit: &gix::Commit<'_>,
    id: &Oid,
    parents: &[Oid],
) -> Result<CommitSummary, Error> {
    let read = |source: Box<dyn std::error::Error + Send + Sync>| Error::ReadCommit {
        id: id.to_string(),
        source,
    };
    let message = commit.message().map_err(|e| read(Box::new(e)))?;
    let author = commit.author().map_err(|e| read(Box::new(e)))?;
    let time = author.time().map_err(|e| read(Box::new(e)))?;
    Ok(CommitSummary {
        id: *id,
        parents: parents.to_vec(),
        summary: message.summary().into_owned().to_string(),
        author_name: author.name.to_string(),
        author_email: author.email.to_string(),
        author_time: time.seconds,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::CancelSignal;

    #[test]
    fn a_page_of_this_repository_is_bounded_and_laid_out() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let page = repo
            .history(&HistoryRequest::from_head(3), &CancelSignal::new())
            .unwrap();
        assert_eq!(page.rows.len(), 3, "the limit was not honoured");
        assert!(
            page.cursor.is_some(),
            "this repository has more than three commits"
        );
        assert_eq!(
            page.decoded,
            page.rows.len(),
            "decoded a commit for nothing"
        );
        for row in &page.rows {
            assert_eq!(
                row.id(),
                cairn_model::RowId::Commit(row.graph.id),
                "the two halves named different commits"
            );
            // Exhaustive: the variant `refs-and-status` adds is a compile
            // error here, not a row silently mishandled.
            match &row.content {
                RowContent::Commit(commit) => {
                    assert!(!commit.summary.is_empty(), "a commit with no summary");
                }
            }
        }
    }

    /// Reporter, not a test: what each order costs at each object-cache size,
    /// where [`HistoryOrder::default`]'s figures come from.
    ///
    /// `CAIRN_BENCH_REPO=<path> CAIRN_BENCH_LIMIT=<n> cargo test -p cairn-git
    /// --lib -- --ignored --nocapture measures_both_orders`
    #[test]
    #[ignore = "needs a large repository named by CAIRN_BENCH_REPO"]
    fn measures_both_orders_against_a_named_repository() {
        let path = std::env::var("CAIRN_BENCH_REPO").expect("set CAIRN_BENCH_REPO");
        let limit: usize = std::env::var("CAIRN_BENCH_LIMIT")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(50_000);
        let repo = Repository::discover(&path).unwrap();
        let commit_graph = repo.git_dir().join("objects/info/commit-graph").exists()
            || repo.git_dir().join("objects/info/commit-graphs").exists();
        eprintln!("repository {path}, limit {limit}, commit-graph file present: {commit_graph}");

        for order in [HistoryOrder::GraphOrder, HistoryOrder::CommitTime] {
            for cache in [0usize, 4 * 1024 * 1024, 16 * 1024 * 1024, 32 * 1024 * 1024] {
                for lay_out in [false, true] {
                    let mut inner = repo.inner().clone();
                    inner.object_cache_size(cache);
                    let started = Instant::now();
                    let mut walked = 0usize;
                    let mut walk = inner
                        .rev_walk(vec![inner.head_id().unwrap().detach()])
                        .sorting(order.sorting())
                        .all()
                        .unwrap();
                    let mut assigner = LaneAssigner::new();
                    while walked < limit {
                        let Some(next) = walk.next() else { break };
                        let info = next.unwrap();
                        let id = model_id(&info.id).unwrap();
                        let parents: Vec<Oid> = info
                            .parent_ids
                            .iter()
                            .map(|p| model_id(p).unwrap())
                            .collect();
                        if lay_out {
                            assigner.push(id, parents);
                        }
                        walked += 1;
                    }
                    let what = if lay_out { "walk+lanes" } else { "walk only " };
                    eprintln!(
                        "  {order:?}\tcache={cache:>9}\t{what}\twalked {walked} in {:?}",
                        started.elapsed()
                    );
                }
            }
        }
    }

    /// Reporter, not a test: [`Repository::history`] over every ref, printing
    /// segments, open lanes and retained bytes per row. Findings:
    /// `docs/systems/history-graph.md`; evidence:
    /// `docs/research/history-graph/scroll-memory-model.md`, Part D.
    ///
    /// `CAIRN_BENCH_REPO=<path> cargo test -p cairn-git --release --lib --
    /// --ignored --nocapture measures_layout`
    #[test]
    #[ignore = "needs a repository named by CAIRN_BENCH_REPO"]
    fn measures_layout_over_every_ref_of_a_named_repository() {
        let path = std::env::var("CAIRN_BENCH_REPO").expect("set CAIRN_BENCH_REPO");
        let limit: usize = std::env::var("CAIRN_BENCH_LIMIT")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(1_000_000);
        let repo = Repository::discover(&path).unwrap();
        let tips = every_ref_tip(&repo);
        eprintln!(
            "repository {path}: {} commit-bearing ref tips, limit {limit}, \
             GraphRow {} B fixed + EdgeSegment {} B each",
            tips.len(),
            size_of::<cairn_model::GraphRow>(),
            size_of::<cairn_model::EdgeSegment>(),
        );
        assert!(!tips.is_empty(), "no ref resolved to a commit");

        for order in [HistoryOrder::CommitTime, HistoryOrder::GraphOrder] {
            let request = HistoryRequest::from_commits(tips.clone(), limit).with_order(order);
            let started = Instant::now();
            let page = repo.history(&request, &CancelSignal::new()).unwrap();
            let elapsed = started.elapsed();
            eprintln!(
                "\n  {order:?}: {} rows, walked {}, in {elapsed:?}{}",
                page.rows.len(),
                page.walked,
                if page.cursor.is_some() {
                    " (LIMIT REACHED — history longer than the limit)"
                } else {
                    ""
                },
            );
            report_layout(&page);
        }
    }

    /// Every ref that peels to a commit. Peeling happens inside the iterator,
    /// which holds the packed-refs buffer; a ref peeling elsewhere is dropped.
    fn every_ref_tip(repo: &Repository) -> Vec<Oid> {
        let inner = repo.inner();
        let platform = inner.references().unwrap();
        let peeled: Vec<gix::hash::ObjectId> = platform
            .all()
            .unwrap()
            .peeled()
            .unwrap()
            .filter_map(Result::ok)
            .map(|reference| reference.id().detach())
            .collect();
        let mut tips = Vec::new();
        for id in peeled {
            let Ok(object) = inner.find_object(id) else {
                continue;
            };
            if object.kind == gix::object::Kind::Commit && !tips.contains(&id) {
                tips.push(id);
            }
        }
        tips.iter().map(|id| model_id(id).unwrap()).collect()
    }

    /// Segments, open lanes and retained bytes per row. Bytes come from
    /// `edges.len()` while a `GraphRow` retains `edges.capacity()`, so real
    /// retained layout is up to about twice this.
    fn report_layout(page: &HistoryPage) {
        let mut segments = Vec::with_capacity(page.rows.len());
        let mut open_lanes = Vec::with_capacity(page.rows.len());
        let mut bytes = Vec::with_capacity(page.rows.len());
        let mut out_of_order_rows = 0usize;
        let mut out_of_order_segments = 0usize;
        let mut widest_lane = 0usize;

        for row in &page.rows {
            let graph = &row.graph;
            segments.push(graph.edges.len());
            bytes.push(
                size_of::<cairn_model::GraphRow>()
                    + graph.edges.len() * size_of::<cairn_model::EdgeSegment>(),
            );

            // Open = occupied by the node or either end of a segment.
            let mut lanes = vec![graph.lane.index()];
            for edge in &graph.edges {
                for lane in [edge.from.index(), edge.to.index()] {
                    if !lanes.contains(&lane) {
                        lanes.push(lane);
                    }
                    widest_lane = widest_lane.max(lane);
                }
            }
            open_lanes.push(lanes.len());

            let flagged = graph.edges.iter().filter(|e| e.out_of_order).count();
            out_of_order_segments += flagged;
            if flagged > 0 {
                out_of_order_rows += 1;
            }
        }

        describe("segments/row ", &mut segments);
        describe("open lanes/row", &mut open_lanes);
        describe("bytes/row     ", &mut bytes);

        let rows = page.rows.len().max(1);
        eprintln!(
            "    out-of-order: {out_of_order_rows} rows ({:.3}%), {out_of_order_segments} segments",
            100.0 * out_of_order_rows as f64 / rows as f64,
        );
        eprintln!("    highest lane number used: {widest_lane}");

        let p99 = percentile(&bytes, 0.99);
        for commits in [10_000usize, 100_000, 500_000] {
            let total = p99 * commits;
            eprintln!(
                "    at p99 {p99} B/row x {commits} commits = {:.1} MB retained layout",
                total as f64 / (1024.0 * 1024.0),
            );
        }
    }

    fn describe(label: &str, values: &mut [usize]) {
        if values.is_empty() {
            eprintln!("    {label}: no rows");
            return;
        }
        values.sort_unstable();
        let sum: usize = values.iter().sum();
        eprintln!(
            "    {label}: mean {:.2}  p50 {}  p95 {}  p99 {}  max {}",
            sum as f64 / values.len() as f64,
            percentile(values, 0.50),
            percentile(values, 0.95),
            percentile(values, 0.99),
            values.last().copied().unwrap_or(0),
        );
    }

    /// Nearest-rank percentile over an already-sorted slice.
    fn percentile(sorted: &[usize], fraction: f64) -> usize {
        if sorted.is_empty() {
            return 0;
        }
        let last = sorted.len() - 1;
        let index = ((last as f64) * fraction).round() as usize;
        sorted.get(index.min(last)).copied().unwrap_or(0)
    }
}
