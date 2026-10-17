# gix's topological walk and the pieces under it, in the versions Cairn links

Evidence record for the `history-topo-order` packet, gathered 2026-10-17. Method: reading
the source that compiles into Cairn, not published docs, then running a throwaway probe
against fixture repositories and checking its output against `git rev-list`. The probe
and fixtures were built in the session's scratchpad. Nothing was written to Cairn's
checkout, and nothing touched the bench repository.

Versions, from `Cargo.lock`: `gix` 0.87.1, `gix-traverse` 0.61.0, `gix-revwalk` 0.35.0,
`gix-commitgraph` 0.39.0. Cairn enables gix's `blame`, `blob-diff`, `revision`,
`status`, `max-performance`, `parallel` and `sha256` features on top of the defaults
(root `Cargo.toml`, `[workspace.dependencies]`). Every `file:line` below is relative to
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/<crate>-<version>/` unless it
names a Cairn path.

## Correction to `docs/research/history-graph/gix-revwalk-ordering.md`

That record's Finding 1 ("gix has no `--topo-order` equivalent") is true of the
**`gix::revision::walk`** layer, the only one it read. `gix::revision::walk::Sorting`
still has exactly `BreadthFirst`, `ByCommitTime` and `ByCommitTimeCutoff`
(`gix-0.87.1/src/revision/walk.rs:32-67`), and `Sorting::into_simple` maps all three
onto `gix_traverse::commit::simple::Sorting` (`walk.rs:70-77`). That record also said
"the traversal layer offers no more than the `gix` layer exposes". That part is
**wrong**. `gix-traverse` has had a second walker, `commit::Topo`, alongside `Simple`
since gix-traverse 0.39.0 (upstream changelog, see section 4), and `gix` re-exports it.
The `rev_walk` platform just never uses it. Cairn already skips `rev_walk` and drives
`gix::traverse::commit::Simple` directly (`crates/cairn-git/src/history/walk.rs:33-35,
110`), so it can reach `Topo` the same way.

---

## 1. `gix_traverse::commit::topo`: git's incremental `--topo-order` / `--date-order`

### 1.1 Reachable from Cairn, with no feature gate

- `gix` re-exports the crate without a `cfg`: `pub use gix_traverse as traverse;`
  (`gix-0.87.1/src/lib.rs:157`). `gix-commitgraph` and `gix-revwalk` are re-exported
  the same way (`lib.rs:118`, `lib.rs:153`).
- `gix-traverse` is a plain, non-optional dependency of gix (`gix-0.87.1/Cargo.toml:447-448`
  has no `optional = true`). So are `gix-commitgraph` (`Cargo.toml:302-303`) and
  `gix-revwalk` (`Cargo.toml:418-419`).
- `gix-traverse`'s only features are `sha1` and `sha256` (`gix-traverse-0.61.0/Cargo.toml:37-39`).
  Nothing gates `commit::topo`.
- The module is public: `pub mod topo;` (`src/commit/mod.rs:40`), with `pub use
  init::Builder;` (`src/commit/topo/mod.rs:69`). The walker type is
  `gix_traverse::commit::Topo<Find, Predicate>` (`src/commit/mod.rs:26-38`).

So from cairn-git the path is `gix::traverse::commit::topo::{Builder, Sorting}` and
`gix::traverse::commit::Topo`. The probe compiled against exactly that path with
gix 0.87.1 (`features = ["revision", "max-performance", "sha256"]`).

### 1.2 Exact API (gix-traverse 0.61.0)

`Builder<Find, Predicate>` (`src/commit/topo/init.rs:11-21`), where `Find: gix_object::Find`:

| Item | Where | Notes |
| --- | --- | --- |
| `Builder::new(find)` | `init.rs:40-51` | No tips, no commit-graph (`commit_graph: Default::default()`, i.e. `None`), `Sorting::DateOrder`, `Parents::All`, predicate `\|_\| true`. |
| `Builder::from_iters(find, tips, ends: Option<_>)` | `init.rs:30-36` | Shorthand for `new().with_tips().with_ends()`. |
| `.with_tips(iter)` | `init.rs:82-89` | De-duplicated through a `HashSet` (`init.rs:84`). Order of first appearance is kept, and that order matters (§1.6). |
| `.with_ends(iter)` | `init.rs:95-98` | Like `^end` in `git rev-list`. Not de-duplicated. |
| `.with_predicate(f: FnMut(&oid) -> bool)` | `init.rs:57-71` | Filters what is **yielded** only. A filtered commit's parents are still walked (doc at `init.rs:53-56`, and `iter.rs:228-239`). |
| `.sorting(topo::Sorting)` | `init.rs:101-104` | `DateOrder` (default, = `git rev-list --date-order`, `topo/mod.rs:54-59`) or `TopoOrder` (= `--topo-order`, `topo/mod.rs:60-65`). |
| `.parents(Parents::{All, First})` | `init.rs:107-110` | `Parents` at `src/commit/mod.rs:42-50`. |
| `.with_commit_graph(Option<gix_commitgraph::Graph>)` | `init.rs:113-116` | Takes the graph **by value**. Cairn's `repo.commit_graph_if_enabled()` returns one owned (`gix-0.87.1/src/repository/graph.rs:34-46`). |
| `.build() -> Result<Topo, topo::Error>` | `init.rs:118-202` | Documented as "currently expensive" (`init.rs:120`). It does the first indegree walk (§1.3). |

`Topo` is an `Iterator<Item = Result<commit::Info, topo::Error>>` (`src/commit/topo/iter.rs:221-240`).
It yields the **same `commit::Info` type as `Simple`** (`src/commit/mod.rs:57-69`): `id`,
`parent_ids` (a `SmallVec<[ObjectId; 1]>`, truncated under `Parents::First`), `generation:
Option<u32>` (`Some` only when the commit came from a commit-graph, `iter.rs:140`,
`init.rs:194`), and `commit_time: Option<i64>`, always `Some` here (`iter.rs:141`,
`init.rs:195`). So Cairn's row layout reads it unchanged.

`topo::Error` (`topo/mod.rs:6-17`) has four variants: `MissingIndegreeUnexpected` and
`MissingStateUnexpected` (internal invariants, surfaced as errors rather than panics),
`ObjectDecode`, and `Find`. The `Find` variant is what a cancelling `Find` wrapper
surfaces (§1.7).

### 1.3 How it works: git's three-queue incremental walk

The structure is git's `init_topo_walk` / `compute_indegrees_to_depth` /
`expand_topo_walk` from `revision.c`, with git's names kept. The flags carry
`revision.h`'s names (comment at `topo/mod.rs:21`, flags at `topo/mod.rs:19-38`). The
fields (`src/commit/mod.rs:26-38`):

- `indegrees: IdMap<i32>` and `states: IdMap<WalkFlags>`. Both are `gix_hashtable`
  maps keyed by `ObjectId`. They hold one entry for every commit the walk has **looked
  at**, and nothing removes entries.
- `explore_queue` and `indegree_queue`: `PriorityQueue<(u32 generation, i64 time),
  ObjectId>`, max-first, so the highest generation goes first and the newest date
  breaks ties. This is git's `compare_commits_by_gen_then_commit_date`.
- `topo_queue`: what gets yielded. For `DateOrder` it is a `PriorityQueue<i64, Info>` by
  commit time. For `TopoOrder` it is a `Vec` used as a LIFO stack (`iter.rs:12-20,
  30-42`). The comment at `iter.rs:12-15` notes that git's prio-queue with no compare
  function is a stack, but git still sorts the initial tips by date.
- `min_gen: u32`, starting at `GENERATION_NUMBER_INFINITY` (`init.rs:132`;
  `gix-commitgraph-0.39.0/src/lib.rs:59` = `0xffff_ffff`).

`build()` (`init.rs:121-202`):
1. For each tip, and then each end, it reads the commit through `find`, which uses the
   commit-graph first and falls back to the object database (`src/commit/mod.rs:103-116`).
   It sets indegree 1, records the lowest generation seen in `min_gen`, and queues the
   commit on both the explore and the indegree queues (`init.rs:142-159`).
2. It marks the ends' parents uninteresting (`init.rs:163-171`).
3. `compute_indegrees_to_depth(min_gen)` (`init.rs:173`, body `iter.rs:55-65`) pops
   the indegree queue while the top's generation is `>= min_gen`. Each pop first runs
   `explore_to_depth(generation)` (`iter.rs:69`, body `iter.rs:85-94`), then adds 1 to
   every parent's indegree (`iter.rs:71-80`).
4. Each tip whose indegree is still exactly 1 (no other tip reaches it) goes on the
   `topo_queue` (`init.rs:178-198`), followed by `initial_sort()` (`init.rs:200`,
   `iter.rs:44-48`).

`next()` → `pop_commit` (`iter.rs:203-218`) pops the `topo_queue`, sets that commit's
indegree to 0, and runs `expand_topo_walk` (`iter.rs:113-147`). For each interesting
parent (`iter.rs:118-121`):
- If the parent's generation is below `min_gen`, it lowers `min_gen` and calls
  `compute_indegrees_to_depth(min_gen)` again (`iter.rs:123-126`). **This is the
  incremental step**: indegrees are computed only down to the generation the output
  has reached.
- It decrements the parent's indegree and queues the parent once the indegree reaches 1,
  meaning every child has been shown (`iter.rs:128-143`).

Correctness rests on generation numbers. Every child of a commit has a strictly higher
generation, so once indegrees have been computed down to `gen(c)`, every child of `c`
that the tips reach has already been counted.

### 1.4 Laziness: incremental with a commit-graph, everything up front without one

Lazy or eager is decided entirely by the generations `find` returns:

- **Without a commit-graph**, every commit's generation is `GENERATION_NUMBER_INFINITY`.
  `gen_and_commit_time` returns `(GENERATION_NUMBER_INFINITY, committer_time)` for any
  commit read from the object database (`iter.rs:300-319`, the value at `iter.rs:318`).
  So `min_gen` stays at infinity. `compute_indegrees_to_depth(INFINITY)` keeps popping
  while generation `>=` infinity, which is every commit. The very first
  `explore_to_depth(INFINITY)` explores the whole reachable history. **`build()` walks
  everything twice (explore, then indegree) before the first row is yielded.**
- **With a commit-graph**, tips are often newer than the graph and so have infinite
  generation. Build walks only those non-graph commits, stops at the graph's finite
  generations, and from then on each yielded row computes indegrees only as deep as the
  output requires.

Measured (probe, release build, AMD desktop; `git` 2.56.0 as the oracle; synthetic
200,000-commit repositories with 40 branches and merges, built with `git fast-import`;
a 4 MiB object cache, Cairn's `Repository::OBJECT_CACHE_BYTES`,
`crates/cairn-git/src/repository.rs:308`, unless noted):

| Case | `build()` | object reads at build | first 100 rows | whole walk | peak RSS |
| --- | --- | --- | --- | --- | --- |
| no commit-graph, no object cache | 1.20 s | 828,293 (≈4.1 per commit) | +0.6 ms | 2.37 s, 1,656,339 reads | 63.6 MB |
| no commit-graph, 4 MiB cache | 0.81 s | 828,293 | +0.3 ms | 1.30 s | 69.2 MB |
| no commit-graph, 64 MiB cache | 0.55 s | 828,293 | | | 120 MB |
| commit-graph covering all | 0.85-1.1 ms | **0** | 1.1-1.6 ms total | 0.18 s, 0 reads | 15 MB at build → 39.7 MB at end |
| commit-graph, 501 newer commits on top (stale graph) | 8.2 ms | 2,006 (the 501 non-graph commits) | 9.5 ms total | 0.19 s, 4,006 reads | 15 MB → 39.8 MB |

On Cairn's own history (840 commits, 18 tips), build read 3,426 objects without the
graph and 174 with it.

**git behaves the same.** On the same 200k repository, the time to `git rev-list
--topo-order --all`'s first line was 339 ms with `-c core.commitGraph=false` and 3 ms
with the graph (`--date-order`: 309 ms / 2 ms), against 1 ms for an unsorted
`rev-list`. git's incremental walk is also only incremental when generation numbers
exist. Without a commit-graph, gix's first-row latency is about 2.4× git's with Cairn's
cache (0.81 s vs 0.34 s), and 3.5× with no cache. The synthetic commits are tiny, so on
real repositories with larger commit objects these ratios are an estimate, not a
measurement.

Why gix reads so many objects: `Topo` keeps no commit cache. It does **not** use
`gix_revwalk::Graph`. Every `collect_parents` re-reads the commit, then re-reads **each
parent** to get its `(generation, time)` (`iter.rs:253-274`; the comment at
`iter.rs:269-270` explains why: a commit outside the graph can have parents inside it).
Explore, indegree and expand each call it, and so does `collect_all_parents` for every
queued `Info` (`iter.rs:134`, `init.rs:187`). So each commit is decoded several times.
The object cache absorbs part of that (table above). On graph-backed commits it costs
only cheap mmap lookups (`iter.rs:276-295`).

### 1.5 Multiple tips, ends and hide

- **Multiple tips** are native. Each starts at indegree 1, and a tip another tip
  reaches is not queued as a starting row (`init.rs:178-183`). This was verified against
  git with 18 and 40 tips (§1.6).
- **Ends** (`^end`) set the `Uninteresting | Bottom` flags (`init.rs:139-140`), and
  `expand_topo_walk` skips uninteresting parents (`iter.rs:118-121`). Uninteresting
  propagates as the explore walk runs, through `process_parents` (`iter.rs:149-183`).
  That marks the parents **and grandparents** of an uninteresting commit
  (`iter.rs:159-171`), which is one level, not git's recursive
  `mark_parents_uninteresting`. Also note that git itself does **not** use the
  incremental topo walk when negative revisions are given: `revs->limited` sends it
  through `limit_list` plus `sort_in_topological_order`. So gix's incremental handling of
  ends goes beyond what git does, and **this probe did not check it against git**.
  Cairn's history walk needs no ends. Show Lost Commits marks reachability separately
  (`crates/cairn-git/src/history/reach.rs`). Treat `with_ends` as unverified.
- **Hide**: `Topo` has no `hide()`. `Simple::hide` (`src/commit/simple.rs:335-338`, with
  the precomputed hidden frontier, `simple.rs:97`, `simple.rs:147-151`,
  `simple.rs:175-178`) exists on `Simple` only.
- **Predicate** hides rows without cutting the walk (§1.2).

### 1.6 Parity with git: exact, except where timestamps tie

Rows were compared to `git rev-list --<order>-order <tips…>`, with the tips in the same
order:

| Fixture | `TopoOrder` | `DateOrder` |
| --- | --- | --- |
| Cairn clone, 840 commits, 18 tips, with and without graph | identical | identical |
| 200k synthetic, committer dates one minute apart, 3% skewed up to 30 days into the past, with and without graph | identical | identical |
| 200k synthetic plus a stale graph with 501 newer commits and a merge | identical | identical |
| 200k synthetic where every 7 consecutive commits share one committer second ("ties"), with and without graph | **differs from row 1** (1,672 rows out of place) | **differs from row 4** (147,251 rows out of place) |

Skewed dates (a parent newer than its child) do not break either order. That is the
whole point of the indegree walk, and it holds with or without generation numbers. The
divergences come from **equal committer timestamps** only. gix's output is identical
with and without the graph in every case.

- **`TopoOrder`, equal-timestamp tips.** git shows tips sorted newest first, stably in
  the order it collected them: its `commit_list_sort_by_date` is a stable merge sort,
  and `prio_queue_reverse` sets the stack up so the first tip is shown first. gix
  `sort_by_key`s ascending (stable) and pops from the **end** (`iter.rs:37-48`), so
  among tips with equal dates the **last** one given comes out first. After the tips,
  the LIFO expansion matches git exactly. **Workaround, verified:** pass the tips to
  `with_tips` in reverse order. On the ties fixture the output was then identical to git
  across all 200,000 rows. git's own output depends on tip order among ties too: the same
  tips reversed give a different `git rev-list --topo-order`. So Cairn would also have to
  feed tips in the order git collects them for the command it treats as the oracle (for
  `--all`, ref iteration order, sorted by refname).
- **`DateOrder`, any equal timestamps.** git's `prio_queue` breaks ties by insertion
  counter (FIFO). `gix_revwalk::PriorityQueue` wraps a `std::collections::BinaryHeap`
  of items compared by **key only** (`gix-revwalk-0.35.0/src/lib.rs:48-49`,
  `src/queue.rs:36-40`), which pops equal keys in an unspecified order. So `DateOrder`
  diverges from git wherever committer seconds tie. That is common: a rebase or a
  scripted series stamps many commits with the same second. Nothing outside the crate
  can fix this, and reversing tips did not help (147,025 rows still differed).

So `TopoOrder` is a git-exact `--topo-order`, provided the tips are passed in reverse
of git's tip order. `DateOrder` is not a git-exact `--date-order`.

### 1.7 Pause, resume and cancel inside Cairn's `HistorySession`

- **Pause/resume** is iterator semantics. `Topo` owns its `Find`, its commit-graph and
  all walk state (`src/commit/mod.rs:26-38`), and does nothing until `next()` is called.
  Holding it open across pages is exactly how Cairn holds `Simple` today: `CommitWalk<'repo>`
  is `Simple<Seeded<'repo>, fn(&oid) -> bool>` (`crates/cairn-git/src/history/walk.rs:33-35`),
  living in a `HistorySession<'repo>` that borrows the repository
  (`crates/cairn-git/src/history/session.rs:17-20`). A `Topo<Grafted<'repo>, _>` borrows
  the same way, and the generic `Find` accepts Cairn's existing wrappers (`Grafted`,
  `Polled`, `Seeded`; `walk.rs:129-168`, `walk.rs:267-322`). Being `Send` does not
  matter: the session never crosses a thread. As with `Simple`, a walk cannot be resumed
  from a value. There is no cursor, so a cold restart replays.
- **Cancel.** `Topo` polls nothing itself, but every object read goes through `Find`, so
  a `Polled`-style wrapper that fails once cancelled ends the walk with
  `topo::Error::Find`. This applies inside `build()` too, which matters because without
  a graph `build()` is the whole-history walk. On a graph-backed commit no `Find` call
  happens (`iter.rs:276-295`, zero reads in the table), so a graph-backed walk could only
  be cancelled between `next()` calls. Each of those does bounded work, so that is enough.
  After an error the state is inconsistent: `pop_commit` already zeroed the indegree
  before expansion failed (`iter.rs:212-215`). A cancelled walk has to be discarded, not
  continued.
- **Opening cost.** Without a graph, `build()` is that whole walk, and it is uncancellable
  except through `Find` (above). Cairn's walk opens under the first page's cancel
  (`session.rs`, `stream: None` until the first page), so the first page carries it.

### 1.8 Behaviour at the edges, and source notes

- **Missing objects.** `find` turns a missing commit into an error
  (`find_commit_iter`, `src/commit/mod.rs:114`), so a missing parent fails the whole walk.
  (`gix_revwalk::Graph`, by contrast, skips missing parents:
  `gix-revwalk-0.35.0/src/graph/mod.rs:145-148`.) For a shallow clone, Cairn's `Grafted`
  already removes the boundary parents (`crates/cairn-git/src/history/walk.rs:294-322`)
  and passes no graph (`walk.rs:71-80`), so the same wrapper serves `Topo`.
- **Corrupt commit-graph.** A bad parent position drops the graph for the rest of the
  walk and retries from the object database (`iter.rs:277-281`). From then on
  generations are infinite. Indegrees stay correct (infinity is `>=` any cutoff, so the
  walk just goes eager).
- **Generation numbers are v1 (topological levels)** (§2). The algorithm only needs a
  valid generation function, so the output is unaffected. What it costs could be: git
  with GDAT data uses corrected commit dates, which cut off earlier on skewed histories.
  That is a cost difference, not an output difference.
- **Commit-graph eligibility** is the caller's decision. `commit_graph_if_enabled` checks
  `core.commitGraph` alone (`gix-0.87.1/src/repository/graph.rs:37-40`,
  `src/config/cache/access.rs:214-220`). git's `commit_graph_compatible` also refuses the
  graph in a shallow repository, with grafts, or with replace refs being read. Cairn
  already handles the shallow case (`walk.rs:17-19`, `71-80`). Replace refs are worth a
  check if the walk is rebuilt.
- **TODO/`expect` in the module.** `topo/mod.rs:7` has a `#[expect(missing_docs)]`.
  `iter.rs:285` has an `.expect("cache exists if CachedCommit was returned")`, which
  cannot fire: it is reached only through a cache hit. Comments at `init.rs:136-138`
  ("by definition (?)"), `init.rs:161-162` ("for some reason") and `init.rs:175-177`
  ("For now the tests pass") mark where the port of git's handling of ends is uncertain,
  which fits §1.5. None of them touch the path Cairn would use (tips only).
- **Upstream `main`** (fetched 2026-10-17) has the same `Queue`, `initial_sort` and
  `collect_parents` as 0.61.0, so neither the tie behaviour nor the re-reads have
  changed upstream.

---

## 2. gix-commitgraph: what it reads

- **Generation v1 only.** `file::Commit::new` reads the CDAT entry's 4-byte field
  `>> 2` as `generation` (`gix-commitgraph-0.39.0/src/file/commit.rs:42`), next to a
  `// TODO: Add support for corrected commit date offset overflow.` (`commit.rs:39-41`).
  The committer timestamp is the 34-bit field (`commit.rs:43-44`). The only chunk IDs the
  crate knows are `BASE`, `CDAT`, `EDGE`, `OIDF` and `OIDL` (`src/file/mod.rs:19-23`).
  **There is no `GDAT`, `GDA2` or `GDOV` handling anywhere in the crate**, so corrected
  commit dates (generation v2) are never read. In git's format, CDAT always carries the
  topological level, even in a v2 file, so the value gix reads is a valid generation
  number for every graph git writes. The upstream changelog's 0.17.0 entry ("support for
  'generation v2' format for overflow correction") does not show up in 0.39.0's source as
  any GDAT/GDOV read. Read it as handling of the v1 field in v2 files, not corrected
  dates.
- **Accessor.** `file::Commit::generation() -> u32` (`commit.rs:55-61`). You get a
  `file::Commit` from `Graph::commit_by_id(id)` (`src/access.rs:22`),
  `Graph::commit_at(Position)` (`access.rs:9`) or `Graph::lookup(id) -> Option<Position>`
  (`access.rs:47`). Constants: `GENERATION_NUMBER_INFINITY = 0xffff_ffff`
  (`src/lib.rs:59`) and `GENERATION_NUMBER_MAX = 0x3fff_ffff` (`lib.rs:65`). Split
  chains (`commit-graphs/graph-*.graph` plus the chain file) are one `Graph` over several
  `File`s (`lib.rs:38-45`).
- **gix-revwalk's `Graph`** exposes generation per commit as `Option<Generation>`:
  `LazyCommit::generation()` returns `None` for a commit read from the object database
  and `Some(..)` for a commit-graph-backed one (`gix-revwalk-0.35.0/src/graph/commit.rs:30-36`).
  The same goes for `generation_and_timestamp()` (`commit.rs:39-58`) and the owned
  `Commit<T>::generation` (`src/graph/mod.rs:405-406`; `None` at `commit.rs:89`, `Some`
  at `commit.rs:107`). **Without a commit-graph file every commit's generation is
  `None`.** gix computes none of its own. `Topo` maps that same absence to
  `GENERATION_NUMBER_INFINITY` (`gix-traverse` `iter.rs:318`) and back to `None` in
  `Info` (`iter.rs:140`).
- **No writer.** The crate's public surface reads, verifies and traverses (`src/lib.rs:1`,
  modules `access`, `file`, `init`, `verify`), and nothing in it writes a graph. Its own
  docs point to `git commit-graph write` (`lib.rs:42`). `gix-0.87.1/src/repository/graph.rs`
  offers `commit_graph()` and `commit_graph_if_enabled()` (read, `:29-46`) and
  `revision_graph()` (`:17-22`), and no write. If Cairn wanted a graph it would have to
  be `git commit-graph write` through `ops/`, which would be a user-visible repository
  mutation and a decision in its own right. The research here does not suggest it.

---

## 3. Building blocks for running git's algorithm in Cairn instead

Only relevant if `Topo` is rejected: for example, to get git-exact `--date-order` ties,
to share a commit cache with the rest of the walk, or to bound re-reads.

- **`gix_revwalk::Graph<'find, 'cache, T>`** (`gix-revwalk-0.35.0/src/lib.rs:28-39`) is a
  commit cache: a `Box<dyn gix_object::Find + 'find>`, an optional
  `&'cache gix_commitgraph::Graph`, an `IdMap<T>` and two buffers. Construct it with
  `Graph::new(objects, cache)` (`src/graph/mod.rs:217-225`) or
  `gix::Repository::revision_graph(cache)` (`gix-0.87.1/src/repository/graph.rs:17-22`,
  which passes `&self.objects`). With `T = graph::Commit<D>`:
  - `get_or_insert_commit(id, update)` (`graph/mod.rs:275-281`) and
    `get_or_insert_commit_default(id, new_data, update)` (`graph/mod.rs:235-257`) read
    the commit **once**, from the commit-graph if it holds the commit, otherwise from the
    object database (`try_lookup`, `graph/mod.rs:363-389`). They store
    `Commit { parents, commit_time, generation, data }` (`graph/mod.rs:400-409`) and
    return `Ok(None)` for a missing commit or a non-commit (`graph/mod.rs:245`,
    `graph/mod.rs:382`).
  - `get_or_insert_full_commit` (`graph/mod.rs:287-308`), `get`/`get_mut`/`contains`
    (`graph/mod.rs:89-101`), `clear_commit_data` (`graph/mod.rs:260-262`) and `detach`
    (`graph/mod.rs:202-204`).
  - The docs say that with `T = Commit<_>` no object cache is needed
    (`gix-revwalk-0.35.0/src/lib.rs:26-27`), because each commit is decoded once and
    kept. A `Graph` borrows its `Find` and is not `Send` (`Box<dyn Find>` without a
    `Send` bound), which fits a session that never leaves its thread.
- **Cheap parents and time without a cache.** `Graph::try_lookup(id) ->
  Option<LazyCommit>` (`graph/mod.rs:346-351`), then `LazyCommit::iter_parents()`
  (`graph/commit.rs:9-15`), `committer_timestamp()` (`graph/commit.rs:21-28`) and
  `generation()` (`graph/commit.rs:31-36`). The object-database path parses only the
  header tokens up to `committer`. The graph path is a mmap read. `to_owned`
  (`graph/commit.rs:62-112`) stops at the committer token. `gix_traverse::commit::find`
  and `Either::commit_time` (`gix-traverse-0.61.0/src/commit/mod.rs:91-116`) do the same
  thing without a map.
- **`gix_revwalk::PriorityQueue<K, T>`** (`gix-revwalk-0.35.0/src/lib.rs:48-49`):
  `new`, `insert(key, value)`, `pop() -> Option<(K, T)>` (the highest key), `pop_value`,
  `peek`, `len`, `is_empty`, `clear`, `iter_unordered`, `into_iter_unordered`
  (`src/queue.rs:65-114`). It is a max-heap (`BinaryHeap`) ordered by key only
  (`queue.rs:36-40`). Note the doc comment at `lib.rs:44` says it sorts "in ascending
  order", but `pop` returns the **greatest** key (`queue.rs:80-83`, `queue.rs:105-108`).
  To get git's stable ties, a reimplementation would key on `(time, Reverse(insertion
  counter))` (or `(gen, time, Reverse(counter))` for the explore and indegree queues),
  which is what git's `prio_queue` `ctr` does.
- **Memory per commit**, measured with `size_of` in the probe (Cairn's features, sha1 and
  sha256 both on, so `ObjectId` is 33 bytes):

  | Value | Bytes |
  | --- | --- |
  | `gix_hash::ObjectId` | 33 |
  | `commit::Info` (`Topo`'s and `Simple`'s yield) | 112 |
  | `(i64, Info)`, a `topo_queue` entry | 120 |
  | `(ObjectId, i32)`, an `indegrees` entry | 40 |
  | `(ObjectId, WalkFlags)`, a `states` entry (`WalkFlags` is a `u8`, `topo/mod.rs:20-24`) | 34 |
  | `((u32, i64), ObjectId)`, an explore/indegree queue entry | 56 |
  | `gix_revwalk::graph::Commit<()>` | 64 (one parent inline, a second spills to the heap) |
  | `(ObjectId, Commit<()>)`, a `Graph` map entry | 104 |

  hashbrown adds 1 control byte per bucket, and its bucket count is a power of two at
  ≤ 7/8 load. So `Topo`'s two maps cost about 76 bytes per commit **looked at**, up to
  about twice that just after a resize. Neither map is ever pruned, so for a walk run to
  the end that means every reachable commit. Measured: 15 MB → 39.7 MB over the 200k
  graph-backed walk, about 125 bytes per commit. Without a graph, both maps are full
  after `build()`. A `Graph<Commit<D>>` costs about 104 bytes plus `D` plus control
  bytes per commit, and in exchange each commit is decoded once.

---

## 4. Version history, and newer gix

`gix-traverse-0.61.0`, like the other published crates here, ships **no `CHANGELOG.md`**:
its package `include` is `/src/**/*` and `/LICENSE-*` (`gix-traverse-0.61.0/Cargo.toml:19-22`).
The changelogs were read upstream on 2026-10-17
(`GitoxideLabs/gitoxide`, `gix-traverse/CHANGELOG.md`, `gix/CHANGELOG.md`,
`gix-commitgraph/CHANGELOG.md` on `main`). These are summaries, not quotes:

- gix-traverse **0.39.0** (2024-04-13): initial topological commit walk; `Ancestors`
  renamed to `Simple`, `topo` made similar to it; `DateOrder` documentation and a
  baseline test. **So `Topo` predates the 2026-09-14 record above.** That record just
  read the `gix::revision` layer, which has never exposed it.
- 0.42.0 (2024-10-22): topo traversal made resilient against broken parent-id queries
  (the corrupt-graph fallback, `iter.rs:277-281`); `CommitTimeOrder` added for `Simple`.
- 0.43.1 (2024-12-22): `topo::Builder::new`, `with_tips`, `with_ends` added.
- 0.45.0 / 0.46.2 (2025): `commit::find`, `commit::Either`, `Either::commit_time` exposed.
- 0.47.0, 0.53.0, 0.56.0: `Simple::hide()`, then graph painting for it, then a faster
  hidden frontier. These are `Simple`'s, not `Topo`'s (§1.5).
- **0.61.0** (2026-08-22, the version Cairn links): `commit::Info::generation` added.
- 0.62.0 (2026-09-25) and 0.63.0 (2026-10-08): no entries touching topo, sorting or
  generation.
- gix: 0.88.0 (2026-09-25) is the newest release. Its changelog has no entry on topo,
  `rev_walk` sorting, generation numbers or commit-graph writing. gix-commitgraph is at
  0.41.0 with no generation-v2 or writer entry.

**Conclusion on upgrading: a newer gix brings nothing here.** Upstream `main` still has
0.61.0's `topo/iter.rs` behaviour (§1.8). Nothing was upgraded.

---

## Summary for the packet

1. `gix::traverse::commit::topo::Builder` with `Sorting::TopoOrder` is a usable,
   git-structured incremental `--topo-order` walk. Cairn's features already reach it
   (no gate). It yields the same `Info` as Cairn's current walk, accepts Cairn's `Find`
   wrappers, and can be held open across pages as `Simple` is.
2. Its output matched `git rev-list --topo-order` exactly on every fixture, skewed dates
   included, once tips are passed in **reverse** of git's tip order. Without that
   reversal, tips with equal timestamps come out in the opposite order to git's.
3. `Sorting::DateOrder` is **not** git-exact. Equal committer seconds pop in arbitrary
   order (a `BinaryHeap` with no insertion counter).
4. It is incremental **only with a commit-graph**. Without one, `build()` walks and
   indegree-counts the whole history before the first row, as git does too (git 0.34 s
   vs gix 0.81 s at 200k synthetic commits with Cairn's 4 MiB cache). Cancellation can
   only reach it through `Find`. Its two maps keep about 76-150 bytes per commit for the
   life of the walk.
5. gix reads v1 generation numbers only (no GDAT), exposes them as `Option` (`None`
   without a graph), and never writes a commit-graph.
