# State — refs-and-status

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 09 done (Local Changes, read only; C12's window check landing refs, the sidebar and an 11,000-path status), QA adjudicated, confirmed findings fixed and the user's Fork decisions built; the packet's independent QA returned MERGE BAR: READY, its documentation drift and notes fixed and the user's four merge-bar decisions recorded and built and the user's end-room report fixed (2026-10-07, below); full gate green; phase 10 next.** Integration branch
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
- **Compact rows, no cap** (brainstorm L13): a row keeps its id, a parent count, text,
  lane and only the lane changes at it (slimmed from L13's parents by L14); the drawn edges are derived. A deep find
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
- For the user's end-of-packet batch (phase 08; not decided): folders open closed but for the
  current branch's, revealed when it becomes current; a detached `HEAD` row first in Branches;
  a gone upstream as Fork's warning icon alone; stash entries by message; the notices' wording
  and place; any scroll of the list supersedes a find; a find's page is 512 rows; a stash with
  no row is known only at the walk's end (an issue for an early stop at its base's date, filed
  at teardown); a ninth query lane, `QueryLane::Walk`. Detail in progress.md, phase 08.
- C11's first-page bar is written as 200 ms because history-graph's A7 has no
  number (L12); the user may revise it at the merge bar.
- For the user's end-of-packet batch (phase 09; not decided): the count is distinct paths (a
  staged deletion and an untracked file of one name once, R9.2's wording), where phase 08
  counted entries and Fork's is said to equal git status's entries; the view's diff takes the
  Changes tab's options, Entire File included; a refresh re-asks the path chosen, its last diff
  drawn meanwhile, and a path gone from the status chooses the first (a path the filter hides
  stays chosen, the Changes tab's rule). (QC3c and the status failure, once here, are decided
  below.)

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
- For the user's end-of-packet batch (phase 07; not decided): label order (current branch
  first, then local, remote-tracking, tags; Fork's tag position is OPEN 3, git puts tags
  before remotes); compaction by the configured upstream only, read from the refresh's
  snapshot; chips cut by building only what a lower bound of their widths fits; glyphs painted
  as paths, the generic remote a cloud; the title bar's counts as Fork prints them, `18↓1↑`
  with a zero left out (unspaced as the user decided at phase 08 QA's 4; R7.1 amended), where
  R7.1 wrote `↓n ↑m`; `HEAD detached at <short>`, `<name> (no
  commits yet)`, `upstream gone`; the repository named by the opened path's last component; no
  `*` for an unreadable index; the title bar drops the full path; the REFS row drawn in the
  row's lane colour and laid out for the window's width, a stash's REFS its `stash@{n}`; and
  painting each place a row's lines reach once (RR2's drawing half: 5,000 open lines painted
  175,000 strokes a viewport). Detail in progress.md, phase 07.
- Bench hygiene (phase 05): a `--shared` scratch clone's `git stash` freshened the bench
  pack's mtime through alternates, and a plain `git status` on the bench moved its `.git`
  directory's mtime; no content changed (progress.md). Later phases: scratch clones with
  no alternates, and `GIT_OPTIONAL_LOCKS=0` for any `git` run on the bench.

## Decided by the user (closed open questions)

- **The merge bar (2026-10-07), the user's four decisions:**
  1. Local Changes' "Showing N of M files" counts DISTINCT PATHS, matching the sidebar's
     Local Changes (N), not rows (was QC3c): built — the filter's pass counts N on the worker
     (`MatchedRows::paths`), M is `LocalChanges::paths`; pinned by
     `what_a_filter_leaves_is_counted_in_distinct_paths`,
     `the_filters_count_is_of_distinct_paths_a_path_in_both_lists_once` and
     `the_filters_count_is_the_sidebars_distinct_paths_a_path_in_both_lists_once`, each red
     under counting rows; `local-changes.md` says so.
  2. A failed status read stays the built `Error::GitFailed` for the whole view, the last
     lists kept under it (was phase 02 QA's open question; the partial-clone case among
     them). A named state for a partial-clone status failure is an issue to file (below).
  3. F5 stays Linux's Refresh chord (⌘R on macOS); the table's rule that a bare chord must
     be a function key is kept (`chords_are_distinct_and_every_bare_one_is_a_function_key`).
  4. A ref whose name git calls invalid (`refs/heads/bad..name`) stays skipped silently and
     uncounted in `RefsSnapshot::unreadable`, as gix skips it (was QA finding QC-F6, phase 01).
- Decided by the user (2026-10-07, phase 06 QA): **RR1** — the refs stay read on the
  repository thread (R11.2), so a page asked during a refresh waits behind one refs read;
  the cost is written in `docs/systems/history-graph.md`, "Refresh". **RR2** — status is
  coalesced, not superseded: R10.3 amended so a refresh leaves a running `git status` to
  finish (its answer drawn) and every refresh asked meanwhile is one follow-up; only a close
  ends one. Ahead/behind and refs still supersede lane by lane.
- Decided by the user (2026-10-07, phase 07): **Q1** — a stash's changes are asked of `git
  stash show --raw` itself, the third porcelain read (`reads/stash_changes.rs`), git reading
  `stash.showIncludeUntracked`; guard `the_porcelain_reads_are_the_three_named_queries`.
  **Q2 (phase 05 QA's QC4)** — a stash commit filed twice is one row, its newest entry's; the
  sidebar (phase 08) still lists every entry.
- Decided by the user (2026-10-07, phase 08 QA): **1** Fork's natural order, folders first
  (case ignored, numbers as numbers, every level and tags; `main` as `master` not done — the
  research does not say what it means); **2** a no-upstream branch its own glyph shape
  (`RefGlyph::LocalOnly`); **3** expansion not remembered across sessions, as built — the
  research line is ambiguous, an issue to be filed at teardown; **4** counts unspaced
  `18↓1↑` (sidebar and title bar), closed sections open while filtering, no caption for a
  section with no match, `origin/HEAD` listed whole.
- Decided by the user (2026-10-07, phase 09 QA): **1** Local Changes' badges are Fork's, told
  apart by shape — `+` for added and untracked, `M` for a type change as for a modification, a
  submodule glyph painted as a shape (`RefGlyph::Submodule`), Fork's triangle for a conflict;
  colour only reinforces; Local Changes only (the Commit tab keeps its letters). **2** A
  draggable splitter between Unstaged and Staged, not remembered across sessions. **3** Natural
  row order (`cairn_model::natural_order`, through `path_order`), untracked mixed in, the search
  for a chosen path in the same order. **4** The filter field is the only toolbar control; the
  eye, Hide Untracked Files and the layout menu are issues to file (below).

## Handed to phase 06 (done)

Every hand-off is done (progress.md, "phase 06"): `from_refs` wired (`HistoryLane`); the unborn
`HEAD` with no ref answers the empty, complete page, `no_walk` gone with `from_head`'s use; a
walk error arrives from the first page; the stale tip re-read and reopened once; RR2 measured;
`History::with_author_capacity` with its model test; the reopen's rows freed on a worker; the
fetch's `reload_if` comparison and `Repository::ref_tips` gone.

## Handed to phase 08 (done)

- Phase 06 QA's TC3: done — `the_sidebars_rows_are_answered_on_a_worker_and_a_newer_ask_supersedes_the_older`
  (red with the answer sent under no epoch) and `a_sidebar_ask_superseded_mid_pass_stops_and_sends_nothing`
  (red with `|| true`); the dropped pre-pass `is_current` is an equivalent mutant, since
  `keep_going` is asked before the first entry (progress.md, phase 08).

## Handed to phase 07 (done)

Every hand-off is done (progress.md, "phase 07"): the chips read `HistoryRow::labels` and the
refresh's snapshot, the title bar `refs().head`, `ahead_behind_of` and `status()` (whose
`expect(dead_code)` went; `ahead_behind()` keeps it for the sidebar, `failure` for the
sidebar and Local Changes); RR2's drawing half measured, and its cost removed by painting
each place once; chips stop being built at the column's edge (RR3); labels' order decided
(batched above); the stash row's R6.2 Commit and Changes built on the user's Q1, QC4 on Q2.

## Handed to phase 08 (from 07, done)

- The glyphs: the sidebar draws `RefGlyph` (with `Folder`, `Gone`, `Opened`, `Closed` added)
  and `counts_text`; Fork's sidebar draws icons, not chips, so `chip_element` is not used
  there.
- `RefreshState::ahead_behind()` is the sidebar's counts (now shared, `BranchCounts`), and
  `RefreshState::failure(Refreshed::Refs)` is said under the sidebar's filter; neither carries
  `expect(dead_code)` now.
- Every stash entry is listed, a duplicate commit's included; pressing one selects its row
  (found by its stash commit, whichever entry) or, with no row, shows `Comparison::Stash` with
  no row selected and says it is not in the graph.

## Handed to phase 09 (done)

- Local Changes' content: done — `LocalChangesPane` in the main region, the placeholder gone;
  `local_changes_count` now counts distinct paths (`LocalChanges::paths`, R9.2's letter);
  `RefreshState::failure(Refreshed::Status)` said in place of the lists, or over the lists kept
  (`a_status_that_could_not_be_read_is_said`).
- C12's `window_check`: done — it opens with a refresh, so the refs, the decorated history and
  the sidebar's rows (answered, not `created()` empty) land in its first phase, and a scratch
  clone's 11,000-path status lands in Local Changes (progress.md, phase 09).

## Handed to phase 10

- Issues to file at teardown (the `file-issue` skill; GitHub while the remote exists), each
  with its evidence in progress.md:
  - Local Changes' toolbar beyond its filter field (the user's phase 09 decision 4,
    2026-10-07) — Fork's eye (side-by-side quick look), Hide Untracked Files, and the layout
    menu (tree, list and combined list; Show Ignored Files) — each an issue; the tree view
    itself is #36 already.
  - Research whether Fork remembers the sidebar's expansion across sessions (the user's
    phase 08 decision 3: not remembered, as built; the research line is ambiguous).
  - An early stop for a stash whose base no ref reaches: known today only at the walk's end
    (phase 08 RR2), it could stop at its base's date.
  - A named state for a status read that fails in a partial clone (the user's merge-bar
    decision 2: `Error::GitFailed` for the whole view stays, as built), classified by the
    repository's state.
  - A faster ahead/behind on a large divergence (phase 01: about two to three times git's
    time — 3.02 s against 1.14 s at 169,679 ahead — since gix's hidden frontier reads far more
    than the divergence and no commit-graph is used so every read can be cancelled).
  - The flaky `diff::working_tree::*` tests ("changed the git directory"), pre-existing from
    #59: they pass on a rerun.
  - `row_finder` could check the history list's index hint first, for an O(1) find of the
    selected row before its pass over the loaded ids (phase 07 QA's RR2 note).
  - A tree with an untracked nested repository is read twice on every refresh (phase 02 QA's
    QC-F3, the accepted cost of the two-read untracked scheme): whether to cache that the
    second read is needed.
  - The refs read stays on the repository thread (the user's phase 06 QA RR1), so a history
    page asked during a refresh waits behind one refs read (measured in
    `docs/systems/history-graph.md`, "Refresh"): revisit if that wait grows on a repository
    with many refs.
  - `RowId` is not in the every-variant guard's roster (merge-bar note N8): a partial read of
    it (`if let RowId::Commit(..)`) compiles silently once a third kind exists.
  - `status.renameLimit` has no fixture (merge-bar note N9): the status read passes nothing
    that overrides it, but no test holds a rename past the limit against git's answer.
  - `window_check` records C12's frames but asserts no 16.7 ms bar (merge-bar note N7): a
    frame past it is read from the numbers, not failed.

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
- `cairn-app`: `Update::Opened { name }` and `View::repository` (phase 07 QA); `Comparison::Stash`; `selection::comparison_of(RowId::Stash) =
  Comparison::Stash`; `row_finder.rs` (`RowFinder`); `detail_pane::refs_of`;
  `window::status_box`; `RefreshState::ahead_behind_of`.
- Guard: `the_porcelain_reads_are_the_three_named_queries` (was `..._two_...`),
  `STASH_READ_FILE`, `STASH_WRITING_SUBCOMMANDS`, `STASH_SHOW_OPTIONS`, `STASH_SHOW_REQUIRED`;
  `scripts/git-floor.sh` floors 82 and 118.
- Tests: `crates/cairn-ui/tests/ref_chips.rs`, `crates/cairn-ui/tests/drawn_lanes.rs` (the
  `#[ignore]`d reporter), `crates/cairn-git/tests/diff/stash.rs`.

Phase 08 (`docs/systems/sidebar.md` is the as-built account):

- `cairn-model`: `src/sidebar_rows.rs` — `SidebarRow` (`Section`, `Folder { first, depth,
  open }`, `Ref { index, depth }`, `DetachedHead`, `Stash { index }`), `SidebarSection`
  (`ALL`), `Disclosure` (`is_section_open`, `is_folder_open`, `toggle_section`,
  `toggle_folder`, `reveal`), `RefsSnapshot::sidebar_rows`, `folder_name`, `folder_path`;
  `RowId::oid`; `History::labelled_position` (and a kept `HEAD` row).
- `cairn-ui`: `src/sidebar.rs` — `Sidebar`, `SidebarRefs`, `SidebarTarget` (`of`),
  `MainView`, `BranchCounts`, `DrawnRow`, `drawn_row`, `local_changes_text`,
  `section_caption`, `SIDEBAR_ROW_HEIGHT`, `SIDEBAR_INDENT`, the captions; `RefGlyph::{Folder,
  Gone, Opened, Closed, LocalOnly}`; `HistoryList::cursor`; `counts_text` unspaced.
  `cairn-model` also: `natural_order`.
- `cairn-app`: `QueryLane::Walk`; `Request::{FindRow { target, rows }, StopFinding}`,
  `Request::FilterRefs { refs, text, disclosure }`, `Update::FilteredRefs { refs, text, rows
  }`; `Retired::sidebar`; `routing::Page::{Open { rows, walk }, Find, Stop}`,
  `Routed::OpenHistory`; `HistoryLane::{find_page, replace_walk}`, `history_lane::Finding`;
  `pool::sidebar_rows`; `serve`'s find step between jobs; `View::sidebar` (`SidebarView`:
  `state`, `filter_text`, `main`, `width`, `finding`); `sidebar_state.rs` (`SidebarState`,
  `SIDEBAR_WIDTH`), `sidebar_pane.rs` (`SidebarPane`, `local_changes_count`), `ref_find.rs`
  (`press`, `pages_arrived`, `superseded`, `row_chosen`, `reopened`, `failed`, `Find`,
  `FIND_PAGE_ROWS`, the notices); `Progress::stopped_finding`; `window::beside`,
  `LOCAL_CHANGES_PLACEHOLDER`; `RefreshState::ahead_behind` now `Option<&BranchCounts>`;
  `View::history_cursor` (the list's hint, set by `ref_find`'s `bring_into_view` and
  `follow_parent`).
- Tests: `crates/cairn-ui/tests/sidebar.rs`, `crates/cairn-app/src/sidebar_tests.rs`,
  `crates/cairn-app/src/worker/find_tests.rs` (with the `#[ignore]`d
  `measures_a_find_through_the_boundary`); `crates/cairn-app/src/worker/written_repository.rs`
  (`WrittenRepository::linear`: a line of commits written object by object with `std::fs`,
  SHA-1 and stored-deflate zlib spelled out, for worker tests that need a history of known
  length without this checkout's or a `git` run); window tests run in a window as wide again as
  the sidebar (`LEFT`).

Phase 09 (`docs/systems/local-changes.md` is the as-built account):

- `cairn-model`: `src/local_changes.rs` — `LocalChanges` (`new`, `status`, `into_status`,
  `paths`, `len`, `is_empty`, `get`, `row_of`, `matching`), `ChangeList` (`ALL`),
  `ChangeKind`, `PathState`, `LocalChange { path, from, kind, state }`, `MatchedRows`.
- `cairn-ui`: `src/local_changes.rs` — `LocalChangesList` (`split`), `ChangeBadge`,
  `change_badge`, `change_text`, `list_caption`, `UNSTAGED_CAPTION`, `STAGED_CAPTION`,
  `NO_PATH_MATCHES`, `LIST_HEADER_HEIGHT`, `LISTS_SPLIT`; `RefGlyph::Submodule`.
- `cairn-model` (QA): `path_order`; the lists sorted and searched in it.
- `cairn-app` (QA): `LocalChangesView::lists_split`; `local_changes_pane::follow` and `Follow`
  (the decision `follow_the_lists` applies); `window_check::scratch_refusal`.
- `cairn-app`: `QueryLane::LocalChangesFilter`; `Request::FilterLocalChanges { changes, text }`,
  `Update::FilteredLocalChanges { changes, text, rows }`, `Update::Status { changes:
  Arc<LocalChanges> }`, `Retired::status(Arc<LocalChanges>)`; `pool::filter_local_changes`;
  `RefreshState::local_changes`; `local_changes_state.rs` (`LocalChangesView`,
  `LocalChangesState`, `drawn_changes`, `shown_rows`), `local_changes_pane.rs`
  (`LocalChangesPane`, `follow_the_lists`, the notices), `diff_state/working.rs`
  (`WorkingChoice`, `WorkingShown`, `DiffState::{choose_working, refresh_working,
  let_go_of_working, working_choice, working_query, working_shown, shown_working,
  working_needs_asking, reask_working, working_arrived, load_working_anyway}`,
  `Asking::Working`, `answered_working`); `diff_actions::{working_query, choose_working,
  load_working_anyway}`; `View::local`; `changes_tab::share_of` crate-visible;
  `window::LOCAL_CHANGES_PLACEHOLDER` gone; `WorkingSide` exported outside tests.
- Tests: `crates/cairn-ui/tests/local_changes.rs`, `crates/cairn-app/src/local_changes_tests.rs`,
  the state tests in `diff_state/working.rs` and `local_changes_state.rs`, two boundary tests in
  `worker/refresh_tests.rs`; `window_check` takes `CAIRN_SCRATCH_REPO`.

Merge bar (`progress.md`, "merge bar"):

- `cairn-model`: `MatchedRows::paths` (the distinct paths a filter left).
- `cairn-ui`: `LocalChangesList::shown_paths`; `src/end_room.rs` (`END_ROOM`,
  `SCROLLBAR_THICKNESS`, `with_end_room`), counted after the last row of the Commit tab's
  list, the diff view, the Changes list and Local Changes' lists.
- `cairn-app`: `local_changes_state::shown_paths`.

## Validation status

| Phase | Status |
| --- | --- |
| 01 refs engine | done: C1, C2, C3 pass; C11 refs numbers in progress.md; QA adjudicated, confirmed findings fixed; full gate green |
| 02 status engine | done: C4, C5, C13 pass (host git, 2.30.9, 2.32.7); C11 status numbers in progress.md; QA adjudicated, confirmed findings fixed; full gate green |
| 03 compact rows | implemented: C15 passes (equivalence over the fixtures, the Cairn checkout and every ref of the bench; find 2.26 s against 2.50 s before); K = 64, derived at draw time; numbers in progress.md; QA adjudicated, confirmed findings fixed; full gate green |
| 04 slim rows | done: C16 passes (comparisons pinned at `4205d5d` over crafted fixtures and the Cairn checkout; 52.6 MiB retained for all of rust-lang/rust from every ref, capacity counted, against 64 MiB; no kept row owns a heap allocation); C15 still passes (equivalence on the bench, find 2.21 s); RR1 closed; numbers in progress.md; QA adjudicated, confirmed findings fixed; full gate green |
| 05 history from every ref | implemented: C6 passes (walked commits = `git rev-list --branches --remotes --tags HEAD` over whole walks, labels = `git log --decorate=full`, stash rows with and without `--include-untracked`, assigner lane and edge tests); C11 first page from every ref 7.3 ms (8.6 ms with the snapshot read) beside `HEAD`'s 7.7 ms, worst stash look-ahead 21.6 ms; C16 52.67 MiB from the snapshot; C15 equivalence holds; the app still walks from `HEAD`; QA adjudicated, confirmed findings fixed (the walk's open cancellable between tips: 103 ms first page at 50,000 tags, cancelled in 6.3 ms); full gate green |
| 06 worker and refresh | done: C10 passes through the real boundary (`a_refresh_reopens_for_a_stash_a_checkout_and_a_moved_ref_and_for_nothing_else`, the refresh tests) and headless with focus set (`focus_gained_after_a_ref_moved_reopens_the_history_keeping_the_chosen_row`); every pin checked against a named mutation (progress.md); RR2 measured; QA adjudicated, confirmed findings fixed (TC1's fetch test drains the whole refresh; status coalesced, R10.3 amended, the user's RR2; RR1 kept by the user; TC3 handed to phase 08, done); full gate green |
| 07 labels and toolbar | done: C7 passes (headless: chips, compaction, clipping, ✓, bold `HEAD`, stash chip, REFS, a stash's list against `git stash show --name-status` off and on, on the host's git and both floors); the QA brief's cases pinned; every pin checked against a named mutation (progress.md); RR2's drawing half measured; QA adjudicated, confirmed findings fixed (the title bar names the repository's folder, `Update::Opened`; the stash read's options guarded); full gate green |
| 08 sidebar | done: C8 passes (headless and through the real boundary; the twins `a_sidebar_of_50000_refs_builds_one_viewport`, `a_folder_of_10000_branches_open_builds_one_viewport`); the QA brief's cases pinned; every pin checked against a named mutation (progress.md); a find of the bench's oldest commit 2.23-2.25 s and 52.67 MiB retained through the boundary, cancelled halfway with no row after the stop; QA adjudicated, confirmed findings fixed (TC1's cancel pinned on lines of commits written for the tests, a stop alone, the list's hint the window's), the user's Fork decisions built; full gate green |
| 09 local changes | done: C9 passes (headless and through the real boundary; the twin `a_status_of_50000_paths_builds_one_viewport_filtered_or_not`); the QA brief's cases pinned; every pin checked against a named mutation (progress.md); C12 measured (every frame under 16.7 ms; an 11,000-path status on a scratch clone, the bench untouched); C11 complete in progress.md; QA adjudicated, confirmed findings fixed (the follow decision pure and pinned, the scratch guard following linked worktrees, badge shapes read pixel for pixel, the filter's mid-pass cancel), the user's Fork decisions built; full gate green |
| 10 QA | not started |
