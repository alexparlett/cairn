# Phase 04 — the history

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/fork-alignment/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/fork-alignment before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/fork-alignment`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/history/ (commit_row.rs, date_text.rs, graph_cell.rs, graph_geometry.rs, ref_chips.rs, list.rs), crates/cairn-model/src/lane_assignment.rs (push_stash) and the drawn-lane tests (crates/cairn-ui/tests/drawn_lanes.rs, drawn_rows.rs), crates/cairn-git/src/history/ (stream.rs, reach.rs, walk.rs), the vendored jiff 0.2.37 tz API, docs/systems/history-graph.md, the evidence FS1 §4 and FS2 §1a, §6 (and the shots uv2, uv7, uw3); read state.md, this file, implementation-plan.md and
        the PRD sections and evidence named below directly; nothing else of the planning docs.
STEP 2  Implement:
        1. The row (R5.1-R5.3, R5.5): no header row, 23 px, the columns in Fork's order,
           initials avatars (also in the Commit tab and the Changes summary, if cheaper here
           than in 06), Fork's chips (a branch's and a remote's by its lane, a tag's and a
           stash's by kind, R1.3); ShowHead (Ctrl+0) selecting `HEAD`'s row and bringing it into
           view, found as a ref press finds its row (`history/ref_find.rs`).
        2. Local dates (R5.4): jiff a direct dependency of cairn-ui and cairn-app (allowlist
           rows, deny.toml feature pin, Cargo.lock unchanged), the zone read once on a worker and
           handed down, UTC fallback said once; the activity popover's times; the guard
           `the_render_layer_is_handed_its_time_zone` and its CLAUDE.md invariant. Closes #103.
        3. `HEAD`'s reach (R5.6): marked by the walk in emission order, a cairn-model field with
           tests in the same commit, real-git test against `git rev-list HEAD`
           (`the_walk_marks_what_head_reaches_as_git_rev_list_does`); drawn greyed. Closes #67.
           The stash lane (R5.7): `push_stash` takes the base's lane when it waits for the base;
           drawn-lane tests; docs/systems/history-graph.md updated.
        4. The graph's geometry (R5.8) measured from uv7; Show Lost Commits through the View
           item, its chord and the strip under the toolbar (R5.9), the header check box gone.
        Invariants in play: `every_view_of_a_row_names_every_kind_of_row` (a new row field read everywhere), the virtualization twins (`only_a_viewport_of_rows_is_built_however_long_the_history` and its labelled twin), the frozen compact-rows oracle, `layer_dependencies_are_allowlisted`, a test in the same commit for every cairn-model change, the UI-thread guard (the zone read on a worker), the seam (cairn-ui never reads the system).
        Out of scope: the topological order itself (packet 5c, already merged); commit search; collapse merges (#1); branch filters (#2); gravatar.
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build over the fixture
        (docs/research/ui-review/fixture-recipe.md) at 1024x768 and compare against FS1/FS2's
        shots (qa-checklist item 7). Then orchestrate this phase's QA in this session: /qa over
        the phase diff, with the reviewers implementation-plan.md's dispatch table names for
        this phase, spawned fresh, plus the qa-checklist.md items and this file's QA brief.
        Adjudication goes to a fresh qa-confirm agent, never this session inline. Log dismissed
        findings with reasons in progress.md. Fix confirmed findings in focused commits.
        Disputed findings go to the user.
STEP 4  Acceptance: C10, C11, C12, C13.
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md (newest first);
        save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/fork-alignment; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/fork-alignment, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when the reach mark cannot be made final at emission without 5c's guarantee (stop: do not work around 5c); the stash-lane change alters what the frozen compact-rows oracle compares; jiff's feature set needed differs from what gix-date enables (which would change Cargo.lock); or Fork's chip for a case the fixture lacks is needed and not observed; otherwise do not stop for permission.
```

## QA brief

- Does the reach mark agree with `git rev-list HEAD` on every fixture, Show Lost Commits on
  included? Is the cost per row constant?
- Is the zone value cheap to clone, and handed to the history, the Commit tab and the popover as a
  value? Does no render file read it?
- Do the drawn-lane twins still pass with only the expected stash rows changed?
- Is the per-row work for the avatar and chips still bounded, with nothing allocated per frame
  that grows with the history?
