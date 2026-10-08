# Phase 09 — The commit box

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-08 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over phase 05's commit, amend and
        reads, phase 06's key policy and dialog, phase 07's Local Changes
        layout, the vendored freya Input (.multiline, .caret, on_pre_key_down)
        at rev caa46f8, ResizableContainer's uses in Cairn,
        docs/prd/staging-and-commit.md (R10, C13, C14, C24) and
        docs/research/staging-and-commit/fork-staging-and-commit.md section 5.
        Do not read the other planning docs directly.
STEP 2  Implement.
        1. The box (R10.1, R10.2, R10.7): subject with its counter, description
           with its ruler, Recent Commit Messages, `Commit N Files` and its
           chord, the draft kept as Cairn's state for the window's life.
        2. Amend (R10.3, R10.6): the HEAD message loaded into an empty draft and
           the user's draft kept; amend's staged list; the button and its
           consequence line, which construct Confirmed (this file joins phase
           01's confirmation-surface roster); the pushed dialog through phase
           06's wrapper. Amend disabled with no HEAD and during a merge (R10.1).
           The Amend toggle reports its checked state to assistive technology:
           the linked freya's `Checkbox` sets no toggled state
           (`freya-ui-apis.md` §3), so the wrapper adds it through
           `a11y_builder`.
        3. Running and failing (R10.4, R10.5): busy, elapsed, Cancel; Fork's Git
           Error dialog with ANSI stripped and the skip only where a hook exists,
           its output drawn through a virtualizing view or bounded, never a
           ScrollView; the draft kept.
        4. An operation in progress (R10.8, L25): with a merge, an empty draft
           filled from MERGE_MSG and the commit the merge commit; during a
           rebase, cherry-pick or revert, the box disabled and naming it.
        Tests: C13, C14 and C24's view halves, headless, and the Amend toggle's
        toggled state read from the accessibility tree.

        Invariants in play: no literal modifier; the UI thread never waits;
        Confirmed constructed only by rostered surfaces; no message on argv.

        Out of scope: the activity popover (11).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 09, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C13 (views), C14 (views), C24 (views: the commit box).
STEP 5  Update state.md and progress.md; docs/systems/local-changes.md (the
        commit box); the root CLAUDE.md status paragraph and repo map rows. Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the linked Input cannot keep the draft
across Amend without losing what was typed; if Fork's counter thresholds
conflict with what the subject field can draw. Otherwise do not stop for
permission.
```

## QA brief

The risk is a draft lost, or an amend nobody meant.

- Type, tick Amend, untick: the typed draft is back exactly.
- A failing hook, then the skip: the commit carries the draft, and `--no-verify`
  was passed only for that one commit.
- The amend button's text, the `Consequence` and the commit replaced: one HEAD
  id in all three.
