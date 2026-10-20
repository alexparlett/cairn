# Fork for Windows' Copy items and its dialogs in Dark (2026-10-20)

Observed by the fork-observer agent for fork-alignment, before Cairn's copy actions and dialog
chrome were settled. Fork for Windows 2.23.2, Dark theme, Windows 10 22H2, git 2.50.1.windows.1
(as recorded by the earlier runs on this image; this run did not open About or run `git
--version` to check them again), the guest at 1024×768 (the first screendump's size). Repository: the ui-review fixture (`fixture-recipe.md`) at `C:\uirev\t`, in the
recipe's state (` M README.md`, `M  src/login.rs`, `?? new.txt`, the stash; "Local Changes (3)"
in the sidebar, cp3). Each clipboard text was read by pasting it into Notepad (Ctrl+A, Ctrl+V,
Ctrl+End) and reading the text and Notepad's status bar "Ln, Col", which places the caret after
the paste: a trailing newline would leave it at Col 1 of a further line. Shots
`~/VMs/fork-win10/shots/cp1` to `cp88`, zooms suffixed with a letter (`<n>a` the Notepad text,
`<n>b` its status bar). Nothing was committed, discarded, staged or created. Each answer
VERIFIED by the shots named, unless marked NOT OBSERVED.

## 1. Copy Commit Info

- **The commit row's context menu** (on "Add note 5", cp4): "New Branch… Ctrl+Shift+B", "New
  Tag… Ctrl+Shift+G", rule, "Interactively Rebase 'main' to Here…", rule, "Reset 'main' to
  Here…", rule, "Checkout Commit…", "Cherry-pick Commit…", "Revert Commit…", "Save as Patch…",
  rule, "Compare to Local Changes", rule, **"Copy Commit SHA Ctrl+C"**, **"Copy Commit Info
  Ctrl+Shift+C"**. On the merge row, labelled `origin/main` and `v0.2`, the menu starts with
  "origin/main ›" and "v0.2 ›" submenus above the same items (cp7).
- **"Add note 5" → Copy Commit Info** pasted as exactly **`635ee8e - Add note 5`**: the 7-character
  short id, a space, a hyphen, a space, the subject; one line, **no trailing newline** (Ln 1,
  Col 21 for 20 characters; cp6, cp6a, cp6b). No author, no date.
- **The merge commit → Copy Commit Info**: **`f93015d - Merge branch 'feature/login'`**, one line,
  no trailing newline (Ln 1, Col 39; cp8, cp8a, cp8b). The fixture's merge was made with `-m`, so
  its message is a subject alone; **no commit in the fixture has a body**, so whether a body is
  included is NOT OBSERVED (making one would change the repository).
- **Ctrl+Shift+C with the history focused on "Add note 5"** (row clicked, focused blue, cp9), no
  menu: the clipboard held the same **`635ee8e - Add note 5`**, no trailing newline (Ln 1,
  Col 21; cp10).

## 2. Copy Commit SHA, Ctrl+C, and two rows

- **Copy Commit SHA from the menu** on "Add note 4" (cp11) pasted as
  **`2f83b6f15e4a464f8763894d67fa658a1904d4c1`**, the full 40-character id, no trailing newline
  (Ln 1, Col 41; cp12). Right-pressing a row that was not selected made it the selected commit
  (the Commit tab showed "Add note 4", cp11).
- **Ctrl+C with the history focused on "Add note 3"**, no menu: **`e3a0d1e9d5c29b4cc7b5fdafb06c3e104563fb25`**,
  full id, no trailing newline (Ln 1, Col 41; cp13, cp14).
- **Two rows selected** ("Add note 3" pressed, "Add note 5" Ctrl-pressed; both drawn blue, the
  detail pane switching to the Changes tab with both commits in its header, cp15). **Right-pressing
  inside the selection opens a different, shorter menu** (cp16, zoom cp16a): "Save Commit Range as
  Patch…", rule, "Cherry-pick…", rule, "Drop…", rule, "Copy Commit SHA Ctrl+C", "Copy Commit Info
  Ctrl+Shift+C". **The copy items keep their singular wording** — no "SHAs".
- **Copy Commit SHA for two** pasted as two lines, one full id each, **separated by a line break,
  no trailing newline** (Ln 2, Col 41; cp17):

  ```
  635ee8ecb7f85006b4c0a7343dfb062526051b61
  e3a0d1e9d5c29b4cc7b5fdafb06c3e104563fb25
  ```

  The order is **the list's order, top (newer) first, not the order pressed**: Ctrl+C with "Add
  note 3" pressed first gave this text (cp20, cp21, though the clipboard already held it from
  cp17), and so did Ctrl+C with "Add note 5" pressed first after the clipboard had been
  overwritten with `x` from Notepad (cp22, cp23), so that paste is that press's. Whether the break
  is CRLF or LF: NOT OBSERVED (Notepad's status bar reads "Windows (CRLF)", which describes its
  document, not necessarily what was pasted).
- **Copy Commit Info for two** (cp18): two lines in the same order, no trailing newline (Ln 2,
  Col 21; cp19):

  ```
  635ee8e - Add note 5
  e3a0d1e - Add note 3
  ```

## 3. The sidebar

From here each copy was preceded by putting `x` on the clipboard from Notepad, so a press that
copies nothing would paste `x`.

- **Local branch `bugfix/crash` selected** (pressed in the sidebar, drawn focused blue with its
  pin, filter and eye glyphs at the row's right, the history selecting its commit, cp24), **Ctrl+C**:
  **`bugfix/crash`**, the full short name with its folder, no trailing newline (Ln 1, Col 13; cp25).
  Its menu (cp35): "Checkout…", rule, "Push to 'origin'…", rule, "Merge into 'main'…", "Rebase
  'main' on 'bugfix/crash'…", "Interactively Rebase 'main' on 'bugfix/crash'…", rule, "New Branch…
  Ctrl+Shift+B", "New Tag… Ctrl+Shift+G", rule, "Tracking ›", "Rename… F2", "Delete… Delete", rule,
  **"Copy Branch Name Ctrl+C"**.
- **Remote branch `main` under Remotes › origin** → its menu (cp27): "Checkout…", rule, "Pull into
  'main'… Ctrl+Shift+L", rule, "Merge into 'main'…", "Rebase 'main' on 'origin/main'…",
  "Interactively Rebase 'main' on 'origin/main'…", rule, "New Branch… Ctrl+Shift+B", "New Tag…
  Ctrl+Shift+G", rule, "Delete… Delete", rule, **"Copy Branch Name Ctrl+C"**. Copy Branch Name
  pasted as **`origin/main`** — the remote's name included — no trailing newline (Ln 1, Col 12;
  cp28).
- **Tag `v0.2` selected**, **Ctrl+C**: **`v0.2`**, no trailing newline (Ln 1, Col 5; cp29, cp30).
  Its menu (cp34): "Checkout Commit…", "Show Tag Details…", rule, "Push to 'origin'…", rule, "Merge
  into 'main'…", "Rebase 'main' on 'v0.2'…", "Interactively Rebase 'main' on 'v0.2'…", rule, "New
  Branch… Ctrl+Shift+B", "New Tag… Ctrl+Shift+G", rule, "Delete… Delete", rule, **"Copy Tag Name
  Ctrl+C"**.
- **Ctrl+Shift+C with the sidebar focused on `v0.2`** (cp31): **the clipboard was unchanged** —
  it pasted the `x` put there beforehand (Ln 1, Col 2; cp32). No sidebar menu shows a Ctrl+Shift+C
  item (cp27, cp34, cp35).
- Seen by accident: a scripting slip sent an `x` keystroke, Ctrl+A and Ctrl+C to Fork with the
  sidebar focused (meant for Notepad). Afterwards every sidebar row — branches, remotes, tags and
  the stash — was drawn selected, and the detail pane read "5 commits selected" (cp33). The slip
  changed nothing else that the shots show, and the test it spoiled was run again (cp31, cp32).

## 4. File lists

### Local Changes

- Opened by pressing "Local Changes (3)" (cp36): Unstaged lists **new.txt above README.md**, Staged
  `src` › login.rs as a tree.
- **README.md's menu** (cp37): "Open Ctrl+Shift+Alt+O", "Show in File Explorer", rule,
  "Blame/Timeline…", "History…", rule, "Stage Enter", "Discard changes… Delete", rule, "Stage All
  Ctrl+Shift+Alt+S", rule, "Ignore ›", rule, "Stash 1 File…", "Save as Patch…", rule, **"Copy Path
  Ctrl+C"**, **"Copy Full Path Ctrl+Shift+C"**.
- **Copy Path** on README.md: **`README.md`** (Ln 1, Col 10; cp38). **Copy Full Path**:
  **`C:\uirev\t\README.md`** (Ln 1, Col 21; cp39). Neither has a trailing newline.
- **A nested path, `src/login.rs` in Staged** (its menu, cp48: "Open", "Show in File Explorer",
  "Blame/Timeline…", "History…", "Unstage Enter", "Unstage All Ctrl+Shift+Alt+S", "Stash 1 File…",
  "Save as Patch…", "Copy Path Ctrl+C", "Copy Full Path Ctrl+Shift+C"): **Copy Path pastes
  `src\login.rs` — with a backslash, not git's slash** (Ln 1, Col 13; cp49, zoom cp49a); **Copy Full
  Path `C:\uirev\t\src\login.rs`** (Ln 1, Col 24; cp50, cp50a); **Ctrl+C with the row focused gives
  the same `src\login.rs`** (cp51, cp51a). None has a trailing newline.
- **Two files** (README.md pressed, new.txt Ctrl-pressed; both blue, cp40). The menu inside the
  selection (cp46) greys "Blame/Timeline…" and "History…", drops "Stage All", reads "Stash 2
  Files…", and keeps **"Copy Path Ctrl+C"** and **"Copy Full Path Ctrl+Shift+C"** in the singular.
  - **Copy Path** pasted two lines, no trailing newline (Ln 2, Col 8; cp47):

    ```
    README.md
    new.txt
    ```

  - **Ctrl+C** with the list focused, right after the presses, gave the same `README.md` /
    `new.txt` (Ln 2, Col 8; cp41).
  - **The order is the order the files were pressed, not the list's** (the list shows new.txt
    first) — **but it does not stay that way**: after Fork was brought forward again from Notepad, the diff
    had moved from README.md to new.txt with both rows still selected (cp43), and then **Ctrl+C gave
    `new.txt` / `README.md`** (Ln 2, Col 10; cp44, cp45) and **Ctrl+Shift+C gave
    `C:\uirev\t\new.txt` / `C:\uirev\t\README.md`** (Ln 2, Col 21; cp42) — the list's order. The
    copied order follows the order of Fork's selection, and something that happened when Fork
    regained focus (a refresh, presumably; not seen directly) reset that order to the list's and
    moved the diff to the first file. Press order on its own is not a stable rule.

### A commit's Changes tab

- "Initial commit" (the one commit touching two files) selected; its Changes tab lists `src` ›
  main.rs and README.md (cp52). **main.rs pressed, Ctrl+C**: **`src\main.rs`** (Ln 1, Col 12;
  cp53, cp54, cp54a). **Ctrl+Shift+C**: **`C:\uirev\t\src\main.rs`** — the working tree's path,
  though the file shown is the commit's (Ln 1, Col 23; cp55, cp55a). No trailing newline on either.
- Its menu (cp56, zoom cp56a): "Open Ctrl+Shift+Alt+O" (greyed), "Show in File Explorer", rule,
  "Reset File to ›", rule, "Blame/Timeline…", "History…", "Show in File Tree", rule, "Save as…",
  rule, **"Copy Path Ctrl+C"**, **"Copy Full Path Ctrl+Shift+C"**.

### The Commit tab's file rows

- **Right-pressing a file row under the message opens a menu** (src/main.rs, cp58, zoom cp58a): the
  same items as the Changes tab's — "Open" (greyed), "Show in File Explorer", rule, **"Reset File
  To ›"** (a capital T here, "Reset File to" in the Changes tab), rule, "Blame/Timeline…",
  "History…", "Show in File Tree", rule, "Save as…", rule, "Copy Path", "Copy Full Path" — but
  **with no shortcuts drawn**. Its Copy Path pasted **`src\main.rs`** (Ln 1, Col 12; cp59).

## 6. Dialogs in Dark

Colours are sampled pixels of the screendump; text colours are the brightest pixel of each run of
glyphs (Windows draws the text with ClearType, so its edges are coloured fringes, cp61b).

### (a) Discard changes

- README.md's menu → "Discard changes…" (cp60) opens a dialog centred over the window (cp61,
  zoom cp61a): the Fork icon at the left (a blue disc with a white fork, about 56 px, x 262-317),
  **"Discard changes"** as its title, **"Do you want to discard all your changes in the selected
  files?"** under it, a ✕ at its top right, and **"Discard"** then **"Cancel"** at its bottom
  right — the primary on the left. No title bar of its own.
- **Box**: x 237-786, y 277-422 (550×146), **background #2B2B2B**, a **1 px border #3BACED** (a
  light blue) on all four sides, square corners; outside the border the window behind is darker
  by a few levels for several pixels (#252526 next to the border on the left, #29292A five pixels
  out), a soft shadow.
- **Title**: #FFFFFF, heavier than the body (semibold or bold by eye, cp61a), cap height 11 px
  ("D" at y 309-319). **Body**: #CAC6C6, regular, cap height 9 px (y 331-339). The ✕ is #F8F8F8.
- **Discard** (x 633-697, y 378-401, 65×24): fill **#373737**, a **1 px #0078D7 border** (its left
  edge blended, #13578C), text **#EAE8E8**. **Cancel** (x 706-765, 60×24): the same fill #373737
  and text #EAE8E8, a **1 px #525252 border** (#565656 along its bottom). The corners of both are
  slightly rounded (cp61b). So the primary differs from Cancel by its blue border alone.
- **Focus on open is not drawn**: no button shows a focus mark (cp61b). **Tab once** (cp62, zoom
  cp62a): **Cancel gains a dashed blue rectangle inset inside its border**, and Discard keeps its
  blue border — so that border marks the default button, not focus. **Tab again** (cp63, cp63a):
  no mark anywhere — the dialog is pixel-identical to its state on open (only the taskbar
  differs). **Tab a third time** (cp64, cp64a, cp64b): **Discard gains the same dotted/dashed
  inset rectangle**, inside its blue border. NOT OBSERVED: which control had focus when the
  dialog opened, and what the unmarked second stop is (the ✕ showed no mark there either).
- Escape closed it; README.md was still listed in Unstaged (cp65).

### (b) Create Branch

- **Ctrl+Shift+B** from Local Changes opens it (cp66, zoom cp66a): the Fork icon, **"Create
  Branch"**, **"Use '/' as a path separator to create folders"**, "Create branch at:" ⑂ main,
  "Branch name:" a field holding a ⑂ glyph and the placeholder "Enter branch name" with the
  caret in it, an unticked **"Check out after create"**, and **"Create"** (greyed) then
  **"Cancel"**. The same chrome as Discard changes: x 212-811, y 249-448 (600×200), background
  #2B2B2B, 1 px #3BACED border, the same shadow outside. Title #FFFFFF; the subtitle #CAC6C6; the
  labels "Create branch at:" and "Branch name:" #FFFFFF; "main" and the check box's label
  #EAE8E8; the placeholder #999898.
- **The field focused**: fill #333333, a 1 px **#0078D7** border; unfocused (cp69) its border is
  #525252. **The check box**: a square about 16 px, fill #333333, 1 px #6B6B6B border, corners
  slightly rounded (cp68b).
- **Create disabled** (cp66): fill #343434, no border distinct from its fill (#353535), text
  **#707070**. **With "x" typed** (cp67, zooms cp67a, cp67b) Create enables: fill **#373737**, a
  **1 px #0078D7 border** (its edges blended to #105F9F-#165A8F where they fall between pixels),
  text **#EAE8E8**. **Cancel**: fill #373737, **1 px #525252 border**, text #EAE8E8. So, as in
  Discard changes, the enabled primary differs from Cancel by its blue border alone; both are
  24 px tall with slightly rounded corners.
- **Focus, Tab by Tab from the field** (each zoomed `a`): Tab 1 changed nothing on screen — cp68 is
  pixel-identical to cp67, the field still drawn focused; Tab 2 put focus on **Create: a dotted
  #2D5184 rectangle (two pixels on, one off) inset two pixels inside its blue border**, the
  field's border turning #525252 (cp69, cp69b); Tab 3 moved the same dotted rectangle to
  **Cancel**, inside its grey border (cp70); Tab 4 showed no mark anywhere (cp71); Tab 5 returned
  to the field, blue border and caret (cp72). Tab, Tab again reached Create (cp73), and **Shift+Tab
  from Create put focus on the check box: a black dotted rectangle (#151515 on #2B2B2B, nearly
  invisible on the dark ground) around the box and its label together** (cp74, zooms cp74a,
  cp74b); Shift+Tab again returned to the field (cp75). So the check box is a tab stop, drawn with
  WPF's black dotted rectangle, which the forward Tab from the field did not visibly reach; what
  the first forward Tab and the fourth stop focus is NOT OBSERVED.
- Escape closed it; no branch "x" appeared in the sidebar (cp76).
- Seen beside it: after Ctrl+Shift+B, the window behind kept drawing its Shift-held labels —
  "Stage All", "Unstage All", "Commit 1 File and Push" — after the dialog closed (cp66, cp76),
  until Shift was pressed and released (cp77).

### (c) Git Error

- NOT OBSERVED: it needs a failing write, which this run was not to make.

## 5. Ctrl+C in the commit box's subject

- "hello world" typed into the subject and selected with Shift+Home (drawn selected, the counter
  reading 39, Commit 1 File enabled; cp78), **Ctrl+C copied `hello world`** (Ln 1, Col 12, no
  trailing newline; cp79). The subject was then cleared with Ctrl+A, Backspace and shows its
  placeholder "Commit subject" again (cp80).

## 7. Tab from the history

- With "Add note 3" pressed in the history (cp81), **Tab moves focus between the history and the
  sidebar and nowhere else**: Tab 1 drew the sidebar's selected `bugfix/crash` blue and the history
  row grey (cp82); Tab 2 the history row blue and the sidebar's grey (cp83); and so on through Tab 6
  (cp84-cp87). Between each pair of shots the only pixels that changed lie in those two selected
  rows' band (y 299-353), so **no focus ring appeared on any other control**.

## Also seen

- Notepad was closed without saving (cp88) and the VM was then shut down. The run took about 16
  minutes (10:58-11:14 guest time).
