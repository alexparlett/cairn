# Fork for Windows, second window survey: dark theme, toolbar menus, keyboard, equal timestamps (2026-10-16)

The second survey run for the Fork-alignment packet, following
`fork-window-survey-2026-10-16.md` (not repeated here). Driven in the Windows 10 VM (QEMU/KVM,
1024x768 guest frame) through `qmp.py`; screenshots `uw*.png` in `~/VMs/fork-win10/shots/`.
VERIFIED = seen on screen in the named shot. NOT OBSERVED = not seen, with why. Pixel sizes are
measured on the 1024x768 screenshots (±2 px); colours are hex values sampled with PIL from the
screenshots (QEMU's VGA screendump, 24-bit).

## What was driven

- Same Fork as the first run (2.23.2.0, bundled git 2.50.1.windows.1; versions not re-read this
  run), Windows 10 22H2, guest frame 1024x768 (uw0's size); clicks landed where aimed. The VM
  signed in on its own; Fork, started from its desktop icon, restored its four tabs `r`, `n`, `a`,
  `t` with `t` (`C:\uirev\t`, the first run's fixture, unchanged) active (uw1). A second throwaway
  repository, `C:\uirev2\t` (bare remote `C:\uirev2\remote.git`), was built for question 6. The
  only settings changed were the theme (Dark, then Light again at the end). Run 12:33-12:50 BST.
- Bonus, light theme, as Fork opened (uw1): `main`'s tip was selected **before the list had focus**,
  and the band was grey **#D9D9D9** with dark bold text — the same grey as the sidebar's selected
  "All Commits" (#D9D9D9). This fills the first run's NOT OBSERVED "unfocused selection" for light.
  Light backgrounds sampled there: history and detail #FFFFFF, sidebar #FAFAFA; lanes `main`
  #FF9502 (its anti-aliased half #FFB041), second lane #FFD940; scrollbar thumb #D9D9D9.

## 1. The dark theme (Appearance ⌄ > Dark)

The Appearance menu (uw2): grey heading "Theme", "✓ Light", "Dark", rule, grey heading "Commit
List Layout", "✓ Horizontal", "Vertical". "Dark" pressed: the whole window, toolbar and caption
row included, turned dark at once, no restart, no dialog (uw3). Text is ClearType (coloured
fringes); text colours below are the solid core pixel.

### (a) All Commits, `main`'s tip selected, Commit tab

VERIFIED (focused uw3, zooms uw3a history, uw3b top bar; unfocused uw4).

| Surface | Colour |
| --- | --- |
| Caption + menu bar, toolbar (one band) | #1A1A1A; menu text #EAE8E8; toolbar labels #B2B2B2 (dimmer than the menus) |
| Status box | #303030 fill; "t" and "main" #EAE8E8; Push's count badge #606060 with white "1" |
| Repository tab strip | inactive tabs #101010, active tab (`t`) #2C2C2D (the sidebar's colour), tab names light grey |
| .gitignore banner | #333333; text #EAE8E8; buttons "Add .gitignore..." and "Close" bordered |
| Sidebar | #2C2C2D; text #EAE8E8 (headings bold, same colour); selected "All Commits" #3F3F3F; Filter field #292929 with placeholder "Filter" #999898; `bugfix/crash`'s row is NOT dimmed here (#EAE8E8) |
| History list | #212121; text #EAE8E8; unreachable rows (`stash@{0}`, `bugfix/crash`'s commit) and the stash's subject #A9A9A9 |
| Selected row, list focused (uw3) | **#236BD2** band, the same blue as the light theme, text white #FFFFFF bold |
| Selected row, list unfocused (clicked into the detail pane, uw4) | **#525252** band, text stays #EAE8E8, still bold |
| `main`'s row's small dot left of its chip | #3E9FF8 (blue) |
| Splitters, rules, detail-pane rules | #525252 (1 px) |
| Scrollbar | track and thumb both #686868 over the list (thumb not distinguishable by colour in this sample) |
| Detail pane (Commit tab) body | #333333; its tab strip #2B2B2B; selected tab "Commit" text #0078D7, other tabs #EAE8E8 |
| Detail labels "AUTHOR", "REFS", "SHA", "PARENTS", the date, the email | #C4C0C0; name, SHA value, subject #EAE8E8; parent link "f93015d" #3E9FF8 |
| REFS chip "main" in the detail pane | fill #545454, light text (no border colour of its own seen) |
| File badge "M" | #FFCC00 square (unchanged from light) |
| Avatar | gradient lavender #D376E9..#DE9FF0 with white initials (unchanged) |

Chips in the history (uw3a, fills sampled as the commonest colour inside each; text #F0F0F0 in
every chip):

| Chip | Fill | Border / icon segment |
| --- | --- | --- |
| Current branch "✓ main" | #5D3D14 (dark brown-orange) | border ~#986736 |
| Local branch "bugfix/crash", "feature/login" | #5B4C0E (dark olive) | border ~#977C2B |
| Remote "origin/main" | name segment #5D3D14 | icon segment unfilled (#212121, the list's background) with the cloud icon grey ~#777676; orange border, divider ~#F28B1F |
| Upstream-only cloud chip beside "feature/login" | unfilled (#212121), grey cloud icon | olive border ~#685727 |
| Tag "v0.2", "v0.1" | name segment #3E3F4E | icon segment unfilled, tag outline icon light grey; border lavender ~#636281 |
| Stash "stash@{0}" | name segment #545454 | icon segment unfilled with a light grey box icon; grey border ~#929292 |

Lanes: `main`'s lane **#FF9502** (orange, the same as light; its 2 px line's other pixel
#C77809), the second lane (the merged `feature/login` and `bugfix/crash`) **~#F4C302** core with
#C7A108 (darker than light's #FFD940); the merge node's hollow circle #FF9502. Row pitch,
column positions and font sizes are unchanged from light (compare uw1 and uw3: every row and
column at the same y and x).

### (b) The merge's Changes tab

VERIFIED (uw5, zoom uw5a). The merge has only an added file, so **no removed line was on
screen here**; see question 6's Preferences for the removed-line colours in Fork's diff preview.

| Surface | Colour |
| --- | --- |
| Summary bar ("Ada Lovelace f93015d03 16 Oct 2026 12:12 Merge branch 'feature/login'") | #333333; name and subject #EAE8E8 (subject bold), id and date #C4C0C0 |
| File list's bar and diff's bar | #333333; icons #CFCFCF |
| File list | #212121; selected file "login.rs" (chosen by Fork, never pressed) #525252; folder icon yellow #FFDF85; "+" badge green #28CD41 with white plus |
| Diff body | **#282828** (a shade lighter than the list's #212121) |
| Hunk header "@@ -0,0 +1,2 @@" | on #282828 (no band of its own), text #959595 monospace |
| Line-number gutter | #282828, numbers ~#A0A0A0, a 1 px rule #4B4B4B at its right |
| Added line band | **#3A5C3F** across the text column (the gutter stays #282828) |
| Added text | #DDDDDD; keywords `pub fn` blue ~#649BD1; comment "// validate" grey ~#9198A1 |
| History's selected merge row (focus now in the detail pane) | #525252 |

### (c) Local Changes

VERIFIED (uw6, zooms uw6a lists and diff bar, uw6b commit box).

| Surface | Colour |
| --- | --- |
| Lists' shared bar (filter, ⌄⌄, eye, View Mode) | #212121, icons ~#787878..#CFCFCF |
| "Unstaged" and "Staged" heading rows | #2B2B2B, heading text #EAE8E8 bold |
| List bodies | #212121 |
| Selected row ("new.txt", chosen by Fork) | #525252, text #EAE8E8 |
| Badges | added "+" #28CD41, modified "M" #FFCC00 (both as light) |
| "Stage" button (enabled) | fill #373737, 1 px border #525252, text #EAE8E8, ~54x22 |
| "Unstage" button (disabled) | fill #343434, border #383838, text #707070 |
| Commit box area | #2B2B2B |
| Subject / Description field | fill #333333, 1 px border #525252 around both and as the rule between them; placeholders "Commit subject", "Description" #999898; the Recent Commit Messages lines icon light grey |
| "Amend" check box | 14x14, fill #333333, border #6B6B6B; label #EAE8E8 |
| "Commit 1 File" split button (disabled, subject empty) | fill #343434, border #383838, text #707070, the ⌄ segment #6B6B6B glyph |
| Sidebar's selected "Local Changes (3)" | #3F3F3F |
| Diff body | #282828, added band #3A5C3F (as (b)) |

### (d) A context menu and a menu-bar menu

VERIFIED: a commit row's menu ("Add note 5", right press, uw7; an item hovered uw8, zoom uw8a);
the View menu (uw9, "Show All Commits" hovered). Both share one style:

| Part | Colour / size |
| --- | --- |
| Menu background | #2C2C2C, 1 px border #525252, a soft drop shadow ~4 px (#1E1E1E fading to the background) at right and bottom |
| Item text | #EAE8E8; the hovered item's text white #FFFFFF |
| Hovered item | #424242 band, ~21 px tall, inset ~3 px from the menu's sides, square ends |
| Separator | 1 px #525252, inset ~3 px from each side |
| Shortcut text | right-aligned, #9A9797 |
| Pitch | 23 px between items (ctx menu "New Branch..." y 332, "New Tag..." 355); the context menu ~312 px wide, View ~327 px |
| Menu-bar name while its menu is open | "View" on a #424242 box (the caption row's #1A1A1A around it) |

The context menu's items were the first run's (uv22), unchanged. Right-pressing the row selected
it with the grey (#525252) band while the menu was open; after Escape the row was the focused
blue #236BD2 (uw9). In uw9 the Changes tab of "Add note 5" also showed **context lines**: plain
#DDDDDD text on the diff's #282828 with two line-number columns ("2 2", "3 3", "4 4") and the
added line "5" on #3A5C3F with its new-side number only; the hunk header carries git's function
context, "@@ -2,3 +2,4 @@ line 1".

### (e) The Create Branch dialog (Ctrl+Shift+B)

VERIFIED (uw10, zoom uw10a). Opened by Ctrl+Shift+B with the history focused on "Add note 5";
closed with Escape. A ~600x200 dialog centred over the window: the Fork logo (blue circle, white
fork, ~56 px) at the left; **"Create Branch"** (bold, white #FFFFFF, ~14 px) and under it
**"Use '/' as a path separator to create folders"** (#CAC6C6); **"Create branch at:"** (#FFFFFF)
then a branch glyph and **"main"**; **"Branch name:"** and a field with a branch glyph and the
placeholder **"Enter branch name"** (#999898), focused; **"☐ Check out after create"**; buttons
**"Create"** (disabled) and **"Cancel"**; a **✕** at the top right.

| Part | Colour |
| --- | --- |
| Dialog background | #2B2B2B |
| Dialog border | 1 px #3BACED (light blue) all round, a dark shadow outside it |
| Name field | fill #333333, focused border 1 px #0078D7 (blue), ~370x24 |
| Check box | 14x14, fill #333333 |
| "Create" (disabled) | fill #343434, border ~#353535, text #707070 |
| "Cancel" | fill #373737, border #525252, text #EAE8E8 |

Font sizes in the dark theme: unchanged from light — every row, label and column in uw3 sits at the
pixel positions of uw1, and the text heights match.

## 4. New Branch's target (asked fourth, observed with 1(e))

- **Ctrl+Shift+B with "Add note 5" selected (not `HEAD`)**: the dialog reads **"Create branch at:
  [branch glyph] main"** — the current branch, **not the selected commit** (uw10; zoom uw11z, top
  line). VERIFIED (uw10).
- **Right press on "Add note 5" > "New Branch..."**: the same dialog reads **"Create branch at:
  [commit glyph ⌀-like, a circle on a vertical line] 635ee8e  Add note 5"** — the row's short id
  and subject (uw11; zoom uw11z, bottom line). VERIFIED (uw11).
- So the chord (and presumably Repository > New Branch..., which lists the same chord — not
  pressed) creates at `HEAD`'s branch whatever the selection, and the row's menu creates at the
  row. Both Escaped; nothing created.

## 2. Toolbar dropdowns and buttons (dark theme)

Each opened, read and closed with Escape; nothing chosen.

- **A slip, Escaped:** the first press aimed at Stash's ⌄ landed on the **Stash button itself**
  (x≈307) and opened the **"Save stash"** dialog (uw13): Fork logo, **"Save stash"**, **"Save your
  local changes to a new stash"**, **"Message:"** with a box glyph and placeholder **"Stash message
  (optional)"**, **"☐ Stage new files"** / **"By default stash ignores new files until you stage
  them"**, buttons "Save Stash" and "Cancel" (partly hidden behind a Windows OneDrive toast).
  Escape closed it with nothing stashed (uw14: the history unchanged, still one stash). The
  OneDrive toast (Windows, not Fork) was closed with its ✕.
- **Stash ⌄** (VERIFIED uw15): grey heading **"Recent Stashes"**, **"WIP on parser"**, a rule,
  **"Save Snapshot..."**.
- **Branch ⌄** (VERIFIED uw16): **"New Branch..."** Ctrl+Shift+B · **"New Worktree..."** — grey
  heading **"Lean Branching"** with a blue **"?"** at its right — **"Start Branch on 'main'..."** ·
  **"Sync 'main' (Rebase on 'origin/main')"** · **"Finish (Merge into 'main')..."** *(greyed)*.
- **Open in ⌄** (VERIFIED uw17): **"Open In Console"** Ctrl+Alt+T (console icon) · **"Open In File
  Explorer"** Ctrl+Alt+O (arrow icon). Nothing else (no editor or IDE entries).
- **Quick Launch, Ctrl+P** (VERIFIED uw18; scrolled by ↓/↑ only, uw19-uw24, combined uw20z,
  uw22z, uw24z): a palette dropped over the window's centre, **~620x370 px** (x 202-821, y 180-548),
  1 px #3BACED border, a search row on #333333 with a magnifier and the placeholder **"Command"**
  (#999898), a 1 px #525252 rule, then a list on #212121 with a scrollbar, rows ~27 px apart, the
  highlighted row #236BD2 with white text (the first row highlighted on opening). Contents, verbatim
  in order: bold heading **"Recent Repositories"** — "a — C:\t2\a", "t — C:\uirev\t", "r —
  C:\Users\fork\r", "n — C:\Users\fork\n" (name #EAE8E8, the path smaller #C4C0C0, a repository
  icon); bold heading **"Commands"** — each with a small console-window icon: "Apply Patch...",
  "Benchmark...", "Bisect", "Blame...", "Checkout Branch", "Create Branch...", "Create Pull Request
  on 'origin'", "Create Tag...", "Create Worktree...", "Delete Branch...", "Delete Tag...",
  "Fast-Forward", "Fetch", "Fetch...", "File History...", "Git Flow: Initialize...", "Merge...",
  "Open In Console", "Open In File Explorer", "Open Repository...", "Pull...", "Push", "Push...",
  "Rebase...", "Rename Branch...", "Repository Manager", "Repository Settings...", "Repository
  Statistics...", "Repository Treemap...", "Save Stash...", "Switch Workspace", "Zoom In", "Zoom
  Out". Alphabetical; no shortcuts shown; ↓ past the last row wraps to the top, ↑ from the top to the
  last. Escape closed it.
- **The status box's list glyph pressed** (VERIFIED uw25, zoom uw25a): Fork's Activity Manager drops
  from the toolbar, ~1004x437 px (x 10-1014, y 66-503). Left pane (#212121, ~320 px): tabs **"All"**
  (selected, blue text), **"User"**, **"Background"**; one entry, selected (#236BD2): **"Fetch
  'origin'"** bold, **"12:34"** right, under it **"Already up to date"** — Fork's automatic fetch
  since it opened. Right pane (#282828): header **"Fetch 'origin'"** and, right-aligned, **"succeeded
  16 Oct 2026 12:34:3…"** — **cut off by the popover's right edge** (the seconds clipped, a Fork
  layout bug at this width); then **"$ git fetch --prune origin"** (bold monospace) and **"Already
  up to date"** (monospace). Escape closed it.
- **The status box's centre pressed** (VERIFIED uw26): nothing opens; the hover funnel shows at its
  right (as the first run saw on hover).
- **The sidebar header's gear** (VERIFIED uw27): **"Rename Repository"** · **"Copy Repository
  Path"** — **"Settings for This Repository..."**.
- **The detail pane's two right icons** (tooltips VERIFIED uw28, uw29, zoom uw29z): the box-arrow is
  **"Open in separate window"**; the rectangle is **"Hide commit details"** (the View menu's Ctrl+Shift+D;
  not pressed, question 3 uses the chord). "Open in separate window" pressed (VERIFIED uw30): a
  separate window (its own minimize, maximize, close, blue 1 px border, over the main window) holding
  the selected commit's detail pane — the summary line "Ada Lovelace 635ee8ecb 16 Oct 2026 12:11
  Add note 5", the Commit / Changes / File Tree tabs (Changes selected), the file list and the diff.
  Closed with its ✕ (uw31); the main window's detail pane was back as before.
- **The Changes tab's View Mode icon** (VERIFIED uw32): **"✓ View as Tree"** · **"View as List"** ·
  **"View as Combined List"**.
- **Local Changes' View Mode icon** (VERIFIED uw33): **"✓ View as Tree"** · **"View as List"** ·
  **"View as Combined List"** — **"Hide Untracked Files"** · **"Show Ignored Files"**.
- **The Commit button's ⌄**: with the subject empty the button and its ⌄ are disabled and pressing ⌄
  opens nothing (VERIFIED uw34). With a subject typed ("x", never committed) (VERIFIED uw35): a menu
  with grey heading **"Commit Options"** and one item **"Commit and Push"** Ctrl+Shift+Enter.
  Escaped; the subject then cleared with Ctrl+A, Backspace (uw37, zoom uw37a: placeholder back,
  button greyed again). While "x" stood (uw36, zoom uw36a): the subject is **bold**, a grey counter
  **"49"** appears at the field's right before the Recent Commit Messages icon (50 minus the
  subject's length: the Preferences' low limit 50), and the enabled button is fill #373737, border
  #525252, text #EAE8E8.

## 3. Keyboard: Ctrl+1, Ctrl+2, Ctrl+0, Ctrl+Shift+D, Tab

The history was focused first by a press on "Add note 4".

- **Ctrl+1** (VERIFIED uw38): Local Changes is shown (the sidebar's "Local Changes (3)" selected,
  #3F3F3F) and **keyboard focus goes to the Unstaged list**: its selected row "new.txt" is drawn
  in the focused blue #236BD2 (where pressing the sidebar row in uw6 left it grey #525252).
- **Ctrl+2** (VERIFIED uw39): All Commits again, the history's selection kept ("Add note 4"), but
  drawn **grey #525252** — focus did not return to the list.
- **Ctrl+0** (VERIFIED uw40): `HEAD`'s row ("main" / "Local only commit") is selected, focused blue,
  its Changes shown ("Ada Lovelace 431a62275 16 Oct 2026 12:12 Local only commit").
- **Ctrl+Shift+D** (VERIFIED uw41): the detail pane **collapses to its tab strip alone** ("Commit",
  "Changes", "File Tree" and the two icons, pinned at the window's bottom, y≈712), the history
  growing to fill the space (the "Initial commit" row now visible, no scrollbar); the right icon
  ("Hide commit details") drawn highlighted blue. **Again** (VERIFIED uw42): the pane is back at
  its earlier height, the Changes tab as before.
- **Tab, eight presses from the focused history** (VERIFIED uw43-uw50, contact sheet
  uw43-50sheet; compared pixel by pixel): a three-stop cycle. Stop 1 (uw43): the Changes tab's
  file list — "notes.txt" turns focused blue, the history's row turns grey. Stop 2 (uw44): **nothing
  on screen shows focus** — the file list's row turns grey, and no other pixel changed (no caret,
  outline or tint anywhere; which control has focus is NOT OBSERVED). Stop 3 (uw45): back to the
  history (its row blue). uw46-uw50 repeat the cycle (file list, nothing, history, file list,
  nothing). Fork draws keyboard focus **only as the selection band's colour** (blue when focused,
  grey #525252 when not); **no focus rectangle or ring** was drawn at any stop. The sidebar, the
  toolbar, the Filter field and the tab strip were never reached by Tab.

## 5. Double presses (each Escaped, nothing confirmed)

- **A commit row, "Fix README typo"** (VERIFIED uw51; Escaped, uw52): the **"Checkout Commit"**
  dialog — Fork logo, **"Checkout Commit"**, **"Checkout a particular revision. Repository will be
  in detached HEAD state."**, **"Commit to checkout:"** [commit glyph] **"34145ac Fix README
  typo"**, **"Local changes:"** radios **"● Don't change"** / **"○ Stash and reapply"** /
  **"○ Discard"**, buttons **"Checkout Commit"** (default, blue border) and **"Cancel"**, a ✕.
- **`bugfix/crash` in the sidebar** (VERIFIED uw53; Escaped, uw54): the **"Checkout Branch"**
  dialog — **"Checkout Branch"**, **"Switch to another branch"**, **"Switch to:"** [branch glyph]
  **"bugfix/crash"**, **"Local changes:"** the same three radios ("Don't change" chosen),
  buttons **"Checkout"** (default) and **"Cancel"**, a ✕. The double press also selected the row
  in the sidebar and the branch's commit in the history.
- **A file in a commit's Changes list** (`src/main.rs` of "Fix crash on empty input") (VERIFIED
  uw55, uw56 4 s later, pixel-identical): **nothing opens** — the row is selected (focused blue)
  and its diff stays shown; no window, dialog or external program appeared (taskbar unchanged).
- A file in Unstaged: not double-pressed, as asked (it stages).

## 6. Equal timestamps: `C:\uirev2\t`

Built in Git Bash (opened by the toolbar's **Console** button, which starts Git Bash in the
repository, "MINGW64:/c/uirev/t", uw57) by `fixture-recipe.md` typed verbatim with `mkdir -p
/c/uirev2 && cd /c/uirev2` first and, after `cd t`, `export
GIT_AUTHOR_DATE="2026-10-16T12:00:00+01:00" GIT_COMMITTER_DATE="2026-10-16T12:00:00+01:00"`
(uw58); the remote and the `git push` line kept. Every commit, the stash's two included, has the
same second (Fork shows "16 Oct 2026 12:00" on every row and "16 Oct 2026 12:00:00 +01:00" in the
Commit tab, uw63, uw64). `git status -s` and `git stash list` matched the recipe's end state (uw59).

**`git log --oneline --all`** (VERIFIED uw59), verbatim, top to bottom:

```
8313cd7 (bugfix/crash) Fix crash on empty input
e49aa17 (origin/feature/login, feature/login) Validate login input
9913e5f (HEAD -> main) Local only commit
6bd753b (tag: v0.2, origin/main) Merge branch 'feature/login'
f7ad061 (refs/stash) On main: WIP on parser
2e78d15 (tag: v0.1) Add note 6
48f0802 Add login module
aaaf1f0 Fix README typo
e5ddb8f index on main: 9913e5f Local only commit
2f84245 Add note 5
9f27c5a Add note 4
20d42bf Add note 3
2962ae9 Add note 2
5d41002 Add note 1
27891b7 Initial commit
```

Without `--graph` (no topological order), git's own log **draws parents above their children**
with every date equal: `2e78d15` "Add note 6" above its children `48f0802` "Add login module" and
`aaaf1f0` "Fix README typo", and `9913e5f` "Local only commit" above its child `e5ddb8f` (the
stash's index commit).

**`git log --oneline --all --graph`** (VERIFIED uw60, zoom uw60a), verbatim:

```
* 8313cd7 (bugfix/crash) Fix crash on empty input
| *   f7ad061 (refs/stash) On main: WIP on parser
| |\
| | * e5ddb8f index on main: 9913e5f Local only commit
| |/
| * 9913e5f (HEAD -> main) Local only commit
|/
*   6bd753b (tag: v0.2, origin/main) Merge branch 'feature/login'
|\
| * e49aa17 (origin/feature/login, feature/login) Validate login input
| * 48f0802 Add login module
* | aaaf1f0 Fix README typo
|/
* 2e78d15 (tag: v0.1) Add note 6
* 2f84245 Add note 5
* 9f27c5a Add note 4
* 20d42bf Add note 3
* 2962ae9 Add note 2
* 5d41002 Add note 1
* 27891b7 Initial commit
```

**Fork over `C:\uirev2\t`** (opened by Ctrl+O, the folder typed into "Open Repository...",
"Select Folder"; VERIFIED uw63, zoom uw63a; the list's end uw64). Rows top to bottom, with the
graph:

| # | Row | Graph (lane 1 orange at the far left, lane 2 yellow) |
| --- | --- | --- |
| 1 | `bugfix/crash` chip, "Fix crash on empty input" (grey, unreachable) | dot on **lane 1** |
| 2 | `stash@{0}` chip, "On main: WIP on parser" (grey) | dot on **lane 2** |
| 3 | "✓ main" chip, **"Local only commit"** (bold, `HEAD`), the small blue dot | dot on lane 2 |
| 4 | `origin/main` + `v0.2` chips, "Merge branch 'feature/log..." | the hollow merge circle on lane 1; lane 2 curves in from above into it and a curve leaves it to lane 2 below |
| 5 | "Fix README typo" | lane 1 |
| 6 | cloud chip + `feature/login`, "Validate login input" | lane 2 |
| 7 | "Add login module" | lane 2 |
| 8 | `v0.1` chip, "Add note 6" | lane 1, lane 2 curving into it |
| 9-14 | "Add note 5" … "Add note 1", "Initial commit" | lane 1 |

Fork's order is the same as `git log --all --graph`'s (topological), **no parent drawn above its
child**. Unlike the first fixture (uw3, where `main`'s line holds lane 1 and `bugfix/crash` lane 2),
here **`bugfix/crash` (first row, since it sorts first) holds lane 1 and `main` with the stash is
pushed to lane 2**: the lane — and with it the chip's tint (bugfix/crash's chip now the orange-brown
#5D3D14-like fill, main's the olive) — follows the row order, not `HEAD`.

**Preferences > General, "Sort commits:"** (VERIFIED uw65): **"○ Topologically"** / **"● By date"**
— By date is the chosen setting (unchanged; read only), yet Fork still never drew a parent above a
child with every date equal. Escaped.

Also in the dark Preferences' diff preview (uw65, zoom uw65a): the **removed line's band #633F3E**,
its changed word **#9F4247**; the added line's band #3A5C3F (no stronger word tint seen on it);
preview background #282828; checked check boxes filled #0078D7; the dialog #2B2B2B.

Side effects seen: with two repositories named `t` open, **Fork renamed both tabs "t (uirev)"
and "t (uirev2)"** and the status box read "t (uirev2)" (uw63, zoom uw63b). After Ctrl+W closed the
`uirev2` tab (uw66), the remaining tab and the status box **still read "t (uirev)"** although no
other `t` was open (a stale disambiguation). Fork's sidebar on the new repository opened with
every folder and Tags closed (uw63), unlike `C:\uirev\t`'s remembered disclosure.

## 7. Back to Light (Appearance ⌄ > Light)

VERIFIED (uw67): the whole window light again, as uw1, with the `C:\uirev\t` tab still named
"t (uirev)" (above). Light values sampled there, for comparison with 1(a)-(c):

| Surface | Light | Dark |
| --- | --- | --- |
| Toolbar band | #F3F3F3 | #1A1A1A |
| Tab strip, inactive / active | #DDDDDD / #FFFFFF | #101010 / #2C2C2D |
| Banner | #FFFFFF | #333333 |
| History, text | #FFFFFF, #222222 | #212121, #EAE8E8 |
| Unreachable row text | #999999 | #A9A9A9 |
| Selected row, unfocused | #D9D9D9 | #525252 |
| Selected row, focused | #236BD2 (first run) | #236BD2 |
| Detail tab strip / summary bar | #FBFBFB / #FFFFFF | #2B2B2B / #333333 |
| Diff body, gutter | #FFFFFF, #FFFFFF | #282828, #282828 |
| Added line band | #E2FDE3 | #3A5C3F |
| Hunk header text | #C4C4C4 | #959595 |
| Chip fills: current branch / local branch / remote / tag / stash | #FFF2DD / #FFF9DC / #FFF2DD / #F6F5FE / #F7F7F7 | #5D3D14 / #5B4C0E / #5D3D14 / #3E3F4E / #545454 |

## What was not observed

- A removed line in a commit's or Local Changes' diff: the fixture has none; its colours were read
  from the Preferences preview only (question 6), in dark. The light removed-line colours were not
  sampled this run.
- Which control holds focus at Tab's second stop: nothing on screen shows it.
- Stash's toolbar tooltip and `bugfix/crash`'s Tracking submenu (still, from the first run).
- Font sizes were not re-measured beyond confirming the dark theme moves nothing (uw1 against uw3).
- Repository > New Branch... was not pressed (its target with a non-`HEAD` selection is inferred
  only from the shared chord, so it is not recorded as seen).
