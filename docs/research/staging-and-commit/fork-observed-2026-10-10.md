# Fork for Windows 2.21.1, observed first-hand (2026-10-10)

Driven in a Windows 10 22H2 VM (QEMU/KVM), git 2.56.0.windows.2, a test repository with a local bare
remote. Screenshots: ~/VMs/fork-win10/shots/ (lc*, ln*, dc*, cb*, am*, lk*, amd*). VERIFIED = seen on screen.

## Local Changes selection
- Click f4, then Ctrl-click f2: both selected; the diff shows **f4, the file clicked first** (not the
  topmost, not the one just toggled). No "k of n selected" indication anywhere. (lc2)
- Focus in the diff, no lines selected, **Return does nothing**. (lc3)
- Focus in the diff, **Ctrl+S stages the whole selection** (f2 and f4); the selection moves to **f3, the row
  in the first acted row's place** (grey, the list unfocused). (lc4)
- f5 selected, then committed from a terminal: on returning, Fork selects **f6, the row that took f5's
  place**. (lc5)
- Drag-select lines in the diff: floating **Stage** and **Discard…** over the hunk (captions do not count
  lines on Windows). Switch to another app and back with the file untouched: **the line selection is gone**,
  no notice. (ln1, ln2)

## Discarding
- Floating Discard… over a line selection: dialog title **"Discard changes"**, text **"Do you want to
  discard all your changes in the selected lines?"**, buttons **"Discard 2 Lines"** (default, focused) and
  **"Cancel"**. No "can't undo". (dc1)

## Create Branch
- Branch toolbar button: **Create Branch**, subtitle "Use '/' as a path separator to create folders",
  "Create branch at: ⑂ main", "Branch name:", "Check out after create". Ticking it adds **Local changes:
  Don't change / Stash and reapply / Discard** (in that order); Discard selected shows **⚠** beside it; the
  button stays **"Create and Checkout"**. (cb2, cb3)
- Create and Checkout with Discard: **no confirmation**; staged and unstaged changes are gone. The
  Activity Manager shows the command: **`git checkout --no-track -b topic refs/heads/main --force`**. (cb4, am1)

## Activity Manager (status box popover)
- Tabs **All / User / Background**. Each row: name, a **result line** under it, time **HH:MM** (local),
  **⚠** on a failure. Seen: "Create branch 'topic'" / "Switched to branch'topic'"; "Stage 2 Files" /
  "staged"; "Fetch 'origin'" / "Already up to date" (a background fetch); "Stage 1 File" ⚠ / "stage failed".
- Right pane header: name, and **"succeeded 10 Oct 2026 09:40:…"** / **"failed 10 Oct 2026 09:40:…"**
  (local date and time). Then `$ git …` lines and git's output. (am1, am2)
- Stage runs `git add --force --verbose --pathspec-from-file=- --pathspec-file-nul --`.

## A stale index.lock
- No banner or standing state while `.git/index.lock` exists. (lk1)
- Stage with the lock present: Fork **retries** — the Activity Manager shows the fatal error, then
  **"Repository is locked. Retrying..."**, repeated. Then **Git Error**: "An unexpected error occured while
  performing the git request", git's output, and **"Remove .git/index.lock"** (bottom left) beside
  **"Close"**. (lk2, am2)
- Pressing Remove .git/index.lock: **no confirmation**; the lock is removed; the stage is **not** retried. (lk3)

## Amend
- Tick Amend with HEAD on a remote: the subject fills with HEAD's message; Staged shows the commit's files;
  an **inline note** beside the checkbox, **cut off by the button**: "Amending commit that has already been
  pus…"; the button reads **"Amend Last Commit"**. Hovering shows no tooltip. (amd1, amd2)
- Pressing it amends **at once, no dialog**; the branch becomes 1↑1↓ against its upstream. (amd3)
