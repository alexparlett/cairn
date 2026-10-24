# Phase 04 — opening, closing and restoring tabs, and the Repository Manager

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/repository-tabs/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/repository-tabs before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/repository-tabs`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/closing.rs, main.rs, menu_bar.rs, toolbar.rs, operations/local_writes.rs (closing_line), worker/pool.rs (Request::Close, Request::Retire, Threads::drop, CLOSE_PATIENCE, WORKER_LOST), worker/request.rs, the holder and strip from 02-03, crates/cairn-guards (layer_dependencies_are_allowlisted), deny.toml, docs/systems/git-processes.md ("Closing"), docs/systems/credentials.md, and FR §5; read state.md, this file, implementation-plan.md
        and the PRD sections and evidence named below directly; nothing else of the planning docs.
        Verify every Freya API you are not certain of in the vendored source
        (~/.cargo/git/checkouts/), never from memory.
STEP 2  Implement:
        1. Open Repository… (R7.2-R7.5): rfd made direct (R12.1: allowlist row, exact feature pin,
           no new package or stop), crates/cairn-app/src/folder_picker.rs (render side, its future
           awaited in a task) behind a seam the tests answer; duplicates switching, closing the
           second of two racing opens of one root; a path inside a repository; a failed open's tab.
        2. The Repository Manager tab (R6, with Recent as this session's opens until 06): New Tab,
           "+", the virtualized Recent list, Open in place, the reduced menu bar and greyed
           toolbar.
        3. Closing (R8.1-R8.5): Close Tab by every route (03's ×, middle press and menu wired),
           the neighbour shown, the tab's prompt refused, the holder kept until its stream ends,
           a closed tab's failed write coming forward with Close alone, nothing freed on the UI
           thread, the last tab leaving a Manager,
           the window's close across every tab and its closing line naming the repository.
        4. Restore Closed Tab (R8.6), and the File and Window menus and the four remaining chords
           (R10). The living docs this phase makes false (implementation-plan.md, "The living
           docs"), and docs/systems/repository-tabs.md extended with opening and closing.
        Invariants in play: the UI-thread guard (a close never waits); the local lane's close
        waiting on its running write; the dependency allowlist and deny.toml; the virtualization
        twins (the Recent list); the accelerator pin. Rewrite the root CLAUDE.md, docs/systems/
        git-processes.md's "Closing" and credentials.md's closing passage in the same change.
        Out of scope: dialogs across tabs (05); Recent remembered across launches and the
        launch rules (06).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build with several repositories open and
        drive the phase's surfaces (qa-checklist item 6). Then orchestrate this phase's QA in this
        session: /qa over the phase diff, with the reviewers implementation-plan.md's dispatch
        table names for this phase, spawned fresh, plus the qa-checklist.md items and this file's
        QA brief. Adjudication goes to a fresh qa-confirm agent, never this session inline. Log
        dismissed findings with reasons in progress.md. Fix confirmed findings in focused
        commits. Disputed findings go to the user.
STEP 4  Acceptance: C8's close half (`a_tab_closes_by_its_cross_its_middle_press_and_its_menu`), C12, C13, C14's other criteria, C15, C16, C17, C18, C21, and C24's rfd half (docs/prd/repository-tabs.md).
STEP 5  Update state.md (interfaces introduced, test renames, validation row) and progress.md
        (newest first); save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/repository-tabs; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/repository-tabs, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when rfd as a direct dependency adds a package to Cargo.lock or cannot be pinned; the portal's dialog cannot be parented to Cairn's window on Wayland; or a tab's close cannot avoid waiting on the UI thread; otherwise do not stop for permission.
```

## QA brief

- Is anything a closed tab held dropped on the UI thread? Which test would fail if the retire came after the close?
- With a commit running in a closed tab, does quitting wait on it and say which repository?
- Does the zenity fallback's process appear in docs/systems/ as the review point it is?
