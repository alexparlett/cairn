# Phase 18 — The commit box, rebuilt

C1 and C4-C8 of the user's decisions of 2026-10-10, over the engine phase 13 built: one Commit
or Amend button, no line under it, the amend dialog only when it matters, the skip in the Git
Error's footer for commits and amends alike, Cancel kept, one rule for every pre-fill, and one
operation line.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 12-17 are present at the integration tip, and
        that the user has decided phase 13's open question (a confirmed
        amend's skip). Direct integration work requires an orchestrator prompt
        that explicitly declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/commit_box.rs,
        git_error_dialog.rs, answer_button.rs, confirm_dialog.rs,
        crates/cairn-app/src/commit_box_pane.rs, commit_box_state.rs, session.rs
        (the Status arm's amend fork), worker/request.rs and routing.rs
        (Amending, StopAmending, QueryLane::Amending), and phase 13's amend at
        the press. Read docs/prd/staging-and-commit.md R10, R6.9, R6.10, R14.4,
        C13, C14, C24, C33, docs/research/staging-and-commit/brief-commit-and-amend.md
        section 4, and fork-observed-2026-10-10.md ("Amend"). Do not read the
        other planning docs directly.
STEP 2  Implement.
        1. One button (R10.6, C6): "Commit N Files", or Fork's "Amend Last
           Commit" while Amend is ticked (the user's answer of 2026-10-10) — enabled with a subject even with nothing new staged
           — with no line under it; ⌘Return / Ctrl+Enter presses it, in every
           state. Pressing Amend asks phase 13's job: a recoverable amend runs;
           otherwise the confirmation dialog opens titled "Amend Commit", in the
           fixed sentences, Cancel returning to the box unchanged, Amend running
           it with the dialog's token. The commit box leaves
           CONFIRMATION_SURFACES in the same commit as its guard row, the seal
           invariant's prose and CLAUDE.md's repo map.
        2. Running (R10.4, C5): "Committing 0:03" or "Amending 0:03" in the
           button's place with Cancel beside it; "Waiting for <name>" when
           queued; a cancel opens no dialog and keeps the draft, "Cancelling…"
           until git is reaped.
        3. A failure (R10.5, C4): Fork's Git Error, its footer in the platform's
           order — Skip pre-commit hooks and commit, and Close, focus on Close —
           one button component for both, for a commit or an amend; the skip runs
           once; a confirmed amend's skip runs with the token the failed run
           handed back (option (a), the user's answer of 2026-10-10), kept in
           the window's Git Error state until the skip or Close — that state's
           CONFIRMED_HOLDERS row and its guard pin land here, with CLAUDE.md's
           seal prose; no prompt text in the dialog.
        4. Fills and lines (R10.3, R10.8, C2, C3, C7, C8): one touched bit per
           draft — a fill goes only into an empty, untouched draft; a merge's,
           cherry-pick's or revert's draft filled from phase 13's cleaned
           message; one operation line at most — "Merging. Committing concludes
           the merge; Amend is unavailable.", "Cherry-picking a1b2c3d.
           Committing concludes it; Amend is unavailable.", the rebase, `git am`
           or sequence line naming git's command, "HEAD is detached: this
           commit will be on no branch." — wrapped, never cut; Amend greyed with
           its reason (rule 4).
        5. Amend's staged list as an overlay (review-code-app-ui.md M3): the
           status always lands in LocalChangesState; amend's staged list, still
           read per status while Amend is ticked — it is what Staged draws — is
           applied over it, so session.rs's status fork goes. The amend's cost
           is never read on a status (rule 3): Request::Amending carries the
           staged list's read alone, or is renamed for it, and StopAmending's
           epoch bump in Request::Write goes with the cost read. (Carried from
           phase 13's QA item 9: until this lands, `ops/amend.rs`'s module doc —
           "read when Amend is pressed … never on a refresh" — is not yet true of
           its caller; it is once this does.)
        6. Before the press is wired (carried from phase 13's QA item 4):
           `AmendAnswer::Amended` carries the replaced commit's `Oid` as a field
           (`AmendAnswer::Amended { performed, replaced }`), with an ops test in
           the same commit (an ops/ change), and the lane reports the replaced
           commit for the way back from the answer, never from the request
           (`LocalWrite::replaces` is `None` for `AmendAtPress` today); the
           popover's "Show in Lost Commits" stays phase 20's. Then the press
           asks `LocalWrite::AmendAtPress`, `WriteEnding::NeedsConfirming` opens
           the dialog, and the Git Error state keeps the token the engine hands
           back (`Error::AmendNotMade`, which the local lane lets go today) — the
           three wirings phase 13's systems docs state as not yet asked.
        7. The token-free amend's window reported (the user's ratification of
           2026-10-10, option B; PRD R1.5, C35) — the engine half: after an
           amend run at the press (`ops::amend_unconfirmed`) is made, read
           again, after the reap, whether a remote has the replaced commit
           (the same `reaches` walk the cost read used) and whether git keeps
           a reflog for it (`reads::log_all_ref_updates` and the log file), and
           return the answer as data on `AmendAnswer::Amended`, beside
           `replaced` — e.g. a `Publication` and a `Reflog` read after the run,
           or `Consequence::Amend` re-read whole, so `needs_confirming()` says
           whether it would now have needed confirming. It reports and does not
           prevent: nothing is undone, no dialog opens, no token is asked. The
           lane carries it on the write's ending for the activity entry, whose
           wording is phase 20's. Tests, real git on the host and both floors:
           a remote ref moved onto the replaced commit by a `pre-commit` hook
           (standing in for a push while the hook runs) answers published; the
           reflog turned off by the hook answers not written; an untouched
           amend answers as its cost did.
        Tests: C13 (views), C14 (views: no dialog for a recoverable amend, the
        dialog exactly when a remote has HEAD or no reflog, the chord and the
        button the same, Cancel returning unchanged), C24 and C33 (views),
        the Git Error's viewport twin kept.

        What goes: CommitButton::{AmendInPlace, AmendAsking, ReadingAmend,
        AmendUnreadable}; AmendButton, AmendSkip, confirm_in_place, on_confirmed
        and the box's confirmed state; the note line and line()'s max_lines(3)
        and ellipsis; READING_AMEND; commit_box_pane.rs's amend_arrived,
        status_arrived_amending, open_amend_dialog, amend_confirmed,
        skip_amend_hooks, the needs_force_push branch, the "(waiting)"
        formatting and AMEND_TITLE; commit_box_state.rs's amendable,
        reading_amend, amend_asked, fill_with_head and merge_filled;
        git_error_dialog.rs's skip_amend and the body-placed skip.

        Review refactors: review-code-app-ui.md M3 — in this phase; M2 (one
        consequence ask) — the amend's half goes with the cost read, the rest a
        follow-up issue at teardown.

        Invariants in play: Confirmed built only by a confirmation surface (the
        roster changes); no component names a literal modifier; no unbounded
        list without virtualization (the Git Error's output); the UI thread
        never waits.

        Out of scope: the status box and "Couldn't <name>" (19); the popover (20).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 18, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C13, C14, C24, C33 (views); C35's engine half; C1 and C22 for
        the roster rows.
STEP 5  Update state.md and progress.md; docs/systems/local-changes.md ("The
        commit box"); the root CLAUDE.md status paragraph, the seal invariant's
        confirmation-surface roster and the repo map rows. Save memory-worthy
        decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the Git Error state would have to keep
a token for anything but a confirmed amend's skip. Otherwise do not stop for
permission.
```

## QA brief

The risk is a draft lost, or an amend nobody meant.

- Type, tick Amend, untick: the typed draft is back exactly; a cleared fill stays cleared.
- A failing hook, then the skip: the commit carries the draft, and `--no-verify` was passed for
  that one commit; for an amend, nothing asks twice.
- A recoverable amend by the chord: no dialog, and the old commit in Show Lost Commits.
