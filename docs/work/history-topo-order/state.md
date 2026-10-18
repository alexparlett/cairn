# State — history-topo-order

The cross-session cheat sheet. Every session updates it before ending.

**Status: phase 02 built, 2026-10-18 (packet mode, on `feature/history-topo-order`).** Its base is `main`. It runs before `fork-alignment`.

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

Handed to phase 03 (from phase 02's QA, confirmed and deferred):
- The opening pass (about 1.2 s on the bench without a commit-graph) runs as one job on the
  repository thread: filters, a refresh's refs and retires wait behind every open and reopen
  until the history thread exists (packet qa-checklist item 7 fails until then).
- A `Request::FindRow` or `StopFinding` bumps the history lane's epoch, so a sidebar press during
  an opening pass cancels it and the find's page opens the walk again from nothing; phase 03's
  supersede rules should keep a pass that a same-walk find or stop-finding interrupts
  (or the user decides otherwise).
- The opening pass's count is discarded (`walk::open_from` passes `&mut |_| {}`); phase 03
  routes it to the window.

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 | done; C1, C2, C3, C10, C8 opening pass met | `scripts/gate.sh` full, exit 0 | done: 4 reviewers + qa-confirm, fixes landed, dismissals and pending decisions in progress.md |
| 02 | done; C4, C5 met | `scripts/gate.sh` full, exit 0 | done: 4 reviewers + qa-confirm, 4 fixes landed, dismissals, deferrals and pending decisions in progress.md |
| 03 | not started | — | — |
| 04 | not started | — | — |
