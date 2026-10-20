# Fork for Windows: the detail pane's committer and the diff bar's toggles, observed first-hand (2026-10-20)

For the Fork-alignment packet's phase 06 (the detail pane). Driven in the Windows 10 VM (QEMU/KVM)
through `qmp.py`; screenshots `cm*.png` in `~/VMs/fork-win10/shots/`. VERIFIED = seen on screen in
the named shot. NOT OBSERVED = not seen, with why.

## What was driven

- Windows 10 22H2, guest frame 1024x768 (cm0's size); the VM signed in on its own. The guest's
  zone is **+0100** (`date` in Git Bash: "Tue Oct 20 09:17:00 GMTDT 2026", `date +%z` "+0100", cm3).
- Repository `C:\cm\t`, built in Git Bash (Git for Windows) by the caller's script, typed one line
  at a time (cm2, cm3). Checked with `git log --format='%h | %an <%ae> %ad | %cn <%ce> %cd | %s'
  --date=iso-strict` (cm3, zoom cm3z):
  ```
  1c63ef3 | Ada Lovelace <ada@example.com> 2026-10-16T12:00:00+01:00 | Grace Hopper <grace@example.com> 2026-10-17T09:30:00+02:00 | Committer differs entirely
  4c593d1 | Ada Lovelace <ada@example.com> 2026-10-16T12:00:00+01:00 | Ada Lovelace <ada@example.com> 2026-10-16T11:00:00Z | Committer offset differs
  8de054c | Ada Lovelace <ada@example.com> 2026-10-16T12:00:00+01:00 | Ada Lovelace <ada@example.com> 2026-10-16T12:00:01+01:00 | Committer one second later
  af35cc3 | Ada Lovelace <ada@example.com> 2026-10-16T12:00:00+01:00 | Ada Lovelace <ada@example.com> 2026-10-16T13:00:00+01:00 | Committer time differs
  4667189 | Ada Lovelace <ada@example.com> 2026-10-16T12:00:00+01:00 | Ada Lovelace <ada@work.example> 2026-10-16T12:00:00+01:00 | Committer email differs
  bdac35f | Ada Lovelace <ada@example.com> 2026-10-16T12:00:00+01:00 | Ada L <ada@example.com> 2026-10-16T12:00:00+01:00 | Committer name differs
  7763536 | Ada Lovelace <ada@example.com> 2026-10-16T12:00:00+01:00 | Ada Lovelace <ada@example.com> 2026-10-16T12:00:00+01:00 | Same committer
  ```
  (git wrote the +00:00 committer date of "Committer offset differs" as `Z`.) In the guest's zone
  (+0100) the author date of every commit is 12:00 on the 16th; "Committer offset differs"'s
  committer date is also 12:00 local; "Committer differs entirely"'s committer date is 08:30 local
  on the 17th.
- **Fork for Windows 2.23.2.0** (the version recorded by `fork-sidebar-2026-10-20.md` on this
  image; Help > About was not reopened this run). Fork opened **maximised in its Dark theme
  already** (cm4), so Appearance was not touched; every shot below is Dark. `C:\cm\t` was opened by
  Ctrl+O ("Open Repository...", folder `C:\cm\t`, "Select Folder", cm5) as a new tab **"t (cm)"**,
  All Commits, `main` checked out (cm6).
- Pixel positions and colours measured with PIL on the 1024x768 shots (±1 px). Detail pane
  background (51,51,51) #333333.

## 1. Is a COMMITTER block drawn, per commit

Each commit selected by a left press on its subject, the pointer then moved off the pane (cm7-cm13;
the seven side by side: cm_all7).

| Commit | Committer differs from author in | COMMITTER block | Shot |
| --- | --- | --- | --- |
| Committer differs entirely (1c63ef3) | name, email, instant, offset | **yes** | VERIFIED cm7 (also cm6), zoom cm7z |
| Committer offset differs (4c593d1) | offset only (same instant) | **no** | VERIFIED cm8 |
| Committer one second later (8de054c) | instant, by 1 s | **yes** | VERIFIED cm9 |
| Committer time differs (af35cc3) | instant, by 1 h | **yes** | VERIFIED cm10 |
| Committer email differs (4667189) | email | **yes** | VERIFIED cm11 |
| Committer name differs (bdac35f) | name | **yes** | VERIFIED cm12 |
| Same committer (7763536) | nothing | **no** | VERIFIED cm13 |

## 2. Where the COMMITTER block sits and what it holds

VERIFIED (cm7, zoom cm7z; measured on cm7, cm8, cm9, cm13).

- **A second column beside AUTHOR, on the same rows**, not under it. Measured (x, y on 1024x768):

  | Part | AUTHOR | COMMITTER |
  | --- | --- | --- |
  | caps label ink | "AUTHOR" x 400-442, y 456-463 | **"COMMITTER"** x 739-798, y 456-463 |
  | avatar | x 342-383, y 475-516 (**42x42**) | x 680-721, y 475-516 (**42x42**) |
  | name + email + ⌄ ink | x 400-600, y 480-491 | x 738-953, y 480-492 |
  | date ink | x 401-550, y 501-509 | x 739-888, y 501-509 |

  The COMMITTER column's avatar starts **338 px right of the author's** (342 → 680); the label and
  text start at x≈739 (author's at x≈400). The pane spans x 302-1024 here (the window maximised,
  sidebar splitter at x≈301); whether that x is fixed or follows the pane's width was **NOT
  OBSERVED** (the pane was not resized).
- **Contents**, verbatim (cm7z): label **"COMMITTER"** in caps (the same small grey caps as
  "AUTHOR"); an avatar of the committer's **initials "GH"** on a **green** square (sampled
  (100,204,112) near its top, lighter toward the bottom — the same top-to-bottom gradient and the
  slightly rounded corners, about 1 px, as the author's lilac "AL" square, sampled (212,121,233));
  the name **"Grace Hopper"** in bold; the email **"grace@example.com"** with a **⌄** after it,
  exactly like the author's "ada@example.com ⌄"; the date **"17 Oct 2026 08:30:00 +01:00"** — day,
  abbreviated month, year, **time with seconds**, and the offset **+01:00, the machine's**, not the
  committer's own +02:00 (git: 2026-10-17T09:30:00+02:00 = 08:30 at +01:00). The author's date is
  drawn the same way: "16 Oct 2026 12:00:00 +01:00".
- Where the committer is the author's name and email ("one second later", "time differs"), the
  committer's avatar is the **same lilac "AL"** square (sampled (212,121,233) on cm9, the author's
  colour) — the avatar follows the person, not the column.
- **The AUTHOR block does not move or change** when COMMITTER appears: its avatar, label and name
  ink are at the same pixels with and without it (cm7/cm9 vs cm8/cm13: avatar x 342-383 y 475-516,
  name ink x 400-600 y 480-491 in all four). The REFS row ("main") appears only on the tip
  (cm7), pushing SHA and PARENTS down; it is unrelated to the committer.
- Aside: on the root commit "Same committer" (cm13) the **"PARENTS" caption is drawn with nothing
  beside it**.

## 3. Which fields Fork compares

VERIFIED (cm8-cm12, table above). A COMMITTER block is drawn for a **different name** (cm12: "Ada
L ada@example.com ⌄", "16 Oct 2026 12:00:00 +01:00"), a **different email** (cm11: "Ada Lovelace
ada@work.example ⌄", "16 Oct 2026 12:00:00 +01:00"), a **different instant by an hour** (cm10:
"16 Oct 2026 13:00:00 +01:00") and **by one second** (cm9: "16 Oct 2026 12:00:01 +01:00"); and
**not** for the **same instant in another offset** (cm8: "Committer offset differs" shows AUTHOR
alone, "16 Oct 2026 12:00:00 +01:00"). So Fork compares name, email and the instant (to the
second), and not the offset. (Since Fork draws every date in the machine's offset, the offset case
would also read identically as text; this run cannot tell "compares the instant" from "compares the
shown text" — both give the same answer here.)
- **The COMMITTER column follows the pane's width.** With the sidebar dragged narrower (cm55: the
  detail pane from x 202 instead of 302, 822 px wide instead of 722) the author's avatar moved with
  the pane (x 242-283, pane left + 40, as in cm7) while the committer's avatar sat
  at **x 630-671**, pane left + 428 (cm7: pane left + 378); its label at x 689-748. Both shots fit
  "the committer column starts at the pane's horizontal middle + 17 px" (722/2 + 17 = 378, 822/2 +
  17 = 428) — two data points, so the rule is consistent with, not proven by, this run. The splitter
  was dragged back afterwards (cm57; the pane starts at x 303, 1 px from where it was).

## 4. "Committer differs entirely": Changes tab summary and history row

- **Changes tab summary** (cm14, zoom cm14z): the **author's** lilac "AL" avatar, **"Ada
  Lovelace"** (bold), the short id **"1c63ef316"**, **"16 Oct 2026 12:00"** (the author date in the
  machine's zone; no seconds, no offset), then the subject **"Committer differs entirely"** in bold.
  Nothing of the committer (Grace Hopper, 17 Oct) appears in it. VERIFIED.
- **History row** (cm6, cm7, cm14): author column **"Ada Lovelace"** with the "AL" avatar; date
  column **"16 Oct 2026 12:00"** — the **author's** date (the committer's would be 17 Oct 08:30 in
  the machine's zone). Bold because it was `HEAD` then. VERIFIED.

## 5. The diff bar's toggles and the context buttons

The bar (cm14, zoom cm14bar), left to right: ⌃ ⌄ (previous/next change), the file glyph and name
centred, then seven buttons. Tooltips, each after a 2 s hover (cm15-cm17, strip cm_tips;
cm22-cm25, strip cm_tips2; a OneDrive notification that covered cm18-cm21 was closed first):

| x (centre) | Glyph | Tooltip, verbatim | Shot |
| --- | --- | --- | --- |
| 893 | ␣ (open box) | **"Ignore whitespaces"** | cm15 |
| 913 | ¶ | **"Show hidden symbols"** | cm16 |
| 933 | wrap arrow | **"Word wrap"** | cm17 |
| 954 | −≡ | **"Decrease number of visible lines"** | cm22 |
| 974 | +≡ | **"Increase number of visible lines"** | cm23 |
| 994 | ↕≡ | **"Show entire file"** | cm24 |
| 1014 | ◫ (two panes) | **"Side by side diff"** | cm25 |

Pressed in order on "Committer differs entirely" (cm26 after Ignore whitespaces, cm27 after Show
entire file, cm28 after Side by side diff; strip cm_toggles, rows: before, cm26, cm27, cm28):

- **A toggled-ON button is a blue glyph and nothing else**: glyph (65,155,249) **#419BF9**; **no
  background box, no border** — the bar's background behind an ON button is (51,51,51) #333333,
  the same as behind an OFF one (sampled at the button's corner on cm14 and cm28). An OFF glyph's
  brightest pixel is (207,207,207) **#CFCFCF**.
- **Show entire file greys "Decrease" and "Increase number of visible lines"** (glyph (114,114,114)
  **#727272**, cm27); **Side by side diff greys "Word wrap"** (#727272, cm28). Ignore whitespaces
  greys nothing (cm26). Side by side drew the two columns (cm28). VERIFIED.
- All three were pressed off again (cm29: every glyph #CFCFCF).

Context buttons, on the 40-line commit added for this (below) since a one-line file shows no
context:

- Seven presses of "Decrease number of visible lines" from the setting as found (cm30-cm36, glyph
  colours sampled after each, all #CFCFCF) left the hunk at **`@@ -19,3 +19,3 @@`: one line of
  context** (cm38). One more press (cm39): still `@@ -19,3 +19,3 @@`. **"Decrease" never greys
  out**, not at one line of context nor when a press there does nothing (cm30-cm39, all #CFCFCF).
  So the floor is **1**, not 0. The value as found is **NOT OBSERVED** (the seven presses were made
  on a one-line file, whose hunk does not show context).
- "Increase number of visible lines", pressed one at a time from 1 (cm40-cm51, strip cm_ctx):
  `-18,5` (2), `-17,7` (3), `-16,9` (4), `-15,11` (5), `-14,13` (6), … `-7,27` (13): **one line per
  press**. Ten more presses (cm52, cm53) reached `@@ -1,40 +1,40 @@`, the whole file; **"Increase"
  never greys out**, even with the whole file shown (cm53, #CFCFCF).
- Put back: twenty presses of "Decrease" from 23 left `@@ -17,7 +17,7 @@`, **three lines** (cm54,
  zoom cm54z), git's default; every toggle off.

## 6. Hover look and the bar's measurements

VERIFIED (cm15-cm17, cm22-cm25; measured with PIL).

- **Hovering a bar button draws a filled square behind it**: (36,36,36) **#242424**, darker than
  the bar's (51,51,51), **16x16 px** (e.g. "Decrease": x 946-961, y 466-481); no border; the glyph
  keeps its colour. The squares of the seven buttons sit at x 886, 906, 926, 946, 966, 986, 1006:
  **a 20 px pitch, so 4 px between squares**.
- **Bar height**: a 1 px rule (82,82,82) #525252 at y 465 above it and one at y 483 below it, the
  bar's background (51,51,51) between them, y 466-482: **17 px inside, 19 px rule to rule**. The
  hover square fills y 466-481 of it.
- Glyph ink (cm14): ␣ x 890-897, ¶ 910-915, wrap 929-938, −≡ 948-959, +≡ 968-979, ↕≡ 988-999,
  ◫ 1008-1019; ink y within 467-480.

## The commits added for question 5

Two commits were added on `main` after the seven (Git Bash, cm37): `dce5f7f Long file` (`seq 1 40 >
long.txt`) and `4dd1fdc Long file middle line` (line 20 changed to `twenty`). Questions 1-4 were
observed before they existed (cm6-cm14).

## What was not observed

- Help > About was not reopened; the version is the one recorded on this image the same morning.
- The context setting's value before this run (question 5): the first presses were on a one-line
  file. It was left at three lines.
- Whether Fork compares the committer's instant or its date as shown (question 3): with every date
  drawn in the machine's offset, the two rules agree on this fixture.
- The committer column's x rule (question 2) rests on two pane widths.
- The Activity Manager: no question turned on a git command, so it was not opened.
- The hover look of a toggled-ON button.
