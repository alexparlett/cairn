# Phase 11 — The activity popover, and the measured bar

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-10 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over the command log
        (cairn_model::command_log, the registry in crates/cairn-git/src/process/,
        what the worker answers of it), Performed and phase 04's outcomes, the
        toolbar's status area, crates/cairn-app/src/window_check.rs,
        docs/research/staging-and-commit/measured-baseline.md (phase 03's),
        docs/prd/staging-and-commit.md (R12, R13, C2, C20, C21, C22),
        crates/cairn-guards/tests/invariants.rs (only_the_ops_module_mutates_a_repository
        and its matcher/self-test pattern) and issues #41 and #46 (gh issue
        view). Do not read the other planning docs directly.
STEP 2  Implement.
        1. The popover (R12.1, R12.3): one entry per operation of the session,
           newest first, bounded and virtualized — name, start, duration,
           outcome, confirmed prompt, each git with its stderr tail (drawn
           through a virtualizing view or bounded, never a ScrollView), a
           recovery pointer (an amend's replaced commit opens Show Lost Commits
           on it).
        2. Scrubbing (R12.2): userinfo removed from every URL in drawn stderr,
           with tests for http(s), ssh-style and a URL split across a line.
        3. `Remove index.lock…` (R12.4, L23): offered exactly when an outcome
           names a stranded lock and the registry holds no running git there; a
           destructive operation in ops/ on phase 01's roster, its Consequence the
           path and age; it re-checks the lock's age and identity against the
           Consequence, then removes exactly <gitdir>/index.lock with std::fs —
           the one mutation not made by git, since git has no verb for it.
           The filesystem-mutation guard (R12.5): no production file outside
           crates/cairn-git/src/ops/ removes, writes, renames, creates or changes
           the permissions of a file or directory, beyond an exceptions roster
           for what writes outside any repository (the askpass channel's socket
           directory, crates/cairn-askpass/src/channel.rs), each row failing when
           no longer needed; a nonzero-files assertion and a matcher self-test.
        4. The measured bar (R13.1, C21): #[ignore]d reporters for stage,
           unstage and discard a hunk and commit, from the press to the
           refreshed lists drawn, and Show Lost Commits' first frame, on phase
           03's tmpfs clone; window_check extended with a running hook and a
           landing stage. Numbers into progress.md beside the baseline.
        Tests: C2's lock half (a lock removed and made again since the prompt
        is refused); C20's popover half, the exact-file removal and the guard;
        C21 by reporters.

        Invariants in play: no credential logged, printed or stored; only ops/
        mutates; destructive operations take Confirmed by value; no unbounded
        list; the UI thread never waits.

        Out of scope: a persisted log (filed).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 11, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C2 (lock), C20 (the popover, Remove index.lock…, the
        filesystem-mutation guard), C21, C22 (D1's one deletion and the guard's
        twin).
STEP 5  Update state.md and progress.md; the activity popover in
        docs/systems/git-processes.md beside the command log; the root CLAUDE.md
        D1 paragraph ("Reads go through gitoxide; writes go through the git
        binary") states the one deletion made without git and its reason (C22),
        and the "only cairn-git/src/ops/ mutates a repository" invariant names
        the filesystem-mutation guard, its twin, self-test, exceptions roster and
        residuals; close #41 and record #46's display half. Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if any C21 number misses its bar; if the
registry cannot tell Cairn's own running git from none; if the bench
repository's .git changes. Otherwise do not stop for permission.
```

## QA brief

The risk is a secret on screen, or a lock removed from under a live git.

- A remote URL with a token in a hook's stderr: not drawn, anywhere — popover,
  Git Error dialog, tooltip.
- `Remove index.lock…` while a stub git holds the lock and runs: not offered.
- The guard: a `std::fs::remove_file` added to a file outside ops/, or reached
  through an alias, must fail it.
- The reporters time what the user waits for (press to drawn), not git alone.
