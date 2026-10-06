# refs-and-status

Packet 4 of the `daily-loop` program (`docs/work/daily-loop/roadmap.md`). It makes
the graph name what it shows — every ref walked and labelled as Fork labels it,
stashes as rows of their own, a sidebar of refs and a toolbar naming the branch —
and lists what `git status` says has changed in a read-only Local Changes view
that opens each file's diff. Status is git's own answer (D1's fourth read git
answers); refs are gix's, read under five parity rules; nothing writes.

Spec: `docs/prd/refs-and-status.md` (the ONE copy of the acceptance criteria).
Decisions: `brainstorm.md`. Evidence: `docs/research/refs-and-status/`.
Integration branch: `feature/refs-and-status`, base `main`.

| Phase | File | Builds |
| --- | --- | --- |
| 01 | `phase-01-refs-engine.md` | The refs snapshot and ahead/behind in the engine; reftable refused at open |
| 02 | `phase-02-status-engine.md` | `reads::status` — `git status --porcelain=v2 -z` — and its model |
| 03 | `phase-03-compact-rows.md` | A row keeps only its own lane changes; the drawn edges are derived |
| 04 | `phase-04-slim-rows.md` | The rest of a row slimmed: one id, a parent count, shared text and author stores |
| 05 | `phase-05-history-every-ref.md` | The walk seeded from every ref, labels on rows, stash rows |
| 06 | `phase-06-worker-and-refresh.md` | The new lanes, refresh on focus/operation/chord, history reopen |
| 07 | `phase-07-labels-and-toolbar.md` | Chips on rows, stash rows drawn, the Commit tab's REFS, the toolbar |
| 08 | `phase-08-sidebar.md` | The sidebar, its filter, and finding a ref's commit |
| 09 | `phase-09-local-changes.md` | The read-only Local Changes view, and the window check |
| 10 | `phase-10-qa.md` | The merge bar, as its own fresh session; teardown |
