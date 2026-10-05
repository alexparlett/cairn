# gix 0.87.1: refs and status, read from the vendored source

Research for the `refs-and-status` packet, recorded 2026-10-05. Every API claim
below cites the vendored source under
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` for the versions
`Cargo.lock` pins today: `gix 0.87.1`, `gix-ref 0.67.1`, `gix-status 0.34.1`,
`gix-dir 0.29.1`, `gix-index 0.55.0`, `gix-diff 0.67.1`, `gix-filter 0.34.0`,
`gix-features 0.49.1`, `gix-command 0.10.1`, `gix-discover 0.55.0`. Paths below are
relative to that directory; line numbers are for those pinned versions. Where a
claim about **git's** side is made it was measured, not recalled, unless marked
otherwise; anything neither read nor measured is marked **OPEN**.

Measurement method: a throwaway crate in the session scratchpad depending on
`gix = "=0.87.1"` with Cairn's exact feature list and Cairn's `Cargo.lock`, built
`--offline`, run against fixture repositories built with real `git version 2.56.0`
(this machine's git) and compared with `git status --porcelain=v2` /
`git for-each-ref`. Nothing in the Cairn tree was built or changed; the crate and
fixtures were scratch and are not kept. One timing ran read-only against the bench
checkout `~/Development/bench/rust`.

## Headlines

1. **Refs enumeration is sound and cheap, with three traps.** `references()?.all()`
   merges loose and packed refs in one bytewise-sorted pass (loose wins), skips
   invalid names silently as git does, and yields broken refs as `Err` items rather
   than stopping. 10,501 refs: 1.7 ms unpeeled, ~22 ms with `.peeled()` (release).
   Traps: (a) `Iter::peeled()` **replaces a symbolic ref with its target, name and
   all** — `refs/remotes/origin/HEAD` comes out as a second `refs/remotes/origin/main`
   (measured); (b) a dangling symref is an `Ok` symbolic item unpeeled and a
   nameless `Err` peeled, where git hides it silently; (c) `HEAD` is not in `all()`
   (it is in `pseudo()`).
2. **No reftable support, and gix opens a reftable repository without complaint.**
   No `extensions.refStorage` check exists; `head()` then fails on
   `refs/heads/.invalid` and `all()` yields one `Err` for the `refs/heads` file
   (measured on a `git init --ref-format=reftable` repo). It fails loudly, not
   silently, but Cairn must detect `extensions.refStorage` itself.
3. **Upstream: names yes, ahead/behind no.** `branch_remote_ref_name` /
   `branch_remote_tracking_ref_name` exist. **`branch.<n>.remote = .`** (a local
   upstream) gives `None` where git gives `refs/heads/<merge>` (measured).
   Ahead/behind has no API; it is two `rev_walk(..).with_hidden(..)` counts.
4. **Stash: nothing stash-specific in gix.** `refs/stash` is an ordinary ref with a
   reflog. `log_iter().rev()` uses a fixed 4 KiB window and **fails and stops on any
   reflog line longer than 4 KiB** (a long `git stash push -m`; measured); `all()`
   then reversing is the safe read.
5. **Status agrees with `git status --porcelain=v2` on every ordinary dimension
   measured** — staged add/modify/delete/rename (incl. rename-with-edit), unstaged
   modify/delete/exec-bit/type-change, intent-to-add, submodule commit/modified/
   untracked and `submodule.<n>.ignore`, the seven conflict kinds, untracked
   collapsing in `normal` and `all`, ignored in `normal`, `core.fileMode=false`,
   autocrlf, `.git/info/exclude`, XDG ignore, `core.excludesFile`, unborn HEAD,
   split index, and the user's clean filter (it ran).
6. **Measured divergences from git, each reproducible:**
   - a conflicted path with **no stage-2 entry** (DU, UA) is *also* reported
     `Untracked` by the dirwalk (`entry_by_path` looks at stages 0 and 2 only);
   - **intent-to-add + deleted original**: git shows a worktree rename `.R`, gix
     shows `Removed` + `IntentToAdd`;
   - `-uall --ignored`: git lists the files inside an ignored directory, gix emits
     the directory;
   - `status.showUntrackedFiles=false` (a boolean spelling git accepts): gix shows
     untracked files (lenient) or errors (strict); git shows none;
   - **sparse index**: the staged half fails with `IsSparse` (an `Err` item); the
     worktree half is right;
   - **split index written while sparse** (a sequence git allowed): gix decoded
     the wrong entries and reported a staged deletion of a file nobody deleted, with
     no error — the one *silent* wrong answer found;
   - staged renames use the same rename tracker as the diff-engine changes query,
     so its known divergence (an unsquared `limit`, no basename stage,
     `docs/research/diff-engine/rename-parity-spike.md`) applies to status too.
     Not re-measured here.
7. **Status never writes.** No write in the status path. The one write API,
   `status::Outcome::write_changes()`, has to be called by name. The index was
   byte-identical after every probe. The consequence is cost rather than
   correctness: stat-dirty entries are re-hashed on every call, because Cairn will
   never write the refresh back.
8. **`core.fsmonitor` and `core.untrackedCache` are ignored entirely.** gix never
   sets `FSMONITOR_VALID` and never queries the hook or daemon, and the untracked
   cache is decoded but unused. Results stay correct (it lstat's everything), but it
   is slower. Measured on a repo with both enabled: correct.
9. **gix status runs the user's clean filter driver as a child of Cairn's own
   process** (`gix-command`, inherited environment plus `GIT_DIR`/`GIT_WORK_TREE`/
   `GIT_NAMESPACE`), outside Cairn's `process/` runner, registry and kill. For D1
   and the process invariants this is a new route by which a program starts.
10. **Cancellation works at file and directory granularity, with gaps.** It is
    checked per index entry, per directory read, per tree-index change, and every
    25 ms in `Iter::next`. Three things are not interruptible: `index_from_tree`,
    rename tracking, and a nested submodule status, which has its own flag. Dropping
    the `Iter` with a caller-owned flag sets that flag and then **restores it**, so
    detached workers can keep running. Cairn must set its flag itself.
11. **Clean-tree cost:** bench (`rust`, 62,892 tracked files): gix status 52 ms
    in-process, `git status --porcelain=v2` 30 ms (both warm, clean). A large *dirty*
    tree is not measured (L6 still owes it).

## A. Refs

### A1. Enumeration

`Repository::references(&self) -> Result<reference::iter::Platform<'_>, reference::iter::Error>`
(`gix-0.87.1/src/repository/reference.rs:359`). The platform
(`gix-0.87.1/src/reference/iter.rs`) offers, each `-> Result<Iter<'_, 'repo>, init::Error>`:

| Method | Line | Prefix |
| --- | --- | --- |
| `all()` | 40 | everything under `refs/`, "excluding pseudo references" |
| `prefixed(prefix: impl TryInto<&RelativePath>)` | 47 | any; no trailing `/` means "starts with" (`refs/heads/foo` also matches `foobar`) |
| `tags()` | 57 | `refs/tags/` |
| `local_branches()` | 65 | `refs/heads/` (marked `// TODO: tests`) |
| `remote_branches()` | 82 | `refs/remotes/` (marked `// TODO: tests`) |
| `pseudo()` | 74 | files ending `HEAD` in the git dir root: `HEAD`, `FETCH_HEAD`, `ORIG_HEAD`, ... |

`Iter` yields `Result<gix::Reference<'r>, Box<dyn Error + Send + Sync>>`.
`Iter::peeled(self)` (line 99) peels each item before yielding it, sharing one
packed-refs snapshot. This is needed because the iterator holds the packed buffer,
so the consumer cannot peel items itself.

**Packed refs are merged.** `gix_ref::file::iter::LooseThenPacked`
(`gix-ref-0.67.1/src/store/file/overlay_iter.rs`) walks loose refs and the packed
buffer side by side in name order. On equal names the packed entry is dropped and
the loose one wins (the `Ordering::Equal` arm). A packed entry's `^` peeled line
becomes `Reference::peeled` (`gix-ref-0.67.1/src/raw.rs`,
`From<packed::Reference>`). gix does not use the header's `fully-peeled` trait, so
a packed branch with no `^` line is still peeled by looking its object up. That is
why `.peeled()` costs ~13x the unpeeled walk below.

**Ordering** is bytewise by full name. The loose walk sorts each directory with a
comparator that treats a directory as if suffixed `/`
(`gix-features-0.49.1/src/fs.rs:217` `walkdir_sorted_new`, `"common." < "common/"
< "common0"`). So the merge with the sorted packed list is a true merge. An
unsorted `packed-refs` (no `sorted` trait) is parsed whole and sorted in memory at
open. There, one bad line fails the whole `references()` call. With the trait, a
bad line is one `Err` item (`gix-ref-0.67.1/src/store/packed/buffer.rs:43`).

**Invalid names** (`bad..name`, `x.lock`) are skipped silently
(`gix-ref-0.67.1/src/store/file/loose/iter.rs:75`, `name_partial` check). git warns
`ignoring ref with broken name` and skips. Measured equal.

**Broken refs** come out as `Err` items and iteration continues: unparsable content
gives `ReferenceCreation`, a missing object gives `NotFound` when peeled, and a
dangling symref gives "Could not follow a single level of a symbolic reference"
when peeled. git skips all three (`git for-each-ref` printed `missingobj` too,
since it does not check objects without an object-reading format). A view needs a
rule for `Err` items. Note that the dangling-symref error **does not name the ref**.

**Cost** (release build, 10,501 refs, 10,001 packed with peeled lines + 500 loose):
`all()` 1.7 ms; `all().peeled()` ~22 ms. `git for-each-ref` was 23 ms as a process,
26 ms with `%(*objectname)`.

**Peeling** (`gix-0.87.1/src/reference/mod.rs`): `peel_to_id(&mut self)` (the
non-deprecated name; `peel_to_id_in_place` is `#[deprecated]`),
`peel_to_id_packed(Option<&packed::Buffer>)`, `into_fully_peeled_id(self)`,
`peel_to_kind(kind)`, `peel_to_commit()`, `peel_to_tag()`, `peel_to_tree()`,
`follow_to_object()`, `follow()`. All of them mutate `self`. Implementation:
`gix-ref-0.67.1/src/store/file/raw_ext.rs:147` (`peel_to_id_packed`). A cached
`peeled` value is used if present. Otherwise the code follows symrefs, then
dereferences tag objects until a non-tag. Afterwards **`target` holds the peeled
id, so the annotated tag's own object id is gone**. Read it with `target()` /
`try_id()` *before* peeling if a view needs it.

**Symbolic refs.** `follow_to_object_packed` does `*cursor = next` (line 216): the
whole `Reference`, *name included*, is replaced by the referent's. Depth is capped
at `MAX_REF_DEPTH = 5` (line 218) and a loop errors with `Cycle`. Consequence,
measured: under `.peeled()`, `refs/remotes/origin/HEAD` is yielded as
`refs/remotes/origin/main`, a duplicate. To decorate `origin/HEAD` or any symref,
iterate **unpeeled**. Read `target()`: it is `TargetRef::Symbolic(name)` for a
symref. Then peel a *clone*, or resolve the name separately.

**HEAD.** `Repository::head() -> Result<Head, ..>` (`reference.rs:187`). Its
`head::Kind` (`gix-0.87.1/src/head/mod.rs:13`) is one of three variants:

- `Symbolic(gix_ref::Reference)`: the branch exists;
- `Unborn(FullName)`: the referent does not exist yet;
- `Detached { target, peeled: Option<ObjectId> }`.

`Head` offers `is_detached()`, `is_unborn()`, `referent_name()`, `id()`,
`try_into_referent()`, plus `peel_to_commit()` and `into_peeled_id()` in
`head/peel.rs`. `head()` follows only one level. Helpers: `head_id`, `head_name`,
`head_ref`, `head_commit`, and `head_tree_id_or_empty` (the empty tree when unborn).
Measured: unborn and detached both report correctly. In a linked worktree, `head()`
is that worktree's HEAD.

### A2. Upstream and tracking

In `gix-0.87.1/src/repository/config/branch.rs`:

- `branch_remote_name(short_name, Direction) -> Option<remote::Name>`. Fetch:
  `branch.<n>.remote`. Push: `branch.<n>.pushRemote`, then `remote.pushDefault`,
  then `branch.<n>.remote`.
- `branch_remote_ref_name(&FullNameRef, Direction) -> Option<Result<FullName, _>>`.
  Fetch: `branch.<n>.merge`, made full if short. Push: from push refspecs or
  `push.default` (`nothing|current|matching|upstream|simple`).
- `branch_remote_tracking_ref_name(&FullNameRef, Direction) -> Option<Result<FullName, _>>`:
  maps the remote ref through the remote's **fetch** refspecs and takes the first
  match. Returns `None` when the remote has no fetch specs.
- `upstream_branch_and_remote_for_tracking_branch(&FullNameRef)`: the reverse map,
  which errors on ambiguity.
- The same functions are on `Reference`: `remote_name`, `remote`,
  `remote_ref_name` and `remote_tracking_ref_name` (`gix-0.87.1/src/reference/remote.rs`).

**Divergence, measured.** With `branch.feature.remote = .` and
`branch.feature.merge = refs/heads/main`, git's `feature@{upstream}` is `main`, but
gix's `branch_remote_tracking_ref_name` is `None`. Cause: `"."` parses as
`remote::Name::Url` (`gix-0.87.1/src/remote/name.rs:67`), which becomes an
anonymous remote with no fetch specs, which hits the `fetch_specs.is_empty()`
early return. Cairn must special-case `remote == "."`: the tracking ref is the
`merge` value itself.

**Ahead/behind: no API** (no `ahead`/`behind` anywhere in gix, gix-revision or
gix-traverse). The building blocks are `Repository::rev_walk(tips)` with
`Platform::with_hidden(tips)` (`gix-0.87.1/src/revision/walk.rs:261`):
`ahead = count(walk([local]).with_hidden([upstream]))`, and `behind` the other way
round. There is also `merge_base` / `merge_bases_many` (`repository/revision.rs`).
These walks cost what they traverse. On a branch far from its upstream that is
large, so they belong on a worker and must be bounded or interruptible like the
history walk.

### A3. Stashes

Nothing in gix or gix-ref names "stash" (grep). `refs/stash` is enumerated by
`all()` as a normal ref; `stash@{n}` is its reflog. `Reference::log_iter()`
returns a `gix_ref::file::log::iter::Platform`
(`gix-ref-0.67.1/src/store/file/log/iter.rs`):

- `rev() -> io::Result<Option<Reverse<File>>>` yields newest first, as
  `Result<log::Line, reverse::Error>` with `Line { previous_oid, new_oid,
  signature, message }`. **It resizes its buffer to 4096 bytes** (line 94), and a
  longer line ends in `"buffer too small for line size"` (line 219). After that
  error the iterator is exhausted. Measured: one stash pushed with a 5,000-character
  message made `rev()` fail on `stash@{0}` and yield nothing more.
- `all() -> io::Result<Option<Forward>>` yields oldest first, as `LineRef`, after
  reading the whole file. No line limit. **This is the safe read**: collect and
  reverse. Stash reflogs are short.

A stash entry is a commit whose first parent is the HEAD at stash time and whose
second parent is the index commit. A third parent, the untracked-files commit, is
present only for `-u`/`-a`. Measured with git 2.56.0: plain stash has 2 parents,
`stash -u` has 3, and `^3`'s subject is `untracked files on main: ...`. Its message
is `WIP on <branch>: <short> <subject>` or `On <branch>: <msg>`. gix reads these as
plain commits. Decoding parent 2/3 as "index" and "untracked" is Cairn's job.
**For the all-refs graph walk:** `refs/stash` is in `all()`. Walking it as a tip
brings the stash commit plus its index and untracked commits into the graph, and
`stash@{1..}` stay invisible. Fork-style inline stashes need the reflog list, and
the walk has to treat the side commits deliberately.

### A4. Worktrees and ref namespaces

`Repository::worktrees() -> io::Result<Vec<worktree::Proxy>>`
(`gix-0.87.1/src/repository/worktree.rs:46`) lists **linked** worktrees only, read
from `<common>/worktrees/*` where a `gitdir` file exists, sorted by private git
dir. The main worktree is not in it. `Proxy` (`src/worktree/proxy.rs`) offers
`id()`, `git_dir()`, `base()` (the checkout path, read from `gitdir`; may not
exist), `is_locked()`, `lock_reason()`, `into_repo()` (which fails if the checkout
is missing) and `into_repo_with_possibly_inaccessible_worktree()`. Nothing reports
git's "prunable" state. **Which branch each worktree has checked out:**
`checked_out_branches()` exists but is `pub(crate)` (line 26). Publicly, either
open each proxy as a repository and call `head()`, or, cheaper,
`repo.find_reference("worktrees/<id>/HEAD")`. That works through
`Category::LinkedPseudoRef`, resolved in
`gix-ref-0.67.1/src/store/file/find.rs` `to_base_dir_and_relative_name`; measured,
it returned `Symbolic(refs/heads/wt-branch)`. The main worktree's HEAD from a
linked worktree is `main-worktree/HEAD` (`Category::MainPseudoRef`).

Namespacing (`gix-ref-0.67.1/src/name.rs:31`, `Category::is_worktree_private`):
`refs/bisect/`, `refs/worktree/`, `refs/rewritten/` and pseudo refs are
per-worktree. `all()` in a linked worktree walks that worktree's private `refs/`
plus the common dir with private categories skipped
(`overlay_iter.rs:121 advance_to_non_private`). Measured: from the main worktree
it lists `refs/worktree/main-private`, and from the linked one it lists
`refs/worktree/mine` and not main's. This matches `git for-each-ref`. Reflogs
resolve the same way: `refs/stash` and branches go to the common dir, `HEAD` to
the worktree's git dir. One cosmetic difference: an error for a common ref seen
from a linked worktree names its path as `../../refs/heads/x`.

### A5. Edge cases against `git for-each-ref`

| Case | gix 0.87.1 | git 2.56.0 | Source / evidence |
| --- | --- | --- | --- |
| broken content | `Err(ReferenceCreation)`, continues | warns, skips | measured |
| invalid name | skipped silently | warns, skips | `loose/iter.rs:75`; measured |
| `*.lock` file | skipped silently | skipped | measured |
| missing object | `Ok` unpeeled; `Err(NotFound)` peeled | listed (default format) | measured |
| dangling symref | `Ok(Symbolic)` unpeeled; nameless `Err` peeled | silently skipped | measured |
| packed `^` lines | become `Reference::peeled` | used | `raw.rs` |
| unsorted packed-refs with a bad line | whole `references()` fails | `fatal` | `packed/buffer.rs:43` (git side from memory, OPEN) |
| `GIT_NAMESPACE` | honoured: env override of `gitoxide.core.refsNamespace` (`config/tree/sections/gitoxide.rs:155`, applied in `config/cache/init.rs` under the `git_prefix` env permission); names come back with the prefix stripped | honoured | source |
| reftable (`extensions.refStorage=reftable`) | **not supported**: opens; `head()` errors on `refs/heads/.invalid`, `all()` yields one `Err` for the `refs/heads` file | works | measured; no `refStorage`/`reftable` in gix, gix-ref or gix-discover (grep) |
| unknown `extensions.*` | not checked: only `objectFormat` and `worktreeConfig` are read; v0/v1 accepted, ≥2 refused (`config/cache/incubate.rs`) | v1 with an unknown extension is refused | source; git side from git's documented rule, OPEN |

**`GIT_NAMESPACE` asymmetry inside Cairn.** gix reads `GIT_NAMESPACE` from Cairn's
own process environment (Cairn opens at full trust,
`crates/cairn-git/src/repository.rs`, `default_for_level`). But the `INHERITED`
roster in `crates/cairn-git/src/process/environment.rs` does not pass it to `git`.
A Cairn launched with `GIT_NAMESPACE` set would show namespaced refs from gix and
un-namespaced refs from every `git` read and write. **OPEN:** decide whether to
strip it or pass it. It was not measured under Cairn's own open options.

## B. Status

### B6. The API at 0.87.1

`Repository::status<P: Progress>(&self, progress: P) -> Result<status::Platform<'_, P>, status::Error>`
(`gix-0.87.1/src/status/mod.rs:99`). Its defaults are documented as closest to
`git status --ignored=no`:

- HEAD tree vs index compared, with renames `AsConfigured`;
- index vs worktree compared, with no worktree renames and no sorting;
- dirwalk on, with untracked from `status.showUntrackedFiles` (default
  `Collapsed`) and ignored not emitted;
- submodules `AsConfigured { check_dirty: false }`;
- no interrupt flag, so a private one is used.

Builder (`src/status/platform.rs`), all `self -> Self` unless noted:

| Method | Effect |
| --- | --- |
| `untracked_files(UntrackedFiles)` | `None` (dirwalk off) / `Collapsed` (`EmissionMode::CollapseDirectory`) / `Files` (`EmissionMode::Matching`) |
| `dirwalk_options(FnOnce(dirwalk::Options) -> dirwalk::Options)`, `dirwalk_options_mut(&mut self, ..)` | full dirwalk control; no-op once the dirwalk is off |
| `index_worktree_rewrites(impl Into<Option<gix_diff::Rewrites>>)` | worktree rename/copy tracking (deleted-tracked ↔ untracked); turns on `ByPathCaseSensitive` sorting |
| `index_worktree_submodules(impl Into<Option<Submodule>>)` | `None` = `Given { ignore: All }` |
| `index_worktree_options_mut(FnOnce(&mut index_worktree::Options))` | `sorting`, `dirwalk_options`, `rewrites`, `thread_limit` |
| `tree_index_track_renames(tree_index::TrackRenames)` | `AsConfigured` (status.renames → diff.renames → on) / `Given(Rewrites)` / `Disabled` |
| `head_tree(impl Into<ObjectId>)` | compare the index against another tree |
| `index(worktree::IndexPersistedOrInMemory)` | supply the index |
| `should_interrupt_shared(&'static AtomicBool)`, `should_interrupt_owned(Arc<AtomicBool>)` | cancellation (B9) |

`status::Submodule` is one of two variants:

- `AsConfigured { check_dirty: bool }`: `diff.ignoreSubmodules` if set, otherwise
  the submodule's own `ignore`;
- `Given { ignore: submodule::config::Ignore, check_dirty: bool }`.

Terminal calls:

- `into_iter(patterns: impl IntoIterator<Item = BString>) -> Result<status::Iter, into_iter::Error>`
  (`src/status/iter/mod.rs:40`) returns both halves, **in undefined order**.
- `into_index_worktree_iter(patterns) -> Result<index_worktree::Iter, ..>` returns
  the worktree half only.
- `Repository::is_dirty()` excludes untracked files.
- The lower-level calls `Repository::tree_index_status(..)` and
  `Repository::index_worktree_status(..)` are available directly.

`status::Iter` yields `Result<status::Item, status::iter::Error>`. When exhausted,
`into_outcome()` / `outcome_mut()` give `status::Outcome`, holding
`index_worktree` (gix-status's counters), `tree_index: Option<tree_index::Outcome>`,
`worktree_index`, `has_changes()` and `write_changes()`.

`status::Item` (`src/status/iter/types.rs:54`):

- `TreeIndex(gix_diff::index::Change)`: HEAD tree vs index, i.e. staged. Its
  variants are `Addition { location, index, entry_mode, id }`,
  `Deletion { location, index, entry_mode, id }`,
  `Modification { location, previous_index, previous_entry_mode, previous_id, index, entry_mode, id }`
  and `Rewrite { source_location, source_index, source_entry_mode, source_id, location, index, entry_mode, id, copy }`
  (`gix-diff-0.67.1/src/index/mod.rs`). There is **no similarity score**; git's
  `R99` is not carried.
- `IndexWorktree(index_worktree::Item)` (`src/status/index_worktree.rs`), one of:
  - `Modification { entry: gix_index::Entry, entry_index, rela_path, status: EntryStatus<(), submodule::Status> }`;
  - `DirectoryContents { entry: gix_dir::Entry, collapsed_directory_status: Option<gix_dir::entry::Status> }`,
    which covers untracked, ignored and the rest;
  - `Rewrite { source: RewriteSource, dirwalk_entry, dirwalk_entry_collapsed_directory_status, dirwalk_entry_id, diff: Option<DiffLineStats>, copy }`,
    present only with `index_worktree_rewrites`. Here `RewriteSource` is
    `RewriteFromIndex { source_entry, source_entry_index, source_rela_path, source_status }`
    or `CopyFromDirectoryEntry { .. }`. `Item::summary()` collapses an item to
    `gix_status::index_as_worktree_with_renames::Summary`.

`gix_status::index_as_worktree::EntryStatus<T, U>`
(`gix-status-0.34.1/src/index_as_worktree/types.rs`):

- `Conflict { summary: Conflict, entries: Box<[Option<ConflictIndexEntry>; 3]> }`:
  stages 1 to 3 at indices 0 to 2;
- `Change(Change<T, U>)`;
- `NeedsUpdate(Stat)`: **swallowed by `status::Iter`, never yielded**
  (`maybe_keep_index_change`);
- `IntentToAdd`.

`Change` is one of:

- `Removed`;
- `Type { worktree_mode }`;
- `Modification { executable_bit_changed: bool, content_change: Option<T>, set_entry_stat_size_zero: bool }`;
- `SubmoduleModification(U)`.

`Conflict` is `BothDeleted | AddedByUs | DeletedByThem | AddedByThem | DeletedByUs | BothAdded | BothModified`.

`gix_dir::Entry` (`gix-dir-0.29.1/src/lib.rs:47`) has fields `rela_path`, `status`,
`property`, `disk_kind`, `index_kind` and `pathspec_match`:

- `status` is `entry::Status::{Pruned, Tracked, Ignored(gix_ignore::Kind::{Expendable, Precious}), Untracked}`;
- `property` is `Option<Property::{DotGit, EmptyDirectory, EmptyDirectoryAndCWD, TrackedExcluded}>`;
- `disk_kind` is `Option<Kind::{Untrackable, File, Symlink, Directory, Repository}>`.

`submodule::Status` (`src/submodule/mod.rs:545`) holds `state` (whether the
repository exists, whether it is old-form, whether the worktree is checked out,
whether the superproject has configuration), plus `index_id`,
`checked_out_head_id` and `changes: Option<Vec<status::Item>>`. Its `is_dirty()`
returns `Option<bool>`. git's porcelain-v2 `S<c><m><u>` is derivable: `c` is
`index_id != checked_out_head_id`, while `m` and `u` come from the item kinds in
`changes`.

### B7. git's dimensions, one by one

Status values: **measured-equal**, **measured-divergent**, **source**, **OPEN**.

| Dimension | gix behaviour | Status |
| --- | --- | --- |
| staged (HEAD↔index) | `tree_index_status`: builds an index from the HEAD tree every call (`index_from_tree`, no cache-tree) and diffs (`gix-diff-0.67.1/src/index/function.rs`); unborn HEAD becomes the empty tree | measured-equal |
| staged renames | on by default (`TrackRenames::AsConfigured`: `status.renames`, then `diff.renames`, then on; `status.renameLimit`; `copies` honoured; `src/status/tree_index.rs`); simple and edited renames measured equal; **the tracker is the diff-engine's, with its known large-change divergence** | measured-equal on small cases; large case divergent per `rename-parity-spike.md` |
| unstaged (index↔worktree) | `gix_status::index_as_worktree` (`function.rs`) | measured-equal |
| index↔worktree renames | off by default; when on, pairs a removed tracked file with an *untracked* file, which git never does. git pairs a removed file with an **intent-to-add** entry (`.R`); gix never does | measured-divergent (ITA case); keep worktree rewrites off |
| untracked, `normal` | `CollapseDirectory`; empty dirs not emitted | measured-equal |
| untracked, `all` | `Matching` | measured-equal |
| untracked, `no` | dirwalk off | source |
| `status.showUntrackedFiles` | honoured for `no`/`normal`/`all` only (`config/tree/sections/status.rs:32`); a boolean (`true`/`false`, which git accepts) errors when strict and falls to `normal` when lenient | measured-divergent (`false`) |
| ignored, with `-unormal` | `emit_ignored(Some(CollapseDirectory))` **plus** `emit_collapsed(Some(OnStatusMismatch))` reproduces `--ignored` (traditional and matching agreed on this fixture); without `emit_collapsed`, an ignored file inside an untracked dir is lost | measured-equal with both options |
| ignored, with `-uall` | an ignored *directory* is emitted whole (`igdir`); git lists its files (`igdir/z`) | measured-divergent |
| `--ignored=matching` vs `traditional` | no exact mode mapping; differences between git's two modes not exercised | OPEN |
| conflicts | stage bitmask to `Conflict` (`function.rs` `Conflict::try_from_entry`): 001 DD, 010 AU, 011 UD, 100 UA, 101 DU, 110 AA, 111 UU. All seven measured equal (built with `update-index --index-info`). Unmerged and ITA entries are excluded from the staged half (`ignore_unmerged_and_intent_to_add`, line 279), matching git's `u` lines | measured-equal |
| conflicts, dirwalk | a conflicted path **without a stage-2 entry** (DU, UA) whose file exists is *also* emitted `Untracked`: `gix-dir` classifies through `State::entry_by_path`, which looks at stage 0 or 2 only (`gix-index-0.55.0/src/access/mod.rs:353`, `gix-dir-0.29.1/src/walk/classify.rs:423`) | measured-divergent: filter dirwalk entries whose path is conflicted |
| intent-to-add | `EntryStatus::IntentToAdd`, with no content check (`function.rs`, after the directory check) | measured-equal (`.A`) |
| type change | `Change::Type { worktree_mode }`; documented deviation: a change to a *non-file* (fifo and so on) is `Type` in gix but a modification in git | file↔symlink measured-equal; non-file OPEN |
| exec bit / `core.fileMode` | `executable_bit_changed`; `core.fileMode` read (`config/cache/access.rs` `fs_capabilities`, default true) | measured-equal both ways |
| symlinks / `core.symlinks` | read; a real symlink whose entry is a link is compared via `readlink` even if `core.symlinks=false` | source; `core.symlinks=false` OPEN |
| `core.ignoreCase` | read; makes dirwalk and pathspec icase and builds `prepare_icase_backing()`; a documented shortcoming for `d` vs `D/a` (`classify.rs`) | OPEN (not measurable on a case-sensitive fs without contrivance) |
| `core.precomposeUnicode` | read | OPEN |
| CRLF / `core.autocrlf` / `core.eol` / `text`/`eol` attrs | filter pipeline options from config (`gix-0.87.1/src/filter.rs` `Pipeline::options`) | autocrlf measured-equal |
| clean filter driver | **runs.** The worktree side goes through `gix_filter::Pipeline::convert_to_git` (`gix-status-0.34.1/src/index_as_worktree/function.rs:601` `stream_worktree_file`); drivers come from `filter.<name>.clean/process/required` (`gix-0.87.1/src/filter.rs:291` `extract_drivers`). It runs only when stat is dirty or racy **and** the sizes match (B7a). A long-running process is offered the `clean`, `smudge` and `delay` capabilities (`gix-filter-0.34.0/src/driver/init.rs:44`); a non-required driver that fails falls back to unfiltered input (`driver/apply.rs`). See B7b for where it runs | measured: the driver ran once and the file was correctly unmodified. **The roadmap claim holds.** |
| racy git / stat | `Stat::matches` + `is_racy` (`gix-index-0.55.0/src/entry/stat.rs`); `core.trustCTime` and `core.checkStat` read; **nanoseconds off by default** (`gitoxide.core.useNsec=false`, git on Linux uses them). Racy entries are hashed, so the result is right and only the cost differs | source |
| index refresh / write | never; see B8 | measured |
| `skip-worktree` / `assume-unchanged` | entries with `UPTODATE`, `SKIP_WORKTREE`, `ASSUME_VALID` or `FSMONITOR_VALID` are skipped (`function.rs:278`) | measured-equal (both hidden). OPEN: git ≥2.37 clears `SKIP_WORKTREE` in memory for files present on disk under sparse checkout; gix does not |
| sparse checkout, full index | correct | measured-equal |
| sparse index (`index.sparse`) | decoded (`gix-index` `is_sparse()`); dir entries are `SKIP_WORKTREE` so the worktree half is right; **the staged half returns `Err(IsSparse)`** (`gix-diff-0.67.1/src/index/function.rs:42`), yielded as an `Err` item | measured-divergent (the staged half is missing) |
| split index | link extension dissolved at open (`gix-index-0.55.0/src/file/init.rs:99`) | measured-equal in a plain split index; **silently wrong** in one repo whose split index was written alongside a sparse index (it reported `D. d3/f1` where git reported `D. top2`; `open_index()` decoded `d3/f1` absent and `top2` present). Root cause OPEN |
| `core.untrackedCache` | the `UNTR` extension is decoded (`extension/untracked_cache.rs`) but nothing in gix-dir, gix-status or gix uses it (grep) | measured-equal; slower |
| `core.fsmonitor` | never consulted. The `FSMN` extension is decoded and unused (`extension/fs_monitor.rs`: `#[expect(dead_code, reason = "for the time when we actually support the git status daemon")]`), and `FSMONITOR_VALID` is never set by decode (`entry/flags.rs` at-rest mapping). So every entry is lstat'ed: **correct but slow, never stale** | measured-equal with the builtin daemon running |
| `core.preloadIndex` / parallelism | not read; the modification check is always parallel (chunks of 500, "just like git"; capped by `index_worktree::Options::thread_limit`). `index.threads` is read, but only for index *decoding* (`src/repository/index.rs` `open_index`) | source |
| submodules | `BuiltinSubmoduleStatus`: per submodule, opens it and runs a **nested full status** (`src/submodule/mod.rs` `status_opts`); `diff.ignoreSubmodules` overrides the per-submodule `ignore`; `ignore=all|dirty|untracked` measured equal; inactive submodules are still included (documented) | measured-equal |
| nested untracked repository | emitted as one untracked dir (`recurse_repositories=false`, `Kind::Repository`) | source |
| excludes | `.gitignore`, `.git/info/exclude`, `$XDG_CONFIG_HOME/git/ignore`, `core.excludesFile` | measured-equal |

**B7a. `FastEq` and size.** The default comparator
(`gix-status-0.34.1/src/index_as_worktree/traits.rs`, `FastEq`) reports a change
*without reading the file* when the stat size differs from the worktree size, and
otherwise hashes the filtered stream. git reports such a file modified too.
Measured: a CRLF rewrite under `autocrlf=true` showed `.M` in both, where a content
comparison would have found no change.

**B7b. Where the filter runs.** The filter pipeline is built by
`gix::diff::resource_cache(.., Mode::ToGit, ..)` from `repo.command_context()`
(`gix-0.87.1/src/repository/config/mod.rs:161`). `gix-command` spawns the driver
(`gix-command-0.10.1/src/prepare.rs:255-290`). It runs in Cairn's **inherited
environment** with no `env_clear`, plus `GIT_DIR`, `GIT_WORK_TREE`,
`GIT_NO_REPLACE_OBJECTS`, `GIT_NAMESPACE` and the pathspec variables, as a direct
child of the Cairn process on a gix worker thread. Compared with today's
working-tree read through `git diff-files`, the environment differs. There is no
`GIT_OPTIONAL_LOCKS`, no askpass pinning and none of the `ALWAYS` table, and git's
exec-path and `GIT_PREFIX` are absent. The process is not in the repository's
registry or command log, and cancelling the status does not end it. A
long-running `filter.<d>.process` such as git-lfs lives as long as the pipeline.
OPEN: whether it is reaped when the pipeline drops.

### B8. Does status write?

No. A grep of `gix-status`, `gix-dir`, `gix/src/status`, `gix/src/submodule`,
`gix/src/dirwalk` and the index-open path for `.write(`, `fs::write`,
`File::create` and `OpenOptions` finds one production hit, `Outcome::write_changes()`
(`src/status/iter/types.rs:87`). It must be called explicitly and writes `.git/index`
(with `index.skipHash`). `NeedsUpdate` stat refreshes and racy `size = 0` markers
are collected in memory only. `open_index()` and `index_or_empty()` read only. A
split index is dissolved in memory. Measured: the index file was byte-identical
before and after every probe run, including fixtures with stat-dirty, racy,
filtered, submodule, split-index and fsmonitor entries. The submodule nested status
also never writes, since it uses the same code. The cost of never writing: entries
git would refresh, such as a checkout by another tool or a `touch`, are re-hashed
on every status call. git status refreshes the index unless `GIT_OPTIONAL_LOCKS=0`.

One caveat for the "a read writes nothing" rule: a **filter driver** gix starts
can write its own state (B7b). This is the same residual
`docs/design/engine.md` already states for git's own reads.

### B9. Cancellation

- The flag is `should_interrupt_shared(&'static AtomicBool)` or
  `should_interrupt_owned(Arc<AtomicBool>)`. The default is a private flag.
- Index↔worktree: checked per chunk (`gix_features::interrupt::Iter`) **and per
  entry** inside a chunk (`function.rs`, `should_interrupt.load` in the chunk loop).
  The hashing of one large file is not interruptible (`// TODO: make all streaming
  IOPs interruptible`, `traits.rs`).
- Dirwalk: checked before each directory is read, and aborts with an error
  (`gix-dir-0.29.1/src/walk/readdir.rs:36`).
- Tree↔index: checked after each emitted change. **`index_from_tree` is not
  interruptible**; the source comment says "can go up to 500ms" on big repositories
  (`src/status/iter/mod.rs`, near line 239). Rename tracking (`Tracker::emit`) is
  not checked.
- `Iter::next` polls `recv_timeout(25 ms)` and returns `None` once the flag is set
  (line 239). So a cancelled status ends the iteration silently: `None`, not an
  error. Check the flag or `into_outcome()` to tell "done" from "cancelled".
- Submodules: the nested `sm_repo.status(..)` is built **without** the parent's
  flag (`src/submodule/mod.rs` `status_opts`), so each submodule's status runs to
  completion.
- **Drop:** `parallel_iter_drop` (`gix-0.87.1/src/util.rs:58`) swaps the flag to
  `true`, detaches the producer threads without joining them, then **restores the
  previous value** for a caller-owned flag. Workers that miss the brief `true` keep
  lstat'ing and hashing in the background, and the worktree producer ignores send
  failures (`Collect::visit_entry` does `tx.send(item).ok()`). Cairn should own an
  `Arc<AtomicBool>`, set it, and leave it set. The epoch-as-cancel model fits this,
  provided the flag is the epoch's and stays raised.
- Threads per call: `gix::status::tree_index::producer` and
  `gix::status::index_worktree::producer`; inside them, scoped threads named
  `gix_status::dirwalk` and `gix_status::index_as_worktree`; and the parallel
  modification check, up to one thread per core. A nested status per submodule adds
  more. A worker panic propagates (`join().expect("no panic")` in `Iter::next`).

### B10. Feature flags

`cargo tree -e features -i gix` over `cairn-git` resolves to: `attributes`,
`auto-chain-error`, `basic`, `blame`, `blob-diff`, `comfort`, `command`,
`credentials`, `default`, `dirwalk`, `excludes`, `extras`, `gix-archive`,
`gix-status`, `gix-worktree-stream`, `index`, `interrupt`, `mailmap`,
`max-control`, `max-performance(-safe)`, `merge`, `notes`, `pack-cache-lru-*`,
`parallel`, `regex`, `revision`, `revparse-regex`, `sha1`, `sha256`, `status`,
`worktree-archive`, `worktree-mutation`, `worktree-stream`.

From `gix-0.87.1/Cargo.toml`:

| Feature | Pulls in |
| --- | --- |
| `status` | `gix-status`, `dirwalk`, `index`, `blob-diff`, `gix-diff/index` |
| `dirwalk` | `gix-dir`, `attributes`, `excludes` |
| `attributes` | `gix-filter`, `gix-pathspec`, `gix-attributes`, `gix-submodule`, `command` |
| `excludes` | `gix-ignore`, `gix-worktree`, `index` |

`parallel` turns on threading in gix-features, gix-filter, gix-worktree and the
rest. Without it, `status::Iter` computes everything up front and is not
interruptible (`src/status/iter/types.rs` docs). **Everything status needs is on**,
explicitly and through `default` → `extras`. `interrupt` only compiles the
signal-handler helpers; nothing installs them unless asked. `worktree-mutation`
(checkout) is compiled because `extras` includes it. That is relevant to the
gitoxide mutation roster in `crates/cairn-guards/src/lib.rs`, not to status.

## C. Timing

Release build, warm, median-ish of 3:

- 10,501 refs: `all()` 1.7 ms, `all().peeled()` ~22 ms, `git for-each-ref` 23 ms
  (process). gix's clean-repo status on that one-file repository took 0.4 ms.
- `~/Development/bench/rust` (62,892 tracked files, clean, no fsmonitor or
  untracked cache configured): gix `status().into_iter(None)` 52 ms in-process;
  `GIT_OPTIONAL_LOCKS=0 git status --porcelain=v2` 30 ms as a process.

Not measured: a large dirty tree, a tree with many stat-dirty entries (where gix's
never-write policy re-hashes every call), and fsmonitor-accelerated git on the bench.

## OPEN

- Root cause and frequency of the split-plus-sparse misdecode. Only one
  repository's history produced it; a fresh split→sparse sequence did not.
- `--ignored=matching` vs `traditional` differences beyond the fixture.
- `core.ignoreCase`, `core.precomposeUnicode`, `core.symlinks=false`, non-file
  type changes, `status.renames=copies` in status.
- The skip-worktree-but-present case under sparse checkout (git ≥2.37 behaviour).
- `GIT_NAMESPACE` under Cairn's own open options and environment roster.
- Whether a long-running filter process gix started is waited on or orphaned when
  the pipeline drops.
- git's handling of an unsorted packed-refs file with a bad line, and of unknown
  v1 extensions, was not measured.
