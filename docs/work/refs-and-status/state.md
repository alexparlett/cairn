# State — refs-and-status

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 01 done (refs engine); phase 02 next.** Integration branch
`feature/refs-and-status`, in the worktree `.claude/worktrees/refs-and-status`,
packet mode.

## Locked decisions

L1-L14 in `brainstorm.md`; the spec is `docs/prd/refs-and-status.md`. The ones
that most constrain implementation:

- **Status is `git status --porcelain=v2 -z`, run as a read** (L1). Not gix's
  status, not a hybrid. Untracked per file unless the user set
  `status.showUntrackedFiles=no`; ignored never listed (L2).
- **Refs are gix's, under five parity rules** (L3): never peel a symbolic ref,
  hide a dangling one, read the stash reflog oldest-first then reverse, resolve
  `remote = .` upstreams by hand, and resolve every other upstream by hand too
  (first `merge`, literal match against the remote's refspecs in order).
  `GIT_NAMESPACE` is not honoured. A ref naming a missing object is skipped and
  counted (user decision; git fatals).
- **Reftable is refused at open** (L4), and so is a format-version-0 repository
  that sets `extensions.refStorage` at all (user decision, as git refuses it).
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

Phase 01 (`docs/systems/refs.md` is the as-built account):

- `cairn-model`: `crates/cairn-model/src/refs.rs` — `RefsSnapshot { refs, head,
  stashes, unreadable }` (`find`, `of_kind`), `Ref { name, kind, target, symbolic,
  upstream }` (`commit_id`), `RefKind`, `RefTarget` (`Commit`, `Tag { object,
  commit }`, `Other`; `commit_id`, `object`), `Upstream` (`Exists { name, commit }`,
  `Gone { name }`), `HeadState` (`Branch`, `Detached`, `Unborn`), `StashEntry {
  index, message, commit, base }`, `AheadBehind { ahead, behind }`. The model's
  accessor is `commit_id`, not `commit`: `.commit(` is on the gitoxide-mutation
  roster for `cairn-git`'s source.
- `cairn-git`: `Repository::refs(&cancel) -> RefsRead { snapshot, cost: RefsCost }`
  (`src/refs.rs`, `src/refs/stash.rs`, `src/refs/upstream.rs`);
  `Repository::ref_tips()` is now the snapshot (the network lane compares
  snapshots); `Repository::ahead_behind(&snapshot, &cancel) -> AheadBehindRead {
  counts, commits_read, elapsed }` (`src/ahead_behind.rs`, over
  `history::walk::hiding`, no commit-graph so every read is cancellable);
  `src/ref_storage.rs` refuses at open (`Error::RefStorageUnsupported`,
  `Error::RefStorageNeedsFormatVersion1`); the open clears gix's refs namespace.
  New errors: `RefsCancelled`, `AheadBehindCancelled { branches }`.
- Gate: `require_reftable_where_possible` in `scripts/gate.sh`'s `test-full`
  sets `CAIRN_REQUIRE_REFTABLE`; twin
  `the_reftable_refusal_is_required_wherever_it_can_run`.

## Validation status

| Phase | Status |
| --- | --- |
| 01 refs engine | done: C1, C2, C3 pass; C11 refs numbers in progress.md; full gate green |
| 02 status engine | not started |
| 03 compact rows | not started |
| 04 slim rows | not started |
| 05 history from every ref | not started |
| 06 worker and refresh | not started |
| 07 labels and toolbar | not started |
| 08 sidebar | not started |
| 09 local changes | not started |
| 10 QA | not started |
