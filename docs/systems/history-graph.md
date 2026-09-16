# History graph

How Cairn draws a repository's history today. As-built: everything here is code
that exists and is pinned by a test named beside it. Intent for this surface
lives in `docs/design/cairn.md` (decisions D3 and D4) and the commitment it was
built against in `docs/prd/history-graph.md`.

What the application does today, end to end: it opens the repository containing
the path named on its command line (or the working directory), walks that
history newest-first on a worker thread, lays each commit into a lane, and draws
the result as a virtualized list of rows with lanes, edges and four columns,
paging as the reader scrolls. Nothing here mutates a repository, and there is no
repository picker.

## The path a commit takes

One commit crosses four crates and changes shape three times.

1. **`gix` hands over a walk entry.** `Repository::history_session`
   (`crates/cairn-git/src/history/session.rs`) opens one gitoxide revision walk
   and keeps it for the life of a scroll. Each step yields an id and its parent
   ids, read from the walk rather than from a decoded object.
2. **`LaneAssigner` turns ids into a picture.**
   `crates/cairn-model/src/lane_assignment.rs` takes `(id, parent_ids)` in walk
   order and produces a `GraphRow`: the commit's `Lane`, and every `EdgeSegment`
   crossing that row. It is pure — no `gix`, no I/O, no clock.
3. **The session pairs the picture with a commit.** Only once the assigner has
   *evicted* a row (so nothing later can repaint it) does the session read that
   commit's object and build a `CommitSummary`. The result is a `HistoryRow`:
   `content: RowContent::Commit(..)` plus `graph: GraphRow`.
4. **The worker hands a page across as owned values.**
   `crates/cairn-app/src/worker/` sends `Update::Rows { rows, complete }` to the
   window; `crates/cairn-ui/src/history_list.rs` draws the rows the viewport
   shows.

Nothing is dropped in transit. `Oid`, parents, subject, author name and email,
and the author timestamp arrive at `CommitRow` as the engine read them; the lane
index and the segments arrive at `graph_geometry` as the assigner emitted them.

## The lane assigner (`cairn-model`)

The vocabulary is `Lane`, `EdgeKind` (`Passing` / `IntoCommit` / `OutOfCommit`),
`EdgeSegment` and `GraphRow`. It is geometric only: nothing in it names
genealogy, so a view draws a row without knowing what the line means.

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
still recognises ids for a further `REMEMBERED_PER_ROW` (16) per row of window,
which is what stops a lane being reserved for a parent that has already gone by.

**The blind spot, carried deliberately.** Beyond `window + remembered` rows of
skew the assigner cannot tell a parent already gone from one still to come. It
is not closable from gitoxide's walk, which keeps its seen-set behind a boxed
iterator with no accessor. Measured on real repositories in the order Cairn
walks, it fires on 0-0.17% of rows.

**Measured shape of a real repository.** The harness
`measures_layout_over_every_ref_of_a_named_repository`
(`crates/cairn-git/src/history.rs`, `#[ignore]`d, driven by `CAIRN_BENCH_REPO`)
runs the real query over every ref. Across three repositories in the default
order: p99 of 8 edge segments per row at the widest, 264 B retained per row, and
single-digit lane counts. Evidence and method:
`docs/research/history-graph/scroll-memory-model.md` Part D. Earlier, much
larger figures in this packet's history described a synthetic fixture and not
any repository; they are retracted there.

## The query (`cairn-git`)

`gix` types never appear in a public signature. Two routes exist, and they
answer identically row for row (`a_session_returns_what_the_cursor_path_returns_row_for_row`):

- **`Repository::history(&HistoryRequest, &impl Cancel) -> HistoryPage`** — the
  cold path. Bounded by a limit, resumable from an opaque `HistoryCursor` that
  replays from pinned tips. Correct, and O(page index x limit), so it is the
  restart route rather than the scroll route.
- **`Repository::history_session(&HistoryRequest) -> HistorySession<'_>`** — the
  scroll path. Holds the walk open, so `next_page(limit, &cancel)` is O(limit)
  after a one-off prime of `window` commits. Pinned by
  `paging_a_session_costs_the_page_and_not_the_pages_before_it`. It borrows the
  repository and is not `Send`, so it lives on the worker that owns that handle.

**A merely-walked commit is never decoded.** `HistoryPage.decoded` never exceeds
the rows returned. Made observable rather than asserted: the fixture writes a
commit-graph and deletes the loose objects of the commits a resumed page
replays, so any stray decode fails to find its object
(`a_replayed_prefix_is_walked_but_never_decoded`, `priming_the_window_walks_but_never_decodes`).

**Cancellation is a poll, once per commit.** `Cancel` is a trait so a test can
stop a walk at a chosen commit; `cairn-git` still knows nothing about threads. A
cancelled cold query reports how far it got and discards the page; a cancelled
*session* keeps its progress, so the next call continues rather than re-walks
(`a_cancelled_query_stops_walking_and_says_where`,
`cancelling_a_session_stops_the_walk_and_keeps_its_progress`).

Order is `HistoryOrder::CommitTime` by default, chosen by measurement; neither
it nor `GraphOrder` is topological, which is why the assigner must be total over
arrival order.

## The worker boundary (`cairn-app`)

`crates/cairn-app/src/worker/` is the only place in the binary that may name
`cairn_git` or `gix`, and the only place that may wait. The partition is by
FILE, and it is a guard, not a convention — see below.

- `worker::open(path) -> (RepositoryHandle, Updates)`. **The worker opens the
  repository**, because discovering one reads the filesystem. A path outside a
  repository therefore arrives as an `Update::Failed` naming the path, not as a
  panic and not as an empty window
  (`opening_a_path_outside_a_repository_is_reported_and_names_the_path`).
- `RepositoryHandle::submit(Request) -> Epoch` returns immediately over an
  unbounded channel and holds no receiving end of anything.
- `Updates::next()` is an `async fn`: it `try_recv`s and otherwise parks on
  `worker/wake.rs`, a one-slot latch a worker sets. There is no timer and no
  async-runtime dependency.
- **The epoch IS the cancel signal.** `Superseded` (`worker/epoch.rs`)
  implements `cairn_git::Cancel` as "is my epoch still current", so superseding
  a request stops its walk at the next commit rather than discarding a finished
  answer (`superseding_a_request_stops_the_walk_that_is_serving_it`). Dropping
  `Updates` stops everything, so closing the window does not wait for a page.
- `WORKERS_PER_REPOSITORY` is 1: the live walk lives on one thread, and a `const`
  assertion fails the build if it is raised, because more workers need a routing
  decision and not a bigger number.

Every `submit` supersedes, and a superseded page delivers nothing — so the
caller must debounce. `Progress::wants_more()`
(`crates/cairn-app/src/history_state.rs`) is that debounce: it is false while a
page is in flight, while the history is complete, and after a failure.

## The view (`cairn-ui`)

`HistoryList` takes the rows as a `State` HANDLE and a per-row builder, so
nothing copies the history to draw it. It renders through
`VirtualScrollView::new_with_data_controlled`, so per-render work is bounded by
the viewport rather than by the history; the only per-render read proportional
to the history is its `len()`. The list is keyed by `RowId`, so a row arriving
above another neither moves the selection nor rebuilds the rows below it.

`RowContent` is read by matching, with no wildcard arm — `crates/cairn-app/src/main.rs`
is the consumer, and because the enum is not `#[non_exhaustive]` the next kind
of row is a compile error there rather than a row silently not drawn.

- **Columns** (Fork's four): graph and subject share the first, then author,
  abbreviated id, and `Date (UTC)`. The date column is labelled UTC because
  `CommitSummary` carries the author time's seconds and not its offset, and
  converting to the reader's local time needs a timezone database, which is a
  dependency decision nobody has taken. `date_text::utc_minutes` is Hinnant's
  `civil_from_days`, total over every `i64`.
- **Lanes are columns; colour is an aid.** `graph_geometry` is pure arithmetic —
  `ROW_HEIGHT` 26 (uniform), `LANE_WIDTH` 14, `MAX_DRAWN_LANES` 24, beyond which
  lanes share the last column. Lane separation is tested against the INK
  (`2 * NODE_RADIUS`, `STROKE_WIDTH`), not against a fraction of `LANE_WIDTH`,
  so the constant cannot decide its own test. `lane_palette` cycles eight
  Okabe-Ito hues, pinned against their published values.
- **An out-of-order line is dashed along its whole length**, in the lane's own
  colour, geometry unchanged — dash rather than colour because colour never
  carries meaning alone, and geometry unchanged because the connection is not a
  different kind of connection. All three `EdgeKind`s of such a line are marked,
  so the dash does not stop halfway.
- **The dash and the hollow merge node are decided against real pixels.**
  `graph_cell`'s tests paint a row onto an offscreen Skia surface and read the
  pixels back — removing the dash or filling the ring each fails a test. Skia is
  already linked through `freya`'s `engine` feature; no test-only dependency was
  added.
- **Loading, empty and failed are three different sentences.**
  `cairn_app::status_text::placeholder` decides which, so the reader never sees
  a blank area that could mean any of the three. A failure that arrives after
  rows are drawn is a banner under them, not a replacement for them.
- **Keyboard**: arrow keys, `PageUp`/`PageDown`, `Home` and `End`, as a pure
  `moved_to(key, current, last)`. The list is auto-focused, so a reader with no
  mouse can select at all. `PREFETCH_ROWS` is 24: every row within that many of
  the end asks for the next page when it becomes visible — every one of them,
  because a single trigger row is a list that quietly stops loading on a tall
  window.

## What enforces this

| Rule | Twin |
| --- | --- |
| `cairn-ui` and `cairn-model` never name `gix` or `cairn_git` | `layers_never_name_the_crates_they_are_sealed_from` |
| Each crate depends only on its allowlist | `layer_dependencies_are_allowlisted` |
| Only `crates/cairn-app/src/worker/` reaches a repository or waits | `the_ui_thread_never_waits_on_repository_work` |
| The history list renders through a virtualizing view | `a_history_sized_list_renders_through_a_virtualizing_view` |

All four live in `crates/cairn-guards/tests/invariants.rs`; each asserts a
nonzero scanned-file count per directory, so a renamed directory reddens rather
than passing on an empty walk. The two matchers behind them
(`cairn_guards::waits_on_work`, and the unbounded-view matcher) each carry a
self-test over synthetic sources, because a matcher that has quietly stopped
matching reports green while the coverage it names is gone.

**What the guards structurally cannot decide**, owned by
`responsiveness-reviewer` (`docs/qa-gate.md`): which THREAD a function runs on,
so the `worker/` functions the UI thread itself calls are exempt from the
matcher; whether an iteration is over a history at all; and whether the
virtualizing view really builds only its viewport.

That last one was measured once, and the measurement is not repeatable without
re-instrumenting: a throwaway build (reverted, never committed) counted builder
invocations per render and found 34-35 rows built per render at 1,000 rows and
the same 34-35 at 100,000, with 60 scroll jumps costing 0.17-0.18 s of CPU at
every size. Pinning it mechanically needs a `freya-testing` headless component
test, which is a dependency addition and therefore a user decision.

## Known limits of what was built

- **Memory is flat in history LENGTH and linear in rows SCROLLED.** A
  100k-commit repository costs nothing until it is scrolled; scrolled rows are
  retained at roughly 660 bytes each and nothing evicts them. Tracked as
  issue #4.
- **There is no random access by row offset.** There is no total row count
  (counting is a full walk) and no way to build a cursor from an offset, so a
  scrollbar drag has no answer; progressive loading is the model, as it is in
  Fork and Sourcetree. Tracked as issue #5.
- **A failed page ends paging for that session.** Deliberate and tested, but
  there is no way back short of reopening; whether it should be retryable is an
  open question for the user.
- **The `HistoryList` keyboard arithmetic is tested; its wiring is not.** That
  `on_select` is called, that the row is revealed, and that `RowRender.selected`
  reaches the row all need a component-test harness that does not exist yet.
- **`GraphRow` still keys a row by `Oid`.** `RowContent` and `RowId` are total
  over rows that are not commits, but the assigner's own output is not — whoever
  lays out the working-tree row meets that first.
- **The first page of a scroll walks `window + limit` commits** before a single
  row can be delivered, because rows leave the assigner only once evicted. Those
  are walk steps, not object reads. Do not shrink the page to make it feel
  faster; shrink the window, and measure.
