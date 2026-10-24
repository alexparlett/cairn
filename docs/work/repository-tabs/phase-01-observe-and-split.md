# Phase 01 — Fork observed, and the window's state in two

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/repository-tabs/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/repository-tabs before editing.
        If feature/repository-tabs does not exist, create it from main and push it — but only
        once fork-alignment (PR #154) has merged to main; if it has not, stop and say so.
        This phase's observation needs the Windows VM (~/VMs/fork-win10/); if it is missing,
        stop and say so before editing anything.
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over docs/research/repository-tabs/ (all four records), crates/cairn-app/src/window.rs (View, test_view), main.rs, session.rs, closing.rs, frame.rs, the sub-states (sidebar/state.rs, local_changes/state.rs, commit_box/state.rs, operations/create_branch.rs, operations/activity.rs, diff/state.rs, detail/split.rs), worker/zone.rs, worker/request.rs (Update::Started), and the guards naming these paths (crates/cairn-guards/tests/invariants.rs: CONFIRMED_HOLDERS, WINDOW_TOKEN_FIELDS, PROCESS_CALL_EXCEPTIONS, ZONE_READER); read state.md, this file, implementation-plan.md
        and the PRD sections and evidence named below directly; nothing else of the planning docs.
        Verify every Freya API you are not certain of in the vendored source
        (~/.cargo/git/checkouts/), never from memory.
STEP 2  Implement:
        1. The observation (R1): run /observe-fork on the questions R1.1 lists; write
           docs/research/repository-tabs/fork-observed-<date>.md, every answer VERIFIED or NOT
           OBSERVED. Then settle each [observe] requirement as the PRD's Markers section says:
           put each amendment, or each deviation row for docs/design/ui.md, to the user, and on
           approval amend the PRD and docs/design/repository-tabs.md in this phase, logging it in
           progress.md.
        2. The split (R2.1, R2.2): WindowView and TabView, every View field in exactly one, one
           constructor and one test builder each; every caller takes the half it reads. One tab,
           opened by main.rs as today; no behaviour changes.
        3. The diff settings' rule (R2.3) as far as one tab shows it: the context seeded from
           diff.context until chosen, held by the window.
        4. The zone and the git description read once per application (R2.4): Update::Started
           loses them; worker/zone.rs stays the zone's reader; they reach WindowView on the
           application stream (worker/application.rs, implementation-plan.md "Startup").
        5. docs/systems/repository-tabs.md created, stating the window's two states as built
           (implementation-plan.md, "The living docs").
        Invariants in play: the UI-thread guard (no waiting primitive outside worker/); the
        confirmation seal's holder rosters (CONFIRMED_HOLDERS, WINDOW_TOKEN_FIELDS) if GitError
        or LocalWrite's file moves; the zone guard; every virtualization twin.
        Out of scope: a second tab, the holder and the strip (02, 03); any menu item (04).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build and drive the phase's surfaces (qa-checklist item 6). Then orchestrate this phase's QA in this
        session: /qa over the phase diff, with the reviewers implementation-plan.md's dispatch
        table names for this phase, spawned fresh, plus the qa-checklist.md items and this file's
        QA brief. Adjudication goes to a fresh qa-confirm agent, never this session inline. Log
        dismissed findings with reasons in progress.md. Fix confirmed findings in focused
        commits. Disputed findings go to the user.
STEP 4  Acceptance: C2; C1's observation half; C3's first half (`the_diff_context_is_each_repositorys_until_the_person_chooses_one`, one tab) (docs/prd/repository-tabs.md).
STEP 5  Update state.md (interfaces introduced, test renames, validation row) and progress.md
        (newest first); save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/repository-tabs; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/repository-tabs, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when an observed Fork behaviour contradicts a decision the user locked in brainstorm.md (T1-T14) rather than an [observe] default; the split cannot be done without changing behaviour; a field of View fits neither half as R2.1 places it; or the phase needs splitting in two; otherwise do not stop for permission.
```

## QA brief

- Does every View field appear in exactly one half, and does the test that says so read the real types, not a list typed beside them?
- Did any test's expectation change, rather than its builder? Each such change is a finding unless the PRD asks for it.
- Is the observation file first-hand throughout — a screenshot per VERIFIED line — and did the user approve each amendment before it went into the PRD?
