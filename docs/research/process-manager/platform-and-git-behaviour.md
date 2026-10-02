# Platform and git behaviour — what a git process manager can rely on

Evidence record, saved in full. Commissioned 2026-10-02 by the process-manager packet planning. Verified against Rust 1.97.1, nix 0.31.3, git v2.56.0. Historical: never retro-edited.

## Sources and how each claim was checked

| Source | Exact version read | How |
| --- | --- | --- |
| Rust std | `rustc 1.97.1 (8bab26f4f 2026-07-14)`, the pinned toolchain's own `rust-src` at `$(rustc --print sysroot)/lib/rustlib/src/rust/library/std/src` (already installed — no `rustup component add` was run) | read the source; `#[stable]`/`#[unstable]` attributes quoted as found |
| nix | `0.31.3` (the one `nix` in `Cargo.lock`), `~/.cargo/registry/src/*/nix-0.31.3/` | read the source; `cargo tree -e features -i nix@0.31.3` for the features actually on |
| git | `v2.56.0` (tag `2478544319…`, commit `a018953688f1…`), blobless clone of `github.com/git/git` into the session scratchpad; older tags read with `git show v2.30.0:<path>` | read the source and `Documentation/`; release notes for "since" versions |
| git, behaviour | `git version 2.56.0`, the binary on this machine (Linux 7.2.8, CachyOS) | small Python harnesses, results quoted below |
| XNU (macOS kernel) | `apple-oss-distributions/xnu` `main`, `bsd/sys/pipe.h`, `bsd/kern/sys_pipe.c`, `bsd/sys/syslimits.h`, fetched 2026-10-02 | read the source; not run on a Mac |
| Linux kernel | `torvalds/linux` `master`, `kernel/pid.c`, `include/linux/pid.h`, fetched 2026-10-02; `pipe(7)` from this machine's man pages | read the source; pipe capacity measured |
| OpenSSH | `openssh/openssh-portable` `master`, `ssh.c`, fetched 2026-10-02 | read the source |
| POSIX | IEEE Std 1003.1-2024, XBD §4.17, `pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap04.html` | fetched |

Line numbers below are era artifacts of those exact versions; item names are the
stable anchors.

Cairn context read alongside (not re-verified here, see
`runner-and-worker-as-built.md`): `crates/cairn-git/src/ops/cli.rs` today spawns
without `process_group`, sends `SIGTERM` to the git pid with
`nix::sys::signal::kill`, escalates with `Child::kill` after
`TERMINATION_GRACE`, and always sets stdin to `Stdio::null()`.
`crates/cairn-git/src/ops/environment.rs` inherits a roster that includes
`LANGUAGE`, `LANG`, `LC_ALL`, `LC_CTYPE`, `LC_MESSAGES` but not `TERM`,
`EDITOR`, `VISUAL` or any `GIT_*` variable.

---

## A. Rust std process API on 1.97.1

### A1. `CommandExt::process_group` — stable, safe

- `std/src/os/unix/process.rs`, trait `CommandExt`:
  `#[stable(feature = "process_set_process_group", since = "1.64.0")] fn process_group(&mut self, pgroup: i32) -> &mut process::Command;`
  — a safe method. Doc: "Equivalent to a `setpgid` call in the child process…
  A process group ID of 0 will use the process ID as the PGID."
- Implementation (`std/src/sys/process/unix/unix.rs`): on the `posix_spawn`
  fast path it sets `POSIX_SPAWN_SETPGROUP` + `posix_spawnattr_setpgroup`; on the
  fork/exec path the child calls `setpgid(0, pgroup)` before exec. Setting it does
  NOT force the slow path (the `posix_spawn` bail-out list is
  `uid`/`gid`/`groups`/`chroot`/`pre_exec` closures/`PATH`-in-env with a
  non-path program — not `pgroup`).
- **So `process_group(0)` gives each git invocation its own process group with
  no `unsafe`.** `pre_exec` (`unsafe fn`, stable 1.34) is not needed for it.

### A2. `setsid` — unstable

- Same trait: `#[unstable(feature = "process_setsid", issue = "105376")] fn setsid(&mut self, setsid: bool)`.
  Not usable on stable 1.97.1. Even where it exists, the `posix_spawn` path
  supports it only on `linux`+`gnu` (`POSIX_SPAWN_SETSID`), else falls back to
  fork/exec.
- The only stable routes to a new session for a child are `pre_exec` (unsafe,
  forbidden) or a different spawner (see B6: `nix::spawn` has no `SETSID` flag).
  `nix::unistd::setsid()` is a safe fn but acts on the CALLING process (it could
  only make Cairn itself a session leader, and fails with `EPERM` if Cairn is
  already a process-group leader, which a shell-launched program is).
- **Consequence:** a child in its own process group still shares Cairn's session
  and controlling terminal (if any). See C2.4 (SIGTTIN).

### A3. `std::io::pipe` / `PipeReader` / `PipeWriter` — stable since 1.87.0

- `std/src/io/pipe.rs`: `#[stable(feature = "anonymous_pipe", since = "1.87.0")] pub fn pipe() -> io::Result<(PipeReader, PipeWriter)>`; both have
  `try_clone` (stable 1.87). `std/src/process.rs`: `impl From<io::PipeWriter> for Stdio` and `impl From<io::PipeReader> for Stdio` (stable 1.87);
  `std/src/os/fd/owned.rs`: `From<PipeReader/PipeWriter> for OwnedFd` (1.87).
- Use: merge a child's stdout and stderr into ONE pipe (`stdout(w.try_clone()?)`,
  `stderr(w)`) when interleaving is wanted, or hand the same writer to several
  children. The doc's own example warns that the parent must drop its copy of
  the writer (including the one held inside the `Command`) or `read_to_end`
  never sees EOF.
- Creation is CLOEXEC: `std/src/sys/pipe/unix.rs` uses `pipe2(O_CLOEXEC)` on
  Linux (atomic); on other targets — **including macOS** — `pipe()` followed by
  `set_cloexec()` on each end, which is NOT atomic (see D3).

### A4. `Child::try_wait`, `Child::wait`, `Child::kill`, `Drop`

- `try_wait`: `#[stable(feature = "process_try_wait", since = "1.18.0")]`.
  Non-blocking; "If the child has exited then on Unix the process ID is reaped";
  repeated calls keep returning the status. Unlike `wait`, it does not drop stdin.
- `wait`: closes stdin first, then blocks.
- `kill`: stable 1.0; doc: "This is equivalent to sending a SIGKILL on Unix
  platforms." SIGKILL only — there is no stable std way to send another signal.
  Implementation `sys/process/unix/unix.rs` `Process::send_signal`: returns
  `Ok(())` without signalling if the child was already reaped ("the pid can be
  recycled"), uses a pidfd if one exists (only with the unstable
  `create_pidfd`), else `libc::kill`.
- No `Drop` for `Child` (`process.rs` doc on `struct Child`): an un-waited child
  keeps running and becomes a zombie when it exits.
- Unstable, not usable: `os::unix::process::ChildExt` (`#[unstable(feature = "unix_send_signal", issue = "141975")]` — `send_signal`,
  `send_process_group_signal`) and `kill_process_group`
  (`#[unstable(feature = "unix_kill_process_group", issue = "156537")]`, SIGKILL
  via `killpg`, or `pidfd_send_signal(PIDFD_SIGNAL_PROCESS_GROUP)` on kernels
  ≥ 6.9).
- **Group kill on stable therefore needs `nix::sys::signal::killpg`** (B1).

### A5. `ExitStatusExt`

- `std/src/os/unix/process.rs` trait `ExitStatusExt`: `signal()` stable 1.0;
  `core_dumped()`, `stopped_signal()`, `into_raw()` stable 1.58; `from_raw()`
  stable 1.12. All safe. git that dies of the signal Cairn sent shows
  `signal() == Some(15)` / `Some(9)` (C1 measurements: `returncode` -15 / -9).

### A6. Writing stdin while draining stdout/stderr — the deadlock and the patterns

- `std/src/process.rs` module docs: "If the child process fills its stdout
  buffer, it may end up waiting until the parent reads the stdout, and not be
  able to read stdin in the meantime, causing a deadlock. Writing from another
  thread ensures that stdout is being read at the same time." `Stdio::piped`
  doc: "Writing more than a pipe buffer's worth of input to stdin without also
  reading stdout and stderr at the same time may cause a deadlock."
- std's own answer for reading two pipes: `Child::wait_with_output` /
  `Command::output` call `sys/process/unix/common.rs` `read_output`, which sets
  both pipes `O_NONBLOCK` and loops on `libc::poll` over stdout+stderr in ONE
  thread. So **reading stdout and stderr together needs no extra thread if you
  are happy to buffer everything**: `wait_with_output` drops stdin, then polls.
- What std does NOT do: write stdin concurrently. The documented pattern is
  `child.stdin.take()` → `std::thread::spawn(move || stdin.write_all(..))` →
  `child.wait_with_output()`.
- Minimal deadlock-free patterns, all safe std:
  1. *Buffered, with stdin:* one writer thread for stdin + `wait_with_output`
     (poll-based) on the caller. Two threads total.
  2. *Streaming:* one thread per stream that must be drained (stdout reader,
     stderr reader, stdin writer as needed), the caller waits. Up to three extra
     threads.
  3. *Single thread:* poll stdout/stderr/stdin yourself — needs `nix::poll`
     (feature `poll`, not on today) and `O_NONBLOCK` via `nix::fcntl`
     (feature `fs`, not on today); see B4/B5. A one-shot `read()` after `POLLIN`
     on a blocking pipe does not block, so reads work without `O_NONBLOCK`;
     writes do not — whether a blocking write of up to `PIPE_BUF` after `POLLOUT`
     can still block is **OPEN** (not verified in the kernel source), so set
     `O_NONBLOCK` on stdin or keep the writer on its own thread.
- Cancel with pattern 1 works without touching the reader: killing the whole
  process group closes every write end, `poll` sees HUP/EOF, `read_output`
  returns, `wait_with_output` reaps. Killing only the leader does not (C2.3).
- Measured (Linux, this machine): a child writing 200 000 bytes to stdout then
  `echo done >&2`, with the parent reading only stderr: after 3 s stderr is not
  readable, the child has not exited, and the writer (`head`) sits in
  `wchan = anon_pipe_write`. Blocked forever.

### A7. `Command::spawn` fd inheritance

- std dup2's only the three stdio fds into the child; everything else relies on
  close-on-exec. std's own fds are CLOEXEC: pipes (A3), files
  (`sys/fs/unix.rs` `OpenOptions` flags start from `libc::O_CLOEXEC`), sockets
  and `socketpair` (`sys/net/connection/socket/unix.rs`, `SOCK_CLOEXEC`).
- Not covered: an fd opened by a C library in-process (the toolkit, fontconfig,
  D-Bus, a GPU driver) without `O_CLOEXEC` is inherited by every git child —
  and if it is a pipe's write end, whoever reads that pipe waits for git too.
  Whether any such fd exists in a running Cairn is **OPEN** (check
  `/proc/<git pid>/fd` of a child spawned from the real app).
- Passing an extra fd (≥ 3) to a child is impossible through `std::process`
  without `pre_exec` — relevant to `GIT_TRACE2_EVENT=<fd>` (C7).
- Signal state in the child (`sys/process/unix/unix.rs`): the signal MASK is
  inherited from the spawning thread ("Inherit the signal mask from the parent
  rather than resetting it"); `SIGPIPE` is reset to `SIG_DFL` (unless
  `-Zon-broken-pipe`). Any other disposition Cairn set to `SIG_IGN` survives
  exec (POSIX). git also restores `SIGPIPE` itself (`common-init.c`,
  `restore_sigpipe_to_default`).

### A8. `std::os::linux::process::{PidFd, CommandExt::create_pidfd, ChildExt}` — unstable

- `std/src/os/linux/process.rs` is `#![unstable(feature = "linux_pidfd", issue = "82971")]` as a whole. Not usable on stable. Linux-only regardless.

### A9. Does anything needed require `unsafe`?

| Need | Safe route on 1.97.1 | Needs |
| --- | --- | --- |
| Child in its own process group | `CommandExt::process_group(0)` | std only |
| Signal the whole group (TERM, then KILL) | `nix::sys::signal::killpg` | nix `signal` (on) |
| Signal one pid with TERM | `nix::sys::signal::kill` | nix `signal` (on) |
| Poll for exit | `Child::try_wait` | std only |
| Timeout / deadline | `try_wait` loop with a sleep, or reader threads + `mpsc::Receiver::recv_timeout` | std only |
| Read stdout+stderr together, buffered | `wait_with_output` (std polls internally) | std only |
| Non-blocking / multiplexed reads by hand | `nix::poll::poll` + `nix::fcntl::fcntl(F_SETFL, O_NONBLOCK)` | nix `poll` + `fs` (both OFF today; add no crates, B7) |
| Read with a timeout without nix | child stdout as a `UnixStream::pair()` end (`OwnedFd` → `Stdio`, stable 1.63) + `set_read_timeout`/`set_nonblocking` (stable 1.10) | std only; git seeing a socket rather than a pipe on stdout is untested (**OPEN**) |
| New session (no controlling tty) | none on stable | `pre_exec` (unsafe) — unavailable |
| Kill on parent death (`PR_SET_PDEATHSIG`) | none: must run in the child | `pre_exec` — unavailable |

**Nothing required for kill-the-group, timeouts or non-blocking reads needs
`unsafe`.** Only a new session and parent-death signalling do, and both are out.

---

## B. nix 0.31.3

Enabled today (root `Cargo.toml`: `nix = { version = "0.31.3", default-features = false, features = ["signal", "process"] }`).
`cargo tree -e features -i nix@0.31.3` shows only `process` and `signal`, both
from `cairn-git`; no other crate in the graph unifies more features in, so
nothing else may be relied on implicitly. One `nix` in `Cargo.lock`.

| Item | Path in nix 0.31.3 | Safe? | Gate | On today? |
| --- | --- | --- | --- | --- |
| `sys::signal::kill(pid: Pid, signal: impl Into<Option<Signal>>) -> Result<()>` | `src/sys/signal.rs` | safe fn (wraps `libc::kill`) | `sys::signal` module is ungated (`pub mod signal;` in `src/sys/mod.rs`); `Signal` items behind `signal` | yes |
| `sys::signal::killpg(pgrp: Pid, signal)` | `src/sys/signal.rs`, `#[cfg(not(target_os = "fuchsia"))]` | safe fn | as above | yes |
| `unistd::Pid` (`from_raw`, `as_raw`) | `src/unistd.rs` | safe | ungated | yes |
| `unistd::getpgid(Option<Pid>)`, `setpgid`, `setsid`, `getpgrp` | `src/unistd.rs`, inside `feature! { #![feature = "process"] }` | safe fns | `process` | yes |
| `sys::wait::waitpid(pid, Option<WaitPidFlag>) -> Result<WaitStatus>`, `WaitPidFlag::WNOHANG`, `WaitStatus::StillAlive` | `src/sys/wait.rs`; module behind `#![feature = "process"]` in `src/sys/mod.rs` | safe fn | `process` | yes |
| `poll::poll(&mut [PollFd], impl Into<PollTimeout>)`, `PollFd::new(BorrowedFd, PollFlags)` | `src/poll.rs`; module behind `#![feature = "poll"]` in `src/lib.rs` | safe fns | `poll` | **no** |
| `fcntl::fcntl(fd: impl AsFd, FcntlArg)`, `F_GETFL`/`F_SETFL(OFlag::O_NONBLOCK)`, Linux `F_GETPIPE_SZ`/`F_SETPIPE_SZ` | `src/fcntl.rs`, the `FcntlArg` enum and `fcntl` fn sit inside `feature! { #![feature = "fs"] }` | safe fn | `fs` | **no** |
| `spawn::posix_spawn(path, &PosixSpawnFileActions, &PosixSpawnAttr, args, env) -> Result<Pid>`, `PosixSpawnAttr::set_pgroup`, `PosixSpawnFileActions::add_dup2/add_open/add_close` | `src/spawn.rs`; `#[cfg(any(freebsd, haiku, linux, netbsd, apple))]` + `process` | safe fns | `process` | yes, but see below |

Notes:

- B1. std's `ChildStdin`/`ChildStdout`/`ChildStderr` implement `AsFd` (stable
  1.63, `std/src/os/unix/process.rs`), so they go straight into
  `PollFd::new(x.as_fd(), ..)` and `fcntl(&x, ..)` — no raw fds, no `unsafe`.
- B2. `kill`/`killpg` take a `Pid` the caller vouches for. The pid-reuse hazard
  is the caller's (B8).
- B3. Do not call `nix::sys::wait::waitpid` on a pid std's `Child` owns: it
  reaps behind std's back, after which `Child::try_wait`/`wait` fail with
  `ECHILD` and std's "already reaped, don't signal" guard (A4) no longer knows.
  `Child::try_wait` is the WNOHANG equivalent and is enough.
- B4/B5. `poll` and `fs` are what a single-threaded multiplexer needs (A6
  pattern 3). Neither is needed for patterns 1–2.
- B6. `nix::spawn::posix_spawn` is a safe alternative spawner that CAN place an
  arbitrary fd at an arbitrary number in the child (`add_dup2`) — the only safe
  way to give git a `GIT_TRACE2_EVENT` fd ≥ 3. Costs: it returns a bare `Pid`,
  not a `std::process::Child` (so waiting is `waitpid` by hand), it has no
  `chdir` action (use `git -C`), its `PosixSpawnFlags` lists `RESETIDS`,
  `SETPGROUP`, `SETSIGDEF`, `SETSIGMASK` — no `SETSID` — and Cairn's guard
  `every_git_invocation_disables_the_terminal_prompt` currently treats
  `nix::spawn::posix_spawn` as a forbidden process-building shape
  (`crates/cairn-guards/tests/invariants.rs` matcher self-test lists it).
- B7. Feature cost: in nix 0.31.3's `Cargo.toml` `[features]`, `poll = []` and
  `fs = []` — **turning them on adds no crate**; it is a feature change in
  `Cargo.toml` and `deny.toml` is unaffected. (`event = ["poll"]` gives kqueue
  on macOS, also no crate.)
- B8. pid / pgid reuse — when a signal can hit a stranger:
  - POSIX XBD §4.17: "A process group ID shall not be reused by the system until
    the process group lifetime ends." and a process ID equal to a live process
    group ID "shall not be reused … until the process group lifetime ends".
  - Linux `kernel/pid.c` `__change_pid`: a `struct pid` number is released only
    when no task uses it as PID, TGID, PGID or SID (`pid_has_task` over all
    `PIDTYPE_MAX` types).
  - So `killpg(leader_pid)` is safe for as long as ANY member of the group is
    alive, even after the leader is reaped. Once every member is gone the number
    can be reused, and a new process could become a group leader with it. A
    `killpg` that returns `ESRCH` means the group is empty. Residual race: group
    empties and the number is reused for a new group leader between Cairn's
    check and its signal — requires PID wraparound in that window; not
    eliminable without pidfds (unstable in std).
  - For the single-pid `kill`, the existing runner's rule holds: only signal
    while `try_wait` under the same lock says not-yet-reaped.

---

## C. git behaviour (source at v2.56.0 unless stated; floor v2.30.0)

### C1. Signals vs lock files

- `tempfile.c` (header comment + `activate_tempfile`): every active tempfile is
  on `tempfile_list`; the first activation registers
  `sigchain_push_common(remove_tempfiles_on_signal)` and
  `atexit(remove_tempfiles_on_exit)`. `remove_tempfiles_on_signal` closes and
  `unlink`s each tempfile OWNED BY THIS PID (`p->owner != me` skipped), then
  `sigchain_pop(signo); raise(signo)` — so git dies of the same signal after
  cleanup.
- `sigchain.c` `sigchain_push_common`: `SIGINT`, `SIGHUP`, `SIGTERM`,
  `SIGQUIT`, `SIGPIPE`. Identical at v2.30.0 (`git show v2.30.0:sigchain.c`).
- `lockfile.h`: "Under the covers, a lockfile is just a tempfile" — `index.lock`,
  ref locks, `config.lock`, `packed-refs.lock` all get this cleanup.
- `SIGKILL` cannot be caught: no cleanup, `*.lock` stays.
- **Exception — editor:** `editor.c` `launch_specified_editor` pushes
  `SIG_IGN` for `SIGINT` and `SIGQUIT` while the editor runs (and re-raises
  them afterwards if the editor died of one). `SIGTERM` is not ignored.
- **Exception — children:** git kills only children registered
  `clean_on_exit` (`run-command.c` `cleanup_children`, sends git's own signal,
  or `SIGTERM` at exit): aliases/dashed externals (`git.c`), long-running
  filter processes (`sub-process.c`), parallel-checkout workers, `difftool`
  children, `odb/source-files.c`. Children run through
  `run_processes_parallel` — **hooks** — get the signal forwarded by
  `handle_children_on_signal` → `kill_children` (direct children only). The
  editor, `ssh`, `git-remote-https`, credential helpers and askpass are NOT
  signalled by git.
- **Measured** (git 2.56.0, `git commit -a` with an editor script that
  `exec sleep 30`, git in its own group via `process_group=0`, `index.lock`
  confirmed held before the signal):

  | Signal → target | `index.lock` after | git exit | Left alive in the group |
  | --- | --- | --- | --- |
  | SIGTERM → git pid | removed | signal 15 | the editor (`sleep`), orphaned |
  | SIGINT → git pid | (git ignored it; the editor ran its 30 s, commit aborted on empty message) | 1 | — |
  | SIGKILL → git pid | **stale** | signal 9 | the editor, orphaned |
  | SIGTERM → group (`killpg`) | removed | signal 15 | nothing |
  | SIGKILL → group (`killpg`) | **stale** | signal 9 | nothing |

  So: `SIGTERM` to the group is the only cancel that both releases git's locks
  and leaves no orphans; `SIGINT` is unreliable (ignored during an editor);
  `SIGKILL` must stay the escalation, and after it a stale lock is expected.
- Each git process removes only the tempfiles it owns. A nested git (one run by
  a hook, `git-remote-*`, a sub-git) holds its own locks and needs its own
  `SIGTERM` — which `killpg` delivers and a pid-only `kill` does not.

### C2. Process groups of git's children

- `git grep 'setpgid\|setsid\|killpg'` over git's C sources at v2.56.0 (excluding
  `t/`, `compat/mingw*`, `contrib/`) finds exactly: `setup.c` `daemonize`
  (`setsid`), `builtin/fsmonitor--daemon.c` (`setsid` under `--detach`), and
  `progress.c` `is_foreground_fd` (`getpgid(0)` read only). `run-command.c`
  `start_command` never changes the child's group. **Therefore every child git
  starts — `ssh`, `git-remote-https`, credential helpers, askpass, hooks, filter
  drivers (`clean`/`smudge` and long-running `process`), the editor, the pager —
  is in git's process group, and `killpg` on a git started with
  `process_group(0)` reaches them**, except the ones that leave on purpose:
  - **Auto-maintenance / `gc --auto`:** `run-command.c`
    `prepare_auto_maintenance` runs `git maintenance run --auto --detach` unless
    `maintenance.autoDetach`/`gc.autoDetach` say otherwise; `--detach` reaches
    `setup.c` `daemonize()`: fork, the parent hands its tempfiles to the child
    (`reassign_tempfile_ownership`) and exits, the child `setsid()`s and closes
    fds 0/1/2. Escapes `killpg`, does not hold Cairn's pipes, may hold
    `gc.pid`/repo locks after git has exited. Callers: `builtin/am.c`,
    `commit.c`, `fetch.c`, `merge.c`, `rebase.c`, `receive-pack.c`. Off switch
    per invocation: `-c maintenance.auto=false` (falls back to `gc.auto` when
    unset; both present at v2.30.0 — `Documentation/config/maintenance.txt`,
    `gc.txt`); `maintenance.autoDetach` is newer than the floor (only
    `gc.autoDetach` exists at v2.30.0).
  - **fsmonitor daemon:** `fsmonitor-ipc.c` `spawn_daemon` runs
    `git fsmonitor--daemon start` with `no_stdin/no_stdout/no_stderr` and
    `close_fd_above_stderr`; `start` launches `run --detach`, which `setsid()`s.
    Escapes `killpg`, holds no Cairn pipe. Linux backend since v2.55.0
    (RelNotes 2.55.0: "The fsmonitor daemon has been implemented for Linux";
    `config.mak.uname` `FSMONITOR_DAEMON_BACKEND = linux`; the
    `core.fsmonitor` doc text still says "Windows and MacOS").
  - **`ssh` ControlPersist master:** openssh `ssh.c` `control_persist_detach`:
    fork, `stdfd_devnull`, `daemon(1, 1)` (which `setsid`s). Escapes the group
    and drops the pipes.
  - **`credential-cache--daemon` does NOT escape.** `builtin/credential-cache.c`
    `spawn_daemon` starts it with `no_stdin` and a stdout pipe it reads `ok\n`
    from; the daemon (`builtin/credential-cache--daemon.c` `serve_cache`)
    `fclose(stdout)`s and reopens stderr on `/dev/null`, ignores only `SIGHUP`,
    and never calls `setsid`/`setpgid`. **Measured:** started by
    `git credential-cache store` under `process_group=0`, the daemon's pgid
    equals the store process's pid (same group, same session). So a `killpg`
    that cancels a fetch which happened to spawn the cache daemon also kills the
    user's credential cache (cached credentials lost; next operation prompts).
    Pipes are released, so it never blocks Cairn's readers.
- C2.3 **Why a pid-only kill is not enough — pipes.** A grandchild inherits
  git's stdout/stderr. **Measured:** `sh -c 'sleep 4 & exec sleep 30'`,
  stderr piped: `SIGKILL` to the pid → stderr EOF after 3.7 s (when the orphan
  exited); `SIGKILL` to the group → EOF after 0.0 s. A `ssh` or
  `git-remote-https` that outlives a killed git keeps the runner's reader blocked
  the same way.
- C2.4 **Background group + controlling terminal (SIGTTIN).** When Cairn is
  started from a terminal, a child in a new process group is a *background*
  group of that terminal's session (no stable `setsid`, A2). `setpgid(2)`: "if
  a background process group tries to read(2) from the terminal, then the group
  is sent a SIGTTIN signal, which suspends it." A git/ssh/helper that opens
  `/dev/tty` and reads would STOP (not die, not error) — a hang the runner sees
  as "still running". Cairn's `GIT_TERMINAL_PROMPT=0` and
  `SSH_ASKPASS_REQUIRE=force` exist to stop exactly those reads; anything else
  that reads the tty (a credential helper of the user's, a GPG pinentry-tty) is
  **OPEN**. Writes only stop under `stty tostop`. Launched from a desktop
  session (no controlling terminal), this does not arise. git's progress
  display is unaffected: `progress.c` `is_foreground_fd` treats
  `tcgetpgrp() < 0` (stderr is a pipe → `ENOTTY`) as foreground.
- C2.5 When Cairn dies, its children are not signalled (no `PDEATHSIG`, A9),
  but every pipe end Cairn held closes: git's next write to stdout/stderr gets
  `SIGPIPE` (default disposition, A7/C1) and dies with cleanup; a git blocked
  reading a stdin PIPE sees EOF. A git that neither reads nor writes keeps
  running to completion.

### C3. `GIT_OPTIONAL_LOCKS=0` / `--no-optional-locks`

- `git.c` maps `--no-optional-locks` to `setenv("GIT_OPTIONAL_LOCKS", "0")`
  (so it propagates to child gits). `environment.c` `use_optional_locks()` =
  `git_env_bool("GIT_OPTIONAL_LOCKS", 1)`.
- **The only caller is `builtin/commit.c` `cmd_status`**: with optional locks
  off it skips `repo_hold_locked_index` and so never writes the refreshed index.
  `Documentation/git-status.adoc` "BACKGROUND REFRESH" recommends it for
  background `status`.
- **Not honoured** (each opportunistically takes `index.lock` and rewrites the
  index, ignoring the variable):
  - porcelain `git diff` comparing against the work tree: `builtin/diff.c`
    `refresh_index_quietly`, called when `skip_stat_unmatch > 1`, i.e. when
    `diff.autoRefreshIndex` (default true) found stat-only changes;
    `Documentation/config/diff.adoc`: "affects only `git diff` Porcelain, and
    not lower level `diff` commands such as `git diff-files`".
  - `git describe --dirty` (`builtin/describe.c`).
  - These silently skip the write if the lock is taken (flags `0`), but while
    they HOLD it, a concurrent writer (the user's terminal `git commit`) fails.
    Plumbing (`diff-files`, `diff-index`, `diff-tree`) never writes the index.
- Available at the floor: `GIT_OPTIONAL_LOCKS` and `--no-optional-locks` are in
  `v2.30.0:Documentation/git.txt` (introduced v2.15.0, RelNotes 2.15.0).
- Measured: with `index.lock` present, `git status --porcelain=v2` exits 0 and
  `git diff --quiet` exits normally (1 = differences); `git add` exits 128.

### C4. `core.fsmonitor`

- `fsmonitor.c`: an index read in a repository with fsmonitor configured
  queries it — the hook form via `capture_command` (a child in git's group,
  C2), the daemon form via IPC, spawning the daemon on first use
  (`fsmonitor-ipc.c` `spawn_daemon`, detached, C2). It sets
  `istate->cache_changed |= FSMONITOR_CHANGED` when the token moves, which makes
  `status` (with optional locks on) write the index.
- With `GIT_OPTIONAL_LOCKS=0` the new token is not persisted, so every
  background `status` re-asks from the old token. Cost unmeasured (**OPEN**).
- First-use daemon start can add latency to the first query of a session;
  `fsmonitor--daemon start` waits up to `fsmonitor__start_timeout_sec` for the
  daemon (`builtin/fsmonitor--daemon.c`, `start_bg_command`). Value and
  observed latency **OPEN**.
- `core.fsmonitor` exists at v2.30.0 (hook form only); the built-in daemon is
  v2.36+ on macOS/Windows and v2.55+ on Linux.

### C5. Locale: what is translated, what is not

- `common-init.c`: `setlocale(LC_CTYPE, "")`; `gettext.c`
  `git_setup_gettext`: `setlocale(LC_MESSAGES, "")`, `setlocale(LC_TIME, "")`.
  Nothing else (no `LC_NUMERIC`, `LC_COLLATE`). Same at v2.30.0
  (`gettext.c` / `common-main.c`). `get_preferred_languages` reads `LANGUAGE`
  first; GNU gettext ignores `LANGUAGE` when the `LC_MESSAGES` locale is `C`.
- **Translated:** anything through `_()` — `die`'s `fatal:` prefix, `error:`,
  `warning:`, `hint:`, most diagnostics — plus libc `strerror` text. **Measured**
  (`LANG=en_GB.utf8 LANGUAGE=de`, index locked): `Schwerwiegend: Unable to
  create '…/index.lock': Die Datei existiert bereits.` — prefix and `strerror`
  German, the newer sentence still English (untranslated in the catalogue). With
  `LC_ALL=C LANGUAGE=de`, English throughout.
- **Not translated:**
  - `status --porcelain` v1: `wt-status.c` sets `s->no_gettext = 1`, and the
    `LABEL()` macro then bypasses `_()`; doc: "guaranteed not to change in a
    backwards-incompatible way between Git versions or based on user
    configuration".
  - `status --porcelain=v2`: none of the eight `wt_porcelain_v2_*` functions in
    `wt-status.c` contains `_(`. Measured under `LANGUAGE=de`: headers
    `# branch.oid` / `# branch.head` and entries unchanged.
  - `diff-tree --raw`: `diff.c` `diff_flush_raw` contains no `_(`.
  - With `-z`, paths are emitted verbatim (no quoting, so `core.quotePath` and
    `LC_CTYPE` play no part).
- `LC_TIME` affects only `strftime`-based dates (`--date=format:…`); git's own
  date names (`date.c`) are fixed English.
- Consequence for the manager: **exit codes and machine formats are
  locale-independent; message text is not.** Classifying a failure by matching
  stderr needs `LC_ALL=C` (which also turns off the user-language diagnostics
  `environment.rs` inherits on purpose) or must match only untranslated
  fragments (paths, `.lock`). Exit codes: `usage.c` `die` → `exit(128)`;
  usage errors / `parse-options.c` → 129. Lock failure under
  `LOCK_DIE_ON_ERROR` → `unable_to_lock_die` → 128 (measured: `git add` with
  `index.lock` present → rc 128).

### C6. Index lock contention and retry knobs

- `lockfile.c` `unable_to_lock_message` (EEXIST):
  `Unable to create '<abs path>.lock': File exists.` + blank line + one of
  "Lock may be held by process N; …", "Lock was held by process N, which is no
  longer running; the lock file appears to be stale", or "Another git process
  seems to be running in this repository, or the lock file may be stale" — the
  PID variants only when `core.lockfilePid` (new in **v2.54.0**, default
  `false`; writes `index~pid.lock` beside `index.lock`) is on.
- Retry: `hold_lock_file_for_update_timeout_mode` → `lock_file_timeout`,
  quadratic backoff with ±25 % jitter (`INITIAL_BACKOFF_MS`). Knobs:
  `core.filesRefLockTimeout` (default 100 ms), `core.packedRefsTimeout`
  (default 1000 ms) — both in `v2.30.0:Documentation/config/core.txt`;
  `core.configLockTimeout` (default 1000 ms) — **new in v2.56.0** (commit
  `df67d73ca`, 2026-05-17), absent at the floor.
- **The index has no retry knob.** `repository.c` `repo_hold_locked_index` →
  `repo_hold_lock_file_for_update` (timeout 0: one attempt). Writers that die
  (exit 128): `add`, `commit`, `stash`, `apply --cached/--index`,
  `update-index`, `checkout`, `reset`, `merge`, `rm`, `mv`, … Opportunistic
  writers that silently skip: `status`, porcelain `diff`, `describe --dirty`.
  Any retry on index contention is Cairn's to implement.

### C7. Trace2 for per-invocation timing

- `Documentation/git.adoc` `GIT_TRACE2` (applies to `GIT_TRACE2_EVENT` and
  `GIT_TRACE2_PERF`): value `1`/`2`/`true` → stderr; an integer 3–9 → that open
  fd; an absolute path → append (a directory → one file per process named from
  the SID); `af_unix:[stream|dgram:]<absolute path>` → a Unix domain socket.
  All present at v2.30.0 (`af_unix` and `GIT_TRACE2_EVENT` in
  `v2.30.0:Documentation/git.txt`).
- The fd form is unreachable from safe std (A7) — `nix::spawn` could, at the
  cost in B6. The **socket form fits Cairn**: it already runs a
  `std::os::unix::net::UnixListener` for askpass. A file path or directory also
  works.
- Child processes inherit the variable, so the target also receives events
  from `git-remote-https`, sub-gits and hooks that are git; events carry `sid`
  (child SIDs are nested `parent/child`) — `GIT_TRACE2_EVENT_NESTING` controls
  region depth. Event format: `Documentation/technical/api-trace2.adoc`
  (`start` with `argv` and `t_abs`, `exit` with `code` and `t_abs`,
  `child_start`/`child_exit`, `region_enter`/`region_leave`).
- `trace2.c` `redact_arg` replaces a URL password in `argv` with `<REDACTED>`
  unless `GIT_TRACE2_REDACT=0`. Cairn's no-credential invariant still needs a
  review of whatever it keeps from the stream.
- The env var overrides the user's `trace2.eventTarget` config; a user who has
  global trace2 config already gets those files written by Cairn's gits too
  (config is read unless overridden) — expected, not Cairn's to stop.

### C8. `--end-of-options`

- Parser-level since **v2.24.0** (RelNotes 2.24.0: "The command line parser
  learned `--end-of-options`"). `git rev-parse` learned it in **v2.30.0**
  (RelNotes 2.30.0). Revision-argument handling after it was fixed for "many
  programs like `reset` and `checkout`" in **v2.43.1 / v2.44.0** (RelNotes:
  "`git $cmd --end-of-options --rev -- --path` for some $cmd failed to interpret
  `--rev` as a rev").
- So at the 2.30 floor it exists everywhere `parse_options` runs, but using it
  before a REVISION argument is only trustworthy from 2.43.1. Below that, a
  rev beginning with `-` must be avoided by other means (pass full OIDs, which
  never start with `-`, or `refs/…` names).

### C9. Editors

- `editor.c` `git_editor`: `GIT_EDITOR` → `core.editor` → `VISUAL` (only when
  the terminal is not dumb) → `EDITOR` → if `TERM` is unset or `dumb`, return
  NULL → `launch_specified_editor` errors **"Terminal is dumb, but EDITOR
  unset"**; else default `vi`. `git_sequence_editor`: `GIT_SEQUENCE_EDITOR` →
  `sequence.editor` → `git_editor()`.
- An editor value of exactly `:` is a no-op (the file is used as is).
- The editor is run with `use_shell = 1` and git's own stdin/stdout/stderr — no
  redirection. With Cairn's environment (`TERM` not inherited → dumb) a user
  with `core.editor = vim` still gets vim launched on `/dev/null` stdin and a
  pipe stdout (behaviour of the editor, not git — it may hang or exit); a user
  with `core.editor = "code --wait"` gets a GUI window. **`GIT_EDITOR` must be
  set per invocation, and `GIT_SEQUENCE_EDITOR` too**, because
  `sequence.editor` in user config beats `GIT_EDITOR` for rebase todo lists.
- `git merge` opens an editor only when stdin and stdout are the same tty
  (`builtin/merge.c` `default_edit_option`), or `GIT_MERGE_AUTOEDIT=yes`; with
  Cairn's pipes it does not.
- While the editor runs, git ignores `SIGINT`/`SIGQUIT` (C1).
- All three variables exist at v2.30.0 (`GIT_SEQUENCE_EDITOR` in
  `v2.30.0:Documentation/git.txt`).

### C10. Hooks: `core.hooksPath`, stdin/stdout

- `core.hooksPath` exists at v2.30.0 (introduced v2.9). Relative paths resolve
  against the directory hooks run in: the work-tree root, or `$GIT_DIR` for bare
  repositories and the push-side hooks (`Documentation/githooks.adoc`).
- `hook.c`: each hook gets `cp->no_stdin = 1` unless the hook feeds input
  (`path_to_stdin` or `feed_pipe` — `pre-push`, `post-rewrite`,
  `reference-transaction`, …). `stdout_to_stderr` defaults to 1
  (`RUN_HOOKS_OPT_INIT`), "the default behavior for all hooks except pre-push";
  with `hook.jobs` > 1 (parallel hooks; `hook.jobs` is absent from `hook.h`
  at v2.55.0 and present at v2.56.0) every hook's stdout goes to stderr.
- So a hook's output lands on git's stderr — i.e. Cairn's stderr pipe — and
  hooks inherit git's stdin only where the docs say so (never Cairn's, which is
  null). Hooks are children in git's group (C2) and receive git's own signal
  (C1).

### C11. `git apply --cached` with the patch on stdin

- `apply.c` `apply_all_patches`: no path arguments, or `-`, → `apply_patch(state, 0, "<stdin>")`.
- `apply_patch` calls `read_patch_file` → `strbuf_read(sb, fd, 0)` — the whole
  stdin is read to EOF **before** parsing and **before** the index lock is
  taken (`repo_hold_locked_index(…, LOCK_DIE_ON_ERROR)` further down the same
  function). Cap: `MAX_APPLY_SIZE` = 1023 MiB ("patch too large").
- So for `apply` specifically, write-everything-then-close-stdin-then-drain is
  deadlock-free (nothing is emitted while stdin is being read); stdin MUST be
  closed or apply never starts. A lock failure is a 128 exit AFTER the patch was
  consumed. Available at the floor (`--cached` in `v2.30.0:Documentation/git-apply.txt`).

### C12. `git cat-file --batch` / `--batch-command`

- `--batch` / `--batch-check`: long-standing (well before 2.30). Without
  `--buffer`, "batch output is flushed after each object is output, so that a
  process can interactively read and write" (`Documentation/git-cat-file.adoc`
  `--buffer`). With `--buffer`, normal stdio buffering — nothing arrives until
  the buffer fills or the process exits.
- `--batch-command`: **v2.36.0** (`batch-command` absent from
  `v2.35.0:Documentation/git-cat-file.txt`, present at v2.36.0; RelNotes 2.36.0).
  Commands `contents <obj>`, `info <obj>`, `flush`; `flush` "Used with
  `--buffer` to execute all preceding commands…"; without `--buffer`, "commands
  are flushed each time without issuing `flush`". Newer commands (`mailmap`
  v2.55.0, `remote-object-info` v2.56.0) are above any reasonable floor.
- Missing object: `<object> SP missing LF` (no contents block) — the request is
  answered, the stream stays in sync.
- `-Z` (NUL-delimited input AND output): **v2.42.0** (absent at v2.40.0); `-z`
  (input only) is deprecated "as the output can otherwise be ambiguous".
- Protocol consequence: a long-lived `cat-file` is a strict request/response
  pipe; the writer must not run ahead of the reader by more than a pipe's worth
  without a concurrent reader (A6). Below 2.36, `--batch` without `--buffer` is
  the interactive form.

### C13. `git diff-tree --stdin`

- `Documentation/git-diff-tree.adoc` `--stdin`: lines of two trees, one commit,
  or a commit plus parents; the header (both tree IDs, or the commit ID) is
  printed before each diff. Present at v2.30.0.
- `builtin/diff-tree.c`: input read with `fgets` into `char line[1000]` — a
  longer line is split. **A line that does not parse as an OID is echoed
  verbatim and `fflush(stdout)`ed** — a natural sync marker.
- Flushing: a commit line goes through `log_tree_commit` →
  `maybe_flush_or_die(stdout)` (`log-tree.c`), which flushes per record when
  stdout is not a regular file (`write-or-die.c`: `GIT_FLUSH` unset →
  `!S_ISREG`), so on a pipe each commit's output is flushed. A **tree-pair**
  line goes through `stdin_diff_trees` → `log_tree_diff_flush` with no stdio
  flush — output may sit in git's buffer until a later flush. Write a non-OID
  sentinel line after a tree pair (or set `GIT_FLUSH=1` — its documented list
  does not name `diff-tree`, so whether it reaches the tree-pair path is
  **OPEN**) and read up to the echoed sentinel.
- With rename detection on, `diff-tree --stdin` reads the index once
  (`repo_read_index`) for the size cache.

### C14. Floor summary (Cairn floor 2.30.0)

| Feature | Since | At floor? |
| --- | --- | --- |
| `--no-optional-locks` / `GIT_OPTIONAL_LOCKS` | 2.15.0 | yes |
| `status --porcelain=v2 [-z]` | 2.11.0 ("Porcelain Format Version 2" absent from `v2.10.0:Documentation/git-status.txt`, present at v2.11.0 and v2.30.0) | yes |
| `diff-tree --raw -z`, `--stdin` | long-standing | yes |
| `apply --cached` from stdin | long-standing | yes |
| `cat-file --batch[-check]`, `--buffer` | long-standing (`--buffer` in v2.30.0 docs) | yes |
| `cat-file --batch-command`, `flush` | 2.36.0 | **no** |
| `cat-file -Z` | 2.42.0 | **no** |
| `--end-of-options` (parser) | 2.24.0 | yes |
| `--end-of-options` before revs, reliable | 2.43.1 / 2.44.0 | **no** |
| `GIT_TRACE2_EVENT` / `_PERF`, `af_unix:` target | ≤ 2.30.0 (in its docs) | yes |
| `GIT_EDITOR`, `GIT_SEQUENCE_EDITOR`, `core.hooksPath` | ≤ 2.30.0 | yes |
| `core.filesRefLockTimeout`, `core.packedRefsTimeout` | ≤ 2.30.0 | yes |
| `core.configLockTimeout` | 2.56.0 | **no** |
| `core.lockfilePid` (PID in lock diagnostics) | 2.54.0 | **no** |
| `maintenance.auto`, `gc.auto`, `gc.autoDetach` | ≤ 2.30.0 | yes |
| built-in fsmonitor daemon | 2.36 (mac/Win), 2.55.0 (Linux) | n/a (user config) |
| index lock retry knob | none in any version read | — |

---

## D. macOS differences, and pipe capacity

### D1. Linux pipe capacity

- `pipe(7)`: "Since Linux 2.6.35, the default pipe capacity is 16 pages";
  queryable/settable with `F_GETPIPE_SZ`/`F_SETPIPE_SZ` (nix `fs` feature);
  "Since Linux 4.5, the default pipe capacity is lower than 16 pages when the
  pipe-user-pages-soft limit is exceeded" — then "limited to two pages (one
  page before Linux 5.14)". `PIPE_BUF` = 4096.
- Measured here: `F_GETPIPE_SZ` = 65536; a non-blocking writer got exactly
  65536 bytes in before `EAGAIN`. `/proc/sys/fs/pipe-max-size` = 1048576,
  `pipe-user-pages-soft` = 16384 pages ("permits creating up to 1024 pipes with
  the default capacity").
- A process manager with many concurrent children (3 pipes each) is far from
  1024 pipes, but the soft limit is per USER across all their processes —
  a desktop session with heavy pipe users could drop Cairn's new pipes to
  8 KiB. Design for small buffers.

### D2. A child filling stdout while the parent reads only stderr

- The child's `write(1, …)` blocks once the pipe holds its capacity (64 KiB
  default on Linux, see D4 for macOS). The child never reaches the stderr write
  or exit the parent is waiting on; the parent's stderr read never returns.
  Permanent deadlock, no error, no timeout. Measured: A6 (`wchan =
  anon_pipe_write`). The same holds for stderr filling while the parent reads
  only stdout, and for stdin when the parent writes while the child is blocked
  writing output. Every piped stream must be drained concurrently, or be
  `Stdio::null()` (the existing runner's `stream()` nulls stdout for exactly
  this reason).

### D3. macOS: spawn and fds

- std on Apple targets takes the `posix_spawn` path
  (`sys/process/unix/unix.rs`, `target_vendor = "apple"` in the supported list);
  `process_group` maps to `POSIX_SPAWN_SETPGROUP` as on Linux. `setsid` would
  force fork/exec (only linux-gnu has the spawn flag) — moot, unstable.
- std pipes on macOS are `pipe()` + `set_cloexec()` (A3): two syscalls. Another
  thread spawning a process between them leaks that pipe end into the unrelated
  child, which then holds it open — a reader of that pipe waits for the
  unrelated child too. On Linux `pipe2(O_CLOEXEC)` closes the window. Whether
  std serialises pipe creation against spawns on macOS: not found in the
  source read (`sys/pipe/unix.rs` takes no lock) — treat as possible.
- No pidfd, no `PR_SET_PDEATHSIG` on macOS; `kqueue` `EVFILT_PROC`/`NOTE_EXIT`
  is the native exit notifier (nix `event` feature) — not needed if
  `try_wait`/reader threads suffice.
- Process groups, `killpg`, `SIGTERM`/`SIGKILL`, `SIGTTIN` for background
  groups: POSIX, same semantics; POSIX XBD §4.17 gives the same pgid-reuse
  guarantee as B8 (XNU's implementation of it not read — **OPEN**).

### D4. macOS pipe capacity

- XNU `bsd/sys/pipe.h`: `PIPE_SIZE 16384`, `BIG_PIPE_SIZE (64*1024)`,
  `SMALL_PIPE_SIZE PAGE_SIZE`, `PIPE_KVAMAX (1024 * 1024 * 16)`;
  `bsd/sys/syslimits.h`: `PIPE_BUF 512`.
- `bsd/kern/sys_pipe.c`: buffers are allocated lazily on first write and sized
  by `choose_pipespace` from `pipesize_blocks[] = {512, 1024, 2048, 4096,
  8192, PIPE_SIZE, PIPE_SIZE * 4}` — i.e. grown to fit the pending write, up to
  64 KiB, and only while system-wide pipe KVA (`amountpipekva < maxpipekva`)
  allows. Under KVA pressure a pipe can stay as small as 512 bytes.
- So on macOS the deadlock in D2 can trigger after **as little as 512 bytes and
  at most 64 KiB**; code tuned on Linux's fixed 64 KiB must not assume it.
  (`F_GETPIPE_SZ` is Linux-only.)
- Not run on a Mac; read from XNU `main` as of 2026-10-02, which may differ from
  the shipping macOS kernel — **OPEN** for a measured number on the target
  macOS versions.

---

## OPEN items (not verified, not guessed)

1. Whether a running Cairn (Freya/toolkit, D-Bus, fontconfig) holds fds without
   `O_CLOEXEC` that leak into git children (A7).
2. Whether a blocking write ≤ `PIPE_BUF` after `POLLOUT` can block on Linux /
   macOS (A6 pattern 3).
3. git behaviour with stdout/stderr as a Unix socket instead of a pipe (A9
   socketpair option).
4. Tty readers other than git's prompt and ssh (user credential helpers,
   pinentry-tty) under a background process group (C2.4).
5. Cost of `GIT_OPTIONAL_LOCKS=0` with fsmonitor (token never persisted), and
   first-use daemon start latency (C4).
6. Whether `GIT_FLUSH=1` makes `diff-tree --stdin` flush after tree-pair
   records (C13).
7. XNU's pgid-reuse implementation and measured macOS pipe sizes (D3, D4).
