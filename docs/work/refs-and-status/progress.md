# Progress — refs-and-status

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

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
