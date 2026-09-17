# Phase 05 — The detail pane, the Commit tab and the keyboard

```
STEP 0  Pre-flight: read docs/work/diff-engine/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/diff-engine before editing.
        Verify phases 01 to 04 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-ui/,
        crates/cairn-app/src/window.rs, docs/prd/diff-engine.md (R5, R8 and
        criteria C10 and C13), docs/research/diff-engine/fork-detail-and-diff-ui.md
        (the pane and the Commit tab) and docs/research/diff-engine/ui-and-app-as-built.md
        (what the toolkit can do, and the guards that bear on a render file).
        Verify every Freya API against the vendored source before writing it.
STEP 2  Implement. L9 fixed the layout: it is Fork's, and where Cairn's own
        mockup disagrees, the mockup is superseded.

        Deliverables:
        1. The pane of R5.1 and R5.2: below the commit list, behind a draggable
           splitter, collapsible, with Commit and Changes tabs, Commit the
           default and the last tab kept for the session. The Changes tab's
           contents arrive in phase 07; a placeholder is fine here.
        2. The Commit tab of R5.3, minus expansion, which is phase 08: author and
           committer with full timestamps, the full commit id, parents as links
           that select a loaded parent, the whole message, and the changed-file
           list, virtualised, collapsed, renames showing both names. No avatar,
           no network call, no ref chips.
        3. The accelerator table of R8, with this packet's actions, and the new
           invariant with its guard twin: no component names a literal modifier.
           The guard, its matcher self-test, and the CLAUDE.md invariant land in
           the same commit as the table.

        Invariants in play: no plain ScrollView on a render path — the exceptions
        roster is empty and stays empty; every list is virtualised; cairn-ui
        never names gix or cairn_git and never touches the filesystem; a match
        over RowContent names every variant; a new invariant gets its twin in the
        same change.

        Out of scope: diff rows (phase 06), the Changes tab's contents (phase
        07), expansion in place and compare (phase 08).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with responsiveness-reviewer,
        gate-integrity-reviewer (this phase changes the enforcement layer) and
        test-coverage-auditor spawned fresh, plus the qa-checklist.md items for
        this phase and the QA brief below. Adjudication goes to qa-confirm
        (fresh); log dismissals with reasons in progress.md.
STEP 4  Acceptance: C13, and the parts of C10 this phase lands — the Commit tab's
        fields and the kept tab.
STEP 5  Update state.md and progress.md. Extend docs/systems/diff.md with the
        pane as built, and record the accelerator table's contract there. Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if a splitter or a collapsible pane cannot
be built without a plain ScrollView — a roster row is a user decision, not a
phase's — or if the toolkit's modifier spellings cannot be guarded, which would
leave R8.3 as a convention rather than an invariant. Otherwise do not stop for
permission.
```

## QA brief

A pane is easy to make look right and easy to make slow.

- The changed-file list must be virtualised now, not later: the packet's own
  measured subject touches 55,184 paths, and a list that builds a row each is the
  invariant this repository already broke once by accident.
- Check the guard fires. Write a component that names a literal modifier, watch
  the guard go red, then remove it. A guard nobody has seen fail is a promise.
- Confirm the guard's matcher sees the disguised forms: an aliased import of the
  modifier type, a qualified path, a constant defined next to the component.
- Read what happens when the selection is not a commit. The row kinds are an
  enumeration for a reason, and a pane that assumes a commit will not compile
  once the working-tree row exists — make sure it does not assume one in a way
  the compiler cannot catch.
- Check the parent links against a parent that is not loaded. They must do
  nothing visible rather than jump to the wrong row.
- Confirm no answer for a previous selection can be drawn in this tab; phase 04
  gave every answer a target, and this is the first phase that could ignore it.
