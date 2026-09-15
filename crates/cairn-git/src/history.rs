//! Reading a page of history and laying it out.
//!
//! The query is bounded by construction: it takes a limit and, to continue,
//! a [`HistoryCursor`] from the page before it. There is deliberately no call
//! that walks to the root — a repository is somebody's ten-year monorepo, and
//! an unbounded read of one is a hang wearing a function signature.
//!
//! Parent ids come from the walk itself, never from a decoded commit object.
//! The object is read only for the rows the page actually returns, and only
//! because the summary line and the author genuinely live in it;
//! `Info::object()` is documented as expensive and is treated that way.
//! [`HistoryPage::walked`] and [`HistoryPage::decoded`] report the difference,
//! so the claim is something a test can check rather than a comment.

mod session;

use cairn_model::{CommitSummary, HistoryRow, LaneAssigner, Oid};

pub use session::HistorySession;

use crate::{Cancel, Error, Repository};

/// The order commits come back in.
///
/// Neither order is topological — gitoxide has no `--topo-order` equivalent —
/// so either can hand over a parent before its child when committer dates are
/// skewed. The lane assigner is total over that, by design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryOrder {
    /// Newest committer date first, across everything queued. What a history
    /// list wants: the order a human reads the log in.
    CommitTime,
    /// The order the commit graph mentions them in — cheaper, because nothing
    /// has to be sorted, but branches interleave in a way a reader will not
    /// recognise as chronology.
    GraphOrder,
}

impl Default for HistoryOrder {
    /// Commit time, because it is both the readable order and — once the
    /// repository carries the object cache [`Repository::discover`] installs —
    /// the cheaper one end to end. Measured over the same 50k commits of a
    /// repository with no commit-graph file: walking *and laying out* took
    /// 129 ms by commit time against 196 ms in graph order, because graph order
    /// interleaves branches and leaves far more lanes open per row. (Walking
    /// alone is the other way round — 116 ms against 101 ms — which is why the
    /// two numbers quoted here and on [`Repository::discover`] differ.) Open
    /// question O2, `docs/work/history-graph/progress.md`.
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

/// Where a page of history picks up from.
///
/// Opaque on purpose: it records the resolved starting points, the order, the
/// window, and how far the walk had got, and how it does that is free to
/// change. Hand it back to [`HistoryRequest::resume`] and nothing else.
///
/// It carries the order and the window because both decide lane numbering, and
/// a page that quietly changed either would renumber lanes the caller has
/// already drawn — the one thing R1.2 does not allow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCursor {
    tips: Vec<Oid>,
    order: HistoryOrder,
    window: usize,
    /// Commits laid out by every page so far.
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

/// What to read: where from, how they are ordered, and how many.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRequest {
    start: Start,
    order: HistoryOrder,
    limit: usize,
    window: usize,
}

impl HistoryRequest {
    /// Start at whatever `HEAD` resolves to.
    pub fn from_head(limit: usize) -> Self {
        Self::starting(Start::Head, limit)
    }

    /// Start at the given commits — a set of branch tips, typically.
    pub fn from_commits(tips: impl IntoIterator<Item = Oid>, limit: usize) -> Self {
        Self::starting(Start::Commits(tips.into_iter().collect()), limit)
    }

    /// Continue where the page that produced `cursor` stopped. The cursor
    /// carries the starting points, the order and the window, so none of them
    /// is asked for again and none of them can drift between pages.
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

    /// Order the walk differently. Ignored when resuming: the cursor's order is
    /// what its rows were laid out in, and changing it mid-history would
    /// renumber lanes the caller has already drawn.
    pub fn with_order(mut self, order: HistoryOrder) -> Self {
        if !matches!(self.start, Start::Resume(_)) {
            self.order = order;
        }
        self
    }

    /// How many rows the lane assigner holds before it makes them final. The
    /// default is [`LaneAssigner::DEFAULT_WINDOW`]; widen it for a repository
    /// whose committer dates are further out than that.
    ///
    /// Ignored when resuming, for the same reason [`Self::with_order`] is: the
    /// window decides which parents can still be joined to their children, so
    /// changing it mid-history would renumber lanes already drawn.
    pub fn with_window(mut self, window: usize) -> Self {
        if !matches!(self.start, Start::Resume(_)) {
            self.window = window;
        }
        self
    }
}

/// One page of history, and how to ask for the next one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPage {
    /// The commits, newest first, each with the lane and edges that draw it.
    ///
    /// Lane indices are the same however the history was paged. Edges are not
    /// quite: a line back to a parent the walk delivered early is drawn by the
    /// page that reaches the *child*, and its upper end lands on a row an
    /// earlier page already returned. Pages never overlap, so that upper end is
    /// computed during the replay and then dropped — a backward line spanning a
    /// page boundary reaches the view as the lower half only, flagged
    /// [`cairn_model::EdgeSegment::out_of_order`]. Reading the history in one
    /// page draws it whole. Nothing here is wrong in the picture, but a view
    /// must not assume a flagged segment has a visible other end.
    pub rows: Vec<HistoryRow>,
    /// Where the next page starts, or `None` because this page reached the end
    /// of the history. A request with a limit of zero reads nothing and always
    /// hands back a cursor at the position it was given, since it learned
    /// nothing about where the history ends.
    pub cursor: Option<HistoryCursor>,
    /// Commits laid out to produce this page, including the ones before its
    /// first row. Larger than `rows.len()` on a resumed page: resuming replays
    /// the walk that led here.
    pub walked: usize,
    /// Commit objects actually read. One per returned row and not one more —
    /// the replayed prefix costs a walk step each, never an object.
    pub decoded: usize,
}

impl Repository {
    /// Read one page of history, laid out in lanes.
    ///
    /// `cancel` is polled once per commit visited; when it fires the walk stops
    /// and [`Error::Cancelled`] says how far it had got. Pass a
    /// [`crate::CancelSignal`] that is never set for a query nobody will
    /// abandon.
    ///
    /// Resuming replays the walk from the same starting points and skips what
    /// earlier pages covered, so two pages of `n` describe the same commits in
    /// the same lanes as one page of `2n`. The cost is that page `k` walks
    /// `k x n` commits; it decodes only the `n` it returns.
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
        // Nothing to read, so nothing is walked and nothing is claimed about
        // where the history ends. The cursor points at the same place the
        // request came in at, because `resume` consumes the caller's copy and
        // returning `None` would leave them no way back.
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

/// Whether anything remains after `target`, and therefore whether there is a
/// next page at all. Costs one walk step and no object read; a cursor handed
/// out for a page that turns out to be empty is a worse trade.
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

/// The rows this page keeps, and where the assigner's output has got to.
///
/// Rows leave the assigner in walk order — the ones the window made final
/// first, then the ones still inside it — so counting them gives each row its
/// position in the walk. `summaries` is indexed by that position rather than
/// consumed in order, so a row and its commit are matched by where they sit,
/// not by two sequences staying in step.
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
            commit: commit.clone(),
            graph,
        });
    }
}

/// Everything the request settles before the walk starts.
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
    // Both sides hold the digest itself, so crossing the seam is a copy of
    // twenty or thirty-two bytes rather than a round trip through hex.
    gix::hash::ObjectId::try_from(oid.as_bytes()).map_err(|source| Error::ReadCommit {
        id: oid.to_string(),
        source: Box::new(source),
    })
}

fn model_id(id: &gix::hash::oid) -> Result<Oid, Error> {
    // The hex form is built only to name the commit in an error, which is a
    // path this never takes for a digest gitoxide itself produced.
    Oid::from_bytes(id.as_bytes()).map_err(|source| Error::ReadCommit {
        id: id.to_hex().to_string(),
        source: Box::new(source),
    })
}

/// The one place a commit object is read. Everything a layout needs — the id
/// and the parent ids — came off the walk; the summary line and the author do
/// not exist outside the object, which is the exception R2.3 allows.
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

/// The same read, reached from an id rather than from a walk step.
///
/// [`HistorySession`] needs this: it defers the object read until a row is
/// about to be handed out, by which point the walk has moved on and the `Info`
/// is gone. The id and the parent ids still came off the walk, which is what
/// R2.3 is about.
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
                &row.graph.id,
                "the two halves named different commits"
            );
            assert!(!row.commit.summary.is_empty(), "a commit with no summary");
        }
    }

    /// Open question O2: gitoxide's docs say `ByCommitTime` "benefits greatly"
    /// from an object cache, implying the unaccelerated path looks each commit
    /// up twice. This is the harness that measured it — ignored by default
    /// because it needs a repository worth measuring, and because a timing
    /// assertion in the gate would be flaky within a week.
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
}
