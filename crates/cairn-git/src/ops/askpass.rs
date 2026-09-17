//! Where git and ssh are sent for a secret.

use std::path::{Path, PathBuf};

/// Where Cairn's askpass helper is, and where it should connect.
///
/// `program` becomes `GIT_ASKPASS` and `SSH_ASKPASS`: the helper git and ssh
/// run to ask for a secret. `socket` becomes `CAIRN_ASKPASS_SOCKET`, the
/// channel the helper reaches the running Cairn over; with `None` the helper
/// is still named, so neither git nor ssh falls back to a terminal, but it
/// finds no channel and fails closed — every prompt becomes a clean failure
/// rather than a hang. `None` is for a Cairn with no channel, not for an
/// operation that forgot to open one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Askpass {
    program: PathBuf,
    socket: Option<PathBuf>,
}

impl Askpass {
    pub fn new(program: impl Into<PathBuf>, socket: Option<PathBuf>) -> Self {
        Self {
            program: program.into(),
            socket,
        }
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    pub fn socket(&self) -> Option<&Path> {
        self.socket.as_deref()
    }
}
