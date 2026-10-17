# The history walk as built, read for `history-topo-order`

Evidence record for the `history-topo-order` packet (daily-loop roadmap 5c). It decides
nothing. It reads the code as it stands and records what a walk that never hands on a commit
before every child it walks would change. Recorded 2026-10-17 at `735ae10`
(`feature/plan-ui-review`). Read-only audit. Every `file:line` anchor is from that commit and
will drift. Dependency versions are the ones `Cargo.lock` pins: gix 0.87.1, gix-traverse
0.61.0, gix-revwalk 0.35.0. One measurement was taken for this record: §3.6, git's own
rev-list on the bench, read only, `GIT_OPTIONAL_LOCKS=0`. Every other number is quoted from
the doc it names.

## Headlines

1. **The bug starts in gix's queue, not in Cairn.** The walk is
   `gix::traverse::commit::Simple` with `Sorting::ByCommitTime(NewestFirst)`
   (`crates/cairn-git/src/history.rs:47-53`). Its queue is gix-revwalk's `PriorityQueue`,
   which is a `BinaryHeap` ordered by the key alone (`gix-revwalk-0.35.0/src/queue.rs:36-39`,
   `71-73`). The key is the committer time (`gix-traverse-0.61.0/src/commit/simple.rs:86`).
   Equal times therefore pop in heap order, and git's prio_queue breaks those ties by
   insertion order. So with tied times gix can pop a parent before a sibling that is its
   child, and skew does the same with no tie at all. Cairn has no ordering logic of its
   own: it lays out whatever the walk yields.
2. **gix 0.87.1 does ship a topological walk.** `docs/research/history-graph/gix-revwalk-ordering.md`
   and `docs/design/history-graph.md:78` both say gix has no `--topo-order` equivalent.
   That is out of date for the pinned crates. `gix::traverse` re-exports gix-traverse
   (`gix-0.87.1/src/lib.rs:157`), which has `commit::Topo`, built with `topo::Builder`.
   Its `topo::Sorting` has two variants, `DateOrder` ("equivalent to `git rev-list
   --date-order`") and `TopoOrder` ("equivalent to `git rev-list --topo-order`")
   (`gix-traverse-0.61.0/src/commit/topo/mod.rs:39-66`). It is git's incremental
   algorithm: an explore walk and an in-degree walk, run down to the lowest generation
   number seen. Without a commit-graph every generation is `GENERATION_NUMBER_INFINITY`
   (`topo/iter.rs:303-317`), so `Builder::build` walks the whole history before the first
   commit comes out (`topo/init.rs:121-173`). It does that in one call that polls nothing.
   Its parity with git on tie order is NOT verified here (§1.12).
3. **What that costs on the bench.** git itself, on the bench clone of rust-lang/rust
   (345,545 commits from every ref, no commit-graph), returns 64 rows with
   `rev-list --max-count=64 --branches --remotes --tags HEAD` in:
   - 3-4 ms in default order
   - 1,247-1,300 ms with `--date-order`
   - 1,266-1,279 ms with `--topo-order`

   (§3.6, measured here). Cairn's first page today is 7.27 ms. The C11 bar is 200 ms.
   With a commit-graph, git's `--graph` first row on a synthetic 200k history drops from
   337 ms to 1 ms (`scroll-memory-model.md` Finding 2).
4. **The lane assigner absorbs out-of-order arrival, and on rust that is not rare.** Each
   row stays repaintable inside a 1,024-row window. The C15 equivalence run over the whole
   bench from every ref counted 49,609 rows repainted (14%) and 627 late lines
   (`docs/research/refs-and-status/measured.md`, section 3). `docs/systems/history-graph.md`
   still says "0-0.2% of rows", which is a figure from seven small repositories.
5. **What a guaranteed child-before-parent stream would make dead.** The assigner's
   late-parent path, `Reach`'s "reached after all" correction, and the window's priming
   (the first page lays out 1,088 rows to hand over 64). What has to stay:
   - The frozen oracle and its fingerprint.
   - The C15 tests. They feed skewed walks to the assigner and require repaints.
   - `HistoryOrder::GraphOrder` and `HistoryOrder::CommitTime`. Tests and reporters use
     both, and `recent_messages` needs commit-time order for `git log -n 10` parity.

   So the assigner has to stay total. A topological walk would be an added order, not a
   replacement.

---

## 1. The walk end to end (`crates/cairn-git/src/history/`)

### 1.1 Orders and sorting

`HistoryOrder` (`history.rs:30-54`) has two variants. Its doc comment says "Neither order is
topological: a parent can arrive before its child".

- `CommitTime` maps to `Sorting::ByCommitTime(CommitTimeOrder::NewestFirst)`. It is the
  default (`history.rs:39-44`).
- `GraphOrder` maps to `Sorting::BreadthFirst`.

Nothing in `cairn-app` sets an order, so the application always walks in `CommitTime`.
`with_order` is called only from tests and reporters (`compact_rows.rs`, `tests/history.rs`,
the `history.rs` reporters) and from `recent_messages.rs:29-34`, which passes `CommitTime`
explicitly.

**Where the tie bug comes from.** `next_by_commit_date`
(`gix-traverse-0.61.0/src/commit/simple.rs:517-580`) pops the newest time and pushes each
parent keyed by the parent's committer time. With equal keys the `BinaryHeap` order is
whatever the heap's layout gives. Git's `prio_queue` instead compares an insertion counter
on a tie. Fork's survey shows the symptom: `git log` without `--graph` draws parents above
their children when every date is equal, and `git log --graph` and Fork never do
(`docs/research/ui-review/fork-window-survey-2-2026-10-16.md:304-357`). Fork does this even
with "Sort commits: By date" chosen.

### 1.2 Opening the walk (`walk.rs`)

`walk::open` and `walk::open_dated` (`walk.rs:46-116`) build `gix::traverse::commit::Simple`
over `Grafted` (`walk.rs:294-322`). `Grafted` is the object database with a shallow
boundary commit's `parent` lines removed, which is how git applies its graft. The
commit-graph is used only when the repository is not shallow (`walk.rs:70-78`).

In commit-time order gix reads every tip for its date inside `Simple::sorting`, which polls
nothing. So Cairn reads the tips first, in its own loop:

- `cancel` is polled before each tip but the first (`walk.rs:84-104`).
- Each tip's date comes from the commit-graph where the graph holds it (`graph_date`,
  `walk.rs:119-122`), otherwise from the object.
- Dates already known are reused. Show Lost Commits' reflog tips arrive with theirs.
- gix's own tip reads are then answered by `Seeded` (`walk.rs:129-168`). `Seeded` serves a
  fake commit that holds only the date and **no parents** while its date map is
  non-empty. The map is cleared once `sorting` returns (`walk.rs:113-114`).

**Note for the packet.** `Seeded` would be wrong under `Topo::build`.
`topo/init.rs:149-158` and `183-198` read each tip both for its date and for its parents
(`collect_all_parents`). A tip served parentless would cut the walk off at the tips.

`walk.rs` has two other walks that are not the history:

- `hiding` (`walk.rs:177-197`), a `Simple` with `hide` behind a `Polled` object source
  that fails a read once cancelled. Used by `ahead_behind.rs:94` and `ops/amend.rs`.
- `reaches_through_graph` (`walk.rs:207-260`), used by `ops/amend.rs`.

`Polled` is a pattern that already exists for cancelling a gix call that polls nothing
itself, which is exactly what a `Topo::build` would need.

### 1.3 Tips (seeds)

- **`Start::Head`.** `HEAD` peeled to one id; an unborn `HEAD` is `Error::UnbornHead`
  (`history.rs:581-594`).
- **`Start::Commits`.** Ids checked against the repository's hash kind (`walk_tips`,
  `history.rs:611-628`).
- **`Start::Refs`.** `RefSeeds::of` (`seeds.rs:57-101`) takes every commit a listed ref
  identifies, each once, in the snapshot's order. A tag on a tree or blob is skipped. It
  also keeps the labels per commit and the stash list. `seeds::resolve`
  (`seeds.rs:159-193`) adds `HEAD`'s commit when no label names it; a detached `HEAD`
  naming a non-commit seeds nothing. It reads each stash's commit (`stash_of`,
  `seeds.rs:202-231`) for its author, its author date, its committer date (`committed`)
  and its base's committer date (`base_committed`). Nothing about a stash seeds the walk.
- **Show Lost Commits.** `Marking` (`history.rs:59-83`) starts as `Unread(branches)`.
  `open_stream` (`history.rs:88-128`) reads `reflogs::reflog_tips` (`reflogs.rs:30-90`)
  under the first page's cancel. That gives the old and new id of every entry of
  `HEAD`'s reflog and each local branch's, de-duplicated and dated (commit-graph first).
  Those ids are appended after the refs' tips, and the marking becomes
  `Read { refs: n }`. `Marking::reach` builds `Reach::new(refs, reflogs)`.

The tips are shared as `Arc<[ObjectId]>` (`history.rs:57`), so a cursor carries them on
without copying.

### 1.4 Stash rows merged into the stream (`stream.rs`)

`Stream` (`stream.rs:91-113`) wraps the walk. It keeps a `VecDeque` of commits pulled but
not yet handed on (`ahead`), a `Place` per stash (`Waiting`, `Held` or `Done`), stashes by
base and by their own commit, and stashes sorted newest committed first, `stash@{0}` first
on a tie (`stream.rs:133-148`). `next_entry` (`stream.rs:203-251`) runs four rules:

- **Directly above its base.** `above` (`stream.rs:254-263`) finds the newest unplaced
  stash on the commit about to be handed on. Placing it there is how "never after the
  commit it was made on" holds.
- **At its date.** `due` (`stream.rs:268-292`) makes a waiting stash due when it is **no
  older than the front commit's time**. A tie puts the stash first, matching git's date
  order with a stash above its index commit. Stash rows keep their own order.
- **Look ahead for the base.** `look_ahead` (`stream.rs:296-330`) searches `ahead`, then
  pulls up to `LOOKAHEAD` (4,096, `stream.rs:48`) more. Found: the stash goes here, at its
  date. Not found: it is held. The search **stops early once a pulled commit is older than
  the base's committer date** (`stream.rs:322-327`).
- **The walk reaches the stash itself.** The stash is dropped, and that commit's row is a
  commit's row (`stream.rs:240-246`, `301-308`, `316-318`).

In graph order `front.time` is `None`, so `due` never fires and every stash goes directly
above its base.

**What this assumes about order.** Both `due` and the early stop assume the front's time
falls more or less monotonically. That is true of commit-time order and close to true of
git's `--date-order`, where times fall except where a commit is held back by a child. It is
not true of `--topo-order`, which drains one line of history before switching, so times
rise again. Under `--topo-order`, `due` could place a stash after an old side-branch commit
and above newer main-line commits. The early stop could also give up on a base that comes
soon after, holding the stash and drawing it directly above its base instead of at its
date. The parity pins are against `git log --date-order` (§3.1), so a `TopoOrder` walk
would need both the rule and its oracle re-decided.

### 1.5 Show Lost Commits' reachability, decided as the walk goes (`reach.rs`)

The module doc (`reach.rs:1-17`) states its assumption: "The walk yields the newest
committed first, so a commit's children come off it before the commit itself — except where
a child is dated older than its parent."

- `pulled` (`reach.rs:64-83`). A commit is reached if it is in `frontier`; its parents then
  join the frontier through `reach`. Otherwise it is recorded as `Lost { parents, row }`.
- `reach` (`reach.rs:111-127`) walks back through the commits taken for lost. Any that was
  already handed on is pushed to `reached` with its row: the correction.
- `handed` (`reach.rs:86-94`) records a lost commit's row.
- `settle` (`reach.rs:131-136`) stops all tracking once no reflog tip is pending and
  nothing is lost.

The stream calls `pulled` at pull time, look-ahead included (`stream.rs:359-361`), and
`handed` at hand-on (`stream.rs:193-199`).

A row reads whether it is lost when it is carried, not when it is laid out
(`Laid::push_to`, `history.rs:458-460`; `Stream::is_lost`, `stream.rs:168-172`). A row an
earlier page already carried is named reached by a later one:

- `take_reached(drawn)` (`stream.rs:176-181`, `reach.rs:104-108`) feeds
  `RowsPage::reached` (`crates/cairn-model/src/rows_page.rs:145`).
- That is called in `read_page` (`history.rs:375-377`) and in `next_page`
  (`session.rs:140-142`).
- `History::append` then clears the `LOST` flag of the row named, checked by number and
  commit (`crates/cairn-model/src/history.rs:396-405`).

Under a topological walk every child is pulled before its parent, so a lost verdict at pull
time is final. `Lost::row`, `reached`, `take_reached`, `RowsPage::reached` and the clearing
in `append` would never fire. The internal code review already called that path "a patch
over a streaming assumption … git itself does not make"
(`docs/research/staging-and-commit/review-code-engine.md:78`).

### 1.6 How a page is produced: the cold path (`history.rs:280-391`)

1. `starting_points` (`history.rs:563-609`) resolves the tips, the decoration, the marking
   and `skip`. `skip` is the cursor's `walked` on a resume, otherwise 0.
2. `open_stream` opens the walk.
3. The assigner is `LaneAssigner::with_window(window).drawn_from(skip)`
   (`history.rs:335`). The first row the page keeps carries a lane snapshot, so the page
   draws without the replayed prefix.
4. The loop (`history.rs:347-371`) pulls one entry, polls cancel first, and reads the
   commit (`Laid::read`, `history.rs:438-446`, the only object decode) only for entries
   inside `skip..target`. It then lays each entry out (`lay_out`, `history.rs:395-403`:
   `push`, or `push_stash` for a stash). Every row the window evicts goes to `Page::take`
   (`history.rs:511-523`), which drops rows before `skip` and pairs the rest with their
   `Laid` by position.
5. At the target the assigner is drained (`into_rows`, `history.rs:372-374`), which makes
   the window's rows final early. That is why a backward line crossing a page boundary on
   the cold path arrives as its lower half only.
6. `next_cursor` (`history.rs:486-500`) pulls one more entry, reading no object, to decide
   whether a cursor is due.

### 1.7 `HistorySession` holds the walk across pages (`session.rs`)

- **State** (`session.rs:16-45`): the borrowed `gix::Repository`, `stream: Option<Stream>`
  (opened by the first page under that page's cancel), the assigner, `ready` (final rows
  already read), `pending` (entries still inside the window, oldest first), tips,
  decoration, marking, order, window, lookahead, skip, counters, and `exhausted`.
- **`next_page`** (`session.rs:104-152`). It opens the stream if needed; a cancelled open
  keeps nothing and the next page opens again (`session.rs:108-123`). It pulls until
  `ready.len() >= limit` (`session.rs:125-131`). Rows become ready only when the window
  evicts them (`place`, `session.rs:214-231`) or at the walk's end (`session.rs:192-200`).
  **So the first page pulls `window + limit` entries.** At the default 1,024 + 64 that is
  1,088 rows laid out and pulled for a 64-row page (`measured.md`, "The first page from
  every ref"). Each later page costs O(limit).
- **Cancellation** (`session.rs:126-130`) returns `Cancelled` and keeps everything done.
  Any other error poisons the session, and the worker drops it (`lane.rs:325-328`).
- **`cursor()`** (`session.rs:155-165`) is where a cold restart resumes: `walked = skip +
  delivered`.
- The session borrows the repository and is not `Send`. It lives in `HistoryLane` on the
  repository thread.

### 1.8 The cursor's cold restart

`HistoryCursor` (`history.rs:131-143`) carries the tips `Arc`, the decoration `Arc`, the
marking, the order, the window, the lookahead and `walked`. `HistoryRequest::resume`
(`history.rs:190-200`) ignores any order, window or lookahead the caller passes; the
cursor's win.

A resumed walk replays from the same tips. It lays out the first `skip` rows for lane
numbering alone, never decoding them (`Page::take`, `session::place`) and drawing them from
a snapshot at `skip` (`drawn_from`). That costs O(skip + limit) per page. On rust that is
60 ms at depth 10k and 1.61 s at 300k (`docs/research/refs-and-status/deep-find-measured.md`,
headline 5).

The worker uses it only after a non-cancel failure dropped the session
(`crates/cairn-app/src/worker/history/lane.rs:181-193`). Replay is correct only if the walk
is deterministic for the same tips and objects. A `Topo` replay would also redo the
in-degree pass first: the whole history without a commit-graph.

### 1.9 Cancellation by epoch

`cancel` is polled:

- once per row laid out (`history.rs:348`, `session.rs:126`)
- once per commit looked ahead at (`stream.rs:310`)
- once per tip read to open, but the first (`walk.rs:88`)
- per reflog and per reflog id (`reflogs.rs:42`, `69`)

On the worker the `Cancel` is the history lane's epoch watch, `answering.epochs.watch(epoch)`
(`lane.rs:180`, `235`). Superseding the lane is what stops the walk. A page cancelled
mid-walk sends nothing and keeps the session (`lane.rs:194-197`). The measured cancel
latency is 0.6-17 µs, with a median poll gap of 6.3 µs (`deep-find-measured.md`, headline 4).

`Topo::build` polls nothing. Without a commit-graph it reads every commit. Cancelling it
would need a `Polled`-style object source (§1.2). As the session stands, a cancelled open
discards everything, so a superseded topological open would lose the whole in-degree pass.

### 1.10 Find-a-ref paging

The sidebar press is `ref_find::press` (`crates/cairn-app/src/history/ref_find.rs:79`).
It looks first among the loaded labelled rows, the stash rows and `HEAD`'s row
(`History::labelled_position`, `crates/cairn-model/src/history.rs:289-305`;
`ref_find.rs:165`). It does not search every loaded row.

When the row is not loaded it asks `Request::FindRow { target, rows: FIND_PAGE_ROWS }`,
with `FIND_PAGE_ROWS = 512` (`ref_find.rs:31`, `197`). The worker serves a find one page at
a time, whenever no other repository-thread job is waiting (`pool.rs:1020-1030`). Each page
goes through `HistoryLane::find_page` (`lane.rs:156-165`) and `answer`. It is the same held
session, answered under the walk lane's number so a superseded find's pages still append
(`lane.rs:205-221`). `ref_find::pages_arrived` (`ref_find.rs:207`) looks through each page
once.

A find's cost is the target's row index × the per-row cost: 2.2-2.4 s to rust's oldest
commit (`docs/systems/history-graph.md`, "Measured" under "From every ref").

### 1.11 Two facts about gix's `Topo` that bear on the design

- **Memory.** `Topo` keeps `indegrees: IdMap<i32>` and `states: IdMap<WalkFlags>` for every
  commit it explores (`gix-traverse-0.61.0/src/commit/mod.rs:26-38`). Without a
  commit-graph that is every commit, held for the session's life. My estimate for rust is
  about 345k × 2 hash-map entries, in the order of 25 MB, beside the rows the window keeps.
  This is NOT measured.
- **Info.** `Topo` yields the same `Info` as `Simple`: `commit_time` is always `Some`, and
  `parent_ids` comes through the object source, so `Grafted` still applies. Shallow
  repositories would get no commit-graph (`walk.rs:70`), so a topological open there always
  walks the whole history.

### 1.12 What this record did not establish

- Whether gix `Topo`'s orders match git's on ties. `DateOrder` uses the same key-only
  `PriorityQueue` (`topo/iter.rs:18-26`). `TopoOrder` is a LIFO `Vec` whose tips are
  stable-sorted by time (`topo/iter.rs:44-48`). The packet must pin both against
  `git rev-list --date-order` and `--topo-order` on a tied-time fixture.
- Whether `Topo` with a commit-graph serves pages in O(page) on the bench. The bench has no
  graph and is read-only, so this needs a scratch clone with one written.

---

## 2. The lane assigner (`crates/cairn-model/src/lane_assignment.rs`)

### 2.1 How it tolerates out-of-order arrival

State (`lane_assignment.rs:8-27`):

- `lanes: Vec<Option<Oid>>`, one slot per lane, each waiting for a commit
- `laid_out: HashMap<Oid, (row, Lane)>` for every row still in the window
- `gone` and `gone_count`, the ids evicted and still remembered
- `rows: VecDeque<WindowRow>`, each row with its edges and its changes
- `first_row`
- `window`, default `DEFAULT_WINDOW = 1024` (`:54`)
- `leaving: LaneState`, the lines crossing into the next row to leave
- `snapshot_every`, `SNAPSHOT_EVERY = 64` (`:57`), clamped to `MAX_SNAPSHOT_EVERY = 4096`
- `drawn_from`

`lay_out` (`lane_assignment.rs:165-243`) works in this order:

1. Evict first (`:167`, `evict_past_the_window`, `:245-256`).
2. Take every lane waiting for this id. The first one becomes the row's lane; otherwise the
   lowest free slot.
3. Push a passing edge for every other occupied lane.
4. Sort each distinct parent into one of four cases:
   - **Awaited.** A lane already waits for it: join that lane (`Starts`).
   - **Already laid out above in the window** (`:214-216`): a late parent. It is
     connected once the row exists, by `connect_upward` (`:307-343`). That finds
     `free_lane_across` the span (`:346-367`, which reads every edge of every row in the
     span) and repaints three things: the parent's row gains `StartsLate { lane, rows,
     order }` and an `out_of_commit` edge marked out of order; each row between gains a
     passing edge marked out of order; the child's row gains an `into_commit` edge marked
     out of order. Only rows still in the window can be repainted, so a row that has left
     never changes (`a_row_that_leaves_the_window_never_changes_again`, `:495`).
   - **Already gone, but still remembered** (`:217-218`): no lane is reserved, so none
     leaks.
   - **Still to come**: reserve a lane, the row's own when it is free.

**What a late parent costs.** It costs one `free_lane_across` scan over the rows between
parent and child: rows × edges, at most the window. It also repaints that many rows. The
derived form keeps one `StartsLate` on the parent's row. Lines crossing a snapshot are
carried in its `late` list (`edge_derivation.rs:12-60`, advanced at `:221-243`). They are
drawn by a merge of crossing and starting late lines (`:276-313`) and dashed in the view
(`crates/cairn-ui/src/history/graph_geometry.rs:131`, `dashed: edge.out_of_order`).

**The blind spot.** A parent can arrive after it has left the window and after its id has
been forgotten. That is beyond `window + remembered()`, with `remembered = window × 16`
(`REMEMBERED_PER_ROW`, `:105-110`), 16,384 ids at the default. Such a parent is
indistinguishable from one still to come. A lane is reserved for it and never freed, and no
line is drawn (`a_parent_beyond_the_remembered_ids_is_placed_anyway_and_costs_one_lane`,
`:608`). How often that fires is not measured (`docs/systems/history-graph.md`, "The blind
spot"). A topological walk never produces it.

**`finalise`** (`:260-268`) runs when a row leaves the window. It turns the row into a
`GraphRow` holding its changes, and a `LaneSnapshot` of `leaving` on every 64th row and on
`drawn_from`. It then advances `leaving`. Window cost was measured as non-linear: 66 ms at
1,024, 749 ms at 4,096 and 23.3 s at 16,384 (systems doc, "It holds a bounded window").

**`push_stash`** (`:161-163`) is `lay_out` with `ParentLines::OwnLane`. A stash never joins
an awaited lane; it takes its own lane down to its base. The stream always pushes a stash
before its base, so the "base already laid out above, joined late" case is only
theoretical (`stream.rs:4-6`; tests at `lane_assignment.rs:701`, `730`, `789`).

### 2.2 Who calls it

- `history.rs:335` (the cold path) and `session.rs:79` (the session).
- The reporters at `history.rs:760`, `history.rs:802`, and `measures_layout_cost_against_lane_count`
  (`lane_assignment.rs:826`).
- Tests: `crates/cairn-model/tests/lane_assignment.rs`, `history_allocations.rs:27`,
  `crates/cairn-model/src/history.rs:1458`, `crates/cairn-ui/tests/history_list.rs:740`,
  `crates/cairn-ui/tests/drawn_lanes.rs:52`, `crates/cairn-app/src/history/state.rs:353`.

### 2.3 What would become dead or simpler if commits arrived child-before-parent

For commit rows, in `lay_out` the second and third parent cases (`:214-218`) would never
fire. It follows that:

- **Never called:** `connect_upward`, `free_lane_across`, `EdgeSegment::marked_out_of_order`
  on the assigner side. `LaneChange::StartsLate` would never be produced.
- **Never non-empty:** `LaneSnapshot::late`, `LateLine`, `advance`'s and `edges`' late
  handling.
- **Never dashed:** the view's dash.
- **Unneeded:** `WindowRow.edges`, which exists only for `free_lane_across`. Also
  `laid_out`, `gone`, `gone_count` and `remembered()`.
- **The window itself.** Rows are held only so a repaint can reach them, so a window of 1
  would give the same rows. That would remove the session's priming: the first page would
  lay out 65 rows rather than 1,088, and `ready`/`pending` would carry one row. It would
  also remove the cold path's early-final half-line (`HistoryPage::rows` doc,
  `history.rs:254-257`).
- **Engine side:** `Reach`'s correction (§1.5).
- **`stream.rs`:** unchanged apart from the order-dependent rules in §1.4.

### 2.4 What must stay

- **The frozen oracle.** `crates/cairn-model/tests/support/layout_before_compaction.rs`
  must never be edited. `the_reference_assigner_is_the_one_compact_rows_were_checked_against`
  (`crates/cairn-model/tests/lane_assignment.rs:676-693`) fingerprints its body as
  `REFERENCE_FINGERPRINT = 0xf6bc_be46_1014_e960`. It is included by path from
  `crates/cairn-git/tests/compact_rows.rs:7` and `crates/cairn-ui/tests/history_list.rs:3`.
- **C15's equivalence tests.** Each requires late lines to exist, so each keeps the
  late-parent path alive:
  - `every_row_draws_the_edges_the_assigner_retained_before_compaction`
    (`lane_assignment.rs:446-488`, tests dir). It runs generated skewed walks fed straight
    to the assigner at windows 1, 2, 3, 5 and 1,024 and snapshot intervals 1, 2, 3, 7 and
    64. It asserts `repainted > 0`, a snapshot carrying a late line, a late start advanced
    through, and the furthest advance.
  - `two_late_lines_cross_a_row_in_their_childs_parent_order` (`:490`).
  - `the_cairn_checkouts_rows_draw_what_the_assigner_drew_before_compaction` and
    `each_cold_page_and_resumed_session_draws_on_its_own_what_the_assigner_drew_for_it`
    (`compact_rows.rs:134-231`). These loop over `[CommitTime, GraphOrder]` and assert
    `checked.repainted > 0`; the comment says "Graph order delivers parents before children
    throughout".
  - `rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew` (`history_list.rs:730-830`).
    Its `braided_walk` swaps every 13th pair to force skew (`:719-721`), and it asserts some
    drawn row is out of order.
- **The assigner's public contract.** "Arrival order cannot break it" is pinned by the
  skew tests in `crates/cairn-model/tests/lane_assignment.rs`:
  - `skewed_history_places_every_commit_and_draws_every_parent_edge` (`:130`)
  - `a_line_to_a_parent_delivered_early_runs_down_a_lane_that_is_free_throughout` (`:166`)
  - `generated_skewed_histories_are_all_placed_and_all_connected` (`:193`)
  - `malformed_input_is_placed_rather_than_rejected` (`:311`)

  It is also pinned by the in-file tests at `lane_assignment.rs:418-687`. `LaneAssigner`
  is `pub` in `cairn-model` and cannot know its caller's order.
- **`GraphOrder` and `CommitTime`.** `GraphOrder` stays non-topological for as long as it
  exists. `CommitTime` is what `recent_messages` must keep, since `git log -n 10` is not
  topological and a topological open would cost about 1.3 s per commit-box read on the
  bench.

So the cheap, safe change is in the walk. A shrunk window for a topological order alone is
optional and touches the priming tests listed in §3.1.F.

---

## 3. Tests, parity oracles, reporters and docs on ordering

### 3.1 Tests, grouped by what a topological walk does to them

**A. These pin today's non-topological order and fail by design.**

- `crates/cairn-git/tests/history.rs`:
  - `a_backward_line_across_a_page_boundary_is_a_gained_segment` (`:287`). Skewed fixture,
    `from_head`; asserts the HEAD walk repaints (`repaints(..) > 0`).
  - `a_row_that_left_the_window_is_never_repainted` (`:426`). Asserts repaints > 0 at
    window 8.
  - `a_session_over_a_skewed_history_returns_every_commit` (`:910`). Oracle
    `git rev-list HEAD` in default order, which puts `shared` above its child `stale`.
  - The `skewed()` fixture's doc (`:1168-1180`) states that newest-first hands over a
    parent first.
- `crates/cairn-git/tests/lost_commits.rs`: `a_parent_dated_after_its_reached_child_is_not_drawn_as_lost`
  (`:391`). It asserts `at(&skewed) < at(&child)`, "the fixture's parent comes off first".

**B. These are ordered parity tests whose oracle is git's order.** They pass under a
`DateOrder` walk, since their dates rise strictly, and need a new oracle under `TopoOrder`.

- `every_row_matches_what_git_reports` (`history.rs:84`) and
  `a_narrow_window_returns_every_commit` (`:397`). Oracle `fixture.rev_list()` =
  `git rev-list HEAD` (`tests/support/fixtures.rs:27-32`) over `braided`, which interleaves
  `main` and `side` by time.
- `a_shallow_clones_boundary_commits_have_the_parents_git_log_shows` (`history.rs:1032`).
  An ordered comparison with `git log --format=%H %P HEAD` (`:987-1009`, `1108-1115`).
- `crates/cairn-git/tests/every_ref.rs`:
  - `a_braided_history_from_every_ref_is_git_rev_lists` (`:797`): `rev-list --date-order`.
  - `a_stash_commit_met_while_looking_ahead_is_a_commits_row_and_no_stashs` (`:921`):
    `--date-order`.
  - `stash_rows_are_in_gits_date_order` (`:970`): `git log --date-order` over the refs and
    the stash commits, plus a stash above its same-second index commit.
- Placement tests that depend on date-falling order:
  - `a_stash_on_a_deep_base_is_drawn_at_its_date_or_directly_above_its_base` (`:695`)
  - `each_stash_on_a_walked_commit_is_one_row_with_one_line_to_it` (`:519`)
  - `looking_ahead_stops_once_the_walk_passes_the_bases_date` (`:1082`), which asserts
    `commits_walked() == newer + 1`. A `Topo::build` would already have read everything,
    so the bound would decide nothing.
  - `in_graph_order_a_stash_is_directly_above_its_base` (`:838`)

**C. These compare sets, not order, and are unaffected.**

- `the_commits_walked_from_every_ref_are_git_rev_lists` (`every_ref.rs:413`, BTreeSet)
- `every_rows_labels_are_what_git_log_decorates_it_with` (`:456`)
- `graph_order_reaches_the_same_commits_as_commit_time_order` (`history.rs:540`)
- `show_lost_commits_draws_what_git_reads_from_every_reflog_entry_and_dims_what_no_ref_reaches`
  (`lost_commits.rs:302`)
- `where_every_reflog_entry_is_reached_the_toggle_draws_the_same_rows` (`:472`)
- `a_commit_graph_dates_the_tips_as_the_objects_do` (`:504`)

**D. These check self-consistency and must keep passing. They are order-agnostic.**

- `a_limit_bounds_the_page_and_a_cursor_continues_it` (`history.rs:153`)
- `moving_a_ref_between_cold_pages_does_not_move_the_walk` (`:190`)
- `two_pages_of_n_match_one_page_of_2n_including_lanes` (`:228`)
- `a_session_returns_what_the_cursor_path_returns_row_for_row` (`:767`)
- `a_walk_paged_any_way_draws_the_rows_one_page_does` (`every_ref.rs:625`)
- `session.rs:267-420` (session unit tests)
- `crates/cairn-app/src/worker/history/tests.rs:65-290`, the find tests. Their oracle is
  `whole_walk` of the Cairn checkout through the same engine.
- `lane.rs:369`, over a linear `WrittenRepository`.

**E. These count cancel polls and are sensitive to any new cost when the walk opens.**

- `opening_a_walk_from_many_refs_is_cancelled_between_tips` (`every_ref.rs:1234`): asserts
  `polls == stop_at + 1`.
- `a_page_cancelled_while_looking_ahead_stops_and_resumes_to_the_same_rows` (`:1040`)
- `cancelling_stops_the_walk_and_keeps_what_it_had` (`session.rs:318`): three polls, then
  asserts `walked > 0`. A polled open would cancel inside the open with `walked: 0`.
- `a_cancelled_query_stops_walking_and_says_where` (`history.rs:352`)
- `cancelling_exactly_at_the_page_boundary_is_still_a_cancellation` (`:718`)
- `cancelling_a_session_stops_the_walk_and_keeps_its_progress` (`:841`)

**F. These are tied to the window's priming and to "walked but never decoded".**

- `paging_a_session_walks_only_what_the_page_needs` (`session.rs:295`)
- `a_session_resumed_from_a_cursor_continues_where_it_stopped` (`session.rs:341`)
- `a_replayed_prefix_is_walked_but_never_decoded` (`history.rs:682`)
- `paging_a_session_costs_the_page_and_not_the_pages_before_it` (`:789`)
- `priming_the_window_walks_but_never_decodes` (`:921`)
- `a_session_names_the_commit_it_cannot_read` (`:948`). It deletes an object with a
  commit-graph present.

**G. These feed walks straight to the assigner or to `Reach` and are independent of the
walk.**

- The assigner tests in §2.4.
- `reach.rs:163`, `181` and `197`. `a_parent_taken_for_lost_before_its_reached_child_is_reached_after_all`
  drives the correction path directly.
- `graph_cell.rs:117` and `graph_geometry.rs:303`, the dashed line.

### 3.2 Parity tests against real git that bear on order

Real git, run in each fixture at test time:

- `rev-list HEAD` (default): `history.rs` via `fixtures::rev_list`.
- `rev-list --date-order --branches --remotes --tags HEAD`: `every_ref.rs:818`, `950`.
- `log --date-order --format=%H …`: `every_ref.rs:1002`.
- `log --format=%H %P HEAD`: the shallow test.
- `rev-list --branches --remotes --tags HEAD` and `--reflog`, as sets: `every_ref.rs:240`,
  `lost_commits.rs:92`, `322`, `335`, `381`, `436`.

No test today compares against `--topo-order` or `--graph`, and none uses tied commit
times.

### 3.3 Measurement reporters (`#[ignore]`d)

Each is driven by `CAIRN_BENCH_REPO`; the bench is `~/Development/bench/rust`.

| Reporter | Where | What it measures |
| --- | --- | --- |
| `measures_both_orders_against_a_named_repository` | `crates/cairn-git/src/history.rs:737` | `walk::open` from `HEAD` in `GraphOrder` and `CommitTime` × object cache 0/4/16/32 MiB × walk only or walk plus lanes, to `CAIRN_BENCH_LIMIT` (50k). Prints whether a commit-graph file is present. The order choice rests on it: 129 ms for `CommitTime` against 196 ms for graph order, both walk plus lanes over 50k. |
| `measures_layout_over_every_ref_of_a_named_repository` | `history.rs:783` | The whole history from every ref's tip in both orders. Segments and open lanes per row (mean, p50, p95, p99, max), out-of-order rows and segments, highest lane, rows with no snapshot, retained bytes. |
| `measures_layout_cost_against_lane_count` | `crates/cairn-model/src/lane_assignment.rs:826` | Assigner cost as lanes grow. |
| `measures_the_first_page_from_every_ref` | `crates/cairn-git/tests/every_ref.rs:1299` | First 64 rows from `HEAD` and from every ref, with and without the snapshot read; median of seven; commits pulled; stashes drawn. On the bench: 7.69 / 7.27 / 8.56 ms, each laying out 1,088 rows. |
| `measures_layout_over_unmerged_refs` | `every_ref.rs:1387` | A synthetic 12k main line with N branches newer than everything. First page 3.12-25.2 ms and whole walk 36-208 ms at 100-5,000 branches. |
| `measures_compact_rows_over_a_named_repository` | `crates/cairn-git/tests/compact_rows.rs:288` | Three modes. `find`: time to the last row paging 64 rows at a time, plus retained bytes. `equivalence`: every row against the frozen assigner; on the bench 345,545 rows, 49,609 repainted, 627 late lines. `derive`: worst-case edge derivation per snapshot interval. |
| `measures_the_first_page_with_show_lost_commits` | `crates/cairn-git/tests/lost_commits.rs:565` | First page with the toggle on, on a tmpfs clone. 0.2-0.3 ms over the plain open. 7.95 ms with 1,000 reflog ids and 49.8 ms with 10,000. The whole walk is 2,184 ms against 2,079 ms. |
| `measures_a_find_through_the_boundary` | `crates/cairn-app/src/worker/history/tests.rs:505` | The whole walk as one page through the worker, then a find to the last row. |
| `window_check` | `crates/cairn-app/src/window_check.rs:458` | "opening" lands the refs, a page of `PAGE_ROWS` rows, the sidebar and the status (`landed`, `:714-720`). On the bench, 35 frames with the slowest 7.5 ms (`measured.md`, phase 09). |
| `writes_check` | `window_check.rs:1023` | Includes Show Lost Commits' toggle to first page drawn: 17.5-18.2 ms. |
| `measures_drawing_a_viewport_over_many_open_lanes_and_a_crowded_rows_chips` | `crates/cairn-ui/tests/drawn_lanes.rs:110` | Draw cost, independent of order. |

The bench is a full clone of rust-lang/rust at `c999cef531e`. It has 340,228 commits from
`HEAD`, 345,545 from every ref and 175 refs, one 1.0 GB pack, and **no commit-graph**
(`.git/objects/info/` is empty, re-checked for this record). It is read-only, metadata
included.

### 3.4 `docs/systems/history-graph.md` on ordering

- "The path a commit takes" (`:33-80`) says a backward line, "a parent drawn above its
  child", is repainted while it is still held, and that across a page or window boundary it
  arrives as its lower half, flagged.
- "The lane assigner" (`:82-128`) says lane indices are final and segments repaintable;
  "Arrival order cannot break it"; the bounded window is 1,024, with costs 66 ms / 749 ms /
  23.3 s; and "The blind spot, carried deliberately", not measured. It also says out-of-order
  rows are "0-0.2% of rows" in Cairn's order over seven small repositories, "most rows" in
  `GraphOrder`. The bench's C15 figure is 14%.
- "The query" (`:356-364`) says: "Order is `HistoryOrder::CommitTime` by default, chosen by
  measurement; neither it nor `GraphOrder` is topological, which is why the assigner must be
  total over arrival order."
- "From every ref" (`:386-456`) gives the stash rule "newest committed first, the walk's own
  order" and its pins against `--date-order`.
- "Show Lost Commits" (`:884-897`) says "The walk yields the newest committed first, so a
  commit's children come off before it" and describes the correction.
- `docs/design/history-graph.md:78-85` gives decision D4's reasoning: "gix offers no
  `--topo-order` equivalent", so the assigner is total and skew deeper than the window is a
  stated blind spot.

### 3.5 What `docs/research/history-graph/*.md` concluded on order and cost

- **`gix-revwalk-ordering.md` (2026-09-14).** gix's `revision::walk::Sorting` has only
  `BreadthFirst`, `ByCommitTime` and `ByCommitTimeCutoff`, and commit time can emit a
  parent first (rebases, imports, skew). `Info` carries `parent_ids`, `generation` (only
  from a commit-graph) and `commit_time`. Three candidates were listed: make the assigner
  total (chosen), sort within a bounded window, or precompute a topological order
  ("wrong for Cairn: it requires walking the whole history before drawing anything"). Left
  open: whether `BreadthFirst` is ever topological, and `ByCommitTime`'s cost without a
  graph. That record looked at `gix::revision::walk` only and missed
  `gix::traverse::commit::Topo` (headline 2).
- **`scroll-memory-model.md`.**
  - Finding 1: `git log --graph` implies `--topo-order`, and git's first row on synthetic
    200k histories costs a whole walk: 337 ms linear and 360 ms branchy `--all`, against
    1 ms for plain log; `--topo-order` is 311/340 ms and `--date-order` 305/337 ms. "Ordering
    is not a streaming operation, it buffers."
  - Finding 2: a commit-graph collapses the first row to 1 ms, takes `rev-list --count`
    from 291 to 32 ms, and takes peak RSS of a `--graph` full walk from 107 to 41 MiB. Its
    implication: "any design that only works with a commit-graph needs a degraded path …
    and any design that ignores the commit-graph is leaving a 300x acceleration on the
    table".
  - Its closing note (`:455-460`) says Cairn does not pay Finding 1's cost because gix had no
    topological walk.
  - Part D measured seven small repositories in the default order at p99 8 segments per
    row, the source of the systems doc's 0-0.2%.
- **`what-clients-show.md`.** Sourcetree's documented default is `git log --graph --all
  --date-order` (Finding 11), with a "Date Order / Ancestor Order" control (Finding 12).
  GitHub Desktop walks `HEAD` with no graph and no order flag (Finding 20).
- Beside these:
  - `docs/research/refs-and-status/fork-deep-history.md:50`, `134`: Fork for Windows runs
    `git log HEAD --branches --remotes --tags --max-count=N --date-order`.
  - `fork-window-survey-2-2026-10-16.md:350-358`: Fork's rows are topological whether
    "Sort commits" is set to "Topologically" or to "By date". By date is `--date-order`,
    which git also keeps topological.
  - The packet's own brief: `docs/work/fork-alignment/brainstorm.md:41-48`. `docs/prd/fork-alignment.md:26`,
    `272` and `503` depend on it.

### 3.6 Measured for this record: git's first 64 rows on the bench, by order

Run with git 2.56.0, `GIT_OPTIONAL_LOCKS=0`, `-c core.commitGraph=true` (no graph exists, so
this changes nothing), `rev-list <order> --max-count=64 --branches --remotes --tags HEAD`,
three runs each, wall clock including process start:

| Order | Runs |
| --- | --- |
| default | 4, 3, 3 ms |
| `--date-order` | 1,247, 1,277, 1,300 ms |
| `--topo-order` | 1,266, 1,266, 1,279 ms |

This is git's own floor for a topological first page on this repository without a graph:
about 175× Cairn's 7.27 ms today and about 6× the C11 bar of 200 ms. A gix `Topo` open would
do comparable work: it reads every commit for its parents and committer time.

---

## 4. The worker side (`crates/cairn-app/src/worker/history/lane.rs`)

### 4.1 How pages are asked, sized and superseded

- **Opening.** A refresh reads the refs on the repository thread (`HistoryLane::refresh`,
  `lane.rs:79-119`) and answers `Update::Refs { reopen }`. `reopen` is true only when the
  snapshot does not `walks_as` the walk's own (`lane.rs:111-114`), so a refresh that changes
  nothing the graph draws never reopens. On reopen the window calls `session::reopen_history`
  (`crates/cairn-app/src/session.rs:458-478`), which does three things:
  - moves the `History` out and replaces it with an empty one sized for its authors;
  - sets `Progress::opening()` (`history/state.rs:29`), which is `Status::Loading`;
  - asks `Request::OpenHistory { rows: PAGE_ROWS, lost }` (`PAGE_ROWS = 64`,
    `crates/cairn-app/src/main.rs:35`), then retires the old history.
- **Routing.** `OpenHistory` is routed (`worker/routing.rs:244`) to bump the history and
  walk lanes and is sent as `Page::Open { rows, walk, lost }` (`worker/pool.rs:657-663`).
  `HistoryLane::page` (`lane.rs:122-140`) calls `replace_walk` (`:145-151`) and then
  `answer` (`:172-203`), which goes to `open` (`:224-292`). `open` uses the last refresh's
  snapshot, or reads the refs itself. On a non-cancel failure from a snapshot that has
  since gone stale it reads the refs again and retries once (`:257-289`).
- **`first_page`** (`:295-311`) builds `HistoryRequest::from_refs` (`.with_lost_commits()`
  when `lost` is set), creates the session, and calls `next_page`.
- **Scrolling.** `HistoryList::on_reach_end` (`crates/cairn-app/src/window.rs:455-469`) asks
  `Request::MoreHistory { rows: PAGE_ROWS }`, but only while no find runs and
  `Progress::wants_more()` holds: not complete, not in flight, not ended (`state.rs:65-67`).
  So at most one page is in flight. `Page::More` is served through the same `answer`.
- **A find** asks 512 rows a page, served between other jobs (§1.10).
- **Superseding.** Every history-lane `submit` bumps that lane's epoch (`pool.rs:623`). The
  walk watches it. A page superseded mid-walk keeps the session and sends nothing
  (`lane.rs:194-197`, pinned by
  `a_page_asked_under_a_superseded_number_walks_nothing_and_the_next_takes_the_walk_up`,
  `:369`). A superseded open also keeps nothing (`session.rs:118-121`), and the next ask
  reopens. Every page is answered under the walk lane's number (`lane.rs:215-218`). Any
  non-cancel failure drops the session, and the next ask cold-restarts from the last good
  cursor (`lane.rs:181-193`, `313-331`).
- **One thread.** The repository thread serves its jobs one at a time (`pool.rs:998` on).
  Those jobs are history pages, refs refreshes, `FilterRefs` (the sidebar filter),
  `Filter` (the Changes tab's filter), `FilterLocalChanges`, `ListRemotes`, `Fetch`'s hand
  off, `CommandLog`, `Retire` and `Close` (`routing.rs:246-278`). Only a history-lane bump
  cancels a running walk.

### 4.2 What a slower first page would do to the window

These follow from the code above, taking a topological open at about git's 1.3 s on the
bench (§3.6):

- **Every open and reopen shows an empty list for the whole open.** That covers the
  application starting, a fetch that moves a ref, a commit, Create Branch, a refresh after
  an outside change, and the Show Lost Commits toggle. `reopen_history` empties the history
  at the ask, and the list shows the opening sentence until the first page arrives. Today
  that gap is 7-18 ms. On the bench it would be about 1.3 s each time. With a commit-graph,
  git's figure suggests a few milliseconds.
- **Jobs behind the open wait for it.** A sidebar filter keystroke, the Changes tab's
  filter, Local Changes' filter, a refs refresh and a retire all wait behind the open on
  the repository thread. A filter keystroke does not supersede it, since it is in a
  different lane.
- **A superseded open throws its work away.** A reopen asked during an open, for example
  two refreshes moving refs back to back, discards the in-degree pass in full, because a
  cancelled open keeps nothing. That only happens if the open polls at all.
  `Topo::build` polls nothing, so without a `Polled` source the walk could not be
  superseded. The window's close would also wait for it: `Request::Close` stops the epochs,
  and the repository thread joins only after its current job (CLAUDE.md, "The UI thread
  never waits").
- **Later pages get cheaper.** Once the in-degree pass is done, `Topo` streams. Pages and
  finds become O(page), and the 1,024-row priming could go (§2.3). A find to rust's oldest
  commit would cost about the open plus the page walk.
- **A cold restart repeats the open.** It redoes the in-degree pass, then replays `walked`
  rows (§1.8).
- **The window check's "opening" figure** (`window_check.rs:458-475`) would carry the open.
  The C11 bar of 200 ms for the first page (`measured.md:80`) would fail on the bench
  without a commit-graph.

---

## 5. Where the order shows beyond row order, and what relies on it

- **Lanes and chip tints.** The assigner gives the leftmost free lane in arrival order, and
  a ref chip is lane-tinted. Fork's survey shows the lane, and with it the chip's colour,
  "follows the row order, not `HEAD`" (`fork-window-survey-2-2026-10-16.md:351-354`). A
  topological order moves lanes and tints, and the dashed out-of-order lines disappear from
  the default walk (`graph_geometry.rs:131`).
- **Comparing two commits.** `selection.rs:134` makes the lower row of the pair the base. In
  a topological order, for related commits, the base is always the ancestor. Today a parent
  drawn above its child makes the child the base.
- **The sidebar find.** It finds the target by paging forward, so its cost is the target's
  row index, and that index changes with the order (`ref_find.rs`). "Not in the graph" is
  known only at the walk's end. The labelled-row lookup (`History::labelled_position`) does
  not depend on order.
- **The REFS row finder** (`crates/cairn-app/src/detail/row_finder.rs:1-30`) makes one pass
  per history and selection, spread over the pages. It does not depend on order.
- **The selection across a reopen** is kept as an id and drawn again when its row arrives
  (`session.rs:450-458`, doc of `reopen_history`). A slower open means the selected row and
  the Commit tab's REFS row are missing for longer. The list's cursor hint
  (`HistoryList::index_of`, `crates/cairn-ui/src/history/list.rs:487-492`) falls back to a
  scan when it misses. Rows only append; no order change makes one arrive above another.
- **Show Lost Commits' dimming.** Today a row can be drawn lost and then cleared by a later
  page (`RowsPage::reached`, `History::append`), so its dimming can change after it is
  drawn. Under a topological walk the first verdict is final.
- **Stash rows.** Placement "at its date, never below its base" relies on the front's date
  falling (§1.4). Under `--date-order` it nearly reproduces git. Under `--topo-order` both
  the rule and its oracle need re-deciding.
- **A parent link in the Commit tab** (`detail/pane.rs` `follow_parent`, found by
  `selection::loaded_row`) always moves down the list under a topological walk. Today it can
  move up.
- **Recent Commit Messages** (`recent_messages.rs:29-34`) uses `walk::open` with `CommitTime`
  to match `git log -n 10`, which is not topological. It must not follow the history onto a
  topological walk.
- **Order no view reads.** `History::serial`, the `RowFinder` and `labelled_position`
  rely on appending only, never on order. No code reads `commit_time` order outside
  `stream.rs`.
