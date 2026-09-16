# State — history-graph

The cross-session cheat sheet. Every session updates this before ending.

**Status: phases 01-05 complete on `feature/history-graph`, unmerged. The packet
is at its merge bar and READY; teardown and the packet PR to `main` are the
user's, and no agent merges either.** The lane assigner is bounded and lives in `cairn-model`;
`cairn-git` answers a bounded, resumable, cancellable history query and keeps a
walk alive across a scroll; `cairn-app` runs both off the UI thread behind a
boundary a guard pins; and `cairn-ui` draws the result as a virtualised graph
with lanes, edges, four columns and keyboard selection, over the repository named
on the command line. Phase 05 ran the merge bar over the whole diff: five fresh
agents, 16 confirmed findings fixed, A1-A10 each verified against a test that
decides it, all four guards proven by violation, and `scripts/gate.sh` green as
one command. `docs/systems/history-graph.md` is written.

Corrections this line has collected: R1.3 and R1.2's finality sentence are no
longer TRAP-marked in the PRD — the user narrowed both on 2026-09-15 and phase 02
built the window they describe.

**Still open for the user.** Phase 02 raised three; the first is built, the other
two stand, phase 03 adds a correction to one of them, R6's QA raises one more,
and phase 05 adds two — `crates/cairn-ui/src/commit_row.rs` has no test and
cannot get a meaningful one without the `freya-testing` decision (its four-column
header/row mismatch is undetectable today), and `.claude/hooks/qa-stop.sh`'s
remedy text names `tracing`, a crate this workspace does not depend on, which is
either a dependency decision or a rewording. That hook line was escalated in
phase 02's progress entry and again in phase 03's and has never been closed.

Two things that were claimed filed and were not now are: **issue #4** (bound or
evict the retained history rows — the one measured limitation of A7) and
**issue #5** (what the scrollbar does without random access by row offset,
which is what R2.5 puts out of scope).

- **Nothing stops a wildcard arm over `RowContent`, and A9's "pinned by" column
  names a component that should not name it.** Raised by `qa-checklist` and the
  `test-coverage-auditor` over the R6 change, confirmed by `qa-confirm` as the
  user's call, two halves. (a) A `_ =>` arm over `RowContent` is a hard error
  today only because a single-variant enum makes it `unreachable_patterns` under
  `-D warnings`; the moment `refs-and-status` lands variant two, both `_ =>` and
  `if let RowContent::Commit(..)` become legal and silently undraw a row — which
  is the exact failure the enum exists to prevent. Does "no wildcard over
  `RowContent`" become a CLAUDE.md invariant with a source-scanning guard twin
  in `crates/cairn-guards/tests/invariants.rs`, or a review obligation with a
  named owner in `docs/qa-gate.md`? Either is a user decision, not an agent's.
  (b) A9 says it is pinned by "unit test in `cairn-model`, plus the view's row
  component", and `cairn_ui::CommitRow` takes a `CommitSummary` and never names
  `HistoryRow` or `RowContent` — deliberately, because a component draws one
  kind of thing. The match lives in `cairn-app`'s render instead. Proposed
  amendment to the PRD, for the user to approve: "unit test in `cairn-model`,
  plus the app's exhaustive render match in `cairn-app/src/main.rs`".
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

**O4 is resolved** by phase 04, and the question turned out to be smaller than it
was framed as: an out-of-order line is drawn **dashed along its whole length, in
the lane's own colour, with its geometry unchanged**. Dash rather than colour,
because the product rules say colour never carries meaning alone and a dash
survives a monochrome screenshot and every colour-vision deficiency; geometry
unchanged, because the connection is not a different KIND of connection, only its
ancestry runs the other way. The correction to the question itself: **a view
never observes the repaint O4 was about.** `HistorySession::next_page` hands out
only rows the assigner has already evicted, and `connect_upward` repaints only
rows still inside the window, so an out-of-order line is complete before its rows
are delivered. Evidence: 5 rows of 2,896 (0.173%) in the default order over every
ref of `/home/alexparlett/Development/freya` — see `progress.md`.

**O1 is resolved**: L9 settled it before phase 01
started — a bounded reordering window can always be exceeded by larger skew, so
the total assigner is the floor rather than one of two options. Phase 01 built
that total assigner and phase 02 bounded it. **O2 is resolved** by measurement in
phase 02. **O3 is resolved** by measurement in phase 03: one worker per open
repository, `WORKERS_PER_REPOSITORY` in `crates/cairn-app/src/worker/pool.rs` —
and note that the measurement contradicted the question's own premise, since
concurrent walks scaled 7.9x on 8 threads rather than oversubscribing. The reason
for one worker is structural (a live walk cannot be split across threads), not
throughput. Numbers and caveats in `progress.md`.

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
| `HistoryRow` | `cairn-model` | One line of history: `content: RowContent` + `graph: GraphRow`. `id()` derives the row's identity from its content rather than storing it beside it. The pairing of the two halves is made once, by whoever built the row. (Phase 01's entry said `commit: CommitSummary`; R6 made a row a list entry, not by definition a commit.) |
| `RowContent` | `cairn-model` | What a row is *about*: `Commit(CommitSummary)` today, and the seat the working-tree row takes when `refs-and-status` adds it. Read by matching, never by reaching for a commit field. Deliberately not `#[non_exhaustive]` — the next row kind should stop a view compiling, not be silently undrawn. |
| `RowId` | `cairn-model` | A row's stable identity: what selection survives on and what a detail pane opens from. `RowId::Commit(Oid)` today; a sum rather than an `Oid` because the working-tree row has no object id, and not an index because an index means something else once rows arrive above it. `Copy` (a view holds one per selection) and `Hash` (a keyed list reconciles rows by it); deliberately NOT ordered. |
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
| `HistoryList` | `cairn-ui` | The virtualised history list: `new(rows: State<Vec<HistoryRow>>, row)` plus `.lanes()`, `.selected()`, `.on_select()`, `.on_reach_end()`. Takes the rows as a HANDLE and a per-row builder, so nothing copies the history to draw it and the caller keeps the obligation to match on `RowContent`. Owns the scroll controller, the focus and the keyboard. |
| `RowRender` | `cairn-ui` | What `HistoryList` hands its builder for one row: the `HistoryRow`, whether it is selected, and how many lane columns the whole list reserves. One clone per VISIBLE row, which is the sanctioned place for one. |
| `cairn_ui::PREFETCH_ROWS` | `cairn-ui` | 24. Every row within this many of the end asks for the next page when it becomes visible — every one of them, not a single trigger row, because one trigger row is a list that quietly stops loading on a tall window. So `on_reach_end` fires repeatedly BY DESIGN and the caller must debounce. |
| `CommitRow` / `HistoryHeader` | `cairn-ui` | One commit's line and the headings above it, in Fork's four columns: graph and subject share the first, then author, abbreviated id, date. `CommitRow` holds no event handler, so two rows with the same content compare equal and an unchanged one is not re-rendered. |
| `cairn_ui::graph_geometry` | `cairn-ui` | Pure arithmetic: `row_geometry(&GraphRow, parents) -> RowGeometry` in row-local coordinates, plus `ROW_HEIGHT` (26, uniform by L7), `LANE_WIDTH`, `MAX_DRAWN_LANES` (24, beyond which lanes share the last column) and `graph_width(lanes)`. Free of Freya and Skia, so where a line goes is decided by a unit test. |
| `cairn_ui::lane_palette::lane_colour` | `cairn-ui` | Lane → colour, cycling eight Okabe-Ito hues. Colour is an AID: lane identity is the COLUMN, which is why the geometry test asserts column separation against the ink and this file's test pins the palette against Okabe-Ito's published values. |
| `cairn_app::history_state::Progress` | `cairn-app` | Everything about the history except its rows: `status()` (`Loading`/`Empty`/`Ready`/`Failed`), `lanes()`, `loaded()`, `has_rows()`, `complete()`, and `wants_more()` — which is the debounce the worker boundary needs, because every `submit` supersedes. Folded by `received(widest_lane, complete, loaded)` / `failed(..)` / `stream_ended(..)`. |
| `cairn_app::status_text` | `cairn-app` | `placeholder(&Status, has_rows) -> Option<String>` and `loaded_count(&Progress)`. R4.3 lives here: that loading and an empty repository are different SENTENCES is a thing a test decides, not a screenshot. |
| `cairn_app::repository_path::chosen` | `cairn-app` | R5.1, as a pure function over the process arguments INCLUDING the program name — dropping it is part of the rule, so it sits where the tests can reach it. Later arguments are ignored (R5.3). Deliberately not a picker. |
| `worker::RepositoryHandle` (re-exported) | `cairn-app` | Now named by the view, because the window holds one across renders and passes it to the function that builds the list. Safe to name: one method, returns immediately, carries no receiving end of anything. |
| `a_history_sized_list_renders_through_a_virtualizing_view` | `cairn-guards` | The twin for "no unbounded list renders without virtualization", REBUILT by 186d68e around what a token scan can decide — this row used to describe the first version, whose `children` half matched zero lines because Cairn writes `.child(`. It now fails when any render file names `ScrollView` at all (exceptions roster empty by design) and when no render file uses `VirtualScrollView` over `HistoryRow`s. Phase 05 verified all three regression shapes fire. |

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 lane assignment | implemented | `scripts/gate.sh` green | see progress.md's phase 01 QA entry |
| 02 history query | implemented | `scripts/gate.sh` green | four fresh agents, adjudicated by `qa-confirm`; see progress.md's newest entry |
| 03 worker boundary | implemented | `scripts/gate.sh` green | four fresh agents, adjudicated by `qa-confirm`; then a mutation-executing coverage audit whose findings are closed — see progress.md's newest entry, which also names the two gaps held for the scroll/memory design pass |
| 04 graph view | implemented | `scripts/gate.sh` green | four fresh agents (`qa-checklist`, `responsiveness-reviewer`, `test-coverage-auditor`), adjudicated by a fresh `qa-confirm`; 18 of 26 findings confirmed, fixed or filed — see progress.md's newest entry |
| 05 QA | complete | `scripts/gate.sh` green as one command: format, lint, typecheck, guards, deps, test-full, `gate: PASS` | five fresh agents (`qa-checklist`, `test-coverage-auditor`, `responsiveness-reviewer`, `gate-integrity-reviewer`), adjudicated by a fresh `qa-confirm`; 16 of 32 confirmed, all fixed — see progress.md's newest entry |

**Both of phase 04's debts are paid; this paragraph replaces what it used to
say.** The virtualization invariant now has its twin —
`a_history_sized_list_renders_through_a_virtualizing_view` in
`crates/cairn-guards/tests/invariants.rs`, promoted out of CLAUDE.md's "not yet
mechanically pinned" list by the change that made it load-bearing — and
`ROWS_DRAWN` is gone: the list is a `VirtualScrollView` keyed by `RowId`, and the
`CommitSummary` clone that used to happen per row of the whole vector now happens
once per VISIBLE row inside the viewport's builder, which is where it belongs.
Measured: 34-35 rows built per render at 1,000 rows, and the same 34-35 at
100,000. The residual the guard cannot express — that the toolkit really builds
only visible items — is stated in `CLAUDE.md` and owned by
`responsiveness-reviewer`.

R5 is built and kept to its brief: `repository_path::chosen` takes the first
command-line argument or the working directory, and there is no picker, manager,
tab or recent list. A path outside a repository reaches the window as the
sentence "no git repository at <path>", filling the list's place rather than
leaving it blank.

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

**Constraints phase 03 handed phase 04** — phase 04 has landed, so the ones it
DISCHARGED say so in place; the rest still hold for whoever comes next:

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
- ~~**`main.rs` renders a bounded 256 rows and is not virtualised.**~~
  **Discharged.** `ROWS_DRAWN` and the `.take(..)` are gone; the list is
  `cairn_ui::HistoryList` over a `VirtualScrollView`, keyed by `RowId`, and the
  invariant has a guard twin. `PAGE_ROWS` (64) remains, as the size of a page.
- **Only `crates/cairn-app/src/worker/` may name `cairn_git` or wait.** A new
  file in `cairn-app` that needs repository data asks for it through a
  `worker::Request`; the guard fails the build otherwise, naming the line.
- **Selection survives more rows arriving (R4.4) is not free.** Rows append to
  one `Vec` and the placeholder selected by index, which held only because rows
  are appended and never renumbered. Settled by R6 for selection AND for list
  reconciliation: `main.rs` holds `Option<RowId>` and compares identities, and
  the rendered list is keyed by `RowId` rather than by position, so a row
  arriving ABOVE another — which is exactly what the working-tree row does —
  neither moves the selection nor makes Freya rebuild every row below it.
  **Phase 04 kept the identity and added keyboard reach** — arrow keys,
  `PageUp`/`PageDown`, `Home` and `End`, with the list auto-focused so a reader
  with no mouse can select at all. The arithmetic is a pure `moved_to` and is
  tested; what still has no test is the WIRING of it (that `on_select` is called,
  that the row is revealed, that `RowRender.selected` reaches the row), because
  `cairn-app` still has no component-test harness — see the `freya-testing`
  decision held for the user.
- **The working-tree row will need a lane without an `Oid`, and `GraphRow.id`
  is one.** R6 made a row's CONTENT and IDENTITY total over non-commits;
  `cairn_model::GraphRow` — the assigner's own output — still keys a row by
  `Oid`, deliberately untouched here because it is the assigner's vocabulary and
  out of R6's scope. Whoever lays out the working-tree row meets that first.
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
