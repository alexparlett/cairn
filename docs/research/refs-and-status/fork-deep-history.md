# Fork and a history too long to hold

Research for the `refs-and-status` packet, recorded 2026-10-05, following
`deep-find-measured.md` (Cairn's own numbers: on rust-lang/rust, paging the held
walk to the oldest commit takes about 2.4 s and retains about 1.4 GiB of rows, 89%
of it per-row lane edge segments). It decides nothing. It answers how Fork, on Mac
and Windows, handles a very long history — the commit-list cap, whether the list is
loaded up front or paged, what happens when the user asks for a commit the list has
not loaded or will never load, and what Fork does to bound a wide graph — with
secondary precedent from Sourcetree, GitKraken (Desktop and GitLens), Tower,
SmartGit, Sublime Merge, GitUp, gitk and tig. Evidence record, saved in full;
historical, never retro-edited.

Fork has no Linux build, so nothing was installed or clicked. Sources: the Mac and
Windows release notes (downloaded whole and searched), `fork-dev/Tracker` and
`fork-dev/TrackerWin` issues and comments (GitHub search API over "maxCommitCount",
"commit limit", "50000", "100000", "max-count", "--date-order", "pagination",
"reveal", "jump to commit", "parent sha", "memory", "large repository" and
similar; every hit with a vendor reply read in full), Fork log excerpts users
pasted, `fork-dev/Docs` (FAQ and both shortcut pages), the Fork blog, and vendor
docs, release notes, trackers or source for the other clients.

Conventions are the companions' (`fork-refs-and-status-ui.md`,
`fork-unreachable-stash-base.md`): `Tracker #N` is
`https://github.com/fork-dev/Tracker/issues/N`, `TrackerWin #N` is
`https://github.com/fork-dev/TrackerWin/issues/N`; "Mac RN" is
<https://git-fork.com/releasenotes>, "Win RN" is
<https://git-fork.com/releasenoteswin>; `DanPristupov` is the vendor. Quotes are at
most 15 words. Labels: RN, DOC (vendor documentation), VENDOR (a vendor comment),
LOG (a Fork log a user pasted), USER, SOURCE (read the code), INFERRED, OPEN.

## Headlines

1. **Fork caps the commit list at the newest N commits, and has since 2017.** The
   cap was 25,000 when introduced (VENDOR, Tracker #138, Oct 2017), 50,000 by 2020
   (VENDOR, Tracker #804, Sep 2020), and 100,000 on Mac from 2.41 (22 Mar 2024):
   "In Fork 2.41 we increased the commit limit from 50,000 to 100,000"
   (VENDOR, Tracker #2076). Windows was still 50,000 in March 2025 — "Fork reads
   50000 commits by default" (VENDOR, TrackerWin #2463). Neither release-note list
   mentions the cap or any change to it; the 2.41 Mac notes were checked.
2. **The cap is configurable only outside the UI.** Mac:
   `defaults write com.DanPristupov.Fork maxCommitCount 150000`, capitalisation
   significant since a macOS change (VENDOR, Tracker #2076). Windows: the
   `MaxCommitCount` field of `%localappdata%\fork\settings.json`, added in 2020
   (VENDOR, TrackerWin #663), edited with Fork closed. The vendor gives this
   answer to every report; repeated requests for a Preferences control
   (TrackerWin #1379, #1794; Tracker #1922) have no reply.
3. **N counts the newest commits across every ref, by date — so the cap also
   bounds a branch filter.** Windows logs show the walk is one
   `git log HEAD --branches --remotes --tags --max-count=50000 --date-order`
   (LOG, TrackerWin #1543 2022, #2380 Nov 2024). A branch filter selects from
   that capped set rather than walking the branch: "My guess is the commit window
   has a limit" — vendor: "You are absolutely right!" (Tracker #1401). An old
   branch filtered alone shows an empty list (Tracker #1194, #1626; TrackerWin
   #1379, #1419, #1854). A busy branch can push a quiet one out of the window
   entirely (Tracker #2076).
4. **The list is loaded whole (up to N) on each refresh, not paged.** One
   `git log --max-count=N` per refresh; no load-more, no paging. Pagination was
   promised and never shipped: "I'm going to implement a pagination one day"
   (2017), "we need to start loading commits dynamically" (2020), "we are working
   on that" (2023) — then the cap was raised instead, because "Internally Fork
   becomes faster and now we can afford it" (VENDOR, Tracker #2076, Feb 2024).
5. **What Fork keeps per row is small, and its lane layout is computed for the
   visible rows only.** Since at least Windows 2.2.3 (Nov 2024) the walk reads
   only `%H%n%P` — ids and parents (LOG, TrackerWin #2380); messages are read
   separately and can lag the scroll (TrackerWin #2847, 2026). And in 2018 the
   vendor said graph layout "used to be a bottleneck", so "I made it lazy, so it
   calculates only the visible area" (VENDOR, Tracker #345). No retained per-row
   edge list, as far as published. This is the part of Fork's design that bears
   most directly on Cairn's 89%.
6. **A ref, a search hit, a blame or file-history jump, or a parent link to a
   commit beyond the cap does nothing to the list.** "when you try to access a
   commit older than the history limit, nothing happens" (USER,
   Tracker #411); "If your branch is out of this range, it will not be visible"
   (VENDOR, TrackerWin #663). The details pane may still show the commit
   (TrackerWin #2131, #2463: USER, INFERRED). There is no message, no "load more"
   and no automatic paging. In Aug 2026 the vendor said "Fork must beep" when a
   branch is not visible in the topology (TrackerWin #2828); no release note
   through Mac 2.70 / Win 2.23 records it: OPEN.
7. **The end of the list is just the end.** There is no row, count or notice: the
   2018 request for "+ 100,000 more commits not shown" or a "Load more commits"
   button is still open (Tracker #411), and a 2023 report describes lanes cut off
   at the bottom with no way to scroll on (TrackerWin #1794).
8. **Fork bounds a wide graph by letting the user collapse it, not by capping
   lanes.** Collapse merges (Mac 1.0.94, Win 1.50, 2020), Collapse All Merges,
   first-parent view (Mac 1.0.89), hiding branches, filters; no lane cap, no
   horizontal scroll ("will not appear in the near future", VENDOR, Tracker #516).
9. **Most useful precedent: GitLens's Commit Graph (GitKraken) pages a not-yet-loaded
   target in on a jump and says so when it cannot**; Sourcetree has paged forward on
   a sidebar click since 2017; GitKraken Desktop, gitk and tig search only what is
   loaded; Sublime Merge and GitUp load everything. Section 5.

## 1. The cap: numbers, platforms, configuration

**Timeline.**

| When | Mac | Windows | Evidence |
| --- | --- | --- | --- |
| By Oct 2017 | 25,000, introduced after a Chromium user's report | — | VENDOR, Tracker #138: "I introduced the limit when a guy from Chromium project mentioned" Fork "used few gigabytes of RAM" |
| May 2019 | — | `--max-count=25000` | LOG, TrackerWin #284, #86 (`log --branches --remotes --tags HEAD --max-count=25000 --date-order`) |
| Feb 2020 (Win 1.45) | — | 25,000 | VENDOR, TrackerWin #663: "Currently Fork loads `25000` commits (which I expected to be enough for everyone)" |
| Aug 2020 | — | `MaxCommitCount` setting added | VENDOR, TrackerWin #663: "Now you can increase the max commit count in" settings.json |
| Sep 2020 | 50,000, and `maxCommitCount` | — | VENDOR, Tracker #804: "Some time ago the commit limit was increased to 50,000." |
| Oct 2020 | 50,000 | — | VENDOR, Tracker #1144: "Fork UI only shows the latest 50000 commits." |
| 2021-2024 | — | `--max-count=50000` | LOG, Tracker #1506 (a Windows-format log posted to the Mac tracker, 2021; INFERRED Windows), TrackerWin #1543 and #1647 (2022), #2380 (Fork 2.2.3, Nov 2024) |
| Nov 2021 | — | 50,000 | VENDOR, TrackerWin #1379: "the limit which is 50000 commits by default" |
| Feb 2024 (2.40) | 50,000, raise announced | — | VENDOR, Tracker #2076: "we are going to increase the default limit soon" |
| 22 Mar 2024 (2.41) | **100,000** | — | VENDOR, <https://github.com/fork-dev/Tracker/issues/2076#issuecomment-2015468825> |
| Mar 2025 (Win 2.6.1) | — | **50,000** | VENDOR, <https://github.com/fork-dev/TrackerWin/issues/2463#issuecomment-2753608732> |

- **Release notes are silent.** Neither the Mac nor the Windows release notes
  mention a commit limit, `maxCommitCount`, 25,000, 50,000 or 100,000 (RN, both
  lists searched in full). Mac 2.41's notes list worktrees, a git update and
  binary-diff items only. Whether Windows has raised its default since March 2025:
  OPEN — nothing published says so, and no Windows log after Nov 2024 shows the
  command.
- **Configuration.** Mac: `defaults write com.DanPristupov.Fork maxCommitCount
  <n>`; the vendor corrected the key's case in 2024 ("MacOS changed behavior
  recently. Previously it was case-insensitive", Tracker #2076). Windows:
  "Edit `MaxCommitCount` field in the `%localappdata%\fork\settings.json` file"
  (VENDOR, Tracker #1922; TrackerWin #1379, #1794, #1854, #2425, #2463); a user
  notes Fork overwrites the file unless closed first (USER, TrackerWin #663).
  `fork-dev/Docs`' FAQ entry is still headed "Fork shows only 50000 commits" and
  gives the Mac key in lower case
  (<https://github.com/fork-dev/Docs/blob/master/faq.md>). No Preferences control
  exists in either version (requests unanswered: TrackerWin #1379, #1794;
  Tracker #1922 "Looking forward to adding this option to the settings panel").
- **The cost the vendor attributes to it.** Asked whether raising it hurts:
  "Yes, of course ;), that's why we have the limit in the first case", but "the
  performance degradation is not linear" — 150K "will slow down some operations by
  20-30% may be" (VENDOR, Tracker #1144, Oct 2020). The original reason was memory
  (Chromium, "few gigabytes of RAM", Tracker #138).
- **What N counts.** The Windows command is `log HEAD --branches --remotes --tags
  --max-count=N --no-show-signature --date-order` (LOG, TrackerWin #2380). So N is
  the newest N commits by commit date across `HEAD`, every branch, remote-tracking
  branch and tag — stashes are not seeds (consistent with
  `fork-unreachable-stash-base.md` Headline 2). How the Mac version walks is OPEN:
  its logs show an in-process object reader ("Bt error: Failed to read stashes",
  Tracker #2075) and a benchmark step called "Reading commits" (Tracker #2341),
  but no Mac log quotes the walk. The behaviour users report is the same on both.

  Note for readers of `fork-unreachable-stash-base.md` Headline 6 ("No issue or
  log shows Fork running `git log` for the commit list"): the Windows logs above
  do show it. That record is left as it was; this is the correction.
- **All Commits and a branch filter alike.** The filter narrows the capped set, it
  does not re-walk. Evidence:
  - Tracker #1401 (Mac 2.8, 2021, open): "Filter by Active Branch shows blank";
    the user's diagnosis that the filter operates on the capped collection is
    confirmed by the vendor. The user's ask — rebuild the list from the branch
    alone — has no reply.
  - Tracker #1626 (2022, open, no reply): a branch "a couple hundred thousand
    commits behind HEAD" in a million-commit repository shows no commits when
    filtered; raising the cap past 500,000 "is not really a solution".
  - TrackerWin #1379 (Unreal Engine): "Even if I try to do Show 'branch' commits
    only", nothing older appears; TrackerWin #1854: a year-old branch filtered
    alone shows nothing; TrackerWin #1419 the same; Tracker #1194 the same, fixed
    by raising the cap.
  - TrackerWin #663 places the change: before Windows 1.45 (Feb 2020) filtering an
    old branch put "its latest commit … at the top of the view"; after 1.45's
    "branch filtering performance improvements" the same filter shows "an empty
    white screen" (USER). The vendor: "This is the case I absolutely missed."
    Mac's matching note is 1.0.89 (Jan 2020), "Filter performance" (RN). INFERRED:
    both versions moved filtering from a per-branch walk to a selection over the
    one capped walk at that time.
  - A user's suggestion that the cap count from the filtered branch's tip
    ("count from the last commit may be a better way?", TrackerWin #1379, #1419)
    has no reply.

**Evidence:** VENDOR, consistent 2017-2025, on both trackers; LOG for the command.
**Platform:** both; numbers differ from Mac 2.41.

## 2. Loaded up front or paged?

- **Up front, to N, on every refresh.** Each refresh logs
  `RefreshRepositoryDataGitCommand All` followed by the one capped `git log`
  (LOG, TrackerWin #1543, #1647; Tracker #1506). Nothing published shows a second
  walk on scroll, a "loading more" state, or a resume. Neither release-note list
  has a line about loading more commits, paging or lazy commit loading (searched
  for "load more", "lazy", "pagination", "commit list" and "revision log"); the
  nearest are Mac 1.0.15 "Revision log loading is 3 times faster now" (2016) and
  Mac 1.0.19 "Load revision details in background thread" (2016).
- **Pagination was planned and replaced by a bigger cap.** "I'm going to implement
  a pagination one day, but it's a low-priority task" (VENDOR, Tracker #138,
  2017); "most probably we need to start loading commits dynamically" (VENDOR,
  TrackerWin #663, 2020); "we are working on that" (VENDOR, Tracker #138, Oct
  2023); then the 100,000 raise (Tracker #2076, 2024). Users asked for paging
  repeatedly (Tracker #138, #411, #1461 "paging and properly handling old commits
  … is clearly needed"; TrackerWin #663 "try loading 10,000 at a time until the
  commit they requested is visible"). Status today: not shipped, as far as
  published.
- **What a row holds.** The 2021-2022 Windows walk read
  `%H%n%P%n%aN%n%aE%n%at%n%s` (id, parents, author, e-mail, time, subject; LOG,
  Tracker #1506, TrackerWin #1543, #1647). The Nov 2024 walk (Fork 2.2.3) reads
  `%H%n%P` only (LOG, TrackerWin #2380). INFERRED: since some Windows version
  between 2022 and 2.2.3, the capped walk holds ids and parents and the row text
  is read separately. Consistent with TrackerWin #2847 (Sep 2026, Win 2.22, "about
  100K commits in current branch"): messages blank until "scrolling the view down
  slightly and waiting a few seconds"; the vendor's fix shipped as Win 2.23's
  "Blank commit messages for some users" (RN), and he described the trigger as a
  repository with "a massive number of merges" and a commit count that "wouldn't
  even fit in memory". Mac's earlier blank-messages bug (2.38-2.40, Tracker #2075,
  linked to `git maintenance`) points the same way for Mac: INFERRED.
- **Lane layout is computed for the visible area.** Graph visualization "used to
  be a bottleneck"; "I made it lazy, so it calculates only the visible area", and
  "ideally the algorithm must be lazy" (VENDOR, Tracker #345, 16 Jul 2018,
  <https://github.com/fork-dev/Tracker/issues/345#issuecomment-405181105>). Mac
  1.0.69 (Aug 2018) "Improve revision graph drawing" and Win 1.71 (Mar 2022)
  "Simplify graph drawing" are the only later graph-drawing notes (RN). Whether
  anything per-row is retained after the visible area moves on (a cache, a
  per-row lane assignment): OPEN.
- **Virtualization.** No public statement. INFERRED: the list is drawn by the
  platforms' row-recycling list views and only visible rows are built; nobody has
  reported per-row slowness that scales with N. OPEN as a stated fact.
- **Memory on huge repositories.** The published complaints are few and none ties
  memory to graph width or lanes:
  - The cap's origin: Chromium, "few gigabytes of RAM" (VENDOR, Tracker #138).
  - Windows was 32-bit until 1.82 (Feb 2023), made 64-bit "In order to solve"
    an `OutOfMemoryException` on an Unreal Engine 5 clone; the reporter saw Fork
    at "~1.5GB at startup, then drops to around a constant 350MB" (TrackerWin
    #1784; VENDOR for the 64-bit move).
  - "6 repos and over 1GB of memory usage" (USER, Tracker #28, 2018);
    1-1.5 GB on a large Windows repository, a leak fixed in Win 1.27 (TrackerWin
    #122, 2019; vendor "I can't really make it use more than 350 MB").
  - The 2025-2026 reports of 42-600 GB (Tracker #2512, #2700) are a repository
    scan and a regex, not the commit list.
  - No vendor statement about lane width and memory, or about memory per commit,
    was found.
- **Measurement hook.** Repository → Benchmark (Mac 2.25, Win 1.80, RN) times
  "Reading commits" among other steps (Tracker #2341); the vendor asked a huge-repo
  user for it in 2026 "to understand what needs to be optimized in huge repos"
  (TrackerWin #2847). No published report gives that step on a repository near the
  cap.

**Evidence:** LOG and VENDOR for the walk and the plans; INFERRED for row contents
and virtualization. **Platform:** Windows logs; Mac by the vendor's shared answers.

## 3. Asking for a commit the list has not loaded

Fork never loads more in response to a request, so "not yet loaded" and "beyond the
cap" are the same case: the commit is not in the list.

| Affordance | What Fork does when the commit is not in the list | Evidence |
| --- | --- | --- |
| Sidebar branch, tag or stash press | List does not move; nothing says why. Details pane may show the commit (see below). | USER, Tracker #411: "nothing happens"; VENDOR, TrackerWin #663: "it will not be visible" |
| Filter to that branch (sidebar or ⌘⇧A / Ctrl+Shift+A, Filter by Active Branch) | Empty list ("blank commit tree window") | Tracker #1401, #1194, #1626; TrackerWin #1379, #1419, #1854, #663 |
| Context search, ⌘F / Ctrl+F, by SHA | Not found: it searches the rows the list holds | USER, TrackerWin #2425: "Fork does not find commits (F3) outside that range"; Tracker #804 comment (a 7-year-old commit "with search") |
| Advanced search panel, click a result | Reveals it only "if it's in the commit tree" | VENDOR, Tracker #1144 |
| Quick Launch (⌘P / Ctrl+P), type a hash, Reveal | Not revealed; no warning (request open since 2020) | USER, TrackerWin #990 |
| File History or Blame: double-click a commit, or click its SHA | Does not reveal it in the main list; Win 1.45 showed "the same empty graph view" | USER, Tracker #804; TrackerWin #663; jump mechanics VENDOR, TrackerWin #941, #980 |
| "Reveal in Fork" from file history | Shown "only in detail window"; not selected in the list | USER, TrackerWin #2131 (2024, open; a commenter suggests a collapsed branch) |
| Parent SHA link in the Commit tab | "UI does not navigate to the parent commit"; vendor's first diagnosis is the cap | USER and VENDOR, Tracker #1669: "is it possible that this commit is out of the range?" |

- **No dedicated "go to commit".** Neither shortcut page in `fork-dev/Docs` lists
  one: the All Commits keys are ⌘0 / Ctrl+0 "Jump to HEAD", ⌘F / Ctrl+F "Commit
  search" with Return / F3 to the next result, and ⌘2 / Ctrl+2 whose "second press
  will jump to HEAD"
  (<https://github.com/fork-dev/Docs/blob/master/keyboard-shortcuts-mac.md>,
  `keyboard-shortcuts-windows.md`; last changed 2023). The vendor's answer to "jump
  to a SHA" is the context search: "useful when you want to quickly jump to
  something (sha, branch, author, message)" (VENDOR, TrackerWin #1088). That search
  also skips collapsed merges (TrackerWin #1076, open) and, on Windows, searches
  downward from the selection only — "Shift+Enter will probably find it" (VENDOR,
  TrackerWin #2698, 2026; kept open "as we need to fix this").
- **The details pane is decoupled from the list.** A commit that is never a row
  can still be shown: a stash's third parent is reached through the Commit tab's
  parent link (USER, Tracker #898, 2022). A Windows user whose recent branches had
  been hidden reported that pressing them "does not jump to it in the history
  window" while "the lower part of the history window" still updated (USER,
  TrackerWin #2463, cause: a hidden branch folder). INFERRED: pressing a ref whose
  commit is not in the list selects nothing in the list but may show that commit's
  details. What exactly the pane shows, and whether this holds for a commit beyond
  the cap rather than hidden: OPEN.
- **What users are told to do.** Every vendor answer is "raise `maxCommitCount`"
  (Section 1) or, once, rebase the old branch "on a newer commit to make it
  visible" (VENDOR, TrackerWin #663). In 2020 a user proposed "showing a warning
  message when the commit you're trying to view is past the 25,000" (USER,
  TrackerWin #663); TrackerWin #990 (open since 2020) asks for a warning with a
  "Clear all branch filters and select the commit" action. Neither is answered.
- **The one stated intent.** For a branch hidden by a collapse or another branch's
  filter, the vendor said in Aug 2026: "An explicit click must always reveal a
  branch when it's collapsed", and "Fork must beep when the branch is not visible
  in the current topology" (VENDOR,
  <https://github.com/fork-dev/TrackerWin/issues/2828#issuecomment-5227510103>).
  A beep is the whole of the planned feedback; the cap is not mentioned. No
  release note through Mac 2.70 (4 Sep 2026) or Win 2.23 (25 Sep 2026) records
  either: OPEN.

**Evidence:** USER, consistent over 2018-2025; VENDOR for the cause; no published
counter-example. **Platform:** both.

## 4. Simplifying a wide graph

- **Collapse merges, the vendor's answer to width.** Mac 1.0.94 (Jun 2020)
  "Ability to selectively collapse merges in commit graph" and "collapse/expand
  all"; Win 1.50/1.51 the same; arrow keys collapse and expand (RN). The blog post
  demonstrates it on `apple/swift`, "more than 100,000 commits"
  (<https://fork.dev/blog/posts/collapsible-graph/>, 3 Aug 2020). Asked for a
  horizontally scrollable graph column, the vendor: "the best way to navigate in
  a messy graph is to collapse all" (VENDOR, Tracker #516, 2023). Collapse state
  is remembered (Mac 2.67, Win 2.20) and stored per worktree (Win 2.23) (RN).
- **First-parent view.** Mac 1.0.89 (Jan 2020) "'First parent' view mode" (RN). No
  matching Windows note was found: OPEN for Windows.
- **Hiding and filtering refs.** Hide remote branches (Mac 1.0.68, Win 1.41), hide
  tags, hide particular branches, folders or remotes (Mac 2.6, Win 1.60), branch
  filters on folders and remotes (same releases) (RN). The vendor's first advice
  on a busy repository is a filter (TrackerWin #883).
- **Drawing simplifications.** Win 1.71 (Mar 2022) "Simplify graph drawing"; Win
  1.98 (May 2024) "Show simplified graph in tooltip for merge nodes"; compact
  branch labels (Win 1.55, Mac 2.19), which shorten labels, not lanes (RN).
- **No lane cap, no scroll.** The graph column is cropped on the right when too
  wide (USER screenshot, Tracker #516, 2019); "Horizontal scrolling will not appear
  in the near future" (VENDOR, 2023). A request for a configurable graph width
  (TrackerWin #757) was answered with collapse.
- **None of these is published as bounding per-row cost or memory.** INFERRED:
  collapse and first-parent change what is drawn; whether they change what is
  walked or counted against the cap is OPEN. The per-row bound, if any, is the lazy
  visible-area layout (Section 2).

**Evidence:** RN and VENDOR. **Platform:** both, except first-parent (Mac).

## 5. Secondary precedent

| Client | Cap or paging | Asking for an unloaded commit | Memory on huge repos | Evidence |
| --- | --- | --- | --- | --- |
| Sourcetree | Pages: "Log rows to fetch per load"; "Fetching older commits" | A sidebar branch click pages forward ("fetching older commits..."); Jump To → Commit pages too, slowly | 100 GB report (prior record) | Below |
| GitKraken Desktop | "initially displays up to 2000 commits"; lazy load setting | Search covers only loaded history | Remedy: lower the cap | Below |
| GitLens Commit Graph | Pages as needed | A jump pages the target in, with feedback and remedies | — | Below |
| Tower | Loads more on scroll | "Reveal in History" | — | Below |
| SmartGit | No cap documented; graph limited to selected branches | Filter search runs incrementally over the whole repository | Raise the heap | Below |
| Sublime Merge | No cap; loads in the background | Navigate to Commit can fail on the Linux kernel | Vendor: should run smoothly at millions | Below |
| GitUp | Loads the entire history | n/a | — | Below |
| gitk | Reads the whole log progressively | Goto finds only commits already read | Retains everything (prior record) | SOURCE |
| tig | Reads progressively | `:goto` scans loaded lines | Unbounded lines (prior record) | SOURCE |

- **Sourcetree (Atlassian).** Paging is a user setting, "Log rows to fetch per
  load", with "Fetching older commits" shown while a batch loads; prior research
  covers the setting and its fragility (`docs/research/history-graph/scroll-memory-model.md`
  Findings 16-19). New here: a sidebar branch click pages forward until the branch
  arrives — "click on branch -> 'fetching older commits...'", reported as a bug
  when it fired for a branch already loaded (SRCTREEWIN-7276, v2.0.20, fixed
  3.4.17, <https://jira.atlassian.com/browse/SRCTREEWIN-7276>). And Jump To →
  Commit pages to its target: "Using 'Jump To | Commit...' to jump to the initial
  commit takes several minutes" on 15,000+ commits, where gitk was instantaneous
  (SRCTREEWIN-7024, v2.0.18, closed as duplicate,
  <https://jira.atlassian.com/browse/SRCTREEWIN-7024>). In 3.4.27-3.4.28 Sourcetree
  could not load earlier commits at all (SRCTREEWIN-14669, fixed). Sourcetree is
  the precedent for "find a ref by paging the walk forward"; its record is that the
  path is slow and breaks.
- **GitKraken Desktop.** "GitKraken Desktop initially displays up to 2000 commits
  in the Commit Graph"; "If the graph is not loading enough history, search results
  can appear incomplete" (DOC, <https://help.gitkraken.com/gitkraken-desktop/search/>).
  Preferences: "Initial Commits in Graph" ("minimum: 500"), "Lazy Load Commits"
  ("Loads more commits only as needed"), "Show All Commits in Graph" ("May affect
  performance on large repos") (DOC,
  <https://help.gitkraken.com/gitkraken-desktop/preferences/>). The performance page
  says to "Lower the Max Commits in Graph value"
  (<https://help.gitkraken.com/gitkraken-desktop/performance-issues/>). A feedback
  item titled "Search commits beyond graph limit" exists
  (<https://feedback.gitkraken.com/suggestions/228441/search-commits-beyond-graph-limit>,
  not readable: HTTP 403); its content and status are OPEN. Whether a Desktop
  sidebar click on an unloaded ref pages it in: OPEN.
- **GitLens Commit Graph (GitKraken, VS Code).** The closest precedent to the
  question. GitLens 19.0 (12 Aug 2026): a jump to "a reference, a search result, or
  a deep link" now "reliably lands on it even when the commit has to be paged in
  first"; "`Ctrl`+`↓` jumps straight to the first parent even when it isn't loaded
  yet" (DOC, <https://help.gitkraken.com/gitlens/gitlens-release-notes-current/>).
  GitLens 19.1 (1 Sep 2026): keyboard navigation "reaches across not-yet-loaded
  history"; a jump blocked by filters, visibility, scope or first-parent following
  "names the blocker and offers a one-click remedy"; "A commit that isn't in the
  repository is reported immediately instead of silently timing out" (same page).
  The issue behind it records the failure mode Fork has today: before the fix a
  sidebar click ran a header progress bar "and then nothing. Up to 30 seconds
  later, still nothing", and the requirements were to distinguish "hidden by your
  current filters or scope" from "not in this repository's loaded history", offer
  the remedy as an action, and "Stay completely silent when a navigation is simply
  superseded" (VENDOR author, <https://github.com/gitkraken/vscode-gitlens/issues/5699>,
  Aug 2026). A tester could not produce the failure feedback and left it
  "partially verified" (same issue). Its memory cost on a deep page-in is not
  published: OPEN.
- **Tower.** Loads more history as the list scrolls: "The commit history now loads
  correctly when scrolling down to the bottom" (RN, Nov 2021)
  and "Loading more commits now preserves scroll position and keeps graph lines
  accurate" (RN, Jun 2026); "Reveal in History" reveals "a branch, tag, or any
  commit" (RN, 2022-2023), and a 2026 fix for "Reveal in History doing nothing
  while viewing File History or Blame" (<https://www.git-tower.com/release-notes/mac>;
  the page interleaves Mac and Windows entries, so version-to-platform attribution
  is uncertain). Whether Reveal pages a deep commit in, and how far Tower loads per
  page: OPEN.
- **SmartGit (syntevo).** The graph shows the branches toggled in the Branches view
  (DOC, <https://docs.syntevo.com/SmartGit/Latest/Manual/GUI/Log-Window>); its
  filter search restarts "from the selected Branches and return[s] matching commits
  incrementally", and "eventually, SmartGit will find all matching commits in the
  entire repository" (DOC, <https://docs.syntevo.com/SmartGit/Latest/Manual/GUI/Graph-View>).
  Performance advice is to "deselect as many Branches as possible" and raise the
  memory limit; `log.graph.filterCommitCache` can be disabled for "memory-related
  problems when filtering" (DOC,
  <https://docs.syntevo.com/SmartGit/HowTos/Performance-Tuning.html>). A commit cap,
  paging, and what Reveal Commit does for a commit not in the graph are not
  documented: OPEN. ("Show More Commits" exists only in the Working Tree window's
  Journal, a different view.)
- **Sublime Merge.** No cap is documented, and the vendor's stance is "Even with
  millions of commits, Sublime Merge should run smoothly" (VENDOR,
  <https://github.com/sublimehq/sublime_merge/issues/1937#issuecomment-2224101003>,
  2024). It loads in the background: build 2020 "Improved selection behavior while
  loading large repositories" (RN, <https://www.sublimemerge.com/download>), and on
  the Linux kernel "Navigate to Commit" failed for some ids; the vendor's first
  question was "After waiting 1 minute after opening the repository, does the
  issue still occur?" (VENDOR, #1463, 2022, still open; the user: still failing
  after an hour). Build 1084: "Navigate to Commit now works as expected for hidden
  commits"; collapsed merges can hide a target from navigation (#1940, open). A
  compact-graph request is open since 2018 (#19). Memory on huge histories: no
  published figure; the large-memory reports found concern working-tree changes
  (#1721).
- **GitUp.** Loads everything: "Load the *entire* repo history in memory for fast
  access" (README, <https://github.com/git-up/GitUp>); it "loads and renders the
  entire graph of 40,000 commits" of git.git "in less than a second"
  (<https://gitup.co/>). A contributor timed graph generation on a ~237,000-commit,
  ~22,000-ref repository at 76 s, then 3.6 s after a fix
  (<https://github.com/git-up/GitUp/pull/3>, 2015). No memory figure published.
- **gitk.** Reads the whole log progressively and keeps it (prior record,
  `scroll-memory-model.md` Findings 6-11). `gotocommit` resolves a prefix with
  `longid`, which looks only at commits already read (`vshortids`, `varcid`), then
  `commitinview`; a miss is an error popup, "Commit ID %s is not known" or
  "Revision %s is not in the current view" — it does not wait for the walk
  (SOURCE, `/usr/bin/gitk` from git 2.56.0, `proc gotocommit`, `proc longid`). Only
  the initial selection waits: `pending_select` is selected when its commit
  arrives (same file).
- **tig.** `:goto <rev>` resolves the expression with `git rev-parse`, then scans the
  lines loaded so far; a miss reports "Unable to find commit '%s'" (SOURCE,
  `goto_id` in <https://github.com/jonas/tig/blob/7d841c9/src/view.c>).
- **lazygit and VS Code Git Graph** are covered in `scroll-memory-model.md`
  Findings 13-14 (a 300-row cap dropped wholesale; a growing prefix re-laid out
  per page).

## Bearing on the question (no decision)

- Fork's answer to "too long to hold" is a fixed window plus lazy layout: hold ids
  and parents for the newest N commits across every ref, compute lanes only for
  what is on screen, and refuse — silently — anything older. Its cost is visible in
  the tracker for nine years: blank filters, inert sidebar clicks, search that
  cannot find, and a configuration key users learn from the vendor one issue at a
  time.
- For scale against `deep-find-measured.md`: Fork's Mac window, 100,000 commits,
  is the depth at which Cairn's current rows on rust retain 575 MiB in 0.82 s, and
  Windows' 50,000 the depth at about 0.43 s. Fork's own per-row cost at that depth
  is not published.
- The behaviour the packet's R8.5 describes — page the held walk forward until the
  ref's row arrives — is what Sourcetree has done since at least 2017 and what
  GitLens shipped in Aug 2026, with GitLens adding what Sourcetree lacked: failure
  messages that name the cause, silence on supersession, and an immediate answer
  for an id the repository does not have. Fork does not do it.

## OPEN

1. Whether Fork for Windows has raised its 50,000 default since March 2025.
2. How the Mac version walks the commit list (a `git log` like Windows', or its
   in-process reader), and whether its cap counts the same set.
3. Whether Fork virtualizes the list, and whether any per-row graph state is
   retained after its rows scroll off (the vendor says layout is lazy, not whether
   it is cached).
4. Fork's memory per commit or per row near its cap; no Benchmark report on a
   huge repository is public.
5. What the details pane shows when a sidebar ref, Quick Launch Reveal or parent
   link names a commit beyond the cap, as opposed to a hidden one.
6. Whether the Aug 2026 "beep" and "explicit click must always reveal" intents
   (TrackerWin #2828) shipped, and whether they cover the cap.
7. Whether collapse or first-parent mode changes the walk or the cap, or only the
   drawing; whether Windows has a first-parent mode.
8. GitKraken Desktop: what a sidebar click on an unloaded ref does; the content of
   the "Search commits beyond graph limit" request.
9. GitLens: the memory and time cost of a deep page-in, and whether the failure
   feedback has been verified in a release.
10. Tower and SmartGit: page size, any cap, and what Reveal does for a deep commit.
11. Sublime Merge: whether its background load holds the whole history, and its
    memory on the Linux kernel or rust.
