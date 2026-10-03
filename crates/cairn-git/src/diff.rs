//! What a commit, or a pair of commits, changed — and what one of those changes looks like
//! line by line.
//!
//! Two queries (R2), and git answers both (decision E, `docs/design/engine.md`, "Where git
//! answers a read"). The **changes query** answers the changed files with `git diff-tree
//! --raw`: which paths changed, their statuses, modes and ids and every rename and copy pair
//! are git's own, under the rename detection the user's configuration asks for. The
//! **content query** takes one of those files and answers its [`FileDiff`]: gix reads both
//! versions and decides what is not text, and `git diff-tree -p` says which lines changed,
//! with and without whitespace, and what function context each hunk carries (the
//! content-parity decision of 2026-10-03, which amends packet decision L3). Both start
//! their processes through `crate::reads`. gix also reads the commits a query names and the
//! user's configuration; nothing here implements a diff algorithm but the intra-line
//! highlights, which git has no equivalent of, and no gix type reaches a public signature.

mod algorithm;
mod changes;
mod content;
mod git_config;
mod intraline;
mod renames;
mod submodules;
mod working_tree;

use cairn_model::{ChangeSet, ChangedFile, Context, DiffLimits, FileDiff, Oid, RepoPath};

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// Which two versions a changes query compares.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Subject {
    /// One commit against its first parent — against the empty tree when it is a root
    /// commit, which is decision L5 and what makes a root commit's diff its whole content.
    Commit(Oid),
    /// Two commits, tip against tip and never against a merge base (R7.2).
    Between { old: Oid, new: Oid },
}

/// What to compare (R2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangesRequest {
    subject: Subject,
}

impl ChangesRequest {
    /// One commit against its first parent. A merge is compared with its first parent like
    /// any other commit; a combined diff is out of scope by decision D6.
    pub fn commit(id: Oid) -> Self {
        Self {
            subject: Subject::Commit(id),
        }
    }

    /// Two commits, `old` as the base. The caller decides which is which (R7.2).
    pub fn between(old: Oid, new: Oid) -> Self {
        Self {
            subject: Subject::Between { old, new },
        }
    }
}

/// What a content query is allowed to read, and what it should compute (R2.6, R2.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContentOptions {
    pub limits: DiffLimits,
    /// Read a file past [`DiffLimits::max_bytes`] anyway, up to
    /// [`DiffLimits::load_anyway_bytes`]. The line limits do not apply to a file loaded
    /// this way either — a view draws its long lines truncated instead (R6.9).
    pub load_anyway: bool,
    /// Also compute the display-only ranges that comparing lines without their whitespace
    /// produces (R2.8), as `git diff -w` does. The lines drawn are always the original
    /// bytes, and the patch emitter can reach none of this by construction (R1.7).
    pub ignore_whitespace: bool,
    /// The context the view groups hunks at (R6.3), which is the context git is asked at,
    /// so that the function context its headers carry is the text `git diff -U<n>` prints
    /// for those very hunks ([`cairn_model::FunctionContext`]). The changed ranges are the
    /// same at every context; the entire file asks at one line.
    pub context: Context,
}

/// Which of a path's working-tree diffs to answer (R3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkingTreeDiff {
    /// `HEAD` against the index: what `git diff --cached -- <path>` shows. Against the empty
    /// tree on an unborn branch, as git compares it.
    Staged,
    /// The index against the working tree: what `git diff -- <path>` shows.
    Unstaged,
    /// Nothing against the working tree: what `git diff --no-index /dev/null <path>` shows
    /// for a file git does not track. Which files are untracked is status's to say (R3.6);
    /// this answers the one named, whatever the index holds.
    Untracked,
}

/// A run of diff queries over one repository, holding gix's resource cache for the length
/// of it.
///
/// Building that cache reads the index and the attribute stack, which on a repository of
/// 62,892 entries is several megabytes — so a query that builds one per call pays for it
/// per call. It borrows the repository and is not `Send`, exactly like
/// [`crate::HistorySession`]: it lives on the worker that owns that handle.
pub struct DiffSession<'repo> {
    repo: &'repo Repository,
    cache: gix::diff::blob::Platform,
}

impl std::fmt::Debug for DiffSession<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffSession")
            .field("repository", self.repo)
            .finish_non_exhaustive()
    }
}

impl Repository {
    /// Opens a run of diff queries. Keep it for as long as the queries keep coming.
    pub fn diff_session(&self) -> Result<DiffSession<'_>, Error> {
        // `ToGit` is the mode that never runs a textconv program, and the one git's own
        // rename detection uses (R2.3). No worktree root: this packet's queries read trees.
        let cache = self
            .inner()
            .diff_resource_cache(
                gix::diff::blob::pipeline::Mode::ToGit,
                gix::diff::blob::pipeline::WorktreeRoots::default(),
            )
            .map_err(|source| Error::DiffSetup {
                source: Box::new(source),
            })?;
        Ok(DiffSession { repo: self, cache })
    }
}

impl DiffSession<'_> {
    /// What changed between the two versions the request names: [`Repository::changes`],
    /// on this session's repository.
    pub fn changes(
        &mut self,
        git: &GitBinary,
        request: &ChangesRequest,
        cancel: &impl Cancel,
    ) -> Result<ChangeSet, Error> {
        self.repo.changes(git, request, cancel)
    }

    /// What one changed file of `request`'s change set turned out to be (R2.3 through
    /// R2.8): gix reads both versions and decides what is not text, and — for a file whose
    /// lines git has to compare — one `git diff-tree -p` read says which changed, a second
    /// does with `-w` when `options` asks to ignore whitespace, and a `git check-attr` read
    /// precedes them when a diff driver may name its own algorithm. Each blocks, so it is a
    /// worker's call; `cancel` is polled while git runs and a superseded query answers
    /// [`Error::ContentCancelled`]. Lines git prints that are not the lines gix read are
    /// [`Error::ContentReadsDisagree`].
    pub fn file_diff(
        &mut self,
        git: &GitBinary,
        request: &ChangesRequest,
        file: &ChangedFile,
        options: &ContentOptions,
        cancel: &impl Cancel,
    ) -> Result<FileDiff, Error> {
        let (old, new, _) = changes::subject(self.repo.inner(), request)?;
        let trees = content::Trees { old, new };
        content::file_diff(
            self.repo,
            &mut self.cache,
            git,
            trees,
            file,
            options,
            cancel,
        )
    }

    /// One path's working-tree diff (R3): [`Repository::working_tree_diff`], on this
    /// session's repository.
    pub fn working_tree_diff(
        &mut self,
        git: &GitBinary,
        path: &RepoPath,
        which: WorkingTreeDiff,
        options: &ContentOptions,
        cancel: &impl Cancel,
    ) -> Result<Option<FileDiff>, Error> {
        self.repo
            .working_tree_diff(git, path, which, options, cancel)
    }

    /// Every file of `changes`, the change set `request` answered, in its order — what
    /// Expand All shows. Each is decided as [`DiffSession::file_diff`] decides it, but the
    /// lines of every text file git has to compare come from ONE `git diff-tree -p` over the
    /// whole comparison (two with whitespace ignored), asked with the change set's own rename
    /// detection; a file that answer does not hold as the change set does is asked about on
    /// its own. `cancel` is checked between files and polled while git runs.
    pub fn file_diffs(
        &mut self,
        git: &GitBinary,
        request: &ChangesRequest,
        changes: &ChangeSet,
        options: &ContentOptions,
        cancel: &impl Cancel,
    ) -> Result<Vec<FileDiff>, Error> {
        let (old, new, _) = changes::subject(self.repo.inner(), request)?;
        let trees = content::Trees { old, new };
        content::file_diffs(
            self.repo,
            &mut self.cache,
            git,
            trees,
            changes,
            options,
            cancel,
        )
    }
}

impl Repository {
    /// What changed between the two versions the request names (R2.1, R2.2), as
    /// `git diff-tree` run by `git` answers it, sorted by a total key.
    ///
    /// It runs one `git` process, a read, in this repository (`crate::reads`); `git` is the
    /// binary the application found at startup, and the call blocks until it ends, so it is
    /// a worker's call. `cancel` is polled while it runs: once it says the query was
    /// superseded, the process is ended rather than waited for and the answer is
    /// [`Error::ChangesCancelled`] (R2.9). A commit that cannot be read is
    /// [`Error::ReadCommit`] and starts no process; `git` failing is [`Error::GitFailed`].
    pub fn changes(
        &self,
        git: &GitBinary,
        request: &ChangesRequest,
        cancel: &impl Cancel,
    ) -> Result<ChangeSet, Error> {
        changes::changes(git, self, request, cancel)
    }

    /// One path's staged, unstaged or untracked diff (R3.1 through R3.5), or `None` where
    /// the user's `git diff` of it — `--cached`, plain, or `--no-index` against
    /// `/dev/null` — shows nothing: a clean path, one whose stat alone moved, a file whose
    /// working-tree form is its index's after the clean filter and line-ending conversion,
    /// a submodule its `ignore` setting hides, an intent-to-add path's staged diff.
    ///
    /// git computes the diff and reads the working tree (`git diff-index --cached`, `git
    /// diff-files`, `git diff --no-index`, each run as a read), converting it to git's form
    /// as the user's `git diff` does, the path's clean filter driver included; the lines of
    /// a working-tree side are rebuilt from git's patch, so they are that form, and are
    /// checked against the object id git names for them. Every other answer is a state
    /// rather than an error (R3.4): [`DiffContent::Conflicted`](cairn_model::DiffContent)
    /// for a path the index holds unmerged (never diffed against one stage),
    /// `Unsupported` for a sparse index or a bare repository, `ModeChangeOnly`, `Submodule`
    /// with the commit each side names and whether the checkout is dirty, and the states
    /// R2.6 and R2.5 name. For the stand-in states the file carries the path and no mode or
    /// id. The new side's id is the object id of the content git read — the working tree's
    /// in git's form, computed and written nowhere — or absent where the answer did not
    /// read it (a side refused as too large).
    ///
    /// The index and the attributes are read fresh for every call (R3.3) and nothing is
    /// written (R3.5). It blocks on one `git` process, two with whitespace ignored, and one
    /// more where a diff driver may name an algorithm (`check-attr`); `cancel` is polled
    /// while each runs, and a superseded query answers [`Error::ContentCancelled`]. Lines
    /// git printed that are not the content it named are [`Error::ContentReadsDisagree`]
    /// — the file changed while git read it: ask again. A failure of git's, such as a
    /// required clean filter that failed or whose program does not exist, is
    /// [`Error::GitFailed`] with git's diagnostic. An untracked path that is not relative
    /// to the top of the working tree is [`Error::NotAWorkTreePath`], and one that is
    /// neither a file nor a symlink is answered `Unsupported`, both before git runs.
    pub fn working_tree_diff(
        &self,
        git: &GitBinary,
        path: &RepoPath,
        which: WorkingTreeDiff,
        options: &ContentOptions,
        cancel: &impl Cancel,
    ) -> Result<Option<FileDiff>, Error> {
        working_tree::working_tree_diff(self, git, path, which, options, cancel)
    }

    /// One content query, on a session of its own: [`DiffSession::file_diff`].
    pub fn file_diff(
        &self,
        git: &GitBinary,
        request: &ChangesRequest,
        file: &ChangedFile,
        options: &ContentOptions,
        cancel: &impl Cancel,
    ) -> Result<FileDiff, Error> {
        self.diff_session()?
            .file_diff(git, request, file, options, cancel)
    }
}
