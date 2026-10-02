# Git processes

How Cairn builds and runs a `git` process today, and what keeps that in one
place. As-built: everything here is code that exists, with the test that pins
each rule named beside it. The commitment it is being built against is
`docs/prd/process-manager.md` (in flight); the intent is
`docs/design/processes.md`, with D1 in `docs/design/engine.md`. What the askpass
helper does with the environment described here, and fetch end to end, are
`docs/systems/credentials.md`.

**What exists:** one crate-private module that builds every `git` process, an
invocation typed as a read or a write, the two environments those kinds get,
and the runner: each process leads its own process group, each pipe it uses has
a thread, stdout is handed over as it arrives or collected under a ceiling,
stderr is kept as a bounded tail, and an invocation ends by a cancel signal, a
kill handle or a drop — `SIGTERM` to the group, then `SIGKILL` after two
seconds. Beside it are the two older paths the credential-prompts packet built,
`GitCommand::run` and `GitCommand::stream`; the version probe and fetch still
run on them, and they signal `git` alone. The registry and the command log are
not built yet.

## The layout

```
crates/cairn-git/src/
  process/      crate-private; the only place a process is built, spawned, waited on or read
    binary.rs       GitBinary, GitVersion — discovery, the 2.30 floor, the read and write builders
    environment.rs  GitEnvironment — the base, ALWAYS, INHERITED, READ_ONLY, Profile
    askpass.rs      Askpass — where git and ssh are sent for a secret
    cli.rs          GitCommand<K>, Read, Write, Kind, GitDirs, Output; `start` (the runner's entry),
                    and the older `run`, `stream`, Running, ProcessKill
    runner.rs       Invocation<K>: driving a started process to its end, and its outcome
    group.rs        the process group: when it may be signalled, ending it, KillHandle
    pipes.rs        the pipe threads: stdout chunks, stderr lines, stdin; the tail, the records
    stub_git.rs     (tests) a stub `git` on a PATH the test controls
  ops/          every mutation; constructs WriteAuthority; re-exports what the app needs
    authority.rs    WriteAuthority, and the tests that need one (real `git` writes among them)
    fetch.rs        fetch, built as a write
    stranded_locks.rs  every `*.lock` under a git directory; the runner reports them for a write
  reads/        each read `git` answers, one named function each — empty until diff-engine
```

`process` is a private module (`mod process;` in `lib.rs`). The application
reaches `GitBinary`, `GitVersion`, `GitEnvironment` and `Askpass` through
`cairn_git::ops`, which re-exports them because the application owns startup
and the helper's channel. It cannot reach the runner: nothing in it is `pub`.

## The seal

An invocation is a `GitCommand<'_, K>`, and `K` says what kind:

- **`GitBinary::read_invocation()`** builds a `GitCommand<'_, Read>`. `Read` is a
  unit struct, so a read has no field that could hold an askpass token.
- **`GitBinary::write_invocation(WriteAuthority)`** builds a
  `GitCommand<'_, Write>`. `Write` keeps the authority for the invocation's life,
  plus the token when `authorized_by` is called — a method that exists on a
  write and nowhere else.
- **`WriteAuthority`** (`ops/authority.rs`) is `pub(crate)`, with a private field
  and one constructor, `pub(in crate::ops) fn new()`. So the compiler refuses a
  write anywhere in the crate but `ops/`: in `reads/`, in `history/`, and in
  `process/` itself, which cannot even build a `Write` without one. It is a
  token rather than a visibility on the builder because `process/` cannot write
  `pub(in crate::ops)` on an item of its own: a restricted visibility must name
  an ancestor of the item.
- **`GitCommand::new`** and **`GitEnvironment::command`** are `pub(super)`:
  visible to `process/` alone. The version probe is the one invocation built
  outside the two builders. It runs before there is a `GitBinary`, and it is a
  read.

Fetch is built as a write (`ops/fetch.rs`). The probe is built as a read
(`process/binary.rs`).

From outside the crate nothing is nameable. The `compile_fail` doctests in
`crates/cairn-git/src/ops/mod.rs` each add one line to a passing scaffold:
naming a `WriteAuthority`, calling its constructor, calling either builder, and
calling `GitEnvironment::command`. Every argument is `unreachable!()`, so no
block can fail on its arguments. Stable `rustdoc` checks that a block fails,
not why, so each was checked by hand, by turning it into a plain block: each
fails on privacy alone (E0603 for the type, E0624 for the methods). With a
builder or `command` widened to `pub`, its block still fails on privacy, of
the crate-private type it returns or takes (`GitCommand`, `Profile`). So the
doctests decide that the public surface offers no way in. The methods' own
visibility is pinned by the guard, which also requires each refused block to
be exactly the scaffold plus its line.

## The environment

`GitEnvironment::new(parent, &Askpass)` builds the **base** every invocation
gets:

- the `INHERITED` roster, copied from the parent when present;
- the `ALWAYS` table: `GIT_TERMINAL_PROMPT=0`, `SSH_ASKPASS_REQUIRE=force`,
  `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false`;
- `GIT_ASKPASS` and `SSH_ASKPASS` naming the helper;
- `CAIRN_ASKPASS_SOCKET` when the `Askpass` names a socket.

`GitEnvironment::command(program, Profile)` is the only place a
`std::process::Command` is built. It clears the inherited environment, applies
the base, and then applies what the invocation's kind adds:

- **A read** (`Profile::Read`) adds the `READ_ONLY` table, `GIT_OPTIONAL_LOCKS=0`,
  and never a token, because the variant has no field for one.
- **A write** (`Profile::Write { token }`) adds `CAIRN_ASKPASS_TOKEN` when the
  operation was given a token, and nothing else.

The kind chooses the profile (`Kind::profile` in `cli.rs`). The caller never
does.

Why each variable is there, with its evidence, is beside it in
`environment.rs`. Two need a word here:

- **The editor is pinned to `false`, not `:`.** A verb that wants an editor fails
  instead of waiting on one nobody can see. `:` would silently accept whatever
  message git proposed. Both editor variables are set, because a user's
  `sequence.editor` outranks `GIT_EDITOR` for the rebase todo list.
- **`GIT_OPTIONAL_LOCKS=0` covers `git status` and nothing else.** Porcelain
  `diff` and `describe --dirty` refresh the index anyway. That is why a read in
  `reads/` runs query plumbing or `status` only, as the module's own docs say where
  `diff-engine` will read them.

Pinned:

- **Variable by variable, from the builder.** `environment.rs`:
  `the_environment_is_exactly_the_deliberate_entries`,
  `a_read_is_the_base_with_optional_locks_off_and_no_token`,
  `a_write_is_the_base_with_its_token_only_when_given` and
  `the_always_table_wins_whatever_the_parent_says`.
- **From what a stub `git` printed.** Each test spells out the exact set the stub
  must have seen:
  - a read: `a_read_sees_exactly_the_read_environment_and_nothing_inherited`;
  - the version probe: `the_version_probe_runs_with_the_read_environment`;
  - a write: `a_write_sees_exactly_the_write_environment_and_nothing_inherited`;
  - a write with its token: `an_authorised_write_carries_its_token_and_only_that_one`;
  - fetch, with and without a token: `a_fetch_runs_with_the_write_environment_and_its_token`.
- **The effect, against real `git`** (`ops/authority.rs`):
  - `a_status_read_leaves_a_stale_index_byte_identical`. The index's stat
    information is stale and the working tree is dirty. The read leaves the
    index byte for byte as it was, and the same `status` run as a write
    rewrites it — the control that proves the fixture was stale.
  - `a_commit_without_a_message_fails_promptly_instead_of_opening_an_editor`
    and `an_interactive_rebase_fails_promptly_instead_of_opening_the_sequence_editor`.
    Each configures an editor that records it ran and then hangs. Each verb
    fails within the deadline, the editor never runs, and `HEAD` does not move.

## The runner

`GitCommand::start()` spawns the process and hands back an `Invocation<K>`,
still running. The caller drives it on its own thread with one of three calls,
each taking the cancel signal it polls:

- `finish(cancel, stdout, progress)` hands stdout to a closure as it arrives;
- `records(cancel, record, progress)` splits stdout into the NUL-terminated
  records of a `-z` format and hands each on as soon as it is whole;
- `collect(cancel, ceiling, progress)` returns the whole of stdout, or
  `Error::GitOutputTooLarge { ceiling }` once git writes more — the process is
  ended at once and nothing it wrote is returned.

`GitCommand::input(bytes)` gives the process those bytes on stdin, written and
then closed; without it stdin is `/dev/null`. `Invocation::kill_handle()` hands
out a `KillHandle`. Each call returns an `Output` holding the retained stderr
(and stdout, for `collect`). On an `Err` from `records`, the records already
handed on are a prefix of an answer that did not complete, for the caller to
discard; a last record without its NUL is handed on only on success
(`a_cancelled_read_hands_on_no_partial_last_record`).

`start` runs on the caller's thread and may wait — it spawns, and a pipe thread
that cannot start makes it end the process there — so it is a worker's call.
What the UI thread may call is `KillHandle::kill` and the drop, neither of
which waits.

### Spawning and the pipes

- **Its own process group.** `start` sets `CommandExt::process_group(0)`, so
  the process leads a new group whose id is its pid, and everything git starts
  — hooks, `ssh`, helpers — is in it. Pinned by
  `every_invocation_leads_a_process_group_of_its_own`, which reads the stub's
  group from `/proc` and finds the stub and its `sleep` in it, and this test
  process not.
- **A thread per pipe** (`pipes.rs`): `cairn-git-stdout`, `cairn-git-stderr`,
  and `cairn-git-stdin` when there is input. No thread reads one pipe while
  another is read or written on it, so no output volume on one stream can stall
  another at any pipe capacity — 64 KiB, two pages at a user's pipe limit, or
  512 bytes on macOS. Readers send `Event`s to the driving thread on a channel
  bounded at `EVENTS_BOUND`, so a caller slower than git holds git back rather
  than buffering without limit. Pinned by
  `five_mib_of_records_beside_continuous_stderr_arrive_whole_and_in_order`
  (G6's volume of records while another member of the group writes many
  pipes' worth of stderr) and
  `sixty_four_mib_of_stdin_beside_busy_stdout_and_stderr_completes`; each runs
  under a deadline, so a deadlock fails rather than hangs. With real `git`:
  `sixty_four_mib_through_hash_object_gives_gits_own_id` (end of input, and the
  id gitoxide computes for the same bytes) and
  `a_real_read_of_over_five_mib_of_records_arrives_whole_and_in_order`
  (a generated tree, made by `mktree` fed on stdin and read back by
  `ls-tree -z`).
- **stdin first, and held until its thread runs.** The writer thread starts
  before the readers, and the pipe stays with the runner until that thread has
  started: a thread that cannot start would otherwise drop it, and git would
  read the end of an EMPTY input as the whole of it —
  `reset --pathspec-from-file=-` given nothing unstages everything. On that
  failure the process is ended first and the pipe dropped after
  (`a_stdin_thread_that_cannot_start_ends_the_process_before_its_input_closes`).
  A write failure other than git closing its end (`EPIPE`, git's own choice)
  likewise ends the process BEFORE the pipe closes, and the outcome is
  `Error::GitUnwatched`
  (`a_failed_stdin_write_ends_the_process_and_a_closed_stdin_does_not`,
  `a_failed_stdin_write_signals_before_it_closes_the_pipe`).
- **A thread that cannot start** is `Error::GitUnwatched`, and the process is
  ended and reaped on the calling thread first, never left running with nobody
  waiting on it (`a_thread_that_cannot_start_ends_and_reaps_the_process`, with
  a thread starter that fails at the stdout and the stderr reader).
- **stderr** is split into lines at `\r` and `\n`, and each read's lines cross
  to the driver as one event, so what git wrote before it exited is a handful
  of events however many lines it was. Every non-blank line goes to `progress`,
  and the last `TAIL_BYTES` (256 KiB) is retained, starting at a line's start
  where one falls inside the window; while the process runs the tail holds at
  most twice that and the line being added, because it cuts in batches. A line
  with no terminator is cut into pieces of that size. Pinned by
  `a_mib_of_stderr_is_forwarded_whole_and_retained_as_a_bounded_tail`,
  `a_final_burst_of_stderr_is_kept_whole_though_a_holder_keeps_the_pipe` (a
  burst written just before git fails, with a slow progress callback and a pipe
  still held: its last line arrives) and the unit tests in `pipes.rs`.
- **Thread hygiene.** Every thread is counted while it runs;
  `every_thread_an_invocation_starts_ends_with_it` sees the three names started
  and the count back at zero once the invocation is over. The one thread that
  may outlive its invocation is a reader left on a pipe that a process git left
  behind still holds; it ends when that process lets the pipe go, or at its
  next send, since the receiver is gone, which closes the pipe on a writer
  that keeps writing
  (`a_reader_left_on_a_pipe_still_written_ends_when_its_invocation_lets_go`).
  A stdin writer whose reader is such a process and never reads is the same
  case.

### When it is over

`Driver::run` (`runner.rs`) ends the invocation when its leader has been reaped
and its pipes are drained — or `DRAIN_BOUND` (250 ms) after the exit, when
something else still holds a pipe.

- **The common case costs no tick.** git exiting closes its pipes, the readers
  end, the channel disconnects, and the driver reaps with `try_wait`, backing
  off from 50 µs to `TICK` (20 ms) because the kernel makes the process a
  zombie a moment after its files close.
- **A pipe still held** after git exits is noticed on the tick, which runs
  `try_wait` while the pipes are open. The driver then waits for what is
  already queued, and no longer than `DRAIN_BOUND` from the exit, the tick
  included. Pinned by
  `an_exit_with_a_grandchild_holding_the_pipes_returns_within_the_drain_bound`:
  the stub leaves a `sleep` holding both pipes, its last output still arrives,
  the return comes within the bound of the stub's last line and not before the
  drain, and the `sleep` — never cancelled — is left alone.

The values are pinned by `the_fixed_bounds_have_the_values_the_packet_recorded`:
the 2 s grace, the 256 KiB tail, the 250 ms drain bound.

**What the runner costs** (PRD G19): `git diff-tree -r -M -z --raw` on the
rust-lang/rust bench commit `5a3292f163d` against its first parent, through a
read invocation and through a bare `std::process::Command`, release build,
warm, on an AMD Ryzen 7 9800X3D with git 2.56.0: within noise of each other,
0.38 ms apart at the median in one run and none in another, against the 2 ms
the criterion allows; on the empty-diff floor, about 12 µs. The reporter is
`g19_reports_the_runners_overhead_over_a_bare_command`, `#[ignore]`d and driven
by `CAIRN_BENCH_REPO`.

### Cancelling

Three ways, all ending the process the same way:

- **The cancel signal** passed to `finish`, `records` or `collect`, polled every
  tick (`a_superseded_cancel_signal_ends_the_whole_group_and_reaps_it`).
- **A `KillHandle`** (`group.rs`): `Send`, `Clone`, and never blocking. It
  records the request, then only TRIES for the lock; a miss is not a lost
  cancel, because the driving thread sends the signal itself when it sees the
  request (`a_kill_handle_ends_the_whole_group_and_reaps_it`,
  `a_kill_that_misses_the_lock_returns_at_once_and_the_driver_finishes_it`,
  `the_kill_handle_is_send_and_clone`).
- **Dropping** an unfinished invocation asks for the end without waiting
  (`SIGTERM` if the lock is free) and drives it to the reap on a
  `cairn-git-reaper` thread
  (`dropping_an_unfinished_invocation_ends_and_reaps_its_group_without_blocking`
  times the drop and watches the group go, and
  `dropping_never_waits_for_a_process_that_outlasts_sigterm` times it against a
  process that takes the whole grace to end). If even that thread cannot start,
  the group is sent `SIGKILL` at once and the leader reaped if it already can
  be; one that outlives that is a zombie until Cairn exits, and a write's
  locks a `SIGKILL` strands go unreported, since nothing is left to report
  them (`a_drop_with_no_thread_to_reap_on_kills_the_group_at_once`). The same
  hand-off happens when a caller's callback panics: the driver, unwinding,
  gives the invocation to a reaper
  (`a_panicking_callback_still_ends_and_reaps_the_process`).

**Ending** is `SIGTERM` to the group (`killpg`), then `SIGKILL` to it once
`TERMINATION_GRACE` (2 s) has passed. A cancelled invocation waits for its
group: the leader reaped and its pipes closed, within the grace plus a tick.
When a pipe's holder has left the group, neither signal reaches it, and the
pipes are abandoned `DRAIN_BOUND` after the `SIGKILL`, so that cancel takes the
grace plus the bound
(`a_cancel_whose_pipe_holder_left_the_group_returns_after_the_grace_and_the_bound`).
A cancel that arrives after the leader has exited while a pipe is still held
signals the group too — whatever is left of what git started — and can take
the same; an uncancelled invocation in that state waits only the bound.
Pinned by `a_cancel_sends_sigterm_first_and_a_process_that_acts_on_it_is_not_killed`
(a trap that reports `SIGTERM`, ending inside the grace) and
`a_process_that_ignores_sigterm_is_killed_after_the_grace`; each G8 test then
reads `/proc` and finds no member of the group alive or unreaped. With real
`git`, `a_commit_cancelled_inside_a_sleeping_hook_leaves_no_index_lock` cancels
a `commit -a` while its `pre-commit` hook sleeps, having seen `index.lock` held,
and finds the lock gone, the hook ended and nothing committed.

**When the group may be signalled.** Only while a member is believed alive —
the leader not yet reaped, or one of the pipes the readers hold still open —
and never once the invocation is over. The check and the signal are made under
the one lock the reap also takes, and the open pipes are read after that reap,
so nothing signals a reaped pid unless an open pipe still names the group
(`a_kill_after_the_invocation_is_over_signals_nothing`; and after a drain-bound
return, with the leftover process still holding the pipes, the G10 test's kill
signals nothing). A group id is not reused while any member lives, which
closes all but a stated race: an open pipe does not prove its holder is still
in the group, and if every member exits between the check and the signal the
id could in principle be reused. The escalation widens that window: once the
leader is reaped and only an open pipe keeps the group believed alive, the
`SIGKILL` goes out up to the whole grace after the leader's pid was freed. One
more window is stated rather than closed: git may exit 0 between the reap's
`try_wait` and the `killpg`, and is then counted as signalled while running —
a completed invocation reported as cancelled.

### Outcomes

`Invocation::drive` decides, after the reap, in this order:

1. A crossed ceiling is `Error::GitOutputTooLarge { arguments, ceiling,
   stranded_locks }`
   (`a_collect_over_its_ceiling_is_refused_whole_and_the_process_ended`;
   `output_exactly_at_the_ceiling_is_collected_and_one_byte_over_is_refused`).
2. A failed stdin write, a pipe thread that could not start, or a `try_wait`
   that itself failed is `Error::GitUnwatched { arguments, source,
   stranded_locks }`. In the first two the process was ended; in the last,
   what became of it is not known.
   For a write, both list the lock files present after the reap, as a
   cancelled write does; for a read, nothing.
3. A cancel that lost the race to a clean exit — status 0 with no signal sent
   while the leader was running — is the success it was
   (`a_cancel_after_a_clean_exit_is_reported_as_success`). Any other invocation
   asked to end is cancelled, whatever its status
   (`a_cancelled_process_that_exits_zero_after_the_signal_is_reported_cancelled`,
   `a_cancelled_process_that_exits_nonzero_is_reported_cancelled`). What that
   means is the kind's (`Kind::cancelled` in `cli.rs`):
   - **a read** is `Error::GitReadCancelled { arguments }` and nothing more;
   - **a write** is `Error::GitCancelled { arguments, stranded_locks }`, the
     `*.lock` files under its git and common directories (recorded by
     `in_repository`) listed after the reap, so what git removed on its way out
     is not reported (`a_cancelled_write_lists_the_locks_present_after_the_reap`:
     a stale lock is listed and the `index.lock` git held is not;
     `a_cancelled_write_lists_its_locks_only_once_it_is_reaped`: a process that
     takes 300 ms to remove its lock on `SIGTERM` has it not listed). The
     listing is what was there at that moment. A cancelled write may still
     have taken effect, in part or whole — the signal can land after git made
     its change and before it exited — so an operation that must know
     compares the repository's state before and after, as fetch does with its
     refs.
4. A non-zero exit is `Error::GitFailed { arguments, status, stderr,
   present_locks }`, `stderr` the retained tail; for a write, `present_locks`
   lists the lock files present, which is what a write fails on and git never
   waits for (`a_failed_write_names_a_present_index_lock`). Nothing retries,
   and nothing removes a lock. A read's is always empty
   (`a_failure_carries_the_arguments_the_status_and_stderr`,
   `a_real_read_fails_with_its_diagnostic_and_cancels_as_a_read`).

## The guards

All in `crates/cairn-guards/tests/invariants.rs`. Each asserts a nonzero file
count, and each has a matcher self-test:

| Twin | What it decides |
| --- | --- |
| `every_git_invocation_disables_the_terminal_prompt` | Outside `process/environment.rs`, no product file names or builds a `Command`, sets a process environment variable, or builds or implements `GitEnvironment`. Inside it: one `Command`, one literal, `env_clear` and `envs`, and the `ALWAYS` table's four pins. The `READ_ONLY` table carries `GIT_OPTIONAL_LOCKS=0` and is applied. The askpass names are set. |
| `only_the_process_module_builds_or_runs_a_process` | Outside `process/`, no product file names `Stdio`, `Child` or its pipes, `CommandExt` or `nix`. It calls none of `.spawn()`, `.output()`, `.status()`, `.wait()`, `.try_wait()` or `.wait_with_output()`, and does not call `GitEnvironment::command`. `GitEnvironment::command` stays `pub(super)`. `process/` itself must show `.spawn()`, `.output()`, `.wait()`, `.try_wait()`, `Stdio`, `Child`, `ChildStderr`, `nix` and `.command(..)`, so the matcher is proven to read real code. The one exception row, `.status()` in `cairn-app` (`HistoryProgress::status`), fails once it is no longer needed. |
| `the_runner_is_named_only_by_ops_and_reads` | In `crates/cairn-git/src`, the runner's names (`GitCommand`, `read_invocation`, `Running`, `ProcessKill`, `Invocation`, `KillHandle`) are allowed in `process/`, `ops/` and `reads/` only. The write builder and `WriteAuthority` are allowed in `process/` and `ops/`. Constructing, writing a literal of or implementing `WriteAuthority` is allowed in `ops/` only. Nothing is declared or re-exported `pub`, and `process` stays private. The authority keeps its shape, and the doctests stay. |
| `only_the_ops_module_mutates_a_repository` | No product file outside `ops/` and `process/` spawns `git` by its literal name. No file of `crates/cairn-git/src` outside `ops/` names gitoxide's mutation API. That roster was enumerated from the vendored gix 0.87.1 source and sits, with each entry's file and line, in `crates/cairn-guards/src/lib.rs`. |

`the_unguarded_routes_to_a_process_now_fail_a_twin` pins the routes that
`docs/research/process-manager/runner-and-worker-as-built.md` section 3 found
open, each now failing a twin:

- `git.command().run()`, under its new name and its old one;
- `.stream()` with its kill;
- `GitCommand::new`;
- `git.environment().command(..).output()`;
- a `reads/` file constructing a `WriteAuthority`.

What the guards cannot decide is stated in the root `CLAUDE.md` beside each
invariant. That a read runs query plumbing or `status` is
`destructive-ops-reviewer`'s check 10. These are `qa-checklist`'s item 7:

- a process or a gix write reached through an alias, a trait object or a macro;
- a built or started invocation (`GitCommand`, `Running`, `Invocation`) or a
  kill handle (`ProcessKill`, `KillHandle`) handed out of `ops/` or `reads/` and driven
  elsewhere by inference. The runner guard reads names, so whether `ops/` and
  `reads/` hand out only named operation types (as fetch does) is review.
