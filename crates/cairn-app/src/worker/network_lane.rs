//! The network lane: the thread where a network operation runs — fetch,
//! today — so it neither blocks the history walk nor the window. One at a
//! time, in the order asked (PRD R7, `docs/design/concurrency.md`,
//! "Operations").
//!
//! The lane is chosen per operation ([`Operation::lane`]), never assumed:
//! the local lane, for writes to the index, the working tree and local refs,
//! lands beside this one with the first local write. A second copy of an
//! operation already running or waiting in its lane is refused with a reason
//! the window draws ([`Refusal`]), never dropped.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, PoisonError};

use cairn_askpass::Channel;
use cairn_git::ops::{FetchCancel, GitBinary, fetch};
use cairn_git::{Error, SharedRepository};

use super::pool::Outbox;
use super::request::Update;

/// What the repository thread forwards to a write lane.
#[derive(Debug)]
pub(super) enum Operation {
    Fetch { remote: String },
}

/// The write lanes an operation can run in. One, until the first local write
/// adds `Local` and the table below routes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lane {
    /// Fetch, push, and the transfer half of pull.
    Network,
}

impl Operation {
    /// The lane this operation runs in. Every operation names its own: no arm
    /// may default, so a new operation does not compile until it has one.
    pub(super) fn lane(&self) -> Lane {
        match self {
            Self::Fetch { .. } => Lane::Network,
        }
    }
}

/// Why a fetch was not started: the one in flight, and how far it has got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Refusal {
    /// The remote the fetch in flight is for.
    in_flight: String,
    /// `true` once git is running; `false` while it is waiting to start.
    running: bool,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let how_far = if self.running {
            "running"
        } else {
            "waiting to start"
        };
        write!(f, "a fetch of {} is already {how_far}", self.in_flight)
    }
}

/// Where the one fetch at a time is, and how to cancel it. Shared between
/// the window's handle (which cancels), the repository thread (which arms a
/// fetch it forwards) and the network lane (which installs the kill handle
/// once git is running).
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
        remote: String,
        cancelled: bool,
    },
    Running {
        remote: String,
        cancel: FetchCancel,
    },
}

impl FetchControl {
    /// Claims the slot for a fetch of `remote` about to be forwarded, or
    /// refuses it, naming the fetch already in flight: nothing is forwarded
    /// then, and the refusal is the window's to draw. The window hides its
    /// button while a fetch is in flight, so this is what a second fetch
    /// meets when anything else asks for one (PRD R7.2).
    pub(super) fn arm(&self, remote: &str) -> Result<(), Refusal> {
        let mut stage = self.lock();
        let cancelled = match &*stage {
            Stage::Idle => false,
            // The cancel that crossed this fetch on the way in; it is claimed here
            // and nowhere else, so it kills this fetch and not the one after it.
            Stage::CancelledBeforeStarting => true,
            Stage::Starting { remote, .. } => {
                return Err(Refusal {
                    in_flight: remote.clone(),
                    running: false,
                });
            }
            Stage::Running { remote, .. } => {
                return Err(Refusal {
                    in_flight: remote.clone(),
                    running: true,
                });
            }
        };
        *stage = Stage::Starting {
            remote: remote.to_owned(),
            cancelled,
        };
        Ok(())
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
            Stage::Starting { cancelled, .. } => *cancelled = true,
            Stage::Running { cancel, .. } => cancel.cancel(),
        }
    }

    /// git is running: from now on a cancel kills it, and one that arrived
    /// while it was starting kills it now.
    fn install(&self, remote: &str, cancel: FetchCancel) {
        let mut stage = self.lock();
        if matches!(
            *stage,
            Stage::Starting {
                cancelled: true,
                ..
            }
        ) {
            cancel.cancel();
        }
        *stage = Stage::Running {
            remote: remote.to_owned(),
            cancel,
        };
    }

    fn clear(&self) {
        *self.lock() = Stage::Idle;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Stage> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The network lane's loop: runs until the sender of `operations` is gone.
pub(super) fn serve_network_lane(
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
                // Whether anything moved is not this lane's to say: the window refreshes on
                // every outcome (refs-and-status R10.1), and the refresh decides.
                // One token for the whole invocation, retired below before the outcome
                // goes out, so no helper of a dead git is accepted afterwards.
                let authorised = channel.and_then(|channel| channel.begin_for(remote.clone()).ok());
                // What this fetch runs on this thread from here is its own (R12.1).
                let mark = repo.command_mark();
                let outcome = fetch(
                    git,
                    &repo,
                    &remote,
                    authorised.as_ref().map(cairn_askpass::Operation::token),
                )
                .and_then(|started| {
                    // Installed BEFORE the window hears "started", so a cancel it sends
                    // in answer always has something to kill.
                    control.install(&remote, started.canceller());
                    outbox.send(
                        None,
                        Update::FetchStarted {
                            remote: remote.clone(),
                        },
                    );
                    // Scrubbed before it leaves the lane (R12.2): a remote URL with a token
                    // in it is never drawn.
                    let mut shown = cairn_model::Scrubber::new();
                    started.finish(|line| {
                        outbox.send(
                            None,
                            Update::FetchProgress {
                                line: shown.line(line),
                            },
                        );
                    })
                });
                control.clear();
                drop(authorised);
                let locks: &[std::path::PathBuf] = match &outcome {
                    Err(Error::GitCancelled { stranded_locks, .. })
                    | Err(Error::GitUnwatched { stranded_locks, .. }) => stranded_locks,
                    Err(Error::GitFailed { present_locks, .. }) => present_locks,
                    Ok(_) | Err(_) => &[],
                };
                let lock = super::local_lane::removable_lock(&repo, locks);
                outbox.send(
                    None,
                    Update::OperationRan {
                        by: super::request::RanBy::Fetch,
                        commands: super::local_lane::ran_since(&repo, mark),
                        lock,
                    },
                );
                outbox.send(None, fetch_outcome(remote, outcome.map(|_| ()), prompting));
            }
        }
    }
}

/// The update for how a fetch ended. A failure while no prompt could have been
/// answered says so, since git's own message ("terminal prompts disabled") does
/// not say why nothing answered.
fn fetch_outcome(
    remote: String,
    outcome: Result<(), Error>,
    prompting: &Result<(), String>,
) -> Update {
    match outcome {
        Ok(()) => Update::FetchFinished { remote },
        Err(Error::GitCancelled { stranded_locks, .. }) => Update::FetchCancelled {
            remote,
            stranded_locks,
        },
        Err(error) => {
            let error = super::local_lane::scrubbed_error(error);
            Update::FetchFailed {
                remote,
                message: crate::shown_output::scrubbed(&match prompting {
                    Ok(()) => error.to_string(),
                    Err(why) => format!(
                        "{error}. Cairn could not have asked for a credential in this session: {why}"
                    ),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_outcome_names_how_the_fetch_ended() {
        let origin = || "origin".to_owned();
        assert_eq!(
            fetch_outcome(origin(), Ok(()), &Ok(())),
            Update::FetchFinished { remote: origin() }
        );
        assert_eq!(
            fetch_outcome(
                origin(),
                Err(Error::GitCancelled {
                    arguments: "fetch origin".to_owned(),
                    stranded_locks: Vec::new(),
                }),
                &Ok(())
            ),
            Update::FetchCancelled {
                remote: origin(),
                stranded_locks: Vec::new(),
            }
        );
    }

    /// Caught by: dropping the paths on the way to the window, which is the only place
    /// the user can hear about them.
    #[test]
    fn a_cancelled_fetch_carries_the_lock_files_it_stranded() {
        let lock = std::path::PathBuf::from("/r/.git/refs/remotes/origin/main.lock");
        assert_eq!(
            fetch_outcome(
                "origin".to_owned(),
                Err(Error::GitCancelled {
                    arguments: "fetch origin".to_owned(),
                    stranded_locks: vec![lock.clone()],
                }),
                &Ok(())
            ),
            Update::FetchCancelled {
                remote: "origin".to_owned(),
                stranded_locks: vec![lock],
            }
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
        match fetch_outcome("origin".to_owned(), failure(), &Ok(())) {
            Update::FetchFailed { message, .. } => {
                assert!(message.contains("boom"));
                assert!(!message.contains("could not have asked"));
            }
            other => panic!("{other:?}"),
        }
        match fetch_outcome(
            "origin".to_owned(),
            failure(),
            &Err("the askpass helper is not at /x".to_owned()),
        ) {
            Update::FetchFailed { message, .. } => {
                assert!(message.contains("boom"), "{message}");
                assert!(message.contains("helper is not at /x"), "{message}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// Caught by: a cancel while starting being dropped, or a second fetch being armed
    /// over a running one.
    #[test]
    fn a_cancel_while_starting_is_kept_and_one_fetch_at_a_time_is_armed() {
        let control = FetchControl::default();
        assert_eq!(control.arm("origin"), Ok(()));
        assert!(
            control.arm("origin").is_err(),
            "a second fetch was armed over the first"
        );
        control.cancel();
        assert!(
            matches!(
                *control.lock(),
                Stage::Starting {
                    cancelled: true,
                    ..
                }
            ),
            "the cancel that arrived before git ran was lost"
        );
        control.clear();
        assert_eq!(control.arm("origin"), Ok(()));
    }

    /// PRD R7.2: a second fetch is refused naming the fetch in flight and how far it has
    /// got, and the refusal leaves that fetch as it was. Caught by: refusing in silence
    /// (no reason to draw), naming the refused remote instead of the one in flight, or a
    /// refusal that disarms or re-arms the fetch it was refused for.
    #[test]
    fn a_second_fetch_is_refused_naming_the_fetch_in_flight() {
        let control = FetchControl::default();
        assert_eq!(control.arm("upstream"), Ok(()));
        let waiting = control.arm("origin");
        assert_eq!(
            waiting.as_ref().map_err(ToString::to_string),
            Err("a fetch of upstream is already waiting to start".to_owned())
        );
        assert!(
            matches!(
                &*control.lock(),
                Stage::Starting { remote, cancelled: false } if remote == "upstream"
            ),
            "the refusal changed the fetch it was refused for"
        );
        control.clear();
        assert_eq!(
            control.arm("origin"),
            Ok(()),
            "a refusal outlived its fetch"
        );
        // Once git is running the reason says so; through a real fetch in
        // `lifecycle_tests::a_second_fetch_while_one_runs_is_refused_with_a_reason`.
        assert_eq!(
            Refusal {
                in_flight: "upstream".to_owned(),
                running: true
            }
            .to_string(),
            "a fetch of upstream is already running"
        );
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
        assert_eq!(
            control.arm("origin"),
            Ok(()),
            "the queued fetch could not be armed"
        );
        assert!(
            matches!(
                *control.lock(),
                Stage::Starting {
                    cancelled: true,
                    ..
                }
            ),
            "the fetch was armed without the cancel that overtook it"
        );

        control.clear();
        assert_eq!(control.arm("origin"), Ok(()));
        assert!(
            matches!(
                *control.lock(),
                Stage::Starting {
                    cancelled: false,
                    ..
                }
            ),
            "the next fetch inherited a cancel that was already spent"
        );
    }

    /// Caught by: an operation routed to a lane it does not belong in, once there are two.
    #[test]
    fn a_fetch_runs_in_the_network_lane() {
        assert_eq!(
            Operation::Fetch {
                remote: "origin".to_owned()
            }
            .lane(),
            Lane::Network
        );
    }
}
