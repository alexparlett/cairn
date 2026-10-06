# Phase 02 — Status, as git answers it

```
STEP 0  Pre-flight: read docs/work/refs-and-status/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/refs-and-status before editing.
        Verify phase 01's work is present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/reads/
        (its module doc, working_tree.rs and fetch_settings.rs as the two
        precedents), crates/cairn-git/src/process/ (read_invocation, the
        runner, cancellation by process group), crates/cairn-git/src/ops/authority.rs
        (the status-as-read test), crates/cairn-guards/tests/invariants.rs
        (the_porcelain_reads_are_the_two_named_queries and the process twins),
        scripts/git-floor.sh, docs/prd/refs-and-status.md (R3, C4, C5, C11, C13)
        and docs/research/refs-and-status/status-agreement-spike.md (its fixture
        list and appendix). Do not read the other planning docs directly.
STEP 2  Implement.
        1. cairn-model: the status answer of R3.2 — per path, bytes not str:
           staged change (with a rename or copy's source), unstaged change,
           intent-to-add, submodule state, one of the seven conflict kinds, or
           untracked; and R3.7's "git cannot read this index" state. Plain data,
           tests in the same commit.
        2. cairn-git: reads::status, one named function, running
           `git status --porcelain=v2 -z` as a read invocation, untracked per
           L2, no --ignored, nothing overriding renames or submodules; parsing
           v2's `1`, `2`, `u` and `?` lines from bytes; cancelled by its epoch
           (the process group ended). Decide how status.showUntrackedFiles is
           read so it is git's reading (L2) — if that needs a second porcelain
           read, STOP and ask.
        3. Tests for C4 against independent git oracles (the PRD lists them),
           on the spike's fixture classes, under the host git and under
           git-floor's 2.30.9 and 2.32.7; the index byte-identical after every
           read; a stub-git test printing the exact argv; C5 through the runner
           (the registry empty after a cancel).
        4. An #[ignore]d reporter for C11's status numbers: clean on
           ~/Development/bench/rust (read only, never written), and 1,000
           modified + 10,000 untracked and every-stat-changed on a scratch clone
           (`git clone --local --no-hardlinks`, never inside the bench
           repository); `-uall` at scale is new data (the spike measured
           `normal`). Numbers into progress.md beside the spike's.
        5. D1: the root CLAUDE.md's D1 paragraph and repo map row list
           reads::status, its argv and R3.8's residuals (C13);
           docs/design/engine.md already says it.

        Invariants in play: only ops/ mutates (status is built with
        read_invocation and nothing else); every git subprocess runs with a
        built environment and a read's two variables; the porcelain-reads guard
        and its rosters (a "status" literal passes it today — do not loosen
        anything to make it pass; if #54's roster is extended, it is
        gate-integrity-reviewer's); no unwrap/expect on git's bytes; cairn-model
        changes carry tests.

        Out of scope: the worker lane (06), Local Changes (09), any index
        refresh or write.
STEP 3  Validate: scripts/gate.sh (git-floor included). Then orchestrate this
        phase's QA in this session: /qa over the phase diff with the reviewers
        implementation-plan.md names for phase 02, spawned fresh, plus the
        qa-checklist.md items this phase covers and the QA brief below.
        Adjudication goes to qa-confirm (fresh), never this session inline; log
        dismissed findings with reasons in progress.md; fix confirmed findings
        in focused fixes; disputed findings go to the user.
STEP 4  Acceptance: C4, C5 and C13 pass; C11's status numbers are recorded.
STEP 5  Update state.md and progress.md; docs/systems/git-processes.md and
        docs/systems/diff.md where they list the reads git answers; the status
        section of docs/systems/ (refs.md or a new status.md). Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/refs-and-status; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main — teardown raises that one PR
        and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if reading status.showUntrackedFiles as git
does needs a new porcelain read; if git 2.30.9 or 2.32.7 answers a fixture in a
way porcelain v2 parsing cannot represent; if any read is found to write a byte
of the index; if C11's status bars look unreachable. Otherwise do not stop for
permission.
```

## QA brief

The risk is a parser that is right on the fixtures someone thought of.

- C4's oracles must be independent of `git status` itself: comparing parsed
  status against `git status --porcelain=v1` checks only that two printers agree.
- Paths: a name with a newline, one with a leading space, one with invalid
  UTF-8, and a rename whose source has a space. `-z` is what makes these
  parseable; check nothing re-splits on `\n`.
- All seven `u` codes, each from a real conflicted index — check each maps to its
  own kind, not to "conflicted".
- The submodule field: check `S.M.`, `S..U` and `SC..` each reach the model.
- Read the argv in code, then in the stub test: no `--ignored`, no `-M`/`--no-renames`,
  no `--ignore-submodules`, and `--untracked-files` only as L2 says.
- Cancellation: a superseded read must end its process group, not wait for git to
  finish; prove it on a slow status (the every-stat-changed scratch clone is one).
