# Phase 02 — The runner

```
STEP 0  Pre-flight: read docs/work/process-manager/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/process-manager before editing.
        Verify phase 01's work is present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/process/
        (the moved runner and its stub-git tests), crates/cairn-git/src/
        cancel.rs and error.rs, docs/prd/process-manager.md (R3, R4, R5,
        G6-G11, G16), docs/design/processes.md ("The runner", "Cancellation",
        "Failure"), and docs/research/process-manager/
        platform-and-git-behaviour.md sections A, B, C1, C2, C6 and D. Do not
        read the other planning docs directly.
STEP 2  Implement. L4, L6 and L7 decided the mechanisms; build to them. Verify
        every std and nix item against the linked source (state.md environment
        notes) before writing it.

        Deliverables:
        1. Spawn and pipes (R3.1, R3.2): every invocation is spawned with
           CommandExt::process_group(0). Each pipe in use gets its own thread:
           stdout reader, stderr reader, stdin writer. Thread-spawn failure is
           an error that ends and reaps the process — never a running git with
           nobody waiting on it (the as-built leaks one; see the research
           record's runner findings).
        2. Streams (R3.3-R3.6): stdout reaches the caller as bytes or NUL
           records as it arrives, plus a collect with a ceiling that errors
           (R5.4). stdin is written from caller bytes and closed. stderr is
           forwarded as progress lines split on \r and \n, and a 256 KiB tail is
           retained. The invocation finishes on leader exit plus drained pipes,
           or DRAIN_BOUND after exit (G10); fix its value and record it in
           state.md. Exit is noticed when the pipes close; the cancel tick also
           runs try_wait, for a grandchild holding the pipes after git exits.
           Neither may add a tick's latency to the common case (G19).
        3. Cancellation (R4): a read polls &impl Cancel. A KillHandle is Send +
           Clone and never blocks (try_lock, as ProcessKill does today). Drop
           ends the process and reaps it on a reaper thread. Ending is killpg
           SIGTERM, then killpg SIGKILL after 2 s, and only while a member can
           believed alive. A clean exit that beat the signal is success; any
           other cancelled invocation reports cancellation, even with status 0
           (R4.5, G12). Nothing signals after the reap. stderr: every line to
           the progress callback, at most 256 KiB retained (G13).
        4. Outcomes and proof (R5; G6-G13; G19): read-cancelled distinct from
           write-cancelled-with-stranded-locks; a failed write names a present
           lock file; failure carries args, status and the stderr tail.
           Stub-git tests read the process table to show no group member alive
           or unreaped (G8). Integration tests run real git: a large -z read
           (G6), 64 MiB through hash-object --stdin for end of input and id
           parity, with a stub for stdin against busy stdout and stderr (G7),
           and a commit cancelled inside a sleeping pre-commit hook leaving no
           index.lock (G9). Add the G19 reporter, #[ignore]d and driven by
           CAIRN_BENCH_REPO.

        The old run and stream paths stay in place this phase; fetch and the
        probe move in phase 03.

        Invariants in play: no unsafe (process groups and signals come from std
        and nix safe functions only); never call nix waitpid on a pid std owns;
        no panic in shipping code; the kill handle runs on the UI thread and must
        never block; a change under cairn-git's process path carries its tests
        in the same commit.

        Out of scope: migrating fetch or the probe; the registry and the
        command log (phase 03); anything in cairn-app; timeouts; retries;
        batch children; enabling any nix feature beyond process and signal
        (that is a dependency decision — stop and ask).
STEP 3  Validate: scripts/gate.sh, and run the G19 reporter once against the
        bench repository. Then orchestrate this phase's QA in this session:
        /qa over the phase diff, with destructive-ops-reviewer,
        responsiveness-reviewer and test-coverage-auditor spawned fresh, plus
        the qa-checklist.md items for this phase and the QA brief below.
        Adjudication goes to the qa-confirm agent (fresh), never this session
        inline. Log dismissed findings with reasons in progress.md. Fix
        confirmed findings in focused fixes. Disputed findings go to the user.
STEP 4  Acceptance: G6 to G13 pass against their pinned tests. G19 is
        recorded in progress.md with the machine, the git version and the
        numbers.
STEP 5  Update state.md and progress.md. Extend docs/systems/git-processes.md
        with the runner, cancellation and outcomes as built. Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/process-manager; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if any mechanism needs unsafe, a new
dependency or a new nix feature; if G19's 2 ms cannot be met with safe std
exit detection; or if a group signal can reach a group that is not the
invocation's beyond the residual race the design states. Otherwise do not stop for permission.
```

## QA brief

Process code fails on the machine that didn't write it: a different pipe size, a
slower child, a race that only loses one time in a thousand.

- For each stub-git test, state the mutation that makes it fail. A
  "no orphan left" test that never started an orphan decides nothing.
- Reason about each test on macOS and on a Linux user at their pipe limit, not
  only on a default Linux pipe. Capacity can be 512 bytes or two pages, so a
  test that passes because Linux buffers 64 KiB is not proof.
- Walk every interleaving of exit, the pipes closing, cancel and drop. Answer
  three things for each: who reaps; can anything signal after the reap; can
  anything block forever.
- Confirm the kill handle's path from the UI thread takes no lock it waits for,
  and that no blocking call hides in a `Drop` impl.
- A cancelled write must list stranded locks only after the reap, as fetch's
  does today. Check the ordering, not just the presence.
- Check thread hygiene. Every thread this runner spawns must end when its
  invocation ends, except a reader left on a pipe held by a process git
  detached. Count the threads in a test if you can.
