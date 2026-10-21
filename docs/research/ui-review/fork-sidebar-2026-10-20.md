# Fork for Windows: the sidebar's tags, ahead/behind, stashes and keys, observed first-hand (2026-10-20)

For the Fork-alignment packet's phase 05 (the sidebar). Driven in the Windows 10 VM (QEMU/KVM)
through `qmp.py`; screenshots `sb*.png` in `~/VMs/fork-win10/shots/`. VERIFIED = seen on screen in
the named shot. NOT OBSERVED = not seen, with why. Settled elsewhere and not re-observed: the
sidebar's layout, default disclosure and opened rows (`fork-window-survey-2026-10-16.md` §3), the
ref context menus (same file §8), the gear menu (`fork-window-survey-2-2026-10-16.md` §2).

## What was driven

- Windows 10 22H2, guest frame 1024x768 (sb0's size); clicks landed where aimed; the VM signed in
  on its own. Run 07:54-08:07 BST, boot and shutdown included; the VM powered off by QMP
  `system_powerdown` at the end.
- Repository `C:\sb\t`, bare remote `C:\sb\remote.git`, built in Git Bash (Git for Windows) by
  the caller's script. The first paste was typed too fast and garbled after the first tag (sb3:
  the commits `one`, `two`, `three` and the tag `a-annot-new` were made, the rest failed with no
  effect); the rest was then typed one line at a time, unchanged (sb5, sb6). Commits after
  `unset GIT_COMMITTER_DATE GIT_AUTHOR_DATE` carry the run's date (2026-10-20).
- Checked in Git Bash (sb6):
  - `git branch -vv`: `behind 7a5cd31 [origin/behind: behind 1] three`; `both 66cab89
    [origin/both: ahead 1, behind 2] local z`; `gone 7a5cd31 [origin/gone: gone] three`;
    `* main ee3931b [origin/main: ahead 1] ahead one`.
  - `git tag -n`: `a-annot-new new`, `b-light one`, `m-light three`, `z-annot-old old`.
  - `git stash list`: `stash@{0}: On main: named stash`, `stash@{1}: WIP on main: ee3931b ahead one`.
  - `git for-each-ref refs/tags --format='%(refname:short) %(creatordate:iso)
    %(*committerdate:iso) %(committerdate:iso)'`:
    ```
    a-annot-new 2025-01-01 12:00:00 +0000 2020-01-01 12:00:00 +0000
    b-light 2020-01-01 12:00:00 +0000  2020-01-01 12:00:00 +0000
    m-light 2022-01-01 12:00:00 +0000  2022-01-01 12:00:00 +0000
    z-annot-old 2019-01-01 12:00:00 +0000 2021-01-01 12:00:00 +0000
    ```
    (for an annotated tag the third column is the target commit's date and the fourth is empty;
    for a lightweight tag the third is empty and the fourth is the commit's). Creation order:
    a-annot-new, z-annot-old, m-light, b-light.
- **Fork for Windows 2.23.2.0** (Help > About Fork: "Version 2.23.2.0", sb10). Fork opened in its
  **Dark** theme already (sb8), so Appearance was not touched; every shot below is Dark. Fork
  restored its earlier tabs `r`, `n`, `a`, `t`; `C:\sb\t` was opened by Ctrl+O ("Open
  Repository...", folder `C:\sb\t`, "Select Folder", sb11-sb13) as a new tab titled **"t (sb)"**
  (the earlier `t` tab renamed itself **"t (uirev)"**, sb13).
- Opened as Fork shows it first (sb13): Branches open, Remotes open with `origin` closed, **Tags
  closed**, Stashes open. `Tags` and `origin` were then opened by their chevrons (sb14; zoom sb14z).
- Pixel positions and colours measured with PIL on the 1024x768 shots (±2 px). Fork draws text
  with ClearType subpixel antialiasing, so glyph edges carry coloured fringes; "colour" below is the
  brightest neutral pixel of a glyph.

## 1. Tag order

VERIFIED (sb14, zoom sb14z). The Tags section, top to bottom:

1. **m-light**
2. **z-annot-old**
3. **b-light**
4. **a-annot-new**

Beside the `for-each-ref` output above (sb6), per tag: tagger date (annotated) else commit date —
m-light 2022, z-annot-old 2019, b-light 2020, a-annot-new 2025; **target commit's date** — m-light
2022-01-01 (`three`), z-annot-old 2021-01-01 (`two`), b-light 2020-01-01 (`one`), a-annot-new
2020-01-01 (`one`). The order seen is **descending by the target commit's date**; it is not name
order (either way), not tagger-else-commit date order (that would put a-annot-new first) and not
creation order or its reverse. b-light and a-annot-new point at the same commit (`one`), so this
fixture does not decide how Fork breaks that tie; it drew **b-light above a-annot-new**
(consistent with name descending, or with lightweight-before-annotated, or with something else —
not decided by this run). All four tags have the same tag-outline glyph; annotated and lightweight
are not drawn differently (sb14z).

## 2. Ahead/behind on branch rows

VERIFIED (sb14; zooms sb14z, sb14zz, sb14g). At each row's right edge, right-aligned (last ink at
x=291-292 of a sidebar whose splitter is at x≈301):

| Row | Upstream state (git) | Drawn at the right |
| --- | --- | --- |
| `behind` | behind 1 | **"1↓"** (x 281-292) |
| `both` | ahead 1, behind 2 | **"1↑ 2↓"** — ahead first, then behind (x 264-291) |
| `gone` | upstream gone | **nothing** at the right |
| `main` (current, ✓, bold) | ahead 1 | **"1↑"** (x 281-292) |

- Each count is the digit then its arrow, with no space between them: in "1↑ 2↓" the columns with
  ink are `1` x 264-267, `↑` x 271-275, then **a gap of 4 empty columns (x 276-279)**, then `2↓`
  x 280-291 (the `2` and the `↓` touching). Glyph height y 307-315 (~9 px), on a 22 px row.
- Colour: digits and arrows are the **same near-white as the branch names**, brightest pixel
  (234,232,232) #EAE8E8 on `both` (the label text's brightest is the same (234,232,232)); the arrows
  are not coloured differently from the digits, no red/green. Row background (44,44,45) #2C2C2D.
- `gone`: **a small yellow warning triangle** (fill (255,197,1) #FFC501) overlaid at the bottom right
  of the branch glyph (sb14g); the branch glyph itself and the name are **not greyed** (glyph's
  brightest pixel (255,255,255), name's (234,232,232), the same as `behind`'s name). No count, no
  text such as "gone".
- `origin`'s remote branches (`behind`, `both`, `main`) carry no counts (sb14z); `origin/gone` is
  not listed (pruned).
- Hover (sb15, sb16, sb17, sb18, sb19, sb20; strip sb15all; zooms sb17z, sb18z, sb19z, sb20z):
  - Hovering a branch row adds the pin, funnel and eye icons at its right, and the count **moves
    left** to make room for them ("1↑ 2↓" then the three icons, sb15/sb20). The row gets no hover
    background (pixel (150,311) on the hovered `both` row = (44,44,45), the same as an unhovered
    row, sb20).
  - **Hovering the count shows no tooltip** (pointer on "2↓" for 3 s, sb20). The pointer at the
    count's unhovered place landed on the eye icon, which showed "Hide 'both' in the comm…" (cut by
    the crop; sb15).
  - **Hovering the warning triangle of `gone` shows no tooltip** (2.5 s, sb16).
  - **Hovering a branch's name shows a two-line tooltip**: on `gone` — "Local branch:  gone" /
    "Tracked branch:  origin/gone [removed]" (sb17z); on `both` — "Local branch:  both" / "Tracked
    branch:  origin/both" (sb18z); on `behind` — "Local branch:  behind" / "Tracked branch:
    origin/behind" (sb19z). The tooltip names no ahead/behind count.

## 3. Stash wording

VERIFIED (sb13, sb14z). Stashes, top to bottom:

1. **"named stash"** — `stash@{0}`, git's "On main: named stash": Fork drops "On main: ".
2. **"WIP on main: ee3931b ahead one"** — `stash@{1}`, git's default message, drawn whole (git's
   own subject, "WIP on main: ee3931b ahead one", sb6).

Each with the archive-box glyph. The history list draws the same two as `stash@{0}` "On main:
named stash" and `stash@{1}` "WIP on main: ee3931b ahead one" (sb13), i.e. the history keeps "On
main: " where the sidebar drops it.

Aside, not a sidebar question: the garbled first paste left two untracked files in the work tree,
which Local Changes lists as **"GIT"** and **"DREMOTE"** (sb75) — that is why the sidebar reads
"Local Changes (2)". They change no ref, tag or stash.

## 4. Sidebar keys

Starting point: `main` pressed (left click on its name) in the sidebar (sb22; a OneDrive
notification over the window was closed first, sb21). Each step below is one key press and one
shot. Strips: sb_down_23 (sb23-sb27), sb_down_28 (sb28-sb32), sb_down_33 (sb33-sb37), sb_up_38
(sb38-sb55), sb_up_56 (sb56-sb58), sb_follow (sb23, sb25, sb34 history and detail zoomed).

- **↓ from `main`** visits, in order (VERIFIED sb23-sb37): **"Remotes"** heading (sb23) →
  **`origin`** (sb24) → origin's **`behind`** (sb25) → **`both`** (sb26) → **`main`** (sb27) →
  **"Tags"** heading (sb28) → **m-light** (sb29) → **z-annot-old** (sb30) → **b-light** (sb31) →
  **a-annot-new** (sb32) → **"Stashes"** heading (sb33) → **named stash** (sb34) → **WIP on main:
  ee3931b ahead one** (sb35) → **"Submodules"** heading (sb36, a heading with no rows and no
  chevron) → stays on "Submodules" (sb37, the last row). So the selection **stops on every section
  heading, on the remote `origin`, on every remote branch, every tag and every stash**.
- **↑ from "Submodules"** visits the same rows in reverse (VERIFIED sb38-sb55): WIP stash, named
  stash, Stashes, a-annot-new, b-light, z-annot-old, m-light, Tags, origin's main, both, behind,
  origin, Remotes, **`main`, `gone`, `both`, `behind`** (the local branches, sb51-sb54), then the
  **"Branches"** heading (sb55). Further ↑ (sb56, sb57, sb58): **stays on "Branches"** — the
  selection **does not reach the Filter field, "All Commits" or "Local Changes"**.
- **Band colour with the sidebar focused**: a filled band of **(35,107,210) #236BD2** (blue), the
  row's text and glyph white (sb22, measured on `main`'s row). **With focus elsewhere** (the history
  focused after "All Commits" was pressed, sb73): the sidebar's selected row (`origin`) keeps a band
  of **(82,82,82) #525252** (grey), while the history's selected row turns blue (35,107,210).
- **The history follows each step at once** (VERIFIED sb_follow, sb25, sb34): with origin's `behind`
  selected by ↓ the history selects `9845ea5` "behind tip" (origin/behind's commit) and the Commit
  tab shows it ("behind tip", REFS `origin/behind`, SHA 9845ea5…); with "named stash" selected it
  selects `stash@{0}` and the Commit tab shows "On main: named stash" (sb34); with a tag, its commit
  (m-light → `three`, z-annot-old → `two`, b-light and a-annot-new → `one`, sb29-sb32). No Return
  is needed. The history's selected row is drawn with the **grey** band (the history is not
  focused; the sidebar keeps focus). On a **section heading or `origin`** the history keeps the
  commit it had (sb23, sb24: still "ahead one"; sb33: still `one`).
- **"All Commits" and "Local Changes" by keys**: not reachable by arrow keys (above), so whether the
  view switches on reaching them is **NOT OBSERVED (the arrows never reach them)**. **Pressed with
  the pointer**: **"All Commits"** (sb73) switches to the history at once and **moves keyboard
  focus to the history list** — the history selects `HEAD`'s commit (`main` "ahead one") with the
  blue band, the sidebar's band goes grey, and the next ↑ moves **the history's** selection (to
  `stash@{1}`, sb74), not the sidebar's. **"Local Changes"** (sb75) switches the main view to Local
  Changes at once (Unstaged/Staged lists, diff, commit box); the next ↓ did not move the sidebar's
  selection to "All Commits" (sb76).
- **Return**, each VERIFIED by a shot after the key:
  - on the **"Branches"** heading: **nothing** — no change in the sidebar, history or detail
    (sb59, sb60, twice);
  - on the local branch **`behind`**: **nothing** — no Checkout dialog, no prompt, the window
    unchanged (sb66; the history already showing `behind`'s commit `three`);
  - on **`origin`** (the remote, the one folder-like row this fixture has — no local branch is in a
    folder): **nothing**, `origin` stays open (sb67 → sb68);
  - on **"Local Changes"** and **"All Commits"**: **NOT OBSERVED** (the keyboard cannot select
    them).
- **← and →** (a standard tree's keys):
  - On the open **"Branches"** heading, the sequence ←, ←, →, → ended on `behind` (sb61); then
    from `behind`: **← moved to the parent heading "Branches"** (sb62); **← on the open heading
    closed it** (chevron `›`, its rows hidden, sb63); **→ on the closed heading opened it**, the
    selection staying on the heading (sb64); **→ on the open heading moved to its first child**
    `behind` (sb65).
  - On **`origin`** open (sb67): **← closed it** (sb69); **← on the closed `origin` moved to its
    parent "Remotes"** (sb70); **→ on the open "Remotes" moved to its first child `origin`** (sb71);
    **→ on the closed `origin` opened it**, selection staying on `origin` (sb72).

## 5. Selected band shape

VERIFIED (sb22, zoom sb22z; measured with PIL).

- A ref row selected in the focused sidebar (`main`): a **rounded** rectangle — the corner pixels
  blend over about 3 px ((44,47,53), (39,81,140), (37,97,182) stepping into (35,107,210)) — **22 px
  tall** (y 344-365, the row's full pitch) and **x 5-296**: it spans **the whole sidebar width less
  a ~5 px margin each side**, **not** starting at the row's indent (the `main` row's glyph is at
  x≈36).
- At the same time **"All Commits" is still drawn selected**: its band (63,63,63) **#3F3F3F**,
  x 5-296, y 166-187 (22 px), the same rounded shape, stays while a ref is selected below (sb22 and
  every key shot after it).

## 6. "New Branch..." from a sidebar row: the "Create branch at:" line

Each opened by a right press on the row, then "New Branch..." (Ctrl+Shift+B in the menu); each
dialog closed by Escape, nothing created. The dialog: title "Create Branch", "Use '/' as a path
separator to create folders", "Create branch at:" line, "Branch name:" field (branch glyph,
placeholder "Enter branch name"), "Check out after create" check box, "Create" (greyed) and
"Cancel". Glyphs side by side: sb_at_glyphs.

| Row pressed | Menu | "Create branch at:" reads | Shot |
| --- | --- | --- | --- |
| local `behind` | sb77 | branch glyph + **"behind"** | VERIFIED sb78 |
| remote `origin` > `main` | sb79 | the same branch glyph (not the cloud) + **"origin/main"** | VERIFIED sb80, zoom sb80z |
| tag `m-light` | sb81 | tag glyph + **"m-light"** | VERIFIED sb82, zoom sb82z |

## 7. "Copy Branch Name" / "Copy Tag Name": the text copied

Each copied from the row's context menu (the last item, Ctrl+C in the menu), then pasted
(Shift+Insert) into the Git Bash window inside an `echo` and run.

- origin > `main` > "Copy Branch Name" (menu sb83): pasted **`origin/main`** (sb84: "$ clear; echo
  ORIGINMAIN:origin/main", echoed as "ORIGINMAIN:origin/main", sb86's top line). VERIFIED.
- local `behind` > "Copy Branch Name": pasted **`behind`** ("BEHIND:behind", sb86). No local
  branch is in a folder in this fixture, so a foldered name's copy is NOT OBSERVED. VERIFIED.
- tag `m-light` > "Copy Tag Name" (menu sb85, zoom sb85z): pasted **`m-light`** ("MLIGHT:m-light",
  sb86). VERIFIED.

## What was not observed

- Return on "Local Changes" and "All Commits", and whether reaching them by arrow keys switches the
  view: the arrow keys never reach them (question 4).
- How Fork orders two tags on the same commit: this fixture's tie (b-light, a-annot-new on `one`)
  shows one order and does not decide the rule (question 1).
- A local branch in a folder (for ←/→ on a folder, Return on a folder, and its copied name): this
  fixture has none; `origin` stood in for a folder row.
- The Activity Manager: no question here turned on a git command Fork ran, so it was not opened.
