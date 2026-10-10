# Phase 20 — Fork's Activity Manager

F3, F4, F7 and F8 of the user's decisions of 2026-10-10, rules 5 and 6, and what was applied as
Fork after the Fork check (each row carries Fork's result line, `fork-observed-2026-10-10.md`
`am1`, `am2`): the popover limited to Fork's contents plus the user's kept additions, and the
activity log's stores made one.

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 12-19 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/activity.rs,
        crates/cairn-ui/src/activity_popover.rs, crates/cairn-app/src/worker/output_flow.rs
        (OutputReceipt, HELD_LINES, HELD_BYTES), commit_box_state.rs (OutputTail),
        lost_commits.rs (Show Replaced Commit) and session.rs's activity arms.
        Read docs/prd/staging-and-commit.md R12.1-R12.3, C29,
        docs/research/staging-and-commit/brief-feedback-and-activity.md
        sections 1.7, 3.2 and 5, review-ux-activity-and-chrome.md and
        review-code-app-ui.md H3 and M6. Do not read the other planning docs
        directly.
STEP 2  Implement.
        1. The rows (R12.1): the operation's one name, Fork's result line under
           it (a short line per kind of ending — "staged", "Switched to branch
           'topic'", "Already up to date", "stage failed" — taken from git's own
           last line where Fork's is git's), the start time in HH:MM:SS UTC
           (F7), ⚠ on a failure and a distinct shape on a cancel; no status word
           in the row.
        2. The right pane: the name, one of running, succeeded, failed,
           cancelled with the time, and the duration (F8); for a failed or
           cancelled entry its one sentence; the prompt it confirmed, whole and
           wrapped, only for an operation that ran (F3); then one append-only
           list of `$ git …` lines and their output, each ending in one form
           (exit code N, killed by signal N, cancelled). An amend's way back
           reads "Show in Lost Commits", turning the mode on and selecting the
           commit (F4). An amend run at the press whose engine answer (phase
           18, step 2.7) says it would now have needed confirming says so in
           its entry, once, beside its result line (the user's ratification of
           2026-10-10, option B; PRD R1.5, C35): "Amended 3f2a1c9, which was
           pushed to origin/main while the amend ran. Sharing the amended
           commit needs a force push." where a remote has it, and that the
           replaced commit can't be recovered where git kept no reflog. It
           reports and does not prevent; no dialog opens.
        3. One store (review-code-app-ui.md H3 (b) and (c), M6): one bounded tail
           type — lines and bytes — used by the lane's hold, the commit box's
           output and each activity entry; the receipt protocol
           (OutputReceipt, its Clone that returns nothing and PartialEq that is
           always true) replaced by a shared bounded tail the window drains on
           a wake, latest wins; the log evicts whole oldest entries at its
           bound, an entry larger alone cut under "Earlier output not kept.";
           the popover reads a snapshot, no RefCell cache; is_open read in a
           child so a closed popover does not re-render the root per line (the
           merge bar's responsiveness 3).
        4. Show All goes with the cut it unfolded: the prompt row needs no
           measured height, so the lines' list keeps ItemSize::Fixed (the merge
           bar's responsiveness 2), or the prompt is laid out above the list.
        Tests: C29 whole; the popover's viewport twin (200 operations, 100,000
        lines) kept; a test that a closed popover is not re-rendered by a
        streamed line.

        What goes: the eight status words and their strings; Show All —
        PROMPT_CHARS_PER_LINE, longer_than_its_cut, Lines.prompt's measured
        State, the ItemSize::Dynamic closure, SHOW_ALL_CAPTION and
        SHOW_LESS_CAPTION; LINES_LET_GO and let_go; the streamed and commands
        two-phase store; the command endings "(not started)" and "(how it ended
        is not known)"; "Show Replaced Commit"; OutputReceipt and the duplicated
        bounds (HELD_LINES/HELD_BYTES, OUTPUT_LINES/OUTPUT_BYTES, ACTIVITY_LINES/
        ACTIVITY_BYTES as separate copies).

        Review refactors: review-code-app-ui.md H3 (b), (c) and M6 — in this
        phase; H4 (the write ledger) — follow-up issue at teardown.

        Carried from phase 12's QA (2026-10-10): #15 — OutputFlow::read lets go
        of the front of one String on every read once at its cap (up to 1 MiB
        moved a read, where the old VecDeque popped in O(1)); the one shared
        bounded tail lets go in batches, as pipes::Tail does at twice its
        bound. #16 — Update::FetchProgress is not flow-controlled (an
        unbounded queue, one update a read); fetch's progress joins the shared
        bounded tail the window drains, as a commit's output does.

        Invariants in play: no unbounded list renders without virtualization
        (both lists; CLAUDE.md's ItemSize::Dynamic residual removed with Show
        All); the UI thread never waits; no credential value is drawn (the
        lines arrive scrubbed, phase 12).

        Out of scope: Fork's All / User / Background tabs — filed as an issue
        at teardown, the user's answer of 2026-10-10 (Cairn has no background
        operations yet).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 20, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C29; C35's entry; C21's window check re-run under a hook flood with the
        popover open and closed (numbers in progress.md).
STEP 5  Update state.md and progress.md; docs/systems/git-processes.md ("The
        activity popover"); the root CLAUDE.md status paragraph, repo map rows and
        the virtualization twins' paragraph. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if a kind of ending has no result line
Fork's observation or git's own output gives, rather than inventing one.
Otherwise do not stop for permission.
```

## QA brief

The risk is the newest output lost, or a credential drawn.

- A hook that prints 100,000 lines: the latest lines are drawn while it runs and after it ends.
- Eviction drops whole oldest operations; the newest is never cut unless it alone passes the
  bound.
- No line in the popover has passed anything but phase 12's one scrub.
