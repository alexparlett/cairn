# Phase 14 — Create Branch as Fork runs it

The user's decisions of 2026-10-10 (B1 as changed after the Fork check, B2, B3, B4, and the
decision on how Discard is confirmed, `redesign-decisions-2026-10-10.md` §3): Create Branch's
Discard runs Fork's forced checkout, the dialog's press with Discard chosen is the confirmation,
and nothing predicts what git deletes. Engine and dialog together, since the dialog becomes a
confirmation surface.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 12-13 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-git/src/ops/checkout.rs,
        ops/branch.rs, branch_names.rs, reads/branch_name.rs, reads/untracked.rs,
        reads/change_lines.rs, crates/cairn-model/src/consequence.rs (the
        CheckoutDiscarding variant, ChangeLoss, RemovedKind, ChangedKind,
        LostChange, checkout_discarding_prompt), crates/cairn-model/src/branch_name.rs,
        crates/cairn-ui/src/create_branch_dialog.rs, crates/cairn-app/src/create_branch.rs,
        shortcuts.rs (keys_inert), and the seal guard's CONFIRMATION_SURFACES,
        DESTRUCTIVE_OPERATIONS and CONSEQUENCE_TYPES rows. Read
        docs/prd/staging-and-commit.md R1.1, R1.4, R1.5, R3.6, R11.3, C20, C32,
        C34, docs/research/staging-and-commit/fork-observed-2026-10-10.md
        ("Create Branch") and brief-discard-and-create-branch.md section 4c.
        Do not read the other planning docs directly.
STEP 2  Implement, test-first against real git on the host and both floors.
        1. The engine (R11.3, C34): ops::create_branch_discarding runs `git
           checkout -q --no-track -f -b <name> <oid> --`, Fork's command with
           Cairn's `-q` and `--`; it stays on DESTRUCTIVE_OPERATIONS and takes
           Confirmed by value. Its Consequence is fixed and generic — the branch
           name, the commit, HEAD — and its prompt names no file: it discards
           local changes and any untracked files in the way, then checks out
           <name> at <commit>. Its re-check: HEAD, the commit and the name
           unchanged, nothing else. The prediction goes:
           checkout_discarding_consequence's counting, untracked_losses, held,
           content_under, first_difference, ChangeLoss (Overwritten, Removed),
           RemovedKind, ChangedKind and LostChange where nothing else uses them,
           kept_untracked and its three endings, reads/untracked.rs and
           reads/change_lines.rs (each used only by the prediction — confirm
           with a grep before deleting), and their pins and D1's mention of them.
           The refusals of decision 3 stay: an operation in progress, a
           conflicted path, a submodule's change, each before git runs.
           (Amended 2026-10-10, the user's answer to this phase's stopping
           rule, evidence create-branch-discard-probe-2026-10-10.md: only an
           operation in progress is refused before git runs — "<Operation> is
           in progress. Finish or abort it first." in the dialog's refusal
           row, Create and Checkout disabled while Discard is chosen; a
           submodule's change and a conflicted path with no operation in
           progress go to git as Fork's do — the submodule's change survives
           and the rest is discarded, a conflict is discarded. Pinned on the
           host and both floors: the submodule's change survives and the rest
           is discarded; a stash-apply conflict is discarded; a merge in
           progress is refused with git not run.)
        2. The name (R11.3's parity amendment, C32): first establish, with a
           real-git test on the host and both floors, what `git branch -- <name>`
           and `git checkout -b <name>` do with `@{-1}`, `@{`, `a..b`, a taken
           name, a name a folder holds and one that holds a branch; then make
           the check agree with that — the review's proposal is `git
           check-ref-format refs/heads/<name>` without `--branch`, whose
           `@{-N}` resolution is the mismatch; if git's verbs resolve it too,
           the finding is dismissed and the test pins why (established
           2026-10-10: the verbs resolve @{-N} as --branch does, and
           refs/heads/<name> accepts -x and HEAD, which the verbs refuse — M3
           dismissed). The refusal becomes a
           typed value (invalid, taken, a folder holds it, it holds a branch),
           worded by the view; decision F's "A branch name can't contain '@{'"
           stays.
        3. The dialog (B2, B3, B4; R1.1): "Local changes:" Don't change / Stash
           and reapply / Discard in Fork's order; Stash and reapply greyed with
           "Comes with stashing." beside it; ⚠ beside Discard while it is
           chosen; the button "Create and Checkout". With Discard chosen,
           pressing the button or Return builds Confirmed in
           create_branch_dialog.rs from the Consequence the engine computed for
           it — the dialog joins CONFIRMATION_SURFACES in the same commit as its
           guard row — and no second dialog opens. The dialog no longer closes
           to confirm: confirm_arrived's close, kept_name as Cancel's recovery,
           DISCARD_BEFORE_CHECKOUT_TITLE and the Discard Changes and Check Out
           caption go. A Git Error, or a refusal (the "Couldn't Create branch
           '<name>'" form phase 19 builds; until then the Git Error with the
           refusal's sentence), opens over the dialog, which stays open and as
           left beneath it. While it is open, keys_inert holds (it does not
           today: review-code-app-ui.md M5's live gap). "has_changes" asks a
           named status query rather than the first entry's kind.
        Tests: C34 whole — argv by the stub git; real git leaving staged,
        unstaged and an untracked file in the way gone; the re-check refusing
        each of HEAD, the commit and the name moved and nothing else; the
        dialog's press and Return building the token, a press with Don't change
        building none; the seal guard's roster rows; keys inert; the dialog
        surviving a Git Error. C20's Create Branch half re-run.

        What goes: listed in 1 and 3; docs/design/engine.md's and the root
        CLAUDE.md's mentions of the --numstat pair and ls-files.

        Review refactors: review-code-engine.md H2 (the prediction) — in this
        phase, by deletion; M3 (the name oracle and the typed refusal) — in this
        phase; review-code-app-ui.md M5's Create Branch gap — in this phase, the
        one modal state in phase 19.

        Invariants in play: destructive operations take Confirmed by value and
        only a confirmation surface builds one (the roster changes: the Create
        Branch dialog joins; the CLAUDE.md seal invariant and its twin are
        updated in the same commit, with the residual stated that the
        acknowledgement is the radio plus the press); only ops/ mutates; every
        git runs with Cairn's environment; the UI thread never waits.

        Out of scope: stashing (packet 5b); checking out an existing branch.
STEP 3  Validate: scripts/gate.sh, git-floor included. Then orchestrate this
        phase's QA in this session: /qa over the phase diff with the reviewers
        implementation-plan.md names for phase 14, spawned fresh, plus the
        qa-checklist.md items this phase covers and the QA brief below.
        Adjudication goes to qa-confirm (fresh), never this session inline; log
        dismissed findings with reasons in progress.md; fix confirmed findings in
        focused fixes; disputed findings go to the user.
STEP 4  Acceptance: C34; C32 (the name); C1 and C22 for the roster rows.
STEP 5  Update state.md and progress.md; docs/systems/staging.md (Create Branch's
        verbs), docs/systems/git-processes.md's reads tree,
        docs/systems/history-graph.md's Create Branch; the root CLAUDE.md D1
        paragraph, the seal invariant's rosters and the repo map rows. Save
        memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if `git checkout -f -b` refuses, or
behaves differently from Fork's observed run, over a conflicted index or a
submodule on the host or a floor (decision 3's refusals then need revisiting);
if git's verbs and every check-ref-format form disagree on a name in a way no
check reproduces. Otherwise do not stop for permission.
```

## QA brief

The risk is a discard nobody chose, or one that discards more than the person was told.

- Only Discard chosen plus the press (or Return) builds the token; Don't change never does, and
  the remembered choice is never Discard.
- The fixed prompt says untracked files in the way go: an untracked file the commit holds at
  its path is gone afterwards, one elsewhere is kept.
- `HEAD` moved, the commit gone or the name taken between the press and the run: nothing is
  written.
