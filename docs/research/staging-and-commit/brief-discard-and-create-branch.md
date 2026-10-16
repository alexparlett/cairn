# Design brief: Discarding, and Create Branch

Date: 2026-10-10. Research for the redesign of two flows in the staging-and-commit packet:
discarding (files, chunks, lines; the confirmation; what cannot be discarded; mixed
selections) and Create Branch (New Branch…, the chord, the name check, "Check out after
create", "Local changes:", and what Discard does and how it is confirmed).
Inputs: review findings H4, H5, M4, M9, M10 (`ux-commit-and-dialogs.md`), M1–M3 and L2
(`ux-local-changes.md`), H1 and H2 (`code-engine.md`); the packet's Fork evidence; the code
on `staging-and-commit` at `2bf8394`; a fresh pass over Fork's release notes and both
trackers; and git 2.56 experiments in a throwaway repository (never the bench).

Confidence labels: **VERIFIED** (seen in a primary source or reproduced), **INFERRED**
(reasoned from evidence), **UNKNOWN**. Quotes are at most 15 words, each with its URL.

---

## 1. Fork's behaviour

### 1a. Discarding whole files

1. The person selects one or more rows in Unstaged. Mac context menu `Discard Changes...`,
   Windows `Discard changes...`; chords ⌫ / ⇧⌘D (Mac), Backspace, Delete, Ctrl+Shift+D
   (Windows). VERIFIED (`fork-staging-and-commit.md` §2, vendor docs).
2. A dialog always opens. No modifier skips it; the vendor refuses to make it faster:
   "I don't want users to speed up the discard."
   (<https://github.com/fork-dev/Tracker/issues/2264>, 2024-12). VERIFIED.
3. Wording, per platform:
   - Windows (2021 screenshot, TrackerWin #1348): title `Discard changes`; text
     `Do you want to discard all your changes in the selected files?` then
     `You can't undo this action.`; buttons `Discard Changes in 18 Files`, `Cancel`
     (primary first). VERIFIED for 2021; current text UNKNOWN but no release note since
     touches it.
   - Mac (2022 vendor video, Tracker #1754): the question is the title, text
     `All uncommitted changes will be lost.`; buttons `Cancel`, `Discard` (default).
     VERIFIED for 2022.
   - Untracked files get the same dialog, never "delete" wording. VERIFIED (Tracker #1754).
   - The dialog names no files and no counts beyond the button's file count. VERIFIED.
4. Return confirms (Discard is default). A 2020 request to make Cancel default is still
   open (<https://github.com/fork-dev/Tracker/issues/1080>). VERIFIED that it is open;
   current default UNKNOWN (INFERRED unchanged).
5. Mixed selection of files and submodules: Fork asks twice — files first, then a second
   `Discard Submodule Changes` confirmation (vendor, 2019:
   "will ask a confirmation to discard local changes and then ask again",
   <https://github.com/fork-dev/TrackerWin/issues/431>). VERIFIED.
6. No Discard All, no trash, no undo, no backup. VERIFIED (§2 of the existing record).

### 1b. Discarding a chunk or lines

1. Hovering a chunk in the unstaged diff floats `Stage` and `Discard Changes…` (Mac) /
   `Stage` and `Discard…` (Windows). Drag-selecting lines narrows them to the lines.
   VERIFIED (Finding 23 of the detail/diff record).
2. The lines dialog (Mac, 2017): `Do you want to discard the changes in the selected lines?`,
   `You can't undo this action.`, buttons `Cancel`, `Discard 2 Lines`. VERIFIED for 2017;
   the chunk dialog's current wording UNKNOWN.
3. Discarding lines of a new untracked file fails on Windows today with git's
   `new file … depends on old contents` (<https://github.com/fork-dev/TrackerWin/issues/2850>,
   2026-09, open). VERIFIED as reported; it shows Fork applies a reverse patch with git.

### 1c. What Fork will not discard

- **Staged changes**: never, file or chunk; reaffirmed 2026-04: "this may lead to conflicts
  and potential data loss" (<https://github.com/fork-dev/TrackerWin/issues/2767>).
  On the staged side Fork offers no discard at all (nothing happens). VERIFIED.
- **Conflicted files**: Fork runs the discard and shows git's `checkout-index … is unmerged`
  error (TrackerWin #1859, open). VERIFIED. Fork does not pre-refuse.
- **Submodules**: Fork *does* discard them (`Discard Submodule Changes`), and has bugs doing
  so (TrackerWin #2103: it deleted new files inside the submodule). VERIFIED. Cairn's
  refusal (R3.10) is a deliberate deviation.

### 1d. Create Branch

1. `New Branch…` on any commit row's menu (⇧⌘B / Ctrl+Shift+B opens it at HEAD). VERIFIED.
2. Dialog (Windows, 2025, <https://github.com/fork-dev/TrackerWin/issues/2472>): title
   `Create Branch`; subtitle `Use '/' as a path separator to create folders`;
   `Create branch at:` (read only); `Branch name:` field; `Check out after create`;
   while ticked and the tree is dirty, `Local changes:` radios **in this order**:
   `Don't change`, `Stash and reapply`, `Discard`; refusal beside the buttons with a
   warning triangle (`Name cannot end with '.'`); buttons `Create and Checkout` (disabled
   while the name is refused), `Cancel`. VERIFIED (screenshot viewed).
3. **The primary button does not change with the radio**: with `Stash and reapply` chosen it
   still reads `Create and Checkout`. VERIFIED for Stash; for Discard INFERRED (no capture
   with Discard selected).
4. History of the options: the Checkout dialog had them by 2018 (Tracker #419), Mac order
   then `Stash and reapply / Don't change / Discard`, button `Checkout` whatever was chosen
   (vendor screenshot, <https://github.com/fork-dev/Tracker/issues/2186>, 2024-08,
   VERIFIED). Fork 2.47 Mac (25 Oct 2024) / 2.2 Windows (30 Oct 2024):
   "Rework uncommitted changes options in Checkout and Create Branch dialogs"
   (<https://git-fork.com/releasenotes>). Windows 2.2 briefly dropped `Don't change`
   and 2.2.3 brought it back after users objected
   (<https://github.com/fork-dev/TrackerWin/issues/2360>). VERIFIED.
5. `Don't change` = `git checkout <name>` with default arguments, so git refuses where
   changes would be overwritten (vendor: "runs checkout with default arguments",
   <https://github.com/fork-dev/Tracker/issues/419>, 2024-03). VERIFIED. A failed checkout
   shows Fork's error dialog with git's words (fixed regression, TrackerWin #2325,
   2024-09). VERIFIED.
6. `Discard` ran **with no confirmation** in 2018; Fork's only response was to stop
   remembering it (1.0.71). VERIFIED. Whether 2.47's rework added a confirmation: UNKNOWN
   (no release note says so; INFERRED not).
7. **What command Discard runs: UNKNOWN.** No tracker log, release note or blog post shows
   it. INFERRED `git checkout -f` (git's own spelling of "switch and discard local
   changes", and Fork's Hard reset "discard all local changes" naming,
   Tracker #1785) — but equally possibly `reset --hard` then checkout. The difference only
   shows when an untracked file is in the way (§2).
8. The name typed is kept after a failed create (Mac 2.7, Win 1.82;
   <https://github.com/fork-dev/Tracker/issues/1363>). VERIFIED.
9. A Windows request to add Stash and reapply to Create Branch (TrackerWin #1311) was closed
   2026-10-07 — consistent with 2.47 having shipped it. VERIFIED closed; reason INFERRED.

## 2. Git's behaviour (git 2.56, reproduced)

| Case | `git checkout -b t <c>` (Don't change) | `git checkout -f -b t <c>` (today's Discard) |
| --- | --- | --- |
| Tracked edit the target does not touch | carried over | discarded |
| Tracked edit the target also changes | refused: `Your local changes to the following files would be overwritten by checkout:` + list | discarded |
| Staged new file | carried over | deleted |
| Untracked file at a path the target holds | refused: `The following untracked working tree files would be overwritten by checkout:` + list, `Please move or remove them…` | overwritten silently |
| Untracked folder (or nested repo) where the target holds a file | refused: `Updating the following directories would lose untracked files in them:` | deleted whole |
| **Ignored** file at a path the target holds | **overwritten silently** | overwritten silently |
| Untracked file nowhere in the target | kept | kept |

All VERIFIED in a throwaway repository. Observations that drive the design:

- A refused `checkout -b` creates **no branch** (`git branch --list` empty). VERIFIED.
- git reports every category in one refusal (tracked and untracked lists together).
- Ignored files are lost on *every* checkout, kept or discarding — git parity, not a Cairn
  gap; Cairn's "ignored files not in the prompt" residual (`checkout.rs` header) is the
  same loss `Don't change` already has.
- `git switch --discard-changes` is git's own name for `-f`.
- A tracked-only discard then a plain checkout is composable from git: `git stash create`
  (objects only, no ref, no stash list entry) → `git reset --hard` → `git checkout -b`;
  if git refuses, `git stash apply --index <id>` puts the staged and unstaged changes back
  **exactly** (status byte-identical, a staged new file, a staged+unstaged file and an
  unstaged edit all restored). VERIFIED.

## 3. Cairn today, and where it diverges

### 3a. Discarding files
1. Unstaged rows selected → menu `Discard Changes…` / chord / ⌫. VERIFIED (`local_changes_menu.rs`).
2. Engine reads the consequence ("Reading what the discard would lose…"), then the dialog:
   title `Discard changes`; prompt e.g. *"Do you want to discard the changes in 3 files
   (a.rs, b.rs and c.rs)? 2 modified (14 lines), 1 untracked file deleted (2.1 KiB). You
   can't undo this action."*; one file adds mode prose `(100644 to 100755)`; kinds
   `modified / emptied / restored / deleted`; button `Discard Changes in 3 Files`; focus on
   Cancel (ratified deviation). (`consequence.rs::discard_files_prompt`.)
3. Refusals: Staged list → line "Staged changes can't be discarded: unstage them first."
   (ratified deviation, Fork does nothing); a submodule or conflicted path **anywhere in the
   selection refuses the whole discard, naming only the first** (`no_discard` → M3); the menu
   item is disabled with its reason cut at `max_lines(2)` (M1); nested repository refused
   before the dialog.
- Divergences from Fork: counts by kind and named files in the sentence (Fork: none);
  mixed selection refused whole (Fork: asks twice); submodule never discarded (Fork: does);
  conflicted pre-refused (Fork: lets git fail).

### 3b. Discarding lines/chunks
1. Floating `Discard Changes…` (chunk) / `Discard 2 Lines…` (selection) / `Discard Mode
   Change…`. Prompt *"Do you want to discard 2 lines in a.rs? You can't undo this action."*
   (or "… and the mode change (100644 to 100755) …"); buttons `Discard 2 Lines`,
   `Discard 2 Lines and Mode Change`, `Discard Mode Change`. VERIFIED (`staging_gesture.rs`,
   `consequence.rs`).
- Divergence: title case floating label vs sentence-case title `Discard changes` (L2); octal
  modes in prose.

### 3c. Create Branch
1. Dialog as Fork's, but `Local changes:` offers only `Don't change` and `Discard` (Stash
   waits for packet 5b, decision 2; the dialog does not say so — M9). Shown only when
   `entries.first()` is a tracked change or conflict (status-order dependence; an
   untracked-only tree never shows it — M9).
2. With `Discard` the button still reads `Create and Checkout` (as Fork). Pressing it asks
   the engine for `CheckoutDiscarding`; **the Create Branch dialog closes** and a second
   dialog titled `Discard changes` opens (M4) with e.g. *"Do you want to create branch
   'topic' at 1a2b3c4, check it out and discard the changes in 3 files (a.rs, b.rs and
   c.rs)? 2 modified (14 lines), 1 new file deleted (3 lines), 1 untracked file overwritten
   (2.1 KiB). Deleted because the branch has a file there: folder d/ (4 untracked files)
   and repository vendor/lib/ with its history. Other untracked files are kept. You can't
   undo this action."* (H5); button `Discard Changes and Check Out`.
3. **Cancel there throws the whole Create Branch away** (target commit, tick, radio); only
   the name returns on a later opening (H4; `create_branch.rs::confirm_arrived` sets
   `state.open = None`).
4. Confirm runs `git checkout -q -f -b <name> <oid> --` after re-reading the whole
   consequence (`checkout.rs`). Pre-prompt refusals (operation in progress, conflict,
   submodule change) appear in the dialog's refusal row (max 2 lines).
- Divergences: a confirmation Fork lacks (ratified, good); a predicted, enumerated loss
  model git does not offer (H2: D/F folders, nested repos, `showUntrackedFiles`, ignored
  files unmodelled; "lines" double-counted for a re-edited staged hunk).

## 4. Proposed clean flows (storyboards)

Shared rules for every destructive confirmation:
- **Frame**: `<Question naming verb + object>? [Worst loss, only when worse than "changes".]
  You can't undo this.` One verb family: **Discard** (changes, lines) — and the word
  **delete** used only inside the worst-loss sentence for whole untracked files/folders.
- **Detail behind a disclosure**: a `Show files` toggle under the prompt expands a
  virtualized list (path + what happens: `Restored`, `Deleted`, `Emptied`, `14 lines`,
  `Mode`), never in the sentence, never capped. Not shown when the object is one path or
  lines on screen (never repeat what is on screen).
- **Titles and buttons in Title Case** (`Discard Changes`, `Create Branch`): one convention,
  matching Fork's `Create Branch` title and both platforms' buttons (L2).
- Buttons in platform order (Linux/Windows: primary first; macOS: Cancel first); focus on
  Cancel; Escape = Cancel = back one step.

### 4a. Discard files (list, chord, menu)
1. Person selects rows in Unstaged; presses ⌫/Delete/Ctrl+Shift+D or menu `Discard Changes…`.
2. While the engine reads (usually instant): the list row area shows nothing new; if it takes
   >150 ms the dialog opens with a spinner in place of the prompt (Fork's in-dialog spinner,
   Windows 1.48).
3. Dialog, title **Discard Changes**:
   - one path: *"Discard all changes in `src/main.rs`? You can't undo this."*
   - several, no untracked: *"Discard all changes in 31 files? You can't undo this."*
   - with untracked files (worst first): *"Discard all changes in 31 files? 2 untracked files
     will be deleted. You can't undo this."*
   - `Show files` disclosure (31 rows).
   - Buttons (Linux): **Discard Changes in 31 Files** · Cancel.
4. Mixed selection (submodule / conflicted rows among them): the dialog acts on the
   discardable rows and adds one line under the prompt: *"1 submodule and 1 conflicted file
   are left as they are."* The button counts only what it discards. (Fork asks twice; Cairn
   never discards submodules, so the second ask becomes this line.)
5. Selection with nothing discardable: menu item **disabled**, reason drawn as a wrapping
   line under it (no line cap), one template `<what> can't be discarded. <way forward>.`:
   - *"Staged changes can't be discarded. Unstage them first."*
   - *"Submodule changes can't be discarded."* (Fork does; Cairn's ratified deviation)
   - *"Conflicted files can't be discarded. Resolve or stage them first."*
   The chord over such a selection writes that same line in the list's status strip.
6. Confirm → rows leave the list; failure → the strip says *"Nothing was discarded: <git's
   words>"* or the partial outcome (*"3 of 31 files were not discarded"*, with `Show files`
   in a Git Error dialog — never paths enumerated inline).

### 4b. Discard lines / a chunk / a mode change (diff gesture)
1. Hover chunk → floating **Stage** · **Discard…**; drag-select → **Stage 2 Lines** ·
   **Discard 2 Lines…**; mode row → **Discard Mode Change…**.
2. Dialog, title **Discard Changes**: *"Discard 2 changed lines in `src/main.rs`? You can't
   undo this."* / *"Discard this chunk (6 lines) in `src/main.rs`? …"* / *"Discard the mode
   change of `run.sh`? …"* (no octal in prose: the mode row is on screen). No disclosure.
3. Buttons: **Discard 2 Lines** · Cancel (or **Discard Chunk**, **Discard Mode Change**).
4. Staged diff: no discard control at all (as Fork); chord there → the 4a.5 staged line.

### 4c. Create Branch
1. History row → `New Branch…` (or Ctrl+Shift+B / ⇧⌘B at HEAD). Dialog **Create Branch**,
   subtitle *"Use '/' as a path separator to create folders"*, `Create branch at:` ◇ short id
   + subject, `Branch name:` field, `Check out after create` (sticky for the session).
2. Typing: the name is checked on the worker; a refusal sits left of the buttons behind the
   warning glyph, wrapping (not cut): Fork's *"Branch topic already exists"*, otherwise git's
   words. Button disabled while refused.
3. Box ticked and **any tracked change** (staged, unstaged, conflicted — a named
   `has_tracked_changes()`, not `entries.first()`): `Local changes:` with Fork's order
   **Don't change** · **Stash and reapply** · **Discard**. Until 5b ships, `Stash and
   reapply` is drawn **disabled** with a one-line hint *"Comes with stashing."* (honest about
   the deviation, M9) — or omitted if the user prefers (§5, choice E). Default each opening:
   Don't change; Discard never remembered.
4. Buttons (Linux): **Create and Checkout** · Cancel; with Discard selected:
   **Create and Checkout…** (the ellipsis says another step follows) — §5 choice C.
5. Don't change → `git checkout -q -b <name> <oid> --`. If git refuses, Git Error opens
   **over** the still-open Create Branch dialog with git's words; closing it returns to the
   dialog, name, tick and radio intact.
6. Discard → the engine reads the tracked changes; a pre-refusal (operation in progress,
   conflicted path, submodule change) appears in the dialog's refusal row in the 4a.5
   template (*"Conflicted files can't be discarded. Resolve or stage them first."*).
   Otherwise a confirmation opens **as a layer over** Create Branch:
   - Title **Discard Changes**.
   - *"Discard all changes in 3 files, staged changes included, and check out new branch
     'topic'? You can't undo this."* (the staged clause is the one exception to "staged is
     never discarded", so it is said; omitted when nothing is staged).
   - `Show files` disclosure.
   - Buttons (Linux): **Discard and Check Out** · Cancel. **Cancel returns to Create Branch
     exactly as it was.**
7. Confirm → under the recommended engine (§5 choice A, option A3):
   `git stash create` → `git reset -q --hard` → `git checkout -q -b <name> <oid> --`.
   - Success: both dialogs close; refs re-read; the chip sits on the commit, `HEAD`.
   - git refuses (an untracked file or folder in the way): `git stash apply --index <id>`
     restores every change; Git Error opens over Create Branch with git's own list and one
     Cairn line *"Nothing was discarded."*; closing it returns to Create Branch.
8. Activity entry: *"Create branch 'topic'"* with the prompt and every git run.

## 5. Unknowns and choices

### A. What Create Branch's Discard runs (the user decided `checkout -f`, 2026-10-09)

| Option | Person experiences | Cost |
| --- | --- | --- |
| **A1 — keep `git checkout -f -b`** (today) | Untracked files/folders/nested repos where the target has a file are deleted; the prompt must name them (headline when a folder or repo is lost, rest in the disclosure). One git process, atomic. | Keeps the prediction engine (`untracked_losses`, D/F, nested repos, `showUntrackedFiles`, re-check by count+bytes) and its known holes; every new git edge (case-insensitive FS, sparse, symlinks) is a new CRITICAL risk. Matches git's `--discard-changes` and Fork's INFERRED command. |
| **A2 — tracked discard (`reset --hard`), then plain `checkout -b`** | Same as A1 unless something untracked is in the way; then git refuses — **but the changes are already gone and no branch exists**. | No prediction. Partial outcome after a confirmation: the person loses what they agreed to lose and gets nothing they asked for. |
| **A3 — A2 made all-or-nothing with `git stash create` / `stash apply --index`** (recommended) | Same as A1 in every case except untracked-in-the-way, where nothing changes and Git Error lists git's paths with "Nothing was discarded." | No prediction for untracked content; ~3 git calls; the prompt is the Local Changes frame. Adds `stash create`/`stash apply` (writes, `ops/`) — same machinery 5b's Stash and reapply needs anyway. Edge to test: intent-to-add entries under `stash create` on git 2.30–2.34. |

**Recommendation: A3.** Why change the 2026-10-09 decision: (1) git parity — every byte
Cairn loses is one git chose to lose under a command Cairn ran, and every byte git refuses
to lose stays, so Cairn never has to *predict* git's unpack-trees (H2; the CRITICAL class of
phase 10 QA items 1–3 disappears rather than being patched); (2) the prompt shrinks to the
same frame as Local Changes' discard (H5, M10); (3) the ignored-file residual stops being a
Cairn-specific gap — it is plain checkout's behaviour, identical under Don't change.
What the person sees differently from today: only when an untracked file, folder or nested
repository sits where the target commit has a file — today Cairn names it and deletes it;
under A3 nothing is lost and git's refusal says what to move. Fork's command is UNKNOWN, so
neither A1 nor A3 is a proven Fork deviation. If stash machinery is not wanted before 5b,
**A1** with the new prompt frame is the fallback; **never A2**.

### B. Confirm Create Branch's Discard at all?
Fork (2018) did not and may still not (UNKNOWN). Options: (B1) confirm (today, ratified
deviation) — recommended, Cairn's promise is "tells you what it will cost"; (B2) no
confirmation, as Fork — fastest, loses work on a misclick. Keep B1.

### C. The Create Branch button while Discard is selected
- C1 `Create and Checkout` (Fork, INFERRED) — honest only because a confirmation follows,
  but reads safe (H4).
- C2 `Create and Checkout…` — Fork's words, the ellipsis signals a further step;
  smallest deviation. **Recommended.**
- C3 `Discard and Check Out…` — most explicit, but drops "Create" and duplicates the
  confirmation's button (repeats what is on screen next).

### D. Discard dialog default button
Fork: Discard default (INFERRED current). Cairn: Cancel (ratified). Keep — Tracker #1080 is
exactly the accident it prevents.

### E. Fork's third radio before packet 5b
- E1 omit (today) — the dialog silently differs from Fork (M9).
- E2 show disabled with *"Comes with stashing."* — honest, Fork's layout; recommended.
- E3 ship Stash and reapply now — needs 5b's stash verbs; with A3 most of it exists
  (`stash create` + `apply --index`), so E3 may be cheap: worth asking.

### F. Mixed selections
- F1 discard the discardable rows, one line saying what is left (recommended; one dialog).
- F2 Fork's two confirmations — not applicable: Cairn never discards submodules.
- F3 refuse whole, name all offenders with a count — safe but makes the person deselect.

### G. Submodules and conflicts
Fork discards submodules and lets git fail on conflicts. Cairn refuses both (R3.10, R3.11,
user-decided). Keep; only the wording changes (4a.5). UNKNOWN: Fork's current submodule
discard command (TrackerWin #2103 shows it deletes untracked files inside the submodule).

### H. Title wording
Fork Windows `Discard changes` (sentence) vs its own `Create Branch` (title). Recommend Title
Case everywhere (`Discard Changes`), one convention for both platforms (L2).

### I. Remaining Fork UNKNOWNs a few minutes on the owner's Mac would settle
Fork's current chunk-discard dialog text; whether 2.47+ confirms Create Branch's Discard;
the Discard command (Activity Manager shows it); the default button today.

## 6. What the redesign removes (roughly)

Model (`cairn-model/src/consequence.rs`):
- `ChangeLoss::Overwritten`, `ChangeLoss::Removed`, `RemovedKind`, `kept_untracked` and its
  three conditional endings; `checkout_discarding_prompt`'s `in_the_way` branch and
  `named_list` for folders/repos (A3). `CheckoutDiscarding` becomes "branch + at + the same
  tracked-file losses `DiscardFiles` uses, staged included".
- `discard_files_prompt`'s per-kind sentence grammar (`modified / emptied / restored /
  deleted`, `line_detail`, mode prose with octal, `NAMED_FILES` naming) → one sentence + a
  per-file detail list for the disclosure.
- `discarded_lines`' octal mode prose.
Engine (`cairn-git`):
- `ops/checkout.rs`: `untracked_losses`, `held`, `content_under`, `first_difference` and the
  `-f` argument (~300 lines, H2); `reads/untracked.rs` (used only by checkout) if A3.
- Re-check simplifies toward H1's one witness (independent of this brief, but A3 leaves only
  tracked-path witnesses to compare).
App/UI:
- `create_branch.rs::confirm_arrived` closing the dialog, `kept_name` as a recovery path for
  Cancel, `DISCARD_BEFORE_CHECKOUT_TITLE` alias (M4); `has_changes`' `entries.first()`
  dependence.
- `local_changes_menu.rs::no_discard` first-offender refusal and its three voices (M2, M3);
  the menu reason's `max_lines(2)`; inline path enumeration in `acting_line` (M1).
- Mixed-case caption set (`Discard changes` title vs `Discard Changes…`) → one set (L2).
Adds: a `Show files` disclosure in the confirm dialog (virtualized), a layered confirm over
Create Branch, `stash create`/`apply --index` in `ops/` (A3), `has_tracked_changes()`.
