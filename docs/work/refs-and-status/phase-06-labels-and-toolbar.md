# Phase 06 — Labels on rows, stash rows, REFS and the toolbar

```
STEP 0  Pre-flight: read docs/work/refs-and-status/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/refs-and-status before editing.
        Verify phases 01-05 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/ (the history
        row and list, commit_tab.rs and its Line enum and HeaderKey cache,
        toggle_glyphs.rs for how glyphs are drawn as shapes, diff_palette.rs),
        crates/cairn-app/src/window.rs and detail_pane.rs, the title bar,
        crates/cairn-ui/tests/, docs/design/ui.md, docs/prd/refs-and-status.md
        (R5, R6, R7, C7) and docs/research/refs-and-status/fork-refs-and-status-ui.md
        sections 1, 3, 8 and 9. Use the freya skill, and verify every Freya API
        in the fork checkout at the pinned rev. Do not read the other planning
        docs directly.
STEP 2  Implement.
        1. Chips on rows (R5): Fork's outlined chips between graph and subject,
           lane-tinted; tag indigo with a tag glyph; a generic remote glyph;
           current branch ✓; HEAD row bold; compact labels; clipped at the
           column edge. Glyphs drawn as shapes (no new font). Each kind told
           apart without colour.
        2. Stash rows drawn (R5.4); selecting one shows R6.2's Commit and Changes.
        3. The Commit tab's REFS row (R6.1), cached with the header per commit.
        4. The title bar (R7): repository, `*` when status reports a change,
           current branch, ↓behind ↑ahead; detached and unborn states.
        5. Headless tests for C7; the history list's viewport twin still holds
           with labelled rows.

        Invariants in play: no unbounded list without virtualization; no
        component names a literal modifier; meaning never on colour alone;
        cairn-ui names neither gix nor cairn_git and touches no file.

        Out of scope: the sidebar (07), Local Changes (08), forge icons
        (packet 6), greying off-branch commits and push/pull dots (filed).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 06, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C7 passes.
STEP 5  Update state.md and progress.md; docs/systems/history-graph.md (labels
        and stash rows drawn) and docs/systems/diff.md (REFS in the Commit tab);
        the root CLAUDE.md status paragraph. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/refs-and-status; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main — teardown raises that one PR
        and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user before any deviation from Fork beyond the
two the PRD names; if a glyph seems to need a font; if labels cost the row's
draw enough to threaten the frame budget. Otherwise do not stop for permission.
```

## QA brief

- A row with twenty refs must clip, not wrap, and must not grow the row height.
- Compact labels: a local branch and its upstream at one commit draw one chip
  with a glyph; a second remote's ref at the same commit keeps its own chip;
  an upstream at a different commit is not compacted.
- Detached HEAD, unborn branch, and a branch with a gone upstream in the title
  bar, each tested.
- Check the label layout is computed once per row build, not per frame.
