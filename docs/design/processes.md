# Running `git`

Intent, not as-built; `docs/systems/git-processes.md` describes what exists.
Spine: `docs/design/cairn.md`. This designs how Cairn starts, feeds, reads, cancels and
accounts for every `git` process it runs. Which work goes to `git` at all is D1
(`engine.md`); which thread waits on it is D3 (`concurrency.md`); what a process
may ask the user for is D2 (`credentials.md`).

## One place builds a process

Every `git` process Cairn starts is built in one crate-private module of
`cairn-git`, `process/`, and nowhere else. It holds the binary Cairn found, the
environment Cairn builds, the runner that drives a process from spawn to reap,
and the registry of what is still running. The application constructs the
binary, the environment and the askpass target through re-exports, because it
owns startup and the helper's channel; it never reaches the runner. Inside the
crate only two modules may run an invocation: `ops/`, where every mutation lives,
and `reads/`, where every read that runs `git` lives, one named function each.
The one other invocation is `process/`'s own version probe, which reads no
repository.

An invocation is typed as a **read** or a **write** when it is built, and the type
decides its environment. A write can only be built with a `WriteAuthority`, a
value only `ops/` constructs, so a read path cannot spawn a mutation by
building the wrong kind of invocation. Privacy stops at the crate, so a guard
pins what it cannot: nothing but `process/` builds or names a process, spawns,
waits on or reads one, or calls a method that yields one; nothing but `ops/` and
`reads/` names the runner; nothing but `ops/` names the `WriteAuthority`
constructor; and no gitoxide mutation API is named outside `ops/`.

Rejected: rechartering `ops/` as "every git process", which is simpler and blurs
the line the operation log and the confirmation seal stand on — `ops/` is where
the repository changes, and a reader of it should be able to trust that. Rejected:
a separate crate, whose seal would come from the dependency allowlist, because it
moves the environment invariant's file-level twin across crates for no gain the
token does not already buy.

## The environment

Every process runs with an environment Cairn built: the inherited roster, git's
terminal prompt off, ssh forced to the askpass helper, and both askpass variables
naming Cairn's helper (`credentials.md`). On top of that base, every invocation
also gets:

- **`GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false`.** A GUI has no terminal,
  and git still launches `core.editor` when `TERM` is unset, so a verb that wants
  an editor must fail rather than hang on one nobody can see. `false` and not `:`,
  because `:` silently accepts whatever message git proposed — a merge message the
  user never read. Both are set because `sequence.editor` outranks `GIT_EDITOR`.
  They belong to the fixed table every environment applies, so no invocation can
  forget them. Interactive rebase, the one verb whose purpose is the editor, needs
  them to name a helper of Cairn's own instead, and that is a change to the
  environment's construction and its guard, argued when that verb is designed —
  never a per-call override.

A **read** additionally gets:

- **`GIT_OPTIONAL_LOCKS=0`**, so `git status` never refreshes the index behind the
  user's back. Only `status` honours it: porcelain `diff` and `describe --dirty`
  take `index.lock` to refresh anyway. So a read runs plumbing or `status`, never
  another porcelain verb, and Cairn never refreshes the index as a side effect of
  looking at it — a refresh, if one is ever wanted, is a write.
- **No askpass token.** A read never asks the user for anything, so the helper it
  could reach fails closed.

**The locale stays the user's.** Cairn parses only output git does not translate —
`-z` records, `--raw`, porcelain v2 — and classifies a failure by its exit code
and the repository's state, never by matching stderr, which is prose for a person
and shown to them verbatim in their own language. Rejected: `LC_ALL=C` or
`LC_MESSAGES=C` for parsed output, which buys nothing once nothing parses
messages and puts every error the user reads in English.

Auto-maintenance is git's: a write that triggers `gc --auto` gets the detached
maintenance git itself would run, because a client that maintains a repository
differently from git is a client the user's next `git` command disagrees with.

## One process per invocation

Every invocation is one short-lived process. Cairn keeps no long-running
`git cat-file --batch` or `--batch-command` child: no read needs one while gix
answers object reads in process, and the batch modes that would be worth keeping
need a git newer than the 2.30 floor.

## The runner

The runner drives one process from spawn to reap, and the shapes it serves are
the ones the daily loop needs: a captured read, a mutation fed on stdin, a
hooked mutation that may run for minutes, and a network operation reporting
progress.

- **Its own process group.** Every `git` starts as the leader of a new process
  group. git never moves its children out of its group, so the group holds
  everything the invocation started — `ssh`, the remote helper, credential
  helpers, hooks, filter drivers — and a signal to the group reaches all of it.
- **Every pipe drained on its own thread.** A child that fills one pipe while the
  parent reads another deadlocks — on Linux after 64 KiB, or as little as two
  pages once the user's pipe limit is reached, and on macOS after as little as
  512 bytes. stdout, stderr and stdin each get a thread when they are used, so no
  stream can wait on another. Rejected: one thread polling every pipe, which needs
  more of `nix` than process groups do and buys one thread per invocation at the
  cost of handling partial writes by hand.
- **stdout is handed over, not hoarded.** The caller consumes stdout as it
  arrives, NUL-separated records for a `-z` format, and can stop early by
  cancelling. Where a caller wants the whole answer it states a ceiling, and
  crossing it is an error, never a silently shortened answer.
- **stdin is written by the caller's bytes and then closed.** git reads a patch
  to its end before it takes a lock, so a stdin left open would hold the
  operation forever.
- **stderr is kept as a bounded tail.** Progress lines, split on `\r` and `\n`,
  are forwarded to the caller as they come and then dropped; what is retained for
  an error or the log is the last 256 KiB.
- **It finishes when `git` does.** Once the leader has exited and its output is
  drained, the invocation is over. Exit is noticed when the pipes close, and
  checked on the cancel tick for the case where something else still holds them;
  then the remaining output gets a short drain bound. A process git left behind
  holding a pipe — detached on purpose, or simply slow to die — is never waited
  on beyond it.

## Cancellation

There are three ways to cancel an invocation, and all three end it the same way.

- **A query's epoch.** A read carries its lane's cancel signal, and the runner
  polls it while it waits, so a superseded read stops its process rather than
  finishing and being thrown away (`concurrency.md`).
- **The user.** An operation hands out a kill handle that any thread may use
  without waiting; the UI thread's cancel button is one.
- **Dropping it.** An invocation dropped before it finished is killed and reaped
  by a reaper thread, so a drop never blocks and never leaves a zombie.

Ending a process means `SIGTERM` to its group, then `SIGKILL` to the group after a
grace of two seconds. `SIGTERM` first, because git removes its lock files on
`SIGTERM` and cannot on `SIGKILL`; the group, because killing only `git` leaves its
children holding the pipes Cairn reads from, which is a hang, not a cancel. The
runner signals a group only while it has reason to believe a member is alive —
the leader not yet reaped, or one of its pipes still open. A group id cannot be
reused while any member lives, so that rule closes all but a narrow race, stated
below. A cancel that loses the race to a clean exit is a success: git finished,
and reporting otherwise would tell the user an operation did not happen when it
did. After a cancelled write, the lock files git left behind are listed for the
user, never deleted for them.

What a group kill does not reach, stated rather than implied:

- **Processes that leave the group on purpose** — detached auto-maintenance, the
  fsmonitor daemon, an ssh `ControlPersist` master — outlive the invocation. That
  is what they are for, and none of them holds Cairn's pipes.
- **The user's credential cache.** `git credential-cache--daemon` stays in the
  group of the git that started it, so cancelling the operation that happened to
  start it ends the cache too, and the next operation prompts again.
- **A child reading the terminal.** When Cairn was started from a terminal, a
  child in its own group that opens `/dev/tty` and reads is stopped rather than
  refused. The askpass settings exist to stop exactly those reads; anything else
  that tries waits until it is cancelled.
- **A crash.** A child cannot be told to die with its parent without `unsafe`, so
  if Cairn itself dies, its children run on. Each one's next write to a closed
  pipe ends it.
- **A reused group id.** An open pipe does not prove its holder is still in the
  group — it may have left it, or on macOS been handed the pipe by accident — and
  if every member exits between the check and the signal, the id can in principle
  be reused. Closing that window needs process handles std does not offer
  stably.

No invocation has a timeout. A `pre-commit` hook running a test suite and a push
of a large pack are both legitimately long; the window shows how long an
operation has run and offers its cancel instead.

## Failure

A failed invocation reports its arguments, its exit status and the retained tail
of its stderr, so "git failed" is never the whole message. A cancelled read is not
a cancelled write: a read reports that it was cancelled and nothing more, while a
cancelled write reports which lock files it left. When a write fails and a lock
file it needs is present, Cairn names the file — another git process, or a stale
lock — rather than retrying: git never waits on `index.lock`, and a retry Cairn
invented could re-run a mutation the user confirmed once.

## Lifecycle

Cairn finds `git` and checks its version once, at startup, and every thread that
runs `git` holds a copy of that answer. Each open repository keeps a registry of
its running invocations. Closing a repository, or the window, ends every one of
them the way a cancel does and waits a bounded time for their reaps.

## The command log

Every finished invocation is recorded, once, however it ended: its arguments, its working directory,
when it started, how long it ran, how it ended, whether it was cancelled, and its
retained stderr. The log is bounded and lives in memory. It never records the
environment, so the askpass token never enters it, and it never holds a secret.
It is plain data in `cairn-model`, for a view that shows the user what Cairn ran
on their behalf, the way Fork's does. It is kept without a logging crate: it is
state the application owns, not a diagnostic stream.

## Platforms

Process groups and signals are Unix: Linux, and macOS, which supports both the
same way (`platform.md`). Windows is not a goal, so the signal path is not
abstracted for it.

Evidence for every rule here: `docs/research/process-manager/` —
`platform-and-git-behaviour.md` (the std, nix and git behaviour each rule rests
on, verified against Rust 1.97.1, nix 0.31.3 and git 2.56.0),
`precedent-study.md` (how other clients run git), `consumer-invocations.md`
(every invocation the daily loop needs) and `runner-and-worker-as-built.md`.
Spec: `docs/prd/process-manager.md`.
