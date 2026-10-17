# Cairn's window, surface by surface, over the ui-review fixture (2026-10-16)

What Cairn's real window drew over the fixture of `fixture-recipe.md`, recorded so it
can be compared side by side with Fork for Windows over the same repository. Only what
was on the screen is recorded; nothing here is filled in from the source.

- **Cairn commit:** `967bb72eef2c3a1ef31cd09a874bc5ff1040846e` (branch `feature/plan-ui-review`, no local edits to tracked files).
- **Build:** `cargo build --workspace`, dev profile; binary `target/debug/cairn` (the
  `cairn-app` package's binary is named `cairn`), run as `timeout 900 target/debug/cairn <fixture>/t`.
- **Host:** Hyprland 0.56.2 (Lua config) on Wayland, one 3440x1440 monitor at scale 1, Cairn's
  dark theme. The window has no client-side decorations: there is no OS title bar, and the
  1024x768 frame is all Cairn's own drawing.
- **Window size:** exactly **1024x768**, floating, set with
  `hyprctl dispatch "hl.dsp.window.resize({ x = 1024, y = 768, relative = false, window = 'address:…' })"`
  (this Hyprland takes Lua dispatchers; the classic `resizewindowpixel` spelling is refused)
  and captured with `grim -g "400,200 1024x768"`. One extra shot (`cairn-3-tall.png`) is
  1024x1200, labelled where it is used.
- **How driven:** keyboard only. `wtype -k Down/Up/Tab/Escape` for plain keys. Chords with
  a physical-key binding (Ctrl+Alt+1/2, Ctrl+Shift+B) did nothing through `wtype`, whose
  virtual keymap does not carry the physical key code, and were sent with Hyprland's
  `hl.dsp.send_shortcut({ mods = '…', key = '…', window = 'address:…' })` instead, which did
  work. There is no pointer tool: a state reachable only by a click is marked NOT CAPTURED.
- **Pixel sizes** are measured from the 1024x768 PNGs (scale 1, so 1 px = 1 logical px).
  Font sizes are estimates from cap height (a 9 px capital ≈ 13 px type in this sans face).

## The fixture as git describes it

Built in `/tmp/claude-1000/-home-alexparlett-Development-cairn/7792de58-51d8-448f-92d4-988b20e0534e/scratchpad/uirev/t`
exactly as the recipe says. Every commit has the same timestamp (the recipe runs in one second).

```
$ git log --oneline --graph --all --decorate
* e50958d (bugfix/crash) Fix crash on empty input
| *   8d0d64f (refs/stash) On main: WIP on parser
| |\
| | * 03fa6ab index on main: 748ca2e Local only commit
| |/
| * 748ca2e (HEAD -> main) Local only commit
|/
*   8f71429 (tag: v0.2, origin/main) Merge branch 'feature/login'
|\
| * 7708126 (origin/feature/login, feature/login) Validate login input
| * cf477f1 Add login module
* | af50cb9 Fix README typo
|/
* f882f6f (tag: v0.1) Add note 6
* 73ce8c1 Add note 5
* ef37d8d Add note 4
* 4d51721 Add note 3
* 3ad1e33 Add note 2
* 2396f3f Add note 1
* 3b9c691 Initial commit

$ git status
On branch main
Your branch is ahead of 'origin/main' by 1 commit.
  (use "git push" to publish your local commits)

Changes to be committed:
  (use "git restore --staged <file>..." to unstage)
	modified:   src/login.rs

Changes not staged for commit:
  (use "git add <file>..." to update what will be committed)
  (use "git restore <file>..." to discard changes in working directory)
	modified:   README.md

Untracked files:
  (use "git add <file>..." to include in what will be committed)
	new.txt
```

After the session, `git branch -a` and `git status --short` were unchanged (no branch
created, nothing staged or discarded).

## Summary

| State | Shot | Status |
| --- | --- | --- |
| (a) Window as it opens, All Commits | `~/.cache/cairn/ui-review-shots/cairn-1.png` | CAPTURED |
| (b) A commit selected, Commit tab | `cairn-2.png` (stash row), `cairn-3.png` (`Local only commit`), `cairn-3-tall.png` (1024x1200) | CAPTURED (files list under the message NOT CAPTURED, below the fold) |
| (c) Changes tab | `cairn-4.png` (`Local only commit`) | CAPTURED |
| (d) The merge commit | `cairn-5.png` (Changes tab), `cairn-6.png` (Commit tab) | CAPTURED |
| (e) Local Changes | none | NOT CAPTURED — no keyboard route reaches it |
| (f) Create Branch dialog (Ctrl+Shift+B), Escaped | `cairn-7.png` (open), `cairn-8.png` (after Escape) | CAPTURED |
| (f) Show Lost Commits (Ctrl+Shift+.) | `cairn-9.png` | NOT CAPTURED — the chord as sent changed nothing |
| (f) Tab focus order | contact sheets only (not kept) | Observed, described below |

All shots are in `/home/alexparlett/.cache/cairn/ui-review-shots/`.

---

## (a) The window as it opens — CAPTURED

Shot: `~/.cache/cairn/ui-review-shots/cairn-1.png` (1024x768). No row selected.

### Layout (px)

- **Title bar:** y 0–~40, full width, same background as the rest (no visible rule under it).
- **Sidebar:** x 0–239 (**240 px wide**), from y ≈ 48 down to the bottom.
- **Vertical splitter:** x 240–243 (4 px, a lighter grey `#2d2d2d` on `#141414`).
- **History list:** x 244–1024, header y ≈ 42–66, rows from y ≈ 68 to the horizontal splitter.
- **Horizontal splitter** between the history and the detail pane: y ≈ 503–507.
- **Detail pane:** y ≈ 508–768 (~260 px tall). In the 1024x1200 shot it is the same height:
  a taller window grows the history, not the pane.

### Title bar (left to right)

- `Cairn` — larger sans, cap height 12 px (≈ 17 px type).
- `t*` — the repository's directory name `t`, starred (bold, small).
- A branch glyph (⑂-like), then `main` (≈ 13 px).
- `1↑` — ahead count with an up arrow.
- Right side: `14 commits` (grey, ≈ 13 px), then a bordered button **`Fetch origin`**
  (larger text, cap height 12 px ≈ 17 px; ~100x24 px at x ≈ 911–1011).
- No toolbar row of buttons exists; nothing else is in the title bar.

### Sidebar (top to bottom, rows ~22 px apart; text ≈ 13 px)

| y (centre) | Row |
| --- | --- |
| 58 | `Local Changes (3)` |
| 80 | `All Commits` — selected: a lighter full-width band (~22 px) |
| 97–124 | Text box, placeholder `Filter` (~27 px tall, ~228 px wide, rounded border) |
| 141 | `▼ Branches` (bold, section heading, grey-white) |
| 163 | `▶ 🗀 bugfix` (disclosure triangle, folder outline glyph) |
| 185 | `▶ 🗀 feature` |
| 207 | `✓ main` (bold) … right-aligned `1↑` at x ≈ 217–232 |
| 229 | `▼ Remotes` |
| 251 | `▶ 🗀 origin` |
| 273 | `▼ Tags` |
| 295 | tag glyph + `v0.1` |
| 317 | tag glyph + `v0.2` |
| 339 | `▼ Stashes` |
| 361 | stash (box) glyph + `On main: WIP on parser` |

Row anatomy: a leading glyph column (triangle, check or ref-kind glyph) at x ≈ 8–18, text at
x ≈ 23 (folders and leaves of a section are not indented further than their heading's
text; the folder triangle sits at x ≈ 10 and the folder glyph at x ≈ 23, its name at x ≈ 38).
Folders are collapsed. No counts beside headings. No icons on the section headings.

### History list

**Header** (grey, ≈ 13 px), left to right: `Graph and subject` (x ≈ 254, baseline about 5 px
higher than the other three), a check box with `Show Lost Commits` (box at x ≈ 488–501,
label to x ≈ 630, unchecked), `Author` (x ≈ 640), `Commit` (x ≈ 800), `Date (UTC)` (x ≈ 882).
A thin rule under the header at y ≈ 66.

**Columns:** graph x ≈ 250–340 (~90 px for 4 lanes); subject from x ≈ 348 to ≈ 630 (chips
first, then the subject); author x ≈ 640–790; short id x ≈ 800–870; date x ≈ 882–1000.
**Row height 26 px** (row centres 80, 106, 132, …). Text ≈ 13 px (cap height 9 px), white;
id and date the same face and size, not monospace in the list.

**Rows, top to bottom, verbatim** (chips in `[ ]`, a chip's glyph described in parentheses):

| # | Chips | Subject | Author | Commit | Date (UTC) |
| --- | --- | --- | --- | --- | --- |
| 1 | `[(box) stash@{0}]` light-blue outline | `On main: WIP on parser` | Ada Lovelace | 8d0d64f | 2026-10-16 11:09 |
| 2 | `[bugfix/crash]` amber outline, no glyph | `Fix crash on empty input` | Ada Lovelace | e50958d | 2026-10-16 11:09 |
| 3 | `[✓ main]` light-blue outline | **`Local only commit`** (bold) | Ada Lovelace | 748ca2e | 2026-10-16 11:09 |
| 4 | `[(tag) v0.1]` indigo fill | `Add note 6` | Ada Lovelace | f882f6f | 2026-10-16 11:09 |
| 5 | `[(cloud) feature/login]` green fill | `Validate login input` | Ada Lovelace | 7708126 | 2026-10-16 11:09 |
| 6 | `[(cloud) origin/main]` amber fill, `[(tag) v0.2]` indigo fill | `Merge branch 'feat…` (cut with an ellipsis at x ≈ 625) | Ada Lovelace | 8f71429 | 2026-10-16 11:09 |
| 7 | — | `Add note 5` | Ada Lovelace | 73ce8c1 | 2026-10-16 11:09 |
| 8 | — | `Add login module` | Ada Lovelace | cf477f1 | 2026-10-16 11:09 |
| 9 | — | `Fix README typo` | Ada Lovelace | af50cb9 | 2026-10-16 11:09 |
| 10 | — | `Add note 4` | Ada Lovelace | ef37d8d | 2026-10-16 11:09 |
| 11 | — | `Add note 3` | Ada Lovelace | 4d51721 | 2026-10-16 11:09 |
| 12 | — | `Add note 2` | Ada Lovelace | 3ad1e33 | 2026-10-16 11:09 |
| 13 | — | `Add note 1` | Ada Lovelace | 2396f3f | 2026-10-16 11:09 |
| 14 | — | `Initial commit` | Ada Lovelace | 3b9c691 | 2026-10-16 11:09 |

Chips are ~18 px tall, rounded, text ≈ 12 px; `feature/login` is one chip (the local branch
and `origin/feature/login` at the same commit, cloud glyph); `main` and `origin/main` are at
different commits and so are separate chips. Below row 14 the list is empty to y ≈ 503.

**Graph as drawn** (lanes at x ≈ 261 blue, 275 amber, 289 green, plus dashed lines at
x ≈ 317 yellow and x ≈ 331 dark blue):

- Blue lane: dots at rows 1 (stash), 3 (main), 4 (Add note 6), 7, 10–14; solid line from row 1 down to row 14.
- Amber lane: dot at row 2 (bugfix/crash), a hollow ring at row 6 (the merge), a dot at row 9
  (Fix README typo); a short diagonal from the blue lane at row 3 joining it.
- Green lane: dots at rows 5 and 8 (the feature/login commits), joined by a solid line; a
  short pink dashed chevron from row 5's dot out right and back towards the merge ring.
- Two dashed lines leave row 4's blue dot to the right, run down at x ≈ 317 (yellow) and
  x ≈ 331 (dark blue), and turn back left: the yellow one into row 8's green dot, the blue
  one into row 9's amber dot.

Observed, not interpreted: rows are not in the order `git log --graph --all` prints.
`Add note 6` (row 4) is drawn above three of its descendants (rows 5, 6, 8, 9), and
`Add note 5` (row 7) sits between the merge and `Add login module`; the dashed lines are
the edges that run upward from those descendants to `Add note 6`. Fork, given the same
same-second timestamps, is the comparison to make here.

### Detail pane (no selection)

- Tab strip (y ≈ 508–538, ~30 px): `Commit` (selected: white text, a blue underline ~68 px
  wide under it), `Changes` (grey). Right: a bordered button **`Collapse`** (larger text,
  ≈ 17 px; ~78x24 px at x ≈ 934–1012).
- Body: centred grey text `Select a commit to see its details.` (≈ 13 px) at y ≈ 653.

---

## (b) A commit selected, Commit tab — CAPTURED

The first Down arrow selected row 1 (the stash); two more selected row 3. A selected row is a
lighter grey band the full width of the list (x 244–1024), 26 px tall; no focus outline.

### b1. The stash's row — `cairn-2.png`

Commit tab, verbatim:

- Two columns of small grey caps headings: `AUTHOR` (x ≈ 256, y ≈ 551) and `COMMITTER` (x ≈ 639).
- Under each: `Ada Lovelace <ada@example.com>` (≈ 13 px, white), then `16 Oct 2026 12:09:55 +01:00`.
- Right-aligned grey labels at x ≈ 300–329 with values at x ≈ 338:
  - `REFS` → chip `[(box) stash@{0}]`
  - `SHA` → `8d0d64fc15cc57d24c40d9b9b23f72c7bdbf3f46` (monospace, ≈ 12 px)
  - `PARENTS` → two underlined blue links `748ca2e` `03fa6ab` (monospace)
- A rule at y ≈ 694, then the message in bold: `On main: WIP on parser`.
- A rule at y ≈ 742, then a button `Expand All` at the right edge, cut by the window's bottom
  edge at y = 768 (only its top half shows).

### b2. `Local only commit` (HEAD) — `cairn-3.png`

Same layout: `AUTHOR` / `COMMITTER` both `Ada Lovelace <ada@example.com>`,
`16 Oct 2026 12:09:55 +01:00`; `REFS` `[✓ main]`;
`SHA` `748ca2e579db6e5fd84edbea29b0993ab28922f6`; `PARENTS` `8f71429` (one link); message
**`Local only commit`**; `Expand All` cut at the bottom edge.

Field rows are ~24 px apart (y 575, 598, 622, 646, 670). The history list keeps showing all
fourteen rows; the stash's row and the bold subject of row 3 are unchanged by selection.

**Files list under the message: NOT CAPTURED.** It is below the pane's bottom edge at
1024x768, and the pane does not grow with the window (`cairn-3-tall.png`, 1024x1200, shows
the same ~260 px pane with `Expand All` still at its foot). Scrolling the pane needs the
pointer; dragging the splitter needs the pointer.

---

## (c) The Changes tab — CAPTURED

Shot: `cairn-4.png`, `Local only commit` selected, Ctrl+Alt+2.

- Tab strip: `Commit` grey, `Changes` selected (white, blue underline, a slightly lighter tab
  background ~75 px wide); `Collapse` at right.
- One-line summary (y ≈ 553): `Ada Lovelace` (bold) `748ca2e` `16 Oct 2026 12:09:55 +01:00`
  `Local only commit` (bold), single spaces of ~12 px between.
- **Left: file list**, x ≈ 248–512 (~265 px), then a vertical divider at x ≈ 515–518.
  - `Filter` text box (~27 px tall) at y ≈ 573–599.
  - One row (~24 px, selected band): a bold amber `M` badge at x ≈ 252, then `notes.txt` in
    monospace (≈ 12–13 px) at x ≈ 280.
- **Right: diff**, x ≈ 520–1024.
  - Bar (y ≈ 571–595): two square buttons `^` and `v` (previous / next change, ~24x24 px),
    the path `notes.txt` (monospace), and at the right five square icon buttons (~26x24 px
    each): a narrow bar (ignore whitespace?), stacked lines, `±` over a line, a two-way arrow
    over a line, and a split rectangle (side by side). No text labels or tooltips visible.
  - Rows (monospace, ≈ 12 px type, cap/ascender 8 px, **row height 17 px**):
    - `@@ -4,3 +4,4 @@ line 3` (grey hunk header, no line numbers)
    - `4 4  line 4`
    - `5 5  line 5`
    - `6 6  line 6`
    - `  7  line 7` — added: whole row tinted dark green from x ≈ 546 to the right edge
  - Gutter: two narrow line-number columns (old, new) at x ≈ 522–540, small grey digits; no
    `+`/`-` marker column.

---

## (d) The merge commit — CAPTURED

Row 6 (`Merge branch 'feat…`, 8f71429) selected with three more Down arrows.

### d1. Changes tab — `cairn-5.png`

- Summary: `Ada Lovelace` `8f71429` `16 Oct 2026 12:09:55 +01:00` `Merge branch 'feature/login'`.
- File list: one row, a green `A` badge, `src/login.rs`.
- Diff bar: `^` `v`, path drawn as `src/` grey + `login.rs` white, the five icon buttons.
- Rows: `@@ -0,0 +1,2 @@`; `  1  pub fn login() {}` and `  2  // validate`, both green
  (added), new-side numbers only.
- No header naming which parent the merge is compared against is visible; the file list
  shows the change from the first parent (`af50cb9`, Fix README typo) only.

### d2. Commit tab — `cairn-6.png` (Ctrl+Alt+1)

- `AUTHOR` / `COMMITTER`: `Ada Lovelace <ada@example.com>`, `16 Oct 2026 12:09:55 +01:00`.
- `REFS`: chips `[(cloud) origin/main]` (amber) `[(tag) v0.2]` (indigo).
- `SHA`: `8f714290dbe09e2423b8d587fb9beb77021e6602`.
- `PARENTS`: links `af50cb9` `7708126`.
- Message: **`Merge branch 'feature/login'`**; `Expand All` cut at the bottom edge.
- The annotated tag's message (`Release 0.2`) is not shown anywhere visible.

---

## (e) Local Changes — NOT CAPTURED

The sidebar's `Local Changes (3)` row is the only way into the view, and it takes a click.
No accelerator switches the main region, and Tab focus (below) never lands on the sidebar's
rows, so the Unstaged and Staged lists, the badges for `README.md`, `src/login.rs` and
`new.txt`, the working-tree diff and the commit box were not seen. The sidebar's count reads
`(3)` throughout, matching the fixture's three paths.

---

## (f) Other keyboard-reachable states

### f1. Create Branch dialog — CAPTURED

Shot: `cairn-7.png`, sent Ctrl+Shift+B while the merge row was selected.

- The whole window behind is dimmed (a dark translucent overlay); the dialog is a rounded,
  slightly lighter panel at x ≈ 252–771, y ≈ 285–482 (~520x197 px), horizontally centred
  on the window but vertically placed in its upper middle, no shadow beyond the dim.
- Contents, verbatim, top to bottom:
  - Title `Create Branch` (cap height 13 px ≈ 18 px type), x ≈ 268.
  - `Use '/' as a path separator to create folders` (≈ 12 px, x ≈ 260).
  - Right-aligned grey label `Create branch at:` then `◇ 748ca2e Local only commit` (bold
    white; a hollow diamond glyph). It names HEAD, not the selected merge row.
  - Label `Branch name:` then a text field (~330x32 px, rounded light border) with the grey
    placeholder `Enter branch name`. No caret was visible in the shot.
  - A check box, unchecked, `Check out after create`.
  - Footer, right-aligned: **`Create`** (filled, lighter grey, the primary, on the left)
    then `Cancel` (outlined), each ~74x30 px.
- No radio choices (Stash / Discard / Don't change) were shown — the working tree here has
  changes, but the dialog drew only the check box.

Escape closed it (`cairn-8.png`: the window as in `cairn-6.png`, no dim). `git branch -a`
afterwards lists no new branch.

### f2. Show Lost Commits (Ctrl+Shift+.) — NOT CAPTURED

Sent twice through `send_shortcut` (key named `period`, then `.`), with the history list
refocused by an Up/Down first. The header's check box stayed unchecked and the rows did not
change (`cairn-9.png` is identical to `cairn-6.png`). Whether the chord is mis-delivered by
the injection or not heard is not decidable from the screen. The check box itself needs a click.

### f3. Tab focus order — observed

Starting with the history list focused, nine Tab presses showed a cycle of seven stops:
1. the `Fetch origin` button (its border turns a brighter grey, ~2 px);
2. the sidebar's `Filter` box (light border);
3. no visible focus;
4. a thin light outline around the history list's area;
5. to 7. no visible focus anywhere;
8. `Fetch origin` again; 9. `Filter` again.

The sidebar's rows, the detail pane's tabs, `Collapse`, `Expand All` and the diff bar's
buttons were never seen focused; three stops of the cycle show no focus at all. Enter was
never pressed on a focused control.

### Not attempted

- F5 (Refresh): nothing to show over an unchanged fixture.
- `Fetch origin`, `Collapse`, `Expand All`, the diff bar's buttons, a file's diff opened in
  place, two commits compared, the sidebar's folders and ref presses, the activity popover:
  all need a click — NOT CAPTURED.

## Measurements at a glance

| Thing | Size |
| --- | --- |
| Window | 1024x768 (client area; no OS decorations) |
| Title bar | ~40 px tall |
| Sidebar | 240 px wide, rows ~22 px |
| Vertical splitter | 4 px |
| History header | ~24 px; rule at y ≈ 66 |
| History row | 26 px |
| Graph column | ~90 px for 4 lanes + 2 dashed detours, lane pitch ~14 px |
| Ref chip | ~18 px tall, text ≈ 12 px |
| Horizontal splitter | ~4 px at y ≈ 503 |
| Detail pane | ~260 px tall (fixed when the window grows) |
| Detail tab strip | ~30 px |
| Commit tab field rows | ~24 px |
| Changes file list | ~265 px wide, rows ~24 px |
| Diff rows | 17 px, monospace ≈ 12 px |
| Body text | ≈ 13 px (cap height 9 px) |
| `Cairn`, `Fetch origin`, `Collapse` | ≈ 17 px (cap height 12 px) |
| Dialog title | ≈ 18 px (cap height 13 px) |
