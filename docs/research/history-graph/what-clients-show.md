# What mature git clients actually SHOW in a history graph, and what the user can scope

Evidence record. Gathered 2026-09-16 for the `history-graph` packet, to settle a
question nobody in the packet had asked: **what is on screen in a commit-graph
view by default, and how many lanes does that actually come to?**

Its companion, `docs/research/history-graph/scroll-memory-model.md`, covered
scroll, paging and memory MECHANISM — how rows arrive and what is retained. It
never asked what those rows contain. This record covers only the product surface:
default ref scope, scoping controls, lane counts, and what else is in the view.
Findings are not repeated between the two.

## Why this record exists

Phase 01 benchmarked a 200-branch history, measured 361 edge segments per row and
~8.5 KB per row, and the packet's memory analysis has been bounded against that
number ever since. But "the graph shows all branches at once" entered the packet
as a *benchmark parameter* and was then written into the PRD as "the ordinary
view" without anyone checking it against a real client. If real clients scope to
one branch, open lanes are single digits and ~8.5 KB per row is an artefact.

The project owner's observation to test: **Fork shows no memory or performance
strain in normal use** — and the hypothesis that this is because Fork never has
200 lanes open, not because its eviction is cleverer.

The short answer is that the hypothesis is **half right, and wrong about the
half that mattered**. Fork's default really is all refs (so the packet's premise
survives), but the lane count that default produces is 2-3 in ordinary use rather
than 200 (so the benchmark's cost model does not).

## Method, and what each class of evidence is worth

Fork and Sourcetree are closed source. Nothing below claims to describe their
implementation, and no inference is written as a fact. Every finding carries one
of these labels:

- **DOCUMENTED BY VENDOR** — a vendor KB page, manual or release note.
- **VENDOR STATEMENT IN A PUBLIC TRACKER** — for Fork specifically, a comment by
  Dan Pristupov, who is listed on <https://git-fork.com/> "About Us" as Fork's
  developer and whose GitHub account carries company `@fork-dev` and blog
  `fork.dev` (<https://api.github.com/users/DanPristupov>). This is weaker than a
  KB page (it is conversational and can be dated) and stronger than a user report.
- **MEASURED FROM A PUBLISHED SCREENSHOT** — a published image was downloaded and
  its graph gutter scanned programmatically, counting distinct coloured vertical
  runs per scanline and snapping them to the detected lane pitch. The method and
  the source URL are given so the number can be rechecked.
- **VISIBLE IN AN OFFICIAL SCREENSHOT** — read off a vendor-published image.
- **CONSISTENTLY REPORTED BY USERS** / **SINGLE USER REPORT** — community reports,
  with the count distinguished.
- **READ THE SOURCE** — open-source clients only.
- **INFERRED** — reasoning over the above, never stated as observation.

GitHub issue metadata (state, dates, reaction counts, comment bodies) was taken
from `api.github.com` rather than the rendered pages, because the rendered pages
summarise unreliably.

## Part A — Fork

### Finding 1 — Fork's default view is a global all-refs graph, and it is literally named "All Commits"

Fork's sidebar carries a fixed item **`All Commits`** directly under `Changes`,
above the `Branches` / `Remotes` / `Tags` / `Stashes` / `Submodules` sections. It
is the selected item in Fork's own marketing screenshot for Windows
(<https://git-fork.com/images/carousel/carousel_mainWin.jpg>) and in the
screenshot Fork's developer posted of the `facebook/react` repository
(<https://github.com/fork-dev/Tracker/issues/85#issuecomment-324906327>). Users
and the vendor both refer to the default view by that name — a 2026 bug report is
titled "Unable to see branch in **'all commits' view** unless I filter to only
that branch" (<https://github.com/fork-dev/TrackerWin/issues/2828>).

That it shows all refs rather than the checked-out branch is settled by the
developer's own answer to a 2017 request for a current-branch-only view:

> commits which belong to the active branch are black, and the ones which don't
> are gray

— DanPristupov, <https://github.com/fork-dev/Tracker/issues/85#issuecomment-324906327>

A view containing commits that do not belong to the active branch is not scoped
to the active branch.

**Confidence:** DOCUMENTED BY VENDOR (the marketing screenshot) plus VENDOR
STATEMENT IN A PUBLIC TRACKER.
**Implies:** the packet's premise that "all branches at once" is the ordinary
view is CORRECT for Fork. What it does not establish is what that costs — see
Finding 6.

### Finding 2 — Fork's default is global-with-highlighting, not scoped; clicking a branch selects, it does not filter

This is the distinction the brief flagged as looking similar and differing
completely in cost, and for Fork the evidence is direct. The de-emphasis
mechanism is Finding 1's black/gray split: every commit is present in the list,
and membership of the active branch is conveyed by text colour. Both official
screenshots show it — in the Windows marketing shot, "Accept new baselines",
"Add tests" and "Normalize ... fix getTypeFacts" render grey among black
neighbours.

Clicking a branch in the sidebar moves the selection; it does not re-scope:

> In Fork, currently it seems like clicking on a branch name (just) pinpoints on
> this branch's last commit, in a sea of branches and commits.

— andersennl, <https://github.com/fork-dev/Tracker/issues/637#issuecomment-503450159>

The same thread carries the counter-proposal that clicking a branch *should*
scope ("When I click on a specific branch, I expect to see only this branch's
history"), which only makes sense if it currently does not. The request has been
open since 2019.

**Confidence:** VENDOR STATEMENT IN A PUBLIC TRACKER for the highlighting.
Sidebar-click-selects rests on a **SINGLE USER REPORT** — but a well-corroborated
one: it stands uncontradicted in a thread the vendor participates in, the same
thread's request for click-to-scope presupposes it, and Fork subsequently shipped
a *separate* funnel control to do the scoping (Finding 3), which would be
redundant if clicking already did it. Sourcetree's vendor documents the identical
behaviour explicitly (Finding 12), so the shape is not unique to Fork. It is still
one report, and it is labelled as one.
**Implies:** for Fork, selecting a ref is a cheap operation over an already-walked
graph, not a re-query. Whatever Cairn does on sidebar click, Fork's precedent
costs nothing at click time because it changes nothing about the walk.

### Finding 3 — scoping in Fork is a deliberate, per-ref, opt-in filter, and it is poorly discovered

There are two distinct mechanisms, added years apart.

**The branch filter** (a funnel icon that appears on hover beside a ref in the
sidebar) restricts the commit list to commits reachable from the filtered refs:

> You can click the "Filter" icon beside a branch's name to only see commits that
> are in that branch.

— clounie, <https://github.com/fork-dev/Tracker/issues/85#issuecomment-1185856632>

Multiple refs can be filtered together — Fork for Mac 1.0.59 (17 Nov 2017) lists
"Ability to select multiple branches to filter" — and the filter follows the
working tree: Fork for Windows 1.34 (8 Jun 2019) lists "Switch branch filter
automatically on checkout" (<https://git-fork.com/releasenotes>,
<https://git-fork.com/releasenoteswin>).

**Per-ref visibility (hide)** is separate and much newer. In 2019 the developer
stated flatly that it did not exist —

> (*currently*) it's not possible to hide particular branches or tags. However we
> can introduce an option to hide all tags/remote branches

— DanPristupov, <https://github.com/fork-dev/TrackerWin/issues/466#issuecomment-539253404>

He announced two coarse options for 1.41 — "hide remote branches from commit
list" and "hide tags completely" — and per-ref hiding landed only in 1.60 /
Mac 2.6 (Mar 2021), whose release notes read "Ability to hide particular branches,
folders or remotes" and "Ability to set branch filter on folders or remotes".
(The 1.41 line is a vendor statement of intent, not a release note; the 1.60 / 2.6
lines are release notes.)

A coarser scope control predates all of this on Mac: a menu item named
**"Show/Hide Remote Branches in Commit List"**, described by a user in 2019 as one
they "use quite a lot to switch between a clean and tidy tree to the complete one"
(<https://github.com/fork-dev/Tracker/issues/637>). Its state was not persisted
across restarts at the time, per a reply in the same thread — which means the view
reverted to showing remote branches on every launch.

The filter is a *positive* filter (name what to show); users specifically ask for
negation: "I have to constantly add local branches to the filter instead of just
saying what to **not** show" (sglienke,
<https://github.com/fork-dev/TrackerWin/issues/466#issuecomment-539689240>).

Discoverability is poor, and this matters for reading every other finding. Three
separate users, in 2019, 2022 and 2023, report not knowing the filter existed:
"Wow! So there is filtering for tree view! Never noticed these icons."
(<https://github.com/fork-dev/TrackerWin/issues/44#issuecomment-505483572>); "I
had been trying to mark everything invisible that I didn't want to see"
(<https://github.com/fork-dev/Tracker/issues/85#issuecomment-1185918622>); "I had
no idea those icons did that!"
(<https://github.com/fork-dev/Tracker/issues/85#issuecomment-1711427279>).

**Confidence:** DOCUMENTED BY VENDOR for the release-note lines and the 2019
absence; CONSISTENTLY REPORTED BY USERS for the filter's behaviour and its poor
discoverability.
**Implies:** the all-refs default is not merely the default — for a substantial
share of Fork's users it is the *only* mode they ever see, because they never
find the control. A design that assumes users will scope the view is assuming
something Fork's own users demonstrably do not do.

### Finding 4 — Fork deliberately does not filter the commit list by content, because it breaks the graph

Asked for an author/message filter over the history, the developer answered:

> We can't filter the commit history because this will break the graph.

— DanPristupov, <https://github.com/fork-dev/TrackerWin/issues/627#issuecomment-692602339>

Fork's search (introduced 1.52) is therefore a *separate results list* beside the
graph rather than a filter over it, and it searches all branches regardless of
any active branch filter — which users report as a regression:
"commits from filtered out branches were not taken into account when searching.
Now all branches are searched."
(<https://github.com/fork-dev/TrackerWin/issues/879>).

**Confidence:** VENDOR STATEMENT IN A PUBLIC TRACKER, corroborated by
CONSISTENTLY REPORTED BY USERS for the search behaviour.
**Implies:** a vendor with a decade in this problem treats ref-scoping and
content-filtering as different in kind. Dropping refs from the walk keeps the
graph well-formed; dropping individual commits does not. Cairn gets this
distinction for free if it scopes only by ref set.

### Finding 5 — Fork's answer to a graph that is too wide is to collapse it, not to render it better

Fork's one published feature about graph width is collapsible merges: "the
ability to expand and collapse merge commits in the commit graph by clicking on
their tips or using ←/→ keyboard shortcuts", so users can "collapse all branches
using the context menu of the graph and expand the ones you'd like to keep"
(<https://fork.dev/blog/posts/collapsible-graph/>, 3 Aug 2020, demonstrated on
`apple/swift`). Windows 1.50 (29 May 2020) lists "Ability to selectively collapse
branches in graph".

Asked directly for horizontal scrolling when the graph exceeds its column, the
vendor declined and pointed at collapse instead:

> I think the best way to navigate in a messy graph is to collapse all
> (View -> Collapse All Merges) and expand the branches which you need.
> […] Horizontal scrolling will not appear in the near future.

— DanPristupov, <https://github.com/fork-dev/Tracker/issues/516#issuecomment-1413400276>

Collapse state is persisted per repository in `.git/fork-settings`, and can
silently hide a branch from the "all commits" view — a 2026 bug where a branch
was missing turned out to be a collapsed merge, and the vendor's conclusion was
"An explicit click must always reveal a branch when it's collapsed"
(<https://github.com/fork-dev/TrackerWin/issues/2828>).

**Confidence:** DOCUMENTED BY VENDOR for the feature; VENDOR STATEMENT IN A
PUBLIC TRACKER for the refusal and the persistence.
**Implies:** the wide-graph case is real enough that Fork shipped a feature for
it, and Fork's chosen remedy reduces *rows and lanes together* rather than
paging, scrolling or truncating. Note also that graph width is capped by the
column, with no horizontal scroll: beyond the column width, lanes are simply
cropped.

### Finding 6 — measured lane counts in Fork: 2 and 3 in ordinary use, 21 in the vendor's own "messy" example, 118 in the worst user-reported case

This is the quantitative heart of the record. Four published Fork screenshots were
downloaded and their graph gutters scanned for distinct coloured vertical runs.

| Source | Repository | Refs in view | **Lanes measured** |
| --- | --- | --- | --- |
| Fork's Windows marketing screenshot, <https://git-fork.com/images/carousel/carousel_mainWin.jpg> | `microsoft/TypeScript` | 4 local branches + `origin` + tags, `All Commits` selected | **2** |
| Fork developer's screenshot, <https://github.com/fork-dev/Tracker/issues/85#issuecomment-324906327> | `facebook/react` | 4 local + ~16 remote branches visible in sidebar before it scrolls | **3** |
| Fork's own blog, <https://fork.dev/blog/posts/collapsible-graph/swift-show-all.png>, chosen by the vendor to illustrate a graph needing collapse | `apple/swift` (100k+ commits) | all | **≥21** (left edge cropped) |
| Same post, after collapsing, <https://fork.dev/blog/posts/collapsible-graph/swift-collapsed.png> | `apple/swift` | all | **3** |
| User's "graph too wide" report, <https://github.com/fork-dev/Tracker/issues/516> | not stated | all | **118** |

Method: each image's graph gutter was scanned per row for coloured runs against
the background, the lane pitch recovered from the run spacing, and centres snapped
to that pitch. In the marketing and react screenshots the pitch is 22 px and the
whole gutter is under 70 px of a 2000+ px window. In `swift-show-all.png` the
pitch is 24 px and 21 lanes are present on every scanned row with the leftmost
clipped by the crop, so 21 is a floor. In the `#516` screenshot (3360x2100) the
pitch is 22 px and the widest row carries 118 occupied lanes across 123 grid
slots, spanning 2702 px — the graph fills four-fifths of the window.

**Confidence:** MEASURED FROM A PUBLISHED SCREENSHOT for every number. The
repository identities are read off the window chrome and are unambiguous except
for the `#516` case, where the reporter does not name the repository.
**Implies:** the single most important number in this record. Fork's all-refs
default produces **2-3 lanes in the cases the vendor chooses to advertise**, very
nearly two orders of magnitude below the packet's 200. But the tail is real and is
worse than
the benchmark: one user reached 118 concurrent lanes. Lane count is not bounded
by anything, and it is also not predicted by ref count — see Finding 7.

### Finding 7 — lane count tracks concurrently-divergent history, not ref count

The react screenshot is the clearest case: the sidebar lists 4 local branches and
at least 16 remote-tracking branches (`origin/0.3-stable` through
`origin/15.1.0-dev` and more below the fold) with `All Commits` selected, and the
graph is **3 lanes wide**. Sourcetree's official screenshot is the same shape at
smaller scale — 6 local branches, 6 remote-tracking branches and a tag, all shown,
rendering in ~2 lanes (Finding 13).

Fork assigns lanes in recency order — "the commit graph in Fork displays branches
from left to right, sorted in order of their latest update"
(<https://github.com/fork-dev/TrackerWin/issues/2642>) — so a lane is occupied at
a given row only while that ref's history is still unmerged at that depth. Most
refs in a normal repository are either merged (their lane closes) or clustered
near the tip (their lane never opens at depth).

**Confidence:** MEASURED FROM A PUBLISHED SCREENSHOT for the counts; DOCUMENTED
BY VENDOR (tracker) for the recency ordering. That the mechanism *explaining* the
gap between 20 refs and 3 lanes is merge-closure is **INFERRED** — no source
states it, and an alternative explanation (that most react release branches are
ancestors of master and so never open a lane at all) is equally consistent with
the image.
**Implies:** any cost model keyed to `refs.len()` is keyed to the wrong number.
The packet's benchmark chose 200 branches and got 361 edge segments per row
because a synthetic 200-branch fixture keeps every branch open at every depth.
A real 200-branch repository need not, and the evidence here says it usually does
not.

### Finding 8 — Fork has no uncommitted-changes row in the history; it is a sidebar item, and the absence is a standing complaint

`Changes (n)` is a sidebar entry above `All Commits`, not a pseudo-row at the top
of the graph — visible in both official screenshots. Adding such a row has been an
open request since 2018 with 28 👍
(<https://github.com/fork-dev/Tracker/issues/308>), explicitly from users
migrating off Sourcetree, which does have one. The vendor's objection is about the
detail pane, not cost: "What should be shown in details view when the
'uncommitted changes' item is selected? That is my biggest concern in this
feature." and "I think it will be very confusing to show the changes in two views,
but allow to commit only in one."

**Confidence:** VISIBLE IN AN OFFICIAL SCREENSHOT for the absence; DOCUMENTED BY
VENDOR (tracker) for the reasoning; SINGLE USER REPORT class for each migration
complaint, but there are many of them over seven years.
**Implies:** the two clients Cairn is positioned against disagree on this, so it
is a product choice rather than a convention. It is also the one row in the view
that is not a commit and cannot come from the walk.

### Finding 9 — what else is in Fork's view

From the two official screenshots, consistent across Mac and Windows:

- **Columns:** graph and subject share one column; then Author (with avatar),
  abbreviated commit id (7 hex), and Date (`27 Nov 2020 07:21`). Four columns.
- **Ref decoration:** inline rounded labels immediately before the subject,
  coloured to match the branch's graph lane (Windows 1.34: "Draw branch labels
  using their graph colors"), carrying a host icon for refs with a known forge
  remote (`origin/fix41651`, `✓ master`).
- **No date grouping.** A flat list with a per-row date column; no "Today" /
  "Yesterday" section headers.
- **Commit detail pane** below the list, tabbed `Commit` / `Changes` /
  `File Tree`, showing Author and Committer separately, full SHA, parents as
  clickable links (both parents for a merge), subject and body, and the changed
  file list.
- **Branch ahead/behind counts** as pill badges in the sidebar, not in the graph.
- Ref labels are not wrapped or collapsed, and a commit carrying many refs pushes
  the subject off-screen — an open complaint, with the vendor asking "Why does a
  commit have a lot of branches? What is the use case?"
  (<https://github.com/fork-dev/Tracker/issues/516#issuecomment-1413400276>,
  <https://github.com/fork-dev/Tracker/issues/1635>).

**Confidence:** VISIBLE IN AN OFFICIAL SCREENSHOT, except the label-colour and
label-overflow points which are DOCUMENTED BY VENDOR.

### Finding 10 — on large and branchy repositories, Fork's reported pain is checkout and staging latency, not graph memory

This is the finding that tests the project owner's observation, and it is an
absence argument, so it is stated as one.

Searching both Fork trackers through the GitHub search API for memory and
large-repository performance returns very little. The most concrete report is a
user on a monorepo who published its statistics: **578 branches, 36,771 tags,
73,432 commits, 327 MiB pack**. Their complaint was that "changing a branch takes
~10s or more to complete" and that operations race — not that the graph is slow,
wide, or large (<https://github.com/fork-dev/Tracker/issues/28>). The only memory
figure found anywhere is "6 repos and over 1GB of memory usage" on macOS in the
same thread, unattributed to any component. A separate report describes freezing
on a repository with "more than 10k files changed per commit" — a tree-size
problem, not a history problem
(<https://github.com/fork-dev/TrackerWin/issues/122>).

Set against that, a user with **more than a thousand remote branches** opened a
request about Fork — and the request was to filter the *sidebar list*, not to fix
the graph (<https://github.com/fork-dev/TrackerWin/issues/44>).

**Confidence:** the individual reports are SINGLE USER REPORT. The *absence* is
observed (the searches were run and returned what is described). Any explanation
of the absence is **INFERRED**, and at least three survive the evidence: Fork's
graph genuinely costs little at realistic lane counts; or the lane counts users
hit are small for the reasons in Finding 7; or users hit legibility limits
(Finding 5, Finding 6's 118-lane case) and reach for collapse before they hit a
memory limit. This record cannot distinguish them.
**Implies:** the observation that "Fork shows no strain" is supported as an
observation, and is NOT evidence about eviction, retention or layout, because no
public source describes any of those for Fork.

## Part B — Sourcetree

### Finding 11 — Sourcetree documents its default as `git log --graph --all --date-order`, verbatim

Atlassian's own KB states it in one sentence:

> Graph log, by default, is essentially SourceTree's version of the command:
> `git log --graph --all --date-order`

— <https://support.atlassian.com/sourcetree/kb/viewing-log-history-of-a-repository/>

This is the strongest single piece of evidence in the record: the vendor names
`--all` and "by default" in the same sentence. It is corroborated by a Windows
3.4.32 release note (28 Aug 2026) fixing "Branch filter preference not saved and
resets to 'All Branches' on restart" — a bug that only makes sense if All Branches
is the fallback state
(<https://product-downloads.atlassian.com/software/sourcetree/windows/ga/ReleaseNotes_3.4.32.html>),
and by the KB's own annotated screenshot, which shows the dropdown reading
`All Branches` with `Show Remote Branches` checked.

**Confidence:** DOCUMENTED BY VENDOR.
**Implies:** Fork and Sourcetree agree. Both clients Cairn is positioned against
default to every ref. The packet's premise is not an artefact.

### Finding 12 — Sourcetree's scoping controls, and what its sidebar click does

Three controls sit on one toolbar row above the graph, all DOCUMENTED BY VENDOR at
the KB URL above:

- **`All Branches` / `Current Branch` dropdown.** "Users can choose to display all
  the branches or just the current checked out branch in the graph log by
  selecting **All Branches or Current Branch** in the drop-down list." `All
  Branches` is the default (Finding 11). "Current Branch" means the *checked-out*
  branch, not the sidebar selection — a seven-year-old request asks for it to be
  renamed for exactly that reason
  (<https://jira.atlassian.com/browse/SRCTREEWIN-542>).
- **`Show Remote Branches` checkbox**, checked in the official screenshot.
- **`Date Order` / `Ancestor Order` dropdown** — an ordering control (`--date-order`
  vs ancestry), not a filter.

Windows 3.4.27 (4 Dec 2025) replaced the two-option toggle with a picker —
"Enhanced Log/History with Selectable Branch Filtering" — closing a request from a
user with **135 remote branches** who wanted to show a subset
(<https://jira.atlassian.com/browse/SRCTREEWIN-2323>).

As in Fork, the sidebar selects rather than scopes:

> Clicking on any of the branch will cause SourceTree navigate to the branch's
> latest commit the graph log.

**Confidence:** DOCUMENTED BY VENDOR throughout.
**Implies:** two independent closed-source clients, built by different teams a
decade apart, landed on the same shape: global all-refs walk by default, an
explicit opt-in scope control, and a sidebar that navigates within the walk
rather than re-running it.

### Finding 13 — Sourcetree's official screenshot renders 13 refs in about 2 lanes

The KB's annotated screenshot
(<https://images.ctfassets.net/zsv3d0ugroxu/P7RxKd474iWoA40uRa3F7/5df1dc9dacb442c6ed8eec71f565881a/LogView.png>)
shows a repository with 6 local branches, 6 remote-tracking branches and 1 tag —
13 refs — with `All Branches` selected and `Show Remote Branches` checked, and
about 2 distinguishable lanes. The graph column is roughly a tenth of the width
given to Description.

**Confidence:** VISIBLE IN AN OFFICIAL SCREENSHOT. It is a toy repository (7
commits), so it bounds nothing about real histories; it is corroborating, not
load-bearing.
**Implies:** same direction as Finding 7 — refs shown ≫ lanes drawn.

### Finding 14 — Sourcetree's view carries an uncommitted-changes row, and Fork's does not

The row sits at the top of the graph, above the first commit and below the
Description header, and is enough of a first-class thing to have its own bugs
(<https://jira.atlassian.com/browse/SRCTREEWIN-6947>,
<https://jira.atlassian.com/browse/SRCTREEWIN-7112>). Users migrating to Fork miss
it specifically (Finding 8). Selecting it together with a commit diffs the working
tree against that commit — a capability Fork lacks, per a user who uses it often
(<https://github.com/fork-dev/Tracker/issues/308>).

**Confidence:** CONSISTENTLY REPORTED BY USERS, with the vendor tracker items as
corroboration that the row exists.

### Finding 15 — the rest of Sourcetree's view

- **Columns:** `Graph` | `Description` | `Date` | `Author` | `Commit`. Resizable
  and reorderable, persisted; no documented show/hide menu. The recurring
  complaint is that Description expands and pushes Date/Author/Commit off-screen,
  reported across versions from 2.0 to 4.1
  (<https://community.atlassian.com/t5/Sourcetree-questions/Date-and-Author-column-missing-in-Sourcetree-v2-7/qaq-p/715840>).
- **Ref decoration:** inline coloured labels on the row, including `N ahead` /
  `N behind` badges. A global option, Tools → Options → General → "Collapsed
  Tags/Branches of Log Row", collapses them to `...` when a commit carries several
  (<https://community.atlassian.com/forums/Sourcetree-questions/How-do-I-prevent-branch-tag-labels-being-truncated-in-the/qaq-p/2956386>).
  This is the control Fork lacks per Finding 9.
- **No date grouping** — a flat list with a per-row date column.
- **Search is a separate tab** (`File Status` | `Log / History` | `Search`,
  `Ctrl+1/2/3`), not a filter box in the graph toolbar — the same separation Fork
  arrived at in Finding 4.
- **`Jump to` control** at the top right of the log pane, accepting a commit hash.
- **Commit detail** below the graph: full hash, parents, author, date, labels, and
  the file list, with a diff pane beside it. Selecting multiple commits shows the
  cumulative diff between them.

**Confidence:** DOCUMENTED BY VENDOR for the detail pane, multi-select and the
toolbar; VISIBLE IN AN OFFICIAL SCREENSHOT for columns, decoration and the absence
of grouping; CONSISTENTLY REPORTED BY USERS for column behaviour and the label
option.

### Finding 16 — no Sourcetree complaint about graph WIDTH was found; the complaints are all about loading

Searching the Atlassian trackers and community for lane count, graph width or lane
stacking surfaced nothing Sourcetree-specific. What the search did surface, in
volume, is the batched scroll-loader hanging — "the screen will lock up and say
that it is loading 100 commits, but it will never finish", 9 votes / 11 watchers,
closed as a duplicate of a second such report
(<https://jira.atlassian.com/browse/SRCTREEWIN-14641>). Atlassian's own
performance KB names disk space, network drives and cache bloat, and names no
log-view, commit-count, branch-count or graph setting at all
(<https://support.atlassian.com/sourcetree/kb/troubleshooting-performance-problems-in-sourcetree/>).

**Confidence:** the absence is observed; both available explanations — that lane
assignment is good enough to go unremarked, or that users hit the loading wall
before the width wall — are INFERRED and this record cannot distinguish them.
**Implies:** across both closed-source clients, the reported failure mode of an
all-refs graph is *time to load rows*, not *width of the graph or size of a row*.
That is the opposite of where this packet's analysis has been spending.

## Part C — the wider field

Secondary clients, included only where they bear on default scope. The
open-source ones are the strongest evidence in the record because the default is
readable in the code.

### Finding 17 — the field splits cleanly, and the split is GUI-graph versus everything else

| Client | Default graph scope | Confidence |
| --- | --- | --- |
| **Fork** | all refs (`All Commits`) | Finding 1 |
| **Sourcetree** | all refs (`--all`, vendor-documented) | Finding 11 |
| **GitKraken** | all refs | INFERRED from vendor-documented hide/solo mechanics; no vendor sentence states the initial state |
| **Sublime Merge** | all refs — "The Commits column displays a graph of all commits contained in the repository" (<https://www.sublimemerge.com/docs/getting_started>) | DOCUMENTED BY VENDOR |
| **Tower** | all refs — "see the repository's **full** commit log - including local branches, remote branches, and tags" (<https://www.git-tower.com/help/guides/commit-history/display-commits/mac>) | DOCUMENTED BY VENDOR |
| **GitLens Commit Graph** | all local branches; `gitlens.graph.branchesVisibility` defaults to `"all"` (<https://help.gitkraken.com/gitlens/gl-commit-graph/>) | DOCUMENTED BY VENDOR |
| **VS Code Git Graph** (extension) | all refs — `--branches --tags --remotes <stash bases> HEAD` | READ THE SOURCE, Finding 19 |
| **VS Code built-in Source Control Graph** | HEAD-scoped — the reference picker defaults to `Auto`, "the current history item reference, its remote, and an optional base" (<https://code.visualstudio.com/updates/v1_94>) | DOCUMENTED BY VENDOR |
| **GitHub Desktop** | HEAD only — and draws no graph at all | READ THE SOURCE, Finding 20 |
| **gitk** | `HEAD` only | READ THE SOURCE, Finding 18 |
| **tig** | `HEAD` only | READ THE SOURCE, Finding 18 |
| **lazygit** | `HEAD` only | READ THE SOURCE, Finding 18 |
| **magit** | current branch only | READ THE SOURCE, Finding 18 |

Notably, Tower narrows by sidebar selection rather than by a toggle — "Select one
or multiple local or remote branches or tags to see a combined history of only
these refs" — which is the scoping behaviour Fork users asked for in Finding 2 and
did not get.

### Finding 18 — every readable client except one defaults to a single ref, and `--all` is opt-in in all of them

This is the strongest evidence class in the record, because the default is in the
code. All five were read at pinned commits.

**gitk — `HEAD` only.** View 0 is created with `set viewargs(0) {}`, and
`parseviewrevs` supplies the default explicitly:

```tcl
proc parseviewrevs {view revs} {
    ...
    if {$revs eq {}} {
        set revs HEAD
    } elseif {[lsearch -exact $revs --all] >= 0} {
        lappend revs HEAD
    }
```

Those revs are fed to `git log … --stdin` in `start_rev_list`. In the View editor,
"All refs" (`--all`) is an unchecked checkbox alongside "All (local) branches",
"All tags" and "All remote-tracking branches". The manual agrees: of
`gitk --max-count=100 --all -- Makefile` it says "Instead of only looking for
changes in the current branch look in all branches."
(<https://github.com/git/git/blob/f0ef1b96a076d08dc972a8d2cb0d1cfd60931eb6/gitk-git/gitk#L520-L527>,
`#L4302-L4310`,
<https://github.com/git/git/blob/f0ef1b96a076d08dc972a8d2cb0d1cfd60931eb6/Documentation/gitk.adoc>).
A saved view marked "Remember this view" is re-created at startup, but `curview`
stays 0 — **so even a user who has saved an `--all` view gets HEAD on open.**

**tig — `HEAD` only.** The main view's command template carries no ref and no
`--all`; `%(revargs)` expands to the command-line revs and to the empty string
when there are none, and `%(mainargs)` (`main-options`) has no default value. The
manual: "Display the list of commits for the current branch: `$ tig`" versus
"Pretend as if all the refs in `refs/` are listed on the command line:
`$ tig --all`". Opting in persistently is `set main-options = --all` in `~/.tigrc`;
there is no default key binding that toggles ref scope (`g` toggles the graph
column, not the scope).
(<https://github.com/jonas/tig/blob/1b86f070a1f6d4c686a09b997fd4249d52a2a272/include/tig/git.h#L53-L56>,
<https://github.com/jonas/tig/blob/1b86f070a1f6d4c686a09b997fd4249d52a2a272/doc/tig.1.adoc#L132-L145>)

**lazygit — `HEAD` only.** The commits panel passes `refForLog`, which returns
`"HEAD"` unless a bisect is running, and `ArgIf(opts.All, "--all")` is gated on
`showWholeGitGraph`, whose config default is `ShowWholeGraph: false`. Opting in is
`<ctrl+l>` → "Toggle show whole git graph" (which also drops the 300-commit
limit), or `git.log.showWholeGraph: true`. The separate `a` binding in the
*status* panel shells out to `git log --graph --all …` as text — git's renderer,
not lazygit's.
(<https://github.com/jesseduffield/lazygit/blob/71d3e7dfa5f9278172013bfa1bb83d60d155436a/pkg/commands/git_commands/commit_loader.go#L601-L609>,
<https://github.com/jesseduffield/lazygit/blob/71d3e7dfa5f9278172013bfa1bb83d60d155436a/pkg/config/user_config.go#L949-L953>)

**magit — the current branch only.** `l l` is `magit-log-current`, whose revs are
`(list (or (magit-get-current-branch) "HEAD"))`; `b` ("all branches") and `a`
("all references") are separate commands passing `("--branches" "--remotes")` and
`("--all")`.
(<https://github.com/magit/magit/blob/83ba66c8ab6fcdbd809077ae4db1f7e2ed832655/lisp/magit-log.el#L698-L705>,
`#L531-L543`)

Magit also shows *why* the two decisions are coupled: it bounds cost by rewriting
a single rev into `REV~LIMIT..REV`, and it **explicitly refuses to do so when the
scope is `--all` or `--branches`** (`magit-log.el` L1182-L1197). The cheap path
exists only because the default scope is one ref.

**Confidence:** READ THE SOURCE, pinned, for all four.
**Implies:** the split in Finding 17 is not GUI-versus-terminal quality, it is
**graph-drawing versus log-listing**. Every client whose selling point is a
rendered multi-lane graph defaults to all refs; every client that primarily lists
commits defaults to one. Cairn is building the former, which puts the all-refs
default squarely in its tradition — and inherits the cost the latter avoids.

### Finding 19 — one client bounds the graph's rendered WIDTH, and nobody bounds the lane count

Across gitk, tig, lazygit, magit and VS Code Git Graph, **no client caps the
number of lanes it computes.** tig's `graph_canvas` carries an unbounded `size`;
lazygit allocates exactly `maxPos+1` cells from whatever pipes exist; Git Graph's
`getAvailableColour` grows without limit (colours merely cycle); magit delegates
to git entirely. gitk has two prefs labelled "Maximum graph width (lines)" and
"(% of pane)" (`maxwidth 16`, `maxgraphpct 50`) that **appear vestigial** — at the
pinned sha they occur only in the preferences dialog, the prefs-changed
comparison and the persisted-variable list, and never in the layout math.

The single exception, and the one piece of real prior art for bounding the graph,
is VS Code Git Graph, which computes every lane and then bounds the *presentation*
at one third of the view width, fading the overflow with an SVG gradient rather
than clipping it hard:

```ts
let maxWidth = Math.round(this.viewElem.clientWidth * 0.333);
if (Math.max(graphWidth, colWidth) > maxWidth) {
    this.graph.limitMaxWidth(maxWidth);
```

(<https://github.com/mhutchie/vscode-git-graph/blob/d7f43f429a9e024e896bac9fc65fdc530935c812/web/main.ts#L1738-L1741>)

It is also the one readable client that defaults to all refs
(`--branches --tags --remotes <stash bases> HEAD`, with
`repository.onLoad.showCheckedOutBranch` defaulting to `false`), which is
presumably why it is the one that needed the bound.

Separately: `--graph-lane-limit=<n>` is a **`git log`** option new in git 2.55, not
a gitk one — a grep of the gitk source at the pinned sha finds no occurrence, and
magit is the only client that exposes it, as a hidden `:level 5` infix, off by
default.

What every client bounds instead is **commit count**: lazygit `-300`, Git Graph
300 + 100 per page, magit `-n256`, tig's shipped `contrib/large-repo.tigrc`
suggesting `-n 1000`, gitk offering `--max-count` in the view dialog.

**Confidence:** READ THE SOURCE throughout; the gitk-prefs-are-vestigial claim is
stated as "appears" because it rests on absence of a read site, not on a test.
**Implies:** rows are the budget the whole field bounds; lanes are the budget
nobody bounds. Cairn's packet has been treating them as one number, and this is
independent confirmation of the companion record's Finding 11 that they are
separate budgets. Git Graph's compute-all-bound-the-render is the only shipped
answer to lane width anywhere in this record, and it bounds pixels, not memory.

### Finding 20 — GitHub Desktop walks HEAD only, and renders no graph

`app/src/lib/stores/app-store.ts` loads the first page with the literal comment
"load initial group of commits for current branch" and the argument `'HEAD'`;
`getCommits` in `app/src/lib/git/log.ts` builds `git log HEAD --date=raw
--max-count=100 --skip=0 …`. Verified absent across `app/src`: `--all`,
`--topo-order`, `--date-order`, `--graph`. Parents are parsed into the commit
model and nothing consumes them for drawing; `commit-list.tsx` sets a fixed
`RowHeight = 50` and renders one text row per commit. Searching `app/src` and
`app/styles` for lane or graph rendering returns nothing.
(<https://github.com/desktop/desktop/blob/e25aac9bbce8e4431d81e79c81cc61d5b83d7cf0/app/src/lib/git/log.ts>,
<https://github.com/desktop/desktop/blob/e25aac9bbce8e4431d81e79c81cc61d5b83d7cf0/app/src/lib/stores/app-store.ts>)

A graph view is the client's most-requested missing feature —
<https://github.com/desktop/desktop/issues/9452>, open, 294 reactions, labelled
`not-planned` ("Not in the team's roadmap") — and has been since 2017
(<https://github.com/desktop/desktop/issues/1634>, 177 reactions).

**Confidence:** READ THE SOURCE, pinned.
**Implies:** the mass-market client that scoped hardest also declined to draw a
graph, and its users have spent nine years asking for one. Read with Finding 18,
that is the clearest statement in the record of what HEAD-only scope buys and what
it costs: the cheap walk, and a feature request with 294 reactions.

### Finding 21 — GitKraken is the only vendor that names REF COUNT as a performance trigger

> Performance issues are commonly linked to large repositories with many
> references.

> Common trigger: Large repositories with many references or heavy graph
> rendering.

— <https://help.gitkraken.com/gitkraken-desktop/performance-issues/>

Its remedies, in the vendor's order: lower the max-commits value; "Solo or Hide
branches and tags to reduce visual complexity"; "Delete unnecessary local branches
to reduce reference overhead." GitKraken also caps the graph by default —
"GitKraken Desktop initially displays up to 2000 commits in the Commit Graph"
(<https://help.gitkraken.com/gitkraken-desktop/search/>) — configurable as
"Initial Commits in Graph", minimum 500, alongside "Show All Commits in Graph"
("May affect performance on large repos") and "Lazy Load Commits"
(<https://help.gitkraken.com/gitkraken-desktop/preferences/>).

Its own product-page screenshot shows roughly 5-6 concurrent lanes
(<https://www.gitkraken.com/features/commit-graph>).

**Confidence:** DOCUMENTED BY VENDOR.
**Implies:** the one vendor that publishes a cost model for this says ref count
is a first-order term while shipping an all-refs default — and its remedy is to
cut rows and hide refs, i.e. to reduce both budgets by hand rather than to bound
either automatically.

## What the evidence does NOT settle

- **Whether Fork or Sourcetree walk all refs or merely *display* all refs.** Both
  could scope the walk to a ref subset and still present an all-refs view; nothing
  public distinguishes a global walk from a union of per-ref walks. Every finding
  here is about what is on screen.
- **What Fork's or Sourcetree's lane assignment costs per row.** No public source
  gives a memory figure, a lane cap, or a per-row size for either. Finding 10's
  absence of complaints is not a measurement.
- **Whether Fork bounds lane count at all.** Finding 6 measured 118 lanes, and
  Finding 5 establishes the graph column simply crops with no horizontal scroll —
  but whether lanes beyond the column are laid out and clipped, or never
  computed, is not observable from outside.
- **Whether the gap between ref count and lane count is merge-closure or
  ancestry.** Finding 7's mechanism is inferred; two explanations fit the images
  equally well. **This is the most load-bearing unsettled question in the record**,
  because the realistic lane count for Cairn's own benchmark depends on which it
  is.
- **Sourcetree's default "Log rows to fetch per load" value**, and whether the
  post-3.4.27 Windows branch picker still defaults to All Branches. The 3.4.32
  release note is the only hint that it does.
- **What "All Branches" includes beyond heads, remotes and tags** — stash refs,
  notes, `refs/*` generally. No public information found for either client.
- **Fork's behaviour on a repository with 200 long-lived *concurrently divergent*
  branches**, which is the packet's actual benchmark shape. The 118-lane
  screenshot is the closest evidence and its repository is not identified.
- **Whether any client's default scope was chosen for cost reasons.** No vendor
  states a rationale for the default anywhere found. GitKraken comes closest by
  naming ref count as a cost, but still defaults to all refs. Magit's refusal to
  apply its `REV~LIMIT..REV` optimization under `--all` (Finding 18) is the only
  place in the record where scope and cost are visibly coupled in code, and even
  there the comment does not say the default was chosen for it.
- **Whether gitk's "Maximum graph width" preferences still do anything.**
  Finding 19 establishes only that no read site exists at the pinned sha. Nobody
  appears to have tested them.
- **Why VS Code Git Graph chose 33% of the view width.** The constant carries no
  comment and no test, the same gap the companion record found behind gitk's
  2000-row render cap.
- **Lane counts on a real branchy repository in Sourcetree.** The only official
  screenshot is a 7-commit toy repo, and no community screenshot with countable
  lanes surfaced.

## What this implies for Cairn

### The premise survives; the cost model does not

"All branches at once" as the default view is **not** an artefact of the
benchmark — for the kind of client Cairn is. Fork (Finding 1), Sourcetree
(Finding 11), Sublime Merge, Tower, GitKraken and VS Code Git Graph all default to
every ref, and Sourcetree's vendor writes `--all` into its own documentation. The
PRD's "the ordinary view" is, on this evidence, correct.

The caveat is Finding 18: gitk, tig, lazygit and magit all default to a single
ref, and the line between the two camps is not GUI-versus-terminal but
**graph-versus-list**. Every client whose product is a drawn multi-lane graph
defaults to all refs; every client that primarily lists commits defaults to one.
Cairn has already chosen to be the former, so it has already chosen the all-refs
tradition — but that is a *product* argument, not a cost argument, and nothing in
this record shows any vendor choosing it for cost reasons.

What is an artefact is **200 lanes**. Fork's all-refs default measures at 2-3
lanes in the vendor's own screenshots of `microsoft/TypeScript` and
`facebook/react` — the latter with ~20 refs in the sidebar (Findings 6, 7), and
Sourcetree's official screenshot renders 13 refs in ~2 lanes (Finding 13). At 2-3
open lanes a row costs roughly what the brief estimated at ~400 B, not 8.5 KB.
The phase-01 fixture produced 361 edge segments per row because a synthetic
200-branch history keeps every branch divergent at every depth; the published
evidence says real repositories do not.

So: the packet has spent three days bounding a number that its own benchmark
manufactured. That is the finding. But it is **not** a finding that the problem is
imaginary — see the next section.

### The tail is real, and it is worse than the benchmark

One Fork user's published screenshot carries **118 concurrent lanes** (Finding 6),
comfortably past the packet's 200-branch fixture in lane terms if not in ref
terms. Fork's response to that case is instructive and is the opposite of what
this packet has been designing: it does not bound, evict or virtualize the graph.
It **crops** — the column has no horizontal scroll and the vendor has said it is
not coming — and it offers the user a manual remedy, Collapse All Merges
(Finding 5). GitKraken's documented remedy is the same shape: hide refs, delete
branches, lower the commit cap (Finding 21).

**No client in this record bounds computed lane count** (Finding 19). One bounds
the rendered width — VS Code Git Graph, at a third of the view, with a gradient
fade — and it is the only readable client that also defaults to all refs. Every
other client bounds rows instead, and several bound nothing at all. That is the
same conclusion the companion record reached about bounded row retention, and for
the same reason: the clients' answer is to make the *user* reduce the problem.

### The realistic open-lane count

On the published evidence: **2-3 lanes for ordinary repositories (Fork's own two
screenshots, Sourcetree's official one), 5-6 in GitKraken's marketing, ~21 for a
deliberately-chosen messy example on a 100k-commit repository, and a long tail
reaching ~120.** A design that is correct at 3 and degrades gracefully to 120
covers everything observed. A design that assumes 200 as the ordinary case is
designing for a case no published screenshot shows.

The open-source clients contribute nothing to this number, and that is itself
worth stating: they default to one ref, so their lane counts measure their
defaults rather than what an all-refs graph costs (Finding 18).

### The options the evidence supports

These are named, not chosen; that is the design pass's call.

1. **Match Fork and Sourcetree exactly** — all refs by default, an opt-in ref
   filter, sidebar selection that navigates rather than re-scopes (Findings 1, 2,
   11, 12). Highest fidelity to what Cairn is positioned against, and it inherits
   their unbounded-width tail. Cairn would need an answer for the 118-lane case,
   and the evidence says the incumbents' answer is "crop it and give the user a
   collapse key."
2. **All refs by default, computing every lane but bounding the RENDER** — VS Code
   Git Graph's shipped answer: cap the graph column at a fraction of the view and
   fade the overflow, so the truncation is visible rather than silent (Finding 19).
   This is the only precedent in the record for bounding graph width at all. Note
   what it does and does not buy: it bounds pixels and draw calls, not the lane
   state behind them, so it addresses legibility and frame time but not the
   packet's memory figure.
3. **All refs by default with a bounded lane BUDGET**, lanes past the budget
   merged or marked rather than tracked — `git log --graph-lane-limit=<n>`
   (git 2.55, quoted in the companion record's Finding 5) is the precedent for the
   marking, and magit exposes it, hidden and off by default (Finding 19). No GUI
   client does this. It is the only option
   that bounds the cost rather than the picture, and it is the one Cairn would be
   inventing.
4. **Scope to a ref set by default** (HEAD plus its upstream, or starred refs),
   with all-refs opt-in — VS Code's built-in graph and GitHub Desktop take this
   route, as do gitk, tig, lazygit and magit (Findings 17, 18, 20). Cheapest by
   far, and the best-precedented option in the record by raw client count. But it
   puts Cairn on the opposite side of the graph-versus-list line from Fork and
   Sourcetree on the most visible decision in the view, and the client that
   scoped hardest also stopped drawing a graph and has a 294-reaction request
   asking it to start.
5. **All refs, but make the collapse affordance first-class rather than a remedy**
   — Fork's Collapse All Merges reduced its own worst published example from 21
   lanes to 3 (Finding 6). Attacking rows and lanes together is the only remedy in
   this record with a measured effect size, and it is also the one that depends on
   the user knowing it exists — which Finding 3 says Fork's users largely do not.

Whatever is chosen, two things in this record are cheap and worth taking
regardless:

- **Scope by ref set, never by commit predicate.** Fork's developer's reason —
  "We can't filter the commit history because this will break the graph"
  (Finding 4) — is a topology argument that applies to Cairn identically, and both
  clients independently pushed search out into a separate results list rather than
  filtering the graph (Findings 4, 15).
- **Re-run the phase-01 benchmark against a fixture whose branches close.** The
  200-branch number is not wrong as a *stress* fixture; it is wrong as "the
  ordinary view". The evidence supports benchmarking both, and the gap between
  them is the whole question.
