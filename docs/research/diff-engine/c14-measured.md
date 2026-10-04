# C14 as measured: Cairn's diff queries and window on rust-lang/rust

Research for the `diff-engine` packet, recorded as the packet tore down (2026-10-04)
from the measurements its phases took, which were logged in the packet's
`progress.md` (deleted at teardown; git history holds it). Criterion C14 of
`docs/prd/diff-engine.md` names the bar; `measured-baseline.md` beside this file
chose the subjects and measured **git's** own warm numbers for them. This record is
**Cairn's** numbers against that baseline, every time they were taken, so a later
change can be compared with them. Timing is not asserted in CI (flaky and bound to a
machine, as `history-graph`'s A7 was not); these are reports.

## Headlines

- **Every C14 engine bar met, at every measurement.** The changes query on S7, S1
  and M1 inside 100 / 500 / 500 ms; the content query on F7 inside 100 ms; F1
  answered too large without its content read; M1's rename pairs git's own, all
  2,774, compared pair by pair.
- **Since git answers the reads, Cairn costs git's time plus a little.** Once the
  changes query moved to `git diff-tree` (decision E) and the changed lines to `git
  diff-tree -p` (content parity), every figure but the refusal is worse than gix's
  was and close to git's baseline: S1 35.5 ms against git's 28.6, M1 83.2 against
  78.5, F7 17.7 against 9.8.
- **The window kept every frame under 16.7 ms** of UI-thread work while the heaviest
  subjects loaded, headless (`window_check`), across three runs in each of two
  measurements. The one frame near the bar is the Commit tab's first draw in a
  session, 12-15 ms, the toolkit's first text of each face; every draw after it is
  under 4 ms.
- **Paint is not measured.** The headless runner can only take a raster snapshot
  encoded to PNG (33-43 ms for the whole window, dominated by the encoding). The
  literal look at the window, by hand, as C14 words it, was the user's.

## 1. Machine, repository, method

- **Machine:** AMD Ryzen 7 9800X3D (16 threads), 60.4 GiB, NVMe — the machine of
  `measured-baseline.md` section 1.
- **Repository:** a full clone of rust-lang/rust at `c999cef531e` in
  `~/Development/bench/rust` (340,228 commits, 62,892 index entries), read only: no
  `gc`, `repack` or configuration write, or the numbers stop comparing.
- **git:** 2.56.0 for every Cairn number from the changes-query rework on; the
  baseline's own figures are git 2.55.0's.
- **Build:** release profile (`cargo test --release`), warm: one run to warm up, then
  the median of seven (five where marked).
- **Reporters**, `#[ignore]`d and driven by `CAIRN_BENCH_REPO`, which only read it:
  - `measures_the_diff_queries_against_a_named_repository`
    (`crates/cairn-git/tests/diff/bench.rs`) — the engine numbers, git's baseline
    printed beside each, and M1's rename pairs asserted against `git diff-tree -M`
    on the same commit (every pair, source, destination and score).
  - `measures_expand_all_budgets_against_a_named_repository` (same file) — Expand
    All's sweep over budgets.
  - `window_check` (`crates/cairn-app/src/window_check.rs`; `cargo test -p cairn-app
    --release -- --ignored --nocapture window_check`) — the real window, every
    component, over the real worker and engine, headless through `freya-testing` at
    1440×900, frames paced as a 60 Hz display paces them; each update applied with
    `session::apply` and timed (the UI thread's work per update), each frame's
    `sync_and_update` timed (events, components, layout, accessibility — everything
    but paint).
- **Subjects** (`measured-baseline.md` section 2): S7 `f0845adb0c1` (1,017 edited
  files), S1 `cf2dff2b1e3` (55,184 paths, 27,592 exact renames), M1 `5a3292f163d`
  (5,602 paths, a rollup with 2,774 renames by git's count), F7 `3b09522c34b`
  (`library/stdarch/crates/core_arch/src/arm_shared/neon/generated.rs`, 2.5 MB, the
  content subject), F1 `6a6e8446b97` (`library/stdarch/intrinsics_data/arm_intrinsics.json`,
  1.9 MB to 5.2 MB, the too-large subject).

## 2. The engine, as finally built (phase 08, git 2.56.0, medians of seven)

| Subject | Cairn | git (baseline) | Bar |
| --- | --- | --- | --- |
| S7 changes | 8.17 ms | 7.9 ms | 100 ms, met |
| S1 changes | 35.49 ms | 28.6 ms | 500 ms, met |
| M1 changes | 83.20 ms | 78.5 ms | 500 ms, met |
| M1 rename pairs | 2,774, every pair, source, destination and score `git diff-tree`'s | 2,774 | equal, met |
| F7 content, loaded | 17.73 ms | 9.8 ms | 100 ms, met |
| F1 refused as too large | 0.004 ms, content not read | — | answered without reading, met |
| F1 Load Diff | 185.96 ms in the engine; 215 ms answered in the window, `ShownDiff` prepared on the diff thread | 163.0 ms (Myers) | recorded |
| S7 Expand All, no budget (every file, page by page) | 109.07 ms, 1,017 files | — | — |

F7 is 2.5 MB, so the DEFAULT content query refuses it on R2.6's byte ceiling; the
100 ms bar is read as the time to answer it with its lines, the load-anyway path. The
refusal reads the object's header and never inflates it, which is pinned
deterministically by `the_size_ceiling_is_decided_before_the_content_is_read`.

## 3. The window (`window_check`)

Re-measured after phase 08's QA with each frame timed as its updates applied and its
`sync_and_update` together. Same machine, git and build; three runs; the second and third
agreed, the second's figures below. The first, straight after the release build, was
colder (S1's first ask drawn after 161 ms; one 7.96 ms frame as M1's kept answer was
drawn). No run had a frame over 16.7 ms.

| Phase | Answered and drawn | UI work per update (max) | Frames: median / max | Over 16.7 ms |
| --- | --- | --- | --- | --- |
| History opened, first page | 17.2 ms | 0.015 ms | 0.01 / 2.06 ms | 0 |
| History scrolled, nothing loading | — | 0.041 ms | 0.93 / 1.36 ms | 0 |
| The Commit tab drawn the first time in the session (newest commit) | 17.3 ms | 0.007 ms | 0.01 / **12.04 ms** | 0 |
| S1 changes, first ask (git runs), history scrolled every frame | 53.9 ms (arrived 46.1) | 0.004 ms | 0.97 / 1.70 ms | 0 |
| S1 changes, the kept answer | 18.0 ms (arrived 3.0) | 0.002 ms | 0.92 / 1.32 ms | 0 |
| S1's 27,592 files scrolled, 100 rows a frame | — | — | 0.61 / 1.22 ms | 0 |
| S1 Expand All, the tab scrolled every frame | 17.3 ms (first page 9.3) | 0.136 ms a page | 0.33 / 0.91 ms | 0 |
| S1 through its 464 opened files, 100 rows a frame | — | — | 0.59 / 0.80 ms | 0 |
| M1 changes, first ask | 88.9 ms (arrived 85.4) | 0.003 ms | 1.01 / 1.46 ms | 0 |
| M1 changes, the kept answer | 17.9 ms | 0.002 ms | 0.94 / 1.35 ms | 0 |
| M1's 2,828 files scrolled | — | — | 0.11 / 0.77 ms | 0 |
| M1 Expand All | 34.5 ms (first page 9.5) | 0.008 ms a page | 0.52 / 0.70 ms | 0 |
| M1 through its 18 opened files | — | — | 0.12 / 0.73 ms | 0 |
| F1 refused as too large | 51.7 ms | 0.003 ms | 0.02 / 0.90 ms | 0 |
| F1 Load Diff, history scrolled every frame | 216.8 ms (arrived 203.5) | 0.005 ms | 1.12 / 3.20 ms | 0 |
| F1's loaded diff scrolled, ~140 rows a frame | — | — | 0.55 / 1.07 ms | 0 |

Expand All held: S1 464 files, 27,128 left collapsed, resident memory 148.4 → 158.2 MiB;
M1 18 files, 2,810 left collapsed, 162.3 → 164.7 MiB. The paint snapshot: 32.8-34.4 ms.
The Commit tab's first draw in a session: 11.9-12.1 ms across the three runs.

The first measurement (phase 08, before that change; three runs agreed, one run's
figures): no frame over 16.7 ms; the Commit tab's first draw 14.63 ms (14-15 ms across
the runs; a headless probe measured 11.5 ms over 2,828 files and over 27,592 alike, so it
is the toolkit's first text of each face and fallback, not the subject); S1 changes first
ask 72.6 ms (arrived 55.7), M1 106.7 ms (arrived 100.1), F1 Load Diff 216.4 ms (arrived
214.7), every frame's maximum under 4 ms otherwise; Expand All held S1 151.5 → 161.6 MiB
and M1 167.3 → 169.9 MiB; the paint snapshot 40-43 ms.

## 4. Expand All's budget (Q2), `measures_expand_all_budgets_against_a_named_repository`

Release build, warm, medians of five: ms to the first page / to the end, files opened,
lines spent. 50,000 (`EXPAND_ALL_LINES`) was chosen from it — R2.6's line ceiling for one
file drawn without Load Diff, read to its end in at most 23 ms warm on every subject.

| Subject | 10,000 | 25,000 | 50,000 | 100,000 | 200,000 |
| --- | --- | --- | --- | --- | --- |
| S1 `cf2dff2b1e3` (27,592 files) | 0.4 / 0.5, 34 files | 1.1 / 1.7, 191 | 1.1 / 3.7, 464 | 1.1 / 6.9, 691 | 1.1 / 16.8, 2,429 |
| M1 `5a3292f163d` (2,828 files) | 4.1 / 4.2, 4 | 5.7 / 11.2, 10 | 5.9 / 18.3, 18 (53,488 lines) | 5.9 / 86.2, 616 | 8.1 / 250.6, 1,986 |
| S7 `f0845adb0c1` (1,017 files) | 4.5 / 4.6, 12 | 7.8 / 11.5, 59 | 7.5 / 22.9, 167 | 7.8 / 57.8, 573 | 8.7 / 109.1, 1,017 (all) |
| F7 `3b09522c34b` (4 files) | 5.2 / 5.5, 3 | 5.0 / 5.3, 3 | 5.0 / 10.4, 4 (all) | 5.2 / 10.8, 4 | 5.2 / 10.6, 4 |
| F1 `6a6e8446b97` (1 file, refused on its header) | 0.0, 1 file, 1 line | the same | the same | the same | the same |

No file failed in any run. Overshoot is one file at most: M1's 50,000 budget spent
53,488. A page's own bounds, `PAGE_FILES` (256) and `PAGE_LINES` (20,000), were set
beside this sweep and the window check; a change to either is a measured decision,
pinned by `a_pages_bounds_are_the_measured_ones`.

## 5. How the numbers moved, measurement by measurement

| When | S7 changes | S1 changes | M1 changes | F7 loaded | F1 Load Diff | Why it moved |
| --- | --- | --- | --- | --- | --- | --- |
| Phase 02 (gix answers everything; medians of five) | 1.631 ms | 20.658 ms | 2.845 ms | 8.727 ms | 127.190 ms | — |
| Changes query on `git diff-tree` (decision E) | 8.205 ms | 35.816 ms | 83.476 ms | 9.024 ms | 128.4 ms | git's rename and copy search, which gix's disagreed with (231 pairs on M1 where git has 2,774) |
| Changed lines from `git diff-tree -p` (content parity) | 7.808 ms | 33.611 ms | 81.494 ms | 17.513 ms | 186.859 ms | git's line diff, which gix's disagreed with; the content query now costs git's diff plus the gix read Cairn keeps |
| Phase 08, as finally built (section 2) | 8.17 ms | 35.49 ms | 83.20 ms | 17.73 ms | 185.96 ms | — |

At phase 02 the same three changes queries on a session already open took 1.059,
20.741 and 2.377 ms, so building gix's resource cache cost about 0.5 ms. At the content
parity measurement Expand All over S7 (1,017 text files, one `diff-tree -p` over the
commit) took 61.090 ms, before phase 08 made it page by page.

Two further measurements behind as-built rules, release build: dropping a 55,184-file
change set took 1.22-1.46 ms built on the same thread and 1.27-1.99 ms built on another
(seven runs each), which is why a replaced answer is freed on a worker
(`Request::Retire`); and preparing F1's loaded diff (`ShownDiff::new`, 334,688 unified
rows) took 2.4 ms on the diff thread, 20 ms for a 52 MiB file of a million changed lines.
