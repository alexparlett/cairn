# Phase 06 — the tabs remembered across launches

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/repository-tabs/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/repository-tabs before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/repository-tabs`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/main.rs, repository_path.rs, the holder and the window's list of tabs, the Repository Manager from 04, crates/cairn-guards (FILESYSTEM_MUTATION_EXCEPTIONS, layer_dependencies_are_allowlisted), deny.toml, crates/cairn-askpass/src/channel.rs (the existing exception row's shape); read state.md, this file, implementation-plan.md
        and the PRD sections and evidence named below directly; nothing else of the planning docs.
        Verify every Freya API you are not certain of in the vendored source
        (~/.cargo/git/checkouts/), never from memory.
STEP 2  Implement:
        1. serde and toml made direct (R12.2, R12.3): allowlist rows, exact feature pins, no new
           package or stop.
        2. The store (R11.1-R11.4): worker/tab_store.rs, its versioned types, the read before the
           window opens, the application thread writing whole to a temporary file renamed into
           place, latest wins; its FILESYSTEM_MUTATION_EXCEPTIONS row (R11.6) pinned to exactly
           its kinds of write, into the root CLAUDE.md's filesystem invariant.
        3. The launch rules (R11.5): remembered tabs, then the paths named, the shown tab, the
           working directory, the Manager.
        4. Recent remembered: the Manager's Recent from the store; each row's folder checked on
           the store's thread when the Manager is shown, a missing folder's row and Remove from
           Recent (R6.4); a non-UTF-8 path left out (R11.2). docs/systems/repository-tabs.md
           extended with the store.
        Invariants in play: only ops/ changes the filesystem (the new row its one exception for
        cairn-app); the dependency allowlist; the UI thread never waits (the write is the store
        thread's; the one read is before the window exists).
        Out of scope: any setting (Preferences' store).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build with several repositories open and
        drive the phase's surfaces (qa-checklist item 6). Then orchestrate this phase's QA in this
        session: /qa over the phase diff, with the reviewers implementation-plan.md's dispatch
        table names for this phase, spawned fresh, plus the qa-checklist.md items and this file's
        QA brief. Adjudication goes to a fresh qa-confirm agent, never this session inline. Log
        dismissed findings with reasons in progress.md. Fix confirmed findings in focused
        commits. Disputed findings go to the user.
STEP 4  Acceptance: C12's missing-folder criterion, C22, C23, C24's serde and toml half (docs/prd/repository-tabs.md).
STEP 5  Update state.md (interfaces introduced, test renames, validation row) and progress.md
        (newest first); save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/repository-tabs; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/repository-tabs, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when serde or toml as direct dependencies add a package; the store cannot be written atomically on a filesystem Cairn must support; or the read before launch cannot stay within its cap; otherwise do not stop for permission.
```

## QA brief

- Is every write of the store in the one excepted file, and are its kinds exactly the row's?
- Does a corrupt file, a file of another version and a file over the cap each open no tab and say nothing?
- Can two writes interleave so a launch reads half of one?
