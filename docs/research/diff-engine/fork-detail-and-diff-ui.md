# Fork's commit detail pane and diff view: what it shows, how it behaves, what it looks like

Evidence record. Gathered 2026-09-17 for the `diff-engine` packet, whose UI — the
commit detail pane and the diff view — the project owner wants modelled on Fork
(git-fork.com / fork.dev, by Dan and Tanya Pristupov; native apps for macOS and
Windows). It answers, per platform: where the detail pane sits and what each tab
shows; what selecting two commits, a merge commit or a stash shows; how the diff
view behaves (modes, gutters, headers, context, whitespace, highlighting, wrap,
search, keys); whether Fork renders one file or many, and what it does with huge
commits, huge files and non-text changes; how line and hunk staging look (context
only); and what the pane and the diff look like, measured from screenshots.

Companions, not repeated here: `what-clients-show.md` in this directory (the
cross-client precedent study — its Findings 2, 9, 10, 11, 12, 13, 14, 15, 23 and 25
touch Fork; this record extends them and corrects several, listed under
"Corrections"), `docs/research/history-graph/what-clients-show.md` (its Finding 9
listed the detail pane's fields), `ui-and-app-as-built.md` (what Cairn and Freya can
do today) and the mockup `docs/design/mockups/cairn-ui.html` that
`docs/design/ui.md` describes.

## Why this record exists

The packet has to pick a detail-pane layout and a diff presentation before its diff
model locks, and `ui.md` leaves two of those choices open (pane below or to the
right; side-by-side). "Model it on Fork" is only actionable if Fork's behaviour is
known precisely — and Fork is two separately written applications (AppKit on Mac,
WPF on Windows) whose features arrive months or years apart and sometimes differ for
good. This record pins each behaviour to a platform, a version and a date, so a
design decision can cite evidence rather than a recollection of Fork.

## Method

### Sources

| Source | What was read | Where |
| --- | --- | --- |
| Mac release notes | Every entry, from GitClient 1.0.0 (17 Apr 2016) to Fork 2.70 (4 Sep 2026) — 1,156 entries | <https://git-fork.com/releasenotes> |
| Windows release notes | Every entry, from 1.14 (29 Mar 2018, where the page starts) to 2.22 (28 Aug 2026) — 735 entries | <https://git-fork.com/releasenoteswin> |
| Blog | All 14 posts (2018–2020), with their screenshots and GIFs (frames extracted) | <https://fork.dev/blog/posts/> |
| Home page | Feature overview and the six carousel screenshots (Mac and Windows, late 2020) | <https://git-fork.com/> |
| Vendor-hosted docs | `fork-dev/Docs`: keyboard shortcuts (Mac, Windows) and FAQ. Its README calls it user-contributed; the two shortcut lists are identical to the lists the vendor posted as Tracker #309 (Dan Pristupov, Jun 2018) and TrackerWin #333 (Tanya Pristupova, Jun 2019) | <https://github.com/fork-dev/Docs> |
| Trackers | GitHub search over `fork-dev/Tracker` (Mac) and `fork-dev/TrackerWin` (Windows) for the queries listed below; 440 issues fetched with all their comments and read | <https://github.com/fork-dev/Tracker>, <https://github.com/fork-dev/TrackerWin> |
| Screenshots | Vendor and user images attached to those issues and posts: inspected visually; colours sampled and rows measured with a script (Part F) | per finding |
| Third-party | "Best Git GUI" listicles (dev.to, epigra, thesoftwarescout, lithiumgit, slashdot/sourceforge review pages) and one 2026 guide (terminal.guide) | see Limits |

Tracker queries (each run against both repositories): `diff "side by side"`,
`side-by-side`, `"entire file"`, `"full file"`, `"context lines"`, `context`,
`expand diff`, `hunk`, `chunk`, `whitespace`, `"ignore whitespace"`, `"too large"`,
`"load diff"`, `"large file"`, `"commit details"`, `"file tree"`,
`"merge commit" diff`, `"merge commit" changes`, `"first parent"`,
`"second parent"`, `combined diff`, `remerge`, `diff parent merge`,
`compare commits`, `"two commits"`, `"image diff"`, `binary`, `lfs diff`,
`submodule diff`, `symlink`, `encoding`, `rename`, `"word wrap"`,
`"syntax highlighting"`, `highlighting diff`, `"word diff"`, `"inline"`,
`"keyboard shortcut" diff`, `"next change"`, `minimap`, `"line numbers"`,
`"tab width"`, `search diff`, `horizontal scroll diff`, `"font size" diff`,
`"diff colors"`, `color diff dark`, `"vertical layout"`, `layout details`,
`stash diff`, `untracked stash`, `"multiple files" diff`, `"show more"`,
`"collapsed"`, `"expand all"`, `committer`, `author date`, `refs commit details`,
`tags commit details`, `branches contains commit`, `"parent"`, `parents link`,
`"Show in File Tree"`, `"eye icon"`.

Issue references below are written `Tracker #N` (Mac,
`https://github.com/fork-dev/Tracker/issues/N`) and `TrackerWin #N` (Windows,
`https://github.com/fork-dev/TrackerWin/issues/N`); vendor statements carry a
comment permalink. Vendor identity: `DanPristupov` and `TanyaPristupova` — Dan and
Tanya Pristupov, the developers the site names — whose tracker comments GitHub
labels CONTRIBUTOR and COLLABORATOR respectively.

### Version numbering

The two apps are versioned independently. Mac went from 1.0.99 to 2.0 on 13 Nov
2020; Windows stayed on 1.x until 2.0 (23 Aug 2024). "2.21" is therefore Aug 2022
on Mac and Jul 2026 on Windows. Every version below carries its platform and date.

### How Fork is built, as far as it bears on the UI

- **Mac is AppKit**: lists are `NSTableView`/`NSOutlineView` and diff text is
  `NSTextView` (vendor: Tracker #1955,
  <https://github.com/fork-dev/Tracker/issues/1955#issuecomment-1710654980>,
  7 Sep 2023; #773,
  <https://github.com/fork-dev/Tracker/issues/773#issuecomment-562505540>,
  6 Dec 2019; #1852, 28 Mar 2023). **Windows is WPF on .NET Framework 4.7**, with the
  AvalonEdit text control (vendor: TrackerWin #94,
  <https://github.com/fork-dev/TrackerWin/issues/94#issuecomment-475281669>,
  21 Mar 2019; #531,
  <https://github.com/fork-dev/TrackerWin/issues/531#issuecomment-550909225>,
  7 Nov 2019; #480, 9 Apr 2026). Several constraints below trace to these controls.
- **Fork computes no diff of its own; it runs git and parses the patch.** The
  vendor says Fork is not really involved in producing diffs (Tracker #808,
  <https://github.com/fork-dev/Tracker/issues/808#issuecomment-900998253>,
  18 Aug 2021), that it has no diff tool and only displays git's output (Tracker
  #1087, <https://github.com/fork-dev/Tracker/issues/1087#issuecomment-3817991860>,
  29 Jan 2026; TrackerWin #1834,
  <https://github.com/fork-dev/TrackerWin/issues/1834#issuecomment-1460707912>,
  8 Mar 2023), and advises `git config diff.algorithm patience` to change hunks
  (Tracker #2597,
  <https://github.com/fork-dev/Tracker/issues/2597#issuecomment-4235617012>,
  13 Apr 2026). A Mac user's log shows `PatchTokenizer.swift` parsing
  `diff --git` headers that use `forkSrcPrefix/` and `forkDstPrefix/` prefixes
  (Tracker #1926, 7 Jul 2023), and a Windows user captured the exact command used
  to show a commit (TrackerWin #2030, Finding 8).

### Evidence labels

As in the companion record, adapted to this subject:

- **RELEASE NOTES** — an entry on either release-notes page, with version and date.
- **VENDOR DOCS** — the vendor's site, blog, or vendor-hosted docs repository (its
  shortcut lists are the vendor's own; its FAQ is used with the README's
  user-contributed caveat).
- **VENDOR STATEMENT** — a comment by `DanPristupov` or `TanyaPristupova` in a tracker.
- **VENDOR SCREENSHOT** — an image the vendor published (site, blog, tracker comment).
- **USER SCREENSHOT** — an image a user attached, dated.
- **USER REPORT** — a user's description in a tracker (single, or consistent across users).
- **TRACKER STATE** — an issue's existence and state, without a vendor comment.
- **MEASURED** — pixel values or distances sampled from a screenshot; approximate.
- **THIRD-PARTY** — reviews and tutorials.
- **INFERRED** — reasoning over the above, never stated as observation.
- **OPEN** — looked for, not found.

### Limits

- **No hands-on install.** This machine runs Linux and Fork has no Linux build (the
  vendor-hosted FAQ says there is no plan for one,
  <https://github.com/fork-dev/Docs/blob/master/faq.md>). Everything here is
  documentary; items that a few minutes on the owner's Fork would settle are
  collected under "What the evidence does NOT settle".
- **Screenshots span 2016–2026.** Each visual fact is dated to its image. Mac
  captures may carry a display colour profile, so the same colour differs by a few
  units between captures; values are approximations with their sources.
- **Vendor statements are paraphrased**, with permalinks, rather than quoted.
- **Third-party material added nothing verifiable.** The listicles repeat the
  vendor's feature list. The one concrete claim beyond it — a right-click
  `Stage Selected Lines` item in the diff (terminal.guide, 20 Jan 2026,
  <https://www.terminal.guide/tools/git-tool/fork/>) — is supported by no vendor
  source or screenshot found (Finding 23 describes the documented gestures), so no
  third-party source is used as evidence.
- The in-app browser was not needed: pages, images and tracker data were fetched
  read-only (HTTP and the GitHub API); nothing was logged into or submitted.

## Part A — The commit detail area

### Finding 1 — The pane sits below the commit list on both platforms; only Windows can move it to the right; both can now collapse it

- **Default: below the list.** Every vendor screenshot of the main window shows it
  there (Mac carousel
  <https://git-fork.com/images/carousel/carousel_mainMac.jpg>, Windows carousel
  <https://git-fork.com/images/carousel/carousel_mainWin.jpg>, late 2020; Windows
  1.27 blog screenshot, <https://fork.dev/blog/posts/forkwin-1.28/>, Feb 2019).
- **Windows offers a right-hand layout.** The toolbar `Appearance` menu has
  `Commit List Layout` → `Horizontal` / `Vertical` (vendor screenshot, TrackerWin
  #1023, <https://github.com/fork-dev/TrackerWin/issues/1023#issuecomment-725879275>,
  12 Nov 2020). `Vertical` puts Commit / Changes / File Tree to the right of the
  list; commit rows become two lines when the list is narrow. History: an
  experimental vertical layout in Windows 1.28 (22 Feb 2019), then with an
  `Orientation` toolbar button (user screenshot, TrackerWin #55, 28 Feb 2019);
  vendor confirmation
  <https://github.com/fork-dev/TrackerWin/issues/55#issuecomment-466468812>; rows
  made responsive (one line when wide) in 1.33 (17 May 2019;
  <https://github.com/fork-dev/TrackerWin/issues/193#issuecomment-493506411>).
  Asked for a different Local Changes arrangement, the vendor argued that a lower
  element must not drive an upper one, keeping a left-to-right, top-to-bottom flow
  (TrackerWin #2175,
  <https://github.com/fork-dev/TrackerWin/issues/2175#issuecomment-2002037224>,
  16 Mar 2024).
- **Mac has no such option.** Tracker #756 (Sep 2019, open, 37 reactions) asks for
  the Windows layout; vendor: on the to-do list, no ETA
  (<https://github.com/fork-dev/Tracker/issues/756#issuecomment-878467252>,
  12 Jul 2021); later, layout customization is difficult and there is no time for
  it (Tracker #1937,
  <https://github.com/fork-dev/Tracker/issues/1937#issuecomment-1642206355>,
  19 Jul 2023). Duplicates keep arriving (#2434 Aug 2025, #2587 Mar 2026).
- **Resizing and collapsing.** A draggable splitter separates list and pane (Mac
  1.0.71, 10 Oct 2018, splitter headers draggable; Windows 1.20, 13 Aug 2018,
  thicker separators; Windows 1.22, 26 Oct 2018, the whole details bar draggable).
  The pane collapses on Mac 2.66 (10 Apr 2026, ⌘D) and Windows 2.19 (24 Apr 2026,
  Ctrl+Shift+D).
- **Detached views.** A commit opens in a separate window (Mac 2.14, 26 Nov 2021;
  Windows 1.69, 19 Nov 2021) — `Enter` on a commit, per the vendor — and `Space`
  on a commit opens a quick-look popup of its details (Mac 2.67, 15 May 2026;
  Windows 2.20, 5 Jun 2026) carrying an `Open in Separate Window` button (vendor
  screenshots, TrackerWin #2800,
  <https://github.com/fork-dev/TrackerWin/issues/2800#issuecomment-4679210536>,
  11 Jun 2026).

**Evidence:** RELEASE NOTES; VENDOR STATEMENT; VENDOR SCREENSHOT; USER SCREENSHOT.
**Platform:** both; right-hand layout Windows only.
**Implies:** `ui.md`'s open question — pane below the graph, as in Fork, or to its
right — has Fork precedent for both, on one platform. The Mac app has declined the
right-hand layout for seven years on cost.

### Finding 2 — Three tabs; Commit is the default (remembered on Mac since 2022); Changes and File Tree carry a one-line commit summary

- **Tabs `Commit`, `Changes`, `File Tree`** in every screenshot since 2018. Mac draws
  them as a segmented control (the selected tab is a filled rounded pill); Windows
  as text tabs with an accent-coloured underline (vendor and user screenshots,
  2018–2025).
- **No count or badge on any tab** (all screenshots).
- **Default `Commit`.** The vendor preferred remembering the last active tab to a
  default-tab setting (Tracker #573,
  <https://github.com/fork-dev/Tracker/issues/573#issuecomment-472526405>,
  13 Mar 2019); shipped on Mac 2.23 (21 Oct 2022). Windows: a 2020 request to
  remember the tab is open (TrackerWin #930); current Windows behaviour OPEN.
- **Keys.** ⌘⌥1 / ⌘⌥2 / ⌘⌥3 switch the tabs on Mac (vendor, Tracker #1127,
  <https://github.com/fork-dev/Tracker/issues/1127#issuecomment-688719885>,
  8 Sep 2020); Tab / Shift-Tab move focus between the commit list and the details
  (Mac 2.67; Windows 2.20, which includes the sidebar).
- **Summary line.** `Changes` and `File Tree` show a one-line summary above their
  content — avatar, author, abbreviated SHA, date, subject — since Mac 1.0.40
  (10 Mar 2017) (vendor screenshots: Windows 1.38 blog
  <https://fork.dev/blog/posts/forkwin-1.38/>, 2019; Mac, Tracker #2258,
  8 Dec 2024). Users report the abbreviated SHA there can be copied, whereas the
  abbreviated parent hashes in the Commit tab cannot (TrackerWin #994, Oct 2020 and
  Feb 2025).
- **Why two tabs.** The vendor reads descriptions in `Commit` and inspects changes
  across a run of commits in `Changes`, and judged a merged view would cramp one of
  the two (Tracker #194,
  <https://github.com/fork-dev/Tracker/issues/194#issuecomment-359880698>,
  23 Jan 2018).

**Evidence:** RELEASE NOTES; VENDOR STATEMENT; VENDOR SCREENSHOT; TRACKER STATE.
**Platform:** both, except where noted.

### Finding 3 — The Commit tab header: author and committer columns with avatars, the full SHA, parents as short-hash links, refs as chips; the date is the author date

**Arrangement** (vendor screenshots: Mac 1.0.73 blog
<https://fork.dev/blog/posts/fork-1.0.73/>, Feb 2019; Windows 1.28 blog, Feb 2019;
both carousels, late 2020. User screenshots: Mac, Tracker #898, Sep 2022; Windows,
TrackerWin #2102, Dec 2023):

1. Two columns across the top: `AUTHOR` on the left, `COMMITTER` on the right. Each
   has an avatar, the name, the email in a lighter tone, and below them the full
   timestamp with seconds and zone (Mac: a localized long date with the time and
   `GMT+1`; Windows: `25 Nov 2020 01:11:30 +01:00`).
2. Under the author column, grey upper-case labels right-aligned against their values:
   `REFS` (branch and tag chips, drawn like the graph's labels), `SHA` (all 40 hex
   digits, selectable), `PARENTS` (one underlined 7-hex link per parent — two for a
   merge, three for a stash that holds untracked files; Tracker #898 screenshot).
3. A rule; the subject in a larger size, the body below in normal weight. Issue
   references render as links, with configurable bug-tracker patterns such as Jira
   or Redmine (Mac 1.0.75, 14 Mar 2019; Windows 1.33, 17 May 2019). A monospace option
   exists for descriptions (Windows 1.18, 28 Jun 2018; Mac 1.0.76 fixed it not
   applying to commit details).
4. A rule; the changed-file list (Finding 4).

**History.** Author and committer both shown since Mac 1.0.18 (22 Aug 2016); parent
links since Mac 1.0.22 (16 Sep 2016) and Windows 1.27 (8 Feb 2019). Before the
redesign (Mac 1.0.73, 1 Feb 2019; Windows 1.28, 22 Feb 2019) the tab used labelled
rows — `Author:`, `Date:`, `Commit hash:`, `Parents:` beside `Committer:`,
`Committer Date:`, then `Subject:` and `Changes:` (vendor screenshot, Tracker #234,
<https://github.com/fork-dev/Tracker/issues/234>, 1 Mar 2018). `REFS` arrived as a
references section in Mac 1.0.88 (13 Dec 2019) and as branches and tags in Windows
1.43 (13 Jan 2020), wrapping onto several lines on Windows 1.58 (15 Jan 2021). The
vendor declined adding an abbreviated SHA beside the full one, to keep the layout
clean (TrackerWin #994,
<https://github.com/fork-dev/TrackerWin/issues/994#issuecomment-717960876>,
28 Oct 2020), and abbreviations use a fixed length rather than `core.abbrev`
(<https://github.com/fork-dev/TrackerWin/issues/994#issuecomment-717083158>). Hovering a commit link pops up that commit's details
(Mac 2.28, 14 Apr 2023; Windows 1.83, 31 Mar 2023); clicking a parent selects it.

**What it deliberately lacks.**

- *Branches containing the commit.* The vendor calls this virtually impossible to
  present, since a commit usually belongs to many branches (TrackerWin #1251,
  <https://github.com/fork-dev/TrackerWin/issues/1251#issuecomment-886186744>,
  25 Jul 2021), and suggests a custom command running `git branch -a --contains`
  (TrackerWin #2010, 11 Sep 2023; Tracker #1416,
  <https://github.com/fork-dev/Tracker/issues/1416#issuecomment-874611875>,
  6 Jul 2021). `REFS` lists only refs that point at the commit.
- *Diffstat.* No files-changed / insertions / deletions summary (Tracker #455, open
  since 2018); the vendor confirms diffs don't show `--stat` (Tracker #2595,
  <https://github.com/fork-dev/Tracker/issues/2595#issuecomment-4552461702>,
  27 May 2026).
- *Signature state.* Not shown (Tracker #18, open since 2017).
- *Which date.* Details show the author date; the commit list and file history use
  the committer date (vendor, TrackerWin #2422,
  <https://github.com/fork-dev/TrackerWin/issues/2422#issuecomment-2633599743>,
  4–5 Feb 2025).
- *Git notes.* On Windows the vendor said notes added with `git notes add` appear in
  commit details (TrackerWin #1093,
  <https://github.com/fork-dev/TrackerWin/issues/1093#issuecomment-783296026>,
  22 Feb 2021); on Mac, that notes will likely never be added (Tracker #780,
  <https://github.com/fork-dev/Tracker/issues/780#issuecomment-2913186113>,
  27 May 2025). Current state per platform: OPEN.

**Committer column when the author committed.** In every screenshot showing
`COMMITTER`, the committer differs from the author (GitHub's web-flow identity, or a
maintainer). The two screenshots found of commits whose author presumably committed
them — a stash (Tracker #898) and an ordinary Windows commit (TrackerWin #2102) —
show `AUTHOR` only. INFERRED that the column is omitted when identical; OPEN.

**Evidence:** VENDOR SCREENSHOT; USER SCREENSHOT; RELEASE NOTES; VENDOR STATEMENT.
**Platform:** both.

### Finding 4 — The Commit tab lists the changed files and expands each file's diff inline under its row; it does not switch to Changes

- **The list.** Each row: change-type badge, file-type icon, full path (Mac from
  1.0.25, 7 Oct 2016; Windows from 1.21, 25 Sep 2018). Windows rows carry a
  disclosure triangle; Mac rows turn into hyperlinks under the pointer (vendor GIF,
  Tracker #194,
  <https://github.com/fork-dev/Tracker/issues/194#issuecomment-420674248>,
  12 Sep 2018). Renames show `old→new` in the row (user screenshot, Tracker #423,
  Oct 2018, Mac); the vendor favoured trimming long paths from the start because
  rename rows carry both paths and cutting the middle would make both unreadable
  (<https://github.com/fork-dev/Tracker/issues/423#issuecomment-426788426>,
  3 Oct 2018).
- **Clicking a file expands its diff directly under the row**, inside the same
  scrolling tab; clicking again collapses it. `Expand All`, right-aligned above the
  list, expands every file and turns into `Collapse All` (vendor GIF, above).
  Shipped Mac 1.0.70 (17 Sep 2018) after users proposed a GitX-style view; Windows
  1.21 followed (blog <https://fork.dev/blog/posts/forkwin-1.21/>). Mac 1.0.71
  (10 Oct 2018) added buttons here that reveal a file in the Changes tab; what they
  look like is OPEN.
- **No toolbar on these diffs.** The vendor says the Commit tab has no header to host
  the options (TrackerWin #1950,
  <https://github.com/fork-dev/TrackerWin/issues/1950#issuecomment-1610922475>,
  28 Jun 2023). Users report that the options set in the Changes tab —
  whitespace, entire file, side-by-side — govern the Commit tab's diffs (TrackerWin
  #792, May 2020; Tracker #1996, Oct 2023), and a 2025 Windows screenshot shows a
  Commit-tab diff rendered side-by-side (TrackerWin #2575,
  <https://github.com/user-attachments/assets/c80c7d47-88d1-4511-a1ad-de8d959e15a3>).
  A user reports that the optional `-+` marks, shown in Local Changes, are missing
  from diffs viewed in commit details (Tracker #2496, Nov 2025, open).
- **Scrolling.** Each inline diff scrolls horizontally on its own, with its
  horizontal scrollbar at its bottom edge; the vendor explained that these diff
  views are children of the larger Commit control and were designed before
  side-by-side made wide content common (TrackerWin #531,
  <https://github.com/fork-dev/TrackerWin/issues/531#issuecomment-550909225>,
  7 Nov 2019). A Mac 2.0 regression capped their width at 765 px (Tracker #1188,
  closed Dec 2020).
- **Collapsed by default, by choice.** An option to expand all by default has been requested
  since 2018 (Tracker #420, open, 25 reactions). The vendor refused to trade
  performance for it (26 Nov 2022,
  <https://github.com/fork-dev/Tracker/issues/420#issuecomment-1328013756>), then
  said he found no way to keep performance acceptable
  (<https://github.com/fork-dev/Tracker/issues/420#issuecomment-2705693934>,
  7 Mar 2025): text, image, binary, LFS and submodule diffs across dozens or
  hundreds of files must neither freeze the UI nor use gigabytes
  (<https://github.com/fork-dev/Tracker/issues/420#issuecomment-2705775263>).
- **Large content.** Expanding a huge file froze Windows until Fork stopped rendering
  large files by default (TrackerWin #119,
  <https://github.com/fork-dev/TrackerWin/issues/119#issuecomment-455140051>;
  Windows 1.27, 8 Feb 2019). Very long commit messages still freeze the Windows
  Commit tab (TrackerWin #2193, 2024, open).

**Evidence:** RELEASE NOTES; VENDOR STATEMENT; VENDOR SCREENSHOT (GIF); USER REPORT;
USER SCREENSHOT.
**Platform:** both.
**Implies:** Fork already has a continuous multi-file diff — in the Commit tab,
collapsed per file — and refuses both to make it expanded by default and to bring
it into Changes, on performance grounds (Finding 19).

### Finding 5 — The Changes tab: summary line on top, file list on the left, one file's diff on the right

- **Split.** File list left, diff right, with a draggable splitter (user descriptions:
  TrackerWin #48, Nov 2018; Tracker #1188, Nov 2020; vendor screenshots: Windows 1.38
  blog, Mac Tracker #2258
  <https://github.com/user-attachments/assets/d6cbeaf3-528c-4d1a-88d6-8b558a1dae9e>).
  The Windows splitter capped the list at about 600 px until 1.24 (vendor, TrackerWin
  #48, <https://github.com/fork-dev/TrackerWin/issues/48#issuecomment-441364224>,
  24–26 Nov 2018); Mac crashed when the split was dragged too narrow until 2.12
  (24 Sep 2021). Windows does not persist the split position (TrackerWin #1151,
  2021, open); list column widths are persisted (Mac 2.23; Windows 1.77, 1.78).
- **File-list toolbar** (Mac screenshots 2024–2026): a filter field, an eye button
  and a layout button.
  - Filter: Windows 1.35 (21 Jun 2019), Mac 2.9 (25 Jun 2021).
  - Eye: opens the side-by-side quick look, the same as `Space`; a user gives its
    tooltip as `Show diff side-by-side (spacebar)` (Tracker #1376, May 2021), and
    another opens the standalone diff with Space or the eye (TrackerWin #2799,
    Jun 2026).
  - Layout menu: `View as Tree`, `View as List`, `View as Combined List` (Mac, Local
    Changes, user screenshot Tracker #2616,
    <https://github.com/user-attachments/assets/3c20f83b-108f-4696-a14b-92067bfd2fd9>,
    Apr 2026; the same menu adds `Hide Untracked Files` / `Show Ignored Files`
    there). Tree since Mac 1.0.29 (4 Nov 2016). Combined List — a two-column
    `Name` | `Location` table (vendor screenshot, blog
    <https://fork.dev/blog/posts/forkwin-1.23/>) — since Windows 1.22 (26 Oct 2018)
    and Mac 2.9 (25 Jun 2021). Plain List shows full
    paths truncated at the start (user screenshot, Tracker #673, 2019). The vendor
    declined a per-repository layout choice (Tracker #2616, 27 Apr 2026).
- **Rows.** A coloured change-type badge (a yellow dotted badge for a modified file
  in 2019–2020 screenshots; letter badges such as a yellow `M` after the icons were
  redrawn in Mac 2.24 and Windows 1.79, late 2022), a file-type icon, the name, long
  names elided at the start (vendor screenshot, Tracker #2258, 2024). **No
  insertion/deletion counts** (Finding 3). Renames: a tooltip with old and new paths
  (Windows 1.58, 15 Jan 2021; vendor screenshots TrackerWin #204,
  <https://github.com/fork-dev/TrackerWin/issues/204>), then old and new names shown
  (Windows 1.81, 27 Jan 2023; Mac 2.26, 9 Feb 2023 — exactly where: OPEN).
  Submodules get their own icon (Mac 2.69, 10 Jul 2026; Windows 2.21, 16 Jul 2026).
- **Selection.** The first file is selected by default (Windows 1.17, 7 Jun 2018).
  Moving to another commit re-selects the first file rather than the same path
  (TrackerWin #854, 2020, open); tree folding is reset by several actions (TrackerWin
  #1805, Tracker #691, open). Each file's diff scroll position is remembered, keyed
  by commit and path (vendor, Tracker #326,
  <https://github.com/fork-dev/Tracker/issues/326#issuecomment-405735918>,
  17 Jul 2018; Mac 1.0.70; Windows 1.20).
- **Several files selected: only the first selected file's diff shows** (users:
  TrackerWin #786, May 2020, Local Changes; Tracker #261, Jul 2026, Mac flat list —
  the vendor replied in #261 without contradicting it). Whether the Changes tab of a
  commit behaves identically: OPEN.

**Evidence:** USER REPORT; USER SCREENSHOT; VENDOR SCREENSHOT; VENDOR STATEMENT;
RELEASE NOTES.
**Platform:** both.

### Finding 6 — File Tree: the repository at the selected commit, tree on the left and a file preview on the right, with no filter

- **Content.** Every tracked path at that commit as a tree, and the selected file's
  content on the right — the oldest detail tab (Mac 1.0.9, 10 Jun 2016). Line
  numbers in the preview (Windows 1.21, 25 Sep 2018; a Mac four-digit gutter limit
  fixed in 1.0.97, Tracker #1101); images and LFS content (Windows 1.82,
  24 Feb 2023), with a magnifier button to load an image (vendor, TrackerWin #1952,
  <https://github.com/fork-dev/TrackerWin/issues/1952#issuecomment-1616400407>,
  2 Jul 2023).
- **The vendor's model.** File Tree shows content *at* the selected commit, so it
  belongs with that commit's other details; a sidebar tree of the working copy was
  refused as confusing (TrackerWin #919,
  <https://github.com/fork-dev/TrackerWin/issues/919#issuecomment-678646162>,
  22 Aug 2020).
- **Actions.** History and Blame for files and folders (folder history since Mac
  1.0.96 / Windows 1.52, July 2020; fixed in the File Tree tab by Mac 2.25 /
  Windows 1.78); Save As; dragging a file out (user, Tracker #682, 2019); history
  for a hunk from the File Tree context menu (Mac 2.60, 28 Nov 2025);
  `Show in File Tree` from the Changes context menu (Windows 1.30, 22 Mar 2019).
  The selection survives switching commits (Mac 1.0.20; Windows 1.34).
- **No filter or search** — requested since 2018 (Tracker #428, open, 24 reactions;
  TrackerWin #472, #1247, #2093). The vendor cites the cost of building and showing
  a full tree on large repositories, with Chromium's 300,000 files as the example
  (TrackerWin #472,
  <https://github.com/fork-dev/TrackerWin/issues/472#issuecomment-538560661>,
  4 Oct 2019), and points to Quick Launch → History or Blame → file name (Tracker
  #1130 2020, #1555 2022, #1874 2023). Selecting a very large file freezes Windows
  (TrackerWin #753, open).

**Evidence:** RELEASE NOTES; VENDOR STATEMENT; USER REPORT.
**Platform:** both.

## Part B — Two commits, merge commits, stashes

### Finding 7 — Comparing: select exactly two commits (or two refs in the sidebar); Changes shows the tip-to-tip diff, both commits named, with a swap button

- **Gesture.** ⌘-click (Mac) or Ctrl-click (Windows) a second commit (Mac 1.0.37,
  6 Feb 2017; Windows 1.20, 13 Aug 2018; vendor FAQ,
  <https://github.com/fork-dev/Docs/blob/master/faq.md>; vendor, Tracker #33,
  <https://github.com/fork-dev/Tracker/issues/33#issuecomment-303315955>, 2017).
  Two branches or tags selected in the sidebar compare the same way (Mac 1.0.64,
  22 Feb 2018, ⌘-click on the sidebar; Windows 1.27, 8 Feb 2019; vendor, TrackerWin
  #1478, <https://github.com/fork-dev/TrackerWin/issues/1478#issuecomment-1062842523>,
  2022, and #1426,
  <https://github.com/fork-dev/TrackerWin/issues/1426#issuecomment-1847644335>,
  Dec 2023). An active branch filter can hide one side of the comparison (Tracker
  #2022, 2023).
- **Exactly two.** Only two commits can be compared (vendor, Tracker #120,
  <https://github.com/fork-dev/Tracker/issues/120#issuecomment-344834036>,
  Nov 2017). The vendor's position is that a pair defines any range, so more is
  unnecessary (Tracker #1321,
  <https://github.com/fork-dev/Tracker/issues/1321#issuecomment-804683128>,
  23 Mar 2021, where the user reports a message saying only a two-commit diff can be
  shown; TrackerWin #1551,
  <https://github.com/fork-dev/TrackerWin/issues/1551#issuecomment-1146317666>,
  3 Jun 2022).
- **What is shown.** The Changes tab with the file list and diff of the difference.
  On Windows, `Commit` and `File Tree` are disabled while two commits are selected
  (user, TrackerWin #830, 2020). The header names both commits, one per line and
  without branch or tag labels (user, Tracker #1585, 2022), with a swap-direction
  icon at its right (Mac 1.0.51,
  7 Jul 2017, the ability to swap diff order; user screenshot, Tracker #758, 2019).
  Windows orders the pair itself so the diff runs older to newer: the vendor
  proposed sorting the two topologically, falling back to commit date when they sit
  on separate branches (TrackerWin #732,
  <https://github.com/fork-dev/TrackerWin/issues/732#issuecomment-614159060>,
  15 Apr 2020), shipped as the older-to-newer default in Windows 1.58
  (15 Jan 2021), and colours the two SHAs (1.60 / 1.61, 2021). Mac ordering: OPEN.
- **Semantics: two-dot, tip against tip** — not the merge-base comparison a pull
  request shows (user, Tracker #2601, Apr 2026, open).
- **Related comparisons.** `Compare to Local Changes` in a commit's context menu
  diffs the working tree against it (Mac 2.11, 27 Aug 2021; Windows 1.65,
  13 Aug 2021; vendor screenshot TrackerWin #1897, 2023; vendor, TrackerWin #722,
  <https://github.com/fork-dev/TrackerWin/issues/722#issuecomment-2500759178>,
  26 Nov 2024). A stash can be compared to local changes (Mac 2.12, 24 Sep 2021).
  An external diff of the pair can be opened from Mac's History window (2.34,
  22 Sep 2023).

**Evidence:** RELEASE NOTES; VENDOR STATEMENT; VENDOR DOCS; USER REPORT; USER
SCREENSHOT.
**Platform:** both; automatic ordering documented for Windows.

### Finding 8 — Merge commits: always the first-parent diff, from `git show --diff-merges=1`; no parent picker and no combined diff

- **The command.** A Windows user captured the command Fork runs to display a commit
  (TrackerWin #2030, <https://github.com/fork-dev/TrackerWin/issues/2030>,
  26 Sep 2023): one `git show` with `--patch --raw --diff-merges=1 --find-renames
  --src-prefix=forkSrcPrefix/ --dst-prefix=forkDstPrefix/ --full-index
  --submodule=short --no-ext-diff --no-color --no-show-signature --unified=3
  --ignore-all-space`, and a `--pretty=format:` string carrying hash, author
  name/email/time, committer name/email/time, parents and the raw body, fields
  separated by `±.` (the separator is confirmed by the vendor, TrackerWin #1951,
  1 Jul 2023). `--ignore-all-space` reflects that user's toggle.
- **`--diff-merges=1` is git's first-parent diff.** From Dec 2022 — just after Mac
  2.24 (25 Nov 2022), so INFERRED to be when Fork began passing it — users whose Fork
  pointed at an older system git (2.27.0 and 2.30.1 in the reports) saw
  `unknown value for --diff-merges: 1` or an unrecognized-argument error in the
  details pane when clicking a commit or a stash; the vendor's answer each time was
  to update git or switch Fork back to its internal git (Tracker #1757,
  <https://github.com/fork-dev/Tracker/issues/1757#issuecomment-1339387017>,
  6 Dec 2022; #1762, 10 Dec 2022; #1778, 2 Jan 2023; #1805; #1856; #1959).
- **Effect as users describe it.** A merge's Changes tab shows everything the merged
  branch brought in (Tracker #2514,
  <https://github.com/fork-dev/Tracker/issues/2514#issuecomment-3691337094>,
  25 Dec 2025), which is the premise of the open request for `--cc` /
  `--remerge-diff` (TrackerWin #2774, 30 Apr 2026, no vendor reply).
- **No parent picker.** Searches for second parent, combined diff, remerge and
  parent diffs found only that request and navigation proposals (Tracker #443, keys
  to jump to parents, open since 2018). OPEN as an absence.
- **File History and Blame skip merges** (TrackerWin #1261, open, 15 reactions); the
  vendor attributes it to how git itself works (Tracker #1580,
  <https://github.com/fork-dev/Tracker/issues/1580#issuecomment-1075400120>,
  22 Mar 2022).
- **Before Dec 2022** the merge-diff mechanism is OPEN.

**Evidence:** USER REPORT (captured command); VENDOR STATEMENT; TRACKER STATE.
**Platform:** both (command captured on Windows; `--diff-merges=1` errors reported on
Mac).

### Finding 9 — Stashes: shown as commits; the diff is against the stash's first parent, and untracked files are left out

- **Where.** Stashes appear inline in the commit list (hide option Mac 1.0.95,
  18 Jun 2020; Windows 1.51, 2 Jul 2020) and in the sidebar.
- **Commit tab.** The ordinary header — `AUTHOR`, `SHA`, `PARENTS` with two or three
  links — and the stash message (user screenshot, Tracker #898,
  <https://user-images.githubusercontent.com/803905/190285681-cd56622b-99f9-472e-81af-db56a31969f8.png>,
  Sep 2022). No vendor screenshot of a selected stash was found.
- **Changes tab.** The stash commit's diff against its first parent, by the same
  `--diff-merges=1` path (Tracker #1778's error names commits and stashes alike).
- **Untracked files are not listed** (Tracker #898, open since Feb 2020, 8 reactions;
  TrackerWin #2624, 2025). Vendor: Fork shows the stash object as it is, and
  untracked files hang off a separate third-parent commit rather than living in the
  stash (TrackerWin #2624,
  <https://github.com/fork-dev/TrackerWin/issues/2624#issuecomment-4721002124>,
  16 Jun 2026); won't be fixed in the near future
  (<https://github.com/fork-dev/Tracker/issues/898#issuecomment-4935055206>,
  10 Jul 2026). A user's workaround is to click the third parent link.
- **Reselection bugs.** Re-clicking a stash did not refresh the details on Mac; the
  vendor reports it fixed in 2.56 (Tracker #1610,
  <https://github.com/fork-dev/Tracker/issues/1610#issuecomment-3240057961>,
  31 Aug 2025; the issue itself is still open). Similar Windows reports remain open
  (TrackerWin #1281, #1349, #1871).

**Evidence:** USER SCREENSHOT; USER REPORT; VENDOR STATEMENT; RELEASE NOTES.
**Platform:** both.

## Part C — The diff view

### Finding 10 — The diff header: previous/next change on the left, the path in the middle with the file name emphasized, seven icon toggles on the right; no statistics and no file actions

Layout (vendor screenshots: Mac, Tracker #2624,
<https://github.com/user-attachments/assets/cc41f0f0-e18d-44ad-aa19-b22210c518e0>,
May 2026; Windows, TrackerWin #2800, Jun 2026. User screenshots 2023–2026):

- **Left:** up and down chevrons jumping to the previous / next change (Mac 2.33,
  1 Sep 2023; Windows 1.89, 9 Sep 2023).
- **Centre:** a file-type icon and the path, the directory in grey and the file name
  in the normal text colour (Mac 2.14, 26 Nov 2021; Windows 1.70, 28 Jan 2022). The
  file name above the editor dates from Windows 1.17 (7 Jun 2018; blog
  <https://fork.dev/blog/posts/forkwin-1.17/>). Long paths are elided in the middle
  (vendor screenshot, Tracker #2258). Renames show both paths in a tooltip on the
  header path (vendor screenshot, TrackerWin #204, 15 Jan 2021).
- **Right, in order:** ignore whitespace (a `⎵` glyph), show invisible characters
  (`¶`), word wrap, decrease visible lines (`−` over lines), increase visible lines
  (`+` over lines), show entire file (`↕` over lines), side-by-side (a split
  rectangle). Active toggles take the accent colour (vendor, Tracker #2623,
  <https://github.com/fork-dev/Tracker/issues/2623#issuecomment-4371593947>,
  4 May 2026). Tooltips seen: `Ignore whitespaces`, `Word wrap`,
  `Increase number of visible lines` (vendor GIF, Mac 1.0.69 blog
  <https://fork.dev/blog/posts/fork-1.0.69/>), `Decrease number of visible lines`
  (named in Tracker #911 and TrackerWin #1046) and `Show entire file` (vendor
  screenshot, TrackerWin #792, 2020).
- **History of the bar.** The options first lived in the diff's context menu
  (context lines Mac 1.0.32, 2 Dec 2016; word wrap 1.0.48, 8 Jun 2017; whitespace
  1.0.65, 15 Mar 2018); the icon bar arrived on Windows 1.18 (28 Jun 2018; blog
  <https://fork.dev/blog/posts/forkwin-1.18/> lists whitespace, word wrap,
  decrease/increase context and entire file) and Mac 1.0.68 (11 Jul 2018); `¶` on
  Windows 1.28 (22 Feb 2019) and Mac 1.0.77 (23 Apr 2019). The 2020 Mac carousel
  shows six icons in the Local Changes header and a seventh, the side-by-side
  toggle, in the quick look — consistent with Mac adding side-by-side to Local
  Changes only in 2.21. In Aug 2023 both apps replaced the two context
  buttons with one dropdown (Mac 2.32, 11 Aug 2023; Windows 1.88, 18 Aug 2023),
  visible in an Oct 2023 Mac screenshot as an arrow-with-chevron button (Tracker
  #1996). Screenshots from Mar 2025 (Windows, TrackerWin #2460), May 2026 (Mac,
  vendor) and Jun 2026 (Windows, vendor) show separate `−` / `+` buttons again. When
  and why the dropdown went: OPEN — no release note found.
- **Absent from the header:** added/removed counts, a rename line, similarity, mode
  change, and buttons to open, blame, show history or run an external diff. Those
  live in the file's context menu — External Diff (Mac 1.0.62, 15 Jan 2018, with ⌘D
  from 1.0.63; Windows 1.20, Ctrl+D; one file at a time, vendor TrackerWin #113), History,
  Blame, Show in File Tree, resetting the file to a revision's state (Windows 1.20),
  Save As —
  and in the diff text's context menu: Copy as Patch (Mac 2.29, 12 May 2023; Windows
  1.85, 26 May 2023), reveal the line in an editor (Mac 2.2, 10 Dec 2020; Windows
  2.5, 7 Feb 2025), file history of the selected lines (Mac 2.59, 7 Nov 2025;
  Windows 2.14, 21 Nov 2025).
- In the commit Changes tab, an open-in-separate-window icon sits at the far right
  of the tab strip (vendor screenshot, Tracker #2258).

**Evidence:** VENDOR SCREENSHOT; RELEASE NOTES; VENDOR DOCS; USER SCREENSHOT; VENDOR
STATEMENT.
**Platform:** both — the icon set is identical in 2025–2026 screenshots of each.

### Finding 11 — Unified by default; side-by-side is a header toggle in both views, plus a Space-bar quick look

- **Default unified.** The vendor describes unified as the default view when
  side-by-side is off (Tracker #2624,
  <https://github.com/fork-dev/Tracker/issues/2624#issuecomment-4390031027>,
  6 May 2026).
- **Where the toggle exists.** Windows 1.33 (17 May 2019), INFERRED to cover both
  the commit Changes view and Local Changes from the start: within weeks the vendor
  referred to the side-by-side diff's new floating-buttons control, the buttons that
  stage and discard (TrackerWin
  #45, <https://github.com/fork-dev/TrackerWin/issues/45#issuecomment-498604426>,
  4 Jun 2019), users asked for one per-view setting (TrackerWin #39, May 2019;
  #360, Jul 2019), and a Mac user cites the Windows staging area having it (Tracker
  #1526, Dec 2021). Mac: inline in
  commit changes from 1.0.88 (13 Dec 2019), in Local Changes from 2.21
  (19 Aug 2022). Before 1.0.88 Mac's only side-by-side was the quick look. The vendor
  held Local Changes back because a chunk split across two text views made staging
  confusing and broke contiguous mouse selection — side-by-side suits review, not
  editing (Tracker #1526,
  <https://github.com/fork-dev/Tracker/issues/1526#issuecomment-990858614>,
  10 Dec 2021) — and shipped once it behaved as intended
  (<https://github.com/fork-dev/Tracker/issues/1526#issuecomment-1217811934>,
  17 Aug 2022). His initial doubt on Windows was the same: what line staging should
  do in side-by-side (TrackerWin #39,
  <https://github.com/fork-dev/TrackerWin/issues/39#issuecomment-455327645>,
  17 Jan 2019).
- **Scope of the setting.** One setting shared across contexts: a 2019 Windows request
  to make it depend on the view is open (TrackerWin #360), and a 2026 Windows user
  notes that enabling it in another repository's history view disables word wrap in
  Local Changes (TrackerWin #2019). Mac scope: OPEN. The quick look keeps its own
  side-by-side state (vendor, TrackerWin #360,
  <https://github.com/fork-dev/TrackerWin/issues/360#issuecomment-511340344>,
  15 Jul 2019); the number of visible lines and entire-file mode are shared with it
  (vendor, Tracker #1603,
  <https://github.com/fork-dev/Tracker/issues/1603#issuecomment-1128473534>,
  17 May 2022).
- **Quick look.** `Space` on a file (or the eye button) opens a large floating
  side-by-side diff: Mac 1.0.50 (30 Jun 2017), which the vendor likened to Finder's
  Quick Look (Tracker #54,
  <https://github.com/fork-dev/Tracker/issues/54#issuecomment-312284379>);
  Windows 1.36 (12 Jul 2019), where its footer reads `Space bar: close, Up/Down: move
  file selection` (carousel
  <https://git-fork.com/images/carousel/carousel_commitviewWin2.jpg>). On Mac, focus
  stays in the main window so the arrow keys keep moving the file selection (vendor,
  Tracker #101,
  <https://github.com/fork-dev/Tracker/issues/101#issuecomment-331726479>,
  24 Sep 2017). The vendor's rationale: keep the small split for overview and press
  Space when changes get complicated, which doubles as a distraction-free review
  mode (TrackerWin #360). It closes on focus loss on Windows (TrackerWin #1184, open);
  typing a space in its find box closed it (fixed in Windows 1.46 per the vendor,
  TrackerWin #589, 28 Feb 2020; still open on Mac, Tracker #1946).
- **Geometry.** Two equal panes; the vendor says they must be equal and will not be
  resizable (TrackerWin #2800,
  <https://github.com/fork-dev/TrackerWin/issues/2800#issuecomment-4679210536>,
  11 Jun 2026). Each pane has one line-number gutter; lines missing on one side are
  padded with grey filler rows; the hunk header repeats at the top of each pane
  (Mac carousel 2020; Windows 1.38 blog 2019; Tracker #1985 2023; TrackerWin #2460
  2025). Vertical scrolling is synchronised (INFERRED from row alignment in all
  screenshots).
- **Two controls.** The panes are separate text controls, which is why a line
  selection cannot span both (vendor, Tracker #1985,
  <https://github.com/fork-dev/Tracker/issues/1985#issuecomment-1762145791>,
  13 Oct 2023) and why each pane opens its own find bar, misaligning rows while it is
  open (TrackerWin #865 2020, #2723 2026).
- **No word wrap.** Wrapped chunks would have unpredictable heights that cannot stay
  aligned (vendor, Mac, Tracker #424,
  <https://github.com/fork-dev/Tracker/issues/424#issuecomment-426983686>,
  4 Oct 2018); alignment uses inserted newlines (vendor, Windows, TrackerWin #733,
  <https://github.com/fork-dev/TrackerWin/issues/733#issuecomment-611545022>,
  9 Apr 2020); the Windows button is disabled (vendor, TrackerWin #2019,
  <https://github.com/fork-dev/TrackerWin/issues/2019#issuecomment-1719602956>,
  14 Sep 2023). Mac reports conflict: unavailable in Local Changes side-by-side
  (Tracker #1699, 2022); in 2021 toggling wrap in the side-by-side popup wrapped the
  main window's diff instead (Tracker #1338); but a 2025 report describes wrap on in
  the quick look and the separate commit window, at the wrong width (Tracker #2382).
  Current Mac state: OPEN.
- **Intra-line highlighting** is present in side-by-side (Windows 1.36; Mac carousel
  2020).
- **Declined alternatives.** A vertically stacked split (old block above new block)
  is unlikely to be added (Tracker #2624, 7 Jun 2026); another proposed
  side-by-side look the vendor did not know how to implement (Tracker #1809,
  15 Feb 2023).

**Evidence:** RELEASE NOTES; VENDOR STATEMENT; VENDOR SCREENSHOT; USER REPORT.
**Platform:** both.

### Finding 12 — Gutters: two number columns in unified, one per pane in side-by-side, an optional `-+` column; a change minimap on the scrollbar

- **Unified.** Old and new line numbers in two right-aligned columns, then a thin
  separator; removed lines carry only the old number, added lines only the new,
  context lines both (all screenshots, 2016–2026). Line numbers since Mac 1.0.15
  (29 Jul 2016). The gutter widens with digit count (MEASURED, Finding 24). The
  numbers use a different font from the diff text (Tracker #366, open) and were made
  higher-contrast in dark mode (Mac 2.12). The row tint starts at the text column;
  the gutter keeps the background (MEASURED).
- **Side-by-side.** One number column per pane.
- **`-+` marks.** An optional narrow column of `-` / `+` between the numbers and the
  text, added for colour-blind users (Mac 2.10, 23 Jul 2021; Windows 1.64,
  9 Jul 2021; vendor screenshot of the preference, Tracker #307,
  <https://github.com/fork-dev/Tracker/issues/307#issuecomment-886184239>). Off
  unless enabled (INFERRED from its introduction as an opt-in option and from a user
  describing turning it on, Tracker #2496). It sits in Preferences → General →
  `Diff View`, beside the font and `Disable syntax highlighting` (user screenshot,
  Tracker #2496, Nov 2025).
- **Minimap.** The vertical scrollbar carries red and green marks for changes (Mac
  1.0.55, 5 Sep 2017, commit changes; 1.0.58, staging view; Windows 1.40,
  3 Oct 2019), hidden when the document fits (Mac 2.61, 9 Jan 2026; Windows 2.16,
  20 Feb 2026). The Windows scrollbar was widened after complaints that the
  minimap hid it (TrackerWin #471, 2019).

**Evidence:** VENDOR SCREENSHOT; RELEASE NOTES; USER REPORT; MEASURED.
**Platform:** both.

### Finding 13 — Hunk headers are git's raw `@@` lines, drawn as an ordinary muted row; nothing is attached to them

- **Text.** The `@@ -a,b +c,d @@` line exactly as git emits it, including git's
  function-context suffix; the vendor points to git as the source of that signature
  (TrackerWin #1216,
  <https://github.com/fork-dev/TrackerWin/issues/1216#issuecomment-897461238>,
  12 Aug 2021). Not prettified. The vendor's principle is to stay as close as
  possible to git's unified format (TrackerWin #264,
  <https://github.com/fork-dev/TrackerWin/issues/264#issuecomment-1744965306>,
  3 Oct 2023).
- **Style.** Grey text on the diff background — no band, no border — at the same
  row height as a code line, with no line numbers (MEASURED, Finding 24). Greying
  git's header lines dates from the third pre-release build (GitClient 1.0.2,
  19 Apr 2016).
- **`\ No newline at end of file`** appears as its own text row, on the side it
  annotates; the vendor keeps it explicit so that removing a final newline stays
  visible (TrackerWin #94,
  <https://github.com/fork-dev/TrackerWin/issues/94#issuecomment-475168445>,
  21 Mar 2019; #264,
  <https://github.com/fork-dev/TrackerWin/issues/264#issuecomment-500123168>,
  8 Jun 2019). Early parsing bugs were fixed in Mac 1.0.58 (Tracker #109).
- **Buttons: none on the row.** In Local Changes, hovering anywhere in a chunk shows
  floating `Stage` / `Discard` buttons at the chunk's top right, over the right end
  of its header row (Finding 23). In commit view there are no chunk actions:
  reverting a hunk of a past commit was implemented and deliberately not released,
  because applying fragments of old diffs readily conflicts and aborting such a
  conflict can discard uncommitted work (vendor, Tracker #333,
  <https://github.com/fork-dev/Tracker/issues/333#issuecomment-2515203405>,
  3 Dec 2024; TrackerWin #148,
  <https://github.com/fork-dev/TrackerWin/issues/148#issuecomment-2385192327>,
  1 Oct 2024).

**Evidence:** VENDOR STATEMENT; VENDOR SCREENSHOT; RELEASE NOTES; MEASURED.
**Platform:** both.

### Finding 14 — Context: one global number of visible lines, default 3, changed a line at a time; no per-gap expansion; entire-file mode is a separate toggle

- **Default 3.** The captured command passes `--unified=3` (TrackerWin #2030), and
  users describe pressing decrease below 1 snapping back to 3 as the default
  (Tracker #911, 17 Feb 2020; TrackerWin #1046, 27 Nov 2020, and a reply there).
- **Global.** The vendor calls the number of visible lines a global setting (Tracker
  #1603, 17 May 2022) — not per file, not per view.
- **Step: one line per click.** INFERRED from the vendor's Mac 1.0.69 GIF: two
  hunks, `@@ -89,3` and `@@ -95,3`, merge into `@@ -87,13` as context grows, and the
  next frame shows `@@ -86,15` — one more line on each side.
- **Bounds.** Minimum 1. Pressing decrease at 1 snapped back to 3 — fixed on Mac by
  1.0.90 per the reporter (Tracker #911, 29 Feb 2020); the Windows report, which
  also notes that increasing past any visible effect keeps counting silently, was
  closed in Nov 2023 (TrackerWin #1046). No maximum documented: OPEN.
- **Controls over time.** Context menu (Mac 1.0.32, 2 Dec 2016) → header buttons
  (2018) → a dropdown (Aug 2023) → buttons again in 2025–2026 screenshots
  (Finding 10).
- **Collapsed regions have no affordance.** Unchanged lines beyond the context are
  simply absent; the next hunk starts at its own `@@` row (all screenshots). There is
  no "show N more lines" per gap: a 2023 request for temporary GitLab-style
  expansion is open (TrackerWin #1995), and the vendor's answer to a folding request
  was to change the number of visible lines outside entire-file mode (TrackerWin
  #545,
  <https://github.com/fork-dev/TrackerWin/issues/545#issuecomment-553360992>,
  13 Nov 2019).
- **Entire file.** A header toggle (Mac 1.0.44, 20 Apr 2017, revision diff; File
  History fixed in 1.0.45; Windows 1.18). The vendor says Fork tells git the whole
  file is one chunk (TrackerWin #250,
  <https://github.com/fork-dev/TrackerWin/issues/250#issuecomment-487521796>,
  29 Apr 2019) — INFERRED to mean a very large `--unified`. The setting is shared by
  Local Changes and All Commits (request for separate settings open, Tracker #953;
  user, TrackerWin #792). Windows keeps the scroll position when toggling
  (2.4, 17 Jan 2025). The previous/next change controls are how one moves between
  changes in this mode (Finding 18); a user asks that the target change be centred
  rather than placed near the top (TrackerWin #2818, open). Staging interaction:
  Finding 23.
- **Side effect on syntax colouring.** The highlighter sees only the visible text, so
  a multi-line string or comment whose start lies outside the context is mis-coloured
  until entire-file is on (TrackerWin #2627, 2025; Tracker #1730, where the vendor
  describes Fork's highlighting as simple, without a language tokenizer,
  <https://github.com/fork-dev/Tracker/issues/1730#issuecomment-1296786657>,
  31 Oct 2022).

**Evidence:** USER REPORT; VENDOR STATEMENT; VENDOR SCREENSHOT (GIF); RELEASE NOTES;
INFERRED.
**Platform:** both.

### Finding 15 — Ignore whitespace re-runs git with `--ignore-all-space`; it hides changes without saying so

- **Arrival.** Revision diff view Mac 1.0.44 (20 Apr 2017); commit-view context menu
  1.0.65 (15 Mar 2018, vendor
  <https://github.com/fork-dev/Tracker/issues/27#issuecomment-373632498>); header
  bar Windows 1.18 / Mac 1.0.68. For the staging area the vendor first held back,
  because a file can stay in the unstaged list over whitespace changes the user
  cannot see after staging every visible chunk (Tracker #27,
  <https://github.com/fork-dev/Tracker/issues/27#issuecomment-302323067>,
  18 May 2017); it is in the Local Changes header in the 2020 carousel screenshots.
- **The flag.** Mac used `--ignore-space-change` (vendor, Tracker #738,
  <https://github.com/fork-dev/Tracker/issues/738#issuecomment-530477789>,
  11 Sep 2019) — the stated reason being that full ignore makes partial staging
  error-prone because git cannot locate many chunks — and moved to
  `--ignore-all-space` by Jul 2021 (vendor, Tracker #1376,
  <https://github.com/fork-dev/Tracker/issues/1376#issuecomment-886386162>,
  26 Jul 2021). Windows uses `--ignore-all-space` (vendor, TrackerWin #935,
  <https://github.com/fork-dev/TrackerWin/issues/935#issuecomment-688351309>,
  7 Sep 2020; #1834, 7 Mar 2023; the captured command, 2023).
- **What users hit.** A whitespace-only file shows an empty diff with no explanation
  (Tracker #1285, open); a regression hid the whole header — and so the toggle — for
  such files (TrackerWin #1700, fixed 1.79.1, 14 Nov 2022); in side-by-side the left
  pane shows the right side's indentation, because the display is one git patch
  (vendor, TrackerWin #1834, 8 Mar 2023); significant whitespace inside a line
  disappears too (TrackerWin #1974, 2023).
- **Showing whitespace instead.** The separate `¶` toggle renders spaces, tabs and
  newlines as symbols (Windows 1.28, 22 Feb 2019; Mac 1.0.77, 23 Apr 2019 — vendor
  lists the three kinds, Tracker #447,
  <https://github.com/fork-dev/Tracker/issues/447#issuecomment-485786989>).

**Evidence:** RELEASE NOTES; VENDOR STATEMENT; USER REPORT.
**Platform:** both.

### Finding 16 — Intra-line highlighting is always on, a stronger tint of the line's colour; the algorithm has been reworked repeatedly; no off switch was found

- **History.** Exact differences between rows highlighted from Mac 1.0.4
  (8 May 2016); made less aggressive in 1.0.36 (27 Jan 2017); improved in 1.0.77
  (23 Apr 2019); added to side-by-side on Windows 1.36 (12 Jul 2019); token-based on
  Windows 2.20 (5 Jun 2026). The vendor tried a Myers-based word highlight in 2017
  and found too many false positives, judged GitX's algorithm also wrong too often,
  and kept highlighting basic (Tracker #100,
  <https://github.com/fork-dev/Tracker/issues/100#issuecomment-331726803>,
  24 Sep 2017;
  <https://github.com/fork-dev/Tracker/issues/100#issuecomment-358312754>,
  17 Jan 2018). In Sep 2018 he called the current mode a range diff and planned a
  switchable word-diff mode
  (<https://github.com/fork-dev/Tracker/issues/100#issuecomment-423753745>), which
  has not shipped (issue open). A user then described it as highlighting from the
  first to the last change on a line (Tracker #424, Oct 2018). In both 2020 Mac
  carousel screenshots each changed line shows a single highlighted range.
- **Vendor stance.** A precise LCS or Myers highlight reads worse than a coarser
  one; readable and precise are different goals (TrackerWin #226,
  <https://github.com/fork-dev/TrackerWin/issues/226#issuecomment-481165682>,
  9 Apr 2019). Tokenizing words across plain text, XML, JSON and C-like code was the
  hard part (Tracker #100,
  <https://github.com/fork-dev/Tracker/issues/100#issuecomment-426065523>,
  1 Oct 2018). Asked to emulate a richer renderer, the
  vendor argued its emphases looked arbitrary and called Fork's own diff simple but
  accurate (Tracker #2597,
  <https://github.com/fork-dev/Tracker/issues/2597#issuecomment-4232638244>,
  12 Apr 2026).
- **Implementation traces.** A Windows crash stack shows
  `Fork.Git.Diff.Presentation.PatchHighlighter.TokenLCS`, called from
  `AddExtraHighlighting`, running out of memory on a large diff (TrackerWin #2798,
  Jul 2026 comment). On Mac the vendor said improved line highlighting had become
  O(n²) and fixed it for minified files in a 2.67.x build (Tracker #2651,
  <https://github.com/fork-dev/Tracker/issues/2651#issuecomment-4706419073>,
  15 Jun 2026).
- **Emphasis rule.** Where a removed and an added line pair up as a modification, the
  difference is emphasized — the vendor's explanation to a user who saw an added line
  pale in one place and strong in another (Tracker #2288,
  <https://github.com/fork-dev/Tracker/issues/2288#issuecomment-2624652903>,
  30 Jan 2025).
- **Off switch: none found.** The diff preferences in screenshots offer only the diff
  font, `Show -+ marks` and `Disable syntax highlighting` (Mac, user screenshot
  Tracker #2496,
  <https://github.com/user-attachments/assets/349bfa93-aadb-41e5-a51a-242bfad4e2bb>,
  Nov 2025; Windows, vendor TrackerWin #2339,
  <https://github.com/fork-dev/TrackerWin/issues/2339#issuecomment-2412997853>,
  Oct 2024). OPEN as an absence.
- Colours: Findings 25 and 26.

**Evidence:** RELEASE NOTES; VENDOR STATEMENT; USER REPORT (crash stack); VENDOR
SCREENSHOT.
**Platform:** both.

### Finding 17 — Syntax highlighting, word wrap, tab width, fonts, search

- **Syntax highlighting.** Mac 1.0.59 (17 Nov 2017); Windows 2.1 (27 Sep 2024). A
  preference turns it off (Mac: the option had no effect in 2.46.1 and the vendor
  issued a 2.46.2 hotfix, Tracker #2206, 23 Sep 2024;
  Windows: in Preferences, vendor TrackerWin #2339). Languages arrive one by one in
  release notes (e.g. Lua and Rust Mac 2.5, SQL Mac 2.48). The highlighter is simple
  and sees only visible text (Finding 14); users report mis-colouring, for example
  after a regex literal (Tracker #1977, 2023, open).
- **Word wrap.** A header toggle (context menu since Mac 1.0.48, 8 Jun 2017; the
  vendor points to both places, Tracker #424,
  <https://github.com/fork-dev/Tracker/issues/424#issuecomment-426959355>); not
  available in side-by-side (Finding 11). With wrap off, Mac still breaks extremely
  long lines, because `NSTextView` hangs on them and the vendor imposed a maximum
  (Tracker #433,
  <https://github.com/fork-dev/Tracker/issues/433#issuecomment-429608547>,
  14 Oct 2018; #773, 6 Dec 2019). Wrap-width bugs remain (Tracker #2383, 2025,
  open).
- **Tab width.** An option on Mac 1.0.70 (17 Sep 2018) and Windows 1.80
  (9 Dec 2022); a user's wording suggests it is set per repository (Tracker #1568,
  2022) — location OPEN. An `NSTextView` default that broke a line after twelve tabs
  (vendor, Tracker #189, 12 Jan 2018) was fixed in Mac 1.0.62 (15 Jan 2018).
- **Fonts.** Mac: the diff font is chosen in Preferences → General → Diff View (since
  1.0.18, 22 Aug 2016); two screenshots four years apart show `Menlo-Regular - 11.0`
  (vendor 2021, Tracker #307; user 2025, Tracker #2496), INFERRED to be the default.
  The rest of the Mac UI does not scale (vendor, Tracker #1955, 2023–2024).
  Windows: diff font size only (1.24, 26 Nov 2018; applied after restart per a user,
  TrackerWin #480); no face choice, which the vendor ties to ligature support missing
  from WPF on .NET Framework 4.7 (TrackerWin #480, 10 Sep 2021 and 9 Apr 2026);
  whole-UI zoom Ctrl+= / Ctrl+- (1.38, 16 Aug 2019).
- **Search in a diff.** ⌘F / Ctrl+F opens a find bar for the current file only. Mac
  1.0.32 (2 Dec 2016) — `NSTextView`'s inline search, which the vendor found
  non-customizable, so a previous file's highlights lingered after switching files
  (Tracker #1852,
  <https://github.com/fork-dev/Tracker/issues/1852#issuecomment-1486302499>,
  28 Mar 2023) until 2.69 (10 Jul 2026), which also re-applies the search when
  switching files. Windows 1.32 (26 Apr 2019;
  vendor, TrackerWin #219,
  <https://github.com/fork-dev/TrackerWin/issues/219#issuecomment-487005587>): a bar
  above the text, current match in a strong highlight, other matches yellow (vendor
  screenshot, 1.38 blog). Search across every file of a change: not offered (the
  #219 reporter's follow-up).

**Evidence:** RELEASE NOTES; VENDOR STATEMENT; VENDOR SCREENSHOT; USER REPORT.
**Platform:** as noted per item.

### Finding 18 — Keyboard navigation between files, changes and panes

From the vendor-hosted shortcut lists
(<https://github.com/fork-dev/Docs/blob/master/keyboard-shortcuts-mac.md>,
<https://github.com/fork-dev/Docs/blob/master/keyboard-shortcuts-windows.md>),
release notes and vendor comments:

- **Views.** ⌘1 / Ctrl+1 Local Changes (pressed again, focus the commit message);
  ⌘2 / Ctrl+2 All Commits (pressed again, jump to HEAD); ⌘0 / Ctrl+0 reveal HEAD.
- **Detail tabs.** ⌘⌥1 / 2 / 3 on Mac (Finding 2); Windows: OPEN.
- **Files.** Arrow keys in the file list, including while the quick look is open;
  `Space` quick look; ⌘D / Ctrl+D external diff; Tab / Shift-Tab cycle unstaged list →
  staged list → subject → description in Local Changes (vendor, Tracker #309,
  30 Nov 2021) and list ↔ details (Mac 2.67, Windows 2.20).
- **Changes within a file.** The header chevrons (Sep 2023), and ⌘↑ / ⌘↓ (Mac 2.67,
  15 May 2026; vendor, Tracker #1850,
  <https://github.com/fork-dev/Tracker/issues/1850#issuecomment-4460932142>) or
  Ctrl+↑ / Ctrl+↓ (Windows 2.20, 5 Jun 2026). In the Windows Local Changes quick look
  the shortcut switched files instead (TrackerWin #2799, open); in a commit's Changes
  tab a user reports the next-change button skipping to the second change, or doing
  nothing when there is only one (TrackerWin #2393, Dec 2024, open).
- **Commits.** Parent/child navigation keys proposed by the vendor in 2018 (Tracker
  #443) remain unimplemented; ← / → collapse and expand merges in the graph.
- Staging keys: Finding 23.

**Evidence:** VENDOR DOCS; RELEASE NOTES; VENDOR STATEMENT; USER REPORT.
**Platform:** both.

## Part D — One file at a time, big commits, big files, non-text changes

### Finding 19 — Changes shows one file; Commit shows many, collapsed; the vendor refuses a continuous multi-file diff in Changes

- **Changes and Local Changes show exactly one file's diff** (Finding 5). A stacked
  multi-file diff has been requested since Apr 2018 (Tracker #261, open,
  12 reactions; its duplicate Tracker #912, 2020; on Windows, TrackerWin #786, open).
  Vendor, 2019: very
  unlikely soon, for performance, memory, reworking the scrollbar minimap and code
  complexity (<https://github.com/fork-dev/Tracker/issues/261#issuecomment-546602774>,
  26 Oct 2019). Vendor, 2026: Windows lacks a proper virtualized list with
  variable-height items; the selection model is unclear (users would need to select
  everything, and new off-screen files would be missed)
  (<https://github.com/fork-dev/Tracker/issues/261#issuecomment-4969799416>); and a
  flat line-based document would mean reimplementing selection, keyboard handling,
  wrapping and minified-file performance, while Fork's diffs include image, binary,
  dynamic LFS and submodule views, the last with its own scrolling
  (<https://github.com/fork-dev/Tracker/issues/261#issuecomment-4970927305>,
  both 14 Jul 2026). In 2019 he had called multiple files in Changes technically
  possible but a UI complication (Tracker #194,
  <https://github.com/fork-dev/Tracker/issues/194#issuecomment-451440617>,
  4 Jan 2019).
- **The Commit tab has the stacked form**, collapsed by default (Finding 4).
- **One path, two diffs.** A type change (symlink replaced by a file) or a
  rename/copy split yields two diffs for one entry; Fork shows only the first,
  having a single diff view (vendor, Tracker #1938,
  <https://github.com/fork-dev/Tracker/issues/1938#issuecomment-1642404483>,
  19 Jul 2023; TrackerWin #106,
  <https://github.com/fork-dev/TrackerWin/issues/106#issuecomment-1744932487>,
  3 Oct 2023, noting an older build concatenated them into an invalid diff).

**Evidence:** VENDOR STATEMENT; TRACKER STATE; USER REPORT.
**Platform:** both.

### Finding 20 — Commits with thousands of files: one `git show` for the whole commit, native virtualizing lists, and freezes fixed by performance work rather than caps

- **Loading.** The captured command fetches the whole commit's patch (`--patch
  --raw`) plus metadata in a single `git show` (TrackerWin #2030) — INFERRED that a
  commit's details load whole, not file by file.
- **Freezes.** Windows froze on large commits until 1.25 (14 Dec 2018) and on
  commits with more than 10,000 changed files until 1.27 (8 Feb 2019), which the
  vendor described as memory and performance work (TrackerWin #122,
  <https://github.com/fork-dev/TrackerWin/issues/122#issuecomment-461888421>).
  After discarding hundreds of files the Windows diff pane cycled through them
  (TrackerWin #999, 2020, closed 2024).
- **Lists.** Mac uses AppKit table and outline views (vendor, Tracker #1955); Windows a
  WPF ListView in virtual mode (vendor, TrackerWin #193,
  <https://github.com/fork-dev/TrackerWin/issues/193#issuecomment-471525801>,
  11 Mar 2019). No file-count cap or "too many files" state was found: OPEN.
- **Untracked giants.** Diffs of large untracked files are not loaded by default
  (Mac 2.38, 12 Jan 2024); the vendor planned a size check before reading them
  (Tracker #2038, 2 Jan 2024).

**Evidence:** USER REPORT (captured command); RELEASE NOTES; VENDOR STATEMENT;
INFERRED.
**Platform:** both.

### Finding 21 — Huge files: a "too large" placeholder with a load button, tripped by long lines as much as by size; the prompt returns after every stage

- **Windows.** A centred Fork logo, `Changes are too large to display`, and a
  `Load Diff` button (user screenshot, TrackerWin #2245,
  <https://github.com/fork-dev/TrackerWin/assets/3931907/4717c9ae-8653-4ed2-bc30-be29f65eda2a>,
  Jun 2024). Loading anyway since 1.27 (8 Feb 2019). The vendor gives the maximum
  line length as `1024 * 2` characters (TrackerWin #496,
  <https://github.com/fork-dev/TrackerWin/issues/496#issuecomment-1956285351>,
  21 Feb 2024), says the number of lines is irrelevant (TrackerWin #2012,
  <https://github.com/fork-dev/TrackerWin/issues/2012#issuecomment-1714085916>,
  11 Sep 2023), and refuses a setting because it is a limitation of the text
  control, which would freeze the whole UI on wide lines (TrackerWin #2245,
  <https://github.com/fork-dev/TrackerWin/issues/2245#issuecomment-2187412709>,
  24 Jun 2024; #496,
  <https://github.com/fork-dev/TrackerWin/issues/496#issuecomment-612412772>,
  11 Apr 2020). Reports in TrackerWin #496 include a one-line file of 3,000
  characters (2020), a 67-line, 31 kB file (2021), a file whose longest line is 2,075
  columns (2024), and a generated file whose reporter blamed its 195-character lines
  (2019) — which, below the 2,048 figure, cannot be the whole cause; what else trips
  the placeholder is OPEN. Loading a genuinely large
  diff can freeze or crash (TrackerWin #1904, 2023, a single 2,097,170-character line)
  and ran out of memory in 2.20.1 (TrackerWin #2798, closed 1 Sep 2026). Windows 2.8
  (9 May 2025) fixed the load button in the working directory.
- **Mac.** A limit existed for years but a bug kept it from applying until a fix
  that users place between 2.50.1 and 2.51/2.52 (spring 2025). It then fired on
  small diffs with long lines; the vendor said the text control no longer froze on
  such input and the limit would be raised in the next update (Tracker #2347,
  <https://github.com/fork-dev/Tracker/issues/2347#issuecomment-2895690845>,
  20 May 2025; long lines alone also count,
  <https://github.com/fork-dev/Tracker/issues/2347#issuecomment-2869961569>). There
  is no option to render large diffs by default
  (<https://github.com/fork-dev/Tracker/issues/2347#issuecomment-2855660978>,
  6 May 2025). A user gives the 2.58 wording as `Large diffs are not rendered by
  default` with a `Load diff` button (Tracker #2513, Dec 2025). The loaded diff
  reverts to the placeholder after each stage or discard (Tracker #2347, #2513, open)
  and after switching applications (Tracker #1711, open). A 2.67 develop build used
  82 GB after `Load diff` on a 1 MB minified line (Tracker #2651, fixed in 2.67.4).
- **Other guards.** Minified-file performance (Mac 1.0.71); out-of-memory on very
  large minified files (Mac 2.68 fix); not hanging on large or minified files
  (Windows 1.18).
- **Exact byte or line thresholds:** OPEN on both platforms.

**Evidence:** USER SCREENSHOT; VENDOR STATEMENT; RELEASE NOTES; USER REPORT.
**Platform:** as noted.

### Finding 22 — Non-text changes each get a purpose-built view

| Kind | What Fork shows | Evidence · platform |
| --- | --- | --- |
| Binary | `Old` (red) and `New` (green) labels above generic file icons, with the size in KB and bytes under each (vendor screenshot, Windows 1.28 blog, Feb 2019). Custom view Mac 1.0.76 (29 Mar 2019), working-directory binaries 1.0.77; byte size in a tooltip Mac 2.41 / Windows 1.96 (2024). Binary versus text is git's decision; the vendor points to `.gitattributes` (TrackerWin #420, <https://github.com/fork-dev/TrackerWin/issues/420#issuecomment-527384642>, 2019; vendor-hosted FAQ). A UTF-16 text file therefore shows only `old` / `new` (TrackerWin #1811, 2023). | RELEASE NOTES; VENDOR SCREENSHOT; VENDOR STATEMENT · both |
| Images | `Old` / `New` side by side, `W: … px \| H: … px (size)` under each, a `Side-by-Side` / `Swipe` / `Onion Skin` segmented control at the bottom, and a checkerboard pixel-highlight toggle at the header's right (vendor screenshot, Tracker #2258, Dec 2024; a user pointing to the checkerboard button, Tracker #1796, Jun 2025). Image diffs since Mac 1.0.26 (17 Oct 2016) and Windows 1.23 (Nov 2018 blog); swipe and onion skin Windows 1.37 (23 Jul 2019) / Mac 2.12 (24 Sep 2021); pixel highlight Mac 2.42 (19 Apr 2024) / Windows 1.97 (3 May 2024); formats added over time (HEIC Mac 2.17; PDF preview Mac 2.3; WebP Mac 2.45 / Windows 2.1; TGA Mac 2.51 / Windows 2.5; SVG Mac 2.51). The mode resets to side-by-side for every file (TrackerWin #2206, open). No zoom: in 2018 the vendor judged it a lot of work and floated just fit-to-screen and real-size modes (Tracker #404, <https://github.com/fork-dev/Tracker/issues/404#issuecomment-421834406>). | RELEASE NOTES; VENDOR SCREENSHOT; USER REPORT · both |
| LFS pointers | A custom LFS view since Mac 1.0.76 (29 Mar 2019). LFS images carry an `LFS` badge on each side; content downloads on demand and is then cached (vendor, Tracker #2218, <https://github.com/fork-dev/Tracker/issues/2218#issuecomment-2419004237>, 2024), and downloaded images show automatically (Mac 2.40); swipe and onion for LFS images Mac 2.27. Text under LFS gets no text diff — first because LFS objects were assumed large (vendor, Tracker #1087, <https://github.com/fork-dev/Tracker/issues/1087#issuecomment-662875680>, 2020), later because Fork only shows what git outputs (2026). A red `not LFS` badge flags large binaries committed without LFS (Mac 2.0 / Windows 1.56, Nov 2020; vendor screenshot TrackerWin #1531, <https://github.com/fork-dev/TrackerWin/issues/1531#issuecomment-1119737470>, 2022). Binary ↔ LFS transitions get a proper diff (Mac 2.40 / 2.41). | RELEASE NOTES; VENDOR STATEMENT; VENDOR SCREENSHOT · both |
| Submodules | A header naming the submodule as changed, with a commit count (`6↓` in the screenshot) and an uncommitted-files note whose tooltip lists the files; the old and new commits as rows with red and green short-SHA chips; since Jan 2024 the commits between them as a small graph; an `Open Submodule` button (vendor screenshot, Tracker #1751, <https://github.com/fork-dev/Tracker/assets/618115/2b3c65b2-3152-4738-9ae5-ff9f255168e6>, 9 Jan 2024). Custom view Mac 1.0.73 (1 Feb 2019); submodule changes in details Windows 1.25 (14 Dec 2018), improved 1.26; uncommitted-changes indicator Mac 1.0.88; branch names Mac 2.9; commit list Mac 2.38 / Windows 1.93; text-diff fallback when the commit is missing (Windows 2.21 / Mac 2.69 fixes). Driven by `--submodule=short` (captured command); a `diff.submodule=log` config broke it (Tracker #522, 2019; TrackerWin #1265, 2021). | VENDOR SCREENSHOT; RELEASE NOTES; USER REPORT · both |
| Symlinks | As git shows them: the link target as text (user, Tracker #1938, 2023); display fixed Mac 1.0.64 (22 Feb 2018). A symlink replaced by a file is two diffs and only the first shows (Finding 19). | USER REPORT; VENDOR STATEMENT · both |
| Mode changes | Mac 1.0.62 (15 Jan 2018) lists a fix for a chmod change showing no diff or explanation, yet in Mar 2019 a user again saw nothing for one. The vendor's difficulty was where to put it, since mode and content can change together and binaries have no text diff (Tracker #575, <https://github.com/fork-dev/Tracker/issues/575#issuecomment-473428274>, 15 Mar 2019); permission changes for text files were shown from Mac 1.0.83 (30 Aug 2019; <https://github.com/fork-dev/Tracker/issues/575#issuecomment-539442016>); file mode changes in diff Mac 2.8 (28 May 2021) / Windows 1.62 (7 May 2021). Presentation OPEN: no screenshot found, and in Dec 2020 a user still saw nothing for a mode-only change (same issue). | RELEASE NOTES; VENDOR STATEMENT · both |
| Renames | An arrow change-type badge; `old→new` in the Mac Commit-tab row (2018); tooltips (Windows 1.58); old and new names (Windows 1.81 / Mac 2.26); detection by `--find-renames` at git's default threshold (captured command). No similarity percentage anywhere found. Partially unstaging a renamed file also unstages the rename (vendor-confirmed bug, Tracker #1872, <https://github.com/fork-dev/Tracker/issues/1872#issuecomment-1516684161>, 2023). | RELEASE NOTES; VENDOR SCREENSHOT; USER REPORT · both |
| Encodings | Windows: Unicode / UTF-8 only, no encoding selector (vendor, 2020–2024: TrackerWin #695, <https://github.com/fork-dev/TrackerWin/issues/695#issuecomment-593408348>; #944; #1978; #2313, <https://github.com/fork-dev/TrackerWin/issues/2313#issuecomment-2320575734>). Non-UTF-8 files display mangled and partial staging fails with `patch does not apply` (TrackerWin #75, open since 2018; #1652). UTF-16 is binary to git; the vendor recommends `working-tree-encoding` in `.gitattributes` (TrackerWin #1811, #2183). Mac: a 2019 request to choose the encoding has no reply (Tracker #685); the vendor confirmed a bug in processing multibyte characters in the diff panel (Tracker #2061, <https://github.com/fork-dev/Tracker/issues/2061#issuecomment-1920912725>, 1 Feb 2024, open). | VENDOR STATEMENT; TRACKER STATE · Windows documented; Mac OPEN |

## Part E — Line and hunk staging (context only; the staging packet builds it)

### Finding 23 — Hover a chunk for floating `Stage` / `Discard` buttons; drag-select text to act on lines; Return and Backspace act on the selection

- **The active chunk.** Hovering a chunk in Local Changes outlines it — an
  accent-coloured border around the chunk including its header row, with a faint tint
  on the context rows inside — and shows `Stage` and `Discard Changes…` (Mac) or
  `Stage` and `Discard…` (Windows) at its top right; in the staged list the button
  is `Unstage` (vendor screenshots: Mac 1.0.79 blog
  <https://fork.dev/blog/posts/fork-1.0.79/>, 2019; both carousels, 2020. User
  screenshots: TrackerWin #250, 2019; Tracker #1985, 2023. User video of the
  `Unstage` button: Tracker #1681, 2022). Reworked in Mac 1.0.24 (30 Sep 2016) and
  1.0.79 (31 May 2019); Windows got a new floating-button control with side-by-side
  (vendor, TrackerWin #45,
  <https://github.com/fork-dev/TrackerWin/issues/45#issuecomment-498604426>,
  4 Jun 2019). On Mac 1.0.72, clicking a button hid the buttons even when another
  chunk followed, until the mouse moved (Tracker #480, Dec 2018; closed Aug 2019).
- **Lines.** Select text with the mouse across the lines wanted; the same buttons and
  keys then act on those lines only (user, Tracker #1500, Nov 2021). Clicking a line
  without dragging across characters does nothing; the vendor answered the request
  with a video and said line staging has existed since the first public build
  (Tracker #2316,
  <https://github.com/fork-dev/Tracker/issues/2316#issuecomment-2734721784>,
  18 Mar 2025). A partial selection acts on whole lines, not on the selected
  characters (users, TrackerWin #159, Jun 2019). Selecting via line numbers or by
  double-click failed to show the buttons until Windows 1.22.
- **Keys.** Stage or unstage the selected lines or file: Return or ⌘S on Mac, Enter or
  Ctrl+Shift+S on Windows; discard: Backspace, or ⌘⇧D / Ctrl+Shift+D (vendor shortcut
  lists; the 2017 Mac summary gave ⌘⇧S or Enter to stage, Tracker #54,
  <https://github.com/fork-dev/Tracker/issues/54#issuecomment-313667450>,
  7 Jul 2017). Space was deliberately moved from staging to the quick
  look: the vendor wanted reviewing to be the impulsive key and staging the
  deliberate one (Tracker #57,
  <https://github.com/fork-dev/Tracker/issues/57#issuecomment-313913787>,
  9 Jul 2017). With nothing selected the keys apply to the whole file even while a
  chunk is hovered, because hovering is not selecting (vendor, Tracker #103,
  <https://github.com/fork-dev/Tracker/issues/103#issuecomment-331724876>,
  24 Sep 2017); Windows 1.30 applies them to the active selection.
- **Discard** asks for confirmation (the ellipsis on the button). **Discarding staged
  changes is refused by design**: unstaged edits depend on staged ones, so dropping
  the staged layer can lose both, and a staged chunk could not be discarded on its
  own (vendor, TrackerWin #1563,
  <https://github.com/fork-dev/TrackerWin/issues/1563#issuecomment-1157432945>,
  16 Jun 2022). No later release note adds it, although the April 2016 pre-release
  notes (GitClient 1.0.1) describe resetting staged changes.
- **Mechanism.** Patches applied with `git apply --cached`. In 2019 the vendor found
  Fork had been passing `--whitespace=nowarn` where `--ignore-whitespace` was needed —
  a mistake dating from the earliest Mac builds — and switched in Windows 1.38
  (TrackerWin #202,
  <https://github.com/fork-dev/TrackerWin/issues/202#issuecomment-511400934>,
  15 Jul 2019;
  <https://github.com/fork-dev/TrackerWin/issues/202#issuecomment-522046707>,
  16 Aug 2019); the vendor dated the mistake to around Mac 1.0.2. The very first Mac
  notes describe partial staging implemented by parsing the diff into a syntax
  tree, editing it and re-emitting it (GitClient 1.0.1, 17 Apr 2016). Staging runs on a background thread on Windows since 2.13
  (24 Oct 2025).
- **Side-by-side.** Hunk staging works on both (Windows since 1.33; Mac since 2.21).
  Windows 1.76 (29 Jul 2022), which the vendor called one of Fork's biggest updates —
  about 60 commits, nearly all on staging in both modes — introduced side-by-side
  staging bugs fixed in 1.76.5 (TrackerWin #1607,
  <https://github.com/fork-dev/TrackerWin/issues/1607#issuecomment-1201599240>;
  #1612). Lines can be selected on one side at a time only, the sides being separate
  controls (vendor, Tracker #1985). The floating buttons fail right after toggling
  the mode until another file is selected (vendor-confirmed bug, Tracker #2228,
  <https://github.com/fork-dev/Tracker/issues/2228#issuecomment-2452201883>, 2024).
- **Entire-file mode.** In 2019 the buttons applied to the whole file, the file being
  one chunk; the vendor advised selecting lines manually (TrackerWin #250). Mac 2.19
  (17 Jun 2022) allows sub-chunk staging in entire-file mode; no Windows equivalent
  appears in the notes — OPEN.
- **Ignore whitespace — the bug trail.** The issue the precedent study cited,
  Tracker #360 (Mac, Jul 2018: the floating `Stage` button inert with ignore
  whitespace on), was closed by its reporter as resolved in Jan 2022, after the vendor
  said such staging needs extra handling that he believed was in place. The durable
  behaviour is documented elsewhere: staging a whitespace-ignored chunk can pull in
  the hidden whitespace edits or fail with `patch does not apply`. The vendor said
  nothing can be done when the context is too thin for git to find the chunk, and
  that ignore whitespace is for review rather than staging (TrackerWin #202,
  <https://github.com/fork-dev/TrackerWin/issues/202#issuecomment-473311836>,
  15 Mar 2019); added `--ignore-whitespace` (Windows 1.38, Aug 2019) while noting
  that failures persist; and on Mac called a failing case not a bug (Tracker #782,
  <https://github.com/fork-dev/Tracker/issues/782#issuecomment-544951171>,
  22 Oct 2019). Since Mac 2.42 (19 Apr 2024) and Windows 1.97 (3 May 2024), when a
  chunk stage fails Fork proposes turning ignore whitespace off.
- **Other failure edges.** Non-UTF-8 files (Finding 22); renamed files (Tracker
  #1872); and reports that a partial stage still committed the whole file, which the
  vendor traces to pre-commit hooks re-staging files (TrackerWin #1774, 2023;
  Tracker #2192, 2024); in an earlier report the vendor first asked whether an IDE
  or other tool was staging files too (Tracker #647, 2019).

**Evidence:** VENDOR STATEMENT; VENDOR SCREENSHOT; RELEASE NOTES; VENDOR DOCS; USER
REPORT.
**Platform:** both.

## Part F — Visual specifics

All values below are MEASURED from screenshots with a script (row pitch from
line-number glyph positions, advance from the pixel extent of a known string,
colours from region histograms) and are approximate.

### Finding 24 — Row metrics: uniform rows, hunk header included; Mac rows are relatively taller than Windows rows

| Measure | Mac — vendor screenshot, Retina 2×, May 2026 (Tracker #2624) | Windows — vendor screenshot, Aug 2021 (TrackerWin #1216); user screenshot, Mar 2025 (TrackerWin #2460) |
| --- | --- | --- |
| Row pitch | 34 px = 17 pt | 30–31 px (display scale unknown) |
| Monospace advance | 13.2 px = 6.6 pt, consistent with Menlo at 11 pt | 14.3–14.4 px |
| Row pitch ÷ advance | ≈ 2.6 | ≈ 2.1 |
| Row pitch ÷ font size | ≈ 1.55 (17 pt ÷ 11 pt) | ≈ 1.15–1.2 if the face is Consolas; face OPEN |
| Hunk header row | same pitch as a code row | same |
| Changed-row band | fills the whole pitch; consecutive changed rows touch; starts after the gutter | same (30 px band) |
| Line-number glyphs | 6.5 pt tall, light grey | light grey |
| Unified gutter | two columns ≈ 50 pt wide for two-digit numbers, then a 1 px separator | two columns ≈ 9.5 advances wide for four-digit numbers, then a thin separator (2 px in these captures) |

A 2023 Mac dark screenshot at 1× shows an 18 px pitch (Tracker #1985).

### Finding 25 — Dark theme colours

The vendor says Fork uses GitHub's diff colours (Tracker #785,
<https://github.com/fork-dev/Tracker/issues/785#issuecomment-545032725>,
22 Oct 2019); macOS's increased-contrast setting switches to an alternative palette
(<https://github.com/fork-dev/Tracker/issues/785#issuecomment-808443274>,
26 Mar 2021). The red/green pair is a standing accessibility complaint
(Tracker #785, open since 2019; TrackerWin #1504), answered so far by the `-+` marks.

| Element | Windows, 2023–2026 | Mac, 2023 | Mac, 2025 |
| --- | --- | --- | --- |
| Diff background | `#282828` | `#323232` unified; `#2B2D30` side-by-side | `#282828` |
| Removed line | `#633F3E` | `#633F3E`; `#583837` side-by-side | ≈ `#5E413F` |
| Added line | `#3A5C3F` | `#3A5C3F`; `#335138` side-by-side | ≈ `#425B42` |
| Removed, intra-line | `#9F4247` | `#9F4247` | ≈ `#944849` |
| Added, intra-line | `#388442` | `#388442`; `#33793C` side-by-side | OPEN |
| Side-by-side filler rows | `#424242` | `#3A3A3A` | — |
| Diff text (default token colour) | `#DDDDDD` | `#DDDDDD` | `#DDDDDD` |
| Hunk header and line numbers | ≈ `#A0A0A0` | ≈ `#959595` | ≈ `#7C7C7C` (preferences preview) |
| Gutter separator | `#4B4B4B` | `#464646` | — |
| Diff header bar | `#333333` | — | — |
| Active chunk outline | — | `#126CFB` (the system accent); ≈ 2 px on the left edge, 1 px top and bottom | — |
| Floating buttons | — | fill ≈ `#59635D`, label ≈ `#E5EFE9` | — |
| Text selection on an added row | — | ≈ `#36455C` | — |

Sources — Windows: TrackerWin #1834
(<https://user-images.githubusercontent.com/26698188/223504113-709fcbec-dc96-4a28-a281-61876100293d.png>,
2023), #2012 (2023), #2537 (2025), #2627 (2025), #2723
(<https://github.com/user-attachments/assets/5ce70c2b-ca6a-4494-89e5-9826e586bae7>,
2026). Mac: Tracker #785
(<https://github.com/fork-dev/Tracker/assets/229384/76739d5b-50c9-4ede-aabf-9f9679099e77>,
2023), #1985
(<https://github.com/fork-dev/Tracker/assets/11131775/3436b29f-1c45-45ae-8347-a6d0dbc839ed>,
2023), #2496 (2025). Older Mac captures give removed ≈ `#623F3F` and added
≈ `#3B5B40` (a user's colour crops, Tracker #785, 2019) and added ≈ `#425B42` on
`#323232` (Tracker #910, 2020).
Excluded: TrackerWin #2460, whose whole capture is tinted.

### Finding 26 — Light theme colours

| Element | Windows, 2020–2025 | Mac, 2019–2020 (JPEG) | Mac, 2021–2026 |
| --- | --- | --- | --- |
| Diff background | `#FFFFFF` | `#FFFFFF` | `#FFFFFF` |
| Removed line | `#FFE5E5` | ≈ `#FFE5E4` | ≈ `#FBE6E5` |
| Added line | `#E2FDE3` | ≈ `#E3FDE2` | ≈ `#E7FCE4` |
| Removed, intra-line | `#F8CFD3` | ≈ `#FFC4C6` | ≈ `#F6C6C5` |
| Added, intra-line | `#BDF3BD` | ≈ `#B8F5BA` | ≈ `#C7F2C0`; a whole-line modification ≈ `#B6F0B6` (Tracker #2288, 2025) |
| Side-by-side filler rows | `#F9F9F9` | — | — |
| Line numbers and hunk header text | ≈ `#C0C0C0` | — | ≈ `#C8C7C7` |
| Gutter separator | ≈ `#DADAD7` (2021) to `#EDEDEB` (2022) | — | `#E6E6E6` |
| Active chunk outline | ≈ `#4E8BC2` (carousel, 2020) | ≈ `#449DF1` (1.0.79 blog, 2019) | — |
| Context rows inside the active chunk | — | ≈ `#FAFBFF` (2019) | — |

Sources — Windows: TrackerWin #1612 (2022), #1216 (vendor, 2021), #2575 (2025),
carousel 2020. Mac: carousel 2020
(<https://git-fork.com/images/carousel/carousel_commitviewMac1.jpg>), 1.0.79 blog
(2019), Tracker #307 (vendor, 2021), #2288 (2025), #2624 (vendor, 2026).

### Finding 27 — Styling of the remaining parts

- **Hunk header:** a plain grey text row (Finding 13); inside an active chunk it sits
  within the outline, with the floating buttons over its right end.
- **Collapsed regions:** no marker, divider, band or expander between hunks; the next
  `@@` row is the only boundary (all screenshots, both platforms, 2016–2026).
- **Changed rows:** full-height tinted bands behind the text column only; syntax
  colours are drawn over the tint; no `+` / `-` characters unless the option is on.
- **Selection:** the platform's text-selection colour inside the diff text.
- **Tabs and buttons:** native controls — an AppKit segmented control and push buttons
  on Mac; WPF tabs and buttons following the Windows accent colour (Windows 1.49,
  7 May 2020, adopted it).

**Evidence:** MEASURED; VENDOR SCREENSHOT; USER SCREENSHOT.
**Platform:** both.

## Corrections to `what-clients-show.md`

That record is historical and is not edited; these are the corrections a reader of
it should apply.

1. **Finding 2** dates the File Tree in commit details to Mac 2.0 (13 Nov 2020). The
   release note it paraphrases belongs to Mac **1.0.9 (10 Jun 2016)**; 2.0's notes do
   not mention the File Tree.
2. **Finding 2** (and Findings 12 and 23) date the display of old and new names for
   renamed files to Mac 2.28 (14 Apr 2023). It is Mac **2.26 (9 Feb 2023)** and
   Windows **1.81 (27 Jan 2023)**.
3. **Finding 2** says the diff renders below the file list. In the **Changes** tab the
   diff is to the **right** of the file list; only the **Commit** tab expands diffs
   below file rows (Findings 4, 5).
4. **Finding 9's** Fork row reads as if side-by-side reached commit view before Local
   Changes on both platforms. That sequence is Mac's (1.0.50 was a quick-look popup
   only; inline in commit changes 1.0.88; Local Changes 2.21). On **Windows, 1.33
   appears to have brought it to both views at once** (INFERRED, Finding 11).
5. **Finding 11** dates the control bar above diffs to Windows 1.16. It is **Windows
   1.18 (28 Jun 2018)**. The same finding lists text size among the controls; that is
   the Mac 1.0.69 post's own wording, but the control sets the **number of visible
   (context) lines** — the Windows 1.18 post calls it decrease/increase context lines,
   the Mac post's GIF tooltip names visible lines, and Mac 1.0.95 fixed a crash when
   changing the number of visible lines.
6. **Finding 11** cites Tracker #360 as the whitespace-and-staging bug. #360 was
   **closed by its reporter as resolved (Jan 2022)**; the lasting evidence is
   TrackerWin #202, Tracker #782, Tracker #27 and the Mac 2.42 / Windows 1.97
   proposal to disable ignore whitespace when a chunk stage fails (Finding 23).
7. **Finding 12** dates the custom binary/LFS diff view to Mac 1.0.70. It is Mac
   **1.0.76 (29 Mar 2019)**. Swipe/onion-skin image modes reached Mac in **2.12
   (24 Sep 2021)**; the custom submodule view dates from Mac **1.0.73 (1 Feb 2019)**,
   and Windows showed submodule changes in details from **1.25 (14 Dec 2018)** — long
   before the 2.69 icon.
8. **Finding 15** says Tracker #1985 has no vendor reply. The vendor **replied the same
   day** (13 Oct 2023): the two sides are separate controls, so a selection across both
   cannot be implemented.
9. **Finding 13** calls Mac 2.38's large-untracked-file note Fork's only related note.
   There are more: Mac **1.0.32** (large-file performance in the changes view),
   Windows **1.18** (no hang on large or minified files), **1.25 and 1.27**
   (large-commit freezes) and **1.27** (show content even when too large)
   (Findings 20, 21).
10. **Finding 25** leaves Fork's too-large threshold open. Windows is now documented at
    **2,048 characters per line** (vendor, 2024), with line count irrelevant; Mac's
    limit is confirmed to exist and, per users, began firing between 2.50.1 and
    2.51/2.52 in 2025 (Finding 21). Byte thresholds remain open. The finding's
    implication that 195-character lines tripped Fork rests on the TrackerWin #496
    reporter's own guess; below the 2,048 figure, line length alone cannot explain
    that case.
11. **Finding 23** leaves Fork's `diff.algorithm` open. The vendor's advice to set
    `git config diff.algorithm patience` to change hunks implies git's configured
    algorithm is honoured, consistent with Fork running `git show` (Finding 8).
12. **Finding 2's** first-parent premise for merge commits is now directly corroborated
    by the captured `--diff-merges=1` command and the Dec 2022 error reports (Finding 8).
13. **Finding 14** says a discard exists for staged and unstaged changes, citing the
    April 2016 pre-release notes. Current Fork **refuses to discard staged changes by
    design** — staged chunks offer only `Unstage` (vendor, TrackerWin #1563, 2022;
    Finding 23).

## What the evidence does NOT settle

- Whether the `COMMITTER` column is hidden when author and committer are the same
  (Finding 3).
- Current git-notes display on each platform (Finding 3).
- What the Mac 1.0.71 buttons that reveal a file in the Changes view look like
  (Finding 4).
- Whether the commit Changes tab also shows only the first of several selected files
  (Finding 5).
- Windows: whether the active detail tab is remembered; ⌘⌥1/2/3 equivalents
  (Findings 2, 18).
- Mac: how two selected commits are ordered (Finding 7).
- The merge-diff mechanism before Fork 2.24 (Dec 2022) (Finding 8).
- Mac: the scope of the side-by-side setting, and whether word wrap works in the quick
  look today (Finding 11).
- When and why the Aug 2023 context dropdown was replaced by separate buttons again, and
  what the dropdown offered (Findings 10, 14).
- The maximum number of visible lines (Finding 14).
- Windows: sub-chunk staging in entire-file mode (Finding 23).
- Byte or line thresholds for "too large" on both platforms, and what besides a
  2,048-character line trips the Windows placeholder (Finding 21).
- Whether any file-count cap or state exists for huge commits (Finding 20).
- How mode changes are drawn (Finding 22).
- Where the old and new names of renamed files appear since 2023 (Finding 5).
- Tab-width setting location; the Windows diff font face (Findings 17, 24).
- Mac encoding handling for non-UTF-8 files (Finding 22).
- The added intra-line colour on Mac in 2025 (Finding 25).

## Fork's model in one paragraph

Fork puts a resizable, collapsible detail pane under the commit list (Windows can move it
to the right) with three tabs that answer three different questions. **Commit** is for
reading: who authored and committed it, with avatars and full timestamps, the full SHA,
parents as links, refs pointing at it, the message — and then the changed files, each
expandable in place into its diff, collapsed by default because expanding a big commit
eagerly would stall. **Changes** is for inspecting: a one-line commit summary over a split
of file list (tree, list or two-column list, with a filter) and the diff of exactly one
file. **File Tree** is the repository at that commit with a file preview. The diff itself
is git's unified patch drawn faithfully — raw `@@` rows as muted text, two line-number
columns, full-height red and green bands with stronger intra-line ranges — governed by one
global icon toolbar (ignore whitespace, invisibles, wrap, fewer or more context lines,
entire file, side-by-side) whose state also drives the Commit tab's inline diffs. Context
grows only globally, one line at a time, or jumps to the entire file; there is no
per-gap expansion. Side-by-side is a projection of the same patch into two equal, separate
text controls, so wrap is off and a line selection stays on one side; for serious review
the Space bar opens the same diff large, in a quick look. Merges and stashes are shown
against their first parent. Anything that is not text — images, binaries, LFS, submodules
— gets its own view in the same slot. Staging lives in that diff: hover a chunk and its
outline and floating Stage / Discard buttons appear; drag-select characters and the same
buttons and keys act on those lines; everything is turned into a patch for
`git apply --cached`, which is why whitespace-ignored views stage unreliably and why
reverting hunks of past commits was built and withheld.

## Where Cairn's current mockup diverges from Fork

Detail pane and diff view only. Mockup references are to
`docs/design/mockups/cairn-ui.html` by screen and CSS selector, and to
`docs/design/ui.md`.

**Detail pane (History screen; also the Worktrees screen's pane)**

1. **Fixed placement.** `.detail` is a fixed `height: 250px` row under the list. Fork's
   pane is resizable, collapsible (2026) and, on Windows, can sit to the right
   (Finding 1). `ui.md` presents "below" as Fork's layout; Fork offers both, on one
   platform.
2. **Tab strip.** `.dtabs` draws underlined tabs with a count on Changes
   (`Changes <small>8</small>`). Fork has no counts; Mac uses a segmented control,
   Windows underlined tabs (Finding 2).
3. **Identity block.** `.commit .kv` shows two label/value lists: `Author` as
   name · short time, `Commit` as a 7-hex id, `Parents` as plain text, `Committer`
   repeated even when identical, `Refs` as comma-separated text. Fork shows
   avatar + name + email + full timestamp per person in two columns, the full
   40-hex SHA, parents as short-hash links with hover details, refs as chips — and
   apparently omits the committer when identical (Finding 3).
4. **Message.** `.commit .msg` caps a muted body at `70ch` under a bold subject. Fork
   shows a larger subject over a normal-weight body with linked issue references
   (Finding 3).
5. **File list in the Commit tab.** `.files` is a single inline row of paths with a
   green `+N` per file. Fork lists files vertically with change badges and file icons,
   offers Expand All / Collapse All, expands diffs in place, and shows no per-file or
   total counts anywhere (Findings 3, 4).
6. **Changes and File Tree are never drawn.** Fork's Changes is a summary line over a
   file list | diff split; File Tree a tree | preview split (Findings 5, 6).
7. **No selected-stash or two-commit state.** The mockup's inline stash row
   (`.list .r.stash`) has no detail state; Fork shows the stash as a commit with two or
   three parents and a first-parent diff, and a two-commit comparison as a Changes view
   naming both commits with a swap control (Findings 7, 9).

**Diff (Local changes screen; Discard screen)**

8. **Header.** `.diffhd` shows the path, `+9 −3` totals, and three text buttons
   (`Unified`, `Side by side`, `Ignore whitespace`). Fork shows previous/next-change
   chevrons, a centred path with the file name emphasized, and seven icon toggles —
   including a single side-by-side toggle rather than a Unified/Side-by-side pair, and
   context, entire-file, wrap and invisible-character controls the mockup lacks — and
   no totals (Finding 10).
9. **Hunk header.** `.hunk` is a 24 px band on `--raised` with top and bottom borders and
   persistent `Stage hunk` / `Discard hunk…` buttons; `ui.md` says hunk actions live on
   the hunk header. Fork's header is an inert grey text row at code-row height, and its
   Stage / Discard buttons float over the top right of whichever chunk the pointer is in,
   with the chunk outlined (Findings 13, 23, 27).
10. **Gutter and line selection.** `.ln` is a 28 px checkbox column, two 40 px number
    columns, and content with an always-on `+ ` / `- ` prefix; a picked line (`.ln.pick`)
    gets an accent tint and a filled checkbox. Fork has no checkbox column: lines are
    chosen by dragging a text selection that acts on whole lines, the `-+` column is an
    opt-in option, and there are exactly two number columns plus a separator
    (Findings 12, 23).
11. **Intra-line highlighting.** None in the mockup; always on in Fork, with no off
    switch (Finding 16).
12. **Colours.** The mockup's `--add-bg` / `--del-bg` are 13 % washes that composite over
    `--bg #1B1D22` to about `#22302C` and `#35272C`, with `--add` / `--del` foregrounds
    used only for the prefix. Fork's dark rows are opaque mid-tones (`#3A5C3F`,
    `#633F3E`) with stronger intra-line ranges (`#388442`, `#9F4247`) under
    default-coloured text (Finding 25).
13. **Row metrics.** The mockup uses 12 px mono at a 20 px line (≈ 1.67) with 24 px hunk
    rows. Fork uses one row height for code and hunk rows alike — about 1.55 × the font
    size on Mac (Finding 24).
14. **Multiple files selected.** The Discard screen shows `2 files selected` over an empty
    `.diff`. Fork, per users, shows the first selected file's diff (Finding 5).
15. **Side-by-side.** Offered by a button but not drawn. Fork's rendering has constraints
    worth drawing before committing to it: equal non-resizable panes, one gutter each,
    grey filler rows, a repeated hunk header, word wrap disabled, and line selection on
    one side at a time (Finding 11).

## Open questions for the planner

1. **Placement.** Adopt Fork-Windows' two layouts (details below or right) from the start,
   or ship "below" with a splitter and collapse? `ui.md` already says the components must
   not assume either (Finding 1).
2. **Commit tab diffs.** Does Cairn's Commit tab expand file diffs in place (Fork's
   GitX-style stacked form), which is a continuous multi-file render inside the detail
   pane — exactly the thing Fork keeps collapsed for performance — or list files and hand
   off to Changes (Findings 4, 19)?
3. **One file or many in Changes.** Fork: one file, first of a multi-selection. Keep that,
   or show a continuous diff for a multi-selection, like Sourcetree's multi-select
   cumulative diff (Findings 5, 19; history-graph companion record)?
4. **Header fields.** Committer shown only when it differs? Author date in details and
   committer date in lists, as Fork does? Refs at the commit only, with no
   "branches containing" (Fork refuses on cost and meaning)? A diffstat, which Fork lacks
   (Finding 3)?
5. **Merge commits.** First-parent only, as Fork, or a parent choice or combined view — and
   is the combined format a separate model type (Finding 8; companion Finding 1)?
6. **Stashes.** Mirror Fork's first-parent diff that omits untracked files, or show the
   third-parent untracked files that Fork's users keep asking for (Finding 9)?
7. **Context.** A global count (default 3, ± one line, entire-file toggle) as in Fork, or
   per-gap expansion, which Fork lacks and users request (Finding 14)? And how does
   entire-file mode interact with hunk staging (Finding 23)?
8. **Side-by-side scope.** One global setting shared by all views (Fork-Windows) or per
   view? Is wrap disabled in side-by-side, and can a line selection span both sides —
   the one place Fork's split is not a projection of a shared selection model
   (Findings 11, 23; companion Finding 15)?
9. **Whitespace.** Ignore whitespace as `-w` for display with staging disabled, or
   Fork's approach — `--ignore-whitespace` on apply and a prompt to turn the toggle off
   when a patch fails (Findings 15, 23)?
10. **Too large.** Which unit trips the placeholder (Fork-Windows: 2,048 characters in any
    line), what the message says, and whether "load anyway" survives staging — Fork's
    reverting after every stage is an open complaint (Finding 21).
11. **Intra-line highlighting.** Always on without a switch, as Fork? Token-level LCS, as
    Fork-Windows now does — with a cap for minified lines, where Fork twice hit
    memory blow-ups (Finding 16)?
12. **Hunk actions.** Persistent buttons on a header band (the mockup) or Fork's
    hover-outline with floating buttons? And line selection by checkbox gutter (the
    mockup) or by text selection (Fork) (Findings 13, 23)?
13. **Diff header content.** Totals, rename old → new, similarity and mode change are all
    absent from Fork's header; does Cairn show any of them, given the engine will know
    them (Findings 10, 22)?
14. **Colour and density.** Fork's opaque bands with stronger intra-line ranges, or the
    mockup's washes? Row height ratio? A `-+` column for colour-vision accessibility
    (Findings 24–26)?
15. **Keyboard.** Which Fork chords carry into the D5 accelerator table — Space for quick
    look, Return / Backspace for stage / discard of a selection, previous/next change,
    tab switching (Findings 18, 23)?
16. **Hands-on checks** worth a few minutes on the owner's Fork before the design locks:
    committer column when identical; commit Changes multi-selection; the current context
    control and its maximum; Mac side-by-side scope and wrap; Mac two-commit ordering; how
    a mode change and a rename are drawn; the Commit tab's reveal-in-Changes buttons
    (list under "What the evidence does NOT settle").
