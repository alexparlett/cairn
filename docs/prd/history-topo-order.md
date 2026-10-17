---
status: in-flight
packet: history-topo-order
opened: 2026-10-17
---

# PRD — History in git's date order

The history graph drawn in `git log --date-order`'s order, which is Fork's default: no commit
above any of its children, and otherwise newest first by commit time, with ties kept in the
order git queues them.

Today, when commit times tie or the clock was skewed, Cairn draws a parent above its child. On
the bench the lane assigner repaints 49,609 of 345,545 rows (14%) to hide the misordering, and 627
lines still arrive late. This packet writes git's walk in `cairn-git` over gitoxide's
commit graph and gives the history its own worker thread. The opening pass a repository without a
commit-graph needs — git's, Fork's and Cairn's alike — then never holds up anything else.

Decisions and the rejected alternatives: `docs/work/history-topo-order/brainstorm.md` (L1-L8).
Evidence is in `docs/research/history-topo-order/`:
- GT: git's order, algorithm, oracle and costs (`git-topo-order.md`);
- GX: gitoxide's walks (`gix-topo-apis.md`);
- WB: Cairn's walk as built (`walk-as-built.md`).

Design: `docs/design/history-graph.md` ("Lanes are assigned incrementally", "A scroll keeps its
walk open") and `docs/design/concurrency.md`, both rewritten by this planning to the end state.
Packet 5d, `fork-alignment`, depends on this one: it marks what `HEAD` reaches and places stashes
on their base's lane during a walk that emits every child before its parents.

## Requirements

### R1 — The order (L1, L5)

1. The history's rows are in the order `git log --date-order --format=%H --branches --remotes
   --tags HEAD` prints for the same refs.
2. Tips are seeded in the oracle's order, because git keeps the order it was given tips in when
   their times tie:
   - local branches, then remote-tracking refs, then tags, each in git's refname order;
   - then `HEAD`;
   - then the stashes (R1.3), in `stash@{n}` order;
   - then, with Show Lost Commits, the reflog ids (R1.4).
3. **A stash is a tip of the walk** whose one parent is its base, as the oracle grafts it (GT
   §4.2). It is not a row merged into the stream afterwards. It counts toward its base's in-degree,
   so its base is drawn below it, and a tie with a ref tip resolves as git's tip order does.
   - Only stashes whose base the refs reach get a row, as today.
   - How the walk decides that before seeding a stash, at what cost with and without a
     commit-graph, is phase 01's to design and measure. It must never read the whole history on a
     repository whose commit-graph would otherwise spare it.
   - A stash's commit-graph entry, which holds its real two or three parents, is never used for
     its parents.
4. **With Show Lost Commits**, the old and new id of every entry of `HEAD`'s and each local
   branch's reflog are tips after the stashes, in the order the oracle feeds them on stdin (GT).
5. Every other order is unchanged:
   - `HistoryOrder::CommitTime`, which `recent_messages` keeps for `git log -n 10`;
   - `HistoryOrder::GraphOrder`;
   - the lane assigner's tolerance of any arrival order. The frozen compact-rows oracle and its
     tests are untouched.

### R2 — The walk (L2)

1. A new order, `HistoryOrder::DateOrder`, is git's incremental date-order walk, written in
   `crates/cairn-git/src/history/` over `gix_revwalk::Graph`. It follows GT §5.1's pseudo-code,
   and §5.2's condition for a lazy walk to give the same order as git's whole-list path, with
   git's `revision.c` at v2.30.9 and v2.56.0 cited at each step:
   - the explore, indegree and topological queues;
   - generation numbers from the commit-graph only where git would use it, and infinite where it
     would not. git ignores the commit-graph when there is no file, and also with replace refs,
     grafts, a shallow repository or `core.commitGraph` off (GT corrections). Using it anywhere
     else would change the order, not only the cost, because a replaced commit's generation
     describes parents git no longer walks. Fixtures with a replace ref, a graft and a shallow
     clone pin this (C10). Whether this also closes #128 is phase 01's to check against that
     issue's scope. gitoxide reads v1 generations only (GX);
   - a priority queue whose ties pop in insertion order, as git's `prio_queue` does;
   - parents pushed in git's order;
   - synthetic tips (R1.3) whose parents are given by Cairn, never read.
2. It reads each tip's real commit. Cairn's parentless stand-in for a tip read (`Seeded`, WB
   §1.11) would cut the walk off.
3. It checks the epoch's cancel signal between commits, the opening pass included. A superseded
   or closed open stops within one commit's work and keeps nothing.
4. It reports how many commits the opening pass has read, at most once per 100 ms, for the
   window's count (R4.2).
5. It is the history's order (`walk::open` from the refs snapshot, the cold cursor path, a
   find's paging, Show Lost Commits):
   - `HistorySession` holds it across pages, and a page after the opening pass costs a page;
   - a cold restart repeats the opening pass, then replays the rows walked.
6. Show Lost Commits' "reached after all" correction (`reach.rs`, `RowsPage::reached`) never
   fires under this order. It is removed if no other order needs it; otherwise it is kept, with a
   test that it never fires for the history.

### R3 — The history thread (L4)

1. The history lane gets a thread of its own, with its own thread-local repository handle: the
   walk, its pages and its finds.
2. The refs snapshot is still read on the repository thread. That thread keeps the snapshot the
   open history was opened from, compares a refresh's refs with it as today (`walks_as`), and asks
   the history thread to reopen when they differ. The reopen's epoch supersedes a pass still
   running, so a refresh never waits behind an open. Ahead/behind is unaffected.
3. The sidebar's filter, the Changes tab's and Local Changes' filters, retires and every other
   job that ran on the repository thread no longer wait behind an open.
4. The routing table and the request arms move with the lane (`crates/cairn-app/src/worker/`).
   So do the UI-thread guard's partition and the root `CLAUDE.md`'s account of the routing.
5. Closing the window stops the history thread's open within one commit's work; the close's
   reaps are as before.

### R4 — The window during an opening pass (L3)

1. **The first open** (the application starting) shows the opening sentence until the first page
   arrives, as today. Once the pass has run 250 ms, the sentence becomes "Reading history… N
   commits".
2. **A reopen keeps the old rows drawn and usable** until the new first page arrives (brainstorm
   L7). This covers a fetch moving refs, a commit, Create Branch, the Show Lost Commits toggle and
   a refresh after an outside change.
   - Once the pass has run 250 ms, a strip over the list reads "Reading history… N commits".
   - The old history pages no further, since its walk is superseded: a scroll stops at its
     loaded rows.
   - The new first page replaces the old rows, keeping the selection by id as today. The old
     history is handed to a worker to free, as a reopen's is today.
3. The 250 ms is the worker's. The worker sends no count until the pass has run that long, so
   the window keeps no timer for it. The count updates at most once per 100 ms (R2.4).
4. A reopen asked while an open is still reading supersedes it.
5. No frame runs over 16.7 ms while an opening pass runs.

### R5 — Parity, pinned (L5)

1. Fixture repositories are built by real git, the stashes of each grafted onto their bases only
   in a copy made for the oracle (GT §4.2), so that git's commit-graph stays usable in the
   repository Cairn reads:
   - tied commit times across branches, merges, two stashes and a rebased run of five commits
     in one second;
   - clock skew, a child dated before its parent;
   - reflog entries for Show Lost Commits;
   - the ui-review recipe (`docs/research/ui-review/fixture-recipe.md`);
   - a replace ref, a graft and a shallow clone, for commit-graph eligibility (R2.1).

   Each is checked with and without a commit-graph file.
2. On each, Cairn's row ids equal the oracle's, for the refs, with stashes (grafted in the
   fixture, GT) and with Show Lost Commits.
3. The fixtures run in the gate's `git-floor` step at git 2.30.9 and 2.32.7, and in the full
   suite at the host's git.
   - `crates/cairn-git/tests/date_order.rs` is a new row of `scripts/git-floor.sh`'s table, with a
     floor of its own.
   - The history-level parity tests of C4 live in a registered target too.
4. On the bench, a reporter compares every row with git's own order and reports any difference;
   the run is recorded.

### R6 — Measured (L3)

Measured on the bench, read only (the bench rule in `CLAUDE.md`'s `Commands`; any `git` there
under `GIT_OPTIONAL_LOCKS=0`), with a commit-graph only in a copy. The numbers are recorded in
`docs/research/history-topo-order/measured.md`.

1. **The first page, from every ref, without a commit-graph.** No slower than git's own
   `git rev-list --date-order --max-count=64 --branches --remotes --tags HEAD` on the same
   repository: the median of seven within git's median plus 10% for noise. WB §3.6 measured that
   command at 1,247-1,300 ms. Phase 01 measures the walk's opening pass against it before
   phases 02 and 03 build on it.
2. **The same in a copy with a commit-graph**, against git's figure there (GT: 84 ms). gitoxide
   reads only v1 generation numbers, so where git's corrected commit dates let it stop sooner,
   the difference is recorded with its cause.
3. **A page after the opening pass**, and a find to the oldest commit, against today's cost.
4. **The walk's retained memory** after the opening pass and at the end of the walk, per commit.
5. **The window check's opening, and a filter keystroke during an opening pass.**

### R7 — Docs

1. `docs/systems/history-graph.md` describes the walk as built: the order, the opening pass, the
   history thread and the measured costs. Its "0-0.2% of rows" repaint figure is replaced by what
   the bench shows.
2. `docs/research/history-graph/gix-revwalk-ordering.md` is evidence and is not edited. The
   systems doc says what later research found (GX).

## Product rules

1. git's order is the truth. A row order that differs from the oracle's on any fixture, at any
   supported git, is a defect, never a gap to file.
2. Cancellation and closing never wait on a whole-history pass.
3. No write to a repository: Cairn never writes a commit-graph.

## Acceptance criteria

The packet's one authoritative copy. Each is met by the named test, written in the bracketed
phase.

| # | Criterion | Decided by |
| --- | --- | --- |
| C1 | Ties pop in insertion order, and parents are pushed in git's order. | `ties_pop_in_the_order_they_were_put`, `parents_are_queued_in_gits_order` (unit) [01] |
| C2 | `HistoryOrder::DateOrder`'s ids, the stashes seeded as tips, equal the oracle's on every R5.1 fixture, with and without a commit-graph, at the host's git and at both floors. Its opening pass on the bench is measured against R6.1's bar. | `the_walk_is_in_git_date_order` (cairn-git, real git; `git-floor` step) [01] |
| C3 | The opening pass stops within one commit's work once superseded, keeping nothing, and reports its count at most once per 100 ms. | `an_opening_pass_stops_when_superseded`, `an_opening_pass_reports_how_far_it_has_read` [01] |
| C4 | The history from a refs snapshot, the cold cursor path and a find's pages are in git's date order, stashes and Show Lost Commits included. | `the_history_is_in_git_date_order`, `stash_rows_are_where_git_puts_them`, `lost_commits_are_in_git_date_order`, `a_resumed_session_continues_git_date_order` [02] |
| C5 | The tests WB §3.1 lists that change with the order are rewritten with their reason: class A (failing by design), class E (cancel-poll counts, e.g. `opening_a_walk_from_many_refs_is_cancelled_between_tips`, `cancelling_stops_the_walk_and_keeps_what_it_had`), class F (window priming) and class B's stash look-ahead tests, which the stash tips retire. The lane assigner's tests, the frozen oracle's fingerprint and `recent_messages`' parity test pass unchanged. If R2.6 keeps the reach correction, a test shows it never fires for the history. | the named tests; `the_reference_assigner_is_the_one_compact_rows_were_checked_against` [02] |
| C6 | A filter is answered within its usual time while an opening pass runs on the history thread. A refresh whose refs moved supersedes a pass still running. A close during the pass ends within one commit's work. | `a_filter_is_answered_while_the_history_opens`, `a_refresh_supersedes_an_open_still_reading`, `a_close_during_an_opening_pass_does_not_wait_for_it` (worker tests) [03] |
| C7 | A first open shows "Reading history… N commits" once a pass has run 250 ms. A reopen keeps the old rows drawn and usable under the strip until the new first page replaces them, keeping the selection. | `the_history_says_it_is_reading_during_a_long_open`, `a_reopen_keeps_the_old_rows_until_the_new_page` [03] |
| C8 | On the bench, every row equals git's date order. R6's measurements meet R6.1 and are recorded with R6.2-R6.5. No frame runs over 16.7 ms during an opening pass (R4.5). | `#[ignore]`d reporters and the window check, read on the bench machine [01, 03, 04] |
| C9 | `docs/systems/history-graph.md`, every `docs/systems/` file that says what runs on which worker thread (`refs.md`, `sidebar.md`, `git-processes.md`, `diff.md`, `credentials.md`, `local-changes.md`) and the root `CLAUDE.md`'s routing account describe the code as built. | review [03, 04] |
| C10 | The walk uses the commit-graph's generations only where git does: with a replace ref, a graft or a shallow clone, the order still equals the oracle's. | `the_commit_graph_is_used_only_where_git_uses_it` (real git; `git-floor` step) [01] |

## Out of scope

- **`--topo-order`:** as a setting, the `preferences` packet's (L1).
- **Writing a commit-graph,** or offering to (L3).
- **Keeping the opening pass's state across a reopen, so a reopen pays only for what changed.**
  An optimization, filed at teardown.
- **A smaller lane-assigner window for this order:** an optimization the packet does not take.
- **gitoxide's corrected-commit-date reading:** upstream's (GX).
