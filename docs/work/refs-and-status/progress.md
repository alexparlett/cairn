# Progress — refs-and-status

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

## 2026-10-06 — phase 03 QA

Fresh reviewers (`qa-checklist` READY, `responsiveness-reviewer` with no findings,
`test-coverage-auditor`) and a fresh `qa-confirm`, run by the coordinator.

Confirmed findings fixed:

- TC1: `a_rows_heap_bytes_count_what_its_snapshot_holds` — a row carrying a snapshot of
  three words and a late line, its exact heap bytes pinned; fails with the snapshot's own
  `heap_bytes` multiplied by zero in `GraphRow::heap_bytes`, which every earlier test (empty
  snapshots only) let through.
- TC2: `the_reference_assigner_is_the_one_compact_rows_were_checked_against` — an FNV-1a
  fingerprint of C15's reference below its module documentation, taken while it was still
  `f34631c`'s verbatim (no change to it since `f94fbdf`); fails on a one-character edit to
  its code, and not on an edit to its documentation. No dependency added.
- TC3 (reporter half): `measures_layout_over_every_ref_of_a_named_repository` counts rows
  whose edges could not be derived, prints the count and fails on any; fails with the
  reporter asking for the row below.
- QC-A3: the systems doc's twins table names the cold-page and resumed-session test; its
  opening paragraph is re-wrapped.
- RR1: deferred to phase 04 (`state.md`): `RowRender` clones the whole row per built row.

Dismissed, with reasons:

- QC-A2: the `u32` saturation in `connect_upward` cannot be reached — both rows of a late
  line are in the window's `VecDeque`, production never calls `with_window`, and the
  default window is 1024 rows.
- RR2: deriving O(viewport × K) per frame is the design, measured at about 0.5 ms a frame
  at worst.
- RR3: the 100,000-row viewport twin counts rows built, by design; derivation at real
  snapshot spacing, scrolled deep and back, is pinned by
  `rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew`.
- TC2's `EdgeSegment`-constructor half: the constructors have their own unit tests, and the
  literal-edge layout tests are independent of the oracle.
- TC3's UI-fallback half: the node-alone fallback in `build_row` is commented, required by
  the no-panic rule, and its precondition (a snapshot within reach on every row the
  assigner lays out and a reader keeps) is pinned by the model's tests.

## 2026-10-06 — phase 03: compact rows

A row keeps its id, lane and only the lane changes at it (`LaneChange`: a line ends at
its commit, starts from it, or starts late — out of order — to a child laid out below);
every 64th row, and the first row a reader keeps, carries a `LaneSnapshot` of the lines
crossing into it; `cairn_model::row_edges` derives a drawn row's edges from the nearest
snapshot above it, advanced through at most 63 rows. Packet mode, on
`feature/refs-and-status`. No stopping rule fired: no row's derived edges differ from
the old assigner's anywhere they were compared.

**C15's oracle and how it was written first.** `crates/cairn-model/tests/layout_before_compaction/mod.rs`
is `f34631c`'s lane assigner verbatim (two names changed, checked by a `diff`), kept as
test code. Commit `f94fbdf` added the compact form while the old `edges` field still
rode on every row, and its equivalence test compared each row's derived edges with that
field and with the frozen copy; `7c23805` ran the same over the Cairn checkout and, in the
`#[ignore]`d reporter, over every row of the bench repository from every ref (345,545
rows, 49,609 of them repainted, 627 late lines) and from `HEAD` (340,228 rows, 48,988
repainted) — every row equal to the old storage and to the frozen copy, edge for edge and
in order. `097af10` then removed the field; the tests keep comparing against the frozen
copy. The assigner still keeps every edge for the rows inside its window (a repaint's
lane is chosen from them), so its layout is today's; only what it hands out changed.

**Where derivation runs, and K: at draw time, K = 64.** `HistoryList` derives each row
it builds (`row_edges` in `build_row`), from at most `K - 1` rows above it. Measured
with the reporter's `derive` mode (`measures_compact_rows_over_a_named_repository`,
release build, warm, one process, every row exactly `K - 1` below its snapshot over the
whole bench history from every ref; mean 106 edges a row):

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

A 1440×900 window shows 35 rows at 26 px; even with every one at the worst row timed
(14.1 µs) a frame's derivation is 0.5 ms of the 16.7 ms budget, and 0.07 ms at the p99.
K = 64 keeps snapshots at 0.31 MiB, which leaves C16's 64 MiB to phase 04, and matches
the application's page (`PAGE_ROWS`), so every page from the first starts on a snapshot.
Deriving on the worker per viewport would add a round trip to every scroll for work that
costs microseconds on the UI thread; not done. The real window agrees: `window_check`
(release, one run each, same session) scrolled the history 9 rows a frame in 1.31 ms
median, 1.89 ms max with compact rows, against 1.27 / 1.99 ms at `f34631c`.

**C15 numbers** — release build, warm, median of seven after a warm-up, fresh process per
run, interleaved with the old rows' runs; `~/Development/bench/rust` at `c999cef531e`
(read only: its `.git` listing identical before and after, nothing newer than a marker
file, `git status` clean); AMD Ryzen 7 9800X3D, 60 GiB, Linux 7.2.8-2-cachyos. A find of
the oldest commit (`c01efc669f0`, the last row) pages the held session 64 rows at a time
keeping every row, as the window does. "Before" is `deep-find-measured.md`'s appendix
reporter built at `f34631c` and run in the same session:

| Seed | Rows | Before: find | After: find | Change | Before: retained (capacity) | After: retained (capacity) | `RssAnon` growth before / after |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| every ref | 345,545 | 2,495 ms | 2,263 ms | −9.3% | 1,427 MiB | 167.9 MiB | 1,446 / 185.7 MiB |
| `HEAD` | 340,228 | 2,453 ms | 2,210 ms | −9.9% | 1,392 MiB | 167.0 MiB | 1,410 / 184.4 MiB |

Runs (ms, warm-up excluded): every ref before 2486.7-2508.7, after 2255.8-2269.7; `HEAD`
before 2430.4-2461.6, after 2204.7-2227.1. C15's bar — no more than 10% slower than
2.4 s — holds; it is faster, since a row no longer allocates its edges. The retained
figure counts capacity: the row vector's capacity (524,288 rows × 216 B = 108.0 MiB,
its growth slack included), lane changes and snapshots 18.61 MiB (5,400 snapshots),
parents 14.3 MiB, text 27.0 MiB. That is the figure phase 04 starts from (the plan
expected about 160 MiB). A `LaneChange` is 24 B; nothing shrinks it in this phase.

Pinned (mutation each fails, run by hand): `every_row_draws_the_edges_the_assigner_retained_before_compaction`
(crafted fixtures and 1,590 generated histories, windows 1-1024, K 1-64; fails with ended
late lines never retired, ending lanes never closed, the late-line merge reversed,
late lines ranked by end row only, snapshots without late lines, snapshots taken after the
row, a late span one short, ending lanes also drawn passing, and late ranks not recorded),
`two_late_lines_cross_a_row_in_their_childs_parent_order`,
`a_snapshot_falls_every_k_rows_and_on_the_first_row_kept` (fails with no snapshot on the
first kept row), `rows_kept_from_any_row_on_draw_without_the_rows_above_them`,
`a_row_with_no_snapshot_within_reach_draws_nothing_rather_than_guessing`,
`a_row_further_below_its_snapshot_than_any_interval_draws_nothing`,
`the_lanes_kept_rows_name_are_the_lanes_their_edges_reach` (fails with a snapshot's lanes
uncounted), `the_cairn_checkouts_rows_draw_what_the_assigner_drew_before_compaction` and
`each_cold_page_and_resumed_session_draws_on_its_own_what_the_assigner_drew_for_it`
(fails with `drawn_from` dropped from either the cold page or the session), and in the
list `rows_scrolled_away_and_back_draw_the_edges_the_assigner_drew` (fails deriving from
the row above). The viewport twin, `only_a_viewport_of_rows_is_built_however_long_the_history`,
still holds.

## 2026-10-06 — phase 02 QA

Fresh reviewers (`qa-checklist`, `test-coverage-auditor`, `destructive-ops-reviewer`,
`gate-integrity-reviewer`) and a fresh `qa-confirm`, run by the coordinator.

**The user accepted the two-read scheme for `status.showUntrackedFiles` (2026-10-06):** a
first `git status` with no `--untracked-files`, a second with `--untracked-files=all` only
where the first collapsed an untracked directory. Now a locked decision (`state.md`).

Confirmed findings fixed:

- DO1: `in_a_partial_clone_a_status_read_fails_rather_than_fetching` — a blob-less clone
  whose sparse checkout left a blob with the promisor, and a staged inexact rename of it:
  on git 2.44+ `Error::GitFailed`, nothing listed, no pack written; below 2.44 the read
  fetches and answers, pinned as the floor's residual. Fails with the read's
  `GIT_NO_LAZY_FETCH` set to `0`. The docs (`reads/status.rs`, `docs/systems/status.md`)
  say the whole read fails; the residual is in PRD R3.8 and the root `CLAUDE.md`. No new
  model variant, retry or fetch (the read policy of 2026-10-02).
- DO2: `a_status_read_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor` — an
  untracked cache, an fsmonitor token and a split index, each one a locked status
  rewrites, and a caching textconv, an external diff, a driver `command`, a smudge filter
  and a `post-index-change` hook: the git directory byte-identical (the shared index's
  mtime excepted, as documented), only the clean filter and the hook's token answer ran.
  Fails with the read's `GIT_OPTIONAL_LOCKS` set to `1`. Found building it: git 2.30.9 and
  2.32.7 merge a split index when a locked status under `core.fsmonitor` rewrites the index.
  Named in `CLAUDE.md`'s pin list and `docs/systems/status.md`.
- TC1: `a_status_read_superseded_during_its_second_read_ends_it` — fails (after the
  stub's 30 s) when the second read is given a cancel that never fires.
- TC2: rename and unmerged records with submodule fields; fails with the field dropped
  from either; the integration tests compare a conflicted entry's submodule state too.
- TC3: the index walk panics on an unreadable directory, and each read requires the
  superproject's index and each named submodule's among those compared (fails with the
  walk skipping `modules/`, or matching no index).
- QC-F2: the docs say a setting changed between the two reads is not seen.
- QC-D1/GI1, QC-D2: "real-git diff and status tests" in `CLAUDE.md`, `docs/qa-gate.md`,
  `scripts/gate.sh` and `scripts/git-floor.sh`; the banner reflowed; floors raised.
- QC-F4: commit 3b56314 (`fix(app)`) belongs to phase 02 though it touches `cairn-app`:
  `an_identical_ask_is_answered_from_what_is_kept` took the newest commit's first
  modified file, which phase 02's docs commit made `CLAUDE.md` — past the size limits, so
  answered without a patch read — and the test failed on the checkout's history, not on
  the worker. It now searches for a file whose diff asks git, on a handle of its own.

Dismissed, with the adjudicator's reasons:

- QC-F3 (and the destructive-ops note on it): a nested repository makes every status read
  twice — the documented, measured cost of the scheme the user accepted; phase 06 may
  weigh it.

For the user's end batch: what a failed status read does to the view (`state.md`).

## 2026-10-06 — phase 02: the status engine

`Repository::status` — `git status --porcelain=v2 -z` as a read, parsed into
`cairn_model::WorkingTreeStatus` (`docs/systems/status.md`). Packet mode, on
`feature/refs-and-status`. No stopping rule triggered.

**`status.showUntrackedFiles`, read by git itself (decided here; for the user's
review).** The first read passes no `--untracked-files`, so git reads the setting; only
where its answer collapsed an untracked directory (a `?` record ending `/`) is the status
read again with `--untracked-files=all`, whose answer is the answer. No `git config` read,
so no porcelain read beyond `status` was added and the stopping rule did not apply. Why
not gix's reading: besides the `includeIf` and system-file divergences behind
`reads::fetch_settings`, the value's language differs by git version — git 2.56 takes
`false`/`true`, 2.30.9 dies on both as a bad config (reproduced). Cost: a second read
wherever an untracked directory or a nested repository exists (C11 below: 66.7 ms
against 34.6 ms for one read).

**Found while building:**

- The sparse index (`sdir`) is read by git from **2.32.0**, not 2.31: `read-cache.c` at
  v2.31.0 has no `CACHE_EXT_SPARSE_DIRECTORIES` (checked in git's source). R3.7's state is
  a failed read on a git before 2.32 over an index gix reads as sparse.
- `git diff --name-status` is not an oracle for submodules: on 2.32.7 and 2.56.0 it leaves
  out a submodule whose only change is untracked content, which status reports as `S..U`
  (2.30.9's `git diff` includes it). The submodule oracle is the submodule's own state.
- An oracle `git diff -C` under `diff.renames=copies` is "find copies harder"; the oracle
  runs with `-c diff.renames=false` so its flag alone decides.
- git does not verify the index checksum on a read, so an index cut short can be read as
  garbage (a record with an empty path, which the parser refuses) rather than failing; the
  failing-read test breaks the `DIRC` signature instead.
- Porcelain v2 prints no `#` header for `status.branch` or `status.showStash` set in
  configuration (2.30.9, 2.32.7, 2.56.0); the parser skips a header anyway.

**C11, status half** — release build, warm, median of seven [min–max], through
`Repository::status` with the application's `git` (2.56.0), reporter
`measures_the_status_read`; same machine as phase 01. The bench on btrfs, read only (its
index and locks checked unchanged); the clone on tmpfs. Beside the spike's `git` (spawn +
parse, `status-agreement-spike.md`):

| Scenario | Entries | Reads | Cairn | Spike's git |
| --- | --- | --- | --- | --- |
| bench, clean (bar 100 ms) | 0 | 1 | 29.1 ms [28.9–30.0] | 28.8 ms (`-unormal`) |
| scratch clone, clean | 0 | 1 | 24.3 ms [23.8–25.6] | 27.2 ms (`-unormal`) |
| 1,000 modified + 10,000 untracked in 1,000 tracked directories (bar 250 ms) | 10,980 | 1 | 34.6 ms [33.8–43.8] | 26.3 (1,000 modified) and 34.1 (10,000 untracked), measured apart |
| the same, untracked in 1,000 new directories (`-uall` second read; new data) | 11,000 | 2 | 66.7 ms [64.5–78.0] | — |
| every tracked file's stat changed (recorded, not barred) | 0 | 1 | 758.9 ms [756.1–761.3] | 736.2 (`-uno`), 765.6 (`-unormal`) |
| that read superseded at 100 ms | — | 1 | answered cancelled at 100.9 ms; registry empty; logged cancelled | — |

Both bars met. 20 of the 10,000 untracked files fall in ignored directories.

## 2026-10-06 — phase 01 QA

Four fresh reviewers (`qa-checklist`, `test-coverage-auditor`,
`destructive-ops-reviewer`, `gate-integrity-reviewer`); a fresh `qa-confirm`
adjudicated. Confirmed findings fixed:

- GI1, TC10: the reftable twin reads each test only to its closing brace, through a
  matcher (`required_skip_violations`) with a self-test spelling the deleted SKIPPED
  line, the requirement moved to the next test, and an idle `if .. {}`.
- TC1, TC16: the cancelled ahead/behind test cancels inside a 2,000-commit frontier
  paint and requires reads to stop there (`AheadBehindCancelled` now carries
  `commits_read`); it fails with `Polled`'s poll removed (4,023 commits read). A cancel
  between two branches answers `branches: 1`.
- TC2: a loose chain of three tags, peeled object by object; a one-hop peel fails it.
- GI3, GI4, GI6: the setting test's git half skips aloud and is required too; the gate's
  note and `docs/qa-gate.md` name the probe and its twin; "exactly as the test does"
  reworded.
- TC3, TC4, TC5, TC6, TC8, TC9, TC12, TC13, TC14, TC15: the retarget is the only change
  between two reads; chains at and past git's depth; a cancel at every poll of a refs
  query, with the poll count exact; exact costs on a small fixture; a damaged stash
  reflog; ahead/behind in a shallow clone; version 0 with `reftable` and a linked
  worktree of a reftable repository; a merge on the upstream side; the one global key
  read pinned locally in fixtures; the vacuous worktree claim dropped.
- QC-F3: the upstream pass polls the cancel per branch.
- QC-F5: committer dates running against the graph — gix's count equals git's there.
- Probes, both confirmed and fixed against git: QC-F1, a repository with no `config`
  file opens as git and gix open it; QC-F4, `HEAD` through a symbolic branch is the end
  of the chain, unborn when that end is missing (git's `symbolic-ref` follows every
  level). Found while fixing TC4: `%(symref)` names the END of a symbolic chain, and
  git reads at most five refs, the ref itself included — the snapshot read one hop too
  many and named the first hop; both fixed, pinned against git.
- Found while fixing TC8: git numbers a stash list so that an unparseable reflog line
  takes no number while a missing commit keeps its own; the snapshot gave the bad line
  a number; fixed, and the oracle now reads git's own `%gd`.
- DO1, DO2, QC-F2: docs say what `ref_tips` compares, that a gix configuration re-read
  restores the namespace, and that gix's open reads `HEAD` before the refusal.

Dismissed, with the adjudicator's reasons:

- GI2: an inherited residual already stated in the root CLAUDE.md, tracked by open
  issue #58, whose generic fix covers this probe too.
- GI5: CI's `test-full` runs the gate's probe, matching the fsmonitor precedent.
- GI7, TC11, and QC-F7's floor half: every floor-run pattern the twin misses fails
  loudly (a floor git cannot make reftable, so the required test goes red), never
  silently.
- DO3: the gitoxide-mutation twin already scans these files, and they only read.
- DO4: `ref_tips` was uncancellable before, runs off the UI thread, costs 28.4 ms at
  10,000 refs, and phase 06 replaces it.
- TC7: a deliberate two-read determinism check.

For the user's end batch: QC-F6 (invalid ref names not counted), in `state.md`.

## 2026-10-06 — phase 01: the refs engine

The refs snapshot, upstreams, the stash list and ahead/behind in `cairn-git`, the
reftable refusal at open, and their vocabulary in `cairn-model`
(`docs/systems/refs.md`). Packet mode, on `feature/refs-and-status`.

**Stopped once for the user** (a phase-01 stopping rule): gix's upstream API
disagreed with `%(upstream)` on three cases the four parity rules did not cover —
two `branch.<n>.merge` values (gix takes the last, git the first), a short merge with
a named remote (gix expands and maps it, git maps nothing), and two fetch refspecs
both mapping the merge (gix does not take the first). The user chose resolving every
upstream by hand from the configuration (a fifth parity rule, PRD R1.4, L3); kept a
ref naming a missing object skipped and counted (a divergence from git's `fatal:
missing object`, R1.3); and had a format-version-0 repository with
`extensions.refStorage` refused at open, as git refuses it (R1.9).

**C11, refs half** — release build, warm, median of seven, on the machine in
`docs/research/diff-engine/measured-baseline.md` (AMD Ryzen 7 9800X3D), reporter
`measures_the_refs_snapshot_and_ahead_behind`:

| Repository | Refs | Refs snapshot | Snapshot + every ahead/behind |
| --- | --- | --- | --- |
| rust-lang/rust at `c999cef531e` (`main` equal to `origin/main`) | 175 | 0.14 ms | 0.13 ms (one branch, no walk) |
| generated: 3,000 branches, 3,000 remote-tracking, 4,000 tags (1,000 annotated), packed, 500 loose | 10,001 | 28.4 ms | 28.3 ms (ten branches, equal) |

Within C11's 100 ms on the bench. Beyond C11, a divergence built over the bench's
real history (a branch at `HEAD` whose upstream is `HEAD~n`), each count equal to
`git rev-list --left-right --count`:

| Upstream | Ahead | Cairn | Commits read | git |
| --- | --- | --- | --- | --- |
| `HEAD~100` | 1,957 | 189 ms | 39,209 | 82-94 ms |
| `HEAD~1000` | 22,649 | 1.32 s | 290,337 | 547-622 ms |
| `HEAD~10000` | 169,679 | 3.02 s | 732,393 | 1.14 s |

About two to three times git: gix's hidden frontier reads far more than the
divergence, and no commit-graph is used so that every read can be cancelled (the
bench has none either). It runs on the refresh thread (phase 06) and is cancellable,
so it holds up nothing; a faster count is a follow-up, not a C11 failure.

## 2026-10-05 — slim rows added

The user locked L14: the rest of a row is slimmed (one id, a parent count, shared
text and author stores, no email, no per-row allocation), as a phase 04 of its
own; later phases renumbered 05-10. C16 bars all of rust-lang/rust at 64 MiB of
retained rows; C15 keeps the edge equivalence and the find's time.

## 2026-10-05 — plan revised before merge

At the user's request, two audit-settled choices were checked against evidence
before the planning PR merged. Fork draws a stash only on a commit its ref walk
reaches (`fork-unreachable-stash-base.md`), so the plan follows that rather than
seeding the walk with stash bases. A deep find was measured on rust-lang/rust
(`deep-find-measured.md`: 2.4 s, 1.4 GiB retained, 89% edges) and Fork's own
approach researched (`fork-deep-history.md`: a 50,000/100,000-commit cap, layout
of the visible area only); the user locked L13 — compact rows, no cap — and a new
phase 03 for it, renumbering later phases 04-09. The systems doc's per-row memory
figure was corrected. A fresh audit caught L13's 60 MB estimate (it is about
160 MiB); C15's bar is 192 MiB.

## 2026-10-05 — planned

`/feature-plan` from the roadmap's packet 4 brief. Four evidence records saved
under `docs/research/refs-and-status/`: a code audit, gix's refs and status APIs
read from the vendored source and probed, a 38-fixture status agreement spike
against git 2.56.0 and 2.30.9 with costs on rust-lang/rust, and a study of Fork's
labels, sidebar, Local Changes and refresh. The user locked L1-L12 as
recommended, closing program O2 for `git status`. PRD, design rewrites
(`engine.md`, `history-graph.md`, `ui.md`, `concurrency.md`,
`feature-inventory.md`) and this directory written on
`feature/plan-refs-and-status`, for a planning PR into `main`.
