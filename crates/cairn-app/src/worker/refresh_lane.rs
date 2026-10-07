//! The refresh thread, `cairn-refresh`: the working tree's status and each branch's
//! ahead/behind (refs-and-status R11.2), on a thread of their own so neither a history page
//! nor a diff queues behind a slow status (736 ms on a stat-dirty rust-lang/rust) or a long
//! divergence.
//!
//! Ahead/behind is numbered in its lane and cancelled by the next refresh's number there: a
//! superseded count stops its walk. Status is not (R10.3 as amended, the user's decision of
//! 2026-10-07): restarting `git status` on every focus gain meant a window switched faster
//! than a status takes never got one, each switch paying a full stat of the tree. A running
//! status finishes and is answered; every refresh asked while it ran is one follow-up
//! status after it, however many there were ([`serve_refreshes`] coalesces the status jobs
//! queued when it takes one up); and only a close ends a running one, by stopping every
//! lane (and ending every `git` the repository runs).
//!
//! Status is asked by the window's refresh directly; ahead/behind is forwarded here by the
//! repository thread once it has read the refs it counts, under the epoch the refresh was
//! given, so a refresh that supersedes this one supersedes its count too.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use cairn_git::ops::GitBinary;
use cairn_git::{Error, Repository, SharedRepository};
use cairn_model::RefsSnapshot;

use super::epoch::{Epoch, Epochs};
use super::pool::Outbox;
use super::request::{Refreshed, Update};

/// What the refresh thread is sent.
#[derive(Debug)]
pub(super) enum RefreshJob {
    /// The working tree's status, under the status lane's `epoch` — which no refresh moves,
    /// so only a close cancels it.
    Status { epoch: Epoch },
    /// Every local branch of `snapshot` counted against its upstream, under the
    /// ahead/behind lane's `epoch`.
    AheadBehind {
        epoch: Epoch,
        snapshot: Arc<RefsSnapshot>,
    },
    /// The repository is closing.
    Stop,
}

/// What the refresh thread serves with.
pub(super) struct Refreshing<'a> {
    pub git: &'a GitBinary,
    pub epochs: &'a Epochs,
    pub jobs: &'a Receiver<RefreshJob>,
    pub outbox: &'a Outbox,
}

/// The refresh thread's loop: until it is told to stop, every handle is gone, or the
/// repository closes. A status taken up stands for every status queued behind it: they were
/// all asked before it began, so one read answers them all — which is how the refreshes
/// asked while a status ran become exactly one follow-up.
pub(super) fn serve_refreshes(shared: &SharedRepository, serving: &Refreshing<'_>) {
    let repo = shared.to_worker();
    // Jobs read off the queue while coalescing statuses, served in their turn.
    let mut held = VecDeque::new();
    loop {
        let job = match held.pop_front() {
            Some(job) => job,
            None => match serving.jobs.recv() {
                Ok(job) => job,
                Err(_) => break,
            },
        };
        if serving.epochs.is_stopping() {
            break;
        }
        match job {
            RefreshJob::Status { epoch } => {
                held.extend(coalesced(serving.jobs));
                status(&repo, epoch, serving);
            }
            RefreshJob::AheadBehind { epoch, snapshot } => {
                ahead_behind(&repo, &snapshot, epoch, serving);
            }
            RefreshJob::Stop => break,
        }
    }
}

/// Every job queued now but the statuses, which the status about to be read answers.
fn coalesced(jobs: &Receiver<RefreshJob>) -> Vec<RefreshJob> {
    jobs.try_iter()
        .filter(|job| !matches!(job, RefreshJob::Status { .. }))
        .collect()
}

fn status(repo: &Repository, epoch: Epoch, serving: &Refreshing<'_>) {
    // The repository closing: nothing is run.
    if !serving.epochs.is_current(epoch) {
        return;
    }
    match repo.status(serving.git, &serving.epochs.watch(epoch)) {
        Ok(status) => serving.outbox.send(Some(epoch), Update::Status { status }),
        // Ended by a close, which the cancel saw: not a failure.
        Err(Error::StatusCancelled) => {}
        Err(error) => serving.outbox.send(
            Some(epoch),
            Update::RefreshFailed {
                what: Refreshed::Status,
                message: error.to_string(),
            },
        ),
    }
}

fn ahead_behind(
    repo: &Repository,
    snapshot: &RefsSnapshot,
    epoch: Epoch,
    serving: &Refreshing<'_>,
) {
    if !serving.epochs.is_current(epoch) {
        return;
    }
    match repo.ahead_behind(snapshot, &serving.epochs.watch(epoch)) {
        Ok(read) => serving.outbox.send(
            Some(epoch),
            Update::AheadBehind {
                counts: read.counts,
            },
        ),
        Err(Error::AheadBehindCancelled { .. }) => {}
        Err(error) => serving.outbox.send(
            Some(epoch),
            Update::RefreshFailed {
                what: Refreshed::AheadBehind,
                message: error.to_string(),
            },
        ),
    }
}
