# Progress — history-graph

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

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
