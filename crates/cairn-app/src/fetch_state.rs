//! View state for a fetch and the credential prompt it may raise.

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
    Finished {
        remote: String,
    },
    Cancelled {
        remote: String,
    },
    /// `message` is display text, already rendered from the engine's error.
    Failed {
        remote: String,
        message: String,
    },
}

impl FetchStatus {
    /// Starting or running: a fetch is in flight and can be cancelled.
    pub fn is_in_flight(&self) -> bool {
        matches!(self, Self::Starting { .. } | Self::Running { .. })
    }

    /// The remote the fetch in flight is for, which is who a prompt is asking on behalf of.
    pub fn remote_in_flight(&self) -> Option<&str> {
        match self {
            Self::Starting { remote } | Self::Running { remote, .. } => Some(remote),
            Self::Idle | Self::Finished { .. } | Self::Cancelled { .. } | Self::Failed { .. } => {
                None
            }
        }
    }

    /// The window asked; set on the press, before the worker answers.
    pub fn starting(&mut self, remote: String) {
        *self = Self::Starting { remote };
    }

    pub fn started(&mut self, remote: String) {
        *self = Self::Running { remote, line: None };
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
            },
            FetchStatus::Failed {
                remote: "origin".to_owned(),
                message: "no".to_owned(),
            },
        ] {
            assert!(!done.is_in_flight(), "{done:?}");
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
