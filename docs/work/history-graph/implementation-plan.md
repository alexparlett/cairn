# Implementation plan — history-graph

## Shape

```
cairn-model      GraphRow { commit, lane, edges }, Lane, EdgeSegment
                 LaneAssigner — pure, no I/O, no gix
                        │
cairn-git        history::query(repo, Bounds, &mut Cancel) -> Page<GraphRow>
                 gix revwalk → parent ids → LaneAssigner → rows
                        │
cairn-app        RepositoryWorkers: ThreadSafeRepository + pool
                 Request { epoch, query } → Response { epoch, page }
                        │
cairn-ui         GraphView: VirtualScrollView over GraphRow, edge painting
```

The seal already in force: `cairn-ui` cannot name `gix` or `cairn_git`, so the
view receives `GraphRow` values and nothing else. Phase 04 must not be tempted to
"just read one more thing" — that is a new model type and a new engine call.

## Phase order and why

01 and 02 are independent of 03 and could run in parallel in packet mode; 04
needs all three. 01 before 02 because the query's return type is the assigner's
output type — building the query first would invent a shape the assigner then has
to match.

## Review dispatch per phase

The packet's one copy of the rules. Every phase runs `/qa` at its end; these are
the reviewers each phase must include beyond the always-on `qa-checklist` and the
`qa-confirm` adjudication.

| Phase | Reviewers |
| --- | --- |
| 01 | `test-coverage-auditor` — the assigner is pure and fully testable, so a coverage gap here is a choice, not a constraint |
| 02 | `test-coverage-auditor`, `responsiveness-reviewer` (bounds, cancellation, object decoding) |
| 03 | `responsiveness-reviewer` (the whole phase is its subject matter) |
| 04 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 05 | all of the above over the whole packet diff, plus `gate-integrity-reviewer` if any phase touched the enforcement layer |

No phase in this packet touches `cairn-git/src/ops/`, so
`destructive-ops-reviewer` is out of scope throughout. A phase that finds itself
needing it has left the packet's scope and should stop and ask.

## Invariants in play

- The crate seal (`cairn-ui` ⊥ `gix`/`cairn_git`; `cairn-model` depends on
  nothing). Phase 01 adds types to `cairn-model`, which must stay dependency-free.
- No panic on a reachable path: the assigner receives whatever the walk emits,
  including arrival orders L3 describes. An `unwrap` there is a crash on a real
  repository.
- The UI thread never waits on repository work — not yet mechanically pinned, so
  phases 03 and 04 carry it as a review obligation via `responsiveness-reviewer`.

## New enforcement this packet should leave behind

Phase 03 makes "no repository work on the UI thread" checkable for the first
time, because `cairn-app` gains a real boundary to point a guard at. Land a guard
with it — a forbidden-token check over the render paths is the cheap version —
rather than leaving the invariant in the "not yet pinned" list in `CLAUDE.md`.
Moving it out of that list is part of phase 03's definition of done.
