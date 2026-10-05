# Phase 06 — The new lanes, and refresh

```
STEP 0  Pre-flight: read docs/work/refs-and-status/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/refs-and-status before editing.
        Verify phases 01-05 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-app/src/worker/
        (pool.rs, routing.rs, epoch.rs, diff_lane.rs, network_lane.rs),
        crates/cairn-app/src/session.rs (reload_if, apply),
        crates/cairn-app/src/main.rs, crates/cairn-ui/src/accelerators.rs,
        docs/design/concurrency.md, docs/prd/refs-and-status.md (R10, R11, C10)
        and docs/research/refs-and-status/fork-refs-and-status-ui.md section 7
        (refresh, and the shortcut lists it cites). Verify in the Freya fork
        checkout at the pinned rev (~/.cargo/git/checkouts/freya-*/caa46f8/)
        how Platform::is_app_focused is read reactively from a component. Do
        not read the other planning docs directly.
STEP 2  Implement.
        1. Lanes and threads (R11): refs, ahead/behind, status and the sidebar
           filter, each with its own epoch, routed by the table — refs on the
           history thread (it reopens the walk), status and ahead/behind on a
           new third thread (R11.2) — none superseding another lane; a ref's
           find is a history-lane request (phase 08 builds it; leave its shape
           room here). Large superseded answers retired off the UI thread, and
           a reopen's replaced history rows with them (R11.3, closing #52).
        2. Refresh (R10): on focus gained, on a finished operation (fetch), and
           on a Refresh action added to the accelerator table with Fork's chord
           per platform (settle Linux's from fork-dev/Docs; ask if Fork's
           Windows chord clashes with Linux desktops). A refresh supersedes the
           previous refresh lane by lane.
        3. The history reopens when the snapshot differs (any ref, HEAD's
           state, or the stash list — R10.4) and not otherwise, replacing
           fetch's tip comparison; the
           application now walks from phase 05's every-ref request; a reopen
           keeps the selected commit if it arrives again (R10.5).
        4. Tests for C10 through the real worker boundary, and headless with
           focus toggled through freya-testing.

        Invariants in play: the UI thread never waits (the focus subscription
        and the Refresh action only submit; anything new the UI thread calls in
        worker/ is named in the root CLAUDE.md's responsiveness residuals); no
        component names a literal modifier (the chord lives in the table);
        epochs per lane — a refresh must not cancel a scroll or a diff.

        Out of scope: drawing anything new (07-09), file-system watching, any
        index refresh.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 06, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C10 passes.
STEP 5  Update state.md and progress.md; docs/systems/history-graph.md and
        docs/systems/git-processes.md where they describe reload_if and the
        lanes; the root CLAUDE.md where the responsiveness residuals and the
        status paragraph change. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/refs-and-status; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main — teardown raises that one PR
        and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if the Refresh chord Fork uses collides
with a desktop or an existing Cairn chord; if a fifth or later lane needs the
epoch table's shape changed beyond adding entries; or if keeping the selection
across a reopen needs history-sized work on the UI thread. Otherwise do not stop
for permission.
```

## QA brief

- Focus flapping (Alt+Tab twice quickly) must not stack refreshes: the second
  supersedes the first, and only one answer is drawn.
- A refresh while a page is loading must not cancel the page; a refresh while a
  diff is loading must not cancel the diff. Test both through the real boundary.
- A refresh where only the stash list changed must reopen the history, and so
  must a checkout that moves no ref (`git checkout other` where `other` is at
  `HEAD`'s commit); one where nothing changed must not (count reopens, do not
  infer them).
- A status on a stat-dirty tree must not delay a diff or a page: it runs on the
  refresh thread. Prove it with a slow status and a diff asked during it.
- A reopen frees the old rows on a worker, not in `session::reload_if` on the
  UI thread.
- The fetch path: the old `reload_if` comparison is gone, not running beside the
  new one.
- Check what the UI thread does on focus: a submit, nothing else.
