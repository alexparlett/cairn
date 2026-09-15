# State — history-graph

The cross-session cheat sheet. Every session updates this before ending.

**Status: phases 01, 02 and 03 implemented on `feature/history-graph`, unmerged.
The lane assigner is bounded and lives in `cairn-model`; `cairn-git` answers a
bounded, resumable, cancellable history query and keeps a walk alive across a
scroll; `cairn-app` runs both off the UI thread behind a boundary a guard now
pins.** Correction to this line as phase 01 left it: R1.3 and R1.2's finality
sentence are no longer TRAP-marked in the PRD — the user narrowed both on
2026-09-15 and phase 02 built the window they describe.

**Still open for the user.** Phase 02 raised three; the first is built, the other
two stand, and phase 03 adds a correction to one of them.

- ~~**Resumption does not scale to the size A7 names.**~~ **Settled and built.**
  R2.5's live walk session shipped in phase 03: `Repository::history_session`
  holds gitoxide's walk for the life of a scroll, so paging costs O(limit). The
  cursor survives as the cold-restart path.
- **The assigner's remaining blind spot — still open, and the proposed cure does
  not exist.** A parent delivered more than `window + remembered` rows before its
  child cannot be told from one already gone. The decision of 2026-09-15 said the
  live session closes this "exactly", because the walk already holds the seen-set
  that answers it. **It does not, and cannot: gitoxide keeps that set inside the
  `Box<dyn Iterator>` behind `gix::revision::Walk` and exposes no accessor**
  (`gix-0.87.1/src/revision/walk.rs`, `iter_impl`). Holding it would mean keeping
  a second walk-sized copy — the shape R1.3 exists to forbid. So the blind spot
  is unchanged from phase 02, and the PRD's R1.3 note saying phase 03 closes it
  is now wrong. The user's call, not an agent's.
- **R1.3's bound is not the true worst case.** Upward repaints run down lanes
  that are not in the assigner's lane table at all, so per-row segments are
  O(window) and retained state O(window^2) on a history with no branching
  whatsoever. Measured: window 256, one late parent per row, 129 segments on a
  single row. The requirement's wording needs the user, not an agent.

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

O4 in `brainstorm.md`. **O1 is resolved**: L9 settled it before phase 01
started — a bounded reordering window can always be exceeded by larger skew, so
the total assigner is the floor rather than one of two options. Phase 01 built
that total assigner and phase 02 bounded it. **O2 is resolved** by measurement in
phase 02. **O3 is resolved** by measurement in phase 03: one worker per open
repository, `WORKERS_PER_REPOSITORY` in `crates/cairn-app/src/worker/pool.rs` —
and note that the measurement contradicted the question's own premise, since
concurrent walks scaled 7.9x on 8 threads rather than oversubscribing. The reason
for one worker is structural (a live walk cannot be split across threads), not
throughput. Numbers and caveats in `progress.md`. O4 is now a visual question
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
| `Repository::OBJECT_CACHE_BYTES` | `cairn-git` | 4 MiB, installed by every worker handle. Measured: a commit-time walk of 50k commits costs 178 ms without it and 116 ms with it; more bought nothing. |
| `SharedRepository` | `cairn-git` | One open repository, shareable between threads: gitoxide's `ThreadSafeRepository` with the paths cached. `Send + Sync` — pinned by `a_shared_repository_can_cross_threads`, which is also the twin for gix's `parallel` feature staying on. |
| `SharedRepository::to_worker` | `cairn-git` | One worker thread's `Repository`. **Call once per thread, at its start.** Per-request conversion compiles and passes every test while rebuilding the object cache and the pack snapshot each time; the cache is installed here because gitoxide keeps it on the handle, not on the store. |
| `Repository::discover` | `cairn-git` | Unchanged signature, now `SharedRepository::discover(..).to_worker()`. The single-thread route, for tests and callers that will never hand the repository to a worker. |
| `Repository::history_session` | `cairn-git` | `(&HistoryRequest) -> Result<HistorySession<'_>, Error>`. Opens a walk and keeps it. The request's limit is ignored; `next_page` carries it. Borrows the repository, so it cannot leave the thread that owns the handle. |
| `HistorySession::next_page` | `cairn-git` | `(limit, &impl Cancel) -> Result<HistoryPage, Error>`, O(limit) after a one-off prime of `window` commits. **Cancellation keeps the session's progress** — the rows walked stay inside it, so the next call continues rather than re-walking, and no half-consumed walk is left behind. `decoded` never exceeds the rows returned: a commit object is read when its row is handed out, not when it is walked, so priming the window costs walk steps and no object reads. |
| `HistorySession::cursor` | `cairn-git` | The cold-restart handle: a `HistoryCursor` standing where the session does. Hand it to `HistoryRequest::resume` when the session is gone; paging carries on at the replay cost the session exists to avoid. |
| `HistorySession::{delivered, is_exhausted}` | `cairn-git` | What the session has handed out, and whether the history ran out and every row it produced has been handed on. |
| `worker::open` | `cairn-app` | `(path) -> Result<(RepositoryHandle, Updates), OpenError>`. Starts the worker; **the worker opens the repository**, because discovering one reads the filesystem and the UI thread may not. A path outside a repository therefore arrives as an `Update::Failed`, and the only way this call fails is that the OS refused a thread. `open`, `Request` and `Update` are the only three names the module exports — the rest are reachable through this signature and cited below by file, which is what the anchor rule asks when a path does not resolve. |
| `RepositoryHandle` (`crates/cairn-app/src/worker/pool.rs`) | `cairn-app` | What a component holds. Exactly one method — `submit(Request) -> Epoch` — which returns immediately, over an unbounded channel. It holds no receiving end of anything, so there is nothing on it to wait on. |
| `Updates::next` (`crates/cairn-app/src/worker/pool.rs`) | `cairn-app` | `async fn() -> Option<Update>`. The one place a stale answer is dropped (R3.2); an update carrying no epoch — a worker dying — is never dropped. `None` means every worker has gone. |
| `worker::Request` | `cairn-app` | `OpenHistory { rows }` starts a scroll and abandons any open walk; `MoreHistory { rows }` continues it, falling back to the cold cursor when no walk is open, so it is never an error. |
| `worker::Update` | `cairn-app` | `Rows { rows, complete }`, `Failed { message }`, `WorkerLost { message }`. Only `cairn-model` vocabulary: an engine error crosses as the sentence it will be shown as. |
| `Epochs` / `Superseded` (`crates/cairn-app/src/worker/epoch.rs`) | `cairn-app` | The epoch IS the cancel signal: `Superseded` implements `cairn_git::Cancel` as "is my epoch still current". Superseding stops the walk at the next commit rather than discarding a finished answer. |
| `WORKERS_PER_REPOSITORY` | `cairn-app` | 1, by O3. A `const` assertion fails the build if it is raised, because the job channel has one consumer and the live walk lives on it — more workers need a routing decision, not a bigger number. |
| `cairn_guards::waits_on_work` | `cairn-guards` | 1-based lines where source waits or spins. Bare identifiers for the things you must name to alias — the types, the channel CONSTRUCTORS (a receiver can be held without naming one), and the spinning spellings (`try_recv`, `spin_loop`, ...); `join`/`lock`/`recv`/`wait` only when called with NO arguments, which is what tells `handle.join()` from `root.join("crates")`. Reads across newlines, ignores string literals. |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 lane assignment | implemented | `scripts/gate.sh` green | see progress.md's phase 01 QA entry |
| 02 history query | implemented | `scripts/gate.sh` green | four fresh agents, adjudicated by `qa-confirm`; see progress.md's newest entry |
| 03 worker boundary | implemented | `scripts/gate.sh` green | four fresh agents, adjudicated by `qa-confirm`; then a mutation-executing coverage audit whose findings are closed — see progress.md's newest entry, which also names the two gaps held for the scroll/memory design pass |
| 04 graph view | not started | — | — |
| 05 QA | not started | — | — |

Phase 04 owes the virtualization invariant a real twin, and phase 03 is the
change that made it live. `cairn-app` builds one component per element of the
row vector with a full `CommitSummary` clone each — measured by the
`responsiveness-reviewer` at roughly 1,300 allocations and 30 KB copied per
render at the current cap — and it re-runs on every reactive change, including a
selection click that alters one row. The correction to the line this file used
to carry: it is no longer "inert because nothing populates that vector". The
query is wired, the vector fills, and what keeps it bounded is a CAP
(`ROWS_DRAWN`, 256) rather than virtualisation. Lifting the cap without
virtualising is the defect arriving; the clone belongs inside a virtualised
viewport's row builder, where one clone per VISIBLE row is fine.

Phase 04 owes R5: opening a repository from a command-line argument, and nothing
more than that. `main.rs` currently opens the process working directory, which is
R5.1's default and none of the rest of R5.

## Which parts of the worker interface exist for fetch

Phase 03 read `docs/prd/credential-prompts.md` R4 and shaped the boundary so a
long-running, progress-reporting operation that blocks mid-flight on a UI dialog
fits without every call site changing. **Nothing of fetch is built or stubbed.**
Three properties are there for it rather than for the graph:

1. **A request is answered by a stream, not a reply.** `worker::Update` is sent
   by a worker as many times as it likes before a job ends, and `Updates::next`
   is a loop over arrivals rather than a one-shot. The history job happens to
   send one update per request. Progress reporting (R4.3) is therefore an added
   `Update` variant and an added send — not a changed shape. No `Progress`
   variant exists today on purpose: an unused variant is dead code the gate
   rejects, and pre-building one would be abstracting for a hypothetical.
2. **A worker runs ordinary blocking code, so asking the UI a question needs no
   pool surface at all.** A job that must wait for a credential (R2.1, R2.5)
   makes its own reply channel, sends an `Update` carrying the sending half, and
   blocks on the receiving half — blocking a worker is what workers are for. The
   thing that would have made this impossible is a pool that owned the
   request/response cycle; this one does not.
3. **Workers are pinned to a purpose, not fed from an anonymous queue.**
   `WORKERS_PER_REPOSITORY` is 1 because a scroll's live walk lives on one
   thread. Fetch gets its OWN worker rather than a slot in this queue — otherwise
   a password prompt would stall the graph behind it — and O3's measurement says
   a second worker costs the first almost nothing (7.9x scaling at 8 threads).

What is NOT designed for fetch, and should not be assumed: `Request` and `Update`
are one enum each per direction, so adding fetch adds variants to both. That is
deliberate (a trait-object job cannot hold the live walk's borrow), and it is a
two-file change, not a call-site sweep.

**Constraints phase 02 handed phase 03** — the first is discharged, the rest stand:

- ~~`Repository` has no `ThreadSafeRepository` route, so R3.1 needs new public
  surface in `cairn-git`.~~ Built: `SharedRepository` + `to_worker`.
- gitoxide's own walk holds a `HashSet<ObjectId>` of every commit it visits
  (`gix-traverse`'s `simple::Simple`), about 15 MB at 500k commits. A live
  session now keeps exactly one of these for a whole scroll instead of rebuilding
  it per page — which is a further reason the session pays, and also the memory
  a long scroll costs. It is not exposed, which is why the assigner's blind spot
  stays open.
- A `Repository::history` page decodes one commit object per row in its `limit`,
  not per visible row. Keep `limit` to two or three screens: on a cold page
  cache each object read can be a pack seek. (A `HistorySession` page decodes
  one per row it HANDS OUT, which is the narrower promise — phase 03 changed
  that half.)
- Cancellation discards the page rather than returning what it had. That is the
  specified contract (R2.4, D3), so a worker that wants partial results must
  debounce instead. (A `HistorySession` keeps its progress instead of discarding
  it — phase 03 changed that half too, and it is what lets a superseded scroll
  resume rather than restart. The caller still gets no rows from the cancelled
  call, so "debounce" is still the advice.)
- `HistoryRow` is plain data with no cheap-clone handle. Hand pages across as
  owned values, and inside the view share rather than clone per render.
- `HistoryRequest::from_commits` takes an unbounded tip set, and every page
  resolves and copies all of it: `limit` bounds commits walked, not tips. Free
  at 40 branches, 40,000 tip resolutions per page at a ref-heavy remote. Raised
  by the `responsiveness-reviewer` during the `Oid` change and not fixed there.

**Constraints phase 03 hands phase 04:**

- **The first page of a scroll WALKS `limit + window` commits, every later page
  walks `limit`, and neither decodes more commit objects than it returns rows.**
  Rows are handed out only once the assigner has evicted them, which is what
  makes them final. At the default window that is 1024 extra walk steps once per
  scroll — steps, not object reads. Do not shrink `limit` to make the first page
  feel faster; shrink the window, and measure.
- **Ask for the next page, never for a row range.** `Request::MoreHistory`
  continues the open walk. There is no total row count and no offset-to-cursor
  route (filed, not built), so a scrollbar drag has no answer — progressive
  loading is the model, as it is in Fork and Sourcetree.
- **`main.rs` renders a bounded 256 rows and is not virtualised.** A cap is not
  virtualisation. R4.1 is phase 04's, and the `ROWS_DRAWN`/`PAGE_ROWS` constants
  and the `.take(ROWS_DRAWN)` in `app()` are what it replaces.
- **Only `crates/cairn-app/src/worker/` may name `cairn_git` or wait.** A new
  file in `cairn-app` that needs repository data asks for it through a
  `worker::Request`; the guard fails the build otherwise, naming the line.
- **Selection survives more rows arriving (R4.4) is not free.** Rows append to
  one `Vec` and the placeholder selects by index, which holds only because rows
  are appended and never renumbered. If phase 04 selects by index, say so; if it
  selects by `Oid`, that is the safer reading.
- **Every `submit` supersedes, and a superseded page delivers NOTHING.** That is
  correct — the walk stops and its progress stays in the session — but it means
  a scroll handler that submits on every tick cancels the in-flight page every
  tick and the view receives no rows until the user stops moving. **Debounce.**
  Raised by the `responsiveness-reviewer`; not a live defect today, because
  exactly one request is ever outstanding.
- **A live session's memory grows with rows SCROLLED, and is never released
  while the app runs.** gitoxide's walk keeps a `HashSet<ObjectId>` of every
  commit visited (about 20 MB at 500k rows) and the assigner keeps its window;
  the worker holds the session until the next `OpenHistory` or a walk error.
  A7 asks for memory flat in history LENGTH, which this is — but it is linear in
  depth scrolled, and there is no idle drop. Measure it, and if it needs a cap,
  that is a decision with the user.
- **An engine error reaches the view as a sentence, not a type.**
  `Update::Failed` carries a `String` already rendered from `cairn_git::Error`.
  If phase 04 needs to branch on *which* failure it was — R5.2 wants a path named
  and R4.3 wants loading distinguished from empty — that is a new `Update`
  variant, decided there.

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
