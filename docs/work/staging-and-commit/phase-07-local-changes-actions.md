# Phase 07 — Local Changes acts on files

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-06 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over docs/systems/local-changes.md and
        its code (crates/cairn-ui and crates/cairn-app: the two lists,
        WorkingChoice, the follow through refresh), phase 06's dialog, menu host,
        auto-scroll and actions, phase 04's lane as the window submits to it,
        docs/prd/staging-and-commit.md (R8, C8, C18, C24) and
        docs/research/staging-and-commit/fork-staging-and-commit.md sections 1-2.
        Do not read the other planning docs directly.
STEP 2  Implement.
        1. Multi-selection in both lists (R8.1). The lists stay flat (a tree
           view is #36, not built here); a collapsed untracked-directory row,
           where status lists one, acts on everything under it.
        2. Fork's five routes (R8.2): double-click, the chords, a drag between
           the lists (one drop zone per list, auto-scroll), the header's Stage /
           Unstage (⌥-held: all) and the double-chevron Stage All, and the
           context menu. The selection after a stage (R8.3).
        3. Discard (R8.4) on the unstaged side only, through the dialog with its
           Consequence from the engine; untracked rows deleted; a nested
           repository refused before the dialog. No discard on the staged side,
           anywhere.
        4. Ignore Whitespace disabled in Local Changes, the diff exact (R8.5);
           queued, running, failed and stale writes drawn where the user acted
           (R8.6).
        5. Conflicted rows (R8.7, L25): staged whole by every route (`git add`,
           which marks them resolved), no discard by any route; a submodule row
           offers no discard and says why (R8.8, L24).
        Tests: C8's view half (the submodule row included), C18 and C24's
        conflicted-row half, headless.

        Invariants in play: no unbounded list without virtualization (both
        lists stay VirtualScrollView); no literal modifier; the UI thread never
        waits; DiffContent read by naming every state.

        Out of scope: the chunk and line gesture (08), the commit box (09).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 07, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C8 (views), C18, C24 (views: conflicted rows).
STEP 5  Update state.md and progress.md; docs/systems/local-changes.md (the
        actions); the root CLAUDE.md status paragraph. Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if a Fork route cannot be built on the
linked Freya without a new dependency; if the selection-after-stage rule is
ambiguous for a multi-selection. Otherwise do not stop for permission.
```

## QA brief

The risk is an action that reaches a path the user did not select.

- A collapsed untracked-directory row: stage it and read back from git that
  exactly its paths moved.
- A drag that ends outside both lists does nothing; one cancelled with Escape
  does nothing.
- The dialog's counts against what the engine then deletes, for a selection mixing
  modified and untracked rows.
