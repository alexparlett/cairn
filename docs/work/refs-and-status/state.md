# State — refs-and-status

The cross-session cheat sheet. Every session updates this before ending.

**Status: planned. No phase has started. No code exists for this packet.**
Integration branch `feature/refs-and-status` does not exist yet; phase 01 creates
it from `main` and pushes it.

## Locked decisions

L1-L12 in `brainstorm.md`; the spec is `docs/prd/refs-and-status.md`. The ones
that most constrain implementation:

- **Status is `git status --porcelain=v2 -z`, run as a read** (L1). Not gix's
  status, not a hybrid. Untracked per file unless the user set
  `status.showUntrackedFiles=no`; ignored never listed (L2).
- **Refs are gix's, under four parity rules** (L3): never peel a symbolic ref,
  hide a dangling one, read the stash reflog oldest-first then reverse, resolve
  `remote = .` upstreams by hand. `GIT_NAMESPACE` is not honoured.
- **Reftable is refused at open** (L4).
- **The walk seeds from every ref; a stash is a row of its own** with one edge to
  the commit it was made on, never a tip (L5). No working-tree row (L6).
- **Fork's labels, sidebar, toolbar, Local Changes and refresh** (L7-L11), every
  deviation named: a generic remote glyph until packet 6; no working-tree row.
- **Refresh on focus, after an operation, and on the Refresh chord; no watching**
  (L11). A changed snapshot (any ref, `HEAD`'s state, the stash list) reopens the
  history; a reopen frees its old rows off the UI thread (#52).
- **Threads and lanes** (brainstorm, "Settled in the coverage audit"): refs on
  the history thread; status and ahead/behind on a third thread; a ref's find is
  history-lane work; a stash's base seeds the walk.

## Open questions

- How `status.showUntrackedFiles` is read as git reads it (L2) — phase 02; a new
  `git config` porcelain read needs the user.
- The Refresh chord per platform (Fork: ⌘R on macOS, F5 on Windows) — phase 04,
  from `fork-dev/Docs`' shortcut lists.
- C11's first-page bar is written as 200 ms because history-graph's A7 has no
  number (L12); the user may revise it at the merge bar.

## New modules and interfaces

None yet.

## Validation status

| Phase | Status |
| --- | --- |
| 01 refs engine | not started |
| 02 status engine | not started |
| 03 history from every ref | not started |
| 04 worker and refresh | not started |
| 05 labels and toolbar | not started |
| 06 sidebar | not started |
| 07 local changes | not started |
| 08 QA | not started |
