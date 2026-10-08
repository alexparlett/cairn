# Phase 10 — Show Lost Commits

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-09 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over docs/systems/history-graph.md and
        docs/systems/refs.md (the walk's seeds, the reopen, how the stash reflog
        is read whole), crates/cairn-git/src/history (HistoryRequest), the
        assigner and row stores in cairn-model, the vendored gix reflog API in
        ~/.cargo/registry/src/ (gix 0.87.1), docs/prd/staging-and-commit.md
        (R11, C20) and docs/research/staging-and-commit/fork-staging-and-commit.md
        section 7. Do not read the other planning docs directly.
STEP 2  Implement.
        1. The reflogs (R11.2): HEAD's and each local branch's, read whole.
        2. The walk (R11.1, R11.4): the toggle adds their ids to the tips, as a
           reopen; a commit no ref reaches is marked and drawn dimmed. Measure
           the marking's cost on the tmpfs clone of rust-lang/rust and record it.
        3. `Create Branch Here…` (R11.3): a name, then a write in ops/
           (`git branch -- <name> <oid>`), refreshing refs and the history.
        Tests: C20's Show Lost Commits half, against git's own answer.

        Invariants in play: only ops/ mutates; the UI thread never waits; the
        rows stay compact and slim (refs-and-status's C15 and C16 equivalences
        still hold with the toggle on); RowContent read by naming every variant.

        Out of scope: a reflog list of entries; restoring a file from a lost
        commit (filed).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 10, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C20 (Show Lost Commits, Create Branch Here…).
STEP 5  Update state.md and progress.md; docs/systems/history-graph.md (Show
        Lost Commits). Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if marking reachability makes the first
page on rust-lang/rust more than twice refs-and-status's measured first page;
if gix's reflog reading disagrees with `git reflog` on any fixture. Otherwise do
not stop for permission.
```

## QA brief

The risk is a commit drawn as lost that a ref reaches, or the reverse.

- An amended commit, a reset-away commit, a deleted branch's tip still in HEAD's
  reflog, and a commit a tag reaches that is also in a reflog (not dimmed).
- A reflog line over 4 KiB in the middle of a log, not only at the end.
- With the toggle on, scroll and find still page as before (no change to the
  equivalence tests' answers for refs).
