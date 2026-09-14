# Phase 04 — Graph view

```
STEP 0  Pre-flight: read docs/work/history-graph/state.md and this file. Nothing
        else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/history-graph before editing.
        Verify phases 01, 02 and 03 are present at the integration tip.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/,
        crates/cairn-app/src/, and docs/prd/history-graph.md (requirement R4).
        REQUIRED: verify Freya's virtualisation and custom-painting APIs against
        the vendored source at
        ~/.cargo/registry/src/index.crates.io-*/freya-0.5.0-rc.6/ and
        freya-components-0.5.0-rc.6/. Freya 0.5 is a release candidate on a
        builder API; anything recalled from rsx! examples is the wrong API.
        Do not read the other planning docs directly.
STEP 2  Decide O4, then implement.

        DECIDE — O4: how a late-joining edge (the consequence of O1) is drawn so
        it reads as intentional rather than as a rendering bug. Look at real
        skewed history before deciding.

        Deliverables:
        1. A virtualised graph view in `cairn-ui` over GraphRow: only visible
           rows rendered, uniform row height (L7), keyed rows.
        2. Lane and edge painting — merges and branches visually followable, not
           implied by indentation. Colour distinguishes lanes but never carries
           meaning alone: the view must survive a monochrome screenshot and a
           colour-vision deficiency.
        3. Selection: keyboard reachable, surviving the arrival of more rows.
        4. Wiring in `cairn-app` from the worker pool to the view, including a
           visible loading state distinguishable from an empty repository, and
           paging as the user scrolls.
        5. The A7 measurement: run against a named real repository of at least
           100k commits, record the numbers and the repository in progress.md.

        Invariants in play: cairn-ui may not name gix or cairn_git — if the view
        needs something it does not have, that is a new cairn-model type and a
        new engine call, not a shortcut. The UI thread never waits on repository
        work (now guarded, from phase 03). No unbounded list without
        virtualisation.

        Out of scope: commit details, diffs, refs decoration, search, filtering,
        context menus, and every mutation.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: run /qa over the phase diff with responsiveness-reviewer and
        test-coverage-auditor spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to the qa-confirm
        agent (fresh), never this session inline; log dismissed findings with
        reasons in progress.md; fix confirmed findings in focused fixes;
        disputed findings go to the user.
STEP 4  Acceptance: PRD criteria A6 and A7. A7 is a recorded measurement, not an
        assertion — record what you actually observed, including if it is bad.
STEP 5  Update state.md (symbol table, O4 resolved) and progress.md with the
        measurement. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode, as phase 01.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if Freya 0.5-rc lacks a virtualisation or
custom-painting primitive the design needs — that is a toolkit-level problem and
possibly a design decision, not something to work around with a hand-rolled
viewport. Otherwise do not stop for permission.
```

## QA brief

- Virtualisation is the claim most easily faked. Confirm that rendering a 100k
  row list constructs only visible rows — measure or instrument, do not infer
  from the component's name.
- Check for a per-frame clone of the row vector. `render` runs on every reactive
  change, and a clone of history-sized data there costs the frame budget every
  time anything nearby changes. Per-row clones inside the viewport are fine; say
  which you found.
- The loading state (R4.3) must be distinguishable from an empty repository. Test
  both, because the bug is that they look identical and nobody notices until a
  user reports "my repository is empty".
- Selection surviving new rows (R4.4) is the kind of thing that works in a
  30-commit test and breaks at page two. Test it across a page boundary.
- Accessibility is in the product rules, not decoration: confirm keyboard
  reachability and that lane identity survives without colour.
