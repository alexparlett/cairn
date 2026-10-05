# Phase 08 — Local Changes, read only, and the window check

```
STEP 0  Pre-flight: read docs/work/refs-and-status/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/refs-and-status before editing.
        Verify phases 01-07 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/
        (worker/request.rs — FileTarget::WorkingTree and WorkingSide, under
        expect(dead_code) outside tests today — diff_state.rs, changes_tab.rs, diff_actions.rs, window.rs,
        window_check.rs), crates/cairn-ui/src/ (changes_list.rs, diff_view.rs,
        diff_notice.rs), docs/systems/diff.md on the working-tree query,
        docs/prd/refs-and-status.md (R9, C9, C11, C12) and
        docs/research/refs-and-status/fork-refs-and-status-ui.md section 6. Use
        the freya skill and verify every Freya API in the fork checkout at the
        pinned rev. Do not read the other planning docs directly.
STEP 2  Implement.
        1. The Local Changes view (R9): Unstaged above Staged, flat virtualized
           lists with Fork's badges (each kind told apart without colour), a
           path in both lists when it has both; the sidebar count by R9.2.
        2. Choosing a path opens its diff through the working-tree query —
           staged, unstaged or untracked by its list — in the diff view with its
           bar and settings; the first path chosen on open; a conflicted path
           draws its notice (R9.4). Wire FileTarget::WorkingTree for real.
        3. Tests for C9, including the viewport twin over 50,000 paths, named
           in the root CLAUDE.md virtualization invariant.
        4. C12: extend window_check to land the decorated history, the sidebar,
           the refs and a large status on the bench repository (a scratch
           clone for the status), frames recorded. Complete C11 in progress.md.

        Invariants in play: no unbounded list without virtualization; the UI
        thread never waits; an answer is drawn only for the selection it names
        (a path's diff arriving after the list refreshed must name its path and
        side); no literal modifier; nothing stages or discards.

        Out of scope: stage, unstage, discard, commit (packet 5); the tree view
        (#36); ignored files.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 08, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C9 and C12 pass; C11 is complete.
STEP 5  Update state.md and progress.md; docs/systems/diff.md (the working-tree
        diff now reached from the window) and the Local Changes as-built doc;
        the root CLAUDE.md status paragraph, repo map and virtualization
        invariant. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/refs-and-status; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main — teardown raises that one PR
        and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user before any deviation from Fork's Local
Changes layout; if a frame exceeds 16.7 ms in window_check and the fix is not
local; if the working-tree query answers a state the view cannot draw. Otherwise
do not stop for permission.
```

## QA brief

- A refresh that removes the chosen path from the list: what is drawn? It must
  not be the stale diff under no row.
- A path both staged and unstaged: choosing it in each list shows the matching
  diff, and the two answers cannot cross.
- Untracked files under a new directory are listed one per file (L2), and the
  count counts them.
- window_check: confirm the status it lands is large (thousands of paths), and
  that the scratch clone, never the bench repository, was dirtied.
