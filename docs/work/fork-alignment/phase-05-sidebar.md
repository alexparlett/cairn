# Phase 05 — the sidebar

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/fork-alignment/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/fork-alignment before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/fork-alignment`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/sidebar.rs, crates/cairn-app/src/sidebar/ (state.rs, pane.rs), crates/cairn-model/src/sidebar_rows.rs, crates/cairn-ui/tests/sidebar.rs, docs/systems/sidebar.md, the evidence FS1 §3 and FS2 §2 (shots uv2c, uv4z, uv5all, uw27); do not read the planning docs beyond
        state.md and this file directly (the PRD sections and evidence named below are the
        exception: read those).
STEP 2  Implement:
        1. The header row with the repository's name and the gear's menu (Copy Repository Path)
           (R6.2); Local Changes and All Commits with their icons, the rounded selected band, the
           filter field with its magnifier (R6.3).
        2. The sections' order and disclosure defaults, the headings with chevrons and no counts
           (R6.4).
        3. The rows (R6.5): 12 px per level, leaf names in folders, the glyphs, ✓ for the current
           branch, tags newest first, a stash's message after `On <branch>: `, ahead right-
           aligned as "1↑" and behind as Fork draws it, observed first with /observe-fork (only
           ahead was seen); the sidebar walked by ↑/↓ and Return (R10.3); a model change (if any) with tests in the same commit.
        4. Each ref row's context menu through `cairn_ui::menu` with R9.2's items (New
           Branch…, Copy Branch Name / Copy Tag Name), the right press selecting the ref's commit
           (R9.3); the sidebar twins at 22 px rows (C21).
        Invariants in play: the sidebar's virtualization twins (`a_sidebar_of_50000_refs_builds_one_viewport`, `a_folder_of_10000_branches_open_builds_one_viewport`), the ref-filter lane (the filter's pass on the repository thread), a test in the same commit for any cairn-model change, the style guards.
        Out of scope: Submodules (#69); pin, show-only and hide (#2); commit search; remembering disclosure (#72, preferences).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build over the fixture
        (docs/research/ui-review/fixture-recipe.md) at 1024x768 and compare against FS1/FS2's
        shots (qa-checklist item 7). Then orchestrate this phase's QA in this session: /qa over
        the phase diff, with the reviewers implementation-plan.md's dispatch table names for
        this phase, spawned fresh, plus the qa-checklist.md items and this file's QA brief.
        Adjudication goes to a fresh qa-confirm agent, never this session inline. Log dismissed
        findings with reasons in progress.md. Fix confirmed findings in focused commits.
        Disputed findings go to the user.
STEP 4  Acceptance: C14, C21 (sidebar half).
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md (newest first);
        save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/fork-alignment; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/fork-alignment, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when Fork's tag order is not by date on a fixture where tag dates differ from commit dates (observe it with /observe-fork before choosing); or leaf names need the model to keep a folder's prefix it does not; otherwise do not stop for permission.
```

## QA brief

- Do the rows and their order match uv4z exactly on the fixture?
- Does a press, a right press and a find still behave as `docs/systems/sidebar.md` says?
- Is the layout still done on the repository thread, never on the UI thread?
