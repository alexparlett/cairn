# C21 as measured: Cairn's writes, frames and Show Lost Commits on rust-lang/rust

Research for the `staging-and-commit` packet, recorded as the packet tore down (2026-10-14)
from the measurements its merge bar took on the rebuilt packet, which were logged in the
packet's `progress.md` (deleted at teardown; git history holds it). Criterion C21 of
`docs/prd/staging-and-commit.md` names the bars, R13.1 the method and R13.3 the re-measure;
`measured-baseline.md` beside this file is git's own time for each verb plus one status read,
which the bars are written against (git's time plus one status read plus a flat 50 ms, the
user's decision of 2026-10-08). Timing is not asserted in CI (flaky and bound to a machine, as
`history-graph`'s A7 was not); these are reports, read rather than asserted. Nothing here was
re-measured at teardown.

## Headlines

- **Every bar met, in every run.** Press to the refreshed lists drawn, the median of three runs'
  medians of seven: stage 68.9 ms (bar 93), unstage 68.9 ms (bar 93), discard 51.5 ms (bar 76,
  from the confirmation's press), commit 52.5 ms (bar 87).
- **No frame over 16.7 ms of UI-thread work**, in any phase of any run: the slowest while the
  four writes landed 1.20-1.55 ms; while a hook ran, 4.30-4.93 ms at worst (a flood of 20,000
  flushed lines with the Activity Manager open).
- **Cairn's writes cost git's time plus one status read plus 15-26 ms** of its own
  (stage 68.9 against git's 42.9 ms with its status read; discard 51.5 against 25.8; commit 52.5
  against 37.4) — the engine's share is the verb itself (`cairn_write_costs`: stage 29.6-29.9 ms,
  discard 13.6-13.7 of which 13.3-13.4 is its consequence, commit 11.5-11.9), the rest the
  refresh's status read and the frames to draw it.
- **The commit's median is bimodal by one status read** (about 17 ms): one run in three, and
  in earlier runs on the same host other verbs too, lands a status read higher. Every such run
  stayed under its bar except one run's commit during the rebuild, 87.2 ms against 87 ms, which the merge
  bar's three clean runs did not repeat.
- **Show Lost Commits' first page draws 17.5-18.1 ms after the toggle**; the engine answers it
  in 7.4 ms, the same as the page without lost commits. An amend's press-time read
  (`ops::amend_consequence`) costs 0.55-0.58 ms.

## 1. Machine, repository, method

- **Machine:** AMD Ryzen 7 9800X3D (16 threads), 60 GiB, Linux 7.2.8-2-cachyos, `/tmp` on
  tmpfs, git 2.56.0 — the machine of `docs/research/diff-engine/measured-baseline.md` section 1.
- **Repository:** for each run, a fresh `GIT_OPTIONAL_LOCKS=0 git clone --no-local
  --no-hardlinks --no-checkout` of `~/Development/bench/rust` onto tmpfs, `maintenance.auto=false`
  and `gc.auto=0` set in the clone (so a commit's auto-maintenance cannot write a commit-graph
  mid-run), detached at `c999cef531e`, no commit-graph, no alternates, clean, deleted after. The
  bench itself was never written: its `.git` mtime read 2026-10-08 20:35:34.129917376 +0100
  before and after.
- **Build and order, per run:** release, warm. First `measures_the_first_page_with_show_lost_commits`
  (`crates/cairn-git/tests/lost_commits.rs`, read only), then `window_check::writes_check`
  (`crates/cairn-app/src/window_check.rs`, `--ignored`), then `cairn_write_costs`
  (`crates/cairn-git/tests/diff/write_baseline.rs`). Each is an `#[ignore]`d reporter driven by
  `CAIRN_BENCH_REPO` and `CAIRN_SCRATCH_REPO`; its doc comment has the command line.
- **What a write's time is:** from the press (the confirmation's press, for a discard) until the
  refreshed Local Changes lists are drawn, the median of seven after one to warm. A frame's time
  is UI-thread work per frame.
- **Measured at** the packet's merge bar, commit `e246478` on `feature/staging-and-commit`.

## 2. C21 at the merge bar

| Measure | Bar | Run 1 | Run 2 | Run 3 | Median of runs | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| Stage a hunk, press to refreshed lists (median of 7) | 93 ms | 69.1 | 68.6 | 68.9 | 68.9 | pass |
| Unstage a hunk | 93 ms | 68.9 | 68.6 | 69.3 | 68.9 | pass |
| Discard a hunk (from the confirmation's press) | 76 ms | 51.5 | 51.3 | 51.6 | 51.5 | pass |
| Commit | 87 ms | 52.5 | 52.4 | 69.1 | 52.5 | pass |
| Slowest frame while the four writes landed | 16.7 ms | 1.35 | 1.55 | 1.20 | 1.35 | pass |
| Slowest frame while any hook ran (worst of closed/open) | 16.7 ms | 4.93 | 4.75 | 4.30 | 4.75 | pass |
| Frames over 16.7 ms, any phase | 0 | 0 | 0 | 0 | 0 | pass |
| Show Lost Commits' first page, toggle to drawn (window) | recorded | 17.5 | 17.5 | 18.1 | 17.5 | recorded |
| Show Lost Commits' first page, engine (`lost=true`) | recorded | 7.39 | 7.40 | 7.40 | 7.40 | recorded |
| Amend's press-time read, `ops::amend_consequence` | recorded | 0.58 | 0.55 | 0.58 | 0.58 | recorded |

Run 3's commit, 69.1 ms, is the one-status-read-higher level (section 4).

### Per run, in full

- **Run 1** (clone 33 s). Slowest frame while a hook ran, the Activity Manager closed / open: a
  line every 50 ms for 2 s 1.21 / 1.43; 20,000 lines, each flushed 3.29 / 4.93; five lines of
  200 KiB 2.96 / 1.35. The staging gesture hovered over a 10,000-line diff, slowest frame
  0.27 ms; dragged down it, 0.95 ms. 1,000 files selected, the primary's diff drawn 20.4 ms from
  the selection (slowest frame 0.72 ms). Open 260.9 ms (slowest frame 6.65 ms; the first run's
  cold page cache). Show Lost Commits' slowest window frame 0.86 ms. Engine: the first page
  `lost=false` 7.39 ms [7.35-7.69], `lost=true` 7.39 ms [7.35-7.44], 64 rows; the whole walk
  2,277 / 2,340 ms. `cairn_write_costs`: stage lines 29.92 ms, unstage lines 30.14, discard lines
  13.74 (its consequence 13.37), commit 11.46; amend's press-time read 0.58 ms (`amend_staged`
  2.52 ms; each status read while Amend is ticked 3.10 ms).
- **Run 2.** Hooks 1.05 / 1.51, 3.27 / 4.75, 2.64 / 1.34; hover 0.41, drag 1.04; 1,000 files
  37.3 ms (slowest frame 0.53); open 43.8 ms (6.92); Show Lost Commits' frame 0.78. Engine: first
  page 7.40 / 7.40 ms; whole walk 2,282 / 2,266 ms. Costs: stage 29.81, unstage 29.80, discard
  13.71 (13.31), commit 11.85; amend's read 0.55 (2.33; 2.87).
- **Run 3.** Hooks 1.20 / 1.62, 3.46 / 4.30, 2.61 / 1.41; hover 0.44, drag 1.07; 1,000 files
  38.3 ms (0.80); open 43.8 ms (6.74); Show Lost Commits' frame 1.42. Engine: first page
  7.38 / 7.40 ms; whole walk 2,277 / 2,320 ms. Costs: stage 29.64, unstage 29.51, discard 13.64
  (13.42), commit 11.73; amend's read 0.58 (2.61; 3.18).

## 3. Against git's own time

| Verb | git + one status read (`measured-baseline.md`, highest) | Cairn, median | Cairn − git | Bar |
| --- | --- | --- | --- | --- |
| Stage | 42.9 ms | 68.9 ms | 26.0 ms | 93 ms |
| Unstage | 42.8 ms | 68.9 ms | 26.1 ms | 93 ms |
| Discard | 25.8 ms | 51.5 ms | 25.7 ms | 76 ms |
| Commit | 37.4 ms | 52.5 ms | 15.1 ms | 87 ms |

The engine's own verb is git's plus about 12 ms to stage or unstage (29.6-30.1 ms against git's
17.7 ms: the stale check and the patch built and emitted before `git apply --cached`), and a
discard's 13.6-13.7 ms is almost all its consequence, computed before the dialog and re-checked
before it runs (git's discard is 0.7 ms).

## 4. Earlier measurements, on the same machine and method

Kept so a later change can be compared with the packet's whole run, not its last point. Press to
the refreshed lists drawn, median of seven, three runs each, stage / unstage / discard / commit:

- **Before the redesign, as the first build finished** (phases 01-11's activity popover and
  measured bar): stage 85.9 / 69.1 / 68.7 ms, unstage 85.5 / 68.8 / 68.6, discard 68.0 / 51.3 /
  51.4, commit 69.1 / 52.6 / 52.4 — the first run a frame slower throughout. Engine share:
  `stage_lines` 30.0 ms, `unstage_lines` 30.2, `discard_lines` 13.7 (consequence 13.4), commit
  9.2-11.4; git's own in the same run 17.7 / 17.7 / 0.8 / 12.1 ms and a status read 33.8 ms.
  1,000 files selected and drawn together (a view since removed) in 28.5 ms, slowest frame
  12.0 ms; Show Lost Commits' first page 18.2 ms after the toggle.
- **The first merge bar, before the redesign**: stage 68.9 / 69.0 / 69.5, unstage 69.0 / 68.8 /
  68.9, discard 51.3 / 51.4 / 51.4, commit 52.3 / 69.4 / 69.3. Hooks' slowest frames, popover
  closed / open: 50 ms lines 0.99-1.28 / 1.79-2.10; the 20,000-line flood 4.02-6.06 / 4.96-5.26;
  200 KiB lines 4.29-4.85 / 1.64-2.21. (Its first attempt died of SIGBUS while putting the clone
  back — the check rewrote the index in place while a worker had it mapped; `writes_check` now
  replaces each file as git does, a sibling renamed over it.)
- **During the rebuild, after the Activity Manager**: stage 86.0 / 69.4 / 86.1, unstage 86.1 /
  85.4 / 86.1, discard 68.7 / 51.7 / 68.8, commit 87.2 / 69.6 / 70.4 — run 1's commit 0.2 ms over
  its bar. The same check built before that change, on the same host, gave 85.5 / 85.4 / 68.5 /
  69.7 and then 135.8 / 120.9 / 118.4 / 119.7: the host's variance, not the change's. Hooks'
  slowest frames closed / open: 50 ms lines 1.02-1.95 / 1.38-1.57; the flood 4.19-4.76 /
  6.06-6.61 (8,936 closed and 10,420 open wakes applied for 20,000 lines); 200 KiB lines
  3.15-3.87 / 1.68-1.87. Show Lost Commits' first page 17.7-18.5 ms after the toggle.

The bimodality is one status read, about 17 ms, landing in or out of the measured window; it is
the host's, and the merge bar's three runs were the cleanest of the packet.

## 5. What is not measured

- A staged rename's stage, unstage and discard (the user's answer of 2026-10-10: C21 gains no
  such row; filed as an issue).
- A hook that writes faster than one pipe read per line, or for longer than the three hooks
  above.
- Any repository but rust-lang/rust at `c999cef531e`, any machine but this one, and any git but
  2.56.0.
