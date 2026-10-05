# Finding a ref's commit by paging the held walk, measured

Research for the `refs-and-status` packet's R8.5 (`docs/prd/refs-and-status.md`:
"a row not yet loaded is found by paging the held walk forward until it arrives"),
recorded 2026-10-05. It decides nothing: it measures what such a find costs in time
and retained memory on a big real repository, how fast a superseding press stops
it, and what the history-graph PRD's "54-87 minutes" figure measured. Evidence
record, saved in full; historical, never retro-edited.

## Headlines

- **Time is not the problem.** On the full rust-lang/rust clone (340,228 commits
  from `HEAD`, 345,545 from every ref, no commit-graph file), paging Cairn's real
  `HistorySession` 64 rows a page with lane layout and keeping every row reaches
  depth 10k in **91 ms**, 50k in **0.43 s**, 100k in **0.82 s**, 300k in **2.19 s**,
  and the deepest tag, `release-0.1` (row 332,649 from `HEAD`, 337,966 from every
  ref), in **2.37 s / 2.41 s**. Paging to the very end takes **2.40 s / 2.44 s**.
  That is **110-142k rows per second**, rising with depth. Seeding from `HEAD` or
  from all 175 refs costs the same at a given depth.
- **Memory is the problem.** Every row loaded on the way is kept, as the window
  keeps them today (issue #4), and on this repository a row is much larger than the
  ~660 B `docs/systems/history-graph.md` cites. Measured from the rows' own
  capacities, a row holds **4.3-6.3 KB**, so a find **retains 55 MiB at depth 10k,
  575 MiB at 100k, 1.37 GiB at 300k, and 1.39 GiB at the deepest tag.** Process
  anonymous RSS grows by the same amount (67 MiB, 595 MiB, 1.37 GiB, 1.41 GiB).
  About 89% of it is edge segments. Rust's graph is wide: 105-160 segments per
  row on average, against the p99 of 8 that Part D of
  `docs/research/history-graph/scroll-memory-model.md` measured on seven small
  repositories, none of them rust.
- **A miss costs a full walk.** A ref whose commit is not in a `HEAD`-seeded walk
  (tag `1.0.0` is not an ancestor of `HEAD`) is known to be missing only at the end
  of the history: **2.40 s, and 1.39 GiB of rows retained**, before the find can
  say so. Seeded from every ref, every ref's commit is in the walk by construction,
  and `1.0.0` is found at row 302,690 in 2.21 s.
- **A cancel is immediate.** Set mid-find, the cancel makes `next_page` return
  `Error::Cancelled` within **0.6-17 µs** (median 4-6 µs, 48 trials). Deep in the
  find, 190k-274k rows in, the median is **3.8-5.0 µs and the maximum 10 µs**. The
  walk polls once per commit: the median gap between polls is 6.3 µs, the p99
  17 µs, and the worst observed 2-9 ms (1-4 gaps over 1 ms per 333k polls).
- **The "54-87 minutes" figure is the replaying cursor, not a held session, and
  it was extrapolated, not measured.** It is the arithmetic sum of replay costs for
  paging to depth 500k 100 rows at a time. Each replayed page walks `k x limit`
  commits, at 2.6-4.2 µs per commit, timed on a 200,001-commit synthetic repository.
  It does not apply to paging a held `HistorySession`, which walks each commit once:
  the same find on rust takes 2.4 s. The replay path still exists as the cold
  restart: on rust one replayed page costs 60 ms at depth 10k and 1.61 s at 300k,
  and paging to the deepest tag by replay would take about 76 minutes (computed,
  not run).

## Subject, machine, code

- **Repository:** `~/Development/bench/rust`, a full non-shallow clone of
  rust-lang/rust. `HEAD` is `refs/heads/main` at
  `c999cef531ea9059e189e82fe0e82c5daf249bc9`. It has 340,228 commits reachable
  from `HEAD` and 345,545 from all refs (`git rev-list --count`, which agrees with
  Cairn's walk). It has 175 refs (1 head, 11 remote, 163 tags), one 1.0 GB pack,
  **no commit-graph file** (`.git/objects/info/` is empty), and no `core` settings
  beyond the clone's defaults. The repository was only read. See "Bench repository
  unchanged" below.
- **Machine:** AMD Ryzen 7 9800X3D (8 cores / 16 threads, max 5.27 GHz), 60 GiB
  RAM, Linux 7.2.8-2-cachyos, git 2.56.0. Rust 1.97.1, the toolchain
  `rust-toolchain.toml` pins.
- **Cairn:** commit `0e0b227673352b50c5bec0db1c65afd0b2e31a99` (the
  `feature/plan-refs-and-status` tip), in a detached scratch worktree. The worktree
  added one `#[ignore]`d reporter to the test module of
  `crates/cairn-git/src/history.rs` (source in the appendix). It was built with
  `cargo test --release -p cairn-git --no-run` into a scratch target directory,
  then removed.

## Method

The engine path is the one the application's history lane runs
(`crates/cairn-app/src/worker/pool.rs`, `Scroll::page`):

- `Repository::discover` opens the repository, which installs the 4 MiB object
  cache every worker handle carries (`Repository::OBJECT_CACHE_BYTES`).
- `Repository::history_session` takes a `HistoryRequest::from_head(64)` for
  seeding (a). For seeding (b) it takes `HistoryRequest::from_commits(tips, 64)`,
  where `tips` is every ref peeled to a commit (the existing reporter helper
  `every_ref_tip`: 175 refs, deduplicated).
- The default order is `CommitTime` and the default window is
  `LaneAssigner::DEFAULT_WINDOW` (1024 rows). The session lays out every row with
  its `LaneAssigner`.
- Then `HistorySession::next_page(64, cancel)` is called repeatedly. 64 is
  `PAGE_ROWS` in `crates/cairn-app/src/main.rs`, the page the application asks for.
  Each page's rows are appended to one `Vec<HistoryRow>` and kept, as the window
  keeps every loaded row. Each page is searched for the target id, and the find
  stops at the page that holds it, or at the end of the history.

Not included: sending each page to the UI thread as an `Update::Rows` and the
window appending it to its own vector (see OPEN below).

**Targets.** A discovery run paged each seeding to the end and recorded the id at
rows 10,000, 50,000, 100,000 and 300,000 *in Cairn's own walk order*, the last row,
and the row of several tags. The oldest tag by `creatordate`, `release-0.1`
(2012-01-20, 7,578 ancestors), is the deepest tag reachable. The deepest point of
all is the last row. Tag `1.0.0` is not an ancestor of `HEAD`, so a `HEAD`-seeded
walk never reaches it; it is the miss case.

**Runs.** Each find was a fresh process: the test binary was invoked directly with
`--ignored --nocapture --exact` and environment variables naming the seeding, mode
and target (`CAIRN_BENCH_REPO`, `CAIRN_FIND_SEED`, `CAIRN_FIND_MODE`,
`CAIRN_FIND_TARGET`). A fresh process gives each run its own RSS high-water mark
and its own allocator state. Every target got one warm-up run and then five
measured runs. Times are the median of the five. The OS page cache was warm, since
the 1 GB pack had been read many times before.

- *Time* runs from just before `history_session` until the page holding the target
  returns. Session open was 1.1-1.3 ms from `HEAD` and 2.2-2.6 ms from every ref,
  and the first page took about 9-10 ms. Both are included.
- *Retained rows* are summed from each kept row's capacities:
  `size_of::<HistoryRow>()` (216 B), plus edge segments
  (`edges.capacity() x 24 B`), parents, and the summary, name and e-mail strings,
  plus the outer vector's unused capacity. malloc's per-allocation overhead is not
  counted.
- *RSS* is `RssAnon` from `/proc/self/status`, read before the session opened and
  when the target arrived, with the rows and the session still alive. The
  difference includes the session's own state: gitoxide's seen-set, the assigner
  and the 4 MiB object cache. `VmHWM` is also reported. It includes file-backed
  pages of the mapped pack, so it runs about 200 MiB above the anonymous figure.
- *Cancel latency* was measured in a second set of runs. Each run opened the
  session on a worker thread and paged towards the last row with a `CancelSignal`.
  The main thread slept 50 to 2000 ms, read `Instant::now()` and called `cancel()`.
  The worker read `Instant::now()` when `next_page` returned `Err(Cancelled)`.
  There were 8 delays x 3 repetitions per seeding, 48 trials in all. The engine
  polls `Cancel::is_cancelled` once per commit. The application's
  `Superseded::is_cancelled` is one atomic load and a compare, the same cost as
  `CancelSignal`'s.
- *Poll gaps*: a `Cancel` that never cancels recorded the time between successive
  polls over a whole find to `release-0.1` (4 runs from `HEAD`, 3 from every ref).
- *Replay* (the cold-restart path): the session paged to a target, and its cursor
  was handed to `Repository::history(&HistoryRequest::resume(cursor, 64), ..)`.
  That call was timed four times (one warm-up, then three).
- *Walk vs layout split*: the existing reporter
  `measures_both_orders_against_a_named_repository` ran with
  `CAIRN_BENCH_LIMIT=340228`. That was a single run, not a median.

## Results

### Time and rows per second to find a target

The median is over five runs, after one warm-up. "Walked" counts commits pulled
off the walk; it runs ahead of the rows by up to the 1024-row window.

| Seeding | Target | Row found | Pages | Walked | Five runs (ms) | Median | Rows/s |
| --- | --- | ---: | ---: | ---: | --- | ---: | ---: |
| `HEAD` | depth 10k | 10,000 | 157 | 11,072 | 90, 90, 91, 92, 105 | 91 ms | 110k |
| `HEAD` | depth 50k | 50,000 | 782 | 51,072 | 424, 426, 426, 430, 434 | 426 ms | 117k |
| `HEAD` | depth 100k | 100,000 | 1,563 | 101,056 | 812, 814, 817, 821, 824 | 817 ms | 122k |
| `HEAD` | depth 300k | 300,000 | 4,688 | 301,056 | 2172, 2177, 2190, 2199, 2210 | 2.19 s | 137k |
| `HEAD` | `release-0.1` (deepest tag) | 332,649 | 5,198 | 333,696 | 2357, 2370, 2370, 2371, 2377 | 2.37 s | 140k |
| `HEAD` | last row | 340,228 | 5,317 | 340,228 | 2378, 2391, 2400, 2414, 2417 | 2.40 s | 142k |
| `HEAD` | `1.0.0`, **not in the walk** | not found | 5,317 | 340,228 | 2394, 2397, 2398, 2403, 2404 | 2.40 s | 142k |
| every ref | depth 10k | 10,000 | 157 | 11,072 | 91, 91, 93, 93, 93 | 93 ms | 108k |
| every ref | depth 50k | 50,000 | 782 | 51,072 | 422, 426, 430, 431, 433 | 430 ms | 116k |
| every ref | depth 100k | 100,000 | 1,563 | 101,056 | 813, 814, 816, 816, 821 | 816 ms | 123k |
| every ref | depth 300k | 300,000 | 4,688 | 301,056 | 2188, 2190, 2200, 2202, 2211 | 2.20 s | 136k |
| every ref | `1.0.0` | 302,690 | 4,730 | 303,744 | 2198, 2204, 2210, 2210, 2228 | 2.21 s | 137k |
| every ref | `release-0.1` (deepest tag) | 337,966 | 5,281 | 339,008 | 2401, 2404, 2407, 2411, 2418 | 2.41 s | 140k |
| every ref | last row | 345,545 | 5,400 | 345,545 | 2433, 2438, 2438, 2444, 2466 | 2.44 s | 142k |

The warm-up runs were within 2% of the median, except the first process of all
(117 ms at depth 10k from `HEAD`).

Where the time goes, from the existing reporter (single run, `HEAD`, all 340,228
commits, 4 MiB cache):

| Stage | Time | Share |
| --- | ---: | ---: |
| walk only | 1.44 s | 60% |
| lane layout | 0.39 s | 16% |
| decoding each commit's summary and building the rows (the rest of the 2.40 s find) | ~0.58 s | 24% |

Without the object cache the walk alone takes 2.13 s.

### Memory retained by a find

The figures were taken when the target arrived, with every loaded row and the
session still alive. They are the first measured run of each target. The byte
figures and segment counts were identical across runs, and the RSS figures are
the median of five.

| Seeding | Target | Rows kept | Retained rows (capacities) | Bytes/row | Of which edges | Segments/row (len / capacity) | `RssAnon` growth | `VmHWM` |
| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: |
| `HEAD` | depth 10k | 10,048 | 55.1 MiB | 5,753 | 50.5 MiB | 144.5 / 219.5 | 67 MiB | 163 MiB |
| `HEAD` | depth 50k | 50,048 | 302.5 MiB | 6,339 | 282.7 MiB | 159.0 / 246.8 | 321 MiB | 439 MiB |
| `HEAD` | depth 100k | 100,032 | 574.8 MiB | 6,026 | 535.2 MiB | 150.0 / 233.8 | 595 MiB | 735 MiB |
| `HEAD` | depth 300k | 300,032 | 1,366 MiB | 4,774 | 1,222 MiB | 116.7 / 178.0 | 1,373 MiB | 1,583 MiB |
| `HEAD` | `release-0.1` | 332,672 | 1,390 MiB | 4,382 | 1,242 MiB | 107.2 / 163.2 | 1,407 MiB | 1,624 MiB |
| `HEAD` | last row / `1.0.0` miss | 340,228 | 1,392 MiB | 4,291 | 1,244 MiB | 104.9 / 159.7 | 1,410 MiB | 1,629 MiB |
| every ref | depth 10k | 10,048 | 55.3 MiB | 5,772 | 50.7 MiB | 145.7 / 220.3 | 68 MiB | 175 MiB |
| every ref | depth 100k | 100,032 | 579.4 MiB | 6,073 | 539.8 MiB | 151.4 / 235.8 | 600 MiB | 751 MiB |
| every ref | depth 300k | 300,032 | 1,393 MiB | 4,870 | 1,249 MiB | 119.4 / 181.9 | 1,400 MiB | 1,610 MiB |
| every ref | `release-0.1` | 337,984 | 1,425 MiB | 4,422 | 1,277 MiB | 108.5 / 165.1 | 1,442 MiB | 1,660 MiB |
| every ref | last row | 345,545 | 1,428 MiB | 4,332 | 1,278 MiB | 106.2 / 161.6 | 1,445 MiB | 1,665 MiB |

The rest of a row is small:

- the `HistoryRow` structs themselves: 108 MiB of vector capacity at 340k rows,
  216 B each;
- the summary, name and e-mail text: 27 MiB in all;
- the parent ids: 14 MiB.

Three readings of this table, none of them a decision:

- **Rows dominate RSS.** `RssAnon` growth exceeds the summed rows by only
  12-18 MiB. That covers the session itself: gitoxide's seen-set, the assigner and
  the 4 MiB object cache. The ~15 MB seen-set estimate in
  `docs/systems/history-graph.md` is consistent with that.
- **Edges are kept at about 1.5x their length** (capacity against length), so about
  a third of the edge bytes are growth slack.
- **The per-row figure cited elsewhere does not hold here.** It is ~660 B in
  `docs/systems/history-graph.md` "Known limits", and 264 B at p99 in
  scroll-memory-model Part D. Part D's repositories had at most 2,896 commits.
  rust-lang/rust keeps over a hundred lanes open per row: its subtree-merged tools
  and bors merges give it many long-lived lines. So a scrolled or found row costs
  6-25x what those figures suggest.

**Freeing.** Dropping the kept rows, and then the session, lowered `RssAnon` by
only 1-5%. glibc kept the freed small allocations in its arenas. So the memory a
find costs is not given back to the system when its rows are dropped. That
matters to a design that evicts rows rather than never loading them.

### Cancelling a find

`next_page` returned `Err(Cancelled)` in every trial. "Latency" is the time from
`cancel()` to that return.

| Seeding | Trials | Rows loaded when cancelled | Latency median | Latency max |
| --- | ---: | --- | ---: | ---: |
| `HEAD` | 24 | 4,096 - 273,472 | 5.6 µs | 17.4 µs |
| `HEAD`, deep (190k-274k rows in) | 6 | 195,648 - 273,472 | 3.8 µs | 8.1 µs |
| every ref | 24 | 5,248 - 274,048 | 4.2 µs | 11.3 µs |
| every ref, deep (190k-274k rows in) | 6 | 194,432 - 274,048 | 5.0 µs | 10.3 µs |

The latency of a cancel is bounded by the gap between polls, and that gap does not
grow with depth:

| Seeding | Polls in a find to `release-0.1` | Gap p50 | p99 | p99.99 | Max | Gaps > 1 ms | Gaps > 10 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `HEAD` (4 runs) | 333,696 | 6.3 µs | 16.5-17.5 µs | 99-170 µs | 1.9-9.1 ms | 1-4 | 0 |
| every ref (3 runs) | 339,008 | 6.3 µs | 16.5-16.7 µs | 99-132 µs | 1.86-1.93 ms | 2 | 0 |

The few millisecond gaps fall where the kept vector reallocates and copies its
`HistoryRow` structs. The vector doubles at 262,144 and at 524,288 rows of
capacity, up to 113 MiB moved. In the application that copy happens in the
window's vector, not the worker's. On the worker, a cancel that lands mid-page is
honoured at the next commit, and the session keeps the work done
(`HistorySession::next_page`). R8.5's "the next press or scroll supersedes it" is
therefore immediate as far as the engine is concerned.

### The replaying cursor on the same repository

One cold-restart page, `Repository::history` resumed from a cursor at the given
depth with a limit of 64:

| Depth resumed at | Commits walked | Four runs (ms; warm-up first) | Median of the last three |
| ---: | ---: | --- | ---: |
| 10,048 | 10,112 | 60.5, 59.6, 59.4, 59.7 | 59.6 ms |
| 50,048 | 50,112 | 303.8, 300.4, 301.8, 301.0 | 301 ms |
| 100,032 | 100,096 | 591.8, 587.2, 589.5, 599.2 | 590 ms |
| 300,032 | 300,096 | 1616.2, 1603.7, 1612.9, 1613.4 | 1.61 s |
| 332,672 | 332,736 | 1761.7, 1752.3, 1745.5, 1745.7 | 1.75 s |

That is 5.2-5.9 µs per commit replayed: walked and laid out, but not decoded.

Paging to `release-0.1` by replay alone would walk 64 x (1 + 2 + ... + 5,198)
= 864,780,864 commits. At 5.25 µs each that is about **4,540 s, or 76 minutes**.
To depth 300k it would be about 63 minutes. This is computed from the rows above,
not run. It shows the order of magnitude that R2.5's held session removed: a held
session finds the same commit in 2.4 s.

## What "depth 500k, and 54-87 minutes of CPU" measured

The sentence is in `docs/prd/history-graph.md` R2.5 (added 2026-09-15). It came in
with commit `54cbf7f` ("record phase 02, resolve O2, and batch four decisions").
That commit's `docs/work/history-graph/progress.md` entry (since torn down) and its
`state.md` say the same thing in full. Paraphrased: page *k* walks *k x limit*
commits; one page costs 258-420 ms at depth 100k and 1.29-2.1 s at 500k; and it
takes 54-87 minutes of CPU to page sequentially to 500k at 100 rows a page.

- **It is the replaying cursor, `Repository::history` with
  `HistoryRequest::resume`.** Phase 02 had only that path. The cursor cannot carry
  gitoxide's walk, so each page re-walks from the pinned tips and skips what
  earlier pages covered. The figure was the evidence for building the held session
  in phase 03 (`74156c9`, "keep a walk alive for the length of a scroll"). Its doc
  comment repeats the 1.29-2.1 s figure as the reason the session exists.
- **It was extrapolated, not timed, beyond 100k.** The phase 02 bench repository
  was a 200,001-commit synthetic history built with `git fast-import`, with no
  commit-graph. A depth of 500k does not exist there. The 500k per-page range is
  exactly five times the 100k one (0.258 x 5 = 1.29 s, 0.420 x 5 = 2.10 s), which
  is 2.58-4.2 µs per commit. The 54-87 minutes is the sum of those per-page costs
  over 5,000 pages of 100 rows:
  100 x (1 + ... + 5,000) = 1,250,250,000 commits, x 2.58 µs = 3,226 s = 54 min,
  and x 4.2 µs = 5,251 s = 87.5 min.
  The record does not say which configurations gave the two ends of the range.
  The low end matches the O2 table's 2.6 µs per commit walked and laid out.
- **It does not apply to paging a held `HistorySession`.** A held session walks
  each commit once, so the cost of reaching depth *d* is linear in *d*, not
  quadratic. On rust the full 340k history takes 2.4 s from the cold open of a
  session, decoding included. It applies only to the session's cold-restart path:
  a request after the session was dropped (a walk error), which replays once to
  the cursor (1.75 s at 333k, measured above), and then pages normally.

## OPEN

- **The window's side of a find.** Not measured: about 5,300 `Update::Rows`
  messages delivered to the UI thread in about 2.4 s, the window appending each to
  its own vector, and what that does to frame time. That includes the
  reallocations, which copy up to 113 MiB, and dropping the rows, which
  `session::reload_if` does on the UI thread (#52). The engine figures above bound
  the worker's side only.
- **A repository with a commit-graph file.** This clone has none. The phase 02
  control and scroll-memory-model Finding 2 suggest the walk would be several
  times faster with one. The retained memory would not change.
- **Bigger or wider repositories.** These are one repository's numbers. A
  repository with more open lanes per row costs more per row, and a deeper one
  costs proportionally more in time and memory.
- **Peak memory under eviction.** Not measured. A find that kept only rows near
  its target would retain far less, but this record measures the design as built:
  every loaded row kept.

## Bench repository unchanged

Before any run, the list of `.git` entries was recorded with their mtimes and
sizes (`find .git -printf '%p %T@ %s\n'`), along with the index's mtime and
SHA-256, and a marker file was touched. After the last run and the worktree's
removal:

- the `.git` listing was identical;
- `.git/index` was unchanged: mtime 2026-09-17 17:38:22, SHA-256
  `aa92e16b8950806a29cc5e41ce84042581ebae8d25e9ef09f8342ae1902e3ab2`;
- `find .git -newer <marker>` printed nothing;
- `GIT_OPTIONAL_LOCKS=0 git -C ~/Development/bench/rust status --porcelain` was
  empty.

Every `git` command the method ran against it set `GIT_OPTIONAL_LOCKS=0`, and
Cairn's own reads go through gitoxide, which writes nothing.

## Appendix: the temporary reporter

It was appended to the `tests` module of `crates/cairn-git/src/history.rs` in the
scratch worktree, which reuses that module's `every_ref_tip` helper and its
imports. The worktree has since been removed.

It was driven, for example, as:

```
CAIRN_BENCH_REPO=~/Development/bench/rust CAIRN_FIND_SEED=head CAIRN_FIND_MODE=find \
CAIRN_FIND_TARGET=16e4369fe3b5f00aa3cdc584a4e41c51c0d3ca8a \
  target/release/deps/cairn_git-<hash> --ignored --nocapture --exact \
  history::tests::measures_finding_a_commit_by_paging_the_held_walk
```

The modes were `discover`, `find`, `replay`, `polls` and `cancel`, run as listed in
Method.

```rust
    // ---- Temporary reporter: refs-and-status R8.5, finding a ref's commit by paging the held walk.

    /// The application's page: `PAGE_ROWS` in `crates/cairn-app/src/main.rs`.
    const FIND_PAGE: usize = 64;

    fn find_env(name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }

    /// A `/proc/self/status` field, in kB.
    fn proc_kb(field: &str) -> usize {
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        status
            .lines()
            .find(|line| line.starts_with(field))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|n| n.parse().ok())
            .unwrap()
    }

    fn find_request(repo: &Repository, seed: &str) -> HistoryRequest {
        match seed {
            "head" => HistoryRequest::from_head(FIND_PAGE),
            "refs" => HistoryRequest::from_commits(every_ref_tip(repo), FIND_PAGE),
            other => panic!("CAIRN_FIND_SEED={other}: want head or refs"),
        }
    }

    /// Heap a retained row holds, from capacities; malloc's own overhead is not counted.
    fn row_bytes(row: &HistoryRow) -> usize {
        let mut bytes = size_of::<HistoryRow>()
            + row.graph.edges.capacity() * size_of::<cairn_model::EdgeSegment>();
        match &row.content {
            RowContent::Commit(commit) => {
                bytes += commit.parents.capacity() * size_of::<Oid>()
                    + commit.summary.capacity()
                    + commit.author_name.capacity()
                    + commit.author_email.capacity();
            }
        }
        bytes
    }

    fn row_oid(row: &HistoryRow) -> Oid {
        match &row.content {
            RowContent::Commit(commit) => commit.id,
        }
    }

    /// Records the gap between successive polls; never cancels.
    struct PollGaps {
        last: std::cell::Cell<Instant>,
        gaps: std::cell::RefCell<Vec<u64>>,
    }

    impl crate::Cancel for PollGaps {
        fn is_cancelled(&self) -> bool {
            let now = Instant::now();
            let gap = now.duration_since(self.last.get()).as_nanos() as u64;
            self.gaps.borrow_mut().push(gap);
            self.last.set(now);
            false
        }
    }

    /// Reporter. Env: `CAIRN_BENCH_REPO`, `CAIRN_FIND_SEED` (head|refs), `CAIRN_FIND_MODE`
    /// (discover|find|replay|polls|cancel), and per mode `CAIRN_FIND_DEPTHS`,
    /// `CAIRN_FIND_TAGS`, `CAIRN_FIND_TARGET`, `CAIRN_FIND_DELAYS_MS`. Run with `--release`.
    #[test]
    #[ignore = "needs a repository named by CAIRN_BENCH_REPO"]
    fn measures_finding_a_commit_by_paging_the_held_walk() {
        let path = find_env("CAIRN_BENCH_REPO").expect("set CAIRN_BENCH_REPO");
        let seed = find_env("CAIRN_FIND_SEED").unwrap_or_else(|| "head".into());
        let mode = find_env("CAIRN_FIND_MODE").expect("set CAIRN_FIND_MODE");

        match mode.as_str() {
            "discover" => {
                let repo = Repository::discover(&path).unwrap();
                let started = Instant::now();
                let request = find_request(&repo, &seed);
                let mut session = repo.history_session(&request).unwrap();
                let mut ids: Vec<Oid> = Vec::new();
                loop {
                    let page = session.next_page(FIND_PAGE, &CancelSignal::new()).unwrap();
                    ids.extend(page.rows.iter().map(row_oid));
                    if page.cursor.is_none() {
                        break;
                    }
                }
                eprintln!(
                    "DISCOVER seed={seed} rows={} ms={}",
                    ids.len(),
                    started.elapsed().as_millis()
                );
                for depth in find_env("CAIRN_FIND_DEPTHS").unwrap_or_default().split(',') {
                    let Ok(depth) = depth.trim().parse::<usize>() else { continue };
                    if let Some(id) = ids.get(depth - 1) {
                        eprintln!("DEPTH {depth} {id}");
                    }
                }
                for tag in find_env("CAIRN_FIND_TAGS").unwrap_or_default().split(',') {
                    let Some((name, hex)) = tag.split_once('=') else { continue };
                    let id = Oid::parse(hex.trim()).unwrap();
                    match ids.iter().position(|x| *x == id) {
                        Some(at) => eprintln!("TAG {name} {id} row {}", at + 1),
                        None => eprintln!("TAG {name} {id} absent"),
                    }
                }
                if let Some(last) = ids.last() {
                    eprintln!("END {} {last}", ids.len());
                }
            }
            "find" => {
                let target = Oid::parse(&find_env("CAIRN_FIND_TARGET").unwrap()).unwrap();
                let repo = Repository::discover(&path).unwrap();
                let anon_before = proc_kb("RssAnon:");
                let hwm_before = proc_kb("VmHWM:");
                let started = Instant::now();
                let request = find_request(&repo, &seed);
                let mut session = repo.history_session(&request).unwrap();
                let opened = started.elapsed();
                // Every loaded row is kept, as the window keeps them (issue #4).
                let mut retained: Vec<HistoryRow> = Vec::new();
                let (mut pages, mut walked, mut decoded) = (0usize, 0usize, 0usize);
                let mut found_at = None;
                let mut first_page = None;
                loop {
                    let page = session.next_page(FIND_PAGE, &CancelSignal::new()).unwrap();
                    pages += 1;
                    walked += page.walked;
                    decoded += page.decoded;
                    if first_page.is_none() {
                        first_page = Some(started.elapsed());
                    }
                    let hit = page.rows.iter().position(|row| row_oid(row) == target);
                    let base = retained.len();
                    let end = page.cursor.is_none();
                    retained.extend(page.rows);
                    if let Some(at) = hit {
                        found_at = Some(base + at + 1);
                        break;
                    }
                    if end {
                        break;
                    }
                }
                let elapsed = started.elapsed();
                let anon_after = proc_kb("RssAnon:");
                let hwm_after = proc_kb("VmHWM:");
                let est: usize = retained.iter().map(row_bytes).sum::<usize>()
                    + (retained.capacity() - retained.len()) * size_of::<HistoryRow>();
                let rows = retained.len();
                eprintln!(
                    "FIND seed={seed} target={target} found={} rows={rows} pages={pages} \
                     walked={walked} decoded={decoded} open_ms={:.1} first_page_ms={:.1} \
                     ms={:.1} rows_per_s={:.0} est_rows_bytes={est} est_bytes_per_row={:.0} \
                     rss_anon_kb={anon_before}->{anon_after} vm_hwm_kb={hwm_before}->{hwm_after} \
                     session={session:?}",
                    found_at.map_or("no".to_string(), |r| r.to_string()),
                    opened.as_secs_f64() * 1e3,
                    first_page.unwrap_or_default().as_secs_f64() * 1e3,
                    elapsed.as_secs_f64() * 1e3,
                    rows as f64 / elapsed.as_secs_f64(),
                    est as f64 / rows.max(1) as f64,
                );
                let (mut edge_len, mut edge_cap, mut text, mut parents) = (0usize, 0usize, 0usize, 0usize);
                for row in &retained {
                    edge_len += row.graph.edges.len();
                    edge_cap += row.graph.edges.capacity();
                    match &row.content {
                        RowContent::Commit(c) => {
                            text += c.summary.capacity() + c.author_name.capacity() + c.author_email.capacity();
                            parents += c.parents.capacity() * size_of::<Oid>();
                        }
                    }
                }
                eprintln!(
                    "BREAKDOWN HistoryRow={}B EdgeSegment={}B edges len={edge_len} cap={edge_cap} \
                     text_bytes={text} parent_bytes={parents} vec_cap={}",
                    size_of::<HistoryRow>(),
                    size_of::<cairn_model::EdgeSegment>(),
                    retained.capacity(),
                );
                drop(retained);
                let anon_rows_gone = proc_kb("RssAnon:");
                drop(session);
                let anon_session_gone = proc_kb("RssAnon:");
                eprintln!(
                    "FREED rss_anon_kb after dropping rows={anon_rows_gone} \
                     after dropping the session too={anon_session_gone}"
                );
            }
            "replay" => {
                // One cold-restart page (the replaying cursor) resumed at the target's depth.
                let target = Oid::parse(&find_env("CAIRN_FIND_TARGET").unwrap()).unwrap();
                let repo = Repository::discover(&path).unwrap();
                let request = find_request(&repo, &seed);
                let mut session = repo.history_session(&request).unwrap();
                let cursor = loop {
                    let page = session.next_page(FIND_PAGE, &CancelSignal::new()).unwrap();
                    let hit = page.rows.iter().any(|row| row_oid(row) == target);
                    let cursor = page.cursor.expect("target is the last page");
                    if hit {
                        break cursor;
                    }
                };
                drop(session);
                let mut times = Vec::new();
                let mut walked = 0;
                for _ in 0..4 {
                    let started = Instant::now();
                    let page = repo
                        .history(&HistoryRequest::resume(cursor.clone(), FIND_PAGE), &CancelSignal::new())
                        .unwrap();
                    times.push(started.elapsed().as_secs_f64() * 1e3);
                    walked = page.walked;
                }
                eprintln!(
                    "REPLAY seed={seed} depth={} walked={walked} ms(warm-up,then 3)={times:.1?}",
                    cursor.rows_behind()
                );
            }
            "polls" => {
                let target = Oid::parse(&find_env("CAIRN_FIND_TARGET").unwrap()).unwrap();
                let repo = Repository::discover(&path).unwrap();
                let request = find_request(&repo, &seed);
                let mut session = repo.history_session(&request).unwrap();
                let polls = PollGaps {
                    last: std::cell::Cell::new(Instant::now()),
                    gaps: std::cell::RefCell::new(Vec::new()),
                };
                let mut retained: Vec<HistoryRow> = Vec::new();
                loop {
                    let page = session.next_page(FIND_PAGE, &polls).unwrap();
                    let hit = page.rows.iter().any(|row| row_oid(row) == target);
                    let end = page.cursor.is_none();
                    retained.extend(page.rows);
                    if hit || end {
                        break;
                    }
                }
                let mut gaps = polls.gaps.into_inner();
                let n = gaps.len();
                gaps.sort_unstable();
                let at = |f: f64| gaps[((n - 1) as f64 * f).round() as usize] as f64 / 1e3;
                eprintln!(
                    "POLLS seed={seed} rows={} polls={n} gap_us p50={:.2} p99={:.2} p99.99={:.1} \
                     max={:.1} over_1ms={} over_10ms={}",
                    retained.len(),
                    at(0.5),
                    at(0.99),
                    at(0.9999),
                    at(1.0),
                    gaps.iter().filter(|g| **g > 1_000_000).count(),
                    gaps.iter().filter(|g| **g > 10_000_000).count(),
                );
            }
            "cancel" => {
                let delays: Vec<u64> = find_env("CAIRN_FIND_DELAYS_MS")
                    .unwrap_or_else(|| "250,1000,4000".into())
                    .split(',')
                    .map(|d| d.trim().parse().unwrap())
                    .collect();
                for delay in delays {
                    let signal = CancelSignal::new();
                    let theirs = signal.clone();
                    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
                    let (path, seed_for_worker) = (path.clone(), seed.clone());
                    let worker = std::thread::spawn(move || {
                        let repo = Repository::discover(&path).unwrap();
                        let request = find_request(&repo, &seed_for_worker);
                        let mut session = repo.history_session(&request).unwrap();
                        let mut retained: Vec<HistoryRow> = Vec::new();
                        ready_tx.send(()).unwrap();
                        loop {
                            match session.next_page(FIND_PAGE, &theirs) {
                                Ok(page) => {
                                    let end = page.cursor.is_none();
                                    retained.extend(page.rows);
                                    if end {
                                        return (Instant::now(), retained.len(), false);
                                    }
                                }
                                Err(Error::Cancelled { .. }) => {
                                    return (Instant::now(), retained.len(), true);
                                }
                                Err(other) => panic!("{other}"),
                            }
                        }
                    });
                    ready_rx.recv().unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    let set = Instant::now();
                    signal.cancel();
                    let (returned, rows, cancelled) = worker.join().unwrap();
                    eprintln!(
                        "CANCEL seed={seed} after_ms={delay} rows_loaded={rows} cancelled={cancelled} \
                         latency_us={:.1}",
                        returned.saturating_duration_since(set).as_secs_f64() * 1e6
                    );
                }
            }
            other => panic!("CAIRN_FIND_MODE={other}"),
        }
    }
```
