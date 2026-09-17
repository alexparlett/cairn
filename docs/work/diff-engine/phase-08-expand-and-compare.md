# Phase 08 — Expansion in place, comparing two commits, and the measurement

```
STEP 0  Pre-flight: read docs/work/diff-engine/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/diff-engine before editing.
        Verify phases 01 to 07 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-ui/,
        crates/cairn-app/, docs/prd/diff-engine.md (R5.3's expansion, R7, and
        criteria C10, C12, C14), docs/research/diff-engine/fork-detail-and-diff-ui.md
        (expansion in place and comparing two commits) and
        docs/research/diff-engine/measured-baseline.md (the subjects and git's
        own numbers).
STEP 2  Implement.

        Deliverables:
        1. Expansion in place on the Commit tab (R5.3): a file's diff opens under
           its row, files start collapsed, and Expand All opens files in list
           order until a total line budget is spent, then says how many files
           stay collapsed. The budget is Q2 in brainstorm.md: pick it against the
           window check below, as a named constant with a test, and record why.
        2. Comparing two commits (R7): a modifier-click selects exactly two, a
           plain click returns to one, the comparison is tip against tip with the
           lower row as the base, a swap control reverses it, and it shows in the
           Changes tab under a header naming both commits. The Commit tab is
           unavailable while two are selected.
        3. C14's window check, by hand, against ~/Development/bench/rust: the
           window stays responsive while the two heaviest subjects load, with the
           numbers, the hardware and the build profile recorded in progress.md
           beside the engine numbers from phase 02.

        Invariants in play: no plain ScrollView; one viewport of rows however
        many files are expanded — expansion makes the Commit tab's list the
        longest list in the application; the chords come from the accelerator
        table; a match over RowContent names every variant.

        Out of scope: anything filed as #29 to #37; staging.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with responsiveness-reviewer and
        test-coverage-auditor spawned fresh, plus the qa-checklist.md items for
        this phase and the QA brief below. Adjudication goes to qa-confirm
        (fresh); log dismissals with reasons in progress.md.
STEP 4  Acceptance: C10's expansion, C12, and C14's window check.
STEP 5  Update state.md and progress.md, including Q2's answer and the measured
        numbers. Extend docs/systems/diff.md. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the window check misses a C14 ceiling —
whether to cut scope or move a bar is the user's decision, not a phase's.
Otherwise do not stop for permission.
```

## QA brief

Expansion is where the packet's bounded-work promise is most easily lost, because
the list stops being a list of files and becomes a list of every line of every
expanded file.

- Expand a commit with a few thousand files and scroll. One viewport of rows, at
  the top and deep, with several files expanded.
- Check Expand All against the biggest subject in the bar. It must stop at the
  budget and say so, not stall the window and then recover.
- Check the budget is a named constant with a test, and that progress.md says why
  that number.
- Compare two commits that are far apart, and two that are adjacent. Confirm the
  base is the lower row in both, and that swap changes the answer rather than
  just the header.
- Confirm a comparison cannot be left half-selected: a third click, a click on
  the same row, a click while a comparison is loading.
- For C14, record the numbers you actually saw, including the ones that are worse
  than git's. A measurement that only records passes is a press release.
