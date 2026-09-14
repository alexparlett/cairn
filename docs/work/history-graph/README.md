# Packet: history-graph

Draws the commit graph of a real repository and keeps it interactive while it
loads. Lands the pure lane-assignment algorithm, the bounded engine query that
feeds it, the worker boundary that keeps repository work off the UI thread, and a
virtualised view that renders the result.

Spec: `docs/prd/history-graph.md`. Design frame: `docs/design/cairn.md` decisions
D3 and D4. Evidence: `docs/research/history-graph/gix-revwalk-ordering.md`.

Integration branch: `feature/history-graph`, off `main`.

## Phases

| Phase | What it lands |
| --- | --- |
| [01](phase-01-lane-assignment.md) | `cairn-model` graph vocabulary and the pure lane assigner |
| [02](phase-02-history-query.md) | `cairn-git`'s bounded, cancellable history query |
| [03](phase-03-worker-boundary.md) | `cairn-app`'s repository worker pool and request epochs |
| [04](phase-04-graph-view.md) | the virtualised graph view, wired end to end |
| [05](phase-05-qa.md) | merge-bar QA over the whole packet, then teardown |

Phases 01 and 02 are independent of 03; 04 needs all three.
