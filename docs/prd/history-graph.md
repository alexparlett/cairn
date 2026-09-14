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
  emits one row per commit carrying its lane index and the edge segments crossing
  that row.
- R1.2 Processing further commits never changes a lane already emitted.
- R1.3 Work per commit is amortised constant; retained state is proportional to
  the number of simultaneously open lanes, never to the number of commits seen.
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
| A3 | Emitted lanes are stable: assigning N commits then N more yields byte-identical rows for the first N | property test |
| A4 | The history query returns correct rows for a fixture repository built by running real `git` commands, and honours its limit | integration test in `cairn-git` |
| A5 | A cancelled query stops walking — observable, not asserted by comment | integration test |
| A6 | No engine call is reachable from a render path | `responsiveness-reviewer`, plus the existing dependency-seal guards |
| A7 | Scrolling a repository with at least 100k commits keeps frame time bounded and memory flat | a measured check, run by hand against a named real repository, with numbers recorded in `progress.md` |
| A8 | `scripts/gate.sh` passes | the gate |

A7 is deliberately not automated. A frame-time assertion in CI would be flaky and
would be disabled within a month; a recorded measurement against a named
repository is honest about what it is.

## Out of scope

Filed, not done: commit detail panes, diffs, blame, file history, search and
filtering, graph-based operations (checkout, reset, cherry-pick from a row),
multiple repositories open at once, and any mutation whatsoever.
