# State — refs-and-status

The cross-session cheat sheet. Every session updates this before ending.

**Status: planned. No phase has started. No code exists for this packet.**
Integration branch `feature/refs-and-status` does not exist yet; phase 01 creates
it from `main` and pushes it.

## Locked decisions

L1-L14 in `brainstorm.md`; the spec is `docs/prd/refs-and-status.md`. The ones
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
- **Slim rows** (L14): one id, a parent count, shared text and author stores, no
  email, no per-row allocation (phase 04).
- **Compact rows, no cap** (brainstorm L13): a row keeps its id, parents, text,
  lane and only the lane changes at it; the drawn edges are derived. A deep find
  pages as planned and retains only compact rows (C16: 64 MiB for all of
  rust-lang/rust, against 1.4 GiB today). A deliberate deviation from Fork, which
  caps its list and does nothing when a ref is past it.
- **Threads and lanes** (brainstorm, "Settled in the coverage audit"): refs on
  the history thread; status and ahead/behind on a third thread; a ref's find is
  history-lane work. A stash is drawn only on a commit the ref walk reaches, as
  in Fork; nothing about a stash seeds the walk (revised after
  `fork-unreachable-stash-base.md`).

## Open questions

- How `status.showUntrackedFiles` is read as git reads it (L2) — phase 02; a new
  `git config` porcelain read needs the user.
- The Refresh chord per platform (Fork: ⌘R on macOS, F5 on Windows) — phase 06,
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
| 03 compact rows | not started |
| 04 slim rows | not started |
| 05 history from every ref | not started |
| 06 worker and refresh | not started |
| 07 labels and toolbar | not started |
| 08 sidebar | not started |
| 09 local changes | not started |
| 10 QA | not started |
