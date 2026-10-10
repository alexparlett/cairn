# Phase 12 — git's output read once

The first phase of the rebuild the user approved on 2026-10-10
(`docs/research/staging-and-commit/redesign-decisions-2026-10-10.md`). Engine and the lane
boundary only; no view changes what it draws.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-11 and the merge bar's fixes (2bf8394) are
        present at the integration tip. Direct integration work requires an
        orchestrator prompt that explicitly declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/process/
        (pipes.rs, runner.rs, registry.rs, command_log.rs), ops/commit.rs's
        output handling (Lines, whole_characters, STDOUT_TAIL, joined_output),
        error.rs (stderr_cut), crates/cairn-model/src/scrub.rs and
        command_log.rs, and in cairn-app worker/local_lane.rs (scrubbed_error,
        ran_since), worker/network_lane.rs, worker/output_flow.rs,
        shown_output.rs, session.rs (shown_ending), commit_box_state.rs
        (OutputTail, strip_ansi), create_branch.rs and activity.rs where each
        scrubs or cuts. Read docs/prd/staging-and-commit.md R4.8, R4.10, R12.2
        and C31, and docs/research/staging-and-commit/review-code-engine.md H3
        and M7 and review-code-app-ui.md H3. Do not read the other planning
        docs directly.
STEP 2  Implement, test-first (the extract-and-test skill's bug recipe).
        1. The live bug first (R4.10, C31): a failing test that a stderr line
           longer than pipes.rs's piece limit, with a multi-byte character
           straddling the limit, arrives with the character whole; then the fix.
        2. One line type in process/pipes.rs, used for stdout and stderr alike:
           whole lines, split where git splits them (decide `\r` once, for both
           streams, and pin it), never inside a character; a line longer than
           the piece limit is handed on in whole-character pieces.
        3. Scrub once, at the runner, on whole lines before anything is kept or
           cut: each line passes cairn_model::scrub_userinfo as it is split; the
           kept tail holds scrubbed whole lines and drops the oldest whole ones,
           and "older lines dropped" is a plain bool. The scrubber keeps only
           the carry for a line longer than one piece.
        4. ops/commit.rs drives the runner's one line stream; its own Lines,
           whole_characters, STDOUT_TAIL and joined_output go. Commit's stdout
           and stderr reach the operation log as the runner's lines.
        5. The lane boundary (review-code-app-ui.md H3 (a)): git's text crosses
           into cairn-app only as scrubbed lines — a type in cairn-model, say
           ScrubbedLines, that only the engine builds — so no render file takes
           git's raw String; every per-consumer scrub in cairn-app (local_lane,
           network_lane, session, create_branch, commit_box_state, activity)
           reads the type instead of scrubbing again. strip_ansi moves to
           shown_output.rs, its one owner. If a guard is the cheapest way to hold
           "no render file takes git's raw text", write it with a self-test; if
           the type alone holds it, say so in CLAUDE.md.
        Tests: C31 in cairn-git (the straddling character on both streams, a
        URL's userinfo split across two reads, a cut line count), and the app's
        tests that pinned per-consumer scrubbing, moved to the type.

        What goes: process/pipes.rs's second line definition and its lossy
        piece decode; Scrubber::after_cut and the "try every byte as the cut"
        mode; Error::GitFailed's stderr_cut: Vec<usize> and the runner's
        `.then_some(0).into_iter().collect()`; Retained.cut as an offset;
        ops/commit.rs's Lines, whole_characters, STDOUT_TAIL, joined_output and
        its Polled adapter where the runner's stream serves; the scrub calls
        in local_lane::scrubbed_error and ran_since, network_lane, session's
        shown_ending, create_branch and commit_box_state's ShownLines.

        Review refactors: review-code-engine.md H3 and M7 — in this phase.
        review-code-app-ui.md H3 (a), one scrubbed type — in this phase; H3 (b),
        one bounded tail, and (c), the receipt protocol replaced — in phase 20,
        which rewrites the activity log's stores.

        Invariants in play: only process/ builds or runs a process; the runner
        is named only by ops/ and reads/; the bounded-output helpers exist on a
        read alone (R4.8); no credential value is logged or stored; the UI
        thread never waits.

        Out of scope: what any view draws (phases 18-20).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 12, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C31.
STEP 5  Update state.md and progress.md; docs/systems/git-processes.md (the
        runner's pipes, what it reports); the root CLAUDE.md where it names
        Scrubber, shown_output and the runner. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if splitting stderr at `\r` and stdout not
(or the reverse) turns out to be what git's own progress needs, so one line
definition would change what a fetch's progress shows; if a guard against raw
git text in render files cannot be written without a roster of exceptions.
Otherwise do not stop for permission.
```

## QA brief

The risk is a credential drawn because a cut moved, or a line lost between two reads.

- A URL with userinfo split across two reads of a pipe, and across the piece limit: never drawn,
  never kept in the command log's drawn lines.
- A 4-byte character at every offset around the piece limit, on stdout and stderr: whole.
- A commit's hook output still streams line by line to the popover while it runs.
