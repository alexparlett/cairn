# Phase 08 — The diff's staging gesture

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-07 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/diff_view.rs,
        unified_rows.rs, side_by_side_rows.rs, diff_row_parts.rs, the row
        geometry shared with the Commit tab (expansion.rs, commit_tab.rs),
        cairn_model::ShownDiff and Selection, phase 06's auto-scroll and chords,
        phase 07's Local Changes wiring, the vendored freya pointer events at
        rev caa46f8, docs/prd/staging-and-commit.md (R9, C19) and
        docs/research/diff-engine/fork-detail-and-diff-ui.md Finding 23. Do not
        read the other planning docs directly.
STEP 2  Implement.
        1. Hover (R9.1): in Local Changes' diff only, a hovered chunk's outline
           and its floating actions — Stage and Discard Changes… unstaged,
           Unstage staged — drawn outside the recycled rows, taking no focus,
           following the hover after the rows change.
        2. Drag-selection (R9.2, R9.3): tracked by the list, not a row, across
           unmounted rows, auto-scrolling at the edges; it narrows the actions and
           chords to the selected changed lines; side by side keeps it in one
           column; with nothing selected the chords act on the whole file.
        3. The mode row (R9.4) with its own actions; the Commit and Changes tabs
           draw none of this (R9.5), and neither does a conflicted file's diff
           (R8.7) or a submodule's discard (R8.8).
        4. The viewport twin for the diff with the gesture drawn (C19), named in
           the root CLAUDE.md virtualization invariant.
        Tests: C19, headless.

        Invariants in play: no unbounded list without virtualization; UnifiedRow,
        SideBySideRow and DiffContent read by naming every variant; no literal
        modifier; the per-frame work of hover and drag stays bounded by the
        viewport.

        Out of scope: the commit box (09).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 08, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C19; C22 (the gesture's viewport twin named in the root
        CLAUDE.md).
STEP 5  Update state.md and progress.md; docs/systems/local-changes.md (the
        gesture); the root CLAUDE.md virtualization invariant (the twin). Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if Fork's gesture cannot be built on the
linked Freya without a second dependency; if a chunk drawn at a non-default
context maps to more than one exact change in a way Fork's evidence does not
settle. Otherwise do not stop for permission.
```

## QA brief

The risk is a selection that maps to lines the user did not see selected.

- Select lines at context 10 (one drawn chunk covering several exact changes)
  and stage: read back from git exactly those lines.
- Drag from row 10 to row 5,000 with the virtual list unmounting rows on the way:
  the selection is whole.
- Hover, then stage, then do not move the pointer: the actions follow what is
  now under it, or none.
