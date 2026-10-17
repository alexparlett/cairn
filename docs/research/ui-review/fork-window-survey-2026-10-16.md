# Fork for Windows, the whole window surveyed surface by surface (2026-10-16)

A layout and anatomy survey, for comparing Cairn's window against Fork's. Driven in the Windows 10 VM
(QEMU/KVM, 1024x768 guest frame) through `qmp.py`; screenshots `uv*.png` in `~/VMs/fork-win10/shots/`.
VERIFIED = seen on screen in the named shot. NOT OBSERVED = not seen, with why. Pixel sizes are
measured on the 1024x768 screenshots and are approximate (±2 px).

## What was driven

(filled in below as observed)

- **Fork for Windows 2.23.2.0** (Help > About Fork, "Version 2.23.2.0", uv46, zoom uv46z) — newer
  than the 2.21.1 the 2026-10-10/11 observations recorded, so Fork updated itself in between (its
  Preferences > Updates reads "Develop Channel", question 10). Windows 10 22H2. Git for Windows
  2.56.0.windows.2 is installed (`git --version` in Git Bash, uv1), but Fork runs its own bundled
  git, **2.50.1.windows.1 - Fork git instance** (Preferences > Git, uv32).
- Guest frame 1024x768 (first screenshot's size); clicks landed where aimed, so the resolution is
  unchanged. The VM signed in on its own.
- Repository: `C:\uirev\t`, built in Git Bash by `docs/research/ui-review/fixture-recipe.md`, typed
  verbatim in four chunks, bare remote `C:\uirev\remote.git`. Checked afterwards (uv0: `git status`,
  `git stash list`, `git branch -vv`; uv1: `git log --oneline --graph --all --decorate`, `git tag -n`):
  `main` at 431a622 "Local only commit", "ahead 1" of `origin/main`; `feature/login` 4dbc983 tracking
  `origin/feature/login`; `bugfix/crash` 048f0e7; tags `v0.1` (07bec3d, "Add note 6") and `v0.2`
  (annotated, "Release 0.2", on the merge f93015d); `stash@{0}: On main: WIP on parser`; staged
  `modified: src/login.rs`, unstaged `modified: README.md`, untracked `new.txt`. Matches the recipe.
- Fork opened with three earlier repository tabs (`r`, `n`, `a`) restored; `C:\uirev\t` opened
  by Ctrl+O ("Open Repository..." folder picker, "Select Folder") as a fourth tab, `t`. Fork's window
  was already maximized (the caption shows the restore glyph).

## 1. The whole window: All Commits, `main`'s tip selected, Commit tab

VERIFIED (uv2; zooms uv2a, uv2b top bar, uv2c sidebar, uv2d/uv2e history, uv2f detail).

Top to bottom, at 1024x768 maximized:

| Band | y (px) | Height | Contents |
| --- | --- | --- | --- |
| Menu bar + caption, one row | 0-25 | ~26 | Left: menus **"File"**, **"View"**, **"Repository"**, **"Window"**, **"Help"**. Centre: window title **"Fork"** (the app name only; not the repository). Right: a smiley-face icon (feedback), then the window's minimize, restore, close. No separate Windows title bar: menus share the caption row. |
| Toolbar | 26-66 | ~41 | Icon over a grey label, left to right: a sidebar-toggle icon (two-pane rectangle, no label); **"Quick Launch"** (folder icon); gap; **"Fetch"** (curved arrow, down); **"Pull"** (curved arrow down, solid) ; **"Push"** (curved arrow up, solid) with a grey circular badge **"1"** at its top right (the commit ahead); gap; **"Stash"** (box/archive icon) with a separate **⌄** dropdown chevron right of it; then the **status box** centred (x≈362-662, ~300x36, light grey rounded rectangle): a list glyph (three lines) at its left, and centred on two lines the repository name **"t"** (bold) over a branch glyph and **"main"**; then **"Branch"** (branch icon with a +) with a **⌄** chevron; far right: **"Open in"** (curved arrow, with a small ⌄ beside the icon), **"Console"** (a terminal window icon showing `C:\`), **"Appearance"** (two-pane icon with ⌄), **"Work"** (stacked windows icon with ⌄). Labels are all the same mid grey (~#808080); none looked greyed out relative to the others. |
| Repository tab strip | 67-93 | ~26 | One tab per open repository, equal widths across the window: **"r"**, **"n"**, **"a"**, **"t"** (active: white, the rest grey). Each tab has a small grey dot at its left (x≈10, 260, 510, 760) and its name centred. A **"+"** at the far right. |
| Notification banner | 94-117 | ~24 | **⚠** (yellow triangle) **"Consider adding a .gitignore file to your repository"**, right-aligned buttons **"Add .gitignore..."** and **"Close"**. Spans the whole width, above sidebar and list. |
| Main area | 118-728 | ~610 | Sidebar left, history list right top, detail pane right bottom. |

Splitters seen: one vertical between sidebar and the right side (a 1 px line at x=301; the sidebar
is 0-300, ~301 px wide, at this width); one horizontal between the history list (y 118-422, ~305 px
tall) and the detail pane (1 px line at y≈423; the detail pane's tab strip 424-455, its body 456-728).
No splitter inside the detail pane in the Commit tab. The history list has a vertical scrollbar at
its right edge (x≈1012-1022).

Sidebar (0-300): the repository name **"t"** bold at top left with a gear/cog icon at the top
right; **"Local Changes (3)"** (document icon); **"All Commits"** (stacked-pages icon), highlighted
grey (selected); a divider; a centred pair of icons — a branch icon (blue, selected) and a magnifier
(search) — a two-tab switch; then a **"Filter"** box (magnifier glyph, placeholder "Filter"); then
the ref sections (question 3).

History list (302-1012): no column header row is drawn. Columns left to right: graph (x≈305-340
for this history), refs + subject, author (avatar square with initials **"AL"** + **"Ada Lovelace"**,
from x≈665), short SHA (x≈813, 7 chars, e.g. **"431a622"**), date (x≈878, **"16 Oct 2026 12:12"**).
The selected row is a filled blue band (~#236BD2) with white bold text. Details in question 4.

Detail pane: tabs **"Commit"** (selected, blue text with a blue underline), **"Changes"**,
**"File Tree"**; at the right of the tab strip two icons: an open-in-new-window (box with arrow) icon
and a layout icon (a rectangle with a bar: the pane's placement). Body: question 5.

## 2. The toolbar in detail

VERIFIED (uv2a, uv2b zooms; tooltips uv3, uv3b, uv3d, uv3e, uv3f, combined in uv3all).

- Groups, left to right, separated by gaps: [sidebar toggle] [Quick Launch] — [Fetch] [Pull]
  [Push] — [Stash ⌄] — [status box] — [Branch ⌄] — … — [Open in ⌄] [Console] [Appearance ⌄] [Work ⌄].
- Each button: an icon ~20 px over a label ~11-12 px tall text (cap height ~8 px, so roughly a 9 pt
  Segoe UI), the pair ~40 px tall, buttons ~45-60 px wide; hovering draws a light grey rounded
  rectangle behind the button (uv3, uv3b, uv3e).
- Dropdown chevrons: **Stash** and **Branch** have a separate small **⌄** to the right of the
  button (its own hit area, at the label's height); **Open in**, **Appearance** and **Work** have a
  small **⌄** beside the icon (above the label). **Quick Launch**, **Fetch**, **Pull**, **Push** and
  **Console** have none.
- **Push** carries a grey filled circle badge with **"1"** (white) at its icon's top right: the
  commit `main` is ahead. **Pull** has no badge (nothing behind).
- Tooltips, verbatim: hovering **Fetch**: two lines, **"Fetch"** / **"Hold Ctrl for Quick Fetch"**
  (uv3). Hovering **Push**: **"Push"** / **"Hold Ctrl for Quick Push"** (uv3b). Hovering the sidebar
  toggle, the status box and **Branch** showed no tooltip within ~2.5 s (uv3e, uv3d, uv3f); moving
  from Push to Stash left the Push tooltip standing over Stash (uv3c), so Stash's own tooltip was not
  read: NOT OBSERVED.
- The status box on hover: a **funnel (filter) icon** appears at its right end (x≈645) beside the
  repository/branch text (uv3d); the list glyph stays at its left. Not hovered: no funnel (uv2).

## 3. The sidebar

VERIFIED (default state uv2, zoom uv2c; every folder opened uv4, zoom uv4z; hover uv5-uv5d,
combined uv5all; search tab uv6, zoom uv6z).

Top to bottom (sidebar 0-300 px wide):

1. Header row (y≈133): repository name **"t"** bold, left; a gear icon, right.
2. **"Local Changes (3)"** — a document-with-plus icon; the count in parentheses is the number of
   changed paths (README.md, src/login.rs, new.txt).
3. **"All Commits"** — a stacked-pages icon; selected: a grey rounded highlight (~#D9D9D9) across
   the sidebar width, ~22 px tall.
4. A divider, then a two-icon switch centred: a branch icon (blue when active — the refs view) and a
   magnifier (the search view). The search view (uv6z) shows a heading **"Search Commits"** (bold,
   larger), a field with a magnifier and a small ⌄ and placeholder **"Commit message"**, and under it
   a dropdown **"Commit Message ⌄"** (the search kind); nothing else.
5. The **"Filter"** field (refs view): full width, light grey fill, a magnifier glyph and placeholder
   **"Filter"**, ~22 px tall, directly above the sections.
6. Sections, each a bold heading with a **⌄**/**›** chevron at x≈13, in this order:
   **"Branches"**, **"Remotes"**, **"Tags"**, **"Stashes"**, **"Submodules"**. No counts on the
   headings. **"Submodules"** has no chevron (empty). There is no Worktrees section.

Default disclosure (uv2): Branches open, its folders **bugfix** and **feature** closed; Remotes
open, **origin** closed; **Tags closed**; Stashes open; Submodules empty.

Opened (uv4z), the rows are:

```
⌄ Branches
   ⌄ [folder] bugfix
        [branch] crash              (icon and name lighter grey)
   ⌄ [folder] feature
        [branch] login
   ✓ main  (bold)                                   1↑
⌄ Remotes
   ⌄ [cloud] origin
      ⌄ [folder] feature
           [branch] login
        [branch] main
⌄ Tags
     [tag] v0.2
     [tag] v0.1
⌄ Stashes
     [box] WIP on parser
  Submodules
```

- Row anatomy: chevron (folders only), an icon (an open-folder outline for a folder, a branch fork
  glyph for a branch, a cloud with a branch inside for a remote, a tag outline for a tag, an archive
  box for a stash), then the name. Rows are ~22 px apart; each nesting level indents ~12 px.
- The current branch: a **check mark ✓ in place of the branch icon**, the name **bold**.
- Ahead/behind: right-aligned **"1↑"** on `main` (the count, then an up arrow). `feature/login` (in
  step with its upstream) shows nothing; no tracking indicator or upstream name is drawn on any row.
  Hovering the "1↑" showed no tooltip (uv5d).
- A folder's rows show the leaf name only (`crash`, `login`), the folder holding the prefix; remotes
  are grouped under the remote's name and foldered the same way (`origin` › `feature` › `login`).
- Tags are listed **v0.2 above v0.1** (newest/descending), with no distinction drawn between the
  annotated and the lightweight tag.
- The stash row reads its message, **"WIP on parser"**, not `stash@{0}`.
- Hovering a branch row (`main`, uv5all) adds three icons at its right, after the count: a **pin**
  (tooltip **"Pin 'main'"**), a **funnel** (tooltip **"Show 'main' commits only"**) and an **eye**
  (tooltip **"Hide 'main' in the commit list"**).

## 4. The history list

VERIFIED (uv2, zooms uv2d, uv2e; graph at 4x uv7; scrolled to the end uv7b; merge selected uv8).

- **No column header row.** The list starts directly under the banner with its first row; there is
  no header to right-click, so which columns can be shown or hidden was NOT OBSERVED here (the View
  menu, question 9, is where to look).
- Columns, left to right (x at 1024 wide): graph (305-~340 here, as wide as the lanes need); refs as
  chips, then the subject; author — a ~14 px rounded square avatar in lavender with white initials
  **"AL"**, then **"Ada Lovelace"** (x≈665-800); short SHA, 7 characters (x≈813-865); date
  **"16 Oct 2026 12:12"** (`d Mon yyyy HH:mm`, local time; x≈878-975). Then the vertical scrollbar.
  Only the subject column's text is cut, with an ellipsis: **"Merge branch 'feature/log..."**.
- Row pitch **23 px** (row centres 133, 156, 179, …). Text ~12 px Segoe UI (cap height ~8-9 px).
- Selected row: a filled blue band (#236BD2) with rounded ends, the whole row's text white and bold.
  Before the list had focus, the selected row was a grey band with dark text (seen in a working
  screenshot taken as the repository opened, overwritten and not kept: treat as NOT OBSERVED). `HEAD`'s row (`main`, "Local only commit") is bold when not selected too
  (uv7b), its author, SHA and date bold as well.
- Rows not reachable from `HEAD` — `stash@{0}` and `bugfix/crash`'s commit — are drawn with their
  subject, author, SHA and date in light grey (uv2e).
- Graph (uv7): `main`'s lane orange at the far left, a 2 px line; commits are small filled dots
  (~5 px) on the lane. The branch merged in (`feature/login`) and `bugfix/crash` are a second lane in
  a lighter yellow-orange, joined to the main lane by rounded curves (no diagonal straight lines).
  **The merge commit's node is a larger hollow circle (~14 px) with a ⌄ chevron inside it** (the
  collapse handle of the merged branch); when the merge row is selected the circle is filled orange
  (uv8). The stash's row sits above `main`'s, its dot on `main`'s lane with the line running down to
  `main` — no lane of its own. `main`'s row also has a small blue dot just left of its chip (x≈334),
  besides its lane dot.
- Ref chips (uv7), each a rounded rectangle, ~17 px tall, before the subject:
  - local branch: light-yellow fill, orange-yellow border, the name — **"bugfix/crash"**,
    **"feature/login"**; the current branch has a **✓** before the name and bold text: **"✓ main"**;
  - remote branch: a split chip, a cloud-with-branch icon segment then the name, orange border:
    **"origin/main"**;
  - a local branch at the same commit as its upstream: **two chips**, a small chip holding the cloud
    icon alone, then the local branch's chip **"feature/login"** — the remote's name is not written;
  - tag: lavender border, a tag-outline icon segment then the name: **"v0.2"**, **"v0.1"**; the
    annotated and lightweight tags look the same;
  - stash: grey border, an archive-box icon segment then **"stash@{0}"**, and the subject in grey
    **"On main: WIP on parser"** (git's own stash message, not the sidebar's "WIP on parser");
  - `HEAD` has no chip of its own.
  - Order on one row: remote chip before the tag (**origin/main**, then **v0.2**).

## 5. The detail pane's Commit tab

VERIFIED (main's tip uv2, zoom uv2f; Initial commit uv7b; the merge uv8; a file pressed uv9, zoom
uv9z).

Top to bottom, for `main`'s tip:

1. A small grey caps label **"AUTHOR"**.
2. A ~42 px lavender avatar square with **"AL"**, beside it **"Ada Lovelace"** (bold-ish) then
   **"ada@example.com ⌄"** (grey, the ⌄ a dropdown), and under them **"16 Oct 2026 12:12:33 +01:00"**
   (seconds and the offset). No separate committer block when author and committer agree.
3. Right-aligned grey labels with values beside them: **"REFS"** — chip(s), here **"main"** (a
   plain grey-bordered chip, no ✓); for the merge **"origin/main"** (cloud icon segment) and
   **"v0.2"** (tag icon segment); **"SHA"** — the full 40-character id
   **"431a62275114ef0dbdf79946c4a7a264119b7b79"**; **"PARENTS"** — short ids as blue links:
   **"f93015d"**; for the merge two, **"34145ac  4dbc983"**. For a commit with no refs (Initial
   commit) the REFS row is absent; PARENTS is present with nothing after it.
4. A rule, then the subject in bold: **"Local only commit"** (the body under it, if any; none here).
5. A rule, then **"Expand All"** right-aligned, then the changed files, one per row: a **›** chevron,
   a status badge (yellow square **"M"** for modified, green square **"+"** for added), a document
   icon, the path **"notes.txt"**. For the merge, the list is the change against its first parent:
   **"src/login.rs"** (added).
6. Pressing a file (uv9z): its chevron turns to **⌄** and its diff opens in place beneath the row
   (hunk header **"@@ -0,0 +1,2 @@"** then green added lines), and **"Expand All"** becomes
   **"Collapse All"**. The pane gains a vertical scrollbar.

## 6. The detail pane's Changes tab, for a commit (the merge, f93015d)

VERIFIED (uv10, zooms uv10z, uv10y; tooltips uv11, one hover each, shots uv11_0 … uv11_10).

- Under the tabs, a one-line summary (y≈437-455): avatar **"AL"**, **"Ada Lovelace"**, the id at
  ten characters **"f93015d03"** (grey), **"16 Oct 2026 12:12"** (grey), the subject in bold
  **"Merge branch 'feature/login'"**.
- Below it a vertical splitter divides the pane: the file list left (x≈302-580, ~278 px) and the
  diff right (581-1024).
- File list's bar: a magnifier (filter field, no placeholder seen) at its left; at its right an
  **eye** icon (tooltip **"Quick Look (space bar)"**) and a **lines** icon (tooltip
  **"View Mode"**). The list is a tree by default here: **"⌄ [folder] src"**, under it
  **"[+ green badge] [doc icon] login.rs"**, selected (grey band). The first file was chosen without
  a press.
- Diff bar, left to right: **^** (tooltip **"Go to previous change, Ctrl+↑"**), **⌄** (tooltip
  **"Go to next change, Ctrl+↓"**); centred, a document icon and the path **"src/login.rs"** with
  the folder part grey and the file name dark; right group: a space-bar glyph (**"Ignore
  whitespaces"**), **¶** (**"Show hidden symbols"**), a wrap glyph (**"Word wrap"**), **−≡**
  (**"Decrease number of visible lines"**), **+≡** (**"Increase number of visible lines"**), **↕≡**
  (**"Show entire file"**), a two-column glyph (**"Side by side diff"**). All tooltips single-line,
  verbatim as quoted.
- Diff body: the hunk header **"@@ -0,0 +1,2 @@"** in grey monospace, then each line with a small
  grey line-number gutter (**1**, **2**; one number column for an added file) and the text on a
  light green band: **"pub fn login() {}"** (with syntax colouring: `pub`, `fn` red, `{}` dark),
  **"// validate"** (green, a comment). No +/− marker column. Monospace ~12 px. A vertical scrollbar
  at the far right.

## 7. Local Changes

VERIFIED (uv12, zooms uv12a lists' bar, uv12b Staged heading, uv12c commit box; tooltips uv13,
shots uv13_0 … uv13_8).

Pressing **"Local Changes (3)"** turns its row grey (selected) and replaces the history and detail
pane with a three-part view (the sidebar stays):

- Left column (x≈302-552, ~250 px), split by a horizontal splitter at y≈425 into **Unstaged** (top,
  118-424) and **Staged** (425-728). Right column (553-1024): the diff (118-615), under it the commit
  box (616-728). A vertical splitter between the columns at x≈552.
- A bar across the left column's top (y≈118-135): a magnifier (the filter; no placeholder text
  drawn) at the left; at the right a **double-chevron ⌄⌄** (tooltip **"Stage All"**), an **eye**
  (tooltip **"Show diff side-by-side (space bar)"**) and a **square speech/list glyph** (tooltip
  **"View Mode"**).
- **"Unstaged"** heading, bold ~14 px, with a bordered **"Stage"** button at its right (enabled:
  the list has a selection). Rows (~20 px apart): **"[green + badge] [doc icon] new.txt"** (the
  untracked file, selected: grey band, chosen without a press) then **"[yellow M badge] [doc icon]
  README.md"**. Order is case-insensitive by name; root files have no folder row.
- **"Staged"** heading with an **"Unstage"** button at its right, **greyed** (nothing selected in
  Staged). Rows: **"⌄ [folder] src"**, then **"[yellow M] [doc] login.rs"** under it — a tree view.
  The double-chevron Unstage All is not drawn on the Staged heading (only one ⌄⌄ in the shared bar).
  Hovering "Stage" and "Unstage" showed no tooltip.
- Diff (right): the same bar as the Changes tab's: **^ ⌄**, the file icon and **"new.txt"**
  centred, then the whitespace, ¶, wrap, −≡, +≡, ↕≡ and side-by-side glyphs (the side-by-side one is
  off the screen edge at x≈1015). Body: **"@@ -0,0 +1,1 @@"**, line **1** **"untracked"** on green.
- Commit box (uv12c), one bordered field of two parts: **"Commit subject"** (placeholder) with a
  **lines icon** at its right end (tooltip **"Recent Commit Messages"**), a thin rule, then
  **"Description"** (placeholder), ~22 px each, ~440 px wide. Under it: a check box **"Amend"**
  (left) and the split button **"Commit 1 File"** with a separate **⌄** (right), both greyed while the
  subject is empty. No character counter is drawn while the subject is empty. Hovering the button and
  its ⌄ showed no tooltip.

## 8. Context menus

Each opened by a right press, read, then closed with Escape; no item chosen. A rule (`—`) marks a
separator; shortcut text is right-aligned in grey; *(greyed)* marks a disabled item. Right-pressing
a ref in the sidebar also **selects that ref's commit in the history** and shows it in the detail
pane (uv17-uv20: the row turns grey-selected).

**(a) A commit row, "Add note 5"** (VERIFIED uv22): "New Branch..." Ctrl+Shift+B · "New Tag..."
Ctrl+Shift+G — "Interactively Rebase 'main' to Here..." — "Reset 'main' to Here..." — "Checkout
Commit..." · "Cherry-pick Commit..." · "Revert Commit..." · "Save as Patch..." — "Compare to Local
Changes" — "Copy Commit SHA" Ctrl+C · "Copy Commit Info" Ctrl+Shift+C. No submenus, nothing greyed.

**(b) `main` in the sidebar** (VERIFIED uv16, zoom uv16z; Tracking open uv21, zoom uv21z):
"Checkout..." — "Fast-Forward to 'origin/main'" · "Pull 'origin/main'..." · "Push to 'origin'..." —
"Merge into 'main'..." *(greyed)* · "Rebase 'main' on 'main'..." *(greyed)* · "Interactively Rebase
'main' on 'main'..." (not greyed) — "New Branch..." Ctrl+Shift+B · "New Tag..." Ctrl+Shift+G —
"Tracking ›" · "Rename..." F2 · "Delete..." Delete — "Copy Branch Name" Ctrl+C.
Tracking submenu: "Remove tracking reference" — "origin/feature/login" · "✓ origin/main".
Plainly odd: "Checkout..." is offered on the branch already checked out, "Fast-Forward to
'origin/main'" is offered (and not greyed) though `main` is ahead of it, and "Interactively Rebase
'main' on 'main'..." is enabled where its non-interactive sibling is greyed.

**(c) `bugfix/crash`** (VERIFIED uv17): "Checkout..." — "Push to 'origin'..." — "Merge into
'main'..." · "Rebase 'main' on 'bugfix/crash'..." · "Interactively Rebase 'main' on
'bugfix/crash'..." — "New Branch..." Ctrl+Shift+B · "New Tag..." Ctrl+Shift+G — "Tracking ›" ·
"Rename..." F2 · "Delete..." Delete — "Copy Branch Name" Ctrl+C. (No Fast-Forward or Pull: it has no
upstream.) Its Tracking submenu was not opened: NOT OBSERVED.

**(d) `origin/main` under Remotes** (VERIFIED uv18): "Checkout..." — "Pull into 'main'..."
Ctrl+Shift+L — "Merge into 'main'..." · "Rebase 'main' on 'origin/main'..." · "Interactively Rebase
'main' on 'origin/main'..." — "New Branch..." Ctrl+Shift+B · "New Tag..." Ctrl+Shift+G — "Delete..."
Delete — "Copy Branch Name" Ctrl+C.

**(e) `v0.2` under Tags** (VERIFIED uv19): "Checkout Commit..." · "Show Tag Details..." — "Push to
'origin'..." — "Merge into 'main'..." · "Rebase 'main' on 'v0.2'..." · "Interactively Rebase 'main'
on 'v0.2'..." — "New Branch..." Ctrl+Shift+B · "New Tag..." Ctrl+Shift+G — "Delete..." Delete —
"Copy Tag Name" Ctrl+C.

**(f) The stash, "WIP on parser"** (VERIFIED uv20): "Apply..." · "Rename..." · "Save as Patch..." —
"Delete..." Delete. Selecting it shows the stash's Changes tab: summary "Ada Lovelace d1cbf9d87
16 Oct 2026 12:12 On main: WIP on parser", file `src/main.rs`, diff `// wip` added.

**(g) `README.md` in Unstaged** (VERIFIED uv14, zoom uv14z; Ignore open uv14b, zoom uv14bz):
"Open" Ctrl+Shift+Alt+O (drawn lighter, apparently greyed) · "Show in File Explorer" —
"Blame/Timeline..." · "History..." — "Stage" Enter · "Discard changes..." Delete — "Stage All"
Ctrl+Shift+Alt+S — "Ignore ›" — "Stash 1 File..." · "Save as Patch..." — "Copy Path" Ctrl+C ·
"Copy Full Path" Ctrl+Shift+C. Ignore submenu: "Ignore 'README.md'" · "Ignore All .md Files..." —
"Custom Pattern...".

**(h) `src/login.rs` in Staged** (VERIFIED uv15): "Open" Ctrl+Shift+Alt+O (lighter, apparently
greyed) · "Show in File Explorer" — "Blame/Timeline..." · "History..." — "Unstage" Enter —
"Unstage All" Ctrl+Shift+Alt+S — "Stash 1 File..." · "Save as Patch..." — "Copy Path" Ctrl+C ·
"Copy Full Path" Ctrl+Shift+C. No discard and no Ignore on the staged side. Note Stage All and
Unstage All share the chord Ctrl+Shift+Alt+S (each in its own list).

**(i) `notes.txt` in a commit's file list ("Add note 5", Changes tab)** (VERIFIED uv23; Reset File
to open uv23b, zoom uv23bz): "Open" Ctrl+Shift+Alt+O (here not greyed) · "Show in File Explorer" —
"Reset File to ›" — "Blame/Timeline..." · "History..." · "Show in File Tree" — "Save as..." —
"Copy Path" Ctrl+C · "Copy Full Path" Ctrl+Shift+C. Reset File to submenu: "State At Commit..." ·
"State Before Commit...".

## 9. The menu bar

Each menu opened by a press on its name and closed with Escape; nothing chosen. `—` a separator.

**File** (VERIFIED uv24): "Init New Repository..." Ctrl+Shift+N · "Clone..." Ctrl+N — "New Tab"
Ctrl+T · "Open Repository..." Ctrl+O · "Quick Launch..." Ctrl+P · "Close Tab" Ctrl+W · "Restore
Closed Tab" Ctrl+Shift+T — "Configure SSH Keys..." · "Accounts..." — "Check for Updates..." —
"Preferences..." Ctrl+, — "Exit".

**View** (VERIFIED uv25, zoom uv25w): "Show Uncommitted Changes" Ctrl+1 · "Show All Commits"
Ctrl+2 — "Hide Commit Details" Ctrl+Shift+D — "Show HEAD" Ctrl+0 — "Hide Tags" · "Hide Stashes in
Commit List" · "Show Lost Commits (Reflog)" Ctrl+Shift+. · "Collapse All Merges (Show First
Parent)" · "Filter by Active Branch" Ctrl+Shift+A. No column choices here: the history's columns
are not configurable from the View menu, and there is no header to right-click (question 4).

**Repository** (VERIFIED uv26): "Refresh" F5 — "Add .gitignore..." — "Fetch..." Ctrl+Shift+F ·
"Pull..." Ctrl+Shift+L · "Push..." Ctrl+Shift+P — "Save Stash..." Ctrl+Shift+H — "New Branch..."
Ctrl+Shift+B · "New Tag..." Ctrl+Shift+G · "New Worktree..." — "Git Flow ›" · "Git LFS ›" —
"Apply Patch..." — "Bisect" — "Open In File Explorer" Ctrl+Alt+O · "Open In Console" Ctrl+Alt+T —
"Repository Statistics..." · "Repository Treemap..." · "Performance Benchmark..." — "Settings for
This Repository...". Submenus (VERIFIED uv43, uv44, combined uv43z): Git Flow ›
"Initialize Git Flow..."; Git LFS › "Initialize Git LFS".

**Window** (VERIFIED uv27, zoom uv25w): "Select Previous Tab" Ctrl+Shift+Tab · "Select Next Tab"
Ctrl+Tab — "Switch Theme" — "Zoom In" Ctrl+= · "Zoom Out" Ctrl+-.

**Help** (VERIFIED uv28): "Contact Support..." · "Open @git_fork on Twitter" · "Visit Fork
Website" — "Issue Tracker" · "Keyboard Shortcuts" · "Release Notes" — "Fork Activation..." —
"About Fork".

No item in any of the five menus was drawn greyed.

## 10. Preferences (File > Preferences..., Ctrl+,)

VERIFIED (General uv29, uv30 with the Windows Defender toast closed, zoom uv30z; Commit uv31; Git
uv32; Integration uv33; Custom Commands uv34; Updates uv35; closed with Escape, uv36). Nothing changed.

- A modal dialog with no title text, a close **✕** at its top right, a tab row, and a **"Close"**
  button at the bottom right (no OK/Cancel: settings apply as made). The dialog **resizes to each
  tab's content** (General ~575x645, Commit ~575x310, Updates ~575x170; Integration wider, ~695x580).
- Tabs, left to right: **"General"**, **"Commit"**, **"Git"**, **"Integration"**,
  **"Custom Commands"**, **"Updates"**. There is **no Appearance/Theme tab and no Diff tab**: the
  diff's settings are a section of General, and the theme is in the toolbar's Appearance dropdown
  (below).
- **General**, top to bottom: **"Source Code Folder"** — a list holding **"⚠ C:\Users\fork\"** with a
  folder button, and a **"+"** under it; **"Diff View"** — a live preview of a diff (hunk header
  "@@ -1,3 +1,3 @@", two number gutters, a removed line on red with the changed word on a stronger
  red, an added line on green), then **"☐ Show -+ marks"** and **"Font size: [13]"**, and
  **"☐ Disable syntax highlighting"**; **"Sort commits:"** **"○ Topologically"** / **"● By date"**;
  then check boxes **"☑ Fetch remotes automatically"**, **"☐ Always include tags into fetch"**,
  **"☑ Tab indicator for uncommitted changes"**, **"☑ Update submodules automatically on
  checkout"**, **"Space character in branch names: [- ⌄]"**, **"☐ Push automatically on commit"**,
  **"☑ Compact branch labels"**.
- **Commit**: **"Commit Message"** — **"Length indicator: Low limit: [50] High limit: [70]"**,
  **"Spell checking: [Disable ⌄]"**, **"Page guide at column: [72]"**; **"Generate Commit Message"** —
  **"OpenAI: [Log In...]"**.
- **Git**: **"Git Instance: [2.50.1.windows.1 - Fork git instance ⌄]"** — Fork runs its own bundled
  git 2.50.1, not the installed Git for Windows 2.56.0; **"☐ Enable verbose Git output"**;
  **"Global User Information"** — a "?" avatar square, fields with placeholders **"User name"** and
  **"Email"**, **"Change the avatar at gravatar.com"** (link).
- **Integration**: **"Issue Tracker Integration"** — "☑ Highlight issue tracker links in commit
  messages"; **"External Merge Tools"** and **"External Diff Tools"** — each a list (Araxis Merge,
  Beyond Compare, CodeBuddy, Cursor, …, with + and −) beside Name / Path ("External tool location") /
  Arguments; **"Shell"** — Shell [Default ⌄], Shell Path
  `C:\Users\fork\AppData\Local\Fork\gitInstance\2.50.1\bin\bash.exe`, Arguments.
- **Custom Commands**: an empty list with + and −, an empty pane showing Fork's logo.
- **Updates**: **"Automatic Updates: [Develop Channel ⌄]"**.

The toolbar's **Appearance ⌄** (VERIFIED uv37): a menu with a grey heading **"Theme"**, **"✓ Light"**,
**"Dark"**, a rule, a grey heading **"Commit List Layout"**, **"✓ Horizontal"**, **"Vertical"**. The
toolbar's **Work ⌄** (VERIFIED uv38): heading **"Workspaces"**, **"Home"**, **"✓ Work"**, a rule,
**"Configure..."** — so the toolbar's "Work" label is the current workspace's name.

## 11. The start page, and a narrower window

**Repository Manager (File > New Tab, Ctrl+T)** — VERIFIED (uv39, zoom uv39z). A new tab titled
**"Repository Manager"** opens at the end of the tab strip (the other tabs narrow to fit). With it
active:

- The menu bar shrinks to **"File"**, **"Window"**, **"Help"** (no View, no Repository).
- The toolbar keeps every button, but Fetch, Pull, Push, Stash, Branch, Open in and Console are
  greyed; Quick Launch, Appearance and Work stay enabled. The status box reads **"Welcome to Fork!"**
  (bold) over **"Open a repository to start"**.
- Left pane (~350 px): heading **"Repository Manager"** (large), then **"⌄ Recent"** — `t` (selected,
  blue), `r`, `n`, `a` — and **"⌄ Repositories"** — `a`, `n`, `r`, `t` (alphabetical); each row a
  small repository icon and the name.
- Right pane, for the selected repository: **"t"** (large) with the path **"C:\uirev\t ➔"** under
  it and an **"Open"** button at the right; a card: **"Uncommitted Files: 2"**, **"Commits: 15"**,
  **"Initial Commit: 15 minutes ago"**, **"Last Commit: 15 minutes ago"**, and **"Remotes:"**
  **"[cloud] origin"**; tabs **"Summary"** (selected) / **"Statistics"**; cards **"Branches"**
  (rows: branch icon, name, last subject, author), **"Tags"** (tag icon, name, author, "15 minutes
  ago"), **"Commits"** (commit glyph, subject, author, relative time).
- Where Fork contradicts itself or looks wrong here: **"Uncommitted Files: 2"** beside the sidebar's
  **"Local Changes (3)"** (the untracked `new.txt` apparently not counted); **"Commits: 15"** counts
  the stash's two commits ("On main: WIP on parser", "index on main: 431a622 Local only commit" are
  listed first under Commits) where the history holds 13; and the **Branches card lists only remote
  branches** — "origin/HEAD", "origin/main", "origin/feature/login" — none of the three local ones.
- The tab was then closed with Ctrl+W.

**A narrower window** — VERIFIED (restored ~1000 px wide uv40; dragged to ~800 px uv41, zoom uv41z;
to ~600 px uv42). Restoring the window keeps every region; the sidebar keeps its width (~300 px)
throughout.

- At ~800 px: the toolbar drops **"Open in"** first (gone); the status box overlaps the Push
  button, cutting its label to **"Pus"** and hiding **Stash** behind it; Branch, Console, Appearance,
  Work stay. In the history, the **author, SHA and date columns keep their widths and the
  refs+subject column shrinks** to ~130 px, chips and subjects cut with "…" (e.g. "✓ main Loc…",
  "origin/mai"). The tab strip and banner shrink proportionally. The detail pane's diff bar loses
  its side-by-side button off the right edge.
- At ~600 px: the content **stops reflowing and is clipped** at the window's right edge — the
  caption's close button, the banner's "Close", Appearance's label ("ppearance"), the date column
  ("16 O") and the diff are cut off; Pull, Push, Stash are hidden behind the status box; the refs and
  subject column is a single chip or a two-letter "F…". The sidebar still keeps ~300 px. This looks
  like a minimum layout width below which Fork clips rather than adapts (a Fork bug or limit, not a
  design to copy without deciding).
- Fork's window was maximized again afterwards (uv45).
- Also seen in uv45: hovering a **folder** row in the sidebar (`bugfix`) shows the funnel and eye
  icons at its right, without the pin.

## What was not observed

- The selected history row's colours while the list is unfocused (the screenshot was not kept).
- Stash's toolbar tooltip (the Push tooltip lingered over it), and the Tracking submenu of
  `bugfix/crash`.
- A history column chooser: none exists in the View menu, and there is no header row to right-click.
