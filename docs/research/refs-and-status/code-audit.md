# Code audit: what refs-and-status builds on

Research for the `refs-and-status` packet (packet 4 of `docs/work/daily-loop/roadmap.md`),
recorded 2026-10-05. It is a code-audit record of the working tree at `9b6b7d0`
("diff-engine: show what a commit changed as git answers it, drawn as Fork draws it
(#59)") on `feature/plan-refs-and-status`. Method: the files named below were read, the
parts that matter to this packet in full; gix facts come from the vendored source that
links (`~/.cargo/registry/src/*/gix-0.87.1`, `gix-status-0.34.1`, `gix-index-0.55.0`,
`gix-traverse-0.61.0`), not from memory. Anchors are paths and exported symbols. The
record describes what EXISTS and proposes no design. A statement marked **(inferred)** was
reasoned from the code, not read or reproduced.

## Headlines

- **The engine already walks from many tips.** `HistoryRequest::from_commits(tips, limit)`
  exists, is resolved once per walk into an `Arc<[ObjectId]>` that every cursor and session
  shares, and gitoxide's `Simple` walk drops duplicate tips itself (`state.seen.insert`).
  The only caller in the application is `Scroll::page` in
  `crates/cairn-app/src/worker/pool.rs`, which uses `HistoryRequest::from_head(rows)`.
  Switching is a one-line change at that call, plus an enumeration to feed it. No caller
  outside an `#[ignore]`d bench builds a tip set today.
- **One ref read exists, and it is a change detector, not an enumeration.**
  `Repository::ref_tips() -> BTreeMap<RefName, Oid>` (`crates/cairn-git/src/refs.rs`) peels
  every ref (symbolic and annotated tags followed to the end), has no cancel, and drops the
  tag object's id and every symbolic-ref target. Its only caller is the network lane, which
  runs it before and after each fetch and keeps a single `bool` (`refreshed`). Issues #43
  and #25 (item 3) already name these two unbounded, uncancellable scans as a problem.
- **Rows carry no decoration.** `CommitSummary` holds id, parents, subject, author name,
  email and time. `RowContent` has one production variant, `Commit`. `GraphRow` is keyed by
  `Oid`. Nothing reserves a place for ref labels, a HEAD marker or a stash. The
  history-graph PRD says so on purpose: R6.3 says decoration is "added as **fields**", and
  R6.2 says the working-tree row "is laid out by the engine, not decorated by the view".
- **A second `RowContent` variant breaks compilation in a known, small set of places.**
  The production readers are `crates/cairn-app/src/window.rs` (`history`),
  `crates/cairn-app/src/selection.rs` (`comparison_of`, `summary_of`),
  `crates/cairn-model/src/history.rs` (`HistoryRow::id`) and `crates/cairn-git/src/commit.rs`
  (a test). The guard `every_view_of_a_row_names_every_kind_of_row` keeps any reader from
  dodging with a wildcard. `RowId` is not under that guard; the compiler alone holds it,
  since it is not `#[non_exhaustive]`.
- **Working-tree status does not exist.** No code enumerates changed, untracked, ignored
  or conflicted paths. The diff-engine packet left that out on purpose (PRD R3.6:
  "Which paths are untracked is status's to say"). What does exist is the per-path
  working-tree diff, wired end to end through the worker but never reached from the window:
  `FileTarget::WorkingTree` and `WorkingSide` carry
  `#[expect(dead_code)]` outside tests.
- **gix `status` is compiled in but unused.** The workspace turns on gix's `status`
  feature (`Cargo.toml`). Nothing in `crates/cairn-git/src` calls `Repository::status`,
  `dirwalk` or anything stash-related. gix-status 0.34.1 never runs `core.fsmonitor`. It
  skips entries flagged `FSMONITOR_VALID`, but gix-index 0.55.0 declares that flag and
  never sets it, so gix stats every entry. That is O2's divergence class, seen in the
  source.
- **`git status --porcelain=v2 -z` as a read is already proven not to write.** The test in
  `crates/cairn-git/src/ops/authority.rs` (the one that makes a stale index) runs exactly
  that as a read invocation and requires the index to stay byte-identical. The same
  command as a write rewrites it. The read guards accept `status` as a read verb by policy
  (`reads/mod.rs`: "Query plumbing, or `git status`"). No guard checks the literal
  `"status"`, and issue #54's verb roster would list it.
- **The window has no sidebar and no toolbar beyond a title bar.** `window::window` lays
  out a title bar (name, path, loaded count, Fetch/Cancel button), banners, and a vertical
  `ResizableContainer` of the history list over the detail pane. `docs/design/ui.md` places
  a sidebar of refs on the left, with "Local Changes (N)" as its first entry, and nothing
  occupies that place yet.
- **Adding a query lane touches a fixed set of exhaustive matches.** `QueryLane` is backed
  by a fixed `Arc<[AtomicU64; 4]>` in `Epochs`. `Request::lane`, `routing::route`,
  `routing::unroute`, `routing::thread_of` and `Update::into_retired` all have no default
  arm, so the compiler names every place to touch.

## 1. The history query

### 1.1 Public surface (`crates/cairn-git/src/history.rs`, `history/session.rs`, `history/walk.rs`)

Re-exported from `cairn_git`: `HistoryCursor`, `HistoryOrder`, `HistoryPage`,
`HistoryRequest`, `HistorySession`.

- `HistoryRequest` has a private `start: Start`, where `Start` is `Head`,
  `Commits(Vec<Oid>)` or `Resume(HistoryCursor)`. It also holds `order`, `limit` and
  `window`. Constructors:
  - `HistoryRequest::from_head(limit)`
  - `HistoryRequest::from_commits(tips: impl IntoIterator<Item = Oid>, limit)`
  - `HistoryRequest::resume(cursor, limit)`, plus `with_order(HistoryOrder)` and
    `with_window(usize)`. Both are ignored when resuming.
- `HistoryOrder` is `CommitTime` (the default, `Sorting::ByCommitTime(NewestFirst)`) or
  `GraphOrder` (`BreadthFirst`). The doc comment says neither order is topological.
- `HistoryCursor` is opaque. It holds `tips: Arc<[gix::hash::ObjectId]>` ("Resolved once,
  when the walk began; every page of it starts here, wherever refs are now"), `order`,
  `window` and `walked`. `rows_behind()` is its one accessor.
- `HistoryPage { rows: Vec<HistoryRow>, cursor: Option<HistoryCursor>, walked, decoded }`.
- `Repository::history(&request, &cancel)` is the cold route. It replays the walk, so page
  *k* costs `k × limit`.
- `Repository::history_session(&request) -> HistorySession<'_>` is the warm route.
  `HistorySession::next_page(limit, &cancel)` is O(limit) per page. The session borrows the
  repository, is not `Send`, and holds the gix walk, a `LaneAssigner`, a `ready` queue and a
  `pending` queue of `(Oid, Vec<Oid>)`. Its accessors are `cursor()`, `delivered()` and
  `is_exhausted()`.

### 1.2 How the walk is seeded

`starting_points` resolves the tips:

- `Start::Head` runs `repo.head()?.try_peel_to_id()?`. An unborn `HEAD` becomes
  `Error::UnbornHead`, which the worker turns into an empty, complete page
  (`pool.rs::no_walk`).
- `Start::Commits(tips)` runs `walk_tips`, which converts each `Oid` and refuses a hash
  kind that differs from the repository's (`Error::Walk`). It does **not** check that a tip
  is a commit, and it does not deduplicate.
- `Start::Resume(cursor)` reuses `Arc::clone(&cursor.tips)`, pinned by the test
  `resuming_reuses_the_cursors_resolved_tips`.

`walk::open` builds `gix::traverse::commit::Simple::new(tips, Grafted { .. })` with the
order's sorting. It uses a commit-graph unless the repository is shallow. `Grafted` hides
the parents of a shallow boundary commit, as git's graft does. Read in
`gix-traverse-0.61.0/src/commit/simple.rs` (`Simple::filtered`): tips are inserted through
`state.seen.insert(tip)`, so a duplicate tip is dropped. **(Inferred)** A tip that peels to
a tree or a blob, which a tag can name, would fail the walk when it is read as a commit.
The bench helper `every_ref_tip` (in `history.rs` tests) filters with `find_object` to
`Kind::Commit` and deduplicates before it calls `from_commits`, so that is the shape a
caller needs.

**Measured already.** `measures_layout_over_every_ref_of_a_named_repository` (an
`#[ignore]`d reporter, `CAIRN_BENCH_REPO`) seeds `from_commits` with every ref that peels
to a commit. `docs/research/history-graph/scroll-memory-model.md` Finding 24 records the
layout over every ref on seven real repositories: p99 2 to 8 segments per row, at most 166
distinct tips (strata). It does not measure the walk-seeding cost from thousands of tips.
`docs/systems/history-graph.md` ("Known limits") says so: "a walk from thousands of tips
costs what gitoxide's walk costs to seed from them — measured by nothing yet."

### 1.3 Rows (`crates/cairn-model/src/history.rs`, `graph.rs`, `lib.rs`)

```rust
pub struct CommitSummary { pub id: Oid, pub parents: Vec<Oid>, pub summary: String,
    pub author_name: String, pub author_email: String, pub author_time: i64 }

/// Not `#[non_exhaustive]`: consumers match every variant, with no wildcard arm.
pub enum RowContent { Commit(CommitSummary), #[cfg(test)] NotACommit }
pub enum RowId { Commit(Oid), #[cfg(test)] NotACommit }
pub struct HistoryRow { pub content: RowContent, pub graph: GraphRow }
pub struct GraphRow { pub id: Oid, pub lane: Lane, pub edges: Vec<EdgeSegment> }
```

None of these has a ref field, a HEAD flag, or a reserved place for decoration.
`HistoryRow::id()` takes the identity from `content`, not from `graph`. The doc says
"Nothing enforces that `content` and `graph` describe the same entry". The test-only
`NotACommit` variants exist so that model tests can prove the shape admits a second kind.
`docs/systems/history-graph.md` names the residual: "**`GraphRow` still keys a row by
`Oid`.** `RowContent` and `RowId` are total over rows that are not commits, but the
assigner's own output is not — whoever lays out the working-tree row meets that first."

### 1.4 The lane assigner (`crates/cairn-model/src/lane_assignment.rs`)

`LaneAssigner::push(id: Oid, parents: Vec<Oid>) -> Option<GraphRow>`. Parents are in
git's order. It returns the row this push forced out of the window.
`DEFAULT_WINDOW = 1024`. `into_rows()` flushes. Its input is purely `(Oid, parents)`.
Rows leave the assigner only when the window evicts them, so the first page walks
`window + limit` commits (systems doc, "Known limits"). In both routes the walk pushes
each commit's id and parents into the assigner, and `summary_of_commit` decodes only the
rows past the replayed prefix.

### 1.5 What would change (inferred, for the planner)

- **Many tips.** Swap `HistoryRequest::from_head(rows)` in `Scroll::page` (`pool.rs`) for
  `from_commits(tips, rows)`. The tips need enumerating on the repository thread (the walk
  owner), peeled to commits and filtered. `HEAD` must be among them, even when detached.
  `Page::Open` already drops the session and cursor, which honours `Invalidated::refs`
  (`ops/mod.rs` docs). The cold cursor keeps its own pinned tips.
- **A stash row.** A stash is a commit: its first parent is `HEAD` at the time, its second
  the index commit, and its third the untracked-files commit when there is one (Fork shows
  three parent links for that case, `docs/research/diff-engine/fork-detail-and-diff-ui.md`
  Finding 3). Seeding the walk from `refs/stash`, or from its reflog entries, would bring
  the index and untracked commits in as rows **(inferred)**. VS Code Git Graph instead
  seeds `<stash bases>` (`docs/research/history-graph/what-clients-show.md`). `ref_tips`
  includes `refs/stash` today (its top entry only). `stash@{n}` is the reflog of
  `refs/stash`. gix exposes `Reference::log_iter()` (`gix-0.87.1/src/reference/log.rs`),
  which nothing in Cairn calls.
- **An "uncommitted changes" row.** It needs a third `RowContent` and `RowId` variant. It
  has no `Oid`, but `GraphRow.id` and `LaneAssigner::push` both need one (the residual
  above). R6.2 says it occupies a lane, so the engine lays it out, which means seeding the
  assigner with it before `HEAD`.
- **Rows that arrive above another.** `cairn_ui::HistoryList` tracks selection by `RowId`
  (`selected(Option<RowId>)`: "survives rows arriving above it"). `index_of(rows, id,
  hint)` checks a cursor hint and falls back to `rows.iter().position(..)`. Root
  `CLAUDE.md` (virtualization residuals) says this fallback is "unreachable while rows only
  append; the row that arrives ABOVE another is what enters it, which is what the
  working-tree row will do". `selection::loaded_row` and `selection::extend` also scan
  every loaded row once per press.

## 2. The history list UI (`crates/cairn-ui`)

- **`HistoryList`** (`src/history_list.rs`) virtualizes through
  `VirtualScrollView::new_with_data_controlled(data, build_row, controller)` with
  `.item_size(ROW_HEIGHT)`. It takes `rows: State<Vec<HistoryRow>>` and a `row` callback
  `Fn(RowRender) -> Element`, where `RowRender { row: HistoryRow, selected: bool, lanes:
  usize }`. Its builders are `lanes`, `selected(Option<RowId>)`,
  `also_selected(Option<RowId>)` (the second commit of a comparison), `held(HeldKeys)`,
  `on_select(RowId)`, `on_extend((RowId, usize))`, `on_reach_end(())` and `controller`.
  `PREFETCH_ROWS = 24`, so every row in the last 24 asks for more through `on_visible`.
  Each row is keyed by identity: `rect().key(id)`, with the comment "a positional key lets
  a reused slot paint the previous row's graph". The arrow keys, PageUp/PageDown and
  Home/End move the selection (`moved_to`). `reveal_row` scrolls by offset.
- **`CommitRow`** (`src/commit_row.rs`) draws a horizontal flex row with four columns:
  1. a flex cell holding `graph_cell(&graph, commit.parents.len(), lanes)` and then the
     subject label (`commit.summary`, one line, ellipsis);
  2. the author, `AUTHOR_WIDTH = 150`;
  3. the short id, `ID_WIDTH = 72`;
  4. the date, `DATE_WIDTH = 132`, as `date_text::utc_minutes(author_time)`.

  `HistoryHeader` captions the columns "Graph and subject", "Author", "Commit" and
  "Date (UTC)". There is **no ref badge, chip or HEAD marker anywhere**: a grep for
  `HEAD`, `chip` and `badge` over `commit_row.rs` and `graph_cell.rs` finds nothing.
  `docs/design/history-graph.md` names the column as "subject with ref badges", and
  `docs/design/ui.md` gives "One accent ... for HEAD, selection and the primary action".
  **(Inferred)** Badges belong in the flex cell, between `graph_cell` and the subject
  label.
- **The window's row callback** (`crates/cairn-app/src/window.rs`, `history`) is the one
  production match over `RowContent`:

  ```rust
  // No wildcard arm: a new row kind must fail to compile here.
  match render.row.content {
      RowContent::Commit(commit) => CommitRow::new(commit, render.row.graph, render.lanes)
          .selected(render.selected).into(),
  }
  ```

- **Selection** (`crates/cairn-app/src/selection.rs`): `choose(id, view, submit)` sets
  `selected` and asks `comparison_of(id)`, which is `RowId::Commit(oid) =>
  Comparison::Commit(oid)`. Its doc reads: "a row that is not a commit — the working tree's,
  when it lands — must say here what it compares, or the window does not compile."
  `extend` builds a `Pair { base, tip }` of `CommitSummary`s through `summary_of(row)`,
  which also names every variant.

## 3. The Commit tab and the place for ref chips

`crates/cairn-ui/src/commit_tab.rs` (`CommitTab`) is one virtualized list of equal rows
(`DETAIL_ROW_HEIGHT = 24`). `header_lines(changes)` builds a `Vec<Line>` from the
`CommitDetails`:

```rust
enum Line { Captions, People { author, committer }, Dates { author, committer },
    Id(String), Parents(Vec<Oid>), Rule, Subject(String), Body(String), CutShort(usize),
    NoFiles, FilesBar }
```

The caption column carries `ID_CAPTION = "SHA"` and `PARENTS_CAPTION = "PARENTS"`, with
`CAPTION_WIDTH = 72`. The module doc says: "No avatar and no ref chips (L9)". The built
header is cached per commit through `HeaderKey { commit: Option<Oid>, needed_limit,
no_files }` ("one commit's header is built once"). **(Inferred)** Chips that change when
refs move without the commit changing would need their refs in that key.

What the documents say about the place:

- `docs/prd/diff-engine.md` R5.3: "The tab shows no avatar and makes no network call; ref
  chips wait for packet 4's ref enumeration." Its out-of-scope list includes "ref chips on
  the Commit tab (packet 4)".
- `docs/design/diff.md` ("The detail pane"): "**Commit** shows author and committer with
  full timestamps, the full commit id, parents as links, the message and chips for the
  refs pointing at the commit".
- `docs/systems/diff.md`: "No avatar, no ref chips, no network call."
- Fork's arrangement (`docs/research/diff-engine/fork-detail-and-diff-ui.md` Finding 3):
  "grey upper-case labels right-aligned against their values: `REFS` (branch and tag
  chips, drawn like the graph's labels), `SHA` ..., `PARENTS` ...". So the place is a
  `REFS` caption row above `Line::Id`.

No code reserves it: there is no `Line::Refs`, no constant and no field. The "place" is a
documented intent, not a stub. The data source is also missing: `CommitDetails` (id,
parents, author, committer, message) and `ChangeSet.details` carry no refs.

## 4. The worker

### 4.1 Threads (`crates/cairn-app/src/worker/pool.rs`, `routing.rs`)

`worker::open(path, &Discovery)` returns `(RepositoryHandle, Updates, Replier)` and
starts `cairn-repository`, which in turn starts three more threads (`Threads::start`):

| Thread | Holds | Serves |
| --- | --- | --- |
| `cairn-repository` | `shared.to_worker()` taken once in `serve`, never reopened; `Scroll { session: Option<HistorySession>, cursor: Option<HistoryCursor> }` | history lane, `ListRemotes`, `CommandLog`, `FilterFiles`, `Retire`, `Close`; forwards `Fetch` to the network lane |
| `cairn-diff` (`diff_lane.rs`) | its own handle, reopened by `SharedRepository::reopen_for` when a configuration file moves; a `DiffSession` and kept `Answers` | changes lane, file-diff lane (commits, comparisons and working tree), `ConfiguredContext` |
| `cairn-network` (`network_lane.rs`) | its own `shared.to_worker()` | `Operation::Fetch`, one at a time; a second is refused with a `Refusal` |
| `cairn-askpass` | the helper's channel | credential prompts |

The routing table is `routing::route(Request) -> Routed`. `Routed` is one of
`Repository(RepositoryJob)`, `Diff(DiffQuery)`, `ConfiguredContext` or `CancelFetch`. It
is applied on the caller's thread as the request is submitted
(`RepositoryHandle::submit`), so neither worker thread forwards the other's work.
`RepositoryJob` is `History(Page::Open|More)`, `ListRemotes`, `Fetch`, `CommandLog`,
`Filter`, `Retire` or `Close`.

### 4.2 Lanes and epochs (`epoch.rs`)

```rust
pub enum QueryLane { History, Changes, FileDiff, FileFilter }
pub struct Epochs { lanes: Arc<[AtomicU64; 4]>, stopping: Arc<AtomicBool> }
```

`QueryLane::supersedes()` maps `Changes` to `[Changes, FileDiff]` and every other lane to
itself. `Epochs::watch(epoch)` gives the `Superseded` that the engine polls as a
`cairn_git::Cancel`: once per commit on a walk, and every runner tick while a `git` read
runs. Operations carry no epoch.

### 4.3 Requests and updates (`request.rs`)

The `Request` variants are `OpenHistory { rows }`, `MoreHistory { rows }`, `Changes { of }`,
`FileDiff(FileQuery)`, `Expand(ExpandQuery)`, `FilterFiles { of, files, text }`,
`ListRemotes`, `ConfiguredContext`, `Fetch { remote }`, `CancelFetch`, `CommandLog`,
`Retire(Retired)` and `Close`. `Request::lane()` has no default arm.

The `Update` variants are `Rows { rows, complete }`, `Failed`, `WorkerLost`,
`Remotes { remotes }`, `ConfiguredContext`, `FetchStarted`, `FetchProgress`,
`FetchFinished { remote, refreshed }`, `FetchCancelled { .., refreshed, stranded_locks }`,
`FetchFailed { .., refreshed, .. }`, `FetchRefused`, `Prompt`, `CommandLog`,
`Changes { of, changes }`, `FileDiff { query, diff }`, `FilteredFiles`, `Expanded`,
`DiffFailed` and `Superseded(Retired)`. `Update::into_retired()` has no default arm. A
superseded large answer goes back to a worker to be freed. `Retired` carries only
`Option<Arc<ChangeSet>>` and `Vec<ShownDiff>`.

The working-tree targets exist but are not reached from the window:

```rust
#[cfg_attr(not(test), expect(dead_code, reason =
    "the window selects a commit, a file or the working tree from phase 05 on"))]
pub enum WorkingSide { Staged, Unstaged, Untracked }
pub enum FileTarget { Committed { of: Comparison, file: ChangedFile },
                      WorkingTree { path: RepoPath, side: WorkingSide } }
```

`diff_lane.rs` serves `FileTarget::WorkingTree` through `DiffSession::working_tree_diff`
(`which(side)` maps `WorkingSide` to `WorkingTreeDiff`). Its doc says "A working-tree
answer is never kept" (pinned by `a_working_tree_answer_is_never_kept`), and
`a_working_tree_diff_answers_through_the_boundary` (`worker/diff_tests.rs`) drives it.
`Comparison` is `Commit(Oid)` or `Between { old, new }`. Nothing compares the working tree.

### 4.4 Adding a query lane (inferred from the exhaustive matches)

A `Refs` or `Status` lane means adding a `QueryLane` variant, growing the array in
`Epochs` (and `QueryLane::ALL` and `index()`), giving it a `supersedes()` row, and adding
an arm to `Request::lane`, `routing::route`, `unroute`, `thread_of` and
`Update::into_retired`. The tests `every_query_is_numbered_in_its_lane_and_operations_in_none`
and the routing tests (`every_request()`) enumerate the variants, and the compiler forces
the rest.

Where each lane runs is a design choice. Refs feed the walk's tips, and the walk lives on
`cairn-repository`, so an enumeration that only seeds the walk would run there. A status
read is a `git` or gix read that can take seconds on a dirty monorepo, and on
`cairn-repository` it would queue behind a page (and a page behind it). The diff thread
serves the working tree already, and `docs/design/concurrency.md` says diffs ("commit,
comparison and working-tree") have their thread "so a long history page never queues a
diff behind it". Large answers (a status of an untracked build directory, tens of
thousands of tags) would need `into_retired` handling, since `Updates::next` drops a stale
answer on the UI thread otherwise.

### 4.5 What happens after a fetch (`network_lane.rs`, `session.rs`)

```rust
let before = repo.ref_tips().ok();
let outcome = fetch(git, &repo, &remote, token)...;
let refreshed = match (before, repo.ref_tips().ok()) {
    (Some(before), Some(after)) => before != after,
    _ => true,   // "Could not tell: assume the worst, which costs a reload."
};
```

Only `refreshed: bool` crosses to the window, and both maps are dropped on the network
thread. The fetch itself returns a `Performed` whose `Invalidated { refs, index, objects,
working_tree }` declares `refs` and `objects`. The worker ignores that declaration and
compares tips instead (issue #25, item 5). On the window side,
`session.rs::reload_if(refreshed, rows, progress, worker)` runs `rows.write().clear()` on
the UI thread (issue #52), sets `Progress::opening()` and submits `Request::OpenHistory`.
The repository thread's `Page::Open` drops the session and cursor, with the comment "After
a fetch this is also what honours `Invalidated::refs`: the new walk starts from the refs as
they are now". **After a fetch no refs information exists anywhere but that `bool`.** A
refs view would need its own re-read, or the network lane would need to send the
after-map. `CLOSE_PATIENCE` (5 s) in `pool.rs` documents that it "does not bound the
network lane's ref scans".

Startup (`main.rs`, `use_hook`) submits `ListRemotes`, `ConfiguredContext` and
`OpenHistory { rows: PAGE_ROWS }` (`PAGE_ROWS = 64`).

## 5. The engine (`crates/cairn-git`)

### 5.1 Public surface (`src/lib.rs`)

```rust
pub use cancel::{Cancel, CancelSignal};
pub use diff::{ChangesRequest, ContentOptions, DiffInputs, DiffSession, LineBudget, Offered,
    PAGE_FILES, PAGE_LINES, Page, StagedInputs, WorkingTreeDiff};
pub use error::{Error, RefusedWrite};
pub use history::{HistoryCursor, HistoryOrder, HistoryPage, HistoryRequest, HistorySession};
pub use repository::{CLOSE_BOUND, Repository, SharedRepository};
pub mod ops;   // fetch, FetchCancel, FetchInProgress, Askpass, GitBinary, GitEnvironment,
               // GitVersion, Invalidated, Performed, describe_destructive
```

These are the inherent `Repository` methods that matter here: `history`,
`history_session`, `ref_tips` (`src/refs.rs`), `remotes` (`src/remotes.rs`, which returns
`Vec<RemoteSummary>` with the default remote first and a URL password stripped),
`commit_details` (`src/commit.rs`), `diff_session`, `changes`, `working_tree_diff`,
`head_id` (`src/diff/inputs.rs`, `Option<Oid>`), `staged_inputs`, `diff_inputs`,
`git_dir` and `workdir`. `inner()` (the `gix::Repository`) is `pub(crate)`.

### 5.2 Opening (`src/repository.rs`)

`SharedRepository::discover_for(path, &GitBinary, env)` checks `safe.bareRepository` and
ownership as the git in use would, then opens a `gix::ThreadSafeRepository` at full trust
with `gitoxide.objects.allocLimitIfReducedTrust=0`, and refuses a `.git` file that was
swapped between the check and the open. `to_worker()` gives each thread a handle with a
4 MiB object cache. `reopen_for` opens afresh from the same path (the diff thread's
configuration refresh). The comment there says no second open is left to gix's own trust
rule, which is why fetch's refspec check asks git.

### 5.3 Everything that reads refs today

| Where | What | Notes |
| --- | --- | --- |
| `history.rs::starting_points` | `repo.head()?.try_peel_to_id()` | `HEAD` for `from_head`; `Error::UnbornHead` |
| `refs.rs::Repository::ref_tips` | `references()?.all()?`, then `peel_to_id()` on each, into a `BTreeMap<RefName, Oid>` | no cancel; peels annotated tags (gix doc: "peel all annotated tags to their first non-tag target"), so tag object ids are lost; a ref that fails to peel is skipped; the keys are full names, all under `refs/` per its test; includes `refs/stash` and `refs/remotes/*/HEAD` (**inferred** from `all()`) |
| `diff/inputs.rs::Repository::head_id` | `inner().head_id()` | `Option<Oid>`; staged-input freshness |
| `diff/working_tree.rs::head_side` | `repo.head()?.try_peel_to_id()` | the staged diff's base, or the empty tree when unborn |
| `history.rs` tests, `every_ref_tip` | `references().all().peeled()`, filtered to commits and deduplicated | bench only |
| `ops/refspec_policy.rs` | **no refs**: it reads `remote.<name>.*` configuration through `reads::fetch_settings` (`git config` in query form) and `$GIT_DIR/remotes/<name>` files | — |

Nothing reads `HEAD`'s symbolic target (the branch name), whether `HEAD` is detached,
upstream tracking or ahead/behind, the stash reflog, or the worktrees list. gix has the
iterators `references()?.local_branches()`, `.remote_branches()`, `.tags()`, `.prefixed()`
and `.pseudo()` (`gix-0.87.1/src/reference/iter.rs`) and `Reference::log_iter()`, and none
of them is called.

### 5.4 The working-tree query (`src/reads/working_tree.rs`, `src/diff/working_tree.rs`, `src/diff.rs`)

The public entry point is `DiffSession::working_tree_diff(git, path: &RepoPath, which:
WorkingTreeDiff, options: &ContentOptions, cancel) -> Result<Option<FileDiff>, Error>`
(also on `Repository`). `pub enum WorkingTreeDiff { Staged, Unstaged, Untracked }`. Its
doc reads: "Which files are untracked is status's to say (R3.6); this answers the one
named, whatever the index holds."

Underneath sits `reads::working_tree_patch(git, repo, &WorkingTreeQuery, cancel) ->
Result<WorkingTreeAnswer, Error>`, which is crate-private:

```rust
pub(crate) enum Side<'a> { Staged { commit: &'a Oid }, Unstaged, Untracked }
pub(crate) struct WorkingTreeQuery<'a> { side, path: &'a RepoPath, context: u32,
    algorithm: Option<Algorithm>, ignore_whitespace: bool,
    ignore_submodules: Option<&'static str>, raw_only: bool, ceiling: usize }
pub(crate) enum WorkingTreeAnswer { Unlisted, Listed { file: ChangedFile, sections },
    PastCeiling { file: Option<ChangedFile> } }
```

It runs `git diff-index --cached --no-renames <commit>`, `git diff-files --no-renames`
(both with the pathspec `:(literal)<path> :(exclude,glob)<path>/**`) or
`git diff --no-index -- /dev/null <path>`. Before git runs, `diff/working_tree.rs` reads
the index **fresh** through gix (`fresh_index`). That pass answers `Conflicted` for any
entry at a stage other than `Unconflicted`, `Unsupported` for a sparse index, and handles
intent-to-add, blob sizes against R2.6, and a bare repository (`Unsupported`).
**Nothing enumerates changed paths.** The one-path query is the whole of it, and
`raw_only` is the cheapest way to ask about a single path.

### 5.5 gix status, dirwalk and features

- Root `Cargo.toml`: `gix = { version = "0.87.1", features = ["blame", "blob-diff",
  "revision", "status", "max-performance", "parallel", "sha256"] }`, with default features
  left on. gix's `status` feature pulls in `gix-status`, `dirwalk` (`gix-dir`, attributes,
  excludes), `index`, `blob-diff` and `gix-diff/index`. The lockfile pins `gix-status`
  0.34.1 and `gix-dir` 0.29.1.
- No production code calls `Repository::status`, `dirwalk` or `dirwalk_iter`.
- What gix-status does, as read in the source:
  - `Repository::status(progress)` defaults to "most similar to `git status
    --ignored=no`", honours `status.showUntrackedFiles` ("untracked files being collapsed
    by default"), and states a deviation: git runs the index check before the dirwalk,
    gix runs both in parallel.
  - `index_as_worktree/function.rs` skips entries whose flags intersect `UPTODATE |
    SKIP_WORKTREE | ASSUME_VALID | FSMONITOR_VALID`. gix-index 0.55.0 declares
    `FSMONITOR_VALID` (`entry/flags.rs`) and has no other occurrence: the FSMN extension
    never sets it. gix-status has no other fsmonitor code. **(Inferred)** gix status
    neither runs the `core.fsmonitor` hook or daemon, nor benefits from it. Every tracked
    file is stat'ed. Its answer does not depend on the hook, but its cost does, and that
    is a difference from git that the user can see.
  - `status::iter::Outcome::write_changes()` writes the refreshed index back. It is on the
    guard's mutation roster (`GITOXIDE_MUTATION_IDENTS`, "The index, written back after a
    status"), so calling it outside `ops/` fails `only_the_ops_module_mutates_a_repository`.
- `git status --porcelain=v2 -z` as a read: the test in `ops/authority.rs` runs
  `read_invocation().in_repository(&handle).args(["status", "--porcelain=v2",
  "-z"]).collected()` on a stale index with an edit and an untracked file. It requires the
  records to show `edited` and `? untracked`, the index to stay byte-identical, and no
  `index.lock`. The same command built as a write rewrites the index. The runner offers
  `.start()?.records(cancel, on_record, on_stderr)` for streaming `-z` records
  (`process/runner.rs`), which is what `reads::changes` uses.
- Measured for O2 so far (`docs/research/process-manager/consumer-invocations.md`, S1): a
  clean status on the 62,892-entry bench took 29.5 ms median. A dirty tree is unmeasured,
  and that is packet 4's L6 bar ("status on a large, dirty working tree").

## 6. `cairn-model` types that matter here

- **`Oid`** (`src/oid.rs`): `parse(hex)`, `from_bytes(digest)`, `as_bytes`, `hex()`,
  `short()` (an `OidHex`). It covers SHA-1 and SHA-256.
- **`RefName`** (`src/lib.rs`): `pub struct RefName(String)`, derives `Ord` and `Hash`.
  `RefName::new(full_name)` does **no validation**. `as_str()`, and `shorthand()`, which
  strips `refs/heads/`, `refs/remotes/` and `refs/tags/` and leaves anything else whole
  (`refs/stash` stays `refs/stash`; `HEAD` stays `HEAD`). It is a `String`, so a non-UTF-8
  ref name is lossy: `ref_tips` builds it from `reference.name().as_bstr().to_string()`.
  Contrast `RepoPath`, which keeps bytes.
- **`RemoteSummary { name: String, url: Option<String> }`** (`src/remote.rs`).
- **`ChangeSet { files: Vec<ChangedFile>, details: Option<CommitDetails>, renames:
  RenameDetection }`** (`src/change_set.rs`), with `files_matching(text, keep_going)` (the
  filter, run on a worker).
- **`ChangedFile { status: ChangeStatus, old_path, new_path: RepoPath, old_mode, new_mode:
  Option<FileMode>, old_id, new_id: Option<Oid> }`**, where `ChangeStatus` is `Added`,
  `Deleted`, `Modified`, `TypeChanged`, `Renamed(Similarity)` or `Copied(Similarity)`.
  There is no `Untracked`, `Ignored`, `Conflicted` or staged/unstaged notion.
- **`DiffContent`** has a `Conflicted` state (returned by the working-tree query), and is
  guarded by `every_view_of_a_file_diff_names_every_state`.
- **`CommitDetails`** (id, parents, author, committer as `Signature`, message) has no refs.
- There is **no status-shaped type**: nothing for a working-tree entry, an index/worktree
  pair, an untracked or ignored path, or a conflict stage, and nothing for a ref kind
  (branch, remote, tag, stash), `HEAD`, upstream or ahead/behind.

## 7. The app shell

`crates/cairn-app/src/main.rs` builds one window (`WindowConfig::new(..).with_title("Cairn")`)
and keeps the view state in hooks: `rows`, `progress`, `selected`, `fetch`, `prompt`,
`remotes`, `refused`, `diff`, `history_scroll`, `detail_tab`, `pane_collapsed`,
`pane_height`, `diff_settings`, `diff_scroll`, `change_cursor`, `filter_text`,
`changes_list_width`, `pair` and `held_keys`. They are gathered into `window::View`. There
is one repository, named on the command line (`repository_path::chosen`).

`window::window` lays out, top to bottom:

1. `title_bar`: "Cairn", the path, the loaded count, and `fetch_button` ("Fetch
   <default remote>", or "Cancel"). This is the only toolbar.
2. Optional banners: the fetch line and a refusal.
3. `split(list, DetailPane, view)`: a vertical `ResizableContainer` with the history list
   (`HistoryHeader` plus `HistoryList`, or a `notice`) over the detail pane at
   `PANE_HEIGHT = 260`. It collapses to a `DETAIL_STRIP_HEIGHT` strip.
4. An optional credential dialog.

**There is no sidebar and no horizontal split.** `docs/design/ui.md` gives three regions,
a toolbar, a "sidebar refs" column and a main/detail pair, and says: "**"Local Changes
(N)" is the first sidebar entry**, with its count", "main: history graph OR local
changes", and "**Stashes appear inline in the commit list** at the commit they sit on".
**(Inferred)** A sidebar would wrap `split(..)` in a horizontal `ResizableContainer` inside
`window::window`. A Local Changes view would replace `list`, and maybe the pane, while it
is chosen. Every list in it must be a `VirtualScrollView`, because `ScrollView` is banned
on render paths (§9). `cairn_ui::ChangesList` (filter plus virtualized file list over a
`ChangeSet`, reporting by index) and `DiffView` are the reusable pieces. `ChangesList`
takes a `ChangeSet`, which has no status kinds (§6).

`detail_pane.rs` draws the Commit or Changes tab for the selection, and does so only when
the kept answer names the selected row (R4.4). `NOTHING_SELECTED = "Select a commit to see
its details."`.

## 8. Design documents that constrain this packet

- **Roadmap brief** (`docs/work/daily-loop/roadmap.md` §4): "The graph walks every ref by
  default — branches, remotes and tags — as Fork's "All Commits" view does." "**Open:**
  O2 — `gix-status` or `git status --porcelain=v2`. D1 says reads use gix, but status is
  unusually exposed to `core.fsmonitor`, sparse checkout and attributes ... Measure
  agreement against `git` on a repository configured for each before committing." "**Carries
  a measured bar (L6):** status on a large, dirty working tree." "**Out:** acting on any
  ref (packet 7), staging (packet 5)." "This packet owns the changed-path enumeration ...
  whichever of 4 and 5 lands first wires the Local Changes list to them. Note for O2: gix's
  status runs the user's clean filter driver when it hashes a working-tree file ... so a
  filtered path belongs in the agreement measurement".
- **`docs/design/engine.md`** (D1): "Every repository read goes through `gix`, in
  process, unless gix's answer to it differs from git's." "A new one is a decision, argued
  from a measured disagreement, never a convenience." Under "Two implementations of git":
  "**Edge cases can diverge.** gix's status against git's under sparse checkout,
  `core.fsmonitor`, or unusual attribute configuration is a real defect class: Cairn shows
  one answer and the user's next `git` command acts on another." It also says parsing is
  "mitigated by preferring `-z` and porcelain v2 formats", and lists gix's "status,
  dirwalk" under coverage.
- **`docs/design/concurrency.md`** (D3): "History has its thread; diffs — commit,
  comparison and working-tree — have another, so a long history page never queues a diff
  behind it." "Every query belongs to a **lane** ... A new query supersedes older ones in
  its own lane only". "A read that `git` answers ... is a query like any other: it belongs
  to its lane, and superseding it ends its process". On write lanes: "**Local** —
  everything that writes the index, the working tree or a local ref" (not built yet;
  `network_lane::Lane` has `Network` only). "Reads run beside both ... A read computed
  while a write runs may see a moment between two states; the write's invalidation is
  what corrects it."
- **`docs/design/worktrees.md`** (D8): "a branch checked out in another worktree carries a
  chip and a disabled checkout". "Worktrees are ... a sidebar section". Worktrees belong to
  packet 8. **(Inferred)** A ref enumeration that does not record which worktree has a
  branch checked out would have to be revisited then.
- **`docs/design/cairn.md`, "Still open"**: the repository manager shape (tabs, sidebar
  or windows), whether the local write lane is keyed per worktree, interactive rebase,
  auto-stash before destructive working-tree operations, syntax highlighting, the menu
  bar, and a light theme. None of them is refs or status itself. The manager shape decides
  what is "per-repository versus global", which a sidebar meets.
- **`docs/design/history-graph.md`**: "Columns: lanes, subject with ref badges, author,
  short id, date."
- **`docs/prd/history-graph.md`** R6.2: "The working-tree row that Sourcetree shows above
  the first commit sits *in* the graph — it occupies a lane and lines pass it — so it is
  laid out by the engine, not decorated by the view. `refs-and-status` adds that variant".
  R6.3: "Decoration a row may later carry — ref labels, ahead/behind counts, whether the
  commit is on the checked-out branch — is added as **fields** by the packets that own
  them."
- **`docs/design/diff.md`**: the Commit tab shows "chips for the refs pointing at the
  commit" (see §3).
- **`docs/prd/diff-engine.md`** R3.6: "This packet does not enumerate which paths
  changed, which is status and packet 4's, and does not draw a Local Changes screen.
  Whichever of packets 4 and 5 builds the changed-file list wires it to R3 and R6 (L6)."
  R3.3: "The index and the attributes are read fresh for every working-tree query. Nothing
  about a working tree is cached across queries." R3.4: a conflicted path is a state
  ("no diff until D6").
- **`docs/systems/diff.md`**: "Which paths of a working tree changed — status — is not
  here: the working-tree query answers one path it is given." From its limits: "`Untracked`
  answers `git diff --no-index` for the path whatever the index holds: which paths are
  untracked is status's to say." Also "A sparse index is unsupported".
- **`docs/systems/history-graph.md`**: "**It walks from `HEAD`, not from every ref.** ...
  `refs-and-status` is where showing all branches belongs".
- **Related open issues**: #2 (filter, exclude or focus the graph by ref; "Belongs with or
  just after `refs-and-status`", and "Clicking a branch in the sidebar selects and
  highlights; it does not filter"), #4 (retained rows), #5 (scrollbar), #25 (ref scans
  and the synchronous clear), #43 (ref scans hold the close), #52 (free old rows off the UI
  thread on reload) and #54 (read-verb roster).

## 9. Guards this packet is likely to meet

| Guard (`crates/cairn-guards/tests/invariants.rs`) | What it holds | What a refs/status change must satisfy |
| --- | --- | --- |
| `every_view_of_a_row_names_every_kind_of_row` (+ self-test) | outside `cairn-model` and `cairn-guards`, no wildcard, catch-all binding, `if let`, `let..else`, `matches!` or variant `use` over `RowContent`; requires at least one reader | a new `RowContent` variant is matched by name in `window.rs::history`, `selection.rs::summary_of`, `HistoryRow::id` and every test that matches. `RowId` (`comparison_of`, `Pair::other`, `loaded_row`) is compiler-held only |
| `every_view_of_a_file_diff_names_every_state` / `..._diff_row_...` | the same for `DiffContent`, `UnifiedRow` and `SideBySideRow` in production `src/` | a Local Changes diff reuses `DiffView` and these hold |
| `a_history_sized_list_renders_through_a_virtualizing_view` (+ behavioural twins in `crates/cairn-ui/tests/`) | no render file names `ScrollView` (`UNBOUNDED_VIEW_EXCEPTIONS` is empty); some render file uses `VirtualScrollView` over `HistoryRow` | a refs sidebar (tens of thousands of tags) and a Local Changes list use `VirtualScrollView`; a bounded panel needs a reviewed exception row. Root `CLAUDE.md` asks for a viewport twin per list (as for `changes_list.rs`) |
| `the_ui_thread_never_waits_on_repository_work` | render files of `cairn-ui` and `cairn-app` (all but `worker/`) name no `cairn_git`, `gix`, `cairn_askpass` or waiting primitive; `worker/` names no render identifiers | refs and status reads run in `worker/` and arrive as `Update`s; a sidebar count is a value, never a call |
| `layers_never_name_the_crates_they_are_sealed_from`, `layer_dependencies_are_allowlisted` | `cairn-ui` and `cairn-model` never name `gix` or `cairn_git` | new ref and status vocabulary lives in `cairn-model` as plain data; no gix type in a public signature (engine design) |
| `only_the_ops_module_mutates_a_repository` | no product file outside `ops/` and `process/` spawns `git` by literal; no file of `cairn-git/src` outside `ops/` names the gitoxide mutation roster | a gix-status path must not call `write_changes` (roster entry). Generic names like `.commit(`, `.reference(`, `.delete(`, `.transaction(` and `.write(` are banned as calls in `cairn-git/src` outside `ops/`, so a ref-reading helper must avoid those method names **(inferred risk)** |
| `the_runner_is_named_only_by_ops_and_reads` | only `process/`, `ops/` and `reads/` name `GitCommand`, `read_invocation` and the rest | a `git status` or `git for-each-ref` read is a named function in `reads/` (`reads/status.rs`, say), re-exported `pub(crate)` from `reads/mod.rs`, never `pub` |
| `the_porcelain_reads_are_the_two_named_queries` (+ self-test) | the exact literal `"diff"` only in `reads/working_tree.rs` beside `"--no-index"` with `"/dev/null"`; `"config"` only in `reads/fetch_settings.rs` in query form; no config setter literal in `reads/` | `"status"` and `"for-each-ref"` are not checked by it. A new read must not spell `"diff"` or `"config"` (for example `-c` keys are fine, but the bare literal `"config"` is not). `reads/mod.rs` allows "Query plumbing, or `git status`"; anything else "brings its own evidence that it writes nothing". Issue #54 would add a verb roster (`diff-tree`, `diff-index`, `diff-files`, `check-attr`, `status`, the two porcelain queries), so a `for-each-ref` read would be a roster edit |
| `only_the_process_module_builds_or_runs_a_process`, `every_git_invocation_disables_the_terminal_prompt`, `the_retired_runner_is_gone` | no `Command`, `Stdio`, `Child`, `.spawn()` and so on outside `process/`; the environment is built in one place; a read's environment carries `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1` | a status read goes through `GitBinary::read_invocation()...start()?.records(..)` and gets `GIT_OPTIONAL_LOCKS=0`, which is what stops `git status` refreshing the index (the variable "covers `status` alone") |
| `no_component_names_a_literal_modifier`, `the_accelerator_table_holds_data_and_resolution_only` | shortcuts resolve through `cairn_ui::accelerators` | any sidebar or Local Changes shortcut is an `Action` in the table |
| `destructive_operations_are_sealed_behind_the_confirmation_token` | — | not touched: the packet is read-only by its brief |

Reviewer obligations that follow from the root `CLAUDE.md` residuals: whether a new read
really writes nothing beyond its literals is `destructive-ops-reviewer`'s check 10. If the
read may touch a missing promisor object, the git < 2.44 lazy-fetch residual is its
check 10 too. Whether a scan over rows on the UI thread is bounded, including the
`index_of` fallback once a row arrives above another, is `responsiveness-reviewer`'s. Root
`CLAUDE.md` requires any change to `cairn-model` to come with a test in the same commit.

## 10. Test infrastructure

- **Fixture repositories are built by running real `git`.** The shared builders are in
  `crates/cairn-git/tests/fixtures/mod.rs`: `Fixture` (`path`, `git(args)`, `rev_list()`),
  `run(dir, args, at)`, `braided(steps)`, `braided_in(object_format, steps)` and
  `unborn()`. Histories there are made of empty commits. `run` isolates git with
  `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_SYSTEM=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`,
  `HOME`/`XDG_CONFIG_HOME` set to an empty directory, fixed author and committer, and
  dated commits. Repositories with content for the diff tests are built by
  `crates/cairn-git/tests/diff/repositories.rs`: `Repo::new(name)` runs
  `git init --initial-branch=main`, `Repo::borrowed(path)`, `git`/`try_git`, and committer
  dates rising a minute per commit from `EPOCH = 1_600_000_000`.
  `crates/cairn-git/tests/diff/scratch.rs` and `fsmonitor.rs` hold the snapshot-the-git-dir
  pattern ("writes nothing") and the fsmonitor fixtures. Unit tests also open the Cairn
  checkout itself (`Repository::discover(env!("CARGO_MANIFEST_DIR"))`). A CI checkout is
  detached with only remote-tracking refs (`refs.rs` test comment), so tests over this
  checkout must not name a branch. `crates/cairn-git/src/remotes.rs` tests build a bare
  repository by `std::fs` to check remote ordering.
- **Integration test binaries**: `tests/history.rs`, `tests/diff_engine.rs` (declaring
  `tests/diff/*`), `tests/fetch.rs` (`ref_tips_follow_the_refs_git_writes`, with
  `tests/remotes/` for http, ssh and askpass), and `tests/git_binary.rs`. The real-git
  floor runs `diff_engine` against git 2.30.9 and 2.32.7 (`scripts/git-floor.sh`,
  `gate.sh --step git-floor`).
- **The bench reporter pattern.** These are `#[ignore = "needs a large repository named by
  CAIRN_BENCH_REPO"]` tests that print and assert no timings:
  - `crates/cairn-git/tests/diff/bench.rs`, run as `CAIRN_BENCH_REPO=~/Development/bench/rust
    cargo test -p cairn-git --release --test diff_engine -- --ignored --nocapture`. It
    holds named subjects with git's own warm median beside each (`CHANGES_SUBJECTS`,
    `RUNS = 7`, `median`, `report`). Its numbers go to
    `docs/research/diff-engine/c14-measured.md` beside `measured-baseline.md` (git's own,
    on rust-lang/rust).
  - `history.rs` tests `measures_both_orders_against_a_named_repository` (with
    `CAIRN_BENCH_LIMIT`) and `measures_layout_over_every_ref_of_a_named_repository`.
- **The window check** is `crates/cairn-app/src/window_check.rs` (`#[cfg(test)]`,
  `#[ignore]`d, `window_check`). It runs the real `window::window` over the real worker
  and engine, headless through `freya-testing` at 1440×900, with frames paced at 60 Hz.
  Each frame applies the updates that arrived through `session::apply` and one
  `sync_and_update`, timed. Paint is reported apart as an encoded snapshot. It is driven by
  `CAIRN_BENCH_REPO` (and `CAIRN_WINDOW_CHECK_FRAMES`) against fixed subjects (`S1`, `M1`,
  `F1`), run as `cargo test -p cairn-app --release -- --ignored --nocapture
  window_check`.
- **Component tests** use `freya-testing` headlessly: `crates/cairn-ui/tests/history_list.rs`
  (`only_a_viewport_of_rows_is_built_however_long_the_history`), `changes_list.rs`,
  `commit_tab.rs`, `commit_tab_expansion.rs` and `diff_view.rs`. Window states are tested in
  `crates/cairn-app/src/window.rs`, and worker behaviour in `worker/diff_tests.rs`,
  `fetch_tests.rs` and `lifecycle_tests.rs`.
- **A host-dependent test is required wherever it can run**: a skip reads as `ok`, so the
  gate probes and sets `CAIRN_REQUIRE_*`
  (`CAIRN_REQUIRE_FSMONITOR_DAEMON` among them). Any fsmonitor-dependent status agreement
  test would join that pattern (root `CLAUDE.md` invariant, with its twins).

## Open questions for the planner

- Where the refs enumeration runs. On `cairn-repository` it can seed the walk directly; on
  its own lane it can be cancelled and superseded. Whether it shares one read with the
  fetch's before/after comparison (#25, #43) is also open.
- Whether `ref_tips` survives, given that it peels tags and drops symbolic targets, or is
  replaced by a richer read with a cancel.
- How a stash row and an uncommitted-changes row get an `Oid` for `GraphRow` and
  `LaneAssigner`, or whether those types change.
- O2, measured as the brief asks, with gix-status's missing fsmonitor support
  (`FSMONITOR_VALID` never set) and the clean-filter note in scope.
