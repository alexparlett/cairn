# Phase 03 — Fetch on the new runner, the registry and the log

```
STEP 0  Pre-flight: read docs/work/process-manager/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/process-manager before editing.
        Verify phases 01 and 02 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/process/,
        crates/cairn-git/src/ops/ (fetch.rs, stranded_locks.rs, mod.rs),
        crates/cairn-git/tests/ (the fetch and credential tests),
        crates/cairn-model/src/ (secret.rs and lib.rs, for the model's
        conventions), docs/prd/process-manager.md (R3.7, R6.2, R6.3, R8, G17,
        G18)
        and docs/design/processes.md ("Lifecycle", "The command log"). Do not
        read the other planning docs directly.
STEP 2  Implement.

        Deliverables:
        1. The migration (R3.7): the probe and fetch run on the phase 02 runner.
           Fetch keeps its public API — FetchInProgress, its canceller and
           finish — and its outcomes, including stranded locks. Delete the old
           run and stream paths, and every name only they used. Fetch's
           existing tests, the credential-prompts suite included, keep their
           assertions and pass (G18).
        2. The registry (R6.2): a per-repository record of the invocations
           currently running. It can end them all, the way a cancel does, and
           wait a stated bound for their reaps. An invocation enters it when it
           is spawned and leaves when it is reaped, whatever the path — finish,
           cancel or drop. The waiting call never runs on the UI thread; phase 04
           wires it.
        3. The command log (R8): a CommandRecord type in cairn-model, with
           arguments, working directory, start, duration, exit status, cancelled,
           and the stderr tail. The log is bounded by entry count and by bytes.
           Fix LOG_ENTRIES, LOG_BYTES and CLOSE_BOUND and record them in
           state.md. Every invocation writes exactly one entry when it ends,
           whatever the path. It never takes anything from the environment. Tests: one entry
           per invocation over finish, cancel, drop and spawn failure; the bounds
           hold; and an entry from a fetch run with an askpass token contains
           neither the token nor any environment value (G17, engine half).

        Invariants in play: no credential value logged or stored (the
        SECRET_HOLDERS roster stays empty; the record holds no Secret and no
        environment); cairn-model stays plain data on its allowlist, and its
        change carries a test in the same commit; only ops/ mutates (fetch's
        Write still goes through WriteAuthority); no panic in shipping code.

        Out of scope: anything in cairn-app (phase 04); a view of the log (to
        be filed); push or any other new verb.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff, with destructive-ops-reviewer and
        test-coverage-auditor spawned fresh, plus gate-integrity-reviewer if any
        guard roster changed, the qa-checklist.md items for this phase and the QA
        brief below. Adjudication goes to the qa-confirm agent (fresh), never
        this session inline. Log dismissed findings with reasons in
        progress.md. Fix confirmed findings in focused fixes. Disputed findings
        go to the user.
STEP 4  Acceptance: G18, and G17's engine half, pass against their pinned tests.
STEP 5  Update state.md and progress.md. Update docs/systems/git-processes.md
        (the registry, the log), and docs/systems/credentials.md wherever it
        describes the old runner — it should point at git-processes.md rather
        than restate it. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/process-manager; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if a fetch test's assertion has to change
to pass (the behaviour moved, and that is a decision); or if the log cannot
record an invocation without holding something derived from the environment.
Otherwise do not stop for permission.
```

## QA brief

This is the phase where shipped behaviour moves onto new code, and where a
second store of per-invocation state appears beside the askpass token.

- Diff fetch's observable behaviour, not its code. Check five things:
  - the progress lines it forwards;
  - its cancel before git starts;
  - its cancel mid-transfer;
  - its stranded-lock listing;
  - what `Performed` invalidates.

  Each must be pinned by a test that existed before this phase.
- Confirm the registry entry is removed on every exit path, including a spawn
  failure and a reaper-thread reap after a drop. A registry that leaks entries
  makes close wait for nothing.
- Read the `CommandRecord` type field by field. Arguments can carry a URL. The
  rule allows that, because Cairn passes remote names, not URLs. Confirm no
  call site passes a URL it read from config.
- Count log entries per path. An invocation recorded twice, or never, is a
  silent untruth.
