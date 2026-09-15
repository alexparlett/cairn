# State — history-graph

The cross-session cheat sheet. Every session updates this before ending.

**Status: phases 01 and 02 implemented on `feature/history-graph`, unmerged. The
lane assigner is bounded and lives in `cairn-model`; `cairn-git` answers a
bounded, resumable, cancellable history query against a real repository.**
Correction to this line as phase 01 left it: R1.3 and R1.2's finality sentence
are no longer TRAP-marked in the PRD — the user narrowed both on 2026-09-15 and
phase 02 built the window they describe.

**Open for the user before phase 03 designs the worker.** Three decisions, all
raised by phase 02's QA and none of them an agent's to take. They are stated in
full in `progress.md`'s phase 02 decisions entry; in one line each. (The fourth,
`Oid`'s representation, is settled and built — see below.)

- **Resumption does not scale to the size A7 names.** A page resumes by
  replaying the walk, so page *k* walks *k x limit* commits: measured 258-420 ms
  for one page at depth 100k and 1.29-2.1 s at 500k, and 54-87 minutes of CPU to
  page sequentially to 500k at 100 rows a page. R2.2 is met; A7 is not reachable
  through this API. The proposed cure keeps gitoxide's walk alive inside phase
  03's worker as a session, demoting today's cursor to the cold-restart path —
  which changes `cairn-git`'s public surface, so phase 03 must not start until
  this is settled.
- **The assigner's remaining blind spot.** A parent delivered more than
  `window + remembered` rows before its child cannot be told from one still to
  come without remembering every commit walked, which R1.3 forbids. Today's
  answer is to remember ids far past the window; the exact answer would put a
  walk-sized set in the read path — where gitoxide already keeps one.
- **R1.3's bound is not the true worst case.** Upward repaints run down lanes
  that are not in the assigner's lane table at all, so per-row segments are
  O(window) and retained state O(window^2) on a history with no branching
  whatsoever. Measured: window 256, one late parent per row, 129 segments on a
  single row. The requirement's wording needs the user, not an agent.
Phase 04's A7 depends on the first of these.

**Settled and built: `Oid` is fixed-width.** The digest itself — 20 bytes or 32,
plus the width — and `Copy`; text comes from `Oid::hex` / `Oid::short` as an
`OidHex` stack buffer. It bought about 15% off the query path end to end and
about 18% off a 200-lane row, and no allocation per id. It did NOT do what its
justification said: the 1-lane-to-200-lane ratio was 8.2x before and about 7x
after, so the string comparison was never the term behind that gap. Correction
to the line this file used to carry: the quoted 12.6x could not be reproduced,
and the gap is dominated by per-row work proportional to open lanes — one
passing segment built and pushed per open lane per row — which no id
representation touches. See `progress.md`'s newest entry for both harnesses.

## Locked decisions

L1-L10 in `brainstorm.md`; the design-level frame is D3 and D4 in
`docs/design/cairn.md`. The three that most constrain implementation:

- Lanes are computed in `cairn-git` and travel as `cairn-model` values (L1).
- The assigner must be correct when a parent arrives before its child (L3) —
  this is normal, not corruption. See the evidence record.
- The total assigner is the floor, not an option (L9), and R1.2's stability
  covers lane INDICES only, so edges may repaint inside the loaded window (L10).

## Open questions

O3 and O4 in `brainstorm.md`. **O1 is resolved**: L9 settled it before phase 01
started — a bounded reordering window can always be exceeded by larger skew, so
the total assigner is the floor rather than one of two options. Phase 01 built
that total assigner and phase 02 bounded it. **O2 is resolved** by measurement in
phase 02: see `progress.md`. O3 is a measurement; O4 is now a visual question
rather than a structural one, thanks to L10.

### How phase 01 answered R1.4

R1.4 requires a placement strategy and a justification for it. The assigner
places a commit that has no lane reserved for it in the leftmost free lane, and
when a child for it arrives later it draws the joining line *upward*, through a
lane that was free on every row in between, flagging every segment of that line
`out_of_order`. This is the evidence record's candidate 1 plus the rendering rule
it calls for: the line is complete rather than jumping, and the flag is what lets
phase 04 mark a link that runs backwards on screen instead of drawing time in
reverse without comment.

## New modules and interfaces introduced so far

As phases land, record here: the type or function, its crate, and the one-line
contract — so a later phase does not re-derive it from source.

| Symbol | Crate | Contract |
| --- | --- | --- |
| `Lane` | `cairn-model` | A vertical track in the graph, numbered from the left. `Lane::index()` is final once a row is emitted. |
| `EdgeKind` | `cairn-model` | Whether a segment passes a row by (`Passing`), ends at its commit (`IntoCommit`) or starts there (`OutOfCommit`). The whole geometric vocabulary; genealogy is not in it. |
| `EdgeSegment` | `cairn-model` | One piece of a connecting line clipped to one row: `from`/`to` lanes, `kind`, and `out_of_order` for a line joining a commit to a parent drawn above it. |
| `GraphRow` | `cairn-model` | One commit's line: `id`, `lane`, and every segment crossing the row. Self-contained, so drawing row N needs only row N. |
| `LaneAssigner` | `cairn-model` | Lays commits out in lanes from `(id, parent_ids)` in walk order. Pure: no gix, no I/O, no clock. |
| `LaneAssigner::push` | `cairn-model` | Lay out one commit. Returns the row this push pushed out of the window, if any — that row is final and the assigner no longer holds it. (Phase 01's entry said "returns nothing"; the window changed it.) |
| `LaneAssigner::with_window` / `window` | `cairn-model` | An assigner holding at most `window` rows. Widening is superlinear — measured 66 ms at 1024, 749 ms at 4096, 23.3 s at 16384 over 50k skewed rows — so widen on measurement, never as a precaution. |
| `LaneAssigner::DEFAULT_WINDOW` | `cairn-model` | 1024 rows. Sized against skew measured in ROWS; see the open decision above, because skew is naturally measured in time. |
| `LaneAssigner::remembered` | `cairn-model` | How many commits past the window the assigner still recognises by id — 16 per row of window. Ids are cheap where rows are not, and recognising a parent that has already gone by is what stops a lane being reserved for a commit that can never arrive. |
| `LaneAssigner::rows` / `into_rows` | `cairn-model` | The rows still inside the window, oldest first. Rows already made final are not here: `push` handed those back. |
| `LaneAssigner::assign_all` | `cairn-model` | Lay out a whole walk in one call, returning every row — the ones the window made final and the ones left inside it. |
| `HistoryRow` | `cairn-model` | One line of history: `commit: CommitSummary` + `graph: GraphRow`. `id()` reads the commit half. The pairing is made once, by whoever built the row. |
| `Repository::history` | `cairn-git` | One page of history, laid out in lanes: `(&HistoryRequest, &impl Cancel) -> Result<HistoryPage, Error>`. Synchronous; the caller decides what thread it runs on. |
| `HistoryRequest` | `cairn-git` | `from_head(limit)`, `from_commits(tips, limit)`, `resume(cursor, limit)`, `.with_order(..)`, `.with_window(..)`. The last two are ignored when resuming: both decide lane numbering, and the cursor carries what its own rows were laid out under. |
| `HistoryOrder` | `cairn-git` | `CommitTime` (the default, by measurement — O2) or `GraphOrder`. Neither is topological; both can hand over a parent before its child. |
| `HistoryCursor` | `cairn-git` | Opaque continuation: resolved tips, order, window, rows behind. Not tied to a repository, and does not shift when the repository gains commits, because it replays from pinned tips. |
| `HistoryPage` | `cairn-git` | `rows`, `cursor`, `walked`, `decoded`. `walked` exceeds `rows.len()` on a resumed page because resuming replays; `decoded` never does, which is R2.3 made observable. |
| `Cancel` / `CancelSignal` | `cairn-git` | A trait polled once per commit, and an `Arc<AtomicBool>` implementing it. A trait so a test can stop a walk at a chosen commit; `cairn-git` still knows nothing about threads. |
| `Error::{Cancelled, UnbornHead, Walk, ReadCommit}` | `cairn-git` | Cancelled carries how far the walk got. ReadCommit names the commit, so a view can show the rest of the page. |
| `Oid` | `cairn-model` | A git object id as the digest itself: 20 bytes (SHA-1) or 32 (SHA-256) plus the width that keeps them apart, `Copy`, no heap. Built by `Oid::parse` (hex, either case) or `Oid::from_bytes` (raw digest, which is how the engine crosses from gitoxide). `as_bytes` is the digest; comparing and hashing read bytes already in the row. |
| `Oid::hex` / `Oid::short` | `cairn-model` | Text on demand, into an `OidHex` buffer the CALLER owns — not `&str` any more, because there is no string to borrow. `short()` is 7 characters and cannot panic on a short id. A component that must own its text (Freya's `label().text()` takes `Cow<'static, str>`) copies out with `.short().as_str().to_string()`. |
| `OidHex` | `cairn-model` | Hex characters held inline, `Copy`, canonical (nothing past the length it reports). `as_str()` borrows from it, so it must outlive the borrow — `let s = oid.short().as_str();` is a compile error, by design. |
| `Repository::OBJECT_CACHE_BYTES` | `cairn-git` | 4 MiB, installed by `discover`. Measured: a commit-time walk of 50k commits costs 178 ms without it and 116 ms with it; more bought nothing. |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 lane assignment | implemented | `scripts/gate.sh` green | see progress.md's phase 01 QA entry |
| 02 history query | implemented | `scripts/gate.sh` green | four fresh agents, adjudicated by `qa-confirm`; see progress.md's newest entry |
| 03 worker boundary | not started | — | — |
| 04 graph view | not started | — | — |
| 05 QA | not started | — | — |

Phase 04 owes the virtualization invariant a real twin: `cairn-app` builds one
component per element of the whole commit vector, with a full `CommitSummary`
clone each. It is inert today because nothing populates that vector, and it
becomes the unbounded-list defect the moment the query is wired — raised by the
`responsiveness-reviewer` during the `Oid` change, out of scope there.

Phase 03 additionally owes design notes here saying which parts of the worker
interface exist for fetch (`docs/prd/credential-prompts.md` R4) rather than for
the graph. Phase 04 owes R5: opening a repository from a command-line argument,
and nothing more than that.

**Constraints phase 02 hands phase 03**, beyond the open decisions above:

- `Repository` holds a `gix::Repository`, which is a thread-local handle. There
  is no `ThreadSafeRepository` route and no constructor from a shared one, so
  **R3.1 cannot be met without new public surface in `cairn-git`**. Today N
  workers would mean N `discover` calls: N object databases, N ref stores, N
  object caches, N sets of pack-index mmaps.
- gitoxide's own walk holds a `HashSet<ObjectId>` of every commit it visits
  (`gix-traverse`'s `simple::Simple`), about 15 MB at 500k commits, and today's
  replay rebuilds it per page.
- A page decodes one commit object per row in its `limit`, not per visible row.
  Keep `limit` to two or three screens: on a cold page cache each object read
  can be a pack seek.
- Cancellation discards the page rather than returning what it had. That is the
  specified contract (R2.4, D3), so a worker that wants partial results must
  debounce instead.
- `HistoryRow` is plain data with no cheap-clone handle. Hand pages across as
  owned values, and inside the view share rather than clone per render.
- `HistoryRequest::from_commits` takes an unbounded tip set, and every page
  resolves and copies all of it: `limit` bounds commits walked, not tips. Free
  at 40 branches, 40,000 tip resolutions per page at a ref-heavy remote. Raised
  by the `responsiveness-reviewer` during the `Oid` change and not fixed there.

## Environment notes

- `freya` comes from OUR FORK (github.com/alexparlett/freya), pinned by commit in
  the root `Cargo.toml` since 2026-09-15 — not from crates.io. Verify every Freya
  API against the fork: the clone at `/home/alexparlett/Development/freya` or the
  pinned rev under `~/.cargo/git/checkouts/`, never `~/.cargo/registry/src/`, and
  never from memory. It uses a builder API; `rsx!` examples are the old one.
  A Freya limitation gets FIXED IN THE FORK, not worked around in Cairn — patch
  the git source to the local clone while the fix is in flight, and never commit
  that patch as the shipping build.
- `gix` 0.87.1 is pre-1.0 and still comes from crates.io. Verify its APIs against
  the vendored source under `~/.cargo/registry/src/`, never from memory.
- `scripts/gate.sh` is the bar. Never an ad-hoc `&&` chain, never piped through
  `tail` — that masks the exit code.
- Commit explicit paths, never `git add -A`.
- A new crate, dependency or invariant needs its row in
  `crates/cairn-guards/tests/invariants.rs` in the same commit, or the gate fails.
- The repository HAS a remote: `origin` is `git@github.com:alexparlett/cairn.git`,
  and `feature/history-graph` is pushed to it. (This line used to say there was
  none.) Merging is still the user's alone.
- Work in a linked worktree (`.claude/worktrees/<packet>`). A linked worktree's
  git dir is `.git/worktrees/<name>`, not `.git` — phase 01 had to fix a
  `cairn-git` test that assumed otherwise. Assert that a path IS a git directory,
  never what it is called.
