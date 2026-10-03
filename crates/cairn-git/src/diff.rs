//! What a commit, or a pair of commits, changed — and what one of those changes looks like
//! line by line.
//!
//! Two queries (R2). The **changes query** answers the changed files, and `git diff-tree`
//! answers it (decision E, `docs/design/engine.md`, "Where git answers a read"): which
//! paths changed, their statuses, modes and ids and every rename and copy pair are git's
//! own, under the rename detection the user's configuration asks for. It is the one diff
//! query that starts a process, through `crate::reads`. The **content query** takes one of
//! those files and answers its [`FileDiff`], computed by gix (decision L3) without a
//! process. gix also reads the commits a changes query names and the user's
//! configuration; nothing here implements a diff algorithm, and no gix type reaches a
//! public signature.

mod changes;
mod content;
mod intraline;
mod renames;
mod whitespace;

use cairn_model::{ChangedFile, CommitDetails, DiffLimits, FileDiff, Oid};

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

/// How rename and copy detection went, so a view can say when it was cut short (R2.2).
///
/// Detection is what the user's `diff.renames` asks for — off, renames, or renames and
/// copies — searched by git under `diff.renameLimit`, exactly as their own `git show` would.
/// Whether the limit cut the search short is decided from git's answer, never from its
/// stderr (`diff/renames.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenameDetection {
    /// False when `diff.renames` is off, and then every other field is empty.
    pub enabled: bool,
    /// Copies are detected only when `diff.renames` asks for them.
    pub copies: bool,
    /// The limit git applied: `diff.renameLimit`, or git's own default when it is not set.
    /// `None` when nothing limited the search.
    pub limit: Option<u32>,
    /// When the limit stopped git's exhaustive search: the limit that would have let it
    /// run, which is the number git's own warning asks the user to raise it to.
    pub needed_limit: Option<usize>,
}

impl RenameDetection {
    /// Whether `diff.renameLimit` stopped the search before it was exhaustive — the fact
    /// git prints as "exhaustive rename detection was skipped due to too many files". The
    /// answer then holds only the pairs git's cheap stages found, as git's own does.
    pub fn was_cut_short(self) -> bool {
        self.needed_limit.is_some()
    }
}

/// What a commit or a comparison changed (R2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    /// Sorted by path, a rename or a copy under its destination. The order is total, so
    /// two runs of the same query list the same files in the same places.
    pub files: Vec<ChangedFile>,
    /// Present when one commit was named, absent for a comparison of two (R2.1, R7.3).
    pub details: Option<CommitDetails>,
    pub renames: RenameDetection,
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
    /// produces (R2.8). The lines drawn are always the original bytes, and the patch
    /// emitter can reach none of this by construction (R1.7).
    pub ignore_whitespace: bool,
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

    /// What one changed file's change turned out to be (R2.3 through R2.8).
    pub fn file_diff(
        &mut self,
        file: &ChangedFile,
        options: &ContentOptions,
    ) -> Result<FileDiff, Error> {
        content::file_diff(self.repo, &mut self.cache, file, options)
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

    /// One content query, on a session of its own.
    pub fn file_diff(
        &self,
        file: &ChangedFile,
        options: &ContentOptions,
    ) -> Result<FileDiff, Error> {
        self.diff_session()?.file_diff(file, options)
    }
}
