# Phase 02 — the history in date order

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/history-topo-order/state.md and this file, nothing else
        yet. Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/history-topo-order before editing.
        Verify every earlier phase's work is present at the integration tip.
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/history.rs and history/ (walk.rs, stream.rs, seeds.rs, session.rs, reach.rs, reflogs.rs), crates/cairn-model/src/rows_page.rs and history.rs (RowsPage::reached, History::append), crates/cairn-git/tests/every_ref.rs, lost_commits.rs, compact_rows.rs, crates/cairn-ui/tests/history_list.rs, docs/research/history-topo-order/walk-as-built.md §1, §3.1, §5; read state.md, this file, implementation-plan.md and
        the PRD sections and evidence named below directly; nothing else of the planning docs.
STEP 2  Implement:
        1. `walk::open` from a refs snapshot, the cold cursor path, `HistorySession` and a find's
           pages use `DateOrder` (PRD R2.5); tips in the oracle's order (R1.2); stash rows from
           the walk's synthetic tips, stream.rs's stash merge and look-ahead retired (R1.3); Show
           Lost Commits' reflog tips after the stashes (R1.4).
        2. Show Lost Commits' "reached after all" correction removed if no other order needs it,
           else kept with a test that it never fires for the history (R2.6); a cairn-model
           change with tests in the same commit.
        3. The tests that change with the order rewritten with their reason — classes A, B, E and
           F, per implementation-plan.md (C5); the history's git comparisons switched to the
           oracle; C4's tests in a target the git-floor table runs.
        Invariants in play: `every_view_of_a_row_names_every_kind_of_row`, the virtualization twins, the frozen compact-rows oracle and its fingerprint, `recent_messages`' `git log -n 10` parity (CommitTime), a test in the same commit for every cairn-model change.
        Out of scope: the history thread and the window's count (03); any change to the lane assigner beyond what stash placement requires.
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done
        (never piped through tail or head; it runs the git-floor step). Then orchestrate this
        phase's QA in this session: /qa over the phase diff, with implementation-plan.md's
        reviewers for this phase spawned fresh, plus qa-checklist.md's items and this file's QA
        brief. Adjudication goes to a fresh qa-confirm agent. Log dismissed findings with reasons
        in progress.md. Fix confirmed findings in focused commits. Disputed findings go to the
        user.
STEP 4  Acceptance: C4, C5.
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md; save
        memory-worthy decisions.
STEP 6  In user mode: commit explicit paths (never git add -A), Conventional Commits with a scope
        and a body, no session link; push the phase branch; raise a PR from the repository
        template into feature/history-topo-order; never merge it. In explicitly declared packet
        mode only: commit and push onto feature/history-topo-order, no per-phase PR. NEVER merge
        or PR to main. The USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when a test listed as failing by design turns out to pin behaviour the user cares about beyond order; a test of class A, B, E or F cannot be rewritten without losing what it pinned; or a find to a deep ref becomes slower than today after the opening pass; otherwise do not stop for permission.
```

## QA brief

- Does every history-level comparison with git now use the oracle, with stashes and lost
  commits?
- Does each rewritten test still fail when its behaviour is broken?
- Is any order-dependent code left that assumes commit-time order for the history (the stash
  look-ahead, reach)?
