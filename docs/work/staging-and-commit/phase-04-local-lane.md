# Phase 04 — The local write lane

```
STEP 0  Pre-flight: read docs/work/staging-and-commit/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/staging-and-commit before
        editing. Verify phases 01-03 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/worker/
        (pool.rs, routing.rs, epoch.rs, network_lane.rs, the askpass acceptor),
        crates/cairn-app/src/session.rs (prompts, refresh after an operation),
        crates/cairn-app/src/closing.rs, crates/cairn-git/src/process/environment.rs
        (INHERITED), docs/design/concurrency.md (the write lanes),
        docs/prd/staging-and-commit.md (R4, R5, C10, C11, C12) and
        docs/research/staging-and-commit/write-path-as-built.md (sections 3,
        4 and 11). Do not read the other planning docs directly.
STEP 2  Implement.
        1. The lane (R4.1-R4.3): a local-write thread the routing table names,
           reached directly; FIFO with a queued state the window can draw;
           operation ids; a cancel by id, reaching a commit only. The fetch-only
           types generalise or gain siblings without changing fetch's behaviour.
        2. Freshness (R4.4-R4.6): a write counter — a status begun before the
           latest write ended is dropped, never drawn; the refresh after a write
           follows its Invalidated; no refresh of Cairn's own starts while a
           commit runs, and none is lost (it runs after).
        3. Endings (R4.7, R4.9): "may have taken effect" for a cancelled or
           unwatched write; the first close during a write waits and says
           which; a second after CLOSE_PATIENCE closes; a stranded lock named on
           the next open.
        4. Prompts and environment (R5): every local write carries an askpass
           token (L11), and the window shows a prompt during any write; the four
           display and GPG variables and the five identity variables
           (GIT_AUTHOR_NAME, GIT_AUTHOR_EMAIL, GIT_COMMITTER_NAME,
           GIT_COMMITTER_EMAIL, EMAIL; L26) join INHERITED with their reasons, and
           neither date variable does; R5.3's residual written into
           docs/design/processes.md and filed with file-issue.
        Tests: C10, C11 through the real worker boundary; C12 with the sshd
        fixture's key setup or a stub that asks, and the roster's nine. Commit
        does not exist until phase 05, so the commit-dependent halves of C10,
        C11 and C12 — a stage waiting behind a running commit, no refresh during
        a commit, a close during a commit, the SSH-signing prompt — are tested
        here against a stub git's long-running write that the lane treats as a
        commit, and phase 05 re-runs them against real `git commit`. C12's
        identity case (a commit carries GIT_AUTHOR_EMAIL from Cairn's
        environment) is phase 05's.

        Invariants in play: the UI thread never waits (the submit path stays an
        atomic bump and a send; anything new the UI thread calls is named in the
        root CLAUDE.md's residuals); every git subprocess runs with a built
        environment (the twin reads INHERITED); no credential stored or logged.

        Out of scope: commit's own verb (05), any view beyond the prompt and the
        close message.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 04, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C10, C11, C12, their commit-dependent halves against the
        stub's long-running write (phase 05 re-runs them against real git
        commit).
STEP 5  Update state.md and progress.md; docs/systems/git-processes.md (the
        local lane, prompts during writes, the roster, closing mid-write); the
        root CLAUDE.md responsiveness residuals. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/staging-and-commit; never merge it. In explicitly declared
        packet mode only, commit and push directly onto the integration branch
        with no per-phase PR. NEVER merge or PR to main — teardown raises that
        one PR and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the write counter would drop a status
that no later read replaces; if generalising fetch's types changes fetch's
observable behaviour; if a roster variable turns out to carry a secret. Otherwise
do not stop for permission.
```

## QA brief

The risk is a lane that is right one write at a time and wrong under a burst.

- Five stages asked faster than they run: all five run, in order, each with its
  own outcome; a stale one is dropped with its note and does not stall the rest.
- A status that starts before a write and ends after it must not be drawn —
  drive it with a slow status (a stub git that sleeps).
- A refresh asked during a commit runs after it, once.
- A cancel for operation N must not reach operation N+1 queued behind it.
