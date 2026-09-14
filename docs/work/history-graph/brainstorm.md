# Brainstorm — history-graph

Locked decisions and rejected alternatives. Historical record: never retro-edited.

## Locked 2026-09-14

**L1. Lanes are computed in the engine, not the view.** The lane index is part of
the answer to "what is the history", so it is `cairn-model` vocabulary. Rejected:
computing lanes in `cairn-ui` from a list of commits — it would require the whole
history in memory to draw the first screen, which is the failure the design is
built to avoid. Cites: `docs/design/cairn.md` D4.

**L2. Newest-first incremental lane assignment.** Active-lane vector, leftmost
waiting lane wins, first parent inherits the lane, extra parents open or join
lanes. Rejected: a global layout pass (correct, but needs all commits before
drawing anything) and per-screen recomputation (stable only by accident, and
quadratic when scrolling). Cites: D4.

**L3. The assigner must be total over arrival order.** gix offers no
`--topo-order` equivalent, and commit-time order genuinely emits parents before
children under committer-date skew — routine in rebased and imported history.
Rejected: asserting child-before-parent and treating violations as corrupt input,
which would make Cairn crash on ordinary repositories. Cites:
`docs/research/history-graph/gix-revwalk-ordering.md` findings 1 and 2.

**L4. The worker boundary lands in this packet.** It is infrastructure, but
building it without a consumer gets the interface wrong, and the history query is
its first and most demanding consumer. Rejected: a separate infrastructure packet
first. Cites: D3.

**L5. One `ThreadSafeRepository` per repository, a pool of thread-local handles.**
`gix::Repository` is not `Send`; gitoxide's model is a `ThreadSafeRepository`
converted per thread with `to_thread_local()`. Rejected: one dedicated thread per
repository — it serialises independent queries for no benefit, given gitoxide
already shares the object database across handles. Cites: D3.

**L6. Request epochs from the start.** A superseded query's result is discarded
rather than rendered. Rejected: adding cancellation later — it reaches into every
call site, so retrofitting it is the expensive path. Cites: D3.

**L7. Uniform row height.** Rejected: variable-height rows for merge commits or
long subjects. Virtualisation with variable heights needs a measurement cache and
produces scrollbar jitter; the packet is not spending its budget there.

**L8. A7 (frame time on a 100k-commit repository) is measured by hand, not in
CI.** Rejected: a frame-time assertion in CI — it would be flaky on shared
runners and disabled within a month, which is worse than an honest recorded
measurement.

## Locked 2026-09-14, second pass

Reviewing the open questions with the user showed two of them were badly framed.
Both corrections are recorded here rather than by editing the entries above.

**L9. The total assigner is not an alternative to the bounded window — it is the
floor.** O1 originally offered them as a choice. Any window can be exceeded by
skew larger than the window, so the total behaviour from L3 is required either
way; the window is a pure refinement that only reduces how often the visually
ugly case appears. So phase 01 builds the total assigner and does NOT decide
between two options. Whether the window earns its keep is deferred until there
is a real repository showing it is needed — filed, not built.

**L10. R1.2's stability requirement covers lane INDICES, not edge segments.** As
originally written it blocked O4 outright: a late-joining parent should draw a
line through the rows between it and its child, and if those rows are frozen the
edge visually begins nowhere. Narrowed in `docs/prd/history-graph.md` R1.2 —
edges may repaint inside the loaded window, rows that have scrolled out are
final. This costs nothing, because the view re-renders visible rows from the
model on every change anyway. Rejected: freezing edges and drawing a
"joins above" stub marker instead — it is a worse picture, chosen only to satisfy
a requirement that turned out to be arbitrary.

## Open, for the phase that meets them

- **O2 (phase 02).** Whether `Sorting::ByCommitTime` is affordable on a repository
  with no commit-graph file. The gix docs imply an unaccelerated path looks each
  commit up twice. Measure before choosing a default sorting.
- **O3 (phase 03).** Pool size, and whether it is fixed or scales with cores. A
  pool of one is a valid starting answer if the measurement says so.
- **O4 (phase 04).** How a late-joining edge is drawn so it reads as intentional
  rather than as a rendering bug. No longer blocked: L10 permits repainting the
  connecting line through the intervening rows, so the question is now a visual
  one — how the repaint reads to someone watching it happen during a scroll —
  rather than whether it is expressible at all.
