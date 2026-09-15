# How mature git clients scroll, page and bound memory over a large history

Evidence record. Gathered 2026-09-15 for the `history-graph` packet, to settle
what the right scroll and memory model is for a history view over an unbounded
repository — and whether the packet's `LaneAssigner` window, replay cursor and
live walk session are three parts of one design or three answers to one question.

## Method, and what each class of evidence is worth

Three tiers, and every finding below says which one it rests on.

- **Measured here.** Two synthetic repositories built with `git fast-import` and
  timed against the installed `git 2.55.0`: one linear (200,000 commits, one
  file, no branches) and one branchy (200,001 commits, 9 refs, eight long-lived
  lanes, a nine-parent merge every 25 commits). Warm cache, times are the median
  shape of repeated runs. **Caveat that bounds every number below:** the trees
  are one file and the messages are a dozen bytes, so object decode is far
  cheaper than in a real repository. Treat these as a *floor* on cost and a
  statement about *complexity*, not as predictions of absolute time.
- **Read the source.** `gitk` ships as a readable Tcl script and was read in
  full locally at `/usr/bin/gitk` (git 2.55.0; upstream
  <https://github.com/git/git/blob/master/gitk-git/gitk>). `tig`, `lazygit` and
  the VS Code Git Graph extension were read at the GitHub URLs cited per finding.
- **Reported.** Fork, Sourcetree and GitKraken are closed source. Nothing below
  claims to describe their implementation. Each finding is labelled DOCUMENTED BY
  VENDOR, CONSISTENTLY REPORTED BY USERS, SINGLE USER REPORT, or INFERRED, and an
  inference is never written as a fact.

## Part A — what git itself gives you, and what it does not

### Finding 1 — a plain log streams; a graph log buffers the whole reachable set

`git log --graph` "implies the `--topo-order` option by default" (git 2.55.0
`man git-log`). Measured time to the **first output row**:

| Repository | plain `git log` | `--graph` | `--topo-order` | `--date-order` |
| --- | --- | --- | --- | --- |
| linear 200k | 1 ms | 337 ms | 311 ms | 305 ms |
| branchy 200k (`--all`) | 1 ms | 360 ms | 340 ms | 337 ms |

For reference, a full `git rev-list` over the same linear repository is 324 ms.
The first `--graph` row therefore costs as much as walking everything: ordering
is not a streaming operation, it buffers.

The git documentation never says this. `Documentation/rev-list-options.adoc`
describes `--topo-order` and `--date-order` in ordering terms only, with no
statement about memory or cost; the one option in that file that admits to the
problem is `--simplify-merges`, which says it "requires walking the entire commit
history before returning a single result"
(<https://github.com/git/git/blob/master/Documentation/rev-list-options.adoc>).

**Confidence:** measured here, plus a verbatim doc quote.
**Implies:** a view that wants a first row promptly cannot ask the ordering layer
for a global order first. Either it streams in whatever order the walk produces
and lays out incrementally, or it waits for the whole history.

### Finding 2 — a commit-graph file collapses that, and it is optional

Writing `git commit-graph write --reachable` (12 MB for 200k commits) changed the
same measurements:

| Operation | no commit-graph | with commit-graph |
| --- | --- | --- |
| `--graph` first row, linear | 337 ms | 1 ms |
| `--graph --all` first row, branchy | 360 ms | 1 ms |
| `rev-list --count`, linear | 291 ms | 32 ms |
| `rev-list --count --all`, branchy | 315 ms | 32 ms |
| peak RSS, `--graph` full walk, linear | 107 MiB | 41 MiB |

Generation numbers let the ordering start emitting without the in-degree pass.
But writing a commit-graph is maintenance the user may never have run, and Cairn
cannot require it: the same repository is fast or slow by a factor of ~300 on
first-row latency depending on a file that may not exist.

**Confidence:** measured here.
**Implies:** any design that only works with a commit-graph needs a degraded path
that still works without one, and any design that ignores the commit-graph is
leaving a 300x acceleration on the table on repositories that have one.

### Finding 3 — there is no random access into history, with or without acceleration

`--skip=<number>` is documented as "Skip `<number>` commits before starting to
show the commit output" and says nothing about cost. Measured cost of
`git log --skip=N -n 100`:

| N | linear, no c-graph | linear, c-graph | branchy, no c-graph | branchy, c-graph |
| --- | --- | --- | --- | --- |
| 0 | 1 ms | 1 ms | 0 ms | 0 ms |
| 50,000 | 95 ms | 13 ms | 80 ms | 7 ms |
| 100,000 | 152 ms | 16 ms | 160 ms | 15 ms |
| 190,000 | 278 ms | 28 ms | 302 ms | 27 ms |

Linear in N in all four columns. The commit-graph divides the constant by roughly
ten; it does not change the complexity. Asking for "rows 500,000 to 500,100"
costs walking 500,000 commits, always.

**Confidence:** measured here.
**Implies:** Cairn's replay cursor is not an unfortunate implementation of
paging — it is the *only* thing git offers for positional access, and it has the
cost it has because that cost is intrinsic. No amount of engineering makes an
offset seek cheap. Whatever answers "drag the scrollbar to depth 500k" cannot be
an offset.

### Finding 4 — a total row count is a full walk

`rev-list --count` measured at 291 ms (linear) and 315 ms (branchy) without a
commit-graph, 32 ms with one. That is the same order as the full walk, because it
is the full walk.

**Confidence:** measured here.
**Implies:** "Cairn has no total row count" is not a gap to be closed cheaply. A
scrollbar whose thumb is proportional to total history is buying a full walk
before the first paint, unless a commit-graph exists.

### Finding 5 — what grows in git's own memory is the walk, not the render

Peak RSS of `git log --graph` over the whole history: 107 MiB (linear 200k) and
114 MiB (branchy 200k, `--all`) without a commit-graph, 41 MiB with one. That is
roughly 0.55 KiB per commit for a repository whose commits are almost empty, and
it is spent on the traversal's own commit set, not on anything rendered.

Separately, the one thing git's graph renderer *does* bound is width, not depth:
`--graph-lane-limit=<n>` limits "the number of graph lanes to be shown. Lanes
over the limit are replaced with a truncation mark `~`" (git 2.55.0
`man git-log`), default 0 meaning no limit.

**Confidence:** measured here; the lane-limit quote is from the shipped man page.
**Implies:** two different budgets exist and git distinguishes them. Row count
and lane count are separately unbounded, and a design can bound one without the
other.

## Part B — open-source clients: mechanism

### Finding 6 — gitk never blocks the UI on the walk; it reads the pipe in chunks

`start_rev_list` (`/usr/bin/gitk`) spawns one
`git log --pretty=raw --parents --boundary` subprocess, sets the channel
non-blocking, and registers `getcommitlines` with `filerun`. `getcommitlines`
does `read $fd 500000` per call and returns to the event loop; when the read
yields nothing and the pipe is not at EOF it returns 1 to be called again. The
window is live from the first chunk.

**Confidence:** read the source.
**Implies:** the oldest graph client in wide use already had Cairn's D3 shape —
one long-lived walk, incremental delivery, UI never waiting. This is the baseline
expectation, not an ambition.

### Finding 7 — gitk retains every commit it has ever read, forever

`getcommitlines` stores the raw commit body as `commitdata($id)` per commit, and
topology in the arc arrays initialised by `varcinit` (`varccommits`, `varcid`,
`parents`, `children`). Grepping the whole script, `commitdata` is written per
commit and **never unset anywhere** — not on view reset, not on scroll. The arc
arrays are the only thing `resetvarcs` clears, and only when a view is rebuilt.
There is no eviction path keyed to scroll position, and no cap of any kind.

**Confidence:** read the source.
**Implies:** gitk's memory grows monotonically with commits *read*, which on a
completed load means the whole repository. It buys responsiveness and unrestricted
scrolling with unbounded retention, deliberately.

### Finding 8 — gitk's scrollbar is sized to rows read so far, and is deliberately throttled

`setcanvscroll` sets the canvas scrollregion to
`canvy0 + (numcommits - 0.5) * linespc + 2`, where `numcommits` is the count read
so far. `layoutmore` refuses to re-set it on every arrival:

```tcl
if {$lastscrollrows < 100 || $viewcomplete($curview) ||
    [clock clicks -milliseconds] - $lastscrollset > 500} {
    setcanvscroll
}
```

The toolbar shows the selected row over `numcommits` (`ttk::label
.tf.bar.numcommits ... -textvariable numcommits`) — a running count, never a
total.

**Confidence:** read the source.
**Implies:** a scrollbar over a history of unknown length is a *loaded-rows*
scrollbar that grows, and the growth is rate-limited so the thumb does not
shimmer. gitk does not pretend to know the total and does not try to.

### Finding 9 — gitk anchors the viewport to a commit id, not to an offset

`drawvisible` keeps `targetid` (a commit) and `targetrow` (where it currently
sits). When rows arrive above it and the row moves, gitk computes
`diff = (r - targetrow) * linespc`, calls `setcanvscroll`, and shifts the view by
that many pixels so the anchored commit stays under the cursor. `commitonrow`
materialises a row's id on demand via `make_disporder`.

**Confidence:** read the source.
**Implies:** this is the direct answer to "the list grows while the user reads
it". The stable identity of a scroll position is an `Oid`, not an index — which
is exactly the shape of a resume token.

### Finding 10 — gitk computes lane layout lazily, per visible range, and treats it as revisable

`drawcommits` calls `layoutrows` only for the range it is about to draw, extended
by lookahead constants (`uparrowlen` 5, `downarrowlen` 5, `mingaplen` 100), then
runs `optimize_rows` over a trailing range. `rowfinal` is a per-row flag marking
whether that row's layout is settled; `undolayout` truncates `rowidlist`,
`rowfinal` and `rowisopt` from a row downward and sets `need_redisplay` when
layout must be redone.

**Confidence:** read the source.
**Implies:** layout in gitk is neither global-up-front nor write-once. It is
computed on demand for what is visible, kept revisable, and discarded when
invalidated. Layout is a cache, not a record.

### Finding 11 — gitk bounds the *render* at 2000 rows while leaving the data unbounded

In `drawcommits`:

```tcl
if {$need_redisplay || $nrows_drawn > 2000} {
    clear_display
}
```

`clear_display` deletes every canvas item, unsets `iddrawn` and `linesegs`, and
resets `nrows_drawn` to 0; the visible rows are then redrawn. `nrows_drawn` is
incremented per row actually drawn.

**Confidence:** read the source.
**Implies:** the single most transferable idea in this record. gitk keeps **three
separate budgets** — unbounded commit data, lazily computed and discardable
layout, and a hard 2000-row cap on retained render objects with wholesale
eviction and redraw. Scrolling past the bound and back costs a redraw of the
visible rows only, because the data and the layout are still there.

### Finding 12 — tig: incremental graph, unbounded lines, and an honest position indicator

`main_read` (`src/main.c`) is called per parsed line and calls
`main_register_commit`, which calls `graph->add_commit` then
`graph->render_parents` immediately; `src/graph-v2.c` needs only `prev_row`,
`row` and `next_row`, so layout is O(1) state per commit. Retained lines grow via
`DEFINE_ALLOCATOR(realloc_lines, struct line, 256)` in `src/view.c` with no cap
anywhere in the source. `update_view` reads only what `io_can_read` reports, so
loading is progressive; `move_view` with `REQ_MOVE_LAST_LINE` jumps to
`view->lines - 1`, the last line read *so far*, not the end of history. While the
pipe is open, `update_view_title` prints the total rounded down to the nearest
`update_increment` (100) rather than a real count.
<https://github.com/jonas/tig/blob/1b86f07/src/main.c>,
<https://github.com/jonas/tig/blob/1b86f07/src/graph-v2.c>,
<https://github.com/jonas/tig/blob/1b86f07/src/view.c>

**Confidence:** read the source.
**Implies:** independent confirmation of Findings 8 and 10 in a different
codebase: incremental per-row layout is sufficient, and an approximate,
visibly-rounded position indicator is an acceptable UI for an unknown total.

### Finding 13 — lazygit: a 300-row cap whose only escape is loading everything

`commit_loader.go` builds the log command with `ArgIf(opts.Limit, "-300")`;
`opts.Limit` is threaded from `LocalCommitsContext`'s `limitCommits atomic.Bool`,
default true. `local_commits_controller.go` defines `COMMIT_THRESHOLD = 200`, and
its focus handler does:

```go
if context.GetSelectedLineIdx() > COMMIT_THRESHOLD && context.GetLimitCommits() {
    context.SetLimitCommits(false)
    self.c.Refresh(...)
}
```

Crossing row 200 drops the cap entirely and re-runs an unbounded `git log`.
Search and "show whole git graph" do the same. `GetPipeSets(commits []*models.Commit, ...)`
takes the whole slice and recomputes every pipe on each render.
<https://github.com/jesseduffield/lazygit/blob/71d3e7d/pkg/commands/git_commands/commit_loader.go>,
<https://github.com/jesseduffield/lazygit/blob/71d3e7d/pkg/gui/controllers/local_commits_controller.go>,
<https://github.com/jesseduffield/lazygit/blob/71d3e7d/pkg/gui/presentation/graph/graph.go>

**Confidence:** read the source.
**Implies:** a bound with a cliff. It is cheap to build and it is not a scroll
model — there is no page 3.

### Finding 14 — VS Code Git Graph: a monotonically growing prefix, fully re-laid out each time

`config.ts` defaults: `repository.commits.initialLoad` 300,
`repository.commits.loadMore` 100, `loadMoreCommitsAutomatically` true.
`dataSource.ts` calls `getLog(repo, branches, maxCommits + 1, ...)` — the extra
row is a peek to decide whether a "load more" is available — and `getLog` runs
`git log --max-count=<num>`, so the bound is a real git argument, not a
client-side truncation. `web/main.ts`'s `loadMoreCommits` does
`this.maxCommits += this.config.loadMoreCommits` and re-requests; `web/graph.ts`'s
`Graph.loadCommits` then resets `this.vertices`, `this.branches` and
`this.availableColours` and rebuilds the entire graph over the new, larger array.
<https://github.com/mhutchie/vscode-git-graph/blob/d7f43f4/src/config.ts>,
<https://github.com/mhutchie/vscode-git-graph/blob/d7f43f4/src/dataSource.ts>,
<https://github.com/mhutchie/vscode-git-graph/blob/d7f43f4/web/main.ts>,
<https://github.com/mhutchie/vscode-git-graph/blob/d7f43f4/web/graph.ts>

**Confidence:** read the source.
**Implies:** paging by re-walking a growing prefix from HEAD — the same cost
curve as Cairn's replay cursor (page *k* walks *k* x limit), accepted because the
limit is 300+100k and nobody is expected to reach depth 500,000. It also
re-derives the whole layout on every page, which Finding 10 shows is not
necessary.

### Finding 15 — no readable client implements bounded rows with random access

Across gitk, tig, lazygit and Git Graph, the mechanisms are: unbounded retention
(gitk, tig), a cap that is abandoned wholesale (lazygit), or a growing prefix
re-fetched from HEAD (Git Graph). None supports seeking to an arbitrary depth
without having loaded everything above it, and none evicts loaded rows and
re-fetches them on scroll-back.

**Confidence:** read the source, for all four.
**Implies:** if Cairn wants bounded retained rows *with* scroll-back, it has no
precedent to copy and is inventing. That is not a reason not to do it, but it
means the design cannot lean on "this is how clients do it."

## Part C — closed-source clients: reported behaviour

### Finding 16 — Sourcetree pages the log explicitly, and Atlassian says it is for memory

A setting "Log rows to fetch per load" exists under Tools/Options/General; in the
same Atlassian support thread an Atlassian participant gives the reason as
libgit2's behaviour, that it "can consume excessive memory unless it is batched".
Users report configuring values from 10 to 50,000. The user-visible indicator
while a batch loads is the "fetching older commits" state.
<https://community.atlassian.com/forums/Sourcetree-questions/Speeding-up-quot-Fetching-older-commits-quot/qaq-p/587217>

**Confidence:** DOCUMENTED BY VENDOR for the setting's existence and the stated
reason; CONSISTENTLY REPORTED BY USERS for the values and the indicator. The
default value is not stated in any source found.
**Implies:** the client Cairn is positioned against has a user-visible page size
whose justification is memory, and the paging is load-more, not windowed seek.

### Finding 17 — Sourcetree's paging is the fragile part, and has been for years

A public bug on Atlassian's tracker reports that with the setting at 500, "if I
attempt to scroll down and load the next 500 rows, SourceTree will end up in a
death loop", across values from 10 to 10,000, with 50,000 as the workaround —
i.e. set the page large enough that the second page never happens.
<https://jira.atlassian.com/browse/SRCTREEWIN-14669>. The support thread above
carries a comment years later that it is still the same.

**Confidence:** SINGLE USER REPORT for the loop (on the vendor's own tracker);
CONSISTENTLY REPORTED BY USERS for the general slowness persisting.
**Implies:** the incremental-fetch path is the one that rots, because it is the
one exercised least during development. Whatever Cairn builds here needs a test
that actually pages more than once.

### Finding 18 — Sourcetree shows no evidence of a retained-row bound in practice

An out-of-memory report describes memory growing "from several GB to almost
100GB" after opening a repository
(<https://community.developer.atlassian.com/t/sourcetree-out-of-memory-exception/79165>).
Atlassian's own "Sourcetree appears to be slow" KB attributes slowness to low
disk space and network drives and never mentions commit count, history depth or
a memory ceiling (<https://support.atlassian.com/sourcetree/kb/sourcetree-appears-to-be-slow/>).

**Confidence:** SINGLE USER REPORT for the 100 GB figure; DOCUMENTED BY VENDOR
for what the KB does and does not say. Neither isolates the cause to the commit
list. This is evidence of *no working bound*, not proof of unbounded rows.
**Implies:** treat as weak. It is consistent with the pattern in Part B, not
independent confirmation of it.

### Finding 19 — Sourcetree's answer to "go to an arbitrary commit" is search, not the scrollbar

Navigating to a deep commit is done through a "Jump to commit/branch" box that
takes a SHA or ref.
<https://community.atlassian.com/forums/Sourcetree-questions/How-do-I-search-by-commit-hash-on-Sourcetree-2-5-5-in-windows/qaq-p/787306>

**Confidence:** CONSISTENTLY REPORTED BY USERS for the feature's existence. That
this implies the scrollbar is not a seek affordance is INFERRED, and no source
states it.
**Implies:** an identity-based seek is the affordance users are actually given.
Same shape as Finding 9's anchor and Finding 22's cursor.

### Finding 20 — Fork exposes no total commit count, and there is evidence of a bounded graph window

Two open feature requests on Fork's public tracker ask for commit counts that do
not exist: one for search result counts and position
(<https://github.com/fork-dev/TrackerWin/issues/441>), one asking Fork to "count
the max number of commits in history panel"
(<https://github.com/fork-dev/TrackerWin/issues/1854>). Separately, a bug report
about a branch's commits being invisible carries the reporter's guess that "Fork
seems to only show the last X commits in the graph, presumably for performance
reasons ... I'm not sure exactly what this limit is (1000?)"
(<https://github.com/fork-dev/TrackerWin/issues/663>, against Fork 1.45).

**Confidence:** the absent count is INFERRED from two open feature requests —
the strongest available evidence, but an absence argument. The commit limit is a
SINGLE USER REPORT, explicitly a guess by the reporter, on an old version.
**Implies:** the client Cairn is most directly positioned against appears to have
solved the total-count problem by not having a total count. Do not cite the
"1000" figure as a number; cite only that a bound of some kind was observed once.

### Finding 21 — Fork loads commit details separately from the commit rows

Fork's release notes record "Load revision details in background thread" and
"Implemented repository refresh in background thread" (1.0.19, 2016), and
"Revision log loading is 3 times faster now" (1.0.30, 2016)
(<https://git-fork.com/releasenotes>). A bug report on a ~100,000-commit
repository describes commit messages rendering blank, where "scrolling the view
down slightly and waiting a few seconds causes it to draw correctly"
(<https://github.com/fork-dev/TrackerWin/issues/2847>).

**Confidence:** DOCUMENTED BY VENDOR for the changelog lines (2016, may not
describe current behaviour); SINGLE USER REPORT for the blank messages. That
these two describe the same lazy-detail mechanism is INFERRED and could equally
be a redraw bug.
**Implies:** weak support for splitting "the row exists at this position" from
"the row's text is loaded" — a split Cairn's `CommitSummary` currently does not
make.

### Finding 22 — GitKraken's documented remedy for a large history is truncation

GitKraken's own performance troubleshooting page instructs users to "Lower the
Max Commits in Graph value in Preferences > General to reduce the number of
commits rendered", naming large repositories with many references as the trigger.
<https://help.gitkraken.com/gitkraken-desktop/performance-issues/>

**Confidence:** DOCUMENTED BY VENDOR.
**Implies:** a hard cap you cannot scroll past, surfaced to the user as a knob.
It bounds memory perfectly and answers the scroll question by deleting it.

### Finding 23 — GitHub, at the largest scale, pages by commit id and publishes no total

The web commit list's "Older" control is a `?after=<sha>+<n>` link — a resume
from a named commit, not a page number, and the page shows no total commit count
(<https://github.com/git/git/commits/master/>). The REST endpoint offers
`page`/`per_page` with `per_page` capped at 100
(<https://docs.github.com/en/rest/commits/commits>), while GraphQL's
`Commit.history` is a cursor connection taking `first`/`after` with a 1-100 range
and no backwards pagination
(<https://docs.github.com/en/graphql/guides/using-pagination-in-the-graphql-api>).

**Confidence:** DOCUMENTED BY VENDOR for the API shapes; observed for the web URL
form.
**Implies:** the organisation with the most git history and the most incentive to
make deep paging cheap ships next-page-from-a-commit-id and declines to offer a
total or an arbitrary jump. Findings 3 and 4 say why.

## What this evidence does NOT settle

- **Whether any client evicts loaded rows and re-fetches them on scroll-back.**
  No client in this record does. The question is therefore open on its merits,
  with no precedent either way.
- **Whether Fork or Sourcetree compute lane layout globally or incrementally.**
  Nothing public was found on either. Findings 20 and 21 are the closest, and
  neither addresses layout.
- **How Fork sizes its scrollbar, and what a drag into unloaded depth does.** No
  public information found.
- **Sourcetree's default "Log rows to fetch per load" value.** No source found
  states it.
- **Whether gitk's 2000-row render cap is tuned or arbitrary.** The source
  carries the constant with no comment and no test.
- **Absolute costs on a real repository.** The measurements here use synthetic
  histories with one-file trees. They pin complexity (Findings 1-5) and should
  not be read as predicting wall-clock time on a real monorepo, where object
  decode dominates.
- **Whether gix's walk behaves like `git rev-list` under these same shapes.**
  Everything in Part A was measured against the `git` binary. Cairn's reads go
  through gitoxide, and `docs/research/history-graph/gix-revwalk-ordering.md`
  already establishes that gix offers no `--topo-order` equivalent at all — so
  Finding 1's buffering cost is one Cairn does not currently pay, and Finding 2's
  commit-graph acceleration is one it may not currently get.

## What this implies for Cairn's window, cursor and session

The packet's three mechanisms are not three answers to one question. The evidence
separates them cleanly, and separates a fourth thing the current design has no
name for.

**The cursor is a cold-start path, and its cost is intrinsic (Findings 3, 23).**
Replay-and-skip is `--skip`, and `--skip` is O(N) even with a commit-graph. That
is not an implementation defect to be optimised away. Every client that pages by
position pays it (Finding 14 pays it and gets away with it only because its
depths are small). GitHub, at the largest scale, resumes from a commit id
instead. The evidence supports the cursor existing as the resume-from-cold path
and does not support it as the scroll mechanism.

**The session is the industry-standard scroll mechanism (Findings 6, 12).** One
long-lived walk, delivery in chunks, UI never waiting — gitk has done exactly
this since before Cairn existed, and tig does it too. That the session "retains
every row it has laid out" is also what gitk and tig do (Findings 7, 12): the
norm is unbounded retention. The open question is whether Cairn wants to match
the norm or beat it, and the evidence does not answer that — it only says that
beating it has no precedent (Finding 15).

**The window is currently doing three jobs that the evidence says are separate
budgets (Findings 5, 10, 11).** gitk keeps commit data unbounded, layout lazily
computed and *revisable*, and rendered objects hard-capped at 2000 with wholesale
eviction and redraw. Cairn's `LaneAssigner` window conflates retention, layout
finality and page sizing into one 1024-row number. Two specific consequences:

- **"Rows evicted from the window are final" has no support in this record and
  one argument against it.** gitk deliberately keeps layout revisable
  (`rowfinal`, `undolayout`, `optimize_rows` over a trailing range), and
  `gix-revwalk-ordering.md` Finding 2 establishes that commit-time order can
  deliver a parent after its child, which is precisely a case where an earlier
  row's lane may need to change. Finality is a choice with a cost, not a free
  property of a window.
- **Bounding the render is cheap and separable from bounding the data.** Finding
  11 is the one bound in the record that survives scroll-back gracefully, and it
  works because the data behind it was never dropped.

**On the scrollbar drag, the evidence supports three options and rules out a
fourth.** Ruled out: a scrollbar proportional to total history, with positional
seek, on a repository without a commit-graph — Findings 3 and 4 price both the
count and the seek at a full walk. Supported:

1. **gitk's answer (Findings 8, 9).** Scrollbar sized to rows loaded so far,
   growing as the walk advances, re-set at most every 500 ms, with the viewport
   anchored to an `Oid` so content under the cursor does not slide. Honest,
   proven, and Cairn already has the anchor type.
2. **Identity seek instead of positional seek (Findings 19, 23).** Jump to a ref,
   a SHA or a date; the scrollbar covers loaded rows only. This is what Sourcetree
   gives users and what GitHub's API gives clients.
3. **Commit-graph-accelerated offset seek as an opportunistic fast path
   (Finding 2).** A count at 32 ms and a 190,000-deep skip at 27 ms make a real
   scrollbar defensible *when the file exists*. It cannot be the only path, and
   it doubles the design rather than replacing it.

**On initial load, the evidence is unanimous (Findings 1, 6, 12, 14).** Paint the
first page as soon as it is known; never wait for a total; never wait for a global
order. Cairn's session already does this, and Finding 1 says the thing that would
break it is asking for a global ordering first.

**On bounding retained rows, the honest statement is that Cairn would be first.**
Finding 15: nobody in this record evicts and re-fetches. The cost of re-fetching
an evicted region is exactly the cursor's cost from Finding 3, which is why no
one does it. If the design pass wants bounded retention, the thing that makes it
affordable is not a smaller window — it is a cheaper way back, and this record
found only two candidates for that: a commit-graph (Finding 2, optional) and
anchoring resumption on an `Oid` near the target rather than an offset
(Findings 9, 23).
