# Phase 01 — git's date-order walk

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/history-topo-order/state.md and this file, nothing else
        yet. Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/history-topo-order before editing.
        This is phase 01: if feature/history-topo-order does not exist, first create it from
        main in its own worktree and push it.
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over docs/research/history-topo-order/git-topo-order.md (§2, §5.1, §5.2: the algorithm to copy) and gix-topo-apis.md (§3, the building blocks), git's source at ~/.cache/cairn/git-src/v2.30.9 and v2.56.0 (revision.c, prio-queue.c, commit-graph.c), crates/cairn-git/src/history.rs and history/ (walk.rs, seeds.rs, the Grafted and Polled wrappers), crates/cairn-git/tests/every_ref.rs (how fixtures and git comparisons are built today), scripts/git-floor.sh (the filtered run and its floor count); read state.md, this file, implementation-plan.md and
        the PRD sections and evidence named below directly; nothing else of the planning docs.
STEP 2  Implement:
        1. `HistoryOrder::DateOrder` and its walk in `crates/cairn-git/src/history/date_order.rs`,
           per implementation-plan.md "The walk": git's three queues over gix_revwalk::Graph,
           generation numbers where git would use the commit-graph, a queue whose ties pop in
           insertion order, parents in git's order, tips from real commits, each step cited to
           revision.c / prio-queue.c at both tags (PRD R2.1, R2.2); stashes as synthetic
           one-parent tips after HEAD, their base's reach decided before seeding without forcing
           a whole-history read where a commit-graph would spare one (R1.3); the commit-graph's
           generations used only where git uses them (R2.1, C10; check and close #128 if it is
           the same rule).
        2. Cancellation between commits in every loop, the opening pass included, and the count
           callback at most once per 100 ms (PRD R2.3, R2.4).
        3. The unit tests of C1 and C3, and `crates/cairn-git/tests/date_order.rs`: every PRD R5.1
           fixture, with and without a commit-graph, compared with the oracle (PRD R1, L5) — C2,
           C10; a new row of scripts/git-floor.sh's table with its own floor.
        4. The opening pass measured on the bench, read only, against PRD R6.1's command and for
           memory per commit, recorded in docs/research/history-topo-order/measured.md (C8's
           first part).
        Invariants in play: `only_the_ops_module_mutates_a_repository` (the fixtures are built in test code; the walk reads only), the process guards (no `git` spawned from the walk; the oracle runs in tests), the frozen compact-rows oracle (not touched), `CommitTime` and `GraphOrder` unchanged, no `unwrap`/`expect` in production code.
        Out of scope: using the new order for the history (02); the history thread and window (03); writing a commit-graph anywhere but a test fixture.
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done
        (never piped through tail or head; it runs the git-floor step). Then orchestrate this
        phase's QA in this session: /qa over the phase diff, with implementation-plan.md's
        reviewers for this phase spawned fresh, plus qa-checklist.md's items and this file's QA
        brief. Adjudication goes to a fresh qa-confirm agent. Log dismissed findings with reasons
        in progress.md. Fix confirmed findings in focused commits. Disputed findings go to the
        user.
STEP 4  Acceptance: C1, C2, C3, C10, and C8's opening-pass measurement.
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md; save
        memory-worthy decisions.
STEP 6  In user mode: commit explicit paths (never git add -A), Conventional Commits with a scope
        and a body, no session link; push the phase branch; raise a PR from the repository
        template into feature/history-topo-order; never merge it. In explicitly declared packet
        mode only: commit and push onto feature/history-topo-order, no per-phase PR. NEVER merge
        or PR to main. The USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when the walk's order differs from the oracle's on a fixture and the source does not explain why; git's whole-list path and incremental path disagree on a fixture; Cairn's per-commit state on the bench's opening pass approaches git's 522 MiB; the opening pass misses PRD R6.1's bar on the bench; deciding a stash's base is reached forces a whole-history read where a commit-graph would spare it; or a gix_revwalk API the plan names does not exist in the vendored 0.35.0; otherwise do not stop for permission.
```

## QA brief

- Does each step cite git's source, and is every tie-break of GT §5.1 present?
- Do the parity tests run git's oracle, not a stored list, at the floors too? Does each fail
  when the tie-break is swapped (try it)?
- Is the opening pass's loop the one that polls the cancel?
