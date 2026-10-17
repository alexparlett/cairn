# Phase 02 — the menu component, the caption row and the menu bar

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/fork-alignment/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/fork-alignment before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/fork-alignment`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/main.rs, window.rs, modal.rs, shortcuts.rs, crates/cairn-ui/src/accelerators.rs (and chord_names.rs), the existing context menus (crates/cairn-ui/src/history/list.rs, local_changes/menu.rs, commit_box.rs), the vendored Freya Menu/SubMenu/MenuButton/ContextMenu, titlebar, window_drag and BorderlessPlugin sources (FA §1, §2, §5); do not read the planning docs beyond
        state.md and this file directly (the PRD sections and evidence named below are the
        exception: read those).
STEP 2  Implement:
        1. `cairn_ui::menu`: one component drawing a menu from data — groups with separators,
           a right-aligned chord column from `accelerators::chord_name`, greyed items with their
           reason as tooltip, submenus, keys per PRD R2.6 and R9.1; the three existing context
           menus moved onto it with their items unchanged (phase 08 changes the items).
        2. Linux: the caption row (R2.1, R2.2) — no OS decorations, menus left, "Cairn"
           centred, three `TitlebarButton`s, `window_drag`, double-press maximize, the window
           menu, `BorderlessPlugin` resize edges; `freya`'s `titlebar` and `borderless`
           features added to the `deny.toml` pin, `freya-borderless-plugin` the one new crate
           (L10). The OS-level title "<repository> — Cairn" (R2.4).
        3. The menu bar's items, exactly R2.5, in Fork's order and groups; on macOS the same
           menus as a row inside the native frame, no `muda` (R2.3, L10). First observe with
           /observe-fork how Fork marks Hide Commit Details and Show Lost Commits while on (a
           changed label, a tick) and build what it shows. New accelerator actions this needs
           (ShowLocalChanges, ShowAllCommits, ShowHead, ToggleDetails, Fetch, ZoomIn, ZoomOut)
           in the table with their Linux chords — ToggleDetails with none until phase 03 moves
           Ctrl+Shift+D to it; its menu item shows no chord meanwhile; their
           behaviour where it already exists (Refresh, New Branch, Show Lost Commits, Fetch);
           Zoom through Freya's runtime scale factor (never `zoom-shortcuts`).
        4. About Cairn (R11.2) under the one modal state.
        Invariants in play: the accelerator table's pin (no chord twice, bare keys only where admitted), `no_component_names_a_literal_modifier` and the chord-name roster, the text-field policy, the one modal state and `keys_inert`, the UI-thread guard (nothing waits on a render path), the style guards from phase 01.
        Out of scope: the toolbar and status box (03); Show Lost Commits' header check box (04 removes it); Ctrl+Shift+D's move from Discard (03; until then the View menu's Hide Commit Details shows no chord); context-menu item changes (08).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build over the fixture
        (docs/research/ui-review/fixture-recipe.md) at 1024x768 and compare against FS1/FS2's
        shots (qa-checklist item 7). Then orchestrate this phase's QA in this session: /qa over
        the phase diff, with the reviewers implementation-plan.md's dispatch table names for
        this phase, spawned fresh, plus the qa-checklist.md items and this file's QA brief.
        Adjudication goes to a fresh qa-confirm agent, never this session inline. Log dismissed
        findings with reasons in progress.md. Fix confirmed findings in focused commits.
        Disputed findings go to the user.
STEP 4  Acceptance: C5, C6, and C20's About Cairn half.
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md (newest first);
        save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/fork-alignment; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/fork-alignment, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when the caption row cannot resize, drag or maximize on Hyprland or under X11; a compositor misbehaves in a way that suggests a fallback to the OS frame (a decision the user made the other way); a new accelerator chord collides with an existing one; or the macOS menu row cannot be built from the same data as Linux's caption-row menus; otherwise do not stop for permission.
```

## QA brief

- Every menu item does something, or is greyed with a reason. Is any item from L2's
  never-planned list present?
- Does each chord shown come from the table, never typed?
- Does a drag, a double press and a resize from each edge work in the real window on Hyprland?
  Say what was and was not verified on other compositors.
- Does the macOS menu row come from the same data? Is it compiled by `cargo check --target` where
  feasible? If not, is that
  recorded as unverified?
