---
status: in-flight
packet: process-manager
opened: 2026-10-02
---

# PRD — Process manager

**In flight. Authoritative while the packet is open.** Teardown stamps this file
and points at `docs/systems/git-processes.md`; until then this is the one copy of
what the packet commits to.

Design frame: `docs/design/processes.md` (how every `git` process is built, run,
cancelled and accounted for), with **D1** in `docs/design/engine.md` (reads `git`
answers) and **D3** in `docs/design/concurrency.md` (the write lanes). Program:
`docs/work/daily-loop/roadmap.md`, where this packet sits between
`credential-prompts` and the rest of `diff-engine`, which is in flight and needs it. Decisions and rejected
alternatives: `docs/work/process-manager/brainstorm.md` L1-L15. Evidence, all
under `docs/research/process-manager/`: `runner-and-worker-as-built.md` (the
runner, worker and guards as they stand), `consumer-invocations.md` (every `git`
invocation the daily loop needs), `precedent-study.md` (how other clients run
git) and `platform-and-git-behaviour.md` (std, nix and git behaviour, verified
against Rust 1.97.1, nix 0.31.3 and git 2.56.0); and under
`docs/research/diff-engine/`, `git-process-survey.md` and
`rename-parity-spike.md`, which commissioned it.

## What this packet delivers

One place in Cairn that starts `git`, and a runner able to carry every kind of
invocation the daily loop needs: a read whose output is parsed and which a newer
query can stop, a mutation fed on stdin, a mutation that runs the user's hooks
for minutes, and a network operation reporting progress. Every invocation runs
in its own process group and can be ended, by the user, by a superseding query or
by being dropped, without leaving a process holding Cairn's pipes or a lock file
nobody was told about.

The packet adds no verb the user can see. Fetch and the startup probe move onto
the new runner and the old one is deleted. The first consumer of a read is
`diff-engine`'s changes query, which continues on top of this packet; the first
consumers of stdin and the local write lane are `staging-and-commit`'s. Every
shape is proven here against real `git` so that neither of them has to reopen the
runner.

## Requirements

### R1 — One place builds a process, and the type says what kind

- R1.1 Every `git` process is built in `crates/cairn-git/src/process/`, a
  crate-private module holding the binary, the environment, the runner and the
  registry. `GitBinary`, `GitEnvironment` and `Askpass` move there from `ops/`.
- R1.2 An invocation is a **read** or a **write**, chosen when it is built. A write
  needs a `WriteAuthority`, whose only constructor is private to `ops/`, so code
  outside `ops/` cannot build one.
- R1.3 Only `ops/` and `reads/` invoke the runner, apart from `process/`'s own
  version probe. Nothing outside `process/` builds, spawns, waits on or reads a
  process, or calls a method that yields one. `reads/` holds one named function
  per read `git` answers; this packet creates the module's place in the guard,
  and `diff-engine` adds its first function. The application reaches the
  binary, the environment and the askpass target through re-exports, never the
  runner.
- R1.4 No gitoxide mutation API is named outside `ops/`, closing the half of
  "only `ops` mutates" that has no twin today.

### R2 — The environment

- R2.1 The base environment is unchanged: built from the roster, `ALWAYS` applied,
  both askpass variables naming Cairn's helper.
- R2.2 `ALWAYS` also sets `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false`, so a
  verb that would open an editor fails instead of waiting on one.
- R2.3 A read also gets `GIT_OPTIONAL_LOCKS=0` and never carries an askpass token.
  A read runs plumbing or `status` only: porcelain `diff` and `describe --dirty`
  ignore the variable and refresh the index.
- R2.4 The locale stays inherited. Nothing parses translated text: output is read
  only in formats git does not translate, and failure is classified by exit
  status and repository state. No new dependency: no logging crate, and nix keeps
  its `process` and `signal` features, unconditional on Unix (Linux and macOS).

### R3 — The runner

- R3.1 Every invocation's process is the leader of a new process group.
- R3.2 Each pipe the invocation uses is drained or fed on its own thread, so no
  output volume on one stream can stall another.
- R3.3 stdout reaches the caller as it arrives, as bytes or as NUL-separated
  records, and the caller can stop early by cancelling. A caller collecting the
  whole of stdout states a ceiling, and crossing it is an error, never a
  shortened answer.
- R3.4 stdin, when used, is the caller's bytes, written and then closed.
- R3.5 stderr is forwarded line by line, split on `\r` and `\n`, to a caller that
  asks for progress, and the last 256 KiB of it is retained for the error and the
  log; nothing retains more.
- R3.6 An invocation is finished when its leader has exited and its pipes are
  drained, or `DRAIN_BOUND` has passed since the exit. Exit is noticed when the
  pipes close, or on the cancel tick when something else still holds them. A
  process left holding a pipe is never waited on beyond the bound. Phase 02 fixes
  the bound's value and records it in `state.md`.
- R3.7 The probe (`git --version`) and fetch run on this runner; the old `run`
  and `stream` paths are deleted.

### R4 — Cancellation

- R4.1 A read takes the query's cancel signal and polls it while it waits; once
  the signal says superseded, the process is ended.
- R4.2 An invocation hands out a kill handle that is `Send`, `Clone`, and never
  blocks the thread using it.
- R4.3 Dropping an invocation that has not finished ends it and reaps it on a
  reaper thread; the drop does not block.
- R4.4 Ending means `SIGTERM` to the group, then `SIGKILL` to the group after two
  seconds. The group is signalled only while a member is believed alive — the
  leader unreaped, or a pipe still open — and the race that rule leaves is stated
  as a residual, not claimed closed.
- R4.5 A cancel that loses the race to a clean exit is reported as success, as
  fetch's is today. Any other cancelled invocation reports cancellation, whatever
  its exit status.

### R5 — Outcomes

- R5.1 A failure carries the arguments, the exit status and the retained stderr.
- R5.2 A cancelled read reports cancellation and nothing else; a cancelled write
  reports the lock files left behind, as fetch's does.
- R5.3 A write that fails while a lock file it needs is present names that file.
  Nothing retries, and nothing deletes a lock.
- R5.4 Exceeding a stdout ceiling is its own error, naming the ceiling.

### R6 — Lifecycle

- R6.1 `git` is found and its version checked once, when the application starts;
  each thread that runs `git` holds a copy of that answer.
- R6.2 Each open repository keeps a registry of its running invocations.
- R6.3 Closing a repository ends every invocation in its registry and waits up
  to `CLOSE_BOUND` for their reaps, on a worker thread. Closing the window closes
  its repository. Phase 03 fixes the bound's value and records it in
  `state.md`.

### R7 — Write lanes

- R7.1 The operations thread becomes the network lane. Fetch runs there.
- R7.2 Starting a network operation while the same operation is running or queued
  is refused with a reason the window shows, never silently dropped.
- R7.3 The local lane, and queueing between different operations, are designed in
  `docs/design/concurrency.md` and built by the first packet with a local write —
  `staging-and-commit` in the roadmap's order, unless `branch-ops` lands first.
  Nothing in this packet may make that harder: the lane is chosen per operation,
  not assumed.

### R8 — The command log

- R8.1 Every finished invocation is recorded with its arguments, working
  directory, start time, duration, exit status, whether it was cancelled, and its
  retained stderr.
- R8.2 The log is bounded by `LOG_ENTRIES` and `LOG_BYTES`, lives in memory, and
  never records the environment or a `Secret`. Phase 03 fixes both values and
  records them in `state.md`.
- R8.3 A log entry is a `cairn-model` type, and the worker can answer the log for
  a repository. No view draws it in this packet.

### R9 — The record

- R9.1 `docs/design/processes.md`, D1 and D3 describe the design this packet
  builds, and the root `CLAUDE.md` states the new invariants with their twins.
- R9.2 `docs/systems/git-processes.md` describes the runner, the seal, the
  environment, cancellation, the registry, the lanes and the log as built, and
  `docs/systems/credentials.md` points at it instead of describing the old runner;
  the root `CLAUDE.md` Pointers name it.
- R9.3 Issue #18's editor half is answered by R2.2: commented on, and closed if
  nothing else remains in it.

## Product rules

- Nothing waits on `git` without a way to stop it. A user-started operation has a
  cancel; a query has its epoch; everything has the window's close.
- A user never meets an editor, a terminal prompt or an unseen dialog through
  Cairn's `git`: a verb that needs one fails and says so.
- Cairn never deletes a lock file and never re-runs a mutation on its own.
- What Cairn ran on the user's behalf is recorded truthfully: every invocation,
  however it ended.

## Acceptance criteria

The single authoritative copy. `docs/work/process-manager/qa-checklist.md` points
here and does not restate them.

| # | Criterion | Pinned by |
| --- | --- | --- |
| G1 | From outside `cairn-git`, no write invocation and no `WriteAuthority` can be named or constructed | `compile_fail` doctests, run by the gate's `test-doc` step. The in-crate half — that `reads/` cannot construct one — is G2's, because a doctest compiles as another crate and fails on privacy whatever the seal does |
| G2 | The guard fails on each of these, outside the modules allowed them: a process type named or built; `spawn`, `output`, `status` or `wait` on one; a call to a `GitBinary` or `GitEnvironment` method that yields a process; an environment-setting call; the runner named outside `ops/`, `reads/` and `process/`; `WriteAuthority`'s constructor named outside `ops/`; a gitoxide mutation API named outside `ops/`. Its self-test shows each shape caught and each legitimate one passed, and each of the four unguarded routes in `runner-and-worker-as-built.md` section 3 fails it | the rewritten guard twins and their matcher self-tests in `cairn-guards` |
| G3 | The read and write environments are spelled out variable by variable — `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false` on both, `GIT_OPTIONAL_LOCKS=0` and no token on a read — and a stub `git` run through each prints exactly that set | `process/` unit tests and stub-git tests |
| G4 | A read of `git status` on a dirty fixture with a stale index leaves the index file byte-identical | integration test, real `git` |
| G5 | A verb that would open an editor fails promptly instead of waiting: `git commit` without a message on a fixture | integration test, real `git` |
| G6 | At least 5 MiB of stdout, interleaved with continuous stderr, arrives whole and in order as `-z` records, without deadlock. A collect with a smaller ceiling fails with the ceiling error and returns no partial answer | stub-git test, plus a real `git` read over a generated fixture |
| G7 | 64 MiB written to stdin while the process writes stdout and stderr continuously completes without deadlock. Real `git hash-object --stdin` over 64 MiB sees end of input and gives `git`'s own id for those bytes | stub-git test for the concurrent streams; integration test for end of input and id parity |
| G8 | A cancel through the kill handle, a superseded cancel signal and a drop each end a stub whose grandchild holds stderr open. The invocation returns within the grace plus a tick, and no member of the group is left alive or unreaped | stub-git tests that read the process table |
| G9 | A stub that traps `SIGTERM` receives it first; one that ignores it is killed after the grace; and a real `git commit` cancelled while a `pre-commit` hook sleeps leaves no `index.lock` | stub-git tests, plus an integration test with real `git` |
| G10 | An uncancelled invocation whose stub exits while a grandchild still holds stdout and stderr returns within `DRAIN_BOUND` of the exit | stub-git test |
| G11 | A failure carries the arguments, exit status and retained stderr. A cancelled read reports cancellation and no lock files. A cancelled write lists the lock files present after the reap. A failed write names a pre-existing `index.lock` | integration tests |
| G12 | A cancel that arrives after a clean exit is reported as success; a cancelled invocation that exits nonzero, or zero after the signal, is reported cancelled | stub-git tests that control the ordering |
| G13 | A stub writing 1 MiB of stderr in lines delivers every line to the progress callback, and the retained tail is at most 256 KiB and ends with the last line | stub-git test |
| G14 | Closing a repository with a fetch in flight ends and reaps its whole group within `CLOSE_BOUND` | worker test through the real boundary, with a stub `git` |
| G15 | A second fetch while one runs is refused with a reason the window draws | worker test, plus a headless window test |
| G16 | `git` is discovered once per application start, not once per repository opened | worker test counting the stub's probe invocations |
| G17 | Every invocation appears in the command log exactly once, with every R8.1 field, whether it finished, was cancelled, was dropped or failed to spawn. The log stops at `LOG_ENTRIES` and `LOG_BYTES`. A fetch run with an askpass token leaves no trace of the token or of any environment value | `process/` tests for the engine half; worker test for the log answered through the worker |
| G18 | Fetch's existing tests, the credential-prompts suite included, pass with their assertions unchanged on the new runner, and nothing names the old `run` or `stream` | the existing tests, plus the guard |
| G19 | On rust-lang/rust at `c999cef531e`, warm, in a release build: `git diff-tree -r -M -z --raw` on `5a3292f163d` through a read invocation costs no more than 2 ms over the same command through a bare `std::process::Command` | an `#[ignore]`d reporter driven by `CAIRN_BENCH_REPO`, recorded in `progress.md` |
| G20 | Closing the window with a fetch in flight leaves no `git` process and no askpass socket behind | a check by hand on a desktop session, recorded in `progress.md` |
| G21 | `docs/design/processes.md`, D1 and D3 match what was built. The root `CLAUDE.md` states each new invariant with its twin and points at `docs/systems/git-processes.md`, which is current. Every parse site reads an untranslated format (R2.4). The write lane is chosen per operation (R7.3). #18 is answered (R9.3) | review |
| G22 | `scripts/gate.sh` passes | the gate |

G19 is not automated, for the reason `diff-engine`'s C14 is not: a timing
assertion in CI is flaky and bound to a machine. The machine and the repository
are the ones `docs/research/diff-engine/measured-baseline.md` records. G20 is by
hand because what the toolkit does with the window's handle at exit is only
observable with a display (issue #27).

## Out of scope

Another packet's: the changes query on `git diff-tree` and its parser
(`diff-engine`, continuing on top of this packet); the local write lane, stdin's first verb and lock
contention beyond naming the lock (`staging-and-commit`); push and its
`pre-push` hook (`remote-sync`); the editor helper for interactive rebase (the
second lap); a view of the command log (to be filed); coalescing fetch progress to
one update a frame (#25, which this packet narrows by bounding what is retained).

Another packet's, and in tension with this one: `diff-engine`'s R2.2 says a
changes answer cut short by `diff.renameLimit` says so, but `git diff-tree`
reports that only as a translated warning on stderr, which nothing here may
parse. That packet must find a signal that is not prose, or bring the conflict
to the user.

Not planned: timeouts on any invocation; a long-lived `cat-file --batch` child;
`LC_ALL=C`; automatic retry on a lock; deleting a stale lock; Windows.

Residuals this packet states rather than closes, in `docs/design/processes.md`:
processes that leave the group on purpose; the user's credential cache ending
with an operation that started it; a child stopped reading a terminal when Cairn
was launched from one; children outliving a crash of Cairn itself; and the narrow
race in which a group id is reused between the liveness check and the signal.
