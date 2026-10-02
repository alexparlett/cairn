# Progress — process-manager

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-10-02 — phase 04: the application — discovery, the network lane, close, the log

Packet mode, committed directly to `feature/process-manager` (`49ece74..HEAD`).

**What landed:**
- `a9021bb` — `GitBinary::with_environment`: the found path and version onto a
  new environment, no second search or probe (pinned by
  `a_found_git_takes_a_new_environment_without_being_searched_for_or_probed_again`;
  mutation: re-probing inside it fails the test).
- `3160095` — R6.1: `worker::Discovery` (`worker/discovery.rs`), started in
  `main` before the window on a `cairn-discovery` thread; each repository
  takes a copy pointed at its own channel (`Backend::open`). R7: the
  operations thread is the network lane (`worker/network_lane.rs`,
  `cairn-network`), `Operation::lane` chosen per operation, and a second
  fetch is refused (`Update::FetchRefused`), kept in the view's `refused`
  state and drawn as a banner until the next press. R6.3: `Request::Close`;
  `Threads::drop` closes the lane's queue and calls
  `SharedRepository::end_invocations(CLOSE_BOUND)` on the repository thread —
  the registry is the one authority for ending a fetch on close; `closing.rs`
  is the window's `with_on_close` hook (first request asks and keeps the
  window open; the stream's end closes it past the hook; a second request
  after `worker::CLOSE_PATIENCE` = 5 s closes it anyway). R8.3:
  `Request::CommandLog` / `Update::CommandLog`.
- `e3108f2` — `docs/systems/git-processes.md` (Discovery, The network lane,
  Closing, the log through the worker), `credentials.md`, `history-graph.md`.
- `8f00c66` — deliverable B: root `CLAUDE.md` (status, repo map, the read
  profile's `GIT_NO_LAZY_FETCH=1` and its 2.44 caveat, the process twin's real
  required shapes, `Running`/`ProcessKill` dropped from the residual
  examples, `the_retired_runner_is_gone` named with its residuals, the new
  UI-thread residuals), qa-checklist item 7, destructive-ops check 10. Each
  claim was checked against `crates/cairn-guards/tests/invariants.rs`
  (`only_the_process_module_builds_or_runs_a_process`'s required shapes,
  `READ_ONLY_PINS`, `RUNNER_NAMES`, `RETIRED_RUNNER_NAMES`,
  `the_retired_runner_is_gone`) and the code; phase 03's draft text was
  treated as a draft.
- QA fixes: `e3b26b3`, `9bf402c`, `9e4f8f8`, `9d3d38e`, `d40c319`, `c01d55e`, `0c28987`
  (below).

**Acceptance, each with the test that pins it and a mutation run that turns it red:**
- G14 — `closing_a_repository_ends_and_reaps_every_git_in_it_within_the_bound`
  (stub fetch leading its group with a grandchild holding its pipes; stream
  ends inside `CLOSE_BOUND`, nothing in the group alive, leader reaped, socket
  gone) and `a_fetch_closed_as_it_starts_is_still_ended` (fetch forwarded to
  the lane, then closed: ends once as a cancel). Mutation: deleting the
  `end_invocations` call fails both (3/3 runs for the second). Window side:
  `closing.rs` tests.
- G15 — worker `a_second_fetch_while_one_runs_is_refused_with_a_reason`
  (mutation: `perform` returning without sending the refusal fails it);
  `a_second_fetch_is_refused_naming_the_fetch_in_flight`; session
  `a_refused_fetch_is_kept_for_the_window_and_the_fetch_in_flight_is_untouched`;
  window (headless) `a_refused_fetch_is_drawn_with_its_reason_until_the_next_press`.
- G16 — `git_is_found_once_per_application_not_once_per_repository` (three
  opens on one discovery probe once, raced by the start thread; two opens
  with a discovery each probe twice). Mutation: a fresh `Discovery` per open
  fails it. `a_git_refused_at_discovery_is_refused_to_every_repository_that_asks`
  (mutation: re-probing after a refusal fails it). A missing or too-old git
  still fails loudly with the minimum version, in the same place and words.
- G17 (worker half) — `the_command_log_is_answered_through_the_worker_with_the_fetch_in_it`.
  Mutation: answering an empty log fails it.
- G21 — review: `docs/design/processes.md` ("Lifecycle"), D1 (`engine.md`,
  "locates `git` and checks its version at startup") and D3 (`concurrency.md`
  "Operations": lanes per operation, a duplicate refused with a reason) match
  what was built; the queueing of different operations is the local lane's,
  out of scope. The write lane is chosen per operation (`Operation::lane`,
  exhaustive). Every parse site reads an untranslated format: this phase
  added none. #18 was answered by phase 01's comment (editor half); the rest
  of #18 is a user decision. The root `CLAUDE.md` points at
  `git-processes.md`, which is current.

**G20 — verified by hand on this machine's desktop session (Hyprland 0.56.2,
Linux), not by a test:**
1. Build: the dev profile (`cargo build --workspace`, so the helper
   was beside the binary). A scratch repository in the session scratchpad
   whose `origin` was a local path with `remote.origin.uploadpack` set to
   `sleep 600; git-upload-pack`, so `git fetch` stays alive with a child.
2. X11 backend (`env -u WAYLAND_DISPLAY target/debug/cairn <repo>`): Fetch
   pressed through XTEST; the process table showed `git fetch --progress
   --no-prune-tags --end-of-options origin` (pid 924443, pgid 924443), its
   `sh -c sleep 600; git-upload-pack …` and `sleep 600` in that group, and
   `$XDG_RUNTIME_DIR/cairn-923077-dff7bd83/askpass`. Closed through the
   desktop's own close action (`qs … ipc call windows close`, what the title
   bar's × and Super+Q run). Five seconds later: the Cairn process gone, no
   process in pgid 924443, no `sleep 600`, no `cairn-*` directory.
3. Wayland backend (`target/debug/cairn <repo>`): Fetch pressed by keyboard
   (Tab, Space via `wtype`); `git fetch` pgid 926264 with `sh` and `sleep 600`;
   `cairn-925872-5f9f0385` present. Closed the same way at 21:43:09.541; the
   Cairn process had exited by 21:43:09.669 (128 ms); no process in the group,
   no socket directory.
4. Issue #27's case, Wayland: a loopback HTTP remote answering `401` to
   everything; Fetch pressed; the credential dialog up ("origin is asking for
   a credential", `cairn-askpass Username for 'http://127.0.0.1:38917': `
   running in git's group 943471, `git-remote-http` beside it); closed the
   same way: Cairn exited in 125 ms, no process in the group, no helper, no
   `cairn-*` directory, no `*.lock` in the repository.
Everything made for the check was removed afterwards.

**QA** (fresh: qa-checklist, responsiveness-reviewer, test-coverage-auditor,
gate-integrity-reviewer, destructive-ops-reviewer; four hit their turn limit
and were re-asked once, and every report was received). qa-confirm (fresh;
re-asked once after its turn limit) adjudicated 41 raw findings: 22
confirmed, 7 confirmed-defer, 6 duplicates merged, 5 dismissed (one finding
split). Fixed:
- `e3b26b3` — QC1/TC1 (the mid-spawn close test could not fail: the close
  stopped the epochs before the fetch was forwarded; it now waits for the
  command log's answer), TC4 (`a_close_stops_the_epochs_as_it_is_submitted_and_is_queued`;
  mutation: deleting `epochs.stop()` fails it), TC5, TC7, TC8, TC2.
- `9bf402c` — TC9.
- `9e4f8f8` — RR2: an outcome produced by the close no longer clears the
  rows and asks a stopped worker for the history.
- `9d3d38e` — DO6 (`Discovery`'s Debug).
- `d40c319` — QC4 (a)(b), DO2, DO4, GI2 in the code comments.
- `c01d55e` — GI1, GI2, GI3, GI4, GI5, GI6, RR3, TC3 (stated as review),
  DO2 in `CLAUDE.md`.
- TC6 — probed: replacing `with_environment` with a clone in `Backend::open`
  fails five tests (`a_runtime_directory_gives_the_environment_a_socket_and_prompting`
  and four prompt tests), so it is already pinned; nothing added.

Dismissed, with reasons (qa-confirm):
- QC3 — the window cannot produce a refusal today (button hidden in flight,
  slot cleared before the outcome), and "drawn until another fetch is asked
  for" is the documented design; revisit when something else can ask.
- QC4 (c) — step 4 already covers a process entering after `end_all` returns.
- QC7 — STEP 4/5 own it; G20 is recorded above (the sentence in
  `git-processes.md` describes a check that was done).
- QC8 — the stray experiment was reverted; the tree was clean.
- DO5 — `end_invocations`' doc already states the latch.

**Re-review of the fixes** (fresh test-coverage-auditor and
gate-integrity-reviewer; qa-confirm, fresh): every fix test fails on its
mutation (QC1 3/3 with `end_invocations` deleted and 5/5 with the registry's
late-entry check deleted; TC4; RR2; unmutated lifecycle suite 10/10). 7 raw
findings, all confirmed (RG1 with its restatement corrected, RG3 in part),
fixed in `0c28987`: RG1 (what no twin sees is a path-call
start inside `process/environment.rs` and `nix`'s `fork`; the terminal-prompt
twin catches the rest), RG2, RG3 (`withdraw`), RG4 (state.md), RG5, RT1
(the `is_requested` hop named as review; its test-level fix filed as #42),
RT2 (the reload test over all three endings).

**Pending user decision, not decided (CONFIRMED-DEFER):**
- **RR1 / DO1 (silent close half)** — the first close keeps the window open
  and draws nothing; a close can take up to `CLOSE_BOUND` + 1 s, and a worker
  stuck before `serve` (a hung `git --version`, a slow `discover`) never
  reaches the close, so it takes a second request after 5 s. Options: (a) as
  built; (b) a "Closing — ending fetch of origin…" banner while closing;
  (c) (b) plus a timer that closes the window by itself after
  `CLOSE_PATIENCE`. Recommendation: (b). User-visible beyond the PRD.
- **DO1 (stranded locks half) / phase 03's DO3** — locks a close `SIGKILL`
  strands are listed on a `FetchCancelled` the window never shows. Options:
  (a) stated residual (done: `git-processes.md` "Closing"); (b) keep the
  window open when the last outcome carried locks, showing them, and let the
  next request close it. Recommendation: (a) for now — a `SIGKILL` on close
  needs a git that ignored `SIGTERM` for 2 s, and git names the lock itself
  on the next write.
- **QC6 / phase 03's DO2** — a failed fetch's `present_locks` reach the
  message but not the banner (`why_it_failed` shows the first
  `fatal:`/`error:` line). Options: (a) as is; (b) carry `present_locks`
  structurally to `FetchStatus::Failed` and append them to the banner.
  Recommendation: (b), small, in the next packet that touches the banner.
- **QC5 / DO3** — a fetch already forwarded to the lane when the close lands
  is spawned, then ended at once (reaches the network). Options: (a) as is —
  the registry is the one authority; (b) a `Closed` stage in `FetchControl`
  so the lane answers it cancelled without spawning, the registry kept as the
  backstop. Recommendation: (b), later; harmless today.
- **QC2** — pre-existing: a Cancel pressed in the moment after the lane
  clears the control and before the outcome arrives cancels the user's NEXT
  fetch, whenever it is started. Options: tie a cancel to a fetch identity,
  or drop the overtaking-cancel stage once the outcome is sent.
  Recommendation: an issue, fixed with the next change to `FetchControl`.
- **RR4** — `git --version` has no bound; a hung probe leaves every
  repository unable to open or close. The PRD says no timeouts. Options: a
  probe deadline (a refusal after N seconds) vs. as is.
- **DO4** — a close forced after `CLOSE_PATIENCE` orphans a late git (now
  stated). Option: `SIGKILL` the registered groups before that close.
- **GI7** — CI's git version is unchecked, so the partial-clone test may be
  skipped on the merge bar; for phase 05: assert CI's git is 2.44 or later.
- **DO6 (older derives)** — `GitBinary`/`GitEnvironment` derive `Debug` and
  would render proxy values if anything printed them; nothing does. Options:
  a value-free `Debug` on `GitEnvironment`.
- **Banner lock list (state.md DO2)** — decided not to change in this phase:
  no phase 04 deliverable covers the failure banner and the change is
  user-visible; batched above (QC6).

**Issues:** #41 filed (the command-log view, R8.3's deferral); #42 filed (a
headless test of `main.rs`'s close wiring, TC3/RT1). Not closed:
#27 (recommend closing — its exact case, the window closed with the dialog
up, was checked by hand above and leaves no socket, helper or git) and #18
(its remaining half, `DISPLAY`/`WAYLAND_DISPLAY`/`GNUPGHOME`, is the user's).

## 2026-10-02 — phase 03: fetch on the runner, the registry and the log

Packet mode, committed directly to `feature/process-manager` (`edee9cf..HEAD`).

**What landed:**
- `49cc25e` — D3, **decided by the user on 2026-10-02**: the 2.30 floor
  stays and `READ_ONLY` gains `GIT_NO_LAZY_FETCH=1`. Git older than 2.44
  ignores it, a constraint `diff-engine` designs around (`reads/mod.rs`,
  `docs/systems/git-processes.md`). Pinned by the builder and stub tests' read
  sets, `READ_ONLY_PINS` in `every_git_invocation_disables_the_terminal_prompt`
  (self-test cases), and `a_read_in_a_partial_clone_does_not_fetch_a_missing_object`
  against real git with a write control.
- `a895e81` — R3.7: fetch and the probe run on `GitCommand::start`;
  `GitCommand::run`, `stream`, `Running`, `ProcessKill` deleted, with the new
  twin `the_retired_runner_is_gone`. Discharged phase 02's QC3, QC6 and GI2
  (the `.output()`/`.wait()` positive rows SWAPPED for `CommandExt` and
  `ChildStdout`, said so in the body).
- `277ed60` — `cairn_model::CommandRecord`, `CommandExit`.
- `2b27b77` — the registry and log (`process/registry.rs`,
  `process/command_log.rs`), `SharedRepository::end_invocations`,
  `command_log`, `CLOSE_BOUND` 3 s, `LOG_ENTRIES` 1000, `LOG_BYTES` 4 MiB
  (reasons in `state.md`).
- `7f88857` — docs.

**G18:** `git diff edee9cf..HEAD -- crates/cairn-git/tests crates/cairn-app`
shows one ADDED test (`a_failed_fetch_names_the_lock_files_present_and_only_those`)
and no changed assertion. The fetch tests that pin fetch's five behaviours
predate this phase: progress — `the_arguments_leave_prune_to_git_forbid_pruning_tags_and_end_the_options`
(exact lines through `fetch()`), `a_fetch_over_http_prompts_for_each_half_of_the_credential_and_succeeds`,
the app's `a_fetch_reports_progress_finishes_and_the_history_reloads_from_the_new_refs`;
cancel before git starts — `the_recording_stub_reports_a_fetch_that_was_allowed_to_start`
(cancel before `finish`) and the operations.rs `FetchControl` tests; cancel
mid-transfer — `cancelling_a_fetch_that_is_waiting_on_a_prompt_kills_git_and_leaves_nothing_behind`;
stranded locks — `a_cancel_names_the_lock_files_left_under_the_git_directory_and_only_those`;
`Performed` — `tests/fetch.rs`'s `Invalidated::refs().and(Invalidated::objects())`.
Mutation: restoring a `run` on `GitCommand` fails `the_retired_runner_is_gone`.

**G17 (engine half):** the `process/registry.rs` tests, one per exit path
(finish, failure, cancel by handle and by signal, drop, drop with no reaper,
never started, runner-ended, cancel beaten by a clean exit, failed stdin
write), the bounds' tests in `process/command_log.rs`, and
`a_fetch_with_a_token_is_logged_once_without_the_token_or_the_environment`.
Mutations: `cancelled = asked_to_end` fails two; `end_all` asking nothing to
end fails both close tests; dropping the char-boundary step panics the trim
test; recording `Unknown` for a failed stdin write fails its test.

**QA** (fresh: qa-checklist, destructive-ops-reviewer, test-coverage-auditor,
gate-integrity-reviewer; each was re-asked once after hitting its turn limit
and delivered). qa-confirm adjudicated 26 raw findings: 13 confirmed (4
duplicates merged), 1 escalated, 5 dismissed, 3 unverified then settled.
Fixed in `a279103`, `53a9440`, `96421a0`, `1d01608`: QC7, DO4 (stderr half),
TC4, TC1, TC2, TC5, QC4, DO3, DO2/QC1 (test; outcome recorded in `state.md`),
GI1 (path calls, spawn-once pin, residual), GI5, DO5/QC2 (residual stated),
DO1 (stated in `git-processes.md` and `credentials.md`; policy batched).
A fresh re-review of the fixes (gate-integrity, test-coverage; qa-confirm)
confirmed 9 of 10, fixed in `152e952` and `576bc61`: RG1 (the spawn pin
passed `.status()`/`.output()`; now banned in production `process/`), RG2,
RG4, RT1, RT2, RT3, RT4, RT6.

Dismissed, with reasons (qa-confirm):
- GI6 — the production `const READ_ONLY` precedes `mod tests`, and a test
  module above it trips clippy's `items_after_test_module`; builder tests
  spell the set out anyway.
- QC3 — the only caller passes `RemoteSummary.name`, never a configured URL.
- QC8 — process note, not a defect; the full gate was run.
- TC3 — skipping the partial-clone test below git 2.44 is the stated design;
  the value stays pinned by the guard and the builder tests. Whether CI's git
  is 2.44 or later was not checked here.
- TC6 — `CommandRecord` derives `PartialEq`.
- RT5 — only `leave` holds two locks, so no reverse order exists to observe.
- DO4's `cancelled: true` half — per phase 02's rule a cancel that lands
  before the reap is a cancel.
- QC4, TC7 (unverified by qa-confirm) — settled by the pre-existing fetch-level
  tests named under G18, and QC4 also by the added
  `a_kill_before_the_invocation_is_driven_still_ends_it`.

**Pending user review, not decided:**
- **DO1 (escalated by qa-confirm).** A fetch that exits 0 in the microseconds
  between the runner's last `try_wait` and its `SIGTERM` is now reported
  cancelled; the old fetch path checked success first. It is phase 02's R4.5
  rule (and the race `group.rs` states) applied to a write. Options: (a) keep
  and state it (done: `git-processes.md`, `credentials.md`); (b) for a write,
  report any exit 0 as success whatever was signalled; (c) as (a), and reword
  the cancelled banner to say a cancelled write may have taken effect.
  Related to phase 02's DO4 and DO2 policy items.
- **A failed fetch's outcome grew** (DO2/QC1): `GitFailed::present_locks` is
  filled, and `GitUnwatched`/`GitOutputTooLarge` are reachable. Within R5.3;
  whether the banner shows the lock list (it shows only the first
  `fatal:`/`error:` line) is phase 04's.
- **Stderr tail retention** (DO5/QC2): the log keeps up to 256 KiB of git's
  stderr per record; a URL with userinfo there would be a credential outside
  `Secret`. Stated as a residual; scrubbing is a design change.
- **CLAUDE.md and `.claude/agents/qa-checklist.md` are stale** (GI2, GI3, GI4,
  RG3). Not edited by this phase, because they are the user's instruction
  files. Proposed: in root `CLAUDE.md`, the process twin's required shapes
  become `.spawn()`, `.try_wait()`, `CommandExt`, `Stdio`, `Child`,
  `ChildStdout`, `ChildStderr`, `nix` and `.command(..)`; a read adds
  `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1` (the latter ignored below git
  2.44) and the `READ_ONLY` twin text names both; drop `Running`/`ProcessKill`
  from the residual's examples and name `the_retired_runner_is_gone`; and add
  to qa-checklist item 7 the retired-runner residual (alias, free function or
  macro entry points seen only if they start a process; a wrapper on `start`
  not seen).

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

**Re-review of the fixes** (fresh destructive-ops-reviewer and
test-coverage-auditor over `bf17825..f98f5f7`; a fresh qa-confirm adjudicated
9 findings: 8 confirmed, 1 dismissed), fixed in the commit after `f98f5f7`:
- **R2D1** — `GitUnwatched` now says a write ended that way may have taken
  effect (its stdin writer starts first and may have delivered it all).
- **R2D2** — per-read stderr events could hold tens of thousands of strings and
  starve the tick behind a slow callback. A read's lines now travel as one
  string, and the driver breaks off its batch once a tick has passed so the
  cancel poll and escalation still run. `EVENTS_BOUND`'s doc restated.
- **R2D3** — the lock advice reads "stale if no git is running here", true even
  when Cairn lost track of its own git.
- **R2D-n1, R2D-n2** — the drop's bounded wait in the no-thread fallback, and the
  open-pipe count lagging its pipe, are stated where the docs claimed more.
- **R2T1, R2T2, R2T3** — new tests: a write the runner ends lists its locks
  (`a_write_the_runner_ends_lists_the_locks_present`; the runner-ended errors
  are now built in one place), the fallback waits out a held lock
  (`a_drop_with_no_thread_to_reap_on_waits_out_a_held_lock_and_kills_the_group`),
  and a finished invocation is not asked to end. Each turned red by its
  mutation (empty locks; `try_lock`; `concluded` never set); the per-line
  stderr mutation still fails the burst test.
- Dismissed: **R2D-n3** — the blocked stdin writer is already documented and
  its pipe gives no signalling evidence.
- One more load flake this round, fixed in `f98f5f7`: the setsid holder test
  could cancel before the holder had left the group.
- G19 re-run after the last fix: M1 overhead 0 at the median (0.43 ms at the
  min); floor 0 at the median.

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
  SIGKILL goes out up to 2 s after its pid was freed (and, per the re-review,
  the open-pipe count can lag a pipe whose writers have all gone). Options: (a) don't reap
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
