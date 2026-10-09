use std::path::PathBuf;
use std::process::ExitStatus;

use crate::ops::GitVersion;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no git repository at {path}")]
    NotARepository { path: PathBuf },

    /// Opening a repository found another git directory than the one it had checked: a
    /// `.git` file rewritten between the ownership check's read of it and gix's, or —
    /// opening again from the path it was opened from — a repository that is not the one
    /// the application holds. Nothing read from the new one may be answered as the one
    /// checked; `was` is that one.
    #[error(
        "{path} now names the repository at {now}, not the one at {was} the application \
         checked and opened"
    )]
    RepositoryReplaced {
        path: PathBuf,
        was: PathBuf,
        now: PathBuf,
    },

    #[error("failed to open the repository at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The repository was found by searching upwards from the directory Cairn was asked to
    /// open, and is a bare one that the user's `safe.bareRepository = explicit` tells their
    /// own `git` to refuse — the protection against a bare repository planted inside a
    /// cloned working tree, whose configuration names programs to run. It is not opened,
    /// and no `git` runs in it. `path` is its git directory.
    #[error(
        "{path} is a bare repository found by searching, which git refuses because \
         safe.bareRepository is explicit; it was not opened"
    )]
    BareRepositoryFoundBySearching { path: PathBuf },

    /// The repository was found by searching, and the user's own `git` refuses it for
    /// dubious ownership: a path git checks — the `.git` file, the working tree's top, the
    /// git directory — is not the current user's, and `safe.directory`, in the
    /// configuration git protects, does not name it (`crate::ownership`). It is not
    /// opened, and no `git` runs in it; `path` is what git names in its refusal, the
    /// working tree's top or, for a bare repository, its git directory. The remedy is
    /// git's own: `git config --global --add safe.directory <path>`.
    #[error(
        "detected dubious ownership in repository at {path}: it is not all yours, and \
         safe.directory does not name it, so git refuses it; it was not opened (git config \
         --global --add safe.directory {path} adds an exception)"
    )]
    DubiousOwnership { path: PathBuf },

    /// Whether the repository is the current user's could not be decided, because the
    /// process's effective uid could not be read by any route Cairn has (`/proc/self/status`,
    /// then the owner of a file it creates; `crate::ownership`) — git's own `geteuid` cannot
    /// fail — and `safe.directory` does not name it. It is not opened, and no `git` runs in
    /// it; `path` is the working tree's top or, for a bare repository, its git directory.
    /// Naming it in `safe.directory` opens it, as it would for git whoever owns it.
    #[error(
        "could not tell which user Cairn is running as, so whether git would open the \
         repository at {path} — which it does only for its owner, or where safe.directory \
         names it — cannot be decided; it was not opened (git config --global --add \
         safe.directory {path} names it)"
    )]
    CurrentUserUnknown { path: PathBuf },

    /// The repository keeps its refs somewhere other than in files — git's reftable, which
    /// `extensions.refStorage` names in the repository's own configuration — and Cairn reads
    /// refs only as files, so it was not opened: gix would open it and then fail on the
    /// first ref it read (`crate::ref_storage`). `path` is the working tree's top or, for a
    /// bare repository, its git directory; `storage` is the setting as configured. The
    /// user's own `git` reads it; Cairn does not yet.
    #[error(
        "the repository at {path} keeps its refs in {storage} (extensions.refStorage), which \
         Cairn does not read; it was not opened"
    )]
    RefStorageUnsupported { path: PathBuf, storage: String },

    /// The repository's configuration sets `extensions.refStorage` while its
    /// `core.repositoryFormatVersion` is 0, and git refuses such a repository ("repo version
    /// is 0, but v1-only extension found"), whatever the storage named — so it was not
    /// opened either. `path` is the working tree's top or, for a bare repository, its git
    /// directory; `storage` is the setting as configured.
    #[error(
        "the repository at {path} sets extensions.refStorage = {storage} but its \
         core.repositoryFormatVersion is 0, which git refuses (a version-1 extension in a \
         version-0 repository); it was not opened"
    )]
    RefStorageNeedsFormatVersion1 { path: PathBuf, storage: String },

    /// The system or global configuration, which says whether a bare repository found by
    /// searching may be opened, could not be read; git refuses to work until it can.
    #[error("failed to read the system or global git configuration: {source}")]
    ProtectedConfig {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// `walked` is how many commits were laid out before stopping.
    #[error("the history query was cancelled after {walked} commits")]
    Cancelled { walked: usize },

    /// `HEAD` points at a branch with no commits yet.
    #[error("the repository at {path} has no commits yet")]
    UnbornHead { path: PathBuf },

    /// A changes query was cancelled — superseded before or while `git diff-tree` ran, which
    /// is then ended — and `changed` is how many files it had read of git's answer. Not a
    /// failure to report as one: the caller asked for this by superseding it.
    #[error("the changes query was cancelled after {changed} files")]
    ChangesCancelled { changed: usize },

    /// Counting what a discard of files would lose was cancelled — a newer ask superseded
    /// it, the view that asked moved on, or the repository is closing — between paths or
    /// while a path's `git` read ran, which is then ended. No prompt is offered for it, and
    /// it is not a failure to report: the caller asked for this.
    #[error("counting what the discard would lose was cancelled")]
    ConsequenceCancelled,

    /// A content query was cancelled — superseded before or while its `git` read ran, which
    /// is then ended. Not a failure to report as one: the caller asked for this by
    /// superseding it.
    #[error("the content query was cancelled")]
    ContentCancelled,
    /// `git`'s diff of a file printed lines that are not the lines Cairn read of it: the
    /// content changed between the two reads, or one of them read something else. Neither
    /// answer is used, since drawing either could show a diff of content that is not there;
    /// asking again is the remedy. `detail` says which line differed.
    #[error("git's diff of {path} does not match the content read for it ({detail}); ask again")]
    ContentReadsDisagree { path: String, detail: String },
    /// `git` answered a read in a shape Cairn does not read — a record cut off, or a status
    /// or a mode git does not print for the question asked. Nothing of the answer is used:
    /// a guess could put a wrong row in front of the user.
    #[error("git {arguments} answered with a record Cairn cannot read: {record}")]
    UnexpectedGitOutput { arguments: String, record: String },

    /// A working-tree query was asked about a path that is not relative to the working
    /// tree as git holds one: empty, absolute, or with a `.` or `..` component. Nothing
    /// was read and no `git` ran — such a path could name a file outside the working tree.
    #[error("{path:?} is not a path inside the working tree, relative to its top")]
    NotAWorkTreePath { path: String },

    /// A configuration value git itself refuses, so the user's own `git log` and
    /// `git show` refuse to answer too until it is changed. `value` is as configured.
    #[error("the configuration value {key} = {value} is not one git accepts")]
    InvalidConfig { key: String, value: String },

    /// The diff machinery could not be built for this repository — the index or the
    /// attribute stack could not be read. No query ran.
    #[error("failed to prepare the repository for diffing: {source}")]
    DiffSetup {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// One path's content could not be read or compared; every other path in the same
    /// change set is unaffected.
    #[error("failed to diff {path}: {source}")]
    DiffFile {
        path: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Usually a corrupt or missing object.
    #[error("failed to walk the history: {source}")]
    Walk {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("failed to read commit {id}: {source}")]
    ReadCommit {
        id: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// No executable `git` in any directory of the `PATH` Cairn hands to
    /// subprocesses; `searched` is every directory looked in, in order.
    #[error(
        "git was not found on PATH (searched {}); Cairn needs git {required} or newer",
        Directories(searched)
    )]
    GitNotFound {
        searched: Vec<PathBuf>,
        required: GitVersion,
    },

    /// Raising `required` is a support-policy decision, not an implementation one.
    #[error("git {found} at {path} is too old; Cairn needs git {required} or newer")]
    GitTooOld {
        path: PathBuf,
        found: GitVersion,
        required: GitVersion,
    },

    /// `git --version` ran but printed something that is not a version.
    #[error(
        "could not read the version of git at {path}: `git --version` printed {output:?}; \
         Cairn needs git {required} or newer"
    )]
    GitVersionUnreadable {
        path: PathBuf,
        output: String,
        required: GitVersion,
    },

    /// The process never ran: the file is gone, not executable, or the OS refused.
    #[error("could not start {program}: {source}")]
    GitNotStarted {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// git ran and exited non-zero. `stderr` is git's own diagnostic, for the
    /// user: the last 256 KiB of it, whatever it said before that dropped.
    /// `present_locks` is every `*.lock` under the git directory once a WRITE
    /// had failed — the file another git holds, or a stale one from a crash,
    /// which is what a write fails on and git does not wait for. Never retried
    /// and never removed. Filled for a write run in a repository (fetch among
    /// them); empty for a read, the version probe included, and for a write
    /// given no repository to look in.
    #[error(
        "git {arguments} failed ({status}): {stderr}{}",
        PresentLocks(present_locks)
    )]
    GitFailed {
        arguments: String,
        status: ExitStatus,
        stderr: String,
        present_locks: Vec<PathBuf>,
    },

    /// A read was cancelled — superseded, cancelled through its handle, or
    /// dropped — and its process ended. Nothing else to report: a read writes
    /// nothing, so it can leave nothing behind.
    #[error("git {arguments} was cancelled")]
    GitReadCancelled { arguments: String },

    /// git wrote more to stdout than the caller said it would take, so the
    /// process was ended and nothing it wrote is returned: an answer cut short
    /// would look like a whole one. Only a read is given a ceiling (the
    /// bounded-output helpers exist on a read alone), so it leaves no lock
    /// behind to list.
    #[error(
        "git {arguments} wrote more than {ceiling} bytes; the answer was refused, not cut \
         short"
    )]
    GitOutputTooLarge { arguments: String, ceiling: usize },

    /// Cairn lost hold of a running git. A thread to read or feed one of its
    /// pipes could not start, or writing its input failed: the process was
    /// ended rather than left running with nobody watching, or acting on part
    /// of its input. Or waiting on it failed, and what became of it is not
    /// known. A write ended this way may still have taken effect, in part or
    /// whole — its input may have been delivered before it was ended — so an
    /// operation that must know compares the repository's state before and
    /// after. For a write, `stranded_locks` lists the lock files present
    /// afterwards; empty for a read.
    #[error(
        "lost hold of git {arguments}: {source}{}",
        StrandedLocks(stranded_locks)
    )]
    GitUnwatched {
        arguments: String,
        #[source]
        source: std::io::Error,
        stranded_locks: Vec<PathBuf>,
    },

    /// The user cancelled the operation and the process was ended. Not a
    /// failure to report as one: the caller asked for this. Not a promise that
    /// nothing happened either: the signal can land after git made its change
    /// and before it exited, so a cancelled write may have taken effect in part
    /// or in whole, and an operation that must know compares the repository's
    /// state before and after, as fetch does with its refs. `stranded_locks`
    /// is every `*.lock` found under the git directory once the process was
    /// gone. A `SIGKILL` that landed mid-write leaves one, and so does an
    /// earlier crash — and so does a git running in a terminal right now,
    /// which the search cannot tell apart; each is what a later write to
    /// that file fails on while it is there. Empty in the common case.
    #[error("git {arguments} was cancelled{}", StrandedLocks(stranded_locks))]
    GitCancelled {
        arguments: String,
        stranded_locks: Vec<PathBuf>,
    },

    /// A status read was cancelled — superseded before or while `git status` ran, which is
    /// then ended with its process group. Not a failure to report as one: the caller asked
    /// for this by superseding it.
    #[error("the status read was cancelled")]
    StatusCancelled,

    /// A refs query was cancelled — superseded before it finished reading. Not a failure
    /// to report as one: the caller asked for this by superseding it.
    #[error("the refs query was cancelled")]
    RefsCancelled,

    /// An ahead/behind query was cancelled — superseded while it walked — after answering
    /// `branches` local branches and reading `commits_read` commit objects (the cost it
    /// paid before it stopped); what it had counted is dropped with it. Not a failure to
    /// report as one.
    #[error(
        "the ahead/behind query was cancelled after {branches} branches and {commits_read} \
         commits read"
    )]
    AheadBehindCancelled {
        branches: usize,
        commits_read: usize,
    },

    /// Reading the refs failed: the store could not be listed, or `HEAD` could not be read.
    /// One ref that cannot be read is skipped and counted instead (`RefsSnapshot::unreadable`).
    #[error("failed to read the refs of the repository: {source}")]
    Refs {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The remote's configuration would have a fetch write where Cairn's
    /// never does, so no process was started. `setting` is the entry as
    /// configured (a refspec, or `remote.<name>.mirror`), `write` what it
    /// would have done; the caller shows both, and the user changes the
    /// setting or fetches from a terminal.
    #[error(
        "fetch from {remote} refused: {setting} would {write}; change that setting or fetch \
         from a terminal"
    )]
    FetchRefused {
        remote: String,
        setting: String,
        write: RefusedWrite,
    },

    /// The index file could not be read, so what a write is checked against is unknown and
    /// nothing was written.
    #[error("failed to read the index: {source}")]
    ReadIndex {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// A working-tree file could not be read to be hashed, so what a write is checked
    /// against is unknown and nothing was written.
    #[error("failed to read {path} in the working tree: {source}")]
    ReadWorkingTree {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// A write verb was refused before any `git` ran, because what it was asked to do is
    /// not something it does (`docs/prd/staging-and-commit.md` R3): `why` says what, for
    /// the caller to show. Nothing was written.
    #[error("{path}: {why}; nothing was written")]
    Refused { path: String, why: Refusal },

    /// A verb of files was given no path at all. Nothing was written, and for a discard no
    /// confirmation can be made: a prompt that counts nothing is refused before one is
    /// offered.
    #[error("no path was given; nothing was written")]
    NoPaths,

    /// What a patch was built from moved after it was read (R3.7): the index entry, or for
    /// a discard the working-tree file, is no longer the side the drawn diff names. The
    /// patch was not applied — `git apply` could otherwise land it at an offset, on lines
    /// the user never saw — and nothing was written; the remedy is to read the diff again.
    #[error("{path} changed after its diff was read; nothing was written")]
    ChangedSinceRead { path: String },

    /// What a destructive operation's `Consequence` names moved between the confirmation
    /// and the run (R1.4): a file edited, replaced or removed, an entry restaged. The
    /// operation refused, writing nothing — an outcome of its own, never git's failure.
    #[error("{path} changed since you confirmed; nothing was discarded")]
    ChangedSinceConfirmed { path: String },

    /// A discard of files ran but did not discard every file it was confirmed for: a `git`
    /// failed part way — `git clean` deletes what it can and exits non-zero on the rest,
    /// after `git restore` has run — or git left a file it does not touch (an ignored file,
    /// under `git clean -f`). Not a bare failure, since something was destroyed:
    /// `performed` is the record of the operation, quoting the prompt the user accepted;
    /// `kept` is every confirmed path still exactly as it was confirmed, found by reading
    /// each again after the run, never by parsing git's prose; `failure` is git's error,
    /// when one failed. The refresh after it shows the rest.
    #[error(
        "the discard did not take every file: {} kept as {} were{}",
        Paths(kept),
        if kept.len() == 1 { "it" } else { "they" },
        Failure(failure.as_deref())
    )]
    DiscardIncomplete {
        performed: Box<crate::ops::Performed>,
        kept: Vec<String>,
        failure: Option<Box<Error>>,
    },

    /// Create Branch's discard before its checkout was refused before any confirmation or any
    /// `git` ran (`docs/prd/staging-and-commit.md` R11.3): `why` says what, for the caller to
    /// show. Nothing was written.
    #[error("{why}; nothing was written")]
    CheckoutRefused { why: CheckoutRefusal },

    /// A commit or an amend was refused before any `git` ran (`docs/prd/staging-and-commit.md`
    /// R6.1, R6.3, R6.9): `why` says what, for the caller to show. Nothing was written.
    #[error("{why}; nothing was committed")]
    CommitRefused { why: CommitRefusal },

    /// What an amend's `Consequence` names moved between the confirmation and the run
    /// (R1.4): `HEAD` is another commit, a remote has it now, or the reflog setting changed.
    /// The amend refused, writing nothing — an outcome of its own, never git's failure.
    #[error("HEAD changed since you confirmed the amend; nothing was amended")]
    AmendChangedSinceConfirmed,

    /// `Remove index.lock…` refused before touching anything (staging-and-commit R12.4): no
    /// lock to remove, one that is not a plain file, a `git` Cairn runs in the repository, or
    /// a confirmation of another operation.
    #[error("index.lock was not removed: {why}")]
    LockRefused { why: LockRefusal },

    /// The lock a removal was confirmed for was removed, replaced or written since the
    /// confirmation — its size, its time or the file itself differs (R1.4, R12.4). Nothing was
    /// removed: the lock now there may be a running git's.
    #[error("{path} changed since you confirmed; nothing was removed")]
    LockChangedSinceConfirmed { path: String },

    /// The lock could not be removed: the filesystem refused.
    #[error("could not remove {path}: {source}")]
    LockNotRemoved {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// A commit or an amend was cancelled before its `git` started — while what it checks
    /// first was still being read. Nothing was written.
    #[error("the commit was cancelled before git ran; nothing was written")]
    CommitCancelledBeforeRunning,

    /// The remote's configuration could not be read — `git config` failed, was
    /// cancelled or answered what it never prints, or a configured refspec does
    /// not parse — so the fetch could not be checked and did not run.
    #[error("could not read the configuration of remote {remote}: {source}")]
    RemoteConfig {
        remote: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Why a fetch was refused; see [`Error::FetchRefused`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusedWrite {
    /// A destination under `refs/heads/`: local branches, overwritten with
    /// no reflog in a bare repository.
    LocalBranches,
    /// A destination under `refs/tags/` while pruning is on: local tags,
    /// which have no reflog, deleted. The `setting` beside it names the
    /// refspec and the prune setting that together would do it.
    LocalTags,
    /// `remote.<name>.mirror` is set. git reads it for push, not fetch, so
    /// the setting itself writes nothing on a fetch; it is refused on sight
    /// because it is what `git clone --mirror` leaves beside the
    /// `+refs/*:refs/*` refspec, and a mirror is a repository whose every
    /// ref is the remote's to overwrite (decided on issue #17).
    Mirror,
    /// The remote is defined by a file git reads in place of configuration,
    /// `$GIT_DIR/remotes/<name>` or `$GIT_DIR/branches/<name>`, which no query
    /// of git's prints; a `branches/` file fetches into a local branch named
    /// after the remote. The `setting` beside it is the file's path.
    DefinedByFile,
}

impl std::fmt::Display for RefusedWrite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::LocalBranches => {
                "write local branches (refs/heads/), which Cairn's fetch never does"
            }
            Self::LocalTags => "delete local tags while pruning, which Cairn's fetch never does",
            Self::Mirror => {
                "declare the remote a mirror, whose fetch Cairn refuses on sight (the setting \
                 itself governs push; it is what a mirror clone carries)"
            }
            Self::DefinedByFile => {
                "be read in place of the remote's configuration, which Cairn refuses on sight \
                 (a branches/ file fetches into a local branch; define the remote with `git \
                 remote add` and remove the file)"
            }
        })
    }
}

/// Why a write verb refused before any `git` ran; see [`Error::Refused`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A verb of lines was given a selection of nothing: no line and no mode change.
    NothingSelected,
    /// The change is taken whole, by a file verb, never by a patch of part of it — a
    /// deletion, a type change, a state that is not text — or the selection is every line
    /// of a file that is new, whose discard deletes it: that is the file verb's, whose
    /// prompt says the file is deleted (R2.3, L8).
    WholeFileOnly,
    /// The path is conflicted: it is staged whole with `git add`, which marks it resolved,
    /// and takes no patch and no discard by any verb (R3.11, L25).
    Conflicted,
    /// A submodule's changes are never discarded: `git restore` leaves its commit where it
    /// was, only `git submodule update` moves it back, and no prompt can count what is
    /// dirty inside it (R3.10, L24).
    Submodule,
    /// An untracked directory holding a repository of its own: deleting it would take
    /// every commit only it has, so it is refused before any confirmation (R3.5, L8).
    NestedRepository,
    /// The path is a directory or a special file, not a file `git status` lists one per
    /// file: nothing a discard deletes or restores.
    NotAFile,
    /// A directory on the path is not a real directory — a file, or a symlink — so the path
    /// is not in the working tree as git holds it: restoring it would replace what stands
    /// there (a file no prompt named, or a link), and hashing it would read through a link
    /// to a file outside the working tree.
    Obstructed,
    /// The path has no unstaged change — the working tree matches the index — so there is
    /// nothing to discard; staged changes are never discarded (R3.6).
    NoUnstagedChange,
    /// A destructive operation was handed a confirmation of another operation: a discard
    /// of lines given a discard of files' token, or the reverse.
    NotWhatWasConfirmed,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NothingSelected => "nothing is selected",
            Self::WholeFileOnly => {
                "this change is staged, unstaged or discarded as a whole file, never in part"
            }
            Self::Conflicted => {
                "the path is conflicted, so it is staged whole and is never patched or discarded"
            }
            Self::Submodule => {
                "a submodule's changes are never discarded: only `git submodule update` moves \
                 its commit back, and nothing can count what is changed inside it"
            }
            Self::NestedRepository => {
                "a repository nested in the working tree is never deleted: it would take every \
                 commit only it has"
            }
            Self::NotAFile => "the path is not a file: a directory or a special file",
            Self::Obstructed => {
                "a directory on the path is a file or a symlink, which restoring the path would \
                 replace"
            }
            Self::NoUnstagedChange => {
                "the path has no unstaged change to discard, and staged changes are never \
                 discarded"
            }
            Self::NotWhatWasConfirmed => {
                "what the discard was handed is not what its confirmation names"
            }
        })
    }
}

/// Why Create Branch's discard refused before any `git` ran; see [`Error::CheckoutRefused`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckoutRefusal {
    /// git is in the middle of an operation — a merge, a rebase, `git am`, a cherry-pick or a
    /// revert — whose state a forced checkout would leave behind or lose.
    InProgress(cairn_model::OperationInProgress),
    /// No tracked file has a staged or an unstaged change: there is nothing to discard.
    NothingToDiscard,
    /// The discard was handed a confirmation of another operation.
    NotWhatWasConfirmed,
}

impl std::fmt::Display for CheckoutRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InProgress(operation) => write!(
                f,
                "{} is in progress: continue or abort it first",
                operation.name()
            ),
            Self::NothingToDiscard => f.write_str("there are no changes to discard"),
            Self::NotWhatWasConfirmed => {
                f.write_str("what the discard was handed is not what its confirmation names")
            }
        }
    }
}

/// Why a commit or an amend refused before any `git` ran; see [`Error::CommitRefused`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitRefusal {
    /// git is in the middle of an operation a commit or an amend is refused during (L25): a
    /// rebase, `git am`, a cherry-pick or a revert — or, for an amend, a merge, which git
    /// itself refuses to amend in.
    InProgress(cairn_model::OperationInProgress),
    /// The branch has no commit yet, so there is nothing to amend (R6.3).
    NothingToAmend,
    /// `i18n.commitEncoding` names an encoding other than UTF-8 (R6.1): git stores the
    /// message's bytes as they are under that encoding's name, so a UTF-8 draft would be
    /// read back as other characters (git-write-verbs.md §5). The value as configured.
    CommitEncoding { encoding: String },
    /// An amend was handed a confirmation of another operation.
    NotWhatWasConfirmed,
}

impl std::fmt::Display for CommitRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InProgress(operation) => write!(
                f,
                "{} is in progress: continue or abort it first",
                operation.name()
            ),
            Self::NothingToAmend => f.write_str("the branch has no commit to amend yet"),
            Self::CommitEncoding { encoding } => write!(
                f,
                "i18n.commitEncoding is {encoding:?}, and Cairn writes a message only as UTF-8"
            ),
            Self::NotWhatWasConfirmed => {
                f.write_str("what the amend was handed is not what its confirmation names")
            }
        }
    }
}

/// Why `Remove index.lock…` refused; see [`Error::LockRefused`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockRefusal {
    /// There is no `index.lock` in the repository's git directory.
    NoLock,
    /// `index.lock` is not a plain file (a directory, a symbolic link).
    NotAFile,
    /// Cairn runs this many `git` invocations in the repository now; the lock may be one of
    /// theirs.
    GitRunning(usize),
    /// The removal was handed a confirmation of another operation, or of another repository's
    /// lock.
    NotWhatWasConfirmed,
}

impl std::fmt::Display for LockRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoLock => f.write_str("there is no index.lock"),
            Self::NotAFile => f.write_str("index.lock is not a file"),
            Self::GitRunning(1) => f.write_str("Cairn is running git in this repository"),
            Self::GitRunning(n) => {
                write!(f, "Cairn is running {n} git processes in this repository")
            }
            Self::NotWhatWasConfirmed => {
                f.write_str("what the removal was handed is not what its confirmation names")
            }
        }
    }
}

/// Lock files in a message: nothing when there are none, otherwise the
/// sentence a user needs to act on them, naming each path.
struct StrandedLocks<'a>(&'a [PathBuf]);

impl std::fmt::Display for StrandedLocks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }
        f.write_str(
            "; lock files remain under the git directory, which later writes will fail on while \
             they are there (stale if no git is running here): ",
        )?;
        for (i, path) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", path.display())?;
        }
        Ok(())
    }
}

/// Lock files present when a write failed: nothing when there are none,
/// otherwise the sentence naming each, which says what the user can do.
struct PresentLocks<'a>(&'a [PathBuf]);

impl std::fmt::Display for PresentLocks<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }
        f.write_str(
            "; lock files are present under the git directory — another git process is \
             running here, or one was stopped before it could remove them: ",
        )?;
        for (i, path) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", path.display())?;
        }
        Ok(())
    }
}

/// Paths in a message, comma-separated.
struct Paths<'a>(&'a [String]);

impl std::fmt::Display for Paths<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return f.write_str("nothing");
        }
        f.write_str(&self.0.join(", "))
    }
}

/// A failure beside a partial outcome: nothing when there was none.
struct Failure<'a>(Option<&'a Error>);

impl std::fmt::Display for Failure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(error) => write!(f, "; {error}"),
            None => Ok(()),
        }
    }
}

/// A search path in a message: `/usr/local/bin, /usr/bin`, or `nothing` when
/// `PATH` was unset.
struct Directories<'a>(&'a [PathBuf]);

impl std::fmt::Display for Directories<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return f.write_str("nothing: PATH is unset");
        }
        for (i, directory) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{}", directory.display())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R5.3: a failed write's message names the lock files present, and a failure
    /// with none reads as before.
    #[test]
    fn a_failure_names_the_locks_present_and_is_silent_when_there_are_none() {
        use std::os::unix::process::ExitStatusExt as _;
        let status = ExitStatus::from_raw(128 << 8);
        let clean = Error::GitFailed {
            arguments: "add x".to_owned(),
            status,
            stderr: "fatal: no".to_owned(),
            present_locks: Vec::new(),
        };
        assert_eq!(
            clean.to_string(),
            "git add x failed (exit status: 128): fatal: no"
        );
        let locked = Error::GitFailed {
            arguments: "add x".to_owned(),
            status,
            stderr: "fatal: Unable to create index.lock".to_owned(),
            present_locks: vec![PathBuf::from("/r/.git/index.lock")],
        };
        let text = locked.to_string();
        assert!(text.contains("lock files are present"), "{text}");
        assert!(text.ends_with("/r/.git/index.lock"), "{text}");
    }

    /// Issue #19: the message names every stranded lock, and says nothing about locks
    /// when there are none — the common case, which must read as before.
    #[test]
    fn a_cancellation_names_what_it_stranded_and_is_silent_when_nothing_was() {
        let clean = Error::GitCancelled {
            arguments: "fetch origin".to_owned(),
            stranded_locks: Vec::new(),
        };
        assert_eq!(clean.to_string(), "git fetch origin was cancelled");

        let stranded = Error::GitCancelled {
            arguments: "fetch origin".to_owned(),
            stranded_locks: vec![
                PathBuf::from("/r/.git/packed-refs.lock"),
                PathBuf::from("/r/.git/refs/remotes/origin/main.lock"),
            ],
        };
        let text = stranded.to_string();
        assert!(
            text.starts_with("git fetch origin was cancelled; "),
            "{text}"
        );
        assert!(text.contains("lock file"), "{text}");
        assert!(
            text.ends_with("/r/.git/packed-refs.lock, /r/.git/refs/remotes/origin/main.lock"),
            "{text}"
        );
    }
}
