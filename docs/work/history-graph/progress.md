# Progress — history-graph

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-09-16 — phase 04: the graph view, and what A7 could actually be measured against

The packet is visible. `cairn-ui` draws a virtualised list of `HistoryRow`s with
lanes, edges and Fork's four columns; `cairn-app` opens the repository named on
the command line and pages as the reader scrolls. `scripts/gate.sh` is green and
the phase ran `/qa` with four fresh agents.

**O4 is decided, and the question turned out to be smaller than it looked.**
An out-of-order line — one joining a commit to a parent drawn *above* it — is
**dashed along its whole length, in the lane's own colour, with its geometry
unchanged**. Dash rather than colour because the product rules say colour never
carries meaning alone, and a dash survives a monochrome screenshot and every
colour-vision deficiency; geometry unchanged because the connection is not a
different KIND of connection, only its ancestry runs the other way, and giving it
a different shape would say something untrue.

The correction to O4's own framing: **the view never sees the repaint the
question was about.** L10 permits the assigner to repaint edges inside its
window, and phase 04 expected to have to make that repaint read as intentional
mid-scroll. It cannot happen through the session path: `HistorySession::next_page`
hands out only rows the assigner has already EVICTED, and `connect_upward` can
only repaint rows still inside the window, so by the time a row reaches a view
its out-of-order segments are already on it. O4 is therefore a static legibility
question, not a question about motion.

Evidence it was decided against, rather than guessed: the layout harness
(`measures_layout_over_every_ref_of_a_named_repository`) run over every ref of
`/home/alexparlett/Development/freya`, 2,896 rows, in the order Cairn walks by
default —

```
  CommitTime: 2896 rows, walked 2896, in 15.1ms
    segments/row : mean 3.12  p50 3  p95 7  p99 8  max 11
    open lanes/row: mean 2.12  p50 2  p95 6  p99 7  max 9
    out-of-order: 5 rows (0.173%), 5 segments
    highest lane number used: 8
```

0.173% matches the PRD's "0-0.2% of rows in the default order" exactly. The same
harness in `GraphOrder` reports 86.9% of rows out of order and 132 lanes, which is
why the default order is the one the view is designed against.

Two more repositories, same harness, same default order: `strata` (1,081 rows,
166 ref tips) and `hyprland` (535 rows) both report **0 out-of-order rows**, with
6 and 1 lanes at their widest. In `GraphOrder` the same two report 84.4% and
80.9%. So across three real repositories the default order produces 0-0.17% —
inside the PRD's stated range at its bottom end — and the dashed line is
something a reader will rarely meet. That is an argument FOR making it
unobtrusive and self-explanatory rather than loud: a marker seen once a year has
to read on its own, and a marker seen constantly would not be worth a distinct
treatment at all.

## A7 — measured, and the 100k target was NOT reachable on this machine

Stated plainly because a fabricated number would be worse than a recorded
shortfall: **no repository with 100k commits exists on this machine.** Every git
repository under `/home/alexparlett`, `/usr/src` and `/opt` was enumerated; the
largest is `/home/alexparlett/Development/freya` at **2,540 commits from `HEAD`**
(2,896 across all refs). So A7's named-repository half was run against that, and
the "at least 100k" half was run against a synthetic row vector instead, with the
difference stated below rather than blurred.

**Against the real repository** — `/home/alexparlett/Development/freya`, 2,540
commits, release build, paged to completion with the `End` key:

| | |
| --- | --- |
| rows loaded | 2,540 (title bar reports `2540 commits`, no ellipsis — complete) |
| RSS at first page (64 rows) | 192.1 MB |
| RSS at full history (2,540 rows) | 196.2 MB |
| growth | +4.0 MB for 2,476 rows ≈ 1.6 KB/row, which includes gitoxide's walk seen-set and the assigner's window, not only the rows |

**Against a synthetic history** — a throwaway instrumented build (reverted, not
committed) that filled the row vector with N generated rows and counted
`build_row` invocations per render. This measures the VIEW's half of A7 and makes
no claim about a repository:

| rows in the list | rows built per render | renders | CPU for 60 `PageDown`s | RSS |
| --- | --- | --- | --- | --- |
| 1,000 | 34-35 steady, 119 on a viewport jump | 15 | 0.17 s | 152.3 MB |
| 10,000 | — | — | 0.18 s | 157.6 MB |
| 100,000 | 34-35 steady, 119 on a viewport jump | 15 | 0.18 s | 216.6 MB |

**What that decides.** Row construction per render is *identical* at 1,000 and at
100,000 rows — same steady count, same jump count, same number of renders — so
R4.1's "only visible rows are rendered" is measured rather than inferred from a
component's name, which is what the phase's QA brief asked for. Frame cost is
flat in list length: the same 60 scroll jumps cost 0.17-0.18 s of CPU at every
size, about 3 ms per jump.

**What it does not decide, and the caveat A7's wording hides.** Memory is flat in
*history length* — a 100k-commit repository costs nothing until it is scrolled —
but it is **linear in rows SCROLLED**: 1,000 → 100,000 rows added 64 MB, about
660 bytes per retained row, and nothing evicts. At a million rows scrolled that
is ~660 MB. Phase 03's state.md already flagged this shape; phase 04 has now put
a number on it. Capping or windowing the retained vector is a decision for the
user, and it is filed, not built.

Also not measured: frame time from inside the renderer. Freya's performance
overlay exists only in debug builds and its toggle could not be driven through
synthetic input in this session, so the frame-cost figure above is CPU consumed
per scroll gesture, not a per-frame histogram.

## What QA found, and what was done about it

Four fresh agents: `qa-checklist`, `responsiveness-reviewer`,
`test-coverage-auditor`, adjudicated by a fresh `qa-confirm`. 18 of 26 raw
findings confirmed. Fixed in this phase:

- **The lane-separation test was a tautology.** `every_lane_below_the_cap_has_its_own_column`
  compared the gap between columns to `LANE_WIDTH / 2` — the same constant that
  produced the gap — so it was true for every positive `LANE_WIDTH`. The auditor
  demonstrated it by setting `LANE_WIDTH = 1.0` and watching 22/22 stay green
  with `NODE_RADIUS = 4.0`, i.e. every dot swallowing both neighbours. It now
  compares against the INK (`2 * NODE_RADIUS`, `STROKE_WIDTH`), and that mutation
  fails two tests. This was the test carrying R4.2's "lane identity survives a
  monochrome screenshot".
- **The keyboard half of R4.4 had no test at all.** The arithmetic is now a pure
  `moved_to(key, current, last)` with both arrow directions asserted distinct,
  `Home`/`End` distinct, the page jump bounded above one row and below the list,
  both end clamps, the nothing-selected case, and a key the list does not own.
  Verified against mutations: `End => Some(0)` and `PAGE_JUMP = 1` both now fail.
- **`no_walk`'s call site was unpinned**, so restoring `Update::Failed` at the
  call site left every test green while a fresh `git init` repository showed a
  red error instead of "No commits yet." — the exact R4.3 bug. There is now an
  end-to-end test through `serve` against an unborn `HEAD`, with the repository
  built by `std::fs` rather than by running `git init`, because
  `only_the_ops_module_mutates_a_repository` scans this crate's test code too.
- **O4 was tested for one `EdgeKind`.** The assigner marks all three pieces of an
  out-of-order line; a rule that dashed only `OutOfCommit` would draw one dashed
  row and then a solid line. All three kinds are asserted now.
- **The virtualization invariant got its twin**,
  `a_history_sized_list_renders_through_a_virtualizing_view`, which fails when a
  render file builds `children` from a collection of `HistoryRow`s or puts them
  in a plain `ScrollView` without naming `VirtualScrollView`, and when nothing
  names `VirtualScrollView` at all. Verified by swapping the view for
  `ScrollView::new_controlled` and watching it fail. The residual — that the
  toolkit really builds only visible items — is stated in `CLAUDE.md` as a review
  obligation and answered today by the measurement above.
- Smaller: the selection's prepend case (a row arriving ABOVE the selection,
  which is what the working-tree row will do) is now tested; the program name is
  dropped inside `repository_path::chosen` so the whole of R5.1 is under test
  rather than one line of it sitting at an untested call site; the status →
  sentence mapping moved to `status_text` so R4.3's *rendering* decision is
  decided by a test and not by a screenshot; the palette is pinned against
  Okabe-Ito's published values so the colour-vision claim is decided by
  something; and the keyboard handler drops its read guard before calling out,
  because a future handler that reloaded the list would otherwise re-enter the
  borrow and panic on the UI thread.

**Dismissed, with the reason** (adjudicated by `qa-confirm`, not by the
implementer):

- *"No reachable cancellation path for a superseded request."* The premise is
  right and the defect does not follow: one-request-at-a-time is deliberate and
  tested, the epoch wiring is pinned end to end by
  `superseding_a_request_stops_the_walk_that_is_serving_it`, and a second cancel
  path IS reachable — closing the window drops `Updates` and calls `stop()`.
- *"`index_of`'s fallback scan runs on the UI thread."* Not entered today: rows
  only append and the cursor is written on every selection path. It is the
  correctness fallback by design. Filed against the packet that prepends a row.
- *"`f32` scroll offsets lose row resolution past ~645,000 rows."* The threshold
  is wrong by about 16x. At that index the `f32` ulp is 2 px and rows are 26 px
  apart; collisions need an offset of 2^28, about 10.3 million rows — which at
  660 bytes a row is ~6.8 GB of retained rows, unreachable before the vector is.
- *"A9's second clause has no twin."* A `_ =>` arm over a single-variant enum is
  `unreachable_patterns`, which `-D warnings` already denies. The residual — a
  wildcard added in the same change that adds variant two — is real and is filed
  against that packet.
- *Progress.md entries said to be stale.* `docs/CLAUDE.md` forbids retro-editing
  a historical record; the obligation was a new entry, which this is.

- **`graph_cell` had no test, and two of its decisions were reachable by no
  arithmetic test**: whether a dashed stroke is actually dashed, and whether a
  merge is actually hollow. Both are what "colour never carries meaning alone"
  comes down to on screen. The auditor filed this against a `freya-testing`
  dependency; it did not need one — Skia is already linked, so the row is painted
  onto an offscreen raster surface and the pixels read back. Verified against
  both mutations: removing the dash and filling the ring each fail a test.

**Filed as follow-up, not built:** the first page's size against the viewport
(filling a 4K window costs three serial round-trips at `PAGE_ROWS = 64`); hoisting
the constant dash `PathEffect` and the `PathBuilder` out of the per-repaint path;
windowing or capping the retained row vector; a mid-scroll "fetching a page"
affordance beyond the title bar's ellipsis; and the component tests that need
`freya-testing` — the paging call chain and R4.3/R5.2's *rendering* (the decision
of WHICH sentence is now tested; that the sentence reaches the window is not).

**Needs the user, batched:** whether a transient page failure should be
retryable (today one failed page ends paging for the session, which is deliberate
and tested but has no way back); and whether to add `freya-testing` as a
dev-dependency, which is what a mechanical twin for "only visible rows are built"
and every other component test would need.

## 2026-09-16 — R6: a row is a list entry, not by definition a commit

`HistoryRow { commit: CommitSummary, graph: GraphRow }` made every row a commit
by construction. It is now `HistoryRow { content: RowContent, graph: GraphRow }`,
with `RowContent::Commit(CommitSummary)` the only variant this packet emits, and
`HistoryRow::id() -> RowId` (`RowId::Commit(Oid)`) as the stable identity R6.1
asks for. `refs-and-status` owns the working-tree variant and its fields;
nothing here invents them, and R6.3's decoration is still added as fields, not
reserved now.

**Why a sum and not a nullable commit.** A row that is not a commit is not a
commit with missing parts — it is a different kind of entry that still occupies
a lane and still has lines passing it. Making the *content* the sum keeps the
graph half untouched: the working-tree row is laid out by the engine like any
other row.

**`RowContent` is deliberately not `#[non_exhaustive]`.** A wildcard arm in a
view is a row silently not drawn; a compile error is a decision someone has to
make, in the places that must choose what to render. That is the cost R6.2 says
is worth paying now rather than later, so it is paid loudly.

**Identity is a sum too, and derived rather than stored.** `RowId` is not an
`Oid` (the working-tree row has none) and not an index (an index means something
different the moment rows arrive above it). `id()` computes it from content, so
there is no second field that can disagree with the first. `cairn-app` now holds
`Option<RowId>` for the selection instead of a `usize`, which is R4.4's premise
settled early and for free.

**A9 is pinned by a test-only variant, and that is a deliberate call.**
`RowContent::NotACommit` and `RowId::NotACommit` exist under `#[cfg(test)]` in
`cairn-model` only. Without them A9 could not be decided today: every test would
build a commit row, and collapsing the enum back into a `commit:` field would
leave the suite green. With them,
`a_row_can_be_about_something_that_is_not_a_commit` builds a non-commit row,
checks it keeps its lane and edges, and reads an identity off it that carries no
`Oid` — none of which compiles against a commit-only struct. They are invisible
to every other crate (integration tests and all three consumers see exactly one
variant), so they cannot become the shape `refs-and-status` inherits.

**What R6 did NOT touch, on purpose.** `GraphRow.id` is still an `Oid`: it is
the lane assigner's vocabulary, out of R6's scope, and the first thing whoever
lays out the working-tree row will have to face. Recorded in `state.md` rather
than fixed here.

**QA over this change.** Fresh `qa-checklist`, `test-coverage-auditor` (audit by
reading, not by building a mutation harness) and `responsiveness-reviewer`;
adjudication by a fresh `qa-confirm`. Six findings confirmed, seven dismissed,
one escalated to the user.

Fixed here: the rendered list was still keyed by the enumerate index while
selection had moved to identity — zero cost while rows only append, and exactly
wrong the day the working-tree row is PREPENDED, so `main.rs` now keys by
`RowId` (`KeyExt::key` takes anything hashable, verified in the fork at
`crates/freya-core/src/elements/extensions.rs`). `state.md`'s claim that nothing
in the view depends on position was false while `.key(i)` stood, and is
rewritten to say what is and is not positional. Two assertions in the A9 test
re-read values the test's own helper had just constructed — no shipping code
between them — and are gone; the docstrings that oversold what the two shape
tests decide now say they pin a shape at compile time. `RowId` no longer derives
`PartialOrd`/`Ord`: nothing orders it, and an ordering would assert that one
kind of row sorts before another, which the model does not know.

Dismissed, with reasons: **the initial selection changing from "row 0
highlighted" to "nothing selected"** — index `0` was an artifact of the
placeholder `usize`, no requirement pins an initial selection, and it is phase
04's UX call. **"Storing `RowId::Commit(row.graph.id)` instead of the
content-derived id would leave the suite green"** — it would not: the property
is pinned on the real code path three times, by
`a_row_takes_its_identity_from_its_content` (halves deliberately disagreeing)
and by the two `row.id() == RowId::Commit(row.graph.id)` assertions that run
against real repositories. **"`state.md` says R4.4's premise is settled but
nothing tests it"** — the entry already assigns the untested half forward, and
the inaccurate clause in it was the one fixed above. **A stale assert message
saying "a different commit"** — every row this query emits IS a commit, so it
still reports accurately. **The per-render `CommitSummary` clones and the
unvirtualised 256-row cap** — inherited, identical before and after this change,
and R4.1's. **`progress.md` overstating A9** — the entry scopes itself in its own
body.

Escalated, and named under "Still open for the user" in `state.md`: nothing
stops a wildcard arm over `RowContent` once a second variant exists, and A9's
"pinned by" column names a `cairn-ui` row component that does not and should not
name `RowContent`.

Consumers updated: `cairn-git/src/history.rs` and `src/history/session.rs`
(construction), `cairn-git/tests/history.rs` (two helpers that match rather than
reach), `cairn-app/src/main.rs` (renders by matching on content, selects by
identity) and `cairn-app/src/worker/pool.rs` (`id()` returns a value now).
`cairn-ui` was untouched: `CommitRow` renders a `CommitSummary` and never named
a row, which is what "a component draws one kind of thing" should look like.

## 2026-09-15 — pinning phase 03's seams, and what is deliberately still open

A fresh `test-coverage-auditor` went over phase 03 by executing mutations rather
than reading, and found that every gap it could reach was at a seam BETWEEN two
well-tested pieces. The session-level tests are strong on both sides of each
seam; nothing crossed them. Closed here, tests-only apart from two comment
fixes, each pin verified by applying the named mutation, watching it go red, and
restoring.

**The epoch was never pinned as the cancel signal at the call site.** `epoch.rs`
proves `Superseded` flips; `cancelling_a_session_stops_the_walk_and_keeps_its_progress`
proves `next_page` honours *a* cancel. Nothing proved `serve` hands the engine
*the epoch* — swapping `epochs.watch(epoch)` for a fresh `CancelSignal` left
22/22 `cairn-app` tests green, which is the weak form of R3.2 (answer discarded,
walk still burning a core) restored in silence. The pool tests that look like
coverage for this run on `inbox_only()`, which has no repository behind it, and
decide the epoch filter alone. Now
`superseding_a_request_stops_the_walk_that_is_serving_it` runs a real worker
over the real repository and reads a superseded request's fate off the NEXT
page, where its three possible fates differ: stopped mid-walk starts the next
page again at the row the scroll opened on, never-picked-up carries on past the
rows the previous scroll handed out, ran-to-completion returns nothing. Only the
first is a stop, and only a cancel wired to the epoch produces it. Under the
mutation, 40 rounds out of 40 ran to the end of the history.

**And the first version of that test was flaky, which the review caught and the
implementer did not.** It aimed a swept `sleep` at the walk it had to interrupt,
which reads reasonably and is unusable: a walk of this 41-commit repository is
microseconds long, and the `responsiveness-reviewer` measured 20 failures in 25
runs with sixteen CPU burners against one failure in 130 idle runs. A test whose
green depends on an idle machine is not a pin. The rewrite stops aiming at the
walk and keeps the worker BUSY instead — one measured round trip to learn what
an answer costs here, then a batch of 2,000 requests posted under a single epoch
so the worker never returns to `recv`, then the supersession. What is left
racing is the gap between two requests of the batch, and because that gap holds
two syscalls it is not a sliver under load, so rounds are cheap and many and the
failure is that EVERY one of forty saw a walk finish. 40/40 green under 24
busy-loop processes; still 40-out-of-40 red under the mutation. The general
lesson: when a test has to interrupt something, do not aim at the interval —
make the interval long enough to be hit.

**The worker-panic path tested the unit, never the wiring.**
`a_panicking_worker_says_so_instead_of_disappearing` builds its own `WorkerExit`
inside its own thread, so it decides that the guard works when something puts it
there — not that `open` does. Deleting `WorkerExit` from `open`'s closure and
ending it with `drop(outbox); worker_wake.signal();` keeps clean shutdown working
and everything green, while a panic inside `serve` unwinds past the signal: the
phase-03 deadlock, returned.
`a_panic_inside_a_real_worker_is_announced_by_the_pool_that_opened_it` raises a
real panic inside `serve` on a real worker. The injection point is the waker:
the engine is written not to panic (`unwrap` and friends denied outside tests,
and a commit it cannot read comes back as `Error::ReadCommit`), but
`Wake::signal` calls the window's waker ON THE WORKER THREAD, so a waker that
gives way once is a panic inside `serve`, raised by the very line that answers a
request. The wait for the notice is bounded at ten seconds, because the failure
being pinned is a park that never ends and a hung suite decides nothing.

**`Outbox` being non-`Clone` was the whole of the one-sender fix (500f6eb) with
nothing reading it.** The fix's own test admits in its doc comment that it
passed against the broken code — the race is too narrow to see. The twin is now
the compiler, at the strongest tier available short of changing shipping code:
`OneSenderPerWorker` is implemented for `Outbox` by hand and for everything
`Clone` by blanket impl, so the two overlap exactly when `Outbox` is `Clone` and
an overlap is `error[E0119]`. Deriving `Clone` was verified to stop the test
build. It is a test-tier artifact by choice — the gate compiles tests twice
(`cargo clippy --all-targets` and the test run), so a `Clone` cannot reach it,
though `cargo build` alone would still pass.

What the compiler decides there is the `Clone`, not the rule, and the
`qa-checklist` agent proved the difference by writing the bug back in:
`Sender<Envelope>` is `Clone` and `Outbox`'s fields are visible throughout
module `pool`, so `Outbox { updates: outgoing.clone(), .. }` beside the first
compiles and leaves all 25 tests green. The residual is now STATED at the twin
rather than implied, which is what the meta-invariant asks when a check cannot
reach the whole rule. A guard over one module's struct literals would be a
bigger instrument than the hole — but `qa-confirm` named a stronger tier that is
neither, and it is a decision for the user rather than a fix to slip in here:
give `Outbox` its own module with private fields and a constructor that creates
the channel and hands back `(Outbox, Receiver<Envelope>)`, and a second sender
into the same channel stops being writable at all from outside it.

**Two comment fixes, and one claim that was simply false.** The note above
`drop(shared)` in `serve` credited that line with a compile-tier guarantee it
does not carry. Both halves re-checked by execution: moving `to_worker()` into
the request loop DOES fail to compile, but with `error[E0597]: repo does not
live long enough` — the `HistorySession<'_>` borrow held in `scroll` is what
refuses it — while deleting `drop(shared)` alone leaves clippy `-D warnings`
clean and every test green. The comment now says which line carries what, and
`drop(shared)` is kept for the narrower thing it does do: a SECOND `to_worker()`
beside the first, one no session borrows, is a use-after-move. The module
header made the same wrong claim in its opening sentence ("per-request
conversion compiles and passes every test"), which 2d0bdf6 had already made
untrue; it is corrected in the same change. A duplicated doc-comment line is
deleted.

**What QA found, and what was done with it.** Three fresh agents —
`qa-checklist`, `test-coverage-auditor` (told to audit by reading; a mutation
harness had twice eaten a whole turn budget here) and `responsiveness-reviewer`
— adjudicated by a fresh `qa-confirm`. Six findings, all acted on; none
dismissed:

- The flaky first version of the supersession test, above. Fixed by rewriting
  it.
- The non-`Clone` twin was described as sealing "one sender per worker thread"
  when it seals the derive. Residual now stated at the twin and above.
- The supersession test's comment listed three fates where there are four: a
  request that FAILED also leaves the next page starting at the row the scroll
  opened on, because `serve` drops a poisoned session and the cold restart goes
  back to `HEAD`, and its `Update::Failed` is filtered out by epoch before
  anyone sees it. Not a way for the broken version to pass — the mutation raises
  no error — but the comment now says so.
- The panic test's comment claimed the mutation parks the reader forever. It
  does that only when the reader was ALREADY parked when the worker died;
  otherwise the channel closes and the stream just ends. Measured: three times
  in four the ten-second bound was spent in full, once it reported at once.
  Both are red; the comment now names both.
- And the panic test had a flake of its own, which the fix above is what made
  legible: a first version asked for a page, waited for it, then checked whether
  the waker had given way — but the worker SENDS the page before it signals, so
  a reader that took the page in that window asked for another that nobody was
  left to answer, and got `WorkerLost` where it wanted rows (once in 40 runs).
  The loop now treats the answer and the notice as the same news, and the whole
  of it — including everything that can park — runs on one thread the main
  thread waits on with a bound. 150 idle runs and 40 at loadavg 36, all green.
- A literal count ("the three tests that look like this one") against the anchor
  rule. Removed.
- The corrected `drop(shared)` comment named `E0597` alone, where a reader who
  tries the mutation with `drop(shared)` still in place gets `E0382` for the
  move as well. Both are now named, with which one is load-bearing.
- The empty panic hook the test installs is process-global, and it was held
  across the whole ask-again loop, so the loop's own failures would have been
  silent. The restructure closes that differently and better: every failure on
  the spawned thread comes back as a value rather than as a panic, and the main
  thread restores the hook when the bounded wait returns, before it asserts
  anything.

One observation is recorded rather than closed: the `test-coverage-auditor` saw
the panic test fail 3 times in 20 whole-suite runs against the PRE-rewrite file,
spending the ten-second bound, and could not reproduce it against the file that
landed. The likeliest mechanism is now the send-before-signal flake in the
bullet above, found later and in the same test — a reader that takes the page
before the signal asks for another, and against the pre-rewrite structure that
second ask waited on a worker that was already dying, which is a ten-second
bound spent for exactly the reason the bound exists to report. Contention is the
other candidate and was certainly real: three agents and this session were
running `cargo` against ONE target directory, one of them rebuilding half a
gigabyte of dependencies, and that left a stale test binary whose baked
`CARGO_MANIFEST_DIR` pointed at a deleted scratchpad copy — spuriously
reddening every repository-opening test until `cargo clean -p cairn-app`.
Neither was proven. If a ten-second bound is ever spent again, capture a stack
during it rather than retrying; do not paper it over.

**Held deliberately, so they are not lost:**

- The cold-restart branch in `serve`, and "a superseded session keeps its
  laid-out rows". Both are real gaps and both encode cursor and session
  semantics that the paused design pass on the scroll/memory model may change.
  Pinning them now would pin a decision that has not been made. They belong to
  whatever phase closes that design pass.
- `crates/cairn-app/src/main.rs`'s paging loop and `ROWS_DRAWN`, carried as
  phase-04 debt: deciding them needs component tests, and `freya-testing`'s
  headless runner is still planned rather than available.

## 2026-09-15 — correcting a claim I put in the PRD, and what phase 03 proved

R1.3 carried a sentence saying phase 03's live walk session would close the
assigner's skew blind spot "exactly", because the walk already holds the seen-set
that answers it. I wrote that when settling phase 02's decision 2, from the
reasonable-sounding assumption that a live walk would expose its own seen-set.

It does not. gitoxide keeps that set inside the `Box<dyn Iterator>` behind
`gix::revision::Walk` with no accessor (`gix-0.87.1/src/revision/walk.rs`, module
`iter_impl`), so reaching it would mean keeping a second walk-sized copy — the
exact shape R1.3 exists to forbid. Phase 03 found this while building against the
claim, TRAP-marked it rather than silently rewording an in-flight requirement, and
was right to. The clause is now corrected: the blind spot stands as phase 02 left
it and is accepted for this packet.

The lesson is not about gix. Decision 2 was settled on a mechanism nobody had
read the source for, and it was settled in the same breath as decision 1, which
was measured. A measured decision and an assumed one travelled together and were
recorded with equal confidence.

Phase 03 also contradicted the premise the plan handed it for O3. The phase doc
argued against scaling the pool with cores because the work is I/O- and
cache-bound and gix already parallelises internally, so extra threads would
oversubscribe. Measured on a 200,001-commit repository with no commit-graph:
eight concurrent walks scale 7.9x, near-linear. The pool is still one worker, but
for a structural reason rather than the one the plan gave — a live walk borrows a
single thread's handle and cannot be split, and there is exactly one scroll. What
the measurement changes is the next decision: fetch can have its own worker for
almost nothing, which is the opposite of what the plan implied.

## 2026-09-15 — phase 03: the worker boundary, the live walk, and O3 measured

Built in packet mode on `feature/history-graph`. Five deliverables: the
`ThreadSafeRepository` route R3.1 needed (`cairn_git::SharedRepository`), the
live walk session R2.5 called for (`cairn_git::HistorySession`), the worker and
the epoch-carrying request/response channel (`crates/cairn-app/src/worker/`), the
seam that hands rows to the view as values, and the guard that finally pins "the
UI thread never waits on repository work".

**The live walk session, and what it costs.** `Repository::history_session`
opens gitoxide's walk and keeps it; `next_page` takes rows off it. Paging is
O(limit) — pinned by
`paging_a_session_costs_the_page_and_not_the_pages_before_it`, which asserts that
no page after the first walks more commits than it returns AND that the whole
scroll walks each commit exactly once, with the cursor path's growing cost as its
negative control in the same test. The price is a one-off prime: rows are handed
out only once the assigner has evicted them from its window, so the first page
walks `limit + window` commits. At the default window that is 1024 extra commits
once per scroll, about 2.4 ms at the 2.34 us/commit phase 02 measured — and in
exchange a row is final when the view first sees it, so no repaint protocol is
needed across the seam at all.

**What happens to a live session when its request is superseded.** Nothing is
thrown away. `next_page` returns `Error::Cancelled` but keeps every row it had
already laid out inside the session, so the request that replaced it continues
from there instead of re-walking; the walk is never left half-consumed for the
next reader, which is what R2.5 asks. The worker drops a session only when the
new request is a *different* walk (`Request::OpenHistory`, which is how a reload
arrives) or when the walk itself failed — and in the failure case the cursor from
the last good page survives, so the next request cold-restarts from it. Pinned by
`cancelling_a_session_stops_the_walk_and_keeps_its_progress`: the walk stops at
the commit the signal fires on, the signal is polled once per commit, and
resuming returns four rows having walked fewer than four commits.

**Epochs are the cancel signal, not a tag on the reply.** `Epochs::watch(epoch)`
returns a `Superseded`, which implements `cairn_git::Cancel` as "is my epoch
still the current one". Superseding therefore stops the walk at the next commit
rather than letting it run to completion and discarding the answer — the weak
version of R3.2, which looks identical from the window while it burns a core.
The stale answer is dropped as well, by `Updates::next`, which is the one place
that decides what reaches the view.

**O3 is resolved, and the measurement contradicted the question's premise.**
Harness: `measures_concurrent_walks_against_a_named_repository` in
`crates/cairn-git/src/repository.rs`, `#[ignore]`d, driven by
`CAIRN_BENCH_REPO`. Repository: a 200,001-commit history built with `git
fast-import` (16 interleaved branches, periodic merges into the trunk, 196,625
commits reachable from `HEAD`), **no commit-graph file**, 50,000 commits walked
and laid out per thread, each thread taking its own handle from one
`SharedRepository`. Release build, four runs, medians:

| threads | slowest single walk | rows/s together | scaling |
| --- | --- | --- | --- |
| 1 | 179 ms | 279,000 | 1.0x |
| 2 | 167 ms | 597,000 | 2.1x |
| 4 | 171 ms | 1,184,000 | 4.2x |
| 8 | 181 ms | 2,196,000 | 7.9x |

The phase brief's argument against core-scaling — "the work is I/O- and
cache-bound and gix already parallelises internally via `max-performance`, so
scaling with cores oversubscribes against gitoxide's own threads" — **is not what
this machine measures.** Throughput rose nearly linearly to 8 threads on 16
cores, and the time for any one walk stayed flat within noise. Caveats worth
keeping with the number: the page cache was warm and every thread walked *the
same* 50,000 commits, so they shared it perfectly; a cold cache or divergent
walks would scale worse. Read it as an upper bound on scaling, not a promise.

**The pool is one worker per repository anyway, for a structural reason.** The
live session borrows one worker's repository handle and stays on that thread for
the life of a scroll, so a second worker cannot serve the *next page* of the same
scroll — splitting a scroll across workers is not a tuning question, it is not
expressible. And there is exactly one scroll, so a second worker would have
nothing to do while holding its own 4 MiB object cache. `WORKERS_PER_REPOSITORY`
records the decision and a `const` assertion fails the build if it is raised
without the routing decision that would have to come with it. What the
measurement changes is the *next* decision: a second kind of work should get its
own worker, and the numbers say that will cost the first one almost nothing.

**The guard, and the tier it sits at.** `cairn-app` is now partitioned.
`crates/cairn-app/src/worker/` runs repository work and may block; every other
file in the crate renders and may name neither `cairn_git` nor any waiting
primitive. The sets are disjoint and the guard checks both directions, so a
render path can neither reach a repository nor wait on one — which is PRD
criterion A6 made mechanical rather than a reviewer's judgement. The type carries
the first half: `RepositoryHandle` is what a component holds, it has exactly one
method, and it holds no receiving end of anything, so there is nothing on it to
wait on. Everything a type cannot say is the guard's:
`the_ui_thread_never_waits_on_repository_work`, on the new `waits_on_work`
matcher.

The matcher forbids spellings rather than trying to decide what is being waited
FOR, which is not decidable from source. Bare identifiers (`Receiver`, `Mutex`,
`Condvar`, `JoinHandle`, `Barrier`, `RwLock`, `block_on`, `blocking_recv`,
`park`, `sleep`, `scope`, `recv_timeout`, ...) catch aliases, because you cannot
alias what you have not first named — `use std::sync::mpsc::Receiver as Rx` is
caught on its import line. The four with innocent namesakes — `join`, `lock`,
`recv`, `wait` — are matched only when called with NO arguments, which is what
separates `handle.join()` from `root.join("crates")` and `parts.join(", ")`; the
check reads across newlines, so wrapping the parentheses does not hide it.

**It was proved red before it was believed green.** Five violating forms were
written into the tree, watched to fail, and deleted:

| what was written | what the guard said |
| --- | --- |
| `main.rs` takes a `&cairn_git::Repository` | `main.rs:119 names cairn_git on a render path` |
| `main.rs` calls `rx.recv()` | `main.rs:119 waits for something, and it is on a render path` |
| the same, aliased to `Rx` with the parentheses wrapped over two lines | same failure, same line |
| `worker/pool.rs` returns an `impl freya::prelude::IntoElement` | `pool.rs:679 names freya inside crates/cairn-app/src/worker` |
| the worker module moved out of the crate entirely | `found no files under crates/cairn-app/src/worker` |

The last is the nonzero-count check the qa-gate design rules ask for, and it is
asserted on BOTH sides of the partition. A sixth attempt — renaming `worker/` to
`engine/` in place — also failed, because the module then counts as a render path
and its own waiting trips the first rule.

**What the interface carries for fetch, and what it does not.** Recorded in
`state.md` in full. In short: a request is answered by a *stream* of updates
rather than one reply, so progress reporting is an added variant and not a
changed shape; a worker runs ordinary blocking code, so a job that must wait for
a UI answer makes its own reply channel and blocks on it, needing no pool surface
at all; and workers are pinned to a purpose, so fetch gets its own rather than
stalling the graph behind a password prompt. No part of fetch is built or
stubbed, and no `Progress` variant exists — an unused variant is dead code the
gate would reject, and the point of reading R4 was to check the shape, not to
pre-build it.

**Two things deliberately NOT done, both recorded rather than silently skipped.**
R1.3's "known blind spot" says phase 03's live session closes it, because the
walk already holds the seen-set that tells a parent already gone from one still
to come. It cannot: `gix::revision::Walk` keeps that set inside a
`Box<dyn Iterator>` with no accessor (`gix-0.87.1/src/revision/walk.rs`, the
`iter_impl` module), so the only way to have it is to keep a second copy — the
walk-sized set R1.3 exists to forbid. The blind spot is therefore still open and
is raised below. And the history list is still not virtualised: `main.rs` now
loads and draws a bounded 256 rows rather than one component per row of whatever
arrived, which is a cap and not virtualisation, and phase 04 owns R4.1.

**QA: four fresh agents, adjudicated by a fresh `qa-confirm`.** Agents, none of
them the implementer: `responsiveness-reviewer` (this phase is entirely its
subject matter, 7 findings with its own measurements), `gate-integrity-reviewer`
(the phase changes the enforcement layer, 10 findings, several proved by
mutating the real tree) and `qa-checklist` (NOT READY, 7 findings). A fourth,
`test-coverage-auditor`, **did not deliver**: it ran out of turns, was re-asked
once, and then stalled without reporting — recorded rather than counted, because
a reviewer that returns nothing is a delivery failure and not a clean bill.

Before it stalled the second time it did manage one line, and that line was
worth having: moving `to_worker()` into the request loop — per-request
conversion, the QA brief's named worry — was caught by NOTHING. It is caught by
the compiler now. `serve` drops the shared handle after converting, so a second
conversion is a use-after-move: a compile error, which is a stronger twin than
any test could be. The object-cache install has a twin now too
(`every_worker_handle_carries_the_object_cache`), since deleting it costs 53% on
a commit-time walk and changed no answer any test was looking at. In its place
the implementer ran the other mutations itself and they are named where they
appear below; the packet's final QA phase should still treat this surface as
un-audited by a fresh agent.

*The one that mattered most was found by the gate itself.* `scripts/gate.sh`
did not fail — it HUNG, on
`the_stream_ends_when_every_worker_has_gone`. `Updates::next` only ever noticed
a closed channel through `try_recv`, and the only thing that woke a parked
waiter was a send; a worker that exited *cleanly* sent nothing, so the UI task
parked forever with nothing coming. That is precisely the hang this whole
boundary exists to prevent, arrived at from the other side, and the panic path
was fine only because it happens to send first. `WorkerExit` now carries the
LAST sender and drops it *before* signalling, so the waiter wakes to a closed
channel rather than an empty one. Pinned by
`a_worker_ending_in_silence_still_wakes_the_waiting_task`.

*Confirmed and fixed here.* Opening the repository ran on the UI thread, inside
the first render — 131-140 us measured, bounded, but filesystem I/O before the
first frame and structurally invisible to the guard, which partitions by file;
`open` now spawns the thread first and discovers on it, and a path outside a
repository arrives as an `Update::Failed`. The first page walked AND DECODED
1,088 commit objects to deliver 64 rows, because no row leaves the assigner
until its window is full; the session now reads a commit object when its row is
handed out rather than when it is walked, so priming costs walk steps and no
object reads, and a scroll abandoned after one page no longer pays for 1,024
rows nobody asked for. A dead worker announced itself and was then immediately
un-announced by a generic "stopped" line. `HistorySession::order()` and
`window()` had no callers at all. The day loop could not see the phase's own
tests: `--lib` skips bin targets and `cairn-app` is a binary, so all 21 worker
tests ran in neither `--fast` nor pre-push.

*Confirmed and fixed in the enforcement layer, which is where the review bit
hardest.* Four of the guard's holes were proved by mutating the real tree, not
argued: a render file could hold a receiver without ever naming one
(`let (tx, rx) = channel(); for u in rx {}` matched nothing — the roster now
carries the constructors); eight of eighteen roster entries could be deleted
with the whole suite green (every entry now has a literal line in
`every_waiting_spelling_in_the_roster_is_matched`, so deleting one turns it
red); the nonzero file-count pin was aggregate rather than per-directory, so
`cairn-ui` alone satisfied it; and `waits_on_work` scanned string literals, so a
status line mentioning a lock would have reddened a render file — it now runs on
`code_without_strings`. One hole the review only STATED was closed instead: a
busy poll over `try_recv` or `spin_loop` costs the UI thread the same core a
block would, and those spellings are now on the roster too — so on a render
path they are checked rather than reviewed. Inside `worker/` they remain the
reviewer's, and `CLAUDE.md` says which is which.

*The finding with the longest reach was about honesty, not code.* The guard
partitions by FILE, so it cannot decide which THREAD a function runs on: the few
`worker/` functions the UI thread itself calls are exempt from the matcher while
running on the UI thread, and a busy `try_recv` loop names nothing. The first
draft of this change wrote a type-level tier into `CLAUDE.md` that nothing
enforces, and narrowed the `responsiveness-reviewer`'s obligation in
`docs/qa-gate.md` on the strength of a guard answering a narrower question —
coverage moving backwards in the one change that was supposed to move it
forwards. Both are rewritten: the invariant now states its residuals explicitly,
per the meta-invariant, and the reviewer keeps "does this code block the UI
thread?" in full.

*Dismissed, with reasons.* "`WORKERS_PER_REPOSITORY`'s `const` assertion pins the
constant rather than the behaviour — spawn from `for _ in 0..N` instead" — that
loop does not compile at any count, because `incoming` is a single-consumer
receiver moved into one closure and the compiler cannot know a loop runs once.
The impossibility is the enforcement; the assertion is the sign that says so,
and its comment now explains this. "The new guard duplicates
`layers_never_name_the_crates_they_are_sealed_from` on `cairn-ui`" — accepted
and narrowed rather than dismissed: the crate-naming half now applies to
`crates/cairn-app/` only, one authority each. "`std::env::current_dir()` is a
syscall on the render path" — one call in a `use_hook`, and R5's command-line
argument replaces it in phase 04.

*What the fresh `qa-confirm` confirmed after the first round of fixes, and what
followed.* Five of twenty-four raw findings survived adjudication, four of them
second-order — defects in the fixes or in the prose describing them, which is
exactly what an adversarial pass is for. The worker thread held TWO senders into
the update channel (one owned by the exit guard, one captured by the closure),
and on the discover-FAILURE path the captured one outlived the guard, so the
wake that announces "the worker is gone" could fire while the channel was still
open and a task asking for a second update would park forever. A narrow race —
the test written for it passed against the broken code too, and is kept as a
contract pin with that said in its doc comment — so it is closed by the type
instead: `Outbox` is no longer `Clone`, one sender lives on the thread, and the
guard owns it. `worker/mod.rs`'s own doc still carried the sentence `CLAUDE.md`
had just been corrected for, claiming the partition separates threads when it
separates files. And `state.md` still described the unvirtualised list as "inert
today because nothing populates that vector" — this is the change that
populates it.

*Adjudicated as dismissed, with the reasons recorded.* `std::env::current_dir()`
on the render path is inside a `use_hook`, which runs once at mount rather than
per render. The pre-push hook "missing" the worker tests was never a defect: it
deliberately runs no test suite at all.

*Recorded in `state.md` as constraints for phase 04 rather than fixed here.*
Every `submit` supersedes and a superseded page delivers nothing, so a scroll
handler that submits per tick would starve the view until the user stopped
moving — debounce. A live session's memory is flat in history length but linear
in rows scrolled, with no idle drop. And `main.rs` clones the visible model on
every reactive change, which the 256-row cap bounds and virtualisation will
replace.

**Enforcement-layer parity, done in the same change.** `docs/qa-gate.md`'s
dispatch table said the `responsiveness-reviewer` had no deterministic twin; it
has one now, and the row says what the guard decides and what is left to
judgement. `.claude/hooks/qa-stop.sh` gained the half of the partition a line
scan can express — nothing outside `crates/cairn-app/src/worker/` names `gix` or
`cairn_git`, and nothing inside it names `freya` — and deliberately does not try
the waiting half, because telling `handle.join()` from `root.join("crates")`
needs the matcher rather than awk with no parser.

*Debris dismissal, per the qa-gate contract.* This change adds one `#[ignore]`d
test and two `eprintln!` lines, all three of which `.claude/hooks/qa-stop.sh`
classifies as blocking debris. They are O3's measurement harness and its output,
reasoned in its doc comment, and they match the precedent set by phase 02's O2
harness and the `Oid` benchmark. The hook's remedy text still names `tracing`, a
crate this workspace does not depend on; that was escalated in the entry below
and is not patched here.

**The Freya side, for the record.** Delivery to the window is a push, not a
poll: `worker/wake.rs` is a one-slot latch that a worker sets and the waiting
task's `Waker` is woken from. The application root drives it from one `spawn`ed
task, so the UI thread yields to its event loop between pages instead of holding
it. No timer, and no async-runtime dependency — the alternative was `tokio` for
its channels and timers, which is a dependency decision and was not taken. The
one-task executor the tests use is hand-written on `std::task::Wake`, since this
workspace forbids `unsafe` in tests too.

## 2026-09-15 — `Oid` made fixed-width, and its justification corrected

Decision 4 from phase 02, built between phases in packet mode on
`feature/history-graph`. `cairn_model::Oid` is now the digest itself — `[u8; 32]`
holding 20 bytes of SHA-1 or 32 of SHA-256, plus the width that tells the two
apart — and it is `Copy`, 33 bytes, with `Option<Oid>` still 33 because the width
supplies the niche. `Oid::from_bytes` is the new cheap boundary: `cairn-git`'s
`model_id` copies gitoxide's digest instead of rendering hex and parsing it back,
and `object_id` hands the digest straight to `ObjectId::try_from`. `as_str` is
gone, because there is no string to borrow: `Oid::hex` and `Oid::short` format
into an `OidHex` buffer the caller owns. `short()` no longer slices `[..7]`, so
the panic that hazard carried is gone rather than moved.

**The justification did not survive its own measurement, and that is the
headline.** The decision was taken on the sentence "the assigner's lane scan is
an O(open lanes) string comparison per parent per row — the term behind a
measured 12.6x gap between laying out 1 lane and 200". If that were the term, the
gap would have collapsed. It did not. A re-runnable harness now lives in the tree
(`measures_layout_cost_against_lane_count`, `#[ignore]`d, medians of three after
a warm-up), and running it against the OLD representation as well as the new one
gives, per commit laid out, over 50,000 commits in release:

| lanes open | `String` id | fixed-width id |
| --- | --- | --- |
| 1 | 261 ns | 247 ns |
| 2 | 267 ns | 257 ns |
| 8 | 336 ns | 312 ns |
| 32 | 628 ns | 569 ns |
| 200 | 2,128 ns | ~1,750 ns |

The 200-lane point is the noisy one — eight paired runs gave 1,610-2,204 ns
before and 1,250-1,941 ns after, apparently bimodal — so read it as a median,
not a figure. **The ratio between 1 lane and 200 went from 8.2x to about 7x.**
Two corrections follow. The 12.6x could not be reproduced at all: this harness
measures 8.2x on the unchanged code, so the original number's method is
unrecoverable and it should be treated as superseded. And the gap is not an id
comparison: every open lane puts a passing segment on every row, built and pushed
one at a time, so a 200-lane row constructs about 200 segments whatever an id
costs to compare. Making the comparison cheaper shaved about 20% off the
marginal cost per open lane per row — 9.4 ns to 7.6 ns and left the shape alone. **The attribution
was wrong; the change still pays, on other grounds.**

Those grounds, measured on the engine harness the same way — a 200,001-commit
synthetic repository built with `git fast-import`, 16 interleaved branches,
periodic merges, **no commit-graph file**, 50,000 commits walked from `HEAD`,
release, medians of three:

| order | object cache | walk only | walk + lane assignment |
| --- | --- | --- | --- |
| graph order, before | none | 96 ms | 208 ms |
| graph order, after | none | 80 ms | 178 ms |
| commit time, before | 4 MiB | 108 ms | 138 ms |
| commit time, after | 4 MiB | 93 ms | 117 ms |

The default path — commit time, 4 MiB — is about 15% cheaper end to end, 2.76 us
per commit down to 2.34, and the walk-only column moves too because `model_id`
no longer allocates twice per id. Retained state falls as well: an id costs 33
inline bytes where it used to cost a 24-byte header plus a 40-byte heap block, so
the assigner's `gone` deque is 540 KB flat instead of 393 KB plus 16,384 separate
allocations. Nothing moved the wrong way: `CommitSummary` grew 128 to 144 bytes
and `GraphRow` 56 to 72, against a 200-branch row's ~8.6 KB of segments.

**QA: four fresh agents, 29 raw findings, adjudicated by a fresh `qa-confirm`.**
Agents, none of them the implementer: `qa-checklist` (NOT READY, 7 findings),
`test-coverage-auditor` (8 findings from 24 mutations run against a scratch copy),
`responsiveness-reviewer` (5, with its own size and timing measurements) and a
correctness/dead-code pass (9). Adjudication merged 5 duplicate ids, confirmed
15, dismissed 7 and escalated 3.

*Confirmed and fixed here.* The parser's tests could not tell the two halves of a
hex pair apart — every invalid fixture was invalid in both — so either nibble
check could be deleted and the suite stayed green; one mistyped character in a
pasted SHA would have become a well-formed WRONG id. Pinned now at both halves of
a pair and at the ends of the string, along with the lengths either side of each
width (32, 39, 41, 63, 65) and the byte counts either side of each digest (19,
21, 24, 31, 33) — `40 => Sha1` widened to `32..=40`, `64 => Sha256` widened to
`n >= 64`, and `from_bytes` widened to accept 24 all survived before, and all die
now. Cross-width ordering had no test, so the field order carrying the struct's
own ordering claim was unpinned; swapping the fields now fails. `Display` and
`Debug` for both types had no assertion at all. Two doc claims over-reached and
were narrowed: `OidHex` does not let the history list render without allocating —
Freya's `label().text()` takes `Cow<'static, str>`, so a component must own its
copy — and the benchmark cannot compare two representations when only one exists.
`commit_row.rs` now spells the copy `.short().as_str().to_string()`, since the
change had quietly rerouted that line through a formatter. `OidHex` is canonical
past its length, so a later `PartialEq` derive cannot tell two identical
abbreviations apart; `as_bytes`'s unreachable fallback returns an empty digest,
which fails at the boundary, rather than the whole buffer, which would look up
the wrong object. The benchmark itself now warms up and takes medians, because
its divisor was its noisiest point.

*Dismissed, with reasons.* "The ignored benchmark's only assertion duplicates an
existing test" — it is declared a measurement, not an assertion, and that
assertion checks the harness rather than the code. "Two non-decisive assertions
in the round-trip test" — `SHORT_HEX` is pinned by a literal elsewhere, so
neither leaves a hole. "`Debug` output changed from `Oid("...")` to `Oid(...)`"
— nothing parses it. "`OidParseError` should be `#[non_exhaustive]`" — both
crates are `publish = false`, so an in-tree exhaustive match failing to compile
is the stronger signal. Three inherited items were dismissed as out of scope and
recorded in `state.md` instead of fixed: `cairn-app`'s unvirtualized list (inert
until the query is wired, phase 04's), the unbounded tip set (phase 02 code this
change only makes cheaper), and replay paging (already open decision 1).

*Debris dismissal, per the qa-gate contract.* This change adds one `#[ignore]`d
test and two `eprintln!` lines, all three of which `.claude/hooks/qa-stop.sh`
classifies as blocking debris. They are the benchmark and its output, deliberate
and reasoned in its doc comment, matching the precedent phase 02 set in
`crates/cairn-git/src/history.rs` — which was never logged, so this entry logs
both. The hook's own remedy text names `tracing`, a crate this workspace does not
depend on; that is an enforcement-layer question, raised below rather than
patched here.

*Escalated to the user, not an agent's to take.* Cairn is SHA-1 only in practice:
`cairn-model` accepts SHA-256 ids, but gix is built without its `sha256` feature,
so a SHA-256 id fails at `object_id` — enabling the feature is a dependency
decision. `OidHex` carries no `PartialEq`/`AsRef`/`Deref`, which is a seam-type
trait-surface call better made with phase 04's call sites in hand. And the debris
hook's remedy text names an absent crate while scanning only uncommitted lines,
so the rule is unenforceable the moment work is committed.

## 2026-09-15 — phase 02's four decisions, settled by the user

**1. The live walk session (blocks phase 03, now unblocked).** Replay paging is
O(page index x limit) and does not reach A7's size. Phase 03 will keep gitoxide's
walk alive for the life of a scroll, making paging O(limit); phase 02's cursor
demotes to the cold-restart path rather than being removed. This fits D3 exactly:
the walk borrows the repository and is not `Send`, and the worker already owns
its `to_thread_local()` handle for its whole lifetime, so the session never
crosses a thread. Recorded as R2.5. Rejected: capping paging depth (concedes the
packet's headline capability), and building the session as its own phase before
03 (cleaner boundaries, but an unplanned phase for a design the worker has to
carry anyway).

**2. The assigner's blind spot — closed by 1, not carried.** Beyond
`window + remembered` rows of skew the assigner cannot tell a parent already gone
from one still to come. A live walk already holds the seen-set that answers it
exactly, so the fix rides along with decision 1 instead of costing memory
proportional to commits walked, which is the shape R1.3 was narrowed to avoid.

**3. R1.3 reworded again, to the bound phase 02 measured.** Upward repaints run
down lanes chosen by `free_lane_across`, which are not in the assigner's lane
table, so "open lanes" never counted them: on a history with no branching at all,
window 256 with one late parent per row gives 129 segments on one row and 24,768
retained. R1.3 now reads `window x lanes in play across it`, naming both terms.
Phase 02 was right to TRAP-mark rather than reword an in-flight requirement; the
TRAP is now discharged and the PRD carries no TRAPs.

**4. `Oid` becomes fixed-width.** Every id allocates today and the assigner's
lane scan is an O(open lanes) string comparison per parent per row — the term
behind a measured 12.6x gap between laying out 1 lane and 200. Done now, before
phases 03 and 04 build on it, and while phases 01 and 02 are green and freshly
reviewed so the blast radius is visible. The text form stays available at the
edges, because that is what the UI and the `git` binary both speak.

## 2026-09-15 — phase 02: the history query, the assigner's window, and O2 measured

Built in packet mode on `feature/history-graph`. Five deliverables: the bounded
resumable query (`crates/cairn-git/src/history.rs`), parent ids and commit time
taken from the walk rather than from decoded objects, a cancel signal polled once
per commit (`src/cancel.rs`), integration tests against repositories built by
running real `git` (`crates/cairn-git/tests/`), and the window the assigner owns
(`crates/cairn-model/src/lane_assignment.rs`).

**O2 is resolved by measurement, and the answer was not the default.** gitoxide's
docs say `ByCommitTime` "benefits greatly" from an object cache, implying the
unaccelerated path looks each commit up twice. It does. Harness:
`measures_both_orders_against_a_named_repository`, `#[ignore]`d, driven by
`CAIRN_BENCH_REPO`. Repository: a 200,001-commit history built with `git
fast-import` (16 interleaved branches, periodic merges into the trunk), **no
commit-graph file**, 50,000 commits walked from `HEAD`, release build. Medians of
three runs, in milliseconds:

| order | object cache | walk only | walk + lane assignment |
| --- | --- | --- | --- |
| graph order (`BreadthFirst`) | none | 101 | 196 |
| graph order | 4 / 16 / 32 MiB | 101 | 195 |
| commit time (`ByCommitTime`) | none | 178 | 190 |
| commit time | 4 MiB | 116 | 130 |
| commit time | 16 / 32 MiB | 117 | 129 |

Three conclusions. The doc's claim is real: a commit-time walk costs 53% more
without an object cache, and 4 MiB captures the whole win — larger caches bought
nothing measurable, so `Repository::discover` installs 4 MiB. Graph order is
cheaper to *walk* and dearer to *lay out*, because it interleaves branches and so
leaves far more lanes open per row; end to end commit time wins, 130 ms against
195 ms. And no stopping rule fired — 2.6 us per commit walked and laid out is
affordable, so D4's shape is unchanged. The default is `HistoryOrder::CommitTime`
because it is both the readable order and the cheaper one.

A control worth recording: the same harness against `/home/alexparlett/Development/freya`
(2,540 commits reachable from `HEAD`) shows the two orders within noise of each
other — that repository **has** a commit-graph file, which is exactly the case
O2 was asked about the absence of.

**The cursor replays.** gitoxide's walk holds a priority queue and a seen-set
that borrow the repository, so it cannot be serialised into a value. Resuming
therefore re-walks from the same pinned tips and skips what earlier pages
covered. That is what makes two pages of N describe the same commits in the same
lanes as one page of 2N, and it is stable by construction rather than by luck.
The rejected alternative was a cursor carrying the walk's frontier: with
committer-date skew a commit already emitted can be an ancestor of a frontier
tip, so the next page would emit it twice, and detecting that needs the set of
everything emitted. The cost of replaying is page *k* walking *k x limit*
commits, and it does not scale to the size A7 names — see the decisions below.

**The window, and the defect it introduced.** The assigner keeps at most
`window` rows and hands each row back as it leaves; a row that has left is final
because the assigner no longer holds it, which is what makes R1.2's finality
clause enforceable at last. Room is made *before* the new row goes in rather than
after, so the window never reaches one row further back than it says it does.

Eviction introduced one real defect, found by three fresh reviewers
independently and reproduced here before it was fixed. Before the window, a
parent that was neither reserved nor still on screen could only be one still to
come, so reserving a lane for it was right. Eviction added a second case: a
parent the walk delivered early whose row has since left the window. The walk
emits each commit once, so that reservation was never consumed — a line
descending a lane nothing could ever free, painted on every row after it. Lanes
grew linearly with the walk and retained segments quadratically (40 leaked lanes
after 40 such events; 2,023,804 segments and 48.6 MB over 200k rows at one event
per 100 commits, a 34x per-row regression), and because leaked lanes ride out on
the rows the caller keeps, it would have widened the drawn graph without bound
too. The assigner now keeps the bare ids of commits that have left the window —
16 per row of window, about 1 MB against roughly 9 MB of rows at the default —
and recognises such a parent instead of reserving for it. Past that horizon the
two cases are genuinely indistinguishable without remembering every commit
walked, which R1.3 forbids; that residual is one of the decisions below.

**QA: four fresh agents, 34 raw findings, adjudicated by a fresh `qa-confirm`.**
Agents, none of them the implementer: `qa-checklist` (NOT READY, 9 findings),
`test-coverage-auditor` (11 findings, 12 mutations applied and reverted),
`responsiveness-reviewer` (14 findings, with its own release benchmarks), and a
correctness/dead-code pass (14 findings). Adjudication: 24 confirmed after
merging 8 duplicate pairs, 3 dismissed, 4 escalated to the user.

*Confirmed and fixed here.* The lane leak above. The cursor carried the order but
not the window, so resuming after `with_window` silently reverted to the default
and could renumber lanes across a page boundary. A limit of zero reported the
history exhausted and swallowed the cursor `resume` consumes by value. The page
paired rows with commits through two sequences that had to stay in step, guarded
by a `debug_assert` compiled out of release; it indexes by walk position now, so
drifting apart is not expressible. Dead API found by deletion — a blanket
`Cancel` impl nobody selected, an unused `limit()` accessor, an unused fixture
parameter — is gone.

*Confirmed pins that decided nothing, and now do.* R2.3's was the worst: reading
parents from a decoded object on **every** walked commit left all fifteen tests
green, because `decoded` is a counter the query increments beside its own object
read. It is observable now — the fixture writes a commit-graph and deletes the
loose objects of the commits a resumed page replays, so any decode of a
merely-walked commit fails to find it, with a negative proving the objects really
are unreadable. The R1.3 bound derived the lane count from the rows it was
bounding and so moved with any defect; it is pinned against the history's own
width now. Also newly decisive: `assign_all` past the window (dropping every
finalised row had passed all 24 model tests), the look-ahead's cancellation
check, `HistoryRow::id` reading the wrong half, a smoke test that passed on an
empty page, and graph order, the resume guards and bad starting points, which
had no tests at all.

*Dismissed, with reasons.* "Cancellation discards the whole page" — abandonment
is the specified contract (R2.4, D3: a superseded query is abandoned rather than
rendered), so returning a partial page would be the defect. "`OBJECT_CACHE_BYTES`
has no enforcement twin" — gix 0.87.1 exposes only a setter, so the only twin
expressible is a timing assertion, and this phase deliberately keeps timing out
of the gate. "The blanket `Cancel for &T` impl is untested" — it no longer
exists; it was deleted as dead in the same pass.

*Confirmed and NOT fixed, because they are the user's call.* The four decisions
below, plus two low residuals recorded rather than patched: the model's
acceptance corpus is still far shorter than `DEFAULT_WINDOW`, so the streaming
test's finalised-row arm stays dormant (the dedicated `assign_all` test covers
the mutation that mattered); and A5's cancellation observable cannot tell
"stopped walking" from "kept walking without polling", which the deleted-objects
technique could now pin.

**Four decisions batched to the user, none of them an agent's to take.**

1. *Resumption does not scale to A7's size.* Page *k* walks *k x limit* commits:
   258-420 ms for one page at depth 100k, 1.29-2.1 s at 500k, and 54-87 minutes
   of CPU to page sequentially to 500k at 100 rows a page. R2.2 is met; A7 is not
   reachable through this API. The proposed cure keeps gitoxide's walk alive
   inside phase 03's worker as a session — the worker already owns its repository
   handle for its lifetime — demoting today's cursor to the cold-restart path.
   That changes `cairn-git`'s public surface, so phase 03 should not start until
   it is settled. The same decision covers random access: there is no total row
   count (counting is a full walk) and no way to build a cursor from an offset,
   so a scrollbar drag has no answer today.
2. *The assigner's remaining blind spot.* Beyond `window + remembered` rows of
   skew, a parent already gone cannot be told from one still to come. Options:
   leave it (a leak returns in that range), age reservations (bounds it
   unconditionally, but a merge whose second parent is far down the walk loses
   the tail of a line that is correct today), or move the seen-set into the query
   — exact, loses nothing, and costs memory proportional to commits walked, which
   is the shape R1.3 was narrowed to avoid. Worth knowing before rejecting the
   last: gitoxide's walk already holds exactly such a set.
3. *R1.3's stated bound is not the true worst case.* Upward repaints run down
   lanes chosen by `free_lane_across`, which are not in the assigner's lane table
   at all, so the "open lanes" term does not count them. Measured on a history
   with **no branching whatsoever**: window 256 and one late parent per row gives
   129 segments on a single row, 130 lanes wide, 24,768 segments retained — per
   row O(window), retained O(window^2). The honest bound is `window x (open lanes
   + concurrent repaint lanes)`. Rewording an in-flight requirement is the user's.
4. *`Oid` is a `String`.* Every id allocates, `model_id` allocates twice, and the
   assigner's lane scan is an O(open lanes) string comparison per parent per row
   — the term behind a measured 12.6x gap between laying out 1 lane and 200. A
   fixed-width `[u8; 20]`/`[u8; 32]` `Oid` would make those register comparisons,
   but it is a `cairn-model` design change and the text form is what lets the UI
   name a commit without linking the engine.

Also recorded for phase 03, in `state.md`: `Repository` holds a thread-local
`gix::Repository` with no `ThreadSafeRepository` route, so R3.1 needs new public
surface in `cairn-git`.

## 2026-09-15 — R1.3 narrowed, Freya moved to our fork, packet rebased on main

Three integration decisions between phases 01 and 02, none of them phase 01's to
make.

**R1.3 is narrowed; the window moves inside the assigner.** Phase 01 measured the
shipped assigner at 361 segments per row and ~5.4 GB across 500k rows on a
200-branch history with no clock skew at all — the ordinary "show all branches"
view. The user chose narrowing the requirement over changing the design. R1.3 now
states the real bound (retained state is window x open lanes), and R1.2's
"rows that have left the window are final" is repaired rather than deleted: it is
enforceable exactly when the window belongs to the assigner, because an assigner
cannot repaint a row it has evicted. A window imposed from outside never could.
Phase 02 owns building it, and owes the test phase 01 deliberately did not write —
writing it then would have ratified the change before the user made it. The
rejected option was keeping R1.3 as written and computing passing segments at
paint time, which buys O(open lanes) by giving up the self-contained row that
phase 04's virtualised painting depends on.

**Freya now comes from our fork**, github.com/alexparlett/freya, pinned at
caa46f87 — the same revision the other Freya projects in this tree pin. The
reason is a policy, not a version: a toolkit limitation gets fixed in the fork,
never worked around in Cairn. `deny.toml` denied git sources outright, so it
gained `allow-git` for that one source with its reason recorded. The fork's crates
are numbered 0.5.0-rc.4 against crates.io's rc.6, but they carry the builder API
Cairn is written against: `cairn-ui` and `cairn-app` compile unchanged. The
in-flight workflow is strata's and is now in the root manifest's comment — patch
the git source to a local clone while a fork fix is being written, never commit
that patch as the shipping build. `phase-04-graph-view.md` and `state.md` both
sent agents to the vendored rc.6 source and were corrected.

**The packet was rebased onto main** at 7f9c648, which had moved four docs commits
ahead during phase 01 and revised this packet's own plan: R5/A8 (opening a
repository) added and A8 renumbered to A9, phase 03 told to design against fetch
as a second consumer, and phase 03 declared parallelisable. Orchestration stays
sequential regardless — phases share one branch, and the plan's parallelism is an
optimisation while a collision on that branch is a correctness problem. One doc
defect arrived with those commits and is fixed here: the phase-03 note had been
inserted inside the validation-status table, splitting it above the `05` row.

## 2026-09-14 — phase 01 QA: four fresh agents, 19 raw findings, 14 confirmed

Correction to the entry below, which said "see the entry below this one once
`/qa` has run": this log is newest-first, so the QA record is here, above it.
Second correction: that entry said lane bookkeeping is one slot per open lane
and left the impression that retention is rows x constant. The term that
actually grows is the retained *edge lists* — one segment per open lane on every
row, so rows x open lanes. See the TRAPs now on R1.2 and R1.3 in the PRD.

Agents, all spawned fresh, none the implementer: `qa-checklist` (verdict NOT
READY, 10 findings), `test-coverage-auditor` (9 findings, 16 mutations applied
and reverted), `responsiveness-reviewer` (7 findings, dispatched on the
qa-checklist's judgement call that the assigner is "what runs per repository
query" even though no UI crate changed). Adjudication went to two separate
`qa-confirm` agents, never inline.

**Confirmed and fixed here.** The whole-picture check the test suite was
missing: what leaves the bottom of a row must be exactly what enters the top of
the next, no lane carrying two lines at once. An edge count only bounds lines
that leave a node, so a fabricated `Passing` or `IntoCommit` — a line drawn from
nowhere — was invisible; four mutations that survived the original suite now
fail. A second skew fixture across a busy span, because the first has one
intermediate row and every lane below it occupied, so it could distinguish
neither a partial repaint from a full one nor the free-lane rule from its
degenerate case. A3's gate on a repainted segment now derives the rows entitled
to be repainted from the walk order itself rather than trusting the flag the
code under test set, with an in-order generated control that may not repaint any
row at all. The duplicate-commit case pins its layout instead of only its row
count. `state.md` no longer claims a QA pass that had not happened. The root
`CLAUDE.md` and `cairn-model`'s crate doc no longer call the crate plain data
now that a stateful algorithm lives there.

**Dismissed, with reasons.** Packet-mode branch authority (the orchestrating
prompt declared it explicitly, and the repository has no remote). "A2's doc
comment is mis-attributed" — the named mutation does fail A2; what the auditor
found was that a one-intermediate-row fixture cannot distinguish partial from
full repaint, which is the coverage gap fixed above. Two of three "unreachable
defensive branch" claims: the reversed-range guard prevents a panic and the
`let ... else` on a missing entry is the non-panicking idiom the clippy floor
mandates — unreachable today is not a defect when the alternative is a panic in
a git client. "No cancellation in the assigner API" — under D3 the epoch belongs
to `cairn-app`'s worker and R1.5 requires the assigner to stay pure, so a
cancellation signal in `cairn-model` would be the defect. "The API forces a
clone at the worker boundary" — D3 hands results across as owned values by
design, and `GraphRow` is self-contained so a range copies without the graph.

**Confirmed and NOT fixed, because it is the user's call.** R1.3 is unmet in
both clauses and R1.2's "rows that have left the window are final" has nothing
enforcing it. An agent may not rewrite a requirement in an in-flight PRD, so
both carry a `TRAP:` marker naming what to trust instead — the sanctioned
interim step — and the disposition is batched to the user. Nothing pins whatever
bound replaces R1.3; that test cannot be written before the decision, because
pinning today's behaviour would silently ratify the change.

**One prescribed fix turned out to be impossible.** `free_lane_across` picks the
lowest lane free across a span; the reuse-a-hole branch was found unwitnessed,
and the prescribed fix was a fixture that exercises it. There isn't one:
`free_slot` always fills the lowest hole, so at the row where a parent delivered
early sits, lanes 0..its own are contiguously occupied, and a hole can only lie
above it. Probing roughly 12,000 generated histories with 415 deep backward
links produced zero reuses. Left as written — the lowest-free answer is the
better one if it ever becomes reachable — and recorded here rather than papered
over with a fixture that does not decide it.

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
