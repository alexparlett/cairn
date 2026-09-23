# History graph

Intent, not as-built; `docs/systems/history-graph.md` describes what exists.
Spine: `docs/design/cairn.md`, where lane assignment is decision **D4**.

## What it is for

The graph is the main view when a repository opens, and it stays readable at
scale: an actual graph that stays legible across a repository with many
long-lived branches, not a list of commits with a decorative gutter, and it stays
interactive while it loads. Columns: lanes, subject with ref badges, author,
short id, date. Lane identity never depends on colour alone; the column position
carries it (`ui.md`).

## Lanes are assigned incrementally, in the engine

The walk runs newest-first. The assigner keeps a vector of active lanes, each
holding the commit id it is waiting for. A commit takes the leftmost lane waiting
for it; that lane then waits for the commit's first parent, and further parents
take new or joined lanes. Each row carries its lane plus the edge segments
crossing it.

gix offers no `--topo-order` equivalent: its sortings are `BreadthFirst`,
`ByCommitTime` and `ByCommitTimeCutoff`, and commit-time order emits a parent
before its child under clock skew, which is common in rebased and imported
history. So the assigner is correct under out-of-order arrival rather than
trusting the walk. It holds a bounded window of recent rows, so that a line to a
parent arriving late has a row to repaint. A lane index, once emitted, is final;
an edge segment inside the window is not. Skew deeper than the window is a stated
blind spot. Evidence: `docs/research/history-graph/gix-revwalk-ordering.md`.

The cost is amortised constant work per commit and retained state proportional
to the window times the lanes across it, never to history length; lane width on
real repositories is single digits. Because appending never renumbers an emitted
lane, rows stream into a virtualised list. Evidence:
`docs/research/history-graph/scroll-memory-model.md`.

Lanes belong in `cairn-git`, not `cairn-ui`: the lane is part of the answer, so it
is `cairn-model` vocabulary. A component that computed lanes would need the whole
history in memory, which is the failure this design exists to avoid.

## A scroll keeps its walk open

gitoxide's walk cannot be resumed from a value, so resuming from a cursor means
replaying, and page *k* would cost *k* × the page size. A scroll therefore holds
its walk for its whole life, making each page cost one page; the cursor remains
as the cold-restart path. The held walk is what pins history to one worker thread
(`concurrency.md`). Only a viewport's worth of rows is ever built, however long
the history. Spec: `docs/prd/history-graph.md`.
