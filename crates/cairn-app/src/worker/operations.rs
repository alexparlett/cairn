//! The operations thread: where a `git` verb runs, so a fetch neither blocks
//! the history walk nor the window. One at a time, in the order asked.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, PoisonError};

use cairn_askpass::Channel;
use cairn_git::ops::{FetchCancel, GitBinary, fetch};
use cairn_git::{Error, SharedRepository};

use super::pool::Outbox;
use super::request::Update;

/// What the repository thread forwards here.
#[derive(Debug)]
pub(super) enum Operation {
    Fetch { remote: String },
}

/// Where the one fetch at a time is, and how to cancel it. Shared between
/// the window's handle (which cancels), the repository thread (which arms a
/// fetch it forwards) and the operations thread (which installs the kill
/// handle once git is running).
#[derive(Debug, Clone, Default)]
pub(super) struct FetchControl(Arc<Mutex<Stage>>);

#[derive(Debug, Default)]
enum Stage {
    #[default]
    Idle,
    /// A cancel that arrived before the fetch it answers had been dequeued. The
    /// window shows its Cancel button the moment the button is pressed, but
    /// `Request::Fetch` queues behind whatever page the repository thread is
    /// walking, while `CancelFetch` reaches this control directly — so on a cold
    /// repository the two cross, and without this the cancel is dropped and the
    /// fetch the user cancelled runs to completion. The next `arm` takes it, which
    /// is why it is a stage and not a flag: one cancel is claimed by one fetch, and
    /// a later fetch is not poisoned by it.
    CancelledBeforeStarting,
    /// Forwarded, not yet running; a cancel that arrives now is kept until it is.
    Starting {
        cancelled: bool,
    },
    Running(FetchCancel),
}

impl FetchControl {
    /// Claims the slot for a fetch about to be forwarded; `false` when one is
    /// already in flight, in which case nothing is forwarded — the window hides
    /// the button while a fetch runs, so this is the race it cannot close.
    pub(super) fn arm(&self) -> bool {
        let mut stage = self.lock();
        match *stage {
            Stage::Idle => {
                *stage = Stage::Starting { cancelled: false };
                true
            }
            // The cancel that crossed this fetch on the way in; it is claimed here
            // and nowhere else, so it kills this fetch and not the one after it.
            Stage::CancelledBeforeStarting => {
                *stage = Stage::Starting { cancelled: true };
                true
            }
            Stage::Starting { .. } | Stage::Running(_) => false,
        }
    }

    /// Kills the fetch in flight, or remembers to kill the one that is starting or
    /// still queued. Never blocks past a momentary lock: the kill itself only tries
    /// for the child's lock.
    pub(super) fn cancel(&self) {
        let mut stage = self.lock();
        match &mut *stage {
            Stage::Idle => *stage = Stage::CancelledBeforeStarting,
            // Pressing Cancel twice is one cancel.
            Stage::CancelledBeforeStarting => {}
            Stage::Starting { cancelled } => *cancelled = true,
            Stage::Running(cancel) => cancel.cancel(),
        }
    }

    /// git is running: from now on a cancel kills it, and one that arrived
    /// while it was starting kills it now.
    fn install(&self, cancel: FetchCancel) {
        let mut stage = self.lock();
        if matches!(*stage, Stage::Starting { cancelled: true }) {
            cancel.cancel();
        }
        *stage = Stage::Running(cancel);
    }

    fn clear(&self) {
        *self.lock() = Stage::Idle;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Stage> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Runs until the sender of `operations` is gone.
pub(super) fn serve_operations(
    git: &GitBinary,
    shared: &SharedRepository,
    channel: Option<&Arc<Channel>>,
    prompting: &Result<(), String>,
    control: &FetchControl,
    operations: &Receiver<Operation>,
    outbox: &Outbox,
) {
    let repo = shared.to_worker();
    while let Ok(operation) = operations.recv() {
        match operation {
            Operation::Fetch { remote } => {
                // What the refs were, so the window is told whether anything moved —
                // on every outcome, since a failed or killed fetch may have moved some.
                let before = repo.ref_tips().ok();
                // One token for the whole invocation, retired below before the outcome
                // goes out, so no helper of a dead git is accepted afterwards.
                let authorised = channel.and_then(|channel| channel.begin().ok());
                let outcome = fetch(
                    git,
                    &repo,
                    &remote,
                    authorised.as_ref().map(cairn_askpass::Operation::token),
                )
                .and_then(|started| {
                    // Installed BEFORE the window hears "started", so a cancel it sends
                    // in answer always has something to kill.
                    control.install(started.canceller());
                    outbox.send(
                        None,
                        Update::FetchStarted {
                            remote: remote.clone(),
                        },
                    );
                    started.finish(|line| {
                        outbox.send(
                            None,
                            Update::FetchProgress {
                                line: line.to_owned(),
                            },
                        );
                    })
                });
                control.clear();
                drop(authorised);
                let refreshed = match (before, repo.ref_tips().ok()) {
                    (Some(before), Some(after)) => before != after,
                    // Could not tell: assume the worst, which costs a reload.
                    _ => true,
                };
                outbox.send(
                    None,
                    fetch_outcome(remote, outcome.map(|_| ()), refreshed, prompting),
                );
            }
        }
    }
}

/// The update for how a fetch ended; `refreshed` says whether a ref moved,
/// whatever the outcome. A failure while no prompt could have been answered
/// says so, since git's own message ("terminal prompts disabled") does not
/// say why nothing answered.
fn fetch_outcome(
    remote: String,
    outcome: Result<(), Error>,
    refreshed: bool,
    prompting: &Result<(), String>,
) -> Update {
    match outcome {
        Ok(()) => Update::FetchFinished { remote, refreshed },
        Err(Error::GitCancelled { .. }) => Update::FetchCancelled { remote, refreshed },
        Err(error) => Update::FetchFailed {
            remote,
            refreshed,
            message: match prompting {
                Ok(()) => error.to_string(),
                Err(why) => format!(
                    "{error}. Cairn could not have asked for a credential in this session: {why}"
                ),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_outcome_carries_whether_the_refs_moved() {
        let origin = || "origin".to_owned();
        assert_eq!(
            fetch_outcome(origin(), Ok(()), true, &Ok(())),
            Update::FetchFinished {
                remote: origin(),
                refreshed: true
            }
        );
        assert_eq!(
            fetch_outcome(origin(), Ok(()), false, &Ok(())),
            Update::FetchFinished {
                remote: origin(),
                refreshed: false
            }
        );
        assert_eq!(
            fetch_outcome(
                origin(),
                Err(Error::GitCancelled {
                    arguments: "fetch origin".to_owned(),
                }),
                true,
                &Ok(())
            ),
            Update::FetchCancelled {
                remote: origin(),
                refreshed: true
            },
            "a cancelled fetch that moved refs must still say so"
        );
    }

    /// Caught by: dropping the reason, which leaves the user with git's "terminal
    /// prompts disabled" and no idea why Cairn did not ask.
    #[test]
    fn a_failure_with_no_way_to_prompt_says_why_nothing_asked() {
        let failure = || {
            Err(Error::GitNotStarted {
                program: "/usr/bin/git".into(),
                source: std::io::Error::other("boom"),
            })
        };
        match fetch_outcome("origin".to_owned(), failure(), false, &Ok(())) {
            Update::FetchFailed { message, .. } => {
                assert!(message.contains("boom"));
                assert!(!message.contains("could not have asked"));
            }
            other => panic!("{other:?}"),
        }
        match fetch_outcome(
            "origin".to_owned(),
            failure(),
            true,
            &Err("the askpass helper is not at /x".to_owned()),
        ) {
            Update::FetchFailed {
                message, refreshed, ..
            } => {
                assert!(message.contains("boom"), "{message}");
                assert!(message.contains("helper is not at /x"), "{message}");
                assert!(refreshed);
            }
            other => panic!("{other:?}"),
        }
    }

    /// Caught by: a cancel while starting being dropped, or a second fetch being armed
    /// over a running one.
    #[test]
    fn a_cancel_while_starting_is_kept_and_one_fetch_at_a_time_is_armed() {
        let control = FetchControl::default();
        assert!(control.arm());
        assert!(!control.arm(), "a second fetch was armed over the first");
        control.cancel();
        assert!(
            matches!(*control.lock(), Stage::Starting { cancelled: true }),
            "the cancel that arrived before git ran was lost"
        );
        control.clear();
        assert!(control.arm());
    }

    /// The window shows Cancel the instant the button is pressed, but the fetch queues
    /// behind the page the repository thread is walking, so a cancel can reach the
    /// control before `arm` does. Caught by: that cancel being dropped, which ran the
    /// fetch the user had already cancelled. The second half is the other direction —
    /// one cancel is claimed by one fetch, never by the fetch after it.
    #[test]
    fn a_cancel_that_overtakes_the_fetch_it_answers_is_claimed_by_that_fetch_alone() {
        let control = FetchControl::default();
        control.cancel();
        assert!(
            matches!(*control.lock(), Stage::CancelledBeforeStarting),
            "a cancel with nothing yet armed was dropped"
        );
        assert!(control.arm(), "the queued fetch could not be armed");
        assert!(
            matches!(*control.lock(), Stage::Starting { cancelled: true }),
            "the fetch was armed without the cancel that overtook it"
        );

        control.clear();
        assert!(control.arm());
        assert!(
            matches!(*control.lock(), Stage::Starting { cancelled: false }),
            "the next fetch inherited a cancel that was already spent"
        );
    }
}
