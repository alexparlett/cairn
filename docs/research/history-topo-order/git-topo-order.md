# What order `git log --graph` shows, what git pays for it, and Cairn's oracle

Evidence record for the `history-topo-order` packet, gathered 2026-10-17. The question: Cairn's
history walks in gix's `Sorting::ByCommitTime(NewestFirst)` (`crates/cairn-git/src/history.rs`,
`HistoryOrder::sorting`), which can draw a parent above its child when commit times tie or skew;
git's graph never does. Which of git's orders should Cairn's rows equal, how does git produce it,
what does git pay for its first page, and which git command is the test oracle in each of Cairn's
walk modes?

Read first: `docs/research/history-graph/gix-revwalk-ordering.md` (gix has no topological sort)
and `docs/research/history-graph/scroll-memory-model.md` Part A (a graph log buffers the whole
reachable set; a commit-graph file collapses that). This record extends both with git's own
algorithm, the bench repository's numbers and an oracle that agrees across git 2.30.9, 2.32.7
and 2.56.0.

## Corrections

Section 2 was first written from recall, marked [recall], with git's source not on disk. It has
since been checked against git's source at v2.30.9 and v2.56.0 (method below) and every [recall]
mark replaced by a file:line citation at both tags. What the source contradicted or narrowed:

1. **`--graph` sets `topo_order` unconditionally**, not "when no order was given". The order it
   implies comes from `sort_order`, which starts as `REV_SORT_IN_GRAPH_ORDER` and is replaced by
   `--date-order` wherever that appears on the command line. 2.30.9 does it while parsing
   `--graph` (`revision.c:2433-2436`); 2.56.0 moved it to `revision_opts_finish`, after every
   argument is read (`revision.c:2782-2790`). What §1.1 measured holds.
2. **"Without a usable commit-graph" covers more than "no file".** The incremental walk is
   skipped whenever `generation_numbers_enabled` is false, and that is false when the repository
   has replace refs, grafts or is shallow (`commit_graph_compatible`), or when `core.commitGraph`
   is off, as well as when no graph exists. **Negative tips do not force the whole-list path when
   generation numbers are on**: `handle_commit` sets `limited` for an `UNINTERESTING` tip only
   when `!revs->topo_order || !generation_numbers_enabled` (2.30.9 `revision.c:441-442`, 2.56.0
   `revision.c:435-436`). §2.1 has the full list.
3. **§4.2's "[recall]" on replace refs is confirmed, with a consequence**: a replace ref turns the
   commit-graph off (2.30.9 `commit-graph.c:206-209`, 2.56.0 `commit-graph.c:228-231`), so the
   grafted-stash oracle always runs the whole-list path, `limit_list` then
   `sort_in_topological_order`, never the incremental walk. Measured on a fixture with a
   commit-graph, 2.56.0: no replace ref, the trace2 stream carries `topo_walk` statistics (the
   incremental walk ran); one `git replace --graft`, it carries none.
4. **`get_revision_1` in 2.56.0 does not "otherwise pop `revs->commits`"** in every case. It
   picks one of five modes (`get_walk_mode`, `revision.c:4423`): the whole-list path pops
   `revs->commits`, but the default order's streaming walk pops a date-ordered `prio_queue`
   (`revs->commit_queue`), where 2.30.9 popped a commit list kept in date order. Both break equal
   dates first in, first out, so the default order is unchanged.
5. **`GENERATION_NUMBER_INFINITY` "from 2.31"** cannot be confirmed: only v2.30.9
   (`0xFFFFFFFF`, `commit.h:14`) and v2.56.0 (`(1ULL << 63) - 1`, `commit.h:12`) were read. The
   tag that made the change was not read.
6. **2.56.0's `prio_queue` defers the sift-down of a `get`** (`get_pending`, `prio-queue.c:58-111`),
   and its `limit_list` uses a `prio_queue` where 2.30.9 used a date-sorted commit list. Neither
   changes an order. Every key is unique, being (compare, insertion counter), so any correct
   priority queue pops the same sequence.

No other [recall] claim was contradicted. §2.2's and §2.3's mechanics hold at both tags as they
were written.

## Method, and what each class of evidence is worth

- **Documentation**: the `git-log(1)`, `git-config(1)` and `gitformat-commit-graph(5)` man pages
  shipped with the host's git 2.56.0 (`man -P cat git-log`), quoted verbatim.
- **git's C source, at v2.30.9 and v2.56.0.** Shallow clones, fetched with the user's approval:
  `git clone --depth 1 --branch v2.30.9 https://github.com/git/git.git
  ~/.cache/cairn/git-src/v2.30.9`, and the same for v2.56.0 (the floor builds under
  `~/.cache/cairn/git-floor/` are install trees, because `scripts/git-floor.sh` deletes each
  source tree after `make install`). Section 2 cites `file:line` at both tags as "2.30.9
  `revision.c:N`, 2.56.0 `revision.c:M`". 2.32.7's source was not read: its behaviour is
  measured only, and the location its trace2 stream prints for the topo walk's statistics,
  `revision.c:3311`, is its own report. The same event is at `revision.c:3763` in 2.56.0
  (`trace2_data_json("topo_walk", …)`, confirmed in the source). 2.30.9 has no such statistics:
  `count_topo_walked` does not occur in its `revision.c`.
- **Measurement**: fixtures built in a scratch directory with fixed dates, every ordering run by
  all three gits with `GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1`; the bench
  (`~/Development/bench/rust`, rust-lang/rust) read with `GIT_OPTIONAL_LOCKS=0` and nothing
  else, timed by a wrapper that records wall time to the first line, to the 100th and to exit,
  and the child's max RSS from `wait4` (the host has no `/usr/bin/time`; `ru_maxrss` is the value
  `time -v` reports). Machine: 16 threads, 60 GiB, the bench's 1.1 GiB pack in page cache (one
  warm-up run each). Timings are the median of three runs unless stated.

## 1. Which order `git log --graph` uses

### 1.1 The documented orders

`git-log(1)`, "Commit Ordering" (2.56.0):

> By default, the commits are shown in reverse chronological order.

> `--date-order` Show no parents before all of its children are shown, but otherwise show
> commits in the commit timestamp order.

> `--topo-order` Show no parents before all of its children are shown, and avoid showing commits
> on multiple lines of history intermixed.

with its example: for `---1----2----4----7` / `\----3----5----6----8---` (numbers are timestamp
order), `--date-order` shows `8 7 6 5 4 3 2 1` and `--topo-order` shows `8 6 5 3 7 4 2 1 (or 8 7
4 2 6 5 3 1)`. And under `--graph`:

> This implies the `--topo-order` option by default, but the `--date-order` option may also be
> specified.

Verified on the bench: the commit ids of `git log --all --graph` are in exactly
`--topo-order`'s order (sha256 of the 345,545 ids `7f120c68fdc1` for both), and `--graph
--date-order`'s exactly `--date-order`'s (`a8471e2d2464` for both).

### 1.2 The tie-breaks (verified on fixtures; mechanism in section 2)

| Order | Among commits whose children are all shown ("ready") | At a merge | Equal timestamps |
| --- | --- | --- | --- |
| default | no readiness at all: commits leave a date-ordered frontier as they are reached, so a parent reached before its child is shown above it | — | the order they entered the frontier |
| `--date-order` | newest committer date first; ties by the order they became ready (FIFO) | first parent's line first | breadth-first: parallel lines interleave a row at a time |
| `--topo-order` | the one made ready most recently (LIFO stack) | the **last** parent's line first, followed to its fork point before the first parent's | depth-first: one line drained before the next |
| both ordered modes, the initial tips | sorted newest committer date first, stably, so equal dates keep the order the tips were given — argument order, and for `--branches`/`--remotes`/`--tags`/`--all` refname order (`refs/heads/` < `refs/remotes/` < `refs/stash` < `refs/tags/`, then `HEAD`) | | |

Evidence for the last row (a root, then `a`, `b`, `main` each one commit on it, all at `@1000`;
identical at 2.30.9, 2.32.7 and 2.56.0, and identical given on `--stdin`):

```
a b main:   on a,on b,on main,root     (topo and date)
b a main:   on b,on a,on main,root
main b a:   on main,on b,on a,root
--branches: on a,on b,on main,root
```

A tag on a tree (`git tag treetag HEAD^{tree}`, and an annotated one) under `--tags` is ignored
without an error.

### 1.3 Fork's "Sort commits: By date" is `--date-order`

The fixture of `docs/research/ui-review/fixture-recipe.md` built here with
`GIT_AUTHOR_DATE=GIT_COMMITTER_DATE=2026-10-16T12:00:00+01:00` gives **byte-identical commits**
to the one Fork was shown in `docs/research/ui-review/fork-window-survey-2-2026-10-16.md` §6
(`8313cd7`, `e49aa17`, `9913e5f`, `6bd753b`, `f7ad061`, … `27891b7`). Fork's 14 rows there, with
"By date" selected:

`8313cd7` Fix crash · `f7ad061` stash · `9913e5f` Local only · `6bd753b` Merge · `aaaf1f0` Fix
README typo · `e49aa17` Validate login · `48f0802` Add login module · `2e78d15` Add note 6 · … ·
`27891b7` Initial commit.

That is git's `--date-order` with the stash's index commit `e5ddb8f` left out (Fork draws a
stash as one row) — row for row. `--topo-order` (and so plain `--graph`) differs at rows 5-7:
`e49aa17`, `48f0802`, then `aaaf1f0`, the merge's second parent's line first. The survey's note
that Fork's order "is the same as `git log --all --graph`'s" holds only for the set and the
no-parent-above-child property, not for the row order. Fork's Preferences offer "Topologically"
as the other choice. **Not established**: that Fork's By date is `--date-order` in general —
one fixture, one merge; Fork's breadth-first interleaving at equal times (the tied fixture below,
rows 2-7) has not been shown to Fork.

### 1.4 The orders on a fixture with tied times

`fx/build-tied.sh` (below, "Fixture scripts"): the recipe, then a branch `topic` merged into
`main` twice, then a branch `rebased` of five commits made on `main~2` and rebased onto `main`,
then "After the rebase" on `main` and a stash — every commit in one second. Output of `git log
--format='%h %s'` over Cairn's tips (`--branches --remotes --tags HEAD`), identical at 2.30.9,
2.32.7 and 2.56.0, with and without a commit-graph each version wrote itself:

| # | default | `--date-order` | `--topo-order` (= `--graph`) |
| --- | --- | --- | --- |
| 1 | 8313cd7 Fix crash on empty input | 8313cd7 Fix crash on empty input | 8313cd7 Fix crash on empty input |
| 2 | **e49aa17 Validate login input** | abed6f2 After the rebase | abed6f2 After the rebase |
| 3 | abed6f2 After the rebase | 2d11c10 Rebased 5 | 2d11c10 Rebased 5 |
| 4 | 2d11c10 Rebased 5 | fdf1094 Rebased 4 | fdf1094 Rebased 4 |
| 5 | 30b2c69 Topic two | 1fd7929 Rebased 3 | 1fd7929 Rebased 3 |
| 6 | 6bd753b Merge branch 'feature/login' | 431a3ca Rebased 2 | 431a3ca Rebased 2 |
| 7 | **2e78d15 Add note 6** | 48b8f14 Rebased 1 | 48b8f14 Rebased 1 |
| 8 | 48f0802 Add login module | de02e88 Merge topic twice | de02e88 Merge topic twice |
| 9 | de02e88 Merge topic twice | 20c5a3d Merge topic once | 30b2c69 Topic two |
| 10 | fdf1094 Rebased 4 | 30b2c69 Topic two | 20c5a3d Merge topic once |
| 11 | **35b9938 Topic one** | ea353a5 Main between merges | 35b9938 Topic one |
| 12 | aaaf1f0 Fix README typo | 35b9938 Topic one | ea353a5 Main between merges |
| 13 | 2f84245 Add note 5 | 9913e5f Local only commit | 9913e5f Local only commit |
| 14 | 20c5a3d Merge topic once | 6bd753b Merge branch 'feature/login' | 6bd753b Merge branch 'feature/login' |
| 15 | 1fd7929 Rebased 3 | aaaf1f0 Fix README typo | e49aa17 Validate login input |
| 16 | 9913e5f Local only commit | e49aa17 Validate login input | 48f0802 Add login module |
| 17 | 9f27c5a Add note 4 | 48f0802 Add login module | aaaf1f0 Fix README typo |
| 18 | ea353a5 Main between merges | 2e78d15 Add note 6 | 2e78d15 Add note 6 |
| 19 | 431a3ca Rebased 2 | 2f84245 Add note 5 | 2f84245 Add note 5 |
| 20 | 20d42bf Add note 3 | 9f27c5a Add note 4 | 9f27c5a Add note 4 |
| 21 | 48b8f14 Rebased 1 | 20d42bf Add note 3 | 20d42bf Add note 3 |
| 22 | 2962ae9 Add note 2 | 2962ae9 Add note 2 | 2962ae9 Add note 2 |
| 23 | 5d41002 Add note 1 | 5d41002 Add note 1 | 5d41002 Add note 1 |
| 24 | 27891b7 Initial commit | 27891b7 Initial commit | 27891b7 Initial commit |

The default order shows parents above children throughout (bold): `e49aa17` at row 2, above its
child `6bd753b` at row 6; `2e78d15` at row 7, above its children `48f0802` and `aaaf1f0`;
`35b9938` at row 11, above its child `20c5a3d` — the defect this packet removes. With
`--all` the stash joins as a tip with its index commit: `--date-order` interleaves breadth-first
(`8313cd7, 2d11c10 Rebased 5, edb659b stash, fdf1094 Rebased 4, 67d862f index, 1fd7929 Rebased
3, abed6f2, 431a3ca, 48b8f14, de02e88, 20c5a3d, 30b2c69, …`), `--topo-order` drains each line
(`8313cd7, 2d11c10 … 48b8f14, edb659b, 67d862f, abed6f2, de02e88, 30b2c69, 20c5a3d, …`).

`git log --all --graph`:

```
* 8313cd7 Fix crash on empty input
| * 2d11c10 Rebased 5
| * fdf1094 Rebased 4
| * 1fd7929 Rebased 3
| * 431a3ca Rebased 2
| * 48b8f14 Rebased 1
| | *   edb659b On main: WIP on parser
| | |\
| | | * 67d862f index on main: abed6f2 After the rebase
| | |/
| | * abed6f2 After the rebase
| |/
| *   de02e88 Merge topic twice
| |\
| | * 30b2c69 Topic two
| * | 20c5a3d Merge topic once
| |\|
| | * 35b9938 Topic one
| * | ea353a5 Main between merges
| |/
| * 9913e5f Local only commit
|/
*   6bd753b Merge branch 'feature/login'
|\
| * e49aa17 Validate login input
| * 48f0802 Add login module
* | aaaf1f0 Fix README typo
|/
* 2e78d15 Add note 6
… (Add note 5 to Initial commit, one lane)
```

### 1.5 The orders under clock skew

`fx/build-skew.sh`: `A`(t=1000) `B`(2000) on `main`; `side` from `B`: `S1`(2500)
`S2-skewed`(**500**, older than its parent and than `A`) `S3`(2600); `main`: `C`(3000)
`D-skewed`(**1500**, older than its parent) `E`(3100), then `M` merging `side` (4000); `late`
from `B`: `L1`(5000). Identical across the three gits:

| # | default | `--date-order` | `--topo-order` |
| --- | --- | --- | --- |
| 1 | L1 (t=5000) | L1 (t=5000) | L1 (t=5000) |
| 2 | M merge side (t=4000) | M merge side (t=4000) | M merge side (t=4000) |
| 3 | E (t=3100) | E (t=3100) | S3 (t=2600) |
| 4 | S3 (t=2600) | S3 (t=2600) | S2-skewed (t=500) |
| 5 | **B (t=2000)** | D-skewed (t=1500) | S1 (t=2500) |
| 6 | D-skewed (t=1500) | C (t=3000) | E (t=3100) |
| 7 | C (t=3000) | S2-skewed (t=500) | D-skewed (t=1500) |
| 8 | **A (t=1000)** | S1 (t=2500) | C (t=3000) |
| 9 | S2-skewed (t=500) | B (t=2000) | B (t=2000) |
| 10 | S1 (t=2500) | A (t=1000) | A (t=1000) |

Default shows `B` above its children `C` and `S1`, and `A` above `S1`'s line. Both ordered modes
show a skewed child before its parent, `--date-order` holding `C` (3000) back until its child
`D-skewed` (1500) is out.

### 1.6 On the bench

`git log --all --format='%H %ct %P'` over the bench (345,545 commits, 454,203 parent edges, of
which 55,201 join commits with equal committer times and 726 a child older than its parent):

| | default | `--date-order` | `--topo-order` |
| --- | --- | --- | --- |
| edges drawn parent above child | **644** (18 tied, 605 skewed, 21 reached early) | 0 | 0 |
| lines crossing a row (commits awaited: a shown child, not shown yet), max / median / p99 | 221 / 114 / 194 | 222 / 112 / 191 | **149 / 89 / 139** |

How far apart the orders are (0-based first differing row; rows at the same index; shared ids
among the first 100):

| | first difference | same index | first-100 overlap |
| --- | --- | --- | --- |
| default vs `--date-order` | row 460 | 219,825 (63.6%) | 100 |
| default vs `--topo-order` | row 2 | 9,839 (2.8%) | 60 |
| `--date-order` vs `--topo-order` | row 2 | 9,874 (2.9%) | 60 |

A commit's position in `--topo-order` against `--date-order`: median 5,391 rows away, p99
140,035, max 236,696 — topological order pulls whole side lines up against their merges.

### 1.7 Recommendation: `--date-order`, with `--topo-order` as a second mode

**Cairn's oracle should be `git log --date-order`** (what `git log --graph --date-order`
shows):

- It is Fork's default ("By date", §1.3, row for row on the one fixture Fork was shown) and git's
  own documented order: no parent before its children, otherwise commit timestamp order.
- It is the smallest change from what Cairn draws today: on the bench it agrees with commit-time
  order for the first 460 rows and in 64% of positions; the first page is the same 100 commits.
- Cairn's stash rule — "merged into the stream by its commit time but never after the commit it
  was made on" (`docs/design/history-graph.md`, "Stash rows") — *is* `--date-order`'s rule for a
  one-parent row (section 4); under `--topo-order` a stash lands wherever the depth-first drain
  reaches it.

The trade-off: `--date-order` is not plain `git log --graph`'s default. `--topo-order` draws a
narrower graph (149 lines at most across a row of rust-lang/rust against 222) and keeps a
branch's commits together, which is why `--graph` defaults to it, but it moves most rows a long
way from commit-time order (median 5,391 rows on the bench) and is Fork's non-default setting. In
git both orders are the same machine with a different ready queue (section 2), so an engine that
reproduces one reproduces the other by swapping a date-ordered queue for a stack, and Fork's
"Sort commits: Topologically" can follow as a setting without a second algorithm. What neither
order can be is cheap without generation numbers (section 3).

## 2. git's algorithm

Citations give both tags as "2.30.9 `file:N`, 2.56.0 `file:M`". Where only one location is
given, the line is the same at both, or the other tag lacks the code, as stated.

### 2.1 Which path a walk takes

- **The options.** `--topo-order` sets `sort_order = REV_SORT_IN_GRAPH_ORDER` and `topo_order =
  1` (2.30.9 `revision.c:2230-2232`, 2.56.0 `revision.c:2468-2470`). `--date-order` sets
  `REV_SORT_BY_COMMIT_DATE` and `topo_order = 1` (2.30.9 `revision.c:2247-2249`, 2.56.0
  `revision.c:2485-2487`). `--author-date-order` sets `REV_SORT_BY_AUTHOR_DATE`. The enum is
  `commit.h:211-213` (2.30.9) and `commit.h:226-228` (2.56.0), with `REV_SORT_IN_GRAPH_ORDER =
  0`, and a `rev_info` starts at it (2.30.9 `revision.c:1819`, 2.56.0 `revision.h:432`).
  `--graph` sets `topo_order = 1` and leaves `sort_order` alone (2.30.9 `revision.c:2433-2436`
  while parsing; 2.56.0 `revision_opts_finish`, `revision.c:2782-2790`, after all arguments).
  So `--graph` alone is `--topo-order`, and `--graph --date-order` in either order is
  `--date-order` (Corrections 1).
- **What forces the whole-list path.** `setup_revisions` runs `if (revs->topo_order &&
  !generation_numbers_enabled(the_repository)) revs->limited = 1;` (2.30.9
  `revision.c:2858-2859`, 2.56.0 `revision.c:3195-3196`). `generation_numbers_enabled` (2.30.9
  `commit-graph.c:658`, 2.56.0 `commit-graph.c:778`) is false when `prepare_commit_graph` loads
  no graph. That happens when there is no commit-graph file, when `core.commitGraph` is not on
  (2.30.9 `commit-graph.c:637-638`, 2.56.0 `commit-graph.c:757`), and when
  `commit_graph_compatible` refuses (2.30.9 `commit-graph.c:201-217`, 2.56.0
  `commit-graph.c:223-239`) because of a replace ref (unless replace objects are turned off), a
  graft or a shallow repository. It is also false when the graph's first commit has generation 0,
  that is, a graph written without generations. `limited` is forced, whatever the graph, by
  `--ancestry-path`, `--simplify-merges`, `--simplify-by-decoration`, `--cherry-pick`,
  `--cherry-mark`, `--left-only`, `--right-only`, `--cherry`, `--children` and line-level log
  (`revision.c`, `handle_revision_opt`). An `UNINTERESTING` tip (`^c`, `a..b`, `--not`) forces it
  only when there are no generation numbers. In `handle_commit`: `if (!revs->topo_order ||
  !generation_numbers_enabled(the_repository)) revs->limited = 1;` (2.30.9
  `revision.c:441-442`, 2.56.0 `revision.c:435-436`). `--reflog`, `--stdin`, `-n` and the ref
  globs set nothing (`add_reflogs_to_pending`, 2.30.9 `revision.c:2657-2658`, 2.56.0
  `revision.c:2938-2939`).
- **`prepare_revision_walk`** (2.30.9 `revision.c:3535`, 2.56.0 `revision.c:4016`) turns each
  pending object into a commit through `handle_commit`, which peels tags until it reaches a
  non-tag. A tree or blob is dropped (`return NULL`) unless object listing was asked for. Each
  commit is appended to `revs->commits` only the first time it is seen (`SEEN`; 2.30.9
  `revision.c:3549`, 2.56.0 `revision.c:4030`). The list is sorted by
  `commit_list_sort_by_date` (2.30.9 `revision.c:3570`, 2.56.0 `revision.c:4050`), newest
  committer date first (`commit_list_compare_by_date`, 2.30.9 `commit.c:585`, 2.56.0
  `commit.c:763`). The sort is a stable merge sort: 2.30.9's `llist_mergesort` takes from the
  earlier run unless `compare > 0` (`mergesort.c:45`, `:58`), and 2.56.0's `DEFINE_LIST_SORT`
  "Take[s] from `list` on equality" (`mergesort.h:4`). So equal dates keep the order the tips
  were given (§1.2). Then: `if (revs->limited) { limit_list(revs); if (revs->topo_order)
  sort_in_topological_order(&revs->commits, revs->sort_order); } else if (revs->topo_order)
  init_topo_walk(revs);` (2.30.9 `revision.c:3573-3579`, 2.56.0 `revision.c:4053-4059`).
- **`get_revision_1`** (2.30.9 `revision.c:3950`, 2.56.0 `revision.c:4436`): with a topo walk
  set up, it takes `next_topo_commit` and calls `expand_topo_walk` on it (2.30.9
  `revision.c:3958`, `:3981`; 2.56.0 `revision.c:4451`, `:4484`). With `limited`, it pops the
  finished `revs->commits` and walks no further. 2.56.0 names these as modes (`get_walk_mode`,
  `revision.c:4423`: reflog, topo, limited, no-walk, streaming).
- **`limit_list`** (2.30.9 `revision.c:1391`, 2.56.0 `revision.c:1446`) walks every reachable
  commit newest date first, from a date-ordered frontier. 2.30.9 uses a commit list fed by
  `commit_list_insert_by_date`, which inserts after every equal date (`commit.c:572`); 2.56.0
  uses a `prio_queue` by `compare_commits_by_commit_date` (`revision.c:1454`). It keeps each
  interesting commit in the order popped. Parents are queued by `process_parents` in parent order
  (2.30.9 `revision.c:1146`, 2.56.0 `revision.c:1189`), each the first time it is seen.

So **without a usable commit-graph an ordered log never uses the incremental topo walk**: it
walks every reachable commit (`limit_list`), then sorts the whole list. Verified: on the bench
(no commit-graph) 2.56.0 prints no `topo_walk` statistics for `--topo-order`, which it does for
every incremental topo walk; the first line costs what the whole output costs (§3); and RSS is
the same 522 MiB for 100 lines as for all of them.

### 2.2 The whole-list sort: `sort_in_topological_order`

`commit.c` (2.30.9 `commit.c:765`, 2.56.0 `commit.c:938`; identical at both but for a renamed
free). The queue's compare is NULL for `REV_SORT_IN_GRAPH_ORDER`, `compare_commits_by_commit_date`
for `REV_SORT_BY_COMMIT_DATE` and `compare_commits_by_author_date` for the author order (2.30.9
`commit.c:781-792`, 2.56.0 `commit.c:954-965`). Every commit in the list gets an in-degree slot
set to 1. One pass over the list adds 1 to a parent's slot for each (child, parent) entry, but
only if the slot is non-zero, meaning the parent is in the list (2.30.9 `commit.c:811`, 2.56.0
`commit.c:984`). A parent listed twice counts twice. Every commit still at 1 (no child in the
list) is put into the queue in list order (2.30.9 `commit.c:828`, 2.56.0 `commit.c:1001`). For
graph order the queue is then reversed (`prio_queue_reverse`, 2.30.9 `commit.c:836`, 2.56.0
`commit.c:1009`), with the comment that "the initial tips need to be shown in the order given
from the revision traversal machinery", so that the first in the list pops first from the LIFO
queue.

The loop pops a commit. For each parent in parent order whose slot is non-zero, it does
`if (--(*pi) == 1) prio_queue_put(&queue, parent)` (2.30.9 `commit.c:858`, 2.56.0
`commit.c:1031`). Then it sets the popped commit's slot to 0 and appends it to the output.

`prio-queue.c`: `compare` calls the queue's compare and breaks a 0 by the insertion counter
`ctr`, earlier first (2.30.9 `prio-queue.c:4-11`: `cmp = ctr_i - ctr_j`; 2.56.0
`prio-queue.c:4-12`, the same as a three-way compare). `prio_queue_put` stamps each element with
`insertion_ctr++` (2.30.9 `prio-queue.c:42`, 2.56.0 `prio-queue.c:73`, `:81`). With a NULL
compare, `put` appends and `get` takes the array's last element, which makes a LIFO stack
(2.30.9 `prio-queue.c:45-46`, `:65-66`; 2.56.0 `prio-queue.c:84-85`, `:104-105`).
`prio_queue_reverse` refuses a queue with a compare (2.30.9 `prio-queue.c:18-26`, 2.56.0
`prio-queue.c:19-29`).

`compare_commits_by_commit_date` (2.30.9 `commit.c:751`, 2.56.0 `commit.c:923`) returns
`a->date < b->date ? 1 : a->date > b->date ? -1 : 0`, so the newest pops first. `commit->date`
is the committer timestamp in seconds, the time zone ignored (`parse_commit_date`, 2.30.9
`commit.c:81`, 2.56.0 `commit.c:128`). A missing or unparsable committer line gives 0. 2.30.9
reads from the first `>` of the committer line and 2.56.0 from the last, which differ only for a
malformed ident. A commit read from a commit-graph takes its date from CDAT's 34-bit field (2.56.0
`commit-graph.c:897`), the same value for any real date.

### 2.3 The incremental topo walk (`revision.c`)

State is `struct topo_walk_info` (2.30.9 `revision.c:3304`, 2.56.0 `revision.c:3739`):
`min_generation` (`uint32_t` in 2.30.9, `timestamp_t` in 2.56.0), three `prio_queue`s, an
`indegree` slab and an `author_date` slab. `explore_queue` and `indegree_queue` compare by
`compare_commits_by_gen_then_commit_date` (2.30.9 `revision.c:3443-3444`, 2.56.0
`revision.c:3910-3911`; the function at 2.30.9 `commit.c:731`, 2.56.0 `commit.c:902`): the
higher `commit_graph_generation` first, then the newer commit date, then insertion order.
`topo_queue`'s compare is chosen exactly as in 2.2 (2.30.9 `revision.c:3429-3440`, 2.56.0
`revision.c:3896-3907`). A commit enters `explore_queue` or `indegree_queue` at most once, by the
`TOPO_WALK_EXPLORED` and `TOPO_WALK_INDEGREE` flags (`test_flag_and_insert`, 2.30.9
`revision.c:3313`, 2.56.0 `revision.c:3768`).

The in-degree slot means: 0, not reached by the in-degree walk, or already emitted; otherwise 1
plus the number of the commit's children that the in-degree walk has reached and that have not
been emitted. A commit is ready when its slot is 1.

- **`init_topo_walk(revs)`** (2.30.9 `revision.c:3413`, 2.56.0 `revision.c:3880`):
  `min_generation = GENERATION_NUMBER_INFINITY` (2.30.9 `:3446`, 2.56.0 `:3913`). For each tip
  in `revs->commits` order (already date-sorted, 2.1): parse it, skipping one that fails; put it
  into `explore_queue` and `indegree_queue`; lower `min_generation` to its generation; set its
  slot to 1. Then `compute_indegrees_to_depth(revs, min_generation)` (2.30.9 `:3466`, 2.56.0
  `:3933`). Then, again in `revs->commits` order, put every tip whose slot is still 1 into
  `topo_queue` (2.30.9 `:3472`, 2.56.0 `:3939`), reversing it for graph order (2.30.9 `:3480`,
  2.56.0 `:3947`).
- **`compute_indegrees_to_depth(revs, gen_cutoff)`** (2.30.9 `revision.c:3390`, 2.56.0
  `revision.c:3852`): while `indegree_queue`'s head has `commit_graph_generation >= gen_cutoff`,
  run `indegree_walk_step`.
- **`indegree_walk_step(revs)`** (2.30.9 `revision.c:3360`, 2.56.0 `revision.c:3817`): pop
  `indegree_queue`'s head and parse it. Run `explore_to_depth(revs, its generation)`. Then, for
  each parent in order: the slot goes 0 → 2 or is incremented (`*pi = 2` at 2.30.9 `:3381`,
  2.56.0 `:3843`), and the parent is queued in `indegree_queue`. With `--first-parent`, the first
  parent only. 2.56.0 also parses each parent first and returns early if that fails.
- **`explore_to_depth` / `explore_walk_step`** (2.30.9 `revision.c:3350`, `:3322`; 2.56.0
  `revision.c:3807`, `:3777`): the same descent over `explore_queue`. It marks a commit older
  than `--since` `UNINTERESTING`, runs `process_parents`, which propagates `UNINTERESTING`, and
  queues the parents. Its job is to settle what is uninteresting far enough down that
  `expand_topo_walk` skips those parents. With no negative tip it walks the same commits as the
  in-degree walk (bench: `count_explore_walked` equals `count_indegree_walked` in every run
  below).
- **`next_topo_commit(revs)`** (2.30.9 `revision.c:3483`, 2.56.0 `revision.c:3955`): pop
  `topo_queue` and set the commit's slot to 0.
- **`expand_topo_walk(revs, commit)`** (2.30.9 `revision.c:3497`, 2.56.0 `revision.c:3969`):
  `process_parents` on the emitted commit, then for each parent in order: skip it if
  `UNINTERESTING` or unparsable. If its generation is below `min_generation`, lower
  `min_generation` and `compute_indegrees_to_depth(revs, min_generation)` (2.30.9 `:3521`, 2.56.0
  `:3995`). Then `(*pi)--` (2.30.9 `:3526`, 2.56.0 `:4000`), and put the parent into
  `topo_queue` when the slot reaches 1 (2.30.9 `:3528`, 2.56.0 `:4002`). With `--first-parent`,
  stop after the first.

**Why generation numbers bound the lookahead.** Both generation numbers satisfy gen(child) >
gen(parent). v1, the topological level, is 1 for a root and otherwise one more than the largest
parent's (2.30.9 `Documentation/technical/commit-graph.txt:41-46`; 2.56.0
`Documentation/technical/commit-graph.adoc:59-64`). v2, the corrected committer date, is the
committer date for a root (1 for a root at time 0) and otherwise the maximum of the committer date
and one more than the largest parent's corrected date (2.56.0
`Documentation/technical/commit-graph.adoc:46-57`). So every commit that can reach `c` and has
not been walked has a generation above gen(`c`). Once the in-degree walk has consumed every queued
commit with generation ≥ gen(`c`), `c`'s slot counts all of its children, and `c` may be emitted
when the slot drops to 1. `expand_topo_walk` deepens the walk to a parent's generation before it
decrements that parent's slot, so a parent is never declared ready on a partial count. The walk
never looks further than the lowest generation it is about to emit, or, at `init_topo_walk`, than
the lowest generation among the tips (section 3.3 measures what that costs).

**Same order on both paths.** A commit becomes ready on both paths at the same moment: when its
last child is emitted, during the same parent loop. Tips that start ready are put in the same
relative order on both paths. For the incremental walk that is `revs->commits` order. For the
whole-list sort it is `limit_list`'s output order, in which the given tips keep their date-sorted,
stable relative order (2.1). So the two queues receive the same commits in the same insertion
order, and they pop the same sequence. §3.4 measures the same thing.

**Without generation numbers.** `commit_graph_generation` returns `GENERATION_NUMBER_INFINITY`
for a commit with no graph data (2.30.9 `commit-graph.c:102-113`, also for a commit not from the
graph; 2.56.0 `commit-graph.c:126-135`, for a missing or zero generation). The value is
`0xFFFFFFFF` at 2.30.9 (`commit.h:14`) and `(1ULL << 63) - 1` at 2.56.0 (`commit.h:12`). So in a
graph that does not cover every commit, `compute_indegrees_to_depth(INFINITY)` walks every
uncovered commit reachable before the first line. With no usable graph at all, git never gets
that far: `generation_numbers_enabled` is false and the walk takes the `limit_list` path of 2.1,
the same cost by another route. Verified with a stale graph: 5,000 commits added past a
commit-graph of the bench copy, `git log -n 100 --topo-order` from their tip walked exactly 5,000
commits (`count_indegree_walked: 5000`). It walked every commit outside the graph and none inside
it.

**`--date-order` with and without the graph** takes the same two paths. `topo_order` is set, so
it is `limit_list` plus the whole-list sort without a graph, and the incremental walk with one,
the date-ordered `topo_queue` being the only difference. Measured in §3: the same cost as
`--topo-order` in both cases.

**Generation versions by git version.** Chunks of the commit-graph each git wrote for the tied
fixture: 2.30.9 `OIDF OIDL CDAT` (v1 only); 2.32.7 `OIDF OIDL CDAT GDAT`; 2.56.0 `OIDF OIDL CDAT
GDA2`. `gitformat-commit-graph(5)`, "HISTORICAL NOTES": "a previous version of Git wrote possibly
erroneous data in these chunks with the IDs "GDAT" and "GDOV". By changing the IDs, newer versions
of Git will silently ignore those older chunks". `git-config(1)`: `commitGraph.generationVersion`
"Defaults to 2"; "If version 1 is specified, then the corrected commit dates will not be written
or read." Order is unaffected by any of this (§3.4).

### 2.4 The tips: `--all`, `refs/stash` and `--reflog`

- `--all` — "Pretend as if all the refs in refs/, along with HEAD, are listed on the command line
  as <commit>" — includes `refs/stash`: `stash@{0}` alone (`stash@{1}` and older live only in
  the stash's reflog), and the stash's index commit (and untracked commit) become ordinary
  parents in the graph (`e5ddb8f index on main` in §1.3's fixture). It also includes
  `refs/notes/*`, `refs/replace/*`, `refs/bisect/*`, `refs/original/*` and, unless
  `--single-worktree`, every other worktree's `HEAD` — none of which Cairn walks.
- `--exclude=<glob-pattern>` "Do not include refs matching <glob-pattern> that the next --all,
  --branches, --tags, --remotes, or --glob would otherwise consider" — it does **not** narrow
  `--reflog`: on a two-stash fixture `git log --reflog` and `git log --exclude=refs/stash
  --reflog` list the same 17 commits at 2.30.9, 2.32.7 and 2.56.0.
- `--reflog` — "Pretend as if all objects mentioned by reflogs are listed on the command line as
  <commit>" — every reflog: `HEAD`'s, every local branch's, every remote-tracking ref's and
  `refs/stash`'s (so every stash entry with its index commits), and other worktrees' `HEAD`s
  unless `--single-worktree`. Each entry adds its old id as well as its new: with the only entry
  naming a commit `B` being the amend that replaced it (old id `B`), `git log --reflog` lists `B`
  at all three versions.
- **The order `--reflog` adds its tips changed between the floor and today**, and at the floor it
  depends on the filesystem. Branches `aaa`, `zzz`, `mmm` each given one commit then reset, all
  at one second, `HEAD`'s reflog removed: 2.30.9 and 2.32.7 show `mmm, zzz, aaa` — the directory
  order `ls -U .git/logs/refs/heads` printed on this tmpfs — and 2.56.0 shows `aaa, mmm, zzz`.
  With `HEAD`'s reflog present, 2.30.9 and 2.32.7 read it after the branches' and 2.56.0 first.
  At equal dates, then, `--reflog` is not an oracle that holds across versions.
- `-g`/`--walk-reflogs` walks reflog entries instead of ancestry and cannot draw a graph: `git
  log -g --graph` fails with "fatal: cannot combine --walk-reflogs with history-limiting options"
  at all three versions. git has no option naming just `HEAD`'s and the local branches'
  reflogs.

## 3. What git pays for its first page, on the bench

### 3.1 The bench

`~/Development/bench/rust`: 345,545 commits from every ref (`git rev-list --all --count`, the
same from `--branches --remotes --tags HEAD`), 340,228 from `HEAD`; one local branch, 11
remote-tracking refs, 163 tags (annotated, back to `0.1`), no stash, one entry in `HEAD`'s
reflog; one 1.0 GB pack, no alternates, **no commit-graph** (`objects/info/` empty).

The commit-graph runs used a copy: `cp -r ~/Development/bench/rust/.git` into the scratchpad
(tmpfs), `core.bare true`, `gc.auto 0`, `maintenance.auto false`, then `git commit-graph write
--reachable` with 2.56.0: 1.52 s, a 20,734,804-byte file with chunks `OIDF OIDL CDAT GDA2 EDGE`
— generation v2, 2.56.0's default. The copy without a graph measured the same as the bench
(`--topo-order`, 100 lines: 1.36 s, 1.39 s; 523 MiB).

### 3.2 Timings, `git log --all <order> --format=%H`, git 2.56.0

| Order | No commit-graph: 100 lines | No commit-graph: all | v2 commit-graph: 100 lines | v2 commit-graph: all |
| --- | --- | --- | --- | --- |
| default | 7 ms, 66 MiB | 2.16 s, 295 MiB | 3 ms, 74 MiB | 0.161 s, 116 MiB |
| `--date-order` | **1.323 s, 522 MiB** | 1.522 s, 523 MiB | **84 ms, 130 MiB** | 0.204 s, 131 MiB |
| `--topo-order` | **1.314 s, 522 MiB** | 1.508 s, 523 MiB | **108 ms, 130 MiB** | 0.242 s, 131 MiB |
| `--graph` | **1.319 s, 523 MiB** | 3.214 s, 523 MiB (640,205 lines) | **109 ms, 130 MiB** | 1.951 s, 131 MiB |

Each cell: wall time to the 100th line (or to exit) and max RSS. Without a graph the first line
of every ordered mode arrives at 1.31-1.39 s — after the whole walk, as 2.1 says. The floor gits
are the same: 100 lines of `--topo-order` / `--date-order` without a graph 1.32 s / 1.34 s
(2.30.9) and 1.32 s / 1.37 s (2.32.7), 516 MiB; reading the 2.56.0-written graph (they read its
CDAT levels, not GDA2) 150 ms / 137 ms (2.30.9) and 144 ms / 148 ms (2.32.7), 120-124 MiB. The
default order's whole output is slower than the ordered modes' without a graph (2.16 s against
1.51 s, the runs spread 1.78-2.51 s); not investigated.

### 3.3 With a graph, the first page is the oldest tip's lookahead, not the page

The topo walk's own counters (2.56.0, `GIT_TRACE2_PERF`, `revision.c:3763`), for `git log -n 100`
over the v2-graph copy:

| Tips | `count_indegree_walked` (v2) | (v1, `-c commitGraph.generationVersion=1`) | first line |
| --- | --- | --- | --- |
| `--all` (= Cairn's tips here) | **337,966** of 345,545 | 283,323 | 82-118 ms |
| `--branches --remotes` | 18,910 | 16,568 | 6 ms |
| `HEAD` | 4,147 (`--date-order` 4,265) | 3,654 (4,441) | 1 ms |

`count_topo_walked` is 100 in every row: git emits 100 commits but must first compute in-degrees
down to `min_generation`, the lowest generation among **all tips** (`init_topo_walk`, 2.3). The
bench's tags reach back to 2012, so the in-degree walk before line 1 covers 98% of the history —
graph-accelerated (0.1 s against 1.3 s) but not proportional to the page. From `HEAD` alone the
lookahead is set by what is emitted: 4,147 commits for 100 rows, 18,457 for 1,000, 33,372 for
10,000, because each bors merge's second parent carries an older corrected date that pulls the
in-degree walk down to it (2.3, `expand_topo_walk`). The oldest remote-tracking ref
(`origin/cargo_update`, 2026-03-15) sets the 18,910.

So git's own first page, on this repository with every ref as a tip, is either the whole walk (no
graph, 1.3 s, 522 MiB) or a walk to the oldest tip's generation (graph, 0.1 s, 130 MiB). The
eager part is `init_topo_walk` lowering `min_generation` over every tip before the first emit;
whether an engine can produce the identical order while walking in-degrees only to the
generation of the next commit it emits — deferring the readiness check of an old tip until the
queue reaches it — is a design question for the packet, to be decided by the oracle tests of
section 4, not settled here.

### 3.4 The order does not depend on the path, the graph or the version

The full 345,545-id output's sha256, per tip set and order, from 2.56.0 on the bench (no graph),
2.56.0 on the graph copy, 2.30.9 on both, and 2.32.7 on the graph copy:

| | `--all` | `--branches --remotes --tags HEAD` |
| --- | --- | --- |
| `--topo-order` | `7f120c68fdc1` ×5 | `7f120c68fdc1` ×5 |
| `--date-order` | `a8471e2d2464` ×5 | `a8471e2d2464` ×5 |
| default | `d5e326deb3f2` ×5 | `d5e326deb3f2` ×5 |

And on every fixture of this record, all three gits with and without a graph each wrote gave
identical output (54 comparisons). The whole-list sort and the incremental walk produce the same
order, so one oracle serves whether or not a repository has a commit-graph.

### 3.5 The bench was not touched

A marker file was touched in the scratchpad before the first command (2026-10-17 13:38:54
+0100), and `find ~/Development/bench/rust/.git -newer <marker>` (bfs 4.1.1, stderr shown) was
run before any git, after the timings, after the copy and at the end: empty each time, exit 0.
Every git run on the bench carried `GIT_OPTIONAL_LOCKS=0`; no commit-graph or other file was
written there.

## 4. The parity oracle for Cairn

### 4.1 Tips and their order

Cairn walks every local branch, every remote-tracking ref, every tag that identifies a commit,
and `HEAD` (`docs/design/history-graph.md`, "What it walks") — `--branches --remotes --tags HEAD`,
**not** `--all --exclude=refs/stash`, which would still add notes, replace, bisect and
`refs/original` refs and other worktrees' `HEAD`s. Because equal dates keep the order the tips
were given (§1.2), the oracle fixes Cairn's tip order too: `refs/heads/*`, `refs/remotes/*`,
`refs/tags/*`, each in refname byte order, then `HEAD`, then the stashes, then the reflog tips. A
tip already given keeps its first place.

### 4.2 Stash rows: each stash grafted to its base

A stash is a row with one edge, to its base, "drawn … as a commit whose one parent is the stash's
base would be". git can be asked exactly that in a fixture by replacing each stash commit with a
one-parent copy: `git replace --graft <stash> <stash>^1` (writes `refs/replace/<id>` — fixtures
only, never the bench). Then, with the stashes on stdin in `stash@{0}`, `stash@{1}`, … order:

```sh
for s in $(git stash list --format=%H); do git replace --graft "$s" "$s^1"; done
git stash list --format=%H |
  git log --date-order --format=%H --branches --remotes --tags HEAD --stdin
```

Only stashes whose base the ref walk reaches may be given (a stash whose base no ref reaches has
no row in Cairn, and giving it would add its base's history): filter each with `git merge-base
--is-ancestor <stash>^1 <tip>` over the tips, or with `git rev-list --branches --remotes --tags
HEAD` membership. Under `--date-order` the grafted stash is a tip ordered by its own committer
date and, having its base as its only parent, always before its base — Cairn's stated rule. On
§1.3's fixture this oracle yields Fork's 14 rows exactly; on a two-stash fixture (`stash@{0}` on
`bugfix/crash`, `stash@{1}` on `main`, all tied) `--date-order` gives `dcb62f5 stash@{0},
f7ad061 stash@{1}, 8313cd7, 9913e5f, 6bd753b, aaaf1f0, e49aa17, 48f0802, 2e78d15, …` whichever
end of the argument list the stashes are given at. A replace ref turns the commit-graph off
(`commit_graph_compatible`: 2.30.9 `commit-graph.c:206-209`, 2.56.0 `commit-graph.c:228-231`), so
this oracle always runs the whole-list path, `limit_list` then `sort_in_topological_order` (§2.1,
§5.3). That does not matter, because §2.3 and §3.4 show the order is the same on both paths.

### 4.3 Show Lost Commits: the reflog tips given explicitly

Cairn adds the old and the new id of every entry of `HEAD`'s reflog and each local branch's.
Since `--reflog` takes every reflog, cannot be narrowed by `--exclude`, and orders its tips
differently at the floor (§2.4), the oracle gives the tips itself, after the stashes: `HEAD`'s
log, then each `refs/heads/*` log in refname order, each read oldest entry first, old id then new,
null ids skipped (files backend, the only one Cairn opens):

```sh
reflog_tips() {
  { cat .git/logs/HEAD
    for f in $(cd .git/logs && find refs/heads -type f | LC_ALL=C sort); do cat ".git/logs/$f"; done
  } | awk '{ if ($1 !~ /^0+$/) print $1; if ($2 !~ /^0+$/) print $2 }'
}
{ git stash list --format=%H; reflog_tips; } |
  git log --date-order --format=%H --branches --remotes --tags HEAD --stdin
```

On a fixture whose only reflogs are `HEAD`'s and the branches' this reproduces 2.56.0's own
`--branches --remotes --tags HEAD --reflog` order, and it gives that same order at 2.30.9 and
2.32.7, where `--reflog` itself does not.

### 4.4 Agreement across versions

The oracle (4.2 and 4.3; normal and Show Lost Commits; `--date-order` and `--topo-order`) over the
recipe fixture, the tied fixture, the two-stash fixture, the skew fixture and the reflog fixture:
sha256 of the output identical at 2.30.9, 2.32.7 and 2.56.0 in all 20 combinations. In the tied
fixture Show Lost Commits adds the five pre-rebase commits and changes the order, as it should.

## 5. The algorithm as Cairn must copy it

This is `--date-order` as git runs it when the tips are given explicitly and none is excluded,
which is Cairn's case. It is written so that an engine over `gix_revwalk`'s commit access (or
plain object reads) reproduces git's rows exactly, every tie included. Each step cites the code
of §2 it reproduces.

### 5.1 The order, step by step

```text
Input: the pending tips, in the order git receives them.

1. Tips (prepare_revision_walk, §2.1)
   pending := refs/heads/*  in refname byte order        -- --branches
            ++ refs/remotes/* in refname byte order       -- --remotes
            ++ refs/tags/*    in refname byte order       -- --tags
            ++ [HEAD]                                     -- the positional argument
            ++ stdin lines, in order                      -- --stdin: stashes, then reflog tips (§4.2, §4.3)
   tips := []
   for o in pending:
       while o is an annotated tag: o := its target       -- handle_commit peels
       if o is not a commit: skip it                      -- a tree or blob is dropped, no error
       if o is already in tips: skip it                   -- SEEN: the first place is kept
       tips.append(o)
   tips := STABLE_SORT(tips, by committer_time, newest first)
                                                          -- commit_list_sort_by_date; equal
                                                          -- times keep the order above
2. In-degrees (sort_in_topological_order, §2.2)
   R := every commit reachable from tips through every parent
        (with the parents git sees: replace refs and grafts applied)
   for c in R: indeg[c] := 1
   for c in R: for p in parents(c), every entry, duplicates included:
                   indeg[p] += 1
   -- now indeg[c] = 1 + the number of parent entries naming c, over c's children in R

3. The ready queue (prio_queue, §2.2)
   Q: a priority queue of commits, ordered by
        key(c) = (committer_time(c) descending, seq(c) ascending)
      seq(c) = the value of one counter, incremented on every push, never reset.
      Pop always returns the least key: the newest committer time, and among equal times
      the commit pushed earliest.
   for t in tips, in their sorted order:
       if indeg[t] == 1: push(Q, t)                       -- a tip that is no other tip's
                                                          -- ancestor; the others wait

4. Emission
   while Q is not empty:
       c := pop(Q)
       emit c                                             -- the next row
       for p in parents(c), in the commit's parent order, every entry:
           indeg[p] -= 1
           if indeg[p] == 1: push(Q, p)                   -- all of p's children are out
       indeg[c] := 0
```

The ties, all of them:

- **committer_time** is the committer timestamp in whole seconds as written in the commit, the
  time zone ignored (`parse_commit_date`, §2.2). git holds it unsigned (`timestamp_t` is
  `uintmax_t`, 2.30.9 `git-compat-util.h:329`, 2.56.0 `compat/posix.h:267`, parsed by
  `strtoumax`). A missing or unparsable committer date counts as 0.
- **Equal times among the initial tips** pop in their sorted order, which is the given order
  (steps 1 and 3).
- **Equal times between commits made ready later** pop in the order they became ready (`seq`).
  A merge's parents that become ready in one emission get `seq` in parent order, so at equal
  times the first parent comes first. Commits made ready by an earlier emission come before
  those made ready by a later one, so parallel lines at one time interleave a row at a time (§1.4).
- **No other key exists.** Generation numbers, author dates and object ids never decide the
  order. The generation-ordered queues of §2.3 only decide how far the in-degree walk has gone.

**`--topo-order`** is the same machine with Q a stack. Push appends, pop takes the last element.
Step 3's initial pushes are followed by reversing the stack, so the first tip pops first. A
merge's parents are then popped last parent first (§2.2, `prio_queue_reverse`).

### 5.2 Computing step 2 lazily

Step 2 as written walks all of R before the first row, which is git's whole-list path. The
order stays the same under any lazier scheme that keeps one invariant: **a commit is pushed only
when `indeg` counts every one of its children in R**. Then each push happens in the same
emission and the same parent loop as above, so it gets the same `seq` (§2.3, "Same order on both
paths"). git keeps the invariant with generation numbers. Before deciding any commit `c`, it walks
in-degrees down through every queued commit with generation ≥ gen(`c`): first to the lowest tip
generation (`init_topo_walk`), then to each parent's generation before decrementing it
(`expand_topo_walk`). gix reads a commit-graph's generations. Without them, no bound is known and
git walks everything. Whether Cairn defers the old tips' share of `init_topo_walk`'s eager walk
(§3.3) is the packet's design question. Any such scheme is correct exactly when it keeps the
invariant, and the oracle decides.

### 5.3 Which path git takes for Cairn's oracle

The oracle is `git log --date-order --format=%H --branches --remotes --tags HEAD --stdin`, the
stashes and reflog tips on stdin and no exclusion.

- **With the stashes grafted (§4.2)**, the replace refs turn the commit-graph off
  (`commit_graph_compatible`), `generation_numbers_enabled` is false, `setup_revisions` sets
  `limited`, and git runs **`limit_list` then `sort_in_topological_order`**, the whole-list path,
  at every version and whether or not a commit-graph file exists.
- **With no stash, and so no replace ref**, the path depends on the fixture. With no commit-graph
  file, which is how the fixtures here are built, it is again `limit_list`. With a commit-graph
  file it is the incremental walk, `init_topo_walk`, `next_topo_commit` and `expand_topo_walk`:
  no option the oracle passes sets `limited`, `--stdin` included, and an excluded tip would not
  set it either while generation numbers are on.
- Measured at 2.56.0 on a three-commit fixture with a stash and a commit-graph, by whether
  `GIT_TRACE2_PERF` carries `topo_walk` statistics (emitted only by the incremental walk,
  `revision.c:3763`): graph and no replace ref, present; graph and one `git replace --graft`,
  absent; no graph, absent.

Both paths give the same order (§2.3, §3.4), so the oracle is right on either. Step 5.1 is the
whole-list path written out, and §5.2 is the condition under which the incremental one equals
it.

## Fixture scripts

Each run from an empty directory with `GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1`.

`build-tied.sh` (with the argument `recipe` it stops at the recipe's stash, giving Fork's
fixture byte for byte; without, it goes on):

```bash
set -e
rm -rf tied && mkdir tied && cd tied
git init -q -b main --bare remote.git
git init -q -b main t && cd t
git config user.name "Ada Lovelace"; git config user.email ada@example.com
export GIT_AUTHOR_DATE="2026-10-16T12:00:00+01:00" GIT_COMMITTER_DATE="2026-10-16T12:00:00+01:00"
# ... docs/research/ui-review/fixture-recipe.md verbatim, from `mkdir src` to
# `echo "line 7" >> notes.txt; git commit -qam "Local only commit"` ...
if [ "$1" = recipe ]; then echo "// wip" >> src/main.rs; git stash push -qm "WIP on parser"; exit 0; fi
git checkout -qb topic
echo "t1" > topic.txt; git add -A; git commit -qm "Topic one"
git checkout -q main
echo "m1" >> notes.txt; git commit -qam "Main between merges"
git merge -q --no-ff topic -m "Merge topic once"
git checkout -q topic
echo "t2" >> topic.txt; git commit -qam "Topic two"
git checkout -q main
git merge -q --no-ff topic -m "Merge topic twice"
git checkout -q -b rebased main~2
for i in 1 2 3 4 5; do echo "r$i" >> rebased.txt; git add -A; git commit -qm "Rebased $i"; done
git rebase -q main
git checkout -q main
echo "line 8" >> notes.txt; git commit -qam "After the rebase"
echo "// wip" >> src/main.rs; git stash push -qm "WIP on parser"
```

`build-skew.sh`:

```bash
set -e
rm -rf skew && mkdir skew && cd skew
git init -q -b main && git config user.name "Ada Lovelace" && git config user.email ada@example.com
c() { export GIT_AUTHOR_DATE="@$1 +0000" GIT_COMMITTER_DATE="@$1 +0000"
      echo "$2" > "$2"; git add -A; git commit -qm "$2 (t=$1)"; }
c 1000 A; c 2000 B
git checkout -qb side
c 2500 S1; c 500 S2-skewed; c 2600 S3
git checkout -q main
c 3000 C; c 1500 D-skewed; c 3100 E
export GIT_AUTHOR_DATE="@4000 +0000" GIT_COMMITTER_DATE="@4000 +0000"
git merge -q --no-ff side -m "M merge side (t=4000)"
git checkout -qb late main~4
c 5000 L1
git checkout -q main
```

The two-stash fixture is the recipe fixture plus, on `bugfix/crash`, `echo "// wip2" >>
src/main.rs; git stash push -qm "WIP on crash"`. The reflog fixture: at `@1000`, a root commit;
branch `zzz` given two commits then reset to `main`; branch `aaa` one commit then reset; a
detached commit; then `git commit --amend` of the root.

## What this record does NOT settle

- git 2.32.7's source (only v2.30.9 and v2.56.0 were read; 2.32.7's order is measured, §3.4,
  §4.4), and the tag in which `GENERATION_NUMBER_INFINITY` became 64-bit.
- Whether Fork's "By date" is `--date-order` beyond the one merge of §1.3 — in particular its
  breadth-first interleaving of tied parallel lines.
- Whether an engine can match git's order while computing in-degrees lazily from the tips (3.3),
  and what that costs without a commit-graph, where no generation number bounds the lookahead:
  git's own answer there is to walk everything first.
- Where among equal-dated tips the stashes belong. 4.2 puts them after `HEAD`; git's `--all`
  would put `refs/stash` between `refs/remotes/` and `refs/tags/`. Either is reproducible; the
  packet chooses and the oracle follows.
