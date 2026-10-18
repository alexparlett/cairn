# The date-order walk measured on the bench

Evidence record for the `history-topo-order` packet: what Cairn's `HistoryOrder::DateOrder`
walk (`crates/cairn-git/src/history/date_order.rs`) costs on rust-lang/rust, against git's own
`--date-order` on the same repository, measured 2026-10-18 when the walk was first built. This
is the opening-pass half of the PRD's R6 (R6.1, R6.2, R6.4's per-commit state); the history
thread's and the window's numbers are added when they exist.

## Method

- **The repository.** `~/Development/bench/rust`, rust-lang/rust at `c999cef531e`: 345,545
  commits from every ref (`--branches --remotes --tags HEAD`, 176 tips once each ref is
  peeled and the duplicates kept, as git is given them), one 1.04 GiB pack, no commit-graph.
  Read only: every `git` run on it carried `GIT_OPTIONAL_LOCKS=0`, and Cairn opens it with
  gitoxide, which writes nothing. A marker file was touched in the session's scratchpad before
  the first command (2026-10-18 08:18:49 +0100), and `find ~/Development/bench/rust/.git
  -newer <marker>` was run after the oracle was written, after the copy, after the timings and
  at the end: empty each time, exit 0, nothing on stderr.
- **The copy with a commit-graph.** `cp -r ~/Development/bench/rust/.git` into the scratchpad
  (tmpfs), `core.bare true`, `gc.auto 0`, `maintenance.auto false`, then `git commit-graph
  write --reachable` with git 2.56.0 (1.6 s; a 20,734,804-byte file, generation v2 in GDA2 with
  the v1 levels in CDAT, which is what gitoxide reads). No alternates.
- **git.** 2.56.0, `git rev-list --date-order --max-count=64 --branches --remotes --tags HEAD
  > /dev/null`, wall time per run from the shell, seven runs per round after a warm-up.
- **Cairn.** `cargo test --release -p cairn-git --lib -- --ignored --nocapture
  measures_the_opening_pass_against_a_named_repository` with `CAIRN_BENCH_REPO` naming the
  repository: the walk opened from the same tips in git's order — `refs/heads/*`,
  `refs/remotes/*`, `refs/tags/*` each in refname order, then `HEAD` — and taken to its 64th
  row, seven times in one process; and with `CAIRN_BENCH_ORACLE` naming a file of git's
  `git log --date-order --format=%H --branches --remotes --tags HEAD` for the same tips, the
  whole walk compared with it row by row. Retained bytes are the walk's own count of its map
  (buckets at capacity, a control byte each, and the parents of merges that leave the record).
- **Interleaved.** The machine (16 threads, 60 GiB, the pack in page cache) showed run-to-run
  drift of several percent between sessions, so git and Cairn were run in alternating rounds
  and each round's medians are given side by side.

## The order

On the bench and on the copy with a commit-graph, every one of the 345,545 rows of the whole
walk equals git's `--date-order` row for row ("first difference at None"). git's own output is
the same on both (`cmp` of the two files).

## The first page from every ref, without a commit-graph (R6.1)

Median of seven, wall time to the 64th row:

| Round | git `rev-list --date-order --max-count=64` | Cairn |
| --- | --- | --- |
| 1 | 1,260 ms | 1,196 ms |
| 2 | 1,246 ms | 1,204 ms |
| 3 | 1,253 ms | 1,190 ms |

R6.1's bar is git's median plus 10% (about 1,380 ms): met, with Cairn 3-5% under git's own
time. Both read every one of the 345,545 commits before the first row, git through
`limit_list` and `sort_in_topological_order`, Cairn through its in-degree walk with every
generation infinite (`walked 345545`).

Two things were measured on the way, each a cost the walk no longer pays:

- **The object cache.** Each commit is read once and kept, so the shared 4 MiB object cache was
  only filled: reading through a handle with none took the median from 1,462 ms to 1,361 ms
  (a session's run against git's 1,345 ms then).
- **A full commit parse.** gitoxide's `CommitRefIter` parses both signatures to reach the
  committer's date. Reading the 345,545 commits through `Grafted` and parsing them so cost
  1.22 s against 1.05 s for the reads alone. The walk now reads only what git's
  `parse_commit_buffer` reads — the `tree` and `parent` lines and the date after the
  committer line's last `>` — which brought the median from about 1,350 ms to 1,195 ms.

## The first page from every ref, with a commit-graph (R6.2)

| | git | Cairn |
| --- | --- | --- |
| median of seven, to the 64th row | 82-84 ms (two rounds) | 95 ms |
| commits whose in-degrees the opening pass walked | 337,966 (v2, GT §3.3) | 283,323 |
| commits read from the graph | — | 283,651, none from objects |

git's `count_indegree_walked` with corrected commit dates is 337,966 (GT §3.3, `--all`), and
283,323 with `-c commitGraph.generationVersion=1`; Cairn reads the v1 levels and walks exactly
the v1 count. So here gitoxide's v1-only reading costs nothing — the bench's old tags set the
depth, and v1 stops sooner than v2 on this history. The 13 ms difference is per commit, not
per generation: Cairn keeps each commit read in a hash map (`IdMap`) where git keeps flags on
objects it allocated anyway and in-degrees in a slab. An earlier, noisier session measured git
at 112-130 ms and Cairn at 98-110 ms; the table's rounds were back to back. The whole walk
with the graph: 149-158 ms.

## What the walk keeps (R6.4's per-commit state)

| | Retained by the walk | Per commit read |
| --- | --- | --- |
| no commit-graph, after the opening pass (and at the end: all are read at the open) | 56.3 MiB | 170 B |
| commit-graph, after the opening pass (283,651 read) | 55.4 MiB | 204 B (the map's buckets sized for growth) |
| commit-graph, at the end of the walk | 56.3 MiB | 170 B |

The process's peak resident set over the whole walk was 377 MB without a graph and 219 MB
with one; most of it is the pack's mapped pages, which git's own walk touches too. git's
`rev-list --date-order --max-count=64` peaked at 316 MiB resident on the bench (`ru_maxrss` of
the child), and GT §3.2 measured `git log --all --date-order` at 522 MiB. The walk's own state,
56 MiB, is far from the 522 MiB the plan's stopping rule names.

## What deciding a stash's base costs

`measures_what_deciding_a_stashs_base_costs`, from `HEAD` alone, a commit standing in for a
stash (its date deciding how soon the walk meets it; the walk reaches each stand-in itself, so
it is dropped, and only the walk down to its base's generation is measured), median of seven
to the 64th row:

| Stash | Copy with a commit-graph | Commits walked by row 64 | Bench, no graph |
| --- | --- | --- | --- |
| none | 1.5 ms | 3,637 | 1,168 ms (340,228) |
| dated `HEAD~1`, base `HEAD~1000` | 6.8 ms | 19,266 | 1,169 ms |
| dated `HEAD~1`, base `HEAD~20000` | 57 ms | 187,783 | 1,168 ms |
| dated `HEAD~1`, base the root | 116 ms | 340,228 | 1,164 ms |
| dated `HEAD~5000`, base `HEAD~20000` | 1.2 ms | 3,637 | 1,172 ms |
| dated `HEAD~5000`, base the root | 1.2 ms | 3,637 | 1,169 ms |

Every row opened having walked 1 commit with the graph. A stash the first page meets — dated
among its rows — is decided there, at the cost of walking down to its base's generation, which
is git's own cost for the stash given as a tip (`init_topo_walk` lowers `min_generation` to it).
A stash dated below the first page costs it nothing: it holds its place among the tips until the
walk reaches its date, or its base is about to be counted down. Without a graph every row costs
the same, since the opening pass reads everything anyway.

## What this record does not settle

- The history thread's opening, a page after the opening pass and the window's frames during
  an opening pass (R6.3, R6.5): measured once the walk has a thread of its own (a find to the
  oldest commit is measured below).
- Why git's first page with a graph is 13 ms faster per 283k commits than Cairn's: not profiled
  (no profiler on the machine); the hash map is the likely cause, not established.

## A find after the opening pass, with the walk as the history's order

Measured when the history's routes first walked in date order, before the history thread
existed. `cargo test --release -p cairn-app --bin cairn measures_a_find_through_the_boundary --
--ignored --nocapture` with `CAIRN_BENCH_REPO` naming the bench: the repository thread opens the
history from the refs snapshot (the first page of `PAGE_ROWS`, its opening pass included and
not timed), then finds the walk's last row — the oldest commit, `c01efc669f0`, row 345,544 in
both orders — in pages of `FIND_PAGE_ROWS`, every page appended to one `History` as the window
appends it; median of seven after a warm-up. The same reporter was run on the tree before the
change (the integration branch at `c05ece5`, the history then in commit-time order), from a
source copy built in the scratchpad.

| | Commit-time order (`c05ece5`) | Date order |
| --- | --- | --- |
| the find, after the first page | 3,829 ms [3,822-3,839] | 1,952 ms [1,933-2,017] |
| the whole walk as one page (warm-up, opening pass included) | 4,285 ms | 3,950 ms |
| a find stopped halfway | 1,914 ms, 158,784 rows kept | 976 ms, 160,832 rows kept |
| retained by the history at the end | 52.67 MiB | 52.62 MiB |

After its opening pass the date-order walk holds every commit's parents and date, so each
page reads only the commits it draws (`summary_of_commit`), where commit-time order's walk
read each commit to step and again to draw it. A marker was touched before each run and `find
~/Development/bench/rust/.git -newer <marker>` was empty after each, exit 0, nothing on stderr.
