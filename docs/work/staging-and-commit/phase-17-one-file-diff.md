# Phase 17 — The diff shows one file

Rule 8 and L1, L4 of the user's decisions of 2026-10-10, and what was applied as Fork after the
Fork check (`fork-observed-2026-10-10.md` `lc2`-`lc4`, `ln1`, `ln2`): the diff shows the primary
file alone, the stage chord from the diff stages the whole selection, Return there does nothing,
and a finished line selection survives a refresh that leaves the text unchanged. Files drawn
together go.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 12-16 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/diff_state.rs,
        diff_state/together.rs, diff_state/working.rs, worker/diff_lane.rs and
        worker/request.rs (Together, TogetherQuery, TogetherOutcome),
        local_changes_pane.rs (together_wanted, draw_together, together_side),
        crates/cairn-ui/src/stacked_diff.rs, staging_gesture.rs (LineDrag,
        drawn), crates/cairn-model's ShownDiff, accelerators.rs (Scope, the
        bare-key pin, LOCAL_CHANGES_BARE_KEYS) and the guards that read it.
        Read docs/prd/staging-and-commit.md R7.2, R7.3, R8.1, R9.2, R9.6, C16,
        C19, C25, C26, docs/research/staging-and-commit/brief-local-changes-selection.md
        sections 3.2, 3.5 and 5, and review-code-app-ui.md H1. Do not read the
        other planning docs directly.
STEP 2  Implement.
        1. One file (R8.1, L1): the diff shows the selection's primary with
           everything one file's diff has; with several selected its bar keeps
           the primary's own header and adds "k of n selected" — unless the user
           has dropped the count (state.md, open questions), in which case the
           bar draws nothing extra; no fabricated ChangedFile. None selected:
           "No file selected".
        2. The lane owner (review-code-app-ui.md H1): with Together gone, the
           file-diff lane's holder becomes one Option of an enum (File, Working,
           Expansion) with one take, in place of the in_lane booleans and their
           cross-resets.
        3. The chords (R7.3 as amended, C16, C25): from the diff with no lines
           selected, Ctrl+S (⌘S) stages the whole selection; bare Return and
           Enter are heard from the file lists alone, never the diff, and
           Backspace and Delete stay heard from both. This splits Local
           Changes' scope (lists, diff) or adds a rule to it; amend the root
           CLAUDE.md modifier invariant's bare-key sentence, the table's pin and
           the guard's LOCAL_CHANGES_BARE_KEYS_DECLARATION in the same commit,
           each failing toward fewer bare chords, with the self-test showing a
           bare Enter in the diff's scope refused.
        4. The line selection keeps to its text (R9.6, L4, C26): ShownDiff gains
           a content hash computed on the diff thread from the path, the side,
           the options and the patch text; LineDrag records it in place of the
           answer number; an act is refused unless the drawn diff's content is
           the one it was made over. A refresh that draws the same content keeps
           the selection, its tint and its actions; one that differs clears it
           and the bar says "The file changed — line selection cleared." once,
           gone at the next key or press. A drag in progress is still cancelled
           when focus is lost.
        Tests: C19 (gesture over one file; the tabs draw no action), C25's diff
        half (the file clicked first; Ctrl+S from the diff stages the whole
        selection; Return in the diff does nothing; the selection then moves to
        the row in the first acted row's place), C26 headless and through a real
        refresh in a window test, C16's pin amended.

        What goes: diff_state/together.rs whole; stacked_diff.rs whole and its
        tests in staging_gesture.rs; Request::Together, Update::Together,
        TogetherQuery, TogetherOutcome and the diff lane's together arm;
        local_changes_pane.rs's together_wanted, draw_together and together_side;
        the four lane booleans; working.rs's answered, previous_answered and
        working_drawn; the tests that pinned files drawn together
        (the_chords_over_files_drawn_together_take_only_the_files_drawn,
        a_selection_over_files_drawn_together_is_nothing_once_a_page_replaces_its_diff,
        files_drawn_together_build_one_viewport) and CLAUDE.md's prose on the
        files-together view's per-frame cost.

        Review refactors: review-code-app-ui.md H1 — in this phase; H2 (one
        pointer-drag primitive) — follow-up issue at teardown.

        Invariants in play: no component names a literal modifier and every
        bare chord is a function key's but in Local Changes (amended here,
        narrower); DiffContent, UnifiedRow and SideBySideRow read by naming every
        variant; no unbounded list renders without virtualization (the
        gesture's 10,000-line twin stays); the UI thread never waits (the
        content hash is built on the diff thread).

        Out of scope: the commit box (18).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 17, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C16, C19, C25, C26.
STEP 5  Update state.md and progress.md; docs/systems/local-changes.md ("Several
        paths, drawn together" removed, "A refresh follows the path" rewritten,
        the gesture's line selection); docs/systems/diff.md's accelerator table
        contract; the root CLAUDE.md status paragraph, repo map rows
        (stacked_diff.rs, diff_state/together.rs gone), the modifier invariant
        and the virtualization twins' list. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the bare-key scope cannot be narrowed
without a guard that adapts rather than fails; if Ctrl+S is already a chord of
another action on Linux. Otherwise do not stop for permission.
```

## QA brief

The risk is a line selection that outlives the rows it named.

- A refresh whose re-read differs by one byte clears the selection; one identical in every byte
  keeps it, and an act after it stages exactly the selected lines.
- No act made before a refresh lands after it on other content.
- With several files selected and no lines, Ctrl+S from the diff and the stage chord from the
  list stage the same set.
