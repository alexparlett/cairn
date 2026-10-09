# Fork's Create Branch dialog, its "Local changes", and lost commits drawn dimmed

Evidence record. Date: 2026-10-09. Commissioned by: staging-and-commit phase 10, for the user's
decisions on Show Lost Commits' recovery path (the user chose Fork's full Create Branch dialog)
and on how a lost commit is dimmed. Gathered by the packet coordinator's two research passes and
filed here by phase 10. Historical: never retro-edited. A companion to
`fork-staging-and-commit.md` (2026-10-07, section 7 on the reflog) and
`fork-merge-and-amend-evidence.md`, which it adds to and does not repeat.

## Method and sources

Documentary, as the companions': `gh` searches over `fork-dev/Tracker` (macOS) and
`fork-dev/TrackerWin` (Windows), both release-notes pages (Mac RN, Win RN), the vendor's docs
repository (`fork-dev/Docs`: `keyboard-shortcuts-mac.md`, `keyboard-shortcuts-windows.md`,
`faq.md`), and the screenshots attached to the issues named. Fork has no Linux build, so nothing
was installed or clicked. Labels as the companions': VENDOR (Fork's developers), USER, RN
(release notes), USHOT / VSHOT (a user's or the vendor's screenshot), INFERRED (reasoned, not
documented), UNVERIFIED.

## 1. Where the dialog is opened: "New Branch…" on every commit row

- USHOT Tracker #1683 (macOS): a commit row's context menu — over a row labelled
  `origin/master` — reads, in order: the ref's own submenu; `New Branch…` (⇧⌘B) and `New
  Tag…` (⇧⌘T); `Interactive Rebase`; `Reset '<branch>' to Here…`; `Checkout Commit…`,
  `Cherry-pick Commit…`, `Revert Commit…`, `Save as Patch…`; `Compare to Local Changes`; `Copy
  Commit SHA` (⌘C). `New Branch…` is a commit's action, on any commit row.
- VENDOR `keyboard-shortcuts-windows.md`: Ctrl+Shift+B "New branch" (Repository section); the
  macOS menu above shows ⇧⌘B. No chord for it is decided for Cairn (not in this record's
  decisions).
- A lost commit's row is a commit row like any other (VSHOT Tracker #351: the vendor's demo
  recovers a lost commit with the commit menu's tag action).

## 2. The dialog's anatomy

- VSHOT TrackerWin #1394 (Windows, recent): title "Create Branch"; subtitle "Use '/' as a path
  separator to create folders"; "Create branch at:" a commit glyph (a diamond), the short id and
  the subject (`0430317 initial commit`), read only; "Branch name:" a field with a branch glyph;
  "Check out after create" ticked; buttons "Create and Checkout" (primary, left) and "Cancel".
- USHOT Tracker #263 (macOS, older, `m263a`): title "Create branch"; subtitle "Creates new
  branch. All uncommitted changes will be moved to the new branch."; "Create branch at:" ◇
  `f480804 Add IDEWorkspaceChecks.plist`; "Branch name:" `testname`; "Check out after create"
  unticked; buttons "Cancel" and "Create" (primary, right). The primary button reads "Create"
  unticked and "Create and Checkout" ticked.
- USHOT TrackerWin #2472 (Windows, `w2472a`): with the box ticked and the working tree dirty, a
  "Local changes:" group of radio buttons: "Don't change", "Stash and reapply" (selected) and
  "Discard"; the primary button disabled while the name is invalid, the reason beside the
  buttons with a warning glyph ("Name cannot end with '.'").
- USHOT Tracker #1911 (macOS, `t1911a`): a name taken refused inline — "Branch test already
  exists" beside the buttons, "Create and Checkout" disabled — before git runs.
- USHOT TrackerWin #128, Tracker #2299: the same dialog in older builds; the placeholder "Enter
  branch name" (Tracker #2299).
- Button order follows the platform: the primary button first on Windows, last on macOS.

## 3. "Local changes": when it shows, and what each choice does

- RN Mac 2.47 / Win 2.2: "Local changes" options added to the checkout and create-branch dialogs
  (stash and reapply, discard, don't change).
- USHOT TrackerWin #2472, USHOT Tracker #2359 (`m2359`: the Checkout dialog, "Local changes:"
  Don't change / Stash and reapply / Discard), Tracker #2186 (`m2186`): the group appears only
  when the box is ticked and the working tree has uncommitted changes (INFERRED from every
  capture: no capture shows it over a clean tree or with the box unticked).
- VENDOR Tracker #419 (2024-03-31): "Don't change" is a plain `git checkout` — the changes are
  carried to the new branch, and git refuses where they would be overwritten.
- VENDOR Tracker #419 (2018): Discard was performed with no confirmation, and Fork stopped
  remembering it as the dialog's selection (Fork 1.0.71); "Discard" is never the remembered
  choice.
- USHOT Tracker #2234 (`m2234a`, `m2234b`): a related dialog ("Checkout and Fast-Forward") with
  "Uncommitted Changes:" Stash and reapply / Leave as stash / Discard; on git's refusal Fork
  shows its Git Error dialog with git's own words.

## 4. What is remembered

- USER TrackerWin #623, #2702; Tracker #1844, #263: "Check out after create" keeps the state the
  user last left it in (across launches, INFERRED from the reports' wording); the vendor added
  it as a preference.
- RN Win 1.82: "Remember branch name if create branch failed" — the name typed is offered again
  when the dialog is next opened after a failure.

## 5. A lost commit's row: its text dimmed, its graph in colour

- VSHOT Tracker #351 (GIF, 2018) and USHOT Tracker #351 (a user's screenshot, `t351n`): with
  Show Lost Commits on, a lost commit's subject, author, short id and date are drawn in a grey
  about half as dark as a reachable row's; its graph's lane, edges and node keep their lane
  colours, and its ref chips (a tag the vendor creates on one) are drawn as on any row. A lost
  commit that gains a ref is drawn as any other.

## What this record does not settle

- Whether Fork persists "Check out after create" in a settings file or the macOS defaults: not
  documented (the user decided Cairn's: sticky for the session now, across restarts with a
  settings store later — issue #89).
- Fork's full set of name-refusal messages: only "Name cannot end with '.'" and "Branch <name>
  already exists" are captured; Cairn says git's own reason for a name git refuses.
- The exact command Fork runs for "Discard" (`checkout -f`, or a reset first): UNVERIFIED.
