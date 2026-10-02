# State — process-manager

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 03 (engine lifecycle) done on `feature/process-manager`;
phase 04 (application) is next** — it wires `SharedRepository::end_invocations`
with `CLOSE_BOUND` to the window's close on a worker thread, and has the worker
answer `SharedRepository::command_log`. Fetch and the version probe run on the
runner, and `GitCommand::run`, `stream`, `Running` and `ProcessKill` are gone.
Items awaiting user review are in `progress.md` (the phase 01, 02 and 03
entries). `diff-engine` is in flight on `feature/diff-engine` and switched to
this packet because its changes query needs a `git` process; it continues
once this merges.

## Locked decisions

L1-L15 are in `brainstorm.md`. The design frame is `docs/design/processes.md`,
with D1 in `engine.md` and D3 in `concurrency.md`. These constrain the
implementation most:

- **Only `process/` builds a process (L2).** Only `ops/` and `reads/` call the
  runner. A write needs a `WriteAuthority`, which only `ops/` can construct.
- **Reads run free; writes go through two lanes (L3).** This packet builds the
  network lane (fetch) and the refusal of a duplicate. The local lane is built by
  `staging-and-commit`.
- **Every `git` gets its own process group and ends `SIGTERM` → 2 s → `SIGKILL`
  to the group (L4).** Cancel comes by poll (`&impl Cancel`), by handle, or by
  drop.
- **A thread per pipe (L6).** stdout goes to the caller; a collect has a ceiling
  that errors when crossed; stderr keeps a 256 KiB tail (L7).
- **Nothing parses translated text, and the locale stays the user's (L8).**
  `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false` always; a read adds
  `GIT_OPTIONAL_LOCKS=0` and carries no token (L9).
- **No timeouts, no batch children, no retries, no lock deletion** (L5, L8, L12).

## Obligations for later phases

Discharged in phase 03 (from phase 02 QA):

- **QC3** — `Error::GitFailed`'s doc in `crates/cairn-git/src/error.rs` no
  longer mentions the older paths: `present_locks` is filled for a write run
  in a repository, fetch among them, and empty for a read and the probe.
- **QC6** — the module-level `expect(dead_code)` in `process/runner.rs`,
  `group.rs` and `pipes.rs` is gone; what is still unused outside tests
  carries its own item-level expect (`Invocation::records`, `pipes::Records`
  and its impl, beside the items that already had one).
- **GI2** — `only_the_process_module_builds_or_runs_a_process`'s positive
  rows `.output()` and `.wait()` were SWAPPED for `CommandExt` and
  `ChildStdout` (beside `.try_wait()`), in the same commit that deleted the
  old paths and saying so in its body; both calls stay on the banned roster
  outside `process/`. A new twin, `the_retired_runner_is_gone`, bans the old
  runner's names and entry points everywhere in `cairn-git`.

Raised in phase 03 for later phases:

- **Phase 04:** a failed fetch's `GitFailed` now carries `present_locks`, but
  the banner (`crates/cairn-app/src/status_text.rs`) shows only the first
  `fatal:`/`error:` line, so the list may not reach the user (QA DO2).
- **Phase 04:** a close `SIGKILL`s a write that outlasts the grace; the locks
  it strands are listed on a cancellation nobody may be left to show (QA DO3).
- **User:** DO1 (a fetch exiting 0 inside the signal race is reported
  cancelled), the stderr-tail retention residual, and the stale CLAUDE.md and
  qa-checklist text — all in `progress.md`'s phase 03 entry.

Still owed:

- **Phase 04:** call `SharedRepository::end_invocations(CLOSE_BOUND)` on a
  worker thread when the repository closes (and the window with it), and
  answer `SharedRepository::command_log()` through the worker (G14, G17's
  worker half). The UI thread must not call either: `end_invocations` waits.
- **Phase 05:** the G8 drop tests, G10, G12's success arm and R3.1's test are
  wholly `#[cfg(target_os = "linux")]`; only their `/proc` reads need Linux.
  Narrow the gating so macOS keeps the rest (QA TC16).

## Decided by the user

- **Partial-clone lazy fetch on a read (D3, raised by phase 01 QA) — decided
  by the user on 2026-10-02.** Keep the git floor at 2.30 and add
  `("GIT_NO_LAZY_FETCH", "1")` to `READ_ONLY`. Git older than 2.44 ignores the
  variable, so a read in a partial clone on such a git may still lazy-fetch (a
  pack written, the network reached; with no token an authenticated promisor
  fails closed) — a constraint `diff-engine` designs around, recorded in
  `crates/cairn-git/src/reads/mod.rs` and `docs/systems/git-processes.md`.
  Landed in phase 03: the builder's and the stub tests' read sets,
  `READ_ONLY_PINS` in `every_git_invocation_disables_the_terminal_prompt` with
  self-test cases, and `a_read_in_a_partial_clone_does_not_fetch_a_missing_object`
  against real git.

## Open questions

None of this packet's own. The questions it leaves to other packets are listed
at the end of `brainstorm.md`.

## New modules and interfaces introduced so far

None yet. As phases land, record here the type or function, its crate, and its
one-line contract.

| Symbol | Crate | Contract |
| --- | --- | --- |
| `process` module (`src/process/`) | cairn-git | Crate-private; the only place a process is built, spawned, waited on or read. Holds `GitBinary`, `GitVersion`, `GitEnvironment`, `Askpass` (re-exported from `ops`) and the runner (`cli.rs`, moved unchanged). |
| `GitBinary::read_invocation()` | cairn-git | `pub(crate)`; a `GitCommand<'_, Read>`: `READ_ONLY` env, no token field. For `reads/` and `ops/`. |
| `GitBinary::write_invocation(WriteAuthority)` | cairn-git | `pub(crate)`; a `GitCommand<'_, Write>` holding the authority; `authorized_by` exists only on it. |
| `ops::WriteAuthority` | cairn-git | `pub(crate)`, private field, `pub(in crate::ops) fn new()`; no Clone/Copy/Default. The write seal. |
| `GitCommand<'a, K>`, `Read`, `Write`, `Kind` | cairn-git | In private `process/cli.rs`; `GitCommand::new` is `pub(super)`. The kind picks the profile. |
| `environment::Profile` (`Read` / `Write { token }`), `READ_ONLY` | cairn-git | `GitEnvironment::command(program, Profile)` is `pub(super)`; a read adds `GIT_OPTIONAL_LOCKS=0`. `ALWAYS` now also pins `GIT_EDITOR=false`, `GIT_SEQUENCE_EDITOR=false`. |
| `reads` module (`src/reads/`) | cairn-git | Empty, documented: one named function per read git answers; query plumbing or `status` only. |
| `GitCommand::start()`, `GitCommand::input(bytes)` | cairn-git | `pub(crate)`; spawns with `process_group(0)`, a thread per pipe, stdin fed then closed; returns `Invocation<K>`. `start_with(&Spawner)` is `pub(super)` for tests. |
| `Invocation<K>` (`process/runner.rs`) | cairn-git | `pub(crate)`, not re-exported. `finish(cancel, stdout, progress)`, `records(cancel, record, progress)`, `collect(cancel, ceiling, progress)` → `Result<Output, Error>`; `kill_handle()`. Drop ends and reaps on a `cairn-git-reaper` thread. |
| `KillHandle` (`process/group.rs`) | cairn-git | `pub(crate)`, `Send + Sync + Clone`; `kill()` records the request and only try-locks. Nothing signals once the invocation is over. |
| `Kind::cancelled`, `Kind::present_locks`, `GitDirs` (`process/cli.rs`) | cairn-git | The kind decides the outcome: a read's cancel is `GitReadCancelled`; a write's is `GitCancelled` with locks listed after the reap, and its failure names the locks present. `in_repository` records the git and common directories. |
| `Error::GitReadCancelled`, `GitOutputTooLarge`, `GitUnwatched`; `GitFailed::present_locks` | cairn-git | New public error shapes (R5). `GitUnwatched` covers a pipe thread that could not start, a failed stdin write and a failed `try_wait`. |
| `ops::stranded_locks` | cairn-git | Now `pub(crate) mod`, so the runner can list a write's locks. |
| `RUNNER_NAMES` += `Invocation`, `KillHandle` | cairn-guards | The runner guard and its self-test cover the new names. |
| `only_the_process_module_builds_or_runs_a_process`, `the_runner_is_named_only_by_ops_and_reads`, gitoxide half of `only_the_ops_module_mutates_a_repository` | cairn-guards | New twins; rosters `PROCESS_IDENTS`, `PROCESS_NULLARY_CALLS`, `PROCESS_CALL_EXCEPTIONS`, `RUNNER_NAMES`, `WRITE_NAMES`, `GITOXIDE_MUTATION_*`. |
| `cairn_model::CommandRecord`, `CommandExit` (phase 03) | cairn-model | One invocation that is over: arguments, directory, start, duration, exit, cancelled, stderr tail. No environment field; `held_bytes()` is what the log's byte bound counts. |
| `process/registry.rs`: `Processes`, `Registration`, `CLOSE_BOUND` (phase 03) | cairn-git | `Processes` is `pub(crate)`, one per `SharedRepository`, shared by every `to_worker` handle. `GitCommand::in_repository` books the invocation: in at spawn, out (writing one record) when the runner concludes it on any thread; a failed spawn is logged as `NotStarted`. `CLOSE_BOUND` is `pub`, re-exported as `cairn_git::CLOSE_BOUND`. |
| `process/command_log.rs`: `CommandLog`, `LOG_ENTRIES`, `LOG_BYTES` (phase 03) | cairn-git | `pub(crate)`; oldest dropped first to hold both bounds; a record alone over `LOG_BYTES` is trimmed to fit and says so. |
| `SharedRepository::end_invocations(bound) -> usize`, `SharedRepository::command_log() -> Vec<CommandRecord>` (phase 03) | cairn-git | Public. `end_invocations` asks every running invocation to end as a cancel does and waits up to `bound` for the registry to empty, returning how many were left; afterwards the repository is closing and a new invocation is ended at once. It waits: a worker's call. `command_log` is the log, oldest first. |
| `the_retired_runner_is_gone` (phase 03) | cairn-guards | Bans `Running`, `ProcessKill`, `run`/`stream` declared on `GitCommand`, and `.stream(..)` calls anywhere in `cairn-git` (G18); self-test `the_retired_runner_matcher_catches_the_shapes_it_claims`. |
| `READ_ONLY_PINS` (phase 03) | cairn-guards | The read table's pins, `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1`, checked by `every_git_invocation_disables_the_terminal_prompt` through `const_table` and `missing_pins`, with self-test cases. |

## Bounds fixed by phases

The PRD names these and leaves their values to the phase that builds them.
Record each value and its reason here when it is chosen.

| Bound | Phase | Value | Why |
| --- | --- | --- | --- |
| `DRAIN_BOUND` — output read after the leader exits (R3.6) | 02 | 250 ms from the exit, the 20 ms tick included (`process/runner.rs`) | Bounds only the case where something other than git holds a pipe after git exits; the common case ends when the pipes close. What is still owed then is output git wrote before exiting: at most a pipe's capacity per stream plus what the readers queued. Readers send one event per read (stderr's lines together), so that is a handful of events, moved in well under a millisecond, and the driver takes what is queued once more before it lets go; 250 ms is two orders of margin for a loaded machine's scheduler, and short enough that a finished operation still reads as finished. The same bound applies after `SIGKILL` on a cancel, for a holder that left the group. Pinned by `the_fixed_bounds_have_the_values_the_packet_recorded`. |
| `CLOSE_BOUND` — wait for reaps on repository close (R6.3) | 03 | 3 s (`process/registry.rs`, `cairn_git::CLOSE_BOUND`) | A close ends each invocation as a cancel does: `SIGTERM`, then `SIGKILL` after the 2 s `TERMINATION_GRACE`, and a pipe held by a process that left the group is abandoned `DRAIN_BOUND` (250 ms) after that — so 2.25 s is the longest the runner itself takes. 3 s adds three-quarters of a second for a loaded machine's scheduler; anything not reaped by then has had `SIGKILL` and is past what waiting changes, so the window closes rather than hangs. A compile-time assertion keeps it above grace + drain. Pinned by `the_log_and_close_bounds_have_the_values_the_packet_recorded` and `ending_every_invocation_waits_no_longer_than_its_bound`. |
| `LOG_ENTRIES`, `LOG_BYTES` — command log size (R8.2) | 03 | 1000 records; 4 MiB (`process/command_log.rs`) | The count bounds the common case, where a record is a few hundred bytes: a thousand is a long session's fetches and per-selection reads, so the log reaches back through the afternoon, not the last minute. The bytes bound the case the count cannot — git saying a lot, each record up to a 256 KiB tail, which a thousand of would make 256 MiB: 4 MiB keeps sixteen full tails, and is above what one record holds at the default `ARG_MAX` (2 MiB Linux, 1 MiB macOS), so a record is trimmed to fit only past that. Pinned by `the_log_and_close_bounds_have_the_values_the_packet_recorded`, `the_log_keeps_the_newest_log_entries_records`, `the_log_holds_no_more_than_log_bytes`. |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 seal and environment | done (`main...HEAD` through phase 01's commits) | `scripts/gate.sh` exit 0 | qa-checklist, gate-integrity, destructive-ops, test-coverage; qa-confirm: 11 confirmed and fixed, 5 dismissed, 1 escalated (lazy fetch), 1 probed and resolved |
| 02 runner | done (`a5f5160..HEAD`) | `scripts/gate.sh` exit 0 (`progress.md`) | qa-checklist, destructive-ops, responsiveness, test-coverage, gate-integrity; qa-confirm: 34 confirmed and fixed or deferred, 6 dismissed, 3 escalated; re-review of the fixes (destructive-ops, test-coverage; qa-confirm): 8 confirmed and fixed, 1 dismissed (`progress.md`) |
| 03 engine lifecycle | done (`edee9cf..HEAD`) | `scripts/gate.sh` exit 0 (`progress.md`) | see `progress.md`'s phase 03 entry |
| 04 application | not started | — | — |
| 05 QA | not started | — | — |

## Environment notes

- **The bench repository is a clone of rust-lang/rust at `c999cef531e` in
  `~/Development/bench/rust`.** G16 is measured there, and the harness reads
  `CAIRN_BENCH_REPO`. Read it only: no `gc`, no `repack`, no config writes.
- **Verify every std and nix API against the source that links**, never from
  memory:
  - std is under `$(rustc --print sysroot)/lib/rustlib/src/rust/library`
    (rust-src is installed for 1.97.1).
  - nix 0.31.3 is under `~/.cargo/registry/src/`.
  - `docs/research/process-manager/platform-and-git-behaviour.md` records what was
    verified, and where.
- **Don't call nix's `waitpid` on a pid std's `Child` owns.** It reaps behind
  std's back.
- **No `unsafe`.** `pre_exec`, child-side `setsid` and `PR_SET_PDEATHSIG` are all
  out.
- **Run `scripts/gate.sh` for the bar.** Never an ad-hoc `&&` chain, and never
  pipe it through `tail`.
- **Commit explicit paths**, never `git add -A`. No commit, PR or comment carries
  a Claude Code session link (root `CLAUDE.md`).
- **A new invariant or guard change needs its row and self-test in
  `crates/cairn-guards/` in the same commit.**
- The remote is `github.com/alexparlett/cairn`. Issues and pull requests go
  there, and every pull request is merged by the user, never by a session.
