# Phase 05 — dialogs across tabs

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/repository-tabs/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/repository-tabs before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/repository-tabs`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/modal.rs, session.rs (Update::Prompt, withdraw), operations/fetch.rs (PromptView), operations/ending_dialog.rs, operations/activity.rs, toolbar.rs and the status box, worker/askpass.rs (PromptId), worker/pool.rs (Replier), crates/cairn-ui/src/dialogs/, and the tab holder; read state.md, this file, implementation-plan.md
        and the PRD sections and evidence named below directly; nothing else of the planning docs.
        Verify every Freya API you are not certain of in the vendored source
        (~/.cargo/git/checkouts/), never from memory.
STEP 2  Implement:
        1. Prompts (R9.1): a process-unique PromptId; the window's prompts a queue, each with its
           tab, answered through its own tab's Replier; the prompt titled with its tab's name and
           coming forward over any tab.
        2. One modal state across tabs (R9.2): the strip takes no press while a dialog is drawn.
        3. Endings waiting (R9.3): a background tab's endings kept on its tab, the waiting mark
           drawn, the dialog opened when the tab is shown.
        4. The status box and the activity popover the shown tab's (R9.4).
        Invariants in play: no credential value logged or stored (Secret, the Replier never a
        field); the confirmation seal (a confirmation is the shown tab's); one modal state.
        Out of scope: the store (06).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build with several repositories open and
        drive the phase's surfaces (qa-checklist item 6). Then orchestrate this phase's QA in this
        session: /qa over the phase diff, with the reviewers implementation-plan.md's dispatch
        table names for this phase, spawned fresh, plus the qa-checklist.md items and this file's
        QA brief. Adjudication goes to a fresh qa-confirm agent, never this session inline. Log
        dismissed findings with reasons in progress.md. Fix confirmed findings in focused
        commits. Disputed findings go to the user.
STEP 4  Acceptance: C10's waiting half, C19, C20 (docs/prd/repository-tabs.md).
STEP 5  Update state.md (interfaces introduced, test renames, validation row) and progress.md
        (newest first); save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/repository-tabs; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/repository-tabs, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when a prompt's answer could reach another tab's git; or a background tab's prompt cannot come forward without switching tabs; otherwise do not stop for permission.
```

## QA brief

- Can a prompt's id, or its answer, ever cross from one tab to another? Name the test that fails if it can.
- Does a background tab's Git Error open exactly once, when its tab is shown, and never over another tab?
