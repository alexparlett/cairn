//! The local write lane, `cairn-local`: the thread where every write to the index, the
//! working tree and a local ref runs — staging, unstaging and discarding today, commit and
//! amend from phase 05 — one at a time, in the order asked (staging-and-commit R4,
//! `docs/design/concurrency.md`, "Operations").
//!
//! A write reaches this thread straight from `RepositoryHandle::submit`, never through the
//! repository thread, where it would wait behind a deep find (R4.1). It is queued behind the
//! write running, never refused for being second (R4.2), and carries an [`OperationId`] the
//! window chose as it asked, so the window draws it queued, then running, then ended, and a
//! cancel names it (R4.3): only a commit is cancellable, and only while it runs, so a cancel
//! that arrives late never reaches the write queued behind the one it named.
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
//!   repository or refresh thread, while a commit runs starts nothing; it is remembered, and
//!   the commit's ending says to read everything again, so none is lost.
//! - **The running write**, its id and its cancel.
//!
//! Every write carries an askpass token of its own (R5.1, L11): a hook, a signing program or
//! an LFS filter it runs may ask, and the window shows the prompt while a write runs.
//!
//! Closing the window during a write waits for it (R4.9): the repository thread, closing,
//! marks the lane closing, which runs nothing more it is sent, and waits for the write
//! running to end before it ends what else `git` runs. The window says which write it is
//! waiting for, and a second close after `CLOSE_PATIENCE` closes it anyway; a lock the write
//! leaves then is listed as the repository next opens (`Update::LocksAtOpen`).

use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use cairn_askpass::Channel;
use cairn_git::ops::{self, GitBinary, Invalidated, Performed, UnstageTo};
use cairn_git::{Error, Repository, SharedRepository};
use cairn_model::{AskpassToken, Confirmed, FileDiff, Oid, RepoPath, Selection};

use super::pool::Outbox;
use super::request::Update;

/// Names one local write from the moment the window asks for it: what its start, its
/// ending and a cancel of it name. Never reused within the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperationId(u64);

impl OperationId {
    /// A fresh id: an atomic increment, so the window can call it as it asks.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "no view asks for a write until phase 07; the lane serves them, and its \
                      tests ask"
        )
    )]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no view asks for a write until phase 07; the lane serves each, and its tests \
                  ask"
    )
)]
pub enum LocalWrite {
    /// `selection` of `diff` — the path's unstaged or untracked diff — into the index.
    StageLines {
        diff: Box<FileDiff>,
        selection: Selection,
    },
    /// `selection` of `diff` — the path's staged diff — out of the index.
    UnstageLines {
        diff: Box<FileDiff>,
        selection: Selection,
    },
    /// Every one of `paths`, whole, into the index.
    StageFiles { paths: Vec<RepoPath> },
    /// Every one of `paths`, whole, out of the index, back to `to`.
    UnstageFiles {
        paths: Vec<RepoPath>,
        to: UnstageTarget,
    },
    /// The lines the confirmation names, discarded from the working tree.
    DiscardLines(Confirmed),
    /// The files the confirmation names, discarded, untracked ones deleted.
    DiscardFiles(Confirmed),
    /// A long-running, cancellable write the lane treats as a commit, for the tests of what
    /// a commit does to the lane before commit exists (phase 05 runs them again against
    /// `git commit`): `git fetch` of `remote`, through a stub `git` that holds it, asks or
    /// ends as the test says.
    #[cfg(test)]
    HeldCommit { remote: String },
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
            Self::DiscardLines(_) | Self::DiscardFiles(_) => {
                panic!("a destructive write's confirmation is spent once; a test may not copy it")
            }
            Self::HeldCommit { remote } => Self::HeldCommit {
                remote: remote.clone(),
            },
        }
    }
}

/// What the window reads again once a write has ended (R4.5): what its `Invalidated` names
/// and no more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadAgain {
    /// The refs, ahead/behind and status — and the history, if the refs it draws moved: a
    /// write that moved a ref (a commit), or the refresh a commit kept back while it ran.
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
    /// Whether this is a commit: cancellable while it runs, and the lane is quiet behind it.
    fn is_commit(&self) -> bool {
        match self {
            Self::StageLines { .. }
            | Self::UnstageLines { .. }
            | Self::StageFiles { .. }
            | Self::UnstageFiles { .. }
            | Self::DiscardLines(_)
            | Self::DiscardFiles(_) => false,
            #[cfg(test)]
            Self::HeldCommit { .. } => true,
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
            | Self::DiscardLines(_)
            | Self::DiscardFiles(_) => ReadAgain::Status,
            #[cfg(test)]
            Self::HeldCommit { .. } => ReadAgain::Everything,
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
            Self::DiscardLines(_) => "discarding lines".to_owned(),
            Self::DiscardFiles(_) => "discarding files".to_owned(),
            #[cfg(test)]
            Self::HeldCommit { .. } => "commit".to_owned(),
        }
    }

    /// Runs the write, with `token` as its askpass authorisation. A commit's cancel is handed
    /// to `lane` once git is running, under `id`.
    fn perform(
        self,
        git: &GitBinary,
        repo: &Repository,
        token: Option<&AskpassToken>,
        #[cfg_attr(
            not(test),
            expect(
                unused_variables,
                reason = "commit, phase 05, is the first write to cancel"
            )
        )]
        (lane, id): (&LaneState, OperationId),
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
            Self::DiscardLines(confirmed) => ops::discard_lines(git, repo, confirmed, token),
            Self::DiscardFiles(confirmed) => ops::discard_files(git, repo, confirmed, token),
            #[cfg(test)]
            Self::HeldCommit { remote } => {
                let started = ops::fetch(git, repo, &remote, token)?;
                let cancel = started.canceller();
                lane.install(id, Box::new(move || cancel.cancel()));
                started.finish(|_| {})
            }
        }
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
    /// Nothing was written: refused before git ran (nothing selected, a conflicted path, …).
    Refused { message: String },
    /// git ran and failed: its message, and the lock files present once it had — the one it
    /// failed on among them, when that was it.
    Failed {
        message: String,
        locks: Vec<PathBuf>,
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
    /// Not run: the repository was closing when its turn came.
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
        let message = match prompting {
            Ok(()) => error.to_string(),
            Err(why) => format!(
                "{error}. Cairn could not have asked for a credential in this session: {why}"
            ),
        };
        let ending = match error {
            Error::ChangedSinceRead { path } | Error::ChangedSinceConfirmed { path } => {
                Self::Stale { path, message }
            }
            Error::Refused { .. } | Error::NoPaths => Self::Refused { message },
            Error::GitCancelled { stranded_locks, .. }
            | Error::GitUnwatched { stranded_locks, .. } => Self::MayHaveTakenEffect {
                message,
                locks: stranded_locks,
            },
            Error::GitFailed { present_locks, .. } => Self::Failed {
                message,
                locks: present_locks,
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
    /// A refresh was kept back while a commit ran.
    deferred: bool,
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
            .field("deferred", &lane.deferred)
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

    /// `true` when a commit is running, and remembers that a refresh was kept back for it
    /// (R4.6): the commit's ending reads everything again.
    pub(super) fn defer_refresh(&self) -> bool {
        let mut lane = self.lock();
        let commit = lane.running.as_ref().is_some_and(|running| running.commit);
        if commit {
            lane.deferred = true;
        }
        commit
    }

    /// Cancels the write `id` names, if it is a commit and running; anything else — a write
    /// that is not a commit, one queued, one that has ended — is left alone (R4.3).
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

    /// `id` is running, and from now on `cancel` ends it; a cancel that came before ends it
    /// now.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "commit, phase 05, is the first write to cancel")
    )]
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

    /// The write running has ended: the clock ticks and `announce` tells the window, given
    /// whether a refresh was kept back meanwhile, under one lock.
    fn end(&self, announce: impl FnOnce(bool)) {
        let mut lane = self.lock();
        lane.ticks += 1;
        lane.running = None;
        let deferred = std::mem::take(&mut lane.deferred);
        announce(deferred);
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
    if let Some(locks) = shared.lock_files(&Closing(serving.lane))
        && !locks.is_empty()
    {
        serving.outbox.send(None, Update::LocksAtOpen { locks });
    }
    let repo = shared.to_worker();
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
            LocalJob::Write { id, write } => run(&repo, id, *write, serving),
        }
    }
}

/// Runs one write: announced, given its own askpass token, ended with what it did.
fn run(repo: &Repository, id: OperationId, write: LocalWrite, serving: &Local<'_>) {
    let commit = write.is_commit();
    let asked = write.read_again();
    serving.lane.begin(id, commit, || {
        serving.outbox.send(None, Update::WriteStarted { id })
    });
    // One token for the whole write, retired before its ending goes out, so no helper of a
    // git that is gone is accepted afterwards (L11).
    let authorised = serving.channel.and_then(|channel| channel.begin().ok());
    let outcome = write.perform(
        serving.git,
        repo,
        authorised.as_ref().map(cairn_askpass::Operation::token),
        (serving.lane, id),
    );
    drop(authorised);
    let (ending, invalidated) = WriteEnding::of(outcome, serving.prompting);
    let read_again = invalidated.map_or(asked, ReadAgain::after);
    serving.lane.end(|deferred| {
        serving.outbox.send(
            None,
            Update::WriteEnded {
                id,
                ending,
                read_again: if deferred {
                    ReadAgain::Everything
                } else {
                    read_again
                },
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
        lane.end(|_| {});
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
        lane.end(|_| {});
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

    /// R4.6: a refresh is kept back while a commit runs, and only then, and the commit's
    /// ending is told so, once. Caught by: deferring behind a stage (a refresh lost while
    /// staging), or a deferral that outlives the commit.
    #[test]
    fn a_refresh_is_kept_back_only_while_a_commit_runs_and_its_ending_says_so() {
        let lane = running(OperationId::next(), false);
        assert!(
            !lane.defer_refresh(),
            "a refresh was kept back behind a stage"
        );
        lane.end(|deferred| assert!(!deferred));

        lane.begin(OperationId::next(), true, || {});
        assert!(lane.defer_refresh());
        assert!(lane.defer_refresh());
        let mut told = None;
        lane.end(|deferred| told = Some(deferred));
        assert_eq!(told, Some(true), "the commit's ending was not told");
        assert!(
            !lane.defer_refresh(),
            "a refresh is kept back with no commit running"
        );
        lane.begin(OperationId::next(), true, || {});
        lane.end(|deferred| told = Some(deferred));
        assert_eq!(
            told,
            Some(false),
            "a deferral outlived the commit it was for"
        );
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
