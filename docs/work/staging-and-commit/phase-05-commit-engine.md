# Phase 05 — Commit and amend in the engine

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-04 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/ops/ (phase
        03's verbs as precedents), crates/cairn-git/src/reads/ (phase 03's
        git-path read), crates/cairn-git/src/diff/working_tree.rs (the
        staged side, diff-index --cached), the refs snapshot and ahead/behind
        (docs/systems/refs.md), crates/cairn-app/src/worker/ (phase 04's lane),
        docs/prd/staging-and-commit.md (R6, C2, C10-C14, C24) and
        docs/research/staging-and-commit/git-write-verbs.md sections 5-6. Do
        not read the other planning docs directly.
STEP 2  Implement.
        1. ops/: commit (R6.1) and amend (R6.2) — `-F -`, no --cleanup,
           --no-verify only when asked by the hook failure's skip, a non-UTF-8
           i18n.commitEncoding refused before git runs; amend takes Confirmed by
           value, is rostered, and re-checks HEAD's id (R1.4); both carry an
           askpass token and stream stderr to the lane's outcome.
        2. Amend's staged list (R6.3): the index against HEAD^ (the empty tree
           for a root commit) through the plumbing the working-tree query runs;
           `git reset -q HEAD^ --` to unstage from it, and `git rm --cached -q --`
           out of a root commit's amend, which has no HEAD^; amend unavailable
           with no HEAD and during a merge.
        3. Amend's Consequence (R6.4): HEAD's id and subject, and whether a
           remote has it — the upstream's ahead count, or a hidden walk of
           HEAD --not --remotes with no upstream, cancellable.
        4. The hooks found (R6.6: a hook counts when it exists and is
           executable) and recent messages (R6.7), read on a worker.
        5. Identity (R6.8): no author or committer passed; git's own error
           carried in the outcome when it has none.
        6. The operation in progress (R6.9, L25): a merge, a rebase, a
           cherry-pick or a revert, detected as git records it (MERGE_HEAD and
           MERGE_MSG, the rebase directories, CHERRY_PICK_HEAD, REVERT_HEAD —
           verify the gix API before using it); with a merge, commit makes the
           merge commit; with any of the others, commit and amend are refused
           with the operation's name before git runs.
        Tests: C2 (amend half), C13, C14 and C24's engine halves, on the host's
        git and the floors, with real hooks in fixtures; C12's identity case (a
        commit made with GIT_AUTHOR_EMAIL in Cairn's environment has that
        author); and phase 04's commit-dependent halves of C10, C11 and C12 — a
        stage waiting behind a running commit, no refresh during a commit, a
        close during a commit, the SSH-signing prompt — re-run against real
        `git commit` with a slow hook.

        Invariants in play: only ops/ mutates; destructive operations take
        Confirmed by value; the message never reaches argv; no
        unwrap/expect on git's bytes.

        Out of scope: the commit box (09), the Git Error dialog (09).
STEP 3  Validate: scripts/gate.sh (git-floor included). Then orchestrate this
        phase's QA in this session: /qa over the phase diff with the reviewers
        implementation-plan.md names for phase 05, spawned fresh, plus the
        qa-checklist.md items this phase covers and the QA brief below.
        Adjudication goes to qa-confirm (fresh), never this session inline; log
        dismissed findings with reasons in progress.md; fix confirmed findings
        in focused fixes; disputed findings go to the user.
STEP 4  Acceptance: C2 (amend), C13 (engine), C14 (engine), C24 (engine); C10,
        C11 and C12's commit-dependent halves against real git commit.
STEP 5  Update state.md and progress.md; docs/systems/git-processes.md and the
        staging doc (commit, amend, hooks). Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if `-F -` leaves any commit.cleanup value
different from `git commit -F`; if the pushed check cannot stay bounded on
rust-lang/rust; if a floor git handles amend differently. Otherwise do not stop
for permission.
```

## QA brief

The risk is a message or a commit that is subtly not what git would have made.

- Byte-compare the committed message with what `git commit -F` makes from the
  same bytes, under every `commit.cleanup` value and with `#` lines, trailing
  blank lines and CRLF in the draft.
- A failing `pre-commit`: the outcome carries its output; nothing was committed;
  no lock is left.
- The pushed check: an upstream at, behind and ahead of HEAD; no upstream with
  another remote branch containing HEAD; a fork whose remote branch was deleted.
