# State — history-topo-order

The cross-session cheat sheet. Every session updates it before ending.

**Status: phase 04 QA done, READY for teardown, 2026-10-18 (packet mode, on `feature/history-topo-order`).** Its base is `main`. It runs before `fork-alignment`. Teardown (phase 04 STEP 5 onward) has not run; the decisions progress.md lists as PENDING the user's review are not approved.

## Locked decisions (brainstorm.md has the evidence and the rejections)

- **L1. Order.** `git log --date-order`, which is Fork's default. `--topo-order` is a possible
  later setting.
- **L2. The walk.** Cairn's own git date-order walk over `gix_revwalk::Graph`, with ties popping
  in insertion order. Not gix's `Topo`: its DateOrder breaks ties arbitrarily.
- **L3. The opening pass.** Accept it, visibly:
  - "Reading history… N commits";
  - cancellable;
  - the first-page bar is git's own `--date-order` time, plus 10%;
  - the memory rule is restated.
- **L4. The history thread.** The history gets a thread of its own, so filters and retires never
  wait on an open.
- **L5. The oracle.** `git log --date-order --format=%H --branches --remotes --tags HEAD`, with
  tips in a fixed order:
  - stashes grafted onto their base and fed on stdin;
  - Show Lost Commits' reflog ids fed on stdin;
  - identical at git 2.30.9, 2.32.7 and 2.56.0.
- **L6.** git's source is at `~/.cache/cairn/git-src/v2.30.9` and `v2.56.0`, read only.
- **L7.** A reopen keeps the old rows drawn and usable, with a "Reading history… N commits"
  strip, until the new first page replaces them. A first open shows the sentence.
- **L8.** Two corrections:
  - stashes are synthetic one-parent tips of the walk (after `HEAD`, before the reflog tips),
    not rows merged into the stream;
  - the refs read stays on the repository thread, which asks the history thread to reopen,
    superseding a running pass.

## Interfaces introduced so far

Phase 01 (`crates/cairn-git`):
- `HistoryOrder::DateOrder` — git's `--date-order`; `HistoryRequest::with_order` accepts it on
  every start, and the stream still merges stash rows by date over it (phase 02 replaces that
  with the walk's own stash tips). `HistoryOrder::default()` is still `CommitTime`.
- `history/date_order.rs`: `DateOrder<'repo>` (crate-internal) — `open(repo, Tips { refs,
  stashes: &[(stash, base)], reflogs }, cancel, report: &mut dyn FnMut(usize), every)` →
  `Ok(None)` when cancelled; `next(cancel)` → `Popped::{Commit { info, stash: Option<usize> },
  End, Cancelled}` (a cancelled step changes nothing); `read()`, `read_from_graph()`,
  `walked()`, `retained()`. `REPORT_EVERY` = 100 ms. Stashes are decided lazily (see
  `decide`); with reflog tips, before they join.
- Public: `Repository::date_order(&DateOrderTips, cancel, read) -> DateOrderWalk`
  (`DateOrderTips::new(refs).with_stashes(..).with_reflog_tips(..)`, `DateOrderWalk::next`
  → `DateOrdered { id, parents, stash }`, `commits_read`, `read_from_graph`, `walked`,
  `retained_bytes`) — for the parity tests and reporters; phase 02 is expected to drive the
  internal `DateOrder` from `open_stream` instead.
- `walk::CommitWalk` is now an enum (`Simple` | `DateOrder`) stepped by `next(cancel)` →
  `walk::Step::{Commit, End, Cancelled}`; `Stream::pull` maps `Cancelled` to `Next::Cancelled`.
- Substitutions as git reads them, for every walk: `src/grafts.rs` (`Grafts`: shallow file +
  `info/grafts`, replacing `src/shallow.rs`), `src/replacements.rs` (`Replacements`:
  `refs/replace/`, because gix 0.87.1 reads `core.useReplaceRefs` inverted and applies none by
  default), `walk::Substitutions`, `walk::Grafted::{new, uncached}` (owns a handle clone with
  gix's replacements off), and `walk::commit_graph(repo, &substitutions)`, the one graph
  eligibility rule (used by the walks, Show Lost Commits' tip dates and the amend's pushed
  check). A missing replacement is an error (`walk::ReplacementMissing`), as git dies.
- `crates/cairn-git/tests/date_order.rs` holds the oracle helpers phase 02's history-level
  parity tests can reuse (`oracle`, `tips_for`, `ref_tips`, `stash_tips`, `reflog_tips`); its
  oracle grafts only stashes whose base the refs reach and whose commit they do not.

Phase 02 (`crates/cairn-git`, `crates/cairn-model`):
- `HistoryOrder::default()` is `DateOrder`: `from_refs`, `from_head`, `from_commits`, the cold
  cursor, `HistorySession` and so a find's pages all walk git's date order. `CommitTime`
  (Recent Commit Messages, explicit) and `GraphOrder` are unchanged.
- `history.rs` `open_stream` gives the date-order walk `date_order::Tips { refs, stashes,
  reflogs }` — the decoration's stashes in `stash@{n}` order, the reflog ids after them — through
  `walk::open_from`; `walk::open` keeps its old signature (refs only) for Recent Commit
  Messages and reporters; `walk::Step::Stash(index)` is a stash's row (date order only), its
  index into the decoration's stashes.
- `stream.rs` is thin: date order hands the walk's rows on; commit-time and graph order put each
  stash directly above its base (`by_base`, `by_commit` kept for them). `LOOKAHEAD`,
  `with_stash_lookahead`, the cursor's/request's `lookahead`, `seeds::Stash::base_committed`
  are gone. `Entry::Commit` carries `lost`.
- Show Lost Commits forces `DateOrder` (`starting_points`, on an unread marking); `reach.rs` is
  `Reach { frontier, below_lost, pending, settled }`, `pulled(id, parents) -> bool` (final) and
  `passed(stash id)`; `reflogs::reflog_tips` returns ids only (still filtered to commits).
- `RowsPage::reached`, its `reached` field, `History::append`'s flag clearing and
  `Chunks::get_mut` are removed.
- `DateOrder::open` fails a ref tip whose object is missing (`BadObject`, git's "bad object").
- Tests: C4's four in `tests/date_order.rs` (`the_history_is_in_git_date_order`,
  `stash_rows_are_where_git_puts_them`, `lost_commits_are_in_git_date_order`,
  `a_resumed_session_continues_git_date_order`, over `history_fixtures()` incl. `old_stashes`);
  `tests/support/date_order_oracle.rs` (`git_date_order`, `stash_tips`; grafts via
  `GIT_GRAFT_FILE`, needs the binary's `fixtures` module, whose `empty_home` is now `pub`),
  used by `every_ref.rs` and `lost_commits.rs`. git-floor `--test date_order` floor 10 (11 tests).

Phase 03 (`crates/cairn-git`, `crates/cairn-app`):
- `HistorySession::open_walk(cancel, every, report)` (public): runs the session's opening pass
  alone, under its own cancel, reporting commits read at most once per `every`; a cancelled
  open keeps nothing; `next_page` opens with a no-op reporter if not yet open.
  `walk::open_from` and `history::open_stream` take `report` and `every`.
- The history thread, `cairn-history` (`worker/history/lane.rs`: `serve_history`, `Serving`,
  `HistoryLane::{new, open_walk, more, find_page}`): its own `to_worker()` handle; serves
  `HistoryJob::Page { epoch, page }` (`routing.rs`, `Routed::History(Page)`,
  `Thread::History`) and `HistoryJob::Stop`. An `Page::Open` whose walk is current always runs
  (pass + first page under `epochs.watch(walk)`), whatever the history lane asked since;
  More/Find under the history lane's epoch. A cold restart's pass also runs under the walk.
- `worker/history/refs.rs`: `WalkedRefs` (latest + walked_from behind a mutex shared by the
  repository and history threads; `opening()` names the snapshot taken the walked-from one at
  once), `refs::refresh` (the repository thread's refs read, ex-`HistoryLane::refresh`).
- `worker/history/reading.rs`: `READING_AFTER` 250 ms, `READING_EVERY` 100 ms, `Pacing`,
  `ReadingPace`; `Update::Reading { commits }` under the walk's epoch.
- `Threads::drop` stops the epochs, tells the history, diff and refresh threads to stop,
  cancels a fetch, then joins the history thread, before the local lane join and
  `end_invocations`. A cold restart's pass (`answer`) runs under the walk's number and tells no
  count (`open_pass(.., told)`). `pool::open_paced_with` (test) opens with a given `Pacing`.
- Window: `Progress::{reopen, is_reopening, reading, read_so_far}`; `session::reopen_history(progress,
  lost, submit)` only marks and asks; `session::apply`'s `Update::Rows` replaces the history
  (sized, old retired) when reopening; `status_text::{placeholder(&Progress), reading_strip}`;
  `window.rs`'s `reading_strip`.
- Test fixture: `WrittenRepository::packed(name, count)` (one pack + v2 index, CRC-32).
- Reporters: `measures_the_history_thread_against_a_named_repository` (worker),
  `opening_pass_check` (window); `measures_layout_over_every_ref_of_a_named_repository` walks
  date order too.

Handed to phase 03 (from phase 02's QA, confirmed and deferred) — all three fixed in phase 03
(progress.md, RR-1/2/3):
- The opening pass (about 1.2 s on the bench without a commit-graph) runs as one job on the
  repository thread: filters, a refresh's refs and retires wait behind every open and reopen
  until the history thread exists (packet qa-checklist item 7 fails until then).
- A `Request::FindRow` or `StopFinding` bumps the history lane's epoch, so a sidebar press during
  an opening pass cancels it and the find's page opens the walk again from nothing; phase 03's
  supersede rules should keep a pass that a same-walk find or stop-finding interrupts
  (or the user decides otherwise).
- The opening pass's count is discarded (`walk::open_from` passes `&mut |_| {}`); phase 03
  routes it to the window.

Handed to phase 04 (the packet's QA over the whole diff):
- Decisions PENDING the user's review, listed in progress.md's phase 03 entry (the reopen asked
  by the window, a find never superseding an opening pass, a cold restart's pass under the walk's
  number telling no count, the reopen forgetting the old rows' end, a history-thread panic
  announced without ending the stream, `Threads::drop` stopping the epochs).
- To file at teardown: keeping the opening pass's state across a reopen.
- Tests needing a pass that takes a while use `WrittenRepository::packed` (200,000 commits, about
  0.8 s to write in a debug build) and `pool::open_paced_with` (every count told).

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 | done; C1, C2, C3, C10, C8 opening pass met | `scripts/gate.sh` full, exit 0 | done: 4 reviewers + qa-confirm, fixes landed, dismissals and pending decisions in progress.md |
| 02 | done; C4, C5 met | `scripts/gate.sh` full, exit 0 | done: 4 reviewers + qa-confirm, 4 fixes landed, dismissals, deferrals and pending decisions in progress.md |
| 03 | done; C6, C7, C8, C9 met; RR-1/2/3 fixed | `scripts/gate.sh` full, exit 0 | done: 4 reviewers + qa-confirm, 10 fixes landed, dismissals and pending decisions in progress.md |
| 04 | done; C1-C10 each verified against its test (mutations in progress.md), the bench re-measured and git's order equal on 345,545 rows | `scripts/gate.sh` full at 0eab817, `gate: PASS`, exit 0 (git-floor: date_order 11 tests, floor 10, nothing skipped at 2.30.9 or 2.32.7) | done: 5 reviewers + qa-confirm over the whole diff, 8 raw findings, 2 coverage gaps fixed (0eab817), 6 dismissed with reasons in progress.md |
