//! Every `git` process Cairn starts is built here, and nowhere else.
//!
//! Crate-private. It holds the binary Cairn found ([`GitBinary`]), the
//! environment every invocation runs with ([`GitEnvironment`], pointing git and
//! ssh at [`Askpass`]), the runner that drives a process from spawn to reap,
//! and each repository's registry of what is still running and log of what
//! ran (`registry.rs`, `command_log.rs`). The application constructs the binary, the environment and the
//! askpass target through re-exports from [`crate::ops`], because it owns
//! startup and the helper's channel; it never reaches the runner.
//!
//! # Who may run an invocation
//!
//! Two modules, and only they: [`crate::ops`], where every mutation lives,
//! and `crate::reads`, where every read that `git` answers lives, one named
//! function each. The one other invocation is this module's own version
//! probe, which reads no repository.
//!
//! # The seal
//!
//! An invocation is built as a **read** or a **write**, and its type says
//! which: [`GitBinary::read_invocation`] or [`GitBinary::write_invocation`].
//! A write can only be built from a [`crate::ops::WriteAuthority`], a value
//! whose only constructor is private to `ops/` — a token, because a
//! visibility like `pub(in crate::ops)` cannot be written on an item that
//! lives here — so a read path cannot spawn a mutation by building the wrong
//! kind of invocation. The type also decides the environment (see
//! `environment.rs`): every invocation gets the base, with
//! `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false` in it; a read adds
//! `GIT_OPTIONAL_LOCKS=0` and has nowhere to put an askpass token; a write
//! carries the operation's token when it may prompt.
//!
//! Privacy stops at the crate, so the guard suite pins what the compiler
//! cannot (`crates/cairn-guards/tests/invariants.rs`): nothing outside this
//! module builds, spawns, waits on or reads a process, or calls a method that
//! yields one (`only_the_process_module_builds_or_runs_a_process`); nothing
//! outside it, `ops/` and `reads/` names the runner, nothing outside `ops/`
//! constructs a `WriteAuthority` or builds a write
//! (`the_runner_is_named_only_by_ops_and_reads`); and the environment is
//! built in one place (`every_git_invocation_disables_the_terminal_prompt`).

mod askpass;
mod binary;
mod cli;
mod command_log;
mod environment;
mod group;
mod pipes;
mod registry;
mod runner;
#[cfg(all(test, unix))]
pub(crate) mod stub_git;

pub use askpass::Askpass;
pub use binary::{GitBinary, GitVersion};
pub(crate) use cli::Write;
pub use environment::GitEnvironment;
pub(crate) use group::KillHandle;
pub use registry::CLOSE_BOUND;
pub(crate) use registry::Processes;
pub(crate) use runner::Invocation;
