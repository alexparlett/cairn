# Progress — refs-and-status

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

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
