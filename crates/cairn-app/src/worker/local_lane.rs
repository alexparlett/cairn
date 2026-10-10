//! The local write lane, `cairn-local`: the thread where every write to the index, the
//! working tree and a local ref runs — staging, unstaging, discarding, commit and amend —
//! one at a time, in the order asked (staging-and-commit R4,
//! `docs/design/concurrency.md`, "Operations").
//!
//! A write reaches this thread straight from `RepositoryHandle::submit`, never through the
//! repository thread, where it would wait behind a deep find (R4.1). It is queued behind the
//! write running, never refused for being second (R4.2), and carries an [`OperationId`] the
//! window chose as it asked, so the window draws it queued, then running, then ended, and a
//! cancel names it (R4.3): only a commit or an amend is cancellable, and only while it runs,
//! so a cancel that arrives late never reaches the write queued behind the one it named. A
//! cancel that comes while an amend's checks still read stops it before git starts; once git
//! runs, the commit's [`ops::CommitCancel`] is installed under the write's id, and a cancel
//! calls it under the lane's lock — `KillHandle::kill`, an atomic mark, a `try_lock` and a
//! `killpg`, which never waits on the process (`cairn-git`'s `process/group.rs`).
//!
//! What the lane shares with the other threads is [`LaneState`], one mutex held for an
//! assignment and a send:
//!
//! - **The write clock** (R4.4). It ticks as a write starts and as it ends, so it is odd while
//!   one runs. A status is read only while it is even, and is sent only if it has not ticked
//!   since the read began — checked and sent under the lock the lane ticks and announces
//!   under, so a status the window draws was read entirely between two writes, and arrives
//!   before the next write's start. One that began before a write ended is dropped, never
//!   drawn: the window reads status again after every write's ending, which shows it.
//! - **Quiet while a commit runs** (R4.6). A refresh asked of the handle, or reaching the
//!   repository or refresh thread, while a commit runs starts nothing, and is dropped: a
//!   commit's ending always says to read everything again — whatever it did, since a hook may
//!   have moved anything — so nothing a refresh would have shown is lost.
//! - **The running write**, its id and its cancel.
//!
//! Every write carries an askpass token of its own (R5.1, L11), issued under the name the
//! window gives the write ("Commit"): a hook, a signing program or an LFS filter it runs may
//! ask, and the window shows the prompt titled by that name while the write runs. A commit's
//! output — its hooks' lines — reaches the window as it arrives (`Update::WriteOutput`, R6.5).
//!
//! Closing the window during a write waits for it (R4.9): the repository thread, closing,
//! marks the lane closing, which runs nothing more it is sent, and waits for the write
//! running to end before it ends what else `git` runs. The window says which write it is
//! waiting for, and a second close after `CLOSE_PATIENCE` closes it anyway; a lock the write
//! leaves then is listed as the repository next opens (`Update::LocksAtOpen`).

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use cairn_askpass::Channel;
use cairn_git::ops::{self, CommitWatch, GitBinary, Hooks, Invalidated, Performed, UnstageTo};
use cairn_git::{Error, Repository, SharedRepository};
use cairn_model::{
    AskpassToken, ChangeList, CommandRecord, Confirmed, FileDiff, LocalChanges, Oid, RepoPath,
    ScrubbedLines, Selection,
};

use super::epoch::{Epoch, Superseded};
use super::output_flow::{OutputFlow, OutputReceipt};
use super::pool::Outbox;
use super::request::{AmendRead, CommitReads, RanBy, Update};

/// Names one local write from the moment the window asks for it: what its start, its
/// ending and a cancel of it name. Never reused within the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperationId(u64);

impl OperationId {
    /// A fresh id: an atomic increment, so the window can call it as it asks.
    pub fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    /// An id `next` did not issue, for a test that names one.
    #[cfg(test)]
    pub fn for_tests(n: u64) -> Self {
        Self(n)
    }
}

/// What a whole-file unstage puts the index entries back to: `cairn_git::ops::UnstageTo`, in
/// the window's words, since the window may not name the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnstageTarget {
    /// `HEAD`'s entries, or none on an unborn branch.
    Head,
    /// A commit's entries: `HEAD`'s parent, out of an amend.
    Commit(Oid),
    /// No entry at all: out of a root commit's amend.
    Nothing,
}

impl From<UnstageTarget> for UnstageTo {
    fn from(target: UnstageTarget) -> Self {
        match target {
            UnstageTarget::Head => Self::Head,
            UnstageTarget::Commit(commit) => Self::Commit(commit),
            UnstageTarget::Nothing => Self::Nothing,
        }
    }
}

/// A write the local lane runs: one of `cairn_git::ops`' local verbs and what it is given. A
/// destructive one carries the `Confirmed` the user gave it, spent by the verb.
#[derive(Debug, PartialEq, Eq)]
pub enum LocalWrite {
    /// `selection` of `diff` — the path's unstaged or untracked diff — into the index.
    StageLines {
        diff: Arc<FileDiff>,
        selection: Selection,
    },
    /// `selection` of `diff` — the path's staged diff — out of the index.
    UnstageLines {
        diff: Arc<FileDiff>,
        selection: Selection,
    },
    /// Every one of `paths`, whole, into the index.
    StageFiles { paths: Vec<RepoPath> },
    /// Every one of `paths`, whole, out of the index, back to `to`.
    UnstageFiles {
        paths: Vec<RepoPath>,
        to: UnstageTarget,
    },
    /// Every path of `changes`' Unstaged list — or, with a filter on, of the rows it shows,
    /// `shown` (the user's decision, 2026-10-09) — whole, into the index: Stage All (R8.2). The
    /// paths are gathered here, on the lane, never on the UI thread — a status can list tens
    /// of thousands.
    StageAll {
        changes: Arc<LocalChanges>,
        shown: Option<Vec<u32>>,
    },
    /// Every path of `changes`' Staged list, or of its rows `shown`, whole, out of the index,
    /// back to `to`: Unstage All.
    UnstageAll {
        changes: Arc<LocalChanges>,
        shown: Option<Vec<u32>>,
        to: UnstageTarget,
    },
    /// The lines the confirmation names, discarded from the working tree.
    DiscardLines(Confirmed),
    /// The files the confirmation names, discarded, untracked ones deleted.
    DiscardFiles(Confirmed),
    /// What is staged, committed with `message` (R6.1); `skip_hooks` only from the hook
    /// failure's skip (R10.5).
    Commit { message: String, skip_hooks: bool },
    /// `HEAD` amended with `message` and what is staged (R6.2), under the confirmation of the
    /// `Consequence` the commit box drew.
    Amend {
        confirmed: Confirmed,
        message: String,
        skip_hooks: bool,
    },
    /// The branch `name` put on `at`: Create Branch, unticked (staging-and-commit R11.3).
    CreateBranch { name: String, at: Oid },
    /// The branch `name` put on `at` and checked out, the working tree's changes carried over
    /// or git refusing: Create Branch's "Don't change".
    CreateBranchAndCheckout { name: String, at: Oid },
    /// The branch and the commit the confirmation names, checked out with every change it
    /// names discarded: Create Branch's "Discard" (the user's decision 3, 2026-10-09).
    CreateBranchDiscarding(Confirmed),
    /// The stale `<gitdir>/index.lock` the confirmation names, removed: `Remove index.lock…`
    /// (staging-and-commit R12.4), the one mutation not made by git.
    RemoveLock(Confirmed),
}

/// A test's copy of a write it asked for, to compare with what was sent. A destructive write
/// is never copied: its confirmation is spent once, and a test that tries fails.
#[cfg(test)]
impl Clone for LocalWrite {
    fn clone(&self) -> Self {
        match self {
            Self::StageLines { diff, selection } => Self::StageLines {
                diff: diff.clone(),
                selection: selection.clone(),
            },
            Self::UnstageLines { diff, selection } => Self::UnstageLines {
                diff: diff.clone(),
                selection: selection.clone(),
            },
            Self::StageFiles { paths } => Self::StageFiles {
                paths: paths.clone(),
            },
            Self::UnstageFiles { paths, to } => Self::UnstageFiles {
                paths: paths.clone(),
                to: to.clone(),
            },
            Self::StageAll { changes, shown } => Self::StageAll {
                changes: Arc::clone(changes),
                shown: shown.clone(),
            },
            Self::UnstageAll { changes, shown, to } => Self::UnstageAll {
                changes: Arc::clone(changes),
                shown: shown.clone(),
                to: to.clone(),
            },
            Self::Commit {
                message,
                skip_hooks,
            } => Self::Commit {
                message: message.clone(),
                skip_hooks: *skip_hooks,
            },
            Self::CreateBranch { name, at } => Self::CreateBranch {
                name: name.clone(),
                at: *at,
            },
            Self::CreateBranchAndCheckout { name, at } => Self::CreateBranchAndCheckout {
                name: name.clone(),
                at: *at,
            },
            Self::DiscardLines(_)
            | Self::DiscardFiles(_)
            | Self::Amend { .. }
            | Self::CreateBranchDiscarding(_)
            | Self::RemoveLock(_) => {
                panic!("a destructive write's confirmation is spent once; a test may not copy it")
            }
        }
    }
}

/// What the window reads again once a write has ended (R4.5): what its `Invalidated` names
/// and no more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadAgain {
    /// The refs, ahead/behind and status — and the history, if the refs it draws moved: a
    /// write that moved a ref, and every commit or amend however it ended, which also covers
    /// a refresh kept back while it ran.
    Everything,
    /// The working tree's status alone: a write to the index or the working tree.
    Status,
}

impl ReadAgain {
    fn after(invalidated: Invalidated) -> Self {
        if invalidated.refs {
            Self::Everything
        } else {
            Self::Status
        }
    }
}

impl LocalWrite {
    /// Whether this is a commit or an amend: cancellable while it runs, and the lane is quiet
    /// behind it.
    fn is_commit(&self) -> bool {
        match self {
            Self::StageLines { .. }
            | Self::UnstageLines { .. }
            | Self::StageFiles { .. }
            | Self::UnstageFiles { .. }
            | Self::StageAll { .. }
            | Self::UnstageAll { .. }
            | Self::DiscardLines(_)
            | Self::DiscardFiles(_)
            | Self::CreateBranch { .. }
            | Self::CreateBranchAndCheckout { .. }
            | Self::CreateBranchDiscarding(_)
            | Self::RemoveLock(_) => false,
            Self::Commit { .. } | Self::Amend { .. } => true,
        }
    }

    /// What it can leave stale, read again however it ends — a failed or cancelled write may
    /// have done part of its work (R4.7). Its `Performed` says the same of one that ran.
    fn read_again(&self) -> ReadAgain {
        match self {
            Self::StageLines { .. }
            | Self::UnstageLines { .. }
            | Self::StageFiles { .. }
            | Self::UnstageFiles { .. }
            | Self::StageAll { .. }
            | Self::UnstageAll { .. }
            | Self::DiscardLines(_)
            | Self::DiscardFiles(_)
            | Self::RemoveLock(_) => ReadAgain::Status,
            // A branch made moves the refs: the history is read again, and a commit it puts a
            // branch on is no longer lost.
            Self::Commit { .. }
            | Self::Amend { .. }
            | Self::CreateBranch { .. }
            | Self::CreateBranchAndCheckout { .. }
            | Self::CreateBranchDiscarding(_) => ReadAgain::Everything,
        }
    }

    /// What the window calls it while it is queued or running, and as it waits on it to close
    /// ("Finishing commit…", R4.9): a phrase that follows a verb.
    pub fn what(&self) -> String {
        let files = |count: usize| {
            if count == 1 {
                "1 file".to_owned()
            } else {
                format!("{count} files")
            }
        };
        match self {
            Self::StageLines { diff, .. } => format!("staging lines of {}", diff.file.new_path),
            Self::UnstageLines { diff, .. } => {
                format!("unstaging lines of {}", diff.file.new_path)
            }
            Self::StageFiles { paths } => format!("staging {}", files(paths.len())),
            Self::UnstageFiles { paths, .. } => format!("unstaging {}", files(paths.len())),
            Self::StageAll { changes, shown } => format!(
                "staging {}",
                files(
                    shown
                        .as_ref()
                        .map_or(changes.len(ChangeList::Unstaged), Vec::len)
                )
            ),
            Self::UnstageAll { changes, shown, .. } => format!(
                "unstaging {}",
                files(
                    shown
                        .as_ref()
                        .map_or(changes.len(ChangeList::Staged), Vec::len)
                )
            ),
            Self::DiscardLines(_) => "discarding lines".to_owned(),
            Self::DiscardFiles(_) => "discarding files".to_owned(),
            Self::Commit { .. } => "commit".to_owned(),
            Self::Amend { .. } => "amend".to_owned(),
            Self::CreateBranch { name, .. } | Self::CreateBranchAndCheckout { name, .. } => {
                format!("creating branch {name}")
            }
            Self::CreateBranchDiscarding(_) => "creating a branch, discarding changes".to_owned(),
            Self::RemoveLock(_) => "removing index.lock".to_owned(),
        }
    }

    /// What the activity popover calls it, in Fork's imperative form as its Activity Manager
    /// names an operation ("Fetch origin", "Create branch 'develop'"; `fork-staging-and-commit.md`
    /// §7): "Stage 1 file", "Stage lines of a.rs", "Unstage 2 files", "Discard 3 files",
    /// "Commit", "Amend", "Create branch 'topic'", "Remove index.lock". A destructive write's
    /// is its confirmation's (`Consequence::name`).
    pub fn name(&self) -> String {
        let files = |count: usize| {
            if count == 1 {
                "1 file".to_owned()
            } else {
                format!("{count} files")
            }
        };
        match self {
            Self::StageLines { diff, .. } => format!("Stage lines of {}", diff.file.new_path),
            Self::UnstageLines { diff, .. } => format!("Unstage lines of {}", diff.file.new_path),
            Self::StageFiles { paths } => format!("Stage {}", files(paths.len())),
            Self::UnstageFiles { paths, .. } => format!("Unstage {}", files(paths.len())),
            Self::StageAll { changes, shown } => format!(
                "Stage {}",
                files(
                    shown
                        .as_ref()
                        .map_or(changes.len(ChangeList::Unstaged), Vec::len)
                )
            ),
            Self::UnstageAll { changes, shown, .. } => format!(
                "Unstage {}",
                files(
                    shown
                        .as_ref()
                        .map_or(changes.len(ChangeList::Staged), Vec::len)
                )
            ),
            Self::Commit { .. } => "Commit".to_owned(),
            Self::CreateBranch { name, .. } | Self::CreateBranchAndCheckout { name, .. } => {
                format!("Create branch '{name}'")
            }
            Self::DiscardLines(confirmed)
            | Self::DiscardFiles(confirmed)
            | Self::CreateBranchDiscarding(confirmed)
            | Self::RemoveLock(confirmed)
            | Self::Amend { confirmed, .. } => confirmed.consequence().name(),
        }
    }

    /// Whether it can be cancelled while it runs: a commit or an amend (R4.3).
    pub fn is_cancellable(&self) -> bool {
        self.is_commit()
    }

    /// The prompt a destructive write's confirmation recorded — what the user accepted — copied
    /// as the write is asked, before the verb spends the token, so the activity popover quotes
    /// it however the write ends (R12.1): `None` for a write that asks no confirmation.
    pub fn prompt(&self) -> Option<String> {
        match self {
            Self::DiscardLines(confirmed)
            | Self::DiscardFiles(confirmed)
            | Self::Amend { confirmed, .. }
            | Self::CreateBranchDiscarding(confirmed)
            | Self::RemoveLock(confirmed) => Some(confirmed.prompt().to_owned()),
            Self::StageLines { .. }
            | Self::UnstageLines { .. }
            | Self::StageFiles { .. }
            | Self::UnstageFiles { .. }
            | Self::StageAll { .. }
            | Self::UnstageAll { .. }
            | Self::Commit { .. }
            | Self::CreateBranch { .. }
            | Self::CreateBranchAndCheckout { .. } => None,
        }
    }

    /// The commit an amend replaces, which Show Lost Commits draws once it has (R12.1's way
    /// back): `None` for every other write.
    pub fn replaces(&self) -> Option<Oid> {
        match self {
            Self::Amend { confirmed, .. } => confirmed.consequence().amended(),
            Self::StageLines { .. }
            | Self::UnstageLines { .. }
            | Self::StageFiles { .. }
            | Self::UnstageFiles { .. }
            | Self::StageAll { .. }
            | Self::UnstageAll { .. }
            | Self::DiscardLines(_)
            | Self::DiscardFiles(_)
            | Self::Commit { .. }
            | Self::CreateBranch { .. }
            | Self::CreateBranchAndCheckout { .. }
            | Self::CreateBranchDiscarding(_)
            | Self::RemoveLock(_) => None,
        }
    }

    /// What a wait behind it waits for — "Waiting for the commit to finish…" (the user's
    /// decision D, 2026-10-09), and "Waiting for the branch to be created…" (the user's decision
    /// of 2026-10-09 on phase 11's popover): a plain noun, or a gerund where the write has none,
    /// and what it is waited on to do.
    pub fn awaited(&self) -> &'static str {
        match self {
            Self::StageLines { .. } | Self::StageFiles { .. } | Self::StageAll { .. } => {
                "staging to finish"
            }
            Self::UnstageLines { .. } | Self::UnstageFiles { .. } | Self::UnstageAll { .. } => {
                "unstaging to finish"
            }
            Self::DiscardLines(_) | Self::DiscardFiles(_) => "the discard to finish",
            Self::Commit { .. } => "the commit to finish",
            Self::Amend { .. } => "the amend to finish",
            Self::CreateBranch { .. } => "the branch to be created",
            Self::CreateBranchAndCheckout { .. } | Self::CreateBranchDiscarding(_) => {
                "the checkout to finish"
            }
            Self::RemoveLock(_) => "the lock's removal to finish",
        }
    }

    /// Runs the write, with `token` as its askpass authorisation. A commit's cancel is handed
    /// to the lane once git is running, under its id, and its output to the window.
    fn perform(
        self,
        git: &GitBinary,
        repo: &Repository,
        token: Option<&AskpassToken>,
        watch: &Watch<'_>,
    ) -> Result<Performed, Error> {
        match self {
            Self::StageLines { diff, selection } => {
                ops::stage_lines(git, repo, &diff, &selection, token)
            }
            Self::UnstageLines { diff, selection } => {
                ops::unstage_lines(git, repo, &diff, &selection, token)
            }
            Self::StageFiles { paths } => ops::stage_files(git, repo, &paths, token),
            Self::UnstageFiles { paths, to } => {
                ops::unstage_files(git, repo, &paths, &to.into(), token)
            }
            Self::StageAll { changes, shown } => {
                let paths = every_path(&changes, ChangeList::Unstaged, shown.as_deref());
                drop(changes);
                ops::stage_files(git, repo, &paths, token)
            }
            Self::UnstageAll { changes, shown, to } => {
                let paths = every_path(&changes, ChangeList::Staged, shown.as_deref());
                drop(changes);
                ops::unstage_files(git, repo, &paths, &to.into(), token)
            }
            Self::DiscardLines(confirmed) => ops::discard_lines(git, repo, confirmed, token),
            Self::DiscardFiles(confirmed) => ops::discard_files(git, repo, confirmed, token),
            Self::Commit {
                message,
                skip_hooks,
            } => watch.commit(|commit| {
                ops::commit(git, repo, &message, hooks(skip_hooks), token, commit)
            }),
            Self::Amend {
                confirmed,
                message,
                skip_hooks,
            } => watch.commit(|commit| {
                ops::amend(
                    git,
                    repo,
                    confirmed,
                    &message,
                    hooks(skip_hooks),
                    token,
                    commit,
                )
            }),
            Self::CreateBranch { name, at } => ops::create_branch(git, repo, &name, at, token),
            Self::CreateBranchAndCheckout { name, at } => {
                ops::create_branch_and_checkout(git, repo, &name, at, token)
            }
            Self::CreateBranchDiscarding(confirmed) => {
                ops::create_branch_discarding(git, repo, confirmed, token)
            }
            Self::RemoveLock(confirmed) => ops::remove_index_lock(repo, confirmed),
        }
    }
}

/// Every path `list` lists — or, with a filter on, of its rows `shown` — and each rename's
/// source, as a whole-file action names them.
fn every_path(changes: &LocalChanges, list: ChangeList, shown: Option<&[u32]>) -> Vec<RepoPath> {
    match shown {
        Some(rows) => changes.whole_file_paths(list, rows.iter().map(|row| *row as usize)),
        None => changes.whole_file_paths(list, 0..changes.len(list)),
    }
}

fn hooks(skip: bool) -> Hooks {
    if skip { Hooks::Skip } else { Hooks::Run }
}

/// What a running write reports through, and how a commit is cancelled: its id, the lane it
/// installs its cancel on, and the outbox its output goes to.
struct Watch<'a> {
    lane: &'a LaneState,
    id: OperationId,
    outbox: &'a Outbox,
    /// What the lane's output waiting for the window counts against.
    budget: &'a Arc<AtomicUsize>,
}

impl Watch<'_> {
    /// Runs `commit` with a [`CommitWatch`]: cancelled before git starts by a cancel the lane
    /// kept for this id, its cancel installed as git starts, each line it writes sent on.
    fn commit<R>(&self, commit: impl FnOnce(CommitWatch<'_>) -> R) -> R {
        let (lane, id, outbox) = (self.lane, self.id, self.outbox);
        let before = BeforeRunning { lane, id };
        let mut running = |cancel: ops::CommitCancel| {
            lane.install(id, Box::new(move || cancel.cancel()));
        };
        let mut send = |lines: ScrubbedLines, receipt: OutputReceipt| {
            outbox.send(None, Update::WriteOutput { id, lines, receipt });
        };
        let flow = std::cell::RefCell::new(OutputFlow::new(Arc::clone(self.budget)));
        let mut output = |said: &ScrubbedLines| flow.borrow_mut().read(said, &mut send);
        let outcome = commit(CommitWatch {
            cancel: &before,
            running: &mut running,
            output: &mut output,
        });
        flow.borrow_mut().flush(&mut |lines, receipt| {
            outbox.send(None, Update::WriteOutput { id, lines, receipt });
        });
        outcome
    }
}

/// A cancel the lane holds for a commit that has not started its `git`, as its checks poll.
struct BeforeRunning<'a> {
    lane: &'a LaneState,
    id: OperationId,
}

impl cairn_git::Cancel for BeforeRunning<'_> {
    fn is_cancelled(&self) -> bool {
        self.lane.cancel_kept(self.id)
    }
}

/// How a write ended, as the window keeps it: what it did, or why it did nothing, or that it
/// may have done part — with the lock files around it (R3.8, #44).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteEnding {
    /// It ran: what it did, the prompt it confirmed when it was destructive, and the lock
    /// files before and after it.
    Done(Done),
    /// Nothing was written, because what it was built from moved first — a patch the writes
    /// ahead of it made stale, or a file edited since its discard was confirmed: dropped,
    /// naming its path (R3.7, R1.4).
    Stale { path: String, message: String },
    /// Nothing was written: refused before git ran (nothing selected, a conflicted path, a
    /// commit during a rebase, …).
    Refused { message: String },
    /// git ran and failed: its message, and the lock files present once it had — the one it
    /// failed on among them, when that was it. When it was git that failed, `command` is what
    /// ran (`git commit -q -F -`) and `output` git's words as the engine kept them — its
    /// runner's whole lines, scrubbed as they were split, stdout's and stderr's in the order
    /// they arrived (R4.10) — for the Git Error dialog (R10.5); both empty otherwise.
    Failed {
        message: String,
        locks: Vec<PathBuf>,
        command: Option<String>,
        output: ScrubbedLines,
    },
    /// Cancelled, or Cairn lost hold of its git: it may have taken effect, in part or whole,
    /// and the read after it shows what did (R4.7). With the lock files left.
    MayHaveTakenEffect {
        message: String,
        locks: Vec<PathBuf>,
    },
    /// A discard of files that did not take every file: what it did, the prompt it confirmed,
    /// and the paths still as they were confirmed.
    Incomplete {
        done: Done,
        kept: Vec<String>,
        message: String,
    },
    /// Not run: the repository was closing when its turn came, or a commit was cancelled
    /// before its `git` started.
    NotRun { message: String },
}

/// A write that ran (`cairn_git::ops::Performed`, in the window's words).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Done {
    pub description: String,
    /// The prompt the user accepted, for a destructive write (R1.6).
    pub acknowledged: Option<String>,
    pub locks_before: Vec<PathBuf>,
    pub locks_after: Vec<PathBuf>,
}

impl Done {
    fn of(performed: &Performed) -> Self {
        Self {
            description: performed.description().to_owned(),
            acknowledged: performed.acknowledged().map(str::to_owned),
            locks_before: performed.locks().before.clone(),
            locks_after: performed.locks().after.clone(),
        }
    }
}

impl WriteEnding {
    /// The lock files this ending names: what the window says remain.
    pub fn locks(&self) -> &[PathBuf] {
        match self {
            Self::Done(done) | Self::Incomplete { done, .. } => &done.locks_after,
            Self::Failed { locks, .. } | Self::MayHaveTakenEffect { locks, .. } => locks,
            Self::Stale { .. } | Self::Refused { .. } | Self::NotRun { .. } => &[],
        }
    }

    /// How `outcome` ended, and what it left stale when it ran (wholly or in part). A failure
    /// while no prompt could have been answered says so, as a fetch's does.
    fn of(
        outcome: Result<Performed, Error>,
        prompting: &Result<(), String>,
    ) -> (Self, Option<Invalidated>) {
        let error = match outcome {
            Ok(performed) => {
                return (
                    Self::Done(Done::of(&performed)),
                    Some(performed.invalidated()),
                );
            }
            Err(error) => error,
        };
        // The engine's text of it, scrubbed there (R12.2): git's stderr is in the message, and a
        // remote URL with a token in it is never drawn.
        let message = message_of(&error, prompting);
        let ending = match error {
            Error::ChangedSinceRead { path } | Error::ChangedSinceConfirmed { path } => {
                Self::Stale { path, message }
            }
            Error::AmendChangedSinceConfirmed => Self::Stale {
                path: "HEAD".to_owned(),
                message,
            },
            Error::Refused { .. }
            | Error::NoPaths
            | Error::CommitRefused { .. }
            | Error::CheckoutRefused { .. }
            | Error::LockRefused { .. } => Self::Refused { message },
            Error::LockChangedSinceConfirmed { path } => Self::Stale { path, message },
            Error::CommitCancelledBeforeRunning => Self::NotRun { message },
            Error::GitCancelled { stranded_locks, .. }
            | Error::GitUnwatched { stranded_locks, .. } => Self::MayHaveTakenEffect {
                message,
                locks: stranded_locks,
            },
            Error::GitFailed {
                arguments,
                stderr,
                present_locks,
                ..
            } => Self::Failed {
                message,
                locks: present_locks,
                command: Some(format!("git {arguments}")),
                output: stderr,
            },
            Error::DiscardIncomplete {
                performed, kept, ..
            } => {
                return (
                    Self::Incomplete {
                        done: Done::of(&performed),
                        kept,
                        message,
                    },
                    Some(performed.invalidated()),
                );
            }
            _ => Self::Failed {
                message,
                locks: Vec::new(),
                command: None,
                output: ScrubbedLines::new(),
            },
        };
        (ending, None)
    }
}

/// What the local lane is sent.
#[derive(Debug)]
pub(super) enum LocalJob {
    /// Boxed: a write carries a whole diff, and the stop nothing.
    Write {
        id: OperationId,
        write: Box<LocalWrite>,
    },
    /// What discarding `paths` would lose, computed for the window's confirmation
    /// (`ops::discard_files_consequence`), in the lane's order: after every write asked before
    /// it, so it counts what they left (staging-and-commit R8.4).
    Consequence {
        asked: OperationId,
        paths: Vec<RepoPath>,
        /// Its number in the discard-count lane: a newer ask, `Request::StopCounting` or a
        /// close ends the count, between paths or by ending its `git` read.
        cancel: Superseded,
    },
    /// What discarding `selection` of `diff` would lose (`ops::discard_lines_consequence`),
    /// in the lane's order, as [`LocalJob::Consequence`] is; one superseded before it runs is
    /// not read.
    LinesConsequence {
        asked: OperationId,
        diff: Arc<FileDiff>,
        selection: Selection,
        cancel: Superseded,
    },
    /// What the commit box reads (staging-and-commit R10), in the lane's order, answered under
    /// its number in the commit-box lane; one superseded before it is answered is not sent.
    CommitReads { epoch: Epoch, cancel: Superseded },
    /// What amending `HEAD` would replace and amend's lists over `status`, in the lane's order
    /// (R6.3, R6.4, R10.3); a newer ask or `StopAmending` ends its walk and its `git` read.
    Amending {
        status: Arc<LocalChanges>,
        epoch: Epoch,
        cancel: Superseded,
    },
    /// Whether `name` can be a new branch's (R11.3), in the lane's order, answered under its
    /// number in the branch-name lane; one superseded is not sent.
    BranchName {
        name: String,
        epoch: Epoch,
        cancel: Superseded,
    },
    /// What Create Branch's Discard would lose (`ops::checkout_discarding_consequence`), in the
    /// lane's order, after every write asked before it; a newer ask ends it.
    CheckoutConsequence {
        asked: OperationId,
        name: String,
        at: Oid,
        cancel: Superseded,
    },
    /// What removing the stale lock would destroy, read as `Remove index.lock…` is pressed, in
    /// the lane's order.
    LockConsequence { asked: OperationId },
    /// The repository is closing: end once the write running, if any, has.
    Stop,
}

/// What the lane shares with the handle, the repository thread and the refresh thread: one
/// mutex, held for an assignment and, at most, one send (module docs).
#[derive(Clone, Default)]
pub(super) struct LaneState(Arc<Mutex<Lane>>);

#[derive(Default)]
struct Lane {
    /// Ticks as a write starts and as it ends: odd while one runs.
    ticks: u64,
    running: Option<Running>,
    /// The repository is closing: run nothing more.
    closing: bool,
}

struct Running {
    id: OperationId,
    commit: bool,
    /// How to end it: a commit's, once git runs.
    cancel: Option<Box<dyn Fn() + Send>>,
    /// A cancel that came for it before git was running, kept until it is.
    cancelled: bool,
}

impl fmt::Debug for LaneState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lane = self.lock();
        f.debug_struct("LaneState")
            .field("ticks", &lane.ticks)
            .field("running", &lane.running.as_ref().map(|running| running.id))
            .field("closing", &lane.closing)
            .finish()
    }
}

impl LaneState {
    fn lock(&self) -> MutexGuard<'_, Lane> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The clock as a read begins, or `None` while a write runs — its ending reads again, so
    /// a read now would only be dropped.
    pub(super) fn stamp(&self) -> Option<u64> {
        let ticks = self.lock().ticks;
        ticks.is_multiple_of(2).then_some(ticks)
    }

    /// Runs `send` — the answer of a read that began at `stamp` — only if no write has
    /// started or ended since; `false` when it was dropped instead (R4.4). Under the lane's
    /// lock, so the answer is sent before the next write's start.
    pub(super) fn if_unchanged(&self, stamp: u64, send: impl FnOnce()) -> bool {
        let lane = self.lock();
        let unchanged = lane.ticks == stamp;
        if unchanged {
            send();
        }
        drop(lane);
        unchanged
    }

    /// `true` when a commit is running, so a refresh is kept back (R4.6): dropped, since the
    /// commit's ending reads everything again whatever it did.
    pub(super) fn defer_refresh(&self) -> bool {
        self.lock()
            .running
            .as_ref()
            .is_some_and(|running| running.commit)
    }

    /// Cancels the write `id` names, if it is a commit and running; anything else — a write
    /// that is not a commit, one queued, one that has ended — is left alone (R4.3). Called on
    /// the UI thread; the cancel it calls under the lock never waits (module docs).
    pub(super) fn cancel(&self, id: OperationId) {
        let mut lane = self.lock();
        if let Some(running) = lane.running.as_mut()
            && running.id == id
            && running.commit
        {
            match &running.cancel {
                Some(cancel) => cancel(),
                None => running.cancelled = true,
            }
        }
    }

    /// Whether a cancel came for `id` before its `git` was running: what a commit's checks
    /// poll before git starts.
    fn cancel_kept(&self, id: OperationId) -> bool {
        self.lock()
            .running
            .as_ref()
            .is_some_and(|running| running.id == id && running.cancelled)
    }

    /// `id` is running, and from now on `cancel` ends it; a cancel that came before ends it
    /// now, under the lock — a kill that never waits (module docs).
    fn install(&self, id: OperationId, cancel: Box<dyn Fn() + Send>) {
        let mut lane = self.lock();
        if let Some(running) = lane.running.as_mut()
            && running.id == id
        {
            if running.cancelled {
                cancel();
            }
            running.cancel = Some(cancel);
        }
    }

    /// `id` starts: the clock ticks and `announce` tells the window, under one lock.
    fn begin(&self, id: OperationId, commit: bool, announce: impl FnOnce()) {
        let mut lane = self.lock();
        lane.ticks += 1;
        lane.running = Some(Running {
            id,
            commit,
            cancel: None,
            cancelled: false,
        });
        announce();
    }

    /// The write running has ended: the clock ticks and `announce` tells the window, under
    /// one lock.
    fn end(&self, announce: impl FnOnce()) {
        let mut lane = self.lock();
        lane.ticks += 1;
        lane.running = None;
        announce();
    }

    /// The repository is closing: no write is started from now on.
    pub(super) fn close(&self) {
        self.lock().closing = true;
    }

    pub(super) fn is_closing(&self) -> bool {
        self.lock().closing
    }
}

/// The repository closing, as a walk polls it.
struct Closing<'a>(&'a LaneState);

impl cairn_git::Cancel for Closing<'_> {
    fn is_cancelled(&self) -> bool {
        self.0.is_closing()
    }
}

/// What a discard's count polls: its own number in the discard-count lane (a newer ask, a
/// `StopCounting`, or the close's stop of every lane) and the lane closing.
struct Counting<'a> {
    superseded: &'a Superseded,
    lane: &'a LaneState,
}

impl cairn_git::Cancel for Counting<'_> {
    fn is_cancelled(&self) -> bool {
        self.superseded.is_cancelled() || self.lane.is_closing()
    }
}

/// What the local lane serves with.
pub(super) struct Local<'a> {
    pub git: &'a GitBinary,
    /// `None` when no channel could be opened; `prompting` says why.
    pub channel: Option<&'a Arc<Channel>>,
    pub prompting: &'a Result<(), String>,
    pub lane: &'a LaneState,
    pub jobs: &'a Receiver<LocalJob>,
    pub outbox: &'a Outbox,
}

/// The local lane's loop: until it is told to stop or every handle is gone. First, before
/// any write it is sent, the lock files present as the repository opened — one a write left
/// when a close gave up on it among them (R4.9) — so the window names them: a walk of every
/// ref's directory, here rather than ahead of the repository's first answer, and stopped by a
/// close.
pub(super) fn serve_local_lane(shared: &SharedRepository, serving: &Local<'_>) {
    let repo = shared.to_worker();
    if let Some(locks) = shared.lock_files(&Closing(serving.lane))
        && !locks.is_empty()
    {
        let index_lock = names_index_lock(&repo, &locks);
        serving
            .outbox
            .send(None, Update::LocksAtOpen { locks, index_lock });
    }
    let budget = Arc::new(AtomicUsize::new(0));
    while let Ok(job) = serving.jobs.recv() {
        match job {
            LocalJob::Stop => break,
            LocalJob::Write { id, write } if serving.lane.is_closing() => {
                serving.outbox.send(
                    None,
                    Update::WriteEnded {
                        id,
                        ending: WriteEnding::NotRun {
                            message: format!(
                                "{} was not started: the repository is closing",
                                write.what()
                            ),
                        },
                        read_again: write.read_again(),
                    },
                );
            }
            LocalJob::Write { id, write } => run(&repo, id, *write, serving, &budget),
            LocalJob::Consequence {
                asked,
                paths,
                cancel,
            } => {
                let counting = Counting {
                    superseded: &cancel,
                    lane: serving.lane,
                };
                let outcome = ops::discard_files_consequence(serving.git, &repo, &paths, &counting);
                drop(paths);
                let outcome = match outcome {
                    // Superseded, let go of or closing: nobody waits on it.
                    Err(Error::ConsequenceCancelled) => continue,
                    Ok(consequence) => Ok(consequence),
                    Err(error) => Err(error.to_string()),
                };
                serving
                    .outbox
                    .send(None, Update::DiscardConsequence { asked, outcome });
            }
            LocalJob::LinesConsequence {
                asked,
                diff,
                selection,
                cancel,
            } => {
                // One path's reads, short: asked only while it is still the newest.
                if cairn_git::Cancel::is_cancelled(&cancel) || serving.lane.is_closing() {
                    continue;
                }
                let outcome = ops::discard_lines_consequence(serving.git, &repo, &diff, selection)
                    .map_err(|error| error.to_string());
                drop(diff);
                serving
                    .outbox
                    .send(None, Update::DiscardConsequence { asked, outcome });
            }
            LocalJob::CommitReads { epoch, cancel } => {
                let reading = Counting {
                    superseded: &cancel,
                    lane: serving.lane,
                };
                if let Some(reads) = commit_reads(serving.git, &repo, &reading) {
                    serving
                        .outbox
                        .send(Some(epoch), Update::CommitReads(Box::new(reads)));
                }
            }
            LocalJob::BranchName {
                name,
                epoch,
                cancel,
            } => {
                // Superseded while it waited its turn: no `git` is started for it.
                if cairn_git::Cancel::is_cancelled(&cancel) || serving.lane.is_closing() {
                    continue;
                }
                let reading = Counting {
                    superseded: &cancel,
                    lane: serving.lane,
                };
                let outcome = match repo.branch_name(serving.git, &name, &reading) {
                    Ok(answer) => Ok(answer),
                    // Superseded by the next keystroke, or closing: nobody waits on it.
                    Err(Error::GitReadCancelled { .. }) => continue,
                    Err(error) => Err(error.to_string()),
                };
                serving
                    .outbox
                    .send(Some(epoch), Update::BranchName { name, outcome });
            }
            LocalJob::LockConsequence { asked } => {
                // Read now, at the press (the user's decision H): its age is current, and a git
                // of Cairn's running is said rather than the offer lost.
                let outcome = match ops::remove_lock_consequence(&repo) {
                    Ok(consequence) => Ok(consequence),
                    Err(Error::LockRefused { why }) => Err(why.to_string()),
                    Err(error) => Err(error.to_string()),
                };
                serving
                    .outbox
                    .send(None, Update::LockConsequence { asked, outcome });
            }
            LocalJob::CheckoutConsequence {
                asked,
                name,
                at,
                cancel,
            } => {
                let counting = Counting {
                    superseded: &cancel,
                    lane: serving.lane,
                };
                let outcome =
                    ops::checkout_discarding_consequence(serving.git, &repo, &name, at, &counting);
                let outcome = match outcome {
                    Err(Error::ConsequenceCancelled | Error::GitReadCancelled { .. }) => continue,
                    Ok(consequence) => Ok(consequence),
                    Err(error) => Err(error.to_string()),
                };
                serving
                    .outbox
                    .send(None, Update::CheckoutConsequence { asked, outcome });
            }
            LocalJob::Amending {
                status,
                epoch,
                cancel,
            } => {
                let reading = Counting {
                    superseded: &cancel,
                    lane: serving.lane,
                };
                if let Some(read) = amend_read(serving.git, &repo, &status, &reading) {
                    serving.outbox.send(
                        Some(epoch),
                        Update::Amending {
                            status,
                            read: Box::new(read),
                        },
                    );
                }
            }
        }
    }
}

/// What the commit box reads (R6.6, R6.7, R6.9): each read's answer or its failure, or `None`
/// once `reading` is cancelled — nobody waits on it.
fn commit_reads(git: &GitBinary, repo: &Repository, reading: &Counting<'_>) -> Option<CommitReads> {
    use cairn_git::Cancel as _;
    if reading.is_cancelled() {
        return None;
    }
    let operation = repo.operation_in_progress();
    let hooks = repo
        .commit_hooks(git, reading)
        .map_err(|error| error.to_string());
    let recent = repo
        .recent_messages(reading)
        .map_err(|error| error.to_string());
    (!reading.is_cancelled()).then_some(CommitReads {
        operation,
        hooks,
        recent,
    })
}

/// What amending `HEAD` would replace, its message and amend's lists over `status` (R6.3,
/// R6.4, R10.3), each part or its failure; `None` once `reading` is cancelled. The message is
/// the commit the `Consequence` names, so the draft an amend fills is of the commit it
/// replaces.
fn amend_read(
    git: &GitBinary,
    repo: &Repository,
    status: &LocalChanges,
    reading: &Counting<'_>,
) -> Option<AmendRead> {
    use cairn_git::Cancel as _;
    if reading.is_cancelled() {
        return None;
    }
    let consequence = ops::amend_consequence(repo, reading);
    let message = match &consequence {
        Ok(consequence) => match consequence.amended() {
            Some(commit) => repo
                .commit_details(&commit)
                .map(|details| details.message)
                .map_err(|error| error.to_string()),
            None => Err("there is no commit to amend".to_owned()),
        },
        Err(error) => Err(error.to_string()),
    };
    let lists = repo
        .amend_parent()
        .and_then(|parent| {
            let staged = repo.amend_staged(git, reading)?;
            Ok(LocalChanges::amending(
                status.status().clone(),
                staged,
                parent,
            ))
        })
        .map(Arc::new)
        .map_err(|error| error.to_string());
    (!reading.is_cancelled()).then_some(AmendRead {
        consequence: consequence.map_err(|error| error.to_string()),
        message,
        lists,
    })
}

/// What the window reads again after a write (R4.5): what its `Invalidated` names when it ran,
/// what it could have changed otherwise — and everything after a commit, whatever its ending,
/// which covers a refresh kept back while it ran (R4.6).
fn after(commit: bool, asked: ReadAgain, invalidated: Option<Invalidated>) -> ReadAgain {
    if commit {
        ReadAgain::Everything
    } else {
        invalidated.map_or(asked, ReadAgain::after)
    }
}

/// `error`'s text as the window draws it — the engine's ([`Error::shown`], git's words in it
/// scrubbed there, R12.2) — and, where no prompt could have been answered, why: a failure's own
/// message ("terminal prompts disabled") does not say why nothing answered.
pub(super) fn message_of(error: &Error, prompting: &Result<(), String>) -> String {
    let shown = error.shown();
    match prompting {
        Ok(()) => shown.to_string(),
        Err(why) => {
            format!("{shown}. Cairn could not have asked for a credential in this session: {why}")
        }
    }
}

/// The command log's records of what this thread ran in `repo` since `mark` — each argument
/// and its output scrubbed of a URL's userinfo by the engine as it was recorded (R12.2, R4.10).
pub(super) fn ran_since(repo: &Repository, mark: cairn_git::CommandMark) -> Vec<CommandRecord> {
    repo.commands_since(mark)
}

/// `Remove index.lock…`'s offer (R12.4): whether `locks` — an ending's, or the repository's
/// as it opened — name `repo`'s own `index.lock`. What removing it would cost, and whether a git
/// of Cairn's runs, is read when the offer is pressed (the user's decision H, 2026-10-09).
pub(super) fn names_index_lock(repo: &Repository, locks: &[PathBuf]) -> bool {
    locks.contains(&repo.git_dir().join("index.lock"))
}

/// The write's askpass operation, if one began, and what its ending says of prompting: the
/// session's, or — where the channel could not make a token — why not.
fn token_and_prompting<O, E: std::fmt::Display>(
    began: Option<Result<O, E>>,
    session: &Result<(), String>,
) -> (Option<O>, Result<(), String>) {
    match began {
        Some(Ok(operation)) => (Some(operation), session.clone()),
        Some(Err(why)) => (None, Err(no_token(&why))),
        None => (None, session.clone()),
    }
}

/// Why a write had no askpass token, as its ending says it.
fn no_token(why: &impl std::fmt::Display) -> String {
    format!("no askpass token could be made for this write ({why})")
}

/// Runs one write: announced, given its own askpass token, ended with what it did.
fn run(
    repo: &Repository,
    id: OperationId,
    write: LocalWrite,
    serving: &Local<'_>,
    budget: &Arc<AtomicUsize>,
) {
    let commit = write.is_commit();
    let asked = write.read_again();
    // Where the command log stands: what this write runs on this thread from here is its own.
    let mark = repo.command_mark();
    serving.lane.begin(id, commit, || {
        serving.outbox.send(None, Update::WriteStarted { id })
    });
    // One token for the whole write, retired before its ending goes out, so no helper of a
    // git that is gone is accepted afterwards (L11).
    // Named as the window names the write, so a prompt it raises is titled by it (R5.1).
    // A token that could not be had is said in the ending, as a channel never opened is
    // (phase 04's QA item 11): a write that then fails for want of a prompt says why.
    let (authorised, prompting) = token_and_prompting(
        serving
            .channel
            .map(|channel| channel.begin_for(crate::status_text::capitalised(&write.what()))),
        serving.prompting,
    );
    let outcome = write.perform(
        serving.git,
        repo,
        authorised.as_ref().map(cairn_askpass::Operation::token),
        &Watch {
            lane: serving.lane,
            id,
            outbox: serving.outbox,
            budget,
        },
    );
    drop(authorised);
    let (ending, invalidated) = WriteEnding::of(outcome, &prompting);
    let read_again = after(commit, asked, invalidated);
    serving.outbox.send(
        None,
        Update::OperationRan {
            by: RanBy::Write(id),
            commands: ran_since(repo, mark),
            lock_named: names_index_lock(repo, ending.locks()),
        },
    );
    serving.lane.end(|| {
        serving.outbox.send(
            None,
            Update::WriteEnded {
                id,
                ending,
                read_again,
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 04's QA item 11: a write whose askpass token could not be made says why in its
    /// ending, as one under a channel never opened does. Caught by: the failure dropped with
    /// `.ok()`, the write failing with no word of the token.
    #[test]
    fn a_token_that_could_not_be_made_is_said_in_the_ending() {
        // Phase 11's QA (TC9): the choice `run` makes, its failing arm.
        let (operation, prompting) =
            token_and_prompting::<(), _>(Some(Err("could not generate an askpass token")), &Ok(()));
        assert!(operation.is_none());
        let (began, fine) = token_and_prompting::<u8, &str>(Some(Ok(7)), &Ok(()));
        assert_eq!((began, fine), (Some(7), Ok(())));
        let (_, never) = token_and_prompting::<u8, &str>(None, &Err("no channel".to_owned()));
        assert_eq!(never, Err("no channel".to_owned()));
        let (ending, _) = WriteEnding::of(Err(Error::NoPaths), &prompting);
        let WriteEnding::Refused { message } = ending else {
            panic!("{ending:?}");
        };
        assert!(
            message.contains("no askpass token could be made for this write")
                && message.contains("could not generate an askpass token"),
            "{message}"
        );
    }

    /// Phase 11's QA (Fork-settled): each write is named in the activity popover in Fork's
    /// imperative form, as its Activity Manager names an operation ("Fetch origin", "Create
    /// branch 'develop'"), a destructive one by its confirmation. Caught by: the progress
    /// phrase ("staging 1 file") drawn as an operation's name.
    #[test]
    fn each_write_is_named_in_forks_imperative_form() {
        use cairn_model::Consequence;
        let paths = |n: usize| (0..n).map(|i| RepoPath::new(format!("{i}.rs"))).collect();
        assert_eq!(
            LocalWrite::StageFiles { paths: paths(1) }.name(),
            "Stage 1 file"
        );
        assert_eq!(
            LocalWrite::UnstageFiles {
                paths: paths(2),
                to: UnstageTarget::Head,
            }
            .name(),
            "Unstage 2 files"
        );
        assert_eq!(
            LocalWrite::Commit {
                message: "m".to_owned(),
                skip_hooks: false,
            }
            .name(),
            "Commit"
        );
        assert_eq!(
            LocalWrite::CreateBranch {
                name: "develop".to_owned(),
                at: Oid::from_bytes(&[1; 20]).unwrap(),
            }
            .name(),
            "Create branch 'develop'"
        );
        let lock = Consequence::RemoveLock {
            path: PathBuf::from("/r/.git/index.lock"),
            modified: std::time::SystemTime::UNIX_EPOCH,
            read_at: std::time::SystemTime::UNIX_EPOCH,
            bytes: 0,
            device: 0,
            inode: 0,
        };
        assert_eq!(
            LocalWrite::RemoveLock(Confirmed::by_user(lock)).name(),
            "Remove index.lock"
        );
    }

    /// Phase 11's QA (TC11): `Remove index.lock…` is offered only where the ending names the
    /// stale `index.lock` — not for one naming only another lock, nor one naming none — though
    /// the lock is there either way. Caught by: an offer made on the lock's presence alone.
    #[test]
    fn the_lock_is_offered_only_where_the_ending_names_it() {
        let dir = std::env::temp_dir().join(format!("cairn-lock-offer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let program = cairn_git::ops::GitBinary::discover(&cairn_git::ops::Askpass::new(
            "/nonexistent/cairn-askpass",
            None,
        ))
        .unwrap();
        let status = std::process::Command::new(program.path())
            .args(["init", "-q"])
            .current_dir(&dir)
            .status()
            .unwrap();
        assert!(status.success());
        let repo = SharedRepository::discover(&dir).unwrap().to_worker();
        let index_lock = repo.git_dir().join("index.lock");
        std::fs::write(&index_lock, b"").unwrap();
        let head_lock = repo.git_dir().join("HEAD.lock");
        assert!(
            !names_index_lock(&repo, &[]),
            "offered for an ending naming no lock"
        );
        assert!(
            !names_index_lock(&repo, &[head_lock]),
            "offered for an ending naming another lock"
        );
        assert!(names_index_lock(&repo, &[index_lock]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// R12.2 and R4.10 at the lane: a failure's output is the engine's scrubbed lines, handed on
    /// as they are, and its message is the engine's text of the error — no token in either,
    /// whatever part quoted one — with why no prompt could be answered after it. Caught by: the
    /// lane formatting an error's text itself, or a failure's output rebuilt from text.
    #[test]
    fn an_ending_carries_the_engines_scrubbed_text() {
        use std::os::unix::process::ExitStatusExt as _;
        let output =
            cairn_model::ScrubbedLines::scrubbing("fatal: 'https://u:TOKEN@host/r' denied");
        let failed = Error::GitFailed {
            arguments: "commit -q -F -".to_owned(),
            status: std::process::ExitStatus::from_raw(1 << 8),
            stderr: output.clone(),
            present_locks: Vec::new(),
        };
        let (ending, _) = WriteEnding::of(Err(failed), &Err("no channel".to_owned()));
        let WriteEnding::Failed {
            message,
            output: kept,
            command,
            ..
        } = ending
        else {
            panic!("{ending:?}");
        };
        assert_eq!(kept, output);
        assert!(!kept.contains("TOKEN"), "{kept}");
        assert!(!message.contains("TOKEN"), "{message}");
        assert!(
            message.ends_with("in this session: no channel"),
            "{message}"
        );
        assert_eq!(command.as_deref(), Some("git commit -q -F -"));
        let quoting = Error::InvalidConfig {
            key: "remote.origin.url".to_owned(),
            value: "https://u:TOKEN@host/r".to_owned(),
        };
        let (ending, _) = WriteEnding::of(Err(quoting), &Ok(()));
        let WriteEnding::Failed { message, .. } = ending else {
            panic!("{ending:?}");
        };
        assert!(
            !message.contains("TOKEN") && message.contains("https://host/r"),
            "{message}"
        );
    }

    /// The user's decision (2026-10-09): Stage All and Unstage All take the rows a filter shows
    /// — a hidden row, a conflicted one among them, left as it is — and every row with none
    /// on; a rename's source comes with its row. Caught by: the lane gathering every row
    /// whatever the filter showed.
    #[test]
    fn an_all_gathers_the_rows_the_filter_shows_or_every_row() {
        use cairn_model::{
            ChangedEntry, ConflictKind, ConflictedEntry, Similarity, StagedChange, StatusEntry,
            UnstagedChange, WorkingTreeStatus,
        };
        let changed = |path: &str, staged, unstaged| {
            StatusEntry::Changed(ChangedEntry {
                path: RepoPath::from(path),
                staged,
                unstaged,
                submodule: None,
            })
        };
        let changes = LocalChanges::new(WorkingTreeStatus::Listed(vec![
            changed("a.rs", None, Some(UnstagedChange::Modified)),
            StatusEntry::Conflicted(ConflictedEntry {
                path: RepoPath::from("clash.rs"),
                kind: ConflictKind::BothModified,
                submodule: None,
            }),
            changed("cx.rs", None, Some(UnstagedChange::Modified)),
            changed(
                "new.rs",
                Some(StagedChange::Renamed {
                    from: RepoPath::from("old.rs"),
                    similarity: Similarity::from_percent(90),
                }),
                None,
            ),
            changed("t.rs", Some(StagedChange::Modified), None),
        ]));
        let names = |paths: Vec<RepoPath>| -> Vec<String> {
            paths.iter().map(ToString::to_string).collect()
        };
        assert_eq!(
            names(every_path(&changes, ChangeList::Unstaged, Some(&[2]))),
            ["cx.rs"]
        );
        assert_eq!(
            names(every_path(&changes, ChangeList::Unstaged, None)),
            ["a.rs", "clash.rs", "cx.rs"]
        );
        assert_eq!(
            names(every_path(&changes, ChangeList::Staged, Some(&[0]))),
            ["new.rs", "old.rs"]
        );
    }

    /// A lane with `id` running, as `begin` leaves it.
    fn running(id: OperationId, commit: bool) -> LaneState {
        let lane = LaneState::default();
        lane.begin(id, commit, || {});
        lane
    }

    /// R4.3: a cancel ends the commit it names while it runs — at once once git is, or as git
    /// starts when it came first — and nothing else: not a write that is not a commit, not a
    /// commit with another id. Caught by: a cancel matched on anything but the id, or one
    /// lost when it beats the install.
    #[test]
    fn a_cancel_reaches_the_running_commit_it_names_and_nothing_else() {
        use std::sync::atomic::AtomicUsize;
        let first = OperationId::next();
        let second = OperationId::next();
        let ended = Arc::new(AtomicUsize::new(0));
        let ending = || {
            let ended = Arc::clone(&ended);
            Box::new(move || {
                ended.fetch_add(1, Ordering::SeqCst);
            }) as Box<dyn Fn() + Send>
        };

        let lane = running(first, true);
        lane.install(first, ending());
        lane.cancel(second);
        assert_eq!(
            ended.load(Ordering::SeqCst),
            0,
            "another id's cancel ended it"
        );
        lane.cancel(first);
        assert_eq!(ended.load(Ordering::SeqCst), 1, "its own cancel did not");

        // A cancel before git runs is kept until it does.
        let lane = running(second, true);
        lane.cancel(second);
        assert_eq!(ended.load(Ordering::SeqCst), 1);
        lane.install(second, ending());
        assert_eq!(ended.load(Ordering::SeqCst), 2, "the early cancel was lost");

        // A write that is not a commit is never cancelled.
        let stage = OperationId::next();
        let lane = running(stage, false);
        lane.install(stage, ending());
        lane.cancel(stage);
        assert_eq!(ended.load(Ordering::SeqCst), 2, "a stage was cancelled");

        // Nor is one after it has ended.
        let lane = running(first, true);
        lane.install(first, ending());
        lane.end(|| {});
        lane.cancel(first);
        assert_eq!(
            ended.load(Ordering::SeqCst),
            2,
            "an ended write was cancelled"
        );
    }

    /// R4.4: a read stamped while no write runs is sent only if no write started or ended
    /// before it was answered; none is stamped while one runs. Caught by: a clock that does
    /// not tick at both ends, or a check made outside the lock it ticks under.
    #[test]
    fn a_read_is_sent_only_if_no_write_started_or_ended_since_it_began() {
        let lane = LaneState::default();
        let Some(stamp) = lane.stamp() else {
            panic!("no write runs, yet nothing could be stamped");
        };
        assert!(
            lane.if_unchanged(stamp, || {}),
            "an undisturbed read was dropped"
        );

        let id = OperationId::next();
        lane.begin(id, false, || {});
        assert_eq!(lane.stamp(), None, "a read was stamped while a write ran");
        let mut sent = false;
        assert!(!lane.if_unchanged(stamp, || sent = true));
        assert!(!sent, "a read that began before a write started was sent");
        lane.end(|| {});
        assert!(
            !lane.if_unchanged(stamp, || sent = true),
            "a read that began before a write ended was sent"
        );
        assert!(!sent);
        let Some(after) = lane.stamp() else {
            panic!("nothing could be stamped after the write");
        };
        assert!(lane.if_unchanged(after, || sent = true));
        assert!(sent, "a read begun after the write was dropped");
    }

    /// R4.6: a refresh is kept back while a commit runs, and only then. Caught by: deferring
    /// behind a stage (a refresh lost while staging), or a deferral that outlives the commit.
    #[test]
    fn a_refresh_is_kept_back_only_while_a_commit_runs() {
        let lane = running(OperationId::next(), false);
        assert!(
            !lane.defer_refresh(),
            "a refresh was kept back behind a stage"
        );
        lane.end(|| {});
        lane.begin(OperationId::next(), true, || {});
        assert!(lane.defer_refresh());
        lane.end(|| {});
        assert!(
            !lane.defer_refresh(),
            "a refresh is kept back with no commit running"
        );
    }

    /// R4.6 and phase 04's QA item 7: a refresh kept back is dropped, not remembered, so a
    /// commit's ending must read everything again whatever it did — done, refused, failed,
    /// cancelled — and a stage's only what it names. Caught by: a commit that ran reading only
    /// what its `Invalidated` names (a hook may have moved more), or one that failed reading
    /// status alone, which would lose the refresh kept back for it.
    #[test]
    fn a_commits_ending_reads_everything_whatever_it_did() {
        let commit = LocalWrite::Commit {
            message: "x".to_owned(),
            skip_hooks: false,
        };
        assert!(commit.is_commit());
        assert_eq!(commit.read_again(), ReadAgain::Everything);
        for invalidated in [None, Some(Invalidated::index()), Some(Invalidated::NOTHING)] {
            assert_eq!(
                after(true, ReadAgain::Everything, invalidated),
                ReadAgain::Everything,
                "{invalidated:?}"
            );
        }
        assert_eq!(
            after(false, ReadAgain::Status, Some(Invalidated::index())),
            ReadAgain::Status
        );
        assert_eq!(after(false, ReadAgain::Status, None), ReadAgain::Status);
    }

    /// R4.3: a cancel that comes before a commit's `git` runs is what its checks poll, and only
    /// for the commit it names. Caught by: a cancel kept for another id, or one lost before
    /// git starts.
    #[test]
    fn a_cancel_before_git_runs_is_kept_for_the_commit_it_names() {
        let id = OperationId::next();
        let lane = running(id, true);
        assert!(!lane.cancel_kept(id));
        lane.cancel(OperationId::next());
        assert!(!lane.cancel_kept(id), "another id's cancel was kept for it");
        lane.cancel(id);
        assert!(lane.cancel_kept(id));
        let stage = OperationId::next();
        let lane = running(stage, false);
        lane.cancel(stage);
        assert!(!lane.cancel_kept(stage), "a stage was cancelled");
    }

    /// R4.5, R4.7: a write that ran reads again what its `Invalidated` names; one that failed
    /// or was cancelled reads what it could have changed; a stale patch is dropped naming its
    /// path; a cancelled write may have taken effect, with its locks. Caught by: an ending
    /// that loses the path, the locks or the may-have.
    #[test]
    fn every_ending_says_what_happened_and_what_to_read_again() {
        let lock = PathBuf::from("/r/.git/index.lock");
        let (stale, read) = WriteEnding::of(
            Err(Error::ChangedSinceRead {
                path: "src/a.rs".to_owned(),
            }),
            &Ok(()),
        );
        assert_eq!(read, None);
        match stale {
            WriteEnding::Stale { path, message } => {
                assert_eq!(path, "src/a.rs");
                assert!(message.contains("src/a.rs"), "{message}");
            }
            other => panic!("{other:?}"),
        }
        let (cancelled, _) = WriteEnding::of(
            Err(Error::GitCancelled {
                arguments: "commit".to_owned(),
                stranded_locks: vec![lock.clone()],
            }),
            &Ok(()),
        );
        assert!(
            matches!(&cancelled, WriteEnding::MayHaveTakenEffect { locks, .. } if *locks == [lock.clone()]),
            "{cancelled:?}"
        );
        assert_eq!(cancelled.locks(), [lock]);
        let (refused, _) = WriteEnding::of(Err(Error::NoPaths), &Err("no socket".to_owned()));
        match refused {
            WriteEnding::Refused { message } => {
                assert!(message.contains("no socket"), "{message}");
            }
            other => panic!("{other:?}"),
        }
        let (refused, _) = WriteEnding::of(
            Err(Error::CommitRefused {
                why: cairn_git::CommitRefusal::InProgress(cairn_model::OperationInProgress::Rebase),
            }),
            &Ok(()),
        );
        assert!(
            matches!(&refused, WriteEnding::Refused { message } if message.contains("a rebase")),
            "{refused:?}"
        );
        let (moved, _) = WriteEnding::of(Err(Error::AmendChangedSinceConfirmed), &Ok(()));
        assert!(
            matches!(&moved, WriteEnding::Stale { path, .. } if path == "HEAD"),
            "{moved:?}"
        );
        let (early, _) = WriteEnding::of(Err(Error::CommitCancelledBeforeRunning), &Ok(()));
        assert!(
            matches!(&early, WriteEnding::NotRun { message } if message.contains("before git ran")),
            "a commit cancelled before git ran may have taken effect: {early:?}"
        );
        assert_eq!(
            ReadAgain::after(Invalidated::index()),
            ReadAgain::Status,
            "a stage reads more than status again"
        );
        assert_eq!(
            ReadAgain::after(Invalidated::working_tree().and(Invalidated::index())),
            ReadAgain::Status
        );
        assert_eq!(
            ReadAgain::after(Invalidated::refs().and(Invalidated::index())),
            ReadAgain::Everything,
            "a commit does not read the refs again"
        );
    }
}
