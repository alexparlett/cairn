# Fork's refs, stashes and status, as published

Evidence record for the `refs-and-status` packet, recorded 2026-10-05. It answers
how Fork (git-fork.com / fork.dev; AppKit on macOS, WPF on Windows) draws refs on
the commit graph, scopes the graph, places stashes, represents the working tree,
lays out its sidebar, shows read-only working-tree status, refreshes that status,
lists refs in the commit detail pane, and shows the current branch in its toolbar.
Everything here is documentary: Fork has no Linux build, so nothing was installed
or clicked. Sources are the Mac and Windows release notes (every entry, parsed),
the vendor's blog and home-page screenshots, the vendor-hosted shortcut lists in
`fork-dev/Docs`, and issues in `fork-dev/Tracker` (Mac) and `fork-dev/TrackerWin`
(Windows) read through the GitHub API with their comments.

Companions, cited rather than repeated:
`docs/research/history-graph/what-clients-show.md` (Findings 1-4 on All Commits,
selection and filtering; 8 on the absent working-tree row; 9 on columns and label
overflow) and `docs/research/diff-engine/fork-detail-and-diff-ui.md` (Finding 3 on
the `REFS` row and the refusal of "contained in"; Finding 5 on the Changes file
list and its change-type badges; Finding 9 on what a selected stash shows).
`docs/design/ui.md` is the design these findings test.

Conventions. `Tracker #N` is `https://github.com/fork-dev/Tracker/issues/N`;
`TrackerWin #N` is `https://github.com/fork-dev/TrackerWin/issues/N`. "Mac RN" is
<https://git-fork.com/releasenotes>, "Win RN" is <https://git-fork.com/releasenoteswin>;
the two apps are versioned independently (Mac 2.21 is Aug 2022, Windows 2.21 is
Jul 2026). `DanPristupov` is the vendor. Quotes are at most 15 words. Labels:
RN (release note), VENDOR (vendor comment), VSHOT (vendor screenshot), USHOT (user
screenshot, dated), USER (user report), INFERRED, OPEN.

## Headlines

1. **Ref labels are rounded, outlined chips immediately left of the subject, after
   the graph**, filled with a tint of the lane colour on both platforms (Windows
   since 1.34, Jun 2019; Mac only since 2.53, May 2025 — before that Mac coloured
   by kind, local red and remote green). Tags are their own colour (indigo) with a
   tag-icon cap. A remote-tracking label carries a forge icon (GitHub, GitLab, ...)
   in a separate cap. The current branch's label carries a check mark and the HEAD
   row's subject is bold.
2. **"Compact labels" merge a local branch and its remote-tracking branch into one
   chip when both point at the same commit** — the remote collapses to a bare forge
   icon in front of the local name. On by default (Windows 1.55, Oct 2020; Mac 2.19,
   Jun 2022, with a preference to disable it). The vendor's framing: the icon means
   "it is up to date".
3. **No overflow handling.** Fork does not wrap, elide or count ("+N") labels; many
   refs on one commit push the subject out of the column, and a long label is
   clipped at the column edge. The vendor has declined this repeatedly.
4. **All Commits is the default, always** — the vendor says Fork "always opens the
   All Commits view", and the request to remember Local Changes is open since 2018.
   Scoping is an opt-in branch filter (toolbar button and ⌘⇧A / Ctrl+Shift+A for
   "active branch"), per-ref hiding, and View-menu toggles for remote branches,
   tags and stashes (`what-clients-show.md` Findings 1-3).
5. **Stashes are rows of their own in the graph**, each with a `stash@{n}` chip
   (box icon) and the stash message as subject, drawn as a one-commit side branch
   off the commit it was made on and ordered by date like any commit. Several
   stashes on one commit each get their own short lane. One row per stash: the
   index and untracked parents are not drawn. Hideable (View → Hide stashes in the
   commit list), remembered per repository and, on Windows 2.23, per worktree.
6. **There is no working-tree row in the graph**, by vendor decision; the request
   (Tracker #308) is open since 2018 and still collecting comments in 2025. The
   substitutes are the sidebar's `Local Changes (N)` item, a dirty marker on the
   repository tab, and "compare a commit to local changes".
7. **The sidebar, top to bottom (2026):** repository name, `Local Changes (N)`, `All
   Commits`, a small tab strip (refs, search; plus pull requests on Windows), a
   filter box, then collapsible sections **Worktrees** (since 2023, moved first in
   2026), **Pinned** (was Starred), **Branches**, **Remotes**, **Tags**,
   **Stashes**, **Submodules**. Branch names are grouped into folders by `/`. Each
   local branch shows its ahead/behind counts as arrows (`3↑1↓`, `853↓`), an icon
   for its upstream state, and the current branch a check mark in bold. Click
   selects and scrolls to the ref's commit; double click checks out.
8. **Fork has had a worktree UI since 2023** (Mac 2.31 / Windows 1.87: worktrees
   on the sidebar; create/delete since 2024; a worktree icon on branches checked out
   elsewhere since late 2025). `docs/design/ui.md`'s "Fork has no worktree UI" is
   out of date.
9. **Status is `git status --porcelain --untracked-files=all`, run on window
   activation, after Fork's own operations, and on ⌘R / F5** — the vendor states
   each. Fork does not watch the file system (vendor, 2019; it tried and gave up,
   2022) and recommends git's own fsmonitor for big repositories. The standing
   complaint is the opposite of staleness: re-running status on every focus locks
   the index for seconds on large repositories, with no off switch.
10. **The Local Changes view** is two lists, Unstaged over Staged, each a tree (by
    default), flat list or two-column combined list; a coloured badge per file
    (yellow `M`, green `+` for untracked, a yellow warning for a conflict);
    untracked files are shown by default and can be hidden; ignored files are
    hidden by default and can be shown. The sidebar count matches the number of
    entries `git status` lists, and is omitted when the tree is clean.
11. **Commit details list only the refs pointing at the commit** (`REFS`), never the
    branches containing it — settled in `fork-detail-and-diff-ui.md` Finding 3.
12. **The toolbar's central status box names the repository and the current branch
    (a checkout dropdown) and shows the branch's behind/ahead counts**; since Jan /
    Feb 2026 the Pull and Push buttons carry the counts too.

## 1. Ref decoration on the commit graph

**Position.** Labels sit in the subject column, after the graph and before the
subject text, one chip per ref, left to right (VSHOT: home-page carousel
<https://git-fork.com/images/carousel/carousel_mainMac.jpg> and
<https://git-fork.com/images/carousel/carousel_mainWin.jpg>, late 2020; blog Windows
1.38 <https://fork.dev/blog/posts/forkwin-1.38/colored-branch-labels.jpg>, 2019).
Labels arrived in the first week of the app: "Commit list renders the branch and tag
labels now" (Mac RN, GitClient 1.0.2, 19 Apr 2016).

**Shape and colour.**

- A rounded rectangle with a 1 px outline in the lane colour and a light tint of it
  as fill, dark text (VSHOT, Windows 1.38 blog image above). Windows: "Draw branch
  labels using their graph colors" (Win RN 1.34, 8 Jun 2019). Mac: "Draw branch
  labels with correspoding graph colors" (Mac RN 2.53, 21 May 2025).
- **Mac before 2.53 coloured by kind, not lane**: users protested the change —
  "Red and green for local and remotes" (Tracker #2358,
  <https://github.com/fork-dev/Tracker/issues/2358#issuecomment-2915165277>, May
  2025; one commenter in the thread recalls the opposite mapping). USHOT 2022
  (Tracker #1713,
  <https://user-images.githubusercontent.com/41138925/192750315-7f038390-cfb7-490f-9f37-55fac72ff204.png>)
  settles it for the pre-2.53 Mac dark theme: local labels red, remote-tracking
  labels green. The vendor kept the change: "Branch colors looked this way on the
  Windows version for many years"
  (<https://github.com/fork-dev/Tracker/issues/2358#issuecomment-2915495193>,
  28 May 2025).
- **Tags** are drawn in their own colour — an indigo fill with a tag-icon cap on the
  left, independent of lane (USHOT, Mac 2.53.1, Tracker #2358 body,
  <https://github.com/user-attachments/assets/7effab54-8e4e-41b3-9a0c-1f780e27eb07>,
  May 2025: `staging-2025-05-27.0`).
- **Icons**: "Add icons to branch labels" (Mac RN 2.0, 13 Nov 2020). A
  remote-tracking label for a known forge carries the forge's logo in a separate
  square cap left of the name (`origin/fix41651` in the Windows carousel; a custom
  icon for Azure DevOps remotes, Mac RN 1.0.89, 24 Jan 2020). A local branch with
  no remote counterpart at that commit has no cap (USHOT, Tracker #2474,
  <https://github.com/user-attachments/assets/d44d8aa9-d89c-4529-83e3-a66249b7c24e>,
  Oct 2025).
- **Stash** chips: box icon + `stash@{n}` (Section 3).
- **Bisect** labels exist (Mac RN 1.0.73, 1 Feb 2019; skip labels, Mac RN 2.67).
- **Search** highlights matches inside labels (Mac RN 2.50, 7 Feb 2025).

**The current branch and HEAD.**

- The checked-out branch's label starts with a check mark: `✓ master` (Windows
  carousel), `✓ main` (Mac carousel). Before 2.0 it was a dot inside the label
  (VSHOT, blog Mac 1.0.73 <https://fork.dev/blog/posts/fork-1.0.73/>, Feb 2019).
  "Highlight the current branch label in the revision list control" (Mac RN 1.0.5,
  May 2016); "Emphasize active branch in commit list" (Mac RN 2.30, 23 Jun 2023) —
  what 2.30 changed visually: OPEN.
- The HEAD commit's row is **bold**: "The HEAD revision is marked with bold font"
  (Mac RN GitClient 1.0.2); visible in both carousels.
- Commits **not reachable from the current branch are drawn grey**: "The commits
  which belong to the active branch are black" (VENDOR, Tracker #1171,
  <https://github.com/fork-dev/Tracker/issues/1171#issuecomment-720477620>, Nov 2020);
  "show unreachable revisions with gray color" (Mac RN 1.0.18, Aug 2016).
- **No `HEAD` label in the graph.** In detached HEAD state the sidebar shows a
  `HEAD` item under Branches (Mac RN 1.0.60, 1 Dec 2017); a graph label was
  offered only for that state — "It's probably excessive to show such a label all
  the time" (VENDOR, Tracker #107,
  <https://github.com/fork-dev/Tracker/issues/107#issuecomment-368455772>,
  Feb 2018). Whether it shipped: OPEN (no release note found; a 2020 user
  describes detached HEAD only by the bold row, Tracker #1171).

**Push/pull markers.** "Tiny coloured bubbles next to commits" mark commits to push
and to pull (blog Mac 1.0.73, <https://fork.dev/blog/posts/fork-1.0.73/>, Feb 2019;
Win RN 1.30, Mar 2019, "Highlight commits to-push and to-pull"). In the vendor
image a blue dot precedes the subject of an unpushed commit and a grey dot that of
a commit only on the upstream (VSHOT,
<https://fork.dev/blog/posts/fork-1.0.73/ahead-behind-info.jpg>).

**A local branch and its remote at the same commit — merged ("compact labels").**

- Shipped as "Compact branch labels in commit graph" (Win RN 1.55, 30 Oct 2020;
  Mac RN 2.19, 17 Jun 2022, "can be disabled in preferences"). Vendor on Windows: it
  collapses "the remote tracking branch" when it and the local branch share a
  commit (Tracker #1440,
  <https://github.com/fork-dev/Tracker/issues/1440#issuecomment-897048459>,
  Aug 2021), and on the meaning: we think "'it is up to date'"
  (<https://github.com/fork-dev/Tracker/issues/1440#issuecomment-1160185768>,
  Jun 2022). Vendor on launch: "we were a bit nervous about the new labels"
  (TrackerWin #1000,
  <https://github.com/fork-dev/TrackerWin/issues/1000#issuecomment-720370092>).
- Drawn as the forge-icon cap followed by the local label: `[GitHub] [✓ master]`
  (Windows carousel; USHOT Tracker #2358,
  <https://github.com/user-attachments/assets/9e5a5d3e-882b-4d68-9bee-9bb53be2ab9e>).
- Only the upstream's counterpart collapses; a second remote at the same commit
  keeps its own full chip (`master` + `mike/master` with `origin/master`
  collapsed), and a user asks for the remote name to be kept when there is more
  than one remote (Tracker #1365, open since Apr 2021, with compact-vs-full
  screenshots). Which remote collapses when several match — the upstream, or any
  same-named one: OPEN.

**Ordering when several refs share a commit.** In the screenshots the local branch
comes first, then remote-tracking branches (Tracker #1365's full-mode image:
`✓ npm-install`, `mike/npm-install`; `master`, `mike/master`, `origin/master`). Where
tags fall relative to branches, and the sort within a kind: OPEN.

**Many refs, long names.** No wrapping, no elision, no `+N`, no overflow tooltip.
"all horizontal space in the commit list is filled by branch names", answered by the
vendor "Why a commit has a lot of branches? What is the use case?" (Tracker #516,
<https://github.com/fork-dev/Tracker/issues/516#issuecomment-1413337054>, Feb 2023);
Tracker #1635 (Jul 2022, open) reports the same in the details' `REFS` row on Mac.
A request to toggle labels to a short form (Tracker #1440) was answered by compact
labels, not by truncation. A long label is clipped at the column's right edge with
the subject pushed out (USHOT, Tracker #2358, image above:
`3404-stop-Event-loop-when-db-is-of`). Remedies offered instead: hide remote
branches, hide tags, per-ref hide, collapse merges (`what-clients-show.md` Findings
3 and 5). A tooltip on a commit-list label: OPEN (tag messages show in sidebar
tooltips, Win RN 1.38).

**What refs Fork reads.** Fork reads every ref namespace, not only heads, remotes and
tags: a 2026 bug shows it dating `refs/jj/remote-tags/...` (Tracker #2675, USER,
Jul-Aug 2026). Which namespaces it *draws*: OPEN.

**Double click on a label checks that branch out** (Mac RN 2.16, Feb 2022; Win RN
1.71, Mar 2022) — out of scope here, noted.

**Evidence:** RN; VSHOT; USHOT; VENDOR. **Platform:** both, with the Mac colouring
difference before May 2025.

## 2. All Commits, current-branch scoping, hiding

Settled in `what-clients-show.md` Findings 1-4; additions only.

- **Default on open is All Commits, always**: "Fork always opens the All Commits
  view" (VENDOR, Tracker #295,
  <https://github.com/fork-dev/Tracker/issues/295#issuecomment-393047101>,
  May 2018). Requests to remember Local Changes instead are still arriving in 2026
  (same issue, Jan and Jul 2026 comments).
- **"Filter by active branch"** is a toolbar button and ⌘⇧A / Ctrl+Shift+A (Mac RN
  1.0.82, Jul 2019; Win RN 1.37, Jul 2019; `fork-dev/Docs` keyboard shortcuts,
  <https://github.com/fork-dev/Docs/blob/master/keyboard-shortcuts-mac.md>). The
  filter follows checkout (Mac RN 1.0.75; Win RN 1.34), persists across launches
  (Mac RN 1.0.59), and a "filter status panel above commit list" says it is on
  (Mac RN 1.0.73, Feb 2019). Filters apply to folders and remotes (Mac RN 2.6, Win
  RN 1.60, Mar 2021). On Windows 2.23 (25 Sep 2026) the filter, hidden tags and
  stashes and collapse state are stored per worktree.
- **Hiding**: remote branches (Mac RN 1.0.68; View menu "Hide Remote Branches in
  Commit List", VSHOT blog Mac 1.0.69,
  <https://fork.dev/blog/posts/fork-1.0.69/hide-remotes.png>), tags (Mac RN 1.0.70,
  Win RN 1.41), stashes (Section 3), and particular branches, folders or remotes
  (Mac RN 2.6, Win RN 1.60). "Hide Tags" is a performance feature: it "was created to
  improve performance and memory use in repos with hundreds of thousands tags", so
  it hides tags from the sidebar too (VENDOR, Tracker #1934,
  <https://github.com/fork-dev/Tracker/issues/1934#issuecomment-1638210076>,
  Jul 2023).
- **Keyboard**: ⌘2 / Ctrl+2 shows All Commits ("second press will jump to HEAD"),
  ⌘0 / Ctrl+0 reveals HEAD and the current branch on the sidebar (`fork-dev/Docs`;
  Mac RN 1.0.75, 1.0.89).

**Evidence:** VENDOR; RN; VSHOT. **Platform:** both.

## 3. Stashes inline in the commit list

**How a stash row looks.** A row of its own: a chip with a box (archive) icon and
`stash@{n}`, then the stash message as the subject — git's own `On main: s6-2` or
`WIP on main: <sha> <subject>` (USHOT, Windows, TrackerWin #2645,
<https://github.com/user-attachments/assets/264f1a29-5c4f-4d73-ad61-4ef63c0c3425>,
Nov 2025; USHOT, Mac 2.22.1, Tracker #1713, Sep 2022, `stash@{0}  CocoaDebug`). A
2017 Mac screenshot shows the same shape without the icon (Tracker #107,
<https://user-images.githubusercontent.com/2022976/33492675-0d5ac626-d6be-11e7-8a8b-7ea718bc0efc.jpg>).
Home-page feature list: "See your stashes right in the commit list"
(<https://git-fork.com/>).

**Where it is placed.** As a commit in the graph: its node sits on a short lane of
its own that joins the commit the stash was made on, and rows are ordered by date
with everything else — in TrackerWin #2645 three stashes newer than `main`'s tip sit
above it, each on its own lane; in Tracker #107 (2017) `stash@{2}` sits lower in the
list among older commits. INFERRED from screenshots: placement is the ordinary
graph order of the stash commit, not a fixed slot at the top. Several stashes on the
same commit each get a separate lane (TrackerWin #2645).

**Internal commits hidden.** Each stash is one row; its index commit (second parent)
and untracked-files commit (third parent) are not drawn as rows (both screenshots;
INFERRED). They remain reachable as parent links in the Commit tab (three `PARENTS`
for a stash with untracked files, `fork-detail-and-diff-ui.md` Finding 3). An early
Mac release note reads "Do not show stash revisions in commit list" (Mac RN 1.0.12,
Jul 2016), before stashes became a feature (Mac RN 1.0.16, Aug 2016).

**Selecting it** shows the ordinary commit details and the first-parent diff,
without untracked files (`fork-detail-and-diff-ui.md` Finding 9). A stash can be
compared to local changes (Mac RN 2.12, Sep 2021).

**Hiding and the sidebar.**

- "Add option to hide stashes in commit list" (Mac RN 1.0.95, Jun 2020; Win RN 1.51,
  Jul 2020); the Windows path is "View -> Hide stashes in the commit list" (VENDOR,
  TrackerWin #2677,
  <https://github.com/fork-dev/TrackerWin/issues/2677#issuecomment-3687434907>,
  Dec 2025); remembered per repository (Mac RN 2.1) and per worktree (Win RN 2.23).
- The sidebar has a **Stashes** section listing every stash by message (TrackerWin
  #2645 shows `s6-2`, `ai`, `s6`, `WIP on main: fc47d18 Add CI pipeline` — the
  `On <branch>:` prefix of a named stash dropped, INFERRED). In 2022 Fork moved it
  to a separate sidebar tab (Mac RN 2.22, Win RN 1.77) and rolled it back a month
  later (Mac RN 2.23, Win RN 1.79: "Roll the stashes layout back") after users
  objected (Tracker #1696). The vendor's reasoning then: "The recent stashes are
  already displayed in the commit list" plus the toolbar dropdown
  (<https://github.com/fork-dev/Tracker/issues/1696#issuecomment-1251994365>), which
  shows recent stashes (Win RN 1.19; limited to 15 per a user in the same thread).
- Whether "recent" means a cap on how many stashes the commit list draws: OPEN.

**Evidence:** USHOT; RN; VENDOR; INFERRED. **Platform:** both.

## 4. No working-tree row in the graph

Confirmed and extended from `what-clients-show.md` Finding 8.

- Still no such row in October 2026: Tracker #308 (Jun 2018) and TrackerWin #103
  (Jan 2019) are open; Tracker #308 drew new comments in 2024 and Aug 2025. The
  vendor's 2018 objections stand: "It seems strange to show a fake revision which
  semi-duplicates the 'Changes' view" (Tracker #107,
  <https://github.com/fork-dev/Tracker/issues/107#issuecomment-368455772>).
- A long-time user reports adapting: ⌘1 / ⌘2 toggle the two views (Tracker #308,
  2018-2020 comments; `fork-dev/Docs`: "⌘1 - Show Changes view").
- **Substitutes Fork shipped**: the sidebar count (Section 6); a dirty marker on each
  repository tab — "Show uncommitted changes indicator in tabs" (Mac RN 1.0.74, Feb
  2019; Win RN 1.29), drawn as `name*` (carousels) and later as tab badges / dots
  (Mac RN 2.45, Aug 2024); and "Ability to compare local changes to a commit" (Mac
  RN 2.11, Aug 2021; Win RN 1.65), the answer to Tracker #308's diff-against-a-commit
  use case.
- The tab marker is refreshed on application focus and does not count untracked
  files, "because getting untracked changes is a very expensive operation"
  (VENDOR, TrackerWin #2514,
  <https://github.com/fork-dev/TrackerWin/issues/2514#issuecomment-2953899662>,
  Jun 2025). It can be turned off: Preferences → "Tab indicator for uncommitted
  changes" (VENDOR, TrackerWin #553,
  <https://github.com/fork-dev/TrackerWin/issues/553#issuecomment-553769552>, 2019).

**Evidence:** VENDOR; RN; USER. **Platform:** both.

## 5. The sidebar

**Sections and order (2026).** Repository name (with a menu), `Local Changes (N)`,
`All Commits`; a tab strip (branch icon = refs, magnifier = search; Windows adds a
GitHub/pull-requests tab); a `Filter` box; then **Worktrees**, **Pinned**,
**Branches**, **Remotes**, **Tags**, **Stashes**, **Submodules** (USHOTs: Tracker
#2598, Mac 2026,
<https://github.com/user-attachments/assets/be31234c-c784-46e2-8063-233e6065a1cc>;
Tracker #2474, Mac 2025; TrackerWin #2645, Windows 2025). History:

- Branches / Remotes / Tags / Stashes / Submodules since 2016-2017 (Mac RN 1.0.3,
  1.0.16, 1.0.19). On Windows a 2020 tabbed sidebar layout drew protests (TrackerWin
  #739, Apr 2020); the vendor's reasoning — repositories have "Tens of local branches,
  hundreds of remote branches and thousands of tags" in one scroll view
  (<https://github.com/fork-dev/TrackerWin/issues/739#issuecomment-615837810>).
- Starred → **Pinned** (Mac RN 1.0.56 "pin branches and tags", 2017; 2.7 "Replace
  star icon with pin", 2021).
- **Worktrees**: "Show existing git worktrees on sidebar" (Mac RN 2.31, Jul 2023; Win
  RN 1.87, Aug 2023), nested (Mac RN 2.32), create/delete (Mac RN 2.41, Mar 2024;
  Win RN 1.96), "Show Worktrees before Pinned items in sidebar" (Mac RN 2.63, Feb
  2026; Win RN 2.17). Users find them now too prominent (TrackerWin #2735, Mar 2026).
- Detached HEAD shows a `HEAD` item in Branches (Mac RN 1.0.60).
- "Show if submodules have uncommitted changes" / "Show submodule status on sidebar"
  (Mac RN 1.0.88; Win RN 1.43), with folder nesting.
- The label is `Local Changes` on Mac and was `Changes` on Windows in 2019-2020
  (VSHOT carousels and blog Windows 1.28,
  <https://fork.dev/blog/posts/forkwin-1.28/new-layout.jpg>); Windows reads `Local
  Changes` by 2025 (TrackerWin #2645 USHOT).

**Tree grouping by `/`.** "Implemented grouping branches by folders" (Mac RN 1.0.20,
Sep 2016); remote branches are folded the same way under each remote, with a forge
icon on the remote (carousels: `release/5.3` under `origin`). Folders can be
filtered or hidden whole (Mac RN 2.6), branches moved into existing folders (Mac RN
2.2). Sorting: natural (Mac RN 1.0.15), alphabetical or recently used (Mac RN
1.0.74), "alphabetically, folders first" (Mac RN 1.0.85), `main` treated as `master`
(Mac RN 1.0.97).

**Ahead / behind.** Shown as digits with arrows right-aligned on the branch row:
"Use arrow symbols to show behind/ahead branch labels" (Mac RN 1.0.57, Oct 2017);
`853↓`, `1↓`, `2↓` (Mac carousel), `3↑1↓` (VSHOT blog Windows 1.48,
<https://fork.dev/blog/posts/forkwin-1.48/branch-status.jpg>), `18↓1↑` and
`79↓79↑` (USHOT Mac 2023, Tracker #1938). MEASURED from those images: Windows puts
ahead first, Mac behind first. The counts are against the branch's upstream (INFERRED
from the toolbar box's per-branch counts; the release notes say only "how many
commits behind or ahead", Mac RN 1.0.31). Shown for local branches only; for
branches checked out in worktrees since Mac RN 2.63 (a fix).

**Upstream state icon.** "Show different branch icons according to upstream status"
(Win RN 1.44, Jan 2020; Mac RN 1.0.90 "Show branch upstream status on sidebar") —
one icon each for local-only, pushed, and pushed with the remote branch "has been
removed" (blog Windows 1.48,
<https://fork.dev/blog/posts/forkwin-1.48/>). In the vendor image the local-only icon
is greyed and the gone-upstream icon carries a yellow warning triangle. "Warning
icon for active branch with invalid upstream" (Mac RN 2.69, Jul 2026; Win RN 2.21).
A worktree icon marks branches checked out in another worktree (Mac RN 2.59, Nov
2025; Win RN 2.17).

**Current branch.** A check mark replaces the branch icon and the name is bold, in
Branches and in Pinned (carousels). It is revealed on checkout (Mac RN 2.15) and by
⌘0 (Mac RN 1.0.89).

**Filter box.** "Implemented quick filter for sidebar" (Mac RN 1.0.50, Jun 2017;
Win RN 1.35 "Add filter to sidebar and changed files", VSHOT blog Windows 1.38);
"Move filter on top in sidebar" (Mac RN 2.0). It filters every section at once,
which users find unhelpful (Tracker #2585, Mar 2026); a 1,000-remote-branch user
asked for it in the first place (TrackerWin #44, `what-clients-show.md` Finding 10).

**Click, ⌘-click, double click.** Click selects the ref and scrolls the commit list to
its commit, without filtering (`what-clients-show.md` Finding 2; Win RN 1.33 fixed
"Click on tag on sidebar doesn't scroll to that tag"; Mac RN 2.56 fixed details
not updating on re-click). Selecting two refs compares them (Mac RN 1.0.64 "Compare
branches with ⌘+click on the sidebar"; Win RN 1.27; Mac RN 1.0.71 across ref
kinds). Double click: checkout of a local branch (Mac RN 1.0.12) or tag (Mac RN
1.0.65), track for a remote branch (Mac RN 1.0.41; Win RN 1.18), open for a
submodule (Win RN 1.18) — out of scope for this packet. Arrow-key navigation and Tab
between sidebar and list: Mac RN 2.66, Win RN 2.20 (2026).

**Evidence:** RN; VSHOT; USHOT; VENDOR; MEASURED. **Platform:** both.

## 6. The Local Changes view (read-only aspects)

**Layout.** Unstaged list above Staged list, each headed with its name and a
Stage / Unstage button; diff on the right; commit box below the diff (VSHOT
carousels <https://git-fork.com/images/carousel/carousel_commitviewMac1.jpg>,
<https://git-fork.com/images/carousel/carousel_commitviewWin1.jpg>). Above the lists:
a filter field (Mac RN 2.9), a collapse-all chevron, an eye (side-by-side quick
look) and a layout menu.

**List vs tree.** The layout menu offers `View as Tree`, `View as List`, `View as
Combined List`, then `Hide Untracked Files` and `Show Ignored Files` (USHOT, Mac,
Tracker #2616,
<https://github.com/user-attachments/assets/3c20f83b-108f-4696-a14b-92067bfd2fd9>,
Apr 2026; the same menu in VSHOT blog Mac 1.0.69,
<https://fork.dev/blog/posts/fork-1.0.69/show-untracked.png>). Tree in both
carousels; the choice is global, not per repository (VENDOR, Tracker #2616, Apr
2026). Combined list is `Name` | `Location` (`fork-detail-and-diff-ui.md` Finding 5).

**Status badges** (detail beyond `fork-detail-and-diff-ui.md` Finding 5):

- Modified: a yellow rounded square — `···` until late 2022, then a letter `M` (Mac
  RN 2.24 / Win RN 1.79, "Update change type icons"; USHOTs Tracker #1938, 2023,
  and #2598, 2026).
- Untracked (and added): "a green box with a white + sign" (USER, Tracker #1593, Apr
  2022, asking for something less prominent); still a green `+` in 2026 (Tracker
  #2616 USHOT). Untracked and added share it — whether the 2022 icon set
  distinguishes them: OPEN.
- Conflicted: "marked with a yellow warning icon", which stays after an external
  tool resolves the file because Fork cannot tell without opening it (USER and
  VENDOR, TrackerWin #956,
  <https://github.com/fork-dev/TrackerWin/issues/956#issuecomment-696820633>,
  Sep 2020). Conflicted files sit in Unstaged (TrackerWin #2600, 2025). A conflict
  count once showed in the status bar (Mac RN 1.0.65 fix).
- Deleted, renamed, copied: icons for "modified/added/deleted/renamed" since Mac RN
  1.0.4 (May 2016); renames show old and new names (Mac RN 2.26, Win RN 1.81, 2023).
  Their post-2022 glyphs and colours: OPEN.
- Renames appear only among **staged** changes: "Renames can only be detected in
  staged changes" (VENDOR, Tracker #2598,
  <https://github.com/fork-dev/Tracker/issues/2598#issuecomment-4237219856>, Apr
  2026); an unstaged `R` row from git (an intent-to-add rename) is a logged
  "Unhandled unstaged case" and the file is dropped from view (USER log, same
  issue) — fixed in a following update per the vendor.
- Type change (symlink → file): drawn as `M` (USHOT, Tracker #1938); the vendor
  explains git reports it as two diffs and Fork shows only the first
  (<https://github.com/fork-dev/Tracker/issues/1938#issuecomment-1642404483>, 2023).
- Mode change: shown in the diff (Mac RN 2.8, Win RN 1.62), not as its own badge.
- Submodule: its own file icon (Mac RN 2.69, Win RN 2.21, 2026); submodule entries
  get a dedicated diff view (`fork-detail-and-diff-ui.md` Part D).
- The vendor reads git's porcelain codes as the vocabulary: he answered a request
  for a "moved only" icon by quoting `git status` short-format letters — "Git
  doesn't provide that info" (TrackerWin #1580,
  <https://github.com/fork-dev/TrackerWin/issues/1580#issuecomment-1176417473>,
  2022). There are no hover hints for the icons, by choice (VENDOR, TrackerWin #2849,
  Sep 2026).
- Ordering: alphabetical within the tree; tracked-before-untracked was declined
  (VENDOR, TrackerWin #2207,
  <https://github.com/fork-dev/TrackerWin/issues/2207#issuecomment-2074373386>,
  2024).

**Untracked and ignored.** Untracked are shown by default; ignored are hidden by
default: "In the past, you could not see the files listed in your .gitignore"
— now an option (blog Mac 1.0.69, <https://fork.dev/blog/posts/fork-1.0.69/>; Mac RN
1.0.68, Jul 2018; Win blog 1.23 "Hide untracked files and show ignored files").
"Hide Untracked Files" is remembered (Mac RN 2.46, Win RN 2.2, 2024) and announced
in the Unstaged header when on (same notes). ⌘⇧. / Ctrl+Shift+. toggles ignored
files (Mac RN 2.61, Win RN 2.16, 2026).

**What the count counts.** The sidebar number equals the entries `git status`
lists: the count is "same as the number of files changed according to git status"
(USER, Tracker #2598, Apr 2026). The vendor
carousels agree with that being unstaged plus staged entries: `Local Changes (41)`
over six unstaged files and a `Commit 35 Files` button; `Changes (11)` over two
unstaged and `Commit 9 Files` (MEASURED/INFERRED; a file with both staged and
unstaged hunks may count twice: OPEN). It includes untracked files (they are in
the list it matches) unless hidden (INFERRED). A clean tree shows `Local Changes`
with no number (USHOTs Tracker #2474, TrackerWin #2645). The count was unreliable in
2017-2018 (Tracker #147).

**Evidence:** VSHOT; USHOT; USER; VENDOR; RN. **Platform:** both.

## 7. Refresh

**The status read.** "Fork runs `git status --porcelain --untracked-files=all` to get
the list of the changed files" (VENDOR, Tracker #2341,
<https://github.com/fork-dev/Tracker/issues/2341#issuecomment-2830169629>, Apr 2025;
the same command is his first diagnostic in Tracker #1692, #2598 and TrackerWin
#2777). With untracked files hidden he points at `--untracked-files=no` as the
cheaper read (Tracker #1919,
<https://github.com/fork-dev/Tracker/issues/1919#issuecomment-2119320391>, May 2024).
A 2021 Mac log names the steps `GetRepositoryStatusGitCommand` and
`RefreshRepositoryDataGitCommand`, and shows a refresh being cancelled by the next
(Tracker #1393). Repository → Benchmark times "Reading changed files", "Reading
branches and tags" and "Reading commits" (USER output, Tracker #2341).

**When it runs.**

- **On window activation.** "The repository status is updated on focus" (VENDOR,
  TrackerWin #348,
  <https://github.com/fork-dev/TrackerWin/issues/348#issuecomment-508688116>, 2019);
  precisely on WPF's `Window.Activated` (TrackerWin #1418,
  <https://github.com/fork-dev/TrackerWin/issues/1418#issuecomment-999533838>,
  2021). On Mac since the first build: "When application awakes from being
  unfocused ... the current view ... will be refreshed" (Mac RN GitClient 1.0.1).
- **After Fork's own operations**: `git status` "will be called after each operation
  affecting the working directory (commit, checkout, merge, etc)" (VENDOR, Tracker
  #1919, link above); refs are re-read after a fetch (TrackerWin #553, 2019).
- **Manually**: ⌘R (Mac) and F5 (Windows) — "Refresh" (`fork-dev/Docs` shortcut
  lists; View → Refresh ⌘R in VSHOT blog Mac 1.0.69). The vendor calls ⌘R a "full
  refresh" that fixes a stale rename status (Tracker #2125, 2024).
- **No file-system watching**: "No, Fork doesn't watch FS. It only updates when the
  window gets focus." (VENDOR, TrackerWin #553,
  <https://github.com/fork-dev/TrackerWin/issues/553#issuecomment-553910791>, Nov
  2019). On Mac the vendor "spent a lot of time" on his own file-system watcher
  and hit "a lot of problems", submodules among them (VENDOR, Tracker #1591,
  <https://github.com/fork-dev/Tracker/issues/1591#issuecomment-1104989752>, Apr
  2022). A 2025 request for FSEvents (Tracker #2300) was closed after a user noted
  in Jul 2026 that Mac Fork "now automatically picks up local changes" in the
  background — no release note says so; mechanism and version: OPEN.
- **Background fetch** is separate, on a timer (10 minutes, Mac RN 2.36), and can be
  disabled; status cannot (TrackerWin #348).

**Complaints.** Two kinds, both on big repositories:

- *Status-on-focus is too eager.* Each focus re-runs status and holds the index lock
  for seconds, so the user's own commit fails with `index.lock` exists (Tracker
  #1633, Jun 2022, open — "an improvement in Fork 2.30", vendor 2023; TrackerWin
  #2114, #348); "every time I refocus it's ~8-10s" on Chromium (Tracker #1919,
  2024); a ~1 minute spinner on every focus (Tracker #2341, 2025); "for big repos
  this is a nightmare" (TrackerWin #1418, Jul 2026). The asks: an option to disable
  refresh on focus and rely on F5 (TrackerWin #348 since 2019, Tracker #1919 since
  2023, both open). The vendor doubts it "will make a big difference" since
  operations re-run status anyway (Tracker #1919).
- *Stale status.* Users who edit elsewhere expect updates without refocusing
  (TrackerWin #583, 2019; Tracker #2300, 2025); occasional missed refreshes on focus
  (Win RN 1.44 and 1.55 fixes).

**Large repositories.** The vendor tests on Chromium ("my main testing repository",
Tracker #1919) and attributes slowness to `git status` itself: 3.5-4 s with
`--untracked-files=all` vs 2.5 s with `=no` there (same comment); "repos > 1GB start
being slow" (Tracker #1692,
<https://github.com/fork-dev/Tracker/issues/1692#issuecomment-1249040375>). His
remedies are git's: `git config core.fsmonitor true` (Chromium status 5000 ms → 1000
ms, Tracker #1591, Jun 2022; bundled git 2.36.1 for it), `git gc`, and hiding
untracked files. Fork itself has no fsmonitor or untracked-cache setting of its own
(no release note found), and many `git` processes left running are an open Windows
report (TrackerWin #1177).

**Evidence:** VENDOR; RN; USER. **Platform:** both.

## 8. Refs in the commit detail pane

Settled in `fork-detail-and-diff-ui.md` Finding 3: a `REFS` row of chips under the
author lists refs **pointing at** the commit ("Add 'References' section to the
commit details view", Mac RN 1.0.88, Dec 2019; "Show branches and tags in commit
details", Win RN 1.43, Jan 2020); "contained in branches" is refused as "virtually
impossible to present" (TrackerWin #1251), with `git branch -a --contains` offered
as a custom command. Additions: on Mac the row does not wrap, so many refs are
unreadable (Tracker #1635, open since Jul 2022); Windows wraps it (Win RN 1.58, Jan
2021). The `REFS` row is absent when nothing points at the commit (Windows carousel:
no `REFS` row on an unlabelled commit). A tag's annotation opens in a separate tag
details dialog (Mac RN 2.0 "Option to see annotated tag details"; Win RN 1.87).

**Evidence:** RN; VSHOT; VENDOR. **Platform:** both.

## 9. The toolbar's current branch and ahead/behind

- A central **status box**: repository name (with `*` when dirty), a branch icon and
  the current branch as a dropdown, and the branch's counts at its right — `swift*` /
  `main` / `853↓` (Mac carousel); `laravel` / `master` / `7↓` (VSHOT blog Windows
  1.28); `iris-2` / `develop` / `18↓1↑` (USHOT Mac 2023, Tracker #1938). "Added status
  box on the toolbar" (Mac RN 1.0.11, Jul 2016); "Implemented branch drop down
  selector on toolbar" (Mac RN 1.0.51); "shows the info about active processes and
  behind/ahead commit count for the active branch" (blog Mac 1.0.79,
  <https://fork.dev/blog/posts/fork-1.0.79/>, 2019). Clicking the box opens the
  activity manager — git commands and their output (VENDOR, TrackerWin #2849,
  Sep 2026).
- **Counts on Pull and Push**: "Show ahead/behind counts on pull/push toolbar
  buttons" (Mac RN 2.62, 23 Jan 2026; Win RN 2.16, 20 Feb 2026 "badges").
- In detached HEAD the box says so (Mac RN 1.0.23 and 1.0.97 fixes mention the
  'Detached HEAD' message); exact text: OPEN.

**Evidence:** VSHOT; USHOT; RN; VENDOR. **Platform:** both.

## Corrections to Cairn's records

- `docs/design/ui.md`, "What changes" table: "Fork has no worktree UI" — false since
  Jul / Aug 2023 (Section 5). Fork lists worktrees first in the sidebar and marks a
  branch checked out elsewhere with an icon; whether it also disables checkout of
  such a branch: OPEN (Windows opens that worktree's tab instead, Win RN 2.2,
  Oct 2024).
- `docs/design/ui.md`, "Kept": stashes appear "at the commit they sit on" — more
  precisely, as a row of their own on a short lane joined to that commit, at the
  stash's own place in date order (Section 3).
- `docs/research/history-graph/what-clients-show.md` Finding 9 says labels are
  coloured by lane citing Windows 1.34; on Mac that holds only from 2.53 (May 2025)
  — before, Mac coloured by kind (Section 1). Its sidebar item is `Changes (n)` on
  2020 Windows and `Local Changes (n)` on Mac and on Windows by 2025.

## OPEN

1. What "Emphasize active branch in commit list" (Mac 2.30) changed visually.
2. Whether a `HEAD` label is ever drawn in the graph in detached HEAD state.
3. Order of tags relative to branches on one row, and the sort within each kind.
4. Which remote-tracking branch collapses under compact labels when several remotes
   point at the same commit (the upstream only, or any same-named one).
5. A tooltip on a commit-list label; any elision for a single over-long label beyond
   column clipping.
6. Which ref namespaces Fork draws (it reads all, Tracker #2675).
7. Whether "recent stashes" means the commit list caps how many stashes it draws;
   what "Hide old tags and stashes on the sidebar by default" (Win RN 1.47) hides
   and how it is revealed.
8. The post-2022 glyphs and colours for added, deleted, renamed, copied and
   type-changed files, and whether added and untracked differ.
9. Whether a file with both staged and unstaged changes counts once or twice in
   `Local Changes (N)`.
10. How, and since which version, Mac Fork picks up working-tree changes while in the
    background (Tracker #2300, Jul 2026 user report); whether Windows does.
11. Whether Fork disables checkout of a branch checked out in another worktree.
12. The toolbar status box's exact text in detached HEAD.
13. Colour values for label fills, tag indigo and the ahead/behind digits, which
    the screenshots cited would allow measuring (not done here).
