# Fork for Windows' Local Changes bar, View Mode, Stage All and commit box (2026-10-20)

Observed by the fork-observer agent for fork-alignment phase 07, before Cairn's Local Changes bar
was settled. Fork for Windows 2.23.2, Dark theme, Windows 10 22H2, git 2.50.1.windows.1, the
guest at 1024×768. Repository: the ui-review fixture (`fixture-recipe.md`) at `C:\uirev\t`, in
the recipe's state before and after (` M README.md`, `M  src/login.rs`, `?? new.txt`, the stash
present; the index's blob ids matched the baseline at the end, lc27). Local Changes opened by
pressing "Local Changes (3)" in the sidebar (lc7). Shots `~/VMs/fork-win10/shots/lc6` to `lc33`,
zooms suffixed with a letter (`lc1`-`lc5` are the 2026-10-10 staging survey's and were kept).
Nothing was committed or discarded. Each answer VERIFIED by the shots named, unless marked NOT
OBSERVED.

## 1. The eye, "Show diff side-by-side (space bar)"

- **Pressed with new.txt selected, it opens a separate window over Fork's** (lc8 before, with
  zooms lc8a-lc8c and lc8z; lc9 after, zooms lc9a-lc9c). Borderless with a 1 px blue border, at
  x 52-987, y 37-706 (936×670), no taskbar entry, a ✕ at its top right, a diff bar of its own
  (↑ ↓, "new.txt", the whitespace, ¶, wrap, −, + and ↕ glyphs, and its side-by-side button lit
  blue; wrap dimmed), the diff inside drawn side by side, a footer reading **"Space bar: close,
  Up/Down: move file selection"** and a resize grip at its bottom right. Inside it ↓ moved to
  README.md and showed its diff (lc10); ↑ moved back (lc11).
- **The main diff does not change**: it stays unified and the main bar's "Side by side diff"
  button never lights. The eye's own pixels are the same afterwards (lc8z against lc13z).
- **Escape closes the window** (lc12, lc13). After a pointer-opened window, the eye's tooltip
  stayed on screen once the pointer had left, and the selected row was drawn in the unfocused
  grey.
- **Space with the Unstaged list focused opens the same window**; Space again closes it, the
  row still focused blue (lc14, lc15).
- Whether the window's own side-by-side toggle is remembered: NOT OBSERVED.

## 2. View Mode

- Its menu: "✓ View as Tree", "View as List", "View as Combined List", a rule, "Hide Untracked
  Files", "Show Ignored Files" (lc16).
- With View as List chosen the glyph **does not change**: the same filled grey square with a
  notch at its bottom left as in tree mode (lc17a against lc8b). Staged then shows one flat row,
  "src/login.rs", the whole path in one tone, #EAE8E8 (lc17d). The menu then ticks View as List
  (lc19). Its tooltip is still "View Mode" (lc18, lc18a). View as Tree restored the tree (lc20).

## 3. Stage All with nothing left to stage

- Everything staged with the bar's double chevron, **the chevron turns upward, ⌃⌃, with the
  tooltip "Unstage All"**, the same grey; it neither greys nor goes (lc21, lc21a, lc22, lc22a).
  Unstaged's Stage button greyed, the commit button read "Commit 3 Files", and the selection
  moved to src/login.rs in Staged, focused blue.
- The Activity Manager logged "Stage 2 Files / staged", `git add --force --verbose
  --pathspec-from-file=- --pathspec-file-nul --` (lc26); the way back, two files unstaged by
  the Staged button, "Unstage 2 Files / unstaged", `git update-index --force-remove -z --stdin`
  then `git reset -q --no-refresh HEAD --pathspec-from-file=- --pathspec-file-nul --` (lc25).

## 4. The commit box's description

- **One line, 22 px, while empty** (655-676 inside a box 631-677), and **it grows upward a line
  at a time** as lines are typed, its bottom edge fixed and the diff above shrinking: about
  35 px at two lines (642-676), about 50 px at three (627-676), roughly 15 px a line (lc28-lc32,
  zooms lc30a, lc32a). Cleared with Ctrl+A, Backspace it is 22 px again with its placeholder
  "Description" (lc33, lc33a). No focus ring was visible on it.

## 5. The selected row's band

- In Unstaged, whose body runs x 303-552 between 1 px #525252 rules, the band spans x 307-548
  — **4 px in from either side** — 20 px tall (y 173-192), the first row 4 px below the body's
  top (169), the rows 20 px apart, its corners rounded about 4 px; the geometry is the same
  focused (#236BD2, lc8) and not (#525252, lc20) (zooms lc8d, lc8e, lc20a).
- Two adjacent selected rows merge into one band, 40 px, rounded at its outer corners only
  (lc23, lc23a).

## Also seen

- Fork fetched `origin` by itself when the repository opened ("Fetch 'origin' … Already up to
  date"); the working state was unchanged.
