# Phase 06 — the detail pane and the diff bar

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/fork-alignment/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/fork-alignment before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/fork-alignment`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/detail/ (commit_tab.rs, changes_list.rs, expansion.rs), crates/cairn-app/src/detail/ (pane.rs, changes_tab.rs), crates/cairn-ui/src/diff/ (header.rs, toggle_glyphs.rs, palette.rs, view.rs), docs/systems/diff.md, the evidence FS1 §5-6 and FS2 §1b (shots uv2f, uv8, uv9z, uv10, uw5); do not read the planning docs beyond
        state.md and this file directly (the PRD sections and evidence named below are the
        exception: read those).
STEP 2  Implement:
        1. The tab strip (R7.1) with the hide-details icon on the right (03's action).
        2. The Commit tab (R7.2): the author block with its 42 px avatar, name, email and date;
           COMMITTER only where it differs — first observe with /observe-fork a commit whose
           committer differs from its author (name alone, email alone, time alone), and build
           what Fork shows; REFS absent without refs; an opened file's diff at the
           diff's row height.
        3. The Changes summary's avatar (R7.3); the diff bar (R7.4): Fork's order, Lucide glyphs,
           tooltips verbatim with chords from the table; `toggle_glyphs.rs` retired if Lucide
           replaces every glyph.
        4. The diff's bands and hunk header in Fork Dark's tones (R7.5).
        Invariants in play: `every_view_of_a_file_diff_names_every_state`, `every_view_of_a_diff_row_names_every_kind_of_row`, the diff and Commit-tab virtualization twins, the style guards, no literal chord in a tooltip.
        Out of scope: File Tree (#31), Quick Look (#35), hidden symbols (#33), word wrap (#34), the pop-out window, the pane to the right (#30).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build over the fixture
        (docs/research/ui-review/fixture-recipe.md) at 1024x768 and compare against FS1/FS2's
        shots (qa-checklist item 7). Then orchestrate this phase's QA in this session: /qa over
        the phase diff, with the reviewers implementation-plan.md's dispatch table names for
        this phase, spawned fresh, plus the qa-checklist.md items and this file's QA brief.
        Adjudication goes to a fresh qa-confirm agent, never this session inline. Log dismissed
        findings with reasons in progress.md. Fix confirmed findings in focused commits.
        Disputed findings go to the user.
STEP 4  Acceptance: C15.
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md (newest first);
        save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/fork-alignment; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/fork-alignment, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when Fork's glyph for a bar button has no Lucide equivalent close enough to read as Fork's (show the user both), or the in-place row height change breaks `cairn_ui::Expansion`'s placement; otherwise do not stop for permission.
```

## QA brief

- Are the tooltips verbatim to FS1 §6? Are the chords from the table?
- Is COMMITTER hidden only where all three of name, email and time agree?
- Do the Commit tab twins (`only_a_viewport_of_rows_is_built_however_many_files_are_open`) still
  hold at the new row height?
