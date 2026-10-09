//! Requests and updates crossing the worker boundary.
//!
//! A query is numbered in its lane ([`QueryLane`]) and every diff answer names what it
//! answers — the commit or the pair, the file, the options — so the window keeps an answer
//! only for the selection it names (PRD R4.4).

use std::path::PathBuf;
use std::sync::Arc;

use cairn_model::ShownDiff;
use cairn_model::{
    AheadBehind, ChangeSet, ChangedFile, CommandRecord, Consequence, Context, Disclosure, FileDiff,
    History, LocalChanges, MatchedRows, Oid, RefName, RefsSnapshot, RemoteSummary, RepoPath,
    RowsPage, Selection, SidebarRow,
};

use super::askpass::PromptId;
use super::epoch::QueryLane;
use super::local_lane::{LocalWrite, OperationId, ReadAgain, WriteEnding};

/// What a changes query compares (R2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    /// One commit against its first parent (the empty tree for a root commit).
    Commit(Oid),
    /// Two commits, tip against tip, `old` the base (R7.2).
    Between { old: Oid, new: Oid },
    /// A stash commit, as `git stash show` lists it (refs-and-status R6.2): against the
    /// commit it was made on, with its untracked files where the user's
    /// `stash.showIncludeUntracked` says so.
    Stash(Oid),
}

/// Which of a path's working-tree diffs (R3.1): what a path chosen in Local Changes is asked
/// as, by the list it was chosen in (refs-and-status R9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkingSide {
    /// `HEAD` against the index: `git diff --cached`.
    Staged,
    /// The index against the working tree: `git diff`.
    Unstaged,
    /// Nothing against the working tree, for a path git does not track.
    Untracked,
}

/// What a view asks of one file's diff — every option an answer depends on that the view
/// chooses. R2.6's ceilings are not among them: they are fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffOptions {
    /// The context the view groups hunks at, which git is asked at (so a change of context
    /// is a new query: git's function context is the text its hunks at that context carry).
    pub context: Context,
    /// Compute the whitespace-ignoring ranges (R2.8).
    pub ignore_whitespace: bool,
    /// Read a file past R2.6's byte ceiling anyway, up to its load-anyway ceiling.
    pub load_anyway: bool,
}

/// The file a diff is of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTarget {
    /// One file of `of`'s change set, exactly as that answer named it.
    Committed { of: Comparison, file: ChangedFile },
    /// One path's diff in the working tree: a path chosen in Local Changes.
    WorkingTree { path: RepoPath, side: WorkingSide },
}

/// One file's diff as asked: what its answer names, and what the window compares with
/// what it has selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileQuery {
    pub target: FileTarget,
    pub options: DiffOptions,
}

/// A query the diff thread answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffQuery {
    /// What `Comparison` changed.
    Changes(Comparison),
    /// One file's diff.
    File(FileQuery),
    /// Files of `of`'s change set opened in place: some by name, and Expand All's from where it
    /// stands.
    Expand(ExpandQuery),
    /// Several paths of Local Changes' lists, their diffs drawn together.
    Together(TogetherQuery),
}

/// Several paths selected in one of Local Changes' lists, whose diffs the view draws together
/// (staging-and-commit R8.1, the user's decision of 2026-10-09: as Fork does): each path's
/// working-tree diff, in the order the lists show them, read on the diff thread one after
/// another under Expand All's line budget and answered a page at a time
/// ([`Update::Together`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TogetherQuery {
    /// The window's number for this ask: every page answering it names it.
    pub asked: u64,
    /// The paths to read, each by its place among the paths drawn and the side it is asked
    /// as; a conflicted path, which has no diff, is not among them. Shared, not copied.
    pub files: Arc<Vec<TogetherFile>>,
    /// What every path is asked at: always the exact diff (R8.5).
    pub options: DiffOptions,
}

/// One path of a [`TogetherQuery`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TogetherFile {
    /// Its place among the paths drawn together.
    pub index: usize,
    pub path: RepoPath,
    pub side: WorkingSide,
}

/// One path's answer in an [`Update::Together`] page: its diff prepared for the views, `None`
/// where git prints nothing for it, or why it could not be read, as display text — that path's
/// alone.
pub type TogetherOutcome = Result<Option<Box<ShownDiff>>, String>;

/// Files of a change set to open in place in the Commit tab (PRD R5.3): the ones opened one at
/// a time, by name, and — when Expand All is on its way — the rest of the change set from the
/// file it reached, under its line budget. Read on the diff thread a page at a time, each page
/// answered as it is read ([`Update::Expanded`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandQuery {
    pub of: Comparison,
    /// The window's own change set, shared rather than copied: the files are named by their
    /// index in it.
    pub changes: Arc<ChangeSet>,
    /// What every file is asked at — the view's context and whether whitespace is ignored.
    /// Its `load_anyway` is never set: that is per file, in `files`.
    pub options: DiffOptions,
    /// The files opened one at a time and not answered yet, in the order to read them.
    pub files: Vec<OpenedFile>,
    /// Expand All, taken up where it stands; `None` when it is not on its way.
    pub all: Option<AllFrom>,
    /// The files already open, in index order, which Expand All passes over: kept as they are
    /// drawn, never read again (the user's decision, 2026-10-04). A file named in `files` is
    /// among them, read once, by name.
    pub kept_open: Vec<usize>,
}

/// One file opened in place, by its index in the change set, and whether it is read past
/// R2.6's limits (its Load Diff pressed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OpenedFile {
    pub index: usize,
    pub load_anyway: bool,
}

/// Where an Expand All stands: the first file it has not decided, and the lines its files have
/// spent of its budget so far — so one superseded midway is taken up again where it stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AllFrom {
    pub next: usize,
    pub spent: u64,
}

/// What a page of Expand All leaves it at: where it stands, and whether it has ended — at the
/// last file, or with its budget spent and the rest left collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllProgress {
    pub at: AllFrom,
    pub ended: Option<AllEnded>,
}

/// Why an Expand All ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllEnded {
    /// Every file was opened.
    Every,
    /// Its line budget was spent: the files from `next` on stay collapsed.
    Budget,
}

/// One file of an [`Update::Expanded`] page: which file, read with or without the limits, and
/// what it turned out to be — prepared for the views on the worker — or why it could not be
/// read, as display text. A file's failure is that file's alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedFile {
    pub file: OpenedFile,
    /// Whether Expand All read it, rather than its being opened by name.
    pub by_all: bool,
    pub outcome: Result<Box<ShownDiff>, String>,
}

impl DiffQuery {
    /// The lane the query is numbered in — [`Request::lane`] of the request it came from.
    /// No arm defaults.
    #[cfg(test)]
    pub fn lane(&self) -> QueryLane {
        match self {
            Self::Changes(_) => QueryLane::Changes,
            Self::File(_) | Self::Expand(_) | Self::Together(_) => QueryLane::FileDiff,
        }
    }
}

/// Answers the window has let go of, handed to a worker thread to be freed there rather than
/// on the UI thread: dropping a change set of 55,184 files took 1.2-2.0 ms (release build,
/// measured 2026-10-03, R2) — more than a frame can spare, for nothing the window draws. So
/// are the history a reopen replaces (#52, refs-and-status R11.3) and a refresh's superseded
/// or replaced answers: a refs snapshot, ahead/behind, a status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retired(Box<RetiredAnswers>);

/// Boxed, so a request carrying them is a pointer wide.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct RetiredAnswers {
    changes: Option<Arc<ChangeSet>>,
    shown: Vec<ShownDiff>,
    history: Option<ReplacedHistory>,
    refs: Option<Arc<RefsSnapshot>>,
    ahead_behind: Option<Arc<Vec<(RefName, AheadBehind)>>>,
    status: Option<Arc<LocalChanges>>,
    sidebar: Option<Arc<Vec<SidebarRow>>>,
}

/// A history a reopen replaced, shared so a request carrying it can be cloned; two are the
/// same when they are one history, since a history has no equality of its own.
#[derive(Debug, Clone)]
struct ReplacedHistory(Arc<History>);

impl PartialEq for ReplacedHistory {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ReplacedHistory {}

impl Retired {
    /// What is let go of: a change set, and diffs as the views drew them (their rows' indexes
    /// with them) — a file's, and the files opened in place. `None` when that is nothing.
    pub fn of(changes: Option<Arc<ChangeSet>>, shown: Vec<ShownDiff>) -> Option<Self> {
        (changes.is_some() || !shown.is_empty()).then(|| {
            Self(Box::new(RetiredAnswers {
                changes,
                shown,
                ..RetiredAnswers::default()
            }))
        })
    }

    /// The history a reopen replaced, every loaded row of it.
    pub fn history(history: History) -> Self {
        Self(Box::new(RetiredAnswers {
            history: Some(ReplacedHistory(Arc::new(history))),
            ..RetiredAnswers::default()
        }))
    }

    /// A refs snapshot the window replaced or never kept.
    pub fn refs(snapshot: Arc<RefsSnapshot>) -> Self {
        Self(Box::new(RetiredAnswers {
            refs: Some(snapshot),
            ..RetiredAnswers::default()
        }))
    }

    /// Ahead/behind counts the window replaced or never kept — shared with the sidebar that
    /// draws them, so freed where the last hold on them goes; `None` when there are none.
    pub fn ahead_behind(counts: Arc<Vec<(RefName, AheadBehind)>>) -> Option<Self> {
        (!counts.is_empty()).then(|| {
            Self(Box::new(RetiredAnswers {
                ahead_behind: Some(counts),
                ..RetiredAnswers::default()
            }))
        })
    }

    /// The sidebar's rows the window replaced or never kept, and the snapshot they index,
    /// which they may be the last hold on.
    pub fn sidebar(refs: Arc<RefsSnapshot>, rows: Arc<Vec<SidebarRow>>) -> Self {
        Self(Box::new(RetiredAnswers {
            refs: Some(refs),
            sidebar: Some(rows),
            ..RetiredAnswers::default()
        }))
    }

    /// A status the window replaced or never kept, and the lists laid out over it — shared
    /// with a filter of them that may be the last to hold them.
    pub fn status(status: Arc<LocalChanges>) -> Self {
        Self(Box::new(RetiredAnswers {
            status: Some(status),
            ..RetiredAnswers::default()
        }))
    }

    /// The history let go of, and how many rows it held, for a test that checks what was
    /// handed over.
    #[cfg(test)]
    pub fn replaced_rows(&self) -> Option<usize> {
        self.0.history.as_ref().map(|history| history.0.len())
    }

    /// The refs snapshot let go of, for a test that checks what was handed over.
    #[cfg(test)]
    pub fn refs_snapshot(&self) -> Option<&RefsSnapshot> {
        self.0.refs.as_deref()
    }

    /// The sidebar's rows let go of, for a test that checks what was handed over.
    #[cfg(test)]
    pub fn sidebar_rows(&self) -> Option<&[SidebarRow]> {
        self.0.sidebar.as_deref().map(Vec::as_slice)
    }

    /// The status let go of, for a test that checks what was handed over.
    #[cfg(test)]
    pub fn working_tree_status(&self) -> Option<&cairn_model::WorkingTreeStatus> {
        self.0.status.as_deref().map(LocalChanges::status)
    }

    /// The ahead/behind counts let go of, for a test that checks what was handed over.
    #[cfg(test)]
    pub fn counts(&self) -> &[(RefName, AheadBehind)] {
        self.0.ahead_behind.as_deref().map_or(&[], Vec::as_slice)
    }

    /// The change set let go of, for a test that checks what was handed over.
    #[cfg(test)]
    pub fn changes(&self) -> Option<&ChangeSet> {
        self.0.changes.as_deref()
    }

    /// The diffs as drawn let go of, for a test that checks what was handed over.
    #[cfg(test)]
    pub fn shown(&self) -> &[ShownDiff] {
        &self.0.shown
    }
}

/// Not `Clone`: a destructive write carries the `Confirmed` the user gave it, which is spent
/// once. A test may copy one that carries none.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(Clone))]
pub enum Request {
    /// Starts a walk from every ref (refs-and-status R4.1), abandoning any walk already open:
    /// from the refs snapshot the last refresh read on the history thread, or — when none has
    /// been read, or the walk from it fails because one of its commits has gone since — from
    /// one read now, which is answered as [`Update::Refs`] in the history lane before the
    /// first page. Numbered in the walk lane too: every page of the walk it opens is answered
    /// under that number, so the pages of the walk it replaces are never drawn.
    OpenHistory { rows: usize },
    /// The next `rows` rows; falls back to the cold cursor when no walk is open.
    MoreHistory { rows: usize },
    /// Finds the row of `target` — a commit's, or a stash's by its stash commit — by paging
    /// the walk open forward `rows` at a time until a page holds it or the walk ends
    /// (refs-and-status R8.5): every page answered as [`Update::Rows`], as a scroll's are, so
    /// the window keeps what scrolling there would and no more (R8.6). History-lane work: the
    /// next press, scroll or stop supersedes it, and it supersedes the page or find in
    /// flight. On the repository thread a page at a time, so a filter or a refresh asked
    /// meanwhile is served between its pages.
    FindRow { target: Oid, rows: usize },
    /// Supersedes the find in flight and asks nothing: what a scroll of the list, a row
    /// chosen in it or a find answered by a page already loaded asks of the history lane.
    StopFinding,
    /// What `of` changed, answered by [`Update::Changes`]. Supersedes the changes query
    /// and the file diff in flight, and nothing else.
    Changes { of: Comparison },
    /// One file's diff, answered by [`Update::FileDiff`]. Supersedes the file diff in
    /// flight, and nothing else.
    FileDiff(FileQuery),
    /// Files of a change set opened in place (R5.3): those opened by name, and Expand All's
    /// from where it stands, answered a page at a time by [`Update::Expanded`] — in the
    /// file-diff lane, so a file diff, a changes query or a newer expansion asked after it
    /// ends it between files or kills its read. One that names nothing to read asks nothing:
    /// what it is for is to supersede the one in flight (Collapse All).
    Expand(ExpandQuery),
    /// The paths selected in one of Local Changes' lists, their diffs read to be drawn
    /// together, answered a page at a time by [`Update::Together`]. File-diff lane work: it
    /// supersedes the path chosen's diff and the files opened in place, and they it.
    Together(TogetherQuery),
    /// Which of `files`' files hold `text` in a path (the Changes tab's filter, R5.4),
    /// answered by [`Update::FilteredFiles`]: a pass over every path, so run on a worker and
    /// never on the UI thread. Numbered in the file-filter lane, so the next keystroke
    /// supersedes it and nothing else does; `files` is the window's own change set, shared.
    FilterFiles {
        of: Comparison,
        files: Arc<ChangeSet>,
        text: String,
    },
    /// The configured remotes, answered by [`Update::Remotes`].
    ListRemotes,
    /// The context the user's `git diff` shows — `diff.context` — answered by
    /// [`Update::ConfiguredContext`]: where the diff views' context starts (phase 06), and
    /// answered again whenever the diff thread opens its handle afresh because the
    /// configuration moved, so an edit reaches a session that has not moved its context.
    ConfiguredContext,
    /// Reads the refs, ahead/behind and the working tree's status again (refs-and-status
    /// R10): on focus gained, after an operation, and on the Refresh action. Three reads. The
    /// refs and ahead/behind are numbered in lanes of their own, superseding the refresh
    /// before them lane by lane and nothing else — no page, no diff, no filter. Status is
    /// not superseded (R10.3 as amended, the user's decision of 2026-10-07): a refresh
    /// leaves a running `git status` to finish, and every refresh asked while it runs is
    /// one follow-up status after it; only a close ends it. The refs are read on the
    /// history thread and answered by [`Update::Refs`], saying whether the history must be
    /// reopened; ahead/behind is counted for that snapshot, and status read, on the refresh
    /// thread ([`Update::AheadBehind`], [`Update::Status`]), so neither a page nor a diff
    /// queues behind a slow one. A failure is [`Update::RefreshFailed`].
    Refresh,
    /// Reads the working tree's status again, and nothing else: what a write to the index or
    /// the working tree leaves stale (staging-and-commit R4.5). On the refresh thread, as a
    /// refresh's status is, never superseding one that runs; answered by [`Update::Status`].
    RefreshStatus,
    /// The sidebar's rows for `refs` (R8.1-R8.3): its sections, the refs and stashes whose
    /// names hold `text`, in folders, as far as `disclosure` opens them
    /// (`RefsSnapshot::sidebar_rows`), answered by [`Update::FilteredRefs`]. A pass over
    /// every ref, run on the repository thread, numbered in the ref-filter lane so the next
    /// keystroke, folder opened or snapshot superseded it and nothing else does; `refs` is
    /// the window's own snapshot, shared.
    FilterRefs {
        refs: Arc<RefsSnapshot>,
        text: String,
        disclosure: Arc<Disclosure>,
    },
    /// Which rows of Local Changes' two lists hold `text` in a path (refs-and-status R9; the
    /// Changes tab's filter's rule), answered by [`Update::FilteredLocalChanges`]: a pass over
    /// every path the status listed, run on the repository thread, numbered in the
    /// local-changes-filter lane so the next keystroke, or a status arriving, supersedes it and
    /// nothing else does; `changes` is the window's own, shared.
    FilterLocalChanges {
        changes: Arc<LocalChanges>,
        text: String,
    },
    /// Fetches `remote` (a configured name or a URL) in the network lane;
    /// its progress and outcome arrive as the `Fetch*` updates, or
    /// [`Update::FetchRefused`] when a fetch is already in flight.
    Fetch { remote: String },
    /// Kills the fetch in flight, if any.
    CancelFetch,
    /// Runs `write` on the local write lane (staging-and-commit R4): straight to its thread,
    /// queued behind the write running, in the order asked, answered by
    /// [`Update::WriteStarted`] and [`Update::WriteEnded`] under `id`, which the window
    /// chose ([`OperationId::next`]) so it can draw the write queued from the moment it asks.
    Write { id: OperationId, write: LocalWrite },
    /// What discarding `paths` — rows of Local Changes' Unstaged list — would lose, computed on
    /// the local lane in the order asked, after every write asked before it, and answered by
    /// [`Update::DiscardConsequence`] under `asked`: the `Consequence` the confirmation draws,
    /// or why no discard is offered (a nested repository, a submodule, a conflict, …), before
    /// any dialog (staging-and-commit R8.4). A read: it writes nothing, and the window may ask
    /// again. Numbered in the discard-count lane, so a newer ask ends it — between paths, or
    /// its `git` read's process — and so does a close; a count ended so answers nothing.
    DiscardConsequence {
        asked: OperationId,
        paths: Vec<RepoPath>,
    },
    /// What discarding `selection` of `diff` — a path's unstaged or untracked diff as Local
    /// Changes drew it — would lose (`ops::discard_lines_consequence`), computed on the local
    /// lane in the order asked and answered by [`Update::DiscardConsequence`] under `asked`, as
    /// a discard of files is: the diff's staging gesture's discard (staging-and-commit R9.1,
    /// R9.2). Numbered in the discard-count lane with it; one superseded before it runs answers
    /// nothing. Boxed: it carries a whole diff.
    DiscardLinesConsequence {
        asked: OperationId,
        diff: Box<FileDiff>,
        selection: Selection,
    },
    /// Ends the count of a discard's loss in flight, and asks nothing: what Local Changes asks
    /// when it is no longer shown, so a count nobody will confirm stops holding the local lane.
    StopCounting,
    /// Cancels the write `id` names, if it is a commit and running; nothing otherwise (R4.3).
    /// Never queued: it reaches the lane's state directly, ahead of any write.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the commit box (staging-and-commit phase 09) is the first view to cancel \
                      a commit; the lane serves it, and its tests ask"
        )
    )]
    CancelWrite { id: OperationId },
    /// Every `git` invocation this repository has run that is over, oldest
    /// first, answered by [`Update::CommandLog`].
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "no view asks for the log in this packet (PRD R8.3); the worker answers it, \
                      and its tests ask"
        )
    )]
    CommandLog,
    /// Frees answers the window no longer keeps, on the repository thread. Superseding
    /// nothing, and answered by nothing.
    Retire(Retired),
    /// Closes the repository: stops any walk, ends every `git` running in it
    /// and waits a bounded time for their reaps — on the worker, never the
    /// caller — then lets every worker thread go, which ends the update
    /// stream. What closing the window asks for.
    Close,
}

/// The most lanes one request is numbered in: a refresh's two, and an open's.
pub(super) const MOST_LANES: usize = 2;

impl Request {
    /// The lanes a query is numbered in, superseding what each has in flight — one lane,
    /// but a refresh's two; none for an operation, which is numbered in none — a scroll
    /// must not cancel a fetch, nor a fetch a scroll or a diff. No arm defaults, so a new
    /// request does not compile until it is placed.
    pub fn lanes(&self) -> &'static [QueryLane] {
        match self {
            Self::OpenHistory { .. } => &[QueryLane::History, QueryLane::Walk],
            Self::MoreHistory { .. } | Self::FindRow { .. } | Self::StopFinding => {
                &[QueryLane::History]
            }
            Self::Changes { .. } => &[QueryLane::Changes],
            Self::FileDiff(_) | Self::Expand(_) | Self::Together(_) => &[QueryLane::FileDiff],
            Self::FilterFiles { .. } => &[QueryLane::FileFilter],
            // Not the status lane: a refresh never supersedes a status (R10.3 as amended).
            Self::Refresh => &[QueryLane::Refs, QueryLane::AheadBehind],
            Self::FilterRefs { .. } => &[QueryLane::RefFilter],
            Self::FilterLocalChanges { .. } => &[QueryLane::LocalChangesFilter],
            Self::DiscardConsequence { .. }
            | Self::DiscardLinesConsequence { .. }
            | Self::StopCounting => &[QueryLane::DiscardCount],
            Self::ListRemotes
            | Self::ConfiguredContext
            | Self::Fetch { .. }
            | Self::CancelFetch
            | Self::Write { .. }
            | Self::CancelWrite { .. }
            // Status is numbered in its lane, which nothing moves (R10.3 as amended).
            | Self::RefreshStatus
            | Self::CommandLog
            | Self::Retire(_)
            | Self::Close => &[],
        }
    }
}

/// Which of a refresh's answers failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refreshed {
    Refs,
    AheadBehind,
    Status,
}

/// A request is answered by a stream of these. Everything about a fetch or a
/// prompt is tied to no epoch, so a scroll cannot make it vanish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// `complete` says the history has no more to give.
    /// One page of rows, its text, authors, lane changes and snapshots beside them, for the
    /// window to append to its history (`cairn_model::History::append`).
    Rows {
        rows: RowsPage,
        complete: bool,
    },
    /// `message` is display text, already rendered from the engine's error.
    Failed {
        message: String,
    },
    /// A worker died. Tied to no request, so never filtered out by epoch.
    WorkerLost {
        message: String,
    },
    /// The repository is open: `name` is what the title bar calls it — the last component of
    /// its working tree, or of its git directory when it is bare, as Fork names a
    /// repository's folder — whatever path it was opened at (`.`, a subdirectory, its `.git`).
    /// Sent once, first, tied to no request.
    Opened {
        name: String,
    },
    /// The default remote first, when there is one.
    Remotes {
        remotes: Vec<RemoteSummary>,
    },
    /// `diff.context` as the user's `git diff` reads it, raised to one: once asked, and again
    /// each time the configuration moves under the diff thread. Not sent when the
    /// configuration holds a value git refuses: every diff then fails saying so, as the
    /// user's own `git diff` does.
    ConfiguredContext {
        context: Context,
    },
    /// git is running; a `CancelFetch` from here on has something to kill.
    FetchStarted {
        remote: String,
    },
    /// One redraw of git's own progress meter, for the one fetch in flight.
    FetchProgress {
        line: String,
    },
    /// The fetch ended. Whether it moved a ref is the refresh's to find out, which the
    /// window asks for on every ending, since a fetch that failed or was killed may have
    /// moved some refs before it stopped (refs-and-status R10.1).
    FetchFinished {
        remote: String,
    },
    /// `stranded_locks` is every `*.lock` found under the git directory once
    /// git was gone — a lock this cancel stranded, one from an earlier crash,
    /// or one a git in a terminal holds right now, which the engine cannot
    /// tell apart; usually none, since the cancel is `SIGTERM` first and git
    /// cleans up on that.
    FetchCancelled {
        remote: String,
        stranded_locks: Vec<PathBuf>,
    },
    FetchFailed {
        remote: String,
        message: String,
    },
    /// A fetch of `remote` was not started because one is already in flight;
    /// `reason` says which, as display text. Nothing else about the fetch in
    /// flight changes.
    FetchRefused {
        remote: String,
        reason: String,
    },
    /// git or ssh is asking, through the helper: `text` is the prompt as
    /// given, `id` is what the answer must name, and `asking` is the operation
    /// whose askpass token the helper presented — the remote a fetch is of, or a
    /// local write as the window names it ("Commit") — so a prompt raised while a
    /// fetch and a write both run is titled by its own; `None` for a token issued
    /// with no name.
    Prompt {
        id: PromptId,
        text: String,
        asking: Option<String>,
    },
    /// The refs as they stand, read on the history thread (refs-and-status R1, R11.2): by a
    /// refresh, in the refs lane, with `reopen` saying whether what the history draws differs
    /// from the snapshot its walk began from (R10.4) — so the window reopens it, and leaves
    /// it alone otherwise; or by an open that read its own, in the history lane, with
    /// `reopen` false, since the walk being opened is from this snapshot.
    Refs {
        snapshot: Arc<RefsSnapshot>,
        reopen: bool,
    },
    /// Each local branch's distance from its upstream, for the snapshot the same refresh
    /// read, in that snapshot's order (R2).
    AheadBehind {
        counts: Vec<(RefName, AheadBehind)>,
    },
    /// The working tree's status as `git status` lists it (R3), laid out on the refresh
    /// thread as Local Changes' two lists (R9.1), so the window only keeps it.
    Status {
        changes: Arc<LocalChanges>,
    },
    /// One of a refresh's reads failed: `message` is display text. The answer kept before
    /// stays as it was.
    RefreshFailed {
        what: Refreshed,
        message: String,
    },
    /// The sidebar's rows for `refs`, the snapshot they index, with `text` the filter they
    /// were laid out for. Superseded by the next ask, so the one read is the latest asked.
    FilteredRefs {
        refs: Arc<RefsSnapshot>,
        text: String,
        rows: Vec<SidebarRow>,
    },
    /// The rows of `changes`' two lists that hold `text`, with the lists they index, so the
    /// window keeps the rows only for the lists it draws.
    FilteredLocalChanges {
        changes: Arc<LocalChanges>,
        text: String,
        rows: MatchedRows,
    },
    /// A local write has started (staging-and-commit R4.2): the writes asked before it have
    /// ended, and it is no longer queued.
    WriteStarted {
        id: OperationId,
    },
    /// A line a running commit or amend's `git` — or a hook it runs — wrote, stdout's and
    /// stderr's alike, as it arrived (staging-and-commit R6.5): what the Git Error dialog
    /// (phase 09) and the activity popover (phase 11) draw.
    WriteOutput {
        id: OperationId,
        line: String,
    },
    /// A local write has ended, how, and what the window reads again after it (R4.5): what
    /// its `Invalidated` names — everything, for a commit, whatever its ending, so a refresh
    /// kept back while it ran (R4.6) is never lost.
    WriteEnded {
        id: OperationId,
        ending: WriteEnding,
        read_again: ReadAgain,
    },
    /// What discarding the paths asked under `asked` would lose: the `Consequence` the
    /// confirmation draws, or why the engine refused before any prompt (display text).
    DiscardConsequence {
        asked: OperationId,
        outcome: Result<Consequence, String>,
    },
    /// The lock files under the git directories as the repository opened — one a write left
    /// when a close gave up on it among them (staging-and-commit R4.9) — listed by the local
    /// lane before it runs any write, and sent only when there are some.
    LocksAtOpen {
        locks: Vec<PathBuf>,
    },
    /// The repository's command log, oldest first, as far back as it keeps.
    CommandLog {
        records: Vec<CommandRecord>,
    },
    /// What `of` changed (R2.1, R2.2): its files, the commit's details when one commit was
    /// named, and how the rename search went.
    Changes {
        of: Comparison,
        changes: ChangeSet,
    },
    /// One file's diff as `query` asked for it, prepared for the views on the worker that
    /// answered it (both rows' indexes, the widest line; phase 07) so the window only keeps
    /// it; `None` where the working tree's `git diff` of the path prints nothing (a clean
    /// path).
    FileDiff {
        query: FileQuery,
        /// Boxed: a prepared diff is far larger than any other update.
        diff: Option<Box<ShownDiff>>,
    },
    /// The files of `of`'s change set that hold `text` in a path, by index, in order.
    FilteredFiles {
        of: Comparison,
        text: String,
        files: Vec<u32>,
    },
    /// A page of files opened in place, of `of`'s change set at `options`: each file's
    /// outcome, and — for a page of Expand All — where it stands after it. Pages arrive in the
    /// order they are read; the files opened by name come before Expand All's.
    Expanded {
        of: Comparison,
        options: DiffOptions,
        files: Vec<ExpandedFile>,
        all: Option<AllProgress>,
    },
    /// A page of the paths drawn together, answering the ask numbered `asked`: each path read,
    /// by its place, and — on the last page — where the read ended: at the last path, or with
    /// Expand All's line budget spent before the path at `next`, which is not read.
    Together {
        asked: u64,
        files: Vec<(usize, TogetherOutcome)>,
        ended: Option<TogetherEnded>,
    },
    /// A diff query failed: `query` is what was asked, so the window shows the failure
    /// only for the selection it names, and `message` is display text. A query that was
    /// superseded is not a failure and sends nothing.
    DiffFailed {
        query: DiffQuery,
        message: String,
    },
    /// An answer whose query was superseded before the window read it — a change set, a
    /// file's prepared diff (up to 64 MiB, both layouts), Expand All's batch — handed back
    /// unread rather than dropped where the epoch filter sees it, which is the UI thread's
    /// task: the window hands it straight back to a worker to free ([`Request::Retire`]).
    /// Never drawn. Sent by no worker: [`super::Updates::next`] makes it out of a stale
    /// answer.
    Superseded(Retired),
}

impl Update {
    /// What a superseded answer holds that is worth a worker's freeing, handed over unread: a
    /// change set, a file's prepared diff, Expand All's batch, a refs snapshot, ahead/behind,
    /// a status. `None` for everything else — a page of rows, a filter's indices, a failure
    /// — which is small, or (a fetch's and a prompt's news) carries no epoch and is never
    /// superseded. No arm defaults, so a new update does not compile until it is placed.
    pub(super) fn into_retired(self) -> Option<Retired> {
        match self {
            Self::Changes { changes, .. } => Retired::of(Some(Arc::new(changes)), Vec::new()),
            Self::FileDiff { diff, .. } => {
                Retired::of(None, diff.map(|shown| *shown).into_iter().collect())
            }
            Self::Expanded { files, .. } => Retired::of(None, expanded_diffs(files)),
            Self::Together { files, .. } => Retired::of(None, together_diffs(files)),
            Self::Refs { snapshot, .. } => Some(Retired::refs(snapshot)),
            Self::AheadBehind { counts } => Retired::ahead_behind(Arc::new(counts)),
            Self::Status { changes } => Some(Retired::status(changes)),
            // The lists may be the last hold on a status the window has replaced since.
            Self::FilteredLocalChanges { changes, .. } => Some(Retired::status(changes)),
            // The snapshot may be the last hold on one the window has replaced since.
            Self::FilteredRefs { refs, rows, .. } => Some(Retired::sidebar(refs, Arc::new(rows))),
            Self::Superseded(retired) => Some(retired),
            Self::Rows { .. }
            | Self::RefreshFailed { .. }
            | Self::Failed { .. }
            | Self::WorkerLost { .. }
            | Self::Remotes { .. }
            | Self::Opened { .. }
            | Self::ConfiguredContext { .. }
            | Self::FetchStarted { .. }
            | Self::FetchProgress { .. }
            | Self::FetchFinished { .. }
            | Self::FetchCancelled { .. }
            | Self::FetchFailed { .. }
            | Self::FetchRefused { .. }
            | Self::Prompt { .. }
            | Self::WriteStarted { .. }
            | Self::WriteOutput { .. }
            | Self::WriteEnded { .. }
            | Self::DiscardConsequence { .. }
            | Self::LocksAtOpen { .. }
            | Self::CommandLog { .. }
            | Self::FilteredFiles { .. }
            | Self::DiffFailed { .. } => None,
        }
    }
}

/// Why the paths drawn together stopped being read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TogetherEnded {
    /// Every path was read.
    Every,
    /// The line budget was spent: the paths from `next` on are not read.
    Budget { next: usize },
}

/// The diffs a page of paths drawn together holds, for a worker to free.
pub fn together_diffs(files: Vec<(usize, TogetherOutcome)>) -> Vec<ShownDiff> {
    files
        .into_iter()
        .filter_map(|(_, outcome)| outcome.ok().flatten())
        .map(|shown| *shown)
        .collect()
}

/// The diffs a page of files opened in place holds, for a worker to free.
pub fn expanded_diffs(files: Vec<ExpandedFile>) -> Vec<ShownDiff> {
    files
        .into_iter()
        .filter_map(|file| file.outcome.ok())
        .map(|shown| *shown)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: an operation numbered like a query, which a scroll would then supersede
    /// (the fetch never runs) or which would supersede a page or a diff; or a query put in
    /// the wrong lane, where a scroll would cancel a diff or a file diff the changes query.
    #[test]
    fn every_query_is_numbered_in_its_lane_and_operations_in_none() {
        let commit = Comparison::Commit(cairn_model::Oid::from_bytes(&[1; 20]).unwrap());
        let file = FileQuery {
            target: FileTarget::WorkingTree {
                path: RepoPath::from("src/lib.rs"),
                side: WorkingSide::Unstaged,
            },
            options: DiffOptions::default(),
        };
        for (query, lane) in [
            (Request::MoreHistory { rows: 1 }, QueryLane::History),
            (
                Request::FindRow {
                    target: cairn_model::Oid::from_bytes(&[2; 20]).unwrap(),
                    rows: 1,
                },
                QueryLane::History,
            ),
            (Request::StopFinding, QueryLane::History),
            (Request::Changes { of: commit }, QueryLane::Changes),
            (Request::FileDiff(file.clone()), QueryLane::FileDiff),
            (
                Request::FilterFiles {
                    of: commit,
                    files: Arc::new(ChangeSet {
                        files: Vec::new(),
                        details: None,
                        renames: cairn_model::RenameDetection::default(),
                    }),
                    text: "src".to_owned(),
                },
                QueryLane::FileFilter,
            ),
            (
                Request::Expand(ExpandQuery {
                    of: commit,
                    changes: Arc::new(ChangeSet {
                        files: Vec::new(),
                        details: None,
                        renames: cairn_model::RenameDetection::default(),
                    }),
                    options: DiffOptions::default(),
                    files: Vec::new(),
                    all: Some(AllFrom::default()),
                    kept_open: Vec::new(),
                }),
                QueryLane::FileDiff,
            ),
            (
                Request::FilterRefs {
                    refs: Arc::new(cairn_model::RefsSnapshot {
                        refs: Vec::new(),
                        head: cairn_model::HeadState::Unborn(RefName::new("refs/heads/main")),
                        stashes: Vec::new(),
                        unreadable: 0,
                    }),
                    text: "main".to_owned(),
                    disclosure: Arc::new(Disclosure::default()),
                },
                QueryLane::RefFilter,
            ),
            (
                Request::FilterLocalChanges {
                    changes: Arc::new(LocalChanges::new(cairn_model::WorkingTreeStatus::Listed(
                        Vec::new(),
                    ))),
                    text: "src".to_owned(),
                },
                QueryLane::LocalChangesFilter,
            ),
            (
                Request::DiscardConsequence {
                    asked: OperationId::next(),
                    paths: vec![RepoPath::from("a")],
                },
                QueryLane::DiscardCount,
            ),
            (
                Request::DiscardLinesConsequence {
                    asked: OperationId::next(),
                    diff: Box::new(FileDiff {
                        file: cairn_model::ChangedFile {
                            status: cairn_model::ChangeStatus::Modified,
                            old_path: RepoPath::from("a"),
                            new_path: RepoPath::from("a"),
                            old_mode: None,
                            new_mode: None,
                            old_id: None,
                            new_id: None,
                        },
                        content: cairn_model::DiffContent::ModeChangeOnly,
                    }),
                    selection: Selection::empty(),
                },
                QueryLane::DiscardCount,
            ),
            (Request::StopCounting, QueryLane::DiscardCount),
        ] {
            assert_eq!(query.lanes(), [lane], "{query:?}");
        }
        // An open is numbered in the history lane, superseding a scroll or a find, and in the
        // walk lane, so the pages of the walk it replaces are dropped.
        assert_eq!(
            Request::OpenHistory { rows: 1 }.lanes(),
            [QueryLane::History, QueryLane::Walk]
        );
        // A refresh is numbered in the refs and ahead/behind lanes and nothing else: no page,
        // no diff, no filter is superseded by one, and no status (R10.3 as amended).
        assert_eq!(
            Request::Refresh.lanes(),
            [QueryLane::Refs, QueryLane::AheadBehind]
        );
        assert!(Request::Refresh.lanes().len() <= MOST_LANES);
        for operation in [
            Request::ListRemotes,
            Request::ConfiguredContext,
            Request::Fetch {
                remote: "origin".to_owned(),
            },
            Request::CancelFetch,
            Request::Write {
                id: OperationId::next(),
                write: LocalWrite::StageFiles {
                    paths: vec![RepoPath::from("a")],
                },
            },
            Request::CancelWrite {
                id: OperationId::next(),
            },
            Request::RefreshStatus,
            Request::CommandLog,
            Request::Retire(Retired(Box::default())),
            Request::Close,
        ] {
            assert_eq!(operation.lanes(), [], "{operation:?}");
        }
        assert_eq!(DiffQuery::Changes(commit).lane(), QueryLane::Changes);
        assert_eq!(DiffQuery::File(file).lane(), QueryLane::FileDiff);
    }
}
