# Fork's staging, discard, ignore, commit box, stash, reflog and activity log, as published

Evidence record. Date: 2026-10-07. Commissioned by: staging-and-commit planning.

It answers, per platform (macOS first, Windows differences noted), how Fork
(git-fork.com / fork.dev; AppKit on Mac, WPF on Windows) lets a person stage and
unstage files, hunks and lines; discard them and what it says when it does; clean
untracked files; add to `.gitignore`; write, amend and push a commit, and what
happens when a hook fails; save, list, apply and delete stashes; see lost commits
through the reflog; and see the `git` it ran. Historical: never retro-edited.

Companions, cited rather than repeated:

- `docs/research/diff-engine/fork-detail-and-diff-ui.md` — Finding 23 is the
  hover-chunk / drag-select staging gesture, the Return / Backspace keys, the
  refusal to discard staged changes and the `git apply --cached` mechanism. This
  record adds labels, dialogs and later evidence around it, and does not repeat it.
- `docs/research/diff-engine/fork-shortcuts.md` — the vendor shortcut lists and
  the AltGr / desktop-clash hazards.
- `docs/research/refs-and-status/fork-refs-and-status-ui.md` — Section 3 (stashes
  inline in the commit list and in the sidebar), Section 6 (the Local Changes
  layout, badges, untracked and ignored toggles) and Section 7 (refresh). One
  correction to Section 6 is recorded below.
- `docs/design/ui.md` — the design this evidence tests.

## Method and sources

Everything here is documentary. Fork has no Linux build, so nothing was installed
or clicked.

| Source | What was read |
| --- | --- |
| Mac release notes, <https://git-fork.com/releasenotes> | Every entry, GitClient 1.0.0 (Apr 2016) to Fork 2.70 (4 Sep 2026) — 1,171 entries, flattened one per line and searched for stash, discard, untracked, clean, ignore, exclude, trash, commit, amend, sign, GPG, template, hook, push, stage, chunk, lines, reflog, lost, undo, output, activity, error, refresh, progress |
| Windows release notes, <https://git-fork.com/releasenoteswin> | Every entry, 1.14 (Mar 2018) to 2.23 (25 Sep 2026) — 752 entries, same searches |
| Blog, <https://fork.dev/blog/posts/> | All 14 posts; GIFs reduced to frames and read (Mac 1.0.67 activity manager, Mac 1.0.70 stage-all, Win 1.48 partial stash and git output) |
| Home page, <https://git-fork.com/> | Feature list; the two Local Changes carousel images (Mac and Windows) |
| Vendor-hosted docs, `fork-dev/Docs` | `keyboard-shortcuts-mac.md`, `keyboard-shortcuts-windows.md`, `faq.md` (last commit 2023-10-04) |
| Trackers | `gh search issues` over `fork-dev/Tracker` (Mac) and `fork-dev/TrackerWin` (Windows), 52 queries per tracker plus targeted phrase queries; 157 Mac and 116 Windows issues fetched with every comment and read; screenshots and three vendor/user videos (Tracker #1754, #2095 ×2) reduced to frames and read |

Conventions. `Tracker #N` is `https://github.com/fork-dev/Tracker/issues/N`;
`TrackerWin #N` is `https://github.com/fork-dev/TrackerWin/issues/N`. "Mac RN x" and
"Win RN x" are entries on the two release-notes pages; the apps are versioned
independently (Mac 2.21 is Aug 2022, Windows 2.21 is Jul 2026). `DanPristupov`
is the vendor. Labels: **RN** release note; **VENDOR** a vendor comment (paraphrased,
permalink given); **VSHOT** vendor screenshot or video; **USHOT** user screenshot or
video, dated; **USER** user report; **DOCS** the vendor shortcut lists or FAQ;
**INFERRED** reasoning over the above; **UNVERIFIED** looked for, not established.
UI strings are quoted exactly as drawn; vendor prose is paraphrased.

## Headlines

1. **File staging has five routes, all toggles**: double-click a row, Return (or
   ⌘S / Ctrl+Shift+S) on the selection, drag between the lists, the `Stage` /
   `Unstage` button in each list's header, and the context menu. Stage All is a
   small double-chevron above the lists, ⌥⇧⌘S / Ctrl+Alt+Shift+S, or ⌥ held over
   the header button — and the vendor keeps it **deliberately hard to hit** because
   changes must be reviewed before staging (TrackerWin #2565). Committing with
   nothing staged is not offered and `commit -a` is refused (Tracker #2409).
2. **Discard always confirms, never undoes, and is kept slow on purpose.** The
   dialog asks whether to discard all changes in the selected files (or lines) and
   says the loss is permanent; the button carries the count. There is no
   discard-all button, no modifier to skip the dialog, no trash and no backup —
   each refused by the vendor (TrackerWin #399, #1466; Tracker #2264; TrackerWin
   #657). Staged changes cannot be discarded (TrackerWin #1563, reaffirmed #2767,
   Apr 2026). On Windows an untracked file is deleted outright with `File.Delete`
   (TrackerWin #1457, #1807).
3. **No clean.** Untracked files are discarded like any other change; a `git clean`
   command was declined (TrackerWin #2220).
4. **Ignore** is a context submenu: `Ignore '<file>'` writes at once; `Ignore All
   Files in '<folder>'...` and `Custom Pattern...` open an `Add Pattern to
   .gitignore` dialog with a live preview of matching files. It writes the root
   `.gitignore` only, and untracks matching tracked files (since Mac 1.0.94).
   No `.git/info/exclude`, global ignore, nested `.gitignore` or un-ignore.
5. **The commit box** is a subject field (with a remaining-characters counter and a
   Recent Commit Messages menu), an auto-growing description (ruler, wrap to ruler),
   an `Amend` checkbox, and a `Commit N Files` button — ⌘Return / Ctrl+Enter.
   Commit and Push is ⌥ held (Mac, ⌥⌘Return) or the button's dropdown (Windows,
   Ctrl+Shift+Enter). Sign-off and GPG are per-repository settings, not
   commit-box controls. There is no author override and no up-front `--no-verify`.
6. **A failing hook** produces the `Git Error` modal with git's output and a `Skip
   pre-commit hooks and commit` button beside `Close`. That button is the only
   route to `--no-verify` in Fork, by design (Tracker #948, #1970, #2568).
7. **Stash** is a toolbar button with a dialog (message, `Stage new files`), a
   dropdown of recent stashes plus `Save Snapshot...`, a per-file `Stash N
   Files...`, and an `Apply Stash` dialog whose `Delete stash after applying`
   checkbox is the difference between `apply` and `pop`. No keep-index, no
   staged-only, no single-file apply.
8. **Reflog is a graph mode, not a list**: View → Show Lost Commits (Reflog) draws
   unreachable reflog commits into the commit graph, dimmed; you recover by
   putting a branch or tag on one. No undo. Fork's own operation log is the
   **Activity Manager**, a popover off the toolbar's status box listing operations
   and the `git` commands and output behind them — not every command, and nothing
   kept after quitting (TrackerWin #514, Tracker #2232).

## 1. Staging and unstaging

### Files

- **Header buttons.** Each list has a header — `Unstaged` with a `Stage` button,
  `Staged` with an `Unstage` button (VSHOT Mac and Windows carousels,
  <https://git-fork.com/images/carousel/carousel_commitviewMac1.jpg>,
  <https://git-fork.com/images/carousel/carousel_commitviewWin1.jpg>). The button
  acts on the selected rows and is greyed with nothing selected (VSHOT Tracker
  #1754, Nov 2022). Older captures show the headers as `Unstaged Files` /
  `Staged Files` (blog Mac 1.0.70) and `Unstaged Changes` / `Staged Changes` (blog
  Win 1.48, USHOT Tracker #901, 2020).
- **⌥ turns them into Stage All / Unstage All** (Mac RN 1.0.23, Sep 2016; VENDOR
  Tracker #47, <https://github.com/fork-dev/Tracker/issues/47#issuecomment-310434795>).
  The Tracker #901 capture (Mac, 2020) shows the headers reading `Stage All` /
  `Unstage All`. Windows equivalent: UNVERIFIED.
- **Stage All as a double chevron.** Above the two lists sit three icons: a
  double-chevron, an eye (quick look) and a layout menu. The Mac 1.0.70 blog GIF
  (<https://fork.dev/blog/posts/fork-1.0.70/>) shows clicking the downward double
  chevron moving every unstaged file to Staged, after which it flips to an upward
  double chevron that moves them all back. Windows got a `Stage All` button in the
  Unstaged header in 1.18 (blog Win 1.18), drawn as the same double chevron in
  later captures (USHOT TrackerWin #1348, 2021). The vendor calls the chevron
  intentionally hard to hit and the shortcut intentionally complicated (VENDOR,
  TrackerWin #2565,
  <https://github.com/fork-dev/TrackerWin/issues/2565#issuecomment-3178813235>,
  Aug 2025).
- **Double-click** a row stages or unstages it (Mac RN 1.0.20, Sep 2016; regression
  and fix Tracker #970, 2020; VENDOR Tracker #974 recommends it). Windows users ask
  to turn it off because double-clicking a folder to expand it stages the whole
  folder; open (TrackerWin #2062, 2023–2026).
- **Keys.** Return or ⌘S (Mac), Enter or Ctrl+Shift+S (Windows) stage or unstage
  the selected files or lines; ⌥⇧⌘S / Ctrl+Alt+Shift+S stage or unstage all (DOCS
  shortcut lists; Mac RN 1.0.81 added ⌥⇧⌘S). ⌘⇧S / Ctrl+Shift+S toggles in either
  list (VENDOR Tracker #54, 2017). Win RN 1.48 fixed AltGr+S staging for Polish
  users.
- **Drag** files or folders between the two lists (Mac RN 1.0.31, Nov 2016; Win RN
  1.40, Oct 2019; Mac RN 1.0.95 fixed folder drags).
- **Context menu** (unstaged row, Mac, USHOT Tracker #2095 video, Mar 2024): `Open
  ⌥⇧⌘O`, `Open With ›`, `External Diff ⌘D`, `Show in Finder` | `Blame/Timeline...`,
  `History...` | `Stage ⌘S`, `Discard Changes... ⇧⌘D` | `Stage All ⌥⇧⌘S` | `Ignore
  ›` | `Stash 1 File...`, `Save as Patch...` | `Copy Path ⌘C`. Windows (VSHOT
  TrackerWin #326, Jan 2022): `Open Ctrl+Shift+Alt+O`, `External Diff Ctrl+D`, `Show
  in File Explorer` | `Blame/Timeline...`, `History...` | `Stage Enter`, `Discard
  changes... Delete` | `Stage All Ctrl+Shift+Alt+S` | `Ignore ›` | `LFS ›` | `Stash 1
  File...`, `Save as Patch...` | `Copy Full Path Ctrl+C`. The staged list's menu
  offers Unstage and Unstage All (USER Tracker #1152, 2020); its full contents:
  UNVERIFIED.
- **A folder acts on everything under it.** Staging or discarding a selected folder
  includes every file inside, even rows that look deselected; the vendor defends
  this as how file managers and IDEs behave (VENDOR TrackerWin #1348, 2021; #2682,
  Dec 2025).
- **Selection after staging** moves to the nearest remaining unstaged file; the
  vendor considers review-stage-repeat the common pattern (VENDOR Tracker #514,
  <https://github.com/fork-dev/Tracker/issues/514#issuecomment-460979499>, 2019;
  Mac RN 1.0.55 fixed which file is chosen next). When the last file is staged the
  diff goes blank; auto-switching is an open request (Tracker #1663). After a
  commit Fork stays on Local Changes (VENDOR TrackerWin #118, 2019; switching to
  All Commits is an open request, Tracker #1668). There is always a selected file
  — Fork selects one when files appear (VENDOR Tracker #2333, 2025).
- **Undo of staging** was refused: another tool may have staged in between (VENDOR
  Tracker #768, 2019).
- **Mechanism / threading.** Windows moved stage, unstage and discard to a
  background thread in 2.13 (Win RN, Oct 2025). Staging while a commit hook runs is
  still possible on Windows (open, TrackerWin #2547).

### Hunks and lines

Fork's gesture is recorded in `fork-detail-and-diff-ui.md` Finding 23: hover a chunk
for an outline with floating `Stage` and `Discard Changes…` (Mac) / `Stage` and
`Discard…` (Windows); in the staged diff the one button is `Unstage`; drag-select
text across lines and the same buttons and keys act on those lines only; Return /
Backspace act on the selection, or the whole file when nothing is selected
(hovering is not selecting). Additions found here:

- Marketing names the feature "Stage / unstage changes line-by-line" (home page).
- Lines can be selected on one side of a side-by-side diff at a time only (open,
  Tracker #1985); the vendor answered a 2025 request for line staging with a video
  and said it had existed since the first public build (Tracker #2316).
- Partial staging and partial discard of **new, untracked files** have been
  supported since the first builds (Mac RN GitClient 1.0.1, Apr 2016). How Fork
  does it (for example `git add -N` first): UNVERIFIED.
- The chunk Discard asks first (§2); the hunk-level dialog's exact wording on
  current versions: UNVERIFIED (the 2017 lines dialog is quoted in §2).

## 2. Discard

**Where.** File context menu `Discard Changes...` (Mac) / `Discard changes...`
(Windows); ⌫ or ⇧⌘D (Mac), Backspace, Delete or Ctrl+Shift+D (Windows) on the
selected files or lines (DOCS; VSHOT TrackerWin #326); the floating chunk button.
The word "discard" replaced "reset" in Mac 1.0.37 (Mac RN, Feb 2017).

**The dialog.**

- Mac, files (VSHOT Tracker #1754 video, Nov 2022, discarding an untracked file):
  title `Do you want to discard all your changes in the selected files?`, text `All
  uncommitted changes will be lost.`, buttons `Cancel` and `Discard` (the latter
  styled as the default).
- Windows, files (USHOT TrackerWin #1348, Oct 2021): title `Discard changes`, text
  `Do you want to discard all your changes in the selected files?` and `You can't
  undo this action.`, buttons `Discard Changes in 18 Files` and `Cancel`.
- Mac, lines (USHOT Tracker #103, Sep 2017): `Do you want to discard the changes
  in the selected lines?`, `You can't undo this action.`, buttons `Cancel` and
  `Discard 2 Lines`.
- Untracked files get the same dialog as modified ones — no "delete" wording
  (VSHOT Tracker #1754).
- Return confirms: a user who pressed ⇧⌘D for ⌘⇧S nearly confirmed by Return and
  asked for Cancel to be the default; open (Tracker #1080, 2020). Whether the
  current default is Discard: UNVERIFIED beyond the 2022 styling.
- Submodules: `Discard Submodule Changes`; with files and submodules selected Fork
  asks twice, once for each (VENDOR TrackerWin #431, 2019).

**What Fork refuses.**

- **Staged changes**: never discardable, file or chunk — unstaged edits depend on
  staged ones, so dropping the staged layer can lose both (VENDOR TrackerWin #1563,
  <https://github.com/fork-dev/TrackerWin/issues/1563#issuecomment-1157432945>,
  2022); reaffirmed as risking conflicts and data loss (VENDOR TrackerWin #2767,
  <https://github.com/fork-dev/TrackerWin/issues/2767#issuecomment-4322051402>,
  Apr 2026).
- **Discard All** as one action: none in the sidebar, header or menus. The vendor
  says select all and discard (⌘A/Ctrl+A then ⇧⌘D/Ctrl+Shift+D), so the user sees
  what they lose (VENDOR TrackerWin #399,
  <https://github.com/fork-dev/TrackerWin/issues/399#issuecomment-3241922657>,
  Sep 2025; #1466, 2022). In 2018 the vendor had liked a `Discard All Changes...`
  sidebar menu (Tracker #459); it was never built.
- **Skipping the confirmation** (⌥-discard): refused — discard should not be easily
  accessible (VENDOR Tracker #2264,
  <https://github.com/fork-dev/Tracker/issues/2264#issuecomment-2551782436>, Dec 2024).
- **Conflicted files**: discard fails with git's `checkout-index ... is unmerged`;
  the vendor says choose a side instead (VENDOR TrackerWin #1859, 2023; open).
- **Hard reset to the current commit** as a discard: declined; discard explicitly
  (VENDOR TrackerWin #2286, 2024).

**Backup and undo: none.** Requests for trash, recycle bin, a stash-backed undo or
patch backups are open and unanswered or declined: Tracker #1265 (trash, open since
2021), #2175 (undo discard; closed as close to #1265), #981 (undo, open);
TrackerWin #657 (undo discard). The vendor's answers: git discards changes, not
files, and Windows can only move a file to the recycle bin by deleting it (VENDOR
TrackerWin #657,
<https://github.com/fork-dev/TrackerWin/issues/657#issuecomment-1629263545>, 2023);
use stash when you want to keep them (same thread). In 2018 he floated saving the
staged area as a patch in the Trash before a checkout-discard (VENDOR Tracker #419);
not built. The only safety change: the checkout dialog stopped remembering
`Discard` (Mac 1.0.71, same issue). Fork keeps no permanent log of what it did
(VENDOR Tracker #2232, 2024).

**Mechanism.**

- Tracked files: one `git checkout -- <file>` per file in 2019, slow on large
  indexes (USER measurement, Tracker #535); batched after (Mac RN 1.0.75 "Improved
  discard performance for many files"). The current command: UNVERIFIED (a 2026
  Windows user guesses `git checkout` / `git restore`, TrackerWin #2780).
- **Untracked files on Windows are deleted by Fork itself, permanently**: stack
  traces show `System.IO.File.Delete` inside `DiscardFileChangesGitCommand`
  (USER TrackerWin #1457, 2022; #1807, 2023); a user found the file in no recycle
  bin and no recovery tool (TrackerWin #657). Read-only and in-use files used to
  fail or crash (Win RN 1.82, 1.95; TrackerWin #1981).
- Untracked files on Mac: mechanism UNVERIFIED.
- Progress: Windows shows a small `Discarding files` progress window for large
  batches (USER TrackerWin #1191, 2021; #2780, May 2026, where it never closed for
  15,000 files). Mac: a progress request is open (Tracker #2310) — whether Mac
  shows any: UNVERIFIED.

**Related: local changes at checkout.** The Checkout dialog has `Local Changes:`
`Stash and reapply` / `Don't change` / `Discard` (VSHOT Tracker #2186, Aug 2024);
`Don't change` runs a plain `git checkout <branch>` (VENDOR Tracker #419, 2024).

## 3. Clean, untracked and ignored files

- **No clean command.** Select the untracked files and discard them; a custom
  command is the vendor's suggestion for `git clean` (VENDOR TrackerWin #2220,
  <https://github.com/fork-dev/TrackerWin/issues/2220#issuecomment-2106973615>,
  May 2024). `reset --hard` leaves untracked files alone in Fork as in git (VENDOR
  Tracker #1785, 2023).
- **Ignored files** are hidden by default and shown by `Show Ignored Files` in the
  list's layout menu or ⌘⇧. / Ctrl+Shift+. (refs-and-status Section 6). Nothing in
  that view un-ignores a file (USER TrackerWin #2498, 2026). Whether ignored files
  shown that way can be discarded: UNVERIFIED.
- **Untracked files** are shown by default; `Hide Untracked Files` is remembered
  and announced in the Unstaged header (refs-and-status Section 6).

## 4. `.gitignore`

**The submenu** (USHOT Tracker #2095 videos, Mac 2.40, Mar 2024):

- On a file: `Ignore '<file name>'` (seen as `Ignore '.DS_Store'`) and `Custom
  Pattern...`. `Ignore '<file>'` writes at once, no dialog: the file leaves the
  list and `.gitignore` appears modified.
- On a folder: `Ignore All Files in '<folder>'...` and `Custom Pattern...`; the
  first opens the pattern dialog pre-filled with the folder path (`test`).
- An extension entry (`*.ext`): UNVERIFIED — not in the 2024 menus seen; the 2017
  note promised "files, directories or custom patterns" (Mac RN 1.0.39). Folder
  ignore was fixed in Mac RN 1.0.93 after Tracker #990.
- Several selected files become explicit entries since Windows 2.20 (VENDOR
  TrackerWin #2498, Jun 2026); Mac: UNVERIFIED.

**The dialog** (VSHOT blog Win 1.18, 2018; USHOT Tracker #2095, Mac 2024): title `Add
Pattern to .gitignore`; explanatory text that tracked files will be untracked (2018
Windows text said they are not affected — changed with the auto-untrack below);
`Pattern:` multi-line field `(one pattern per line)`; `Preview:` list of matching
files with `N files match`; buttons `Add to .gitignore` and `Cancel` (Mac adds a
`?` help button).

**Which file.** The repository root `.gitignore` only. Adding to a nested
`.gitignore` was declined — edit it by hand; nested files are honoured because git
reads them (VENDOR TrackerWin #2033,
<https://github.com/fork-dev/TrackerWin/issues/2033#issuecomment-1739102915>,
2023). Fork does not parse `.gitignore` itself; the list is `git status` (VENDOR
TrackerWin #869, 2023; #2329, 2024).

**Behaviour.** Ignoring untracks matching tracked files (Mac RN 1.0.94, Jun 2020;
Win RN 1.50–1.51; VENDOR Tracker #1008, TrackerWin #869). `#` in a name is escaped
(Mac RN 2.42, Tracker #2094; Windows 2.4, VENDOR TrackerWin #2395). Line endings: Fork once appended
without a newline (TrackerWin #8, 2018) and still mixes CRLF and LF on Windows
(open, TrackerWin #1361). Mac RN 2.42 fixed the list not refreshing after `Ignore
All Files in`.

**Absent.** `.git/info/exclude` ("ignore locally") — open requests since 2019
(Tracker #688, TrackerWin #1428, Tracker #2670, Jul 2026); a global-ignore entry
(Tracker #688); an `Edit .gitignore` item — none found, UNVERIFIED. A `.gitignore`
template picker exists (Mac RN 2.65, Win RN 2.18, Mar 2026; its placement —
repository creation or elsewhere — UNVERIFIED; TrackerWin #659 asked for it at init).

## 5. The commit box

**Layout** (VSHOT carousels, 2020; USHOT Tracker #901, 2020; #1371, 2021): under the
diff, a bordered box with a single-line `Commit subject` field over a `Description`
field; at the subject's right a remaining-characters counter and a `≡` icon; below,
an `Amend` checkbox at the left and the commit button at the right. Since 1.0.22
(Mac RN, Sep 2016) the box lives in the main window rather than a separate commit
window. Subject font enlarged in Mac 2.56 (Aug 2025).

**Subject counter.** Mac RN 1.0.20 (Sep 2016); Win blog 1.17 (2018). The number is
characters left before a soft limit of 50 (grey); past it, it goes negative, and red
past the hard limit of 70 — a preview of where git tools truncate, not a hard limit
(VENDOR Tracker #633,
<https://github.com/fork-dev/Tracker/issues/633#issuecomment-496451369>, 2019;
VSHOT Win blog 1.17 shows `-38` in red). The limit became configurable (Mac RN
1.0.89, Win RN 1.43, Jan 2020). The subject is required: the button is disabled
with an empty subject even when a description is written (open, Tracker #1490).

**Description.** A page guide (ruler) and optional monospace font (Mac RN 1.0.24,
Win RN 1.18); `Wrap paragraph at ruler` in the field's context menu (Mac RN 1.0.50,
Win RN 1.26 / blog Win 1.28); spell checking (Mac 1.0.22, Win 1.21); grows as you
type (Mac 1.0.69, Win 1.20); minor autocomplete (Mac 1.0.23) and `Co-authored-by`
autocomplete (Win RN 1.72). A draft is remembered across views and, on Windows
since 2.23, per worktree; merge, revert and cherry-pick pre-fill git's message (Mac
RN 1.0.16).

**Recent messages.** The `≡` icon opens `Recent Commit Messages` (USHOT Tracker
#1371); ↑/↓ in the subject cycle them (VENDOR Tracker #587, 2019). Contents: the
latest 10 commits on the current branch (VENDOR Tracker #720, 2019) — earlier
described as the user's own commits (VENDOR Tracker #374, 2018); a 2022 Windows user
says other people's appear (TrackerWin #1685). Selecting one fills subject and
description (Win RN 1.38). The same menu runs the `prepare-commit-msg` hook to
draft a message (Win RN 1.70; USER Tracker #1749 names the item `Run
prepare-commit-msg hook`; Mac availability UNVERIFIED). Since 2025 an AI agent
(Claude, Codex, Cursor) can generate a message (Mac RN 2.59, 2.64, 2.70; Win RN
2.14); its control: UNVERIFIED.

**Template.** `commit.template` pre-fills the box (Mac RN 1.0.64, Win RN 1.26); the
repository settings have a `Commit Template` tab with `Use global Git configuration
file` (VSHOT Tracker #593, 2019). A generated default message was refused as an easy
route to bad messages (VENDOR Tracker #593).

**The button.** `Commit N Files` — the staged file count on the button (Mac RN
1.0.9; VSHOT carousels `Commit 35 Files`, `Commit 9 Files`); plain `Commit`,
disabled, when nothing is staged (USHOT Tracker #901). Disabled while a commit runs
(Win RN 1.29). ⌘Return / Ctrl+Enter commits (DOCS). ⌘1 / Ctrl+1 pressed twice
focuses the subject; Tab moves unstaged list → staged list → subject → description
(`fork-detail-and-diff-ui.md` Finding 18). The commit runs `git commit
--file=<temporary file>` (USER log, Tracker #2485, 2025; a 2019 capture showed `-m`,
Tracker #776).

**Commit and Push.**

- Mac: hold ⌥ and the button reads `Commit N Files and Push` (Mac RN 1.0.50; VENDOR
  Tracker #47, #77; USER Tracker #561); ⌥⌘Return triggers it (VENDOR Tracker #561,
  added 1.0.75; broken in 2.67, fixed 2.68). The vendor shortcut list says ⌘⇧Return
  — which of the two works today, or both: UNVERIFIED (users in Tracker #2634
  found ⌘⇧Return did not trigger muscle memory but did not test it).
- Windows: the button is a split button; its dropdown (`Commit Options`) has
  `Commit and Push  Ctrl+Shift+Enter` (USHOT Tracker #2227, Oct 2024; Win RN 1.24).
  Win RN 1.36 had added Ctrl+Alt+Enter (2019); the current menu and list say
  Ctrl+Shift+Enter.
- Both: a preference pushes on every commit — `Automatically push on commit` (Mac
  RN 1.0.34) / `Push automatically on commit` (Win RN 1.43; VENDOR TrackerWin
  #2691). ⌥ inverts it (VENDOR Tracker #2160). The vendor refused a separate
  button and a push checkbox (Tracker #47, #279), `Commit and Force Push`
  (TrackerWin #2173 — force push needs an extra action) and a commit-pull-push
  button (push must be manual, TrackerWin #1541).

**Amend.**

- Checking `Amend` loads the last commit's message and lists its files among the
  staged changes, so they can be unstaged out of it (Mac RN 1.0.44, Apr 2017; VENDOR
  Tracker #10; USER Tracker #2499, #1121; VENDOR TrackerWin #1581 recommends amend to
  fix the last message). It unchecks itself after the commit (USER Tracker #997).
- The button is enabled immediately in amend mode, so an amend with nothing new is
  possible; a warning is requested (open, Tracker #2499).
- Windows warns when the commit to amend is already pushed (Win RN 1.91, Nov 2023;
  its wording, per a false-positive report, is that the commit is already pushed —
  TrackerWin #2561; exact text UNVERIFIED). Mac: no release note — OPEN.
- Keyboard: ⌘⇧A toggles Amend on Mac (USER Tracker #309, #2449, 2024–2025; not in
  the vendor list, UNVERIFIED); no shortcut or Tab stop on Windows (VENDOR TrackerWin
  #333, 2019; open TrackerWin #2837, Aug 2026).
- Author date is kept on amend (USER Tracker #857); no way to set it (refused,
  TrackerWin #2707: keep the UI clean).

**Sign-off and signing.** `Add Signed-off-by line` is a per-repository setting
(Mac RN 1.0.61; VENDOR Tracker #1396; later on the settings' Commit tab, USER same
issue); a global default is requested (TrackerWin #1858). GPG signing is configured
per repository in settings (Mac RN 1.0.58; VENDOR Tracker #1396); SSH-key signing
is discussed in later issues (Tracker #1612, #1688, #2537) — its settings UI:
UNVERIFIED. Neither is a commit-box control, and the box
does not show who will author or whether it will be signed (open, Tracker #1057).

**Author override.** None: use the command line (VENDOR Tracker #2028,
<https://github.com/fork-dev/Tracker/issues/2028#issuecomment-1837180921>, 2023).

**Nothing staged.** The button stays disabled; requests to offer "stage all and
commit" are open (Tracker #901, #1560) and `commit -a` was refused — changes must be
reviewed before committing (VENDOR Tracker #2409,
<https://github.com/fork-dev/Tracker/issues/2409#issuecomment-3152449287>, Aug 2025).
Empty (`--allow-empty`) commits: refused, keep the UI clean (VENDOR TrackerWin
#2288, 2024).

**Hooks.**

- Commits run as a background activity so a slow hook no longer freezes the UI (Mac
  RN 1.0.69; VENDOR Tracker #317). While it runs, the toolbar status control shows a
  busy indicator and message, and the Activity Manager shows the output live
  (VENDOR TrackerWin #125,
  <https://github.com/fork-dev/TrackerWin/issues/125#issuecomment-457201254>, 2019;
  Mac RN 1.0.69 "Show output of pre-commit hooks interactively", VENDOR Tracker #303:
  `commit-msg` and `pre-commit` hook output and the commit output). Mac RN 1.0.81
  "Show commit hook output runtime" — whether that means live or duration:
  UNVERIFIED.
- On failure: the `Git Error` modal — warning icon, `Git Error`, `An unexpected
  error occurred while performing the git request.`, `Error Details:` with the
  command and git's output, buttons `Skip pre-commit hooks and commit` and `Close`
  (VSHOT Tracker #776, Oct 2019; the 2017 first version read `Suppress pre-commit
  hooks and commit`, VSHOT Tracker #114). `Close` is Esc; nothing is the default
  (VENDOR Tracker #2173, 2024). The skip button appears when Fork detects a
  `pre-commit` or `commit-msg` hook — in 2019 only under `.git/hooks`, not
  `core.hooksPath` (VENDOR Tracker #776); later status: UNVERIFIED (TrackerWin #771
  closed 2023 without a statement).
- `--no-verify` before committing: refused repeatedly — skip only on failure, keep
  the commit UI clean (VENDOR Tracker #114, #776, #948,
  <https://github.com/fork-dev/Tracker/issues/948#issuecomment-604000807>; #1970,
  #2568, Mar 2026, with an offer to refund). Hiding the skip button was also
  refused (Tracker #1370).
- Known edges: skip-and-commit after Commit and Push did not push (Tracker #1939,
  fixed); Amend stayed checked after skip (Mac RN 2.7, Win #1712); the modal is small
  and resets its size (Tracker #1636 open; TrackerWin #1141, #1248, closed as
  completed 7 Oct 2026); ANSI colour codes print raw (Tracker #1218, #1636; Win
  #2299); closing the modal does not refresh the file list (open, Tracker #2024);
  Mac RN 1.0.83 fixed status not refreshing when a hook fails. Text wraps in error
  dialogs (Mac RN 2.47, Win RN 2.2) and auto-scrolls to the end (Win RN 2.15).
- Partial stages that "committed the whole file" were traced to hooks re-staging
  (`fork-detail-and-diff-ui.md` Finding 23).

## 6. Stash

**Saving.**

- Toolbar `Stash` button: ⌘⇧H / Ctrl+Shift+H (DOCS "Create stash"; VENDOR Tracker
  #129 chose H). Quick Stash ⌥⇧⌘H on Mac (DOCS; VENDOR Tracker #672); Windows:
  UNVERIFIED.
- The full-stash dialog has a message field and a `Stage new files` checkbox, which
  stages untracked files so the stash includes them; there is no include-untracked
  option as such (VENDOR TrackerWin #212,
  <https://github.com/fork-dev/TrackerWin/issues/212#issuecomment-477235879>, 2019;
  USER Tracker #625). Its exact layout: UNVERIFIED (no capture found).
- Missing by refusal or neglect: keep-index (TrackerWin #444, open since 2019 — the
  vendor identified `--keep-index`), staged-only / `--staged` (TrackerWin #598 open;
  #1608 closed as duplicate 7 Oct 2026; the vendor knew no safe way, Tracker #281),
  hunk stash (Tracker #281).
- The toolbar button's dropdown lists `Recent Stashes:` and `Save Snapshot...` (Mac
  RN 1.0.99, Win RN 1.55; VSHOT Tracker #1927, 2023) — a snapshot stashes without
  removing the changes, which the vendor offers as a backup before risky work
  (VENDOR Tracker #1166).
- **Partial stash** `Stash N Files...` from the file context menu (Mac RN 1.0.92,
  Win RN 1.48; blog Win 1.48 GIF): dialog `Save stash`, text `Save your local
  modifications to a new stash. BOTH staged and unstaged changes will be stashed`,
  a checklist of changed files with the selection ticked, `stash message
  (optional)`, `Save Stash` / `Cancel`, and an in-dialog `Stashing...` spinner.
  Every staged file is stashed too, whatever is ticked — that is `git stash --
  <paths>` (VENDOR Tracker #2607, Apr 2026; #1541; repeatedly reported as a bug,
  Tracker #1783, #2500, TrackerWin #2257). Esc during the spinner closes the dialog
  and leaves the UI stale (open, TrackerWin #2792).
- Automatic stashes: `Stash and reapply` on pull, rebase, checkout, track and sync
  (Mac RN 1.0.72, 1.0.83, 2.18, 2.20); git's messages show as `Fork autostash
  <date>` and `Pull autostash <date>` (VSHOT Tracker #1927; USHOT Tracker #878).

**Where they are listed.** Inline in the commit list and in the sidebar's Stashes
section (refs-and-status Section 3); the toolbar dropdown's recent list. Searching
does not reach stashes (VENDOR Tracker #1140).

**Applying.**

- Double-click a stash applies it to the current branch, nothing more (VENDOR
  TrackerWin #304, 2019).
- The `Apply Stash` dialog (USHOT Tracker #878, Jan 2020): `Apply changes of the
  stash to your working copy`, `Stash to apply:` with the stash chip and message, a
  checkbox `Delete stash after applying` with the note `Stash will not be deleted if
  conflicts occur`, `Cancel` / `Apply`. Ticked runs `git stash pop <stash>` (VENDOR
  TrackerWin #622,
  <https://github.com/fork-dev/TrackerWin/issues/622#issuecomment-577281688>, 2020);
  unticked runs `git stash apply` (USER log `stash apply refs/stash@{0}`, Tracker
  #1928). There is no separate "Pop" verb.
- Index: new files come back staged, as git does it (VENDOR Tracker #2116, 2024);
  restoring the staged state was requested (Tracker #878) and Windows now tries to
  (Win RN 2.10, Jul 2025; flag UNVERIFIED, presumably `--index`). Mac: UNVERIFIED.
- Conflicts: the stash is kept on conflict (dialog note above); aborting a stash
  conflict keeps local changes (Mac RN 1.0.89). A pop shortcut was refused as
  error-prone (VENDOR Tracker #2092).
- Applying one file or one region of a stash: not supported (open, Tracker #1009,
  TrackerWin #758); the vendor uses `Reset file to state on commit` on the stash
  instead (VENDOR TrackerWin #758).

**Deleting, renaming, exporting.** ⌫ / Delete removes the selected branch or stash
in the commit list (DOCS; VENDOR Tracker #2390); multiple stashes at once (Mac RN
1.0.71, Win RN 1.28; `Delete x stashes`, USER TrackerWin #718). The 2017 confirmation
read `Are you sure you want to delete the stash stash@{1}?` with No / Yes; the
vendor changed such dialogs to verb buttons in 1.0.62 (USER and VENDOR Tracker
#186); the current wording: UNVERIFIED. Rename (Mac RN 2.3, Win RN 1.58); `Save as
Patch` from the stash menu (Mac RN 2.69, Win RN 2.21; Win RN 1.49 earlier). Lost
(dropped) stashes have no UI; the vendor points at `git fsck` (VENDOR TrackerWin
#2184; open TrackerWin #774).

## 7. Reflog, undo and the activity log

**Reflog.** "Restore lost commits with Reflog" is on the home-page feature list, and
the FAQ's answer to lost commits is to use reflog (DOCS `faq.md`).

- It is a mode of the commit graph: the reachable history plus the commits the
  reflogs still reach, drawn in the same graph with their lanes, dimmed (VSHOT
  Tracker #351 GIF, Jul 2018; USHOT TrackerWin #1307, 2021). The vendor's
  equivalent is `git log --all --reflog` (VENDOR TrackerWin #1307,
  <https://github.com/fork-dev/TrackerWin/issues/1307#issuecomment-918504470>).
- Toggle: View → `Show Lost Commits (Reflog)` (USER Tracker #1140, 2020; TrackerWin
  #1514, #1558; the 2018 prototype read `Show Reflog in Commit List`); ⌘⇧. /
  Ctrl+Shift+. toggles it in the commit list (Mac RN 2.61, Win RN 2.16, 2026). No
  toolbar button (refused, TrackerWin #378). Mac RN 1.0.69 / Win RN 1.19 (Jul 2018).
- Recovery is ordinary commit actions on a dimmed commit — the vendor's demo
  creates a tag (`recovered`) on one (VSHOT Tracker #351); branch, checkout and reset
  are the commit menu's (INFERRED).
- There is no list of reflog entries with their messages (`checkout: moving from…`),
  and no view of dangling objects (VENDOR Tracker #351: use `git fsck`). Commit search
  includes reflog commits since Mac 2.34 (VENDOR Tracker #1140). Hiding branches or
  tags used to hide reflog commits (fixed Mac RN 2.39 / Win RN 1.94).

**Undo.** None for git operations or for stage / discard: requests open or declined
(Tracker #981, #768, #2175; TrackerWin #657). The vendor separates undo (hard for
merge and rebase) from reflog (easy) (VENDOR Tracker #351), suggests backup branches
and snapshots (Tracker #1166), and Fork offers backup branches before an interactive
rebase (Mac RN 1.0.79). Text undo exists in the commit fields (Tracker #2027, #2037).

**Activity Manager** — Fork's operation log (Mac RN 1.0.67, May 2018; Win RN 1.16):

- Opened by clicking the toolbar's central status box (blog Mac 1.0.67; Mac RN
  1.0.84, Win RN 1.39; VENDOR Tracker #2232 "click on the repository name on the
  toolbar"). A popover: on the left the operations — name (`Fetch origin`, `Push
  develop to origin`, `Create branch 'develop'`, `Delete 'tag1'`), status (`running`,
  `succeeded`) and time, with an × to cancel a running one; on the right the
  selected operation's `$ git ...` command lines and output (VSHOT blog Mac 1.0.67 and
  Win 1.48 GIFs).
- Grew to show git commands (Mac RN 1.0.83, Win RN 1.38), highlighting (Mac RN
  1.0.68), hook output (Mac 1.0.69, Win 1.20/1.22), all dialog commands (Mac RN
  1.0.88), operation results and statuses (Mac RN 2.18, Win RN 1.68/1.75),
  sections so automatic fetches do not bury the rest (Mac RN 2.48, Win RN 2.3), and
  a verbose git output preference (Mac RN 2.62, Win RN 2.16).
- Not everything: commands are shown mostly for operations that talk to a server
  (VENDOR TrackerWin #514, 2019). Nothing persists across launches — no permanent
  log (VENDOR Tracker #2232,
  <https://github.com/fork-dev/Tracker/issues/2232#issuecomment-2541654341>, Dec
  2024). Pinning it open, colour and durations are open requests (Tracker #849,
  #1218; TrackerWin #1550, #2179); per-entry reset was suggested and the vendor
  pointed at reflog instead (TrackerWin #1810).
- No quotation of a confirmation the user accepted exists anywhere in it (INFERRED
  from every capture and note above).

## 8. Refresh, progress and errors around these operations

- **Refresh**: `git status` runs after every operation that touches the working
  tree, and on focus (refs-and-status Section 7). The vendor attributes a slow
  feeling commit to that refresh of the file list and commit list, not to the
  commit (VENDOR Tracker #2485,
  <https://github.com/fork-dev/Tracker/issues/2485#issuecomment-3436890150>, 2025).
  Missed refreshes after stage, unstage or discard have been reported and fixed
  (Tracker #1152, an old git rejecting `--renames`; TrackerWin #766, 2020; #1191,
  2021).
- **Progress**: the toolbar status control spins and names the running operation
  (VENDOR TrackerWin #125; blog Mac 1.0.79 "Improved status control"); Windows
  dialogs for stash and discard show an in-dialog spinner (USHOT blog Win 1.48;
  TrackerWin #1191, #2780). Hunk staging and diffs revert a loaded large file to its
  "too large" placeholder after each stage or discard (open, Tracker #2513;
  `fork-detail-and-diff-ui.md` Finding 21).
- **Errors**: one `Git Error` modal for every failing git request, with `Error
  Details:` (the command and output) and context buttons (hook skip; on Windows a
  Credential Manager button, Win RN 2.0). Esc closes it.

## Corrections to Cairn's records

1. `docs/research/refs-and-status/fork-refs-and-status-ui.md`, Section 6, lists
   above the lists "a collapse-all chevron". The double chevron is **Stage All /
   Unstage All**: the Mac 1.0.70 blog GIF shows it staging every file and flipping
   direction, Windows 1.18 introduced it as the Stage All button, and the vendor
   calls it intentionally hard to hit (TrackerWin #2565). Whether a separate
   collapse-all control also exists: UNVERIFIED.
2. `docs/design/ui.md`, "Staging gestures", is consistent with this record. Its
   "What changes" row assumes Fork's destructive dialogs are generic; the evidence
   refines that: Fork's discard dialog does say the loss is permanent and carries a
   count, but not what is lost line by line, nor that an untracked file is deleted
   rather than reverted, and no Fork dialog offers to stash first.

## Open questions Fork's evidence does not settle

1. The current Mac and Windows wording of the **chunk** discard dialog, the
   stash-delete confirmation, and the amend-already-pushed warning (Windows) — and
   whether Mac has that warning at all.
2. How Mac Fork discards an **untracked** file (unlink, `git clean`, trash?) — only
   Windows' `File.Delete` is evidenced.
3. Which Commit and Push chord works on Mac today: ⌥⌘Return (release note 2.68) or
   ⌘⇧Return (vendor list), or both.
4. Whether the discard dialog's default (Return) button is `Discard` or `Cancel` on
   current versions; Tracker #1080 asks for Cancel and is open.
5. The full-stash dialog's exact layout and whether it has more than a message and
   `Stage new files`; whether Mac applies a stash with `--index` as Windows 2.10 now
   tries to.
6. Whether `.gitignore` can be targeted at extensions from the menu, whether
   multiple-file ignore exists on Mac, and where the 2026 template picker lives.
7. Whether the hook-skip button now detects `core.hooksPath` hooks (Husky).
8. Whether Mac shows progress for a long multi-file discard.
9. The staged list's context menu contents, and whether ignored files (shown with
   Show Ignored Files) can be discarded or un-ignored from it.
10. How Fork stages or discards **lines of an untracked file** (intent-to-add first,
    or a whole-file patch).
11. The control for AI-generated commit messages and whether it sits in the commit
    box.

A few minutes on the owner's Fork (Mac) would settle 1, 3, 4, 5, 6, 8 and 9.
