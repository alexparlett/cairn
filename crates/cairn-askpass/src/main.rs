//! `cairn-askpass`: the program `git` and `ssh` run to ask for a secret.
//!
//! Invoked with the prompt on `argv[1]`; finds the running Cairn through
//! `CAIRN_ASKPASS_SOCKET` and `CAIRN_ASKPASS_TOKEN` in its environment; writes
//! the answer to stdout with a trailing newline, and exits. On any failure it
//! writes nothing to stdout, one line to stderr that names no prompt, token or
//! secret, and exits non-zero — git then fails the operation cleanly. It does
//! nothing else, links as little as it can, and never writes a log.

use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::ExitCode;

use cairn_model::{AskpassToken, SOCKET_VARIABLE, Secret, TOKEN_VARIABLE};
use zeroize::Zeroizing;

fn main() -> ExitCode {
    match answer() {
        Ok(secret) => {
            // One write, ending in the newline git reads up to. Two writes would
            // leave the first — the secret without its newline — sitting in std's
            // line buffer, which is never zeroed; a single newline-terminated
            // write with nothing buffered goes straight to the descriptor. The
            // concatenation is the one copy made, and it is zeroed on drop.
            let mut line = Zeroizing::new(Vec::with_capacity(secret.len() + 1));
            line.extend_from_slice(secret.expose_secret());
            line.push(b'\n');
            let mut stdout = std::io::stdout().lock();
            let written = stdout.write_all(&line).and_then(|()| stdout.flush());
            match written {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => refuse("could not write the answer to git"),
            }
        }
        Err(reason) => refuse(&reason),
    }
}

fn answer() -> Result<Secret, String> {
    let prompt = std::env::args_os()
        .nth(1)
        .ok_or_else(|| "no prompt was given on the command line".to_owned())?;
    let socket = std::env::var_os(SOCKET_VARIABLE)
        .map(PathBuf::from)
        .ok_or_else(|| {
            format!("{SOCKET_VARIABLE} is not set; only a git started by Cairn can ask")
        })?;
    let token = std::env::var_os(TOKEN_VARIABLE).ok_or_else(|| {
        format!("{TOKEN_VARIABLE} is not set; only a git started by Cairn can ask")
    })?;
    let token = AskpassToken::new(String::from_utf8_lossy(token.as_bytes()).into_owned());
    cairn_askpass::ask(&socket, &token, prompt.as_bytes()).map_err(|refusal| refusal.to_string())
}

/// Nothing on stdout, one line on stderr, non-zero exit.
fn refuse(reason: &str) -> ExitCode {
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(b"cairn-askpass: ");
    let _ = stderr.write_all(reason.as_bytes());
    let _ = stderr.write_all(b"\n");
    ExitCode::FAILURE
}
