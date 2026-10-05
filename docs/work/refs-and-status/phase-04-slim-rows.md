# Phase 04 — Slim rows: one id, a parent count, shared stores

```
STEP 0  Pre-flight: read docs/work/refs-and-status/state.md and this file.
        Nothing else yet. Declare the mode. User mode is the default: create a
        runtime-owned phase branch from feature/refs-and-status before editing.
        Verify phases 01-03 are present at the integration tip. Direct
        integration work requires an orchestrator prompt that explicitly
        declares packet mode.
STEP 1  Load context via an Explore agent over crates/cairn-model/src/ (lib.rs's
        CommitSummary, history.rs's HistoryRow and RowContent, graph.rs as
        phase 03 left it, oid.rs), crates/cairn-git/src/history*.rs (how a page
        is built), crates/cairn-app/src/ (history_state.rs, session.rs,
        selection.rs, window.rs, worker/ where pages cross the boundary),
        crates/cairn-ui/src/ (commit_row.rs, changes_list.rs, history_list.rs),
        crates/cairn-ui/tests/history_list.rs, docs/prd/refs-and-status.md
        (R4.7, C16) and docs/research/refs-and-status/deep-find-measured.md
        ("The rest of a row is small"). Do not read the other planning docs
        directly.
STEP 2  Implement.
        1. cairn-model: the slim row of R4.7 — the commit id once, a parent
           count, the subject as an offset and length into a chunked text store,
           the author as an index into an author table, the date, the lane and
           the lane-change offset; no author email; no per-row heap allocation.
           The history owns the stores (chunked, never doubling) and the rows (a
           chunked store as well); a row is read through it. Each page from the
           worker carries its own text and any authors new to the history, and
           appending a page appends to the stores. Tests in the same commit.
        2. Every reader moves to reading through the history: commit_row.rs
           (merge marker from the count, subject, author, date),
           changes_list.rs, selection.rs and whatever else the compiler names.
           Anything that needs a commit's full parents or email asks the details
           query it already has; nothing re-adds them to a row.
        3. C16: headless and model tests that every row draws what it drew
           before — subject, author, date, short id, merge marker — over the
           crafted fixtures and the Cairn checkout (write the comparison BEFORE
           removing the old fields, so it compares against the real old
           output); the #[ignore]d reporter's retained bytes for all of
           ~/Development/bench/rust (read only) from every ref, counting
           capacity, against 64 MiB; and a check that no retained row owns a
           heap allocation.

        Invariants in play: cairn-model plain data with tests in the same
        commit, and nothing new on its allowlist; RowContent read by naming
        every variant (the commit variant's shape changes; no wildcard to
        satisfy the compiler); no history-sized work on a render path (reading
        a row through the stores is constant work per drawn row); the UI thread
        never waits (appending a page's stores happens where pages are applied
        today); the crate seal.

        Out of scope: stash rows, labels and seeding from every ref (05);
        evicting rows (#4).
STEP 3  Validate: scripts/gate.sh. Then orchestrate this phase's QA in this
        session: /qa over the phase diff with the reviewers implementation-plan.md
        names for phase 04, spawned fresh, plus the qa-checklist.md items this
        phase covers and the QA brief below. Adjudication goes to qa-confirm
        (fresh), never this session inline; log dismissed findings with reasons
        in progress.md; fix confirmed findings in focused fixes; disputed
        findings go to the user.
STEP 4  Acceptance: C16 passes, its numbers recorded; C15 still passes.
STEP 5  Update state.md and progress.md; docs/systems/history-graph.md (what a
        row keeps, the stores, the per-row memory re-measured); the root
        CLAUDE.md repo map row for cairn-model. Save memory-worthy decisions.
STEP 6  Branch authority follows the declared mode. In user mode, commit
        explicit paths (never git add -A), push the runtime-owned phase branch,
        and raise a PR using the repository template into
        feature/refs-and-status; never merge it. In explicitly declared packet
        mode only, commit and push directly onto the integration branch with no
        per-phase PR. NEVER merge or PR to main — teardown raises that one PR
        and the USER merges every PR.
STEP 7  Final response: what shipped, what is deferred, exact follow-ups.
STOPPING RULES: stop and ask the user if a production view turns out to need a
row's full parents or email; if C16's 64 MiB is out of reach without evicting
rows; or if reading rows through the stores costs a frame measurably. Otherwise
do not stop for permission.
```

## QA brief

- The comparison must be against the old fields' real output, captured before
  they were removed — not against a re-read of the new stores.
- A page that brings a new author and a page that brings only known ones: both
  draw correctly, and the author table holds each name once.
- A subject with non-UTF-8 bytes, an empty subject and a very long one: each
  reads back exactly.
- Count capacity, not length, in the retained-bytes figure; chunked stores have
  their own slack in the last chunk.
- Grep for any `String` or `Vec` left inside a retained row type.
