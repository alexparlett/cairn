# Brainstorm — history-topo-order

Locked decisions and the alternatives the user rejected, with their evidence. Planning
session 2026-10-17, after `fork-alignment`'s planning found the bug. Evidence, all in
`docs/research/history-topo-order/`:

| Short name | File |
| --- | --- |
| GT | `git-topo-order.md` — what `git log` orders by; git's algorithm cited in git's source at v2.30.9 and v2.56.0; the oracle command; costs on the bench with and without a commit-graph |
| GX | `gix-topo-apis.md` — gix 0.87.1's `commit::Topo` walk, its parity with git, its cost, gix's commit-graph reading, the building blocks |
| WB | `walk-as-built.md` — Cairn's walk, lane assigner, tests, reporters and worker as built |

## The bug

With tied commit times Cairn draws a parent above its child, as the dashed edges running up
the graph show. The same happens under clock skew, with no tie at all. `git log --graph`
and Fork never do this (`docs/research/ui-review/fork-window-survey-2-2026-10-16.md` §6).

The cause is gix's commit-time queue. It is a `BinaryHeap` keyed by time alone, so ties pop
in heap order, where git's queue keeps insertion order (WB headline 1).

The lane assigner hides the misordering inside its 1,024-row window by repainting rows. On
the bench, 49,609 of 345,545 rows (14%) are repainted and 627 lines arrive late (WB §2.1,
`docs/research/refs-and-status/measured.md`). `docs/systems/history-graph.md` still says
0-0.2%.

## Locked decisions

**L1. Match `git log --date-order`.** Each commit comes after every child the walk shows,
and otherwise in commit-time order.

Why this order:
- It is Fork's default, "Sort commits: By date". Fork's order on the ui-review recipe built with
  tied dates (FS2's `uirev2`) is byte-identical to it (GT). That is one fixture: how Fork
  interleaves other ties is not established (GT §1.3).
- On the bench it agrees with today's commit-time order for the first 460 rows, so it is
  the smallest visible change (GT).
- A stash fits it as a tip whose one parent is its base, which is how the oracle grafts it (L8).

*Rejected:* `--topo-order`, `git log --graph`'s own default.
- It draws a narrower graph: at most 149 lines across a row on the bench, against 222.
- But it moves rows a median of 5,391 places on the bench, and it is not what Fork shows by
  default.
- git runs both orders through one algorithm with a different queue, so topo order can be a
  Preferences setting later.

**L2. Cairn's own walk, over gix's building blocks.** git's date-order walk is written in
`cairn-git` over `gix_revwalk::Graph` (cached commit reads) and a priority queue that breaks
ties by insertion order, as git's `prio_queue` does. It is exact by construction and tested
against git.

Why not gix's own walk: gix's `commit::Topo` matches `--topo-order` row for row (once its
tips are given in reverse), but its `DateOrder` breaks ties arbitrarily. On a tied-time
history that put 147k of 200k rows out of place, and nothing outside gix can fix it (GX).

*Rejected:*
- Patching gix-traverse in a fork and pinning it until upstream merges the fix. Its walk
  also polls nothing during the opening pass.
- gix's `Topo` with `TopoOrder`, which follows only from L1's rejected order.

**L3. Accept the opening pass, visibly.** Without a commit-graph every ordered walk — git's,
gix's, Fork's — reads the whole history before its first commit.

| On the bench (345,545 commits) | First 100 lines | Memory |
| --- | --- | --- |
| No commit-graph | 1.31 s | 522 MiB for git |
| With a commit-graph | 84 ms | — |

The 84 ms is not O(page): the indegree walk must reach down to the oldest tip's generation,
and old tags put that 337,966 commits deep (GT).

How Cairn takes it:
- While the pass runs, Cairn says "Reading history…" with a count. A newer open supersedes
  it, and a close never waits on it for more than one poll.
- The first-page bar is restated relative to git's own `--date-order` on the same
  repository.
- The design's memory rule ("what scrolling retains grows with the rows passed") is restated
  for the walk's per-commit state, measured on the bench.

*Rejected:*
- Offering to write a commit-graph (`git commit-graph write --reachable`, a new write verb).
- Drawing commit-time rows at once and reordering them when the pass ends, which makes rows
  jump visibly on every slow open.

**L4. The history walk gets a thread of its own.** On the repository thread an opening pass
would make sidebar, Changes-tab and Local Changes filter keystrokes, a refresh's refs and
retires wait up to 1.3 s behind it (WB §4.2).

- The history lane, its pages and its finds move to a history thread.
- That thread takes its own thread-local repository handle, as the diff and refresh lanes do.
- The walk is still not `Send` and never leaves the thread.

*Rejected:* accepting the wait; slicing the pass so it serves queued jobs between slices,
which is more complex to get right and to test.

**L5. The oracle.** Each mode's order must equal git's:
- Tips: `git log --date-order --format=%H --branches --remotes --tags HEAD`.
  - It is not `--all --exclude=refs/stash`, which also adds notes, replace refs and other
    worktrees' refs.
  - Tips are given in a fixed order, since git keeps tip order on ties.
- Stashes: each stash grafted onto its base (`git replace --graft <stash> <stash>^1`, in the
  fixture) and fed on stdin, only those whose base the refs reach.
- Show Lost Commits: the old and new id of each entry of `HEAD`'s and each local branch's
  reflog, given explicitly on stdin.
  - Not `--reflog`, which takes every reflog and cannot be narrowed.
  - Its tip order also differs across git versions: listed by directory in 2.30.9 and
    2.32.7, sorted in 2.56.0.

The oracle gives identical output at 2.30.9, 2.32.7 and 2.56.0 on every fixture, with and
without a commit-graph (GT).

**L6. The git source used for the citations.** The user approved a shallow clone of git at
v2.30.9 and v2.56.0 into `~/.cache/cairn/git-src`, read-only, for file:line citations of
`revision.c`'s walk (GT's corrections and "The algorithm as Cairn must copy it").

**L7. A reopen keeps the old rows.** On a repository without a commit-graph every reopen —
a commit, a fetch that moves refs, Create Branch, a refresh after an outside change — takes the
full pass (about 1.3 s on the bench, WB §4.2). The old history stays drawn and usable, with
"Reading history… N commits" on a strip over it, until the new first page replaces it with the
selection kept by id. A first open still shows the sentence. *Rejected:* blanking the list at
the reopen, as today, which on such a repository blanks it for about a second on every commit.

**L8. Two corrections from the coverage audit, which follow from L4 and L5 rather than choosing
anything new.**
- *Stashes are tips of the walk, not rows merged into the stream.* In the oracle, a grafted
  stash is a tip inside git's in-degree walk. It pops after a ready ref tip it ties with,
  because it was given after `HEAD`, and it counts toward its base's in-degree, which moves other
  rows. On the tied fixture, merging stash rows into the stream gets both the first row and the
  base's place wrong (`8313cd7, 2d11c10, stash, fdf1094, abed6f2…` from the oracle, against
  `stash, 8313cd7, abed6f2…`).
  - So a stash is a synthetic one-parent tip, given after `HEAD` and before the reflog tips.
  - The walk decides whether the refs reach its base before seeding it.
  - The design's "Stash rows" is rewritten to say so.
- *The refs read stays on the repository thread.* A `Refresh` bumps the refs lane, not the
  history lane. On the history thread its refs read would queue behind a running pass, so no
  refresh could supersede it, and ahead/behind would wait too.
  - The repository thread keeps the snapshot the history was opened from and compares a
    refresh's refs with it, as today.
  - When they differ, it asks the history thread to reopen, and that reopen's epoch supersedes
    the pass.

## Settled by the evidence, without a decision

- **The lane assigner stays total.** The frozen compact-rows oracle and its fingerprint, the
  equivalence tests, the skew tests and the `CommitTime` and `GraphOrder` orders all need it
  to accept any arrival order (WB §2.4).
  - Under date order its late-parent path never fires for the history.
  - Its window and priming are kept: after the opening pass, laying out 1,088 rows for a
    64-row page costs milliseconds.
  - A smaller window for this order alone is an optimization the packet does not take.
- **`recent_messages` keeps commit-time order**, for `git log -n 10` parity.
- **Show Lost Commits' "reached after all" correction** (`reach.rs`) never fires under date
  order. It is removed only if no remaining order uses it.
- **The tip trap.** Cairn answers tip reads with a parentless stand-in (`Seeded`), which would
  cut a parent-walking algorithm off at the tips. The new walk must read the real commits
  (WB §1.11).
- **Tests that fail by design** (WB §3.1) are rewritten to the new order with their reason,
  never deleted to make room:
  - `a_backward_line_across_a_page_boundary_is_a_gained_segment`;
  - `a_row_that_left_the_window_is_never_repainted`;
  - `a_session_over_a_skewed_history_returns_every_commit`;
  - `a_parent_dated_after_its_reached_child_is_not_drawn_as_lost`.
