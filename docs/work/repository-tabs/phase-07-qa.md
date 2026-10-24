# Phase 07 — the merge bar

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/repository-tabs/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/repository-tabs before editing. Verify phases 01-06 are all present at the
        integration tip (state.md's validation table, `git log feature/repository-tabs`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode. This phase runs as its own fresh session.
STEP 1  Load context via an Explore agent over the whole packet diff (`git diff
        main...feature/repository-tabs --stat`, then per surface), docs/prd/repository-tabs.md,
        docs/work/repository-tabs/qa-checklist.md and progress.md (the dismissal log and the
        [observe] amendments).
STEP 2  Run /qa over the whole packet diff, with every reviewer implementation-plan.md's
        dispatch table names, spawned fresh. Run the ENTIRE qa-checklist.md. Verify every PRD
        acceptance criterion C1-C26 against its named test: the test exists, decides the
        criterion, and fails when the behaviour is broken. AUDIT the per-phase dismissal log in
        progress.md: a dismissal whose reason no longer holds is a finding. AUDIT every [observe]
        amendment against the observation file and the user's recorded approval (C1). Run the
        #[ignore]d window checks tabs_switch_check, tabs_idle_check, tabs_launch_check and
        tabs_refresh_check on the bench machine over clones of the bench repository (never write
        to the bench itself) and record the numbers in docs/research/repository-tabs/measured.md
        (C25). Run the real window with ten repositories open, then quit and relaunch it, surface
        by surface. Fix confirmed findings in focused commits; adjudication goes to a fresh
        qa-confirm agent; disputed findings go to the user.
STEP 3  Validate: scripts/gate.sh (the full merge bar; never piped through tail or head).
STEP 4  Acceptance: every C1-C26 met against its test, the full gate green, /qa READY. This is
        the merge bar: STEP 5 onward runs only if it passes.
STEP 5  Update state.md and progress.md. Then offer packet teardown per docs/CLAUDE.md:
        - stamp docs/prd/repository-tabs.md `shipped`, pointing at docs/systems/;
        - verify docs/systems/ is current against the as-built code: a systems doc for the tabs
          (the strip, the holder, the store, closing across tabs), and git-processes.md,
          credentials.md, history-graph.md, local-changes.md and window-frame.md wherever they
          said one repository;
        - verify docs/design/repository-tabs.md and ui.md's deviation rows match what shipped;
        - graduate this packet's invariants into the root CLAUDE.md with their twins (the tab
          holder's guard, the Replier alias in the credential guard, the store's filesystem row,
          the dependency rows), checking each is already there, and rewrite its status paragraph
          and the repo map for several repositories ("There is no repository picker" goes);
        - file leftovers with the file-issue skill (the PRD's Out of scope items that are not
          another packet's, and QA leftovers);
        - mark 5e shipped in docs/work/daily-loop/roadmap.md, state.md and progress.md;
        - delete docs/work/repository-tabs/. git history is the archive.
STEP 6  In user mode: commit explicit paths, push the phase branch, and raise its PR into
        feature/repository-tabs, then STOP. After the user merges it, a resumed session verifies
        integration and raises the packet PR from feature/repository-tabs to main. In explicitly
        declared packet mode only, the packet PR may be raised right after teardown. The USER
        merges every PR.
STEP 7  Final response: what shipped, C25's numbers, what was filed, the PR links.
STOPPING RULES: stop and ask the user when a criterion cannot be met as written; a measurement
misses its bar; a dismissal or an [observe] amendment turns out to hide a real defect in a
decision the user locked; or teardown would delete something the user may still want. Otherwise
do not stop for permission.
```

## QA brief

The whole packet against Fork and against one repository: with ten tabs open, is every surface
that worked with one still right on whichever tab is shown, does nothing of a background tab leak
into the shown one, and does a relaunch put the person back where they were?
