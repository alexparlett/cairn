# Phase 03 — the history thread and the opening pass in the window

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/history-topo-order/state.md and this file, nothing else
        yet. Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/history-topo-order before editing.
        Verify every earlier phase's work is present at the integration tip.
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/worker/ (pool.rs, routing.rs, epoch.rs, history/lane.rs, request.rs), crates/cairn-app/src/session.rs (reopen_history), crates/cairn-app/src/history/status_text.rs (the opening sentence), crates/cairn-ui/src/history/, crates/cairn-app/src/window_check.rs, docs/systems/history-graph.md, the root CLAUDE.md (routing and the UI-thread guard's notes), docs/research/history-topo-order/walk-as-built.md §4; read state.md, this file, implementation-plan.md and
        the PRD sections and evidence named below directly; nothing else of the planning docs.
STEP 2  Implement:
        1. The history thread (PRD R3): its own SharedRepository handle, the history lane, its
           pages and finds; the refs read, the opened-from snapshot, filters and Retire stay on
           the repository thread, which asks the history thread to reopen when refs move,
           superseding a running pass; close joins it within one commit's work; the root
           CLAUDE.md's routing account and guard notes in the same commit (C6).
        2. The window during a pass (PRD R4, C7): a reopen keeps the old rows drawn and usable
           under a "Reading history… N commits" strip until the new first page replaces them,
           selection kept; a first open's sentence; the worker holding back the count until
           250 ms, then at most once per 100 ms.
        3. The bench measurements of PRD R6 and the whole-bench order comparison (R5.4), on the
           bench machine, read only, a commit-graph only in a copy; recorded in
           docs/research/history-topo-order/measured.md (C8).
        4. docs/systems/history-graph.md as built, its repaint figure corrected, and every
           systems doc C9 names that says what runs on which thread (PRD R7, C9).
        Invariants in play: the UI-thread guard (`the_ui_thread_never_waits_on_repository_work`) and its stated residuals for `submit`'s arms, the close's shape (never waiting on the UI thread), the per-lane epochs, the bench rule.
        Out of scope: keeping the opening pass across a reopen (out of scope, filed at teardown); writing a commit-graph; a smaller assigner window.
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done
        (never piped through tail or head; it runs the git-floor step). Then orchestrate this
        phase's QA in this session: /qa over the phase diff, with implementation-plan.md's
        reviewers for this phase spawned fresh, plus qa-checklist.md's items and this file's QA
        brief. Adjudication goes to a fresh qa-confirm agent. Log dismissed findings with reasons
        in progress.md. Fix confirmed findings in focused commits. Disputed findings go to the
        user.
STEP 4  Acceptance: C6, C7, C8, C9.
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md; save
        memory-worthy decisions.
STEP 6  In user mode: commit explicit paths (never git add -A), Conventional Commits with a scope
        and a body, no session link; push the phase branch; raise a PR from the repository
        template into feature/history-topo-order; never merge it. In explicitly declared packet
        mode only: commit and push onto feature/history-topo-order, no per-phase PR. NEVER merge
        or PR to main. The USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when the first page on the bench misses PRD R6.1's bar; the opening pass's memory is far beyond git's; a filter still waits on an open after the thread move; or the routing change needs an invariant in CLAUDE.md to change in a way its guard cannot follow; otherwise do not stop for permission.
```

## QA brief

- Does anything that ran on the repository thread still wait on an open?
- Does a close during an opening pass end promptly, measured?
- Are the bench numbers read, not asserted, and recorded with git's beside them?
