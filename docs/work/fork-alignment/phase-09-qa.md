# Phase 09 — the merge bar

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/fork-alignment/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/fork-alignment before editing. Verify phases 01-08 are all present at the
        integration tip (state.md's validation table, `git log feature/fork-alignment`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode. This phase runs as its own fresh session.
STEP 1  Load context via an Explore agent over the whole packet diff (`git diff
        main...feature/fork-alignment --stat`, then per surface), docs/prd/fork-alignment.md,
        docs/work/fork-alignment/qa-checklist.md and progress.md (the dismissal log).
STEP 2  Run /qa over the whole packet diff, with every reviewer implementation-plan.md's
        dispatch table names, spawned fresh. Run the ENTIRE qa-checklist.md. Verify every PRD
        acceptance criterion C1-C22 against its named test: the test exists, decides the
        criterion, and fails when the behaviour is broken. AUDIT the per-phase dismissal log in
        progress.md: a dismissal whose reason no longer holds is a finding. Measure C22 on the
        bench machine with the #[ignore]d window checks (never write to ~/Development/bench;
        use a clone per the repo's rules) and record the numbers in
        docs/research/ui-review/measured.md. Run the real window over the fixture beside FS1/FS2
        once more, surface by surface. Fix confirmed findings in focused commits; adjudication
        goes to a fresh qa-confirm agent; disputed findings go to the user.
STEP 3  Validate: scripts/gate.sh (the full merge bar; never piped through tail or head).
STEP 4  Acceptance: every C1-C22 met against its test, the full gate green, /qa READY. This is
        the merge bar: STEP 5 onward runs only if it passes.
STEP 5  Update state.md and progress.md. Then offer packet teardown per docs/CLAUDE.md:
        - stamp docs/prd/fork-alignment.md `shipped`, pointing at docs/systems/;
        - verify docs/systems/ (window chrome, history-graph.md, sidebar.md, diff.md,
          local-changes.md, git-processes.md) is current against the as-built code;
        - verify docs/design/ui.md's deviation table matches what shipped;
        - graduate this packet's invariants into the root CLAUDE.md with their twins (the style,
          icon and zone guards, the Ctrl+Shift+D move), checking each is already there;
        - file leftovers with the file-issue skill (the PRD's "Out of scope" items that are not
          another packet's, and QA leftovers);
        - mark 5d shipped in docs/work/daily-loop/roadmap.md, state.md and progress.md;
        - delete docs/work/fork-alignment/. git history is the archive.
STEP 6  In user mode: commit explicit paths, push the phase branch, and raise its PR into
        feature/fork-alignment, then STOP. After the user merges it, a resumed session verifies
        integration and raises the packet PR from feature/fork-alignment to main. In explicitly
        declared packet mode only, the packet PR may be raised right after teardown. The USER
        merges every PR.
STEP 7  Final response: what shipped, C22's numbers, what was filed, the PR links.
STOPPING RULES: stop and ask the user when a criterion cannot be met as written; a measurement
misses its bar; a dismissal from an earlier phase turns out to hide a real defect in a decision
the user locked; or teardown would delete something the user may still want. Otherwise do not
stop for permission.
```

## QA brief

The whole packet against Fork: run the real window beside FS1 and FS2's screenshots, surface by
surface, and list every remaining difference. Each one must be Fork's evidence, a `ui.md`
deviation row, or a finding.
