# Phase 03 — Compact rows: a row keeps only its own lane changes

```
STEP 0  Pre-flight: read docs/work/refs-and-status/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/refs-and-status before editing.
        Verify phases 01 and 02 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-model/src/ (GraphRow,
        EdgeSegment, Lane, LaneAssigner and its repaint window, HistoryRow),
        crates/cairn-git/src/history*.rs (the session, the every-ref layout
        reporter), crates/cairn-ui/src/ (the history row's graph drawing and
        HistoryList), crates/cairn-ui/tests/history_list.rs,
        docs/systems/history-graph.md, docs/prd/refs-and-status.md (R4.6, R8.6,
        C15), docs/research/refs-and-status/deep-find-measured.md (the
        breakdown and its reporter in the appendix) and
        docs/research/refs-and-status/fork-deep-history.md section 2. Do not
        read the other planning docs directly.
STEP 2  Implement.
        1. cairn-model: a row keeps its id, parents, text, lane and only the
           lane changes at it (lanes that start, end, merge or fork there),
           with a full lane snapshot every K rows; the edges a drawn row
           crosses are derived from the nearest snapshot advanced through at
           most K rows of changes. The assigner's late-parent repaint must
           still reach every change it repaints. Unit tests in the same commit.
        2. Where derivation runs — at draw time over the drawn rows, or on the
           worker per viewport — and K: settle both by measurement (derivation
           cost per frame against the 16.7 ms budget; snapshot memory against
           C15), and record the choice and its numbers in progress.md.
        3. cairn-ui: the history row draws its edges from the derived set;
           the viewport twin still holds.
        4. C15: an equivalence test — every row's derived edges equal what the
           current assigner retains for it, repaints included — over the crafted
           fixtures and the Cairn checkout, and the #[ignore]d reporter over
           every ref of ~/Development/bench/rust (read only): equivalence over
           the whole history, retained bytes after paging it all, and a find of
           its oldest commit timed beside deep-find-measured.md's 2.4 s. Write
           the equivalence check BEFORE removing the old edge storage, so it
           compares against the real old output.

        Invariants in play: cairn-model plain data with tests in the same
        commit; no unbounded list without virtualization, and no history-sized
        work on a render path (derivation is bounded by K times the drawn rows,
        never by the history); RowContent read by naming every variant (this
        phase adds no variant); the crate seal.

        Out of scope: stash rows and labels (04), seeding from every ref (04),
        evicting rows (#4), any cap.
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 03, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C15 passes, its numbers recorded.
STEP 5  Update state.md and progress.md; docs/systems/history-graph.md (what a
        row keeps, how edges are derived, the per-row memory re-measured, and
        the known-limits entry for #4); the root CLAUDE.md where it describes
        rows. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/refs-and-status; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main — teardown raises that one PR
        and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if any row's derived edges differ from
today's and the difference is not a bug in the old output; if C15's 192 MiB is
out of reach without evicting rows; if derivation threatens the frame budget at
any K that meets C15. Otherwise do not stop for permission.
```

## QA brief

- The equivalence test is the whole proof. Check it ran against the OLD edge
  storage's real output, not a re-derivation of the new one, and that it covers
  rows the assigner repainted after a late parent arrived.
- A row drawn at K-1 rows past a snapshot is the worst case: measure its
  derivation, not the average.
- Check the retained-bytes figure counts capacity, not length; spare Vec capacity
  was a third of today's edge bytes, and the outer row vector's growth slack is
  part of the 150 MiB that stays.
- Scroll deep, then back to the top: rows near the top must draw the same edges
  as before.
