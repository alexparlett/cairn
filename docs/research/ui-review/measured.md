# C22 as measured: the window's frames on rust-lang/rust after the Fork alignment

Research for the `fork-alignment` packet, recorded at its merge-bar review (2026-10-20).
Criterion C22 of `docs/prd/fork-alignment.md` (R12.2) names the bar: no frame of UI-thread
work over 16.7 ms on the bench repository while the history is scrolled, a commit is opened,
or the window switches between All Commits and Local Changes. The instrument is the
`#[ignore]`d window check (`crates/cairn-app/src/window_check.rs`: `window_check` and
`opening_pass_check`), the same reporter `docs/research/refs-and-status/measured.md` and
`docs/research/history-topo-order/measured.md` read, so the numbers compare with theirs.
Timing is not asserted in CI; these are reports.

## Headlines

- **The bar held in every gesture of every run but one frame.** Over nine runs (three of
  the whole check, six more of the opening pass alone) the slowest frame of any gesture was
  13.3 ms, except one frame of 19.3 ms in the first run's history scroll, which no later run
  repeated (5.8-6.1 ms in the other eight). The outlier is recorded below, not explained. The
  user's decision (2026-10-20): C22 is accepted as measured — the frame stays recorded as it
  is, and the criterion passes on the runs that repeated the gesture.
- **Local Changes' first frame grew.** Showing Local Changes over 11,000 paths costs
  12.6-13.3 ms in its slowest frame against 8.2 ms on `main`, the same tree-state and the
  same bench, in every run: under the bar, and the one consistent regression the packet
  made. Nothing in that frame scales with the paths (both lists are virtualized at 20 px, the
  primary found by a binary search); what the packet added to it is the bar over the lists,
  the headings' buttons, the commit box's controls and the badges, each a Lucide icon
  rasterized once per icon, size and tone the first time it is drawn (the responsiveness
  review's reading; not profiled).
- **The other gestures are where `main` left them**: the history scrolled nine rows a frame
  1.7-1.8 ms (main 1.6), a commit's Commit tab first drawn 6.6-6.9 ms (main 6.3), a change
  set asked and drawn while scrolling 2.4-2.7 ms (main 1.6), the opening pass's reopen with
  3,000 rows drawn 2.2-2.3 ms (main's record 2.7).
- **Paint is not measured.** The check times the UI thread's work per frame (updates applied
  plus `sync_and_update`), headless; the raster snapshot encoded to PNG is an upper bound
  (32-35 ms, as on `main`).

## 1. Machine, repository, method

- **Machine:** AMD Ryzen 7 9800X3D (16 threads), 60 GiB, NVMe, Linux 7.2.10-1-cachyos — the
  machine of `docs/research/diff-engine/measured-baseline.md` section 1.
- **Repository:** `~/Development/bench/rust`, the full clone of rust-lang/rust at
  `c999cef531e` (175 refs, no commit-graph), read only: `GIT_OPTIONAL_LOCKS=0` on every git
  run against it, and its `.git` mtime unchanged across the runs. The large status ran on a
  scratch clone (`GIT_OPTIONAL_LOCKS=0 git clone --local --no-hardlinks`, `maintenance.auto`
  off, `gc.auto 0`), dirtied with 1,000 modified `.rs` files and 10,000 untracked files in
  one new directory: 11,000 paths, all in Unstaged.
- **Build:** release profile, warm; the three whole-check runs were the first, second and
  third in the session.
- **Window:** headless at 1440×900, scale 1, frames paced as a 60 Hz display paces them.
- **Baseline:** `main` at `934d5e1` (the packet's base), built from a copy of that tree into
  its own target directory and run once against the same bench and scratch clone.

## 2. C22 — the three gestures

Slowest frame of the gesture, per run (ms of UI-thread work; the bar is 16.7):

| Gesture | Run 1 | Run 2 | Run 3 | `main` |
| --- | --- | --- | --- | --- |
| the history scrolled, 9 rows a frame, nothing loading | 1.82 | 1.70 | 1.68 | 1.59 |
| the history scrolled until 3,000 rows were loaded (pages arriving) | **19.28** | 5.87 | 6.10 | 3.99 |
| the Commit tab drawn the first time, over the newest commit | 6.87 | 6.63 | 6.58 | 6.28 |
| S1's change set asked the first time (`git diff-tree`) and drawn, the history scrolled every frame | 2.56 | 2.72 | 2.43 | 1.64 |
| Local Changes shown over 11,000 paths: both lists drawn, the first path's diff asked and drawn | 13.12 | 13.34 | 12.61 | 8.19 |
| Unstaged scrolled, 100 rows a frame | 0.87 | 1.06 | — | — |

The 3,000-row scroll alone, six more runs of `opening_pass_check` with nothing else running:
5.85, 5.75, 5.80 ms (runs 4-6; runs 2 and 3 above were 5.87 and 6.10). Run 1's slow frame
applied no update ("after applying []"), so it was the frame's own layout, with p99 at
12.97 ms in that run against 3.1-3.3 ms in every other; the run was the session's first.

Every other phase the check reports stayed under 7 ms in every run: the first open
3.2-4.1 ms (103 frames), a reopen with 3,000 rows drawn and scrolled 60 px a frame 2.2-2.3 ms
(the filter keystroke made during it drawn within a frame), each commit's files scrolled
100 rows a frame 1.2-2.0 ms, Expand All 0.9-1.2 ms, F1's refusal and Load Diff 6.3 and
3.0 ms, the loaded diff scrolled 1.5 ms, a filter typed in Local Changes 0.8 ms.

## 3. What did not run

- `window_check`'s last phase, a refresh landed while Local Changes is shown over the scratch
  clone, did not finish within the check's 60 s patience in any run — nor on `main`, with
  the same clone — so the check ended there (exit 101) after every gesture above had been
  reported. The phase waits on the refreshed lists' serial and the primary's diff asked
  against it; what holds it on this clone is not known and is filed from the review.
- `writes_check` (the staging packet's costs) was not run: no write verb changed.
