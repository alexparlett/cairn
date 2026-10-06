# Phase 08 — The sidebar, its filter, and finding a ref

```
STEP 0  Pre-flight: read docs/work/refs-and-status/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/refs-and-status before editing.
        Verify phases 01-07 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-ui/src/
        (changes_list.rs as the virtualized-list-with-filter precedent, the
        history list, the splitter the detail pane uses), crates/cairn-app/src/
        (window.rs, file_filter.rs, selection.rs, worker/ for the filter and find
        lanes phase 06 added), crates/cairn-ui/tests/changes_list.rs,
        docs/prd/refs-and-status.md (R8, C8) and
        docs/research/refs-and-status/fork-refs-and-status-ui.md section 5. Use
        the freya skill and verify every Freya API in the fork checkout at the
        pinned rev. Do not read the other planning docs directly.
STEP 2  Implement.
        1. The sidebar (R8.1, R8.2) left of the history behind a draggable
           splitter: Local Changes (N), All Commits, filter box, Branches
           (folders by `/`, ✓ and bold current, ↑↓, gone), Remotes, Tags,
           Stashes; every list virtualized, one viewport for 50,000 refs.
        2. The filter (R8.3), answered on the worker's filter lane.
        3. Pressing a ref or stash (R8.5): select and scroll; a row not loaded
           is found by paging the held walk forward as a history-lane request,
           saying "Finding <ref>…", superseded by the next press or scroll; a
           ref whose commit is not walked (a tag on a tree) says so; a stash
           with no row (R4.2) shows its changes and says it is not in the
           graph. The find retains only phases 03 and 04's compact, slim rows (R8.6);
           measure a find of the bench repository's oldest commit — time,
           retained memory, cancel — against C15 and C16. Pressing Local Changes and All Commits
           switches the main region (R8.7; Local Changes' content is phase 09's
           — a placeholder is acceptable here).
        4. Tests for C8, including the viewport twin, named in the root
           CLAUDE.md virtualization invariant in the same commit.

        Invariants in play: no unbounded list without virtualization (the
        ScrollView roster stays empty); the UI thread never waits (finding is a
        worker query); no literal modifier; a find is history-lane work, so it
        and a scroll supersede each other and nothing else.

        Out of scope: Local Changes' lists (09), Worktrees and Submodules
        sections, double-click checkout, hiding or filtering the graph (#2).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 08, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C8 passes.
STEP 5  Update state.md and progress.md; the sidebar's as-built doc
        (docs/systems/history-graph.md or a new docs/systems/sidebar.md, with
        its README row); the root CLAUDE.md status paragraph, repo map and
        virtualization invariant. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/refs-and-status; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main — teardown raises that one PR
        and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if finding the bench repository's oldest
commit retains more than C16 allows or takes more than 10% longer than the 2.4 s
measured in deep-find-measured.md (a cap or another mechanism is a product decision); if the sidebar seems to need a plain ScrollView; or
before any deviation from Fork's order or behaviour. Otherwise do not stop for
permission.
```

## QA brief

- Find a tag from the bench repository's early history and time it; record the
  number. Cancel it halfway by scrolling and check the walk stopped.
- The folder grouping over a flat list must stay virtualized: a folder of 10,000
  branches open builds one viewport.
- A press during a find supersedes it; two quick presses draw only the second.
- The filter over 50,000 names runs on the worker; the UI thread only keeps the
  indices it answers.
