# Implementation plan — history-topo-order

The technical plan behind `docs/prd/history-topo-order.md`. It also holds the packet's one copy
of the review-dispatch rules.

## Branches and modes

- **Integration branch:** `feature/history-topo-order`, in its own worktree, created by phase 01
  from `main`.
- **User mode, the default:** each phase works on a runtime-owned phase branch cut from
  integration, and raises a templated PR back into it, never merging.
- **Packet mode:** only under `/orchestrate-packet`, which commits straight onto integration.
- Every PR is the user's to merge. Nothing reaches `main` until phase 04 passes.

## The walk (phase 01)

`crates/cairn-git/src/history/date_order.rs`, a walk behind `HistoryOrder::DateOrder`:

- **The commit store.** `gix_revwalk::Graph`, which caches commit reads, keyed by id. Each entry
  holds:
  - the commit time;
  - the parents, in git's order;
  - the generation, from `gix-commitgraph` where git would use the graph, else infinite;
  - the per-commit flags git keeps (SEEN, ADDED, the indegree).

  What the graph's cache is set to and how big it grows is measured, never assumed (GX gives the
  per-commit sizes).
- **The queues.** git's three — explore, indegree and topological (GT §2, §5.1) — on one queue
  type that pops ties in insertion order, as `prio_queue` does with its `ctr` (GT §2, cited).
  gix-revwalk's `PriorityQueue` is a `BinaryHeap` by key alone (WB headline 1), so the queue is
  Cairn's: a heap keyed by `(key, Reverse(insertion counter))`.
- **The tips.**
  - They are pushed in the oracle's order (PRD R1.2) from the real commits. The `Seeded`
    stand-in must not answer this walk's reads (WB §1.11): the walk asks the object store, or the
    stand-in is built with the commit's parents.
  - Stashes are synthetic tips. The walk is given each stash's id with its base as its one
    parent, never its real parents, whether read from the object store or from a commit-graph
    entry. They come after `HEAD` and before the reflog tips (PRD R1.3).
  - Before seeding a stash, the walk decides whether the refs reach its base. The phase designs
    how, and measures what it costs with and without a commit-graph. It must not force a
    whole-history read where the commit-graph would spare one: for example, the stash's tip is
    held back until the indegree walk has explored down to its base's generation, then seeded
    or dropped. A design that can't avoid that is a stopping rule.
- **The commit-graph's eligibility.** The graph's generations are used only where git uses them:
  a file present, no replace refs, no grafts, not shallow, `core.commitGraph` not off (GT
  corrections, cited). Fixtures with each case are compared with the oracle (C10). Check #128's
  scope and close it if it is the same rule.
- **Cancellation.** The epoch's check runs between commits in every loop, the opening pass
  included. A cancelled walk drops its state.
- **The count.** The number of commits the opening pass has read, given to a callback at most
  once per 100 ms of wall time.
- **Parity tests.** `crates/cairn-git/tests/date_order.rs`:
  - fixtures built by real git, each with and without a commit-graph;
  - each compared row by row with the oracle's output (PRD R1, L5): stashes grafted with `git
    replace --graft` in a copy of the fixture, never in a repository Cairn reads for the other
    tests;
  - a new row of `scripts/git-floor.sh`'s per-target table, with a floor of its own;
  - the gate-integrity reviewer checks the row and the floor count.
- **The early bench measurement.** Phase 01 measures the walk's opening pass on the bench (read
  only) against PRD R6.1's command (WB §3.6: 1,247-1,300 ms) and its memory per commit, before
  phases 02 and 03 build on it. A miss is a stopping rule, not something for phase 03 to find.

The repository's own walk wrappers — `Grafted`, `Polled` and the graft handling — are kept; a
replace ref or a graft changes the commit's parents as git sees them.

## Into the history (phase 02)

`walk::open` from a refs snapshot uses `DateOrder`, as do the cold cursor path, `HistorySession`
and a find's pages. Stash rows come from the walk's synthetic tips (phase 01), so `stream.rs`'s
merging of stash rows by date and its look-ahead are retired. Show Lost Commits' reflog tips
follow the stashes (PRD R1.4).

`reach.rs`'s correction and `RowsPage::reached` are removed if nothing else needs them (PRD
R2.6). `History::append`'s flag clearing goes with them.

Tests that change with the order, each rewritten with its reason, none deleted to make room
(PRD C5, WB §3.1):
- class A, failing by design. `a_backward_line_across_a_page_boundary_is_a_gained_segment`
  feeds a hand-built skewed walk to the assigner, which keeps working; the history-level ones
  assert date order;
- class B, the stash look-ahead tests, which the stash tips retire;
- class E, the cancel-poll counts: `opening_a_walk_from_many_refs_is_cancelled_between_tips`,
  `cancelling_stops_the_walk_and_keeps_what_it_had`;
- class F, the window's priming, where it moves.

The history-level parity tests (C4) compare against the oracle of L5, and live in a target the
`git-floor` table runs.

## The thread and the window (phase 03)

- **The history thread.** The history lane gets a thread in `crates/cairn-app/src/worker/pool.rs`,
  with its own `SharedRepository` handle, routed in `worker/routing.rs`. It takes
  `Request::OpenHistory`, the pages and the finds.
- **What stays on the repository thread:**
  - the refs read;
  - the snapshot the history was opened from, against which `walks_as` compares a refresh's
    refs;
  - the sidebar's rows and every filter;
  - `Retire`.

  When the refs moved, the repository thread asks the history thread to reopen, and the history
  lane's epoch supersedes a pass still running (PRD R3.2). The close joins the history thread
  before the repository thread's reaps. The root `CLAUDE.md`'s routing account and the UI-thread
  guard's notes change in the same commit.
- **A reopen keeps the old rows.** `session.rs`'s `reopen_history` no longer empties the history
  at the ask. It keeps the old rows drawn and usable and stops paging them. When the new first
  page arrives, it swaps the history and retires the old one, keeping the selection by id (PRD
  R4.2).
- **"Reading history… N commits".** The worker sends the count only once the pass has run
  250 ms, then at most once per 100 ms, so the window keeps no timer (PRD R4.3).
  - A first open's sentence: `crates/cairn-app/src/history/status_text.rs`.
  - A reopen's strip: a component over the list.
- **Measurements.** The reporters of PRD R6 on the bench, read only, plus a copy with a
  commit-graph. They are recorded in `docs/research/history-topo-order/measured.md`.
- **Systems docs.** `docs/systems/history-graph.md` and every systems doc that says what runs on
  which thread (PRD C9's list).

## Review dispatch (the packet's one copy)

Every phase runs `/qa` over its diff. Reviewers are spawned fresh, never the implementer.
Adjudication goes to `qa-confirm`. Dismissals are logged in `progress.md`.

| Phase | Reviewers, beyond `qa-checklist` |
| --- | --- |
| 01 | `test-coverage-auditor` (the oracle tests decide the order), `gate-integrity-reviewer` (the `git-floor` registration and floor count) |
| 02 | `test-coverage-auditor` (the tests changed by design), `responsiveness-reviewer` (paging and finds) |
| 03 | `responsiveness-reviewer` (the thread, the count's cadence, close), `test-coverage-auditor`, `gate-integrity-reviewer` (any guard or `CLAUDE.md` invariant text the routing change touches) |
| 04 | the whole set, over the whole packet diff |

No phase touches `ops/`, `process/` or `reads/` unless the oracle tests need a `git` helper in
test code. `destructive-ops-reviewer` is dispatched only if a phase's diff reaches those files.

## Risks

- **The opening pass's memory on a huge repository.** git used 522 MiB on the bench. If Cairn's
  per-commit state approaches that, phase 01 stops and asks before going on.
- **gitoxide's v1 generations against git's v2.** With a commit-graph the pass may read more than
  git does on a skewed history. The order is unchanged, and the cost is recorded (PRD R6.2).
- **A fixture's commit-graph.** git ignores the graph with replace refs (GT corrections). A test
  that wants the incremental path must not graft stashes in the same repository; build the
  grafted copy separately.
