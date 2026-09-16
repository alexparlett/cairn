# Phase 02 — History query

```
STEP 0  Pre-flight: read docs/work/history-graph/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/history-graph before editing.
        Verify phase 01's work is present at the integration tip before starting.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/,
        crates/cairn-model/src/, docs/prd/history-graph.md (requirement R2), and
        docs/research/history-graph/gix-revwalk-ordering.md. Do not read the
        other planning docs directly.
STEP 2  Measure O2, then implement.

        MEASURE FIRST — O2: gix's docs say ByCommitTime "benefits greatly" from
        an object cache, implying the unaccelerated path looks each commit up
        twice. Measure both sortings on a repository WITHOUT a commit-graph file
        before choosing a default. Record the numbers in progress.md.

        Deliverables:
        1. A bounded, resumable history query in `cairn-git`: takes a starting
           point, a limit and a cursor; returns a page of GraphRow plus the
           cursor to continue from. No gix type in the signature (the crate seal
           guard checks the crate boundary; this is the API-design half it
           cannot check).
        2. Parent ids and commit time read from `gix::revision::walk::Info`
           without decoding full commit objects, except where the summary needs
           the object. Info::object() is documented as expensive — treat it that
           way.
        3. Cancellation: a cancel signal the caller can set, checked often enough
           that an abandoned walk stops promptly. Observable in a test, not
           asserted by comment.
        4. Integration tests against a fixture repository built by running real
           git commands. A fake object database proves nothing about gitoxide.
        5. THE ASSIGNER'S WINDOW (new scope, added 2026-09-15). Phase 01 shipped
           an unbounded `LaneAssigner`: it retains every row it emits plus an
           index into them, and each row carries one segment per open lane, so
           retained state grows as rows x open lanes. CORRECTED 2026-09-16: the
           361-segments-per-row figure this step originally cited was an artefact
           of breadth-first arrival order in a fixture whose branches never
           merged. Measured on seven real repositories in the default
           `CommitTime` order the p99 is 8 segments per row. The window is a load
           budget, not a memory mitigation — see
           `docs/research/history-graph/scroll-memory-model.md` Part D. R1.3 has been narrowed to match reality and R1.2's
           finality sentence repaired; read both in `docs/prd/history-graph.md`
           before designing. Give the assigner a bounded window it OWNS: a window
           imposed from outside cannot stop it reaching back past the boundary,
           which is exactly why R1.2's "rows that have left the window are final"
           was previously unenforceable. Evicting a row makes it final; a parent
           whose child has already been evicted opens its own lane and simply
           does not draw the joining line, which L9 anticipated and L10 permits.
           Pin the new bound with a test — phase 01 deliberately did not write
           one, because writing it then would have ratified the change before the
           user made it.

        Invariants in play: no gix type crosses the crate boundary; no panic on a
        reachable path; a new dependency is a user decision.

        Out of scope: threading and the worker pool (phase 03), rendering
        (phase 04), diffs, blame, refs decoration.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: run /qa over the phase diff with test-coverage-auditor and
        responsiveness-reviewer spawned fresh, plus the qa-checklist.md items
        this phase covers and the QA brief below. Adjudication goes to the
        qa-confirm agent (fresh), never this session inline; log dismissed
        findings with reasons in progress.md; fix confirmed findings in focused
        fixes; disputed findings go to the user.
STEP 4  Acceptance: PRD criteria A4 and A5 pass against their tests.
STEP 5  Update state.md (symbol table, O2 resolved) and progress.md with the
        measurement. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the measurement says no sorting is
affordable without a commit-graph file (that would change D4's shape, not just
this phase's default); or if bounded resumption cannot be made stable across
pages. Otherwise do not stop for permission.
```

## QA brief

- The fixture repository must be built by running real `git`, and the test must
  fail if gix's answer diverges from git's. A fixture that only proves gix agrees
  with itself decides nothing.
- Cancellation (A5) needs an observable assertion: that the walk stopped early,
  not merely that the call returned. Count visited commits, or make the check
  observable some other way — a test that just calls cancel and sees a return
  value passes whether or not cancellation works.
- Check the cursor: two pages of N must equal one page of 2N, including lanes.
  This is where a resumable query built on a non-resumable assigner breaks.
- Confirm no path decodes a full commit object per row for data available on
  `Info`. That is the difference between this query being fast and being a
  process-spawn's worth of slow.
