//! The application's end of the channel: where the helper's questions arrive.
//!
//! A [`Channel`] is one socket, alive for as long as the Cairn instance that
//! opened it. An [`Operation`] is one token, alive for as long as the git
//! invocation it authorises; a [`Prompt`] is one question from one helper,
//! answered with a secret or refused. The threat model — other users, not
//! same-user processes — is in the crate docs; this module implements the
//! permissions that model rests on and refuses to serve if they did not take.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::net::Shutdown;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use cairn_model::{AskpassToken, Secret};

use crate::protocol::{self, REQUEST_LIMIT};
use crate::token;

/// Mode of the directory the socket lives in: the owner only.
const DIRECTORY_MODE: u32 = 0o700;

/// Mode of the socket itself: the owner only.
const SOCKET_MODE: u32 = 0o600;

/// The socket's file name inside its directory.
const SOCKET_NAME: &str = "askpass";

/// How much of an over-long request is read and discarded past
/// [`REQUEST_LIMIT`] so the helper's write completes: well past the kernel's
/// ceiling for one argument, which is what bounds a prompt.
const REQUEST_DRAIN: u64 = 4 * 1024 * 1024;

/// What can go wrong on the application's side.
#[derive(Debug)]
pub enum Error {
    /// `$XDG_RUNTIME_DIR` is missing or is not a directory; there is nowhere
    /// safe to put the socket, so there is no channel.
    NoRuntimeDirectory { path: PathBuf, source: io::Error },
    /// The socket's directory or the socket could not be created.
    Create { path: PathBuf, source: io::Error },
    /// Something was created, but not with the mode the threat model needs;
    /// the channel refuses to serve on it.
    Permissions {
        path: PathBuf,
        mode: u32,
        required: u32,
    },
    /// The kernel had no randomness to issue a token from.
    Token { source: io::Error },
    /// Waiting for a helper failed.
    Accept { source: io::Error },
    /// A connection that was not a helper's request; nothing was served.
    Malformed,
    /// A helper presented a token this channel is not serving: the operation
    /// ended, a prompt under it was refused, or it was never issued here.
    UnknownToken,
    /// Writing the answer to the helper failed; it may have gone away.
    Answer { source: io::Error },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRuntimeDirectory { path, source } => write!(
                f,
                "no runtime directory at {} to put the askpass socket in: {source}",
                path.display()
            ),
            Self::Create { path, source } => {
                write!(f, "could not create {}: {source}", path.display())
            }
            Self::Permissions {
                path,
                mode,
                required,
            } => write!(
                f,
                "{} has mode {mode:o}, not {required:o}; refusing to serve credentials on it",
                path.display()
            ),
            Self::Token { source } => write!(f, "could not generate an askpass token: {source}"),
            Self::Accept { source } => write!(f, "waiting for the askpass helper failed: {source}"),
            Self::Malformed => f.write_str("a connection to the askpass socket was not a helper"),
            Self::UnknownToken => {
                f.write_str("an askpass helper presented a token for no operation in progress")
            }
            Self::Answer { source } => write!(f, "answering the askpass helper failed: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoRuntimeDirectory { source, .. }
            | Self::Create { source, .. }
            | Self::Token { source }
            | Self::Accept { source }
            | Self::Answer { source } => Some(source),
            Self::Permissions { .. } | Self::Malformed | Self::UnknownToken => None,
        }
    }
}

/// The tokens currently good for a prompt.
type Live = Arc<Mutex<BTreeSet<AskpassToken>>>;

fn live(tokens: &Live) -> std::sync::MutexGuard<'_, BTreeSet<AskpassToken>> {
    tokens.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One socket, owned by the Cairn instance; removed with its directory on drop.
#[derive(Debug)]
pub struct Channel {
    directory: PathBuf,
    path: PathBuf,
    listener: UnixListener,
    tokens: Live,
}

impl Channel {
    /// Creates `runtime_dir/cairn-<pid>-<random>/` with mode `0700` and binds
    /// the socket inside it with mode `0600`, then checks both modes took;
    /// anything else is an [`Error`] and no channel. `runtime_dir` is the
    /// caller's `$XDG_RUNTIME_DIR`, which must already exist.
    pub fn open(runtime_dir: &Path) -> Result<Self, Error> {
        let runtime = fs::metadata(runtime_dir).map_err(|source| Error::NoRuntimeDirectory {
            path: runtime_dir.to_owned(),
            source,
        })?;
        if !runtime.is_dir() {
            return Err(Error::NoRuntimeDirectory {
                path: runtime_dir.to_owned(),
                source: io::Error::other("not a directory"),
            });
        }

        // Random, so a stale directory of a dead process never collides; and
        // created with the mode rather than chmod'd afterwards, so there is no
        // instant at which the directory is wider than it will end up.
        let suffix = token::fresh().map_err(|source| Error::Token { source })?;
        let directory = runtime_dir.join(format!(
            "cairn-{}-{}",
            std::process::id(),
            &suffix.as_str()[..8]
        ));
        fs::DirBuilder::new()
            .mode(DIRECTORY_MODE)
            .create(&directory)
            .map_err(|source| Error::Create {
                path: directory.clone(),
                source,
            })?;
        // Removed again if anything below fails; disarmed once the channel owns it.
        let mut cleanup = RemoveOnFailure {
            directory: Some(directory.clone()),
        };
        // Belt and braces over the mode above: a umask cannot widen it, but an
        // inherited default ACL could.
        set_mode(&directory, DIRECTORY_MODE)?;

        // Inside a 0700 directory, so the moment between bind and chmod is not
        // one anybody else can use.
        let path = directory.join(SOCKET_NAME);
        let listener = UnixListener::bind(&path).map_err(|source| Error::Create {
            path: path.clone(),
            source,
        })?;
        set_mode(&path, SOCKET_MODE)?;

        cleanup.directory = None;
        Ok(Self {
            directory,
            path,
            listener,
            tokens: Arc::new(Mutex::new(BTreeSet::new())),
        })
    }

    /// Where the helper connects: the value of `CAIRN_ASKPASS_SOCKET`.
    pub fn socket_path(&self) -> &Path {
        &self.path
    }

    /// Issues the token for one git invocation. Prompts presenting it are
    /// served until the returned [`Operation`] is dropped.
    pub fn begin(&self) -> Result<Operation, Error> {
        let token = token::fresh().map_err(|source| Error::Token { source })?;
        live(&self.tokens).insert(token.clone());
        Ok(Operation {
            token,
            tokens: Arc::clone(&self.tokens),
        })
    }

    /// Blocks until a helper connects, then reads its question. A connection
    /// that is not a well-formed request, or that presents a token this
    /// channel is not serving, is refused on the wire and reported as an
    /// [`Error`]; the caller decides whether to keep accepting.
    pub fn accept(&self) -> Result<Prompt, Error> {
        let (mut stream, _) = self
            .listener
            .accept()
            .map_err(|source| Error::Accept { source })?;
        let request = protocol::read_request((&stream).take(REQUEST_LIMIT))
            .map_err(|source| Error::Accept { source })?;
        // A request longer than the limit is cut there, but the helper may still be
        // writing the rest. Closing with unread bytes in the socket resets the
        // connection, and the helper then loses the answer it was owed; so the rest
        // is read and discarded, up to a bound a real argv cannot exceed, before
        // anything is written back.
        let _ = io::copy(&mut (&stream).take(REQUEST_DRAIN), &mut io::sink());
        let Ok(request) = request else {
            let _ = protocol::write_refusal(&mut stream);
            return Err(Error::Malformed);
        };
        if !live(&self.tokens).contains(&request.token) {
            let _ = protocol::write_refusal(&mut stream);
            return Err(Error::UnknownToken);
        }
        Ok(Prompt {
            text: String::from_utf8_lossy(&request.prompt).into_owned(),
            token: request.token,
            stream: Some(stream),
            tokens: Arc::clone(&self.tokens),
        })
    }
}

/// Removes a half-built socket directory when `open` fails partway.
struct RemoveOnFailure {
    directory: Option<PathBuf>,
}

impl Drop for RemoveOnFailure {
    fn drop(&mut self) {
        if let Some(directory) = self.directory.take() {
            let _ = fs::remove_file(directory.join(SOCKET_NAME));
            let _ = fs::remove_dir(directory);
        }
    }
}

fn set_mode(path: &Path, mode: u32) -> Result<(), Error> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|source| {
        Error::Create {
            path: path.to_owned(),
            source,
        }
    })?;
    let actual = fs::symlink_metadata(path)
        .map_err(|source| Error::Create {
            path: path.to_owned(),
            source,
        })?
        .permissions()
        .mode()
        & 0o777;
    if actual != mode {
        return Err(Error::Permissions {
            path: path.to_owned(),
            mode: actual,
            required: mode,
        });
    }
    Ok(())
}

impl Drop for Channel {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.directory);
    }
}

/// One git invocation's authorisation to be asked. Dropping it retires the
/// token: a helper presenting it afterwards is refused.
#[derive(Debug)]
pub struct Operation {
    token: AskpassToken,
    tokens: Live,
}

impl Operation {
    /// The value of `CAIRN_ASKPASS_TOKEN` for this invocation.
    pub fn token(&self) -> &AskpassToken {
        &self.token
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        live(&self.tokens).remove(&self.token);
    }
}

/// One helper's question, waiting on an answer. Drop it unanswered and the
/// helper is refused and the operation's token retired, exactly as
/// [`Prompt::refuse`] does: a prompt nobody answered is a prompt nobody
/// should be asked again.
#[derive(Debug)]
pub struct Prompt {
    text: String,
    token: AskpassToken,
    stream: Option<UnixStream>,
    tokens: Live,
}

impl Prompt {
    /// What git or ssh asked, as the helper was given it on `argv[1]` —
    /// `Password for 'https://user@host': `, say. Lossily decoded; a prompt
    /// is for showing, never parsing on the wire.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Hands the secret to the helper, which writes it to git. The token
    /// stays live: git may have a second question for the same operation.
    pub fn answer(mut self, secret: &Secret) -> Result<(), Error> {
        let Some(mut stream) = self.stream.take() else {
            return Ok(());
        };
        let written = protocol::write_answer(&mut stream, secret)
            .and_then(|()| stream.shutdown(Shutdown::Write));
        written.map_err(|source| Error::Answer { source })
    }

    /// The user declined. The helper gets nothing, and the operation's token
    /// is retired so git's next ask fails instead of asking again.
    pub fn refuse(mut self) {
        self.refuse_in_place();
    }

    fn refuse_in_place(&mut self) {
        live(&self.tokens).remove(&self.token);
        if let Some(mut stream) = self.stream.take() {
            let _ = protocol::write_refusal(&mut stream);
            let _ = stream.shutdown(Shutdown::Write);
        }
    }
}

impl Drop for Prompt {
    fn drop(&mut self) {
        if self.stream.is_some() {
            self.refuse_in_place();
        }
    }
}
