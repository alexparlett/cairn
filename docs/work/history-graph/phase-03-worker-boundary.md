# Phase 03 — Worker boundary

```
STEP 0  Pre-flight: read docs/work/history-graph/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/history-graph before editing.
        Verify phases 01 and 02 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/,
        crates/cairn-git/src/, docs/prd/history-graph.md (requirement R3), and
        docs/design/cairn.md decision D3. Also read the gix threading model from
        the vendored source: ThreadSafeRepository and to_thread_local() in
        gix-0.87.1/src/types.rs and src/repository/thread_safe.rs.
        ALSO READ docs/prd/credential-prompts.md requirement R4. You are not
        building it — see STEP 2 for why you are reading it.
STEP 2  Decide O3, then implement.

        DESIGN AGAINST TWO CONSUMERS, BUILD ONE. This phase has exactly one
        consumer available — the paged history query — and a boundary shaped only
        by "walk some commits, return a page" will need widening later. The second
        consumer is already specified: fetch, in
        docs/prd/credential-prompts.md R4. It is a different shape — long-running,
        network-bound, reporting progress, and blocking mid-operation on a UI
        dialog for a credential. Read it, make sure the interface could accommodate
        it, and say in state.md which parts of the design exist for it. Do NOT
        build fetch or any part of it; that packet owns it. The point is only to
        avoid an interface that is accidentally graph-shaped.

        DECIDE — O3: pool size, fixed or scaled with cores. A pool of one is a
        legitimate answer if the numbers say so; decide from a measurement, not
        from a default, and record it. Note the argument against core-scaling:
        the work is I/O- and cache-bound, and gix already parallelises internally
        via max-performance, so scaling with cores oversubscribes against
        gitoxide's own threads.

        Deliverables:
        1. A repository worker pool in `cairn-app`: one
           gix::ThreadSafeRepository per open repository, each worker thread
           calling to_thread_local() exactly once at startup. Repository is not
           Send — a design that tries to move one across threads will not
           compile, and a design that reopens per request throws away the caches
           that make gitoxide fast.
        2. A request/response channel carrying an epoch. A response whose epoch
           is stale is dropped without rendering. Cancellation composes with
           phase 02's cancel signal: superseding a request must actually stop the
           walk, not just ignore its result.
        3. The seam in `cairn-app` that hands results to the UI as values, with
           no path by which a component can wait on the pool.
        4. THE GUARD. This phase is the first time "the UI thread never waits on
           repository work" becomes checkable, because there is now a real
           boundary to point a check at. Land a guard in
           crates/cairn-guards/tests/invariants.rs with a matcher self-test
           proving it fires on the disguised forms, and MOVE the invariant out of
           the "not yet mechanically pinned" list in CLAUDE.md in the same
           commit. That move is this phase's definition of done, not a nicety.

        Invariants in play: every invariant gets its twin in the same change;
        enforcement at the strongest tier that can express it — consider whether
        the boundary can be made a type that simply has no blocking method,
        before settling for a token check.

        Out of scope: rendering (phase 04), multiple repositories open at once,
        any query beyond phase 02's.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: run /qa over the phase diff with responsiveness-reviewer spawned
        fresh — this phase is entirely its subject matter — plus
        gate-integrity-reviewer, since the phase changes the enforcement layer.
        Add the qa-checklist.md items this phase covers and the QA brief below.
        Adjudication goes to the qa-confirm agent (fresh), never this session
        inline; log dismissed findings with reasons in progress.md; fix confirmed
        findings in focused fixes; disputed findings go to the user.
STEP 4  Acceptance: PRD criterion A6, jointly with phase 04. The guard exists,
        fires on a deliberate violation, and CLAUDE.md's unpinned list is shorter
        by one entry.
STEP 5  Update state.md (symbol table, O3 resolved) and progress.md. Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the boundary cannot be expressed as a
type and the only available guard is a token check with known holes — that is a
weaker invariant than CLAUDE.md currently promises and the user should decide
whether to accept it. Otherwise do not stop for permission.
```

## QA brief

- The guard is the deliverable most likely to be quietly weak. Prove it fires:
  write the violating code, watch the guard fail, then delete it. A guard whose
  matcher has never seen red is a guard nobody has tested.
- Directory-walking checks must assert a nonzero file count, per the design rules
  in docs/qa-gate.md — a guard pointed at a moved directory otherwise passes
  forever.
- Epoch handling: confirm a superseded request actually stops work. The weak
  version of this ignores the stale response while the walk runs to completion,
  which looks identical from the UI and burns a core.
- Confirm to_thread_local() is called once per worker, not once per request.
  Per-request conversion compiles, passes every test, and discards the caches the
  whole design exists to keep.
- Check what happens when a worker panics. A pool that silently loses a thread
  degrades into a hang, which is the failure mode this packet is supposed to
  prevent.
- Ask whether the interface could carry fetch (credential-prompts R4): a
  long-running operation that reports progress and blocks mid-flight on a UI
  prompt. A request/response shape with no progress channel and no way to await a
  UI answer is graph-shaped, and widening it later means touching every call site.
  This is a design question, not a missing feature — do not treat an absent fetch
  as a finding.
