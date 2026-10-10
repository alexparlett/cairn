# Phase 16 — One selection

Rule 8 and L2, L3, L5 of the user's decisions of 2026-10-10: Local Changes' selection is one
value, with a primary row, every route acts on it, and one pure rule says where it goes after an
action and after any refresh. The diff still draws what it draws today while this lands; phase
17 makes it show the primary alone.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 12-15 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/list_selection.rs,
        local_changes.rs, local_changes_drag.rs, local_changes_menu.rs,
        accelerators.rs (the Stage All press and its held key),
        crates/cairn-app/src/local_changes_actions.rs, local_changes_pane.rs
        (Follow, follow, follow_the_lists), local_changes_state.rs,
        diff_state/working.rs (WorkingChoice), session.rs (the Status and filter
        arms) and the bare-key pin in accelerators.rs. Read
        docs/prd/staging-and-commit.md R7.3, R8.1-R8.3, C18, C25, C27,
        docs/research/staging-and-commit/brief-local-changes-selection.md
        sections 1, 3.1, 3.3, 3.4, 3.6 and 5, and fork-observed-2026-10-10.md
        ("Local Changes selection"). Do not read the other planning docs
        directly.
STEP 2  Implement.
        1. The value (rule 8): ListSelection becomes one LocalSelection — the
           list it is in, its paths (never empty while it exists), a primary and
           an anchor — owned by LocalChangesState alone; None only when nothing
           is selected. A press selects a row (primary and anchor); ⌘- or
           Ctrl-click toggles, a row toggled in becomes primary, the primary
           toggled out moves to the nearest selected path; Shift-click and
           Shift+↑/↓ span anchor to row, the row primary. The primary is the file
           clicked first (observed `lc2`): a Ctrl-click that adds a row does NOT
           make it primary when a primary exists — reconcile with
           brief-local-changes-selection.md 3.1, which says it does; Fork
           observed wins (rule 1). The diff's chosen path is derived from the
           primary; WorkingChoice stops being state of its own.
        2. One rule (R8.3, L5, C25): a pure next_selection(selection, before,
           after, leaving) in cairn-app or cairn-model — kept paths kept, the
           primary kept or the nearest kept; else the row in the first one's
           place, else the nearest above; None when the list is empty — called
           from session::apply as the status or the filter's rows arrive, and at
           once after an action with the acted rows leaving. The filter prunes
           the selection to the rows it shows (L3). Follow, follow and the
           follow_the_lists render effect go, with their five cases.
        3. Every route acts on the selection, wherever focus is (rule 8): the
           header's Stage or Unstage greyed while the selection is in the other
           list; double-click, drag, menu and chords funnel through one action
           over the selection; the fallbacks that synthesised a selection from
           the chosen path go.
        4. Stage All / Unstage All (L2, C27): the held key is Shift on Linux and
           ⌥ on macOS, resolved through the accelerator table and HeldKeys as
           today; while held, both headers read Stage All and Unstage All,
           enabled whatever is selected; ⇊ in Unstaged and ⇈ in Staged, small,
           always drawn. Alt held does nothing. With a filter on, each takes the
           rows shown.
        Tests: C25's rule as unit tests (after an action, after a refresh, a
        vanished path, gaps, a filter, an outside commit like `lc5`); C18 and
        C27 headless; the window test that a refresh no longer goes through a
        render effect.

        What goes: list_selection.rs's "empty but still the list's" state;
        local_changes_pane.rs's Follow enum, follow and follow_the_lists;
        local_changes_actions.rs's chosen-path fallbacks (acted_rows', toggle's
        synthesis, range's anchor fallback, chosen_is) and move_to's and
        everything's separate selection resets; WorkingChoice's list and path as
        independent state; Alt as the held key on Linux.

        Review refactors: review-code-app-ui.md M4 (selection reconcile) — in
        this phase; H1 (the lane owner) — in phase 17, which removes the fourth
        flag; H2 (one pointer-drag primitive) — follow-up issue at teardown:
        this phase does not rewrite the drag's lifecycle.

        Invariants in play: no component names a literal modifier (Shift
        resolves through the table, never `.shift()` in a component); the UI
        thread never waits and works in proportion to the selection, never the
        lists (update CLAUDE.md's residual paragraph on Local Changes' actions);
        no unbounded list renders without virtualization.

        Out of scope: what the diff draws (17); the line selection (17).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 16, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C18, C25 (all but the diff's half), C27.
STEP 5  Update state.md and progress.md; docs/systems/local-changes.md (the
        selection, its rule, the routes); the root CLAUDE.md status paragraph and
        repo map rows (list_selection.rs, local_changes_pane.rs). Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if Shift held on Linux collides with
Shift-click's range extension on the header buttons in a way Fork for Windows
does not show; if the primary's "file clicked first" rule leaves a case the
observation does not settle (the primary toggled out, then a range). Otherwise
do not stop for permission.
```

## QA brief

The risk is a file acted on that the person did not select, or cannot see.

- After every action and every refresh, the selection holds only listed, shown paths.
- A refresh with nothing changed leaves the selection, its primary and its anchor exactly as they
  were.
- No route — chord from the diff, double-click, drag, header, menu — acts on anything but the
  selection.
