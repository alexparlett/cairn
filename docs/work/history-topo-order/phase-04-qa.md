# Phase 04 — the merge bar

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/history-topo-order/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/history-topo-order before editing. Verify phases 01-03 are present at the
        integration tip. Direct integration work requires an orchestrator prompt that explicitly
        declares packet mode. This phase runs as its own fresh session.
STEP 1  Load context via an Explore agent over the whole packet diff (`git diff
        main...feature/history-topo-order`), docs/prd/history-topo-order.md,
        docs/work/history-topo-order/qa-checklist.md and progress.md (the dismissal log).
STEP 2  Run /qa over the whole packet diff, with every reviewer implementation-plan.md names,
        spawned fresh. Run the ENTIRE qa-checklist.md. Verify every PRD acceptance criterion
        C1-C9 against its test: it exists, decides the criterion, and fails when the behaviour is
        broken. AUDIT the dismissal log in progress.md: a dismissal whose reason no longer holds
        is a finding. Re-read C8's numbers against the bench. Fix confirmed findings in focused
        commits; adjudication goes to a fresh qa-confirm agent; disputed findings go to the user.
STEP 3  Validate: scripts/gate.sh, the full merge bar (never piped through tail or head).
STEP 4  Acceptance: every C1-C9 met, the full gate green, /qa READY. This is the merge bar:
        STEP 5 onward runs only if it passes.
STEP 5  Update state.md and progress.md. Then offer packet teardown per docs/CLAUDE.md:
        - stamp docs/prd/history-topo-order.md `shipped`, pointing at docs/systems/history-graph.md;
        - verify docs/systems/ is current;
        - graduate any new invariant into the root CLAUDE.md with its twin;
        - file leftovers with the file-issue skill (keeping the opening pass across a reopen, and
          the PRD's other out-of-scope items not another packet's);
        - mark 5c shipped in docs/work/daily-loop/roadmap.md, state.md and progress.md;
        - delete docs/work/history-topo-order/.
STEP 6  In user mode: commit explicit paths, push the phase branch, raise its PR into
        feature/history-topo-order, then STOP. After the user merges it, a resumed session
        verifies integration and raises the packet PR to main. In explicitly declared packet mode
        only, the packet PR may be raised right after teardown. The USER merges every PR.
STEP 7  Final response: what shipped, C8's numbers, what was filed, the PR links.
STOPPING RULES: stop and ask the user when a criterion cannot be met as written, a measurement
misses its bar, or a dismissal hides a defect in a locked decision. Otherwise do not stop for
permission.
```

## QA brief

The whole packet against git: run the oracle over every fixture and over the bench once more, and
confirm no row differs.
