//! The helper's end of the channel: one question, one answer.

use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::net::Shutdown;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use cairn_model::{AskpassToken, Secret};

use crate::protocol::{self, RESPONSE_LIMIT};

/// Why the helper has nothing to write. Each names a path or a cause, never a
/// prompt, a token or a secret: this text goes to stderr, which git forwards.
#[derive(Debug)]
pub enum Refusal {
    /// The path in the environment is not a socket at all.
    NotASocket { path: PathBuf },
    /// The socket, or the directory it sits in, is readable beyond its owner;
    /// the helper does not hand a secret to it.
    Permissions { path: PathBuf, mode: u32 },
    /// The socket, or the directory it sits in, belongs to somebody else.
    NotOurs { path: PathBuf },
    /// No Cairn is listening there.
    Unreachable { path: PathBuf, source: io::Error },
    /// The conversation broke off before an answer.
    Broken { source: io::Error },
    /// Cairn answered, and the answer was no: the user declined, or the
    /// operation this helper serves is not one Cairn is running.
    Declined,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotASocket { path } => {
                write!(f, "{} is not the Cairn askpass socket", path.display())
            }
            Self::Permissions { path, mode } => write!(
                f,
                "{} is reachable beyond its owner (mode {mode:o}); not handing a secret to it",
                path.display()
            ),
            Self::NotOurs { path } => write!(
                f,
                "{} is not owned by this user; not handing a secret to it",
                path.display()
            ),
            Self::Unreachable { path, source } => {
                write!(f, "no Cairn listening at {}: {source}", path.display())
            }
            Self::Broken { source } => write!(f, "lost the connection to Cairn: {source}"),
            Self::Declined => f.write_str("Cairn declined to answer"),
        }
    }
}

impl std::error::Error for Refusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreachable { source, .. } | Self::Broken { source } => Some(source),
            Self::NotASocket { .. }
            | Self::Permissions { .. }
            | Self::NotOurs { .. }
            | Self::Declined => None,
        }
    }
}

/// Asks the Cairn listening at `socket` to answer `prompt` for the operation
/// `token` authorises. Blocks until Cairn answers or closes the socket: a user
/// takes as long as they take, and a Cairn that dies closes it.
pub fn ask(socket: &Path, token: &AskpassToken, prompt: &[u8]) -> Result<Secret, Refusal> {
    owner_only(socket)?;
    let mut stream = UnixStream::connect(socket).map_err(|source| Refusal::Unreachable {
        path: socket.to_owned(),
        source,
    })?;
    protocol::write_request(&mut stream, token, prompt)
        .and_then(|()| stream.shutdown(Shutdown::Write))
        .map_err(|source| Refusal::Broken { source })?;
    let answer = protocol::read_response((&stream).take(RESPONSE_LIMIT))
        .map_err(|source| Refusal::Broken { source })?;
    answer.ok_or(Refusal::Declined)
}

/// The socket must be a socket, and neither it nor its directory may carry
/// group or other bits or belong to another user. Not followed through a
/// symlink: a link is not ours. Belt and braces over the directory's `0700`,
/// for a runtime directory that is not the private one the XDG spec promises.
fn owner_only(socket: &Path) -> Result<(), Refusal> {
    let meta = fs::symlink_metadata(socket).map_err(|source| Refusal::Unreachable {
        path: socket.to_owned(),
        source,
    })?;
    if !meta.file_type().is_socket() {
        return Err(Refusal::NotASocket {
            path: socket.to_owned(),
        });
    }
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(Refusal::Permissions {
            path: socket.to_owned(),
            mode,
        });
    }
    if !ours(&meta) {
        return Err(Refusal::NotOurs {
            path: socket.to_owned(),
        });
    }
    if let Some(directory) = socket.parent() {
        let meta = fs::symlink_metadata(directory).map_err(|source| Refusal::Unreachable {
            path: directory.to_owned(),
            source,
        })?;
        let mode = meta.permissions().mode() & 0o777;
        if !meta.is_dir() || mode & 0o077 != 0 {
            return Err(Refusal::Permissions {
                path: directory.to_owned(),
                mode,
            });
        }
        if !ours(&meta) {
            return Err(Refusal::NotOurs {
                path: directory.to_owned(),
            });
        }
    }
    Ok(())
}

/// Whether `meta` is owned by the user running this process. Read off
/// `/proc/self`, which the kernel owns to the process's user; without `/proc`
/// (not Linux) the owner cannot be learned without a C binding, and the mode
/// checks above are what remains.
fn ours(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    match fs::metadata("/proc/self") {
        Ok(this_process) => this_process.uid() == meta.uid(),
        Err(_) => true,
    }
}
