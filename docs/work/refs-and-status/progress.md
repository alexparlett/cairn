# Progress — refs-and-status

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

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
