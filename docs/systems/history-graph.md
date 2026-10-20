# History graph

How Cairn draws a repository's history today. As-built: everything here is code
that exists. Behaviour is pinned by a test named beside it; the paragraphs that
report a MEASUREMENT say so in their own words, because a measurement is not a
test — `measures_layout_over_every_ref_of_a_named_repository` and
`measures_compact_rows_over_a_named_repository` are `#[ignore]`d reporters, and
"Known limits" is description rather than anything pinned. Intent for this
surface lives in `docs/design/history-graph.md` (D4) and
`docs/design/concurrency.md` (D3), and the commitment it was built against in
`docs/prd/history-graph.md` (shipped, frozen).

What the application does today, end to end: it opens the repository containing
the path named on its command line (or the working directory), walks that
history newest-first on a worker thread, lays each commit into a lane, and draws
the result as a virtualized list of rows with lanes, edges and four columns,
paging as the reader scrolls. Nothing here mutates a repository, and there is no
repository picker.

**The history is walked from every ref, and reopened when a refresh finds them moved.**
`HistoryRequest::from_refs` seeds a walk from a refs snapshot — every local branch,
remote-tracking ref and tag that identifies a commit, and `HEAD` — labels each row
with the refs pointing at its commit, and draws each stash whose base the walk
reaches as a row of its own ("From every ref, labelled, with stash rows" below).
The window opens its history from the snapshot its first refresh reads on the repository
thread, walked on a history thread of its own, and reopens it whenever a later refresh — on
focus, after a fetch, on the Refresh action — finds what the graph draws changed, the old rows
drawn and usable until the new walk's first page replaces them ("Refresh" and "The opening
pass in the window", under the worker boundary below). Each row draws its refs as Fork's chips between the graph and the
subject, a stash's row its `stash@{n}` chip, and `HEAD`'s subject bold ("Chips on a
row", under the view below); the toolbar's status box names the repository over its branch,
and Push's and Pull's badges how far that branch is from its upstream ("The status box", there
too).

## The path a commit takes

One commit crosses four crates and changes shape three times.

1. **The walk hands over an entry.** `Repository::history_session`
   (`crates/cairn-git/src/history/session.rs`) opens one walk — in git's date order, the
   default, Cairn's own port of git's topological walk over gitoxide's commit reads
   (`crates/cairn-git/src/history/date_order.rs`); in commit-time and graph order a gitoxide
   revision walk — and keeps it for the life of a scroll. Each step yields an id and its parent
   ids, read from the walk rather than from a decoded object — the parents git
   shows, which in a shallow clone are not always the ones the object names
   (see "A shallow clone" below). In git's date order a refs snapshot's stashes are
   tips of the walk, each stepped as a row of its own; in the other orders
   `crates/cairn-git/src/history/stream.rs` puts each directly above its base.
2. **`LaneAssigner` turns ids into a picture.**
   `crates/cairn-model/src/lane_assignment.rs` takes `(id, parent_ids)` in walk
   order and produces a `GraphRow`: the commit's `Lane` and only the lane changes
   at it, every 64th row with a snapshot of the lines crossing into it ("What a row
   keeps" below). It is pure — no `gix`, no I/O, no clock.
3. **The session pairs the picture with a commit.** Only once the assigner has
   *evicted* a row (so nothing later can repaint it) does the session read that
   commit's object, for its subject, its author's name, its author date and how
   many parents the walk read for it — or, for a stash's row, takes what the walk
   read of the stash when it opened. A page is a `RowsPage`
   (`crates/cairn-model/src/rows_page.rs`): its rows, its own text, the authors it
   names (each once), its lane changes and its snapshots, its rows' labels and its
   stashes, in flat vectors.
4. **The worker hands a page across, and the window appends it.**
   `crates/cairn-app/src/worker/` sends `Update::Rows { rows, complete }` to the
   window, which appends it to its `History` (`session::apply`, where pages were
   applied before); `crates/cairn-ui/src/history/list.rs` draws the rows the
   viewport shows, reading each through the history and deriving its edges as it
   builds it.

What the list draws arrives as the engine read it: the subject, the author's name
and the author timestamp, the id, and whether the commit has more than one parent;
the lane index arrives as the assigner emitted it, and the segments
`graph_geometry` draws are exactly the ones the assigner drew for that row,
derived ("What a row keeps"). A row keeps nothing else: a commit's parents and its
author's address are the details query's (`Repository::commit_details`), which is
where the Commit tab reads them.

One thing a line CAN lose is its upper half. A backward line — a parent drawn
above its child — is repainted onto rows the assigner still holds, so a skew
span that crosses a page boundary on the cold path, or the window boundary in a
session, reaches the view as the lower half only, still flagged `out_of_order`.
`HistoryPage::rows`' own doc comment states it and
`a_backward_line_across_a_page_boundary_is_a_gained_segment`
(`crates/cairn-git/tests/history.rs`) pins it. **A view must therefore not
assume a flagged segment has a visible other end** — which is why the dash is a
property of each segment rather than of a line the view reconstructs.

## The lane assigner (`cairn-model`)

The vocabulary is `Lane`, `EdgeKind` (`Passing` / `IntoCommit` / `OutOfCommit`),
`EdgeSegment`, `LaneChange`, `LaneSnapshot`, `GraphRow` and `RowEdges`. It is
geometric only: nothing in it names genealogy, so a view draws a row without
knowing what the line means.

**Lane indices are final; edge segments inside the window are not.** Once a row
has been emitted its lane never changes. Segments may be repainted for rows the
assigner still holds, because a parent that arrives *after* its child — the
normal consequence of committer-date skew — has to be able to draw the line that
connects it. Pinned by
`lane_indices_never_change_when_more_commits_are_assigned`
(`crates/cairn-model/tests/lane_assignment.rs`), whose edge half derives the
rows entitled to repaint from the walk order rather than from the flag the code
under test set.

**Arrival order cannot break it.** A commit whose lane was never reserved is
placed, not rejected, and never mis-parented:
`skewed_history_places_every_commit_and_draws_every_parent_edge`, plus a
generated corpus in `generated_skewed_histories_are_all_placed_and_all_connected`.
A segment on such a line carries `out_of_order`.

**It holds a bounded window.** `LaneAssigner::DEFAULT_WINDOW` is 1024 rows;
`push` returns the row the window pushed out, which is final. Past the window it
still recognises ids for a further 16 per row of window. `LaneAssigner`'s public
`remembered()` reports the total; the 16 is the private `REMEMBERED_PER_ROW` in
`crates/cairn-model/src/lane_assignment.rs`. Recognising a parent that has
already gone by is what stops a lane being reserved for a commit that can never
arrive. Widening the window is not linear in cost: as measured when the window
was chosen, 66 ms at 1024, 749 ms at 4096 and 23.3 s at 16384.

**The blind spot, carried deliberately.** Beyond `window + remembered` rows of
skew the assigner cannot tell a parent already gone from one still to come. It
is not closable from gitoxide's walk, which keeps its seen-set behind a boxed
iterator with no accessor. **How often it fires is not measured**, and no
harness here measures it: the only coverage is
`a_parent_beyond_the_remembered_ids_is_placed_anyway_and_costs_one_lane`
(`crates/cairn-model/src/lane_assignment.rs`), a unit test on a synthetic
history that characterises how it degrades — every commit still placed, walk
order kept, one leaked lane reservation per event.

What IS measured on real repositories is the opposite case, the one the window
catches: rows carrying an `out_of_order` segment, i.e. a late parent the
assigner still held and repainted a line to. In git's date order, the history's, no
parent ever arrives before a child, so there is none: on rust-lang/rust from every ref,
0 of 345,545 rows carry one, and the frozen assigner's comparison counts 0 rows repainted
and 0 late lines (`measures_layout_over_every_ref_of_a_named_repository`,
`measures_compact_rows_over_a_named_repository`; `docs/research/history-topo-order/measured.md`).
In commit-time order, which the same reporter still walks, the skew there makes it 49,609
rows (14.4%, 64,698 segments); in `GraphOrder` it is most rows, by construction. That rate
bounds the blind spot from above without measuring it, since a parent must arrive late at
all before it can arrive too late.

**A stash's row is drawn on its base's lane** (`LaneAssigner::push_stash`), as Fork for
Windows draws it (`docs/research/ui-review/fork-window-survey-2026-10-16.md`, §4, uv7: the
stash's dot on `main`'s lane with the line running down to `main`, no lane of its own). The
stash commit is pushed as the row's id with its base — the commit it was made on, its first
parent — as its only parent: its index and untracked commits are never awaited. Where a line
already descends to the base, the stash's node is put on that line, in its lane — the lowest
such lane — which ends at the node and starts again below it (`Ends` and `Starts` in one lane),
still waiting for the base, so several stashes on one commit sit on one line and only that line
ends at the base. Where no line awaits the base, the stash takes the lowest free lane, as a
commit whose one parent is the base would, and its line descends in that lane to the base,
which takes it. A commit's line instead joins a line already descending to its parent, from a
lane of its own. The assigner is still keyed by `Oid` — the stash commit, which no commit the
walk reaches names as a parent. A base already laid out above is joined by a late line, as any
parent is, though the stream never places a stash below its base. Pinned by
`a_stash_draws_one_line_down_to_the_commit_it_was_made_on`,
`a_stash_is_drawn_on_its_base_commits_lane` (two stashes on one line, beside a commit that
takes the next lane), `a_stash_whose_base_no_line_awaits_takes_a_new_lane` and
`rows_with_stashes_derive_the_edges_the_assigner_kept`
(`crates/cairn-model/src/lane_assignment.rs`), each deriving every row's edges and requiring
them equal to the edges the assigner kept in its window, and by the drawn list's
`a_stash_is_drawn_on_its_base_commits_lane` (`crates/cairn-ui/tests/drawn_lanes.rs`), which
reads the stash's node back from the paint in its base's lane and nothing in the lane beside
it. C15's oracle knows commits alone, so its comparisons run over walks with no stash, and the
stash's lane leaves what it compares unchanged.

**What a row keeps.** A `GraphRow` holds its commit's id, its lane and only the
lane changes at it — never the lines passing through it. A `LaneChange` is a
line that ends at the row's commit (`Ends`), one that leaves it down a lane to a
parent below (`Starts`, opening the lane or joining one already descending to that
parent), or an out-of-order line leaving it for a child laid out below that names
it as a parent (`StartsLate`, with the rows to that child and its rank among the
child's late parents). Every `LaneAssigner::SNAPSHOT_EVERY`th row (64) also holds a
`LaneSnapshot` — a bitset of the lanes carrying a line into it, and the out-of-order
lines crossing into it — and so does the first row a reader keeps
(`LaneAssigner::drawn_from`, which a session and a cold page set to the rows they
replayed and dropped), so every page sequence draws without the rows before it.
The assigner still keeps every edge of the rows inside its window, because a late
parent's line is placed in a lane free across those rows; a row's changes and
snapshot are made final when it leaves the window, from the lines crossing into it
as of then, so a repaint that reached it is in them.

**A kept row is slim** (PRD R4.7). The window keeps its rows in a `History`
(`crates/cairn-model/src/history.rs`), and a kept row is 72 bytes of plain data:
its commit's id once, a parent count (saturating at `u16::MAX`: only whether it is
more than one is drawn), its subject as a span of the history's text store, its
author as a number in the history's author table, its author date, its lane, the
span of its lane changes in the history's lane-change store, and — on a row with
one — the number of its lane snapshot in the history's snapshot store. The row
type is `Copy`, which a type owning a `String`, a `Vec` or a `Box` cannot be, so
no kept row owns a heap allocation (a compile-time assertion beside it). Every
store the rows read grows in fixed chunks, never by doubling
(`crates/cairn-model/src/chunked_store.rs`):
a chunk is allocated once at its full size and never moves, a run of text or of
lane changes never splits across two, a run at least a chunk long gets one of its
own length, and the list of chunks grows by one. Rows sit 1,024 to a chunk, text
in 64 KiB chunks, lane changes 4,096 to a chunk. The author table names each
author once across every page, found by a hash of the name with names that hash
alike chained, so a page that brings only known authors adds none. That index is
the one part of a history that is not chunked: a standard hash map from a
fixed-key SipHash of the name to the first author filed under it, which doubles as
it fills and rehashes every author so far when it does — on the UI thread, inside
the append that crosses the threshold, bounded by distinct authors rather than rows
(0.48 ms measured at 57,000; rust-lang/rust has 8,424). `History::retained`
estimates its table from hashbrown's layout. A page is
copied into the stores where pages were applied before (`History::append`), and
a row is read through the history (`History::row`, a `HistoryRow` view): its id,
lane and changes without copying anything, its content — what the list draws —
copied out of the stores, and its edges derived. `History::retained` counts what
the stores hold, by capacity: every chunk whole, the last one's unused room
included. A history past its stores' 32-bit addresses (about 4 GiB of subjects)
answers `HistoryFull` and keeps the rows of the page before the first it could not
hold; the window ends the scroll there and says so (`Progress::appended`) until a
reopen.

**A row's kind, labels and stash sit beside it.** A kept row's last byte is its
flags: whether it is a stash's (`RowId::Stash`, `RowContent::Stash`), whether its
commit is `HEAD`'s, and whether refs point at it. What a labelled row carries — each
ref's full name in the text store, its kind and whether it is the branch `HEAD` is
on — is a run of a label store of its own, and what a stash's row keeps beyond a
commit's (`stash@{n}` and its base; its message is the row's subject) is an entry of
a stash store; each store holds one entry per such row, filed in row order and
found by a binary search on the row's number, the last entry filed under a number
read (one a row that could not be held left is never read: the row held next under
that number filed its own after it). A row with neither costs nothing for them, and
both stores grow in fixed chunks. A row's labels are kept bytewise by full name — the
snapshot's own order, into which `RowsPage::push_labelled` sorts any other — so
`RowLabels::find` finds one by name with a binary search
(`a_rows_labels_are_kept_by_name_and_each_is_found_by_its_own`). A row is read as
`HistoryRow::labels` (a `RowLabels`, iterating `Label`s, each with its
`Label::short_name`) and `HistoryRow::content`. Pinned by
`each_row_reads_back_its_own_labels_and_no_others`,
`a_stash_row_reads_back_as_its_stash_and_is_found_by_its_identity`,
`a_row_held_after_one_that_could_not_be_reads_its_own_labels_and_stash` and
`labels_and_stashes_are_retained_by_whole_chunks_and_only_when_held`
(`crates/cairn-model/src/history.rs`), with `history_allocations.rs` appending
labelled rows and stashes per chunk and reading labels with no allocation. All of
rust-lang/rust's labels from its snapshot hold 0.006 MiB.

`row_edges(rows, index)` (`crates/cairn-model/src/edge_derivation.rs`) derives
what a row draws: it finds the nearest row at or above `index` that carries a
snapshot, at most `LaneAssigner::MAX_SNAPSHOT_EVERY` rows up, advances it through
the changes of the rows between, and returns the row's lane and every edge
crossing it in the order the assigner drew them — the lines passing by lane, those
ending at the node by lane, those leaving it in parent order, then every
out-of-order line by the row it ends at and its rank there. It answers `None` past
the end or with no snapshot within reach, which never happens to rows a
`LaneAssigner` laid out and a reader kept from the first it was handed. The work is
one snapshot copied and at most `K - 1` rows of changes, bounded by the interval,
never by the history. It reads rows through `LaidOutRows`, which a slice of
`GraphRow`s and a `History` both implement, so the same derivation draws the
assigner's own output and the window's kept rows.

The derived edges are the edges the assigner drew before rows were compacted, row
for row and in order, repaints included. The oracle is that assigner itself, kept
verbatim as test code (`crates/cairn-model/tests/support/layout_before_compaction.rs`,
never edited, its code pinned by a fingerprint in
`the_reference_assigner_is_the_one_compact_rows_were_checked_against`): `every_row_draws_the_edges_the_assigner_retained_before_compaction`
(`crates/cairn-model/tests/lane_assignment.rs`) over the crafted fixtures and
generated skewed histories at windows of one row up and snapshot intervals of one
up, requiring repaints, a snapshot carrying a late line, a late line started below
a snapshot and a row derived from the furthest a snapshot can be;
`the_cairn_checkouts_rows_draw_what_the_assigner_drew_before_compaction` and
`each_cold_page_and_resumed_session_draws_on_its_own_what_the_assigner_drew_for_it`
(`crates/cairn-git/tests/compact_rows.rs`) over the walk a session or a cold page
really paged; and `rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew`
(`crates/cairn-ui/tests/history_list.rs`) over every row the list builds, scrolled
deep and back. The reporter `measures_compact_rows_over_a_named_repository` runs the
same comparison over a whole named repository.

**The snapshot interval was chosen by measurement.** On rust-lang/rust, a row
`K - 1` below its snapshot — the worst a row can be — derived in 0.76 µs at the
median, 2.0 µs at p99 and 14.1 µs at worst with K = 64 (about 106 edges a row),
against 8.3 µs, 24 µs and 37 µs at 1024; snapshots hold 0.31 MiB for the whole
history at 64, 19.8 MiB at one. A window's 35 rows cost well under a millisecond of a
16.7 ms frame, so the list derives on the UI thread as it builds each row rather
than on a worker per viewport. 64 is also the application's page, so every page
from the first starts on a snapshot. Measured by the reporter's `derive` mode
(release build, warm, every row `K - 1` below its snapshot over the whole history
from every ref, on the machine in `docs/research/diff-engine/measured-baseline.md`).

**Measured shape of a real repository.** The harness
`measures_layout_over_every_ref_of_a_named_repository`
(`crates/cairn-git/src/history.rs`, `#[ignore]`d, driven by `CAIRN_BENCH_REPO`)
runs the real query over every ref. Across seven repositories in the default
order: p99 of 8 edge segments per row on the widest of them, 264 B per row at
p99 on that same widest one, and single-digit lane counts throughout — figures from
when a row kept its edges, computed from their length rather than their capacity.
Those seven repositories had at most 2,896 commits; on rust-lang/rust in
commit-time order 105-160 edge segments stay open per row, and a row that kept them
cost 4.3-6.3 KB, 89% of it edge segments
(`docs/research/refs-and-status/deep-find-measured.md`). With compact rows the same
history's lane changes and snapshots came to 18.6 MiB, about 56 B a row, and a whole
retained row to about 509 B; with slim rows a whole retained row, with the text and
lane changes it reads, is about 159 B (Known limits).
Evidence and method for the seven:
`docs/research/history-graph/scroll-memory-model.md` Part D. Earlier, much
larger figures in this packet's history described a synthetic fixture and not
any repository; they are retracted there.

## The query (`cairn-git`)

`gix` types never appear in a public signature. Two routes exist, and they
answer identically row for row (`a_session_returns_what_the_cursor_path_returns_row_for_row`):

- **`Repository::history(&HistoryRequest, &impl Cancel) -> Result<HistoryPage, Error>`** — the
  cold path. Bounded by a limit, resumable from an opaque `HistoryCursor` that
  replays from pinned tips. Correct, and O(page index x limit), so it is the
  restart route rather than the scroll route. **Tips are resolved once per walk,
  not per page:** the first page turns `HEAD` or the given commits into object
  ids, and the cursor carries that shared set on, so a resumed page neither
  re-resolves nor copies it and a ref that moves between pages does not move the
  walk (`moving_a_ref_between_cold_pages_does_not_move_the_walk`,
  `resuming_reuses_the_cursors_resolved_tips`).
- **`Repository::history_session(&HistoryRequest) -> Result<HistorySession<'_>, Error>`** — the
  scroll path. Holds the walk open, so `next_page(limit, &cancel)` is O(limit)
  after a one-off prime of `window` commits. Pinned by
  `paging_a_session_costs_the_page_and_not_the_pages_before_it`. It borrows the
  repository and is not `Send`, so it lives on the worker that owns that handle.

**SHA-1 and SHA-256 repositories both read.** `Oid` holds either width, and gix
is built with its `sha256` feature, so a repository made with
`git init --object-format=sha256` walks, pages and resumes like any other
(`a_sha256_repository_reads_like_any_other`, against a real SHA-256 fixture —
which also fails if the feature is dropped). A starting point of the other width
is `Error::Walk`, refused before it reaches gix, which asserts on a mismatched id
rather than failing (`bad_starting_points_are_errors_rather_than_empty_pages`).

**A merely-walked commit is never decoded.** `HistoryPage.decoded` never exceeds
the rows returned. Made observable rather than asserted: the fixture writes a
commit-graph and deletes the loose objects of the commits a resumed page
replays, so any stray decode fails to find its object
(`a_replayed_prefix_is_walked_but_never_decoded`, `priming_the_window_walks_but_never_decodes`).

**Cancellation is a poll, once per commit** — once per row laid out and, while a walk
opens, once per tip and per commit the date-order walk's opening pass reads parents for
(below), or, in commit-time order, once per tip read but the first. In commit-time order
gitoxide reads every tip for its committer date
before the walk's first step, in one call that polls nothing (`Simple::sorting`); so
`walk::open` (`crates/cairn-git/src/history/walk.rs`) reads the tips in a loop of its
own, polling between them, and answers gitoxide's read of each from the dates it read
(`Seeded`), costing no second object read. A session opens its walk on its first page,
under that page's cancel; a cancelled open keeps nothing, and the next page opens it
again (`opening_a_walk_from_many_refs_is_cancelled_between_tips`,
`a_page_cancelled_while_or_after_the_walk_opens_resumes_to_the_same_rows`). `Cancel` is a trait so a test can
stop a walk at a chosen commit; `cairn-git` still knows nothing about threads. A
cancelled cold query reports how far it got and discards the page; a cancelled
*session* keeps its progress, so the next call continues rather than re-walks
(`a_cancelled_query_stops_walking_and_says_where`,
`cancelling_a_session_stops_the_walk_and_keeps_its_progress`).

**A shallow clone, a graft and a replace ref are walked as git walks them.** A
boundary commit — one the clone's `shallow` file lists — names parents in its object
that the clone does not have, and git reads the file as grafts: the commit has no
parents, so `git log --format=%P` prints nothing for it and `--graph` draws it as a
root, while a parent the clone DOES have, reached through another branch, is still
shown. gix's own `rev_walk` handles the file otherwise: it hands the boundary commit
over with the parents its object names, and skips the next appearance of each of
those ids whichever commit names it — so a merge-shaped history cloned with
`--depth` loses a commit its sibling branch reaches, and the lane assigner holds a
lane open for a parent that never arrives. git also reads `<common dir>/info/grafts`,
each line giving a commit the parents git shows in place of its own, which gix does not
read at all; and `refs/replace/<id>`, reading the object the ref names wherever `<id>`
is read, which gix 0.87.1 reads with `core.useReplaceRefs` the wrong way round — off
when unset or true — so it applies none by default. Every walk therefore goes through
`crates/cairn-git/src/history/walk.rs`, reading each commit through a `Grafted` object
source over a handle of its own, told to apply no replacement of gix's: an object a
replace ref replaces reads as its replacement, followed as git follows it
(`crates/cairn-git/src/replacements.rs`), and a grafted commit — shallow, or from the
graft file, the shallow file winning for one commit as in git — reads with its `parent`
header lines replaced by the graft's, found by the commit's own id
(`crates/cairn-git/src/grafts.rs`). Each row's subject, author and date are read through the
same substitutions, by a `Grafted` reader the walk's stream holds beside the walk, opened from
the same read (`history::open_stream`), so a replaced commit's row is its replacement's as `git
log` shows it; and `Repository::commit_details` reads through one too (`docs/systems/diff.md`,
"What a commit's details carry"). git
reads no commit-graph where any of these applies (`commit_graph_compatible`: a replace
ref, a graft, a shallow repository), since the graph holds each commit's own parents and
generations computed over them; nor does any graph read of Cairn's — the walk's, Show
Lost Commits' tip dates and the amend's pushed check (`walk::commit_graph`). Each is read
once per walk. With no parents a boundary commit is a root to the assigner, so its line
ends there and no lane stays reserved below it. Pinned against `git log --format='%H %P'`
at four depths of one history — a lone tip, a merge cut at the boundary, two boundaries
on two branches, and a boundary whose parent the clone has through a sibling — on both
routes, with the graph required to be the one git's parents lay out
(`a_shallow_clones_boundary_commits_have_the_parents_git_log_shows`,
`crates/cairn-git/tests/history.rs`); against `git log --date-order` with a replace ref,
a graft, a shallow file and a shallow clone over a history with a commit-graph, each laid
out so that the graph's parents would give another order
(`the_commit_graph_is_used_only_where_git_uses_it`, `crates/cairn-git/tests/date_order.rs`,
with and without a graph written, for the refs, for a stash on `main` — a tip pending above
the substituted history, in the graph with its real parents — and for Show Lost Commits'
reflog ids, and with `core.useReplaceRefs` off, and the shallow file beating a graft for one
commit; its oracle grafts the stashes through a copy's graft file, which that setting does
not switch off); the
commit-time and graph-order routes, Show Lost Commits' reflog dates, ahead and behind, and
a commit's details against git under each (`every_route_sees_the_substitutions_git_sees`);
the rows' subjects, authors and dates against `git log`
(`a_substituted_commit_is_described_and_compared_as_git_shows_it`); a replace ref whose
replacement is missing failing the walk — and a detached `HEAD` no ref reaches, replaced by
an object that is not there, failing it too rather than being left out, while a replace ref
naming nothing fails only the reads that meet its object
(`a_replacement_that_is_not_there_fails_only_what_meets_it`) —
as git dies (`a_replacement_that_is_not_there_fails_the_walk`);
the pushed check's graph stepping aside under each
(`the_graph_does_not_answer_where_git_would_not_read_it`); the header rewrite by
`only_the_headers_parent_lines_are_removed` and `a_grafts_parents_replace_the_commits_own`;
the graft file's reading by `a_graft_file_is_read_as_git_reads_it`; and a replacement's
depth by `a_replacement_is_followed_as_far_as_git_follows_it`. A replace ref added after the
repository was opened is read, since each walk reads the refs afresh; and `core.useReplaceRefs`
is read as the configuration says when the walk opens, the history thread opening its handle
again first when a configuration file moved (`crates/cairn-app/src/worker/configured.rs`).
Whether a refresh reopens the history compares the substitutions too (`RefsSnapshot::walks_as`,
"Refresh" below).

Order is `HistoryOrder::DateOrder` by default: git's date order (below), the order
`git log --date-order` and Fork's graph draw, in which no commit comes before any of its
children. A request may ask for `HistoryOrder::CommitTime`, which Recent Commit Messages
keeps for `git log -n 10`'s order, or `HistoryOrder::GraphOrder`; neither is topological,
which is why the assigner stays total over arrival order. Show Lost Commits walks in date
order whatever is asked (below). Of those two, walking *and laying out* 50k commits takes
129 ms in commit-time order against 196 ms in graph order, which leaves far more lanes
open per row; walking alone is the other way round
(`measures_both_orders_against_a_named_repository`, `#[ignore]`d).

### git's date order

`HistoryOrder::DateOrder` (`crates/cairn-git/src/history/date_order.rs`) is
`git log --date-order`'s order: no commit before all of its children, otherwise newest
committer date first, every tie broken as git breaks it. It is git's incremental
topological walk from `revision.c` (`init_topo_walk`, `compute_indegrees_to_depth`,
`indegree_walk_step`, `next_topo_commit`, `expand_topo_walk`), each step citing git's
source at v2.30.9 and v2.56.0: the tips each once, in the order given, stably sorted by
date; an in-degree queue ordered by generation then date; and a topological queue ordered
by date. Both queues pop ties in the order they were put, as git's `prio_queue` does with
its insertion counter (`date_order/queue.rs`); gitoxide's own `PriorityQueue` and its
`Topo` walk break equal dates arbitrarily, which is why neither is used. A merge's parents
are queued in the commit's order, so at one date the first parent comes first. git's
explore walk is left out: it only settles what a negative tip hides, and a history walk
has none.

Each commit is read once — from the commit-graph where git would read one, with its
generation, else from its object through `Grafted`, generation infinite — and kept with
its in-degree in gitoxide's `IdMap` of `gix_revwalk::graph::Commit`s
(`date_order/commits.rs`), through a handle with no object cache, since nothing is read
twice. Without a usable commit-graph every generation is infinite and the opening pass
reads every commit reachable before the first is known, as git's own whole-list path
does; with one, only as deep as the lowest generation among the tips, then as deep as each
commit about to be emitted needs. The opening pass polls its cancel before each tip and
each commit it reads parents for, and returns nothing once cancelled; a later step that
must walk deeper polls the same way, and, cancelled, leaves the walk as it was, its
deepening done before anything is popped. How many commits the pass has read goes to a
listener at most once per 100 ms (`date_order/pace.rs`).

A stash is a tip whose one parent is its base, as `git replace --graft <stash> <stash>^1`
makes it for git; its own parents are never read. It is seeded only when the refs reach
its base, as the in-degree walk decides once it is down to the base's generation, and
dropped when they do not, or when the refs reach the stash's commit itself — a branch
built on it — which is then that commit's row with the parents it was made with, its index
commit among them, as the user's own `git log` shows it (the "a branch at a stash" fixture). It is decided
no sooner than the walk needs it — once the walk is that deep anyway, before the commit
it would pop ahead of, or before its base is counted down — holding meanwhile the place
among git's tips it was given, so a stash made on an old commit costs the first page
nothing where a commit-graph bounds the walk. With Show Lost Commits' reflog ids among the
tips, every stash is decided before they join. A ref's tip whose object is missing fails
the walk, as git dies on it ("bad object"); a tip that is no commit is dropped, and a reflog
id that cannot be read is skipped (`bad_starting_points_are_errors_rather_than_empty_pages`).
The history's routes drive this walk — the refs snapshot's, the cold cursor's, a held
session's and a find's pages (below); it is also exposed as `Repository::date_order` over
`DateOrderTips` (refs and `HEAD`, stashes, reflog ids) for the parity tests and the
reporters.

Pinned against git itself, through one oracle run in each fixture at test time —
`git log --date-order --format=%H --branches --remotes --tags HEAD --stdin`, each stash
whose base the refs reach grafted onto it in a copy and fed with the reflog ids on stdin —
on commit times tied across branches, merges, two stashes and a rebase in one second, the
ui-review recipe, stashes on two branches and on a deleted one, clock skew, and reflog
entries, each with and without a commit-graph, for the refs, with the stashes and with
the reflog ids (`the_walk_is_in_git_date_order`), and at both floor gits in `git-floor`;
the lazy stash decisions by `a_stash_is_decided_no_sooner_than_the_walk_needs_it`; the
ties and parents by `ties_pop_in_the_order_they_were_put` and
`parents_are_queued_in_gits_order`; cancellation and the count by
`an_opening_pass_stops_when_superseded`, `a_cancelled_step_leaves_the_walk_as_it_was` and
`an_opening_pass_reports_how_far_it_has_read` and, through the public open,
`the_public_walk_reports_its_count_every_100_ms`. On rust-lang/rust from every ref the whole
walk equals git's `--date-order` row for row; the opening pass costs a little under git's
own time without a commit-graph (1.19-1.20 s against 1.25-1.26 s) and a little over with
one (95 ms against 82-84 ms), keeping about 170 bytes per commit (`docs/research/history-topo-order/measured.md`,
`measures_the_opening_pass_against_a_named_repository`,
`measures_what_deciding_a_stashs_base_costs`).

The history itself is held to the same oracle on every route a window pages, over those
fixtures and the stashes at every depth, with and without a commit-graph, at the host's git
and both floors (`crates/cairn-git/tests/date_order.rs`): the refs snapshot's history held in
pages and read as one (`the_history_is_in_git_date_order`); each stash's row, its index and
its base, exactly where git puts it, none for a stash on an unreached base, a commit's row
for a stash a branch is built on (`stash_rows_are_where_git_puts_them`); Show Lost Commits'
rows and its lost marks, whatever order the request names (`lost_commits_are_in_git_date_order`);
and the cold cursor and a session resumed from one (`a_resumed_session_continues_git_date_order`).

### From every ref, labelled, with stash rows

`HistoryRequest::from_refs(&RefsSnapshot, limit)` (`crates/cairn-git/src/history/seeds.rs`)
reads the snapshot as plain data: its seeds are every commit a listed ref
identifies — a tag on a tree or a blob identifies none, so seeds nothing and labels
nothing — each once, and `HEAD`'s, a detached `HEAD` naming what is not a commit
seeding nothing; its labels are, per commit, the refs pointing at it in the
snapshot's order, the branch `HEAD` is on marked current; and it carries the stash
list. Neither `refs/stash` nor a stash's base seeds the refs' walk. When the request is
resolved, each stash's commit is read for its author, its author date and its committer
date — one read per stash, polling nothing — and a stash whose commit cannot
be read, dropped and pruned since the snapshot, has no row, the walk going on without it
(`a_stash_whose_commit_is_gone_since_the_snapshot_has_no_row`); and a stash commit
`git stash store` filed twice in the list is one row, its newest entry's — a commit is
one row in a graph (the user's decision, 2026-10-07; the sidebar lists every entry) —
(`a_stash_commit_filed_twice_is_one_row_its_newest_entrys`); the resolved labels and
stashes (`Decoration`) are shared by every page and cursor of the walk, as its tips are
— the labels by reference, never copied — so a cold page resumed from a cursor carries
the labels and stashes of the snapshot the walk began from.

Both routes lay out the same stream (`crates/cairn-git/src/history/stream.rs`): the
walk's entries, stash rows among them. In date order each stash is a tip of the walk itself,
after `HEAD`'s and in `stash@{n}` order, whose one parent is its base (above): the walk
seeds it only when the refs reach its base and not its commit, and yields its row where
`git log --date-order` puts it, so the stream hands that row on as it comes — a stash on a
deep base, on a deleted branch or dated before its base costs no look ahead and is placed
exactly. A stash commit the walk reaches through a ref is that commit's row and no stash's.
In commit-time and graph order, whose walks are given no stash, each stash goes directly
above its base, newest committed first and `stash@{0}` first among equal dates.
`HistoryPage::walked` counts rows laid out, stash rows among them; `decoded` counts
commits read, which a stash's row adds none to; cancellation is polled once per row and
while the walk opens; `HistorySession::commits_walked` counts the commits pulled off the
walk, stash rows aside.

Every expectation is git's, run in the same fixture at test time and over the whole
walk, paged small (`crates/cairn-git/tests/every_ref.rs`):
`the_commits_walked_from_every_ref_are_git_rev_lists` — the walk's commits are
`git rev-list --branches --remotes --tags HEAD`, each once, over a branch `HEAD`
cannot reach, a remote-tracking ref's and a tag's own commits, a tag on a tree, a
stash whose branch was deleted (no row, none of its commits), and stashes made with
and without `--include-untracked` whose internal commits are no rows unless a branch
reaches one; `every_rows_labels_are_what_git_log_decorates_it_with` — each row's
labels against `git log --decorate=full`, `refs/stash` aside, `origin/HEAD` and a
nested tag among them; `each_stash_on_a_walked_commit_is_one_row_with_one_line_to_it`
— each stash's index, base, message, author and date against `git stash list`, on its
base's lane — one line out of its node down that lane into its base, the line already
awaiting the base running through it — two stashes on one commit on one line, a stash dated older than its base still above it, and every row where git's date
order puts it; `a_walk_paged_any_way_draws_the_rows_one_page_does` (held and cold, pages of
one, two and five); `a_stash_on_a_deep_base_is_where_git_puts_it` (held and cold);
`a_detached_head_no_ref_reaches_is_walked_and_labelled`;
`a_stash_commit_a_branch_reaches_is_a_commits_row_and_no_stashs`;
`a_braided_history_from_every_ref_is_git_rev_lists` (order too, against
`--date-order`); `in_graph_order_a_stash_is_directly_above_its_base`;
`a_stash_commit_under_an_older_branch_commit_is_a_commits_row_and_no_stashs`;
`stash_rows_are_in_gits_date_order` (a tie between a stash and the branch's commit it
shares a second with going to the branch, given first);
`a_page_cancelled_while_or_after_the_walk_opens_resumes_to_the_same_rows`;
and `a_deleted_branchs_stash_pulls_nothing_ahead_and_has_no_row`. Each order comparison is
against the oracle above, through `tests/support/date_order_oracle.rs`, which grafts the
stashes from a graft file outside the repository (`GIT_GRAFT_FILE`).

**Measured** on rust-lang/rust at `c999cef531e`, which has no commit-graph file (release,
warm, median of seven, the machine in `docs/research/diff-engine/measured-baseline.md`;
`docs/research/history-topo-order/measured.md`): the first page from every ref pays git's own
opening pass, 1.19-1.20 s against git's `rev-list --date-order --max-count=64` at 1.25-1.26 s,
and 95 ms in a copy with a commit-graph against git's 82-84 ms; through the worker boundary
the history thread's whole open, refs snapshot to its first 64 rows, is 1,174-1,206 ms against
git's 1,259-1,270 ms, and 113-115 ms with a commit-graph against git's 87-88 ms; a page after
the opening pass costs 0.4 ms, and a find of the oldest commit 1,952 ms, every page appended to
one history (`measures_the_history_thread_against_a_named_repository`,
`measures_a_find_through_the_boundary`). The whole history opened through the boundary equals
git's `git log --date-order` row for row, with and without the commit-graph. In commit-time order
(`measures_the_first_page_from_every_ref`, `#[ignore]`d) the first page of 64 rows from
`HEAD` is 7.7 ms, from every ref 7.3 ms with the snapshot already read and 8.6 ms with its
read, each laying out 1,088 rows, and the same find 3,829 ms.

Opening a walk reads one commit per tip. On a synthetic repository of a 50,000-commit
chain with a tag on every commit (`git fast-import`), the first page from every ref
takes 103 ms with the snapshot already read (91 ms when gitoxide read the tips itself,
uncancellably), 123 ms with the snapshot's read from packed refs and 235 ms from 50,000
loose refs, against 2.6 ms from `HEAD`; a first page whose cancel is already set
answers cancelled in 6.3 ms.

## The worker boundary (`cairn-app`)

`crates/cairn-app/src/worker/` is the only place in the binary that may name
`cairn_git` or `gix`, and the only place that may wait. The partition is by
FILE, and it is a guard, not a convention — see below.

- **One open repository, one thread-local handle, taken once.**
  `cairn_git::SharedRepository` is gitoxide's `ThreadSafeRepository` with the
  paths cached, and `SharedRepository::to_worker()` is the `Repository` a worker
  runs on. Each thread that reads calls it **once, at the top of the thread** —
  `serve` on the repository thread, `serve_history` on the history thread,
  `serve_diffs` on the diff thread: a
  per-request conversion compiles and passes every test while rebuilding the
  object cache and the pack snapshot each time, which is why the call site
  carries a comment rather than being left to look obvious.
  `Repository::OBJECT_CACHE_BYTES` (4 MiB) is installed by that handle, and is
  measured on a commit-time walk of 50k commits: 178 ms without the cache and 116 ms
  with it, and more bought nothing. `a_shared_repository_can_cross_threads` pins
  `Send + Sync`, and is also the twin for gix's `parallel` feature staying on.
- `worker::open(path, &Discovery) -> Result<(RepositoryHandle, Updates, Replier), OpenError>`.
  **The worker opens the repository**, because discovering one reads the
  filesystem. A path outside a repository therefore arrives as an
  `Update::Failed` naming the path, not as a panic and not as an empty window
  (`opening_a_path_outside_a_repository_is_reported_and_names_the_path`). `git`
  is found once per application, not here: `main` starts a
  `worker::Discovery` before the window exists, which runs `git --version` on
  a `cairn-discovery` thread of its own, and before the worker looks for the
  repository at all it takes that answer — waiting for it, on the worker's
  thread, if it is not in yet. A missing or too-old `git` arrives the same way
  as a missing repository, naming the version Cairn needs, and nothing is
  served behind it (`a_missing_git_is_refused_naming_the_version_and_nothing_is_served`,
  `a_git_refused_at_discovery_is_refused_to_every_repository_that_asks`); one
  probe per application is `git_is_found_once_per_application_not_once_per_repository`.
- `RepositoryHandle::submit(Request) -> Option<Epoch>` returns immediately and
  holds no receiving end of anything. It numbers a query in its lane, then routes
  it (`worker/routing.rs`) over an unbounded channel straight to the thread that
  serves it, and returns the epoch; an operation is numbered in no lane and
  returns `None`.
- `Updates::next()` is an `async fn`: it `try_recv`s and otherwise parks on
  `worker/wake.rs`, a one-slot latch a worker sets. There is no timer and no
  async-runtime dependency.
- **Epochs are numbered per lane** (`worker/epoch.rs`, PRD R4.1, refs-and-status
  R11.1): `QueryLane` is history, walk, changes, file diff, file filter, refs,
  ahead/behind, status, ref filter or Local Changes' filter, and a new query supersedes the older ones in
  its own lane only — except that a changes query also supersedes the file-diff
  lane (`QueryLane::supersedes`), since a file of the commit that was selected is
  no file of the one that is now, and that nothing supersedes the status lane: a
  refresh sends its status under the lane's unmoved number, so only a close cancels
  one ("Refresh", below). So a scroll never cancels a diff, a selection
  never cancels a scroll, and an operation, numbered in no lane, supersedes
  nothing (`each_lane_supersedes_itself_and_a_changes_query_the_file_diff_too`;
  through the real boundary, `crates/cairn-app/src/worker/diff/tests.rs`). The
  history has two: its query lane, which an open, a scroll's page, a find in the
  sidebar and its stop are numbered in, and its walk lane, which only an open
  moves and every page of rows is answered under — so a page a superseded find had
  laid out still arrives, and a reopen's replaced walk's pages do not
  (`docs/systems/sidebar.md`, "Pressing an entry"). An open's opening pass and its first
  page run under the walk lane's number, not the query lane's: a find or its stop asked
  while a walk opens leaves the pass running and is served once the first page is out,
  and only the next open — a reopen — or a close stops it
  (`an_opening_pass_is_superseded_by_the_next_walk_alone`, on the lane;
  `an_open_runs_whatever_the_history_lane_asked_after_it`, the history thread started with its
  jobs already queued; `a_find_or_its_stop_never_supersedes_a_walk_still_opening`, through the
  boundary). A stop ends a running find within a page
  (`a_stop_ends_a_running_find_within_a_page`).
- **The epoch IS the cancel signal.** `Superseded` (`worker/epoch.rs`)
  implements `cairn_git::Cancel` as "is my epoch still current in its lane", so
  superseding a request stops its walk at the next commit rather than
  discarding a finished answer
  (`a_page_asked_under_a_superseded_number_walks_nothing_and_the_next_takes_the_walk_up`,
  on the lane itself, and `superseding_a_request_stops_the_walk_that_is_serving_it`, one
  queued page superseded mid-walk through the boundary — both over lines of commits written
  for them, `worker/written_repository.rs`),
  and ends a diff's `git` process group at the runner's next poll
  (`a_superseded_diff_kills_its_git`). An answer that finished anyway is dropped
  as it arrives, by `Updates::next`
  (`an_answer_superseded_in_its_lane_is_dropped_on_arrival`). Dropping
  `Updates` stops everything, and so does `Request::Close`, which stops the
  epochs as it is submitted: closing the window does not wait for a page or a
  diff.
- **A dead worker is announced, not waited for.** `Update::WorkerLost` is sent
  from `WorkerExit`'s `Drop`, and the job channel is closed BEFORE the waiting
  task is woken. That ordering is the point: reversed, a worker that panicked
  would leave `Updates::next` parked forever on a latch nobody will ever set
  again, and the window would sit on "Loading" for the life of the process.
  The `WorkerExit` tests in `crates/cairn-app/src/worker/pool.rs` cover it
  (`a_panicking_worker_says_so_instead_of_disappearing`,
  `a_panic_inside_a_real_worker_is_announced_by_the_pool_that_opened_it`,
  `a_panic_on_the_history_thread_is_announced`,
  `a_worker_that_exits_cleanly_raises_no_alarm`,
  `a_failed_open_ends_the_stream_rather_than_leaving_it_open`,
  `a_worker_ending_in_silence_still_wakes_the_waiting_task`). `Outbox` is not
  `Clone`, held there by `a_worker_thread_cannot_be_given_a_second_sender`, which
  fails to compile if `Clone` is derived. And every `Outbox` closes its sender
  and then wakes when it is DROPPED, from whatever thread and with or without a
  `WorkerExit` around it (`an_outbox_dropped_anywhere_ends_the_stream`): that
  is what makes "the last sender closing wakes the task" hold by construction
  rather than by each path remembering to. It did not always — the two extra
  outboxes `open` makes for the network-lane and acceptor threads were
  captured by the repository thread's closure, and on an early exit (no usable
  `git`, no repository at the path) were dropped after the `WorkerExit` had
  signalled, with no wake of their own; a task parked between the two never
  woke, which is issue #21's intermittent hang of
  `a_missing_git_is_refused_naming_the_version_and_nothing_is_served`. What the
  type still cannot decide, and stays a review obligation: `Sender<Envelope>`
  is `Clone` and `Outbox`'s fields are visible throughout `pool.rs`, so a bare
  sender copied out by hand compiles, and one held past every `Outbox` keeps
  the stream open with nothing coming. Every wait in the worker's tests is
  bounded (`network::tests::WAIT`, in `block_on` and `woken_by`), so a hang of
  that shape is a red test with a name.
- **An explicit routing table names the thread for each lane**
  (`worker/routing.rs`, PRD R4.2, refs-and-status R11.2): the history lane — opens,
  pages, finds and their stops — on `cairn-history`, which owns the live walk — it borrows
  that thread's handle across turns and is not `Send`, so it never moves
  (`worker/history/lane.rs`) — and runs nothing else, so an opening pass that reads the
  whole history holds up only the history; a refresh's refs (`worker/history/refs.rs`),
  the Changes tab's, the sidebar's and Local Changes' filters, the freeing of old answers
  and the operations' front door on `cairn-repository`; the changes and file-diff lanes,
  commits, comparisons and the working tree alike, on `cairn-diff`
  (`worker/diff/lane.rs`); and status and ahead/behind on `cairn-refresh`
  (`worker/refresh/lane.rs`), so neither a page nor a diff queues behind a slow status.
  `route` applies it to every request as it is submitted and hands each thread its own job
  type, so no thread forwards another's work
  (`every_query_is_served_on_the_thread_its_lane_is_routed_to`). A filter asked while an
  opening pass runs is answered before its first page
  (`a_filter_is_answered_while_the_history_opens`, asked once the pass has told its first
  count); on rust-lang/rust, asked a quarter of the way into the pass, in 0.09 ms against
  0.03-0.05 ms with nothing opening, and in the window drawn a frame after the keystroke either
  way (`measures_the_history_thread_against_a_named_repository`, `opening_pass_check`). The repository thread stops the epochs, tells every thread to stop
  and cancels a fetch, then joins the history thread as the repository closes, before it waits
  on any reap, so a close during an opening pass ends the pass at its next commit and the
  update stream in what a close with nothing running takes
  (`a_close_during_an_opening_pass_does_not_wait_for_it`; on rust-lang/rust 29 ms against
  38 ms idle).
  A second thread costs the first almost nothing: measured by
  `measures_concurrent_walks_against_a_named_repository` (`#[ignore]`d),
  concurrent walks scale 2.1x, 4.2x and 7.9x at 2, 4 and 8 threads with
  single-walk time flat. How the diff thread schedules, caches and retries is in
  `docs/systems/diff.md`, "In the application".
- **The boundary is shaped for a second consumer, and none of it is stubbed.** A
  request is answered by a STREAM of `Update`s rather than by one reply; a worker
  runs ordinary blocking code, so a job that must wait on a UI answer makes its
  own reply channel and blocks on it; and workers are pinned to a purpose rather
  than fed from an anonymous queue. Fetch (`docs/prd/credential-prompts.md`
  R4) is the second consumer, and landed as exactly that: its own thread, now
  the network lane (`worker/network/lane.rs`, `cairn-network`), its own
  `Update` variants, and requests that carry no epoch so a scroll and a fetch
  cannot supersede each other; the credential
  dialog is a third thread (`worker/askpass.rs`) blocking on the window's
  reply. How it honours `Invalidated::refs` is the refresh below — a fetch's
  every ending makes the window ask for one, which reopens the history if the
  fetch moved what it draws. The
  as-built description is `docs/systems/credentials.md`; the lane, the close
  and the command log are `docs/systems/git-processes.md`.

### Refresh

As-built for refs-and-status R10 and R11. `Request::Refresh` is three
reads. Its refs and ahead/behind are numbered as it is submitted, in the refs and
ahead/behind lanes and in no other, so it supersedes the refresh before it lane by lane
and never a page, a diff or a filter (`Request::lanes`;
`a_refresh_cancels_neither_a_page_being_walked_nor_a_diff_being_read`). Its refs go to
the repository thread, its status to the refresh thread; once the refs are read, the
repository thread hands them to the refresh thread for ahead/behind, under the epoch the
refresh was given. A refresh asked again at once — focus flapping — leaves one refs
answer and one count to be drawn, and one status or two
(`a_refresh_asked_twice_at_once_draws_one_answer_of_each`), and a status that hangs holds
up neither a page nor a diff (`a_slow_status_delays_neither_a_page_nor_a_diff`).

**Status is not superseded** (R10.3 as amended, the user's decision of 2026-10-07). A
refresh leaves a running `git status` to finish, and its answer is drawn; every refresh
asked while it ran becomes one follow-up status after it, however many there were — the
refresh thread takes up a status job and drops every other status job queued behind it,
since one read answers them all, serving the other jobs it took off the queue — the newest
refresh's ahead/behind — before that status starts, so a count waits behind at most the
status that was running when its refresh was asked (`serve_refreshes`). The status lane's number is never
moved by a refresh, so only a close cancels a status — by stopping every lane, and by
ending every `git` in the repository's registry, either of which ends it
(`a_refresh_leaves_a_running_status_to_finish_and_asks_one_more_after_it`,
`a_close_ends_a_running_status`). Ahead/behind waits behind a running status on the same
thread.

**What a refresh costs, as built.** The refs are read on the repository thread (R11.2,
kept by the user's decision of 2026-10-07), so a filter or a free asked while a refresh reads
them waits behind one refs read — about 28 ms for the snapshot of a 10,001-ref
fixture warm (`docs/research/refs-and-status/measured.md`, section 2), more cold — at most
once per trigger. A page never waits on it, being the history thread's, and a refresh never
waits on an open: asked while an opening pass runs, its refs are answered before the pass's
first page — reopening nothing when nothing moved, reopening when a ref did
(`a_refresh_supersedes_an_open_still_reading`); a refresh never cancels a page. A status, once
started, runs to its end whatever refreshes arrive, so a window switched faster than a
status takes still gets one, followed by at most one more; each status is a full stat of
the tree (736 ms on a stat-dirty rust-lang/rust), which a refresh asked while one runs
pays once more after it, not once per switch.

It is asked for on three occasions, and the UI thread only submits on each: the window
gaining focus (`crates/cairn-app/src/refresh/focus.rs`, a side effect on the toolkit's
`Platform::is_app_focused` that submits on a false-to-true change and on nothing else —
`gaining_focus_asks_for_a_refresh_and_nothing_else`); any fetch ending, finished,
cancelled or failed (`session::apply`;
`every_fetch_ending_asks_for_a_refresh_and_touches_no_row`); and the Refresh action,
F5 on Linux and ⌘R on macOS in the accelerator table
(`the_refresh_chord_asks_for_a_refresh`). The window's first refresh, as it opens, is
what opens the history: there is no walk yet to compare with. Nothing watches the
file system (R10.2).

**What reopens the history.** The repository thread and the history thread share the
snapshot the walk began from and the snapshot the last refresh read (`WalkedRefs`,
`worker/history/refs.rs`, behind a lock the history thread holds for a pointer's clone or
assignment and a refresh across its comparison, so an open between the two cannot make a
refresh answer for the wrong walk; the UI thread never takes it). A refresh compares
the refs it reads with the first by `RefsSnapshot::walks_as` — the same refs naming the same
objects, the same `HEAD`, the same stash list, and the same substitutions git walks under
(the replace refs it reads and the grafts, a shallow boundary among them;
`docs/systems/refs.md`) — and answers `Update::Refs { snapshot, reopen }`. The repository
thread reads the refs on a handle that follows the configuration, so `core.useReplaceRefs`
turned off or on reopens the history too (`crates/cairn-app/src/worker/configured.rs`). When
the substitutions differ from the snapshot the window kept, the window also asks the chosen
commit's changes again, since its details and what it changed may be other now
(`history::selection::ask_again`). The history thread names a snapshot the one the walk began from as it takes it to open,
so a refresh while that walk is still opening compares with what it will draw and reopens
nothing when nothing moved (`a_refresh_during_an_open_compares_with_the_refs_the_open_took`).
The window keeps the snapshot (`refresh/state.rs`, which the history's chips, the status box,
the sidebar and the Commit tab's REFS row read) and, on `reopen`, asks the history thread for
the new walk (`session::reopen_history`), whose number supersedes a walk still opening there,
its pass ending at its next commit. The old rows stay drawn and usable — selected, scrolled,
their details and diffs asked — and page no further, since their walk is gone
(`Progress::reopen`); the new walk's first page replaces them: the old `History` is moved out,
a new one is built with `History::with_author_capacity` of the old one's author count, the
page appended to it, and the old one handed to the repository thread to free as a
`Request::Retire` (#52, R11.3); the selection is left as it is, drawn again when its row
arrives (R10.5). A chosen commit that never arrives again — its branch deleted, nothing else
reaching it — stays chosen: no row is drawn chosen, and the detail pane keeps the
answer it holds for it until another row is chosen. A stash pushed, a checkout that moves no ref and a moved ref each reopen; a
refresh that finds nothing changed does not, and neither does an upstream's
configuration alone (`a_walk_is_the_same_unless_what_it_draws_changed`, in the model)
(`a_refresh_reopens_for_a_stash_a_checkout_and_a_moved_ref_and_for_nothing_else`,
counting the window's reopens through the real boundary, and
`a_refresh_reopens_for_what_git_substitutes_and_the_walk_reads_the_configuration_now`, for a
replace ref added and moved, `core.useReplaceRefs` off and on, the graft and shallow files, each
reopened history drawing the replaced commit's row as git reads it then;
`focus_gained_after_a_ref_moved_reopens_the_history_keeping_the_chosen_row`, headless
with focus set; `a_reopen_keeps_the_old_rows_until_the_new_page`).
The fetch's own before-and-after comparison of the refs, and `session::reload_if`
that acted on it, are gone.

**What an open walks from.** `OpenHistory` walks from the snapshot the last refresh
read, so a reopen reads the refs once. With none read — a test that opens without a
refresh, or after a failed open, which forgets them — the open reads its own and
answers it as `Update::Refs { reopen: false }` under the walk's number before its first
page. A snapshot with no ref and an unborn `HEAD` walks nothing and answers the empty,
complete page an unborn repository always answered
(`a_freshly_initialised_repository_reaches_the_view_as_an_empty_history`). A ref whose
commit has gone between the refresh and the open — deleted and pruned — fails the
walk's open; the open then reads the refs again and, when they draw something else,
answers them and opens once more from them
(`an_open_from_refs_gone_stale_reads_them_again_and_opens_from_those`). A walk error
arrives from the first page, since the session opens its walk there.

**What is freed where.** Every refresh answer the window replaces — a snapshot,
ahead/behind, a status — and every snapshot or count superseded before the window read
it goes to the repository thread to free (a status is never superseded: one is dropped
unread only when a close has stopped every lane, which frees it with the worker) (`a_replaced_refresh_answer_is_freed_on_a_worker_and_a_failure_keeps_the_last`,
`a_superseded_refresh_comes_back_to_be_freed_on_a_worker`). A refresh's failure is kept
beside the answer before it, which stays; a failure to read the refs before any were
read is the history's failure too, since it has nothing to walk from.

Every `submit` of a QUERY supersedes the queries before it in its lane (an
operation carries no epoch), and a superseded page delivers nothing — so the
caller must debounce. `Progress::wants_more()`
(`crates/cairn-app/src/history/state.rs`) is that debounce: it is false while a
page is in flight, while the history is complete, and once the update stream has
ended (no worker is left to answer).

**A failed page is retried on the next approach to the end.** A failure leaves
`wants_more()` true, so the next time a row near the end comes into view the
page is asked for again; the history lane dropped the live session on the error
but kept the cursor, so that request cold-restarts from the last good page. The
failure stays on screen — a banner under the rows — until a page arrives. It
cannot loop: `on_reach_end` fires on a row's visibility CHANGING, and a failure
changes nothing that is visible at the end of the list, so each retry needs the
reader to scroll. Pinned by
`a_failed_page_is_asked_for_again_on_the_next_approach_to_the_end`
(`crates/cairn-app/src/window.rs`), by
`a_failed_request_is_answered_when_it_is_asked_again`
(`crates/cairn-app/src/worker/pool.rs`) for the worker recovering, and by
`a_stream_that_has_ended_stops_asking_for_good` for the worker-gone case.

### The opening pass in the window

In git's date order a walk with no commit-graph git would use reads every commit the tips
reach before its first row ("git's date order", above): about 1.2 s on rust-lang/rust. The
history thread runs that pass and nothing else waits on it — a filter, a refresh's refs, a
free and a diff each run on another thread — and the window says it is reading.

- **The count is the worker's to pace.** The walk reports how many commits its pass has read
  at most once per `READING_EVERY` (100 ms, the interval `HistorySession::open_walk` is
  given); the history thread tells the window none of them until the pass has run
  `READING_AFTER` (250 ms), then each at most once per 100 ms (`ReadingPace`,
  `worker/history/reading.rs`; `the_count_is_told_once_the_pass_has_run_250_ms_then_at_most_every_100_ms`),
  as `Update::Reading { commits }` under the walk's number, so a superseded walk's count is
  never drawn (`the_window_is_told_how_far_an_opening_pass_has_read`). A cold restart — the walk
  dropped by a failed page and resumed from its cursor — runs its pass again under the walk's
  number but tells no count, since it replaces no rows
  (`a_cold_restart_tells_the_window_no_count`). The window keeps the
  number in `Progress` (`Progress::reading`) and keeps no timer: a pass shorter than 250 ms
  says nothing. On rust-lang/rust the first count is told at 304-309 ms, nine or ten times an
  open.
- **A first open** draws the opening sentence in the list's place, "Reading history…", and
  "Reading history… N commits" once a count is told (`status_text::placeholder`).
- **A reopen keeps the rows** (`Progress::reopen`, "What reopens the history" above): drawn
  and usable until the new first page replaces them, under a strip over the list reading
  "Reading history… N commits" once a count is told (`status_text::reading_strip`, drawn by
  `window.rs`; `the_history_says_it_is_reading_during_a_long_open`,
  `a_reopen_draws_the_old_rows_under_the_reading_strip`). Over no rows — a reopen of an empty
  history — the count takes the placeholder's place instead. A find or its stop asked
  meanwhile leaves the reopen waiting for its first page, never letting the old rows page
  (`a_reopen_keeps_the_rows_and_asks_for_none_of_them_until_its_first_page`); a failed reopen
  keeps the rows and is asked again as a failed page is.
- **Frames.** On rust-lang/rust no frame took over 16.7 ms during the first open (max 6.7 ms)
  or during a reopen with 3,000 rows drawn and scrolled every frame (max 2.7 ms), and a
  filter keystroke made during the pass was drawn 18 ms after it, as one made with nothing
  opening was (`opening_pass_check`, `docs/research/history-topo-order/measured.md`).

## The view (`cairn-ui`)

`HistoryList` takes the `History` as a `State` HANDLE and a per-row builder, so
nothing copies the history to draw it. It renders through
`VirtualScrollView::new_with_data_controlled`, so per-render work is bounded by
the viewport rather than by the history; the only per-render read proportional
to the history is its `len()`. The list is keyed by `RowId`, so a row arriving
above another neither moves the selection nor rebuilds the rows below it. Each
row it builds is read through the history there (`render_of` in `build_row`): its
content, copied out of the stores, and its edges, derived from at most a snapshot
interval of rows above it. The builder is handed those two and nothing else of the
row — `RowRender::content` and `RowRender::graph`, which `CommitRow` draws; a row
with no snapshot within reach, which the assigner never produces, would draw its
node alone. Reading a row is constant work per drawn row, bounded by the snapshot
interval and the row's own text, never by the history.

`RowContent` is read by matching, with no wildcard arm — `crates/cairn-app/src/window.rs`
is the consumer, and because the enum is not `#[non_exhaustive]` the next kind
of row is a compile error there rather than a row silently not drawn. The
spellings that would compile anyway once a second variant exists — a `_ =>` or
catch-all binding arm, `if let`, `while let`, `let .. else`, `matches!`, a glob
import — are rejected outside `cairn-model` by
`every_view_of_a_row_names_every_kind_of_row`
(`crates/cairn-guards/tests/invariants.rs`), whose matcher
`cairn_guards::reads_row_content_partially` carries its own self-test.

- **Columns** (Fork's four): graph and subject share the first, then author,
  abbreviated id, and `Date (UTC)`. The date column is labelled UTC because
  a kept row holds the author time's seconds and not its offset, and
  converting to the reader's local time needs a timezone database, which is a
  dependency decision nobody has taken. `utc_minutes`
  (`crates/cairn-ui/src/history/date_text.rs`, a private module) is Hinnant's
  `civil_from_days`, total over every `i64`.

  The column is also **not monotonic**, and that is not a defect: the walk is
  ordered by COMMITTER time and the column shows AUTHOR time, which is the same
  pairing `git log` itself uses and the same one every client surveyed shows. A
  rebased or cherry-picked commit therefore sits below a row with an earlier
  date. Recorded because nothing else records it.
- **Lanes are columns; colour is an aid.** `graph_geometry` is pure arithmetic —
  `ROW_HEIGHT` 26 (uniform), `LANE_WIDTH` 14, `MAX_DRAWN_LANES` 24, beyond which
  lanes share the last column. Lane separation is tested against the INK
  (`2 * NODE_RADIUS`, `STROKE_WIDTH`), not against a fraction of `LANE_WIDTH`,
  so the constant cannot decide its own test. `lane_palette` cycles eight hues —
  the seven non-black Okabe-Ito colours plus a neutral grey to bring it back to
  eight — pinned against Okabe-Ito's published values by
  `the_palette_is_the_okabe_ito_set_less_black_plus_a_neutral`.
- **An out-of-order line is dashed along its whole length**, in the lane's own
  colour, geometry unchanged — dash rather than colour because colour never
  carries meaning alone, and geometry unchanged because the connection is not a
  different kind of connection. All three `EdgeKind`s of such a line are marked,
  so the dash does not stop halfway *within* the rows the view was handed — but
  the line itself can be handed over truncated, per "The path a commit takes",
  and no view work recovers the missing half.
- **The dash and the hollow merge node are decided against real pixels.**
  `graph_cell`'s tests paint a row onto an offscreen Skia surface and read the
  pixels back — removing the dash or filling the ring each fails a test. Skia is
  already linked through `freya`'s `engine` feature; no test-only dependency was
  added.
- **Loading, empty and failed are three different sentences.** `placeholder`
  (`crates/cairn-app/src/history/status_text.rs` — `cairn-app` is a binary, so its
  modules are cited by file, not by a `cairn_app::` path) decides which, so the
  reader never sees a blank area that could mean any of the three. A failure
  that arrives after rows are drawn is a banner under them, not a replacement
  for them. `Status::Empty` is reachable because the history lane maps
  `Error::UnbornHead` to `Update::Rows { rows: [], complete: true }` rather than
  to a failure — a repository with no commits is empty, not broken.
- **Keyboard**: arrow keys, `PageUp`/`PageDown`, `Home` and `End`, as a pure
  `moved_to(key, current, last)` (private, in
  `crates/cairn-ui/src/history/list.rs`). The list is auto-focused, so a reader with no
  mouse can select at all. `PREFETCH_ROWS` is 24: every row within that many of
  the end asks for the next page when it becomes visible — every one of them,
  because a single trigger row is a list that quietly stops loading on a tall
  window.

### Chips on a row

A row draws the refs pointing at its commit as Fork's chips between the graph and the
subject (refs-and-status R5; `crates/cairn-ui/src/history/ref_chips.rs`): a branch's and a
remote's outlined in the row's lane colour and filled with a dark shade of it
(`style::tones::chip_fill`), as Fork tints them by lane; a tag's and a stash's in Fork Dark's
tones for their kind (`style::tones::TAG_CHIP`, `STASH_CHIP` and their borders), the glyph's
cap unfilled and the text Fork's chip text; each kind told by its glyph and shape (R5.5): a local branch a plain chip, the current one with a check mark
before its name; a remote-tracking ref and a tag a glyph — a generic remote's cloud
(whatever the forge) and a tag — in a cap of their own at the chip's left; a
stash's row a `stash@{n}` chip with a box. The glyphs are painted as paths on a canvas
(`crates/cairn-ui/src/history/ref_glyphs.rs`), no font involved, every one in the chip's text
colour, so what tells two kinds apart is the shape alone
(`every_glyph_paints_a_shape_no_other_glyph_paints`, over the pixels); a chip is keyed by
its kind and name, since a canvas is repainted only when its layout changes. `HEAD`'s
row's subject is bold.

- **Order**: the current branch first — git's `HEAD -> main` leads too — then the row's
  refs in the snapshot's order, local branches, remote-tracking refs, tags, each by name:
  Fork's local before remote; where Fork puts tags is not recorded
  (`fork-refs-and-status-ui.md`, OPEN 3), and git puts them before the remote-tracking
  refs.
- **Compact labels** (R5.2, Fork's default): a local branch whose upstream is a
  remote-tracking ref labelling the same row draws that upstream as the remote glyph in a
  cap of its own chip, and the upstream draws no chip; any other remote's ref at that
  commit keeps its chip; an upstream that is a local branch (`remote = .`) is never
  folded; with no snapshot yet nothing is. Which ref is a branch's upstream is read from
  the refresh's snapshot (`HistoryList::refs`); that both label the row is read from the
  row (`RowLabels::find`).
- **Clipped, never counted** (R5.3): the chips and the subject share the subject's
  column, which clips them; many chips push the subject out, as in Fork. Chips stop being
  BUILT once the column's room is spent: the list measures its width (`on_sized`), the
  room is what the fixed columns and the graph leave (`label_room`), and each chip is
  counted by a lower bound of its width (`min_width`: its padding, its cap and check mark,
  and two pixels a character, under any letter's advance at the chip's size), so the
  column is always filled to its edge and at most a column's worth of lower bounds is
  built past it. Laying a row out reads the current branch's label and each upstream by
  a search, and stops at the first label past the room: a commit with 10,000 refs lays
  out a column's worth (`chips_stop_being_built_at_the_rooms_edge_however_many_refs_the_commit_has`;
  timed by `measures_drawing_a_viewport_over_many_open_lanes_and_a_crowded_rows_chips`).
- **Laid out once per row built**: `render_of` lays the chips out as the list builds the
  row (`RowRender::chips`, `RowRender::head`), and `CommitRow` only draws them, so a frame
  that does not build the row lays nothing out
  (`a_rows_chips_are_laid_out_once_per_row_built_and_never_per_frame`, counting layouts
  across frames that redraw the window but not the list, and across a scroll). A change
  of the list's width is a change of the room, so while the window is resized sideways
  every visible row is built again each frame — its edges derived and its chips laid out
  — bounded by the viewport's rows and the lanes crossing them, never by the history.

Pinned by `crates/cairn-ui/tests/ref_chips.rs` —
`a_branch_and_its_upstream_at_one_commit_are_one_chip_and_nothing_else_is_folded`,
`only_a_remote_tracking_upstream_is_folded_and_only_with_a_snapshot`,
`the_current_branch_leads_then_branches_remotes_and_tags_in_order`,
`chips_stop_being_built_at_the_rooms_edge_however_many_refs_the_commit_has`,
`each_kind_of_chip_draws_its_glyph_before_its_name_and_a_plain_branch_none`,
`the_head_rows_subject_is_bold_and_no_other`,
`twenty_refs_are_clipped_at_the_columns_edge_and_the_row_keeps_its_height`,
`the_room_a_chip_is_counted_by_never_exceeds_what_it_is_drawn_at`,
`the_room_is_the_subject_columns_width` — by the viewport twin with labelled rows,
`only_a_viewport_of_labelled_rows_is_built_and_each_lays_out_a_columns_worth_of_chips`
(`crates/cairn-ui/tests/history_list.rs`: 100,000 rows, forty of them labelled by 5,003
refs), and, through the window, by
`a_rows_chips_and_its_refs_row_are_drawn_against_the_refreshs_snapshot` and
`a_stash_row_draws_its_message_and_asks_what_it_changed_on_its_base`
(`crates/cairn-app/src/window.rs`). The Commit tab's REFS row draws the same chips
(`docs/systems/diff.md`, "REFS").

### The status box

The toolbar's status box (`cairn_ui::StatusBox`, `crates/cairn-ui/src/toolbar/status_box.rs`;
drawn in the toolbar, `docs/systems/window-frame.md`, "The toolbar") names the repository, bold —
its folder, as Fork names it: the last component of its working tree (of its git directory when
it is bare), which the worker sends once the repository is open (`Update::Opened`, kept in
`View::repository`), so a launch at `.`, `..`, a subdirectory or its `.git` names the repository
and not the path given (`an_open_names_the_repositorys_folder_whatever_path_it_was_opened_at`);
nothing until it is open — over a branch glyph and the current branch, and `upstream gone` after
it for a configured upstream no ref is (git's `[gone]`); a detached `HEAD` as `HEAD detached at
<short id>` and an unborn branch as `<name> (no commits yet)`, in git's words. No star and no
counts, as in Fork: the sidebar's "Local Changes (N)" carries the change count, and the branch's
distance from its upstream is two badges, the commits ahead on Push and those behind on Pull,
each absent at zero. The window builds it from what the last refresh answered
(`crate::toolbar::toolbar`): the snapshot's `HEAD` and the branch's upstream, the branch's counts
found by name (`RefreshState::ahead_behind_of`, a binary search, the counts coming in the
snapshot's order) — each as old as its answer. Pinned by
`the_status_box_names_the_repository_over_the_branch` and
`each_refresh_redraws_the_badges_on_push_and_pull` (`crates/cairn-app/src/window.rs`: a branch
with counts, a gone upstream drawing no badge, a detached and an unborn `HEAD`, badges redrawn
and gone at zero), `a_branchs_counts_are_found_by_its_name`
(`crates/cairn-app/src/refresh/state.rs`), `ahead_and_behind_are_badges_on_push_and_pull`
(`crates/cairn-ui/tests/toolbar.rs`) and the status box's own tests
(`crates/cairn-ui/tests/status_box.rs`).

### Show Lost Commits

`HistoryRequest::with_lost_commits` (staging-and-commit R11) widens a walk from refs: the
old and the new id of every entry of `HEAD`'s reflog and of each local branch the snapshot
lists join the tips, as `git rev-list --reflog` and `git fsck` read a reflog — so the commit
an amend replaced is found even where the only entry naming it is the amend's own, as its
old id, in a log the amend created. Each log is read whole (`gix`'s store reads the file
into a buffer) and parsed by git's own rules (`show_one_reflog_ent`):
`crates/cairn-git/src/history/reflogs.rs` never uses gix's newest-first reader, which stops
for good at a line over 4 KiB, nor gix's line parser, which refuses lines git reads; a line
git skips — a zero time, no `>` before the time, a time zone that is not a sign and four
digits, a last line with no newline — gives no id, and the lines after it are still read
(`every_line_git_reads_is_an_entry_and_no_other`). The null id names nothing; an id whose
object is gone or is not a commit is skipped, as git's walk skips it; the rest join the tips
after the refs' and the stashes', each once. The reflogs are read as the walk opens, under the first
page's cancel (polled before each log and each id's lookup), and a cursor carries the tips
read and which of them the refs named, so a cold page resumes the same walk.

Which commits no ref reaches is decided as the walk goes, never by a walk of its own before
the first page (`crates/cairn-git/src/history/reach.rs`, `Reach`). Show Lost Commits walks in
git's date order whatever order the request names, so no commit comes off the walk before a
child it walks: a commit is reached when a ref names it or a reached commit that came off
before it names it as a parent, and lost otherwise — a verdict final when it is given, its row
carried lost (`RowsPage::push_lost`) or not, however the commits are dated
(`a_parent_dated_after_its_reached_child_is_not_drawn_as_lost`). What is kept is the walk's
reached frontier and the parents of lost commits still to come; once every reflog tip not
known to be reached has come off the walk and no lost commit's parent is still to come,
nothing more can be lost, and the tracking stops. A stash's row is never lost: `refs/stash`
reaches it.

The expectations are git's (`crates/cairn-git/tests/lost_commits.rs`), over the whole walk,
held and cold, at pages of 1, 2, 5 and 64 and windows of 1, 3 and 1,024:
`show_lost_commits_draws_what_git_reads_from_every_reflog_entry_and_dims_what_no_ref_reaches`
— the rows are `git rev-list --reflog --branches --remotes --tags HEAD`, and the rows drawn
lost are `git rev-list <every id the log files hold> --not --branches --remotes --tags HEAD`
(the ids read from the files by the test, and again by git, `--reflog`), on a fixture with an
amend whose log it created (the replaced commit an old id only), an amend, a reset-away
commit, a subject past 4 KiB in the middle of the log, a deleted branch's tip, a commit only
a lost commit reaches and a tagged commit in the log, never lost; and
`where_every_reflog_entry_is_reached_the_toggle_draws_the_same_rows` — on a braided history
every reflog entry of which a ref reaches, every row's identity, lane and edges are the walk's
without the toggle, so the compact and slim rows' equivalences pinned on that walk hold with
it on.

**Measured** (`measures_the_first_page_with_show_lost_commits`, `#[ignore]`d; release, warm,
median of seven, a plain tmpfs clone of rust-lang/rust at `c999cef531e`, no commit-graph):
with the clone's own reflog and an amend and a reset-away commit made in it, the first page
of 64 rows from every ref takes 0.2-0.3 ms more with the toggle than without (three rows
lost; 1,088 commits pulled either way). The open reads one commit per distinct reflog id, as
git's own `--reflog` does: with 1,000 entries naming commits spread across the history,
7.95 ms against 2.72 ms without the toggle; with 10,000, 49.8 ms against 2.14 ms, of which
reading the logs and looking up each id is 8.4 ms and the walk's open, reading each tip's
date, 39 ms (git's `rev-list --max-count=64 --reflog --branches --remotes --tags HEAD` on
the same clone: 57-80 ms, against 3-5 ms without `--reflog`). The whole walk, 345,449 rows
with a deep reflog tip keeping the tracking on to the end: 2,184 ms against 2,079 ms.

Each reflog id is checked once to name a commit — the commit-graph answering where it holds
the id (`reflogs::reflog_tips`) — and the walk reads each tip itself, a ref's or a reflog's,
its date and parents the commit-graph's where the graph holds it, as git's own walk takes
them (`a_commit_graph_dates_the_tips_as_the_objects_do`). Measured in commit-time order,
which isolates the reflog reads, on the clone, back to back against a walk that read each tip
twice and dated it from its object ("before") — Show Lost Commits itself walks date order, so
without a commit-graph its first page also pays the opening pass ("The opening pass in the
window"):
with no graph, 10,000 entries 52.2 ms before and 50.8 ms after (the commits' reads from the
pack are the cost, not their number); with a graph written, 50.4 ms before and 8.8 ms after,
1,000 entries 7.8 ms and 2.8 ms; 200 local branches each with a 50-entry log (10,000 ids),
50.6 ms with no graph and 8.9 ms with one. Every reopen while the toggle is on — a refresh that
moves a ref — pays the same again, since each reads every log; in the window the toggle's first
page is drawn 18.2 ms after the press on the clone's own reflog (`writes_check`).

Turning it on or off is a reopen (`Request::OpenHistory`'s `lost`, the window's
`View::show_lost`, kept for the session and off as the window opens):
the history lane opens the walk as the open asked, and a later reopen — a refresh that finds the
refs moved — asks as the toggle stands
(`an_open_asked_as_show_lost_commits_walks_the_reflog_and_the_next_plain_open_does_not`,
`the_show_lost_commits_chord_reopens_the_history_with_the_toggle_flipped`). It is turned by its
chord, heard wherever focus is in the window as Fork's View menu hears it (`Scope::Window`;
the history list leaves it alone, `a_chord_pressed_in_the_list_moves_nothing`), by the View
menu's "Show Lost Commits (Reflog)", ticked while on (`docs/systems/window-frame.md`), and by a
check box, "Show Lost
Commits", at the right end of the "Graph and subject" heading's cell — laid over the cell, so
every heading stays over its column and no row loses width — ticked while on, its tooltip the
chord the accelerator table lists, spelled for the platform (`HistoryHeader::show_lost_commits`,
`accelerators::chord_name`; `show_lost_commits_is_a_check_box_at_the_end_of_the_first_headings_cell`,
`the_show_lost_commits_box_flips_the_toggle_as_its_chord_does`). The list
keeps the keyboard through the reopen: its old rows stay drawn until the reopened walk's first
page replaces them, so the chord is heard again with no click. A lost row is drawn as Fork draws one (Tracker #351's captures): its subject,
author, short id and date in Fork Dark's grey for a row `HEAD` cannot reach
(`style::tones::UNREACHABLE_TEXT`), and nothing else dimmed —
its graph's lanes, edges and node and any chips keep their own colours (`CommitRow::lost`;
`a_lost_rows_text_alone_is_dimmed`, `a_lost_row_is_drawn_lost_and_no_other`); a lost commit
that gains a ref is reached at the reopen that follows and drawn as any other.

### Create Branch

The way back to a lost commit — or to any — is Fork's Create Branch dialog
(`crates/cairn-ui/src/dialogs/create_branch.rs`, `crates/cairn-app/src/operations/create_branch.rs`; evidence
`docs/research/staging-and-commit/fork-create-branch-evidence.md` and
`fork-observed-2026-10-10.md`), opened by `New Branch…` in
any commit row's context menu, a lost row's too, a stash's row offering none
(`HistoryList::on_new_branch`, `every_commit_rows_menu_offers_new_branch_at_its_commit`) — the
right-click selecting its row first unless it lands in the selection already made, either of a
compared pair
(`a_right_click_selects_its_row_and_keeps_a_selection_it_lands_in`) — or at
`HEAD` by Fork's New Branch chord, Ctrl+Shift+B (⇧⌘B on macOS), heard anywhere
(`Action::NewBranch`, `create_branch::open_at_head`): `HEAD`'s commit as the last refresh read
it, its subject the row's where `History::labelled_position` finds it loaded, nothing opened
before the refs are read, on an unborn `HEAD`, or over a dialog already up
(`the_new_branch_chord_opens_create_branch_at_head`):

- Under the title, Fork's line "Use '/' as a path separator to create folders"; "Create branch
  at:" the commit's glyph, short id and subject, read only; "Branch name:", empty at each
  opening; "Check out after create", sticky for the session (issue
  #89 will keep it across restarts); Cancel and "Create", "Create and Checkout" while ticked
  (`the_dialog_is_forks_its_button_following_the_box`).
- Each change of the name asks the engine (`Request::CheckBranchName`, the branch-name lane on
  the local lane, `Repository::branch_name`): the button is enabled only for an answer of
  "free" to the text shown, and a refusal is said inside the dialog, in the buttons' row,
  left of them, behind the warning triangle (`RefGlyph::Gone`), never in the Git Error dialog
  — the engine's typed answer worded by the dialog
  (`cairn_ui::name_refusal`, `refusals_are_worded_by_the_view`): "Branch <name> already exists",
  git's reason for a name it does not take, git's words for a name a branch's directory holds
  (`'refs/heads/foo/bar' exists; cannot create 'refs/heads/foo'`, either way round), or "A
  branch name can't contain '@{'", said before git is asked
  (`a_name_holding_at_brace_is_refused_in_cairns_words`). While the check waits behind a write
  on the local lane, the same place names what it waits for by its one name
  (`create_branch::waiting`) — "Waiting for Commit 1 file…" — the running write's or else the
  first queued (`a_check_waiting_behind_a_write_is_said_beside_the_buttons`,
  `a_name_check_waiting_behind_a_write_says_which_write`)
  (`a_refusal_is_said_beside_the_buttons_behind_the_warning_glyph`,
  `a_name_clashing_with_a_branchs_directory_is_refused_before_git_runs`,
  `only_a_ready_name_is_created_and_every_change_is_reported`,
  `new_branch_opens_the_dialog_and_creates_only_a_name_the_engine_said_is_free`,
  `create_branchs_reads_are_answered_on_the_lane`).
- "Local changes:" is drawn while the box is ticked and a tracked path is staged, unstaged or
  conflicted (`WorkingTreeStatus::has_tracked_changes`, searched among every entry, never read
  from the first): Fork's three in Fork's order — "Don't change", "Stash and reapply" greyed with
  "Comes with stashing." beside it, since Cairn does not stash, and "Discard", Fork's ⚠
  beside it while it is chosen; each opening starts on "Don't change"
  (`check_out_after_create_is_sticky_and_local_changes_appear_only_over_changes`,
  `local_changes_are_offered_over_a_change_and_never_over_untracked_files_alone`,
  `local_changes_are_forks_three_in_forks_order_stash_greyed_and_a_chosen_discard_warned`).
  The button reads "Create and Checkout" whatever is chosen.
- **The dialog is a confirmation surface** (on the seal guard's `CONFIRMATION_SURFACES`): while Discard is chosen the engine is asked what it would
  be confirmed as for the name shown (`Request::CheckoutConsequence`, the checkout-count lane,
  asked again as the name changes, the newest ask's answer alone kept), and the button waits for
  it; then Create and Checkout — or Return — IS the acknowledgement, the radio chosen plus the
  press, as in Fork: the dialog builds the `Confirmed` from that `Consequence` and the window asks
  `LocalWrite::CreateBranchDiscarding` with it, no second dialog opening
  (`discards_press_builds_the_token_from_the_consequence_it_was_handed`,
  `discards_press_is_its_confirmation_and_builds_the_token_from_the_engines_consequence`). During
  an operation in progress the refusal row says "<Operation> is in progress. Finish or abort it
  first." (`cairn_ui::discard_refusal`), the button disabled while Discard is chosen
  (`an_operation_in_progress_refuses_discard_in_the_dialog`).
- The writes are the local lane's: `LocalWrite::CreateBranch`,
  `LocalWrite::CreateBranchAndCheckout` and `LocalWrite::CreateBranchDiscarding`. The dialog
  stays open, its button disabled, while its write is asked, and closes when it is done
  (`new_branch_opens_the_dialog_and_creates_only_a_name_the_engine_said_is_free`). Each reads
  everything again however it ends, so the new branch's chip is drawn and a commit it is put on
  is no longer lost
  (`a_branch_made_through_the_lane_reads_everything_again_and_a_refusal_says_gits_reason`).
- A failure — git's refusal, or a discard whose `HEAD`, commit or name moved since the press —
  opens Fork's Git Error dialog with its words over the dialog, which stays beneath it as it was
  left, so Close returns to it; Escape closes the Git Error alone (it is first in the window's
  tree, so its Escape is heard first, and one overlay above the dialog;
  `a_failed_create_opens_the_git_error_over_the_dialog_left_as_it_was`). While either is up the
  window's keys are inert (`shortcuts::keys_inert`;
  `the_windows_keys_are_inert_while_create_branch_is_up`). The engine's verbs are
  `docs/systems/staging.md`'s.

## What enforces this

| Rule | Twin |
| --- | --- |
| `cairn-ui` and `cairn-model` never name `gix` or `cairn_git` | `layers_never_name_the_crates_they_are_sealed_from` |
| Each crate depends only on its allowlist | `layer_dependencies_are_allowlisted` |
| A `RowContent` is read by naming every variant | `every_view_of_a_row_names_every_kind_of_row` |
| Only `crates/cairn-app/src/worker/` reaches a repository or waits | `the_ui_thread_never_waits_on_repository_work` |
| The history list renders through a virtualizing view | `a_history_sized_list_renders_through_a_virtualizing_view` |
| That view builds one viewport of rows at 1,000 and at 100,000 | `only_a_viewport_of_rows_is_built_however_long_the_history` |
| ... and with labelled rows, each laying out a column's worth of chips | `only_a_viewport_of_labelled_rows_is_built_and_each_lays_out_a_columns_worth_of_chips` |
| A row's chips are laid out once per row built, never per frame | `a_rows_chips_are_laid_out_once_per_row_built_and_never_per_frame` (`crates/cairn-ui/src/history/list.rs`) |
| Every row draws the edges the assigner drew before rows were compacted | `every_row_draws_the_edges_the_assigner_retained_before_compaction`, `the_cairn_checkouts_rows_draw_what_the_assigner_drew_before_compaction`, `each_cold_page_and_resumed_session_draws_on_its_own_what_the_assigner_drew_for_it`, `rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew` |
| Every row draws the subject, author, date, short id and merge marker it drew before rows were slimmed | `every_crafted_row_draws_what_it_drew_before_rows_were_slimmed`, `every_row_of_the_cairn_checkout_draws_what_it_drew_before_rows_were_slimmed`, `every_row_of_a_braided_history_draws_what_git_prints` (`crates/cairn-git/tests/slim_rows.rs`), `every_row_draws_what_it_drew_before_rows_were_slimmed` (`crates/cairn-ui/tests/drawn_rows.rs`), `the_cairn_checkouts_rows_draw_what_they_drew_before_rows_were_slimmed` and `the_windows_table_agrees_with_the_engines_table_captured_before` (`crates/cairn-app/src/window.rs`) |
| No kept row owns a heap allocation; a history's stores grow in chunks | the `Copy` assertion beside `StoredRow` in `crates/cairn-model/src/history.rs`, `a_kept_row_is_seventy_two_bytes` (`crates/cairn-model/src/history.rs`), `appending_ten_thousand_rows_allocates_per_chunk_never_per_row` and `reading_a_rows_identity_lane_and_changes_allocates_nothing` (`crates/cairn-model/tests/history_allocations.rs`), and the store tests in `crates/cairn-model/src/chunked_store.rs` |
| Each author is named once; a subject reads back exactly | `a_page_of_new_authors_and_a_page_of_known_ones_both_draw_and_each_is_named_once`, `authors_whose_keys_collide_are_told_apart_by_name`, `an_empty_a_non_ascii_and_a_very_long_subject_read_back_exactly` (`crates/cairn-model/src/history.rs`) |
| What a history retains counts every store, chunks whole | `what_a_history_retains_counts_every_chunk_whole`, `the_author_index_is_estimated_by_its_buckets` (`crates/cairn-model/src/history.rs`), `a_run_exactly_a_chunk_long_leaves_the_chunk_being_filled_alone` (`crates/cairn-model/src/chunked_store.rs`) |
| Show Lost Commits draws git's reflog walk, and dims exactly what no ref reaches | `show_lost_commits_draws_what_git_reads_from_every_reflog_entry_and_dims_what_no_ref_reaches`, `a_parent_dated_after_its_reached_child_is_not_drawn_as_lost`, `where_every_reflog_entry_is_reached_the_toggle_draws_the_same_rows` (`crates/cairn-git/tests/lost_commits.rs`), `every_line_git_reads_is_an_entry_and_no_other` (`crates/cairn-git/src/history/reflogs.rs`), `a_lost_row_reads_back_lost_and_no_later_page_changes_it` (`crates/cairn-model/src/history.rs`), `lost_commits_are_in_git_date_order` (`crates/cairn-git/tests/date_order.rs`) |
| The history from a refs snapshot, a resumed walk and its stash rows are in git's date order | `the_history_is_in_git_date_order`, `stash_rows_are_where_git_puts_them`, `a_resumed_session_continues_git_date_order` (`crates/cairn-git/tests/date_order.rs`), `stash_rows_are_in_gits_date_order`, `a_stash_on_a_deep_base_is_where_git_puts_it` (`crates/cairn-git/tests/every_ref.rs`) |
| A full history keeps the rows it held and ends the scroll | `a_page_past_the_historys_limit_keeps_the_rows_before_it_and_says_so` (`crates/cairn-model/src/history.rs`), `a_full_history_stops_asking_and_keeps_saying_so` (`crates/cairn-app/src/history/state.rs`) |

The first five live in `crates/cairn-guards/tests/invariants.rs`; the sixth is a
headless component test in `crates/cairn-ui/tests/history_list.rs`; the rest are
the model, engine, list and window tests named under "What a row keeps" and "Chips on
a row". The
slimmed rows' comparisons hold them to tables of what the rows drew before. The
engine's (`crates/cairn-git/tests/slim_rows.rs`) and the list's
(`crates/cairn-ui/tests/drawn_rows.rs`) were committed at `4205d5d`, the one before
rows were slimmed, and asserted there against the old rows, their full parents and
their author's address still on them. The window's was committed with the
slimming, `1b0ca9f`, from the same test run against a build of `4205d5d`, and
`the_windows_table_agrees_with_the_engines_table_captured_before` holds it to the
engine's. Each is checked beside a live oracle (`git log` and the details query). The four that scan
SOURCE each assert a nonzero scanned-file count per directory, so a renamed
directory reddens rather than passing on an empty walk — the waiting and
virtualization twins inline, and the seal and row-content twins through
`cairn_guards::rust_sources`; the row-content twin also requires that some
scanned file names `RowContent` at all. The seal scan reads each sealed crate's whole
directory, so `tests/` is held to the same seal as `src/`
(`the_seal_scan_reads_tests_as_well_as_src`). `layer_dependencies_are_allowlisted`
scans no source at all: it walks `crates/*/Cargo.toml`, reads every dependency
table through `cairn_guards::declared_dependencies` — dev, build and
`[target.*]` tables included, so `freya-testing` needs its `TEST_ONLY_ALLOWLIST`
row and a test-only `gix` under `cairn-ui` fails
(`the_allowlist_check_rejects_a_sealed_crate_in_every_dependency_table`) —
asserts it saw a nonzero number of CRATES, and additionally fails when an
allowlist row names a crate that does not exist, which is the same "cannot pass
on an empty walk" property in the shape that file set allows.

Four matchers stand behind them — `cairn_guards::waits_on_work`,
`cairn_guards::mentions_crate` (the sealed-crate check, and the worker half of
the waiting check), `cairn_guards::reads_row_content_partially`, and the
unbounded-view matcher — and each carries a
self-test over synthetic sources, because a matcher that has quietly stopped
matching reports green while the coverage it names is gone.

**What the guards structurally cannot decide**, owned by
`responsiveness-reviewer` (`docs/qa-gate.md`): which THREAD a function runs on,
so the `worker/` functions the UI thread itself calls are exempt from the
matcher; whether an iteration is over a history at all; and whether per-frame
work grows with scroll depth while the number of rows built stays flat, which
the component test below counts rows and so cannot see.

**Component tests** render through `freya-testing` (a dev-dependency, same fork
and rev as `freya`). `crates/cairn-ui/tests/history_list.rs` covers the list's
wiring: rows built per viewport (at the top and scrolled deep), click and key
selection reported through `on_select`, a click taking keyboard focus, the
keyboard-focus outline, the selection following its `RowId` when rows arrive
below and above it, a selection moved off screen being revealed in either
direction, and `on_reach_end` firing once the last screen of rows comes into
view but not when visible rows re-render. `crates/cairn-ui/tests/commit_row.rs`
pins each column's width and order, every heading lining up with its column,
the graph column widening with the lane count, the selected row's highlight,
and that the date and short id fit their columns at the rows' font size (a
measured width, so it needs a system font). `crates/cairn-app/src/window.rs`
renders the window from each `Progress` state — the placeholder sentence in the
list's place, the late-failure banner, the count, a clicked row's highlight,
the lane count reaching the rows — and pins that scrolling to the end submits
one `MoreHistory` per page, not one per row that comes into view, and that a
failed page is asked for again when the reader comes back to the end.
`RepositoryHandle::into_submitter`, the callback it is given in the running
app, is tested against the real worker in `crates/cairn-app/src/worker/pool.rs`.

## Known limits of what was built

- **Memory is flat in history LENGTH and linear in rows SCROLLED.** A
  100k-commit repository costs nothing until it is scrolled; scrolled rows are
  retained and nothing evicts them. A row no longer keeps the edges crossing it,
  which on rust-lang/rust cost 4.3-6.3 KB a row and 1.4 GiB to its oldest commit
  (`docs/research/refs-and-status/deep-find-measured.md`), nor its parents, its
  author's address or a heap allocation of its own; scrolling or finding to that
  commit from every ref (345,545 rows) retains 52.7 MiB, about 159 B a row,
  counted by capacity with every chunk whole: rows 23.8 MiB (72 B each), text
  16.0 MiB, lane changes 12.3 MiB (16 B each), snapshots 0.25 MiB and the author
  table with an estimate of its index 0.37 MiB (8,424 authors). Process `RssAnon` grows by
  83 MiB. Still linear in rows scrolled, and nothing evicts: tracked as issue #4.
- **A live scroll's walk retains every commit it visited.** The rows above
  are the application's row vector; separately, gitoxide's walk keeps a
  `HashSet<ObjectId>` of every commit visited (`gix-traverse`'s
  `simple::Simple`) — about 15 MB at 500k commits — and the worker holds that
  session until the next `OpenHistory` or a walk error. Holding one set for a
  whole scroll is cheaper than rebuilding it per page, which is part of why the
  session pays; it is also memory that is never released while the app runs.
  Same decision as issue #4, and noted on it.
- **There is no random access by row offset.** There is no total row count
  (counting is a full walk) and no way to build a cursor from an offset, so a
  scrollbar drag has no answer; progressive loading is the model, as it is in
  Fork and Sourcetree. Tracked as issue #5.
- **`GraphRow` still keys a row by `Oid`.** A stash's row is keyed by its stash
  commit, which no commit the walk reaches names; the kept row says which kind it
  is. A row with no `Oid` at all (Cairn draws no working-tree row) would need
  the assigner keyed otherwise.
- **Opening a walk from every ref reads a commit per tip**: about 0.1 s at 50,000
  tags on one line of history, cancellable between tips (above), and a reopen pays
  it again.
- **A reopen pays the whole opening pass again.** Without a commit-graph a reopen — every
  refresh that moves a ref, Show Lost Commits toggled — reads every commit the tips reach
  before its first row, as the first open did, however little moved (about 1.2 s on
  rust-lang/rust); the old rows stay drawn meanwhile, and nothing else waits on it. The
  walk's state is not kept across a reopen to pay only for what changed.
- **Layout and drawing grow with open lanes, and every ref can open one.** The
  assigner scans every open lane twice per row laid out (`LaneAssigner::push`), a
  snapshot holds one bit per lane, and deriving a drawn row's edges walks the lines
  crossing it on the UI thread. A walk from every ref opens a lane for each ref whose
  line has not yet merged — `refs/pull/*` and stale remote-tracking refs among them —
  so that work grows with the unmerged refs, not with the history. Measured on
  rust-lang/rust (105-160 lines per row), the 50,000-tag chain (one lane), and a fixture
  holding 100, 1,000 and 5,000 unmerged lines open at once
  (`measures_layout_over_unmerged_refs`): the first page from every ref took 3.1, 8.4 and
  25 ms, and deriving a 40-row viewport's edges under every line 0.03, 0.11 and 0.49 ms
  (release; numbers in `docs/research/refs-and-status/measured.md`, section 2). Drawing them
  is bounded by the places a row paints, not the lines crossing it (`graph_geometry::
  row_geometry` paints each place once, the lines past `MAX_DRAWN_LANES` sharing the last
  column): a 35-row viewport under 5,000 open lines builds and lays out in 0.76 ms and
  paints no slower than one under 100 (`measures_drawing_a_viewport_over_many_open_lanes_and_a_crowded_rows_chips`,
  `crates/cairn-ui/tests/drawn_lanes.rs`; before, 175,000 strokes added 119 ms a frame to
  the CPU raster).
- **Chips read two snapshots.** A row's labels are the snapshot its walk began from; which
  ref is a branch's upstream, and which branch `HEAD` is on, are read from the refresh's
  latest. A refresh that changed only an upstream's configuration reopens nothing, so the
  fold follows it at once; between a refresh's refs arriving and the reopened history's
  first page, a row may fold, or lead with, by the newer snapshot's word.
- **No chip is built before the list is measured.** The list learns its width from its
  first layout (`on_sized`), so the first frame of a list draws its rows without chips and
  the next with them.
- **Show Lost Commits reads the reflogs when the walk opens, and a reopen is what reads
  them again.** A reflog that changes while no ref the graph draws moves — `git reflog
  expire`, an entry pruned — leaves the history drawing what it read until the next reopen.
  git expires an unreachable commit's entry after `gc.reflogExpireUnreachable` (30 days by
  default), and from then on the walk no longer has it. Only `HEAD`'s and the local
  branches' logs are read, where Fork's mode reads every reflog (`git log --all --reflog`):
  a remote-tracking ref's log, a deleted branch's leftover log and another worktree's
  `HEAD` are not.
- **A long reflog slows Show Lost Commits' first page** (accepted by the user as a stated
  residual): the open checks each distinct reflog id once — from a commit-graph where one
  holds it — and the walk reads it as a tip, and without a commit-graph those reads from the
  pack are the cost, about 5 µs a read on rust-lang/rust. The reflog reads alone, measured
  above in commit-time order: a `HEAD` log of 10,000 entries
  makes the first page 50.8 ms with no commit-graph and 8.8 ms with one, where git's own
  `rev-list --reflog` takes 57-80 ms; 200 local branches each with a 50-entry log, the same
  10,000 ids, 50.6 ms and 8.9 ms. The cost is the ids across every log read — `HEAD`'s and each
  local branch's — and it is paid again on every refresh that moves a ref while the toggle is
  on, since each reopen reads every log again. Merging the logs' ids with the refs' tips is
  linear (a hash set, `history::open_stream`). Show Lost Commits walks date order, so without
  a commit-graph its first page also pays the whole opening pass (about 1.2 s on
  rust-lang/rust), and so does each reopen while it is on.
- **The first page of a scroll walks `window + limit` commits** before a single
  row can be delivered, because rows leave the assigner only once evicted. Those
  are walk steps, not object reads. Do not shrink the page to make it feel
  faster; shrink the window, and measure.
