# History graph

Intent, not as-built; `docs/systems/history-graph.md` describes what exists.
Spine: `docs/design/cairn.md`, where lane assignment is decision **D4**.

## What it is for

The graph is the main view when a repository opens, and it stays readable at
scale: an actual graph that stays legible across a repository with many
long-lived branches, not a list of commits with a decorative gutter, and it stays
interactive while it loads. Columns: lanes, subject with ref labels, author,
short id, date. Lane identity never depends on colour alone; the column position
carries it (`ui.md`).

## What it walks

Every ref, as Fork's All Commits view does: every local branch, every
remote-tracking ref, every tag that identifies a commit, `HEAD`, and the commit
each stash was made on, read in one snapshot that the walk and its labels
share. A branch the checkout cannot reach
is in the graph, because a user looking for it would otherwise conclude it is
gone. Narrowing the graph to one branch is a filter over this, not a different
walk.

## Labels

Each ref pointing at a commit is drawn on that commit's row as Fork draws it: an
outlined chip between the graph and the subject, tinted with the row's lane
colour, a tag indigo with a tag glyph, a remote-tracking ref with a remote glyph
— a forge's own icon once the forge is known (`forge-links.md`) — and the current
branch marked ✓, and the subject of `HEAD`'s commit bold. A remote-tracking ref at the same commit
as the branch tracking it shrinks to its glyph in front of the branch's chip,
which is how Fork says "up to date". Labels that do not fit are clipped at the
column's edge, as Fork clips them. A label's kind is its glyph and shape, never
its colour alone.

## Stash rows

A stash is a commit with two or three parents — the commit it was made on, a
commit holding the index, and one holding untracked files — and walking it as a
tip would put those inner commits in the graph, while hiding them would hide
their ancestors, which are the real history. So a stash is not a tip: each stash
is a row of its own kind, merged into the stream by its commit time but never
after the commit it was made on, with one edge, to that commit, and a
`stash@{n}` chip before its message. That base commit is a seed of the walk, so
the edge lands even when the branch the stash was made on is gone.
This is Fork's rendering. Selecting one shows what it changed against that
commit, which is what `git stash show` shows.

## No working-tree row

The graph has no row for uncommitted changes. Fork draws none; the sidebar's
Local Changes, with its count, is the entry point to the working tree (`ui.md`).
Every row of the graph is a commit or a stash.

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
history in memory, which is the failure this design exists to avoid. A stash row
takes a lane like a commit does, and its one edge joins the lane of the commit it
was made on.

## A scroll keeps its walk open

gitoxide's walk cannot be resumed from a value, so resuming from a cursor means
replaying, and page *k* would cost *k* × the page size. A scroll therefore holds
its walk for its whole life, making each page cost one page; the cursor remains
as the cold-restart path. The held walk is what pins history to one worker thread
(`concurrency.md`). Only a viewport's worth of rows is ever built, however long
the history. Finding a ref's commit that is not loaded yet pages the held walk
forward until it arrives; it is history work, so the next scroll or press
supersedes it like any page. When the refs move — a fetch, a commit or checkout
made in a terminal, a new stash — the walk is reopened from the new snapshot,
and a refresh that leaves the refs and `HEAD` as they were reopens nothing. Specs:
`docs/prd/history-graph.md`, `docs/prd/refs-and-status.md`.
