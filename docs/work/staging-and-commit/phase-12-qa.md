# Phase 12 — The merge bar, and teardown

Its own fresh session. Nothing here builds a feature.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-11 are present at the integration tip and
        every phase PR is merged into it. Direct integration work requires an
        orchestrator prompt that explicitly declares packet mode.
STEP 1  Load context via an Explore agent over the whole packet diff
        (git diff main...feature/staging-and-commit), docs/prd/staging-and-commit.md,
        qa-checklist.md, progress.md (the dismissal log) and the docs/systems/
        files the phases wrote.
STEP 2  Run /qa over the whole packet diff, with every reviewer
        implementation-plan.md names for any phase, spawned fresh. Run the
        ENTIRE qa-checklist.md. Verify every PRD acceptance criterion C1-C24
        against its pinned test (run it; read that it decides what it claims).
        AUDIT the per-phase dismissal log in progress.md: a dismissal whose
        reason no longer holds is a finding. Adjudication goes to qa-confirm
        (fresh); fix confirmed findings in focused commits; disputed findings
        go to the user.
STEP 3  Validate: scripts/gate.sh (the full gate, git-floor included).
STEP 4  Acceptance — the merge bar, and everything after it is conditional on
        passing: every criterion C1-C24 met against its test, C21's numbers
        recorded and within their bars, the checklist complete, the gate green.
        If it fails, stop here and report.
STEP 5  Update state.md and progress.md. Then offer teardown per docs/CLAUDE.md:
        stamp the PRD `shipped`, pointing at docs/systems/; verify each
        docs/systems/ file this packet touched is current against the code;
        move C21's numbers to docs/research/staging-and-commit/measured.md;
        graduate the new invariants (the two rosters, the chord lists and the
        bare-key scope, the filesystem-mutation guard, the gesture's viewport
        twin) into CLAUDE.md with their twins; update the
        program roadmap and state (packet 5 shipped, O3 closed, 5b's brief
        checked against what this packet left) and the spine's pointers; file
        every PRD out-of-scope item and every leftover with the file-issue
        skill; delete docs/work/staging-and-commit/.
STEP 6  Branch authority follows the declared mode. In user mode, teardown lands
        through this phase's PR into feature/staging-and-commit and the session
        stops; after the user merges it, a resumed session verifies integration
        and raises the packet PR from feature/staging-and-commit to main. In
        explicitly declared packet mode, the packet PR may be raised right after
        teardown on integration. The USER merges every PR.
STEP 7  Final response: the merge-bar verdict, every number, what was filed.
STOPPING RULES: stop and ask the user on any disputed finding, on any criterion
that passes only by reinterpretation, and before stamping the PRD. Otherwise do
not stop for permission.
```

## QA brief

The merge bar's particular risk is a destructive path that drifted between
phases: a `Consequence` computed in 03 and rendered in 07 from a different count,
a verb added in 10 or 11 that never joined the roster, a chord scope widened in a
later phase. Read every destructive operation end to end — engine, roster,
dialog, re-check, `Performed`, the popover's entry — and read D1's paragraph in
`CLAUDE.md`, `docs/design/engine.md` and `docs/systems/git-processes.md` side by
side.
