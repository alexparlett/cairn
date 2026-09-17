# The engine and the worker as built, for the `diff-engine` packet

Code-audit record. Gathered 2026-09-17 for the planning of the `diff-engine`
packet, from the working tree at `main` (`bf93a4e`, "credential-prompts:
authenticated fetch without Cairn holding a credential (#28)"). Method: every
file named below was read in full; gix facts come from the vendored source
actually linked (`~/.cargo/registry/src/*/gix-0.87.1`, `gix-diff-0.67.1`,
`gix-imara-diff-0.2.5`), not from memory. Anchors are file paths and exported
symbols, never line numbers. This record describes what EXISTS; it proposes no
design. Where a fact could not be settled it is listed under "Open questions
for the planner" at the end.

The packet this serves will add commit diffs (a commit against its parent),
working-tree diffs (index against worktree, `HEAD` against index), comparison of
two arbitrary revisions, a patch-capable diff model that can emit a valid
unified patch for an arbitrary subset of hunks or lines (for a later
`git apply --cached`), and diff options (whitespace, word-level intra-line,
context lines, rename detection). Reads go through gitoxide, never a
subprocess.

## 1. `crates/cairn-git/` — the engine

### 1.1 Module layout

`crates/cairn-git/src/lib.rs` declares seven modules, one of them public:

| Module | File | Visibility | What it holds |
| --- | --- | --- | --- |
| `cancel` | `src/cancel.rs` | private, re-exported | `Cancel`, `CancelSignal` |
| `error` | `src/error.rs` | private, re-exported | `Error`, `RefusedWrite` |
| `history` | `src/history.rs` + `src/history/session.rs` | private, re-exported | `HistoryOrder`, `HistoryCursor`, `HistoryRequest`, `HistoryPage`, `HistorySession`, `Repository::history`, `Repository::history_session` |
| `ops` | `src/ops/` | `pub mod` | every mutation (section 2) |
| `refs` | `src/refs.rs` | private | `Repository::ref_tips` |
| `remotes` | `src/remotes.rs` | private | `Repository::remotes` |
| `repository` | `src/repository.rs` | private, re-exported | `SharedRepository`, `Repository` |

The `pub use` list in `lib.rs` is the whole public surface outside `ops`:
`Cancel`, `CancelSignal`, `Error`, `RefusedWrite`, `HistoryCursor`,
`HistoryOrder`, `HistoryPage`, `HistoryRequest`, `HistorySession`,
`Repository`, `SharedRepository`. There is no crate-local `CLAUDE.md` under
`crates/cairn-git/` (nor under any other crate; `find` over the checkout finds
only the root `CLAUDE.md` and `docs/CLAUDE.md`), so the root file's conventions
are the only ones in force.

Read modules follow one pattern the packet inherits: a file named for the
behaviour (`refs.rs`, `remotes.rs`, `history.rs`) that opens an `impl Repository`
block and adds a method, reaching gix through the crate-private
`Repository::inner()`.

### 1.2 Opening a repository: `SharedRepository` and `Repository`

`crates/cairn-git/src/repository.rs`:

- `SharedRepository` wraps `gix::ThreadSafeRepository` plus cached `git_dir:
  PathBuf` and `workdir: Option<PathBuf>`. Hand-written `Debug` prints the two
  paths only. Public API: `SharedRepository::discover(path)` (walks upward like
  `git`; `gix::discover::Error::Discover` maps to `Error::NotARepository`,
  anything else to `Error::Open`), `to_worker()` ("Call once per worker thread
  and keep it: each call rebuilds the object cache and pack snapshot" — it
  calls `to_thread_local()` then `object_cache_size_if_unset(OBJECT_CACHE_BYTES)`),
  `git_dir()`, `workdir()` (`None` for bare).
- `Repository` wraps `gix::Repository` plus `workdir`. Public API:
  `Repository::discover(path)` (= `SharedRepository::discover(..)?.to_worker()`),
  `Repository::OBJECT_CACHE_BYTES = 4 * 1024 * 1024`, `git_dir()`, `workdir()`.
  `pub(crate) fn inner(&self) -> &gix::Repository` is the one door to gix for
  every read module.
- Neither type exposes a gix type in a public signature; the gix handle is a
  private field. Nothing in the crate exposes the index, a tree, or a blob yet.
- Unit tests in the file open the Cairn checkout itself
  (`Repository::discover(env!("CARGO_MANIFEST_DIR"))`); one pins that every
  worker handle carries the object cache (`every_worker_handle_carries_the_object_cache`).

### 1.3 Cancellation: `Cancel` and `CancelSignal`

`crates/cairn-git/src/cancel.rs`:

```rust
pub trait Cancel {
    /// Called once per commit visited. Returning `true` abandons the query.
    fn is_cancelled(&self) -> bool;
}
#[derive(Debug, Clone, Default)]
pub struct CancelSignal(Arc<AtomicBool>);   // new(), cancel(); cannot be unset
```

The engine is generic over `&impl Cancel`; it never names the worker's epoch.
`Repository::history` and `HistorySession::next_page` poll it once per commit
pulled off the walk, and `next_cursor` polls it once more before its one extra
step. On cancellation the engine returns `Error::Cancelled { walked }` where
`walked` counts the commits laid out before stopping (for a session, this call
only). The cancel signal is polled — it is not a gix `should_interrupt`
`AtomicBool`; the walk simply stops being pulled.

### 1.4 Errors: `Error` and `RefusedWrite`

`crates/cairn-git/src/error.rs` — one `thiserror` enum for the whole crate,
reads and ops alike. Variants, with what the caller must handle:

| Variant | Fields | When |
| --- | --- | --- |
| `NotARepository` | `path` | discovery found nothing |
| `Open` | `path`, `source: Box<dyn Error + Send + Sync>` | any other open failure |
| `Cancelled` | `walked: usize` | a query's `Cancel` fired |
| `UnbornHead` | `path` | `HEAD` names a branch with no commits |
| `Walk` | `source` | the rev walk failed (usually corrupt/missing object); also used for a wrong-width `Oid` |
| `ReadCommit` | `id: String`, `source` | a commit object could not be read/decoded |
| `GitNotFound` | `searched: Vec<PathBuf>`, `required: GitVersion` | startup |
| `GitTooOld` | `path`, `found`, `required` | startup |
| `GitVersionUnreadable` | `path`, `output`, `required` | startup |
| `GitNotStarted` | `program`, `source: io::Error` | spawn failed |
| `GitFailed` | `arguments: String`, `status: ExitStatus`, `stderr: String` | non-zero exit |
| `GitCancelled` | `arguments`, `stranded_locks: Vec<PathBuf>` | the user killed it |
| `Refs` | `source` | `ref_tips` could not read refs |
| `FetchRefused` | `remote`, `setting`, `write: RefusedWrite` | refspec policy refused before spawning |
| `RemoteConfig` | `remote`, `source` | remote config unreadable |

Every gix error crosses as `Box<dyn std::error::Error + Send + Sync>` under a
`#[source]` field; no gix error type is named in the enum (the one dependency
type it names is `crate::ops::GitVersion`, Cairn's own). `RefusedWrite` is
`LocalBranches | LocalTags | Mirror` with a `Display`. There is no variant yet
for "object is not a commit/tree/blob", "path not in tree", "index unreadable",
"binary content", or "diff failed"; a diff module will need its own.

### 1.5 History: `HistoryRequest`, `HistoryCursor`, `HistoryPage`, `HistoryOrder`

`crates/cairn-git/src/history.rs`:

- `HistoryOrder { CommitTime, GraphOrder }`, `Default = CommitTime`; a private
  `sorting()` maps to `gix::revision::walk::Sorting::ByCommitTime(NewestFirst)`
  / `BreadthFirst`. "Neither order is topological: a parent can arrive before
  its child."
- `HistoryRequest` (private fields `start: Start`, `order`, `limit`, `window`),
  where private `enum Start { Head, Commits(Vec<Oid>), Resume(HistoryCursor) }`.
  Constructors: `from_head(limit)`, `from_commits(tips, limit)`,
  `resume(cursor, limit)`; builders `with_order`, `with_window` (both ignored
  when resuming; window defaults to `LaneAssigner::DEFAULT_WINDOW`).
- `HistoryCursor` (private `tips: Arc<[gix::hash::ObjectId]>`, `order`,
  `window`, `walked`) — "Opaque: hand it back to `HistoryRequest::resume` and
  nothing else"; one accessor `rows_behind()`. It holds gix ids privately; the
  type is `Send` because `ObjectId` is plain data.
- `HistoryPage { rows: Vec<HistoryRow>, cursor: Option<HistoryCursor>, walked:
  usize, decoded: usize }` — all public fields.
- `Repository::history(&self, &HistoryRequest, &impl Cancel) -> Result<HistoryPage, Error>`:
  the cold, replaying path (page `k` walks `k x limit`).
- Private helpers a diff module can reuse by making them `pub(crate)`:
  `object_id(&Oid) -> gix::hash::ObjectId` (bytes, no hex round trip),
  `model_id(&gix::hash::oid) -> Oid`, `walk_tips` (refuses an `Oid` of the
  other hash width with `Error::Walk`), `summary_from(&gix::Commit, ..)`
  (reads `message().summary()`, `author()`, `author().time().seconds`).
- Two `#[ignore]` reporter tests read `CAIRN_BENCH_REPO` / `CAIRN_BENCH_LIMIT`:
  `measures_both_orders_against_a_named_repository` (orders x object-cache
  sizes 0/4/16/32 MiB x walk-only/walk+lanes) and
  `measures_layout_over_every_ref_of_a_named_repository` (prints segments/row,
  open lanes/row, bytes/row percentiles, out-of-order counts and retained MB at
  10k/100k/500k commits). This is the measurement harness the history packet's
  numbers came from (section 6).

`crates/cairn-git/src/history/session.rs`:

- `pub struct HistorySession<'repo>` holds `gix::revision::Walk<'repo>` (so it
  borrows the `Repository` and is `!Send`), a `LaneAssigner`, a `ready:
  VecDeque<HistoryRow>` and `pending: VecDeque<(Oid, Vec<Oid>)>`, the tips,
  order, window, `skip`, `next_row`, `walked`, `decoded`, `delivered`,
  `exhausted`. Hand-written `Debug`.
- `Repository::history_session(&self, &HistoryRequest) -> Result<HistorySession<'_>, Error>`
  (the request's limit is ignored); `next_page(&mut self, limit, &impl Cancel)
  -> Result<HistoryPage, Error>` ("Any error other than `Error::Cancelled`
  poisons the session"; on cancel, work done stays in the session);
  `cursor()`, `delivered()`, `is_exhausted()`.
- Row summaries are read by id (`summary_of_commit` via `repo.find_commit`),
  "by the time a session hands out a row, the walk's `Info` is gone".

### 1.6 Other reads

- `Repository::ref_tips(&self) -> Result<BTreeMap<RefName, Oid>, Error>`
  (`src/refs.rs`): every ref via `references().all()`, `peel_to_id`, symbolic
  refs resolved, unresolvable ones dropped. The worker compares before/after a
  fetch.
- `Repository::remotes(&self) -> Vec<RemoteSummary>` (`src/remotes.rs`):
  default fetch remote first, password stripped from the URL. Infallible.

### 1.7 How gix stays out of public signatures

- Every public type either wraps gix in a private field (`SharedRepository`,
  `Repository`, `HistoryCursor`, `HistorySession`) or is built purely from
  `cairn-model` values (`HistoryPage`, `HistoryRequest`).
- Errors box gix errors as `dyn Error`.
- Ids cross as `cairn_model::Oid` via `Oid::from_bytes(id.as_bytes())` and back
  via `ObjectId::try_from(oid.as_bytes())`.
- The guard `layers_never_name_the_crates_they_are_sealed_from` (section 5)
  checks the OTHER direction (UI/model never name gix); nothing mechanical
  checks that `cairn-git`'s public signatures are gix-free — that is a review
  judgement today.

### 1.8 Tests and fixtures

Unit tests (`#[cfg(test)] mod tests` in each source file) open the Cairn
checkout itself with `Repository::discover(env!("CARGO_MANIFEST_DIR"))`; the
`remotes.rs` test builds a bare repository by hand with `std::fs`.

Integration tests live in `crates/cairn-git/tests/`:

- `tests/fixtures/mod.rs` — the shared builder, "Repositories built by running
  real `git`". `pub struct Fixture { path }` with `path()`, `git(&[..]) ->
  String` (panics on failure), `rev_list()`; `Drop` removes the directory.
  `pub fn run(dir, args, at: Option<i64>)` runs `git` isolated from the
  machine (`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_SYSTEM=/dev/null`, fixed
  author/committer names and, when `at` is given, `GIT_AUTHOR_DATE`/
  `GIT_COMMITTER_DATE` = `"<seconds> +0000"`). `pub const EPOCH: i64 =
  1_500_000_000`. Builders: `braided(steps)` / `braided_in(object_format,
  steps)` (`main` and `side` branches, a `--no-ff` merge every nine commits and
  at the end, every commit `--allow-empty`, so **no fixture today has any file
  content or tree change**), and `unborn()`. The file's own header says a
  builder one test file alone needs lives in that file, "since an unused item
  here is a warning in every other binary".
- `tests/history.rs` — reads expectations back from `git rev-list`,
  `rev-list --parents` and `log --format=%H%x1f%s%x1f%an%x1f%ae%x1f%at`; local
  builders `skewed()`, `commit_stamped`, `write_commit_graph` (runs `git
  commit-graph write`), `delete_objects` (corrupts a fixture to test
  `ReadCommit`). Test names are in the file; the shape a diff test file would
  copy is `every_row_matches_what_git_reports`.
- `tests/fetch.rs` + `tests/remotes/{http,ssh,askpass}.rs` — a local HTTP
  smart server, an ssh server (skipped when `sshd` is unavailable), and a
  test-side askpass channel; a `RecordingGit` stub; the `Pruning` fixture.
- `tests/git_binary.rs` — startup discovery against stub `git` scripts.
- `src/ops/stub_git.rs` (`#[cfg(all(test, unix))]`) — `StubGit::with_git(script)`
  and `discover_retrying`, a `git` on a private `PATH` that prints what it was
  given; used by `ops/fetch.rs` and `ops/cli.rs` stub tests.

### 1.9 gix features enabled, and what `blob-diff` / `status` / `blame` pull in

Root `Cargo.toml`:

```toml
gix = { version = "0.87.1", features = ["blame", "blob-diff", "revision", "status", "max-performance", "parallel", "sha256"] }
```

with default features ON (`default = max-performance-safe, comfort, basic,
extras, auto-chain-error, sha1`; `basic = blob-diff, revision, index`; `extras`
includes `worktree-stream`, `worktree-archive`, `revparse-regex`, `mailmap`,
`excludes`, `attributes`, ...). From the vendored `gix-0.87.1/Cargo.toml`:

- `blob-diff = ["gix-diff/blob", "attributes"]`; `attributes` brings
  `excludes`, `gix-filter`, `gix-pathspec`, `gix-attributes`, `gix-submodule`.
  `gix-diff/blob` brings `imara-diff` (linked as `gix-imara-diff 0.2.5`),
  `gix-filter`, `gix-worktree`, `gix-command`, `gix-tempfile`, `gix-traverse`.
- `status = ["gix-status", "dirwalk", "index", "blob-diff", "gix-diff/index"]`;
  `dirwalk` brings `gix-dir`. `gix-status 0.34.1` has modules
  `index_as_worktree`, `index_as_worktree_with_renames`, `fscache`, `stack`.
- `blame = ["dep:gix-blame", "blob-diff"]` (`gix-blame 0.17.1`).
- `merge` (`gix-merge 0.20.1`) is in the lock file as a transitive of the
  default set; it is not named by Cairn.

So every gix surface a diff needs is already compiled and linked today:
`gix-diff 0.67.1` with `blob` and `index`, `gix-status`, `gix-index 0.55.0`,
`gix-worktree 0.56.0`, `gix-filter 0.34.0`. **No dependency addition is needed
for tree, index or worktree diffs through gix.** The dependency-policy step
(`gate.sh --step deps`, `deny.toml`) therefore has nothing new to record unless
the packet adds a crate.

Vendored API facts a planner should know exist (verified in source, not
recommended here):

- `gix::Repository::diff_tree_to_tree(old, new, options) -> Vec<ChangeDetached>`
  (`src/repository/diff.rs`) — builds a resource cache with
  `pipeline::Mode::ToGit`, fills `Options` from config when `None` (rename
  tracking on at 50% by git default), collects everything into a `Vec`.
- `gix::Tree::changes()?.for_each_to_obtain_tree[_with_cache](other, cache,
  |Change| -> Result<Action, E>)` (`src/object/tree/diff/for_each.rs`) —
  streaming; the closure returns `Action::Continue | Action::Cancel`, which is a
  natural place to poll a `Cancel`. Returns `Option<gix_diff::rewrites::Outcome>`.
- `gix::diff::Options` (`src/diff.rs`): `location`, `rewrites:
  Option<gix_diff::Rewrites>`; `track_rewrites(None)` disables rename tracking.
  `gix_diff::Rewrites { copies: Option<Copies>, percentage: Option<f32>, limit:
  usize (default 1000), track_empty: bool }`.
- `gix::object::tree::diff::Change::diff(&mut Platform)` gives a blob-diff
  platform; `Repository::diff_resource_cache(mode, WorktreeRoots)` builds one
  (`.gitattributes`, filters, `diff.<driver>` honoured); "resource_cache only
  grows, so one should call `clear_resource_cache` occasionally".
- `gix_diff::blob`: `Algorithm { Histogram, Myers, MyersMinimal }` (imara-diff),
  `diff_with_slider_heuristics`, `UnifiedDiff::new(&Diff, &InternedInput, consume_hunk,
  ContextSize)` with `ContextSize::symmetrical(n)` (default 3), `ConsumeHunk`
  trait (`consume_hunk(HunkHeader, &[(DiffLineKind, &[u8])])`, `finish`),
  `DiffLineKind`, `HunkHeader`, `ConsumeBinaryHunk`. `imara_diff::sources` has
  `lines`, `words`, `bstr_lines`, `byte_lines`. **No whitespace-ignoring mode
  was found in `gix-imara-diff 0.2.5`** (the only `whitespace` mention is in
  `slider_heuristic.rs`); tokenisation is the caller's.
- `gix::Repository::status(progress)` -> `status::Platform` (`src/status/mod.rs`)
  with `should_interrupt_owned(Arc<AtomicBool>)`, `untracked_files`,
  `index_worktree_rewrites`, `head_tree`, `tree_index_track_renames`,
  `dirwalk_options`; `index_worktree_status(..)` and `tree_index_status(tree_id,
  &index::State, pathspec, TrackRenames, cb)` (`src/status/index_worktree.rs`,
  `src/status/tree_index.rs`); `Repository::open_index`, `index`,
  `index_or_empty`, `index_or_load_from_head[_or_empty]` (`src/repository/index.rs`).
- `gix::Repository::rev_parse` / `rev_parse_single` (`src/repository/revision.rs`)
  exist for "two arbitrary revisions" — nothing in `cairn-git` exposes them.

## 2. `crates/cairn-git/src/ops/` — the subprocess backend (shape only)

`src/ops/mod.rs` is the module doc that states the contract; the pieces:

| Symbol | File | Visibility | Role |
| --- | --- | --- | --- |
| `GitBinary`, `GitVersion` | `ops/binary.rs` | `pub` | `GitBinary::discover(&Askpass)` / `discover_with(GitEnvironment)`; `GitVersion::MINIMUM = 2.30.0`; `GitVersion::parse`; `GitBinary::command()` (crate-private use), `environment()` |
| `GitEnvironment` | `ops/environment.rs` | `pub` | the ONLY place a `std::process::Command` is built (`pub(crate) fn command(&self, program, token: Option<&AskpassToken>) -> Command`, which `env_clear()`s then applies `ALWAYS` + `INHERITED` + askpass entries); `new(parent, &Askpass)`, `variables()`, `get()` |
| `Askpass` | `ops/askpass.rs` | `pub` | `new(program, socket: Option<PathBuf>)`, `program()`, `socket()` |
| `GitCommand`, `Running`, `ProcessKill`, `Output` | `ops/cli.rs` | `pub(crate)` | the runner |
| `fetch`, `FetchInProgress`, `FetchCancel` | `ops/fetch.rs` | `pub` | the one verb |
| `Invalidated`, `Performed`, `describe_destructive` | `ops/mod.rs` | `pub` | the cache contract and the operation record |
| `refspec_policy`, `stranded_locks` | `ops/refspec_policy.rs`, `ops/stranded_locks.rs` | crate-private | pre-flight refusal; lock listing after a kill |
| `stub_git` | `ops/stub_git.rs` | `#[cfg(all(test, unix))]` | stub `git` for runner tests |

**The runner and stdin — the fact packet 5 needs.** `ops/cli.rs` module doc:
"Standard input is always closed." Both entry points set `.stdin(Stdio::null())`:

- `GitCommand::run(self) -> Result<Output, Error>` — runs to completion,
  `stdout` and `stderr` piped, non-zero exit is `Error::GitFailed { arguments,
  status, stderr }`.
- `GitCommand::stream(self) -> Result<Running, Error>` — for a verb whose
  stderr is watched (`fetch --progress`); `stdout` is `Stdio::null()`, stderr
  piped and streamed line by line through `Running::finish(progress)`;
  `Running::killer() -> ProcessKill` (`SIGTERM`, then `SIGKILL` after
  `TERMINATION_GRACE = 2s`, via `nix`).

**There is no way to feed a verb standard input today.** A `git apply --cached`
that takes a patch on stdin needs a new runner path (or the patch written to a
temporary file and passed by path). `Output` carries `stdout: Vec<u8>` (bytes,
"because paths are bytes"), `stderr: String`, with `stdout_text()` and
`records()` (splits `-z` output on NUL; marked `expect(dead_code)` "fetch reads
nothing from stdout; the first operation to will be status"). The output policy
in `ops/mod.rs`: machine-readable forms only (`-z`, `--porcelain=v2`,
`--format` with explicit separators); stderr is never parsed.

Builder methods on `GitCommand`: `arg`, `args`, `in_repository(&Repository)`
(runs in the workdir, or the git dir of a bare repository), `authorized_by(&AskpassToken)`.

**`Performed` and `Invalidated`.** `Performed { description: String,
acknowledged: Option<String>, invalidated: Invalidated }` — fields private;
constructors `pub(crate) fn new(description, invalidated)` and `pub(crate) fn
destructive(description, &Confirmed, invalidated)` (the prompt is copied from
the token, "never typed"); accessors `description()`, `acknowledged()`,
`invalidated()`. `Invalidated { refs, index, objects, working_tree: bool }`
with consts/ctors `NOTHING`, `refs()`, `objects()`, `index()`,
`working_tree()`, `and()`, `anything()`. The module doc explains what each flag
obliges the worker to do; the ones a staging packet will declare are `index`
("gix shares one index snapshot between the `SharedRepository` and every worker
handle, and re-reads it when the file's modification time changes ... a write
inside the same timestamp tick as the previous read is the residual gix cannot
see, so an operation that writes the index and then reads it back in one
request must reopen a handle") and `working_tree` ("Nothing in gix caches the
working tree; honouring it means re-running a status or diff query").

`fetch(git, repo, remote, token: Option<&AskpassToken>) -> Result<FetchInProgress, Error>`
is the verb shape: pre-flight check, build the command, `stream()`, return a
handle whose `finish(progress) -> Result<Performed, Error>` declares
`Invalidated::refs().and(Invalidated::objects())`. `fetch` takes no
`Confirmed`; `describe_destructive(&Repository, Confirmed) -> Performed` is the
placeholder proving the seal compiles.

## 3. `crates/cairn-app/src/worker/` — the worker boundary

### 3.1 Files and exports

`worker/mod.rs` exports exactly: `PromptId`, `Reply` (from `askpass.rs`),
`Replier`, `open` (from `pool.rs`), `Request`, `Update` (from `request.rs`).
`epoch.rs`, `operations.rs`, `startup.rs`, `wake.rs`, `fetch_tests.rs`
(`#[cfg(test)]`) are `pub(super)` at most. `RepositoryHandle` and `Updates` are
`pub` structs in `pool.rs` reachable through `open`'s return type.

### 3.2 Threads

`pool.rs` module doc: three threads per open repository, each with its own
`Outbox` so the update stream ends only when every one has gone:

- `cairn-repository` — `Backend::open` (askpass channel, `GitEnvironment`,
  `GitBinary::discover_with`) then `SharedRepository::discover`, then
  `Threads::start` (the other two), then `serve(..)`, then `threads.stop()`.
- `cairn-operations` (`operations.rs`) — `serve_operations`: takes its own
  `shared.to_worker()` handle, loops `operations.recv()`, runs `fetch`.
- `cairn-askpass` (`askpass.rs`) — `serve_prompts`: accepts one helper at a
  time, sends `Update::Prompt`, blocks on `answers.recv()`.

`pub const WORKERS_PER_REPOSITORY: usize = 1` with a `const _: () = assert!`
saying "serve() owns one repository handle and one live walk per thread; more
workers per repository need a routing decision, not a bigger constant".

### 3.3 `open`, `RepositoryHandle`, `Updates`, `Replier`

`pub fn open(path) -> Result<(RepositoryHandle, Updates, Replier), OpenError>`
"Returns before touching a disk"; `open_with(path, Startup)` is the test seam.
Channels created there: `jobs: Sender<(Option<Epoch>, Request)>`, `outgoing:
Sender<Envelope>`, `answers: Sender<Reply>`; one `Wake`, one `Epochs`, one
`FetchControl`.

```rust
#[derive(Debug, Clone)]
pub struct RepositoryHandle { jobs: Sender<(Option<Epoch>, Request)>, epochs: Epochs, control: FetchControl }
impl RepositoryHandle {
    pub fn submit(&self, request: Request) -> Epoch   // never blocks
    pub fn into_submitter(self) -> Rc<dyn Fn(Request)>
}
pub struct Updates { inbox: Receiver<Envelope>, wake: Arc<Wake>, epochs: Epochs }
impl Updates { pub async fn next(&mut self) -> Option<Update> }   // Drop stops every walk
pub type Replier = Rc<dyn Fn(Reply)>;
```

`submit`: `CancelFetch` short-circuits to `control.cancel()`; a query
(`request.is_query()`) bumps the epoch and is sent numbered `Some(epoch)`; an
operation is sent with `None` and the current epoch is returned. `Updates::next`
loops on `try_recv`: an `Envelope { epoch: None, .. }` is always returned; a
numbered one only if `epochs.is_current(epoch)`; `Empty` awaits `Woken(&wake)`;
`Disconnected` returns `None`. The UI-side consumer is the `spawn(async move {
while let Some(update) = updates.next().await { session::apply(..) } })` in
`crates/cairn-app/src/main.rs`.

### 3.4 Epochs (`epoch.rs`)

`Epoch(u64)` (Copy, Ord); `Epochs { current: Arc<AtomicU64>, stopping:
Arc<AtomicBool> }` with `bump()`, `current()`, `is_current(epoch)` (false while
stopping), `stop()`, `is_stopping()`, `watch(mine) -> Superseded`.
`Superseded` implements `cairn_git::Cancel`: cancelled when stopping or when
`current != mine`. This is the ONLY `Cancel` implementation in the app and the
only place `cairn_git::Cancel` is named outside the engine. **One global epoch
counter**: every query kind bumps the same counter, so any two queries
supersede each other regardless of kind.

### 3.5 `Wake` (`wake.rs`)

`Wake { state: Mutex<State { signalled, waker }> }`, `Wake::new() -> Arc<Self>`,
`signal()` (latches), private `poll(&Context)`; `Woken<'a>(&'a Arc<Wake>)` is
the `Future`. Poison-tolerant. `Wake::poll` and `Updates::next` are the
functions the UI thread calls from `worker/` (exempt from the waiting-roster
guard by file, a review obligation per root `CLAUDE.md`).

### 3.6 Request and Update (`request.rs`)

```rust
pub enum Request {
    OpenHistory { rows: usize },     // query
    MoreHistory { rows: usize },     // query
    ListRemotes,                     // NOT a query (answered epochless)
    Fetch { remote: String },        // operation
    CancelFetch,                     // handled in submit
}
impl Request { pub fn is_query(&self) -> bool }   // exhaustive match
pub enum Update {
    Rows { rows: Vec<HistoryRow>, complete: bool },
    Failed { message: String },
    WorkerLost { message: String },
    Remotes { remotes: Vec<RemoteSummary> },
    FetchStarted { remote }, FetchProgress { line }, FetchFinished { remote, refreshed },
    FetchCancelled { remote, refreshed, stranded_locks: Vec<PathBuf> },
    FetchFailed { remote, refreshed, message },
    Prompt { id: PromptId, text: String },
}
```

Both derive `Debug, Clone, PartialEq, Eq` — so any payload put in either must
too (a `Secret` cannot travel here; that is why `Reply` is a separate
channel). `Update::Failed { message }` is "display text, already rendered from
the engine's error" — errors cross the boundary as strings. Test
`queries_are_numbered_and_operations_are_not` lists every variant by hand.

### 3.7 `serve` — how a history page is requested and returned

`fn serve(shared, jobs: Receiver<(Option<Epoch>, Request)>, outbox, epochs, threads)`
in `pool.rs`:

1. `let repo = shared.to_worker();` once. `let mut scroll: Option<HistorySession<'_>>`
   and `let mut cursor: Option<HistoryCursor>` live for the thread ("`scroll`
   borrows `repo` across turns, so moving this inside fails to compile").
2. Loop `jobs.recv()`; break if stopping; `continue` if the request's epoch is
   already stale ("Superseded before it was picked up; never started").
3. `match request` — exhaustive over all five variants. `OpenHistory` drops the
   session and cursor (this is also what honours `Invalidated::refs` after a
   fetch); `MoreHistory` carries on; `ListRemotes` answers inline with
   `repo.remotes()` (epoch `None`); `Fetch` forwards `Operation::Fetch` via
   `threads.perform` (one at a time; a second while one runs is dropped);
   `CancelFetch` cancels the control.
4. The match evaluates to `rows` — **the match is shaped so that every arm that
   is not a history page `continue`s, and the fall-through IS the history page**.
   Then: open a session if none (`HistoryRequest::resume(cursor, rows)` or
   `from_head(rows)`; `Error::UnbornHead` becomes `Update::Rows { rows: [],
   complete: true }` via `no_walk`), call `session.next_page(rows,
   &epochs.watch(epoch))`, send `Update::Rows { rows: page.rows, complete:
   page.cursor.is_none() }` under `Some(epoch)`; on `Error::Cancelled` send
   nothing (the session keeps its rows); on any other error drop the session and
   send `Update::Failed`.

**Assumptions of exactly one query kind, stated so a planner can see them:**

- `Request::is_query` and the `serve` match both hard-code "query means history
  page"; the `let Some(epoch) = epoch else { continue }` after the match assumes
  the only numbered requests are the two history ones.
- One `Epochs` counter: a new query kind that bumps it would cancel an in-flight
  history page (and vice versa) — the design note in `request.rs` says only that
  "a scroll must not cancel a fetch, nor a fetch a scroll"; nothing yet says what
  a scroll and a diff do to each other.
- `Updates::next` filters every numbered envelope against the ONE current epoch,
  so an answer to a diff query would be dropped the moment a scroll bumped the
  counter, unless the diff carried `None` (in which case it could never be
  superseded) or a second counter existed.
- `Update::Rows` is the only value-returning query update; `session::apply`
  (`crates/cairn-app/src/session.rs`) matches `Update` exhaustively, so a new
  variant is a compile error there (and in the tests in `session.rs`,
  `fetch_tests.rs`, `pool.rs` that build `Update`s).
- `serve` owns the one `Repository` handle and the live walk on the
  `cairn-repository` thread; a diff computed there would queue behind a page in
  flight and a page behind a diff. The operations thread owns a second handle
  (`shared.to_worker()` in `serve_operations`) but only receives `Operation::Fetch`.
- `pool.rs`'s tests `cairn()` helper opens the checkout through the real
  boundary; the tests in `fetch_tests.rs` (`block_on`, `woken_by`,
  `collect_until(updates, stop)`, `UnbornRepository`, `RuntimeDir`,
  `built_helper()`) are the harness a new query's tests would reuse.

### 3.8 How fetch (an operation) is dispatched alongside queries

`operations.rs`: `pub(super) enum Operation { Fetch { remote } }`;
`FetchControl(Arc<Mutex<Stage>>)` with `Stage { Idle,
CancelledBeforeStarting, Starting { cancelled }, Running(FetchCancel) }` and
methods `arm`, `cancel`, `install`, `clear`. `serve_operations` reads
`ref_tips` before and after, opens an askpass `Operation` token per fetch
(`channel.begin()`), installs the killer BEFORE sending `FetchStarted`, streams
progress as `Update::FetchProgress`, and sends one of `FetchFinished |
FetchCancelled | FetchFailed` with `refreshed = before != after` (or `true` if
either read failed). All fetch updates carry epoch `None`. The window
(`session::apply`) reacts to `refreshed` by clearing rows and submitting
`OpenHistory { rows: PAGE_ROWS }` (`PAGE_ROWS = 64` in `main.rs`).

### 3.9 Startup (`startup.rs`)

`Startup { parent: Box<dyn Fn(&str) -> Option<OsString> + Send>, helper: PathBuf }`,
`Startup::of_this_process()`, `helper_beside_this_executable()`; `Backend {
git: GitBinary, channel: Option<Arc<Channel>>, prompting: Result<(), String> }`,
`Backend::open(&Startup)` opens the channel under `XDG_RUNTIME_DIR`, builds
`Askpass` and `GitEnvironment`, finds `git`. A missing/old `git` is the one hard
failure (`Update::Failed` before the repository is even opened).

### 3.10 Render side that consumes the boundary (for completeness)

`crates/cairn-app/src/main.rs` — `PAGE_ROWS = 64`; `View { rows:
State<Vec<HistoryRow>>, progress: State<Progress>, selected:
State<Option<RowId>>, fetch: State<FetchStatus>, prompt:
State<Option<PromptView>>, remotes: State<Vec<RemoteSummary>> }`
(`window.rs`). `history_state::Progress` tracks `in_flight`, `complete`,
`ended`, `loaded`, `lanes`; `wants_more()` gates the next `MoreHistory`.
`session::apply(update, view, &Worker { submit, refuse })` is the one place an
`Update` becomes view state. There is no selection-driven query yet: `selected`
is a `RowId` the list sets and nothing reads back into a request.

## 4. `crates/cairn-model/` — the vocabulary

### 4.1 Public types (`src/lib.rs` re-exports)

| Symbol | File | Shape |
| --- | --- | --- |
| `CommitSummary` | `lib.rs` | `{ id: Oid, parents: Vec<Oid>, summary: String, author_name, author_email, author_time: i64 }`, derives `Debug, Clone, PartialEq, Eq` |
| `RefName` | `lib.rs` | newtype over `String`; `new`, `as_str`, `shorthand()` strips `refs/heads/`, `refs/remotes/`, `refs/tags/`; derives incl. `Ord`, `Hash` |
| `Oid`, `OidHex`, `OidParseError` | `oid.rs` | `Oid { bytes: [u8; 32], width: Width { Sha1 = 20, Sha256 = 32 } }` Copy; `parse(hex)` (40 or 64), `from_bytes(&[u8])` (20 or 32), `as_bytes()`, `hex()`, `short()` (7 chars); hand-written `Debug`/`Display` print hex |
| `RowContent` | `history.rs` | `enum { Commit(CommitSummary), #[cfg(test)] NotACommit }` — "Not `#[non_exhaustive]`: consumers match every variant, with no wildcard arm" |
| `RowId` | `history.rs` | `enum { Commit(Oid), #[cfg(test)] NotACommit }` Copy, Hash |
| `HistoryRow` | `history.rs` | `{ content: RowContent, graph: GraphRow }`, `id() -> RowId` |
| `Lane`, `EdgeKind`, `EdgeSegment`, `GraphRow` | `graph.rs` | `Lane(usize)`; `EdgeKind { Passing, IntoCommit, OutOfCommit }`; `EdgeSegment { from, to, kind, out_of_order }` with ctors `passing`, `into_commit`, `out_of_commit`, `marked_out_of_order`; `GraphRow { id: Oid, lane: Lane, edges: Vec<EdgeSegment> }` |
| `LaneAssigner` | `lane_assignment.rs` | `DEFAULT_WINDOW = 1024`, `REMEMBERED_PER_ROW = 16`; `new`, `with_window`, `window`, `remembered`, `assign_all`, `rows`, `into_rows`, `push(id, parents) -> Option<GraphRow>` |
| `Confirmed` | `confirm.rs` | `{ acknowledged: String }` private; `by_user(prompt)`, `acknowledged()`; derives `Debug, Clone, PartialEq, Eq` |
| `Secret` | `secret.rs` | one `Zeroizing<Vec<u8>>` field, NO derives; `new(Vec<u8>)`, `from_string(String)`, `expose_secret() -> &[u8]`, `len`, `is_empty`; `Zeroize`, `ZeroizeOnDrop`; four `compile_fail` doctests |
| `RemoteSummary` | `remote.rs` | `{ name: String, url: Option<String> }` |
| `AskpassToken`, `HELPER_PROGRAM`, `SOCKET_VARIABLE`, `TOKEN_VARIABLE` | `askpass.rs` | token newtype with a redacting `Debug`; the three wire names |
| `PromptKind`, `prompt_subject` | `prompt.rs` | prompt classification for the credential dialog |

There is no path type, no tree-entry type, no hunk/line type, no diff option
type, and no revision-spec type in `cairn-model` today. `Oid` is the only
object identity; nothing names a blob or a tree as distinct from a commit.

### 4.2 Dependency allowlist and rules

`crates/cairn-model/Cargo.toml` has exactly one dependency: `zeroize`
(workspace, `default-features = false`, `features = ["alloc"]`). The root
`CLAUDE.md` rule: "A change to `cairn-model`, to anything under
`cairn-git/src/ops/`, or to `cairn-guards` requires a test in the same commit".
Model tests: unit tests in each file, plus `tests/lane_assignment.rs` with
`tests/histories/mod.rs` (a literal-history builder, `random_history(seed, len,
skew)`, and the three structural assertions `assert_every_parent_edge_is_drawn`,
`assert_rows_are_well_formed`, `assert_the_picture_joins_up`).

The `RowContent` exhaustiveness rule (root `CLAUDE.md` Invariants) means a diff
packet that adds a row kind (e.g. a working-tree row) must touch every match on
`RowContent`/`RowId` outside `cairn-model`: today those are in
`crates/cairn-git/src/history.rs` (tests), `crates/cairn-git/tests/history.rs`
(`hex_id`, `commit_of`), `crates/cairn-ui/src/*` and `crates/cairn-app/src/*`
(the guard in section 5 lists the matcher).

## 5. `crates/cairn-guards/tests/invariants.rs` — the guards a diff packet meets

### 5.1 Every guard test, in file order

| Test | What it pins | What it scans |
| --- | --- | --- |
| `every_product_crate_is_on_the_product_roster` | every crate under `crates/` with a `Cargo.toml` and `src/` is in `PRODUCT_SOURCE_DIRS` (or `NOT_PRODUCT_SOURCE`), and every row still exists | directory walk |
| `every_crate_that_renders_is_on_the_render_roster` | every crate that ships `freya` is in `RENDER_SOURCE_DIRS`, and each row still ships it | all `crates/*/Cargo.toml` |
| `layer_dependencies_are_allowlisted` | every crate has a `DEPENDENCY_ALLOWLIST` row; shipped deps within it; dev-deps within it plus `TEST_ONLY_ALLOWLIST`; no dead rows | all `crates/*/Cargo.toml`, every dependency table including `[target.*]` |
| `the_allowlist_check_rejects_a_sealed_crate_in_every_dependency_table` | matcher self-test over `[dependencies]`, `[build-dependencies]`, `[dev-dependencies]`, target tables | in-memory manifests |
| `the_seal_scan_reads_tests_as_well_as_src` | meta-guard: the seal scan reads `src/` and `tests/` | sealed crate dirs |
| `layers_never_name_the_crates_they_are_sealed_from` | the crate seals (`FORBIDDEN_IDENTS`) | whole crate directories, every `.rs`, comments blanked, strings kept |
| `every_view_of_a_row_names_every_kind_of_row` | no partial read of `RowContent`; at least one non-exempt file names it | every crate except `ROW_CONTENT_EXEMPT` = `cairn-model`, `cairn-guards` |
| `the_row_content_matcher_catches_the_shapes_it_claims` | matcher self-test (24 caught, 14 ignored shapes) | in-memory |
| `only_the_ops_module_mutates_a_repository` | no file outside `crates/cairn-git/src/ops` spawns a `git` subprocess (a line naming `"git"` together with `Command`, `new(` or `cmd(`; strings NOT blanked) | `PRODUCT_SOURCE_DIRS` minus `ops/` |
| `every_git_invocation_disables_the_terminal_prompt` | outside `ops/environment.rs`: no naming of `Command`, no `Command::new`, no `SPAWN_SPELLINGS`, no `.env*()` call, no `GitEnvironment` literal or `impl`; inside it, the positive assertions (one `Command::new`, one literal, `env_clear` and `envs`, `ALWAYS` contents, the four askpass names) | `PRODUCT_SOURCE_DIRS`, test modules and strings blanked |
| `the_process_environment_matcher_catches_the_shapes_it_claims` | matcher self-test | in-memory |
| `the_ui_thread_never_waits_on_repository_work` | worker files never name `RENDERING_IDENTS`; render files never wait (`WAITING_IDENTS`, `WAITING_NULLARY_CALLS`); `cairn-app` render files never name `cairn_git`, `gix`, `cairn_askpass`; nonzero file counts both sides | `RENDER_SOURCE_DIRS` partitioned on `WORKER_DIR` |
| `a_history_sized_list_renders_through_a_virtualizing_view` | no render file names `ScrollView` unless in `UNBOUNDED_VIEW_EXCEPTIONS`; some production render file uses `VirtualScrollView` with `HistoryRow`; no dead rows | `RENDER_SOURCE_DIRS` minus `WORKER_DIR` |
| `the_unbounded_view_matcher_catches_the_shapes_it_claims` | matcher self-test | in-memory |
| `destructive_operations_are_sealed_behind_the_confirmation_token` | `confirm.rs` has the private `acknowledged: String` and exactly two `pub fn`; some `ops/` file names `Confirmed` | `cairn-model/src`, `cairn-git/src/ops` |
| `no_credential_value_is_logged_printed_serialised_or_stored` | the whole `Secret` contract (rosters below) | every crate except `cairn-guards` |
| `the_credential_matcher_catches_the_shapes_it_claims` | matcher self-test | in-memory |
| `ci_runs_every_merge_bar_gate_step` | every `gate.sh --step` CI names exists; every step but `test-fast` runs in CI | `scripts/gate.sh`, `.github/workflows/ci.yml` |
| `the_ssh_criteria_are_required_wherever_they_can_run` | CI sets `CAIRN_REQUIRE_SSH_FIXTURE: 1` and installs `openssh-server`; the gate probes the `SBIN` roster | `ci.yml`, `gate.sh`, `tests/remotes/ssh.rs` |

A second test binary, `crates/cairn-guards/tests/debris_hook.rs`, pins the
Stop-hook debris scan (`.claude/hooks/qa-stop.sh`); it echoes the crate-seal
rule.

### 5.2 The tables, verbatim

```rust
/// Crates whose dependency list is pinned; a crate with no row here fails.
const DEPENDENCY_ALLOWLIST: &[(&str, &[&str])] = &[
    ("cairn-model", &["zeroize"]),
    // `nix`: SIGTERM on cancel, so git can remove its lock files (issue #19).
    ("cairn-git", &["cairn-model", "gix", "nix", "thiserror"]),
    ("cairn-ui", &["cairn-model", "freya"]),
    (
        "cairn-app",
        &[
            "cairn-askpass",
            "cairn-git",
            "cairn-model",
            "cairn-ui",
            "freya",
        ],
    ),
    ("cairn-guards", &["toml"]),
    // Every crate here runs in a process holding a plaintext secret; keep it this short.
    ("cairn-askpass", &["cairn-model", "zeroize"]),
];

/// What a crate may take as a dev-dependency beyond its [`DEPENDENCY_ALLOWLIST`] row.
const TEST_ONLY_ALLOWLIST: &[(&str, &[&str])] = &[
    ("cairn-ui", &["freya-testing"]),
    ("cairn-app", &["freya-testing"]),
    // The fetch tests serve a real askpass channel; the engine never links the helper.
    ("cairn-git", &["cairn-askpass"]),
];

/// Crate directory → crate identifiers it may never name in code, in `src/`, `tests/` or anywhere
/// else under it.
const FORBIDDEN_IDENTS: &[(&str, &[&str])] = &[
    (
        "crates/cairn-model",
        &["gix", "freya", "cairn_git", "cairn_ui"],
    ),
    ("crates/cairn-ui", &["gix", "cairn_git"]),
    ("crates/cairn-git", &["freya", "dioxus", "cairn_ui"]),
    // The helper holds a plaintext secret: no engine, no toolkit, and no logging framework.
    (
        "crates/cairn-askpass",
        &[
            "gix",
            "freya",
            "dioxus",
            "cairn_git",
            "cairn_ui",
            "tracing",
            "log",
        ],
    ),
];

const PRODUCT_SOURCE_DIRS: &[&str] = &[
    "crates/cairn-model/src",
    "crates/cairn-git/src",
    "crates/cairn-ui/src",
    "crates/cairn-app/src",
    "crates/cairn-askpass/src",
];
const RENDER_SOURCE_DIRS: &[&str] = &["crates/cairn-ui/src", "crates/cairn-app/src"];
const NOT_PRODUCT_SOURCE: &[&str] = &["cairn-guards"];

/// Where repository work runs, and so the only place waiting is allowed.
const WORKER_DIR: &str = "crates/cairn-app/src/worker";
/// What a file must not name to count as rendering nothing.
const RENDERING_IDENTS: &[&str] = &["freya", "dioxus", "cairn_ui"];

const UNBOUNDED_VIEW: &str = "ScrollView";
const VIRTUALIZING_VIEW: &str = "VirtualScrollView";
/// Render files allowed to name [`UNBOUNDED_VIEW`] anyway, and why. Empty on purpose.
const UNBOUNDED_VIEW_EXCEPTIONS: &[(&str, &str)] = &[];

/// Crates that may read `RowContent` however they like: its owner, and this suite's fixtures.
const ROW_CONTENT_EXEMPT: &[&str] = &["cairn-model", "cairn-guards"];

const SECRET_TYPE_FILE: &str = "crates/cairn-model/src/secret.rs";
const SECRET_TYPE: &str = "Secret";
const SECRET_ACCESSOR: &str = "expose_secret";
const SECRET_FORBIDDEN_TRAITS: &[&str] = &[
    "Debug", "Display", "Clone", "Copy", "Serialize", "Deserialize", "Encode", "Decode",
];
const SECRET_READERS: &[&str] = &[
    SECRET_TYPE_FILE,
    "crates/cairn-askpass/src/protocol.rs",
    "crates/cairn-askpass/src/main.rs",
];
/// Files whose `struct`s may hold a [`SECRET_TYPE`] in a field. Empty on purpose.
const SECRET_HOLDERS: &[&str] = &[];

const PROCESS_ENVIRONMENT_FILE: &str = "crates/cairn-git/src/ops/environment.rs";
const PROCESS_ENVIRONMENT_TYPE: &str = "GitEnvironment";
const SPAWN_SPELLINGS: &[&str] = &[
    "posix_spawn", "posix_spawnp", "execv", "execve", "execvp", "execvpe", "execveat", "fexecve",
];
```

From `crates/cairn-guards/src/lib.rs` (the matchers):

```rust
/// Identifiers whose presence means the code can wait; an alias is caught on its import line.
const WAITING_IDENTS: &[&str] = &[
    "Barrier", "Condvar", "JoinHandle", "Mutex", "Receiver", "RwLock",
    // Constructors: `for update in rx {}` blocks without naming anything above.
    "bounded", "channel", "sync_channel", "unbounded",
    "block_on", "blocking_lock", "blocking_recv", "blocking_send",
    "park", "park_timeout", "recv_deadline", "recv_timeout", "scope", "select", "sleep",
    "wait_timeout", "wait_while",
    // These spin rather than block.
    "spin_loop", "try_iter", "try_lock", "try_recv", "yield_now",
];
/// Methods that wait when called with no arguments; each has an innocent one-argument namesake.
const WAITING_NULLARY_CALLS: &[&str] = &["join", "lock", "recv", "wait"];
/// Methods that put something into, or take something out of, a child process's environment.
const ENVIRONMENT_METHODS: &[&str] = &["env", "envs", "env_clear", "env_remove"];
```

(The `WAITING_IDENTS` list is reflowed here from one-per-line in the source;
the entries are verbatim.) The `RowContent` matcher
(`reads_row_content_partially`) catches: a wildcard or catch-all arm in a
match that names `RowContent` (including `Some(_)` beside `Some(RowContent::..)`,
`ref`/`mut`/`_x`/`rest @ _`/`&_`/`&other` bindings, guarded and attributed
wildcards, a wildcard inside an or-pattern), `if let`, nested `if let`, `while
let`, let-chain, `let .. else`, `matches!` (even spaced across lines), and a
glob, variant, grouped-variant or aliased `use` of `RowContent`. There is NO
list of exempt `worker/` functions in code; the worker partition is purely the
`WORKER_DIR` path prefix, and the "exempt while on the UI thread" functions the
root `CLAUDE.md` names are a review obligation only.

### 5.3 Partitioning facts

- Worker side = any `.rs` under `crates/cairn-app/src/worker`. It may wait
  freely but must not name `freya`, `dioxus`, `cairn_ui`. It is still inside
  `PRODUCT_SOURCE_DIRS`, so the process guards apply to it too. `crates/cairn-app`
  has no `FORBIDDEN_IDENTS` row, so the worker may name `cairn_git`, `gix`,
  `cairn_askpass`; those three are banned only on `cairn-app` RENDER files.
- Render side = every other `.rs` under `crates/cairn-ui/src` and
  `crates/cairn-app/src`.
- `every_git_invocation_disables_the_terminal_prompt` scans the five product
  `src/` dirs with strings and `#[cfg(test)]` modules blanked; crate `tests/`
  directories are out of its scope.

### 5.4 What a diff packet would trip, concretely

- **A new `cairn-model` type.** Allowed with no roster change so long as it
  needs no crate beyond `zeroize` and its file (tests included) never names
  `gix`, `freya`, `cairn_git`, `cairn_ui`. A diff model therefore cannot alias
  or wrap a `gix_diff` type. If the packet adds a `RowContent` variant, every
  match on `RowContent` outside `cairn-model` must already be exhaustive by
  name (they are), so the addition is a compile error at each, which is the
  intended enforcement; the guard forbids dodging it with `_`, `if let` or
  `matches!`. A separate diff enum gets no exhaustiveness guard of its own.
- **A new `cairn-git` read module (`src/diff.rs`, say).** Allowed: `gix` is on
  the row and `cairn-git` is sealed only from `freya`, `dioxus`, `cairn_ui`. It
  may not name `Command` at all, nor any spawn spelling, nor call `.env*()`; and
  it may not have a line naming `"git"` beside `Command`, `new(` or `cmd(`
  (that matcher does not blank strings, so `"git"` in a string on a `Foo::new(`
  line trips it). A pure-gix read satisfies all of this.
- **A new worker QUERY kind.** Worker files may not name `freya`, `dioxus`,
  `cairn_ui`, so a diff response must be spelled in `cairn-model` (or
  `cairn-git`) types. Any render-side handler in `crates/cairn-app/src` outside
  `worker/` may not name `cairn_git`, `gix`, `cairn_askpass`, nor any
  `WAITING_IDENTS` entry — `channel`, `Receiver`, `Mutex`, `select`, `sleep`,
  `try_recv`, bare `.join()`/`.lock()`/`.recv()`/`.wait()` all fail on a render
  path — so channel plumbing stays inside `worker/`. `Request` and `Update`
  derive `Debug`, so nothing carrying a `Secret` can go in them (already true
  today; `Reply` is separate).
- **A new `cairn-ui` diff view.** Naming `ScrollView` fails outright:
  `UNBOUNDED_VIEW_EXCEPTIONS` is empty and the matcher sees imports, paths and
  type annotations; a `(file, reason)` row is the documented review route.
  `VirtualScrollView` needs no roster edit (word-boundary match, so it does
  not count as `ScrollView`), and the positive arm is satisfied by
  `history_list.rs` regardless. The view may not name `gix` or `cairn_git`, nor
  wait.
- **No dependency addition** is needed for gix diff/status (section 1.9), so
  `layer_dependencies_are_allowlisted` and `--step deps` are untouched unless
  a crate is added.

## 6. What `docs/systems/` says a diff packet inherits

### 6.1 `docs/systems/history-graph.md`

The worker boundary (D3), as the as-built doc states it:

- `crates/cairn-app/src/worker/` is "the only place in the binary that may name
  `cairn_git` or `gix`, and the only place that may wait. The partition is by
  FILE, and it is a guard, not a convention."
- One open repository, one thread-local handle, taken once:
  `SharedRepository::to_worker()` is called once at the top of the thread; a
  per-request conversion "compiles and passes every test while rebuilding the
  object cache and the pack snapshot each time". `a_shared_repository_can_cross_threads`
  pins `Send + Sync` (the twin for gix's `parallel` feature staying on).
- `RepositoryHandle::submit(Request) -> Epoch` returns immediately over an
  unbounded channel; `Updates::next()` is `try_recv` then a park on
  `worker/wake.rs`, "a one-slot latch a worker sets. There is no timer and no
  async-runtime dependency."
- "The epoch IS the cancel signal": `Superseded` implements `cairn_git::Cancel`
  as "is my epoch still current", pinned by
  `superseding_a_request_stops_the_walk_that_is_serving_it`. Dropping
  `Updates` stops everything.
- Debounce contract for any later packet: "Every `submit` of a QUERY supersedes
  (an operation carries no epoch), and a superseded page delivers nothing — so
  the caller must debounce." `Progress::wants_more()` is the history's debounce.
- A failed page is retried on the next approach: `serve` drops the live session
  on error but keeps the cursor, so the retry cold-restarts from the last good
  page (`a_failed_request_is_answered_when_it_is_asked_again`).
- "Shaped for a second consumer, none of it stubbed": a request is answered by
  a STREAM of `Update`s; a worker runs ordinary blocking code; workers are
  pinned to a purpose, not fed from an anonymous queue. Fetch landed as exactly
  that: its own thread, its own `Update` variants, requests that carry no epoch.
- A dead worker is announced (`Update::WorkerLost`), and the job channel is
  closed BEFORE the waiting task is woken; the standing review obligation is
  that `Sender<Envelope>` is `Clone` and a bare copy compiles.

Constants the doc names: `LaneAssigner::DEFAULT_WINDOW` = 1024;
`REMEMBERED_PER_ROW` = 16; `Repository::OBJECT_CACHE_BYTES` = 4 MiB;
`WORKERS_PER_REPOSITORY` = 1; `PAGE_ROWS` = 64; `PREFETCH_ROWS` = 24
(`crates/cairn-ui/src/history_list.rs`); `ROW_HEIGHT` 26, `LANE_WIDTH` 14,
`MAX_DRAWN_LANES` 24 (`crates/cairn-ui/src/graph_geometry.rs`).

Known limits it records that a diff pane meets: memory is flat in history
length but linear in rows scrolled (~660 B/row retained, nothing evicts, issue
#4); the walk's seen-set is ~15 MB at 500k commits; no random access by row
offset (issue #5); "`GraphRow` still keys a row by `Oid` — `RowContent` and
`RowId` are total over rows that are not commits, but the assigner's own output
is not"; the view is `HEAD`'s ancestry (`from_commits` exists; packet 4 owns the
switch); the first page walks `window + limit` commits.

**The measured bar (A7), as recorded.** The PRD's A7 reads: "Scrolling a
repository with at least 100k commits keeps frame time bounded and memory flat
— a measured check, run by hand against a named real repository, with numbers
recorded in `progress.md`", and "A7 is deliberately not automated. A frame-time
assertion in CI would be flaky and would be disabled within a month." The PRD's
teardown stamp calls A7 "the one qualified pass: its 100k half was measured
against a synthetic row vector because no repository that size was available,
and the one limitation it found is issue #4." The numbers live in the deleted
work directory, recoverable with
`git show f4a9261^:docs/work/history-graph/progress.md`:

- Real repository: `/home/alexparlett/Development/freya`, **2,540 commits from
  `HEAD`** (2,896 across all refs) — "no repository with 100k commits exists on
  this machine"; release build, paged to completion with `End`. RSS 192.1 MB
  at the first page (64 rows), 196.2 MB at 2,540 rows: +4.0 MB for 2,476 rows,
  about 1.6 KB/row including the walk's seen-set and the assigner's window.
- Synthetic row vector (instrumented build, reverted, not committed): rows
  built per render 34-35 steady / 119 on a viewport jump at both 1,000 and
  100,000 rows; CPU for 60 `PageDown`s 0.17-0.18 s at every size (~3 ms per
  jump); RSS 152.3 MB at 1,000 rows, 157.6 MB at 10,000, 216.6 MB at 100,000
  (linear in rows SCROLLED, ~660 B/row).
- No hardware is recorded beyond "16 cores" (in the concurrency entry) and
  "this machine".

Other measurements the as-built doc quotes (each from an `#[ignore]`d reporter
in `cairn-git` driven by `CAIRN_BENCH_REPO`, optional `CAIRN_BENCH_LIMIT`, run
`--release`): lane window cost 66 ms at 1024 / 749 ms at 4096 / 23.3 s at 16384
over 50k skewed rows; out-of-order rows 0-0.2% in the default order across
seven local repositories; p99 8 edge segments per row and 264 B per row on the
widest (`freya`), single-digit lanes throughout ("real retained layout is up to
about twice" that, 126-250 MB extrapolated to 500k); 50k commits walked and
laid out in 129 ms commit-time vs 196 ms graph order; object cache 178 ms
without vs 116 ms with 4 MiB ("more bought nothing") on a 200,001-commit
`fast-import` history; concurrent walks 2.1x / 4.2x / 7.9x at 2/4/8 threads
(`measures_concurrent_walks_against_a_named_repository`, `repository.rs`).

The repository sample the history research used
(`docs/research/history-graph/scroll-memory-model.md`, Part D): `freya` (2,896
commits, 91 refs), `strata` (1,081, 176), `cairn` (43, 4), `hyprland` (535,
20), `dungeon-siege-reborn` (71, 3), `nct6687d` (168, 7), and the bare cargo
clone of freya (2,540, 4). Finding 28 states the weakness: "No repository with
more than 50,000 commits exists locally ... Nothing was cloned for this
measurement." **There is no "10-year monorepo" fixture anywhere in the repo**;
the phrase in the root `CLAUDE.md` is doctrine, not a named repository. The
re-run form is:

```
CAIRN_BENCH_REPO=<path> cargo test -p cairn-git --release --lib -- --ignored --nocapture measures_layout
```

The bar doctrine itself, in three places: `docs/design/cairn.md` ("Every read
surface carries a stated bar against a named real repository, the way
`history-graph`'s A7 does"), `docs/design/feature-inventory.md` ("Every surface
in Tier 0 and Tier 1 needs a stated, measured bar"), and
`docs/work/daily-loop/brainstorm.md` L6. The roadmap brief for this packet
says only "Carries a measured bar (L6): diffing a large commit in a large
repository." The house form for a bar is an `#[ignore = "reason"]` reporter
using `eprintln!` (both exempt from the debris hook on committed lines; a bare
`#[ignore]` is flagged).

### 6.2 `docs/systems/credentials.md`

Facts a diff packet (and packet 5 after it) inherits:

- End to end today: fetch only. Push, clone and anything else that could prompt
  are not built.
- `GitEnvironment::new(parent, &Askpass)` sets `GIT_TERMINAL_PROMPT=0`,
  `SSH_ASKPASS_REQUIRE=force`, `GIT_ASKPASS`, `SSH_ASKPASS`,
  `CAIRN_ASKPASS_SOCKET` (when a socket exists); the token
  `CAIRN_ASKPASS_TOKEN` is per invocation via `GitCommand::authorized_by`.
  There is no environment without an `Askpass`. The inherited roster is in
  section 2; `DISPLAY`/`WAYLAND_DISPLAY`, `GNUPGHOME` and pinning `GIT_EDITOR`
  are open on issue #18 (`GIT_EDITOR` "is due with the first verb that can open
  an editor" — a commit verb, packet 5).
- Runner: `GitCommand::stream` reads stderr on its own thread; cancel is
  `SIGTERM` then `SIGKILL` after `TERMINATION_GRACE` (2 s); stranded locks are
  listed after the reap. Nothing in the doc describes stdin for a verb; the
  code closes it (section 2).
- `Performed` and `Invalidated` as in section 2; the doc notes the worker today
  does less than the contract — it compares `ref_tips` and reads nothing else
  off the `Performed` (issue #25).
- Locked decisions L1-L11 (Cairn stores no credential; helper is a separate
  binary; `GIT_TERMINAL_PROMPT=0` always; backend and helper landed together;
  explicit environment; no secret on `argv`; a working setup is not degraded;
  push deferred to issue #16; git floor 2.30; unix socket under
  `$XDG_RUNTIME_DIR` with the stated threat model; `zeroize` accepted).
- Lessons recorded that bear on new code: `cargo test --all-targets` never runs
  doctests (hence `test-doc`); tests in `crates/cairn-app/src` may not name
  `Command` (the terminal-prompt guard scans that crate's `src/` with test
  modules blanked only for the process guard — tests that need a real remote
  live in `crates/cairn-git/tests/`); stub-git tests retry on `ETXTBSY`;
  Clippy's `allow-expect-in-tests` does not cover helpers in `tests/*.rs`
  outside a `#[test]` fn.

### 6.3 `docs/design/cairn.md` decisions in one line each

D1 gitoxide reads, `git` subprocess writes (hooks are the deciding argument;
never the CLI on a read path). D2 credentials delegated to git entirely
(amended 2026-09-17, issue #22: Cairn's helper IS the askpass while Cairn runs
git). D3 a worker pool per repository, one `ThreadSafeRepository`, epoch per
request (as built: one worker; the epoch is the cancel). D4 lanes assigned
incrementally in the engine, correct under out-of-order arrival. D5 macOS
deferred, accelerator table kept. D6 conflicts resolved structurally. D7 the
daily loop is the first version worth having (this program). D8 worktrees
first-class. D9 forge links in scope, forge APIs out.

### 6.4 The roadmap brief for this packet, and packet 5's

`docs/work/daily-loop/roadmap.md`, packet 3 (`diff-engine`, depends on 1):
"Builds: the diff model and its rendering. Commit diffs, working-tree diffs,
and comparing two arbitrary revisions. The commit details pane. Diff options:
whitespace handling, word-level intra-line diff, context lines, rename
detection. The thing this packet must not get wrong (L2): the model is
patch-capable, not display-shaped. It must emit a valid patch for an arbitrary
subset of hunks and lines — correct hunk headers, correct context — because
that is how packet 5 stages a single line. Build the patch emitter and its
round-trip test in this packet even though nothing consumes it yet. Nine
consumers to design against: commit details, working-tree changes, hunk
staging, line staging, compare revisions, conflict resolution (D6), image
diffs, stash contents, interactive-rebase preview. Open: O1 — side-by-side,
unified, or both, and whether the patch model is genuinely independent of
presentation. Carries a measured bar (L6): diffing a large commit in a large
repository. Out: image diffs, conflict resolution, staging of any kind."

Packet 5 (`staging-and-commit`, depends on 2, 3, 4): "Depends on packet 3 for
patch construction and packet 2 for the backend — `git apply --cached` is how a
partial stage happens." It is the first packet with destructive operations,
ships the reflog view and the visible operation log, and decides auto-stash
(O3). Packet 4 (`refs-and-status`) owns working-tree status and the switch from
`from_head` to `from_commits`; it is independent of packet 3.

`docs/research/backend-split/gix-write-path-coverage.md` has no diff row at
all; it lists "Blame, status, dirwalk" as present and "Hunk-level staging" as
absent ("nothing constructs a partial blob from selected hunks"). Nothing in the
research corpus establishes gix's diff coverage for this packet.

## 7. The gate and the reviewer dispatch table

### 7.1 `scripts/gate.sh` steps

| `--step` | Command | `--fast` | full run |
| --- | --- | --- | --- |
| `format` | `cargo fmt --all --check` | yes | yes |
| `lint` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | yes | yes |
| `typecheck` | `cargo check --workspace --all-targets --all-features` | yes | yes |
| `guards` | `cargo test -p cairn-guards` | yes | yes |
| `deps` | `cargo deny check advisories bans sources licenses` | no | yes |
| `test-fast` | `cargo test --workspace --lib --bins` | yes | replaced by `test-full` |
| `test-full` | `require_ssh_fixture_where_possible` then `cargo test --workspace --all-targets` | no | yes |
| `test-doc` | `cargo test --workspace --doc` (the `Secret` compile-fail pins) | no | yes |

An empty step FAILS loudly rather than skipping. `ci_runs_every_merge_bar_gate_step`
requires CI to run every step but `test-fast`. `.githooks/pre-push` runs
format, `cargo check` and the guard suite; `.claude/hooks/qa-stop.sh` is the
per-turn debris scan (its rules are pinned by `crates/cairn-guards/tests/debris_hook.rs`).

### 7.2 `docs/qa-gate.md` reviewer dispatch table

| Diff surface | Reviewer | Deterministic twin |
| --- | --- | --- |
| Anything (end of contribution) | `qa-checklist` | `scripts/gate.sh` |
| Tests added/changed, or behaviour changed without tests | `test-coverage-auditor` | the suite itself |
| The enforcement layer: guards, hooks, `scripts/`, CI workflows, reviewer/skill definitions, `.claude/`, `.githooks/`, `crates/cairn-guards/`, `docs/qa-gate.md` | `gate-integrity-reviewer` | direct guard-rule tests where they exist |
| Docs (any tense) | `qa-checklist` (docs tier: tense discipline per `docs/CLAUDE.md`) | review |
| Anything under `crates/cairn-git/src/ops/`, or any new call site that reaches one | `destructive-ops-reviewer` | `destructive_operations_are_sealed_behind_the_confirmation_token`, `only_the_ops_module_mutates_a_repository`, `every_git_invocation_disables_the_terminal_prompt` |
| Anything naming `cairn_model::Secret` or `expose_secret`, anything under `crates/cairn-askpass/`, a new type holding a credential | `qa-checklist` (item 10) | `no_credential_value_is_logged_printed_serialised_or_stored` + the compile-fail doctests |
| `crates/cairn-ui/`, `crates/cairn-app/`, or anything that changes what runs per frame or per repository query | `responsiveness-reviewer` | `the_ui_thread_never_waits_on_repository_work`, `a_history_sized_list_renders_through_a_virtualizing_view`, `only_a_viewport_of_rows_is_built_however_long_the_history` |

Matching rows dispatch in parallel, each spawned fresh; `qa-confirm`
adjudicates raw findings. For this packet's likely surface — `cairn-model`,
`cairn-git/src/` (not `ops/`), `worker/`, `cairn-ui`, `cairn-app` — the
reviewers that fire are `qa-checklist`, `test-coverage-auditor` and
`responsiveness-reviewer`; `destructive-ops-reviewer` fires only if the packet
touches `ops/` (a stdin path for the runner would). Review diff scope is the
union of `main...HEAD`, staged/unstaged changes and untracked files. Packet
phases run `/qa` at each phase end; the packet's final QA phase is a standalone
fresh session auditing the dismissal log.

`docs/CLAUDE.md` places `research/<slug>/` as evidence — "recon and audit
reports saved IN FULL ... never deleted at teardown and never retro-edited" —
with no frontmatter or heading template; existing research docs open with a
dated "Evidence record" paragraph and a method statement, which this file
follows. Open questions belong in the packet's `brainstorm.md` and the design
spine per that contract; the section below is the audit's list of what it
could not determine, for the planner to carry there.

## Open questions for the planner

What this record could not settle from the code and docs as they stand:

1. **Where a diff query runs.** `serve` owns the one `Repository` handle and the
   live walk on the `cairn-repository` thread; the operations thread owns a
   second handle but takes only `Operation::Fetch`. Nothing states whether a
   diff should queue behind a history page on the repository thread, run on
   the operations thread, or get a fourth thread (D3's "pool" is one worker by
   `const` assertion, with a note that more "need a routing decision").
2. **Whether a diff query shares the history's epoch.** One `Epochs` counter
   numbers every query; `Updates::next` drops any numbered envelope that is not
   current. A diff numbered by it would be cancelled by the next scroll and
   would cancel a page in flight; a diff carrying `None` could never be
   superseded. The design note only settles scroll-vs-fetch.
3. **Whether a diff needs a `Cancel`.** `Cancel` is polled per commit in the
   walk; gix's tree diff offers `Action::Cancel` from the callback and the
   status platform an `AtomicBool`. No engine read but history is cancellable
   today.
4. **The error vocabulary.** `Error` has no variant for a non-commit object, a
   missing path, an unreadable index, binary content, or a filter/driver
   failure; the enum is shared by reads and ops.
5. **Which real repository carries the bar.** Nothing over 2,896 commits is on
   the machine; no repository is named for "a large commit in a large
   repository", and no hardware is recorded for the history numbers.
6. **The unified-patch contract with packet 5.** `GitCommand` closes stdin
   (`Stdio::null()` in both `run` and `stream`); whether packet 5 will pipe a
   patch or pass a file is undecided, and either way is an `ops/` change with
   its own reviewer. What "valid" means for `git apply --cached` (line
   endings, `\ No newline at end of file`, mode lines, rename headers, index
   lines) is not specified anywhere yet.
7. **Whitespace and word-level options.** `gix-imara-diff 0.2.5` exposes
   `Algorithm { Histogram, Myers, MyersMinimal }` and `sources::{lines, words,
   ..}` but no whitespace-ignoring tokeniser; whether those options are
   tokenisation the packet writes, or something gix's blob platform provides
   through `diff.<driver>`/attributes, was not established.
8. **Working-tree diffs and the index snapshot.** The `ops/mod.rs` contract
   says gix shares one index snapshot across handles and re-reads on mtime;
   nothing in `cairn-git` opens the index today, so which handle reads it and
   when it is refreshed is unwritten.
9. **The row identity a diff pane keys on.** `View.selected` is a `RowId`
   nothing reads back; `GraphRow` keys by `Oid`; a working-tree row has no
   identity yet (packet 4's problem, but a commit-details pane meets it).
10. **Rename detection defaults.** gix fills `Options` from `diff.renames` and
    defaults to 50% when unconfigured; whether Cairn honours the user's config
    or fixes its own default was not decided anywhere.
11. **`cairn-model` growth.** Whether the diff model can stay within the
    `zeroize`-only allowlist (it has no reason not to) and whether a diff type
    becomes a `RowContent` variant or a separate type — the guard consequences
    of each are in section 5.4.
