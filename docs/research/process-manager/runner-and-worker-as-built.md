# The `git` runner and the worker, as built — evidence for a process manager

Evidence record, saved in full. Commissioned 2026-10-02 by the process-manager
packet planning. Read against origin/main at 2987a53 and feature/diff-engine at
44fa6a8. File:line references are era artifacts. Historical: never retro-edited.

Purpose: verify `docs/research/diff-engine/git-process-survey.md` (written
2026-09-30 against `7d4d9ad`/`2987a53` and diff-engine `2c8ae1e`) against the code,
and go deeper on what a designer needs to redesign the subprocess layer. Anything
not verified by reading code, docs or vendored dependency source is marked
**OPEN**. Nothing here was compiled or run; behavioural claims are read from the
code and from what its tests assert.

## 0. Refs and drift

- `crates/` is byte-identical between `7d4d9ad` and `2987a53` except
  `crates/cairn-guards/tests/invariants.rs` (+168, the design-docs point-in-time
  guard, appended after line 1945) and the new
  `crates/cairn-guards/tests/session_link_hook.rs`. No process code or process guard
  moved; the survey's guard line numbers still match.
- `feature/diff-engine` at `44fa6a8` is based on `2987a53`. Against it, the branch
  touches nothing under `crates/cairn-git/src/ops/`, `crates/cairn-app/src/worker/`
  or the process guards. Its only guard change is one `TEST_ONLY_ALLOWLIST` row
  (`("cairn-model", &["allocation-counter"])`) and a matching workspace dependency.
  Nothing in `crates/cairn-git/src/diff/` names `GitBinary`, `GitCommand`, `ops`,
  `Command` or a process. Code under `crates/` is unchanged since the survey's
  `2c8ae1e`, apart from the guard changes merged in from main.
- In the planning checkout the survey and `rename-parity-spike.md` are present as
  **staged additions** copied from diff-engine commit `2fef0ca`, not commits on
  `2987a53`.
- No crate-level `CLAUDE.md` exists under `crates/cairn-git` or `crates/cairn-app`;
  the only CLAUDE.md files are the repository root and `docs/CLAUDE.md`.

## 1. `crates/cairn-git/src/ops/` in full

### 1.1 Module layout and visibility (`ops/mod.rs`)

```
mod askpass; mod binary; mod cli; mod environment; mod fetch;
mod refspec_policy; mod stranded_locks;
#[cfg(all(test, unix))] mod stub_git;

pub use askpass::Askpass;
pub use binary::{GitBinary, GitVersion};
pub(crate) use cli::GitCommand;
pub use environment::GitEnvironment;
pub use fetch::{FetchCancel, FetchInProgress, fetch};
```
(`mod.rs:116-132`). `lib.rs` declares `pub mod ops;`, so the `pub use` items are
`cairn_git::ops::*` for `cairn-app`.

Reachability, which matters for any guard design:

| Item | Declared | Reachable from |
|---|---|---|
| `GitBinary`, `GitVersion`, `GitEnvironment`, `Askpass`, `fetch`, `FetchInProgress`, `FetchCancel`, `Invalidated`, `Performed` | `pub` | every crate |
| `GitBinary::command()` → `GitCommand<'_>` | `pub(crate)` method on a `pub` type | **all of `cairn-git`**, not just `ops` |
| `GitCommand` (re-export) | `pub(crate) use` | all of `cairn-git` |
| `GitCommand::{new,arg,args,in_repository,authorized_by,run,stream}` | `pub(crate)` | all of `cairn-git` |
| `GitEnvironment::command(&self, &Path, Option<&AskpassToken>) -> Command` | `pub(crate)` | all of `cairn-git` (via `git.environment().command(..)`) |
| `cli::{Running, ProcessKill, Output, TERMINATION_GRACE}` | `pub(crate)` inside private `mod cli` | **nameable** only inside `ops` (fetch.rs imports `crate::ops::cli::{ProcessKill, Running}`); but values of these types are returned by `GitCommand::run`/`stream`, so any `cairn-git` module can hold and use them through inference |
| `refspec_policy::check`, `stranded_locks::stranded_locks` | `pub(crate)` in private modules | `ops` only |
| `Performed::new`, `Performed::destructive` | `pub(crate)` | all of `cairn-git` |

So the runner's seal is **crate-level**, not module-level. `cli.rs:16-18` ("a
caller outside `ops` cannot run a verb") and `mod.rs:38-40` ("Nothing outside `ops`
can run a raw verb") are true of other crates only.

### 1.2 Stated policies (`ops/mod.rs` docs)

- `mod.rs:1-5`: "This module is the only place in Cairn that writes to a
  repository, and the guard suite pins that: a mutating gitoxide call or a `git`
  subprocess anywhere else fails the gate." The gitoxide half is **not guarded**
  (see 3.2).
- `mod.rs:13-44` "How a mutation runs": `GitBinary` once at startup, refuses
  `< MINIMUM` loudly; `GitEnvironment` built from a roster and the only place a
  `Command` is built; `GitCommand` runs with stdin closed, to completion or
  streaming stderr and killable; cancel is `SIGTERM` then `SIGKILL` after a bounded
  grace; `stranded_locks` reported on the cancel error; `refspec_policy` refuses
  before any process starts.
- `mod.rs:46-54` **Output policy**: machine-readable forms only (`-z` for paths —
  `Output::records` splits it — `--porcelain=v2` for status, `--format` with
  explicit separators); human output never parsed where a machine form exists;
  **stderr is prose, travels into the error verbatim, never matched on.**
- `mod.rs:56-67` **Errors**: `GitNotFound`/`GitTooOld`/`GitVersionUnreadable` at
  startup, `GitNotStarted`, `GitFailed{arguments,status,stderr}`. An outcome that
  must be told apart from "git failed" (a refused credential) must come from a
  channel of its own (askpass), never from git's prose.
- `mod.rs:69-114` **Cache-invalidation contract**: every operation declares
  `Invalidated{refs,index,objects,working_tree}` on its `Performed`; the worker
  honours it. `refs` → drop the open `HistorySession`/cursor and re-query from
  `HEAD`; `index` → rerun queries that read it (same-mtime-tick residual: an op
  that writes then reads the index in one request must reopen a handle);
  `objects` → nothing needed under gix's default `RefreshMode::AfterAllIndicesLoaded`;
  `working_tree` → rerun status/diff.
- `Invalidated` (`mod.rs:138-196`): `pub` fields; `NOTHING`, `refs()`,
  `objects()`, `index()`, `working_tree()`, `and()`, `anything()` — all `const fn`.
- `Performed` (`mod.rs:204-245`): private fields `description`, `acknowledged:
  Option<String>`, `invalidated`; `pub(crate) fn new(description, invalidated)`,
  `pub(crate) fn destructive(description, &Confirmed, invalidated)`; `pub` getters.
- `describe_destructive(repo, Confirmed) -> Performed` (`mod.rs:249-256`): a no-op
  placeholder that is the only thing satisfying the destructive-seal guard today.
- Tests (`mod.rs:258-314`): the placeholder records the prompt; an unconfirmed op
  carries none; every single flag counts as `anything()`; `and` loses no flag.

### 1.3 `binary.rs` — `GitBinary`, `GitVersion`

```rust
pub struct GitVersion { pub major: u32, pub minor: u32, pub patch: u32 }   // derives Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord
impl GitVersion { pub const MINIMUM: Self /* 2.30.0 */; pub fn parse(output: &str) -> Option<Self> }
impl fmt::Display for GitVersion
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBinary { path: PathBuf, version: GitVersion, environment: GitEnvironment }
impl GitBinary {
    pub fn discover(askpass: &Askpass) -> Result<Self, Error>;          // GitEnvironment from std::env::var_os
    pub fn discover_with(environment: GitEnvironment) -> Result<Self, Error>;
    pub fn path(&self) -> &Path;
    pub fn version(&self) -> GitVersion;
    pub fn environment(&self) -> &GitEnvironment;
    pub(crate) fn command(&self) -> GitCommand<'_>;
}
```
- `locate` (`binary.rs:113-132`) searches the **built** environment's `PATH` (not
  the process's), first executable `git` wins; `GitNotFound{searched, required}`.
  `is_executable` has a `cfg(unix)` mode-bit check and a `cfg(not(unix))`
  `is_file` fallback.
- `probe` (`binary.rs:146-154`) runs `git --version` through
  `GitCommand::new(path, env).arg("--version").run()` — **the only production call
  of `run()`**. Unreadable → `GitVersionUnreadable{path, output, required}`; too old
  → `GitTooOld{path, found, required}`.
- `discover` (process environment) has no production caller;
  `cairn-app/src/worker/startup.rs:98` uses `discover_with`.
- `GitBinary` is `Clone` (a `PathBuf`, a `Copy` version and a `BTreeMap` clone), so
  handing a copy to a second thread is cheap and needs no new API.
- Unit tests (`binary.rs:156-206`): field forms of `git --version`, refusals,
  numeric ordering, `MINIMUM` displays as `2.30.0`.
- Integration tests `crates/cairn-git/tests/git_binary.rs` (`#![cfg(unix)]`, stub on
  a controlled `PATH`): missing git names the version; unset `PATH` reported;
  too-old names both versions; unreadable/silent version refused quoting output;
  stderr reaches the error; non-executable file not run; floor accepted; newer
  accepted; first `PATH` dir wins; installed git discovered from the process
  environment; a git that cannot start names program and cause.

### 1.4 `environment.rs` — `GitEnvironment`

```rust
const ALWAYS: &[(&str, &str)] = &[("GIT_TERMINAL_PROMPT", "0"), ("SSH_ASKPASS_REQUIRE", "force")];
const INHERITED: &[&str] = &[ /* 26 names */ ];
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitEnvironment { entries: BTreeMap<String, OsString> }
impl GitEnvironment {
    pub fn new(parent: impl Fn(&str) -> Option<OsString>, askpass: &Askpass) -> Self;
    pub fn variables(&self) -> impl Iterator<Item = (&str, &OsStr)>;
    pub fn get(&self, name: &str) -> Option<&OsStr>;
    pub(crate) fn command(&self, program: &Path, token: Option<&AskpassToken>) -> Command;
}
```
- `INHERITED` (26, verified by count): `PATH HOME XDG_CONFIG_HOME XDG_CACHE_HOME
  DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR SSH_AUTH_SOCK TMPDIR LANGUAGE LANG LC_ALL
  LC_CTYPE LC_MESSAGES http_proxy https_proxy HTTPS_PROXY all_proxy ALL_PROXY
  no_proxy NO_PROXY SSL_CERT_FILE SSL_CERT_DIR GIT_SSL_CAINFO GIT_SSL_CAPATH
  KRB5CCNAME KRB5_CONFIG`, each with its reason inline.
- `new` inserts the inherited names the parent has, then `ALWAYS` (so `ALWAYS`
  beats the parent), then `GIT_ASKPASS` and `SSH_ASKPASS` = helper path, then
  `CAIRN_ASKPASS_SOCKET` when `askpass.socket()` is `Some`.
- `command` is **the only `Command::new` in production**: `Command::new(program)`,
  `env_clear()`, `envs(&self.entries)`, plus `CAIRN_ASKPASS_TOKEN` from the token
  when given. It sets nothing else: no stdio, no cwd, no process group.
- Deliberately absent (doc comment and tests): every other `GIT_*` (incl.
  `GIT_DIR`, `GIT_CONFIG_GLOBAL`, `GIT_SSH_COMMAND`, `GIT_SSH`, `GIT_ASKPASS`),
  `HTTP_PROXY`, `CURL_CA_BUNDLE`, `DISPLAY`/`WAYLAND_DISPLAY`/`GNUPGHOME` (open on
  issue #18), `GIT_EDITOR`/`EDITOR` (pinning `GIT_EDITOR` "due with the first verb
  that can open an editor"). Also on neither list, so never set: `GIT_OPTIONAL_LOCKS`,
  `GIT_PAGER`, `PAGER`, `GIT_EXTERNAL_DIFF`, `GIT_NO_LAZY_FETCH`. Locale is
  **inherited on purpose** (git's stderr is shown verbatim, in the user's language);
  nothing pins `LC_ALL=C`.
- Tests (`environment.rs:395-669`): the whole name set spelled out
  (`the_environment_is_exactly_the_deliberate_entries`); helper named with or
  without a channel; the parent is asked about the roster and nothing else, with a
  poison list; `ALWAYS` beats the parent; an absent inherited variable is left out,
  not set empty; the token is set on the `Command` only when given.

### 1.5 `askpass.rs` — `Askpass`

`#[derive(Debug, Clone, PartialEq, Eq)] pub struct Askpass { program: PathBuf,
socket: Option<PathBuf> }` with `pub fn new(program: impl Into<PathBuf>, socket:
Option<PathBuf>)`, `program()`, `socket()`. A plain value; no I/O.

### 1.6 `cli.rs` — the runner (everything `pub(crate)`)

```rust
#[derive(Debug)]
pub(crate) struct GitCommand<'a> { program: &'a Path, environment: &'a GitEnvironment,
    arguments: Vec<OsString>, directory: Option<PathBuf>, token: Option<AskpassToken> }
impl<'a> GitCommand<'a> {
    pub(crate) fn new(program: &'a Path, environment: &'a GitEnvironment) -> Self;
    pub(crate) fn arg(self, argument: impl AsRef<OsStr>) -> Self;
    pub(crate) fn args(self, arguments: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Self;
    pub(crate) fn in_repository(self, repo: &Repository) -> Self;   // workdir, else git_dir
    pub(crate) fn authorized_by(self, token: &AskpassToken) -> Self; // clones the token
    pub(crate) fn run(self) -> Result<Output, Error>;
    pub(crate) fn stream(self) -> Result<Running, Error>;
}
#[derive(Debug)] pub(crate) struct Running { process: Arc<Mutex<Process>>,
    cancelled: Arc<AtomicBool>, stderr: Option<ChildStderr>, arguments: String }
impl Running {
    pub(crate) fn killer(&self) -> ProcessKill;
    #[cfg(test)] pub(crate) fn id(&self) -> u32;
    pub(crate) fn finish(self, progress: impl FnMut(&str)) -> Result<Output, Error>;
    fn reap(&mut self, already: Option<ExitStatus>, progress: &mut impl FnMut(&str), everything: String) -> Result<Output, Error>;
}
#[derive(Debug, Clone)] pub(crate) struct ProcessKill { process: Arc<Mutex<Process>>, cancelled: Arc<AtomicBool> }
impl ProcessKill { pub(crate) fn kill(&self); }
pub(crate) const TERMINATION_GRACE: Duration = Duration::from_secs(2);
const EXIT_POLL: Duration = Duration::from_millis(20);
const STDERR_CHUNK: usize = 4096;
#[derive(Debug, Clone, PartialEq, Eq)] pub(crate) struct Output { stdout: Vec<u8>, stderr: String }
impl Output { stdout(&self) -> &[u8]; stderr(&self) -> &str; stdout_text(&self) -> Cow<'_, str>;
              records(&self) -> impl Iterator<Item = &[u8]>; }   // all pub(crate)
```

**`run()`** (`cli.rs:85-113`): `environment.command(program, token)`, args, stdin
`Stdio::null()`, stdout and stderr piped, cwd if set, then `command.output()`.
- Blocks until **both pipes reach EOF and the child exits** (std's `output`). Not
  killable, not cancellable, no timeout, no progress, both streams buffered whole
  with no cap. Any `io::Error` from `output()` — spawn or pipe read — becomes
  `GitNotStarted{program, source}`. Non-zero exit → `GitFailed{arguments:
  describe(args), status, stderr: lossy+trim_end}`. Success → `Output{stdout,
  stderr}`.
- Production caller: only `binary.rs::probe`.

**`stream()`** (`cli.rs:120-153`): stdin null, **stdout `Stdio::null()`**, stderr
piped, `spawn()`; takes the stderr pipe (an impossible miss kills/waits the child
and reports `GitNotStarted`); returns `Running` with `Process{child, terminated_at:
None, killed: false}` behind `Arc<Mutex<_>>`, a fresh cancel flag, and the
described arguments.

**`Process`** (`cli.rs:172-235`, private): one lock covers child + cancel progress.
- `terminate()`: once only; refuses if `try_wait` says the child has exited (so a
  reaped pid is never signalled); records `terminated_at`, sends `SIGTERM` to the
  **single pid** via `nix::sys::signal::kill(Pid::from_raw(pid), SIGTERM)`; a pid
  that does not fit `i32` falls back to `child.kill()` (SIGKILL) and sets `killed`.
- `poll(cancelled)`: `try_wait`; if still running and cancelled, sends `SIGTERM`
  if not yet sent, or `child.kill()` (SIGKILL) once `TERMINATION_GRACE` has passed.

**`finish(progress)`** (`cli.rs:272-308`):
- Spawns a detached thread `cairn-git-stderr` running `read_lines` (4 KiB reads,
  lines split on `\n` **or** `\r`, terminator stripped, lossy UTF-8, sent over an
  mpsc channel; a partial last line is sent at EOF). Detached because git's children
  (ssh, remote helper, askpass helper) inherit the pipe and can outlive a killed git.
  A failure to spawn that thread is reported as `GitNotStarted{program: "git"}`
  (mislabelled: the process did start, and is then neither killed nor reaped —
  **the child leaks on that path**).
- Loop: `recv_timeout(EXIT_POLL=20ms)`; each line is appended to `everything`
  (an **unbounded** `String`) and handed to `progress` unless blank after
  `trim_end`. **Only when the cancel flag is set** does it poll the process; a
  non-cancelled `finish` leaves the loop only when the **pipe disconnects**, i.e.
  when every holder of git's stderr has closed it. Consequence (read from code, not
  observed): if git exits but a descendant keeps stderr open, an uncancelled
  `finish` keeps waiting; a cancel then ends it (the poll sees git exited). Which
  descendants git or ssh leave holding stderr after a normal exit (e.g. an ssh
  `ControlPersist` master, a detached auto-maintenance) is **OPEN**.
- Then `reap`: if no status yet, polls `lock().poll(cancelled)` every 20 ms (never
  `wait()`s, so the lock is never held across a block). Outcome ordering: **a clean
  exit wins over a racing cancel** (returns `Ok`, stdout empty); else if cancelled →
  `GitCancelled{arguments, stranded_locks: vec![]}` (the operation fills the locks);
  else `GitFailed{arguments, status, stderr: everything}`.
- `reap` takes `progress` and ignores it (`let _ = progress;`).

**`ProcessKill::kill`** (`cli.rs:428-437`): store the flag (`Release`) first, then
`try_lock` and `terminate()`. Never blocks; a lock miss is covered by the waiter's
poll. This is what makes it legal to call from the UI thread (through
`FetchControl::cancel`).

**`Output`** (`cli.rs:455-503`): `stdout()`, `stderr()` and `records()` each carry
`#[cfg_attr(not(test), expect(dead_code, reason = ...))]` — reasons "the first
operation to will be status" (stdout, records) and "a streamed invocation hands its
stderr on line by line instead" (stderr). `records()` strips one trailing NUL and
splits on NUL; an empty body yields no records. Only `stdout_text()` is used in
production (the version probe). There is no `-z` parser for any git format
(`--raw`, porcelain v2) in production.

**What the runner does not have** (verified by absence): no `Drop` for `Running`
or `FetchInProgress` (dropped without `finish`, the child is neither killed nor
reaped — `std::process::Child`'s drop does neither); no process group or session
(`Command::process_group` is never called, so git shares Cairn's group and a signal
reaches git alone, never ssh/remote helpers/askpass); no timeout or watchdog; no
stdin writer; no mode that both captures stdout and is killable; no output size
cap or back-pressure; no registry of running processes; no logging, timing or
metrics; no `cfg(unix)` gate on the `nix` import (`cli.rs:29-30`).

**Unit tests** (`cli.rs:505-542`, `#[cfg(test)]`): NUL records without a phantom
last one; empty output has none; a record without its final NUL still counts;
arguments described as typed.

**Stub tests** (`cli.rs:546-1120`, `#[cfg(all(test, unix))] mod stub_tests`):

| Test | What it pins |
|---|---|
| `the_child_sees_the_built_environment_and_nothing_inherited` | stub `exec /usr/bin/env`: every built variable reaches the child, nothing else does (no `CARGO_*`; `/bin/sh`'s own `PWD OLDPWD SHLVL _` excused), `GIT_TERMINAL_PROMPT=0`, helper named, no token |
| `an_authorised_invocation_carries_its_token_and_only_that_one` | `authorized_by` puts `CAIRN_ASKPASS_TOKEN` in the child |
| `arguments_arrive_in_order_and_nul_records_split` | argv order; `records()` over real output; empty stderr |
| `a_command_runs_in_the_repository_it_is_asked_to` | `in_repository` sets cwd to the workdir; without it, the caller's cwd |
| `a_streamed_invocation_hands_stderr_on_a_redraw_at_a_time` | `\r` and `\n` both end a line; stdout discarded on `stream` |
| `a_streamed_failure_carries_everything_stderr_said` | `GitFailed{arguments,status 128,stderr}` |
| `a_kill_from_another_thread_ends_a_hung_invocation_and_reaps_it` | kill from another thread ends a stub hung in a `sleep` child (which holds the pipe), `GitCancelled` with empty locks, pid gone from `/proc` |
| `a_kill_after_a_clean_exit_reports_the_success` (linux) | kill after a zombie exit → `Ok` |
| `a_kill_that_ends_the_process_is_reported_as_cancelled` | negative twin |
| `a_cancel_sends_sigterm_first_and_a_process_that_exits_on_it_is_not_killed` | trap prints "terminated" on TERM; ends inside the grace; reaped |
| `a_process_that_ignores_sigterm_is_killed_once_the_grace_period_has_passed` | escalation to SIGKILL after ≥ 2 s, under a 7 s deadline; reaped |
| `a_cancel_that_lands_after_stderr_closed_is_still_escalated_to_sigkill` | escalation happens from the reap loop too |
| `a_cancel_after_the_reap_signals_nothing` | `terminated_at` stays `None` after a post-reap kill |
| `a_kill_that_misses_the_lock_is_finished_by_the_waiter` | lock held across `kill()`; the waiter sends SIGTERM |
| `standard_input_is_closed_not_inherited` (linux) | `readlink /proc/$$/fd/0` = `/dev/null` |

**How the stub works.** `ops/stub_git.rs` (`#[cfg(all(test, unix))]`):
`StubGit::with_git(script)` writes `#!/bin/sh\n<script>\n` as `git`, mode 0755,
into `$TMPDIR/cairn-git-runner-stub-<pid>-<n>` (removed on drop);
`environment_with(parent)` builds a real `GitEnvironment` whose `PATH` is that one
directory and whose askpass is `Askpass::new("/nonexistent/cairn-askpass", None)`;
`discover_retrying(env)` retries `GitBinary::discover_with` up to 50 times on
`ETXTBSY` (26), because a parallel test's fork can hold the stub's write fd for
microseconds. The stubs in `cli.rs` answer `--version` with `git version 2.30.0`
then run the test's shell; they set `PATH=/usr/bin:/bin` themselves when they need
`sleep`, and exit 99 before speaking if `sleep` is missing so a kill never fires on
a stub that died alone. A second, public-API-only copy of the helper lives in
`crates/cairn-git/tests/git_binary.rs` (with its own `ETXTBSY` constant), because
the runner is crate-private. The `/proc` checks make several tests Linux-only.

### 1.7 `fetch.rs`

```rust
pub fn fetch(git: &GitBinary, repo: &Repository, remote: &str, token: Option<&AskpassToken>)
    -> Result<FetchInProgress, Error>;
const ARGUMENTS: [&str; 4] = ["fetch", "--progress", "--no-prune-tags", "--end-of-options"];
#[derive(Debug)] pub struct FetchInProgress { running: Running, remote: String, git_dir: PathBuf, common_dir: PathBuf }
impl FetchInProgress { pub fn canceller(&self) -> FetchCancel;
                       pub fn finish(self, progress: impl FnMut(&str)) -> Result<Performed, Error>; }
#[derive(Debug, Clone)] pub struct FetchCancel(ProcessKill);
impl FetchCancel { pub fn cancel(&self); }
```
- `fetch` runs `refspec_policy::check(repo.git_dir(), remote)` first (no process),
  then `git.command().in_repository(repo).args(ARGUMENTS).arg(remote)`
  [`.authorized_by(token)`]`.stream()`. Not destructive; takes no `Confirmed`
  (module docs: prune honoured as git does, tags never pruned, mirror/local-branch
  refspecs refused).
- `finish` maps `Ok` → `Performed::new("fetched <remote>", refs ∪ objects)`,
  `GitCancelled` → same error with `stranded_locks(git_dir, common_dir)` filled
  **after the reap**, anything else unchanged.
- In-crate stub test pins the exact argv (`fetch --progress --no-prune-tags
  --end-of-options -origin`). End-to-end behaviour is in `crates/cairn-git/tests/fetch.rs`
  (real git, real HTTP and SSH fixtures, a real askpass channel): prompting per half
  of an HTTP credential, a helper that answers means no prompt, refusals ask nothing
  again, **cancel while waiting on a prompt kills git and leaves no process pointed
  at the socket** (`cancelling_a_fetch_that_is_waiting_on_a_prompt_...`), a cancel
  names planted locks, `ref_tips` follow git's writes, the prune matrix, refspec
  refusals before git runs (a recording stub), config from Cairn's own environment
  ignored, SSH passphrase / unknown host key / agent / refusal.

### 1.8 `stranded_locks.rs`

`pub(crate) fn stranded_locks(git_dir: &Path, common_dir: &Path) -> Vec<PathBuf>`:
read-only, never fails; top-level `*.lock` in each distinct directory, a full
iterative walk of `refs/`, and the top level of `objects/pack`, `objects/info`,
`objects/info/commit-graphs`; sorted, deduped. Cannot tell a stranded lock from one
a live git holds. Three tests: what is and is not a lock; linked worktree searched
in both dirs; clean repo and missing dir are empty.

### 1.9 `refspec_policy.rs`

`pub(crate) fn check(git_dir: &Path, remote: &str) -> Result<(), Error>` opens a
**fresh** gix repository (`gix::open_opts`) with `as_the_child_reads()`
permissions — `Permissions::secure()` with `env.git_prefix = Deny` and
`config.env = false` — so Cairn's own `GIT_CONFIG_*` cannot sway it. Refuses mirror,
any destination under `refs/heads/`, and `refs/tags/` under prune. Stated residual
(`refspec_policy.rs:31-36`): gix reads `/etc/gitconfig` while git reads its own
`$(sysconfdir)/gitconfig`; "asking git where its file is would spawn a process
outside `GitEnvironment`". Verified in vendored gix 0.87.1: `Config::all()` sets
`git_binary: false` (`gix-0.87.1/src/open/permissions.rs:37-46`), so neither this
open nor `Repository::discover` makes gix-path run `git config -lz --show-origin`
(`gix-path-0.12.6/src/env/git/mod.rs:114-176`, which would spawn with an inherited,
merely trimmed environment). **No gix code path spawns `git` in Cairn today** as far
as the open permissions go; gix's filter-driver spawns (D1 amendment) are on the
diff-engine branch only.

### 1.10 `error.rs` — process variants, and `cancel.rs`

`pub enum Error` (`thiserror`), process-related variants:
- `GitNotFound { searched: Vec<PathBuf>, required: GitVersion }`
- `GitTooOld { path, found: GitVersion, required: GitVersion }`
- `GitVersionUnreadable { path, output: String, required }`
- `GitNotStarted { program: PathBuf, #[source] source: std::io::Error }` — also
  used for pipe-read errors in `run`, a failed reader-thread spawn and a `try_wait`
  error, all labelled `program: "git"` in the latter two.
- `GitFailed { arguments: String, status: ExitStatus, stderr: String }` — Display
  `git {arguments} failed ({status}): {stderr}`. `arguments` is the lossy argv
  joined by spaces; it is rendered to the user verbatim (a URL with userinfo given
  as `remote` would be shown — a judgement for any new verb, **OPEN** whether it
  matters for fetch, which the window calls with a configured name).
- `GitCancelled { arguments: String, stranded_locks: Vec<PathBuf> }` — Display adds
  the lock sentence only when non-empty.
- Fetch-specific: `FetchRefused{remote, setting, write: RefusedWrite}`,
  `RemoteConfig{remote, source}`, `Refs{source}`.
- Query cancel is a separate vocabulary: `Cancelled { walked: usize }` (history);
  on diff-engine also `ChangesCancelled { changed: usize }`.

`cancel.rs`: `pub trait Cancel { fn is_cancelled(&self) -> bool; }` (doc: "called
once per commit visited"; no `Send`/`Sync` bound) and `#[derive(Debug, Clone,
Default)] pub struct CancelSignal(Arc<AtomicBool>)` with `new()`, `cancel()`,
`impl Cancel`. **Pull-based, polled by the engine; nothing connects it to
`ProcessKill`**, which is push-based.

## 2. `crates/cairn-app/src/worker/`

### 2.1 Files

`mod.rs` (re-exports `PromptId`, `Reply`, `Replier`, `open`, `Request`, `Update`),
`pool.rs`, `operations.rs`, `startup.rs`, `askpass.rs`, `epoch.rs`, `request.rs`,
`wake.rs`, and `fetch_tests.rs` (`#[cfg(test)] mod fetch_tests;`). The survey's
"`pool.rs, operations.rs, startup.rs, epoch.rs, request.rs`" omits `askpass.rs`,
`wake.rs` and `fetch_tests.rs`.

### 2.2 Thread layout (per open repository)

| Thread | Started by | Owns | Blocks on |
|---|---|---|---|
| UI (Freya) | — | `RepositoryHandle` (clones), `Updates`, `Replier` | nothing; `Updates::next` is `try_recv` + `Woken` future |
| `cairn-repository` | `open_with` (`pool.rs:88-139`) | `Arc<SharedRepository>`, its thread-local `Repository`, the open `HistorySession` and `HistoryCursor`, the jobs `Receiver`, `Threads` | `jobs.recv()`; history pages |
| `cairn-operations` | `Threads::start` (`pool.rs:215-237`) | **the only `GitBinary`**, its own `Repository` (`shared.to_worker()`), an `Arc<Channel>` | `operations.recv()`; `FetchInProgress::finish` |
| `cairn-askpass` | `Threads::start` (`pool.rs:189-213`), only when a channel opened | `Arc<Channel>`, the answers `Receiver<Reply>` | `channel.accept()`; `answers.recv()` (the user) |
| `cairn-git-stderr` | `Running::finish`, one per streamed invocation | the `ChildStderr` | pipe read; detached, ends at pipe EOF |

`WORKERS_PER_REPOSITORY = 1` with a compile-time assert (`pool.rs:32-39`). Every
thread is detached; nothing is ever joined.

### 2.3 Startup and who owns `GitBinary`

`open(path)` → `open_with(path, Startup::of_this_process())` returns at once with
`(RepositoryHandle, Updates, Replier)`. On `cairn-repository`, in order:
1. `Backend::open(&startup)` (`startup.rs:71-104`): `Channel::open($XDG_RUNTIME_DIR)`
   (or a named reason), check the helper file exists beside the executable,
   `prompting: Result<(), String>`, build `Askpass(helper, socket?)`, then
   **`GitBinary::discover_with(GitEnvironment::new(&startup.parent, &askpass))`** —
   the `git --version` probe runs here, off the UI thread. A missing/old git →
   `Update::Failed` and the worker returns: **no history is served either**.
2. `SharedRepository::discover(path)`.
3. `Threads::start(...)` destructures `Backend { git, channel, prompting }` and
   **moves `git` into the `cairn-operations` closure** (`pool.rs:183-232`). Nothing
   else holds a `GitBinary`; the repository thread does not.

Discovery therefore happens once per `open`, which is once per application run
today (one repository, from the command line).

### 2.4 Requests, epochs and the result path

- `Request` (`request.rs:25-37`): `OpenHistory{rows}`, `MoreHistory{rows}`,
  `ListRemotes`, `Fetch{remote}`, `CancelFetch`. `is_query()` is true for the two
  history requests only. Test: `queries_are_numbered_and_operations_are_not`.
- `Update` (`request.rs:53-91`): `Rows`, `Failed`, `WorkerLost`, `Remotes`,
  `FetchStarted`, `FetchProgress{line}`, `FetchFinished{remote, refreshed}`,
  `FetchCancelled{remote, refreshed, stranded_locks}`, `FetchFailed{remote,
  refreshed, message}`, `Prompt{id, text}`.
- `Epochs` (`epoch.rs:131-170`): one `AtomicU64` counter **for all queries** (not
  per lane) plus a `stopping` flag; `bump`, `current`, `is_current`, `stop`,
  `watch(mine) -> Superseded`. `Superseded` (`epoch.rs:174-184`, `Debug` only, not
  `Clone`, but `Send + Sync` by construction) implements `cairn_git::Cancel`:
  cancelled when stopping or the counter moved. Nothing ties an epoch to a process.
- `RepositoryHandle::submit` (`pool.rs:293-307`, runs on the UI thread): `CancelFetch`
  short-circuits to `control.cancel()` and is never queued; a query bumps the epoch;
  an operation takes the current epoch but is sent unnumbered (`None`).
- `serve` (`pool.rs:454-555`): drops a superseded numbered job before starting it;
  `OpenHistory` drops session and cursor (which is also how `Invalidated::refs` is
  honoured after a fetch); `ListRemotes` answers inline; `Fetch` →
  `threads.perform(Operation::Fetch{remote})`; pages via
  `session.next_page(rows, &epochs.watch(epoch))`.
- Results return as `Envelope{epoch: Option<Epoch>, update}` over an mpsc channel,
  one `Outbox` (not `Clone`) per thread; `Outbox::send` then `Wake::signal()`
  (`wake.rs`: a latched one-slot waker). `Updates::next` (`pool.rs:328-347`)
  `try_recv`s, returns epochless updates always and numbered ones only if current,
  and awaits `Woken` when empty; `None` once every sender is gone. The UI applies
  each update in `crates/cairn-app/src/session.rs::apply`.
- `WorkerExit` (`pool.rs:427-452`) sends `WorkerLost` from `Drop` while panicking.

### 2.5 The operations thread and `FetchControl`

- `Operation` (`operations.rs:16-18`) has one variant, `Fetch{remote}`.
- `FetchControl(Arc<Mutex<Stage>>)` (`operations.rs:25-99`), `Stage`: `Idle`,
  `CancelledBeforeStarting`, `Starting{cancelled}`, `Running(FetchCancel)`.
  `arm()` (repository thread, in `Threads::perform`) claims the slot or returns
  `false` — a second fetch is **silently dropped**, no update sent. `cancel()` (UI
  thread or `Threads::drop`) kills a running fetch or remembers the cancel; a cancel
  that overtook its `Fetch` in the queue is claimed by the next `arm` only.
  `install(FetchCancel)` applies a pending cancel; `clear()` resets to `Idle`.
  Tests: `a_cancel_while_starting_is_kept_and_one_fetch_at_a_time_is_armed`,
  `a_cancel_that_overtakes_the_fetch_it_answers_is_claimed_by_that_fetch_alone`.
- `serve_operations` (`operations.rs:102-160`), per fetch:
  `before = repo.ref_tips().ok()` → `channel.begin()` (an `Operation` token, live
  in the channel's set) → `fetch(git, &repo, &remote, token)` →
  `control.install(started.canceller())` **before** `FetchStarted` is sent →
  `finish(|line| outbox.send(None, FetchProgress{line}))` (one update per stderr
  line, unbounded — issue #25) → `control.clear()` → `drop(authorised)` (token
  retired before the outcome goes out) → `after = ref_tips()`;
  `refreshed = before != after` (or `true` if either read failed) →
  `fetch_outcome(...)` maps `Ok` → `FetchFinished`, `GitCancelled` →
  `FetchCancelled{stranded_locks}`, else `FetchFailed{message}` with the "could not
  have asked for a credential" suffix when `prompting` is `Err`. Tests:
  `every_outcome_carries_whether_the_refs_moved`,
  `a_cancelled_fetch_carries_the_lock_files_it_stranded`,
  `a_failure_with_no_way_to_prompt_says_why_nothing_asked`.

### 2.6 How the askpass thread meets an invocation token

`Channel` (`crates/cairn-askpass/src/channel.rs`) keeps a shared set of live tokens.
`Channel::begin()` (operations thread) issues a fresh 32-byte hex token and inserts
it; the returned `cairn_askpass::Operation` removes it on drop. The token reaches
git as `CAIRN_ASKPASS_TOKEN` via `GitCommand::authorized_by` →
`GitEnvironment::command`. The helper (`cairn-askpass/src/main.rs`) needs both
`CAIRN_ASKPASS_SOCKET` and `CAIRN_ASKPASS_TOKEN` or **refuses locally without
connecting** — so an invocation run without a token fails any prompt closed. The
acceptor thread (`worker/askpass.rs::serve_prompts`) `accept()`s; a connection whose
token is not live is refused on the wire (`Error::UnknownToken`, acceptor keeps
going). A valid one becomes `Update::Prompt{id, text}` with an acceptor-local
`PromptId` counter, and the acceptor **blocks on `answers.recv()`** until the window
replies for that id (other ids are discarded). `Prompt::refuse` and an unanswered
`Prompt` drop both retire the token. Consequences for a manager:
- One prompt at a time **across all invocations of the repository**; a second
  token-bearing invocation's helper waits in the listener backlog.
- `PromptId` does not name which invocation asked. The window
  (`session.rs::apply`) shows a prompt only while `fetch.read().is_in_flight()` and
  otherwise refuses it; every fetch ending withdraws and refuses the open dialog. The
  UI is written on the assumption that **the only thing that can prompt is the one
  fetch**.
- `AcceptorStop::stop` (`askpass.rs:82-94`) connects-and-hangs-up repeatedly for up
  to `STOP_DEADLINE = 1 s` to wake a blocking `accept`; it runs on the repository
  thread inside `Threads::drop`.

### 2.7 Shutdown

- `Updates` drop → `epochs.stop()` (stops a walk at its next poll; `serve` itself
  only notices on its next job).
- All `RepositoryHandle` clones dropped → jobs channel disconnects → `serve` returns
  → `threads.stop()` → `Threads::drop` (`pool.rs:264-276`, also runs on unwind):
  `control.cancel()` (kills a running fetch: SIGTERM, and the operations thread's
  `finish` escalates and reaps), `operations = None` (the operations thread exits
  after its current operation), `acceptor.stop()`.
- `main.rs` holds the handle in a `use_hook` and has no explicit shutdown. Whether
  Freya drops hook state (and so runs the above) before the process exits on window
  close is **OPEN**. If the process exits first, detached threads die with it and a
  running `git` (a different process, same process group) is **not** killed and is
  reparented — an orphaned fetch. No `PR_SET_PDEATHSIG` is possible without
  `pre_exec`, which is `unsafe` and forbidden.

## 3. Guards that touch processes

All in `crates/cairn-guards/tests/invariants.rs` with matchers in
`crates/cairn-guards/src/lib.rs`. `PRODUCT_SOURCE_DIRS` = `src/` of cairn-model,
cairn-git, cairn-ui, cairn-app, cairn-askpass (closed against the workspace by
`every_product_crate_is_on_the_product_roster`). `tests/` directories are not
scanned by the process guards.

### 3.1 `every_git_invocation_disables_the_terminal_prompt` (`invariants.rs:667-846`)

Over every product `src/` file, text passed through `code_without_strings` then
`code_without_test_modules`:
- Outside `crates/cairn-git/src/ops/environment.rs`, fails on: the identifier
  `Command` anywhere (`mentions_crate`, word-boundary match, so an alias is caught on
  its `use` line); `Command :: new` however spaced/wrapped
  (`constructs_process_command`); any of `posix_spawn posix_spawnp execv execve
  execvp execvpe execveat fexecve`; a method call `.env( .envs( .env_clear(
  .env_remove(` (`configures_process_environment`); a `GitEnvironment { .. }`
  literal; an `impl` block for `GitEnvironment`.
- Inside `environment.rs`: no spawn spelling; exactly one `Command::new`; both
  `env_clear` and `envs` named; exactly one `GitEnvironment`/`Self` literal; no
  `mut self`/`&mut Self`; the `const ALWAYS` table itself contains
  `("GIT_TERMINAL_PROMPT", "0")` and `("SSH_ASKPASS_REQUIRE", "force")`; production
  code names `"GIT_ASKPASS"`, `"SSH_ASKPASS"`, `SOCKET_VARIABLE`; `TOKEN_VARIABLE`
  and `SOCKET_VARIABLE` each named ≥ 2 times; `ALWAYS` named ≥ 2 times.
- `SPAWN_SPELLINGS` is cross-checked against `SPAWN_SPELLINGS_AS_WRITTEN`.
- Self-test: `the_process_environment_matcher_catches_the_shapes_it_claims`.

What it does **not** catch:
- **Running** a process. `.spawn()`, `.output()`, `.status()`, `Stdio`, `Child`,
  `process_group`, `current_dir` are unguarded. Any `cairn-git` file can write
  `git.environment().command(git.path(), None).arg("x").output()` — no `Command`
  identifier, no `::new`, no `.env*(` — and bypass the runner's stdin-null, kill and
  error mapping while keeping the built environment. So "the runner is the only place
  a process is run" (`cli.rs:3`) is a doc claim, not a guarded fact.
- `Command` reached through a `type` alias declared in a dependency, a wrapper crate,
  or a macro (stated as `qa-checklist` item 7's residual).
- Process construction inside dependencies (gix filter drivers, gix-path), by design.
- `code_without_test_modules` blanks only blocks after the exact marker
  `#[cfg(test)]`. A module behind `#[cfg(all(test, unix))]` (`cli.rs` `stub_tests`)
  and a test module in its own file (`worker/fetch_tests.rs`, `ops/stub_git.rs`) are
  scanned as production — the reason `docs/systems/credentials.md` says tests in
  `cairn-app/src` may not name `Command`. Errs toward catching.

### 3.2 `only_the_ops_module_mutates_a_repository` (`invariants.rs:596-622`)

For every product `src/` file not under `crates/cairn-git/src/ops`, fails if
`spawns_git` hits. `spawns_git` (`lib.rs:1304-1316`), over `code_only` (comments
blanked, **strings kept**): a line containing the literal `"git"` or
`"/usr/bin/git"` **and** one of `Command`, `new(`, `cmd(`. Self-tests
(`lib.rs:1587-1602`): four constructor shapes caught; prose and an unrelated
string ignored. Asserts it scanned > 0 files.

- **The survey's guard gap is confirmed, and is wider than stated.** In any
  non-`ops` module of `cairn-git` (`history/`, `diff/`, `remotes.rs`, ...),
  `git.command().args(["diff-tree", "-r", "-M", "-z"]).run()` contains no `"git"`
  literal and so passes; so does `GitCommand::new(path, env)` (it has `new(` but no
  literal), `.stream()` with a `ProcessKill`, and the `environment().command(..)`
  route in 3.1. All four are reachable because `GitBinary::command`, `GitCommand`
  and `GitEnvironment::command` are `pub(crate)`. No guard names `GitCommand`,
  `GitBinary`, `.command()`, `Running`, `ProcessKill` or `fetch`. The only
  precondition is a `&GitBinary` in scope, which a `pub fn` taking one provides.
- **The gitoxide half of this invariant has no twin.** CLAUDE.md states "Only
  `cairn-git/src/ops/` mutates a repository, whether through gitoxide or a `git`
  subprocess. Twin: `only_the_ops_module_mutates_a_repository`", and
  `ops/mod.rs:3-5` says a mutating gitoxide call elsewhere "fails the gate". The
  test body calls only `spawns_git`; no matcher looks for gix mutation calls
  (`edit_reference`, `commit`, index writes, ...). Not in the survey.
- The guard is a literal-and-line heuristic: `Command::new(GIT)` with
  `const GIT: &str = "git"` is caught only by 3.1, not here.

### 3.3 `the_ui_thread_never_waits_on_repository_work` (`invariants.rs:983-1049`)

Over `RENDER_SOURCE_DIRS` (`cairn-ui/src`, `cairn-app/src`): files under
`crates/cairn-app/src/worker` may not name `freya`, `dioxus`, `cairn_ui`; every other
file may not match `waits_on_work` (identifiers `Barrier Condvar JoinHandle Mutex
Receiver RwLock bounded channel sync_channel unbounded block_on blocking_lock
blocking_recv blocking_send park park_timeout recv_deadline recv_timeout scope
select sleep wait_timeout wait_while spin_loop try_iter try_lock try_recv yield_now`,
and nullary `join() lock() recv() wait()`), and `cairn-app` render files may not
name `cairn_git`, `gix`, `cairn_askpass`. Nonzero file counts asserted per side. It
is FILE-scoped: any process code in `cairn-app` must live in `worker/`, and which
thread a `worker/` function runs on (`submit`, `Updates::next`, `Wake::poll`,
`FetchControl::cancel` via `submit`) is `responsiveness-reviewer`'s judgement.

### 3.4 Others in reach

- `layer_dependencies_are_allowlisted` (`invariants.rs:187-247`): `cairn-git` may
  depend on `cairn-model gix nix thiserror` (row comment: "`nix`: SIGTERM on cancel,
  so git can remove its lock files (issue #19)"); dev-only `cairn-askpass`
  (`TEST_ONLY_ALLOWLIST`). `cairn-app`: `cairn-askpass cairn-git cairn-model
  cairn-ui freya`; dev-only `freya-testing`. **Names only — features are not
  checked**, so enabling another `nix` feature passes this guard and `cargo deny`
  (`DEPS_CMD="cargo deny check advisories bans sources licenses"`); it is a policy
  decision only by CLAUDE.md's dependency rule.
- `layers_never_name_the_crates_they_are_sealed_from`: `cairn-git` never names
  `freya dioxus cairn_ui`; `cairn-askpass` never names `gix freya dioxus cairn_git
  cairn_ui tracing log`. A logging crate in `cairn-git` is not forbidden by this
  guard but is not on its allowlist either.
- `destructive_operations_are_sealed_behind_the_confirmation_token`: `confirm.rs`
  keeps a private `acknowledged: String` and exactly two `pub fn`s; some file under
  `ops/` contains `Confirmed` — satisfied today by `describe_destructive` alone.
- `no_credential_value_is_logged_printed_serialised_or_stored`: `SECRET_READERS`
  and the empty `SECRET_HOLDERS` roster bind any new code that touches a `Secret`
  (e.g. a stdin writer that carries one would need a roster row).
- `the_ssh_criteria_are_required_wherever_they_can_run`: CI sets
  `CAIRN_REQUIRE_SSH_FIXTURE: 1` and installs `openssh-server`; `gate.sh`'s
  `run_test_full` probes every `SBIN` dir the fixture uses — relevant to any process
  test that needs sshd.
- `.claude/hooks/qa-stop.sh` (debris hook) has no process rule; it echoes the crate
  seals only.
- Review obligations on record: `qa-checklist` item 7 (a `Command` the guard cannot
  read); `destructive-ops-reviewer` check 9 (the `INHERITED` roster is right) and
  its scope rule (any diff under `ops/`, or `environment.rs`).

## 4. Dependencies and lints

- `nix = { version = "0.31.3", default-features = false, features = ["signal",
  "process"] }` (workspace `Cargo.toml:34`, `Cargo.lock` 0.31.3). Comment: SIGTERM
  from safe code; `process` also compiles `posix_spawn`/`exec*`, which the guard
  forbids by spelling. In nix 0.31.3 `signal = ["process"]`, so `process` is
  implied anyway. `nix::sys::signal::killpg` exists under `signal`
  (`nix-0.31.3/src/sys/signal.rs:1113`) — a process-group kill needs **no new
  feature**. `nix::poll` needs the `poll` feature (not enabled).
- std offers `std::os::unix::process::CommandExt::process_group` (safe, stable since
  1.64; toolchain `1.97.1`, MSRV `1.90`), so a new process group needs no `unsafe`
  and no dependency; `setsid`/`pre_exec` would need `unsafe`, which is forbidden.
  Putting git in its own group also takes it out of the terminal's foreground group
  (terminal Ctrl-C no longer reaches it; a tty read would get `SIGTTIN`) — a design
  consequence, **OPEN** whether it matters given `SSH_ASKPASS_REQUIRE=force`.
- `gix 0.87.1` features `blame blob-diff revision status max-performance parallel
  sha256` (default features on); `thiserror = "2"`; `zeroize 1.9` (`alloc`).
- Workspace lints: `[workspace.lints.rust] unsafe_code = "forbid",
  missing_debug_implementations = "warn"`; `[workspace.lints.clippy] all = deny
  (priority -1), dbg_macro, todo, unimplemented, unwrap_used, expect_used,
  print_stdout = deny`. `clippy.toml`: `allow-unwrap-in-tests = true`,
  `allow-expect-in-tests = true`. Profiles: dev `opt-level = 1`, dependencies 3.
- No logging crate anywhere in the product; adding one to `cairn-git` is an
  allowlist row and a user decision.

## 5. The diff engine (`feature/diff-engine` at 44fa6a8)

### 5.1 The changes query as built

`crates/cairn-git/src/diff.rs` re-exported from `lib.rs`: `ChangeSet`,
`ChangesRequest`, `ContentOptions`, `DiffSession`, `RenameDetection`.

```rust
pub struct ChangesRequest { subject: Subject }    // Subject::Commit(Oid) | Subject::Between{old,new}
impl ChangesRequest { pub fn commit(id: Oid) -> Self; pub fn between(old: Oid, new: Oid) -> Self; }
pub struct RenameDetection { pub enabled: bool, pub copies: bool, pub limit: usize,
    pub similarity_checks: usize, pub renames_skipped_for_limit: usize, pub copies_skipped_for_limit: usize }
impl RenameDetection { pub fn was_cut_short(self) -> bool; }
pub struct ChangeSet { pub files: Vec<ChangedFile>, pub details: Option<CommitDetails>, pub renames: RenameDetection }
pub struct DiffSession<'repo> { repo: &'repo Repository, cache: gix::diff::blob::Platform }  // not Send
impl Repository { pub fn diff_session(&self) -> Result<DiffSession<'_>, Error>;
                  pub fn changes(&self, &ChangesRequest, &impl Cancel) -> Result<ChangeSet, Error>;
                  pub fn file_diff(&self, &ChangedFile, &ContentOptions) -> Result<FileDiff, Error>; }
impl DiffSession<'_> { pub fn changes(&mut self, &ChangesRequest, cancel: &impl Cancel) -> Result<ChangeSet, Error>;
                       pub fn file_diff(&mut self, &ChangedFile, &ContentOptions) -> Result<FileDiff, Error>; }
```
`diff/changes.rs::changes` resolves trees (a commit against its **first parent**, a
root commit against `inner.empty_tree()`; `Between` is tip against tip), runs
`old_tree.changes()` (gix reads `diff.renames`/`diff.renameLimit`) and
`for_each_to_obtain_tree_with_cache`, polling `cancel.is_cancelled()` **once per
change** → `Error::ChangesCancelled{changed: files.len()}`; maps each gix `Change`
to `cairn_model::ChangedFile{status, old_path, new_path, old_mode, new_mode,
old_id, new_id}` (directories skipped; `T` only on a change of kind; similarity
truncated, identity match = 100); `repair_copies` puts a copy's source back as git
reports it; sorts by `(new_path, old_path)`; fills `RenameDetection` from gix's
outcome. Errors: `TreeDiff`, `ReadCommit`, `DiffSetup` (new on the branch), and
`ChangesCancelled`. `ChangedFile` has no line counts (L10). No worker wiring exists
on the branch (`crates/cairn-app/src/worker/` is unchanged; the diff thread is phase
04, unbuilt).

### 5.2 What a `git diff-tree` replacement would need from a runner

Decision E (spike, "What was decided from it"): the changes query — paths, statuses,
modes, ids, rename and copy pairs — comes from `git diff-tree -M` **always**; gix
keeps content diffs, history and the model. Recorded in
`docs/work/diff-engine/state.md` and the spike; **not yet** in
`docs/design/engine.md`, whose D1 still says "The `git` CLI never runs on a read
path". The requirements below are read from the code; the git-side facts marked
OPEN were not checked against git's source or docs here.

1. **A `GitBinary` where the query runs.** `Repository`/`DiffSession` hold none and
   `changes()` takes none, so the public signature changes (a `&GitBinary`
   parameter, or a session that holds one). The worker must clone the binary into
   the diff thread (cheap; `GitBinary: Clone`); today it is moved into
   `cairn-operations` alone.
2. **Captured stdout, killable.** Neither runner path does both: `run()` captures
   but cannot be cancelled and waits for pipe EOF; `stream()` is killable but sends
   stdout to `/dev/null`. Capturing stdout and stderr while staying killable needs
   two concurrent readers (two threads, or `nix` `poll` — a feature addition) since
   a blocking read cannot also watch a cancel.
3. **Cancel by epoch.** A changes query is a query: it carries an epoch and is
   superseded per lane (D3, phase 04's L8). `&impl Cancel` is pull-based; a process
   needs a push. A bridge — poll `cancel.is_cancelled()` in the wait loop (the
   existing 20 ms `EXIT_POLL` loop is the natural place) and call
   `ProcessKill::kill` — is required, and it must map to the **query** vocabulary
   (`ChangesCancelled`), not `GitCancelled{stranded_locks}`; `stranded_locks` is
   meaningless for a read. `Superseded` is `Send + Sync` but not `Clone`
   (`Epochs::watch` can mint another), and `Cancel` has no `Send` bound, so a
   watcher thread would need a bound or a second `watch`.
4. **`-z` records.** `Output::records()` splits on NUL, but `--raw -z` records are
   variable-length: `:<old mode> <new mode> <old id> <new id> <status>\0<path>\0`,
   with a second path for `R`/`C` (status carries the score, e.g. `R086`). A parser
   reading status before deciding how many path fields follow is needed; none exists.
   **OPEN**: exact format checked against git-diff-tree(1) for the pinned floor 2.30.
5. **Arguments.** The spike measured `git diff-tree -r -M -z --raw --no-abbrev`.
   Facts the design must settle, **OPEN** (not verified here): a merge given as a
   single commit prints nothing without `-m`/`--first-parent`, so first-parent
   semantics want the two-tree form `<parent> <commit>`; a root commit needs
   `--root` (measured-baseline: "`git diff-tree <c>` prints nothing for it without
   `--root`") — and the empty tree's id differs under SHA-256, which Cairn supports;
   a single-commit form prints a commit-id header line unless `--no-commit-id`;
   whether plumbing `diff-tree` honours `diff.renames` (copies) and
   `diff.renameLimit` from config, or whether Cairn must read them and pass
   `-C`/`-l<n>`, so the answer matches `git diff`/`git show` and
   `RenameDetection` stays truthful. `--end-of-options` before revisions, as fetch
   does. `--no-ext-diff` is irrelevant to `--raw` but harmless.
6. **Environment.** Through `GitEnvironment` as built (no token: the helper fails
   closed). `GIT_OPTIONAL_LOCKS=0` matters for `git status` (measured-baseline), not
   for `diff-tree`, which does not touch the index. **OPEN**: in a partial clone,
   rename detection may lazily fetch missing blobs from the promisor remote — a
   network access, possibly a credential prompt (failing closed without a token);
   whether to forbid it (`GIT_NO_LAZY_FETCH`, git version unverified) is a decision.
7. **Errors.** Bad object / unknown revision arrive as `GitFailed{status, stderr}`;
   stderr is never matched on (output policy), so a caller cannot distinguish
   "missing object" from other failures except by status code. A missing `git` is
   already refused at startup, so it cannot first appear here.
8. **Output size.** The spike's largest case, S1, is 27,592 records and 5.5 MB of
   stdout; `run()` would buffer it (no cap). Nothing bounds a pathological answer.
9. **Placement.** `ops/` is chartered as "Repository mutations" (`mod.rs:1`) and
   D1 says the CLI never runs on a read path; a read-path invocation amends both,
   and the guards cannot tell a read from a write.

### 5.3 Measured cost (rename-parity spike, warm, rust-lang/rust, Ryzen 7 9800X3D, git 2.55.0)

`git diff-tree -r -M -z --raw --no-abbrev`, spawned with `std::process::Command`
(not Cairn's runner) and an environment of `LC_ALL=C GIT_OPTIONAL_LOCKS=0
GIT_TERMINAL_PROMPT=0 PATH HOME`:

| Subject | git spawn→output ms | git + parse ms | gix query ms |
|---|---|---|---|
| M1 `5a3292f163d` | 82.7 | 83.3 | 2.29 |
| S1 `cf2dff2b1e3` (27,592 records, 5.5 MB) | 32.8 | 37.5 | 20.04 |
| S7 `f0845adb0c1` | 7.8 | 8.0 | 1.04 |
| S3 `3fc7ab23731` | 12.1 | 13.2 | 6.14 |
| S5 `9be35f82c1a` | 8.9 | 9.5 | 3.01 |
| S6 `9e5f7d5631b` | 7.4 | 7.8 | 1.46 |
| `7d52cbce6db` | 32.5 | 33.0 | 2.86 |
| `c798dffac9d` | 31.5 | 32.1 | 2.94 |
| `f45f52532a3` | 31.1 | 32.2 | 4.50 |
| `bb55bd449e6` | 27.5 | 27.7 | 0.77 |
| `2f351415e53` | 14.1 | 14.2 | 0.38 |

Parsing 0.1–0.6 ms, 4.6 ms on S1. On the 143 fired first-parent commits git took
p50 9.3 ms, p90 19 ms, max 83 ms. `measured-baseline.md`: a `git diff-tree` process
on this repository costs **2.4 ms before it diffs anything**. Under E the git
column is the per-selection cost: roughly 8–37 ms, 83 ms worst. Unverified per the
spike: cold cache, copies on, Cairn's real environment and runner.

## 6. Rules any new process path must keep

From root `CLAUDE.md`, `docs/design/engine.md` (D1), `docs/design/concurrency.md`
(D3), `docs/design/credentials.md` (D2) and `docs/systems/credentials.md` (L1-L11
and the decisions taken inside the phases):

1. Every `git` runs with a `GitEnvironment` built by its one constructor:
   `GIT_TERMINAL_PROMPT=0`, `SSH_ASKPASS_REQUIRE=force`, `GIT_ASKPASS`/`SSH_ASKPASS`
   = Cairn's helper, inherited environment cleared; only `environment.rs` builds a
   `Command` (L3, L5; invariant + twin). Every roster change is
   `destructive-ops-reviewer` check 9's.
2. Cairn's helper is the askpass even when the user set another (D2, issue #22).
   An invocation that may prompt carries a per-invocation token; the token is
   retired when the invocation ends (before its outcome is reported) or a prompt
   under it is refused. No token → prompts fail closed.
3. No secret on `argv` (L6); no credential value logged, printed, serialised or
   stored (`Secret`; guard rosters). A stdin path that carries one is a
   `SECRET_READERS` question.
4. Writes go through `git`, reads through gix, and "the `git` CLI never runs on a
   read path" (D1) — to be amended for decision E. The one read-path process
   allowed today is the user's clean filter driver started by gix, with Cairn's
   inherited environment and stderr as stated residuals.
5. Only `crates/cairn-git/src/ops/` mutates a repository; destructive operations
   take `Confirmed` by value and record its prompt on `Performed`.
6. Every operation declares `Invalidated`; the worker honours it (D1 obligation 1).
7. Output policy: `-z`/porcelain v2/`--format`; stderr is shown verbatim and never
   matched on; a distinguishable outcome needs its own channel.
8. Errors: `thiserror` variants naming what the caller handles; no dependency error
   type across the seam. No `unwrap`/`expect`/`todo!`/`dbg!` in shipping code; no
   `unsafe` anywhere (so no `pre_exec`, `fork`, `setsid` via libc).
9. The UI thread never waits: process work lives in `crates/cairn-app/src/worker/`;
   anything the UI thread calls there (today `submit` → `FetchControl::cancel` →
   `ProcessKill::kill`) must not block — `kill` only `try_lock`s.
10. D3: queries carry an epoch per lane and the epoch IS the cancel the engine
    polls; operations carry none and are cancelled by killing their process; a
    scroll and a fetch cannot supersede each other. "Any design that assumes a query
    is fast is wrong."
11. A missing or too-old `git` (< 2.30, L9) is refused at startup, loudly, naming
    the version; nothing degrades silently.
12. Cancel is `SIGTERM` first, `SIGKILL` after `TERMINATION_GRACE` (2 s); stranded
    `*.lock` files are reported, never removed (issue #19).
13. Any change under `ops/` or `cairn-guards` needs a test in the same commit;
    every new invariant needs a deterministic twin in the same change, at the
    strongest tier.
14. Tests against real git and real repositories, not mocks; stub-git tests are the
    accepted way to pin runner behaviour; tests in `cairn-app/src` may not name
    `Command`.
15. Dependency or feature additions are user decisions (`nix` features included,
    by policy if not by guard).

## 7. What the survey got wrong, missed, or what has changed

Corrections:
- **Line numbers in `operations.rs` are off**: `Operation` is at
  `operations.rs:16-18` (survey: 30-33), `FetchControl` 25-99 (survey: 39-114),
  `install` 84-90 (survey: 99-105), `serve_operations` 102-160 (survey: 117-175),
  the per-fetch outcome block 146-156 (survey: 132-171). Every other line reference
  checked (`cli.rs`, `binary.rs`, `environment.rs`, `fetch.rs`, `mod.rs`,
  `error.rs`, `cancel.rs`, `pool.rs`, `startup.rs`, `epoch.rs`, guard lines) matches.
- "`stdout()` and `records()` carry `expect(dead_code)`": so does `stderr()`, with
  a different reason.
- "Only production caller [of `run`]: the version probe" — right; the survey's
  "Most at once: one fetch per open repository plus the startup probe" omits the
  detached `cairn-git-stderr` reader, which can outlive its git.
- Survey gap 5 / 12 (`GIT_OPTIONAL_LOCKS=0` needed for a read): needed for
  `git status`, not for `git diff-tree`, which never touches the index.
- "The worker files: pool, operations, startup, epoch, request" (scope list in the
  commission): `askpass.rs`, `wake.rs`, `fetch_tests.rs` also exist.

Confirmed and widened:
- **The guard gap is real**, and also: `GitCommand::new` direct, `.stream()` from
  any module, and `git.environment().command(..).output()` (which skips the runner
  entirely) all pass every guard; no guard reads `.spawn()`/`.output()`, so "the
  runner is the only place a process runs" is unguarded.
- **Not in the survey:** the gitoxide half of "only `ops` mutates" has no twin at
  all, despite CLAUDE.md naming one and `ops/mod.rs:3-5` claiming it.
- **Not in the survey:** an uncancelled `finish` ends on stderr **EOF**, not on
  git's exit, so a descendant holding the pipe holds the fetch open (OPEN whether
  any does in practice); `run()` has the same property with no cancel at all.
- **Not in the survey:** a failed `cairn-git-stderr` thread spawn returns
  `GitNotStarted` while the child keeps running, unreaped.
- **Not in the survey:** the acceptor serves one prompt at a time per repository and
  `PromptId` is not tied to an invocation; the window refuses any prompt while no
  fetch is in flight. A second prompting invocation needs both changed.
- **Not in the survey:** gix does not spawn `git` for its installation config in
  Cairn's opens (`git_binary: false`); the refspec residual stands as stated.
- **Not in the survey:** a process-group kill needs no new dependency or feature
  (`CommandExt::process_group` in std, `killpg` under nix `signal`); death-of-parent
  cleanup (`PR_SET_PDEATHSIG`) is unreachable without `unsafe`.
- **Not in the survey:** the dependency allowlist checks crate names, not features.
- **Not in the survey:** `code_without_test_modules` blanks only `#[cfg(test)]`, so
  `#[cfg(all(test, unix))]` modules and separate test files are scanned as
  production.

Changed since the survey: nothing in code. The survey's "D1 moved into
`docs/design/engine.md`" is the state on `2987a53`; decision E is still not
reflected in `engine.md` or CLAUDE.md on either branch.

## 8. OPEN

- Whether Freya drops hook state (and so `RepositoryHandle`, triggering
  `Threads::drop` and the fetch kill) before process exit on window close.
- Which descendants of `git`/`ssh` keep stderr open past git's exit (ssh
  `ControlPersist` master, detached auto-maintenance) and so stall an uncancelled
  `finish`.
- `git diff-tree` specifics for decision E at the 2.30 floor: `--raw -z` record
  layout, merge/root handling, `--no-commit-id`, and which `diff.*` config plumbing
  honours (renames/copies, renameLimit).
- Lazy fetch in partial clones during rename detection, and `GIT_NO_LAZY_FETCH`'s
  availability.
- Effect of a separate process group on ssh/askpass behaviour when Cairn is launched
  from a terminal.
- Cold-cache and Cairn-runner timings for `diff-tree` (the spike used raw
  `std::process::Command`).
