# Progress — process-manager

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-10-02 — phase 02: the runner

Packet mode, committed directly to `feature/process-manager`.

**What landed** (`6297fc5`; test and doc follow-ups `c68c082`, `c1d9d8a`,
`bf17825`; QA fixes `a5621a4` and the docs commit after it):
- `GitCommand::start()` / `input(bytes)` and `Invocation<K>` in
  `process/runner.rs`, `group.rs`, `pipes.rs`. Every invocation leads a new
  process group; stdin, stdout and stderr each get a thread; stdout streams as
  bytes (`finish`) or NUL records (`records`) or collects under a ceiling that
  errors (`collect`); stderr is forwarded line by line and a 256 KiB tail kept.
- Cancellation by cancel signal (polled each 20 ms tick), by `KillHandle`
  (`Send + Sync + Clone`, try-lock only) and by drop (a reaper thread). Ending
  is `killpg` SIGTERM, then SIGKILL after 2 s, only while a member is believed
  alive and never after the invocation is over.
- Outcomes typed by kind: `GitReadCancelled` for a read; `GitCancelled` with
  locks listed after the reap for a write; `GitFailed` gains `present_locks`;
  new `GitOutputTooLarge` and `GitUnwatched`.
- `RUNNER_NAMES` gains `Invocation` and `KillHandle`, with self-test cases.
- The old `run`/`stream` paths are untouched; fetch and the probe move in
  phase 03.

**Acceptance (G6–G13), each with a test that a named mutation turns red:**
- G6: `five_mib_of_records_beside_continuous_stderr_arrive_whole_and_in_order`
  (drain the pipes one after the other → deadlock; lose a record at a chunk
  boundary), `a_real_read_of_over_five_mib_of_records_arrives_whole_and_in_order`,
  `a_collect_over_its_ceiling_is_refused_whole_and_the_process_ended` (truncate
  and succeed; not end the process on the crossing),
  `output_exactly_at_the_ceiling_is_collected_and_one_byte_over_is_refused`
  (`>=` for `>`).
- G7: `sixty_four_mib_of_stdin_beside_busy_stdout_and_stderr_completes` (stdin
  written on the driving thread → deadlock),
  `sixty_four_mib_through_hash_object_gives_gits_own_id` (stdin never closed).
- G8: `a_kill_handle_ends_the_whole_group_and_reaps_it`,
  `a_superseded_cancel_signal_ends_the_whole_group_and_reaps_it`,
  `dropping_an_unfinished_invocation_ends_and_reaps_its_group_without_blocking`,
  `dropping_never_waits_for_a_process_that_outlasts_sigterm` (signal the leader
  alone; drop `process_group(0)`; end on the dropping thread).
- G9: `a_cancel_sends_sigterm_first_and_a_process_that_acts_on_it_is_not_killed`
  (SIGKILL first), `a_process_that_ignores_sigterm_is_killed_after_the_grace`
  (no escalation), `a_commit_cancelled_inside_a_sleeping_hook_leaves_no_index_lock`
  (SIGKILL first; signal git alone).
- G10: `an_exit_with_a_grandchild_holding_the_pipes_returns_within_the_drain_bound`
  (no tick-time exit check; drain 0; drain +100 ms; `DRAIN_BOUND` 5 s; no
  `over` flag).
- G11: `a_failure_carries_the_arguments_the_status_and_stderr`,
  `a_real_read_fails_with_its_diagnostic_and_cancels_as_a_read` (a read
  cancelled as a write), `a_cancelled_write_lists_its_locks_only_once_it_is_reaped`
  (list the locks before the reap), `a_cancelled_write_lists_the_locks_present_after_the_reap`,
  `a_failed_write_names_a_present_index_lock`.
- G12: `a_cancel_after_a_clean_exit_is_reported_as_success` (decide by the
  flag), `a_cancelled_process_that_exits_zero_after_the_signal_is_reported_cancelled`
  (decide by the status), `a_cancelled_process_that_exits_nonzero_is_reported_cancelled`.
- G13: `a_mib_of_stderr_is_forwarded_whole_and_retained_as_a_bounded_tail` and
  `the_tail_holds_a_bounded_amount_while_stderr_runs_on` (retain everything;
  cut only at the end); `the_fixed_bounds_have_the_values_the_packet_recorded`
  pins 2 s, 256 KiB and 250 ms.

Mutation runs (`process::` and `ops::authority` suites, each reverted): 18 by
the implementer before QA (one survivor, a blocking drop, fixed with a new
test), 17 by the test-coverage auditor (11 survived), and 20 after the QA fixes
covering every survivor plus the new behaviour — all 20 killed.

**G19** (`g19_reports_the_runners_overhead_over_a_bare_command`, release,
`CAIRN_BENCH_REPO=~/Development/bench/rust` at `c999cef531e`, warm): AMD Ryzen
7 9800X3D, 16 threads, 60 GB, Linux 7.2.8-2-cachyos, git 2.56.0. 40 runs each,
`diff-tree -r -M -z --raw 5a3292f163d^1 5a3292f163d` (M1 against its first
parent; the bare commit, a merge, prints nothing), 554,356 bytes:
- run 1: runner median 82.94 ms, bare `Command` 82.56 ms — overhead 0.38 ms;
- run 2: runner 83.71 ms, bare 83.87 ms — overhead 0 (median and min); the
  empty-diff floor 2.131 ms vs 2.118 ms — 12 µs;
- the responsiveness reviewer's own run: overhead 0 at the median, 110 µs at
  the min. All within the 2 ms G19 allows.

**Flakes:** the known `a_refspec_that_writes_local_branches_is_refused_before_git_runs`
ETXTBSY flake did not hit in any run this phase. One new one did, once, under
the full gate's load: a stub's `SIGTERM` trap printed "No such process" when
the group signal had already ended its child; fixed in `bf17825`.

**QA:** qa-checklist, destructive-ops-reviewer, responsiveness-reviewer,
test-coverage-auditor and gate-integrity-reviewer ran fresh over
`a5f5160..HEAD` (and `main...HEAD`); each report was received in full (four
were re-asked after hitting their turn limits). A fresh qa-confirm adjudicated
51 raw findings into 43: 34 confirmed, 6 dismissed, 3 escalated.

Confirmed and fixed (`a5621a4` and the docs commit):
- **DO1** — a stdin thread that could not start dropped the pipe before the
  process was ended; git reads an empty input as "everything" for
  `--pathspec-from-file` (reproduced: `reset` unstaged all). The pipe is now
  held until the thread runs, and the process ended before it is dropped.
- **RS1** — stderr crossed one event per line, so a final burst with a pipe
  holder and a slow callback lost its end. Now one event per read.
- **QC7/DO10** — a panicking callback left git unwatched. The driver now hands
  an unconcluded invocation to a reaper from its own drop.
- **QC9/DO7** — the no-thread drop fallback only tried for the lock; it now
  takes it (every hold is a non-blocking syscall) and the comment says so.
- **DO5** — the open-pipe count is read after the reap, inside `signal()`.
- **DO3** — runner-ended writes (ceiling, failed stdin, failed thread start)
  carry the locks present after the reap; the `GitUnwatched` doc is honest
  about a failed `try_wait`.
- **DO2/QC10** (docs half) — `GitCancelled`, the runner docs and
  git-processes.md say a cancelled write may have taken effect; the
  `try_wait`/`killpg` window is stated.
- **DO6** (option c, docs) — the 2 s widening of the reuse race on escalation
  is stated in `group.rs` and git-processes.md.
- **RS2** (option a, docs) — a cancel after the exit with a pipe held takes the
  grace plus the bound; stated in the runner docs and git-processes.md.
- Tests: TC1 (lock ordering, new stub test), TC2 (stderr volume), TC3 (ceiling
  stub hangs), TC4 (G10 timed from the exit, literal bound), TC5 (holder that
  left the group), TC6/TC10 (literal values), TC7 (tail held while running),
  TC8/QC4/DO8 (`over`), TC9 (stdin thread failure), TC11 (ceiling boundary),
  TC12 (partial record on cancel), TC13 (reader stops when let go), TC14 (whole
  64 MiB compared), TC15 (`eventually` for init's reap), DO9 (signal before
  close), DO11, DO12, RS6 (start is a worker's call, in the module docs).
- Docs: GI1/QC8 (the runner hand-off residual now names a started invocation
  and the kill handles, in CLAUDE.md, the qa-checklist agent and
  git-processes.md), QC1/RS7 (G19 stated in the systems doc, no pointer into
  `work/`), QC2, QC11, QC13.

Confirmed and deferred, recorded in `state.md` → "Obligations for later
phases": QC3 and QC6 and GI2 (phase 03), TC16 (phase 05).

Dismissed, with qa-confirm's reasons:
- **GI3** — commit `6297fc5` has a scope and a body; the roster change is the
  twin its new names require, in the same commit.
- **RS3** — `Records` holds only the record in progress, which the caller
  needs whole anyway; a non-`-z` stdout given to `records` is a caller bug.
- **RS4** — not feeding stdout once an end is asked for would break R4.5: a
  cancel that loses to a clean exit must deliver the whole answer.
- **RS5** — a cheaper lock check would parse translated stderr (R2.4) or miss
  ref locks R5.3 must name; the walk runs only on a failed write, on a worker.
- **RS6** — cairn-git is synchronous by stated architecture and `start` is
  unreachable from cairn-app; noted in the module docs anyway.
- **QC12** — a `\r\n` blank line is never forwarded and is trimmed from the
  tail; a 256 KiB cut is lossy by design and respects char boundaries.

**Escalated to the user** (current behaviour kept, spec-conformant; batched in
the phase report):
- **RS2** — a cancel that arrives after the leader exited, while a pipe is
  still held, signals the leftovers and waits the 2 s grace plus the bound,
  where an uncancelled run waits 250 ms (R3.6 vs R4.4). Options: (a) keep and
  state it (done); (b) SIGTERM the leftovers but keep the exit + bound
  deadline; (c) send nothing once the leader was already reaped.
- **DO4** — a git that failed on its own before any signal is reported
  cancelled if a cancel request lands before the invocation concludes, losing
  its stderr and locks. Options: (a) keep, per R4.5's letter (current);
  (b) cancelled only when a signal reached the running leader; (c) cancelled,
  carrying the failure's status and stderr.
- **DO6** — once the leader is reaped and only an open pipe remains, the
  SIGKILL goes out up to 2 s after its pid was freed. Options: (a) don't reap
  until conclude (`waitid(WNOWAIT)`: not on Apple in nix 0.31, and against the
  "never nix-waitpid a pid std owns" rule); (b) skip the SIGKILL once only pipe
  evidence remains; (c) keep and state it (done).
- Policy halves of DO2 (make a before/after comparison a stated review
  obligation for every write verb; reword the cancelled message) and DO3
  (whether a crossed ceiling should outrank a write's clean exit, or ceilings
  be forbidden on writes).

## 2026-10-02 — phase 01: the seal and the environment

Packet mode, committed directly to `feature/process-manager`.

**What landed** (`f1d1d5b`, `73c9dd4`; QA fixes `ea52ef7`, `e52abf4`):
- `GitBinary`, `GitEnvironment`, `Askpass` and the runner moved from `ops/` into
  the crate-private `process/`, unchanged in behaviour.
- Invocations are now `GitCommand<'_, Read>` or `GitCommand<'_, Write>`, and a
  write consumes a `WriteAuthority` that only `ops/` can construct. Fetch is a
  write. The version probe is a read.
- `ALWAYS` pins `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false`. A read adds
  `GIT_OPTIONAL_LOCKS=0` and has nowhere to carry a token.
- `reads/` exists, empty and documented.
- Three guard twins are new or rewritten:
  - `only_the_process_module_builds_or_runs_a_process` (new);
  - `the_runner_is_named_only_by_ops_and_reads` (new);
  - `only_the_ops_module_mutates_a_repository`, which gains a gitoxide mutation
    roster enumerated from vendored gix 0.87.1, with file:line per entry.
- The environment twin moved to `process/environment.rs` and gained the new
  pins.
- `docs/systems/git-processes.md` is new.

**Acceptance and the tests that pin each criterion:**
- G1: the compile_fail doctests in `ops/mod.rs`.
- G2: the twins above and their matcher self-tests, plus
  `the_unguarded_routes_to_a_process_now_fail_a_twin`.
- G3: the builder tests in `environment.rs`, and the stub-git tests in
  `process/cli.rs`, `ops/authority.rs` and `ops/fetch.rs`.
- G4: `a_status_read_leaves_a_stale_index_byte_identical`.
- G5: `a_commit_without_a_message_fails_promptly_instead_of_opening_an_editor`,
  plus its `rebase -i` twin.

Removing each environment pin turns its own G3, G4 or G5 test red.
GIT_EDITOR=false broke no fetch test, so the stopping rule did not fire.

**Checks run by hand:**
- The four routes found unguarded in `runner-and-worker-as-built.md` section 3,
  a `reads/` file constructing a `WriteAuthority`, and a gix index write were
  each written as real scratch files. Every one failed the guard suite.
- Every compile_fail doctest was turned into a plain block. Each fails on
  privacy alone, and still does with its method widened to `pub`, because the
  types they take or return are crate-private.

**QA:** qa-checklist, gate-integrity-reviewer, destructive-ops-reviewer and
test-coverage-auditor ran fresh over `main...HEAD`, and a fresh qa-confirm
adjudicated.

Confirmed and fixed:
- **Two doctests failed on their arguments.** The `write_invocation()` and
  `command(.., None)` doctests failed on arity and type, not privacy. Every
  argument is now `unreachable!()`.
- **Nothing pinned `command`'s visibility.** The guard now pins
  `GitEnvironment::command` as `pub(super)`.
- **The doctest pin was too loose.** It now requires each refused block to be
  exactly the scaffold plus its one line.
- **Roster entries could be dropped unnoticed** (`ProcessKill`, `ChildStdin`,
  `ChildStdout`). The self-test cases are now spelled out apart from the
  rosters, with a length check.
- **The "process/ must show these shapes" claim was overstated.** It now lists
  the shapes actually required, and `.wait()` and `ChildStderr` were added to
  them.
- **The guard scope was misstated.** "cairn-git" is now
  "crates/cairn-git/src".
- **`scanned` counted exempt files.** The `process/` files exempt from
  `spawns_git` no longer count.
- **CLAUDE.md's D1 bullet stated planned work as present.** It now says the
  changes query "will be the first".
- **"Plumbing" admitted writers.** It is now "query plumbing", the plumbing
  writers are named, and `ls-files`, `rev-parse` and `cat-file` are no longer
  claimed as backed by C3.
- **The G4 control differed from the read in more than profile.** It now
  differs in profile alone.
- **The runner hand-off residual was unstated.** A built invocation or
  `Running` handed out of `ops/` or `reads/` and driven elsewhere by inference
  is not seen by the guard. This is now stated in CLAUDE.md, git-processes.md
  and qa-checklist item 7, owned by qa-checklist. Raising it to a guard check
  is a possible future choice for the user.

Dismissed, with reasons:
- **C2:** an unsealed `Kind` inside `process/`. `Kind` and `Profile` cannot be
  named outside `process/`, which builds the `Command` by design. The "any verb"
  half is already check 10's stated residual.
- **D5:** line numbers in the gix roster comments. They cite an immutable
  published version that the comment names, so they cannot rot silently.
- **D6:** future editor verbs (revert, cherry-pick, amend, `tag -a`) will fail
  under `GIT_EDITOR=false`. That is the intended G5 behaviour. Those packets
  pass `-m`/`--no-edit`, and git's 'false' message should be translated in the
  UI.
- **D7:** `printed_environment` drops the shell's own variables. That is needed
  for an exact comparison, and `the_parent_is_asked_about_the_inherited_roster_and_nothing_else`
  pins inheritance.
- **E1:** gix sub-crate roster completeness. qa-confirm spot-checked gix-ref,
  gix-index, gix-odb, gix-pack, gix-lock, gix-tempfile, gix-fs, gix-worktree,
  gix-merge and gix-note, and found no missing disk writer.

Probed, then resolved: A2, the `read_invocation()` doctest, still fails when
widened. It fails on privacy of the return type `GitCommand<'_, Read>`, which is
honest. The method's own visibility is pinned by `declares_publicly`.

Escalated to the user: D3, the partial-clone lazy fetch. It is in `state.md`
under Open questions.

**Pending user review, not decided:**
- **The gitoxide guard's method-call bans.** `.write(`, `.write_to(`,
  `.write_stream(` and `.notes(` are banned outside `ops/` in
  `crates/cairn-git/src`.
  - Why: they are the only shapes that catch `open_index()?.write(..)`, a tree
    editor's `.write()`, and note writes.
  - Cost: non-ops engine code cannot call `io::Write::write` or
    `RwLock::write` (`write_all` is fine), and a notes read would have to live
    in `ops/`.
- **The `PROCESS_CALL_EXCEPTIONS` row** excusing `.status()` under
  `crates/cairn-app/src`, which is `HistoryProgress::status`. The row fails once
  it is no longer needed.

**Environment note:** /tmp (tmpfs) filled up during QA, about 30G of it from
other projects' Claude session directories, which were left alone. Running the
gate with TMPDIR pointing inside the worktree breaks the askpass tests: the
socket path exceeds SUN_LEN. The full gate was run once /tmp had room again.

## 2026-10-02 — planned

The packet was planned with `/feature-plan` while `diff-engine` was in flight, when its
changes query hit rename parity (its 2026-09-30 progress entry). Four recon records are saved under
`docs/research/process-manager/`, and the two that commissioned the packet were
copied from the `diff-engine` branch into `docs/research/diff-engine/` so `main`
can cite them.

- L1-L4 were locked by the user. L5-L15 were presented as defaults and stood.
- The design landed in the same PR:
  - `docs/design/processes.md` (new);
  - D1 rewritten in `engine.md` so that `git` answers a read where gix diverges;
  - the write lanes in `concurrency.md`;
  - the spine's map, summary and open list.
- The PRD is `docs/prd/process-manager.md`, with five phases. No code has
  changed.
