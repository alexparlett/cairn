# staging-and-commit

Packet 5 of the `daily-loop` program (`docs/work/daily-loop/roadmap.md`). It makes
Local Changes act: stage, unstage and discard a file, a hunk, a run of lines or a
mode change as Fork does, and commit or amend from a commit box under the diff.
Every destructive operation is sealed behind a confirmation the engine computes
and re-checks before it runs; what a commit-level one leaves behind is
recoverable from Show Lost Commits, and every operation reads back, with the
prompt it confirmed, in the activity popover.

Spec: `docs/prd/staging-and-commit.md` (the ONE copy of the acceptance criteria).
Decisions: `brainstorm.md`. Evidence: `docs/research/staging-and-commit/`.
Integration branch: `feature/staging-and-commit`, base `main`.

| Phase | File | Builds |
| --- | --- | --- |
| 01 | `phase-01-seal.md` | `Confirmed` bound to a `Consequence`, the two rosters and their guards; read-only output ceilings |
| 02 | `phase-02-patch-engine.md` | Inversions, partial untracked files, the mode item, path quoting, staged renames as git pairs them |
| 03 | `phase-03-write-verbs.md` | Every stage, unstage, discard and delete verb, the stale check, lock outcomes; git's baseline measured |
| 04 | `phase-04-local-lane.md` | The local write lane, the write counter over status, prompts during writes, closing mid-write |
| 05 | `phase-05-commit-engine.md` | Commit and amend, amend's staged list and `Consequence`, hooks found, recent messages |
| 06 | `phase-06-render-foundations.md` | The text-field key policy, the bare-key scope and new chords, the confirmation dialog, the menu host, auto-scroll |
| 07 | `phase-07-local-changes-actions.md` | Multi-select, Fork's five file routes, the discard dialog, Ignore Whitespace off |
| 08 | `phase-08-diff-gesture.md` | Hover actions over a chunk, drag-selected lines, the mode row |
| 09 | `phase-09-commit-box.md` | The commit box, Amend, hook progress and failure, amend's confirmation |
| 10 | `phase-10-lost-commits.md` | Show Lost Commits and `Create Branch Here…` |
| 11 | `phase-11-activity-and-measured.md` | The activity popover, `Remove index.lock…`, Cairn's measured numbers and the window check |
| 12 | `phase-12-qa.md` | The merge bar, as its own fresh session; teardown |
