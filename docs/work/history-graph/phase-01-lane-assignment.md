# Phase 01 — Lane assignment

```
STEP 0  Pre-flight: read docs/work/history-graph/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/history-graph before editing.
        This is phase 01, so first create feature/history-graph from main if it
        does not exist (and push it if a remote exists). Direct integration work
        requires an orchestrator prompt that explicitly declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-model/src/,
        docs/research/history-graph/gix-revwalk-ordering.md, and
        docs/prd/history-graph.md (requirement R1). Do not read the other
        planning docs directly.
STEP 2  Implement. O1 was resolved before this phase: brainstorm.md L9 settles
        that the total assigner is the floor, not one of two options, because any
        bounded window can be exceeded by larger skew. Build the total assigner.
        The window refinement is deferred and explicitly NOT in this phase.

        Note L10 while building: R1.2 constrains lane INDICES only. Edge segments
        may be repainted for rows inside the loaded window, which is what lets a
        late-joining parent draw its connecting line.

        Deliverables:
        1. `cairn-model`: GraphRow, Lane, EdgeSegment. Plain data, no
           dependencies, Debug + PartialEq, documented with what each field means
           to a renderer that has never seen git.
        2. `cairn-model` or a new module in it: the LaneAssigner — takes
           (id, parent_ids) in walk order, returns rows. Pure: no I/O, no clock,
           no gix, no allocation proportional to history length.
        3. A fixture set exercising: linear history, branch and merge, octopus
           merge, criss-cross merges, multiple roots, and a skewed history where
           a parent arrives before its child. Write these as literal parent maps,
           not as generated repositories — the assigner never sees a repository.
        4. Tests for A1, A2 and A3 from the PRD. A3 (stability) wants a property
           test: assigning N then N more must yield identical rows for the
           first N.

        Invariants in play: `cairn-model` depends on nothing — adding a crate
        there breaks layer_dependencies_are_allowlisted and is a decision to
        bring to the user, not a convenience. No panic on a reachable path: the
        assigner's input is whatever a real repository produces.

        Out of scope: any gix call, any I/O, any rendering, the history query
        (phase 02), performance tuning beyond the complexity bound in R1.3.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: run /qa over the phase diff with test-coverage-auditor spawned
        fresh (see implementation-plan.md), plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to the qa-confirm
        agent (fresh), never this session inline; log dismissed findings with
        reasons in progress.md; fix confirmed findings in focused fixes;
        disputed findings go to the user.
STEP 4  Acceptance: PRD criteria A1, A2, A3 pass against their tests.
STEP 5  Update state.md (the symbol table, and O1 resolved) and progress.md.
        Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit explicit
        paths (never git add -A), push the phase branch, and raise a PR using the
        repository template into feature/history-graph; never merge it. In
        explicitly declared packet mode only, commit directly onto integration
        with no per-phase PR. NEVER merge or PR to main.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the total assigner cannot be made
correct for real skewed history without the deferred window after all; if the
assigner cannot meet R1.3's complexity bound without a structure that breaks
R1.2's index stability; or if any deliverable seems to need a dependency in
cairn-model. Otherwise do not stop for permission.
```

## QA brief

The thing most likely to be wrong here is a test that passes by construction.
The assigner is pure and fully testable, so there is no excuse for a gap — which
means the failure mode is fixtures too uniform to decide anything.

- For each fixture, state the mutation to the assigner that would make the test
  fail. If you cannot, the test is decorative.
- The skew fixture (A2) is the one that matters: confirm it actually exercises
  the out-of-order path, and is not passing because the parent happens to arrive
  in order anyway.
- The stability property (A3) is about lane INDICES (L10). Check the test asserts
  index stability strictly, and that where an edge differs it differs only by
  gaining a segment for a late-arriving parent — an edge changing for any other
  reason is the instability A3 exists to catch, and a test that accepts all edge
  changes would miss it.
- Check the octopus and criss-cross fixtures assert *edge* correctness, not only
  lane counts. A wrong edge with a right lane count is the defect a lane-only
  assertion misses.
