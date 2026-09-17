# Packet: diff-engine

Reads what a commit changed, what two commits differ by, and what a working-tree
file differs from its staged and committed versions, and draws it the way Fork
does. The model underneath is patch-capable from its first commit: it can emit a
valid patch for an arbitrary subset of hunks and lines, which is how packet 5 will
stage a single line. Nothing consumes that emitter yet, and its round-trip tests
against `git apply` are the point of building it now.

Spec: `docs/prd/diff-engine.md`. Design frame: `docs/design/cairn.md` decisions
D1, D3, D5 and D6, and `docs/design/ui.md` for the layout. Decisions:
`brainstorm.md` L1-L16. Evidence: `docs/research/diff-engine/` (six records).

Integration branch: `feature/diff-engine`, off `main`.

## Phases

| Phase | What it lands |
| --- | --- |
| [01](phase-01-diff-model.md) | the model, its projections and the patch emitter, in `cairn-model` |
| [02](phase-02-engine-commits.md) | commit and comparison diffs in `cairn-git`, with the round-trip tests and the bench reporter |
| [03](phase-03-engine-working-tree.md) | working-tree diffs of one path, the filter pipeline, and D1's amendment |
| [04](phase-04-worker-lanes.md) | per-lane epochs, the diff thread and the application state |
| [05](phase-05-detail-pane.md) | the detail pane, the Commit tab and the accelerator table |
| [06](phase-06-unified-diff.md) | the unified diff view, its font and colours, and the header toggles |
| [07](phase-07-changes-tab.md) | the Changes tab, the non-text states, and side-by-side |
| [08](phase-08-expand-and-compare.md) | expansion in place, comparing two commits, and the window measurement |
| [09](phase-09-qa.md) | merge-bar QA over the whole packet, then teardown |

Strictly sequential. Each phase is the next one's substrate: the engine has
nothing to answer with until the model exists, the worker has nothing to carry
until the engine answers, and every UI phase draws what the one before it built.
