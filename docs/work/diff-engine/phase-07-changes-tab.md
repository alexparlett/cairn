# Phase 07 — The Changes tab, the non-text states and side-by-side

```
STEP 0  Pre-flight: read docs/work/diff-engine/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/diff-engine before editing.
        Verify phases 01 to 06 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-ui/,
        crates/cairn-app/src/window.rs, docs/prd/diff-engine.md (R5.4, R6.1,
        R6.8, R6.9 and criteria C9, C10, C11) and
        docs/research/diff-engine/fork-detail-and-diff-ui.md (the Changes tab,
        side-by-side, and what Fork shows for files it will not diff). Verify
        every Freya API against the vendored source before writing it.
STEP 2  Implement.

        Deliverables:
        1. The Changes tab of R5.4: a one-line summary, the changed-file list on
           the left with a filter, and one file's diff on the right. One file at
           a time, as Fork does.
        2. The states of R6.8 and R6.9: both sizes for a binary; "Changes are too
           large to display" with a Load Diff control, which asks the engine to
           load past the limit; the pointer text and a label for a Git LFS
           pointer; commit ids for a submodule; old and new modes for a mode-only
           change; both names for a rename with no content change; and a
           truncated line with a visible marker when a loaded diff has one past
           the long-line limit.
        3. Side-by-side of R6.1: paired rows with filler on the shorter side, a
           number gutter per side, equal widths, no wrap, one setting shared by
           every diff view and kept for the session. With the C9 test for
           side-by-side rows.

        Invariants in play: no plain ScrollView on a render path — side-by-side
        is two columns inside one virtualising view, not two scroll views; every
        list virtualised; the accelerator table owns the chords.

        Out of scope: expansion in place and Expand All (phase 08), comparing two
        commits (phase 08), staging affordances of any kind (packet 5).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with responsiveness-reviewer and
        test-coverage-auditor spawned fresh, plus the qa-checklist.md items for
        this phase and the QA brief below. Adjudication goes to qa-confirm
        (fresh); log dismissals with reasons in progress.md.
STEP 4  Acceptance: C9 for side-by-side, C10's Changes tab, and C11's non-text
        states.
STEP 5  Update state.md and progress.md. Extend docs/systems/diff.md. Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if side-by-side cannot be built without a
plain ScrollView, or if the 64 MiB ceiling on loading an over-limit file turns out
to be wrong in either direction under measurement — both are decisions the user
took. Otherwise do not stop for permission.
```

## QA brief

The states are where a diff viewer embarrasses itself, because each one is rare
and none of them is exercised by ordinary use.

- Build a fixture for every state in R6.8 and look at each. A notice that says
  "binary" for a submodule is the kind of thing only looking catches.
- Load Diff must stay bounded. Press it on the packet's own over-limit subject
  and watch the window, not just the test.
- Side-by-side pairing is the subtle one: check a change where the two sides have
  different line counts, and a change where one side is empty. Filler rows must
  keep the two columns aligned, and the row count must match the projection's.
- Confirm the side-by-side setting is genuinely shared: change it in the Commit
  tab's diff and see it in the Changes tab's.
- The filter box is a list operation, not a diff operation. Confirm filtering a
  55,184-path list does not rebuild every row.
- Confirm a file selected in the Changes tab cancels the previous file's diff
  rather than racing it.
