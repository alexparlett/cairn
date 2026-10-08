# Phase 06 — Keys, dialogs, menus and auto-scroll in the render layer

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-05 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/accelerators.rs
        (scopes, resolve_key, HeldKeys, Trigger::Press, the pinned tests),
        crates/cairn-app/src/window.rs and shortcuts.rs (the global key
        handler), the credential dialog's component, the filter fields
        (changes_list.rs, the sidebar's), the vendored freya at rev caa46f8
        under ~/.cargo/git/checkouts/ (input.rs, popup, context_menu, a11y_modal,
        drag_zone, use_scroll_controller), docs/prd/staging-and-commit.md (R7,
        C15, C16, C17) and docs/research/staging-and-commit/freya-ui-apis.md.
        Do not read the other planning docs directly. Verify every Freya API in
        the vendored source before writing it.
STEP 2  Implement.
        1. The text-field key policy (R7.1): one shared pre-key handler that asks
           the accelerator table; adopt it in every existing field. Write C15's
           filter-field test FIRST and see it fail on today's code.
        2. The accelerator table (R7.2, R7.3): an action maps to a list of
           distinct chords per platform, where the contract was
           `chord(action, platform) -> Option<Chord>` (L22) — every caller and
           test moves to the list, and the distinctness pin holds across every
           chord of every list; the bare-key scope and the pinned rule's
           amendment (chords_are_distinct_and_every_bare_one_is_a_function_key
           admits bare Enter, Backspace and Delete in Local Changes' list-and-diff
           scope only — the test must fail on a bare chord anywhere else); the new
           actions, their chords per platform and each action's scope — stage,
           unstage, stage all and discard in Local Changes' list-and-diff scope,
           commit in the commit box, Show Lost Commits in the history — so no
           chord but commit's resolves while the commit box holds focus (C16
           tests the scopes); the multi-select presses.
        3. The confirmation dialog (R7.4): a wrapper over Popup with a11y_modal,
           focus trapped and on Cancel, Escape cancelling, the window's chords
           inert while it is open, prompt and button rendered from a
           Consequence; this file is the first entry on phase 01's
           confirmation-surface roster. The context menu host at the window root
           (R7.5).
        4. Edge auto-scroll for a drag over a virtualized list (R7.6), timed with
           async-io's timer — add the dependency to cairn-ui with its allowlist
           row and deny.toml reason (L5 is the user's decision; the row is this
           phase's).

        Invariants in play: no component names a literal modifier (the bare-key
        scope is in the table, nowhere else); the UI thread never waits (the
        timer is a future on the toolkit's executor); every dependency addition
        is recorded; the accelerator table holds data and resolution only.

        Out of scope: Local Changes' actions (07), the gesture (08), the commit
        box (09).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 06, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C15, C16, C17.
STEP 5  Update state.md and progress.md; docs/systems/diff.md's accelerator
        table contract (a list of chords per action, each action's scope); the
        root CLAUDE.md modifier invariant — "at most one chord per platform"
        amended to a list of distinct chords per platform (L22), and its twin
        with it — and the bare-key scope with its twin. Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the key-policy hole turns out not to
exist (C15 passes before the fix); if a Fork chord collides with an existing
Cairn chord; if async-io needs a feature or a second crate beyond what the
lockfile holds. Otherwise do not stop for permission.
```

## QA brief

The risk is a key that does two things, or a dialog that can be clicked past.

- With the commit description focused: Backspace deletes a character and never
  discards; Enter inserts a newline and never stages; ⌘Return / Ctrl+Enter
  commits.
- With the dialog open: F5, Tab out, a click behind it and the bare keys do
  nothing behind it.
- The amended pin: a bare Enter added to any other scope must fail the test, and
  a chord listed twice for one action must fail it too.
