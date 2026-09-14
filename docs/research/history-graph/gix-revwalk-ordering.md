# gix 0.87.1 revision-walk ordering, and what it means for graph layout

Evidence record. Gathered 2026-09-14 for the `history-graph` packet. Method:
reading the linked source under
`~/.cargo/registry/src/index.crates.io-*/gix-0.87.1/`, not the published docs —
gix is pre-1.0 and the registry copy is what actually compiles into Cairn.

## Finding 1 — gix has no `--topo-order` equivalent

`gix::revision::walk::Sorting` (src/revision/walk.rs) has exactly three variants:

| Variant | Order it produces |
| --- | --- |
| `BreadthFirst` (the default) | graph order. Its own doc-comment says it is "not to be confused with `git log/rev-list --topo-order`, which is notably different from as it avoids overlapping branches". |
| `ByCommitTime(CommitTimeOrder)` | pure commit-timestamp order, newest or oldest first. "The sorting applies to all currently queued commit ids and thus is full." |
| `ByCommitTimeCutoff { order, seconds }` | as above, stopping once nothing younger than the cutoff is queued. |

There is no fourth. `Sorting::into_simple` maps all three onto
`gix_traverse::commit::simple::Sorting`, so the traversal layer offers no more
than the `gix` layer exposes.

## Finding 2 — commit-time order can emit a parent before its child

`ByCommitTime` sorts by the committer timestamp alone. Nothing in git makes a
parent's committer date older than its child's:

- Rebases and cherry-picks rewrite committer dates, routinely producing a
  rewritten commit whose date is newer than commits that came after it.
- Imported history (`git fast-import`, CVS/SVN conversions) carries whatever
  dates the source had.
- Clock skew across machines on a shared branch needs no pathology at all.

So a newest-first walk can yield a commit whose child has not been seen yet.

**Why this matters to the layout in decision D4.** The lane assigner reserves a
lane for a commit when it processes that commit's *child*. A commit arriving with
no lane reserved is not a corner case to be asserted away: it is the normal
consequence of skew, and the assigner must place it — plausibly by opening a new
lane and letting the edge join later.

## Finding 3 — the walk carries what a topological ordering would need

`gix::revision::walk::Info` (same file) exposes, per commit and without reading
the full object:

- `id`
- `parent_ids` — the whole point for layout; no object decode needed
- `generation: Option<gix_revwalk::graph::Generation>` — a commit-graph
  generation number, `Some` only when the commit came from a commit-graph file
- `commit_time: Option<SecondsSinceUnixEpoch>` — `Some` only when the chosen
  traversal considered dates

`Info::object()` is documented as expensive and unnecessary "unless one needs
more than parent ids and commit time", which is precisely the layout's position.
`Repository::commit_graph()` and `commit_graph_if_enabled()` exist
(src/repository/graph.rs), so the generation number is available whenever the
repository has a commit-graph file — which is not guaranteed, since writing one
is a `git` maintenance operation the user may never have run.

## Candidate approaches for the packet

Not a decision; the options the packet's first phase must choose between, with
what each costs.

1. **Make the assigner total.** Accept any arrival order: a commit with no
   reserved lane opens one. Cheapest, streams perfectly, and correct in the sense
   of never crashing or mis-parenting — but under skew it can draw an edge that
   visually jumps, because the child was drawn before the parent's lane existed.
2. **Sort within a bounded window.** Buffer N commits, order them by
   `(generation, commit_time)` where generation is available, emit from the back
   of the window. Fixes almost all real skew (which is local) at the cost of N
   rows of latency before the first paint and a window size that is a guess.
3. **Precompute a topological order.** Correct always, and wrong for Cairn: it
   requires walking the whole history before drawing anything, which is the exact
   failure D4 exists to avoid.

Option 1 plus a rendering rule that makes a late-joining edge legible looks like
the shape that fits the constraints, but that is the packet's call to make
against real repositories, including one with deliberately skewed history.

## What was NOT established

- Whether `BreadthFirst` happens to guarantee child-before-parent in practice.
  Its doc comment describes graph order, not a topological guarantee, and no test
  in the source asserts one. Treat it as unguaranteed.
- How expensive `Sorting::ByCommitTime` is on a repository without a commit-graph
  file. The doc notes it "benefits greatly" from an object cache, implying the
  unaccelerated path looks commits up twice. Needs measuring, not guessing.
