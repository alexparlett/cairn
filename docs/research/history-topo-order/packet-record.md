# The packet's record: decisions pending review, QA and the merge bar's numbers

Evidence record for the `history-topo-order` packet, kept when its work directory was torn
down on 2026-10-18. It carries what the work directory's `progress.md` and `state.md` held that
no living doc does:
- the decisions made in flight that are **pending the user's review**, which are not approved;
- each phase's QA adjudication: what was confirmed and fixed, and what was dismissed and why;
- the merge bar's verification of every criterion against its test, and its bench numbers.

How the walk, the history thread and the window work now is in `docs/systems/history-graph.md`.
What the packet committed to is in `docs/prd/history-topo-order.md`, frozen and stamped shipped.
The numbers as each phase first measured them are in `measured.md` beside this file. Git history
keeps the work directory itself, under `docs/work/history-topo-order/`, up to the teardown
commit.

## Decisions pending the user's review

None of these is approved. Each was decided in flight so the work could go on, and is batched
here for the user. Where a living doc already states the behaviour, it is named.

### The walk and its order

1. **Show Lost Commits walks in date order whatever order the request names.** `with_order` is
   ignored with the toggle, so `reach.rs` gives a final verdict as each commit comes off the walk,
   and the "reached after all" correction (`RowsPage::reached`, `History::append`'s flag clearing,
   `Chunks::get_mut`) is removed. PRD R2.6 says it goes "if no other order needs it", and this
   reads that as: no route walks the toggle in another order.
2. **Commit-time and graph order put each stash directly above its base.** Their walks are given
   no stash. This was graph order's old rule; commit-time order used to place a stash by its date.
3. **A stash a ref reaches is that commit's row, with its real parents.** When a branch is built
   on a stash, the row has its real parents, index commit included, where the oracle's global
   `git replace --graft` would show one parent. This is what the user's own `git log --date-order
   --branches --remotes --tags HEAD` shows, with or without the stash given on stdin, and the
   oracle follows git there.
4. **`stash_rows_are_in_gits_date_order` asserts the oracle's order for a tied index commit.** Its
   sub-assertion about a stash and the index commit it shares a second with now asserts that the
   tie goes to the branch's commit, when a branch names that index commit. The refs' tips are given
   to the walk before the stashes.
5. **A ref tip whose object is missing fails the date-order walk**, as git's `get_reference` dies
   ("bad object"). This keeps `bad_starting_points_are_errors_rather_than_empty_pages`. A reflog id
   that cannot be read is still skipped.
6. **Each reflog id is read twice**: once to check it names a commit, once by the walk. This is
   the same count as the commit-time walk's date read plus its traversal read.
7. **`gix_revwalk::Graph` is not the commit store.** It borrows its commit-graph for its own
   lifetime, so a walk held across pages that opened its own graph could not keep one. The walk
   reads into gix_revwalk's `IdMap` of `graph::Commit`s itself.
8. **The environment's substitution variables are not honoured.** `GIT_GRAFT_FILE`,
   `GIT_NO_REPLACE_OBJECTS` and `GIT_REPLACE_REF_BASE`, if set in Cairn's environment, have no
   effect on the walk.
9. **A replace-ref or graft-file read failure surfaces as `Error::Walk`.**
10. **The oracle in `crates/cairn-git/tests/date_order.rs` grafts stashes through a graft file**,
    as `tests/support/date_order_oracle.rs` does, rather than with `git replace --graft`.

### The history thread and the window

11. **A reopen goes repository thread → window → history thread.** The repository thread answers
    `Update::Refs { reopen }`, and the window asks `OpenHistory`. PRD R3.2 has the repository
    thread ask the history thread to reopen itself. The route chosen keeps the window as the one
    numberer of the history lane, and lets it know to keep its rows. `docs/design/concurrency.md`
    says so.
12. **A find or its stop never supersedes or restarts a walk's opening pass.** An open whose walk
    is current runs whatever the history lane asked since. A cold restart's pass also runs under
    the walk's number, so only a reopen or a close stops it, and it tells no count.
13. **During a reopen the old rows' end is forgotten.** `Progress::reopen` sets complete to false,
    so pressing a ref the old rows do not label pages the new walk, rather than saying "not in the
    graph". A failed reopen keeps the rows and is retried, as a failed page is.
14. **A history-thread panic is announced as `WorkerLost` and leaves the other threads
    serving**, as a diff thread's panic does. Only the repository thread's panic ends the stream.
15. **`Threads::drop` stops the epochs itself**, so a walk ends even when every handle went away
    without a close.
16. **The opening pass with a commit-graph is recorded, not optimised.** With a commit-graph it
    runs 25-28 ms over git's time (PRD R6.2).

### Tests and the floor

17. **The floor for `--test date_order` in `scripts/git-floor.sh` rose from 6 to 10.**
18. **Three tests went with the mechanisms they pinned**, rather than being rewritten:
    - `looking_ahead_stops_once_the_walk_passes_the_bases_date`, replaced by
      `a_deleted_branchs_stash_pulls_nothing_ahead_and_has_no_row`;
    - `reach.rs`'s `a_parent_taken_for_lost_before_its_reached_child_is_reached_after_all`,
      replaced by `a_lost_commits_parents_are_lost_unless_a_ref_reaches_them`;
    - `chunked_store.rs`'s `an_item_changed_in_place_is_that_item_alone`, whose function was
      removed.

### Escalated: git parity outside the walk

19. **The amend's pushed check follows git's replaced and grafted view**, as `git branch -r
    --contains` does, not the objects the remote holds. A local replace ref or graft that cuts a
    remote's history short of `HEAD` makes a published commit read as unpublished, and an amend
    git logs then runs without asking. This was kept because it matches git, and
    `docs/systems/staging.md` states it. It is exact for the floors (2.30.9, 2.32.7). It is
    inexact for git 2.56.0 on a missing replacement: there `--contains` names no branch and exits
    0 (fails open), while Cairn fails closed, as the floors' `rev-list` dies.
20. **The Commit tab ignores replace refs.** gitoxide 0.87.1 reads `core.useReplaceRefs` inverted
    and applies no replacement by default. This git-parity divergence is older than the packet,
    and it is now visible beside a graph that applies replacements. `docs/systems/diff.md` states
    it. Whether to file it upstream as an issue, or to fix it in Cairn, is the user's call. It was
    deliberately not filed.
21. **#128 is left open.** The walk applies its rule (the commit-graph off where `refs/replace` or
    `info/grafts` exist, as git does) at every graph read it names. Whether that closes it is the
    user's call.
22. **A replace ref or a graft file changed alone reopens nothing.** This was found at teardown
    and is not filed. The walk now reads `refs/replace/`, `info/grafts` and the shallow file as
    git does. But whether a refresh reopens the history is decided by `RefsSnapshot::walks_as`,
    which compares only the snapshot and holds none of them. So the window draws the old order
    until a ref moves or Show Lost Commits is toggled, where the user's own `git log` would
    already answer differently. `docs/systems/history-graph.md` ("Known limits") and
    `docs/systems/refs.md` state it. Whether to file it, or to fold those files into what a
    refresh compares, is the user's call.

## QA, phase by phase

Each phase was reviewed by fresh reviewers, and their raw findings were adjudicated by a fresh
`qa-confirm`. The finding ids are the reviewers' own: QA and QC are `qa-checklist`, TC is
`test-coverage-auditor`, RR is `responsiveness-reviewer`, GATE is `gate-integrity-reviewer`, and
DO is `destructive-ops-reviewer`.

### Phase 01: the walk

Reviewers: `qa-checklist`, `test-coverage-auditor`, `gate-integrity-reviewer` and
`destructive-ops-reviewer`. The last was dispatched because the amend's pushed check reads the
walk.

Confirmed and fixed:
- **A missing replacement.** A replace ref whose replacement is missing read as a missing object,
  so a hiding walk could answer "not pushed" where git dies. It is now an error
  (`Grafted::try_find`), pinned by `a_replacement_that_is_not_there_fails_the_walk`.
- **`Replacements::read` now follows `register_replace_ref`.** A symbolic replace ref is resolved.
  A name's tail is read for its first id only. An unreadable ref, and two refs for one object, are
  refused, as git refuses them.
- **Coverage added**, each new test checked to fail under the mutation it names:
  - `core.useReplaceRefs=false`;
  - Show Lost Commits' reflog dates, and graph eligibility under substitutions;
  - each substitution fixture with and without a commit-graph;
  - the shallow file beating a graft;
  - commit details with graft-file parents;
  - the commit-time and graph-order routes, and ahead/behind, against git under each
    substitution (`every_route_sees_the_substitutions_git_sees`);
  - `REPORT_EVERY` pinned to 100 ms, and the public open's cadence
    (`the_public_walk_reports_its_count_every_100_ms`).
- **Docs.** `CLAUDE.md` and `docs/qa-gate.md` now say `git-floor` runs the date-order tests.
- **The oracle grafts only stashes whose commit no ref reaches.** A fixture now has a branch built
  on a stash.

Dismissed:
- **GATE-2** (the inline unit tests run only at the host git): no criterion asks for them at the
  floor. The floor's row is `tests/date_order.rs`, which `git-floor` runs at both floors.
- **QA-5** (`date_order.rs` near 1,300 lines, with no ratchet): no rule requires a ratchet.

### Phase 02: into the history

Reviewers: `qa-checklist`, `test-coverage-auditor`, `responsiveness-reviewer` and
`gate-integrity-reviewer`.

Confirmed and fixed:
- **QA-1.** The systems doc's pins table had the model test's old name and lacked C4's pins.
- **QA-5.** A `git-floor` comment cited acceptance ids.
- **TC-1.** `opening_a_walk_from_many_refs_is_cancelled_between_tips` no longer proved a poll
  before each tip. A missing sixth tip now decides it.
- **TC-2.** The stream's `by_commit` arm went undriven once date order was the default.
  `a_stash_commit_a_branch_reaches_is_a_commits_row_and_no_stashs` now runs in all three orders.

Confirmed, deferred to phase 03, and fixed there:
- **RR-1.** The opening pass held up the repository thread.
- **RR-2.** A find or its stop restarted the opening pass. This became pending decision 12.
- **RR-3.** The opening pass's count was not shown.

Dismissed:
- **QA-2** (`Reach` never settling on a dropped reflog id): `reflog_tips` already drops ids that
  are missing or are not commits. A stash's commit is cleared by `passed`, or becomes a tip.
- **QA-3** (Show Lost Commits with a stash only a reflog reaches): covered by `old_stashes` under
  `lost_commits_are_in_git_date_order`.
- **QA-4** ("costs no look ahead" in the systems doc): true of the code as built.
- **TC-3** (a reflog id set in hex order in one oracle call): the dates are distinct and the set
  compared is order-free. Tip order is pinned by `lost_commits_are_in_git_date_order`.
- **RR-4** (170 B per commit for the session's life): accepted in phase 01.

### Phase 03: the thread and the window

Reviewers: `qa-checklist`, `responsiveness-reviewer`, `test-coverage-auditor` and
`gate-integrity-reviewer`. There were 13 raw findings, and 11 were confirmed.

Confirmed and fixed:
- **QC-1.** An over-long doc comment line.
- **RR-1.** `WalkedRefs::refreshed` compared outside the lock, between two holds of it. It now
  holds the lock once.
- **RR-3.** The close cancelled a fetch only after joining the history thread.
- **QC-5.** A cold restart told its pass's counts, so the strip appeared over rows that nothing
  replaced. Pinned by `a_cold_restart_tells_the_window_no_count`.
- **TC-1, with QC-4.** The filter and refresh tests slept a quarter of a timed pass. They now wait
  for the pass's first count on a paced handle (`open_paced_with`).
- **TC-2.** The close test's "no page" assertion could never fail. It was removed.
- **TC-3.** There was no test of a panic on the history thread.
- **TC-4.** The RR-2 test depended on thread ordering. A serve-level test with the jobs queued
  first now decides it.
- **TC-5.** There was no boundary test of `reopen: false` for a refresh mid-pass.
- **QC-3.** The stop's promptness was no longer pinned.
- **The fix round.** A fresh `qa-confirm` found four low inaccuracies the fixes left, and a
  preemption window in `a_stop_ends_a_running_find_within_a_page`. All were fixed.

Dismissed:
- **QC-2** (a replaced history dropped on the UI thread while closing): unreachable. `closing` is
  set only after `Request::Close` has stopped the epochs.
- **RR-2** (each count re-runs the window builder): paced to at most 10 Hz, and bounded by the
  viewport. Measured at most 2.7 ms a frame.

### Phase 04: the merge bar

Reviewers: `qa-checklist`, `test-coverage-auditor`, `responsiveness-reviewer`,
`gate-integrity-reviewer` and `destructive-ops-reviewer`, over the whole packet diff. There were
8 raw findings: 2 confirmed, both coverage gaps, and 6 dismissed.

Confirmed and fixed:
- **DO-1(b).** The remote's side of the amend's pushed check, on a missing replacement, was
  unpinned. It is now pinned by
  `an_amend_is_refused_where_a_remotes_history_names_a_missing_replacement`.
- **TC-3.** `the_commit_graph_is_used_only_where_git_uses_it` now reads a stash on `main` under
  each substitution.

Dismissed:
- **DO-1(a)** (a hidden-side missing replacement read as "unpublished"): gix's hidden-frontier
  paint reads every hidden tip through `Grafted::try_find`, which fails a missing replacement.
- **DO-2** (a generation cut-off at a raw level, where git maps 0 to infinity): a zero level is
  never a descendant of a positive one, so the answer cannot differ.
- **RR-1** (Show Lost Commits' reflog read polls per reflog, not per commit): those polls were
  already on `main`, and the reads are bounded.
- **RR-2** (a replaced `History` dropped on the UI thread while closing): unreachable, as in phase
  03.
- **GATE-1** ("v1 generations at the floor"): 2.30.9 has no GDAT code, and gix reads CDAT's level
  alone.
- **TC-1** (the floor set one below the test count): this is the script's stated convention for
  every row.
- **TC-2** (the close test decided by wall time): it compares against a baseline in the same
  process, with a half-pass margin, so it can only fail falsely.

### Teardown: a test that read a count in place of a page

CI's `gate` job failed twice on the integration branch. On a slow runner,
`worker::pool::tests::superseding_a_request_stops_the_walk_that_is_serving_it` read
`Update::Reading` where it expected a first page. An opening pass that runs past the 250 ms
hold-back sends its count before the first page, and the test's helper skipped only the open's
refs.

This was reproduced locally by pinning the test binary to one core beside 40 busy loops. The fix:
- every test boundary that does not decide the count opens on `Pacing::UNTOLD`
  (`crates/cairn-app/src/worker/history/reading.rs`), through `pool::open_untold`, so no count is
  told however long a pass runs;
- the tests that decide the count keep their own pacing (`open_paced_with`, and the lane tests'
  `Pacing`).

No product code changed.

## The merge bar's verification

Each criterion's test was checked to fail under a mutation, then restored:
- **C1.** `ties_pop_in_the_order_they_were_put` (ties popped in reverse) and
  `parents_are_queued_in_gits_order` (parents pushed in reverse).
- **C2.** `the_walk_is_in_git_date_order` (the tie swap).
- **C3.** `an_opening_pass_stops_when_superseded` (the in-degree loop's poll removed) and
  `an_opening_pass_reports_how_far_it_has_read` (the report disabled).
- **C4.** The four date-order parity tests (`HistoryOrder::default()` put back to `CommitTime`).
- **C5.** The frozen compact-rows oracle is byte-identical to `main`, and
  `crates/cairn-model/tests/` is untouched.
- **C6.** `a_filter_is_answered_while_the_history_opens`,
  `a_refresh_supersedes_an_open_still_reading` and
  `a_close_during_an_opening_pass_does_not_wait_for_it`.
- **C7.** `the_history_says_it_is_reading_during_a_long_open` and
  `a_reopen_keeps_the_old_rows_until_the_new_page`.
- **C10.** `the_commit_graph_is_used_only_where_git_uses_it`.

### C8, measured again on the bench

Conditions: release build, `GIT_OPTIONAL_LOCKS=0`. A marker file was set at 11:30:40 +0100, and
`find ~/Development/bench/rust/.git -newer <marker>` was empty after every run. The commit-graph
copy was made fresh in the scratchpad: a plain copy of `.git`, no alternates, `gc.auto 0`,
`maintenance.auto false`, then `git commit-graph write --reachable`.

**Without a commit-graph:**
- **git's time.** git's `rev-list --date-order --max-count=64 --branches --remotes --tags HEAD`
  took a median of 1,257 ms [1,251-1,296]. The bar is 1,383 ms.
- **Cairn's first rows.** The history thread's open to its first 64 rows took a median of
  1,183 ms [1,178-1,186], so R6.1 is met.
- **The count.** It was told 9 times. The first came at 309 ms (78,085 commits). The smallest gap
  was 100 ms.
- **After the pass.** A page took 0.41 ms. The sidebar's filter took 0.03 ms alone, and 0.06 ms
  when asked 296 ms into a pass, answered before the first page every time.
- **Close.** A close 29.7 ms into a pass took 38.2 ms idle.
- **The order.** The whole history through the boundary equals git's `log --date-order` row for
  row, all 345,545 rows.

**With the commit-graph copy:** git took 86 ms [84-95] and Cairn 115 ms [113-122]. The order was
equal again.

**The window check:**
- The first open was drawn after 1,197 ms: 102 frames, the longest 6.78 ms.
- Scrolling 3,000 rows: the longest frame was 9.23 ms.
- A reopen scrolled every frame: 80 frames, the longest 2.27 ms.
- A filter keystroke during the reopen took 18.1 ms, against 17.2 ms idle.
- No frame was over 16.7 ms.

### C9

The reviewers checked the docs against the code:
- the root `CLAUDE.md`'s routing account, against `worker/pool.rs` and `worker/routing.rs`, arm by
  arm;
- every test name cited in `docs/systems/history-graph.md` and the other touched systems docs,
  found in the tree;
- `docs/qa-gate.md`'s `git-floor` rows, against `scripts/git-floor.sh`.

## Leftovers filed at teardown

- **#139.** Keep the date-order walk's opening state across a reopen. A reopen pays the whole pass
  again.
- **#140.** Lay out date order's first page without the lane assigner's 1,024-row priming.
- **#141.** Use corrected commit dates in the date-order walk once gitoxide reads them.

Not filed:
- **`--topo-order` as a setting.** It is the `preferences` packet's "Sort commits" (roadmap 5g).
- **Writing a commit-graph, or offering to.** Rejected when the packet was planned.
