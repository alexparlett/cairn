# Phase 13 — The commit engine asks git

Engine work for the commit box the user approved on 2026-10-10: what an amend costs is read at
the press, git cleans a merge's message, a single cherry-pick or revert is concluded, the skip
is offered on every failure, and the settings a commit depends on are git's answers. The commit
box is changed only as far as compiling and its tests need; phase 18 rebuilds it.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phase 12 is present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/ops/commit.rs
        and ops/amend.rs (amend_consequence, reflog(), utf8_messages),
        operation_in_progress.rs, commit_hooks.rs, reads/hooks_path.rs,
        reads/fetch_settings.rs, crates/cairn-model/src/consequence.rs (the
        Amend variant, amend_replaces, replaces, force_push_warning,
        needs_force_push), commit_hooks.rs and the seal guard's rows in
        crates/cairn-guards (DESTRUCTIVE_OPERATIONS, CONFIRMATION_SURFACES,
        CONFIRMED_HOLDERS, the porcelain-read guard's CONFIG rows). Read
        docs/prd/staging-and-commit.md R1.1, R1.5, R4.7, R6.1-R6.11, R10.5,
        R10.6, C13, C14, C24, C32, C33, and
        docs/research/staging-and-commit/brief-commit-and-amend.md sections 2,
        4 and 5. Do not read the other planning docs directly.
STEP 2  Implement, test-first against real git on the host and both floors.
        1. Amend at the press (R6.4, rules 2 and 3): one local-lane job reads
           what the amend costs and, in the same job, either runs it — an amend
           git logs and no remote has, unconfirmed, through a new ops entry that
           takes no token and is NOT on DESTRUCTIVE_OPERATIONS — or ends without
           running git and answers the Consequence for the dialog. ops::amend
           keeps its row and its token for the confirmed amend; its re-check
           stops comparing the subject. Consequence::Amend keeps the id, the
           remote ref that has it and the reflog fact; its prompt is the fixed
           sentences of R10.6 ("<id> is already on <remote ref>. Amending it
           rewrites history others may have." / "<id> can't be recovered after
           this: this repository keeps no reflog."), title "Amend Commit", button
           "Amend". amend_replaces, replaces(), force_push_warning() and
           needs_force_push() go; the commit box's in-place line is cut back to
           what still compiles.
        2. git's configuration (R6.11, the git-parity fix): core.logAllRefUpdates
           and i18n.commitEncoding through reads/fetch_settings.rs's one `git
           config` invocation, generalised to a named query of a key (the
           "config" literal stays once, in that file — or, if the file is
           renamed, the porcelain-read guard's row moves with it and its
           self-test proves the new path). Read `always` as git does. No gix
           config read is left for either: amend.rs's reflog() and commit.rs's
           utf8_messages go. Tests: a linked worktree whose includeIf sets the
           key; true, false, always and unset; git 2.30.9, 2.32.7, host.
        3. MERGE_MSG cleaned (R6.10, C2): reads/stripspace.rs, `git stripspace
           --strip-comments`, the message on stdin, run in the repository so git
           reads core.commentChar; a pin that it writes nothing and runs nothing;
           a row for it in the porcelain-read guard (the exact literal
           "stripspace" only in that file, once, `--strip-comments` its one
           option) with a self-test case; D1 in docs/design/engine.md and the
           root CLAUDE.md name it, with the user's acceptance of 2026-10-10.
           Tests: `# Conflicts:`, a scissors line, core.commentChar set to `;`
           and to `auto`, the user's own `#123` line kept where git keeps it.
        4. A single cherry-pick or revert concluded (R6.9, C3): the engine tells
           a single pick or revert from a sequence (`.git/sequencer`); commit
           with CHERRY_PICK_HEAD or REVERT_HEAD present concludes it as `git
           commit -F` does (author kept for a pick, the marker gone); a sequence,
           a rebase and `git am` still refuse, the refusal carrying git's own
           continue and abort commands for the box to name. A detached HEAD is
           reported (C8) and refuses nothing.
        5. The skip on every failure (R6.6 superseded, C4): commit_hooks.rs,
           reads/hooks_path.rs, cairn_model::CommitHooks,
           Request::CommitReads's hooks half and the pin
           the_hooks_path_read_writes_nothing_and_runs_nothing go; the Git Error
           offers the skip on every failed commit or amend. A failed
           CONFIRMED amend's skip runs without asking again by option (a) below,
           the user's answer of 2026-10-10: when git refused before amending,
           ops::amend's error hands the unspent Confirmed back, and a token
           comes back only when git wrote nothing; the engine error's
           CONFIRMED_HOLDERS row and its guard pin land here, the window's Git
           Error state's in phase 18.
        6. A cancelled commit or amend (R4.7): after the reap the engine reads
           HEAD and reports whether the commit was made; the lane's WriteEnding
           gains Cancelled, and MayHaveTakenEffect no longer covers a user's
           cancel; a write Cairn lost hold of is Failed with its sentence.
        Tests: C13, C14 (engine), C24, C32 (the stripspace and config halves),
        C33, all on git-floor.

        What goes: commit_hooks.rs, reads/hooks_path.rs, CommitHooks and their
        pins; amend.rs's gix reflog() and commit.rs's gix utf8_messages; the
        Amend consequence's subject and its four prompt variants; the cherry-pick
        and revert arms of the commit refusal; WriteEnding::MayHaveTakenEffect
        as a cancel's outcome.

        Review refactors: review-code-engine.md M4 (the config route, and the
        hook model) — in this phase; M2 for amend (no display field in the
        freshness check) — in this phase; H1 (the one witness of a path's
        state), M1 (per-operation consequence types), M4's
        operation-in-progress and reflog parsers — follow-up issues at
        teardown, unless this phase rewrites them anyway.

        Invariants in play: destructive operations take Confirmed by value (the
        roster keeps ops::amend; the unconfirmed amend is a new ops function
        that takes no token — say in CLAUDE.md why it is not destructive);
        only ops/ mutates; reads are named functions in reads/ (a new one);
        every git runs with Cairn's environment; no message on argv.

        Out of scope: the commit box's view (18); the Git Error dialog's footer
        (18).
STEP 3  Validate: scripts/gate.sh, git-floor included. Then orchestrate this
        phase's QA in this session: /qa over the phase diff with the reviewers
        implementation-plan.md names for phase 13, spawned fresh, plus the
        qa-checklist.md items this phase covers and the QA brief below.
        Adjudication goes to qa-confirm (fresh), never this session inline; log
        dismissed findings with reasons in progress.md; fix confirmed findings in
        focused fixes; disputed findings go to the user.
STEP 4  Acceptance: C13, C14 and C24 (engine halves), C32 (stripspace and config),
        C33 (engine).
STEP 5  Update state.md and progress.md; docs/systems/staging.md and
        docs/systems/git-processes.md (the reads, commit, amend);
        docs/design/engine.md's D1 if its words drift from what is built; the
        root CLAUDE.md D1 paragraph, its reads list, the seal invariant's roster
        prose and the repo map. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if a confirmed amend's token could come
back from a run in which git wrote anything (option (a) hands back only an
unspent one); if git's own `commit -F` during a cherry-pick
does not keep the picked author on the host or a floor (then C3's promise needs
the user); if `git stripspace` at 2.30.9 cleans differently from the host's git
in a way a person would see. Otherwise do not stop for permission.
```

## Decided 2026-10-10: a confirmed amend's skip — option (a)

The user approved the recommendation, option (a): the failed run hands the unspent token back,
and Skip retries without asking again. The options as they were put:

The user approved "Its skip runs once, never asks a second time" (`redesign-page-2026-10-10.html`,
section 2) for commits and amends alike. For an amend that needed confirming (a remote has
`HEAD`, or no reflog), the skip needs a `Confirmed` to run `ops::amend` again. The token was
spent on the failed run. The options:

- **(a) The failure hands the unspent token back.** `ops::amend` returns its `Confirmed` inside
  the error when git failed before the amend was made, and the window's Git Error state keeps it
  until the skip or Close. The person sees one confirmation per intent. Cost: two new
  `CONFIRMED_HOLDERS` rows (the engine's error, the window's Git Error state), each excused by
  file and type, which the seal guard's twin and CLAUDE.md must state; and the destructive-ops
  reviewer must check that a token comes back only when git wrote nothing.
- **(b) The Git Error dialog is a confirmation surface for a confirmed amend's skip.** The
  window keeps the `Consequence` (not the token), and the skip builds a new token from it. The
  person sees the same thing as in (a). Cost: `git_error_dialog.rs` joins
  `CONFIRMATION_SURFACES`, and one acknowledgement builds two tokens — the residual the seal
  invariant names as the destructive-ops reviewer's to refuse — so the invariant's prose changes.
- **(c) The skip of a confirmed amend asks again.** The skip opens the confirmation dialog. It
  breaks "never asks a second time", for the rare amend that is both published and hook-failed.
  Cost: none to the seal.

## QA brief

The risk is an amend nobody meant, or one confirmed for the wrong reason.

- An amend no remote has and git logs: no `Confirmed` is built, and the old commit is in Show
  Lost Commits afterwards.
- The same press on a commit a remote has, and on a repository with `core.logAllRefUpdates=false`
  and no log: git does not run until the dialog's token arrives, and the re-check refuses a
  `HEAD` moved in between.
- `core.logAllRefUpdates` set only through a linked worktree's `includeIf`: the reflog fact is
  git's.
- `MERGE_MSG` as committed equals what `git commit` with an editor would leave.
