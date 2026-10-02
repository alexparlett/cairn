# Survey — how Cairn spawns git today, and what a process manager lacks

Evidence record, saved in full. Commissioned 2026-09-30 by the diff-engine packet
when the rename-parity decision (`rename-parity-spike.md`) put a `git` subprocess
on a read path, and the user asked for "a proper process manager designed and
built around spawning git processes, if we don't have one already". Read against
`main` at `7d4d9ad` / `origin/main` at `2987a53` and `origin/feature/diff-engine`
at `2c8ae1e`. File:line references are era artifacts. Historical: never
retro-edited.

## Scope and refs

- The process code is identical on `main` and `origin/main`.
  `origin/feature/diff-engine` does not touch `crates/cairn-git/src/ops/`,
  `crates/cairn-app/src/worker/` or the process guards; its only guard change is
  one `TEST_ONLY_ALLOWLIST` row for `allocation-counter`. Its `diff/` modules
  never name `GitBinary`, `GitCommand` or `ops`.
- On `origin/main` (#39), D1 moved out of `docs/design/cairn.md` into
  `docs/design/engine.md`, with the CLAUDE.md pointer updated.

## What exists

Paths under `crates/cairn-git/src/ops/`.

- **`mod.rs`** calls itself the mutation module (1-5). Runner summary at 30-40,
  claiming "Nothing outside `ops` can run a raw verb: the public surface is named
  operations". Output policy 46-54 (`-z`, porcelain v2, stderr never parsed);
  error policy 56-67; `Invalidated`/`Performed` 69-114, 136-245. Re-exports at
  128-132, including **`pub(crate) use cli::GitCommand`** (130).
- **`binary.rs` — `GitBinary`.** `MINIMUM = 2.30.0` (26-30). `discover` /
  `discover_with` (71-92) find `git` on the *built* environment's `PATH`
  (113-132) and probe `git --version` through `GitCommand::run` (146-154).
  Errors `GitNotFound`, `GitTooOld`, `GitVersionUnreadable`. Derives `Clone` (62).
  `command()` is `pub(crate)` (106-110).
- **`environment.rs` — `GitEnvironment`.** `ALWAYS` sets `GIT_TERMINAL_PROMPT=0`
  and `SSH_ASKPASS_REQUIRE=force` (32-42); `INHERITED` is a 26-name roster
  (60-128); the single constructor `new` (143-160) also sets `GIT_ASKPASS`,
  `SSH_ASKPASS`, `CAIRN_ASKPASS_SOCKET`. **`command(program, token)` (178-186) is
  the only `Command::new` in production**: `env_clear()`, `envs`, optional
  `CAIRN_ASKPASS_TOKEN`. Tests spell out the variable set (209-277).
  `GIT_OPTIONAL_LOCKS`, `GIT_PAGER`, `GIT_EXTERNAL_DIFF` are on neither the roster
  nor `ALWAYS`.
- **`askpass.rs` — `Askpass`** (14-35): a plain value, helper path plus optional
  socket.

### The runner, `cli.rs` (all `pub(crate)`)

- **`GitCommand`** (38-45): builder — `arg`/`args`, `in_repository` (70-73; cwd
  is the workdir, or the git dir if bare), `authorized_by(token)` (78-81).
- **`run()` (85-113) runs to completion.** stdin `Stdio::null()` (89), stdout and
  stderr piped, `command.output()` (95): blocks, buffers both streams in full with
  no limit, **cannot be killed, cancelled or timed out**, no progress. Spawn
  failure → `GitNotStarted`; non-zero exit → `GitFailed{arguments, status,
  stderr}`; success → `Output`. Only production caller: the version probe.
- **`stream()` (120-153)** starts the process and hands it back running: stdin
  null (124), **stdout null — discarded** (125), stderr piped. Returns
  `Running{Arc<Mutex<Process>>, cancelled: AtomicBool, stderr}` (158-166,
  172-179).
- **`Running::finish(progress)` (272-308)** spawns a detached `cairn-git-stderr`
  reader thread (278-284; detached because children such as ssh inherit the pipe
  and can outlive git). Lines split on `\n` or `\r` (367-394) go to the callback;
  it also accumulates `everything`, an unbounded `String`. Loops on
  `recv_timeout(EXIT_POLL = 20ms)` (362).
- **Kill and escalation.** `ProcessKill::kill` (408-438) sets the flag, then
  `try_lock`s and sends `SIGTERM` via `nix::sys::signal::kill`
  (`Process::terminate`, 197-214); refuses to signal a reaped pid. The waiter's
  `poll` (219-234) sends `SIGTERM` itself if the killer missed the lock, and
  `SIGKILL`s after `TERMINATION_GRACE = 2s` (185).
- **Reaping and outcome.** `reap` (310-358) always reaps, polling rather than
  `wait()`ing so it never holds the lock while blocked. A clean exit wins over a
  racing cancel (339-344). A cancelled non-successful exit → `GitCancelled{
  stranded_locks: []}` (345-352); other failure → `GitFailed` (353-357).
- **`Output`** (455-503): `stdout()`, `stderr()`, `stdout_text()`, `records()` (a
  `-z` splitter, 489-502). `stdout()` and `records()` carry `expect(dead_code)`,
  reason "the first operation to will be status" (462-468, 491-497).
- **Stub-git tests** (`stub_git.rs`, present twice because the runner is
  crate-private): stdin closed (1112-1119); a kill ends and reaps (763-820);
  `SIGTERM` first (893-934); escalation to `SIGKILL` (988-1006, 1021-1038); a lock
  miss finished by the waiter (1068-1108); no signal after the reap (1044-1062).
- **Not present:** no `Drop` for `Running` (dropped without `finish`, the child is
  neither killed nor reaped); no process group or `setsid` (children survive a
  kill); no timeout; no stdin writer; no stdout that is captured while killable.
- **Unix only as written:** `nix` is an unconditional dependency (guard allowlist
  row "SIGTERM on cancel"), imported at `cli.rs:29-30` without a cfg gate.

### Fetch and its helpers

- **`fetch.rs`.** `fetch()` (68-89) runs `refspec_policy::check` (no spawn), then
  `git.command().in_repository().args(["fetch","--progress","--no-prune-tags",
  "--end-of-options"]).arg(remote)` (94) with the optional token, then
  `.stream()`. `FetchInProgress` (97-105): `canceller()` (108-111), `finish()`
  (124-138) maps success to `Performed` with refs+objects invalidated, and on
  `GitCancelled` fills `stranded_locks`. `FetchCancel` (145-152): `Clone`/`Send`
  wrapper around `ProcessKill`.
- **`stranded_locks.rs`** (38-50): read-only `*.lock` listing after the reap.
- **`refspec_policy.rs`** 33-36: asking git for its system config path "would
  spawn a process outside GitEnvironment", so it is a stated residual.
- **Errors** (`crates/cairn-git/src/error.rs:70-97`): `GitNotStarted`,
  `GitFailed`, `GitCancelled{stranded_locks}`. The query-side `Cancel` trait and
  `CancelSignal` (`cancel.rs:6-30`) are pull-based and **not connected to the
  runner**.

### Lifecycle, end to end

Start through `GitEnvironment::command` → stdin always null → stdout fully
buffered (`run`) or discarded (`stream`) → stderr buffered, or streamed to a
callback and accumulated unbounded → progress via `finish`'s callback only →
cancel via push-based `ProcessKill`, `SIGTERM` then `SIGKILL` after 2 s, `stream`
only → no timeouts ("nothing times a fetch out", `docs/systems/credentials.md`
known limits) → exit mapped as above → reaped only when `finish` is called; git's
own children (ssh, git-remote-https, askpass) are not in a group and end on their
own (`cli.rs:261-265`).

## Concurrency and ownership (`crates/cairn-app/src/worker/`)

- Threads per repository (`pool.rs:4-16, 32-39`): `cairn-repository`,
  `cairn-operations`, `cairn-askpass`; `WORKERS_PER_REPOSITORY == 1` asserted at
  compile time.
- `Backend::open` runs `GitBinary::discover_with` on the repository thread per
  open (`startup.rs:71-104`, line 98; `pool.rs:103`), then moves `git` into the
  operations thread (`pool.rs:183-187, 220-232`). **Only the operations thread
  holds a `GitBinary`.**
- The operations thread owns a running process, one operation at a time, in
  order (`operations.rs:1-2, 117-175`). `Operation` has one variant, `Fetch`
  (30-33).
- **`FetchControl`** (`operations.rs:39-114`): single-slot state machine —
  `Idle`, `CancelledBeforeStarting`, `Starting{cancelled}`,
  `Running(FetchCancel)`. `arm()` refuses a second fetch; `Threads::perform`
  silently drops it (`pool.rs:248-255`).
- **No manager layer**: no registry, pool, global limit or list of in-flight
  processes. Most at once: one fetch per open repository plus the startup probe.
- **Fetch cancel, end to end:** `submit(Request::CancelFetch)` short-circuits to
  `control.cancel()` (`pool.rs:293-297`) → `FetchCancel::cancel` →
  `ProcessKill::kill` (flag, `try_lock`, `SIGTERM`) → the operations thread,
  blocked in `finish`, sees the flag, escalates, reaps, lists stranded locks →
  clears the control, retires the askpass token, compares `ref_tips`, sends
  `FetchCancelled` (`operations.rs:132-171`). A cancel before git is running is
  kept and applied in `install` (99-105).
- **Shutdown:** `Threads::drop` calls `control.cancel()`, closes the queue, stops
  the acceptor (`pool.rs:264-276)`, only when `serve` returns. All threads are
  detached and never joined; `main.rs:52-93` has no explicit shutdown. **No
  guaranteed kill-all on window close**: a fetch in flight at exit can be orphaned.
- **Epochs** (`epoch.rs:55-67`): `Superseded` implements `Cancel` for queries
  only; operations carry none (`request.rs:28-35`); nothing ties an epoch to a
  process kill.

## Invariants that bind any new spawn

- **`every_git_invocation_disables_the_terminal_prompt`**
  (`crates/cairn-guards/tests/invariants.rs:667-846`): over product `src/` with
  test modules blanked, no file but `ops/environment.rs` may name or build
  `Command`, name `posix_spawn*`/`exec*`, call `env`/`envs`/`env_clear`/
  `env_remove`, or write or `impl` a `GitEnvironment`.
- **`only_the_ops_module_mutates_a_repository`** (`invariants.rs:596-622`): its
  matcher `spawns_git` (`crates/cairn-guards/src/lib.rs:1304-1316`) fires only on
  a line with a literal `"git"`/`"/usr/bin/git"` plus `Command`, `new(` or
  `cmd(`. **`git.command().args([...]).run()` inside `crates/cairn-git/src/diff/`
  would not trip it.** The runner's seal is crate-level (`pub(crate)`), not
  module-level, and no guard names `GitCommand` or `GitBinary`. Such a call would
  contradict `ops/mod.rs:38-40`, `cli.rs:16-18`, CLAUDE.md ("Reads never spawn a
  process") and D1, but the gate stays green — a guard gap.
- **UI-thread rule** (`invariants.rs:983-1049`): any call must sit under
  `crates/cairn-app/src/worker/`.
- **D1 as it stands**: the filter-driver amendment lets a read start the user's
  clean filter driver through gix; "never on a read path" means the CLI. Stated
  residual: the driver runs with Cairn's inherited environment and inherited
  stderr. Nothing authorises a read-path `git` CLI spawn.

## How a read-path spawn would have to be wired today

1. Code in `crates/cairn-git/src/`, honestly as a named function beside `fetch`
   in `ops/` or a new read-invocation module — either way amending `ops/`'s
   "mutations" charter. Built through `git.command()`, so it gets the
   `GitEnvironment` with no token (askpass fails closed).
2. `Repository` carries no `GitBinary`; it lives on the operations thread only,
   so the (planned) diff thread needs a clone (`GitBinary` is `Clone`).
3. Neither runner fits: `run()` captures stdout but cannot be cancelled and
   buffers without limit; `stream()` can be cancelled but discards stdout.
4. Tying cancel to the epoch needs a bridge from pull-based `Cancel` to
   push-based `ProcessKill`; `finish`'s 20 ms loop (`cli.rs:288-306`) is the
   natural place to poll a `&impl Cancel`.
5. `GIT_OPTIONAL_LOCKS=0` is needed so a read never rewrites the index
   (`measured-baseline.md`); not on the roster, so `environment.rs` and its tests
   change. `--no-ext-diff` and `-c diff.renameLimit=` go in the arguments.
6. A cancelled read returns `GitCancelled{stranded_locks}`, mutation vocabulary;
   a read-cancel variant is wanted.
7. `Output::records()` already splits `-z` output.

## Gaps a general process manager would need

1. A read-only invocation path — none authorised, no read/write distinction in
   any guard, the runner is crate-visible rather than `ops`-private.
2. stdin — both paths pin `Stdio::null()`, a test enforces it; packet 5's
   `git apply --cached` needs it.
3. stdout captured AND cancellable, streamed or incremental (record by record for
   `-z`).
4. Output size limits and back-pressure — `run()`, `finish`'s `everything` and
   progress updates are unbounded (issue #25).
5. Timeouts or a watchdog — none, for fetch or anything.
6. Epoch-integrated cancellation — `ProcessKill` not driven by `Cancel`.
7. Process tracking — no registry, no kill-all on repository or window close, no
   join or reap at exit, no `Drop` safety on `Running`.
8. Concurrency policy — only the one-fetch `FetchControl` slot; no global or
   per-repository limit, queueing or read/write ordering.
9. Process-group management — children (ssh, remote helpers) not killed with git.
10. Structured-output parsers — only `records()`; no porcelain v2 or `--raw`
    parser in production; `stdout()`/`records()` still `expect(dead_code)`.
11. Observability — no per-invocation logging, timing or metrics; no logging
    crate on `cairn-git`'s allowlist (adding one is a user decision).
12. Environment for reads — `GIT_OPTIONAL_LOCKS=0` and a pinned `LC_ALL=C`
    missing; `LC_*` is inherited on purpose so messages reach the user in their
    language.
13. Non-Unix support for the kill path (unconditional `nix`).
14. Error vocabulary for a read cancel or failure, distinct from `GitCancelled`.
15. One discovery per app — `GitBinary` is discovered per repository open and
    lives on one thread.

## Verdict as reported

**Partially.** Cairn has a careful single-invocation runner, not a process
manager: one guarded construction point with an explicit environment, version
floor discovery, stderr streaming, a cross-thread kill with `SIGTERM`→`SIGKILL`
escalation, guaranteed reaping when `finish` runs, stranded-lock reporting and
clean error mapping, much of it pinned by stub-git tests. It is shaped around one
long-running mutation, fetch, owned by one operations thread with a one-slot
control. Everything a manager adds is missing.
