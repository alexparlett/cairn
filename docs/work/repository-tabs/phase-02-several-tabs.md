# Phase 02 — several repositories open at once, each served by its own tab

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/repository-tabs/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/repository-tabs before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/repository-tabs`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/main.rs, window.rs, session.rs, closing.rs, refresh/focus.rs, repository_path.rs, worker/pool.rs (open, Updates, Replier, Request::Refresh, Request::RefreshStatus), worker/window_check_updates.rs, window_check.rs, worker/askpass.rs, the credential guard's container scan in crates/cairn-guards/tests/invariants.rs, and FR §2-§3 in docs/research/repository-tabs/freya-tab-strip.md; read state.md, this file, implementation-plan.md
        and the PRD sections and evidence named below directly; nothing else of the planning docs.
        Verify every Freya API you are not certain of in the vendored source
        (~/.cargo/git/checkouts/), never from memory.
STEP 2  Implement:
        1. The tab holder (R3.1, R3.6): crates/cairn-app/src/tabs/holder.rs, one keyed child per
           tab, drawing nothing, owning its TabView, its update task and its Replier; the window's
           list of tabs; a headless test first proving an unmounted holder drops its State and
           cancels its task. The guard only_the_tab_holder_opens_a_repository with its self-test,
           and the credential guard following the Replier alias; both into the root CLAUDE.md.
        2. Several tabs (R3.2-R3.4, R7.1): every command-line path a tab, the last shown; only the
           active tab drawn, keyed by its id; switching keeping every per-tab value; a minimal
           strip of names to press (its full look is 03's).
        3. The window's root-scope hooks following the shown tab, and every list's scroll lifted
           into TabView (R3.3, R3.4, implementation-plan.md "The tab holder"); the diff context's
           per-tab seed and the window's chosen context (R2.3's several-tab half).
        4. Refreshes (R5): focus refreshes the shown tab whole and the others' status alone;
           activation refreshes; a commit holds back its own tab's refresh only.
        5. The window checks (R13): tabs_switch_check, tabs_idle_check, tabs_launch_check and
           tabs_refresh_check, #[ignore]d, with the worker-side helper that waits on several
           Updates; run them where the bench exists and record the numbers.
        6. The living docs this phase makes false (implementation-plan.md, "The living docs"):
           the root CLAUDE.md's status paragraph and its one-repository line,
           docs/systems/history-graph.md's opening, window-frame.md's title, the comments that
           count one repository; docs/systems/repository-tabs.md extended with the holder.
        Invariants in play: the UI-thread guard and its residual list in the root CLAUDE.md and
        .claude/agents/responsiveness-reviewer.md (main.rs's use_hook wiring moves to the holder:
        rewrite both); the secret guard (no Replier field); the virtualization twins.
        Out of scope: the strip's look, ×, menu, overflow and chords (03); closing a tab and the
        window across tabs (04) — until 04, closing the window closes every tab as one close each;
        dialogs across tabs (05).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build with several repositories open and
        drive the phase's surfaces (qa-checklist item 6). Then orchestrate this phase's QA in this
        session: /qa over the phase diff, with the reviewers implementation-plan.md's dispatch
        table names for this phase, spawned fresh, plus the qa-checklist.md items and this file's
        QA brief. Adjudication goes to a fresh qa-confirm agent, never this session inline. Log
        dismissed findings with reasons in progress.md. Fix confirmed findings in focused
        commits. Disputed findings go to the user.
STEP 4  Acceptance: C3's second half (`a_toggle_asks_a_background_tabs_diff_again_when_it_is_shown`), C4, C5, C6, C11, C14's first criterion (`every_path_named_opens_a_tab_and_the_last_is_shown`), and C25's checks written (docs/prd/repository-tabs.md).
STEP 5  Update state.md (interfaces introduced, test renames, validation row) and progress.md
        (newest first); save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/repository-tabs; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/repository-tabs, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when the holder's lifetime test shows Freya does not drop an unmounted keyed holder's State or cancel its task; R13's bars fail on the bench; or a background tab's update stream cannot be applied without drawing it; otherwise do not stop for permission.
```

## QA brief

- Does any render-side file wait, or name a waiting primitive, now that main.rs no longer drives the one stream?
- Can a TabView handle be read after its holder has unmounted? Is there a test closing a tab while its view is drawn?
- Does a background tab's update ever write the window's state, or the shown tab's?
