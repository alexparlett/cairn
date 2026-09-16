---
status: in-flight
packet: history-graph
opened: 2026-09-14
---

# PRD — History graph

The authoritative spec for the `history-graph` packet while it is in flight.
Design frame: `docs/design/cairn.md`, decisions **D3** (worker pool) and **D4**
(incremental lane assignment). Evidence:
`docs/research/history-graph/gix-revwalk-ordering.md`.

## What this packet delivers

A history view that draws the commit graph of a real repository and stays
interactive while it loads: the lane-assignment algorithm, the engine query that
feeds it, the worker boundary that keeps it off the UI thread, and a virtualised
view that renders it.

This is the first feature to consume a repository at all, so it also lands the
worker boundary the rest of the application will use. That is deliberate:
infrastructure built without a consumer gets the interface wrong.

## Requirements

### R1 — Lane assignment is incremental and stable

- R1.1 Given commits in newest-first order with their parent ids, the assigner
  emits one **row** per commit carrying its lane index and the edge segments
  crossing that row. A row is the unit the view renders, and is not by definition
  a commit — see R6.
- R1.2 Processing further commits never changes a **lane index** already
  emitted. Edge *segments* may be repainted for any row the assigner still holds:
  a late-joining parent (R1.4) has to be able to draw the line that connects it,
  and the view re-renders visible rows from the model on every change regardless,
  so allowing this costs nothing.
- R1.3 The assigner owns a bounded window. It holds at most `window` rows plus an
  index into them, so retained state is proportional to that window times the
  lanes in play across it, never to the number of commits walked. Work per commit
  is amortised constant while parents arrive in order; a parent delivered early
  costs O(span x segments in the span), bounded by the window.

  The window is a **load budget**, not a memory mitigation — sized like the
  clients that ship one (Git Graph and lazygit both load 300 and page 100), not
  derived from a worst case. Measured cost on real repositories in the order
  Cairn actually walks is 264 B per row at p99 on the widest of seven, which is
  ~126-250 MB extrapolated to 500k commits. The much larger figures this
  requirement once carried were an artefact of breadth-first arrival order in a
  fixture whose branches never merged, not a property of any repository. Evidence
  and method: `docs/research/history-graph/scroll-memory-model.md` Part D.

  Known blind spot, carried and accepted for this packet: beyond
  `window + remembered` rows of skew the assigner cannot distinguish a parent
  already gone from one still to come. It fires on 0-0.2% of rows in the default
  order and is not closable from gitoxide's walk, which keeps its seen-set behind
  a boxed iterator with no accessor.
- R1.4 The assigner is **total over arrival order**: a commit whose lane was never
  reserved — the normal consequence of committer-date skew, per the evidence
  record — is placed, not rejected, and never mis-parented. Which placement
  strategy is chosen is the packet's decision; that one is chosen and justified
  is the requirement.
- R1.5 The assigner is pure: no gix, no I/O, no clock. It takes ids and parent
  ids and returns rows.

### R2 — The engine answers a bounded history query

- R2.1 `cairn-git` exposes a history query returning `cairn-model` rows: commit
  summary plus lane and edges. No `gix` type appears in its signature.
- R2.2 The query is bounded — it takes a limit and resumes from a cursor. There
  is no call that walks to the root by default.
- R2.3 The query reads parent ids and commit time from the walk without decoding
  full commit objects, except where the summary genuinely needs the object.
- R2.4 The query is cancellable: an abandoned query stops walking rather than
  running to completion and discarding its result.
- R2.5 Paging within one scroll costs O(limit), not O(page index x limit).
  Added 2026-09-15. Phase 02's cursor resumes by replaying the walk from pinned
  tips — correct by construction, and the reason two pages of N equal one page
  of 2N — but page *k* then walks *k x limit* commits: 1.29-2.1 s for one page at
  depth 500k, and 54-87 minutes of CPU to page there at 100 rows a page. A7 is
  unreachable through replay alone. The engine therefore keeps a walk alive for
  the life of a scroll; R2.2's cursor remains the cold-restart path for when no
  session exists. gitoxide's walk borrows the repository and is not `Send`, so
  the session lives on the worker that owns that repository handle (R3.1) and
  never crosses a thread. It must compose with R3.2's epochs: superseding a
  request may not leave a half-consumed walk to be read by the next one.

  NOT in scope, filed instead — issue #5: random access by row offset. There is no total
  row count (counting is a full walk) and no way to build a cursor from an
  offset, so a scrollbar drag has no answer. Progressive loading is what Fork
  and Sourcetree do here.

### R3 — Repository work runs on a worker pool

- R3.1 One `gix::ThreadSafeRepository` per open repository; each worker thread
  takes its own handle via `to_thread_local()` exactly once.
- R3.2 Every request carries an epoch. A response whose epoch is stale is
  discarded without rendering.
- R3.3 No repository call is reachable from a component's render path or from an
  event handler that waits for it.

### R4 — The view is virtualised and legible

- R4.1 Only visible rows are rendered; scrolling a repository with a very large
  history does not grow memory or frame time with history length.
- R4.2 Lanes and edges are drawn — merges and branches are visually followable,
  not implied by indentation.
- R4.3 An in-progress load is visible as such; the view never shows an empty list
  that is indistinguishable from an empty repository.
- R4.4 Selecting a row is keyboard reachable, and selection survives more rows
  arriving.

### R5 — Something opens a repository

The packet cannot render history without a repository, and nothing in Cairn opens
one today. The minimum that unblocks it, deliberately not more:

- R5.1 The app opens the repository containing the path given as its first
  command-line argument, defaulting to the process working directory.
- R5.2 A path that is not inside a repository fails with a message naming the
  path, not a panic and not an empty window.
- R5.3 Exactly one repository is open at a time. Choosing between tabs, a
  sidebar and separate windows is parked in the spine's "Still open" and must
  NOT be decided here — a command-line argument is chosen precisely because it
  commits to nothing.

### R6 — A row is a list entry, not by definition a commit

Added 2026-09-16, before the view was built, because this is the one shape that
is expensive to change once consumers exist.

- R6.1 The history is a sequence of rows. Every row carries its lane and edge
  segments (R1.1) plus a **stable identity**: what selection survives on, and what
  a detail pane is opened from.
- R6.2 A row's content is a commit in this packet, and the type admits content
  that is not one. The working-tree row that Sourcetree shows above the first
  commit sits *in* the graph — it occupies a lane and lines pass it — so it is
  laid out by the engine, not decorated by the view. `refs-and-status` adds that
  variant; this packet emits only commits. Deciding it now costs one enum;
  deciding it after the view, the worker and the app wiring consume rows costs
  all three.
- R6.3 Decoration a row may later carry — ref labels, ahead/behind counts,
  whether the commit is on the checked-out branch — is added as **fields** by the
  packets that own them. Fields are additive and keep every consumer compiling,
  so none of them are reserved here.
- R6.4 Rows are not required to be one-to-one with commits. Collapsing merges
  (Fork's remedy for a wide graph, measured taking 21 lanes to 3) removes rows
  without removing commits, so the engine emits rows, not commits.

## Product rules

- The graph is the default view when a repository opens.
- Row height is uniform. Variable-height rows and virtualisation together are a
  trap this packet does not walk into.
- Colour distinguishes lanes but never *carries* meaning alone; lane identity must
  survive a monochrome screenshot and a colour-vision deficiency.
- No operation in this packet mutates a repository. If a design pulls toward one,
  it is out of scope and gets filed.

## Acceptance criteria

The single authoritative copy. `docs/work/history-graph/qa-checklist.md` points
here and does not restate them.

| # | Criterion | Pinned by |
| --- | --- | --- |
| A1 | Lane assignment produces the expected lanes and edges for a fixture set covering: linear history, a simple branch and merge, an octopus merge, criss-cross merges, and multiple roots | unit tests on the pure assigner |
| A2 | Feeding the assigner a deliberately skewed history — a parent whose committer date is newer than its child's — produces rows for every commit, with every parent edge present | a named regression test built from the evidence record's finding 2 |
| A3 | Lane indices for the first N commits are identical whether N or N+M commits were assigned; any edge that differs does so only by gaining a segment for a parent that arrived late | property test |
| A4 | The history query returns correct rows for a fixture repository built by running real `git` commands, and honours its limit | integration test in `cairn-git` |
| A5 | A cancelled query stops walking — observable, not asserted by comment | integration test |
| A6 | No engine call is reachable from a render path | `responsiveness-reviewer`, plus the existing dependency-seal guards |
| A7 | Scrolling a repository with at least 100k commits keeps frame time bounded and memory flat | a measured check, run by hand against a named real repository, with numbers recorded in `progress.md` |
| A8 | The app opens the repository named on the command line, defaults to the working directory, and fails with a clear message when given a path outside a repository | integration test over the argument handling, plus a manual run |
| A9 | A row's content is expressible as something other than a commit, and every consumer matches on it rather than assuming one | unit test in `cairn-model`, plus the app's exhaustive render match in `cairn-app` |
| A10 | `scripts/gate.sh` passes | the gate |

A8 is the new one — R5 was missing from the first draft of this PRD, which
specified a view with nothing to point it at. A7 is deliberately not automated. A frame-time assertion in CI would be flaky and
would be disabled within a month; a recorded measurement against a named
repository is honest about what it is.

## Out of scope

Filed, not done: commit detail panes, diffs, blame, file history, search and
filtering, graph-based operations (checkout, reset, cherry-pick from a row),
multiple repositories open at once, a repository picker or manager of any kind
(R5.3), recent-repository history, and any mutation whatsoever.
