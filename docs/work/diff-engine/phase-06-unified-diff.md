# Phase 06 — The unified diff view

```
STEP 0  Pre-flight: read docs/work/diff-engine/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/diff-engine before editing.
        Verify phases 01 to 05 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-ui/,
        docs/prd/diff-engine.md (R6.1 to R6.7, criteria C9 and C11),
        docs/research/diff-engine/fork-detail-and-diff-ui.md (the diff view, its
        header and its measured colours) and
        docs/research/diff-engine/ui-and-app-as-built.md (the toolkit's text and
        scrolling primitives, and the mockup's palette). Verify every Freya API
        against the vendored source before writing it.
STEP 2  Implement. The view draws projections from phase 01; it computes nothing
        about a diff itself.

        Deliverables:
        1. The rows of R6.4 and R6.5: an old and a new line-number gutter, a
           plus-or-minus marker column, hunk header rows carrying git's @@ line
           in muted text at normal row height with no band and no buttons, solid
           tints starting after the gutters, stronger tints for the intra-line
           ranges, one fixed row height, and horizontal scrolling for long lines.
           Through the virtualising view, with the C9 test for unified rows.
        2. The font and the colour tokens of R6.5 and R6.6: IBM Plex Mono
           embedded with its licence file, and diff colours as named tokens
           rather than literals at the call site.
        3. The header of R6.2, R6.3 and R6.7: previous and next change, the path
           with its filename emphasised, and the toggles for ignore whitespace
           (which says when it is hiding changes), fewer lines, more lines (one
           line per click, never below one) and entire file. Side-by-side is
           phase 07; the toggle may land here disabled or there entire, your
           call, but say which in progress.md.

        Invariants in play: no plain ScrollView on a render path; the diff view
        renders through the virtualising view; cairn-ui names neither the engine
        nor the filesystem; no unwrap or expect; keyboard chords resolve through
        phase 05's table.

        Out of scope: the Changes tab and the non-text states (phase 07),
        side-by-side rows (phase 07), expansion in place and compare (phase 08).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with responsiveness-reviewer and
        test-coverage-auditor spawned fresh, plus the qa-checklist.md items for
        this phase and the QA brief below. Adjudication goes to qa-confirm
        (fresh); log dismissals with reasons in progress.md.
STEP 4  Acceptance: C9 for unified rows, and the parts of C11 this phase lands.
STEP 5  Update state.md and progress.md. Extend docs/systems/diff.md with the
        view as built, including the colour tokens and where they came from.
        Update the root CLAUDE.md status paragraph: the application now draws a
        diff. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: **Ask the user before downloading the font file**, naming the
file, its source and its size; a download is theirs to approve, and the licence
file ships with it. Stop and ask as well if the toolkit cannot draw intra-line
ranges within a line, which would reopen L4's "always on, no toggle", or if fixed
row heights cannot carry the design. Otherwise do not stop for permission.
```

## QA brief

This is the phase where per-frame work hides.

- C9 counts rows built. Read what else the view does per row: a highlight range
  recomputed on every frame, a string formatted per row, a colour looked up per
  row — none of that is counted by the test and all of it grows with the file.
- Scroll deep in the 100,000-line fixture and check the work between frames does
  not grow with the offset. The count staying flat is not the same as the cost
  staying flat.
- Check the horizontal extent. If it is measured from the rows currently built,
  the scrollbar will change as you scroll; say so in progress.md if it does
  rather than leaving it to be discovered.
- Confirm the marker column and the blank gutter carry the meaning without the
  tint, by reading a row with colour ignored.
- Confirm the whitespace notice appears only when changes are actually hidden,
  not whenever the toggle is on.
- Check the context minimum. One line, not zero: the patch is independent of the
  display, but a user who cannot see any context cannot review.
