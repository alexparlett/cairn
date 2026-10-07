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
The window opens its history from the snapshot its first refresh reads on the history
thread, and reopens it whenever a later refresh — on focus, after a fetch, on the
Refresh action — finds what the graph draws changed ("Refresh", under the worker
boundary below). Each row draws its refs as Fork's chips between the graph and the
subject, a stash's row its `stash@{n}` chip, and `HEAD`'s subject bold ("Chips on a
row", under the view below); the title bar names the repository, its branch and how
far that branch is from its upstream ("The title bar", there too).

## The path a commit takes

One commit crosses four crates and changes shape three times.

1. **`gix` hands over a walk entry.** `Repository::history_session`
   (`crates/cairn-git/src/history/session.rs`) opens one gitoxide revision walk
   and keeps it for the life of a scroll. Each step yields an id and its parent
   ids, read from the walk rather than from a decoded object — the parents git
   shows, which in a shallow clone are not always the ones the object names
   (see "A shallow clone" below). A walk from a refs snapshot merges each stash's
   row into those steps (`crates/cairn-git/src/history/stream.rs`).
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
   applied before); `crates/cairn-ui/src/history_list.rs` draws the rows the
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
assigner still held and repainted a line to. In the order Cairn walks that is
0-0.2% of rows (0.197% at the highest, and zero on four of the seven); in
`GraphOrder` it is most rows, by construction. That rate bounds the blind spot
from above without measuring it, since a parent must arrive late at all before
it can arrive too late.

**A stash's row is laid out with one line, in a lane of its own**
(`LaneAssigner::push_stash`). The stash commit is pushed as the row's id with its
base — the commit it was made on, its first parent — as its only parent: its index
and untracked commits are never awaited. Its line leaves its node down a lane of its
own to the base, even where another line already awaits that base, so several
stashes on one commit each keep a lane down to it and every such line ends there
(`Ends` for each); a commit's line instead joins a line already descending to its
parent. The assigner is still keyed by `Oid` — the stash commit, which no commit the
walk reaches names as a parent. A base already laid out above is joined by a late
line, as any parent is, though the stream never places a stash below its base.
Pinned by `a_stash_draws_one_line_down_to_the_commit_it_was_made_on`,
`two_stashes_on_one_commit_each_keep_a_lane_down_to_it` (whose control lays the
same walk out with `push` and gets one lane for both) and
`rows_with_stashes_derive_the_edges_the_assigner_kept`
(`crates/cairn-model/src/lane_assignment.rs`), each deriving every row's edges and
requiring them equal to the edges the assigner kept in its window. C15's oracle
knows commits alone, so its comparisons run over walks with no stash.

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
verbatim as test code (`crates/cairn-model/tests/layout_before_compaction/mod.rs`,
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

**Cancellation is a poll, once per commit** — once per row laid out, once per commit a
walk from a refs snapshot looks ahead at (below), and once per tip read to open the walk
but the first. In commit-time order gitoxide reads every tip for its committer date
before the walk's first step, in one call that polls nothing (`Simple::sorting`); so
`walk::open` (`crates/cairn-git/src/history/walk.rs`) reads the tips in a loop of its
own, polling between them, and answers gitoxide's read of each from the dates it read
(`Seeded`), costing no second object read. A session opens its walk on its first page,
under that page's cancel; a cancelled open keeps nothing, and the next page opens it
again (`opening_a_walk_from_many_refs_is_cancelled_between_tips`). `Cancel` is a trait so a test can
stop a walk at a chosen commit; `cairn-git` still knows nothing about threads. A
cancelled cold query reports how far it got and discards the page; a cancelled
*session* keeps its progress, so the next call continues rather than re-walks
(`a_cancelled_query_stops_walking_and_says_where`,
`cancelling_a_session_stops_the_walk_and_keeps_its_progress`).

**A shallow clone is walked as git walks it.** A boundary commit — one the
clone's `shallow` file lists — names parents in its object that the clone does
not have, and git reads the file as grafts: the commit has no parents, so `git
log --format=%P` prints nothing for it and `--graph` draws it as a root, while
a parent the clone DOES have, reached through another branch, is still shown.
gix's own `rev_walk` handles the file otherwise: it hands the boundary commit
over with the parents its object names, and skips the next appearance of each
of those ids whichever commit names it — so a merge-shaped history cloned with
`--depth` loses a commit its sibling branch reaches, and the lane assigner holds
a lane open for a parent that never arrives. Both routes therefore walk through
`crates/cairn-git/src/history/walk.rs`: gitoxide's `Simple` traversal itself,
reading every commit through a `Grafted` object source that serves a boundary
commit with its `parent` header lines removed — git's graft, applied where the
object is read — and with no commit-graph in a shallow repository, which git
does not read there either (`commit_graph_compatible`). The shallow file is
read once per walk (`crates/cairn-git/src/shallow.rs`, which
`Repository::commit_details` reads too). With no parents a boundary commit is a
root to the assigner, so its line ends there and no lane stays reserved below
it. Pinned against `git log --format='%H %P'` at four depths of one history — a
lone tip, a merge cut at the boundary, two boundaries on two branches, and a
boundary whose parent the clone has through a sibling — on both routes, with
the graph required to be the one git's parents lay out
(`a_shallow_clones_boundary_commits_have_the_parents_git_log_shows`,
`crates/cairn-git/tests/history.rs`); the header rewrite by
`only_the_headers_parent_lines_are_removed`.

Order is `HistoryOrder::CommitTime` by default, chosen by measurement; neither
it nor `GraphOrder` is topological, which is why the assigner must be total over
arrival order. The measurement (`measures_both_orders_against_a_named_repository`,
`#[ignore]`d): walking *and laying out* 50k commits took 129 ms in commit-time
order against 196 ms in graph order, which leaves far more lanes open per row.
Walking alone is the other way round.

### From every ref, labelled, with stash rows

`HistoryRequest::from_refs(&RefsSnapshot, limit)` (`crates/cairn-git/src/history/seeds.rs`)
reads the snapshot as plain data: its seeds are every commit a listed ref
identifies — a tag on a tree or a blob identifies none, so seeds nothing and labels
nothing — each once, and `HEAD`'s, a detached `HEAD` naming what is not a commit
seeding nothing; its labels are, per commit, the refs pointing at it in the
snapshot's order, the branch `HEAD` is on marked current; and it carries the stash
list. Nothing about a stash seeds the walk. When the request is resolved, each stash's
commit is read for its author, its author date and its committer date, and its base's
committer date — two reads per stash, polling nothing — and a stash whose commit cannot
be read, dropped and pruned since the snapshot, has no row, the walk going on without it
(`a_stash_whose_commit_is_gone_since_the_snapshot_has_no_row`); and a stash commit
`git stash store` filed twice in the list is one row, its newest entry's — a commit is
one row in a graph (the user's decision, 2026-10-07; the sidebar lists every entry) —
(`a_stash_commit_filed_twice_is_one_row_its_newest_entrys`); the resolved labels and
stashes (`Decoration`) are shared by every page and cursor of the walk, as its tips are
— the labels by reference, never copied — so a cold page resumed from a cursor carries
the labels and stashes of the snapshot the walk began from.

Both routes lay out the same stream (`crates/cairn-git/src/history/stream.rs`): the
walk's commits, each stash's row merged in. A stash's row goes where its commit's
date falls among the walk's commits — newest committed first, the walk's own order —
but never after its base, and only when the walk reaches the base. How the stream
knows the base is reached when the stash's date comes up is by looking ahead in the
walk itself: the commits it has pulled and not yet handed on are searched for the
base, and more are pulled to find it until `LOOKAHEAD` (4,096) are ahead of the next
one handed on, stopping once the walk passes the base's own committer date without
meeting it. Found, the row goes at the stash's date; not found, the stash is held.
Whenever the next commit to hand on is the base of a stash not yet placed — held, or
not yet due because its date is older than its base's by clock skew — the row goes
directly above it, once every stash newer than it is placed or held: stash rows keep
their own date order, newest first and `stash@{0}` first among equal dates, and
several stashes on one commit go above it newest first. A stash's date comes up when it
is no older than the next commit, so a stash dated the same second as a commit goes
first, as git's date order puts a stash above the index commit it was made with. A
stash still unplaced when the walk ends has no row. So a row is there exactly when the
walk reaches its base, and costs no walk of its own: the commits looked ahead at are the
next the stream hands on. A stash commit the walk reaches through a ref is that commit's
row and no stash's, whether the walk hands it on first or the look-ahead meets it. In
graph order, which has no dates, every stash goes directly above its base.
`HistoryRequest::with_stash_lookahead` sets the distance, and a cursor carries it.
`HistoryPage::walked` counts rows laid out, stash rows among them; `decoded` counts
commits read, which a stash's row adds none to; cancellation is polled once per row
and once per commit looked ahead at; `HistorySession::commits_walked` counts every
commit pulled off the walk.

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
— each stash's index, base, message, author and date against `git stash list`, one
line out of its node down its own lane into its base, two stashes on one commit in
two lanes, a stash dated older than its base directly above it and the rest at their
dates; `a_walk_paged_any_way_draws_the_rows_one_page_does` (held and cold, pages of
one, two and five); `a_stash_on_a_deep_base_is_drawn_at_its_date_or_directly_above_its_base`;
`a_detached_head_no_ref_reaches_is_walked_and_labelled`;
`a_stash_commit_a_branch_reaches_is_a_commits_row_and_no_stashs`;
`a_braided_history_from_every_ref_is_git_rev_lists` (order too, against
`--date-order`); `in_graph_order_a_stash_is_directly_above_its_base`;
`a_stash_commit_met_while_looking_ahead_is_a_commits_row_and_no_stashs`;
`stash_rows_are_in_gits_date_order` (against `git log --date-order` over the refs and
the stash commits); `a_page_cancelled_while_looking_ahead_stops_and_resumes_to_the_same_rows`;
and `looking_ahead_stops_once_the_walk_passes_the_bases_date`.

**Measured** (`measures_the_first_page_from_every_ref`, `#[ignore]`d; release, warm,
median of seven, the machine in `docs/research/diff-engine/measured-baseline.md`), on
rust-lang/rust at `c999cef531e`, which has no commit-graph file: the first page of 64
rows from `HEAD` 7.7 ms, from every ref 7.3 ms with the snapshot already read and
8.6 ms with its read; each laid out 1,088 rows. On a scratch clone of it with real
stashes: a stash on `HEAD` adds nothing (7.4 ms); a stash whose base is 20,000 commits
deep, or a stash on a deleted branch's old commit, looks ahead the whole 4,096 commits
on the first page and costs 21.5 ms (3.1 ms with the clone's own commit-graph); the
stream never holds more than 4,096 commits pulled ahead of the next row, so that is
the most looking ahead adds to a page.
A find of rust's oldest commit from the snapshot takes 2,265 ms against 2,253 ms from
every ref's commit unlabelled and 2,211 ms from `HEAD`.

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
  `serve` on the repository thread, `serve_diffs` on the diff thread: a
  per-request conversion compiles and passes every test while rebuilding the
  object cache and the pack snapshot each time, which is why the call site
  carries a comment rather than being left to look obvious.
  `Repository::OBJECT_CACHE_BYTES` (4 MiB) is installed by that handle, and is
  part of why `HistoryOrder::CommitTime` is the cheaper default: measured, a
  commit-time walk of 50k commits cost 178 ms without the cache and 116 ms with
  it, and more bought nothing. `a_shared_repository_can_cross_threads` pins
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
  ahead/behind, status or ref filter, and a new query supersedes the older ones in
  its own lane only — except that a changes query also supersedes the file-diff
  lane (`QueryLane::supersedes`), since a file of the commit that was selected is
  no file of the one that is now, and that nothing supersedes the status lane: a
  refresh sends its status under the lane's unmoved number, so only a close cancels
  one ("Refresh", below). So a scroll never cancels a diff, a selection
  never cancels a scroll, and an operation, numbered in no lane, supersedes
  nothing (`each_lane_supersedes_itself_and_a_changes_query_the_file_diff_too`;
  through the real boundary, `crates/cairn-app/src/worker/diff_tests.rs`). The
  history has two: its query lane, which an open, a scroll's page, a find in the
  sidebar and its stop are numbered in, and its walk lane, which only an open
  moves and every page of rows is answered under — so a page a superseded find had
  laid out still arrives, and a reopen's replaced walk's pages do not
  (`docs/systems/sidebar.md`, "Pressing an entry").
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
  bounded (`fetch_tests::WAIT`, in `block_on` and `woken_by`), so a hang of
  that shape is a red test with a name.
- **An explicit routing table names the thread for each lane**
  (`worker/routing.rs`, PRD R4.2, refs-and-status R11.2): the history lane and a
  refresh's refs on `cairn-repository`, which owns the live walk — it borrows that
  thread's handle across turns and is not `Send`, so it never moves
  (`worker/history_lane.rs`) — with the Changes tab's and the sidebar's filters; the
  changes and file-diff lanes, commits, comparisons and the working tree alike, on
  `cairn-diff` (`worker/diff_lane.rs`); and status and ahead/behind on
  `cairn-refresh` (`worker/refresh_lane.rs`), so neither a page nor a diff queues
  behind a slow status. `route` applies it to every request as it is
  submitted and hands each thread its own job type, so neither thread forwards
  the other's work (`every_query_is_served_on_the_thread_its_lane_is_routed_to`).
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
  the network lane (`worker/network_lane.rs`, `cairn-network`), its own
  `Update` variants, and requests that carry no epoch so a scroll and a fetch
  cannot supersede each other; the credential
  dialog is a third thread (`worker/askpass.rs`) blocking on the window's
  reply. How it honours `Invalidated::refs` is the refresh below — a fetch's
  every ending makes the window ask for one, which reopens the history if the
  fetch moved what it draws. The
  as-built description is `docs/systems/credentials.md`; the lane, the close
  and the command log are `docs/systems/git-processes.md`.

### Refresh

As-built for refs-and-status R10 and R11 (phase 06). `Request::Refresh` is three
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
kept by the user's decision of 2026-10-07), so a scroll page asked while a refresh reads
them waits behind one refs read — about 22 ms at 10,500 refs warm, more at 40,000 and
cold, measured by phase 06's QA — at most once per trigger; a refresh never cancels the page. A status, once
started, runs to its end whatever refreshes arrive, so a window switched faster than a
status takes still gets one, followed by at most one more; each status is a full stat of
the tree (736 ms on a stat-dirty rust-lang/rust), which a refresh asked while one runs
pays once more after it, not once per switch.

It is asked for on three occasions, and the UI thread only submits on each: the window
gaining focus (`crates/cairn-app/src/refresh.rs`, a side effect on the toolkit's
`Platform::is_app_focused` that submits on a false-to-true change and on nothing else —
`gaining_focus_asks_for_a_refresh_and_nothing_else`); any fetch ending, finished,
cancelled or failed (`session::apply`;
`every_fetch_ending_asks_for_a_refresh_and_touches_no_row`); and the Refresh action,
F5 on Linux and ⌘R on macOS in the accelerator table
(`the_refresh_chord_asks_for_a_refresh`). The window's first refresh, as it opens, is
what opens the history: there is no walk yet to compare with. Nothing watches the
file system (R10.2).

**What reopens the history.** The repository thread keeps, beside the walk
(`HistoryLane`), the snapshot the walk began from and the snapshot the last refresh
read. A refresh compares the refs it reads with the first by
`RefsSnapshot::walks_as` — the same refs naming the same objects, the same `HEAD`,
the same stash list — and answers `Update::Refs { snapshot, reopen }`. The window keeps
the snapshot (`refresh_state.rs`, which the history's chips, the title bar, the sidebar
and the Commit tab's REFS row read) and, on `reopen`,
reopens: the old `History` is moved out, a new one is built with
`History::with_author_capacity` of the old one's author count, the history is asked
for again and then the old one handed to the repository thread to free as a
`Request::Retire` (#52, R11.3) — in that order, so the new first page is not queued
behind the free — and the selection is left as it is, drawn again when its row arrives
(R10.5). A chosen commit that never arrives again — its branch deleted, nothing else
reaching it — stays chosen: no row is drawn chosen, and the detail pane keeps the
answer it holds for it until another row is chosen. A stash pushed, a checkout that moves no ref and a moved ref each reopen; a
refresh that finds nothing changed does not, and neither does an upstream's
configuration alone (`a_walk_is_the_same_unless_what_it_draws_changed`, in the model)
(`a_refresh_reopens_for_a_stash_a_checkout_and_a_moved_ref_and_for_nothing_else`,
counting the window's reopens through the real boundary;
`focus_gained_after_a_ref_moved_reopens_the_history_keeping_the_chosen_row`, headless
with focus set; `a_reopen_frees_the_old_rows_on_a_worker_and_keeps_the_selection`).
The fetch's own before-and-after comparison of the refs, and `session::reload_if`
that acted on it, are gone.

**What an open walks from.** `OpenHistory` walks from the snapshot the last refresh
read, so a reopen reads the refs once. With none read — a test that opens without a
refresh, or after a failed open, which forgets them — the open reads its own and
answers it as `Update::Refs { reopen: false }` in the history lane before its first
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
(`crates/cairn-app/src/history_state.rs`) is that debounce: it is false while a
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
  (`crates/cairn-ui/src/date_text.rs`, a private module) is Hinnant's
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
  (`crates/cairn-app/src/status_text.rs` — `cairn-app` is a binary, so its
  modules are cited by file, not by a `cairn_app::` path) decides which, so the
  reader never sees a blank area that could mean any of the three. A failure
  that arrives after rows are drawn is a banner under them, not a replacement
  for them. `Status::Empty` is reachable because the history lane maps
  `Error::UnbornHead` to `Update::Rows { rows: [], complete: true }` rather than
  to a failure — a repository with no commits is empty, not broken.
- **Keyboard**: arrow keys, `PageUp`/`PageDown`, `Home` and `End`, as a pure
  `moved_to(key, current, last)` (private, in
  `crates/cairn-ui/src/history_list.rs`). The list is auto-focused, so a reader with no
  mouse can select at all. `PREFETCH_ROWS` is 24: every row within that many of
  the end asks for the next page when it becomes visible — every one of them,
  because a single trigger row is a list that quietly stops loading on a tall
  window.

### Chips on a row

A row draws the refs pointing at its commit as Fork's chips between the graph and the
subject (refs-and-status R5; `crates/cairn-ui/src/ref_chips.rs`): outlined, filled with a
tint of the row's lane colour — a tag indigo whatever its lane — each kind told by its
glyph and shape (R5.5): a local branch a plain chip, the current one with a check mark
before its name; a remote-tracking ref and a tag a glyph — a generic remote's cloud (a
forge's icon is packet 6's) and a tag — in a cap of their own at the chip's left; a
stash's row a `stash@{n}` chip with a box. The glyphs are painted as paths on a canvas
(`crates/cairn-ui/src/ref_glyphs.rs`), no font involved, every one in the chip's text
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

### The title bar

The title bar's status box (`cairn_ui::StatusBox`, `crates/cairn-ui/src/status_box.rs`;
R7.1, Fork's section 9) names the repository — its folder, as Fork names it: the last
component of its working tree (of its git directory when it is bare), which the worker
sends once the repository is open (`Update::Opened`, kept in `View::repository`), so a
launch at `.`, `..`, a subdirectory or its `.git` names the repository and not the path
given (`an_open_names_the_repositorys_folder_whatever_path_it_was_opened_at`); nothing
until it is open — `*` after it while the last status read listed a change — then a branch glyph
and the current branch, and its distance from its upstream as Fork prints it, behind then
ahead, `18↓1↑` (no space, as Fork prints it), a zero count left out and nothing for a branch level with its upstream;
or `upstream gone` for a configured upstream no ref is (git's `[gone]`); a detached `HEAD`
as `HEAD detached at <short id>` and an unborn branch as `<name> (no commits yet)`, in
git's words. The arrows are IBM Plex Mono's, which the application embeds. The window
builds it from what the last refresh answered (`window::status_box`): the snapshot's
`HEAD` and the branch's upstream, the branch's counts found by name
(`RefreshState::ahead_behind_of`, a binary search, the counts coming in the snapshot's
order) and whether status listed anything — each as old as its answer, a status up to
one refresh behind (R10.3 as amended). An unreadable index or no working tree draws no
`*`. Pinned by `the_title_bar_names_the_repository_the_branch_and_how_far_it_is_from_its_upstream`
(`crates/cairn-app/src/window.rs`: a dirty branch with counts, a gone upstream, a detached
and an unborn `HEAD`), `a_branchs_counts_are_found_by_its_name`
(`crates/cairn-app/src/refresh_state.rs`) and the status box's own tests.

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
| A row's chips are laid out once per row built, never per frame | `a_rows_chips_are_laid_out_once_per_row_built_and_never_per_frame` (`crates/cairn-ui/src/history_list.rs`) |
| Every row draws the edges the assigner drew before rows were compacted | `every_row_draws_the_edges_the_assigner_retained_before_compaction`, `the_cairn_checkouts_rows_draw_what_the_assigner_drew_before_compaction`, `each_cold_page_and_resumed_session_draws_on_its_own_what_the_assigner_drew_for_it`, `rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew` |
| Every row draws the subject, author, date, short id and merge marker it drew before rows were slimmed | `every_crafted_row_draws_what_it_drew_before_rows_were_slimmed`, `every_row_of_the_cairn_checkout_draws_what_it_drew_before_rows_were_slimmed`, `every_row_of_a_braided_history_draws_what_git_prints` (`crates/cairn-git/tests/slim_rows.rs`), `every_row_draws_what_it_drew_before_rows_were_slimmed` (`crates/cairn-ui/tests/drawn_rows.rs`), `the_cairn_checkouts_rows_draw_what_they_drew_before_rows_were_slimmed` and `the_windows_table_agrees_with_the_engines_table_captured_before` (`crates/cairn-app/src/window.rs`) |
| No kept row owns a heap allocation; a history's stores grow in chunks | the `Copy` assertion beside `StoredRow` in `crates/cairn-model/src/history.rs`, `a_kept_row_is_seventy_two_bytes` (`crates/cairn-model/src/history.rs`), `appending_ten_thousand_rows_allocates_per_chunk_never_per_row` and `reading_a_rows_identity_lane_and_changes_allocates_nothing` (`crates/cairn-model/tests/history_allocations.rs`), and the store tests in `crates/cairn-model/src/chunked_store.rs` |
| Each author is named once; a subject reads back exactly | `a_page_of_new_authors_and_a_page_of_known_ones_both_draw_and_each_is_named_once`, `authors_whose_keys_collide_are_told_apart_by_name`, `an_empty_a_non_ascii_and_a_very_long_subject_read_back_exactly` (`crates/cairn-model/src/history.rs`) |
| What a history retains counts every store, chunks whole | `what_a_history_retains_counts_every_chunk_whole`, `the_author_index_is_estimated_by_its_buckets` (`crates/cairn-model/src/history.rs`), `a_run_exactly_a_chunk_long_leaves_the_chunk_being_filled_alone` (`crates/cairn-model/src/chunked_store.rs`) |
| A full history keeps the rows it held and ends the scroll | `a_page_past_the_historys_limit_keeps_the_rows_before_it_and_says_so` (`crates/cairn-model/src/history.rs`), `a_full_history_stops_asking_and_keeps_saying_so` (`crates/cairn-app/src/history_state.rs`) |

The first five live in `crates/cairn-guards/tests/invariants.rs`; the sixth is a
headless component test in `crates/cairn-ui/tests/history_list.rs`; the rest are
the model, engine, list and window tests named under "What a row keeps". The
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
  is. A row with no `Oid` at all (there is to be no working-tree row) would need
  the assigner keyed otherwise.
- **Opening a walk from every ref reads a commit per tip**: about 0.1 s at 50,000
  tags on one line of history, cancellable between tips (above), and a reopen pays
  it again.
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
  (release; numbers in `docs/work/refs-and-status/progress.md`, phase 06). Drawing them
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
- **A stash's row is at its date only when its base is near.** A stash whose base
  lies more than `LOOKAHEAD` commits below its date, or past a skewed date, is drawn
  directly above its base instead; one whose stash commit is reached through a ref
  only after the stash's row was placed — the stash commit beyond the look-ahead, or
  past a skewed date — would have a stash's row and a commit's. Neither is measured
  on a real repository with stashes.
- **The first page of a scroll walks `window + limit` commits** before a single
  row can be delivered, because rows leave the assigner only once evicted. Those
  are walk steps, not object reads. Do not shrink the page to make it feel
  faster; shrink the window, and measure.
