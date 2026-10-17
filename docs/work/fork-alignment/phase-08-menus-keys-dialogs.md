# Phase 08 — context menus, keys, focus and dialogs

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/fork-alignment/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/fork-alignment before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/fork-alignment`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/accelerators.rs and its tests, crates/cairn-app/src/shortcuts.rs, every context menu's data (history/list.rs, local_changes/menu.rs, detail/changes_list.rs, commit_tab.rs, sidebar), crates/cairn-ui/src/dialogs/ (confirm.rs, git_error.rs, create_branch.rs, button_order.rs, answer_button.rs) and the credential prompt, crates/cairn-ui/tests/dialog_buttons.rs, the root CLAUDE.md's accelerator paragraph, the evidence FS1 §8 and FS2 §1d-e, §3; read state.md, this file, implementation-plan.md and
        the PRD sections and evidence named below directly; nothing else of the planning docs.
STEP 2  Implement:
        1. Every context menu's items, exactly R9.2, through `cairn_ui::menu`; Copy Commit
           Info's form checked in the VM first (/observe-fork) and recorded; a long-menu
           virtualization twin `a_long_menu_builds_one_viewport` (C21).
        2. The `Copy` and `CopyFull` actions (Ctrl+C, Ctrl+Shift+C), each read by the focused
           view's menu data; the macOS column of every action phases 02-08 added, per R10.1 (ToggleDetails with no macOS chord: ⌥⌘D is the Dock's).
        3. Focus at every Tab stop (R10.2) and Local Changes reachable by keyboard (R10.3):
           `every_tab_stop_shows_focus`, `ctrl_1_opens_local_changes_with_unstaged_focused`.
        4. Every dialog's look and filled primary (R11.1); the "reopen the repository" message
           reworded (R11.3).
        Invariants in play: the accelerator table's pin and `the_accelerator_table_holds_data_and_resolution_only`, the modifier and chord-name guards, the text-field policy (Ctrl+C inside a field copies its text), the confirmation seal's surfaces (the confirmation and Create Branch dialogs build the token as before; only their look changes), the dialog button order, the style guards.
        Out of scope: any item of another packet (L2); Quick Launch; Preferences.
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build over the fixture
        (docs/research/ui-review/fixture-recipe.md) at 1024x768 and compare against FS1/FS2's
        shots (qa-checklist item 7). Then orchestrate this phase's QA in this session: /qa over
        the phase diff, with the reviewers implementation-plan.md's dispatch table names for
        this phase, spawned fresh, plus the qa-checklist.md items and this file's QA brief.
        Adjudication goes to a fresh qa-confirm agent, never this session inline. Log dismissed
        findings with reasons in progress.md. Fix confirmed findings in focused commits.
        Disputed findings go to the user.
STEP 4  Acceptance: C17, C18, C19, C20, C21 (menus half).
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md (newest first);
        save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/fork-alignment; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/fork-alignment, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when a chord Fork uses collides with a desktop or a text field's binding on Linux; Copy Commit Info's form is not observable; or a dialog's look change would touch how its token is built; otherwise do not stop for permission.
```

## QA brief

- Does every menu hold exactly R9.2's items, and does no menu hold an item that does nothing?
- Is the pin's refusal of a chord listed twice still shown failing on its examples with
  Copy and CopyFull added?
- Does Ctrl+C in the commit box's subject copy its text, and in the history the commit's id?
- destructive-ops-reviewer: do the confirmation and Create Branch dialogs still build their token
  only on their button, from the engine's `Consequence`?
