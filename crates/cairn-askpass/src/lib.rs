//! The askpass helper and the channel it asks over.
//!
//! `git` cannot ask a GUI for a password; it can only run a program with the
//! prompt on `argv[1]` and read the answer from that program's stdout
//! (`GIT_ASKPASS`; `ssh` does the same through `SSH_ASKPASS`). So the helper
//! is a process — decision L2 of the credential-prompts packet — and this
//! crate is both halves of the conversation: the binary `git` runs, and the
//! [`Channel`] the running Cairn listens on. The secret's whole life is: the
//! dialog in Cairn, this channel, the helper's stdout, git's stdin. It never
//! enters `cairn-git`, and it never enters application state.
//!
//! # The channel, and what it protects against
//!
//! The channel is a unix socket in `$XDG_RUNTIME_DIR`, inside a directory
//! created `0700`, the socket itself `0600`, never the Linux abstract
//! namespace (which carries no permissions at all). The helper finds it, and
//! the token for the operation it serves, in its environment — never on
//! `argv`, which `/proc` makes world-readable (L6). The token is issued when
//! an operation begins and is dead once the operation ends or a prompt under
//! it is refused, so a helper cannot be answered for an operation Cairn is
//! not running.
//!
//! **What this protects against is other users on the machine, not other
//! processes running as the same user** (decision L10). Same-user isolation
//! is not achievable: a process running as the user can read this process's
//! environment through `/proc`, and with it the socket path and the token.
//! It is also not worth pursuing, because that same process could read
//! `~/.git-credentials` or query the ssh-agent directly — the boundary a
//! stronger channel would defend does not exist anywhere else on the system.
//! Writing the limit down is the point: a threat model that overclaims is
//! worse than one that is narrow. Rejected on those grounds: `SO_PEERCRED`
//! peer verification (raises the bar against a racing same-user process, but a
//! determined one defeats it, and it buys complexity for a boundary that
//! cannot hold) and a pre-opened inherited descriptor (depends on git
//! preserving inherited fds across the askpass invocation, which is
//! unverified).
//!
//! # The token is per operation
//!
//! One `git fetch` may ask more than once — `Username for 'https://host':`,
//! then `Password for 'https://user@host':` — and each ask is a fresh helper
//! process with the same environment, since git passes its own through. A
//! token good for exactly one ask would therefore fail the second question of
//! every HTTPS credential. So a token is good for the prompts of ONE
//! operation, and is retired the moment that operation ends, or the moment
//! the user refuses a prompt under it — after a cancel, git's next ask fails
//! closed rather than asking again (PRD R2.5).
//!
//! # The helper's failure mode is silence
//!
//! Whatever goes wrong — no channel in the environment, no Cairn listening, a
//! socket with the wrong permissions, a token the channel does not know, a
//! refused prompt — the helper writes nothing to stdout and exits non-zero.
//! git then reports that it could not read a credential (its own terminal
//! prompt is disabled on every invocation) and the operation fails cleanly.
//! The helper never writes a log; a one-line reason goes to stderr, which
//! git forwards, and that reason never contains the prompt, the token or a
//! secret.

mod ask;
mod channel;
mod protocol;
mod token;

pub use ask::{Refusal, ask};
pub use channel::{Channel, Error, Operation, Prompt};
