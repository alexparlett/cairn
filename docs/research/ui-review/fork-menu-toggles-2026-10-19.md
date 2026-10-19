# Fork for Windows: the menu bar's toggles, menus and caption row, observed first-hand (2026-10-19)

For the Fork-alignment packet's phase 02 (the menu bar). Driven in the Windows 10 VM (QEMU/KVM)
through `qmp.py`; screenshots `mb*.png` in `~/VMs/fork-win10/shots/`. VERIFIED = seen on screen in
the named shot. NOT OBSERVED = not seen, with why.

## What was driven

- Fork for Windows 2.23.2 (as the earlier surveys read it; not re-read this run), Windows 10 22H2,
  guest frame 1024x768 (mb0's size); clicks landed where aimed. The VM signed in on its own; Fork,
  started from its desktop icon, restored its tabs `r`, `n`, `a`, `t` with `t` active — `C:\uirev\t`,
  the ui-review fixture (`fixture-recipe.md`), used as the caller asked (these questions are about
  the window's chrome, not the repository's state; nothing was written to it). Fork Dark theme, as
  Fork opened (mb1). Run 21:42-21:49 BST, boot and shutdown included; the VM powered off by QMP `system_powerdown` at the end.
- Pixel positions and colours are measured with PIL on the 1024x768 screenshots (±2 px).

## 1. View > "Hide Commit Details" (Ctrl+Shift+D)

- **Before** (VERIFIED mb2, zoom mb2z): the View menu, opened by a press on "View", reads top to
  bottom: "Show Uncommitted Changes" Ctrl+1 · "Show All Commits" Ctrl+2 — "Hide Commit Details"
  Ctrl+Shift+D — "Show HEAD" Ctrl+0 — "Hide Tags" · "Hide Stashes in Commit List" · "Show Lost
  Commits (Reflog)" Ctrl+Shift+. · "Collapse All Merges (Show First Parent)" · "Filter by Active
  Branch" Ctrl+Shift+A (— = a rule). No item carries a mark; the labels start at x≈46, the menu's
  right border at x=361.
- **Chosen** (VERIFIED mb3): the detail pane collapses to its tab strip alone ("Commit", "Changes",
  "File Tree" and the two icons, at the window's bottom), the history taking the space ("Initial
  commit" now visible).
- **View reopened while hidden** (VERIFIED mb4, zooms mb4z, mb4zz): **the label is unchanged —
  "Hide Commit Details"** (not "Show Commit Details") — and a **check mark "✓"** is drawn to the
  left of the label, in a gutter of its own: the glyph at x 48-56, y 86-93 (about 9x8 px), the same
  colour as the label text (#EAE8E8), **no box and no highlight** behind it (the row's background
  stays the menu's #2C2C2C). Opening a gutter for the mark **shifts every label of the menu right
  by 18 px** (x 46 → 64, checked or not) and **widens the menu by 18 px** (right border 361 → 379);
  the shortcuts move right with it.
- **Chosen again** (VERIFIED mb5): the pane back at its earlier height. **View reopened** (VERIFIED
  mb6): pixel-identical to mb2 over the menu's area (PIL difference of the region 0,0-400,250: none)
  — no mark, labels back at x≈46, right border back at 361.

## 2. View > "Show Lost Commits (Reflog)" (Ctrl+Shift+.)

- **Chosen** from the View menu (mb6's open menu) (VERIFIED mb7, zoom mb7z): a strip appears across
  the top of the history column only (x 302 to the window's right edge, y≈118-139, above the
  column headers' place; the sidebar unaffected), background **#333333** with a 1 px #525252 rule
  above and below it, reading **"Reflog mode enabled"** (body text colour) followed by **"Exit"** in
  blue (#3C8DD7, a link). The history gains reflog rows — at the top **"On main: WIP on parser"** and
  **"index on main: 431a622 Local only commit"** (the stash's own commits, their subjects in the
  dimmer grey of an unreachable row) — and the history's lanes change (a red stub at the stash row).
  The detail pane is unchanged.
- **View reopened while on** (VERIFIED mb8, zooms mb8z, mb8zz): **the label is unchanged — "Show Lost
  Commits (Reflog)"** with **Ctrl+Shift+.** — and a **check mark "✓"** to its left, drawn exactly as
  question 1's (same gutter, #EAE8E8, no box, no highlight). Again every label shifts right by 18 px
  (x≈46 → 64) and the menu's right border moves 361 → 379, though only this one item is checked.
- **Chosen again** from that menu (VERIFIED mb9): the strip and the reflog rows are gone; the
  history is as in mb1. (The strip's "Exit" link was not pressed; it was not needed to turn the mode
  off.)

## 3. Moving between the menu bar's menus

- **Opened by a press** (VERIFIED mb10, File; also mb2 and mb6, View): the menu opens with **no item
  highlighted** — every row on the menu's #2C2C2C (the pointer stays on the menu's name, so no
  hover band either). File's items, for the record: "Init New Repository..." Ctrl+Shift+N ·
  "Clone..." Ctrl+N — "New Tab" Ctrl+T · "Open Repository..." Ctrl+O · "Quick Launch..." Ctrl+P ·
  "Close Tab" Ctrl+W · "Restore Closed Tab" Ctrl+Shift+T — "Configure SSH Keys..." · "Accounts..." —
  "Check for Updates..." — "Preferences..." Ctrl+, — "Exit". The open menu's name is drawn on a
  lighter box (as the second survey measured, #424242).
- **Hover switches menus while one is open** (VERIFIED mb11): with File open, the pointer moved
  (no press) onto "View": **File's menu closed and View's opened in its place**, "View" now on the
  box; again no item highlighted. Moving the pointer off the menu bar into the window (VERIFIED
  mb12) left View's menu open.
- **Left/Right arrows move to the adjacent menu** (VERIFIED): with View open, **Right** opened
  **Repository**'s menu (mb13); **Left, Left** went back through View to **File** (mb14); **Left**
  from File **wrapped round to Help** (mb15). After each arrow the new menu opened with **no item
  highlighted**.
- **↓ in a menu with nothing highlighted** (VERIFIED mb16): highlights the **first item** ("Contact
  Support..." in Help) with the hover band.
- Repository's items, for the record (mb13): "Refresh" F5 — "Add .gitignore..." — "Fetch..."
  Ctrl+Shift+F · "Pull..." Ctrl+Shift+L · "Push..." Ctrl+Shift+P — "Save Stash..." Ctrl+Shift+H —
  "New Branch..." Ctrl+Shift+B · "New Tag..." Ctrl+Shift+G · "New Worktree..." — "Git Flow" ▸ · "Git
  LFS" ▸ — "Apply Patch..." — "Bisect" — "Open In File Explorer" Ctrl+Alt+O · "Open In Console"
  Ctrl+Alt+T — "Repository Statistics..." · "Repository Treemap..." · "Performance Benchmark..." —
  "Settings for This Repository...". Help's (mb15): "Contact Support..." — "Open @git_fork on
  Twitter" · "Visit Fork Website" — "Issue Tracker" · "Keyboard Shortcuts" · "Release Notes" — "Fork
  Activation..." — "About Fork".

## 4. Help > "About Fork"

VERIFIED (mb17, zoom mb17z): a small borderless window, ~240x348 px (x 392-631, y 176-523),
centred over the history and detail area, background **#2B2B2B**, a **1 px blue border #3BACED**
(the Windows accent of an active window) all round. **No title bar and no title text**; its one
control is a **"✕"** at the top right (no minimize or maximize). Top to bottom, every line
centred:

1. The **Fork logo**: a blue disc with a white rim and a white fork, ~80 px across.
2. **"Fork"** — large (~30 px), light grey.
3. **"Version 2.23.2.0"** — small, white.
4. **"www.fork.dev"** — small, blue and underlined (a link).
5. **"Designed and implemented by"** — small, dimmer grey.
6. **"Dan Pristupov"** — small, white.
7. **"Tanya Pristupova"** — small, white.
8. **"Copyright © 2026 Dan Pristupov"** — small, white.
9. **"Legal"** — smaller, grey, underlined (a link).

**No buttons**: no OK, Close or other labelled button, so nothing filled or primary. The ✕ hovered
(VERIFIED mb18, zoom mb18z) is a red (#E81123) box with a white ✕, the Windows caption close's
hover. **Escape did not close it** (VERIFIED mb19: still open after Escape, its blue border kept).
A press on the main window's detail pane behind it (VERIFIED mb20) did not close it either and it
kept its blue active border (the ✕ stayed drawn red although the pointer had left it); whether
the main window was blocked (modal) was not tested further. Closed by pressing **✕** (VERIFIED mb21).
Neither link was pressed.

## 5. The caption row (menus and window title)

- **Double press on its empty space maximizes and restores** (VERIFIED): with the window maximized
  (mb21), a double press at (350, 14) — between "Help" and the title "Fork" — **restored** it to a
  normal window over the desktop (mb22: the window at about x 25-1024, y 100-698, the caption's
  middle button now the single-square Maximize). A double press on the restored caption's empty
  space at (380, 114) **maximized** it again (mb25).
- **Right press on its empty space opens the Windows system menu** (VERIFIED): restored, at (380,
  114) (mb23, zoom mb23z): the standard light Windows system menu (#F2F2F2, 1 px #CCCCCC border)
  at the pointer, reading **"Restore"** (greyed) · **"Move"** · **"Size"** · **"Minimize"** (with its
  dash glyph) · **"Maximize"** (with its square glyph) — rule — **"Close"** bold, **"Alt+F4"** right,
  ✕ glyph. It is not drawn in Fork Dark. Maximized, at (350, 14) (mb26): the same menu with
  **"Restore"** enabled and **"Move"**, **"Size"** and **"Maximize"** greyed. **Escape closed it**
  each time, nothing chosen (mb24, mb27).

## Extras seen on the way (menu bar)

- **Pressing the open menu's name closes it** (VERIFIED mb28, zoom mb28z): View opened by a press,
  then "View" pressed again: the menu closed. Right after, with the pointer still on "View", the
  name was drawn without its box.
- **Hovering a name with no menu open** (VERIFIED mb29, zoom mb29z): "Repository" is drawn on the
  lighter rounded box; no menu opens.
- **Alt pressed and released alone** (VERIFIED mb30, zoom mb30z): every menu name shows its access
  key underlined — **F**ile, **V**iew, **R**epository, **W**indow, **H**elp — and "File" is drawn on
  the box (keyboard focus in the menu bar), no menu open. **Escape** left that state: underlines and
  box gone (mb31, zoom mb31z).

## State left behind

Hide Commit Details and Show Lost Commits both off (mb6, mb11: View's menu as at the start), the
window maximized as found (mb27), the theme untouched (Fork Dark), no repository written.
