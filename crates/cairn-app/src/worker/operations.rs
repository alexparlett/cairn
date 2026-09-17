//! The operations thread: where a `git` verb runs, so a fetch neither blocks
//! the history walk nor the window. One at a time, in the order asked.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, PoisonError};

use cairn_askpass::Channel;
use cairn_git::ops::{FetchCancel, GitBinary, Invalidated, fetch};
use cairn_git::{Error, SharedRepository};

use super::pool::Outbox;
use super::request::Update;

/// What the repository thread forwards here.
#[derive(Debug)]
pub(super) enum Operation {
    Fetch { remote: String },
}

/// The cancel handle of the fetch in flight, shared with the repository
/// thread, which is where a `CancelFetch` request arrives.
#[derive(Debug, Clone, Default)]
pub(super) struct FetchControl(Arc<Mutex<Option<FetchCancel>>>);

impl FetchControl {
    /// Kills the fetch in flight; nothing to do when there is none.
    pub(super) fn cancel(&self) {
        if let Some(cancel) = self.lock().as_ref() {
            cancel.cancel();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<FetchCancel>> {
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
                // One token for the whole invocation, retired when this drops.
                let authorised = channel.and_then(|channel| channel.begin().ok());
                outbox.send(
                    None,
                    Update::FetchStarted {
                        remote: remote.clone(),
                    },
                );
                let outcome = fetch(
                    git,
                    &repo,
                    &remote,
                    authorised.as_ref().map(cairn_askpass::Operation::token),
                )
                .and_then(|started| {
                    *control.lock() = Some(started.canceller());
                    let finished = started.finish(|line| {
                        outbox.send(
                            None,
                            Update::FetchProgress {
                                remote: remote.clone(),
                                line: line.to_owned(),
                            },
                        );
                    });
                    *control.lock() = None;
                    finished
                });
                outbox.send(
                    None,
                    fetch_outcome(
                        remote,
                        outcome.map(|performed| performed.invalidated()),
                        prompting,
                    ),
                );
            }
        }
    }
}

/// The update for how a fetch ended. A failure while no prompt could have
/// been answered says so, since git's own message ("terminal prompts
/// disabled") does not say why nothing answered.
fn fetch_outcome(
    remote: String,
    outcome: Result<Invalidated, Error>,
    prompting: &Result<(), String>,
) -> Update {
    match outcome {
        Ok(invalidated) => Update::FetchFinished {
            refreshed: invalidated.refs,
            remote,
        },
        Err(Error::GitCancelled { .. }) => Update::FetchCancelled { remote },
        Err(error) => Update::FetchFailed {
            remote,
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
    fn a_finished_fetch_says_whether_the_refs_moved() {
        let moved = fetch_outcome("origin".to_owned(), Ok(Invalidated::refs()), &Ok(()));
        assert_eq!(
            moved,
            Update::FetchFinished {
                remote: "origin".to_owned(),
                refreshed: true
            }
        );
        let unmoved = fetch_outcome("origin".to_owned(), Ok(Invalidated::objects()), &Ok(()));
        assert_eq!(
            unmoved,
            Update::FetchFinished {
                remote: "origin".to_owned(),
                refreshed: false
            }
        );
    }

    #[test]
    fn a_cancelled_fetch_is_not_reported_as_a_failure() {
        let outcome = fetch_outcome(
            "origin".to_owned(),
            Err(Error::GitCancelled {
                arguments: "fetch origin".to_owned(),
            }),
            &Ok(()),
        );
        assert_eq!(
            outcome,
            Update::FetchCancelled {
                remote: "origin".to_owned()
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

    #[test]
    fn cancelling_with_nothing_in_flight_is_nothing() {
        FetchControl::default().cancel();
    }
}
