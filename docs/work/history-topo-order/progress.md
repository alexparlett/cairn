# Progress — history-topo-order

Running log, newest first. Each phase's dismissed QA findings are logged here with their
reasons, for the final QA phase to audit.

## 2026-10-18 — phase 02 built (packet mode)

Built on `feature/history-topo-order` (3c2c347, 4e442d3, then the QA fixes e8cc7e8, 1e2aba2):
the history's default order is `HistoryOrder::DateOrder`, so the refs snapshot's walk, the cold
cursor, `HistorySession` and a find's pages are git's `--date-order`; a refs walk gives each
stash to the walk as a tip on its base after `HEAD`, and Show Lost Commits' reflog ids after
the stashes; `stream.rs`'s date merge and look-ahead are retired; Show Lost Commits forces date
order, so `reach.rs` gives a final verdict as each commit comes off the walk and the
reached-after-all correction (`RowsPage::reached`, `History::append`'s flag clearing,
`Chunks::get_mut`) is removed (R2.6: no other order needs it, since none walks with the toggle).

C4: `the_history_is_in_git_date_order`, `stash_rows_are_where_git_puts_them`,
`lost_commits_are_in_git_date_order` and `a_resumed_session_continues_git_date_order` in
`crates/cairn-git/tests/date_order.rs` (git-floor row, floor 6 → 10 for 11 tests, green at
2.30.9 and 2.32.7). Mutations each failing at least one test: the default back to `CommitTime`;
no stashes given to the walk; Show Lost Commits not forcing date order (with the default kept and
with it reverted); `Reach` deciding lost wrongly or settling early; the graph-order stash
placement removed; the stream's `by_commit` arm removed; the cursor's refs/reflogs split
dropped; the per-tip poll removed; the bad-object arm removed.

Find to a deep ref (the stopping rule): `measures_a_find_through_the_boundary` on the bench, the
oldest commit (row 345,544), timed after the first page: 3,829 ms [3,822-3,839] at c05ece5 in
commit-time order against 1,952 ms [1,933-2,017] in date order — faster, so the rule did not
trigger (`docs/research/history-topo-order/measured.md`). Bench untouched: `find
~/Development/bench/rust/.git -newer <marker>` empty after both runs.

Tests changed by design (C5), each with its reason in its doc and still failing when broken:
- Class A: `a_backward_line_across_a_page_boundary_is_a_gained_segment` and
  `a_row_that_left_the_window_is_never_repainted` (`tests/history.rs`) now ask
  `HistoryOrder::CommitTime`, which still hands a parent over before its child, so they keep
  pinning the assigner's page and window behaviour; `a_session_over_a_skewed_history_returns_every_commit`
  became `..._in_gits_date_order` (oracle `git rev-list --date-order HEAD`, no repaints, and the
  skew asserted to change the order); `a_parent_dated_after_its_reached_child_is_not_drawn_as_lost`
  (`tests/lost_commits.rs`) now asserts the child comes off first and the rows are git's order.
- Class B (`tests/every_ref.rs`): `each_stash_on_a_walked_commit_is_one_row_with_one_line_to_it`
  keeps its row and line checks, placement now the oracle's;
  `a_stash_on_a_deep_base_is_drawn_at_its_date_or_directly_above_its_base` became
  `a_stash_on_a_deep_base_is_where_git_puts_it`; `a_stash_commit_met_while_looking_ahead_...`
  became `a_stash_commit_under_an_older_branch_commit_...`; `stash_rows_are_in_gits_date_order`
  compares every row with the grafted oracle; `looking_ahead_stops_once_the_walk_passes_the_bases_date`
  became `a_deleted_branchs_stash_pulls_nothing_ahead_and_has_no_row`.
- Class E: `a_page_cancelled_while_looking_ahead_...` became
  `a_page_cancelled_while_or_after_the_walk_opens_resumes_to_the_same_rows`;
  `a_cancelled_query_stops_walking_and_says_where`,
  `cancelling_exactly_at_the_page_boundary_is_still_a_cancellation` and
  `cancelling_a_session_stops_the_walk_and_keeps_its_progress` count the opening pass's polls
  (`opening_polls`) or open the walk first; `cancelling_stops_the_walk_and_keeps_what_it_had`
  (`session.rs`) opens the walk with an empty page first;
  `opening_a_walk_from_many_refs_is_cancelled_between_tips` kept its counts and gained a missing
  sixth tip that decides the poll before each tip.
- Class F: none moved — the priming tests and the window's tests pass unchanged.
- Beside them: `every_route_sees_the_substitutions_git_sees` compares Show Lost Commits with
  `git log --date-order`; `reach.rs`'s unit tests are rewritten for the final verdict (the
  clock-skew correction test replaced by `a_lost_commits_parents_are_lost_unless_a_ref_reaches_them`);
  the model's `a_lost_row_reads_back_lost_until_a_page_says_a_ref_reaches_it` became
  `a_lost_row_reads_back_lost_and_no_later_page_changes_it` and the ui's
  `a_lost_row_is_drawn_lost_and_no_other` appends a later lost page instead of a correction;
  `an_item_changed_in_place_is_that_item_alone` went with `Chunks::get_mut`, which nothing calls.
  The lane assigner's tests, the frozen oracle's fingerprint, compact rows' and
  `recent_messages`' tests pass unchanged.

QA: `qa-checklist`, `test-coverage-auditor`, `responsiveness-reviewer` and
`gate-integrity-reviewer` (for `scripts/git-floor.sh`), each spawned fresh over
`c05ece5...4e442d3`; 14 raw findings adjudicated by a fresh `qa-confirm`.

Confirmed and fixed:
- QA-1: the systems doc's pins table named the model test by its old name, and lacked C4's
  pins (1e2aba2).
- QA-5: the git-floor comment this change wrote cited acceptance ids (1e2aba2).
- TC-1: `opening_a_walk_from_many_refs_is_cancelled_between_tips` no longer proved a poll before
  each tip, the in-degree pass polling too; a missing sixth tip now decides it (e8cc7e8, checked
  to fail with the per-tip poll removed).
- TC-2: the stream's `by_commit` arm went undriven once the default was date order;
  `a_stash_commit_a_branch_reaches_is_a_commits_row_and_no_stashs` now runs in all three orders
  (e8cc7e8, checked to fail with the arm removed).

Confirmed, deferred to phase 03 (recorded in state.md's hand-off):
- RR-1: the opening pass runs on the repository thread, so filters, a refresh's refs and retires
  wait behind every open until the history thread exists (packet qa-checklist item 7).
- RR-2: a find or stop-finding bumps the history lane and cancels an opening pass of the same
  walk, which then starts again from nothing; phase 03's spec does not yet say a same-walk find
  must keep the pass. PENDING the user's review: name it in phase 03's brief, or accept it.
- RR-3: the opening pass's count is not yet shown.

Dismissed:
- QA-2 (`Reach` never settling on a dropped reflog id): `reflog_tips` already drops ids that are
  missing or not commits, through the walk's own view; a stash's commit is cleared by `passed` or
  becomes a tip; a commit git cannot parse fails the walk. No case leaves a tip pending.
- QA-3 (Show Lost Commits with a stash only a reflog reaches): covered by `old_stashes` under
  `lost_commits_are_in_git_date_order`, whose oracle grafts on refs-reached bases only.
- QA-4 ("costs no look ahead" in the systems doc): true of the code as built.
- TC-3 (a reflog id set in hex order in one oracle call): the fixture's dates are distinct and the
  set compared is order-free; tip order is pinned by `lost_commits_are_in_git_date_order`.
- RR-4 (170 B per commit for the session's life): accepted in phase 01.

Decisions batched for the user, PENDING review (not approved):
- Show Lost Commits walks in date order whatever order the request names (`with_order` is
  ignored with it), so its verdicts need no correction; R2.6's "removed if no other order needs
  it" read as: no route walks the toggle in another order.
- Commit-time and graph order, given no stash by their walks, put each stash directly above its
  base (graph order's old rule; commit-time order used to place a stash by its date).
- `stash_rows_are_in_gits_date_order`'s sub-assertion that a stash is drawn above the index commit
  it shares a second with now asserts the oracle's opposite: with a branch naming that index
  commit, the tie goes to the branch's commit, the refs' tips being given before the stashes.
- A ref tip whose object is missing fails the date-order walk, as git's `get_reference` dies
  ("bad object"), keeping `bad_starting_points_are_errors_rather_than_empty_pages`; a reflog id
  that cannot be read is still skipped.
- Each reflog id is read twice — once to check it names a commit, once by the walk — the same
  count as the commit-time walk's date read plus traversal read.
- Floor movement for batched review: `--test date_order` 6 → 10 in `scripts/git-floor.sh`.
- Three tests went with the mechanisms they pinned rather than being rewritten:
  `looking_ahead_stops_once_the_walk_passes_the_bases_date` (replaced by
  `a_deleted_branchs_stash_pulls_nothing_ahead_and_has_no_row`), `reach.rs`'s
  `a_parent_taken_for_lost_before_its_reached_child_is_reached_after_all` (replaced by
  `a_lost_commits_parents_are_lost_unless_a_ref_reaches_them`) and `chunked_store.rs`'s
  `an_item_changed_in_place_is_that_item_alone` (its function removed).

## 2026-10-18 — phase 01 built (packet mode)

Built on `feature/history-topo-order`: `HistoryOrder::DateOrder` (git's incremental date-order
walk, `crates/cairn-git/src/history/date_order.rs`), stashes as lazily decided synthetic tips,
cancellation and the 100 ms count; and, for every walk, the shallow file, `info/grafts` and
`refs/replace` read as git reads them, the commit-graph only where git would (`walk::commit_graph`).
Bench: the walk equals git's `--date-order` row for row; first page from every ref 1.19-1.20 s
against git's 1.25-1.26 s without a commit-graph, 95 ms against 82-84 ms with one; 170 B per
commit (`docs/research/history-topo-order/measured.md`). Tie-break swap and six other mutations
fail the tests.

QA: `qa-checklist`, `test-coverage-auditor`, `gate-integrity-reviewer` and
`destructive-ops-reviewer` (dispatched because the amend's pushed check reads the walk), each
spawned fresh over `5da3291...9ff096c`; findings adjudicated by a fresh `qa-confirm`.

Confirmed and fixed:
- A replace ref whose replacement is missing read as a missing object, so a hiding walk could
  answer "not pushed" where git dies: now an error (`Grafted::try_find`), pinned by
  `a_replacement_that_is_not_there_fails_the_walk` (git itself checked to fail there too).
- `Replacements::read` departed from `register_replace_ref`: a symbolic replace ref is now
  resolved, a name's tail read for its first id only, an unreadable ref and two refs for one
  object refused, as git refuses them.
- Coverage: `core.useReplaceRefs=false`; Show Lost Commits' reflog dates and graph eligibility
  under substitutions; each substitution fixture with and without a commit-graph; the shallow
  file beating a graft; commit details with graft-file parents; the commit-time and graph-order
  routes and ahead/behind against git under each substitution
  (`every_route_sees_the_substitutions_git_sees`); `REPORT_EVERY` pinned to 100 ms and the
  public open's cadence (`the_public_walk_reports_its_count_every_100_ms`). Each new test was
  checked to fail under the mutation it names.
- `CLAUDE.md` and `docs/qa-gate.md` now say `git-floor` runs the date-order tests.
- The oracle now grafts only stashes whose commit no ref reaches, with a fixture where a branch is
  built on a stash (see the question below). The floor for `--test date_order` is 6 (7 tests).

Dismissed:
- GATE-2 (the inline unit tests run only at the host git): no criterion asks for them at the floor;
  the floor row is `tests/date_order.rs`, which `git-floor` runs at both floors.
- QA-5 (`date_order.rs` near 1,300 lines, no ratchet): no rule requires a ratchet; about 735 lines
  are production code, the rest its tests.

Escalated, PENDING the user's review (not approved):
- The amend's pushed check follows git's replaced and grafted view, as `git branch -r --contains`
  does, not the objects the remote holds: a local replace ref or graft cutting a remote's history
  short of `HEAD` makes a published commit read as unpublished, and an amend git logs then runs
  without asking. Kept, matching git, and stated in `docs/systems/staging.md`.
- The Commit tab ignores replace refs (gitoxide 0.87.1 reads `core.useReplaceRefs` inverted and
  applies none): a git-parity divergence older than this phase, now visible beside a graph that
  applies them. Stated in `docs/systems/diff.md`; to be filed as an issue or fixed, the user's
  call.

Decisions batched for the user, PENDING review (not approved):
- `gix_revwalk::Graph` is not the commit store: it borrows its commit-graph for its own lifetime,
  so a walk held across pages that opens its own graph could not keep one. The walk reads into
  gix_revwalk's `IdMap` of `graph::Commit`s itself.
- A stash a ref reaches (a branch built on it) is that commit's row with its real parents, index
  commit included, where the oracle's global `git replace --graft` would show one parent. This is
  what the user's own `git log --date-order --branches --remotes --tags HEAD` shows, with or
  without the stash given on stdin; the oracle now follows git there.
- `GIT_GRAFT_FILE`, `GIT_NO_REPLACE_OBJECTS` and `GIT_REPLACE_REF_BASE` in Cairn's environment are
  not honoured.
- A replace-ref or graft-file read failure surfaces as `Error::Walk`.
- Not filed or closed, for the user: the gitoxide replace-refs issue; #128, whose rule this phase
  applies at every graph read it names (left open).

## 2026-10-17 — planned

Planned with `/feature-plan`, on the bug `fork-alignment`'s planning found: with tied commit
times Cairn draws a parent above its child.

Evidence in `docs/research/history-topo-order/`:
- git's order and algorithm, cited from git's source at v2.30.9 and v2.56.0, which the user
  approved fetching;
- gitoxide's walks;
- Cairn's walk as built.

The user locked L1-L7 (`brainstorm.md`); L8 records two corrections a coverage audit found:
- date order;
- Cairn's own walk;
- accept the opening pass, visibly;
- a history thread;
- the oracle;
- the git source fetched for citations;
- a reopen keeping the old rows.

`docs/design/history-graph.md` and `concurrency.md` were rewritten to the end state.
