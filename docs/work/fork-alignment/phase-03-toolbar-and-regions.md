# Phase 03 — the toolbar, the status box and the window's regions

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/fork-alignment/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/fork-alignment before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/fork-alignment`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/window.rs (the title row), crates/cairn-ui/src/title_bar.rs and title_bar/ (status_box.rs, lock_banner.rs, activity_popover.rs), crates/cairn-app/src/operations/ (the fetch wiring), crates/cairn-app/src/detail/pane.rs (the splitter, Collapse), docs/systems/git-processes.md (the status box and banners as built); do not read the planning docs beyond
        state.md and this file directly (the PRD sections and evidence named below are the
        exception: read those).
STEP 2  Implement:
        1. The toolbar (R3.1, R3.2): Fork's buttons in order, icon over label, the greyed ones
           disabled with their "Comes with <feature>." tooltips, Fetch and Branch working, the
           sidebar toggle; Branch's ⌄ a `cairn_ui::menu` of New Branch… alone.
        2. The status box moved into the toolbar's middle (R3.4): the repository over the
           branch, git's words for a detached or unborn HEAD, "upstream gone", its glyph opening
           the popover, a running operation shown as before; the badges on Push and Pull (R3.3);
           the title row's count, star and counts removed. If `title_bar` is renamed, the root
           CLAUDE.md, docs/systems/ and every guard path move in the same commit.
        3. The banners under the toolbar (R4.1); the history and detail pane sharing the height
           by proportion and growing together (R4.2); Hide Commit Details — the View item, the
           chord, the pane's icon replacing Collapse (R4.3) — Ctrl+Shift+D moved from Discard
           to ToggleDetails on Linux (L9): the table, its pin's examples and the root
           CLAUDE.md's accelerator paragraph in one commit, macOS per R10.1; the subject column
           shrinking first (R4.5).
        4. The minimum window size (R4.4), measured in the real window and set in the config;
           recorded in docs/systems/.
        Invariants in play: the status box's 250 ms rule and its timer (a task of its line's scope, never a sleep), the activity popover's virtualization twin, the lock banner's timer, the UI-thread guard, the style guards.
        Out of scope: the history's rows and header (04); the sidebar (05); Local Changes (07).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build over the fixture
        (docs/research/ui-review/fixture-recipe.md) at 1024x768 and compare against FS1/FS2's
        shots (qa-checklist item 7). Then orchestrate this phase's QA in this session: /qa over
        the phase diff, with the reviewers implementation-plan.md's dispatch table names for
        this phase, spawned fresh, plus the qa-checklist.md items and this file's QA brief.
        Adjudication goes to a fresh qa-confirm agent, never this session inline. Log dismissed
        findings with reasons in progress.md. Fix confirmed findings in focused commits.
        Disputed findings go to the user.
STEP 4  Acceptance: C7, C8, C9, and C18's Ctrl+Shift+D half.
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md (newest first);
        save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/fork-alignment; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/fork-alignment, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when the status box cannot show a running operation and the repository at the narrow widths without cutting text (product rule: no text a person needs is cut); the minimum size Fork's layout needs is larger than 1024x640; or moving `title_bar` breaks a guard's path roster in a way the guard's owner must decide; otherwise do not stop for permission.
```

## QA brief

- Are the greyed buttons' sentences exactly L2's, and does no greyed button do anything on a
  press or a key?
- Do the badges update on each refresh's ahead/behind, and are they absent at zero?
- Is Expand All reachable at the minimum size and at 1024x1200?
- Did each guard path that moved with the files move in its guard too, and fail before the
  change?
