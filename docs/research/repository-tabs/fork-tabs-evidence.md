# Fork's repository tabs: what the evidence records and what it does not

Compiled 2026-10-24 by reading the `feature/fork-alignment` tip (b539d9d), read-only. Paths are
repository-relative.

## Grades used here

| Grade | Meaning |
| --- | --- |
| **VERIFIED** | Seen first-hand in the Windows 10 VM (Fork for Windows 2.23.2.0, 1024×768) and written up with a named screenshot. |
| **VERIFIED (absence)** | The write-up lists a menu or surface verbatim, and the thing is not in it. |
| **UNVERIFIED (vendor)** | Fork release notes or a vendor comment in its tracker. Nobody has seen it in the VM. Often Mac-first or old. |
| **INFERRED** | Worked out from first-hand facts, or the observer's own reading. Nobody saw it directly. |

## Sources (short names)

| Short | File | Run |
| --- | --- | --- |
| FS1 | `docs/research/ui-review/fork-window-survey-2026-10-16.md` | VM, shots `uv*` |
| FS2 | `docs/research/ui-review/fork-window-survey-2-2026-10-16.md` | VM, shots `uw*` |
| MT | `docs/research/ui-review/fork-menu-toggles-2026-10-19.md` | VM, shots `mb*` |
| SB | `docs/research/ui-review/fork-sidebar-2026-10-20.md` | VM, shots `sb*` |
| CM | `docs/research/ui-review/fork-committer-2026-10-20.md` | VM, shots `cm*` |
| CP | `docs/research/ui-review/fork-copy-and-dialogs-2026-10-20.md` | VM, shots `cp*` |
| EV | `docs/research/ui-review/fork-evidence-inventory.md` | Audit dated 2026-10-16, written before FS1/FS2 were written up. Never edited afterwards. |
| RS | `docs/research/refs-and-status/fork-refs-and-status-ui.md` | Documentary |
| DD | `docs/research/diff-engine/fork-detail-and-diff-ui.md` | Documentary |
| SK | `docs/research/diff-engine/fork-shortcuts.md` | Documentary |

The ui-review files `fork-local-changes-2026-10-20.md`, `measured.md` and `fixture-recipe.md` say nothing about repository tabs. Neither do the staging-and-commit VM files (`fork-observed-2026-10-10.md`, `fork-activity-result-lines-2026-10-11.md`).

---

## 1. Where the strip sits, and how it looks

| Fact | Source | Grade |
| --- | --- | --- |
| Top to bottom: caption row with menus (y 0-25), toolbar (26-66), **repository tab strip (y 67-93, about 26 px)**, notification banner (94-117), then sidebar and main area. So the strip sits under the toolbar and above the banner. | FS1 §1 table (uv2, uv2b) | VERIFIED |
| "One tab per open repository, **equal widths across the window**": `r`, `n`, `a`, `t`. | FS1 §1 | VERIFIED |
| Each tab has **a small grey dot at its left** and **its name centred**. With 4 tabs at 1024 px the dots sit at x≈10, 260, 510, 760. | FS1 §1 | VERIFIED |
| The dot spacing works out to tabs about 250 px wide, filling the width apart from the "+". | from FS1 §1 positions | INFERRED |
| A **"+"** sits at the far right of the strip. | FS1 §1 | VERIFIED |
| Light theme: the active tab is white **#FFFFFF**, inactive tabs grey **#DDDDDD**. | FS1 §1 ("active: white, the rest grey"); FS2 §7 table | VERIFIED |
| Fork Dark: inactive tabs **#101010**, the active tab **#2C2C2D** (the sidebar's colour). Tab names are light grey. Active and inactive names were not sampled separately. | FS2 §1(a) table (uw3) | VERIFIED |
| Opening a tab narrows the others to fit. | FS1 §11 (uv39) | VERIFIED |
| In a narrower window (about 800 px) "the tab strip and banner shrink proportionally". At about 600 px Fork clips content at the right edge; nothing specific to the strip was recorded there. | FS1 §11 (uv41, uv42) | VERIFIED |
| Keyboard Tab never reaches the tab strip. From the history, Tab moves among the history, the file list and an invisible stop (FS2), or between the history and the sidebar (CP). | FS2 §3 (uw43-uw50); CP §7 (cp81-cp87) | VERIFIED |
| Not recorded anywhere: a close button on a tab, a hover look, an active marker other than fill (no underline is mentioned), the tab font size, separators between tabs, or how a long name is cut. | — | none |

## 2. The "+" button

| Fact | Source | Grade |
| --- | --- | --- |
| It exists, at the far right of the strip. | FS1 §1 | VERIFIED |
| What a press does, its tooltip, and whether it moves along with the tabs: never pressed or hovered. | — | none |
| That it does what New Tab does (opens a Repository Manager tab). | — | INFERRED only |

## 3. What a new tab shows: the Repository Manager

All from FS1 §11, "Repository Manager (File > New Tab, Ctrl+T)", shots uv39 and uv39z. The write-up does not say whether the menu item or the chord opened it.

| Fact | Grade |
| --- | --- |
| A new tab titled **"Repository Manager"** opens **at the end of the strip**. | VERIFIED |
| While it is active, the **menu bar shrinks to "File", "Window", "Help"** (no View, no Repository). | VERIFIED |
| The toolbar keeps every button, but **Fetch, Pull, Push, Stash, Branch, Open in and Console are greyed**. Quick Launch, Appearance and Work stay enabled. | VERIFIED |
| The status box reads **"Welcome to Fork!"** (bold) over **"Open a repository to start"**. | VERIFIED |
| Left pane (about 350 px): heading **"Repository Manager"** (large), then **"⌄ Recent"** (`t` selected in blue, then `r`, `n`, `a`), then **"⌄ Repositories"** (`a`, `n`, `r`, `t`, alphabetical). Each row is a small repository icon and the name. | VERIFIED |
| Right pane, for the selected repository: the name **"t"** (large), the path **"C:\uirev\t ➔"**, and an **"Open"** button at the right. Under them a card: "Uncommitted Files: 2", "Commits: 15", "Initial Commit: 15 minutes ago", "Last Commit: 15 minutes ago", "Remotes:" "[cloud] origin". Then tabs **"Summary"** (selected) and **"Statistics"**, and cards **"Branches"**, **"Tags"**, **"Commits"**. | VERIFIED |
| Fork's own errors on this page: "Uncommitted Files: 2" against the sidebar's "Local Changes (3)" (the untracked file apparently not counted); "Commits: 15" counts the stash's two commits where the history holds 13; the **Branches card lists only remote branches**. | VERIFIED (the counts); the reason for each is INFERRED |
| The tab was closed with **Ctrl+W**. The following shots show `t`'s history again, so `t` became active. | VERIFIED (the close); which tab Fork picks after a close is INFERRED (`t` was both the previous tab and the left neighbour) |
| Quick Launch (Ctrl+P) lists a **"Repository Manager"** command, **"Open Repository..."** and **"Switch Workspace"**, under the heading **"Recent Repositories"**: "a — C:\t2\a", "t — C:\uirev\t", "r — C:\Users\fork\r", "n — C:\Users\fork\n". | FS2 §2 (uw18-uw24) — VERIFIED |
| Recent's order differs between runs: Manager t, r, n, a (FS1 §11) against Quick Launch a, t, r, n (FS2 §2). What sets the order is not known. | VERIFIED (both orders); the rule is unknown |
| The "Repositories" list includes repositories outside Preferences' "Source Code Folder" (`C:\Users\fork\`, drawn with a ⚠): `C:\uirev\t` and `C:\t2\a`. So the list is not just a scan of that folder. | FS1 §10, §11 — INFERRED |
| Never tried: the **Open** button, the ➔, the Statistics tab, a double-press or Enter on a Recent row, a right-press on a row. | none |

## 4. Opening a repository; Clone; Init

| Fact | Source | Grade |
| --- | --- | --- |
| **Ctrl+O** opens "Open Repository...", a Windows folder picker whose button reads "Select Folder". The chosen repository opens **as a new tab** (from a repository tab; `t` became the fourth tab, at the end). | FS1 "What was driven"; FS2 §6 (uw63); SB "What was driven" (sb11-sb13); CM "What was driven" (cm5) | VERIFIED |
| A newly opened repository shows All Commits with `main` checked out. | CM (cm6) | VERIFIED |
| Vendor: Fork "always opens the All Commits view". | RS §2 (Tracker #295, 2018) | UNVERIFIED (vendor) |
| A newly opened repository's sidebar starts with every folder and Tags closed, "unlike `C:\uirev\t`'s remembered disclosure". | FS2 §6 (uw63); SB (sb13) | VERIFIED; that disclosure is kept per repository is INFERRED |
| An automatic fetch ran "since it opened" and is listed under the Activity Manager's Background tab. | FS2 §2 (uw25) | VERIFIED |
| **File > "Clone..." Ctrl+N** and **"Init New Repository..." Ctrl+Shift+N** exist. Neither dialog was opened. | FS1 §9 (uv24); MT §3 (mb10) | VERIFIED (menu text only) |
| Ctrl+O from the Repository Manager tab (new tab, or the Manager tab replaced?), Ctrl+O on a folder that is already open, a non-repository folder, a subfolder, or a bare repository: never tried. | — | none |

## 5. Tab names and how two are told apart

| Fact | Source | Grade |
| --- | --- | --- |
| A tab is named after the repository's folder: `C:\uirev\t` gives **"t"**. | FS1 §1 | VERIFIED |
| With two repositories named `t` open, **both tabs were renamed "t (uirev)" and "t (uirev2)"**: the parent folder in parentheses. The status box read "t (uirev2)". | FS2 §6 (uw63, zoom uw63b) | VERIFIED |
| Again: opening `C:\sb\t` gave a tab **"t (sb)"**, and "the earlier `t` tab renamed itself **'t (uirev)'**". | SB "What was driven" (sb13) | VERIFIED |
| Opening `C:\cm\t` gave a tab **"t (cm)"**. Whether the other `t` was renamed is not recorded. | CM (cm5, cm6) | VERIFIED (new tab name) |
| **Stale name:** after Ctrl+W closed the `uirev2` tab (uw66), the remaining tab and the status box **still read "t (uirev)"**, with no other `t` open. Still so after switching back to Light (uw67). | FS2 §6, §7 | VERIFIED |
| After a relaunch the tab was plain `t` again (MT and SB list the restored tabs as `t`, and SB says it "renamed itself" when another `t` opened). | MT; SB | INFERRED (from the wording of the restore lists) |
| The fork-alignment brainstorm lists "stale tab names" among Fork bugs not to copy. | `docs/work/fork-alignment/brainstorm.md` on `main` (around line 177) | — |
| The window title in the caption row is **"Fork"** alone, never the repository. | FS1 §1 | VERIFIED |
| What the sidebar header shows with a disambiguated name; the rule with more than one shared parent level; three or more same-named repositories; the effect of the gear's "Rename Repository" on the tab. | — | none (the gear's "Rename Repository" item itself is VERIFIED, FS2 §2 uw27) |

## 6. The dot, and the "uncommitted changes" indicator

| Fact | Source | Grade |
| --- | --- | --- |
| **Every** tab (`r`, `n`, `a`, `t`) showed a small **grey** dot at its left. Whether `r`, `n` and `a` had uncommitted changes was not recorded. | FS1 §1 | VERIFIED (the dot); its meaning is unknown |
| Preferences > General has **"☑ Tab indicator for uncommitted changes"**, ticked. | FS1 §10 (uv29, uv30) | VERIFIED |
| Vendor history: "Show uncommitted changes indicator in tabs" (Mac RN 1.0.74, Feb 2019; Win RN 1.29), first drawn as `name*`, later as tab badges or dots (Mac RN 2.45, Aug 2024). | RS §4 | UNVERIFIED (vendor) |
| Vendor: the marker refreshes on application focus and **ignores untracked files** (TrackerWin #2514, Jun 2025). It can be turned off (TrackerWin #553, 2019). | RS §4 | UNVERIFIED (vendor) |
| EV lists "the dot's meaning, the dirty marker in 2.21.1" as a gap. | EV §1.4, §3.1 | — |
| **Overstated in Cairn's docs:** the roadmap's 5h brief says "a dot marking uncommitted changes", citing FS1 §1 and §11. Neither section says what the dot means. | `docs/work/daily-loop/roadmap.md` §5h | INFERRED, written as fact |

## 7. Close buttons, middle-click, drag-reorder, context menu

| Fact | Source | Grade |
| --- | --- | --- |
| Ctrl+W closes the active tab (the Repository Manager tab; the `uirev2` tab). | FS1 §11; FS2 §6 (uw66) | VERIFIED |
| File > "Close Tab" Ctrl+W and "Restore Closed Tab" Ctrl+Shift+T exist. Restore was never pressed. | FS1 §9; MT §3 | VERIFIED (menu text) |
| A × on a tab: none is described in any write-up. Whether one appears on hover is unknown. | FS1 §1 (describes only the dot and name) | none |
| Middle-click, drag to reorder, drag out to a window, a right-press on a tab or on the strip's empty space, a double-press on a tab or on the empty strip: never tried. EV calls "opening, closing, reordering, a tab's context menu, the repository manager" evidence-free. Opening and closing are now partly covered by FS1/FS2. | EV §1.4, §3.1 | none |
| Neither File nor Window has a **New Window** item. | FS1 §9; MT §3 | VERIFIED (absence) |

## 8. Keyboard

| Chord | Item | Source | Grade |
| --- | --- | --- | --- |
| Ctrl+T | File > New Tab | FS1 §9, §11 | VERIFIED (menu text); FS1 §11 names both routes, so which one was pressed is unclear |
| Ctrl+W | File > Close Tab | FS1 §11; FS2 §6 | VERIFIED (pressed) |
| Ctrl+Shift+T | File > Restore Closed Tab | FS1 §9; MT §3 | VERIFIED (menu text), never pressed |
| Ctrl+O | File > Open Repository... | FS1, FS2, SB, CM | VERIFIED (pressed) |
| Ctrl+N | File > Clone... | FS1 §9 | VERIFIED (menu text) |
| Ctrl+Shift+N | File > Init New Repository... | FS1 §9 | VERIFIED (menu text) |
| Ctrl+P | Quick Launch (recent repositories and commands) | FS2 §2 | VERIFIED (pressed) |
| Ctrl+Tab | Window > Select Next Tab | FS1 §9 (uv27) | VERIFIED (menu text), never pressed |
| Ctrl+Shift+Tab | Window > Select Previous Tab | FS1 §9 | VERIFIED (menu text), never pressed |
| Ctrl+1 / Ctrl+2 / Ctrl+0 | **View** items: Show Uncommitted Changes / Show All Commits / Show HEAD. So these are not tab-jump chords. | FS1 §9; FS2 §3 (uw38-uw40) | VERIFIED |
| Ctrl+3..9, Alt+digit, Ctrl+PageUp/PageDown, Ctrl+F4 | Not in any menu. Whether any is heard is unknown. | FS1 §9 | VERIFIED (absence from menus) |
| Mac tab chords | Not recorded. On the Mac ⇧⌘T is **New Tag** (EV §1.16), where Windows' Ctrl+Shift+T is Restore Closed Tab and New Tag is Ctrl+Shift+G. SK's "Unestablished" lists "Fork Windows' tab chords" (that is the detail pane's Commit/Changes tabs, not repository tabs). | EV §1.16; SK | UNVERIFIED (vendor) |

## 9. The File and Window menus, verbatim

**File** (FS1 §9 uv24; repeated in MT §3 mb10; VERIFIED): "Init New Repository..." Ctrl+Shift+N · "Clone..." Ctrl+N — "New Tab" Ctrl+T · "Open Repository..." Ctrl+O · "Quick Launch..." Ctrl+P · "Close Tab" Ctrl+W · "Restore Closed Tab" Ctrl+Shift+T — "Configure SSH Keys..." · "Accounts..." — "Check for Updates..." — "Preferences..." Ctrl+, — "Exit". (— marks a separator.)

**Window** (FS1 §9 uv27, zoom uv25w; VERIFIED): "Select Previous Tab" Ctrl+Shift+Tab · "Select Next Tab" Ctrl+Tab — "Switch Theme" — "Zoom In" Ctrl+= · "Zoom Out" Ctrl+-.

No item in any menu was greyed on a repository tab (FS1 §9, VERIFIED). Whether any File or Window item greys on the Repository Manager tab was not recorded.

## 10. Do tabs come back after a relaunch?

| Fact | Source | Grade |
| --- | --- | --- |
| On launch Fork **restored three earlier tabs `r`, `n`, `a`**. | FS1 "What was driven" | VERIFIED |
| Next launch: Fork "restored its four tabs `r`, `n`, `a`, `t` with `t` (...) active". | FS2 "What was driven" (uw1) | VERIFIED |
| Again `r`, `n`, `a`, `t` with `t` active. | MT "What was driven" (mb0/mb1) | VERIFIED |
| Again `r`, `n`, `a`, `t`. The `uirev2` tab, closed with Ctrl+W before quitting in FS2, did not come back. | SB "What was driven" | VERIFIED |
| The order is kept, and the tab active at quit comes back active. | the four runs above | INFERRED (which tab was active at each quit is not recorded) |
| Whether restoring is a preference, whether a Repository Manager tab open at quit comes back, and whether each tab's view, selection or scroll comes back: not recorded. | — | none |
| The theme apparently carries across launches: MT, SB and CM all opened already in Dark. But FS2 ended by switching to Light, and nothing recorded who switched back to Dark. | MT, SB, CM, FS2 §7 | INFERRED |

## 11. What the menu bar, toolbar, status box and window title act on

| Fact | Source | Grade |
| --- | --- | --- |
| The menu bar changes with the active tab: on the Repository Manager tab only File, Window and Help are shown. | FS1 §11 | VERIFIED |
| The toolbar greys its repository buttons on the Repository Manager tab. | FS1 §11 | VERIFIED |
| The status box shows the active tab's repository and branch (including the "t (uirev2)" name), or the Manager's welcome text. | FS1 §1, §11; FS2 §6 | VERIFIED |
| Push's badge, Fetch and the other actions act on the active tab only. | — | INFERRED (only one repository was ever exercised at a time) |
| The caption title is always "Fork". The OS taskbar title was not observed. | FS1 §1 | VERIFIED (caption) |
| The sidebar header shows the active repository's name ("t"). | FS1 §3 | VERIFIED (single repository) |

## 12. Per-tab or global state

Nothing was observed across two tabs, so no item below is VERIFIED for tabs.

| Setting | What is recorded | Source | Grade |
| --- | --- | --- | --- |
| Theme (Appearance > Light/Dark) | "the whole window, toolbar and caption row included, turned dark at once". Only the active tab was looked at. | FS2 §1 (uw3) | VERIFIED for one tab; global is INFERRED |
| Commit List Layout (Horizontal/Vertical) | In the Appearance menu next to the theme. Scope not recorded. | FS1 §10 (uv37) | VERIFIED (menu only) |
| Zoom (Ctrl+= / Ctrl+-) | In the Window menu. Never used, scope unknown. Vendor: "whole-UI zoom". | FS1 §9; EV §1.15 | VERIFIED (menu) / UNVERIFIED (vendor) |
| Diff context lines | The vendor calls them "a global setting" (Tracker #1603, 2022). | DD Finding 14 | UNVERIFIED (vendor) |
| Diff toggles (whitespace, ¶, wrap, side by side) | Not recorded per tab. | — | none |
| Diff font size, "Show -+ marks", syntax highlighting | Preferences > General, so app-wide. | FS1 §10 | VERIFIED (where they are set) |
| Tab width (meaning the tab character's width in diffs) | A user's wording suggests it is set per repository; location unknown. | DD (around line 910; Tracker #1568) | UNVERIFIED |
| File list View Mode (Tree/List) | The vendor says it is global, not per repository (Tracker #2616, 2026). | RS (around line 417); DD (around line 381) | UNVERIFIED (vendor) |
| Detail split position | Windows does not keep it (TrackerWin #1151, open). List column widths are kept (Windows 1.77/1.78). | DD (around lines 363-364) | UNVERIFIED (vendor) |
| Sidebar width | "the sidebar keeps its width (~300 px)" at every window width. Never compared across tabs. | FS1 §11 | VERIFIED (one tab) |
| Sidebar disclosure | A new repository opens with its own defaults, `C:\uirev\t` with what it had. | FS2 §6; SB | VERIFIED differs between repositories; that it is stored per repository is INFERRED |
| Branch filter, hidden tags and stashes, collapse state | Stored per worktree on Windows 2.23. Hide stashes per repository (Mac RN 2.1). | RS §2 and around line 282 | UNVERIFIED (vendor) |
| Commit message draft | "remembered across views and, on Windows since 2.23, per worktree". | `docs/research/staging-and-commit/fork-staging-and-commit.md` (around line 341) | UNVERIFIED (vendor) |
| Detail tab (Commit/Changes) remembered | Mac since 2.23. On Windows the request is open (TrackerWin #930). | DD Finding 2 | UNVERIFIED (vendor) |
| Per-repository settings | "Settings for This Repository..." is in the Repository menu and the sidebar gear. Its contents were never opened. Vendor: sign-off, GPG and commit template are per repository. | FS1 §9; FS2 §2 (uw27); `fork-staging-and-commit.md` | VERIFIED (menu) / UNVERIFIED (contents) |
| Hide Commit Details, Reflog mode, the sidebar toggle | Observed on one tab only. | MT §1-2; FS2 §3 | none for scope |
| Workspaces | Toolbar "Work ⌄": heading "Workspaces", "Home", "✓ Work", "Configure...". Quick Launch has "Switch Workspace". Whether a workspace holds its own set of tabs is not recorded. | FS1 §10 (uv38); FS2 §2 | VERIFIED (menu); what a workspace contains is unknown |
| Worktree checkout | Checking out a branch held by another worktree opens that worktree's tab (Win RN 2.2). | RS, "Corrections"; EV §1.4 | UNVERIFIED (vendor) |

## 13. A repository that fails to open

Nothing in docs/research records Fork for a deleted or moved folder, a non-repository folder, a permission or `safe.directory` refusal, or a repository that vanishes while its tab is open. This applies to Ctrl+O, the Manager's Open, and the relaunch restore alike. The only nearby fact is the ⚠ beside "C:\Users\fork\" in Preferences' Source Code Folder (FS1 §10, VERIFIED), and nobody found out what it means.

For comparison, Cairn's own failed-open window is described as-built in `docs/research/ui-review/cairn-ui-as-built.md` (around lines 325-346, pre-alignment).

## 14. Fork for macOS

The only Mac facts on tabs are vendor ones: the uncommitted indicator `name*`, later dots or badges (RS §4); ⇧⌘T is New Tag (EV §1.16); "Fork always opens the All Commits view" (RS §2). Nothing first-hand.

---

## 15. What Cairn has built (at `feature/fork-alignment` b539d9d)

**Docs**

- `docs/design/ui.md` lines 17-29 draw "tabs: one per open repository" in the layout. Lines 43-44 keep "Repository tabs across the top. The manager behind them is open (`cairn.md`, 'Still open'); tabs are the least-committing surface to show." No metrics, behaviour or deviations are designed.
- `docs/design/cairn.md` "Still open": "Repository manager shape. Tabs, a sidebar of repositories, or separate windows. It decides how much state is per-repository versus global (...) Worker threads per repository with view settings app-wide fit all three shapes."
- `docs/prd/fork-alignment.md`:
  - R2.5: File holds only **"Exit"**; Window holds only "Zoom In" and "Zoom Out", applied "for the session".
  - R2.4: the OS window title is "<repository> — Cairn". This is Cairn's choice; Fork's caption says "Fork".
  - "Out of scope": "repository tabs, the Repository Manager, New Tab, Open, Clone and Init (`repository-manager`)".
  - **No placeholder tab strip** is required anywhere.
- Brainstorm L8 and S4 (`docs/work/fork-alignment/brainstorm.md` on `main`; torn down on the integration branch): tabs and the Manager are packets of their own, "the File and Window menus offer only what works".
- `docs/work/daily-loop/roadmap.md`:
  - **5f quick-launch**: recent repositories "once the Repository Manager keeps any".
  - **5g preferences**: the first persisted settings store (#89), the sidebar disclosure (#72), the diff settings (#29).
  - **5h repository-manager**: "several repositories in one window, each with its own worker pool and `SharedRepository`, behind Fork's tab strip (... equal-width tabs, a '+', a dot marking uncommitted changes, tabs named apart by their folder when two share a name); Fork's Repository Manager tab (Recent and Repositories, a summary of the one selected), File's Open Repository…, New Tab, Close Tab, Restore Closed Tab, Clone and Init, and Window's tab switching. Needs 5g's store for the recent list." It depends on 5d and 5g.
- `docs/design/feature-inventory.md` lines 26, 112, 121-123: open from a path; Clone, Recent repositories, Multi-repository UI and Init are parked.
- `docs/design/mockups/cairn-ui.html` lines 61-64, 259, 337, 418, 564: Cairn's own pre-alignment mockup strip "cairn ● / freya / gitoxide". It is a 30 px row; the dot is the warn colour with the title "uncommitted changes"; the active tab has an accent underline. This is not Fork's look.
- `docs/research/ui-review/freya-chrome-apis.md` §7: Freya has **no Tabs component**. Cairn would draw the strip from `Button`s, as `detail/tabs.rs` does, with `DragZone`/`DropZone` for reordering.
- `CLAUDE.md`: "There is no repository picker: one repository, named on the command line."

**Code** (grep of `crates/cairn-ui/src` and `crates/cairn-app/src` for "tab strip", "TabStrip", "New Tab", "Repository Manager")

- **No repository tab strip exists.** Every "tab strip" hit is the detail pane's Commit/Changes strip: `cairn-ui/src/accelerators.rs:99`, `style/metrics.rs:64,131-136`, `style/tones.rs:32`, `cairn-app/src/shortcuts.rs:88`, `window.rs:2723-2728`, `window_check.rs:342`. "New Tab" and "Repository Manager" match nothing.
- `crates/cairn-app/src/window.rs:239-262` stacks the caption row (`CaptionRow`), the toolbar (`crate::toolbar::toolbar`), the two banners (leftover lock, closing), then sidebar beside main. Nothing is drawn between the toolbar and the banners, where Fork's strip sits.
- `crates/cairn-app/src/menu_bar.rs:46` builds File as `Exit` alone; line 80 builds Window with Zoom In and Zoom Out. This matches `docs/systems/window-frame.md` (menus table, around lines 143-149).
- `crates/cairn-app/src/repository_path.rs` takes one repository from the command line or working directory.
- `crates/cairn-app/src/main.rs:89-125` holds a single set of state "for the session": detail tab, pane split, diff settings, Changes filter and width, sidebar, Local Changes, writes, Create Branch, activity, frame/zoom. None of it is keyed per repository yet.
- `docs/systems/window-frame.md` "The title": the desktop title is "<repository> — Cairn".

## 16. Places where Cairn's records outrun the evidence

1. The roadmap's 5h brief says "a dot marking uncommitted changes". First-hand, every tab had a grey dot and their dirtiness was not recorded. The meaning rests on vendor notes (§6).
2. EV §1.4 and §3.1 (2026-10-16, never edited) still list opening, closing and the Repository Manager as evidence-free. FS1 and FS2 have since recorded Ctrl+O, Ctrl+W, New Tab's Manager, names, restore and the menus. Reordering, the tab menu and the dot are still open.
3. ui.md's "the least-committing surface" and cairn.md's "view settings app-wide" are design positions. No Fork evidence on per-tab scope supports or contradicts them.

---

## 17. Questions no evidence answers (for a Fork VM observation)

**The strip's look**
1. With 1, 2 and about 15 tabs: do tabs always share the full width, or stop at a maximum width? What is the minimum width, and what overflows (scroll arrows, a dropdown, shrinking)?
2. Does the "+" stay at the far right or follow the last tab?
3. Is the strip drawn when only one tab is open?
4. Measure tab height, font size, padding, separators, and the active and inactive name colours (light and dark). Is there any active marker besides the fill? What does hover look like?
5. How is a long name cut (end ellipsis, middle)? Does hovering a tab show a tooltip, such as the full path?

**The dot**
6. What does the grey dot mean? Compare a clean repository against unstaged-only, staged-only and untracked-only ones. Does it change colour or vanish when clean? What happens with Preferences' "Tab indicator for uncommitted changes" unticked?
7. When does it update: live, on focus, on switching tabs? Does it ever show anything else (an operation running, behind the upstream, fetching)?

**"+", New Tab, the Repository Manager**
8. What does "+" do, and what is its tooltip?
9. Ctrl+T while a Repository Manager tab is already open: a second Manager, or focus the existing one?
10. The Manager's **Open**, a double-press on a Recent or Repositories row, Enter: does the Manager tab turn into the repository in place, or does a new tab open? If that repository is already open in a tab, does Fork switch to it?
11. What do the path's ➔ and the Statistics tab do?
12. Where do "Repositories" entries come from? How is one removed? What does a right-press on a row offer? Is there a filter or search field? Does dropping a folder onto the Manager do anything?
13. What orders "Recent" (last opened or last activated), and how many does it keep?
14. What does the Manager show with no repositories at all (empty state wording)?
15. Which keys work in the Manager (↑/↓, Enter, typing to find)?
16. Which File and Window items are greyed on the Manager tab?

**Open, Clone, Init**
17. Ctrl+O from the Manager tab: replace the Manager or add a tab?
18. Ctrl+O on a folder already open in a tab: a duplicate tab or a switch?
19. Ctrl+O on a non-repository folder, a subfolder of a repository (is the root found?), a bare repository, a `safe.directory`-refused one: exact wording, and dialog or in-tab message?
20. The Clone dialog (Ctrl+N) and the Init dialog (Ctrl+Shift+N): fields, wording, buttons, progress. Does the result open in a new tab?
21. Dropping a folder onto the window or the strip; starting `Fork.exe <path>` while Fork runs: does a tab open in the running window? Can there be several Fork windows?

**Names**
22. The disambiguation rule when the parents also match (`C:\a\x\t` and `C:\b\x\t`), and with three same-named repositories.
23. When does a stale "t (uirev)" clear: only at relaunch, or when tabs reopen or switch?
24. Do the sidebar header, the Manager and Quick Launch show the disambiguated name? What is the OS taskbar title of Fork's window?
25. What does the gear's "Rename Repository" do to the tab, the Manager and Quick Launch?

**Closing, restoring, reordering, the tab's menu**
26. Is there a × on tabs (always, on hover, on the active tab only)? What is its tooltip?
27. Does a middle-press close a tab? A double-press on a tab, or on the empty strip?
28. Ctrl+W on the last remaining tab: does the window close, an empty window stay, or a Manager tab appear?
29. Closing a tab while its fetch or commit runs: is there a warning, and is the operation cancelled?
30. After a close, which tab becomes active: the left neighbour, the right neighbour, or the previously active one? (Test with the closed tab in the middle.)
31. Ctrl+Shift+T: how many closed tabs deep? Does it put the tab back at its old position? Does it restore that tab's view and selection? Does it restore a closed Manager tab?
32. Drag to reorder? Drag out to a new window? Drag between windows?
33. A right-press on a tab, and on the strip's empty space: exact items (Close, Close Others, Close to the Right, Copy Path, Show in Explorer, Rename...)?

**Keyboard**
34. Ctrl+Tab and Ctrl+Shift+Tab: strip order or most-recently-used? Do they wrap at the ends? Do they include the Manager tab?
35. Are Ctrl+PageUp/PageDown, Ctrl+3..9, Alt+1..9 or Ctrl+F4 heard?
36. Are Ctrl+T, Ctrl+W and Ctrl+Tab heard while focus is in the commit subject or a filter field?

**Relaunch**
37. Is restoring tabs a preference? Is the active tab and order restored exactly? Is a Manager tab open at quit restored?
38. Does each restored tab come back on All Commits or on its last view (Local Changes, selection, scroll, detail tab)?
39. Restoring a tab whose folder was deleted or moved: an error tab, silently dropped, or flagged in the Manager?
40. Do workspaces (Home/Work) each hold their own set of tabs? Does "Switch Workspace" swap the tab strip? Is restoring per workspace?

**Per tab or global** (two repositories, change in one, switch to the other, then relaunch)
41. Theme, Commit List Layout and zoom: applied to all tabs? Kept after relaunch?
42. Diff toggles (ignore whitespace, context lines ±, entire file, side by side, wrap, hidden symbols): per tab or global? Kept after relaunch?
43. Splitter positions (sidebar width, history/detail split, Changes file-list width, Local Changes' splits) and the sidebar toggle: per tab or global? Kept after relaunch?
44. Hide Commit Details, reflog mode, Hide Tags or Stashes, Filter by Active Branch, the detail pane's selected tab, View Mode (Tree/List): per tab or global?
45. Is each tab's history selection, scroll, Local Changes selection and commit draft kept when switching tabs?
46. Does the Activity Manager list operations of every open repository or only the active one? Do background tabs fetch automatically? Does a background tab show a running operation on its tab?
47. Is the `.gitignore` banner per tab? Does its Close stay closed?
48. Does switching tabs refresh the repository it lands on?

**Failure**
49. A repository deleted, or made unreadable, while its tab is open: what does the tab show, in what wording?

**Repository Manager bugs, to re-confirm before calling them Fork's**
50. Does "Uncommitted Files" leave out untracked files? Does "Commits" count stash commits? Does the Branches card list only remote branches? (All seen once in FS1 §11, uv39.)
