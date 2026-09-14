# Packet: history-graph

Draws the commit graph of a real repository and keeps it interactive while it
loads. Lands the pure lane-assignment algorithm, the bounded engine query that
feeds it, the worker boundary that keeps repository work off the UI thread, and a
virtualised view that renders the result.

Spec: `docs/prd/history-graph.md`. Design frame: `docs/design/cairn.md` decisions
D3 and D4. Evidence: `docs/research/history-graph/gix-revwalk-ordering.md`.

**This packet goes first.** The dependency on `credential-prompts` runs one way
only: that packet defers to this one's worker boundary, and its
cache-invalidation contract needs this packet's graph view to be observable at
all. This packet also exercises the gix read path, which is the half of decision
D1 that was reasoned about rather than measured.

Integration branch: `feature/history-graph`, off `main`.

## Phases

| Phase | What it lands |
| --- | --- |
| [01](phase-01-lane-assignment.md) | `cairn-model` graph vocabulary and the pure lane assigner |
| [02](phase-02-history-query.md) | `cairn-git`'s bounded, cancellable history query |
| [03](phase-03-worker-boundary.md) | `cairn-app`'s repository worker pool and request epochs |
| [04](phase-04-graph-view.md) | opening a repository (R5), and the virtualised graph view wired end to end |
| [05](phase-05-qa.md) | merge-bar QA over the whole packet, then teardown |

Critical path: 01 → 02 → 04 → 05. Phase 03 is independent of 01 and 02 and can
run in parallel under `/orchestrate-packet`, but 04 needs all three.
