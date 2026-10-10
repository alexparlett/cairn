# Phase 19 — One home for each message, and the leftover lock

Rules 4-6, F1, F2, F5, F6 and F9 of the user's decisions of 2026-10-10, decision G kept, and
Fork observed (`fork-observed-2026-10-10.md`, "A stale index.lock"): progress in the status box,
a git failure in Git Error, a refusal found while running in "Couldn't <name>", a refusal known
in advance as a greyed control, success silent, a cancel quiet, no line under the lists, one
name per operation, and the leftover lock as a state of the repository.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 12-18 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/status_text.rs,
        window.rs (the banners), local_changes_actions.rs (acting_line, Acting),
        local_changes_pane.rs (the strip under the lists), local_writes.rs,
        worker/local_lane.rs (WriteEnding, LocalWrite::name, what, awaited,
        names_index_lock), worker/refresh_lane.rs, shortcuts.rs (keys_inert),
        create_branch.rs (write_ended), fetch_state.rs, activity.rs (the lock
        offer: LockOffer, lock_asked, lock_ready, removing, locks_at_open),
        crates/cairn-ui/src/status_box.rs, git_error_dialog.rs,
        crates/cairn-git's ops/remove_lock.rs and the error type's lock suffixes.
        Read docs/prd/staging-and-commit.md R4.7, R4.9, R12.4, R14, C11, C28,
        C30, docs/research/staging-and-commit/brief-feedback-and-activity.md
        sections 3.1, 3.3, 3.4 and 5, and review-ux-activity-and-chrome.md.
        Do not read the other planning docs directly.
STEP 2  Implement.
        1. One name (R14.2, rule 5): LocalWrite::name, Fork's imperative, is the
           only wording of a write; what(), awaited(), Asked.what and
           Asked.awaited, UNKNOWN_WRITE and status_text::capitalised go.
           WriteEnding (phase 13 gave it Cancelled) carries a short sentence for
           people apart from its command, output and locks; the engine's
           "; nothing was written" tails and the lock Display suffixes stop
           being UI text; the view adds "Nothing was changed." once to a refusal.
           Carried from phase 13's QA: the wording of `Error::MadeButGitFailed`
           ("git committed, but then failed: …", a commit or amend git made and
           then exited non-zero after the ref update; the lane ends it `Failed`
           with git's command and words) and of `Error::CommitUnconfirmed`
           (including a `post-commit` hook that commits, item 10) is plain
           engine text today, and this phase gives each its sentence and home.
        2. Progress (R14.1, F1, F9): the status box shows a spinner and the
           running operation's name once it has run 250 ms (a timer on the
           toolkit's executor, as the commit box's elapsed time is — never a
           sleep on a render path; CLAUDE.md's residual list gains it), with
           " · N waiting"; a fetch adds git's progress line; Fetch is greyed
           while a fetch runs, so the network lane's refusal is never drawn.
           fetch_line, refusal_line and the fetch banner go.
        3. The two dialogs (R14.3, F2): a git failure of any operation the
           person started opens Fork's Git Error; a refusal found while running
           opens the same component titled "Couldn't <name>", its one sentence
           and no Error Details; a partly done discard says so in one sentence,
           the kept files behind Show files. Create Branch's mapping of Stale,
           Refused, Incomplete and NotRun into a Git Error with an empty command
           goes. The strip under the lists (acting_line, Acting's said and quiet,
           its label) goes; a cancel opens nothing.
        4. One modal state (R14.8, review-code-app-ui.md M5): an Option of a
           Modal enum in the window — confirmation, Git Error or Couldn't,
           Create Branch, credential prompt, popover — and keys_inert reads it;
           local_changes_actions::dialog_open and the per-dialog checks go.
           Escape stays a literal key, the convention the user allowed on
           2026-10-10 (the merge bar's F7, PRD R7.2): a new site where Escape
           is heard is named in R7.2's list.
        5. The leftover lock (R12.4, F5, F6, G, C30): every refresh stats
           `<gitdir>/index.lock` on the refresh thread; the window shows the
           banner while it exists, no git of Cairn's runs or waits there, and its
           mtime is older than about ten seconds; it offers Remove index.lock….
           A write that fails because git could not create the lock is retried
           on the local lane about a second later, once or twice (a sleep on the
           worker thread is allowed; say so), before it ends failed, and its Git
           Error then carries Remove index.lock… beside Close. Both open the
           "Remove Stale Lock" confirmation (decision G's prompt, titled in
           Title Case by the user's answer of 2026-10-10), the Consequence read at the
           press, ops::remove_index_lock unchanged; then a refresh, never a
           retry of the write. The closing banner names the write
           ("Closing after Commit finishes. Close again to quit now.").
        Tests: C28 and C30 whole; C11 re-run with the closing line's new words;
        the window's chords inert under every Modal, Create Branch included.

        What goes: status_text.rs's fetch_line, refusal_line and locks_line;
        window.rs's banners but the lock and closing ones; acting_line and the
        strip; Outcome::{NotRun, MayHaveTakenEffect, PartlyDone, Found} as
        status strings; the lock offer apparatus — ActivityKey::Opened,
        locks_at_open, LOCKS_AT_OPEN_NAME, GIT_RUNNING_NOTE, Activity.lock_named
        and lock_note, ActivityLog.lock_asked, lock_ready and removing,
        LockOffer::{Ready, Blocked}, the popover's lock row and
        REMOVE_LOCK_CAPTION there, local_lane's names_index_lock at the ending,
        LocalWrites.locks as a "last listed" list; the engine's reassurance
        tails; the empty-command Git Error.

        Review refactors: review-code-app-ui.md M5 (one modal state) — in this
        phase; H4 (the write ledger) — follow-up issue at teardown, though the
        one name (H4's WriteInfo half) lands here; M1 (declarative lanes) —
        follow-up issue at teardown.

        Invariants in play: the UI thread never waits (the 250 ms timer, the
        stat on the refresh thread; update the residual list for anything new
        the UI thread calls); only ops/ changes the filesystem (the removal is
        unchanged, the stat reads); destructive operations take Confirmed (the
        removal's); no credential value is drawn.

        Out of scope: the popover's contents (20).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 19, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C28, C30; C11.
STEP 5  Update state.md and progress.md; docs/systems/git-processes.md (the
        local write lane's endings, the lock state, closing),
        docs/systems/local-changes.md (no strip under the lists); the root
        CLAUDE.md status paragraph, its UI-thread residuals and repo map rows.
        Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if a retry on a lock could run a write twice that wrote
anything the first time (retry only what git refused before writing). Otherwise
do not stop for permission.
```

## QA brief

The risk is a message that says the wrong thing, or none where one is owed.

- Every ending of every write reaches exactly one home: status box, Git Error, Couldn't, or
  nothing for success and cancel.
- The banner is never shown while Cairn's own git holds the lock, and goes when the file goes,
  whoever removed it.
- A retried write that git refused for the lock writes once, never twice.
