# Progress — history-graph

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-09-14 — phase 01: the lane assigner, in packet mode

Built the total assigner in `cairn-model` (`graph.rs` for the vocabulary,
`lane_assignment.rs` for the algorithm) with the fixture set and the A1/A2/A3
tests in `crates/cairn-model/tests/`. Committed directly onto
`feature/history-graph`; the repository still has no remote, so nothing was
pushed and no pull request was raised.

**How R1.4 was answered.** A commit arriving with no lane reserved for it takes
the leftmost free lane. When a child for it turns up later, the joining line is
drawn *upward* through a lane that was free on every row in between, and every
segment of that line is flagged `out_of_order`. That is the evidence record's
candidate 1 plus the rendering rule it asked for: the line is complete rather
than jumping, and phase 04 has the flag it needs to mark a link that runs
backwards on screen.

**A defect the generated histories caught.** The first cut indexed only commits
that had arrived *without* a reservation, on the theory that those are the only
ones that can gain a late child. False: a commit with two children can have one
of them arrive before it and the other after, so a commit laid out in a reserved
lane can still gain a child later. The random-skew property test found it
(`c2`'s line to `c3` descended into a lane that never filled). The assigner now
indexes every row it lays out.

**The R1.3 tension, stated rather than hidden.** R1.3 asks for retained state
proportional to open lanes, never to commits seen. R1.2's repaint asks the
assigner to add segments to rows already emitted, which it can only do if it
still holds them. The two cannot both be literally true, so the assigner keeps
its rows and an index into them, and its doc comment says so: a caller bounds it
by loading a window into one assigner, not by expecting the assigner to forget.
Lane bookkeeping proper is still one slot per open lane. Flagged to the user.

**Every fixture's stated mutation was verified by making it.** Each fixture test
names the change to the assigner it would catch; each was applied and watched to
fail. One claim was wrong and is now corrected: "the first parent takes a fresh
lane instead of the commit's own" does *not* fail the linear fixture, because the
leftmost free lane is the commit's own lane in a linear history. That exposed a
genuinely untested rule — a branch keeping its lane when a lower one falls empty
— which now has its own fixture.

**Out-of-scope fix the gate forced.** `cairn-git`'s
`discovers_this_repository_from_a_nested_path` asserted `git_dir().ends_with(".git")`,
which is false in a linked worktree (`.git/worktrees/<name>`) — the checkout
layout this repository's own workflow mandates. Pre-existing, unrelated to the
assigner, and it fails the gate for every packet. Fixed in its own commit to
assert the path *is* a git directory rather than what it is called.

QA findings: see the entry below this one once `/qa` has run — dismissed
findings are logged there with their reasons.

## 2026-09-14 — packet order settled; two gaps in this plan closed

This packet goes first. The dependency is one-directional: credential-prompts
phase 03 already defers to this packet's worker boundary, and its
cache-invalidation contract names the graph view as the thing that proves it, so
running that packet first would leave its own acceptance unobservable. This packet
also exercises the gix read path, which is the half of D1 that was asserted from
gitoxide's design goals rather than measured.

Two gaps in the plan as filed, both closed:

Nothing opened a repository. Phase 03 assumed one was open and phase 04 wired a
view to it, but no phase called `Repository::discover`. Added as requirement R5
and phase 04 deliverable 5, deliberately minimal — a command-line argument, not a
picker, because the repository-manager shape is parked in the spine.

Phase 03 would have over-fitted the worker boundary to paged queries. Fetch is
already specified (credential-prompts R4) and is a different shape: long-running,
progress-reporting, and blocking mid-operation on a UI prompt. Phase 03 now reads
that requirement as a second known consumer while designing, without building it.

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
