//! Requests and updates crossing the worker boundary.
//!
//! A query is numbered in its lane ([`QueryLane`]) and every diff answer names what it
//! answers — the commit or the pair, the file, the options — so the window keeps an answer
//! only for the selection it names (PRD R4.4).

use std::path::PathBuf;
use std::sync::Arc;

use cairn_model::ShownDiff;
use cairn_model::{
    ChangeSet, ChangedFile, CommandRecord, Context, FileDiff, HistoryRow, Oid, RemoteSummary,
    RepoPath,
};

use super::askpass::PromptId;
use super::epoch::QueryLane;

/// What a changes query compares (R2.1).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the window selects a commit, a file or the working tree from phase 05 on"
    )
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    /// One commit against its first parent (the empty tree for a root commit).
    Commit(Oid),
    /// Two commits, tip against tip, `old` the base (R7.2).
    Between { old: Oid, new: Oid },
}

/// Which of a path's working-tree diffs (R3.1).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the window selects a commit, a file or the working tree from phase 05 on"
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the window selects a commit, a file or the working tree from phase 05 on"
    )
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTarget {
    /// One file of `of`'s change set, exactly as that answer named it.
    Committed { of: Comparison, file: ChangedFile },
    /// One path's diff in the working tree.
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
    /// Every file of `of`'s change set, in its order (Expand All).
    All {
        of: Comparison,
        options: DiffOptions,
    },
}

impl DiffQuery {
    /// The lane the query is numbered in — [`Request::lane`] of the request it came from.
    /// No arm defaults.
    #[cfg(test)]
    pub fn lane(&self) -> QueryLane {
        match self {
            Self::Changes(_) => QueryLane::Changes,
            Self::File(_) | Self::All { .. } => QueryLane::FileDiff,
        }
    }
}

/// Answers the window has let go of, handed to a worker thread to be freed there rather than
/// on the UI thread: dropping a change set of 55,184 files took 1.2-2.0 ms (release build,
/// measured 2026-10-03, R2) — more than a frame can spare, for nothing the window draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retired(Box<RetiredAnswers>);

/// Boxed, so a request carrying them is a pointer wide.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RetiredAnswers {
    changes: Option<Arc<ChangeSet>>,
    diffs: Vec<FileDiff>,
    shown: Vec<ShownDiff>,
}

impl Retired {
    /// What is let go of: a change set, file diffs, and a diff as the view drew it (its rows'
    /// indexes with it). `None` when that is nothing.
    pub fn of(
        changes: Option<Arc<ChangeSet>>,
        diffs: Vec<FileDiff>,
        shown: Vec<ShownDiff>,
    ) -> Option<Self> {
        (changes.is_some() || !diffs.is_empty() || !shown.is_empty()).then(|| {
            Self(Box::new(RetiredAnswers {
                changes,
                diffs,
                shown,
            }))
        })
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Starts a walk at `HEAD`, abandoning any walk already open.
    OpenHistory { rows: usize },
    /// The next `rows` rows; falls back to the cold cursor when no walk is open.
    MoreHistory { rows: usize },
    /// What `of` changed, answered by [`Update::Changes`]. Supersedes the changes query
    /// and the file diff in flight, and nothing else.
    Changes { of: Comparison },
    /// One file's diff, answered by [`Update::FileDiff`]. Supersedes the file diff in
    /// flight, and nothing else.
    FileDiff(FileQuery),
    /// Every file of `of`'s change set (Expand All), answered by [`Update::FileDiffs`] —
    /// in the file-diff lane, so a file diff or a changes query asked after it ends it.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Expand All is drawn in phase 08; the worker answers it"
        )
    )]
    ExpandAll {
        of: Comparison,
        options: DiffOptions,
    },
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
    /// Fetches `remote` (a configured name or a URL) in the network lane;
    /// its progress and outcome arrive as the `Fetch*` updates, or
    /// [`Update::FetchRefused`] when a fetch is already in flight.
    Fetch { remote: String },
    /// Kills the fetch in flight, if any.
    CancelFetch,
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

impl Request {
    /// The lane a query is numbered in, superseding what that lane has in flight; `None`
    /// for an operation, which is numbered in none — a scroll must not cancel a fetch, nor
    /// a fetch a scroll or a diff. No arm defaults, so a new request does not compile
    /// until it is placed.
    pub fn lane(&self) -> Option<QueryLane> {
        match self {
            Self::OpenHistory { .. } | Self::MoreHistory { .. } => Some(QueryLane::History),
            Self::Changes { .. } => Some(QueryLane::Changes),
            Self::FileDiff(_) | Self::ExpandAll { .. } => Some(QueryLane::FileDiff),
            Self::FilterFiles { .. } => Some(QueryLane::FileFilter),
            Self::ListRemotes
            | Self::ConfiguredContext
            | Self::Fetch { .. }
            | Self::CancelFetch
            | Self::CommandLog
            | Self::Retire(_)
            | Self::Close => None,
        }
    }
}

/// A request is answered by a stream of these. Everything about a fetch or a
/// prompt is tied to no epoch, so a scroll cannot make it vanish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Update {
    /// `complete` says the history has no more to give.
    Rows {
        rows: Vec<HistoryRow>,
        complete: bool,
    },
    /// `message` is display text, already rendered from the engine's error.
    Failed { message: String },
    /// A worker died. Tied to no request, so never filtered out by epoch.
    WorkerLost { message: String },
    /// The default remote first, when there is one.
    Remotes { remotes: Vec<RemoteSummary> },
    /// `diff.context` as the user's `git diff` reads it, raised to one: once asked, and again
    /// each time the configuration moves under the diff thread. Not sent when the
    /// configuration holds a value git refuses: every diff then fails saying so, as the
    /// user's own `git diff` does.
    ConfiguredContext { context: Context },
    /// git is running; a `CancelFetch` from here on has something to kill.
    FetchStarted { remote: String },
    /// One redraw of git's own progress meter, for the one fetch in flight.
    FetchProgress { line: String },
    /// `refreshed` says a ref moved: the history on screen is of the old
    /// ones and must be asked for again. On every ending, since a fetch that
    /// failed or was killed may have moved some refs before it stopped.
    FetchFinished { remote: String, refreshed: bool },
    /// `stranded_locks` is every `*.lock` found under the git directory once
    /// git was gone — a lock this cancel stranded, one from an earlier crash,
    /// or one a git in a terminal holds right now, which the engine cannot
    /// tell apart; usually none, since the cancel is `SIGTERM` first and git
    /// cleans up on that.
    FetchCancelled {
        remote: String,
        refreshed: bool,
        stranded_locks: Vec<PathBuf>,
    },
    FetchFailed {
        remote: String,
        refreshed: bool,
        message: String,
    },
    /// A fetch of `remote` was not started because one is already in flight;
    /// `reason` says which, as display text. Nothing else about the fetch in
    /// flight changes.
    FetchRefused { remote: String, reason: String },
    /// git or ssh is asking, through the helper: `text` is the prompt as
    /// given, and `id` is what the answer must name.
    Prompt { id: PromptId, text: String },
    /// The repository's command log, oldest first, as far back as it keeps.
    CommandLog { records: Vec<CommandRecord> },
    /// What `of` changed (R2.1, R2.2): its files, the commit's details when one commit was
    /// named, and how the rename search went.
    Changes { of: Comparison, changes: ChangeSet },
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
    /// Expand All's answer: files of `of`'s change set in its order, appended to what
    /// came before; `complete` says no more follow. One batch, complete, until phase 08
    /// bounds Expand All and pages it.
    FileDiffs {
        of: Comparison,
        options: DiffOptions,
        diffs: Vec<FileDiff>,
        complete: bool,
    },
    /// A diff query failed: `query` is what was asked, so the window shows the failure
    /// only for the selection it names, and `message` is display text. A query that was
    /// superseded is not a failure and sends nothing.
    DiffFailed { query: DiffQuery, message: String },
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
            (Request::OpenHistory { rows: 1 }, QueryLane::History),
            (Request::MoreHistory { rows: 1 }, QueryLane::History),
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
                Request::ExpandAll {
                    of: commit,
                    options: DiffOptions::default(),
                },
                QueryLane::FileDiff,
            ),
        ] {
            assert_eq!(query.lane(), Some(lane), "{query:?}");
        }
        for operation in [
            Request::ListRemotes,
            Request::ConfiguredContext,
            Request::Fetch {
                remote: "origin".to_owned(),
            },
            Request::CancelFetch,
            Request::CommandLog,
            Request::Retire(Retired(Box::new(RetiredAnswers {
                changes: None,
                diffs: Vec::new(),
                shown: Vec::new(),
            }))),
            Request::Close,
        ] {
            assert_eq!(operation.lane(), None, "{operation:?}");
        }
        assert_eq!(DiffQuery::Changes(commit).lane(), QueryLane::Changes);
        assert_eq!(DiffQuery::File(file).lane(), QueryLane::FileDiff);
    }
}
