# Progress — history-graph

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-09-14 — open questions reviewed with the user, two were badly framed

Walking the packet's open questions with the user closed one and unblocked
another, before any code was written.

O1 was not a choice: a bounded reordering window can always be exceeded by larger
skew, so the total assigner is required either way. Recorded as L9; the window is
deferred rather than decided.

O4 was blocked by R1.2 as I originally wrote it. Freezing edge segments along
with lane indices meant a late-joining parent could not draw the line connecting
it to its child, so the edge would begin nowhere. R1.2 is narrowed to lane
indices; edges may repaint inside the loaded window, which costs nothing because
the view re-renders visible rows from the model anyway. Recorded as L10, and the
PRD's R1.2 and A3 were updated to match.

Both were defects in the plan I filed, not discoveries about git.

## 2026-09-14 — packet planned

Filed from the design decisions locked the same day (`docs/design/cairn.md` D3,
D4). Recon was done directly against the linked gix 0.87.1 source rather than
published docs, and produced one finding that changed the plan: gix has no
`--topo-order` equivalent, so the lane assigner cannot assume child-before-parent
arrival. That became L3 and requirement R1.4, and it is the reason phase 01 opens
with a decision (O1) rather than with code.

No implementation has started.
