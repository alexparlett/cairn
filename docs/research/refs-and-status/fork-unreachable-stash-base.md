# Fork and a stash whose base no ref reaches

Evidence record for the `refs-and-status` packet, recorded 2026-10-05, and a
follow-up to `fork-refs-and-status-ui.md` Section 3 (stashes inline in the commit
list), whose findings are not repeated here. It answers one question: how Fork
draws a stash whose base commit (`stash^1`, the commit the stash was made on) is
reachable from no branch, remote-tracking branch, tag or `HEAD` — the branch it
was made on deleted, its commits amended or rebased away, or a detached `HEAD`
that moved on — and, around it, whether Fork ever draws a stash's index and
untracked commits, whether "hide stashes" changes which commits are walked, and
what the commit list shows for commits that only a stash reaches. Fork has no
Linux build, so nothing was installed or clicked: sources are the Mac and Windows
release notes, `fork-dev/Tracker` and `fork-dev/TrackerWin` issues and their
comments (read through the GitHub API; every issue whose title or body matches
"stash" was listed, 102 Mac and 111 Windows, and the relevant ones read in
full), Fork log excerpts users pasted, and a user's screen recording. Secondary
precedent (git, gitk, Sourcetree, GitKraken, Tower, SmartGit, and two VS Code
graphs whose source is public) is at the end.

Conventions are the companion's: `Tracker #N` is
`https://github.com/fork-dev/Tracker/issues/N`, `TrackerWin #N` is
`https://github.com/fork-dev/TrackerWin/issues/N`; "Mac RN" is
<https://git-fork.com/releasenotes>, "Win RN" is <https://git-fork.com/releasenoteswin>;
`DanPristupov` is the vendor. Quotes are at most 15 words. Labels: RN, VENDOR,
USHOT (user screenshot or recording, dated), USER, INFERRED, OPEN.

## Headlines

1. **Fork does (c): a stash whose base the graph does not reach is not drawn in
   the commit list at all, and stays listed in the sidebar's Stashes section.** The
   vendor says so twice — "Stashes are connected to the commits they were created
   on", so after an amend or rebase "the stash will not be visible" (TrackerWin
   #1050, Dec 2020), and "You can still see all stashes on the sidebar" (Tracker
   #1283, Feb 2021) — and a Windows user's recording (TrackerWin #1622, Aug 2022)
   shows it: three stashes in the sidebar, two rows in the graph, nothing for the
   third, no dangling edge, no placeholder row. Evidence: VENDOR, consistent across
   2020-2024, plus one USHOT. Strong.
2. **Fork never walks from a stash.** The base and its otherwise-unreachable
   ancestors are not brought into the graph (no (a)); the stash row is attached
   only to a commit the ordinary walk already produced. The vendor's wording
   ("connected to the commits they were created on", "the old one (with stash)
   becomes invisible") and the recording, which draws no orphan commit, carry
   this. Its corollary, reported by users: a stash on a commit that a hidden or
   filtered branch alone reaches disappears with that branch (TrackerWin #2463,
   #1871). INFERRED from VENDOR and USHOT; how Fork builds the walk is not
   published.
3. **Selecting such a stash in the sidebar still shows it** — the detail pane
   loads the stash's own changes — but the commit list has no row to select, and
   in 2022 it kept the previous row highlighted (TrackerWin #1622, open; user
   comment on TrackerWin #1871 names "orphaned" stashes as the trigger). That is a
   bug report, not a design; whether a click on it now does anything else, such
   as the "beep" the vendor proposed in 2026 for a branch absent from the current
   topology (TrackerWin #2828), is OPEN.
4. **Index and untracked commits are never rows** — "Do not show stash revisions
   in commit list" (Mac RN 1.0.12, Jul 2016), unchanged since; they are reachable
   only as parent links in the Commit tab (Tracker #898). Whether "Show lost
   commits (reflog)" mode ever draws them is OPEN.
5. **"Hide stashes in the commit list" removes stash rows; nothing published says
   it changes the walk**, and since stashes do not seed the walk (Headline 2) it
   has nothing to change. INFERRED; OPEN as a stated fact.
6. **Fork's graph is not `git log --all`.** No issue or log shows Fork running
   `git log` for the commit list; its Mac log shows stashes read in-process by
   its own object reader ("Failed to read stash objects for given oids", Tracker
   #1953, #2030; TrackerWin #2073), the walk is capped at the newest N commits
   (50,000, 100,000 on Mac since 2.41), and the vendor's "`git log --all
   --reflog`" (TrackerWin #1307) is an analogy for the lost-commits mode, not a
   quoted command. `git log --all` would behave differently (Headline 7). OPEN in
   detail; the difference is established.
7. **Precedent is split.** `git log --all` and `gitk --all` walk `refs/stash`
   (stash@{0} only) like any ref, so an orphaned *latest* stash brings in its base,
   ancestors, index and untracked commits, while older stashes are not drawn at all
   (verified on git 2.56.0). GitLens's Commit Graph (GitKraken) and VS Code's Git
   Graph deliberately walk every stash, or its base, so orphaned bases appear.
   SmartGit draws a stash's commits when its Branches-view checkbox is on. Tower
   documents no stashes in History. Sourcetree's KB calls its graph "essentially"
   `git log --graph --all --date-order`. Fork's choice — draw only what the
   ref walk already reached — is the narrowest.

## 1. What the vendor says

- **TrackerWin #1050** (Dec 2020, Windows 1.56.2). User: after renaming the branch
  the stash was made on, "the stash that I made no longer shows in the commit
  graph anywhere", though it still exists. VENDOR
  (<https://github.com/fork-dev/TrackerWin/issues/1050#issuecomment-740776683>):
  "Stashes are connected to the commits they were created on." and "if you amend
  (or rebase) a commit, the stash will not be visible." The user confirms he had
  amended the base. Closed as explained, not as a bug.
- **Tracker #1283** (Feb 2021, Mac). User: a stash "disappeared" from the graph
  after rebasing its branch. VENDOR
  (<https://github.com/fork-dev/Tracker/issues/1283#issuecomment-779338222>):
  "Rebase recreates the commit tree and the old one (with stash) becomes
  invisible." and "You can still see all stashes on the sidebar." Closed.
- **Tracker #83** (2017), on the detached-`HEAD` case generally: commits made on a
  detached `HEAD` "are visible until you checkout something else" (VENDOR,
  <https://github.com/fork-dev/Tracker/issues/83#issuecomment-335754673>). The
  vendor's fix for invisible commits was to tag them. INFERRED: a stash made on a
  detached `HEAD` that moved on is the same case as an amended base — its base
  leaves the graph, so its row does.

Neither vendor answer treats the behaviour as a defect, and no later release note
changes it (Mac RN and Win RN searched for stash, reflog, lost, unreachable,
orphan, dangling: no entry about drawing orphaned stashes).

**Evidence:** VENDOR. **Platform:** both.

## 2. What it looks like

**TrackerWin #1622** (USHOT, Windows, 18 Aug 2022, open): "Commit selection doesn't
change when the stash has no ref"; the body says the stash "loses the commit
reference" when history is rewritten. Its recording
(<https://user-images.githubusercontent.com/20267678/185386823-41ae01ac-4e36-412c-b6c6-9502e6f9d68b.gif>),
read frame by frame:

- Sidebar, Stashes section: `Stash 3`, `Stash 2`, `Stash 1` — all three listed.
- Commit list: three rows only — `stash@{0} Stash 3` and `stash@{1} Stash 2`, each
  on a short lane joining `master  Commit 1`, which is the only commit drawn.
  `stash@{2}` (`Stash 1`, `ce46652`, "On master: Stash 1") has no row, no lane,
  no edge, no marker; no commit of its old base is drawn.
- Clicking `Stash 1` in the sidebar fills the detail pane's Changes tab with that
  stash's diff (its author, `ce46652`, its message, `README.md`), while the commit
  list keeps its previous selection — the reported bug.

The same reading from a different angle: a user on **TrackerWin #1871** (Oct 2024)
diagnoses the stash-selection glitches as "one stash that is in the commit list and
one that is not", "either because one is filtered out or orphaned"
(<https://github.com/fork-dev/TrackerWin/issues/1871#issuecomment-2414938981>). The
vendor had linked #1622 as a similar issue; no vendor reply to the diagnosis.

**Evidence:** USHOT; USER. **Platform:** Windows; Mac matches by the vendor's
Tracker #1283 answer.

## 3. Hidden and filtered branches take their stashes with them

- **TrackerWin #2463** (Mar 2025, Windows 2.6.1): history appeared to stop two
  days back; branch and stash clicks did not jump — "The same is true for the
  stashes." Cause, found by the user: he had accidentally hidden the `feature/`
  folder. So a stash whose base only a hidden branch reaches is not drawn
  (USER, confirmed by its resolution; the vendor's own first guess was the commit
  cap, "Fork reads 50000 commits by default",
  <https://github.com/fork-dev/TrackerWin/issues/2463#issuecomment-2753608732>).
- **TrackerWin #1871** (above): "filtered out" is the user's other cause.
- **TrackerWin #2828** (Aug 2026): for a branch absent from the drawn topology the
  vendor now wants feedback — "Fork must beep when the branch is not visible in the
  current topology"
  (<https://github.com/fork-dev/TrackerWin/issues/2828#issuecomment-5227510103>).
  Said of branches, not stashes; whether it was built, and covers a sidebar stash
  click: OPEN.
- **Commit cap.** The walk holds the newest N commits — 25,000 then 50,000
  (VENDOR, Tracker #804), and "In Fork 2.41 we increased the commit limit from
  50,000 to 100,000 commits" (VENDOR,
  <https://github.com/fork-dev/Tracker/issues/2076#issuecomment-2015468825>, Mar
  2024); Windows still said 50,000 in Mar 2025. INFERRED: a stash on a base older
  than the cap is not drawn either, by the same rule. Not reported: OPEN.

**Evidence:** USER; VENDOR; INFERRED. **Platform:** both.

## 4. Index and untracked commits; commits only a stash reaches

- "Do not show stash revisions in commit list" (Mac RN 1.0.12, 8 Jul 2016), a
  month before stashes became a feature (Mac RN 1.0.16, 5 Aug 2016: "Stashes.").
  INFERRED: before 1.0.12 Fork's list included what a stash reaches, and it was
  removed deliberately.
- Every screenshot cited in `fork-refs-and-status-ui.md` Section 3, and the #1622
  recording, draws one row per stash and no `index on ...` or `untracked files on
  ...` row. The untracked commit is reached as the stash's third parent link in
  the Commit tab, "we need to navigate to [3rd parent of stash]" (USER, Tracker
  #898, <https://github.com/fork-dev/Tracker/issues/898#issuecomment-1247432066>,
  2022). The vendor will not show untracked stash files in the stash's diff "in
  the near future" (Tracker #898, Jul 2026, pointing at TrackerWin #2624).
- Commits reachable only from a stash — an orphaned base and its unreachable
  ancestors, and every index and untracked commit — are not rows in the commit
  list (Sections 1-2). The vendor groups them apart from the walk: search "only
  searches in the reachable commits" and "probably will not" search "reflog and
  stash revisions" (VENDOR, Tracker #1140,
  <https://github.com/fork-dev/Tracker/issues/1140#issuecomment-698806290>, Sep
  2020); from Fork 2.34 search covers reflog commits (same issue, Jan 2024), with
  stash revisions not mentioned.
- Whether a parent link in the Commit tab to such a commit (`stash^1` when
  orphaned, `stash^2`, `stash^3`) selects anything in the list, or only fills the
  detail pane: OPEN.

**Evidence:** RN; USHOT; USER; VENDOR. **Platform:** both.

## 5. Hide stashes, and the lost-commits mode

- **Hide stashes** ("View -> Hide stashes in the commit list", Mac RN 1.0.95, Win RN
  1.51, 2020) is described only as hiding stash rows (Tracker #540: stashes add
  "noise to the commit list"). Since stashes contribute no commits to the walk
  (Headline 2), INFERRED: toggling it changes no other row. No source states it.
- **Show lost commits (reflog)** (Mac RN 1.0.69, Aug 2018; Win RN 1.19, Jul 2018: "Ability
  to show and recover lost commits (Reflog)"), toggled by ⌘⇧. / Ctrl+Shift+. (Mac
  RN 2.61, Win RN 2.16). The vendor: "equivalent is `git log --all --reflog`"
  (<https://github.com/fork-dev/TrackerWin/issues/1307#issuecomment-918504470>,
  Sep 2021). Literally, `--reflog` includes `refs/stash`'s reflog, so every stash
  and its index, untracked and base commits would be walked; a user saw a
  dropped stash under this mode — "I can see my lost stash when I tick 'show lost
  commits'" (USER, TrackerWin #622, Jan 2020). An amended or rebased base is
  normally in `HEAD`'s reflog too, so this mode would plausibly bring an orphaned
  stash's base back and re-attach the stash. None of that is stated: whether
  Fork's lost-commit walk includes `refs/stash`'s reflog, whether stash rows
  re-attach to lost bases, and whether index commits then appear as rows, is
  OPEN. Hiding a branch, tags or remotes used to blank the whole lost-commit set
  (TrackerWin #825, #1307, #1514, #1558, fixed in Windows 1.94, Feb 2024; Mac
  Tracker #1528; Mac RN 2.39, Jan 2024, fixes the mode "when filter is enabled").
- Mac RN 1.0.18 (22 Aug 2016): "Updated revision list to show unreachable revisions
  with gray color". What "unreachable" meant (likely not reachable from `HEAD`),
  and whether it survives: OPEN; it is not evidence about stashes.

**Evidence:** RN; VENDOR; USER; INFERRED. **Platform:** both.

## 6. How Fork builds the graph, as far as published

- **Not `git log` as far as anyone has seen.** No issue in either tracker pastes a
  `git log` command Fork ran for the commit list; the activity-manager output users
  paste shows verbs (`git stash pop`, `git pull ...`, TrackerWin #622; Tracker
  #2232) and the vendor names the exact stash command, `git stash pop $stashname`
  (VENDOR, <https://github.com/fork-dev/TrackerWin/issues/622#issuecomment-577281688>),
  but never a log command.
- **Stashes are read in-process, as a set.** Mac `Fork.log` lines: "Failed to read
  stash objects for given oids: Failed to find object ..." (Tracker #1953, 2023;
  #2030, 2023; TrackerWin #2073, 2023 — a Mac user filed there) and "Failed to read
  stashes in '...': Cannot open object db" (Tracker #2075, May 2024), from
  `BtResultExtensions.swift`, i.e. Fork's own object-reading layer. One missing
  stash object made every stash vanish from the UI until the stash list was
  cleared or a hotfix shipped (2.37, Tracker #2030). INFERRED: Fork resolves the
  stash list separately and attaches each stash to a commit the walk produced —
  which is exactly why an orphaned base yields no row.
- **The walk is bounded and filtered before stashes attach.** The commit cap
  (Section 3), branch filters and per-ref hiding (Section 3) decide which commits
  exist; "Filter by active branch" filters the already-loaded set rather than
  rewalking (user analysis on Tracker #1401, which the vendor answered "You are
  absolutely right!" about the cap,
  <https://github.com/fork-dev/Tracker/issues/1401#issuecomment-863440334>).
- **Refs are read by a command named `GetReferencesGitCommand`** (log line "Cannot
  parse reference refs/prefetch/...", TrackerWin #2073), and Fork reads every ref
  namespace (Tracker #2675, `refs/jj/...`). Whether `refs/stash` is excluded from
  the walk's tips or never used as one: OPEN; the result is the same.

**Evidence:** USER (logs); VENDOR; INFERRED. **Platform:** Mac logs; Windows by
behaviour.

## 7. Secondary precedent

**`git log --graph --all` and `gitk --all`.** `--all` means "Pretend as if all the
refs in refs/, along with HEAD, are listed" (`git-rev-list(1)`), and "The latest
stash you created is stored in refs/stash; older stashes are found in the reflog"
(`git-stash(1)`). Verified here on git 2.56.0 with a fixture (stash made with `-u`
on `c2`, `c2` amended, a second stash made on the amended commit):

- `git log --graph --all`: draws `refs/stash` (stash@{0}) with its `index on`
  commit as a row; stash@{1} is not drawn at all, nor its base.
- With the orphaned stash as stash@{0}: draws the stash, its `index on` and
  `untracked files on` commits, and the old `c2` as a side branch off `c1` — the
  orphaned base and its unreachable ancestry walked in, (a).
- `git log --graph --all --reflog` (or `--all $(git stash list --format=%H)`): every
  stash, with every internal commit and every orphaned base.
- `gitk --all` passes `--all` to git (the shipped `/usr/bin/gitk` names "stash" only
  in two error messages), so it draws what `git log --all` walks.

**Sourcetree.** Atlassian's KB: the graph "is essentially SourceTree's version of the
command: git log --graph --all --date-order"
(<https://support.atlassian.com/sourcetree/kb/viewing-log-history-of-a-repository/>);
stashes are listed in the sidebar. If that holds literally, the latest stash and an
orphaned base of it are walked as in git. Whether Sourcetree excludes `refs/stash`
or draws stashes in the graph at all: OPEN (no primary source found).

**GitKraken.** Desktop: "Your stash appears in the Commit Graph", and "Stashes can be
hidden from the graph without being deleted"
(<https://help.gitkraken.com/gitkraken-desktop/stashing/>). The orphaned-base case in
Desktop (closed source): OPEN. GitKraken's GitLens Commit Graph, whose source is
public, runs `git log --all --stdin` with every stash's id and its second and
third parents on stdin ("Include the stash's 2nd (index files) and 3rd (untracked
files) parents",
<https://github.com/gitkraken/vscode-gitlens/blob/c773c2e4546d470877f2a38e56707fa5d69b6fc8/packages/git-cli/src/providers/stash.ts>),
remaps the internal commits into the stash's row and draws a stash with its first
parent only ("Always only return the first parent for stashes",
<https://github.com/gitkraken/vscode-gitlens/blob/24a2e9fcf6be657914e3e5eda051feca3e75bc8d/packages/git-cli/src/providers/graph.ts>).
So every stash is walked and an orphaned base and its ancestors appear, (a), with
the internal commits hidden.

**VS Code Git Graph** (mhutchie). In "Show All", it adds each stash's base to the walk,
"so that commits only referenced by stashes are displayed"
(<https://github.com/mhutchie/vscode-git-graph/blob/d7f43f429a9e024e896bac9fc65fdc530935c812/src/dataSource.ts>,
`getLog`), and draws the stash as a row whose one parent is that base; index and
untracked commits are not walked. With one branch selected it does not, and a stash
whose base was rebased away disappears (issue #743,
<https://github.com/mhutchie/vscode-git-graph/issues/743>, open since 2023) — Fork's
behaviour, filed there as a bug.

**Tower.** "Full History ... including local branches, remote branches, and tags"
(<https://www.git-tower.com/help/guides/commit-history/display-commits/mac>);
stashes have their own view: "select the Stash item in Tower's sidebar"
(<https://www.git-tower.com/help/guides/working-copy/stash/mac>). INFERRED: History
draws no stash, so the orphaned case does not arise in the graph.

**SmartGit.** In the Log, "ensure that the checkbox in front of the stash in the
Branches View is selected" and "You will see up to 3 commits"
(<https://docs.syntevo.com/SmartGit/Latest/HowTos/Workflows/How-to-examine-the-content-of-a-stash>):
a stash is a selectable root and its index and untracked commits are drawn. Its
changelog fixes a stash shown "though 'Stashes' node in Branches view was unselected"
(21.1) and one not "aligned with its base commit" under first-parent mode (20.1;
<https://www.syntevo.com/smartgit/changelog-21.1.txt>,
<https://www.syntevo.com/smartgit/changelog-20.1.txt>). INFERRED: a selected stash
seeds the walk, so an orphaned base is drawn; not stated: OPEN.

| Client | Orphaned base drawn? | Internal commits drawn? | Basis |
| --- | --- | --- | --- |
| Fork | No; stash row omitted, sidebar keeps it | Never | VENDOR, USHOT |
| `git log --all`, `gitk --all` | Only for stash@{0}; older stashes never drawn | Yes, for stash@{0} | verified, git 2.56.0 |
| GitLens Commit Graph | Yes, every stash | No (folded into the stash row) | source |
| VS Code Git Graph | Yes in Show All; no on one branch | No | source |
| SmartGit | INFERRED yes when the stash is ticked | Yes, up to 3 | docs |
| GitKraken Desktop | OPEN | OPEN | docs |
| Sourcetree | INFERRED yes for the latest stash, if literally `--all` | OPEN | KB |
| Tower | n/a; no stashes in History | n/a | docs |

## Bearing on the packet

Fork's rule is "a stash is decoration on a commit the ref walk reached" — the walk's
roots are refs and `HEAD`, never stashes, and a stash with nowhere to attach stays
in the sidebar alone. It is the cheapest rule (no extra roots, no extra commits),
matches what the vendor defends, and is the behaviour Git Graph's users file as a
bug when it happens in a single-branch view. Adopting it as Fork's standard costs a
selection path for a sidebar stash with no row (Fork's own open bug, TrackerWin
#1622) — the detail pane can show it while the list has nothing to select.
Deviating toward (a) is argued by git's own `--all` and by GitLens and Git Graph;
it would make a stash's orphaned base and its ancestry visible history rows that
no ref names. That is a design decision for the packet, not settled here.

## OPEN

1. Whether a click on an orphaned stash in the sidebar now gives any feedback (the
   "beep" the vendor proposed for absent branches, TrackerWin #2828), and whether
   TrackerWin #1622's stale selection is still present.
2. Whether "Show lost commits (reflog)" walks `refs/stash`'s reflog, re-attaches an
   orphaned stash to its base when that base is in `HEAD`'s reflog, or draws index
   and untracked commits as rows.
3. Whether "Hide stashes in the commit list" changes anything but stash rows (no
   source; INFERRED not).
4. Whether a stash whose base is older than the commit cap is undrawn (INFERRED
   yes; never reported).
5. What a Commit-tab parent link to a commit with no row (an orphaned `stash^1`,
   `stash^2`, `stash^3`) does in the commit list.
6. How Fork builds its walk — its tips, and whether `refs/stash` is excluded or
   never considered; no published `git log` invocation.
7. What Mac RN 1.0.18's grey "unreachable revisions" were and whether they remain.
8. Secondary: Sourcetree's and GitKraken Desktop's handling of an orphaned stash
   base, and whether Sourcetree draws stashes in its graph at all; SmartGit's walk
   for a ticked stash whose base no branch reaches.
