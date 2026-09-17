//! View state for a fetch and the credential prompt it may raise.

use crate::worker::PromptId;

/// The one fetch the window shows; a second is not offered while one runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    Idle,
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
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    /// The remote the running fetch is for, which is who a prompt is asking on behalf of.
    pub fn running_remote(&self) -> Option<&str> {
        match self {
            Self::Running { remote, .. } => Some(remote),
            Self::Idle | Self::Finished { .. } | Self::Cancelled { .. } | Self::Failed { .. } => {
                None
            }
        }
    }

    pub fn started(&mut self, remote: String) {
        *self = Self::Running { remote, line: None };
    }

    /// Progress for a fetch that is not the running one is ignored.
    pub fn progressed(&mut self, remote: &str, line: String) {
        if let Self::Running {
            remote: running,
            line: latest,
        } = self
            && running == remote
        {
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
        status.progressed("origin", "early".to_owned());
        assert_eq!(
            status,
            FetchStatus::Idle,
            "progress with nothing running was kept"
        );

        status.started("origin".to_owned());
        assert!(status.is_running());
        assert_eq!(status.running_remote(), Some("origin"));
        status.progressed("upstream", "not mine".to_owned());
        assert_eq!(
            status,
            FetchStatus::Running {
                remote: "origin".to_owned(),
                line: None
            }
        );
        status.progressed("origin", "Receiving objects: 40%".to_owned());
        assert_eq!(
            status,
            FetchStatus::Running {
                remote: "origin".to_owned(),
                line: Some("Receiving objects: 40%".to_owned())
            }
        );
    }

    #[test]
    fn only_a_running_fetch_has_a_remote_to_name() {
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
            assert!(!done.is_running(), "{done:?}");
            assert_eq!(done.running_remote(), None, "{done:?}");
        }
    }
}
