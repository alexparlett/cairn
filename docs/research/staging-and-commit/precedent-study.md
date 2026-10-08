# Precedent study: discard safety, auto-stash, reflog, operation log, amend, stash drop, line staging and the commit box

Date: 2026-10-07
Commissioned by: staging-and-commit planning (packet `staging-and-commit`, open
question O3 in `docs/work/daily-loop/roadmap.md` and `docs/design/cairn.md`,
"Still open"; background in `docs/design/feature-inventory.md`, "Recovery: two
classes, two stories").

## Method and sources

This is an evidence record. It decides nothing. It covers how other git clients
handle the surfaces this packet ships: the first destructive operations (discard
by file, hunk and line; clean untracked; amend; stash drop), the reflog view and
a visible operation log. It answers one question in particular: what, if
anything, backs up a discarded uncommitted edit.

The research ran as four parallel passes over primary sources:

- **Open-source clients, read from their source.** Shallow clones at default-branch
  HEAD, fetched 2026-10-07, were grepped and read directly:
  - `desktop/desktop` @ `c2d018e` (branch `development`)
  - `jesseduffield/lazygit` @ `c5f7158`
  - `magit/magit` @ `bd9bce5`
  - `gitui-org/gitui` @ `d7214ec`
  - `microsoft/vscode` @ `91b51fe` (`extensions/git` only)
  - `gitx/gitx` @ `cd87cb3`
  - JetBrains `intellij-community` `master`
  - `git/git` `master`, checked at `v2.30.0` where the floor matters
  - jj (`jj-vcs/jj`), git-branchless (`arxanas/git-branchless`) and GitButler
    (`gitbutler/gitbutler`), from their raw source and docs

  Paths below are relative to each repository's root, so a citation such as
  `app/src/lib/stores/git-store.ts` means
  `github.com/desktop/desktop/blob/development/app/src/lib/stores/git-store.ts`.
  Line numbers are as of those commits. They will rot, so the function name is
  given beside each one.
- **Closed-source clients, from what their vendors publish.** Fork, Tower, GitKraken,
  Sublime Merge and Sourcetree come from vendor docs, release notes and the
  vendors' public issue trackers:
  - Fork: `fork-dev/Tracker` and `fork-dev/TrackerWin`
  - Sublime Merge: `sublimehq/sublime_merge`
  - Sourcetree: `jira.atlassian.com` SRCTREE

  Forum and community posts are marked as such.
- **What is marked UNVERIFIED.** Anything not confirmed from a primary source:
  a search-result summary whose page would not load, a user's forum post, or
  absence of evidence.

No client was run. Every behaviour described here is what the source or the
vendor says, not something observed.

## Headlines

- **Only one out-of-app backup of a discard ships anywhere: the OS Trash, holding
  whole files.**
  - GitHub Desktop trashes every new or modified file before restoring it.
  - VS Code (since 1.98) and Sourcetree for Mac (since 2.3) trash untracked files only.
  - None of them backs up a hunk or line discard, and GitHub Desktop's flow drops the
    staged version of a file.
- **Two closed-source clients undo a discard, but neither documents how.**
  - Tower lists "discarding a chunk or file" among its undoable actions, while its own
    working-copy help page says a discard cannot be undone.
  - GitKraken undoes "Discard", but only the single most recent action.
- **Fork, Sublime Merge, lazygit and gitui keep nothing.** They rely on
  confirmation alone.
  - Fork's maintainer has declined a Trash or undo for discard more than once.
  - Fork keeps hunk-discard confirmation deliberately hard to skip.
- **The snapshot-before-destroy model exists, but only outside the
  conventional GUIs.**
  - magit's `magit-wip-mode`, git-branchless, GitButler's oplog and jj's
    working-copy commit each write a commit of the working tree into the object
    database before the destructive step.
  - Each keeps it alive with a private ref namespace (`refs/wip/`, `refs/branchless/`,
    `refs/jj/keep/`, GitButler's hand-written reflog).
  - All but GitButler leave untracked or large files out, or cap them.
- **git's own autostash is `git stash create` plus, on failure, `git stash store`.**
  - It does not include untracked files.
  - `stash create` refreshes and writes `.git/index` and hashes every modified file.
    Under Cairn's read/write split that makes it a write.
  - `refs/stash`'s reflog never expires unless configured, which a private ref does not
    get for free.
- **Reflog views split three ways:**
  - Fork draws lost commits inside the graph behind a toggle.
  - Tower and magit show a dedicated list, defaulting to HEAD with a per-ref switch.
  - lazygit has a HEAD-only tab with the fullest action set.
- **Visible command logs.**
  - magit's is the most complete: argv, exit code, optional timestamp and output, kept
    to 32 sections.
  - jj's `op log` is the only one that shows an operation's command and lets you restore
    the state before it.
  - None of the GUIs is confirmed to persist its log across sessions.
- **Amend warnings for pushed commits are rare.** Where they exist they are
  computed from refs:
  - GitHub Desktop: `upstream..branch`, or `--not --remotes`.
  - magit: `merge-base --is-ancestor` against `magit-published-branches`.
  - JetBrains: a containing-branches index filtered by protected-branch patterns.
  - Fork for Windows 1.91: mechanism unknown.

---

## 1. Discard safety and backup

### GitHub Desktop: every new or modified file goes to the OS Trash before it is restored

**The sequence.** `GitStore.discardChanges(files, moveToTrash = true, …)`
(`app/src/lib/stores/git-store.ts`, ~L1545–1649) does three things, in order:

1. For every file that is neither `Deleted` nor a submodule, it calls
   `this.shell.moveItemToTrash(<absolute path>)`.
2. It runs `resetPaths(Mixed, 'HEAD', …)` on the paths with index changes, which is
   `git reset HEAD -- <p>`.
3. It runs `checkoutIndex(paths)`, which is `git checkout-index -f -u -- <p>`. A rename
   or copy also checks out its `oldPath`.

**What it saves, and what it loses.**
- **What reaches the Trash:** the whole working-tree file, as it is, not a patch.
  `docs/technical/discard-changes.md` says Desktop moves "all new or modified files" out
  of the repository "as-is".
- **No wrapper folder or timestamp.** Each file goes to the Trash under its own name.
  This is inferred: the bare path is passed to `trashItem`.
- **Not saved:**
  - Content staged in the index that differs from the working tree. The mixed reset
    throws it away and only the working-tree copy is trashed (inferred from the code
    order).
  - Deleted files. They are simply checked out again.
  - Submodules.

**Restoring.** Only by hand, from the Trash. The dialog says changes "can be restored
by retrieving them from the Trash" (`app/src/ui/discard-changes/discard-changes-dialog.tsx`;
label in `app/src/ui/lib/context-menu.ts`). There is no undo inside the app.

**Size limit.** None on this path. A file of any size is trashed whole.

**Line and hunk discard back up nothing.**
`discardChangesFromSelection` (`app/src/lib/git/apply.ts`, ~L102–120) builds a reverse
patch and pipes it to `git apply --unidiff-zero --whitespace=nowarn -` with no Trash
step. The selection dialog (`discard-selection-dialog.tsx`) does not mention the Trash.

**Origin.** Trashing has been there from the start: PR #346, "Discard changed file"
(merged 2016-09-12, fixes #318), already called `shell.moveItemToTrash` before checkout.

**Confirmations.**
- `confirmDiscardChanges` defaults to true; there is also `confirmDiscardChangesPermanently`
  (`app/src/lib/stores/app-store.ts`, ~L490–506).
- One file: no prompt when the setting is off (`app/src/ui/changes/sidebar.tsx`).
- Several files, or Discard All: always confirms, even with the setting off
  (`showDiscardChangesSetting: false`). Past `MaxFilesToList = 10` it shows a count
  instead of a list.
- A line selection: confirms only if the setting is on (`app/src/ui/changes/changes.tsx`).

**When the Trash fails.** This flow carries most of the history.
- **The call itself.** It runs in Electron's main process: `ipcMain.handle('move-to-trash',
  … shell.trashItem(path))` in `app/src/main-process/main.ts`. It moved there in PR #13436
  (Dec 2021, electron#29598).
- **Silent loss before Electron 16.** `trashItem` failed silently, so files were
  "unknowingly unrecoverable" (PR #13904's own description).
- **The 2.9.7 hang.** Desktop 2.9.7 hung (#13888). PR #13899 surfaced the error, and
  PR #13904 (2022-02-24) added `DiscardChangesRetryDialog`, titled "Discarded changes will
  be unrecoverable".
- **The opt-out.** That dialog has its own "do not show again" box. Once ticked, a Trash
  failure discards permanently without asking (`git-store.ts`, ~L1563–1575).
- **Untracked files left behind.** PR #20456 (2025-05-14, fixes #20428) fixed a case
  where, after a Trash failure, untracked files were not deleted at all, because
  reset/checkout acts only on index paths. They are now removed explicitly.
- **Still open.** #10445 asks for an option to skip the Trash (its PR #17436 closed
  unmerged in 2024-12). #7155 says discard is slow.

**Linux.** Electron's `MoveItemToTrash` (`shell/common/platform_util_linux.cc` in
electron/electron) shells out to `kioclient5`/`kioclient move … trash:/` on KDE,
otherwise to `gio trash`, or to whatever the `ELECTRON_TRASH` variable names. Without
those tools the Trash step fails into the retry flow. Exact failure modes (a removable
mount, a Trash on another filesystem) are UNVERIFIED.

The official Desktop does not ship for Linux; the community fork `shiftkey/desktop`
does. That is an observation, UNVERIFIED from the repositories.

### VS Code (built-in git): untracked files to the Trash since 1.98, tracked discards permanent

**Tracked discards are permanent.**
- `Repository.clean` runs `checkout('', paths)` (`extensions/git/src/repository.ts`,
  ~L1569–1619).
- The prompt comes from `_cleanTrackedChanges` (`extensions/git/src/commands.ts`). For
  one file it asks to "discard changes in '{0}'". For several it adds "This is
  IRREVERSIBLE!" and "Your current working set will be FOREVER LOST if you proceed."

**Untracked files go to the Trash.**
- `Repository._clean` (~L1645–1677) trashes them when `git.discardUntrackedChangesToTrash`
  is on (default true), unless the session is remote or a Linux snap (`isLinuxSnap`,
  `util.ts`: both `SNAP` and `SNAP_REVISION` set).
- It trashes one file first as a probe, through `workspace.fs.delete(uri, { useTrash: true })`,
  then the rest, five at a time.
- If the Trash fails, a modal offers permanent deletion.
- The dialog (`getDiscardUntrackedChangesDialogDetails`) says "You can restore this file
  from the Trash".
- History:
  - Issue #240523 (Feb 2025) motivated it: users kept deleting their files despite the modal.
  - Shipped through test plan #241476 and PRs #241953, #242186 and #242188 (the snap
    exclusion).
  - Release note: <https://code.visualstudio.com/updates/v1_98>, "Discard untracked
    changes improvements".

**Still open.**
- #241884: there is no Undo, and Ctrl+Z undoes the file's creation instead. PR #310059
  proposes an in-memory snapshot capped at 5 MB a file and 50 MB in total, with an Undo
  notification.
- #246259 / PR #305686: `files.enableTrash=false` is ignored.

**Discard All.**
- `_cleanAll` (~L2257–2291) offers "Discard N Tracked Files" or "Discard All N Files"
  under the IRREVERSIBLE text.
- No stash option exists anywhere in `clean` or `cleanAll`.
- Separately, when a checkout fails, VS Code offers "Stash & Checkout", "Migrate Changes"
  or "Force Checkout" (`commands.ts`).

**Line and range discard.**
- `git.revertSelectedRanges` does not confirm. It rebuilds the text from HEAD
  (`applyLineChanges`), edits the editor buffer and saves (`commands.ts`, ~L1956–2026).
- That makes it reversible through the editor's undo stack (inferred).
- VS Code's Local History (since 1.66) records each editor save, by default up to
  256 KB a file and 50 entries (<https://code.visualstudio.com/updates/v1_66>). A
  `git checkout` of a file is not an editor save, so it is not captured (inferred from
  "every time you save an editor").

**The founding incident: issue #32405.**
- In #32405 (2017-08-13), a new user pressed Discard All on about 5,000 untracked files
  in a freshly initialised repository, and they were deleted permanently.
- The maintainer linked #32459 ("Semantics of 'Discard All Changes' … considered harmful").
- That issue was fixed by commit `bbe70bc` (2017-08-15, shipped in 1.16), which added:
  - `git.discardAllScope`
  - a separate "Are you sure you want to DELETE…" for untracked files
  - the "This will DELETE {0} untracked files!" text
- Today's split dialog and its IRREVERSIBLE / FOREVER LOST wording descend from that
  change. The Trash came eight years later.

### JetBrains IDEs: Local History is the safety net, and Rollback is recorded in it

**Local History** (<https://www.jetbrains.com/help/idea/local-history.html>).
- **What it records:** every change, independent of VCS.
- **Where:** binary files under `LocalHistory` in the IDE's system directory
  (`~/.cache/JetBrains/<product><version>` on Linux).
- **How long:** the "last 5 working days". Configurable through Advanced Settings or
  `-DlocalHistory.daysToKeep`; 0 disables it.
- **Caveats the help page states:** it is cleared when a new IDE version is installed,
  and revisions are "not guaranteed to persist".

**What it does not cover**, from source:
- **Binary files.** `IdeaGateway.areContentChangesVersioned` returns `!isBinary()`
  (`platform/lvcs-impl/src/com/intellij/history/integration/IdeaGateway.java`).
- **Some files entirely.** `isVersioned` excludes ignored and excluded files,
  `workspace.xml`, `*.class` and files outside the project.
- **Files over 1 MB (inferred, not documented).** Content is held by reference to the
  VFS content store, which caches only up to `idea.vfs.max-file-length-to-cache`,
  default 1 MB (`PersistentFSConstants.java`).

**Rollback** (Ctrl+Alt+Z, <https://www.jetbrains.com/help/idea/undo-changes.html>).
- It confirms with a dialog that lists the files and has a "Delete local copies of added
  files" checkbox.
- In source (`platform/vcs-impl/.../changes/ui/RollbackWorker.java`), it opens
  `LocalHistory.startAction(name, VcsActivity.Rollback)` and puts a system label after
  the refresh. A rollback therefore appears as a labelled activity in Local History and
  can be reverted from there.
- Added files are deleted with `FileUtil.delete`. Whether their content survives in Local
  History depends on the rules above: UNVERIFIED.

**Shelve versus stash**
(<https://www.jetbrains.com/help/idea/shelving-and-unshelving-changes.html>).
- A shelf is a patch file the IDE generates, kept under the project directory.
- Deleted shelves go to a "Recently Deleted" node, whose retention is undocumented.
- **Smart Checkout** shelves (or stashes), checks out, then unshelves
  (<https://www.jetbrains.com/help/idea/manage-branches.html>).

### Fork: confirmation only

**Release notes** (<https://fork.dev/releasenotes>, <https://fork.dev/releasenoteswin>,
both read) have no Trash, backup or undo for a discard. The entries that matter:
- Mac 1.0.37: "Renamed 'reset' to 'discard changes' to avoid confusion."
- 1.0.71: the Checkout dialog stopped remembering "Discard", after fork-dev/Tracker#419,
  where a user lost work.
- Mac 2.29 / Win 1.84: "Do not discard local changes when aborting merge."
- 1.0.79: "Backup interactive rebase using branches". This is Fork's only backup
  mechanism.

**Maintainer positions** (DanPristupov):
- Tracker#419 (2018): floated saving the staged area as a patch to the Trash. It never
  shipped.
- TrackerWin#657, "Undo discard files" (open since 2020): his reply is that git discards
  *changes* while the file stays on disk, and that a user can stash first.
- Tracker#2264 (2024), a request to skip hunk-discard confirmation: refused, because
  discard "should not be easily accessible".
- Tracker#2232: "Fork doesn't have a permanent log."
- Still open: Tracker#1265, "Option To Place Discarded Changes In Native Trash Bin"
  (citing GitHub Desktop), and Tracker#981, "[Feature] Undo". #2175 (undo discard) was
  closed as a duplicate of #1265.

### Tower: Undo is said to cover discard, but how is undocumented

**What can be undone.** Tower's feature page (<https://www.git-tower.com/features/undo>)
lists, under ⌘Z / Ctrl+Z (⇧⌘Z to redo):
- deleting a file
- staging and unstaging
- **discarding a chunk or file**
- a commit
- branch create, checkout and delete
- merge, rebase and interactive rebase
- publishing a branch
- **saving, deleting and applying a stash**

**Version history.**
- Undo arrived in Tower 4.0 (<https://www.git-tower.com/help/guides/faq-and-tips/undoing-things/mac>).
- The release notes (<https://www.git-tower.com/release-notes/mac>) add "undo the 'Reset
  to Revision' action".
- They also fix "Undo could become unavailable … until the window has been closed and
  re-opened", which suggests a per-window stack in memory. That reading is an
  inference: UNVERIFIED.

**The contradiction.** Tower's working-copy help page
(<https://www.git-tower.com/help/mac/working-copy/undo-changes>) says discarding
uncommitted changes "cannot be undone". It contradicts the feature page.

**How it works is unclear.** Tower's engineering post
(<https://www.git-tower.com/blog/how-we-built-undo>) says snapshots "capture the state
of the working tree along each step". The vendor's AI-docs page
(<https://www.git-tower.com/docs/ai/git-features/>) says undo uses the reflog and refs.
Where discarded content is kept, how many steps, and whether it survives quitting:
UNVERIFIED.

**Snapshots, a separate feature.** The release notes describe them as stashes that are
automatically re-applied, so changes are stored but stay in the tree. A later fix lets
them include untracked files.

### GitKraken: undo for the most recent action only

- **Undoable** (<https://help.gitkraken.com/gitkraken-desktop/undo-and-redo/>): Checkout,
  Commit, **Discard**, Delete branch, Remove remote, Reset branch to a commit, and the
  rebase family.
- **Limit:** "the most recent supported action only". Redo works only immediately after
  an undo.
- Stash is not on the list.
- **Unknown:** the mechanism for undoing a discard, and whether undo survives a restart.
  UNVERIFIED.

### Sublime Merge: two-click buttons, no backup

- **Docs.** The official docs (sublimemerge.com/docs) say nothing about confirmation or
  backup on discard.
- **Changelog.** <https://www.sublimemerge.com/download> lists only functional entries,
  such as "Added Stage All and Discard All buttons" and "Fixed backspace not discarding
  selected lines".
- **Confirmation (community sources).** Discard buttons, "Discard lines" included, need a
  second click (forum thread
  <https://forum.sublimetext.com/t/disable-destructive-operations/54444>). A keyboard
  discard asks through a dialog (sublimehq/sublime_merge#1490, a user's description).
- **Undo.** Repository > Undo is reflog-based and does not cover the working directory
  (forum, a user, not staff: <https://forum.sublimetext.com/t/robust-undo-redo-stack-or-clarification-of-what-can-be-undone/39470>).
  UNVERIFIED by staff.
- **Open requests:** #153 (move to the Trash instead of deleting), #1383 ("Protect
  discard button even more!", proposing a recycle bin of discarded changes) and #213 (a
  hunk discard deleted a file through a permission error).

### Sourcetree

- **Mac.** SRCTREE-1791, "Remove unversioned files to trash"
  (<https://jira.atlassian.com/browse/SRCTREE-1791>), is marked Fixed in 2.3 (2016). That
  is the ticket's resolution; no release note confirming it was found.
- **Windows.** A community post says removing untracked files deletes them permanently
  (community.atlassian.com qaq-p/86126; search snippet only). UNVERIFIED.
- **The Discard dialog** has a "Reset All" tab, and new files are removed separately
  (SRCTREE-5164).
- **Backup of tracked changes on discard or reset:** none found. UNVERIFIED.

### lazygit: confirmation only, and its undo excludes the working tree

**Undo.** `docs/Undoing.md` describes undo (`z`) and redo (`Z`) as reflog-driven.
Excluded, in its own words: "changes to your working tree or stash", pushes, branch
creation, and anything done mid-rebase.

**Confirmations** (`pkg/i18n/english.go`; `pkg/gui/controllers/workspace_reset_controller.go`;
`files_controller.go`):

| Action | Confirmation |
| --- | --- |
| Nuke working tree (`ResetAndClean`) | `NukeTreeConfirmation`, which says the discard of everything, untracked included, "is not undoable" |
| Hard reset | Confirms only when the tree is dirty (`ConfirmIf`) |
| Discard a file | The menu choice is the only confirmation |
| Discard a line | `DiscardChangePrompt`, unless `gui.skipDiscardChangeWarning` is set |

**An orphaned stash commit.** "Discard staged changes" runs
`SaveStagedChanges("[lazygit] tmp stash")` and then `DropNewest()`. The content lingers
as an unreachable commit that nothing surfaces.

### gitui: confirmation only

- **Confirmations** (`src/strings.rs`): "confirm file reset?", "are you sure you want to
  discard {lines} selected lines?" and "confirm reset hunk?".
- **The discard itself** is libgit2's `checkout_index` with `force()` and
  `remove_untracked(true)` (`asyncgit/src/sync/reset.rs`, `reset_workdir`).
- **Backup:** none in the source.

### magit: confirmation by default, and wip refs when the mode is on

**Discard and its confirmations.**
- `magit-discard` (`lisp/magit-apply.el`) dispatches to region, hunk, hunks, file or files.
- `magit-no-confirm` defaults to `'(set-and-push)` (`lisp/magit-base.el`). So `discard`,
  `delete`, `trash` and `reverse` all confirm by default.
- The symbol `safe-with-wip` expands to `discard reverse stage-all-changes
  unstage-all-changes` only while `magit-wip-mode` is on. In other words, a user can drop
  the confirmation exactly when a backup exists.

**Untracked files go to the Trash.** `magit-delete-by-moving-to-trash` defaults to `t`.
Its docstring warns that the wip modes "do not track any files that are not tracked".

**`magit-wip-mode`** (`lisp/magit-wip.el`) hooks these points:
- `after-save-hook`
- `before-save-hook` (an initial backup before a file's first save)
- `magit-before-change-functions`, which `magit-discard-files` runs before acting, and
  hunk applies through `magit-apply-patch` unless `magit-inhibit-refresh`
- `magit-after-apply-functions`
- post-commit

**What a wip commit is made of.**
- **Where it is stored.** Refs `refs/wip/index/<branchref>` and `refs/wip/wtree/<branchref>`
  (`HEAD` when detached; the namespace is `magit-wip-namespace`).
- **The index tree** is `git write-tree`.
- **The worktree tree** is built under `magit-with-temp-index`: `read-tree` the parent,
  then `git update-index --add --remove --ignore-skip-worktree-entries -- <files>` (or
  `git add -u .`), then `write-tree`.
- **The commit** is `git commit-tree --no-gpg-sign -p <parent>`, stored by
  `git update-ref --create-reflog -m <msg>`, so every snapshot is a reflog entry.
- **Only on change.** It writes only when `diff-tree --quiet` sees a change.
- **On a locked index** it skips with "Index locked; no worktree wip commit created".
- **Cleanup.** `magit-wip-purge` deletes wip refs whose branch is gone.

**Gaps, read from the source but not tested.**
- `magit-discard-untracked` runs no before-change hook.
- For a file with both staged and unstaged changes, `magit-discard-apply` binds
  `magit-inhibit-refresh t`, which appears to skip the snapshot.
- Whether an explicitly named untracked path is captured by `update-index --add`:
  UNVERIFIED.

**`magit-reverse`** applies the reverse patch to the working tree. It refuses when
there are untracked or unstaged changes.

### Jujutsu (jj): the working copy is a commit, so a discard is always recoverable

**Every command snapshots first.** "Most `jj` commands you run will commit the
working-copy changes if they have changed" (`docs/working-copy.md`). The snapshot is
taken as the command starts, so `jj restore` or `jj abandon` first snapshots the edits
it is about to lose. Every operation records a "view" that includes each workspace's
working-copy commit (`docs/operation-log.md`), and `jj undo` and `jj op restore` go
back to one.

**Limits** (`docs/config.md`):
- New files are auto-tracked (`snapshot.auto-track`). Ignored files never are.
- `snapshot.max-new-file-size` defaults to **1 MiB**. A larger new file is refused;
  tracked files are exempt, and 0 turns the limit off.

**Keeping snapshots alive.**
- jj protects its objects from `git gc` with refs under `refs/jj/keep/`
  (`lib/src/git_backend.rs`, `NO_GC_REF_NAMESPACE`).
- `jj util gc` prunes only after `jj op abandon`, and by default only objects older than
  two weeks (`cli/src/commands/util/gc.rs`).

**The gap.** An edit overwritten by another tool between two jj commands is lost, unless
the watchman trigger `fsmonitor.watchman.register-snapshot-trigger` is on.

### git-branchless

- **What `git undo` covers** (wiki, "Command: git undo"): commits, amends, merges,
  rebases, checkouts and branch moves and deletes.
  - The working copy only "as long as they were captured in a snapshot" (v0.4.0+).
  - Untracked files never: "Changes to untracked files cannot be undone."
- **Snapshots** (`git-branchless-lib/src/git/snapshot.rs`) follow jj's design. A base
  commit holds HEAD and per-stage commits, with the metadata in trailers.
  - Untracked files are excluded because they "might contain sensitive data … or might be
    very large".
  - Snapshots are taken in `check_out_commit`, gated by `branchless.undo.createSnapshots`
    (default true, at a stated latency cost).
- **Storage.** The event log is SQLite at `.git/branchless/db.sqlite3`. Snapshot commits
  are kept alive by `refs/branchless/<oid>` (`core/gc.rs`).

### GitButler: an oplog of snapshots

- **When it snapshots.** "Before GitButler does any major action"
  (<https://docs.gitbutler.com/features/timeline>), including uncommitted work and
  recently changed files.
- **What a snapshot is** (`crates/gitbutler-oplog/src/oplog.rs`): a commit whose tree
  holds `worktree/`, `index/`, `conflicts/` and branch metadata. The oplog head is
  recorded in `.git/gitbutler/operations-log.toml`.
- **How it survives gc** (`reflog.rs`, `set_reference_to_oplog`). GitButler hand-writes
  `logs/refs/heads/gitbutler/target`, so a reflog entry points at the oplog head. That
  keeps it out of `git log --all` while gc still treats it as reachable.
- **A size limit, switched off.** The docstrings mention one for untracked files
  (`SNAPSHOT_FILE_LIMIT_BYTES`), but the code has `AUTO_TRACK_LIMIT_BYTES = 0`, "Inactive
  for now".

### What none of them covers

- **Hunk and line discard.** No conventional GUI backs them up. GitHub Desktop, Fork,
  Sublime Merge, lazygit and gitui keep nothing.
- **Index content that differs from the working tree.** Lost by every OS-Trash design.
- **Untracked and large files.** Excluded or capped by every snapshot design:
  - magit: tracked files only
  - git-branchless: no untracked files
  - jj: new files over 1 MiB refused
- **The OS Trash itself is fragile.** It fails, sometimes silently, when the Trash is
  disabled, in a remote session or a snap, or on Linux without `gio` or `kioclient`
  (Desktop PR #13904; VS Code `_clean`).

---

## 2. Auto-stash before destructive operations

### git's own autostash: `rebase.autoStash`, `merge.autoStash`, `pull --autostash`

**Mechanism** (`sequencer.c`).
- `create_autostash_internal` runs the following steps:
  1. Locks and refreshes the index.
  2. If there are changes, runs the child `git stash create autostash`.
  3. Writes the resulting oid to a file (rebase: `.git/rebase-merge/autostash` or
     `rebase-apply/autostash`, `builtin/rebase.c`) or to a ref (merge: `MERGE_AUTOSTASH`,
     `create_autostash_ref` in `builtin/merge.c`).
  4. Hard-resets the working tree (`reset_working_tree` with `RESET_WORKING_TREE_HARD`;
     the flags omit the post-checkout hook).
- Afterwards, `apply_save_autostash_oid` runs `git stash apply <oid>`. If that conflicts
  or fails, it runs `git stash store -m autostash -q <oid>`, so the stash appears in the
  stash list, and prints that the changes are stashed but applying them conflicted.
- `save_autostash` (for example on `rebase --quit`) stores without applying.

**Settings.** `pull.autostash` is new in git 2.51 (`Documentation/RelNotes/2.51.0.adoc`)
and overrides the other two. It is above Cairn's 2.30 floor.

**Untracked files are not included.** `builtin/stash.c` `create_stash` calls
`do_create_stash(…, 0 /* include_untracked */, …)`. The same holds at `v2.30.0`.

### `git stash create` and `git stash store` as a backup

**What the two commands do** (`Documentation/git-stash.adoc`).
- `create` makes "a regular commit object and return[s] its object name, without storing
  it anywhere in the ref namespace". The working tree and refs are untouched.
- `store` puts that commit on `refs/stash`, updating its reflog (`do_store_stash`:
  `refs_update_ref(… "refs/stash" …, REF_FORCE_CREATE_REFLOG)`).
- The commit's layout: W (the working tree), with parents HEAD and I (the index), plus an
  optional third parent U for untracked files.

**Capturing untracked files by hand.** `stash push -u` does it in `save_untracked_files`:
- feeds the paths to `update-index --add --remove --stdin` with `GIT_INDEX_FILE` pointing
  at a temporary index
- then `write_index_as_tree` and `commit_tree`

A client can reproduce this with plumbing (a temporary `GIT_INDEX_FILE`, `update-index
--add`, `write-tree`, `commit-tree`), which is exactly magit's wip recipe (section 1).

**Newer than the floor.** `git stash export --to-ref` (2.51) turns stashes into an
ordinary commit chain.

**Costs**, read from `builtin/stash.c`:

- **It writes the index.** `do_create_stash` starts with `repo_refresh_and_write_index`,
  which stats every tracked file and rewrites `.git/index`. By Cairn's rules (the
  `READ_ONLY` profile exists to stop exactly this) it is a write, not a read.
- **It hashes every modified file.** `stash_working_tree` builds a temporary index from
  HEAD, runs `run_diff_index`, and pipes the changed paths to `update-index --add`. Each
  modified file is hashed into a new blob, and its **clean filter runs** (git-lfs,
  git-crypt).
- **The cost is in proportion to the dirty tree.** Stat-ing every tracked file is the
  floor; then come the bytes of every changed file. A large binary change becomes a large
  loose object.
- **Hooks.** The `stash create` path runs none.
  - `stash push --keep-index` spawns `git checkout --no-overlay … -- :/`, which runs
    `post_checkout_hook` (`builtin/checkout.c` `checkout_paths`).
  - `push` also spawns `reset --hard` and `clean`.
- **It refuses on an unborn HEAD:** "You do not have the initial commit yet". A
  repository with no commit cannot be backed up this way.

**Reachability and expiry.**
- **What gc keeps.** `git gc` keeps objects reachable from refs, the index, reflogs "and
  anything else in the refs/* namespace" (`Documentation/git-gc.adoc`). Unreachable
  objects are pruned after `gc.pruneExpire` (default 2 weeks).
- **Reflog expiry.** Entries expire after `gc.reflogExpire` (90 days) or
  `gc.reflogExpireUnreachable` (30 days); both can be set per pattern
  (`Documentation/config/gc.adoc`).
- **`refs/stash` never expires unless configured.** `reflog_expire_options_set_refname`
  says "If unconfigured, make stash never expire" (`reflog.c`; at `v2.30.0` in
  `builtin/reflog.c`).
- **A private ref gets no reflog for free.** `core.logAllRefUpdates=true` auto-creates
  reflogs only for `refs/heads`, `refs/remotes`, `refs/notes` and `HEAD`. A private ref
  such as `refs/<client>/backup` needs `update-ref --create-reflog`, as magit uses, and
  then expires on the 90/30-day rules.
- **Visibility (UNVERIFIED, inferred from git's ref semantics).** A private ref shows in
  `git log --all` and is pushed by `push --mirror`.

### Clients that stash around an operation

None of these stash before a *discard*. All of them stash so an operation can proceed
over a dirty tree:

- **Tower** (release notes): an auto-stash dialog when switching branches, merging and on
  every interactive-rebase action, with "Stash Changes" the default button. It also has
  "Snapshots" (section 1).
- **Fork** (<https://fork.dev/releasenotes>):
  - "Stash and re-apply local changes on pull", later turned from a prompt into a
    checkbox
  - "Option to stash and reapply on rebase"
  - a "Stash and reapply" option in the track-remote-branch window
  - "Ability to save snapshot to a stash"
- **VS Code.** `git.autoStash` covers pull only (`package.nls.json`).
  `repository.ts` `maybeAutoStash` creates its own stash with untracked files included
  (`createStash(undefined, true)`) and pops it only for git older than 2.27. Otherwise it
  passes `--autostash` to the pull.
- **JetBrains:** Smart Checkout (shelve or stash, check out, unshelve).
- **GitKraken:** auto-stashes on branch switch, untracked files included. Sourced only
  from user posts on its feedback board (suggestions 198442 and 198443, which returned
  403 to fetch). UNVERIFIED.

**Private backup-ref namespaces in use:**
- magit: `refs/wip/{index,wtree}/…`, with a reflog
- jj: `refs/jj/keep/*`
- git-branchless: `refs/branchless/<oid>`
- GitButler: the hand-written reflog on `refs/heads/gitbutler/target`
- `refs/original/` from `filter-branch`: not researched

---

## 3. Reflog views

| Client | Does it have one, and where | What it shows | Actions from an entry |
| --- | --- | --- | --- |
| **Fork** | No panel. **View > Show Lost Commits (reflog)** draws them inside the commit graph. Added in 1.0.69 ("Ability to show lost commits (reflog)", <https://fork.dev/releasenotes>). | The maintainer's reasoning (fork-dev/Tracker#351): reflog and undo are different features, the graph is the place, and after a rebase the toggle "will show the previous state(s)". No UI for `git fsck` dangling objects (same thread). 2.34: context search includes reflog commits (#1140). 2.39 fix: lost-commit mode broken under a filter. #1528: hiding a remote hid lost commits. | They are graph rows, so presumably the normal commit menu: UNVERIFIED. |
| **Tower** | A dedicated Reflog view, hidden by default: enable "Show reflog in sidebar", or ⌘5 / Ctrl+5 (<https://www.git-tower.com/help/mac/commit-history/reflog>). | HEAD's reflog by default. Clicking the "HEAD" label switches to one branch's reflog. Columns undocumented: UNVERIFIED. | Create New Branch from <hash>. Drag onto the HEAD branch to cherry-pick. Drag part of a changeset onto Working Copy to apply it. |
| **magit** | `l r` `magit-reflog-current` (the current branch, or HEAD when detached), `l O` `magit-reflog-other` (any ref), `l H` `magit-reflog-head`, `l R` `magit-log-reflog` (an ordinary log of everything any reflog names) (<https://docs.magit.vc/magit/Logging.html>; `lisp/magit-reflog.el`, `lisp/magit-log.el`). | Read with `git reflog show --format=%h%x00%aN%x00%gd%x00%gs --date=raw <ref>`. Each row: short hash, entry index, a coloured action label (commit, amend, merge, checkout, reset, rebase, cherry-pick… from `magit-reflog-labels`) and the message. Date and author in the margin. | The keymap inherits `magit-log-mode-map`, so checkout, reset, cherry-pick and branch-at-point all work. |
| **lazygit** | A Reflog tab in the commits panel. | HEAD only: `git log -g --format=+%H%x00%ct%x00%gs%x00%P` (`pkg/commands/git_commands/reflog_commit_loader.go`). Note: `%ct` is the commit's committer time, **not** the time of the reflog entry. Columns: short hash, date (in full-description mode), `%gs` (`pkg/gui/presentation/reflog_commits.go`). | `space` checkout (detached), `n` new branch, `N` move commits to a new branch, `w` worktree, `g` reset soft, mixed or hard, `C` copy for cherry-pick, `y` copy hash or another attribute, `o` open in browser, `enter` view its commits, `/` filter (`docs/keybindings/Keybindings_en.md`). |
| **Sublime Merge** | None. A 2018 forum request has no staff reply; users run `git reflog` as a custom command. | | |
| **GitKraken** | None. "Reflog support" (feedback.gitkraken.com/suggestions/190836) is a request; snippet only, since the page returned 403. | | |
| **Sourcetree, GitHub Desktop, gitui, VS Code, JetBrains** | None found. Absence of evidence: UNVERIFIED. VS Code: no reflog command in `extensions/git/src/commands.ts`. | | |

---

## 4. Operation logs and undo

### Undo as an operation

**GitKraken.** The single most recent action, from the list in section 1
(<https://help.gitkraken.com/gitkraken-desktop/undo-and-redo/>).

**Tower.** The list in section 1 (<https://www.git-tower.com/features/undo>). On the Mac
it is also offered from Notification Center. That comes from a search summary of the
help page: UNVERIFIED.

**lazygit.** Reflog-driven, persists across sessions, and also works for actions taken
outside lazygit. Not covered: the working tree, the stash, pushes, branch creation, and
anything mid-rebase (`docs/Undoing.md`).

**jj** (`docs/operation-log.md`, `docs/tutorial.md`,
<https://docs.jj-vcs.dev/latest/cli-reference/>).
- **What an operation is:** "a snapshot of how the repo looked at the end of the
  operation".
- **What `jj op log` shows** for each entry: op id, user@host, relative time, duration,
  description, and the argv (`args: jj squash`). A working-copy snapshot is an operation
  of its own.
- **Commands:**
  - `jj undo` and `jj redo`
  - `jj op restore` (whole repository, or parts with `--what`)
  - `jj op revert` (the inverse of any one operation)
  - `jj op show`, `jj op abandon`, and `--at-op` to load the repository as it was at an
    operation
- **Concurrency.** Concurrent operations are lock-free, and their divergence is shown.

**GitButler.**
- **UI.** A timeline with a Revert button per entry and file diffs
  (<https://docs.gitbutler.com/features/timeline>).
- **CLI** (<https://docs.gitbutler.com/commands/but-oplog>): `but oplog`,
  `but oplog snapshot -m`, `but undo`, and `but oplog restore <sha>`, which confirms
  unless `-f` is given.
- **Restores are undoable.** "Restorations create a new oplog entry before running"
  (<https://docs.gitbutler.com/cli-guides/cli-tutorial/operations-log>).
- Retention: UNVERIFIED.

**GitHub Desktop.** Undo covers only the last *unpushed* commit
(docs.github.com, "Undoing a commit in GitHub Desktop").

**Fork.** No git-operation undo (fork-dev/Tracker#981, open).

### Visible command and activity logs

| Client | How it is opened | What it shows | How long kept |
| --- | --- | --- | --- |
| **magit** | `$`, the process buffer (<https://docs.magit.vc/magit/Running-Git.html>; `lisp/magit-process.el`) | Per repository. Each section heading shows the exit code ("run" while running, coloured ok or ng when done), an optional timestamp (`magit-process-timestamp-format`), the directory when different, and program plus args. The output sits beneath. `magit-process-popup-time` can pop it up for slow commands (default never). | `magit-process-log-max` defaults to 32 sections; past that the older half is dropped, but running processes are never dropped. Memory only. |
| **VS Code** | Output > Git | `Git._exec` (`extensions/git/src/git.ts`) logs `> git <args> [<ms>ms]`, with `(cancelled)` added if killed. stderr is always logged; stdout only for commands in `git.commandsToLog`. No exit code in the line. A failure modal offers "Show Command Output". | Session. |
| **Fork** | Activity Manager, a button on the status bar (<https://fork.dev/blog/posts/fork-1.0.67>) | Past and current activities with their results and each output; also where a running operation is cancelled. 1.0.88 shows output of all dialog commands; 2.18 "Show results for git commands". Auto-fetch entries crowd out manual ones (#2077). | A user asked for manual operations to be restored on startup, which implies the log is not persisted: UNVERIFIED. The maintainer: "Fork doesn't have a permanent log" (#2232). |
| **GitKraken** | Activity Logs, a footer icon (<https://help.gitkraken.com/gitkraken-desktop/activity-logs>) | Application and Repository tabs; each entry has a timestamp, an action description and a duration in ms. Actions, not argv. | Plain text; location undocumented. |
| **JetBrains** | Git tool window, Console tab (<https://www.jetbrains.com/help/idea/version-control-tool-window-console-tab.html>) | The generated commands and their results. | Session (UNVERIFIED). |
| **Sublime Merge** | The "Show Git Output" toolbar icon; command history added in build 2020 (changelog) | Git commands and their output. | UNVERIFIED. |
| **Sourcetree** | View > Show Command Output; ⌘⌃W history on the Mac | From a community thread whose body would not load. | UNVERIFIED. |
| **lazygit** | Command log panel | An action title (`LogAction`, e.g. "Dropping stash <hash>") plus the command (`LogCommand`) (`stash_controller.go`). | Session. |
| **GitHub Desktop** | None visible | `GitPerf.measure` writes `Executing <cmd> (took <s>s)` to the log file only in dev builds or past 1 s (`app/src/ui/lib/git-perf.ts`). | |

---

## 5. Amend safety: warnings for a commit already pushed

**GitHub Desktop: warns before amending a pushed commit.**
- **Amend is offered only on the top row** (`app/src/ui/history/commit-list.tsx`,
  `getContextMenuForSingleCommit`: `canBeAmended = … && row === 0`).
- **What counts as local** (`git-store.ts`, `loadLocalCommits`): commits in
  `upstream..branch`, or `HEAD --not --remotes` when there is no upstream.
- **The gate** (`app-store.ts`, `_startAmendingRepository`): when the commit is not local
  and `askForConfirmationOnForcePush` is on, it shows `PopupType.WarnForcePush` with
  `operation: 'Amend'`.
- **The dialog** (`app/src/ui/multi-commit-operation/dialog/warn-force-push-dialog.tsx`)
  is titled "Amend will require force push". It is the same dialog Rebase, Squash and
  Reorder use, and has a "do not show again" checkbox.
- **History** (`changelog.json`): amend arrived in 2.9.0 (#1644). Amending pushed commits
  was allowed from 2.9.6-beta1 (#13384).

**magit: warns, against a configured list of published branches.**
- `magit-commit-amend-assert` (`lisp/magit-commit.el`) runs before amend, extend, reword,
  squash and fixup. It confirms `amend-published` with "This commit has already been
  published to %s. Do you really want to modify it".
- **Detection** (`lisp/magit-git.el`, `magit-list-publishing-branches`): it tests each of
  `magit-published-branches` (default `'("origin/master")`, `lisp/magit-branch.el`) with
  `git merge-base --is-ancestor HEAD <branch>`. Only the listed branches are checked, not
  every remote.
- **Silencing:** add `amend-published` to `magit-no-confirm`.

**JetBrains: pushed commits are not offered, or are refused, rather than warned about.**
- `plugins/git4idea/backend/src/GitProtectedBranches.kt`
  (`findProtectedRemoteBranchContainingCommit`) uses the VCS log's containing-branches
  index, falling back to `GitBranchUtil.getBranches`, and matches the result against the
  protected-branch patterns. `isCommitPublished` means "pushed to a protected branch".
- **"Amend specific commit"** (`checkin/GitAmendCommitService.kt`) lists only unpublished
  commits (`GitRecentCommitsProvider(unpublishedOnly = true, …)`).
- **In-log edits are refused:** "The commit is already pushed to protected branch…"
  (`rebase/log/GitCommitEditingActionBase.kt`).
- No warning was found for amending HEAD on an unprotected branch: UNVERIFIED.

**Fork for Windows 1.91 (2023-11-17): "Show warning when amending already pushed
commit"** (<https://fork.dev/releasenoteswin>). The Mac notes have no matching entry.
The mechanism is unknown (closed source). Mac behaviour: UNVERIFIED.

**No warning:**
- **lazygit** shows a generic "Are you sure you want to amend…" (`files_controller.go`,
  `local_commits_controller.go`). Pushed state is computed
  (`commit_loader.go`: `git rev-list <ref> ^<ref>@{u} ^<main>…`) but only colours the
  hash. Divergence is caught at push time instead (`ForcePushPrompt`).
- **VS Code:** nothing in `extensions/git/src` pairs amend with pushed or upstream. Only
  push is guarded (`git.allowForcePush`, `git.confirmForcePush`, `git.branchProtection`).
- **gitui:** `can_amend()` (`src/popups/commit.rs`) checks only the mode, that HEAD
  exists, and the message.
- **Tower and GitKraken** warn in their help text only
  (<https://git-tower.com/help/guides/working-copy/commit-changes/mac>,
  <https://help.gitkraken.com/gitkraken-desktop/commits>). Whether either shows an in-app
  dialog: UNVERIFIED.
- **Sublime Merge and Sourcetree:** UNVERIFIED.

**Detection mechanisms seen, in summary:**

| Mechanism | Used by |
| --- | --- |
| Upstream range, or `--not --remotes` | GitHub Desktop |
| `rev-list ^@{u} ^<main>` | lazygit (display only) |
| `merge-base --is-ancestor` against a list | magit |
| Containing-branches index filtered by protected patterns | JetBrains |

---

## 6. Stash drop safety

**git itself.**
- `do_drop_stash` prints "Dropped <rev> (<full sha>)" (`builtin/stash.c`). `clear` prints
  nothing.
- The git-stash docs warn that cleared entries "may be impossible to recover".
- The docs give a recovery recipe from `git fsck --unreachable`: grep for commits, then
  `git log --merges --no-walk --grep=WIP` over them.
- `git stash store` puts a dangling stash commit back on the stash reflog
  (<https://git-scm.com/docs/git-stash>).

| Client | Confirmation | Recovery path |
| --- | --- | --- |
| lazygit | "Stash drop": "Are you sure you want to drop the selected stash entry(ies)?" (`SureDropStashEntry`). Captures the indices first, drops from the highest down, and blocks input until a refresh. | Logs "Dropping stash <hash>". Undo does not cover the stash. |
| magit | `magit-stash-drop` (`z k`) on one stash only asks which stash; several in the region get `magit-confirm 'drop-stashes`. `magit-stash-clear` always confirms, then runs `update-ref -d` (`lisp/magit-stash.el`). | Echoes "Deleted refs/… (was <short sha>)". It runs `git rev-parse <stash>` first, so the full hash is in the process buffer. |
| gitui | "Drop Stash(es)": "Sure you want to drop following stash(es)?" with the short hashes (`src/strings.rs`). | None. |
| VS Code | Modal: "Are you sure you want to drop the stash: {0}?". Drop All warns the entries "MAY BE IMPOSSIBLE TO RECOVER". No setting turns either off (`commands.ts`). | None. |
| GitHub Desktop | "Discard stash?" with a destructive button and a do-not-show-again box (`askForConfirmationOnDiscardStash`; `app/src/ui/stashing/confirm-discard-stash.tsx`). | None. |
| Tower | UNVERIFIED. | **Undo covers "deleting a stash"** (<https://www.git-tower.com/features/undo>). This is the only client found with documented stash-drop undo; the mechanism is undocumented. |
| GitKraken | Not documented ("Delete Stash: Permanently remove the stash", <https://help.gitkraken.com/gitkraken-desktop/stashing/>). | Not on the undo list. |
| Sourcetree | Confirms (repro steps in SRCTREE-8224); the text is UNVERIFIED. | None found. |
| Fork | UNVERIFIED. | No undo (#981). |
| Sublime Merge | UNVERIFIED. "Drop Stash…" was added in build 2038, `stash clear` in 2047. | A 2018 bug dropped the **latest** stash when the user chose an older one from the sidebar (forum thread 39045). That is a reminder to address a stash by its oid, not its index. |
| JetBrains | Drop and Clear in the Unstash dialog; no confirmation documented. | Shelves have "Recently Deleted"; stashes have nothing. |

---

## 7. Line staging: gestures, selections across hunks, the side-by-side view, and how the patch is built

**GitHub Desktop.** Lines are chosen for the commit, not staged.
- **Gestures** (`app/src/ui/diff/side-by-side-diff.tsx`, `side-by-side-diff-row.tsx`; one
  component draws both the unified and split views):
  - Press on a line number and drag. On release, every line in the range is set to the
    opposite of the start line's state (`onEndSelection`).
  - The hunk handle toggles a run of changed lines bounded by context lines, not the whole
    hunk; the code comment says "a hunk is not granular enough"
    (`findInteractiveOriginalDiffRange`).
- **Split view.** `getDiffRowLineNumber(row, column)` maps a modified row to its before
  or after line by which column was clicked. The selection is a contiguous range of
  unified-diff line indices.
- **Hidden whitespace disables selection.** With whitespace hidden, a press shows a
  popover, "Selecting lines is disabled when hiding whitespace changes", offering to show
  whitespace again (`whitespace-hint-popover.tsx`; changelog 2.8.1 #12129, 2.9.4 #12979).
- **Discard.** Right-clicking the gutter gives "Discard Added/Removed/Modified Line(s)…"
  (`getDiscardLabel`), applied with `git apply --unidiff-zero --whitespace=nowarn -`
  (`app/src/lib/git/apply.ts`).
- **How the patch is built** (`app/src/lib/git/commit.ts` `createCommit`;
  `app/src/lib/patch-formatter.ts`):
  - At commit time Desktop resets everything (`unstageAll`) and re-stages, using
    `applyPatchToIndex`, which is `git apply --cached --unidiff-zero --whitespace=nowarn -`
    fed by `formatPatch`.
  - `formatPatch` writes its own hunk headers. An unselected deletion becomes context, and
    an unselected addition is dropped.

**lazygit** (`docs/keybindings/Keybindings_en.md`, "Main panel").
- **Keys:** `space` stages or unstages; `v` range select; `a` toggles line or hunk mode;
  `d` discards (on the staged side, it unstages); `E` edits the hunk in `$EDITOR`; `tab`
  switches pane.
- **A selection may cross hunks.**
  `pkg/gui/controllers/working_tree_diff_actions.go` `applyDiffLines` maps the chosen
  lines to patch indices, then runs `patch.Transform(…).FormatPlain()` and
  `git apply [--cached] [--reverse]` (`pkg/commands/git_commands/patch.go`).
- **Refusals and shortcuts in the same code:**
  - At context 0 it refuses ("Staging or unstaging changes is not possible with a diff
    context size of 0").
  - A selection covering every change in the file stages the whole file instead of a
    patch.
- **Transform rule** (`pkg/commands/patch/transform.go`): an unselected old-file line
  becomes context, and an unselected new-file line is dropped.
- No side-by-side view (UNVERIFIED).

**magit.**
- A region inside one hunk has scope `region` and stages only those lines. A region
  across several hunks has scope `hunks` and stages them **whole** (`lisp/magit-diff.el`,
  `magit-diff-scope`).
- **Patch** (`magit-diff-hunk-region-patch`): the same context and drop rule, then
  `diff-fixup-modifs`.
- **Apply** (`magit-apply-patch`): `git apply --cached -p0 -C<context> --ignore-space-change -`.
  With too little context it refuses with "Not enough context to apply patch".
- **Keys:** `s` stage, `u` unstage, `k` discard.

**gitui** (`src/keys/key_list.rs`).
- **Keys:** Enter stages the hunk; `s` stages lines; `d` resets lines (with a confirm);
  Shift+D resets the item or hunk; Shift+Up and Shift+Down extend the selection.
- **A selection may cross hunks:** `selected_lines()` flattens all of them
  (`src/components/diff.rs`).
- **Mechanism** (`asyncgit/src/sync/staging/stage_tracked.rs`, `stage_lines`): no
  `git apply`. It rebuilds the index blob through libgit2 and calls `index.add`.
- Line staging in an untracked file is a TODO.

**GitX** (`Resources/html/views/commit/commit.js`).
- **Gestures:** click and drag; shift-click extends.
- **A selection is clamped to one hunk:** `computeSelection`, with the comment "Stay
  inside this hunk".
- **Patch:** the same context and drop rule.
- **Apply:** `git apply --unidiff-zero [--cached] [--reverse]` (`Classes/git/PBGitIndex.m`).

**VS Code.**
- `git.stageSelectedRanges` intersects the editor selections with the line changes
  (`commands.ts`, `stageSelectedChanges`).
- It rebuilds the whole content from the index version (`applyLineChanges`) and writes it
  with `hash-object --stdin -w --path` and `update-index --cacheinfo` (`git.ts`, `stage()`).
- `git.unstageSelectedRanges` and `git.revertSelectedRanges` exist alongside it.

**Fork.**
- Select lines, then Stage or Discard. Hunks have Stage and Discard buttons.
- **Side by side:** a selection covers one column only, so a modified line takes two
  actions. The maintainer said the two sides are "two separate controls", while the
  unified view is "a single text control" (fork-dev/Tracker#1985).
- Release notes 1.0.3 and 1.0.12 introduced line discard and line selection.

**Tower.** Click line numbers to select lines, and "Stage Chunk" turns into "Stage
Lines" (<https://www.git-tower.com/help/guides/working-copy/stage-changes>). Selections
across hunks and the split view: UNVERIFIED.

**Sublime Merge.** Selecting lines turns the hunk buttons into stage and discard lines
(forum, user-reported:
<https://forum.sublimetext.com/t/solved-stage-unstage-individual-lines/39547>). Build
2063 added hunks and lines on untracked files.

**GitKraken.** Highlight lines, then right-click "Stage selected lines"
(<https://help.gitkraken.com/gitkraken-desktop/staging/>).

**JetBrains.** Checkboxes per chunk and per line in the gutter, and "Split Chunks and
Include Selected Lines into Commit"
(<https://www.jetbrains.com/help/idea/commit-and-push-changes.html>).

**What they agree on.**
- **The patch rule.** Desktop, lazygit, magit and GitX state it outright: an unselected
  deletion becomes context and an unselected addition is dropped, the reverse when
  unstaging.
- **How the patch is applied.** Either `git apply --cached` (Desktop and GitX with
  `--unidiff-zero`, magit with `-C<n>`), or by rewriting the index blob (VS Code, gitui).
- **Refused rather than guessed:** zero context (lazygit, magit) and hidden whitespace
  (Desktop).
- **Selections across hunks.**
  - Allowed: Desktop, lazygit, gitui.
  - Clamped to one hunk: GitX.
  - Widened to whole hunks: magit.
- **The side-by-side view.** Fork restricts a selection to one column. Desktop maps a
  click to its column, then selects in unified-row space.

---

## 8. Commit box conventions

**Subject and body split.**
- **Separate fields:**
  - GitHub Desktop: Summary and Description (`app/src/ui/changes/commit-message.tsx`)
  - Fork: 1.0.3, "Split commit message field into two"
  - Tower: Short Commit Summary and Extended Description
  - lazygit: summary and description views, `tab` to switch
- **One box:** VS Code, gitui, magit's `git-commit-mode` buffer, and Sublime Merge
  (UNVERIFIED).

**Length guides.**
- **GitHub Desktop:** `IdealSummaryLength = 50` (`app/src/lib/wrap-rich-text-commit-message.ts`),
  with a hint to move detail into the description, gated by `showCommitLengthWarning`.
- **VS Code:** `git.inputValidation` (off by default), subject 50 and body 72
  (`src/diagnostics.ts`).
- **magit:** `git-commit-summary-max-length` is 68, a highlight only. The style checks
  `overlong-summary-line` and `non-empty-second-line` run at finish. The body auto-fills
  at Emacs's `fill-column`; that its default is 72 is UNVERIFIED.
- **lazygit:** a live character count (`gui.commitLength.show`), and
  `git.commit.autoWrapCommitMessage` at width 72 (`pkg/config/user_config.go`).
- **gitui:** `FIRST_LINE_LIMIT = 50`.
- **JetBrains:** subject and body inspections both at `RIGHT_MARGIN = 72`
  (`SubjectLimitInspection.kt`, `BodyLimitInspection.kt`).
- **Fork:** a subject counter (1.0.20), a configurable limit (Mac 1.0.89, Win 1.43), and
  a ruler with "Wrap paragraph at ruler" (Mac 1.0.50, Win 1.26).
- **Tower:** a configurable title limit, body hard-wrap and highlighting of over-long
  lines.
- **Sublime Merge:** `rulers` and Edit > Wrap Paragraph (a forum moderator).

**Message history.**
- **magit:** `M-p` and `M-n` over `log-edit-comment-ring`. Messages are saved on finish
  and on cancel (`git-commit-save-message`), optionally per repository.
- **lazygit:** Up and Down walk HEAD's log (`git log -1 --skip=N`). These are past
  commits, not drafts.
- **gitui:** Ctrl+N; 20 messages kept (`COMMIT_MSG_HISTORY_LENGTH`).
- **Sublime Merge:** a dropdown (build 2020), per repository (2047).
- **Fork:** "recent commit messages" (1.0.12).
- **JetBrains:** a history button.
- **GitHub Desktop:** none (#14076, closed).

**Amend toggle.**
- **Checkbox:** Fork, GitKraken, JetBrains (with a target dropdown), Sublime Merge
  (forum users).
- **Tower:** an Amend option, or hold ⌥ so Commit becomes Amend
  (<https://www.git-tower.com/help/guides/faq-and-tips/tips-and-tricks/quick-amend/mac>).
- **GitHub Desktop:** amend is entered from the History menu, with a "Stop amending"
  notice.

**Committing with nothing staged.**
- **VS Code:** `git.suggestSmartCommit` asks whether to stage everything and commit, with
  Yes, Always and Never. Always sets `git.enableSmartCommit`; `git.smartCommitChanges`
  is `all` or `tracked`. With nothing at all to commit it offers "Create Empty Commit"
  (`commands.ts`).
- **magit:** refuses (`magit-commit-assert`) unless `--all` is set.

**How hook output is shown.**
- **GitHub Desktop** (since 3.5.5, #21319):
  - It runs hooks through a proxy: `core.hooksPath` is set through `GIT_CONFIG_PARAMETERS`
    (`app/src/lib/hooks/with-hooks-env.ts`), and the proxy runs `git hook run <name>`
    (`hooks-proxy.ts`).
  - Progress is shown live.
  - On failure a "HookFailed" dialog shows the output in a terminal view, with "Ignore and
    Continue" or "Abort".
- **VS Code:** a modal error with "Show Command Output".
- **Fork:**
  - "Skip hook" when pre-commit fails (Mac 1.0.57)
  - interactive pre-commit output (1.0.69)
  - hook output shown as it runs (1.0.81)
- **gitui:** runs the hooks itself (`asyncgit/src/sync/hooks.rs`) and pops up the error.
- **GitKraken:** a notification.

**Skipping hooks (`--no-verify`).**
- GitHub Desktop: "Bypass Commit Hooks" (3.5.5)
- VS Code: `git.commitNoVerify*` behind `git.allowNoVerifyCommit`, with
  `git.confirmNoVerifyCommit`
- lazygit: `w`, and automatically when the summary starts with `git.skipHookPrefix`
  ("WIP")
- gitui: Ctrl+F
- Fork: Win 1.34, Mac 1.0.74
- GitKraken: "Skip Git hooks"
- Sublime Merge: build 1116
- JetBrains: "Run Git hooks"

**Sign-off and co-authors.** Sign-off is a toggle or command nearly everywhere:
- GitHub Desktop, VS Code (`git.alwaysSignOff`), lazygit, gitui (Ctrl+S), magit
  (`C-c C-s`), Fork (1.0.61), JetBrains.

Co-authors:
- GitHub Desktop has a `Co-Authored-By` input and confirms an unknown co-author.
- magit has `git-commit-co-authored`.
- Fork for Windows 1.72 autocompletes co-author signatures.

**`commit.template`.**
- **Honoured** by VS Code (`getCommitTemplate`: `~` expansion, a relative path resolved
  against the repository root, comments stripped, and a pending merge or squash message
  taking precedence; `src/repository.ts` `getInputTemplate`), gitui, Fork (Mac 1.0.64,
  Win 1.26), GitKraken and JetBrains.
- **Not by GitHub Desktop** (#8698, closed not planned).

**Message cleanup.**
- GitHub Desktop commits with `git commit -F -`.
- For merges it adds `--no-edit --cleanup=strip`, because `--no-edit` otherwise switches
  git's cleanup to `whitespace`, which keeps `#` lines (`app/src/lib/git/commit.ts`,
  `createMergeCommit`).

---

## What the precedent suggests for O3, the reflog view and the operation log

These are options with their mechanism and cost. None is chosen here.

### O3: backing up before a destructive working-tree operation

The class at issue: discard file, hunk or line; clean untracked; a hard reset over a
dirty tree.

**Option A: confirmation only.**
- **Who does this:** Fork, Sublime Merge, lazygit, gitui, Sourcetree on Windows. VS Code
  for tracked files.
- **Mechanism:** an honest prompt, possibly a deliberately awkward one (Fork's refusal to
  make hunk discard skippable, Tracker#2264; VS Code's IRREVERSIBLE / FOREVER LOST text).
- **Cost:** nothing at run time.
- **Recovery:** none.
- **Precedent against it:** VS Code #32405 and #240523, and Fork #419, #1265 and #657.
  The VS Code issues both say that a modal, however loud, is clicked through.

**Option B: whole files to the OS Trash.**
- **Who does this:** GitHub Desktop for every modified or new file; VS Code and
  Sourcetree for Mac for untracked files only.
- **Mechanism on Linux:** the freedesktop.org Trash specification (a `files/` and `info/`
  pair under `$XDG_DATA_HOME/Trash`, or `$topdir/.Trash-$uid` on another mount), or
  shelling out to `gio trash` as Electron does.
- **Cost:**
  - a copy or rename per file
  - a cross-device move when the repository and the home Trash sit on different
    filesystems
  - a whole file trashed to save a one-line discard
- **Gaps:**
  - It does not cover hunk or line discards unless the whole pre-discard file is trashed
    too, which no client does.
  - It loses index content that differs from the working tree (Desktop's flow).
  - It fails in exactly the environments Desktop and VS Code keep patching: a disabled
    Trash, a missing helper, a snap or a remote session.
- **Fit with Cairn:** the Trash sits outside the repository, so it is outside `cairn-git`'s
  read/write vocabulary. Somebody would have to own a filesystem write that is not a
  `git` invocation.

**Option C: a hidden snapshot commit under a private ref.**
- **Who does this:** magit wip, git-branchless, GitButler, jj.
- **Mechanism:**
  1. Before the destructive step, run `git stash create` (it captures the index I and
     the working tree W). Or build the trees with plumbing over a temporary
     `GIT_INDEX_FILE`, as magit and `stash push -u` do, which also lets named untracked
     paths in.
  2. Record the result with `git update-ref --create-reflog -m <prompt> refs/cairn/backup <oid>`.
     Every backup is then a reflog entry that names what the user confirmed, which fits
     `Confirmed` and `ops::Performed`.
- **Coverage:** whole-tree, so it covers line and hunk discards as well as file discards.
  The index is preserved too.
- **Cost:**
  - **It is a write.** It refreshes and writes `.git/index` and stats every tracked file.
  - **It runs filters.** Every modified file is hashed and its clean filter runs.
  - **It grows the object store,** in proportion to the dirty bytes, binaries included.
  - **It needs an expiry policy.** A private ref's reflog follows the 90/30-day expiry,
    unless the client prunes it itself.
  - **It is visible to `git log --all`** and is pushed by `--mirror`. GitButler's
    hand-written reflog on a branch ref is one workaround, at the price of writing git's
    files directly.
  - **It fails on an unborn HEAD.**
- **Untracked files are a separate decision.** Every precedent caps or excludes them:
  - jj refuses new files over 1 MiB
  - git-branchless excludes untracked files for size and secrets
  - magit tracks tracked files only

  `clean untracked` would need its own policy: include up to a size cap, or fall back to
  Option A or B for those files.
- **What the user sees:** nothing, unless the reflog view or the operation log shows the
  ref.

**Option D: a visible stash entry ("stash first", as in the `docs/design/ui.md` mockup).**
- **Mechanism:** the same `git stash create`, then `git stash store -m <prompt>`. This is
  exactly what git's own autostash does when applying fails (`sequencer.c`
  `apply_save_autostash_oid`).
- **Advantages:**
  - Recovery goes through an operation the user already knows (stash apply, or pop).
  - `refs/stash` never expires by default (`reflog.c`).
  - No private namespace.
- **Cost:**
  - The same write and filter cost as C.
  - It **pollutes the stash list** with one entry per discard, which is the thing a
    "stash first, on by default" checkbox would make routine.
  - The stash entries are themselves things a stash drop then destroys.
  - Untracked files again need `stash push -u`-style plumbing; `stash create` takes no `-u`.
  - The tree must not be reset by `stash push`: only create plus store leaves it alone.

**Option E: undo in the app, over C or D.**
- **Who does this:** Tower and GitKraken, by their own description. Their mechanism is
  undocumented, and Tower's docs contradict each other.
- **Mechanism:** a session undo action that applies the backup commit's W (and I) back
  over the discarded paths.
- **Cost:** everything in C, plus an undo model with a scope. GitKraken undoes the last
  action only; Tower's stack is per window and in memory (inferred).
- **Risk:** applying a stale snapshot over edits made since then is itself a destructive
  operation.

**Cross-cutting facts any option must respect:**
- **The discard primitives.** A line discard is `git apply -R` (Desktop
  `--unidiff-zero`); a file discard is `checkout-index -f` or `checkout --`.
- **The backup must see the pre-discard state** of exactly the paths about to change.
  magit shows how to miss it: the `magit-inhibit-refresh` path skips the snapshot.
- **The floor.** Everything in C and D works at git 2.30: `stash create`, `stash store`,
  `update-ref --create-reflog`, and the temporary-index plumbing. `stash export` (2.51)
  does not.

### The reflog view

**Option R1: lost commits drawn inside the history graph (Fork).**
- **Mechanism:** walk from reflog entries as extra tips, behind a toggle.
- **Cost:** the walk's tip set grows. Fork's own bugs show where it breaks: filters (2.39)
  and hidden remotes (#1528).
- **Missing:** the reflog's own order and messages. It shows *what* is reachable, not
  *when* or *why*.

**Option R2: a dedicated list, defaulting to HEAD with a per-ref switch (Tower, magit).**
- **Columns** (magit's): short hash, `@{n}`, an action label parsed from `%gs` (commit,
  amend, reset, rebase, checkout…), the message, and the date and author.
- **Actions:** the history row's own (checkout, branch here, reset, cherry-pick).
- **A pitfall seen in lazygit:** use the reflog **entry's** timestamp, not the commit's
  `%ct`. They differ for every reset and checkout.

**Option R3: a HEAD-only tab with the full action set (lazygit).**
- **Actions:** detached checkout, new branch, reset soft/mixed/hard, cherry-pick, copy
  hash.
- **Cost:** the cheapest to build.
- **Gap:** it does not answer "what was `feature` before the amend", which a per-branch
  reflog does.

**Either list could also show the backup ref from Option C,** or `refs/stash`'s reflog,
making the uncommitted class recoverable from the same view as the committed one.

### The operation log

**Option L1: a process log (magit, VS Code, Fork's Activity Manager).**
- **Per invocation:** argv, start time, duration, exit status and captured output
  (magit's set is the most complete).
- **Bounded:** magit keeps 32 sections and never drops a running one.
- **Session only.**
- **Cost:** low. Cairn's registry already keeps a bounded command log per repository
  (`docs/systems/git-processes.md`); what is missing is a view.

**Option L2: an operation log with prompts (jj `op log`, GitButler's timeline).**
- **Per user operation:**
  - time and duration
  - a human description
  - **the prompt the user confirmed** (the text `Confirmed` already carries)
  - the `git` commands it ran
  - for a destructive one, the backup or reflog entry that undoes it
- **The model to follow:** jj's entry (op id, user@host, time, duration, description,
  args). GitButler's rule is worth copying: a restore records an entry before it runs.
- **Cost:** persisting it across sessions is a decision of its own. No GUI studied
  persists its log. jj and GitButler do, inside the repository (`.git/gitbutler/`, jj's
  op store), which is a repository write outside `ops/`'s current vocabulary.

**Option L3: L2 plus restore from an entry.**
- Restore is an operation with its own confirmation, and its own entry before it runs
  (GitButler).
- It depends on O3: without Option C or D there is nothing to restore for the
  uncommitted class.

### Neighbouring decisions the precedent informs

- **Amend over a pushed commit.** The detection options, cheapest first:
  - upstream range (Desktop)
  - `--not --remotes` (Desktop with no upstream)
  - `merge-base --is-ancestor` against a configured list (magit)
  - a protected-branch pattern (JetBrains)

  The cheap check (is HEAD reachable from any remote-tracking ref) is the
  `--not --remotes` form.
- **Stash drop.** Only Tower undoes one. lazygit and magit surface the dropped hash,
  which is git's own recovery handle ("Dropped … (<sha>)"). Addressing a stash by oid
  rather than by index avoids Sublime Merge's 2018 wrong-stash bug.
- **Line staging.**
  - The patch rule is universal (an unselected deletion becomes context, an unselected
    addition is dropped).
  - Refusing at zero context or with whitespace hidden is precedent (lazygit, magit,
    Desktop). That matches Cairn's rule that a patch is emitted from the exact diff at
    three lines of context.
  - In side by side, Fork restricts a selection to one column, and Desktop maps the click
    to its column, then selects in unified-row space.
