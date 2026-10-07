# State — refs-and-status

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 07 implemented (chips on rows, stash rows, the REFS row, a stash's changes as `git stash show` lists them, the title bar), full gate green; its QA is the coordinator's, pending; phase 08 next.** Integration branch
`feature/refs-and-status`, in the worktree `.claude/worktrees/refs-and-status`,
packet mode.

## Locked decisions

L1-L14 in `brainstorm.md`; the spec is `docs/prd/refs-and-status.md`. The ones
that most constrain implementation:

- **Status is `git status --porcelain=v2 -z`, run as a read** (L1). Not gix's
  status, not a hybrid. Untracked per file unless the user set
  `status.showUntrackedFiles=no`; ignored never listed (L2).
- **Refs are gix's, under five parity rules** (L3): never peel a symbolic ref,
  hide a dangling one, read the stash reflog oldest-first then reverse, resolve
  `remote = .` upstreams by hand, and resolve every other upstream by hand too
  (first `merge`, literal match against the remote's refspecs in order).
  `GIT_NAMESPACE` is not honoured. A ref naming a missing object is skipped and
  counted (user decision; git fatals).
- **Reftable is refused at open** (L4), and so is a format-version-0 repository
  that sets `extensions.refStorage` at all (user decision, as git refuses it).
- **The walk seeds from every ref; a stash is a row of its own** with one edge to
  the commit it was made on, never a tip (L5). No working-tree row (L6).
- **Fork's labels, sidebar, toolbar, Local Changes and refresh** (L7-L11), every
  deviation named: a generic remote glyph until packet 6; no working-tree row.
- **Refresh on focus, after an operation, and on the Refresh chord; no watching**
  (L11). A changed snapshot (any ref, `HEAD`'s state, the stash list) reopens the
  history; a reopen frees its old rows off the UI thread (#52).
- **Slim rows** (L14): one id, a parent count, shared text and author stores, no
  email, no per-row allocation (phase 04).
- **Compact rows, no cap** (brainstorm L13): a row keeps its id, parents, text,
  lane and only the lane changes at it; the drawn edges are derived. A deep find
  pages as planned and retains only compact rows (C16: 64 MiB for all of
  rust-lang/rust, against 1.4 GiB today). A deliberate deviation from Fork, which
  caps its list and does nothing when a ref is past it.
- **Threads and lanes** (brainstorm, "Settled in the coverage audit"): refs on
  the history thread; status and ahead/behind on a third thread; a ref's find is
  history-lane work. A stash is drawn only on a commit the ref walk reaches, as
  in Fork; nothing about a stash seeds the walk (revised after
  `fork-unreachable-stash-base.md`).
- **`status.showUntrackedFiles` is read by git itself** (the user's decision,
  2026-10-06): a first `git status` with no `--untracked-files`, and a second with
  `--untracked-files=all` only where the first collapsed an untracked directory — no
  `git config` read, no gix reading. A nested repository makes every read two.

## Open questions

- Decided by the user (2026-10-07, phase 06 QA): **RR1** — the refs stay read on the
  repository thread (R11.2), so a page asked during a refresh waits behind one refs read;
  the cost is written in `docs/systems/history-graph.md`, "Refresh". **RR2** — status is
  coalesced, not superseded: R10.3 amended so a refresh leaves a running `git status` to
  finish (its answer drawn) and every refresh asked meanwhile is one follow-up; only a close
  ends one. Ahead/behind and refs still supersede lane by lane.

- For the user's end-of-packet batch (QA finding QC-F6, phase 01): a ref whose name git
  calls invalid (`refs/heads/bad..name`) is skipped silently, as gix skips it, and not
  counted in `RefsSnapshot::unreadable`, though git warns `ignoring ref with broken name`.
  Kept as built; whether it should be counted is the user's call.

- For the user's end-of-packet batch (phase 02 QA; not decided): a failed status read —
  the partial-clone case among them, where a staged rename needs a blob only the promisor
  holds — blanks the whole working-tree view, where a failed diff fails one query. Options
  when Local Changes draws it (phase 09): keep `Error::GitFailed` (as built); a named state
  classified by the repository's state; retry without rename detection; or let status
  lazy-fetch.
- For the user's end-of-packet batch (phase 06; not decided): the Refresh chord is F5 on
  Linux and ⌘R on macOS, Fork's own (`fork-dev/Docs` `keyboard-shortcuts-windows.md` and
  `-mac.md`, read 2026-10-06; Linux takes Fork's Windows row). F5 collides with no Linux
  desktop chord nor any Cairn chord, so it was not a stopping rule; but it is the table's
  first chord with no modifier, so the table's rule "every chord holds a modifier" was
  amended to "every chord holds a modifier but a function key's"
  (`chords_are_distinct_and_every_bare_one_is_a_function_key`). The alternative is Ctrl+R
  (⌘R's mechanical Linux row).
- For the user's end-of-packet batch (phase 06; not decided): what reopens the history is
  `RefsSnapshot::walks_as` — refs, their targets and symbolic targets, `HEAD`, the stash list
  — so a refresh that finds only an upstream's configuration or the unreadable count changed
  hands the window the new snapshot without reopening; ahead/behind is counted afresh on every
  refresh (it is not part of the comparison, R10.4), on the refresh thread behind status.
- For the user's end-of-packet batch (phase 06; not decided): a ref whose commit is gone
  between a refresh and the open (deleted and pruned) makes the open read the refs again and,
  if they differ, open once more from them (the hand-off's first option), rather than drop the
  tip; a failed open forgets its refs, so the next open reads its own and the next refresh
  reopens.
- C11's first-page bar is written as 200 ms because history-graph's A7 has no
  number (L12); the user may revise it at the merge bar.

- For the user's end-of-packet batch (phase 04; not decided): authors are numbered on
  the window's side — a page names its own authors once and `History::append` adds only
  those the history lacks — where R4.7's letter has the worker send only authors new to
  the history; a parent count saturates at 65,535; a history past its stores' 32-bit
  addresses answers `HistoryFull`; and reading rows through the stores costs about
  0.06 ms a frame more at the median (1.31 → 1.37 ms, `window_check`), judged not to be
  the stopping rule's "costs a frame measurably". Numbers in progress.md. Also (phase 04
  QA, RR1): the author index is a standard hash map that grows by doubling, unlike
  R4.7's "the history's stores grow in fixed chunks, never by doubling" — a rehash of
  every author so far at each doubling, on the UI thread inside one append (0.48 ms
  measured at 57,000 authors; rust-lang/rust has 8,424). Kept as built; no identity
  hasher (the fixed-key SipHash key would let a crafted repository collide buckets) and
  no chunked index.

- For the user's end-of-packet batch (phase 05; not decided): a stash's base is found by
  looking ahead at most 4,096 commits in the walk when the stash's date comes up — beyond
  that the row is drawn directly above its base rather than at its date; each stash keeps
  its own lane down to its base; a stash commit a ref reaches is a commit's row and no
  stash's; a stash's subject is the stash list's message, its date the author date, its
  place by committer date; labels in the snapshot's order. Detail and the alternative in
  progress.md.
- Decided by the user (2026-10-07, phase 07): **Q1** — a stash's changes are asked of `git
  stash show --raw` itself, the third porcelain read (`reads/stash_changes.rs`), git reading
  `stash.showIncludeUntracked`; guard `the_porcelain_reads_are_the_three_named_queries`.
  **Q2 (phase 05 QA's QC4)** — a stash commit filed twice is one row, its newest entry's; the
  sidebar (phase 08) still lists every entry.
- For the user's end-of-packet batch (phase 07; not decided): label order (current branch
  first, then local, remote-tracking, tags; Fork's tag position is OPEN 3, git puts tags
  before remotes); compaction by the configured upstream only, read from the refresh's
  snapshot; chips cut by building only what a lower bound of their widths fits; glyphs painted
  as paths, the generic remote a cloud; the title bar's counts as Fork prints them, `18↓ 1↑`
  with a zero left out, where R7.1 wrote `↓n ↑m`; `HEAD detached at <short>`, `<name> (no
  commits yet)`, `upstream gone`; the repository named by the opened path's last component; no
  `*` for an unreadable index; the title bar drops the full path; the REFS row drawn in the
  row's lane colour and laid out for the window's width, a stash's REFS its `stash@{n}`; and
  painting each place a row's lines reach once (RR2's drawing half: 5,000 open lines painted
  175,000 strokes a viewport). Detail in progress.md, phase 07.
- Bench hygiene (phase 05): a `--shared` scratch clone's `git stash` freshened the bench
  pack's mtime through alternates, and a plain `git status` on the bench moved its `.git`
  directory's mtime; no content changed (progress.md). Later phases: scratch clones with
  no alternates, and `GIT_OPTIONAL_LOCKS=0` for any `git` run on the bench.

## Handed to phase 06 (done)

Every hand-off is done (progress.md, "phase 06"): `from_refs` wired (`HistoryLane`); the unborn
`HEAD` with no ref answers the empty, complete page, `no_walk` gone with `from_head`'s use; a
walk error arrives from the first page; the stale tip re-read and reopened once; RR2 measured;
`History::with_author_capacity` with its model test; the reopen's rows freed on a worker; the
fetch's `reload_if` comparison and `Repository::ref_tips` gone.

## Handed to phase 08

- Phase 06 QA's TC3: the phase that first submits `Request::FilterRefs` adds a boundary test
  of the ref-filter lane — an answer arrives, a superseded one is dropped, and one superseded
  mid-match stops (the `|| true` keep-going, a dropped `is_current` and a `None` epoch each
  slip today, since nothing asks the lane).

## Handed to phase 07 (done)

Every hand-off is done (progress.md, "phase 07"): the chips read `HistoryRow::labels` and the
refresh's snapshot, the title bar `refs().head`, `ahead_behind_of` and `status()` (whose
`expect(dead_code)` went; `ahead_behind()` keeps it for the sidebar, `failure` for the
sidebar and Local Changes); RR2's drawing half measured, and its cost removed by painting
each place once; chips stop being built at the column's edge (RR3); labels' order decided
(batched above); the stash row's R6.2 Commit and Changes built on the user's Q1, QC4 on Q2.

## Handed to phase 08 (from 07)

- The sidebar's chips and glyphs: `cairn_ui::{Chip, ChipKind, RefGlyph, chip_element, tint}`
  draw a ref as the rows do; `RefGlyph::Current` and `RefGlyph::Branch` are there for the
  current branch's mark.
- `RefreshState::ahead_behind()` (each branch's counts, for the sidebar's arrows) and
  `RefreshState::failure` still carry `expect(dead_code)`; no view says a failed refresh yet
  — the title bar draws the last answer it has.
- A stash's sidebar entry lists every entry, a duplicate commit's included; pressing one
  selects its row (one per commit, its newest entry's) or, with no row, asks
  `Comparison::Stash` of it.

## New modules and interfaces

Phase 01 (`docs/systems/refs.md` is the as-built account):

- `cairn-model`: `crates/cairn-model/src/refs.rs` — `RefsSnapshot { refs, head,
  stashes, unreadable }` (`find`, `of_kind`), `Ref { name, kind, target, symbolic,
  upstream }` (`commit_id`), `RefKind`, `RefTarget` (`Commit`, `Tag { object,
  commit }`, `Other`; `commit_id`, `object`), `Upstream` (`Exists { name, commit }`,
  `Gone { name }`), `HeadState` (`Branch`, `Detached`, `Unborn`), `StashEntry {
  index, message, commit, base }`, `AheadBehind { ahead, behind }`. The model's
  accessor is `commit_id`, not `commit`: `.commit(` is on the gitoxide-mutation
  roster for `cairn-git`'s source.
- `cairn-git`: `Repository::refs(&cancel) -> RefsRead { snapshot, cost: RefsCost }`
  (`src/refs.rs`, `src/refs/stash.rs`, `src/refs/upstream.rs`);
  `Repository::ref_tips()` is now the snapshot (the network lane compares
  snapshots); `Repository::ahead_behind(&snapshot, &cancel) -> AheadBehindRead {
  counts, commits_read, elapsed }` (`src/ahead_behind.rs`, over
  `history::walk::hiding`, no commit-graph so every read is cancellable);
  `src/ref_storage.rs` refuses at open (`Error::RefStorageUnsupported`,
  `Error::RefStorageNeedsFormatVersion1`); the open clears gix's refs namespace.
  New errors: `RefsCancelled`, `AheadBehindCancelled { branches }`.
- Gate: `require_reftable_where_possible` in `scripts/gate.sh`'s `test-full`
  sets `CAIRN_REQUIRE_REFTABLE`; twin
  `the_reftable_refusal_is_required_wherever_it_can_run`.

Phase 02 (`docs/systems/status.md` is the as-built account):

- `cairn-model`: `crates/cairn-model/src/status.rs` — `WorkingTreeStatus` (`Listed(Vec<StatusEntry>)`,
  `IndexUnreadable(UnreadableIndex::Sparse)`, `NoWorkingTree`), `StatusEntry` (`Changed`,
  `Conflicted`, `Untracked`; `path`), `ChangedEntry { path, staged, unstaged, submodule }`,
  `StagedChange`, `UnstagedChange` (`IntentToAdd`, and `Renamed`/`Copied` for git's one
  index-to-worktree pairing), `SubmoduleState`, `ConflictedEntry`, `ConflictKind` (`ALL`,
  `code`, `from_code`, `from_stages`).
- `cairn-git`: `Repository::status(&git, &cancel) -> Result<WorkingTreeStatus, Error>`
  (`src/status.rs`), over `reads::status` (`src/reads/status.rs`); new error
  `StatusCancelled`. It blocks: phase 06 runs it on the third thread.
- Tests: `crates/cairn-git/tests/status.rs` (C4's oracles; the `#[ignore]`d C11 reporter
  `measures_the_status_read`), run by `scripts/git-floor.sh` as a third run (`--test
  status`); unit tests in `src/reads/status.rs` (parser, stub argv and environment, C5's
  cancel, the index left byte-identical, R3.7's version and index rule).

Phase 03 (`docs/systems/history-graph.md`, "What a row keeps"):

- `cairn-model`: `GraphRow { id, lane }` plus private lane changes and an optional
  snapshot — `GraphRow::new(id, lane, changes)` (a row drawn on its own), `changes()`,
  `has_snapshot()`, `lanes_named()`, `heap_bytes()`; `LaneChange` (`Ends`, `Starts`,
  `StartsLate { lane, rows, order }`; `lane()`); `LaneSnapshot` (opaque; `heap_bytes`);
  `RowEdges { lane, edges }` and `row_edges(rows, index)` (`src/edge_derivation.rs`,
  generic over `AsRef<GraphRow>`, which `GraphRow` and `HistoryRow` implement).
  `LaneAssigner::SNAPSHOT_EVERY` (64), `MAX_SNAPSHOT_EVERY` (4096), `with_snapshot_every`,
  `snapshot_every`, `drawn_from(row)` (the first kept row carries a snapshot),
  `assign_each`; `rows()` is gone. `GraphRow::edges` is gone: every reader derives.
- `cairn-git`: a session and a cold page call `drawn_from(skip)`.
- `cairn-ui`: `RowRender::graph` is the drawn row's `RowEdges`; `CommitRow::new` takes
  it; `graph_geometry::row_geometry` and `graph_cell` draw a `RowEdges`.
- `cairn-app`: `history_state::widest_lane` reads `GraphRow::lanes_named`.
- Tests: C15's oracle is `crates/cairn-model/tests/layout_before_compaction/mod.rs`, the
  pre-compaction assigner verbatim, included by path from `crates/cairn-git/tests/compact_rows.rs`
  and `crates/cairn-ui/tests/history_list.rs`; never edit it. The `#[ignore]`d reporter
  `measures_compact_rows_over_a_named_repository` (modes `find`, `equivalence`, `derive`).

Phase 04 (`docs/systems/history-graph.md`, "What a row keeps"):

- `cairn-model`: `History` (`new`, `len`, `row`, `rows`, `id`, `position`, `append`,
  `author_count`, `retained`), `HistoryRow<'h>` (a view: `index`, `id`, `lane`,
  `changes`, `has_snapshot`, `lanes_named`, `edges`, `content`), `HistoryFull`,
  `RetainedBytes` (`total`); `RowsPage` (`new`, `push(GraphRow, PagedCommit)`, `len`,
  `ids`, `lanes_named`) and `PagedCommit { parents, subject, author, author_time }`
  (`src/rows_page.rs`); the chunked stores (`src/chunked_store.rs`, crate-private);
  `LaidOutRows`/`LaidOutRow` — `row_edges` takes any `LaidOutRows` (a slice or `Vec` of
  `GraphRow`s, a `History`). `CommitSummary` is now `{ id, parent_count, summary,
  author_name, author_time }`: no parents, no email. `Lane` holds a `u32`
  (`Lane::new(usize)` saturates); `LaneChange` is 16 B. `HistoryRow` is no longer a
  struct with `content` and `graph`.
- `cairn-git`: `HistoryPage::rows` is a `RowsPage`; the walk reads a parent count, a
  subject, an author name and a date per row (`history.rs`'s private `Commit`).
- `cairn-ui`: `HistoryList::new(State<History>, ..)`; `RowRender { content, graph,
  selected, lanes }` (RR1 closed: only what the window reads); `CommitRow` reads
  `parent_count`.
- `cairn-app`: `View::rows` is a `State<History>`; `Update::Rows { rows: RowsPage, .. }`;
  `session::apply` appends through `Progress::appended`, which ends the scroll on a
  `HistoryFull` (no more pages asked, the failure stands until a reopen); `reload_if`
  replaces the history; `selection::loaded_row` is `History::position`.
- Tests: `crates/cairn-git/tests/slim_rows.rs`, `crates/cairn-ui/tests/drawn_rows.rs`,
  `crates/cairn-model/tests/history_allocations.rs`, the window's
  `the_cairn_checkouts_rows_draw_what_they_drew_before_rows_were_slimmed`; the reporter's
  `find` mode reports `History::retained` and asserts C16's 64 MiB (`CAIRN_C16_MIB`).
  C15's walk now reads parents from the details query.

Phase 05 (`docs/systems/history-graph.md`, "From every ref, labelled, with stash rows"):

- `cairn-model`: `RowContent::Stash(StashSummary)` and `RowId::Stash(Oid)` (the stash
  commit); `StashSummary { id, index, base, message, author_name, author_time }`
  (`as_commit`); `HistoryRow::labels() -> RowLabels` (`is_head`, `len`, `is_empty`, `iter`
  of `Label { name, kind: RefKind, current }`; `src/row_labels.rs`); `RowsPage::push_labelled`,
  `RowsPage::push_stash(GraphRow, PagedStash)`; `RetainedBytes::{labels, stashes}`;
  `LaneAssigner::push_stash(id, base)`. A kept row's last byte is its flags (stash, `HEAD`,
  labelled); labels and stashes are side stores found by row number. The test-only
  `NotACommit` variants are gone.
- `cairn-git`: `HistoryRequest::from_refs(&RefsSnapshot, limit)`,
  `HistoryRequest::with_stash_lookahead`, `HistorySession::commits_walked`; private
  `src/history/seeds.rs` (`RefSeeds`, `Decoration`, `resolve`) and `src/history/stream.rs`
  (`Stream`, `LOOKAHEAD`); the cursor carries the decoration. `HistoryPage::walked` counts
  rows laid out; `decoded` counts commits read.
- `cairn-app`: `selection::comparison_of(RowId::Stash(s)) = Comparison::Commit(s)`; `Pair`
  has `base_row` and `tip_row`; the window draws a stash's row with `StashSummary::as_commit`.
- `cairn-ui`: nothing yet — `RowRender` carries no labels; phase 07 reads
  `HistoryRow::labels` in `render_of` (or copies them into `RowRender`) to draw chips.
- Tests: `crates/cairn-git/tests/every_ref.rs` (C6, and the `#[ignore]`d C11 reporter
  `measures_the_first_page_from_every_ref`); the C15/C16 reporter's seed `snapshot`.

Phase 06 (`docs/systems/history-graph.md`, "Refresh"):

- `cairn-model`: `RefsSnapshot::walks_as`, `RefsSnapshot::matching -> Option<RefsMatched {
  refs, stashes }>`, `History::with_author_capacity`; `src/text_filter.rs` (`Folded`,
  shared with `ChangeSet::files_matching`).
- `cairn-git`: `Repository::ref_tips` removed (use `refs`).
- `cairn-ui`: `Action::Refresh` (F5 / ⌘R, `Scope::Window`).
- `cairn-app`: `QueryLane::{Refs, AheadBehind, Status, RefFilter}`; `Request::Refresh` (three
  lanes, `Request::lanes`, `MOST_LANES`), `Request::FilterRefs`; `Update::{Refs { snapshot,
  reopen }, AheadBehind, Status, RefreshFailed { what: Refreshed }, FilteredRefs}`; the fetch
  updates lost `refreshed`; `Retired::{history, refs, ahead_behind, status}`;
  `worker/history_lane.rs` (`HistoryLane`, replacing `Scroll`), `worker/refresh_lane.rs`
  (`cairn-refresh`, `RefreshJob`), `refresh.rs` (`on_focus_gained`), `refresh_state.rs`
  (`RefreshState`), `session::reopen_history`; startup submits `Refresh`, not `OpenHistory`.
- Tests: `worker/refresh_tests.rs` (`Refreshable`, exported for the window's tests), the
  session's and window's C10 tests, `history_lane`'s stale-tip test; the `#[ignore]`d RR2
  reporter `measures_layout_over_unmerged_refs` in `crates/cairn-git/tests/every_ref.rs`.

Phase 07 (`docs/systems/history-graph.md`, "Chips on a row" and "The title bar";
`docs/systems/diff.md`, "REFS" and "A stash's changes"):

- `cairn-model`: `RowLabels::find(name)`, `Label::short_name`; `RowsPage::push_labelled`
  sorts labels by name; `History::serial`.
- `cairn-git`: `ChangesRequest::stash(id)` (`Subject::Stash`); `reads::stash_changes`
  (`src/reads/stash_changes.rs`, `git stash show --raw`); `content::Trees` carries the
  stash's untracked commit, `Trees::for_file`; a stash commit filed twice is one row
  (`seeds::resolve`).
- `cairn-ui`: `ref_chips` (`Chip`, `ChipKind`, `row_chips`, `chips_of_row`, `min_width`,
  `chip_element`, `tint`, `TAG_INDIGO`, `CHIP_*`), `ref_glyphs` (`RefGlyph`, `GLYPH_SIZE`),
  `status_box` (`StatusBox`, `Tracking`, `head_text`, `counts_text`, `name_text`,
  `repository_name`, `current_branch`, `NO_COMMITS_YET`, `UPSTREAM_GONE`); `RowRender::{chips,
  head}`; `HistoryList::refs`; `CommitRow::{chips, head}`; `label_room`; `CommitTab::refs`
  with `Refs`, `REFS_CAPTION`; `graph_geometry::row_geometry` paints each place once.
- `cairn-app`: `Comparison::Stash`; `selection::comparison_of(RowId::Stash) =
  Comparison::Stash`; `row_finder.rs` (`RowFinder`); `detail_pane::refs_of`;
  `window::status_box`; `RefreshState::ahead_behind_of`.
- Guard: `the_porcelain_reads_are_the_three_named_queries` (was `..._two_...`),
  `STASH_READ_FILE`, `STASH_WRITING_SUBCOMMANDS`; `scripts/git-floor.sh` floors 82 and 117.
- Tests: `crates/cairn-ui/tests/ref_chips.rs`, `crates/cairn-ui/tests/drawn_lanes.rs` (the
  `#[ignore]`d reporter), `crates/cairn-git/tests/diff/stash.rs`.

## Validation status

| Phase | Status |
| --- | --- |
| 01 refs engine | done: C1, C2, C3 pass; C11 refs numbers in progress.md; QA adjudicated, confirmed findings fixed; full gate green |
| 02 status engine | done: C4, C5, C13 pass (host git, 2.30.9, 2.32.7); C11 status numbers in progress.md; QA adjudicated, confirmed findings fixed; full gate green |
| 03 compact rows | implemented: C15 passes (equivalence over the fixtures, the Cairn checkout and every ref of the bench; find 2.26 s against 2.50 s before); K = 64, derived at draw time; numbers in progress.md; QA adjudicated, confirmed findings fixed; full gate green |
| 04 slim rows | done: C16 passes (comparisons pinned at `4205d5d` over crafted fixtures and the Cairn checkout; 52.6 MiB retained for all of rust-lang/rust from every ref, capacity counted, against 64 MiB; no kept row owns a heap allocation); C15 still passes (equivalence on the bench, find 2.21 s); RR1 closed; numbers in progress.md; QA adjudicated, confirmed findings fixed; full gate green |
| 05 history from every ref | implemented: C6 passes (walked commits = `git rev-list --branches --remotes --tags HEAD` over whole walks, labels = `git log --decorate=full`, stash rows with and without `--include-untracked`, assigner lane and edge tests); C11 first page from every ref 7.3 ms (8.6 ms with the snapshot read) beside `HEAD`'s 7.7 ms, worst stash look-ahead 21.6 ms; C16 52.67 MiB from the snapshot; C15 equivalence holds; the app still walks from `HEAD`; QA adjudicated, confirmed findings fixed (the walk's open cancellable between tips: 103 ms first page at 50,000 tags, cancelled in 6.3 ms); full gate green |
| 06 worker and refresh | implemented: C10 passes through the real boundary (`a_refresh_reopens_for_a_stash_a_checkout_and_a_moved_ref_and_for_nothing_else`, the refresh tests) and headless with focus set (`focus_gained_after_a_ref_moved_reopens_the_history_keeping_the_chosen_row`); every pin checked against a named mutation (progress.md); RR2 measured; full gate green; QA pending |
| 07 labels and toolbar | implemented: C7 passes (headless: chips, compaction, clipping, ✓, bold `HEAD`, stash chip, REFS, a stash's list against `git stash show --name-status` unset and set on the host's git and both floors); the QA brief's cases pinned; every pin checked against a named mutation (progress.md); RR2's drawing half measured; full gate green; QA pending |
| 08 sidebar | not started |
| 09 local changes | not started |
| 10 QA | not started |
