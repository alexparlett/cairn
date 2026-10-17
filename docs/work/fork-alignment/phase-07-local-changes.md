# Phase 07 — Local Changes and the commit box

## Starter prompt

```
STEP 0  Pre-flight: read docs/work/fork-alignment/state.md and this file, nothing else yet.
        Declare the mode. User mode is the default: create a runtime-owned phase branch from
        feature/fork-alignment before editing.
        Verify every earlier phase's work is present at the integration tip (state.md's
        validation table, `git log feature/fork-alignment`).
        Direct integration work requires an orchestrator prompt that explicitly declares
        packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/local_changes/ (list.rs, selection.rs, drag.rs, menu.rs), crates/cairn-ui/src/commit_box.rs, crates/cairn-app/src/local_changes/ and commit_box/, crates/cairn-ui/tests/local_changes*.rs and commit_box.rs, docs/systems/local-changes.md, the evidence FS1 §7, FS2 §1c, §2 (shots uv12, uv12a-c, uw6, uw34-uw36); do not read the planning docs beyond
        state.md and this file directly (the PRD sections and evidence named below are the
        exception: read those).
STEP 2  Implement:
        1. The bar over both lists (R8.1): the filter, ⌄⌄ Stage All, the side-by-side eye, View
           Mode offering List alone; the Unstaged heading's chevron removed.
        2. The headings (R8.2): bold, bordered Stage and Unstage, greyed with nothing selected
           in their list; the Staged heading's visible Unstage All kept.
        3. The commit box (R8.3): one bordered field, subject over description with Recent
           Commit Messages at the subject's end; Amend at the left; the split button with its ⌄
           a greyed control, tooltip "Comes with pull and push.", opening nothing (L2); the counter
           shown only once the subject has text.
        4. Rows at 20 px (R8.4); every staging route and the selection rule unchanged.
        Invariants in play: the text-field policy (`every_text_field_takes_the_shared_key_policy`), the confirmation seal (a discard route's wiring untouched), Local Changes' virtualization twins (50,000 paths, all selected), `next_selection`'s rule, the style guards.
        Out of scope: the file tree (L7); Commit and Push (remote-sync); Hide Untracked Files and Show Ignored Files (#70, #62).
STEP 3  Validate: `scripts/gate.sh --fast` while iterating, `scripts/gate.sh` before done (never
        pipe it through tail or head). Run the release build over the fixture
        (docs/research/ui-review/fixture-recipe.md) at 1024x768 and compare against FS1/FS2's
        shots (qa-checklist item 7). Then orchestrate this phase's QA in this session: /qa over
        the phase diff, with the reviewers implementation-plan.md's dispatch table names for
        this phase, spawned fresh, plus the qa-checklist.md items and this file's QA brief.
        Adjudication goes to a fresh qa-confirm agent, never this session inline. Log dismissed
        findings with reasons in progress.md. Fix confirmed findings in focused commits.
        Disputed findings go to the user.
STEP 4  Acceptance: C16.
STEP 5  Update state.md (interfaces introduced, validation row) and progress.md (newest first);
        save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode: commit explicit paths (never
        git add -A), Conventional Commits with a scope and a body, no session link; push the
        runtime-owned phase branch; raise a PR from the repository template into
        feature/fork-alignment; never merge it. In explicitly declared packet mode only: commit
        and push directly onto feature/fork-alignment, with no per-phase PR. NEVER merge or PR to
        main. Teardown raises that one PR, and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user when a staging route's behaviour would have to change to fit the shared bar (the PRD changes only where Stage All's chevron sits), or the one-field commit box needs a text-field shape `text_field` cannot build; otherwise do not stop for permission.
```

## QA brief

- Does every staging route's suite pass unchanged? Does any changed expectation cite FS1?
- Is the counter hidden while the subject is empty, and still Fork's limits once it has text?
- Is the split button's ⌄ greyed with the right reason in both states?
