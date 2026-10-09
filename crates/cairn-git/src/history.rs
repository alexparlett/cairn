//! History requests, cursors and pages.
//!
//! A walk starts from `HEAD`, from given commits, or from a refs snapshot — every branch,
//! remote-tracking ref, tag and `HEAD` (PRD R4.1) — whose rows then carry the refs that
//! label them (R4.3) and whose stashes are rows of their own (R4.2; `stream`).

mod reach;
mod reflogs;
mod seeds;
mod session;
mod stream;
pub(crate) mod walk;

use std::sync::Arc;

use cairn_model::{
    GraphRow, LaneAssigner, Oid, PagedCommit, PagedStash, RefName, RefsSnapshot, RowsPage,
};

pub use session::HistorySession;

use reach::Reach;
use seeds::{Decoration, RefSeeds};
use stream::{Entry, Next, Stream};

use crate::commit_encoding::CommitEncoding;
use crate::object_id::{model_id, object_id};
use crate::{Cancel, Error, Repository};

/// Neither order is topological: a parent can arrive before its child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryOrder {
    /// Newest committer date first.
    CommitTime,
    /// Unsorted, as the commit graph mentions them; branches interleave.
    GraphOrder,
}

impl Default for HistoryOrder {
    /// Commit time: cheaper than graph order once lanes are laid out.
    fn default() -> Self {
        Self::CommitTime
    }
}

impl HistoryOrder {
    fn sorting(self) -> gix::traverse::commit::simple::Sorting {
        use gix::traverse::commit::simple::{CommitTimeOrder, Sorting};
        match self {
            Self::CommitTime => Sorting::ByCommitTime(CommitTimeOrder::NewestFirst),
            Self::GraphOrder => Sorting::BreadthFirst,
        }
    }
}

/// Shared, so paging clones a pointer rather than the tip set.
type Tips = Arc<[gix::hash::ObjectId]>;

/// Whether a walk marks the commits no ref reaches — Show Lost Commits
/// (`docs/prd/staging-and-commit.md` R11.1) — and where it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Marking {
    /// Every commit is reached: the walk's tips are the refs'.
    Off,
    /// The reflogs of `HEAD` and of these local branches are read as the walk opens, under
    /// its cancel, and their ids join the tips.
    Unread(Arc<[RefName]>),
    /// The first `refs` tips are the ones refs name; the rest are ids only reflogs name.
    Read { refs: usize },
}

impl Marking {
    /// What a walk from `tips`, resolved under this marking, decides reachability with.
    fn reach(&self, tips: &[gix::hash::ObjectId]) -> Result<Option<Reach>, Error> {
        let Self::Read { refs } = self else {
            return Ok(None);
        };
        let (refs, reflogs) = tips.split_at((*refs).min(tips.len()));
        let ids = |tips: &[gix::hash::ObjectId]| -> Result<Vec<Oid>, Error> {
            tips.iter().map(|id| model_id(id)).collect()
        };
        Ok(Some(Reach::new(&ids(refs)?, &ids(reflogs)?)))
    }
}

/// The walk `tips` begin under `marking`: an unread marking has its reflogs read first, under
/// `cancel`, and the ids they name that are not tips already joined to the tips after the
/// refs' own. `None` when `cancel` fired; nothing is kept.
fn open_stream<'repo>(
    repo: &'repo gix::Repository,
    tips: &mut Tips,
    marking: &mut Marking,
    decoration: &Arc<Decoration>,
    order: HistoryOrder,
    lookahead: usize,
    cancel: &impl Cancel,
) -> Result<Option<Stream<'repo>>, Error> {
    if let Marking::Unread(branches) = marking {
        let Some(reflog_tips) = reflogs::reflog_tips(repo, branches, cancel)? else {
            return Ok(None);
        };
        let refs = tips.len();
        let mut all = tips.to_vec();
        all.extend(reflog_tips.into_iter().filter(|id| !tips.contains(id)));
        *tips = all.into();
        *marking = Marking::Read { refs };
    }
    let reach = marking.reach(tips)?;
    let Some(walk) = walk::open(repo, tips, order, cancel)? else {
        return Ok(None);
    };
    Ok(Some(Stream::new(
        walk,
        Arc::clone(decoration),
        lookahead,
        reach,
    )))
}

/// Opaque: hand it back to [`HistoryRequest::resume`] and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCursor {
    /// Resolved once, when the walk began; every page of it starts here, wherever refs are now.
    tips: Tips,
    /// The labels and stashes of the snapshot the walk began from, likewise.
    decoration: Arc<Decoration>,
    /// Which of `tips` refs name, when the walk marks the commits no ref reaches.
    marking: Marking,
    order: HistoryOrder,
    window: usize,
    lookahead: usize,
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
    Refs(Box<RefSeeds>),
    Resume(HistoryCursor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRequest {
    start: Start,
    /// Show Lost Commits: the reflogs' ids join the tips (R11.1).
    lost: bool,
    order: HistoryOrder,
    limit: usize,
    window: usize,
    lookahead: usize,
}

impl HistoryRequest {
    pub fn from_head(limit: usize) -> Self {
        Self::starting(Start::Head, limit)
    }

    pub fn from_commits(tips: impl IntoIterator<Item = Oid>, limit: usize) -> Self {
        Self::starting(Start::Commits(tips.into_iter().collect()), limit)
    }

    /// Every commit the snapshot's local branches, remote-tracking refs and tags identify,
    /// and `HEAD`'s — git's `rev-list --branches --remotes --tags HEAD` — each row carrying
    /// the refs that point at it and whether it is `HEAD`'s, and each stash whose base the
    /// walk reaches a row of its own (PRD R4.1-R4.3). A tag on a tree or a blob seeds
    /// nothing; nothing about a stash seeds the walk.
    pub fn from_refs(snapshot: &RefsSnapshot, limit: usize) -> Self {
        Self::starting(Start::Refs(Box::new(RefSeeds::of(snapshot))), limit)
    }

    /// Continues where the page that produced `cursor` stopped.
    pub fn resume(cursor: HistoryCursor, limit: usize) -> Self {
        let (order, window, lookahead) = (cursor.order, cursor.window, cursor.lookahead);
        Self {
            start: Start::Resume(cursor),
            lost: false,
            order,
            limit,
            window,
            lookahead,
        }
    }

    fn starting(start: Start, limit: usize) -> Self {
        Self {
            start,
            lost: false,
            order: HistoryOrder::default(),
            limit,
            window: LaneAssigner::DEFAULT_WINDOW,
            lookahead: stream::LOOKAHEAD,
        }
    }

    /// How many commits the walk looks ahead, when a stash's date comes up, for the commit
    /// it was made on before drawing it directly above that commit instead (`stream`).
    /// Zero is raised to one. Ignored when resuming.
    pub fn with_stash_lookahead(mut self, commits: usize) -> Self {
        if !matches!(self.start, Start::Resume(_)) {
            self.lookahead = commits.max(1);
        }
        self
    }

    /// Show Lost Commits (`docs/prd/staging-and-commit.md` R11.1): a walk from refs also
    /// starts from the old and the new id of every entry of `HEAD`'s reflog and of each
    /// local branch's — as `git rev-list --reflog` reads them, each log read whole — and
    /// each row of a commit no ref reaches is carried as lost ([`RowsPage::push_lost`]), or,
    /// when the walk finds a ref reaches it only after its row was carried, named reached by
    /// a later page ([`RowsPage::reached`]). The reflogs are read when the walk opens, under
    /// the first page's cancel. Ignored unless the walk starts from refs; a resumed walk
    /// keeps what its cursor began with.
    pub fn with_lost_commits(mut self) -> Self {
        self.lost = true;
        self
    }

    /// Ignored when resuming: the cursor carries the order.
    pub fn with_order(mut self, order: HistoryOrder) -> Self {
        if !matches!(self.start, Start::Resume(_)) {
            self.order = order;
        }
        self
    }

    /// Rows the lane assigner holds before making them final. Ignored when resuming.
    pub fn with_window(mut self, window: usize) -> Self {
        if !matches!(self.start, Start::Resume(_)) {
            self.window = window;
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPage {
    /// Lane indices do not depend on paging; edges do. A backward line crossing a page
    /// boundary arrives as its lower half only, still flagged out of order. Read by
    /// appending to a [`cairn_model::History`].
    pub rows: RowsPage,
    /// `None` at the end of the history; a zero limit hands back the given cursor.
    pub cursor: Option<HistoryCursor>,
    /// Rows laid out for this page — commits and stashes — including a replayed prefix.
    pub walked: usize,
    /// Commit objects read: one per returned commit's row. A stash's row reads none: its
    /// commit was read when the walk opened.
    pub decoded: usize,
}

impl Repository {
    /// `cancel` is polled once per row laid out, per commit looked ahead at, and per tip
    /// read to open the walk but the first (`walk::open`). Resuming replays the walk, so
    /// page `k` walks `k x limit` rows.
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
        mut tips,
        decoration,
        mut marking,
        order,
        window,
        lookahead,
        skip,
    } = starting_points(repo, request)?;
    if request.limit == 0 {
        // `resume` consumed the caller's cursor; hand it back.
        return Ok(HistoryPage {
            rows: RowsPage::new(),
            cursor: Some(HistoryCursor {
                tips,
                decoration,
                marking,
                order,
                window,
                lookahead,
                walked: skip,
            }),
            walked: 0,
            decoded: 0,
        });
    }
    let Some(mut stream) = open_stream(
        repo.inner(),
        &mut tips,
        &mut marking,
        &decoration,
        order,
        lookahead,
        cancel,
    )?
    else {
        return Err(Error::Cancelled { walked: 0 });
    };
    let cursor_at = |walked: usize| HistoryCursor {
        tips: Arc::clone(&tips),
        decoration: Arc::clone(&decoration),
        marking: marking.clone(),
        order,
        window,
        lookahead,
        walked,
    };
    let target = skip.saturating_add(request.limit);

    // The page's first row carries a snapshot, so it draws without the replayed prefix.
    let mut assigner = LaneAssigner::with_window(window).drawn_from(skip);
    let mut page = Page {
        rows: RowsPage::new(),
        laid: Vec::new(),
        next_row: 0,
        skip,
        decoration: &decoration,
    };
    let mut walked = 0usize;
    let mut decoded = 0usize;
    let mut exhausted = false;

    while walked < target {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled { walked });
        }
        let entry = match stream.next(cancel)? {
            Next::Entry(entry) => entry,
            Next::End => {
                exhausted = true;
                break;
            }
            Next::Cancelled => return Err(Error::Cancelled { walked }),
        };
        if walked >= skip && walked < target {
            let laid = Laid::read(repo.inner(), Pending::of(&entry))?;
            if matches!(laid, Laid::Commit { .. }) {
                decoded += 1;
            }
            page.laid.push(laid);
        }
        walked += 1;

        if let Some(finalised) = lay_out(&mut assigner, &decoration, entry) {
            page.take(finalised, &stream);
        }
    }
    for row in assigner.into_rows() {
        page.take(row, &stream);
    }
    for (row, id) in stream.take_reached(skip + page.rows.len()) {
        page.rows.reached(row, id);
    }

    let cursor = if exhausted {
        None
    } else {
        next_cursor(&mut stream, cursor_at(target), cancel, walked)?
    };

    Ok(HistoryPage {
        rows: page.rows,
        cursor,
        walked,
        decoded,
    })
}

/// `entry` laid out, returning the row it forced out of the window: a stash's with its one
/// line to its base, in a lane of its own ([`LaneAssigner::push_stash`]).
fn lay_out(assigner: &mut LaneAssigner, decoration: &Decoration, entry: Entry) -> Option<GraphRow> {
    match entry {
        Entry::Commit { id, parents } => assigner.push(id, parents),
        Entry::Stash(at) => {
            let stash = decoration.stashes().get(at)?;
            assigner.push_stash(stash.id, stash.base)
        }
    }
}

/// What a laid-out row draws, read once and copied into a page.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Laid {
    Commit {
        id: Oid,
        commit: Commit,
    },
    /// The stash at this index of the walk's decoration, read when the walk opened.
    Stash(usize),
}

/// An entry laid out and not yet read: a commit by its id and how many parents the walk
/// read for it, or a stash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Commit { id: Oid, parents: usize },
    Stash(usize),
}

impl Pending {
    fn of(entry: &Entry) -> Self {
        match entry {
            Entry::Commit { id, parents } => Self::Commit {
                id: *id,
                parents: parents.len(),
            },
            Entry::Stash(at) => Self::Stash(*at),
        }
    }
}

impl Laid {
    /// A commit's row reads its commit; a stash's reads nothing.
    fn read(repo: &gix::Repository, pending: Pending) -> Result<Self, Error> {
        Ok(match pending {
            Pending::Commit { id, parents } => Self::Commit {
                id,
                commit: summary_of_commit(repo, &id, parents)?,
            },
            Pending::Stash(at) => Self::Stash(at),
        })
    }

    /// Appends the row `graph` lays out for this to `rows`, with its labels — or, for a
    /// commit no ref reaches as `stream` stands now, as lost.
    fn push_to(
        &self,
        rows: &mut RowsPage,
        graph: GraphRow,
        decoration: &Decoration,
        stream: &Stream<'_>,
    ) {
        match self {
            Self::Commit { id, commit } if stream.is_lost(id) => {
                rows.push_lost(graph, commit.paged());
            }
            Self::Commit { id, commit } => rows.push_labelled(
                graph,
                commit.paged(),
                decoration.is_head(id),
                &decoration.labels_of(id),
            ),
            Self::Stash(at) => {
                if let Some(stash) = decoration.stashes().get(*at) {
                    rows.push_stash(
                        graph,
                        PagedStash {
                            index: stash.index,
                            base: stash.base,
                            message: &stash.message,
                            author: &stash.author,
                            author_time: stash.author_time,
                        },
                    );
                }
            }
        }
    }
}

/// Whether anything remains after `target`: one entry of the stream, no object read.
fn next_cursor(
    stream: &mut Stream<'_>,
    next: HistoryCursor,
    cancel: &impl Cancel,
    so_far: usize,
) -> Result<Option<HistoryCursor>, Error> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelled { walked: so_far });
    }
    match stream.next(cancel)? {
        Next::Entry(_) => Ok(Some(next)),
        Next::End => Ok(None),
        Next::Cancelled => Err(Error::Cancelled { walked: so_far }),
    }
}

/// `laid` is indexed by position, not consumed in order.
struct Page<'d> {
    rows: RowsPage,
    laid: Vec<Laid>,
    next_row: usize,
    skip: usize,
    decoration: &'d Decoration,
}

impl Page<'_> {
    fn take(&mut self, graph: GraphRow, stream: &Stream<'_>) {
        let position = self.next_row;
        self.next_row += 1;
        let Some(offset) = position.checked_sub(self.skip) else {
            return; // Before this page starts.
        };
        let Some(laid) = self.laid.get(offset) else {
            return; // After it ends: the walk stopped at `target`.
        };
        laid.push_to(&mut self.rows, graph, self.decoration, stream);
    }
}

/// What a row draws of its commit, read once by the walk and copied into a page.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Commit {
    parents: usize,
    subject: String,
    author: String,
    author_time: i64,
}

impl Commit {
    fn paged(&self) -> PagedCommit<'_> {
        PagedCommit {
            parents: self.parents,
            subject: &self.subject,
            author: &self.author,
            author_time: self.author_time,
        }
    }
}

/// The parents the walk read, which are the ones git shows (`walk`).
fn parents_of(info: &gix::traverse::commit::Info) -> Result<Vec<Oid>, Error> {
    info.parent_ids
        .iter()
        .map(|parent| model_id(parent))
        .collect()
}

struct Resolved {
    tips: Tips,
    decoration: Arc<Decoration>,
    marking: Marking,
    order: HistoryOrder,
    window: usize,
    lookahead: usize,
    skip: usize,
}

fn starting_points(repo: &Repository, request: &HistoryRequest) -> Result<Resolved, Error> {
    let mut decoration = None;
    let mut marking = Marking::Off;
    let tips = match &request.start {
        Start::Resume(cursor) => {
            decoration = Some(Arc::clone(&cursor.decoration));
            marking = cursor.marking.clone();
            Arc::clone(&cursor.tips)
        }
        Start::Commits(tips) => walk_tips(repo.inner(), tips)?,
        Start::Refs(seeds) => {
            let (tips, resolved) = seeds::resolve(repo.inner(), seeds)?;
            decoration = Some(Arc::new(resolved));
            if request.lost {
                marking = Marking::Unread(Arc::clone(seeds.branches()));
            }
            walk_tips(repo.inner(), &tips)?
        }
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
            Arc::from([id.detach()])
        }
    };
    let skip = match &request.start {
        Start::Resume(cursor) => cursor.walked,
        _ => 0,
    };
    Ok(Resolved {
        tips,
        decoration: decoration.unwrap_or_default(),
        marking,
        order: request.order,
        window: request.window,
        lookahead: request.lookahead,
        skip,
    })
}

fn walk_tips(repo: &gix::Repository, tips: &[Oid]) -> Result<Tips, Error> {
    let format = repo.object_hash();
    let mut object_ids = Vec::with_capacity(tips.len());
    for tip in tips {
        let id = object_id(tip)?;
        if id.kind() != format {
            return Err(Error::Walk {
                source: format!(
                    "{tip} is a {} id, but this repository names objects by {format}",
                    id.kind()
                )
                .into(),
            });
        }
        object_ids.push(id);
    }
    Ok(object_ids.into())
}

/// The one place a walk step reads a commit object, by id: the walk hands over ids only,
/// and `parents` counts the parents it read for it (`parents_of`).
fn summary_of_commit(repo: &gix::Repository, id: &Oid, parents: usize) -> Result<Commit, Error> {
    let commit = repo
        .find_commit(object_id(id)?)
        .map_err(|source| Error::ReadCommit {
            id: id.to_string(),
            source: Box::new(source),
        })?;
    summary_from(&commit, id, parents)
}

fn summary_from(commit: &gix::Commit<'_>, id: &Oid, parents: usize) -> Result<Commit, Error> {
    let read = |source: Box<dyn std::error::Error + Send + Sync>| Error::ReadCommit {
        id: id.to_string(),
        source,
    };
    // Decoded once: the encoding header, the author and the message are all read from it.
    let decoded = commit.decode().map_err(|e| read(Box::new(e)))?;
    // The characters git shows, as the Commit tab reads them (`crate::commit_encoding`).
    let encoding = CommitEncoding::of_commit(commit, &decoded);
    let author = decoded.author().map_err(|e| read(Box::new(e)))?;
    let time = author.time().map_err(|e| read(Box::new(e)))?;
    Ok(Commit {
        parents,
        subject: encoding.text(&decoded.message().summary()),
        author: encoding.text(author.name),
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
        let ids: Vec<Oid> = page.rows.ids().collect();
        let mut history = cairn_model::History::new();
        history.append(page.rows).unwrap();
        for (row, id) in history.rows().zip(ids) {
            assert_eq!(row.id(), cairn_model::RowId::Commit(id));
            // No wildcard arm: a new variant must fail to compile here.
            match row.content() {
                cairn_model::RowContent::Commit(commit) => {
                    assert_eq!(commit.id, id, "the row's content is another commit's");
                    assert!(!commit.summary.is_empty(), "a commit with no summary");
                }
                cairn_model::RowContent::Stash(stash) => {
                    panic!("a walk from HEAD drew a stash: {stash:?}")
                }
            }
        }
    }

    /// Caught by: copying and converting every tip again on each page of a cold walk.
    #[test]
    fn resuming_reuses_the_cursors_resolved_tips() {
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let page = repo
            .history(&HistoryRequest::from_head(2), &CancelSignal::new())
            .unwrap();
        let cursor = page.cursor.unwrap();
        let request = HistoryRequest::resume(cursor.clone(), 2);

        let resolved = starting_points(&repo, &request).unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&resolved.tips, &cursor.tips),
            "the resumed page built its own copy of the tips"
        );
        let next = repo.history(&request, &CancelSignal::new()).unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&next.cursor.unwrap().tips, &cursor.tips),
            "the next cursor did not carry the same tips on"
        );

        let mut session = repo.history_session(&request).unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&session.cursor().tips, &cursor.tips),
            "a session resumed from the cursor built its own copy of the tips"
        );
        let paged = session.next_page(2, &CancelSignal::new()).unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&paged.cursor.unwrap().tips, &cursor.tips),
            "a session's page did not carry the same tips on"
        );
    }

    /// Reporter. Env: `CAIRN_BENCH_REPO`, `CAIRN_BENCH_LIMIT`.
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
                    let head = inner.head_id().unwrap().detach();
                    let mut walk = walk::open(&inner, &[head], order, &CancelSignal::new())
                        .unwrap()
                        .expect("an uncancelled open was cancelled");
                    let mut assigner = LaneAssigner::new();
                    while walked < limit {
                        let Some(next) = walk.next() else { break };
                        let info = next.unwrap();
                        let id = model_id(&info.id).unwrap();
                        let parents: Vec<Oid> = parents_of(&info).unwrap();
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

    /// Reporter. Env: `CAIRN_BENCH_REPO`; run with `--release`.
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
             GraphRow {} B fixed + LaneChange {} B each",
            tips.len(),
            size_of::<cairn_model::GraphRow>(),
            size_of::<cairn_model::LaneChange>(),
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
            report_layout(page);
        }
    }

    /// Peels inside the iterator, which holds the packed-refs buffer; peeling elsewhere drops the ref.
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

    /// Segments are the derived edges each row draws; bytes are what the page's rows retain
    /// once appended to a history, every store by capacity.
    fn report_layout(page: HistoryPage) {
        let mut history = cairn_model::History::new();
        history.append(page.rows).unwrap();
        let mut segments = Vec::with_capacity(history.len());
        let mut open_lanes = Vec::with_capacity(history.len());
        let mut out_of_order_rows = 0usize;
        let mut out_of_order_segments = 0usize;
        let mut widest_lane = 0usize;
        // Rows with no snapshot within reach: the list would draw their node alone.
        let mut underived = 0usize;

        for row in history.rows() {
            let edges = match row.edges() {
                Some(drawn) => drawn.edges,
                None => {
                    underived += 1;
                    Vec::new()
                }
            };
            segments.push(edges.len());

            // Open = occupied by the node or either end of a segment.
            let mut lanes = vec![row.lane().index()];
            for edge in &edges {
                for lane in [edge.from.index(), edge.to.index()] {
                    if !lanes.contains(&lane) {
                        lanes.push(lane);
                    }
                    widest_lane = widest_lane.max(lane);
                }
            }
            open_lanes.push(lanes.len());

            let flagged = edges.iter().filter(|e| e.out_of_order).count();
            out_of_order_segments += flagged;
            if flagged > 0 {
                out_of_order_rows += 1;
            }
        }

        describe("segments/row ", &mut segments);
        describe("open lanes/row", &mut open_lanes);

        let rows = history.len().max(1);
        eprintln!(
            "    out-of-order: {out_of_order_rows} rows ({:.3}%), {out_of_order_segments} segments",
            100.0 * out_of_order_rows as f64 / rows as f64,
        );
        eprintln!("    highest lane number used: {widest_lane}");
        eprintln!("    rows whose edges could not be derived: {underived}");
        let retained = history.retained();
        eprintln!(
            "    retained {:.1} MiB, {} B/row: {retained:?}",
            retained.total() as f64 / (1024.0 * 1024.0),
            retained.total() / rows,
        );
        assert_eq!(
            underived, 0,
            "{underived} rows had no snapshot within reach, so their edges were not derived"
        );
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
