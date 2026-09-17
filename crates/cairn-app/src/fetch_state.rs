//! View state for a fetch and the credential prompt it may raise.

use std::path::PathBuf;

use crate::worker::PromptId;

/// The one fetch the window shows; a second is not offered while one is in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    Idle,
    /// Asked for, not yet confirmed running by the worker; the button is
    /// already gone so it cannot be asked for twice.
    Starting {
        remote: String,
    },
    /// `line` is git's latest progress redraw, once there is one.
    Running {
        remote: String,
        line: Option<String>,
    },
    /// Cancel was pressed; the worker has not yet said the process is gone,
    /// which takes up to the runner's grace period for a git that ignores
    /// `SIGTERM`. Still in flight, and the button is already gone.
    Cancelling {
        remote: String,
    },
    Finished {
        remote: String,
    },
    /// `stranded_locks`: what the cancel left under the git directory, if
    /// anything, for the banner to name.
    Cancelled {
        remote: String,
        stranded_locks: Vec<PathBuf>,
    },
    /// `message` is display text, already rendered from the engine's error.
    Failed {
        remote: String,
        message: String,
    },
}

impl FetchStatus {
    /// Starting, running or being cancelled: a fetch is in flight.
    pub fn is_in_flight(&self) -> bool {
        matches!(
            self,
            Self::Starting { .. } | Self::Running { .. } | Self::Cancelling { .. }
        )
    }

    /// In flight and not yet asked to stop: what the Cancel button is for.
    pub fn can_be_cancelled(&self) -> bool {
        matches!(self, Self::Starting { .. } | Self::Running { .. })
    }

    /// The remote the fetch in flight is for, which is who a prompt is asking on behalf of.
    pub fn remote_in_flight(&self) -> Option<&str> {
        match self {
            Self::Starting { remote }
            | Self::Running { remote, .. }
            | Self::Cancelling { remote } => Some(remote),
            Self::Idle | Self::Finished { .. } | Self::Cancelled { .. } | Self::Failed { .. } => {
                None
            }
        }
    }

    /// The window asked; set on the press, before the worker answers.
    pub fn starting(&mut self, remote: String) {
        *self = Self::Starting { remote };
    }

    /// The worker confirmed git is running — unless Cancel was pressed in the
    /// meantime, in which case the cancel is what the user is waiting on.
    pub fn started(&mut self, remote: String) {
        if !matches!(self, Self::Cancelling { .. }) {
            *self = Self::Running { remote, line: None };
        }
    }

    /// Cancel was pressed: said at once, since the worker's answer may be a
    /// grace period away.
    pub fn cancelling(&mut self) {
        if let Self::Starting { remote } | Self::Running { remote, .. } = self {
            *self = Self::Cancelling {
                remote: std::mem::take(remote),
            };
        }
    }

    /// Progress with nothing running is ignored.
    pub fn progressed(&mut self, line: String) {
        if let Self::Running { line: latest, .. } = self {
            *latest = Some(line);
        }
    }
}

/// A prompt the window is showing, until it is answered or withdrawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptView {
    pub id: PromptId,
    /// The prompt exactly as git or ssh gave it to the helper.
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: reverting to Running when the worker's `FetchStarted` lands after the
    /// press, which would bring the Cancel button back for a fetch already being killed.
    #[test]
    fn a_cancel_is_said_at_once_and_survives_a_late_start() {
        let mut status = FetchStatus::Idle;
        status.cancelling();
        assert_eq!(status, FetchStatus::Idle, "nothing to cancel was cancelled");

        status.starting("origin".to_owned());
        assert!(status.can_be_cancelled());
        status.cancelling();
        assert_eq!(
            status,
            FetchStatus::Cancelling {
                remote: "origin".to_owned()
            }
        );
        assert!(
            status.is_in_flight(),
            "a cancelling fetch is still in flight"
        );
        assert!(!status.can_be_cancelled(), "a second cancel was offered");
        assert_eq!(status.remote_in_flight(), Some("origin"));

        status.started("origin".to_owned());
        assert_eq!(
            status,
            FetchStatus::Cancelling {
                remote: "origin".to_owned()
            },
            "the worker's late start undid the cancel"
        );
        status.progressed("Receiving objects: 40%".to_owned());
        assert_eq!(
            status,
            FetchStatus::Cancelling {
                remote: "origin".to_owned()
            },
            "progress after a cancel was shown"
        );

        let mut running = FetchStatus::Running {
            remote: "origin".to_owned(),
            line: Some("Receiving objects: 40%".to_owned()),
        };
        running.cancelling();
        assert_eq!(
            running,
            FetchStatus::Cancelling {
                remote: "origin".to_owned()
            }
        );
    }

    #[test]
    fn progress_belongs_to_the_running_fetch_only() {
        let mut status = FetchStatus::Idle;
        status.progressed("early".to_owned());
        assert_eq!(
            status,
            FetchStatus::Idle,
            "progress with nothing running was kept"
        );

        status.starting("origin".to_owned());
        assert!(status.is_in_flight());
        status.progressed("not yet".to_owned());
        assert_eq!(
            status,
            FetchStatus::Starting {
                remote: "origin".to_owned()
            },
            "progress before git ran was kept"
        );
        status.started("origin".to_owned());
        assert!(status.is_in_flight());
        assert_eq!(status.remote_in_flight(), Some("origin"));
        status.progressed("Receiving objects: 40%".to_owned());
        assert_eq!(
            status,
            FetchStatus::Running {
                remote: "origin".to_owned(),
                line: Some("Receiving objects: 40%".to_owned())
            }
        );
    }

    #[test]
    fn only_a_fetch_in_flight_has_a_remote_to_name() {
        for done in [
            FetchStatus::Idle,
            FetchStatus::Finished {
                remote: "origin".to_owned(),
            },
            FetchStatus::Cancelled {
                remote: "origin".to_owned(),
                stranded_locks: Vec::new(),
            },
            FetchStatus::Failed {
                remote: "origin".to_owned(),
                message: "no".to_owned(),
            },
        ] {
            assert!(!done.is_in_flight(), "{done:?}");
            assert!(!done.can_be_cancelled(), "{done:?}");
            assert_eq!(done.remote_in_flight(), None, "{done:?}");
        }
        assert_eq!(
            FetchStatus::Starting {
                remote: "upstream".to_owned()
            }
            .remote_in_flight(),
            Some("upstream")
        );
    }
}
