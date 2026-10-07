# C11, C12, C15 and C16 as measured: refs, status, rows and the window on rust-lang/rust

Research for the `refs-and-status` packet, recorded as the packet tore down
(2026-10-07) from the measurements its phases took, which were logged in the packet's
`progress.md` (deleted at teardown; git history holds it). Criteria C11, C12, C15 and
C16 of `docs/prd/refs-and-status.md` name the bars; `deep-find-measured.md` beside this
file measured the rows as they were before the packet, and
`docs/research/diff-engine/measured-baseline.md` chose the machine and the repository.
This record is every number the phases took against those bars, so a later change can
be compared with them. Timing is not asserted in CI (flaky and bound to a machine, as
`history-graph`'s A7 was not); these are reports. Nothing here was re-measured at
teardown.

## Headlines

- **Every bar met, at every measurement.** C11: a clean status in 29.1 ms (bar 100), a
  status of 1,000 modified and 10,000 untracked files in 34.6 ms (66.7 ms when the
  untracked files sit in 1,000 new directories and git is asked twice; bar 250), the
  refs snapshot with every ahead/behind in 0.13 ms (bar 100), the first page from every
  ref in 7.27 ms (bar 200). C15: every row's derived edges the frozen assigner's over
  every ref of the bench, and a find of the oldest commit 2.2-2.3 s against a ceiling of
  2.64 s. C16: 52.67 MiB retained for all of rust-lang/rust from every ref, against
  64 MiB (1,427 MiB before the packet). C12: no frame over 16.7 ms of UI-thread work in
  any run.
- **Compact and slim rows made the deep find faster, not slower.** 2,495 ms with the
  old rows, 2,263 ms compact, 2,206 ms slim — a row no longer allocates its edges or
  its text.
- **Ahead/behind on a long divergence costs two to three times git's** (3.02 s against
  1.14 s at 169,679 ahead), the one number recorded beyond C11 that is worse than git's.
  It runs on the refresh thread and is cancellable, so it holds up nothing.
- **Paint is not measured.** `window_check` times the UI thread's work per frame
  (updates applied plus `sync_and_update`), headless; the look at the painted window is
  the user's.

## 1. Machine, repository, method

- **Machine:** AMD Ryzen 7 9800X3D (16 threads), 60 GiB, NVMe, Linux 7.2.8-2-cachyos —
  the machine of `docs/research/diff-engine/measured-baseline.md` section 1.
- **Repository:** the full clone of rust-lang/rust at `c999cef531e` in
  `~/Development/bench/rust` (340,228 commits from `HEAD`, 345,545 from every ref, 175
  refs, no commit-graph file, no stash), read only: each run checked its `.git` listing
  and, from phase 05 on, `find .git -newer <marker>` unchanged after it. Anything that
  needed a write (a dirty tree, a stash) ran on a scratch clone: on tmpfs for the status
  (phase 02), `--shared` for the stash look-ahead (phase 05, which freshened the bench
  pack's mtime through alternates, content unchanged), and `GIT_OPTIONAL_LOCKS=0 git
  clone --local --no-hardlinks` for the window (phase 09).
- **git:** 2.56.0, the application's.
- **Build:** release profile, warm: one run to warm up, then the median of seven, a fresh
  process per run where the row says so.
- **Reporters**, `#[ignore]`d and driven by `CAIRN_BENCH_REPO` (which they only read):
  - `measures_the_refs_snapshot_and_ahead_behind` (`crates/cairn-git/tests/refs.rs`) —
    C11's refs half.
  - `measures_the_status_read` (`crates/cairn-git/tests/status.rs`) — C11's status half.
  - `measures_the_first_page_from_every_ref` (`crates/cairn-git/tests/every_ref.rs`) —
    C11's first page.
  - `measures_compact_rows_over_a_named_repository` (`crates/cairn-git/tests/compact_rows.rs`),
    modes `find` (C15's time, C16's memory, asserting 64 MiB — `CAIRN_C16_MIB` may only
    lower it), `equivalence` (C15's edges against the frozen assigner in
    `crates/cairn-model/tests/layout_before_compaction/mod.rs`) and `derive` (the
    snapshot interval).
  - `measures_a_find_through_the_boundary` (`crates/cairn-app/src/worker/find_tests.rs`)
    — the deep find through the real worker, as the window appends it.
  - `measures_layout_over_unmerged_refs` (`crates/cairn-git/tests/every_ref.rs`) — layout
    and edge derivation as open lanes grow.
  - `window_check` (`crates/cairn-app/src/window_check.rs`; `cargo test -p cairn-app
    --release -- --ignored --nocapture window_check`) — C12: the real window over the
    real worker and engine, headless at 1440×900, frames paced as a 60 Hz display paces
    them, with `CAIRN_SCRATCH_REPO` naming a dirty scratch clone for the large status.

## 2. C11 — refs, status and the first page

Bars and the numbers that met them (medians of seven):

| C11 item | Bar | Measured | Date |
| --- | --- | --- | --- |
| status, clean bench | 100 ms | 29.1 ms [28.9–30.0] | 2026-10-06 |
| status, 1,000 modified + 10,000 untracked (scratch clone) | 250 ms | 34.6 ms [33.8–43.8], one read; 66.7 ms [64.5–78.0] with the untracked in 1,000 new directories (two reads) | 2026-10-06 |
| refs snapshot with every ahead/behind, bench | 100 ms | 0.13 ms (175 refs; one branch, level with its upstream) | 2026-10-06 |
| the same, 10,000-ref fixture | recorded | 28.3 ms (10,001 refs) | 2026-10-06 |
| first page from every ref | 200 ms | 7.27 ms [7.24–7.34] (8.56 ms with the snapshot's read) | 2026-10-06 |
| the same from `HEAD`, beside it | recorded | 7.69 ms [7.66–7.83] | 2026-10-06 |
| status with every file's stat changed | recorded, not barred | 758.9 ms [756.1–761.3] | 2026-10-06 |

### The refs snapshot

| Repository | Refs | Snapshot | Snapshot + every ahead/behind |
| --- | --- | --- | --- |
| rust-lang/rust at `c999cef531e` (`main` equal to `origin/main`) | 175 | 0.14 ms | 0.13 ms (one branch, no walk) |
| generated: 3,000 branches, 3,000 remote-tracking, 4,000 tags (1,000 annotated), packed, 500 loose | 10,001 | 28.4 ms | 28.3 ms (ten branches, equal) |

Beyond C11, a divergence built over the bench's real history (a branch at `HEAD` whose
upstream is `HEAD~n`), each count equal to `git rev-list --left-right --count`:

| Upstream | Ahead | Cairn | Commits read | git |
| --- | --- | --- | --- | --- |
| `HEAD~100` | 1,957 | 189 ms | 39,209 | 82-94 ms |
| `HEAD~1000` | 22,649 | 1.32 s | 290,337 | 547-622 ms |
| `HEAD~10000` | 169,679 | 3.02 s | 732,393 | 1.14 s |

gix's hidden frontier reads far more commits than the divergence, and no commit-graph is
used so that every read can be cancelled (the bench has none either).

### Status

Through `Repository::status`, beside the spike's own `git` (spawn and parse,
`status-agreement-spike.md`). The bench on btrfs, read only (its index and locks checked
unchanged); the clone on tmpfs.

| Scenario | Entries | Reads | Cairn | Spike's git |
| --- | --- | --- | --- | --- |
| bench, clean | 0 | 1 | 29.1 ms [28.9–30.0] | 28.8 ms (`-unormal`) |
| scratch clone, clean | 0 | 1 | 24.3 ms [23.8–25.6] | 27.2 ms (`-unormal`) |
| 1,000 modified + 10,000 untracked in 1,000 tracked directories | 10,980 | 1 | 34.6 ms [33.8–43.8] | 26.3 (1,000 modified) and 34.1 (10,000 untracked), measured apart |
| the same, untracked in 1,000 new directories (the second, `--untracked-files=all` read) | 11,000 | 2 | 66.7 ms [64.5–78.0] | — |
| every tracked file's stat changed | 0 | 1 | 758.9 ms [756.1–761.3] | 736.2 (`-uno`), 765.6 (`-unormal`) |
| that read superseded at 100 ms | — | 1 | answered cancelled at 100.9 ms; registry empty; logged cancelled | — |

20 of the 10,000 untracked files fall in ignored directories.

### The first page from every ref

64 rows, each run a fresh open:

| Seed | First page [min–max] | Rows laid out | Commits pulled |
| --- | ---: | ---: | ---: |
| `HEAD` | 7.69 ms [7.66–7.83] | 1,088 | 1,088 |
| every ref, snapshot already read | 7.27 ms [7.24–7.34] | 1,088 | 1,088 |
| every ref, with the snapshot's read | 8.56 ms [8.50–8.60] | 1,088 | 1,088 |

The stash look-ahead, on a `--shared` scratch clone of the bench (166 refs), every ref,
snapshot already read: no stash 7.40 ms; a stash on `HEAD` 7.40 ms (1,087 commits pulled,
the row at its date); plus a stash whose base is `HEAD~20000` (2017) 21.60 ms (4,096
pulled, the row held and drawn directly above its base); plus a stash on a deleted
branch's commit dated 2015 21.53 ms (4,096 pulled, no row). With the clone's own
commit-graph the worst case is 3.09 ms. The stream never holds more than 4,096 commits
ahead of the next row, so that is the most looking ahead adds to a page.

### Layout as open lanes grow (2026-10-07)

`measures_layout_over_unmerged_refs`: a 12,000-commit main line with N one-commit
branches forked at distinct commits down its upper half, each dated after main's tip, so
the walk meets every branch first and holds all N lines open — the widest the graph gets.

| Branches | Rows | Widest | First page (refs read included) | Whole walk | 40 rows' edges: top / under every line / middle |
| --- | --- | --- | --- | --- | --- |
| 100 | 12,100 | 100 | 3.12 ms | 36.3 ms | 0.012 / 0.029 / 0.018 ms |
| 1,000 | 13,000 | 1,000 | 8.37 ms | 61.8 ms | 0.012 / 0.112 / 0.025 ms |
| 5,000 | 17,000 | 5,000 | 25.2 ms | 208 ms | 0.013 / 0.485 / 0.211 ms |

Layout grows with the open lanes (the assigner scans them per row), on the history
thread, inside C11's 200 ms; deriving a viewport's edges stays under half a millisecond
at 5,000 lines crossing it. Drawing those lines is in `docs/systems/history-graph.md`
(`measures_drawing_a_viewport_over_many_open_lanes_and_a_crowded_rows_chips`).

## 3. C15 — compact rows: the same edges, the find no slower

**Equivalence.** Every row's derived edges equal to the frozen pre-compaction assigner's,
edge for edge and in order, over every row of the bench: from every ref 345,545 rows
(49,609 repainted, 627 late lines), from `HEAD` 340,228 (48,988 repainted, 624 late
lines) — at phase 03 (2026-10-06, while the old `edges` field still rode on every row and
was compared too), again at phase 04 after slimming, and at phase 05 for the seeds
`refs` and `snapshot` (the walk with labels and stash rows).

**The find of the oldest commit** (`c01efc669f0`, the walk's last row), paging the held
session 64 rows at a time and keeping every row, as the window does. Bar: no more than
10% over 2.4 s, so 2.64 s.

| When | Seed | Find, median | Runs (warm-up excluded) |
| --- | --- | ---: | --- |
| before the packet (`f34631c`, `deep-find-measured.md`'s reporter, same session as phase 03) | every ref | 2,495 ms | 2,486.7-2,508.7 |
| | `HEAD` | 2,453 ms | 2,430.4-2,461.6 |
| phase 03, compact rows (2026-10-06) | every ref | 2,263 ms (−9.3%) | 2,255.8-2,269.7 |
| | `HEAD` | 2,210 ms (−9.9%) | 2,204.7-2,227.1 |
| phase 04, slim rows (2026-10-06) | every ref | 2,206 ms | 2,198.6-2,220.9 |
| | `HEAD` | 2,169 ms | 2,157.3-2,178.1 |
| phase 05, from the refs snapshot (2026-10-06) | `snapshot` | 2,265 ms | 2,241-2,275 |
| | `refs` | 2,253 ms | 2,245-2,264 |
| | `HEAD` | 2,211 ms | 2,205-2,221 |
| phase 08, through the worker boundary (2026-10-07; `measures_a_find_through_the_boundary`, 512-row find pages) | every ref, oldest commit | 2,227-2,250 ms over three runs | 2,213-2,285 |
| | every ref, `refs/tags/release-0.1` (row 337,965) | 2,208 ms | 2,191-2,214 |

The boundary adds nothing measurable. A find stopped halfway (at 1.1 s, 155-160k rows
kept) sent no row after the stop, and the next page asked began at the row after the
last one kept. A press's lookup among the labelled rows of a 345,545-row history
(`History::labelled_position`) takes under a microsecond, where a whole-history id scan
took 4.75 ms on the UI thread.

**The snapshot interval** (mode `derive`, phase 03): every row exactly `K - 1` below its
snapshot over the whole bench history from every ref, mean 106 edges a row.

| K | Rows timed | Per row p50 | p99 | Max | Snapshots held |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 345,545 | 0.23 µs | 0.41 µs | 9.9 µs | 19.81 MiB |
| 8 | 43,193 | 0.29 µs | 0.65 µs | 15.1 µs | 2.48 MiB |
| 16 | 21,596 | 0.38 µs | 0.86 µs | 15.0 µs | 1.24 MiB |
| 32 | 10,798 | 0.55 µs | 1.34 µs | 13.7 µs | 0.62 MiB |
| **64** | 5,399 | **0.76 µs** | **2.0 µs** | **14.1 µs** | **0.31 MiB** |
| 128 | 2,699 | 1.26 µs | 3.4 µs | 14.8 µs | 0.15 MiB |
| 256 | 1,349 | 2.82 µs | 7.1 µs | 21.0 µs | 0.08 MiB |
| 1024 | 337 | 8.34 µs | 24.4 µs | 36.8 µs | 0.02 MiB |

K = 64 (`LaneAssigner::SNAPSHOT_EVERY`) was chosen from it: a 35-row viewport is at most
0.5 ms of derivation even at the worst row, and the snapshot falls on every page's first
row.

## 4. C16 — slim rows: 64 MiB for all of rust-lang/rust

Mode `find`, paging the whole history and appending every page to one `History`; the
retained figure counts capacity, every chunk whole.

| When | Seed | Rows | Retained | Per row | Breakdown | `RssAnon` growth |
| --- | --- | ---: | ---: | ---: | --- | ---: |
| before the packet (`f34631c`) | every ref | 345,545 | 1,427 MiB | — | 89% edges (`deep-find-measured.md`) | 1,446 MiB |
| | `HEAD` | 340,228 | 1,392 MiB | — | | 1,410 MiB |
| phase 03, compact rows | every ref | 345,545 | 167.9 MiB | — | row vector 108.0 (524,288 × 216 B, capacity), lane changes and snapshots 18.61, parents 14.3, text 27.0 | 185.7 MiB |
| | `HEAD` | 340,228 | 167.0 MiB | — | | 184.4 MiB |
| phase 04, slim rows | every ref | 345,545 | **52.61 MiB** | 159 B | rows 23.77, text 16.01, lane changes 12.25, snapshots 0.25, authors and index 0.32 (8,424 authors) | 83.1 MiB |
| | `HEAD` | 340,228 | 51.82 MiB | 159 B | rows 23.42, text 15.76, lane changes 12.07, snapshots 0.25, authors 0.32 (8,423) | 82.0 MiB |
| phase 04 QA, the author index estimated by its buckets (one run) | every ref | 345,545 | 52.66 MiB | — | authors 0.371 | — |
| phase 05, from the refs snapshot | `snapshot` | 345,545 | **52.67 MiB** | 159 B | rows 23.77, text 16.01, lane changes 12.25, snapshots 0.25, authors 0.371, labels 0.006, stashes 0.000 | 83.2 MiB |
| | `refs` | 345,545 | 52.66 MiB | — | | — |
| | `HEAD` | 340,228 | 51.87 MiB | — | | — |
| phase 08, through the worker boundary | every ref | 345,545 | 52.67 MiB | 159 B | | — |

On the phase 05 scratch clone with two drawn stashes: 52.67 MiB, stashes 0.003 MiB.
Phases 03 and 04 were measured 2026-10-06, phase 08 2026-10-07. A kept row is 72 bytes
(`a_kept_row_is_seventy_two_bytes`) and owns no heap allocation.

**What reading rows through the stores costs a frame** (phase 04, `window_check`, the
history scrolled 9 rows a frame, three runs each interleaved with phase 03's last commit):
medians 1.30 / 1.33 / 1.31 ms before and 1.34 / 1.37 / 1.38 ms after, maxima 1.95 / 2.14 /
1.90 and 1.86 / 1.86 / 2.18 ms; no frame over 16.7 ms either way. About 0.06 ms a frame more
at the median. Applying a page: 0.007-0.008 ms before, 0.008-0.009 ms after. The author
index's doubling rehashes every author so far: 0.48 ms at 57,000 authors. Compact rows
alone (phase 03): 1.31 ms median, 1.89 ms max, against 1.27 / 1.99 ms at `f34631c`.

## 5. C12 — the window while refs, sidebar and a large status land

`window_check`, 2026-10-07: the window opens as the application does (`Request::Refresh`,
the refs opening the history from every ref) and lands the decorated history, the
sidebar's rows, the refs and the status before its first phase ends. Frames are the UI
thread's work: every update applied in the frame plus its `sync_and_update`. The large
status is a scratch clone of the bench (10,750 unstaged and 500 staged paths, 11,000
distinct), the bench itself untouched (`find -newer` empty after the clone and every run).

Phase 09, two runs:

| Phase | Frames | Median | p99 | Max (run 1 / run 2) | Over 16.7 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| bench: opening — refs (175), first page (8 rows labelled), sidebar rows, status | 35 / 33 | 0.02 ms | 7.5 ms | 7.51 / 7.01 ms | 0 |
| bench: the history scrolled | 62 | 1.43 / 1.34 ms | 2.1 ms | 2.12 / 2.09 ms | 0 |
| bench: the diff subjects' phases (S1, M1, F1) | — | — | — | 15.76 (F1 Load Diff, run 1) / 7.52 ms | 0 |
| scratch clone: opening, its status among what lands | 37 | 0.02 ms | 8.3 ms | 6.70 / 8.25 ms | 0 |
| Local Changes shown: both lists and the first path's diff | 24 / 25 | 0.03 ms | 8.6 ms | 7.26 / 8.61 ms | 0 |
| Unstaged scrolled, 100 rows a frame | 122 | 0.68 / 0.75 ms | 1.3 ms | 1.34 / 1.47 ms | 0 |
| a filter typed, its rows answered on a worker | 13 | 0.02 ms | 1.0 ms | 1.02 / 0.94 ms | 0 |
| a refresh landed with the view shown | 28 | 0.02 ms | 1.2 ms | 1.15 / 0.60 ms | 0 |

Applying a status of 11,000 paths costs 0.004-0.014 ms (the lists arrive laid out; the
window moves an `Arc`). The status landed 77 ms after opening on the bench (clean) and about
110 ms on the scratch clone (two reads); the first path's diff 60-80 ms after the view was
shown. The 15.76 ms F1 Load Diff frame in run 1 had no update land in it, is unexplained,
and did not reproduce: three runs after the phase's QA gave 6.67 ms (a file diff applied in
it), 2.96 and 2.93 ms there (`c14-measured.md` recorded 1.12 / 3.20 ms).

Re-run after phase 09's QA (badges painted as shapes, the splitter between the lists), three
runs, a fresh scratch clone of 11,000 paths: every frame under 16.7 ms. Scratch: opening max
6.60 / 6.87 / 8.35 ms; Local Changes shown 6.72 / 6.92 / 6.90 ms; Unstaged scrolled 1.29 /
1.17 / 1.15 ms; a filter typed 0.72 / 0.84 / 1.17 ms; a refresh landed 0.73 / 0.81 / 0.60 ms.
Bench: opening 7.50 / 6.63 / 6.65 ms; the largest other frame the Commit tab's first draw,
6.44 / 6.32 / 6.23 ms.

`window_check` records these frames and asserts no 16.7 ms bar: a frame past it is read
from the numbers, not failed.
