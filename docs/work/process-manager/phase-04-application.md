# Phase 04 — The application: discovery, the network lane, close, the log

```
STEP 0  Pre-flight: read docs/work/process-manager/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/process-manager before editing.
        Verify phases 01 to 03 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/worker/
        (pool.rs, operations.rs, startup.rs, request.rs, askpass.rs,
        fetch_tests.rs), crates/cairn-app/src/main.rs, session.rs,
        fetch_state.rs and window.rs, docs/prd/process-manager.md (R6, R7,
        R8.3, R9, G14-G17, G20, G21), docs/design/concurrency.md ("Operations")
        and docs/research/process-manager/runner-and-worker-as-built.md section
        2. Do not read the other planning docs directly.
STEP 2  Implement. L3, L10 and L11 decided this; build to them.

        Deliverables:
        1. Discovery once (R6.1): GitBinary is found when the application
           starts, not when a repository opens, and handed to each thread that
           runs git. A missing or too-old git still fails loudly, with the
           minimum version named. A worker test counts the stub's probe
           invocations across repeated opens (G16).
        2. The network lane (R7): the operations thread becomes the network
           lane, chosen per operation rather than assumed, so packet 5 can add
           the local lane beside it. A second fetch while one is running or
           queued is refused with a reason the window draws, never silently
           dropped. Worker test plus headless window test (G15).
        3. Close (R6.3): closing a repository ends every invocation in its
           registry and waits a bounded time for their reaps, on a worker
           thread, never the UI thread. Worker test through the real boundary
           with a stub git whose grandchild holds a pipe (G14). Closing the
           window closes its repository. Check that by hand on a desktop session
           and record the result (G20); it may close issue #27.
        4. The log through the worker (R8.3): a request that answers a
           repository's command log as cairn-model values, and a test that a
           fetch's entry arrives (G17, worker half).
        5. The record (R9, G21): docs/systems/git-processes.md is complete
           against the code, and the root CLAUDE.md Pointers name it. docs/systems/credentials.md points at it.
           docs/systems/history-graph.md is updated wherever it describes the
           operations thread or discovery. The root CLAUDE.md status paragraph,
           repo map and UI-thread residuals name the new exempt functions, if
           any. docs/design/processes.md, D1 and D3 are checked against what
           was built; a disagreement is fixed in the code or taken to the user,
           never silently in the doc.

        Invariants in play: the UI thread never waits on repository work (the
        refusal, the cancel and close all start on the UI thread; anything that
        waits lives in worker/); Update and Request derive what the worker's
        tests require, so every new payload must too; no panic in shipping code.

        Out of scope: the local write lane, queueing between different
        operations, and any new verb (staging-and-commit, remote-sync); a view of
        the command log (file it); progress coalescing (#25).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff, with responsiveness-reviewer and
        test-coverage-auditor spawned fresh, plus the qa-checklist.md items for
        this phase and the QA brief below. Adjudication goes to the qa-confirm
        agent (fresh), never this session inline. Log dismissed findings with
        reasons in progress.md. Fix confirmed findings in focused fixes.
        Disputed findings go to the user.
STEP 4  Acceptance: G14, G15, G16, G17 and G21 pass against their pinned tests
        or review, and G20 is recorded in progress.md.
STEP 5  Update state.md and progress.md. File the command-log view as an issue
        with the file-issue skill. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/process-manager; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if closing a repository cannot end its
processes without the UI thread waiting; if the toolkit gives no hook at window
close (G20 then needs a decision, not a workaround); or if discovery at startup
changes what a user sees when git is missing. Otherwise do not stop for
permission.
```

## QA brief

Everything in this phase starts on the UI thread and must not stay there.

- Trace the refusal, the cancel and the close from the click to the worker and
  back. Name each call the UI thread makes, and confirm none can wait. The guard
  cannot see inside the exempt `worker/` functions.
- The refusal must reach the window as a reason, drawn. A test that only checks
  the worker's reply proves half of G15.
- For close, state what happens to an invocation that is mid-spawn when the
  close arrives. It must still be ended, not missed because it was not yet in
  the registry.
- Check the systems doc against the code, sentence by sentence, in the systems
  doc's own tense. Nothing unbuilt may be stated as built.
