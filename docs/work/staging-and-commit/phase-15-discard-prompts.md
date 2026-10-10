# Phase 15 — The discard prompts

The user's decisions of 2026-10-10 D1, D2, D3 and rules 2, 3 and 6: one sentence frame, the
detail behind "Show files", Title Case, a mixed selection discarding what it can, and the dialog
opening at once at the press.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 12-14 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-model/src/consequence.rs
        (discard_files_prompt, discarded_lines, line_detail, NAMED_FILES, the
        mode prose), crates/cairn-git/src/ops/discard.rs (the two consequence
        builders), crates/cairn-ui/src/confirm_dialog.rs,
        crates/cairn-ui/src/local_changes_menu.rs (no_discard, NoDiscard, the
        reason's max_lines(2)), crates/cairn-app/src/local_changes_actions.rs
        (the discard's ask, Acting, READING_DISCARD) and confirming.rs. Read
        docs/prd/staging-and-commit.md R1.2, R7.4, R8.4, R8.8, R14.4, R14.7, C17,
        docs/research/staging-and-commit/brief-discard-and-create-branch.md
        sections 4a, 4b and 5F-5H, and fork-observed-2026-10-10.md
        ("Discarding"). Do not read the other planning docs directly.
STEP 2  Implement.
        1. The frame (R1.2, D2): the Consequence renders one sentence —
           "Discard all changes in <path>?" for one file, "Discard all changes in
           31 files?" for several, the worst loss only when it is worse than
           changes ("2 untracked files will be deleted."), then "You can't undo
           this." — and a per-file list for Show files: each path and what
           happens to it (Restored, Deleted, Emptied, its lines, a mode change in
           words, no octal). Lines: "Discard 2 changed lines in src/main.rs? You
           can't undo this.", a chunk "Discard this chunk (6 lines) in
           src/main.rs?", a mode change "Discard the mode change of run.sh?", no
           list. The button counts what it discards ("Discard Changes in 31
           Files", "Discard 2 Lines"). Before rewriting the "emptied" wording,
           settle N3 with a real-git test: discard an `add -N` file and read
           what git leaves.
        2. Title Case everywhere a dialog or button is named (D3): "Discard
           Changes", "Amend Commit", "Remove Stale Lock" stays the user's
           decision G's words ("Remove stale lock") — ask before changing it.
        3. The dialog (R7.4, rules 3 and 6): opens on the press, "Counting…"
           with its button greyed until the Consequence arrives; Show files a
           disclosure over a virtualized list; the prompt never cut (no
           max_lines on it); Escape and Cancel back one step to what is beneath.
        4. A mixed selection (D1): the discard asks for the discardable rows
           only, and one line under the prompt says what is left ("1 submodule
           and 1 conflicted file are left as they are."); the button counts only
           what it discards. A selection with nothing discardable: the menu item
           greyed with its reason wrapped beneath it, one template "<what> can't
           be discarded. <way forward>." — staged, submodule, conflicted — and
           the discard chord over it does nothing (rule 4, as Fork). A nested
           repository is still refused before any dialog.
        Tests: C17 whole, C8's views re-run; a viewport twin for Show files over
        50,000 paths; the prompt tests pinning literal text rewritten to the
        frame, including hostile paths' escaping.

        What goes: discard_files_prompt's per-kind sentence grammar
        (modified / emptied / restored / deleted), line_detail and the octal
        mode prose in sentences, NAMED_FILES naming the first three; no_discard's
        first-offender refusal and its three voices; the reason's max_lines(2);
        READING_DISCARD as a strip text (it moves into the dialog); the
        "Staged changes can't be discarded: unstage them first." line under the
        lists.

        Review refactors: none of the review's code refactors are in this
        phase; review-ux-commit-and-dialogs.md H5, M4 and review-ux-local-changes.md
        M1-M3 are its UX findings, fixed here.

        Invariants in play: the prompt is rendered from the Consequence and is
        the Confirmed prompt; only a confirmation surface builds a token; no
        unbounded list renders without virtualization (Show files); the UI
        thread never waits (the count stays on the local lane).

        Out of scope: where a discard's failure or refusal is shown (19).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 15, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C17; C8 (views).
STEP 5  Update state.md and progress.md; docs/systems/staging.md and
        docs/systems/local-changes.md (the discard dialog). Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user before changing decision G's "Remove stale
lock" title to Title Case; if N3's real-git check shows git removes an
intent-to-add file (the prompt's word then changes in meaning, not only form).
Otherwise do not stop for permission.
```

## QA brief

The risk is a short prompt that hides a loss.

- The worst loss in the sentence is the worst one in the list: a selection with an untracked
  file always says it will be deleted.
- Show files lists exactly the paths the discard takes, and a path left as it is is never among
  them.
- The button's count equals what is discarded, a mixed selection's included.
