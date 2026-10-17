# Fork evidence inventory, for the UI review

Evidence record. Date: 2026-10-16. Commissioned by the `ui-review` packet's planning, before a
VM observation run, to say what Cairn already knows about Fork's UI, how well it knows it, where
`docs/design/ui.md` departs from it and whether those departures still stand in the code, and
what is still unknown. Read-only audit: no code was changed. Historical: never retro-edited.

## Method

Read in full: every `docs/research/*/fork-*.md`, both `what-clients-show.md`,
`staging-and-commit/review-ux-*.md`, `staging-and-commit/redesign-decisions-2026-10-10.md`,
`docs/design/ui.md`, `docs/design/feature-inventory.md`. Read for their Fork tables: the four
`staging-and-commit/brief-*.md` (they carry Fork facts the evidence files do not) and
`docs/systems/diff.md`, "The diff view" (it records the user-supplied Fork captures of
2026-10-04). Open issues: `gh issue list --state open --limit 200` (107 issues), read for the UI
ones. Code spot-checked by grep and reading in `crates/cairn-ui/src` and `crates/cairn-app/src`.

Seven VM screenshots in `~/VMs/fork-win10/shots/` were looked at directly for this inventory
(`lc1`, `cb1`, `amd3`, `fork1`, `ps1`, `uv0`, `uv2`, `uv2f`). Three of them (`uv0`, `uv2`,
`uv2f`) are dated 2026-10-16 and show this packet's fixture (`fixture-recipe.md`) open in Fork,
so an observation run appears to be under way; nothing of it is written up yet. Facts read from
those shots are graded FIRST-HAND (unrecorded) below and must be confirmed by that run's
write-up before a decision rests on them.

### Grades

| Grade | Meaning |
| --- | --- |
| **FH** | FIRST-HAND: observed in the Windows 10 VM, Fork for Windows 2.21.1, written up in `fork-observed-2026-10-10.md`, `fork-observed-nested-repo-2026-10-10.md` or `fork-activity-result-lines-2026-10-11.md`, screenshot named. |
| **FH-shot** | FIRST-HAND, unrecorded: visible in a VM screenshot on disk, read for this inventory, not yet in any write-up. |
| **FH-user** | A Fork capture the user supplied (2026-10-04, Windows), cited in `docs/systems/diff.md`; the image is not in the repository. |
| **PV** | PRIMARY, vendor: release notes, `fork-dev/Docs` (shortcut lists, FAQ, last changed 2023-10-04), the blog, a vendor tracker comment, a vendor screenshot. |
| **PU** | PRIMARY, user: a user screenshot or report in Fork's own tracker, quoted with its issue. Weaker than PV; dated. |
| **S** | SECONDARY: INFERRED, MEASURED from a screenshot of unknown scale or profile, a third party, a brief's claim with no cited source, or memory. |

Every FH fact is Windows, 2.21.1, light theme, 1024x768. Almost every PV/PU fact is older and
Mac-first; Mac and Windows are separate code bases (AppKit, WPF) whose features diverge for
years (`fork-detail-and-diff-ui.md`, "How Fork is built").

### Sources, by short name

| Short | File | Date | Method | Best grade |
| --- | --- | --- | --- | --- |
| OBS | `staging-and-commit/fork-observed-2026-10-10.md` | 2026-10-10 | VM | FH |
| NEST | `staging-and-commit/fork-observed-nested-repo-2026-10-10.md` | 2026-10-10 | VM | FH |
| ACT | `staging-and-commit/fork-activity-result-lines-2026-10-11.md` | 2026-10-11 | VM | FH |
| SC | `staging-and-commit/fork-staging-and-commit.md` | 2026-10-07 | documentary | PV/PU |
| CB | `staging-and-commit/fork-create-branch-evidence.md` | 2026-10-09 | documentary | PV/PU |
| MA | `staging-and-commit/fork-merge-and-amend-evidence.md` | 2026-10-09 | documentary | PV/PU |
| BR | `staging-and-commit/brief-*.md` (four) | 2026-10-10 | documentary | PV/PU |
| RS | `refs-and-status/fork-refs-and-status-ui.md` | 2026-10-05 | documentary | PV/PU |
| DH | `refs-and-status/fork-deep-history.md` | 2026-10-05 | documentary | PV/PU |
| USB | `refs-and-status/fork-unreachable-stash-base.md` | 2026-10-05 | documentary | PV/PU |
| DD | `diff-engine/fork-detail-and-diff-ui.md` | 2026-09-17 | documentary, measured | PV/PU |
| SK | `diff-engine/fork-shortcuts.md` | 2026-10-03 | documentary | PV |
| WD | `diff-engine/what-clients-show.md` | 2026-09 | documentary; its Fork claims corrected by DD | PV |
| WH | `history-graph/what-clients-show.md` | 2026-09-16 | documentary, measured | PV/PU |
| SD | `docs/systems/diff.md`, "The diff view" table | 2026-10-04 | user captures | FH-user |
| SHOT | `~/VMs/fork-win10/shots/<name>.png` | 2026-10-10/16 | VM, unrecorded | FH-shot |

## 1. Fork facts on record, per surface

Each row is a fact Cairn holds about Fork, digested; the source's section has the citations and
dates. "Flag" rows mark a claim made as Fork's in Cairn's docs that has no source on record, or
that first-hand evidence contradicts.

### 1.1 Window shell and title bar (status box)

| Fact | Source | Grade |
| --- | --- | --- |
| Native OS title bar reading "Fork", OS window controls, menu bar inside the window (Windows). | SHOT lc1, uv2 | FH-shot |
| A central status box: repository name (`*` when dirty), a branch glyph and the current branch as a checkout dropdown, the branch's behind/ahead counts. | RS §9 (Mac carousel, blog Win 1.28, Tracker #1938) | PV/PU |
| In 2.21.1 at 1024 px the box shows the name over "⑂ main" and no counts; the counts are badges on Pull and Push. | SHOT lc1, cb1, amd3 | FH-shot |
| Counts on Pull/Push since Mac 2.62 / Win 2.16 (2026). | RS §9 | PV |
| Clicking the box opens the Activity Manager. | SC §7; OBS; ACT | FH |
| While an operation runs: spinner, its name over its progress line, and an × at the box's right edge; a cancel reads "Canceling...". | ACT ("Long output", §8: ar22z, ar27) | FH |
| No banner or standing state for a stale `index.lock`. | OBS (lk1) | FH |
| A banner under the toolbar can appear: "Consider adding a .gitignore file to your repository" with `Add .gitignore...` and `Close`. | SHOT lc1, uv2 | FH-shot |
| Detached HEAD text in the box. | RS OPEN 12 | none |
| A first-run "Welcome to Fork" dialog (User Information: name, email, default source folder; Finish, Cancel). | SHOT fork1 | FH-shot |

### 1.2 Toolbar

| Fact | Source | Grade |
| --- | --- | --- |
| Left to right (Win 2.21.1): a sidebar toggle, Quick Launch, Fetch, Pull, Push, Stash ▾ · status box · Branch ▾ · Open in ▾, Console, Appearance ▾, Work(space) ▾; a smiley (feedback) by the window controls. | SHOT lc1, uv2 | FH-shot |
| Fetch opens a dialog: "Fetch" / "Fetch latest changes from remote repository", `Remote: origin`, "Fetch all remotes" (unticked). Runs `git fetch --prune origin`. | ACT §7 | FH |
| Branch opens Create Branch. | OBS | FH |
| A background fetch every 10 minutes, listed under Background. | RS §7; ACT §7a | PV; FH |
| Stash button with a dropdown of recent stashes and `Save Snapshot...`. | SC §6 | PV |
| "Filter by active branch" button and ⌘⇧A / Ctrl+Shift+A. | RS §2 | PV |
| Appearance → Commit List Layout → Horizontal / Vertical (Windows only). | DD F1 | PV |
| Pull and Push dialogs, Quick Launch, Open in, Console, Workspace: behaviour. | — | none |

### 1.3 Menu bar

| Fact | Source | Grade |
| --- | --- | --- |
| Windows menus: File, View, Repository, Window, Help. | SHOT lc1 | FH-shot |
| View → Show Lost Commits (Reflog); Hide Remote Branches in Commit List; Hide stashes in the commit list; Collapse All Merges; Refresh ⌘R. | SC §7; RS §2, §3, §7; WH F5 | PU/PV |
| Repository → Benchmark. | RS §7; DH §2 | PU |
| The contents of File, Window, Help and the rest of View and Repository. | — | none |

### 1.4 Repository tabs

| Fact | Source | Grade |
| --- | --- | --- |
| A strip of tabs under the toolbar, one per open repository, each with a leading dot, `+` at the right. | SHOT lc1, uv2 | FH-shot |
| A dirty marker on the tab (`name*`, later badges / dots); it ignores untracked files, refreshed on focus; a preference turns it off. | RS §4 (Mac RN 1.0.74, 2.45; TrackerWin #2514, #553) | PV |
| Checking out a branch held by another worktree opens that worktree's tab (Win 2.2). | RS, Corrections | PV |
| Opening, closing, reordering, a tab's context menu, the repository manager. | — | none |

### 1.5 Sidebar

| Fact | Source | Grade |
| --- | --- | --- |
| Top to bottom: repository name (gear menu), `Local Changes (N)`, `All Commits`, a tab strip (refs, search; Windows adds PRs), `Filter`, then Worktrees, Pinned, Branches, Remotes, Tags, Stashes, Submodules. | RS §5 | PU |
| Branches, Remotes, Tags, Stashes, Submodules are listed even when empty; Worktrees and Pinned are absent when empty. | SHOT lc1, uv2 | FH-shot |
| The count is the entries `git status` lists, omitted when clean; whether a file staged and unstaged counts twice is OPEN. | RS §6 | PU, S |
| Folders split at `/`, drawn with folder glyphs. | RS §5; SHOT uv2 | PV; FH-shot |
| Ahead/behind as arrows, ahead first on Windows (`1↑ 1↓`). | RS §5 (measured); SHOT amd3, uv2 | PV/S; FH-shot |
| Upstream-state icons: local-only greyed, a gone upstream with a warning triangle. | RS §5 (blog Win 1.48) | PV |
| Current branch: check mark in place of the icon, bold. | RS §5; SHOT | PV; FH-shot |
| A remote whose forge Fork does not know (a local path) carries a cloud glyph; a known forge's logo otherwise. | SHOT lc1, uv2; RS §1 | FH-shot; PV |
| Stashes listed by message with the `On <branch>:` prefix dropped ("WIP on parser"). | RS §3 (INFERRED); SHOT uv2 | S; FH-shot |
| Click selects the ref's commit and scrolls to it, never filters. | WH F2 (single user report, corroborated) | PU |
| ⌘-click two refs compares them; double-click checks out / tracks. | RS §5 | PV |
| A filter funnel on hover beside a ref; poorly discovered. | WH F3 | PU |
| The filter box filters every section at once. | RS §5 | PV/PU |
| Arrow keys and Tab between sidebar and list (2026). | RS §5 | PV |
| A ref whose commit is past the cap: nothing happens. | DH §3 | PU/PV |
| Whether expansion is remembered across launches. | issue #72 | none |
| Context menus on a branch, remote, tag or stash. | — | none |

### 1.6 History list

| Fact | Source | Grade |
| --- | --- | --- |
| Opens on All Commits, always; every ref walked. | WH F1; RS §2 | PV |
| Columns: graph and subject in one column, Author with avatar, 7-hex id, Date (`16 Oct 2026 12:12`, local). | WH F9; SHOT uv2 | PV; FH-shot |
| No column-heading row is drawn above the list. | SHOT uv2 | FH-shot (verify) |
| Chips left of the subject, after the graph: rounded, outlined, lane-tinted; tags indigo with a tag cap; a remote's forge cap; the current branch `✓`; compact labels merge a branch and its upstream at one commit; no wrap, elision or `+N`. | RS §1 | PV/PU |
| The stash chip carries a box glyph; a remote chip a cloud cap when the forge is unknown. | SHOT uv2 | FH-shot |
| `HEAD`'s row bold; commits the current branch does not reach drawn grey. | RS §1; WH F1; SHOT uv2 | PV; FH-shot |
| Push/pull dots before the subject (blog Mac 1.0.73, Win RN 1.30). | RS §1 | PV (not seen in uv2) |
| A stash is a row of its own on a short lane to its base, in date order; several on one commit each get a lane. | RS §3 | PU, S (INFERRED order) |
| A stash whose base the walk does not reach is not drawn; sidebar only. | USB H1 | PV |
| No working-tree row, by vendor decision. | WH F8; RS §4 | PV |
| A merge node draws a collapse chevron; merges collapse by click or ←/→. | WH F5; SHOT uv2 | PV; FH-shot |
| Lanes assigned by recency of the branch's update. | WH F7 | PV |
| Lane pitch 22-24 px in vendor screenshots. | WH F6 | S (scale unknown) |
| Capped at the newest 50,000 (Windows) / 100,000 (Mac) commits; loaded whole per refresh. | DH | PV |
| Lost commits: text in lighter grey, lanes, edges and chips in colour. | CB §5 (2018 GIF) | PV |
| No horizontal scroll of the graph; cropped. | WH F5 | PV |
| Search (⌘F) is a separate results list, never a filter. | WH F4 | PV |
| A selected row: filled accent blue, white text. | SHOT uv2 | FH-shot |
| A commit row's context menu (Mac): the ref's submenu; New Branch… ⇧⌘B; New Tag… ⇧⌘T; Interactive Rebase; Reset '<branch>' to Here…; Checkout Commit…; Cherry-pick Commit…; Revert Commit…; Save as Patch…; Compare to Local Changes; Copy Commit SHA ⌘C. | CB §1 (Tracker #1683) | PU |
| Enter opens a commit in its own window; Space a quick-look popup (2026). | DD F1 | PV |
| Row height, chip metrics, lane colours, the Windows context menu, keyboard beyond ⌘2 / ⌘0. | — | none |

### 1.7 Detail pane, Commit tab

| Fact | Source | Grade |
| --- | --- | --- |
| Below the list, behind a splitter; collapsible (⌘D / Ctrl+Shift+D, 2026); Windows can put it on the right. | DD F1 | PV |
| Tabs Commit / Changes / File Tree, no counts; Windows underlined text tabs, Mac a segmented control; at the strip's right an open-in-window icon and a collapse icon. | DD F2; SHOT uv2 | PV; FH-shot |
| Mac remembers the last tab (2.23); Windows: OPEN. Mac ⌘⌥1/2/3; Windows chords: OPEN (#57). | DD F2; SK | PV |
| Header: AUTHOR and COMMITTER columns, each an avatar, name, lighter email, full timestamp with seconds and zone; REFS chips; full 40-hex SHA; PARENTS as short links; a rule; subject larger; body. | DD F3 | PV/PU |
| COMMITTER is omitted when it equals the author; the avatar is initials on a coloured square; the email has a ▾; the date reads `16 Oct 2026 12:12:33 +01:00`; REFS draws a plain `main` chip. | SHOT uv2f | FH-shot (settles DD F3's OPEN) |
| Author date in details, committer date in the list. | DD F3 | PV |
| No diffstat, signature state or "contained in". | DD F3 | PV |
| Files listed under the message, each expanding its diff in place; `Expand All` right-aligned, becoming `Collapse All`; collapsed by default by choice; Windows rows carry a disclosure. | DD F4; SHOT uv2f (chevron, M badge, file icon) | PV; FH-shot |
| In-place diffs have no bar; the Changes tab's settings govern them; each scrolls sideways on its own. | DD F4 | PU/PV |
| A stash shows two or three parents and its first-parent diff, untracked files left out. | DD F9 | PU/PV |
| While two commits are selected, Commit and File Tree are disabled (Windows). | DD F7 | PU |

### 1.8 Detail pane, Changes tab

| Fact | Source | Grade |
| --- | --- | --- |
| A one-line summary (avatar, author, short SHA, date, subject) over a file list (left) and one file's diff (right), with a splitter. | DD F2, F5 | PV |
| File list toolbar: filter, eye (quick look, Space), layout menu (Tree / List / Combined List). | DD F5 | PU/PV |
| First file selected; another commit re-selects the first file. | DD F5 | PV/PU |
| Several files selected: only the first selected file's diff. | DD F5; BR (F9) | PU |
| Two commits: the header names both, one per line, with a swap at its right; Windows orders older to newer. Tip against tip. | DD F7 | PU/PV |
| A file's context menu holds External Diff (⌘D / Ctrl+D), History, Blame, Show in File Tree, reset to a revision, Save As. | DD F10 | PV |
| The menu as drawn on Windows 2.21.1; the filter's words; rename display. | — | none |

### 1.9 Diff view

| Fact | Source | Grade |
| --- | --- | --- |
| Bar: previous/next chevrons left; path centred, directory grey, name normal; right: ignore whitespace `⎵`, invisibles `¶`, wrap, fewer lines, more lines, entire file, side-by-side; on = accent glyph; tooltips "Ignore whitespaces", "Decrease/Increase number of visible lines", "Show entire file". | DD F10; SHOT lc1 | PV; FH-shot |
| Unified by default; side-by-side a header toggle in both views; Space opens a large side-by-side quick look with its own state. | DD F11 | PV |
| Two number columns and a separator; `-+` marks an opt-in preference; red/green change marks on the scrollbar. | DD F12 | PV |
| Hunk header: git's raw `@@` line, grey, same row height, no buttons. | DD F13 | PV/S (measured) |
| Context: one global count, default 3, ±1; entire-file a toggle; no per-gap expansion. | DD F14 | PU/PV; S (step) |
| Ignore whitespace is `--ignore-all-space` and hides changes silently. | DD F15 | PV |
| Intra-line emphasis always on, no switch. | DD F16 | PV |
| Syntax highlighting (Windows since 2.1), wrap (off in side-by-side), tab width, diff font, ⌘F find per file. | DD F17 | PV |
| "Changes are too large to display" with `Load Diff`; a 2,048-character line trips it on Windows. | DD F21 | PU/PV |
| Binary, image, LFS and submodule views of their own. | DD F22 | PV |
| Hover a chunk: outline and floating `Stage`, `Discard…` (Windows; the captions never count lines), `Unstage` in the staged diff; drag-select text for lines. | DD F23; OBS (ln1) | PV; FH |
| A finished line selection is dropped, silently, when the window loses and regains focus. | OBS (ln2) | FH |
| In the diff with no lines selected Return does nothing; Ctrl+S stages the whole selection of files. | OBS (lc3, lc4) | FH |
| A failed chunk stage proposes turning ignore whitespace off (2024). | DD F23 | PV |
| Gutter and number metrics, tint start, side-by-side tint across the column (Windows). | SD | FH-user |
| Row pitch 17 pt and Menlo 11 on the Mac. | DD F24 | S (measured) |

### 1.10 Local Changes lists

| Fact | Source | Grade |
| --- | --- | --- |
| Unstaged over Staged, each headed by its name and a `Stage` / `Unstage` button (greyed with nothing selected there); diff to the right; commit box under the diff. | RS §6; SC §1; SHOT lc1 | PV; FH-shot |
| Above the lists, one row: filter, a double chevron (Stage All), eye, layout menu. | SC §1 (Mac); SHOT lc1 (Windows: the chevron is in this row, not in the Unstaged header) | PV; FH-shot |
| Flag: BR F22 says that on Windows the chevron sits "only in the Unstaged header" (USHOT TrackerWin #1348, 2021); lc1 shows it in the row above the lists. | BR; SHOT lc1 | conflict |
| Badges: yellow `M`, green `+` (untracked and added alike), yellow warning for a conflict; letters since late 2022; no hover hints. | RS §6 | PU/PV |
| Tree by default, List, Combined List; untracked shown, Hide Untracked remembered; Show Ignored ⌘⇧. / Ctrl+Shift+. | RS §6 | PU/PV |
| Five routes, each a toggle: double-click, Enter / Ctrl+Shift+S (Return / ⌘S), drag between lists, the header button, the context menu. | SC §1 | PV/DOCS |
| Windows: Shift held turns the header buttons into Stage All / Unstage All, enabled whatever is selected since 2.7. | BR (F20, TrackerWin #2429, #2466) | PU (never observed in the VM) |
| Ctrl-click adds a row; the diff shows the file clicked first; nothing says "k of n". | OBS (lc2) | FH |
| After a stage, the selection goes to the row that slid into the first acted row's place; after a commit made outside, to the row that took the vanished one's place. | OBS (lc4, lc5) | FH |
| There is always a selected file when files exist. | SC §1 | PV |
| Context menu (Windows, 2022): Open, External Diff, Show in File Explorer · Blame/Timeline…, History… · Stage Enter, Discard changes… Delete · Stage All · Ignore › · LFS › · Stash 1 File…, Save as Patch… · Copy Full Path. The staged list offers Unstage and Unstage All; the rest is UNVERIFIED. | SC §1 (VSHOT TrackerWin #326) | PV; PU |
| A selected folder acts on everything under it. | SC §1 | PV |
| An untracked nested repository is deleted as a folder, with no warning; on Windows Fork half-deletes it and fails. | NEST | FH |
| Discard runs `git checkout-index --index --force --stdin -z`; stage `git add --force --verbose --pathspec-from-file=- ...`; unstage `git reset -q --no-refresh HEAD --pathspec-from-file=- ...`. | ACT §1, §2, Stage | FH |
| A discard of lines makes no Activity row. | ACT §2 | FH |

### 1.11 Commit box

| Fact | Source | Grade |
| --- | --- | --- |
| "Commit subject" with `≡` (Recent Commit Messages) at its right, "Description" under it, `Amend` at the left, the commit button at the right as a split button with ▾. | SC §5; SHOT lc1, cb1 | PV; FH-shot |
| The button reads `Commit` with nothing staged, `Commit 2 Files` with two, disabled with an empty subject. | SC §5; SHOT lc1, cb1 | PV; FH-shot |
| Counter: characters left before 50 (grey), negative past it, red past 70; configurable. | SC §5 | PV |
| Ruler, wrap paragraph at ruler, grows as typed, spell check, monospace option. | SC §5 | PV |
| Recent messages: the latest 10 on the current branch; ↑/↓ cycle them; selecting fills both fields. When ↑/↓ recall is UNKNOWN. | SC §5; BR | PV |
| `commit.template` pre-fills; draft kept across views and per worktree (Win 2.23). | SC §5 | PV |
| Commit and Push: Windows the button's dropdown, Ctrl+Shift+Enter; Mac ⌥. | SC §5 | PU/PV |
| Amend fills `HEAD`'s message and lists its files in Staged; a pushed commit gets an inline note cut by the button ("Amending commit that has already been pus…"); the button reads "Amend Last Commit"; it amends at once with no dialog. | OBS (amd1-3) | FH |
| Mac warns of nothing on a pushed amend. | ui.md | **Flag: unsourced** (SC §5: Mac OPEN) |
| A failing hook opens Git Error with `Skip pre-commit hooks and commit`. | ACT §9; SC §5 | FH; PV |
| A running commit's cancel is the × on its Activity row and in the status box; none in the box. | ACT ("Long output"); BR | FH; S |
| Commit runs `git commit --file "<gitdir>\COMMITMESSAGE"`. | ACT §3 | FH |
| A merge in progress: Commit enabled with nothing staged; `MERGE_MSG` pre-filled; `# Conflicts:` lines committed (open bug). | MA §1-2; ui.md | PU/PV |
| No author override, no up-front `--no-verify`, no empty commit; sign-off and GPG are repository settings. | SC §5 | PV |
| During a rebase, `git am` or a sequence, Fork "pre-fills and commits". | ui.md | **Flag: unsourced** (evidence covers merge, revert, cherry-pick only) |

### 1.12 Dialogs

| Fact | Source | Grade |
| --- | --- | --- |
| Discard, files: "Discard changes" / "Do you want to discard all your changes in the selected files?" / `Discard Changes in 2 Files` (focused) / `Cancel`; one file: `Discard` / `Cancel`; no "can't undo". | NEST (n7); ACT §2 | FH |
| Discard, lines: "Do you want to discard all your changes in the selected lines?" / `Discard 2 Lines` (focused) / `Cancel`; one line `Discard Line`. | OBS (dc1); ACT §2 | FH |
| Older Windows added "You can't undo this action."; Mac's text differs. | SC §2 | PU/PV |
| Git Error: Fork icon with a yellow ⚠, "Git Error", "An unexpected error occured while performing the git request" (sic), a mono box with the bold command and git's output, extra buttons at the left (`Skip pre-commit hooks and commit`, `Remove .git/index.lock`), `Close` at the right. A cancel ends in it too ("[internal] Cancelled", Close only). | ACT §6, §8, §9 | FH |
| An `Error Details:` caption in the Git Error dialog. | SC §5 (2019 VSHOT) | PV; not in the 2.21.1 write-ups (verify) |
| Create Branch: "Create Branch" / "Use '/' as a path separator to create folders" / "Create branch at: ⑂ main" / "Branch name:" (placeholder "Enter branch name") / "Check out after create" / `Create` (disabled until a name) or `Create and Checkout` / `Cancel`; ticked over changes, "Local changes:" Don't change, Stash and reapply, Discard with ⚠; Discard asks nothing and runs `git checkout --no-track -b <name> <commit> --force`. | OBS (cb2-4); SHOT cb1 | FH |
| A refused name: reason beside the buttons with a ⚠, the button disabled. | CB §2 | PU |
| `Remove .git/index.lock` removes it with no confirmation and makes no Activity row; the write is not retried. | OBS (lk3); ACT §6 | FH |
| Fork retries a write that hits the lock ("Repository is locked. Retrying..."). | OBS (lk2); ACT §6 | FH |
| Button order: primary first, Cancel last, on Windows; Cancel first on the Mac. | OBS; CB §2 (Mac: Tracker #263, older) | FH; PU |
| Esc closes Git Error; nothing is its default. | SC §5 | PV |
| Stash, Apply Stash, Add Pattern to .gitignore, Checkout with Local Changes. | SC §4, §6; CB §3 | PU/PV |
| Pull, Push, Fetch's buttons, the chunk discard's wording, the submodule discard, the credential prompt (Fork leans on Git Credential Manager). | — | none |

### 1.13 Context menus

On record: the unstaged file's (Windows 2022, Mac 2024; SC §1), the commit row's (Mac; CB §1),
the Changes tab file's items without a capture (DD F10), the diff text's (Copy as Patch, reveal
in editor, file history of the selected lines; DD F10). No record at all for: the staged file's
full menu, a sidebar branch, remote, tag, stash or submodule, a repository tab, a ref chip, the
Commit tab's file rows, or any Windows 2.21.1 menu as drawn.

### 1.14 Activity popover (Activity Manager)

All FH (ACT, OBS) unless noted.

- Tabs `All / User / Background`; an automatic fetch is Background only.
- Each row: bold name left, local time `HH:MM` right, Fork's result line under it; ⚠ (yellow)
  before the time on a failure and on a cancel; a running row has a spinner and × in place of
  the time.
- Names: "Stage 2 Files", "Unstage 1 File", "Discard Changes", "Commit 2 files" (lower case),
  "Amend", "Create branch 'side'", "Fetch 'origin'". Result lines: "staged", "unstaged",
  "discarded", "Switched to branch'topic'" (sic), "Already up to date", "Fetched 1 branch",
  "stage failed", "canceled"; none for Commit, Amend or a Create Branch without checkout.
- Right pane: the name, then "succeeded / failed / canceled / running" with
  `d Mon yyyy HH:MM:SS` in local time; `$ git …` bold mono; git's output, and for stage, unstage
  and discard Fork's own Old/Requested/Response/Refreshed/Result trace.
- A running entry opens at the end of what has arrived and does not follow; a finished one opens
  at the top; 500 lines were all kept.
- Fork bugs seen: a commit failed by its hook reads "succeeded" with the stuck result "1 file...".
- Not persisted across launches; a verbose-output preference (SC §7, PV). Pin, colour,
  durations are open requests (PV).

### 1.15 Preferences

Nothing first-hand. On record (PV/PU): the diff font (Mac face and size, Windows size only),
`Show -+ marks`, `Disable syntax highlighting` (Preferences → General → Diff View, a Mac USHOT);
the tab dirty indicator; compact labels (Mac); push automatically on commit; the subject limit;
`Check out after create` remembered; per-repository Commit Template, sign-off and GPG; bug
tracker link patterns; verbose git output; tab width (location OPEN); background fetch on/off;
internal or system git; whole-UI zoom Ctrl+= / Ctrl+- (Windows); `MaxCommitCount` only in
`settings.json`. The window itself — its tabs, layout and wording — is not on record.

### 1.16 Keyboard

From the vendor lists (DOCS, last changed 2023) unless noted (SK, DD F18, SC, RS):
⌘1 / Ctrl+1 Local Changes (again: focus the subject); ⌘2 / Ctrl+2 All Commits (again: jump to
`HEAD`); ⌘0 / Ctrl+0 reveal `HEAD`; ⌘F / Ctrl+F search, Return / F3 next; ⌘P / Ctrl+P Quick
Launch; ⌘R / F5 refresh; ⌘⇧A / Ctrl+Shift+A filter by active branch; ⌘⌥1/2/3 detail tabs (Mac;
Windows unknown, #57); ⌘↑/↓ / Ctrl+↑/↓ previous / next change (2026); ⌘- / Ctrl-click a second
commit; Space quick look; ⌘D / Ctrl+D external diff; ⌘D / Ctrl+Shift+D collapse details (2026);
Return or ⌘S / Enter or Ctrl+Shift+S stage; ⌥⇧⌘S / Ctrl+Alt+Shift+S stage all; ⌫ or ⇧⌘D /
Backspace, Delete or Ctrl+Shift+D discard; ⌘Return / Ctrl+Enter commit; Ctrl+Shift+Enter commit
and push (Windows menu); ⇧⌘H / Ctrl+Shift+H stash, ⌥⇧⌘H quick stash; ⇧⌘B / Ctrl+Shift+B new
branch; ⇧⌘T new tag; ⌘C copy SHA; ⌘⇧. / Ctrl+Shift+. lost commits (commit list) or ignored
files (Local Changes); ←/→ collapse merges; Tab cycles list → staged → subject → description.
⌘⇧A toggling Amend on the Mac is a user report (PU). First-hand: only Ctrl+S staging from the
diff and Return doing nothing there (OBS). Fork scopes chords to the focused view (SK, PV).

### 1.17 Palette, typography, density

| Fact | Source | Grade |
| --- | --- | --- |
| Diff colours, dark and light, measured (e.g. dark removed `#633F3E`, added `#3A5C3F`, emphasis `#9F4247` / `#388442`, text `#DDDDDD`). GitHub's diff colours, per the vendor. | DD F25, F26 | S (measured) / PV |
| Mac's active chunk outline is the system accent; Windows follows the Windows accent (1.49). | DD F25, F27 | S; PV |
| Label fill is a tint of the lane colour; tags indigo. Values not measured. | RS §1, OPEN 13 | PU/PV |
| Default theme on a fresh Windows install: light. | SHOT lc1, uv2 | FH-shot |
| Mac diff font Menlo 11; Windows sets size only; the Mac UI does not scale. | DD F17 | PV/S |
| Diff row pitch 17 pt on the Mac, hunk rows the same. | DD F24 | S |
| UI typeface, lane colours, history, sidebar and file-list row heights, chip size. | — | none |

### 1.18 Claims stated as Fork's without a source, or contradicted

1. ui.md, the running-commit row: "Fork's cancel is only in the Activity Manager." Contradicted
   (FH, ACT ar22z): the status box carries an × too.
2. ui.md, the amend row: "Fork for macOS says nothing." No source (SC §5 leaves Mac OPEN).
3. ui.md, the remote-glyph row: "Fork shows a forge icon on every one." Contradicted (FH-shot
   lc1, uv2): a remote with no known forge gets a cloud, as Cairn's does.
4. ui.md, the worktrees row: a branch held by another worktree "carries a chip as in Fork". The
   evidence is a sidebar icon (Mac RN 2.59), not a chip on the graph.
5. ui.md: "Fork resolves per file and opens a resolver for the rest." No research file holds it.
6. ui.md: Fork's lost-commit mode "reaches every reflog, as `git log --all --reflog` does". The
   vendor's analogy, not a quoted command (USB H6).
7. ui.md: during a rebase, `git am` or a sequence "Fork pre-fills and commits". Only merge,
   revert and cherry-pick pre-fill is evidenced.
8. ui.md, "The sidebar, the toolbar": the title bar's counts "as Fork's central status box does".
   True of 2019-2023 captures; in 2.21.1 at 1024 px the counts are on Pull and Push (FH-shot).
9. ui.md, the skip-hooks row: "hooks git finds by configuration Fork misses (Tracker #948)". The
   detection gap is a 2019 vendor statement in Tracker #776; its current state is UNVERIFIED.
10. ui.md, "Kept": "A right-click selects the row it lands on (... Fork for Windows)". No Fork
    source on record.
11. ui.md, "Kept", dialog order on macOS: rests on one older Mac capture (Tracker #263).
12. ui.md, the Show Lost Commits row: the check box sits in the "Graph and subject" heading;
    Fork appears to draw no heading row at all (FH-shot uv2), so the heading row is itself an
    unrecorded difference.
13. ui.md, the double-chevron row and BR F22 place Windows' chevron in the Unstaged header; lc1
    shows it in the row above both lists.
14. ui.md, "Messages": Fork's imperative written "Stage 2 files". FH: "Stage 2 Files"; the code
    (`LocalWrite::name`) already follows FH.
15. The Windows Shift-for-Stage-All rule (ui.md, Staging) rests on user reports only and was
    never seen in the VM.

## 2. The deviations ui.md records, and whether each still holds

"Holds" asks whether the stated reason is still true and the code still does what the row says.

| # | Deviation (ui.md "What changes, and why") | Stated reason | Code today | Verdict |
| --- | --- | --- | --- | --- |
| 1 | One confirmation dialog for whatever cannot be undone or rewrites shared history; recoverable asks nothing; words from the engine. | `cairn.md`; `engine.md` seal | `cairn_ui::dialogs::confirm`, `Consequence::needs_confirming`; Create Branch's Discard is confirmed by its radio and press, no dialog (`dialogs/create_branch.rs`, user decision 2026-10-10) | Holds, but the row's "always gets the one confirmation dialog, whichever route" is no longer literal: Create Branch is the stated exception. Reword. |
| 2 | Focus on Cancel; Escape goes back. | `cairn.md` | `dialogs/confirm.rs` (focus on Cancel), `answer_button.rs` | Holds; Fork's focused Discard is FH. |
| 3 | One sentence frame, Show files, Title Case. | seal | `consequence.rs` (`UNDONE`, one sentence), Show files list in `confirm.rs` | Holds; Fork's 2.21.1 text is FH (sentence case, no "can't undo"). |
| 4 | A mixed discard discards what it can and says what is left; Fork asks twice. | write verbs | `local_changes/menu.rs` (`Kept`, `NoDiscard`) | Holds. Fork side is a 2019 vendor note, never observed. |
| 5 | One home per message; Fork calls everything "unexpected". | `concurrency.md`; `processes.md` | `operations/ending_dialog.rs`, `dialogs/git_error.rs` (`couldnt_title`); "Nothing is drawn under the lists" (`local_changes/pane.rs`) | Holds; Fork side FH. |
| 6 | No text a person needs is cut. | `cairn.md` | Sentences wrap; every remaining `max_lines(1)` is a row label. One exception: a path in the confirmation's Show files rows is `max_lines(1)` with no ellipsis (`dialogs/confirm.rs`), so a long path is clipped. | Holds for sentences; Show files paths break it. |
| 7 | Activity quotes the confirmed prompt, gives a duration, offers "Show in Lost Commits"; UTC. | seal; Recovery | `title_bar/activity_popover.rs` (`CONFIRMED_CAPTION`, `SHOW_IN_LOST_COMMITS_CAPTION`), `operations/activity.rs` (UTC) | Holds. UTC waits on a timezone decision (#103); "Show in Lost Commits" is offered where git logged nothing (#116). |
| 8 | Ignore Whitespace disabled in Local Changes. | `diff.md` | `diff/header.rs` (`exact`) | Holds. |
| 9 | A visible double chevron in each list's header, ⇊ and ⇈. | `platform.md` | `local_changes/list.rs` | Holds as built; its description of Fork needs re-checking (flag 13). |
| 10 | A line selection survives a refresh that draws the same text; one notice when it changed. | `concurrency.md` | `ShownDiff::keyed`, `staging_gesture.rs` | Holds; Fork's silent drop is FH. |
| 11 | Floating actions stay at the list's top for a tall chunk. | `cairn.md` | `diff/staging_gesture.rs` (per `docs/systems/local-changes.md`) | Holds (not re-measured). |
| 12 | A press without a drag, or Escape, lets a selection go; `Discard 2 Lines…`. | `diff.md` | `staging_gesture.rs` | Holds; Fork's uncounted "Discard…" is FH. |
| 13 | Amend asks only when a remote has the commit or no reflog keeps it; "Amend Last Commit". | seal; Recovery | `ops::amend_unconfirmed` / `ops::amend`; `amend_prompt` in `consequence.rs`; `AMEND_BUTTON_CAPTION` | Holds; Fork side FH. |
| 14 | Cancel beside a running commit; "Fork's cancel is only in the Activity Manager". | `processes.md` | `CANCEL_COMMIT_CAPTION` in `commit_box.rs` | Holds as built; the Fork half is wrong (flag 1) — Fork also cancels from the status box. |
| 15 | Skip hooks on every failed commit or amend. | D1 | `dialogs/git_error.rs`, unspent token | Holds; Fork's detection gap is 2019 evidence (flag 9). |
| 16 | A merge message as git's editor leaves it. | D1 | `reads::as_the_editor_leaves` | Holds. |
| 17 | The leftover lock is a banner; removal confirmed. | `processes.md`; D1 | `title_bar/lock_banner.rs` (`LOCK_STALE_AFTER` 10 s), `operations/leftover_lock.rs` | Holds; Fork side FH. |
| 18 | A nested repository among untracked rows is never touched. | write verbs | `Kept::NestedRepository` | Holds for discard; Create Branch's Discard can still delete one (#126). |
| 19 | A submodule's discard greyed, with why. | write verbs | `menu.rs` ("Submodule changes can't be discarded.") | Holds; #97 open. |
| 20 | Rebase, `git am`, sequences disable the box. | milestone | `operation_line` in `commit_box.rs` | Holds; Fork side unsourced (flag 7). |
| 21 | Show Lost Commits has a check box in the history's heading. | Recovery | `history/commit_row.rs` | Holds; underlying cause is also that Cairn has no View menu. See flag 12. |
| 22 | Create Branch's "Stash and reapply" greyed. | stash is its own feature | `dialogs/create_branch.rs` ("Comes with stashing.") | Holds. |
| 23 | Show Lost Commits reaches `HEAD`'s and local branches' reflogs. | `history-graph.md` | per `docs/systems/history-graph.md` | Holds (#110); Fork side is an analogy (flag 6). |
| 24 | Worktrees a sidebar section; a disabled checkout of a held branch. | `worktrees.md` | No worktree code in `cairn-ui` or `cairn-app` | Intent only; nothing built. Fork side half-sourced (flag 4). |
| 25 | A pressed ref is found however deep. | `history-graph.md` | `history/ref_find.rs` | Holds; Fork side PV. |
| 26 | A generic remote glyph where the forge is unknown. | `forge-links.md` | `history/ref_glyphs.rs` draws a cloud for every remote; no forge is ever identified | The row is not a deviation: Fork draws a cloud too (flag 3). The real difference is that Cairn never draws a forge icon. Reword. |
| 27 | Forge links in branch and commit menus; a post-push toast. | `forge-links.md` | None (no push, history menu has New Branch… alone) | Intent only. |
| 28 | Three-way, region-level conflict resolution. | `conflicts.md` | None | Intent only; Fork side unsourced (flag 5). |
| 29 | The toolbar drops Appearance / Workspace / Feedback and adds Open in terminal / editor and a command palette. | `feature-inventory.md` Tier 7 | No toolbar at all: the title bar holds "Cairn", the status box, the loaded count and `Fetch <remote>` (`window.rs`, `title_bar`) | Intent only. The Fork side is now FH-shot (lc1). |
| 30 | No Accounts section. | `credentials.md` | None drawn | Holds. |
| 31 | A loading state distinct from an empty repository. | `concurrency.md` | "Reading history…" vs "No commits yet." (`history/status_text.rs`) | Holds. |
| 32 | Linux-native chrome: client-side decorations, controls at the right, keyboard-first. | `platform.md` | `main.rs` builds a plain `WindowConfig` titled "Cairn": the platform's own decorations, no client-side ones | Intent only; not built. |

The "Kept" list: Local Changes first with its count, the graph as the main view, staging on one
screen, stashes inline, the detail pane with Commit and Changes, platform dialog order and
right-click selecting all hold in code. **Repository tabs do not**: Cairn opens one repository
from its command line and draws no tab strip (root `CLAUDE.md`, "There is no repository picker").

### Differences from Fork that ui.md does not record

Found in code against the evidence above; each is either a decision made elsewhere (cited) or an
unrecorded difference.

| Difference | Fork side | Where decided, if anywhere |
| --- | --- | --- |
| No toolbar (Pull, Push, Stash, Branch, Quick Launch, Open in, Console) and no menu bar. | FH-shot lc1 | Not recorded; scope, by roadmap. |
| `Fetch <remote>` fetches at once; Fork opens a Fetch dialog with a remote and "Fetch all remotes". | FH (ACT §7) | Not recorded; #23 covers choosing the remote. |
| The status box has no branch dropdown and the counts sit in it. | PV; FH-shot | Not recorded (flag 8). |
| A history heading row ("Graph and subject", "Author", "Commit", "Date (UTC)"). | FH-shot uv2 (none) | Not recorded; UTC in #103. |
| No avatars: history author, Commit tab, Changes summary. | PV; FH-shot | `docs/systems/diff.md` (L9), not ui.md. |
| A commit row's menu holds New Branch… alone; no sidebar or Changes-tab menus; no double-click checkout. | PU (CB §1) | Scope; not recorded. |
| "Ignoring whitespace hides some changes" in the diff bar. | PV (Fork silent) | `docs/systems/diff.md`. |
| Detail-tab chords Ctrl+Alt+1/2 on Linux, derived from the Mac. | SK (Windows unknown) | #57. |
| Lane width 14 px (`graph_geometry.rs`); Fork's measured pitch 22-24 px. | S | Not recorded. |
| Dark first, Freya's `dark_theme`; Fork for Windows starts light. | FH-shot | ui.md "Palette" states intent, not as a deviation. |
| ui.md's palette and type are not what is built: no IBM Plex Sans (only Plex Mono is embedded, `assets/fonts/`), Freya's theme rather than graphite and steel blue, eight lane colours (`lane_palette.rs`) rather than six. | — | Not a Fork question; ui.md's "Palette and type" is stale. |
| The commit button is not a split button (no Commit and Push). | PV; FH-shot | ui.md, "The commit box" (designed with push). |

## 3. Gaps the VM run must fill

### 3.1 No Fork evidence at all

- **Shell:** menus' contents; toolbar buttons other than Fetch and Branch (Pull and Push dialogs,
  Stash dialog's layout, Quick Launch, Open in, Console, Appearance, Workspace); the status box's
  dropdown and its detached-HEAD and unborn wording; window minimum size; what a close does with
  an operation running.
- **Repository tabs:** open, close, reorder, a tab's menu, the dot's meaning, the dirty marker in
  2.21.1.
- **Sidebar:** every context menu; Worktrees and Pinned when present; whether expansion is kept
  (#72); the filter funnel; keyboard; whether `Local Changes (N)` counts a staged-and-unstaged
  file once (RS OPEN 9).
- **History:** row height, lane pitch and colours (Windows); chip order with tags (RS OPEN 3);
  compact labels on Windows; detached `HEAD` (RS OPEN 2); push/pull dots in 2.21.1; the Windows
  commit menu; keys (↑/↓, Page, Home/End, ←/→); tooltips on chips; whether a heading row exists
  or can be shown; column resizing; date day padding (#57).
- **Commit tab:** COMMITTER when it differs (layout); a merge's and a stash's PARENTS; long
  messages; the REFS row's chips (`✓`?) and wrap; Windows tab chords and remembered tab (#57).
- **Changes tab:** the Windows file menu; multi-select; rename display; the filter's words; the
  layout menu; the two-commit header, swap and order on Windows.
- **Diff:** the context control in 2.21.1 and its maximum; side-by-side scope; wrap and
  invisibles; find; a mode change and a rename drawn (DD "does NOT settle"); sub-chunk staging in
  entire-file mode; keys in a focused diff.
- **Local Changes:** Tree vs List default; Shift held for Stage All (never observed); the staged
  list's full menu (SC OPEN 9); ignored files shown; a conflicted row; a collapsed untracked
  folder; drag visuals; a double-click inside a multi-selection (BR F24).
- **Commit box:** ↑/↓ recall rules; the counter's colours; the description's menu; the Commit
  and Push dropdown's items; a merge with nothing staged and a cleared merge message (MA's two
  questions); cherry-pick, revert, rebase in progress; a detached or unborn `HEAD`.
- **Dialogs:** `Error Details:` in 2.21.1; a failed fetch; the chunk discard's wording; a
  submodule's discard; Return and Escape per dialog; focus in Create Branch; the Fetch dialog's
  buttons.
- **Activity:** the × tooltip; keyboard; behaviour with many entries; the Background tab beyond
  fetch.
- **Preferences:** the whole window.
- **Palette and density:** Windows dark theme colours, UI typeface, accent, every row height but
  the diff's.

### 3.2 Only SECONDARY evidence

- The diff row pitch and font (measured from a Mac screenshot); the colour tables (measured,
  profile unknown); lane pitch (measured, scale unknown).
- The context step of one line (INFERRED from a GIF); side-by-side scroll sync (INFERRED).
- Stash row placement by date (INFERRED from screenshots); stash sidebar naming (INFERRED, now
  FH-shot).
- That double-click acts on its own row and a drag carries the selection (BR F24, F26, INFERRED
  from native list controls).
- The Mac's silence on a pushed amend; right-click selecting; Fork's conflict resolver; rebase
  pre-fill (flags 2, 5, 7, 10).
- Every PV/PU Mac-only fact that Cairn applies to Linux on Windows' behalf, notably the tab
  chords and the Mac dialog order.

### 3.3 Already on disk, unrecorded

The 2026-10-10 screenshots (`lc*`, `cb*`, `amd*`) show the full Windows window around each
observed detail — menu bar, toolbar, tab strip, sidebar, Local Changes toolbar, commit box — and
the 2026-10-16 shots (`uv2`, `uv2f`) show All Commits and the Commit tab over the fixture. Their
FH-shot facts above should be confirmed and written up by the run in progress rather than
re-observed.

## 4. Open issues that are UI or Fork-parity work

| Surface | Issue | Asks |
| --- | --- | --- |
| Shell | #43 Say the window is closing… | Tell the person a close is waiting, and let it stop the network lane's scans. |
| Shell | #65 Decide whether Cairn watches the file system… | Whether to refresh on changes made while focused (Fork does not). |
| Toolbar | #16 Push through the git backend… | Push, sealed behind `Confirmed`. |
| Toolbar | #23 Choose which remote to fetch | A way to fetch a remote other than the default (Fork's Fetch dialog). |
| Toolbar | #25 Bound what a large fetch costs the window | Keep a big fetch's output from costing the window. |
| Sidebar | #69 Add a Submodules section | Fork's Submodules section with dirty state. |
| Sidebar | #72 Find out whether Fork remembers the sidebar's expansion | A VM question. |
| History | #1 Collapse merges | Fork's collapsible merges for a wide graph. |
| History | #2 Filter, exclude and focus the graph by ref | Fork's branch filter and hiding. |
| History | #3 Jump to a commit by hash | A go-to-commit control. |
| History | #5 Decide what the history scrollbar does… | Scrollbar semantics without random access. |
| History | #67 Grey the commits the current branch does not reach | Fork's black/grey rows. |
| History | #68 Mark the commits to push and to pull | Fork's dots. |
| History | #101 Decide whether to show the reflog as a list | Beyond Fork's graph mode. |
| History | #110 Decide whether Show Lost Commits keeps its streaming reach… | Reach of the lost-commits mode. |
| Detail / diff | #29 Remember diff view settings across sessions | Fork remembers them. |
| Detail / diff | #30 Offer the detail pane to the right | Fork for Windows' vertical layout. |
| Detail / diff | #31 Add a File Tree tab | Fork's third tab. |
| Detail / diff | #32 Expand context between individual hunks | Beyond Fork (requested there). |
| Detail / diff | #33 Show whitespace characters | Fork's `¶`. |
| Detail / diff | #34 Wrap long lines | Fork's wrap. |
| Detail / diff | #35 Open a pop-up side-by-side diff | Fork's Space quick look. |
| Detail / diff | #36 Show the changed-file list as a tree or a table | Fork's layout menu. |
| Detail / diff | #37 Diff images | Fork's image views. |
| Detail / diff | #100 Restore one file to its state at a commit | Fork's "Reset file to state on commit". |
| Keyboard | #57 Establish Fork for Windows' detail-tab chords and its date's day padding | A VM question. |
| Keyboard | #121 Guard that the key resolver reads each chord's scope | Enforcement of the scope rule. |
| Local Changes | #62 List ignored files | Fork's Show Ignored Files. |
| Local Changes | #70 Add Hide Untracked Files | Fork's option. |
| Local Changes | #71 Offer Local Changes' layout menu | Tree and combined list. |
| Local Changes | #90 Decide whether a discard should keep a backup | Fork keeps none. |
| Local Changes | #97 Decide how to discard a submodule's changes | Fork's Discard Submodule Changes. |
| Local Changes | #119 Retry a discard that git refused for the index lock | Fork retries every write. |
| Commit box | #91 Add Fork's per-repository "Add Signed-off-by line" | Fork's setting. |
| Commit box | #92 Decide whether the box shows author and signing | Fork shows neither. |
| Commit box | #93 Pre-fill from `commit.template` | Fork does. |
| Commit box | #94 Wrap the description at the ruler | Fork's menu item. |
| Commit box | #95 Make the subject's limits configurable | Fork's setting. |
| Commit box | #117 Decide whether the Git Error dialog says earlier output was not kept | Consistency with the activity entry. |
| Dialogs | #111 Share one "ask the engine what it costs" flow | One flow for discard, Create Branch and lock removal. |
| Dialogs | #124 Tell an unborn HEAD from an unreadable one in Create Branch's Discard | Correctness of its re-check. |
| Dialogs | #126 Revisit whether Create Branch's Discard refuses or names a nested repository | Fork deletes it silently. |
| Activity | #44 Show the lock files a failed or closed write leaves behind | Surface stranded locks. |
| Activity | #99 Decide whether the operation log persists | Fork's does not. |
| Activity | #102 Add Fork's All / User / Background tabs | With the first background operation. |
| Activity | #103 Decide whether to show times in local time | Fork's are local. |
| Activity | #116 Stop offering "Show in Lost Commits" for an amend git did not log | A wrong offer. |
| Preferences | #89 Persist user settings across restarts | The settings store, starting with "Check out after create". |

## 5. The staging-and-commit UX reviews: fixed or open

The three reviews (`review-ux-*.md`) judged the pre-redesign build; the redesign of 2026-10-10
answered most. Status checked against the code today.

### 5.1 Local Changes (`review-ux-local-changes.md`)

| Finding | Status | Evidence |
| --- | --- | --- |
| H1 double "nothing was written" | Fixed | No strip; endings worded once on the worker (`worker/ending_words.rs`); the phrase survives only in a comment in `worker/local/lane.rs`. |
| H2 the strip under the lists | Fixed | `local_changes/pane.rs`: "Nothing is drawn under the lists". |
| H3 the same chord acts on different files by focus | Fixed | One file drawn, the primary; every route acts on the selection (ui.md, "Files"). |
| H4 two sources of the selection | Fixed | `cairn_ui::LocalSelection` with a primary; one rule, `local_changes/next_selection.rs`. |
| M1 cut refusal text | Fixed | Short reasons, no `max_lines` in `menu.rs`. |
| M2 four voices of refusal | Fixed | One frame: "<what> can't be discarded. <way forward>." (`menu.rs`). |
| M3 a mixed selection refused whole | Fixed | `Kept`; discards what it can. |
| M4 a selection dropped by any refresh | Fixed | `ShownDiff::keyed`. |
| M5 "line budget" jargon | Fixed in Local Changes | The Commit tab's Expand All still says "Expand All stopped at its line budget" (`docs/systems/diff.md`, user-decided). |
| M6 a fake "N files" file | Fixed | No files-together view. |
| M7 Unstage All with no visible control | Fixed | ⇈ in Staged; Shift on Linux. |
| M8 four spellings of a write | Fixed | `LocalWrite::name`, Fork's; one "Waiting for <name>…". |
| L1 a refusal that lingers | Fixed | No strip. |
| L2 the discard wording ladder | Fixed | Title Case throughout (D3). |
| L3 the staged list's menu | Open | Unstage, Unstage All, Copy Path; Fork's still UNVERIFIED. |
| L4 two "what next" rules | Fixed | `next_selection`. |
| L5 "path" vs "file"; the sparse-index parenthetical | Open | `NO_PATH_MATCHES` "No path matches the filter." vs the Changes tab's "No file matches the filter."; `SPARSE_INDEX` still ends "(git 2.32 and later can)". |
| L6 strip text in `status_text.rs` | Fixed (moot) | The strip is gone. |
| L7 double-click ignores the selection | Open, by design | `ListIntent` double press acts on its row; Fork's behaviour INFERRED only. |

### 5.2 Commit box and dialogs (`review-ux-commit-and-dialogs.md`)

| Finding | Status | Evidence |
| --- | --- | --- |
| H1 three amend surfaces | Fixed | `ops::amend_unconfirmed`, or "Amend Commit" in the one dialog; the skip spends an unspent token from the Git Error footer. |
| H2 the amend line repeats and is cut | Fixed | No "Replaces" line, no `max_lines` in `commit_box.rs`; "Amend Last Commit". |
| H3 Git Error for refusals | Fixed | "Couldn't <name>", no Error Details (`couldnt_title`). |
| H4 Create Branch vanishes under a second dialog | Fixed | The dialog is the confirmation surface; it stays beneath a failure. |
| H5 prompts enumerate edge cases | Fixed | One sentence frame plus Show files. |
| H6 `# Conflicts:` committed | Fixed | `reads::as_the_editor_leaves`. |
| H7 three button implementations | Partly | Confirm and Git Error answers share `answer_button`; Create Branch, the credential prompt and Git Error's Show files still use Freya's `Button`. |
| H8 the chord confirms in place | Fixed | `needs_confirming` sends it to the dialog. |
| M1 Amend greyed with no reason | Fixed | The operation line says "Amend is unavailable" (`commit_box/tests.rs`). |
| M2 reading/unreadable amend states | Fixed | The cost is read at the press. |
| M3 force-push advice with no push | Partly | The dialog now says "rewrites history others may have"; `Consequence::amend_reported` (activity) still says "Sharing the amended commit needs a force push." |
| M4 "Discard changes" titling Create Branch | Fixed | No second dialog. |
| M5 the lock prompt's boilerplate | Open, user-decided | Still "Remove <absolute path>? … holds N bytes … You can't undo this." (`consequence.rs`). |
| M6 Git Error's fixed 240 px output | Partly | `OUTPUT_HEIGHT` 240 remains; the amend prompt no longer stacks under it. |
| M7 refusals in four places | Fixed | One home. |
| M8 unpredictable confirm rule | Fixed | Create Branch's Discard excepted, by the user's choice. |
| M9 two of three radios; status-order test | Fixed | "Stash and reapply" greyed; `has_tracked_changes`. |
| M10 `checkout -f` grammar | Superseded | Fork's command chosen knowingly; the prompt grammar went; a nested repository it deletes is #126. |
| M11 waiting phrasing | Fixed in code | Doc comments in `commit_box.rs` and `dialogs/create_branch.rs` still say "Waiting for the commit to finish…". |
| L1 stale `Busy` doc | Open | As M11. |
| L2 "-3 characters left" read aloud | Open | `a11y_alt(format!("{} characters left", …))` in `commit_box.rs`. |
| L3 "Commit" during a merge | Open | Fork unknown. |
| L4 the cleared-merge-message rule | Kept (C7) | Fork unknown (MA Q1). |
| L5 recent messages by subject | As Fork | — |
| L6 confirm width 520, no max height | Moot | Prompts are one sentence; Show files is virtualized. |
| L7 Create Branch captions | Fine | — |

### 5.3 Activity and chrome (`review-ux-activity-and-chrome.md`)

| Finding | Status | Evidence |
| --- | --- | --- |
| H1 a cancel read as "may have taken effect" | Fixed | Four words; `CANCELLED_GLYPH`. |
| H2 the message cut at three lines | Fixed | "one sentence, never cut" (`activity_popover.rs`). |
| H3 the lock in four voices | Fixed | One banner (`lock_banner.rs`) and the Git Error button; no "found" entry. |
| H4 Remove index.lock keyed to past endings | Fixed | A state of the repository. |
| H5 eight status words | Fixed | Four. |
| M1 three wording tables | Fixed | One name. |
| M2 "kept as it were" | Fixed | Gone. |
| M3 doubled reassurance | Fixed | Gone. |
| M4 Show All by estimate | Fixed | The prompt is laid out whole above the lines. |
| M5 the Confirmed quotation | Kept | Recorded deviation (row 7, F3). |
| M6 UTC | Kept | #103. |
| M7 the way back turns a mode on | Partly | Named "Show in Lost Commits"; still turns the mode on; #116. |
| M8 internal vocabulary | Fixed | "Earlier output not kept."; the others gone. |
| M9 no keyboard in the popover | Open | Escape alone (`activity_popover.rs`). |
| M10 `keys_inert` read in many places | Fixed | `modal.rs`, one modal state. |
| L1 fields repeated | Fixed | The row shows no status word. |
| L2 × with no tooltip | Open | `CANCEL_OPERATION_CAPTION` "×", no tooltip. |
| L3 Show All keyed by place | Moot | No Show All. |
| L4 a duplicated engine string | Fixed | `GIT_RUNNING_NOTE` gone. |
| L5 "A write" fallback | Open | `session.rs`, `operations/activity.rs`. |
| L6 two fetch-failure states | Not re-checked | — |
| L7 Ctrl+Alt+1/2 and a four-key Stage All on Linux | Open | `accelerators.rs`; #57. |
| L8 the table's weight | Not user-facing | — |
| L9 lost rows in placeholder grey | As Fork | CB §5 (PV). |
| L10 Show Lost Commits off at launch | Open | #89. |
| L11 banners with no dismiss | Fixed for the lock | It goes with the file. |
