# Refs

How the engine reads a repository's refs: the refs snapshot, the five rules that make
gix's answer git's, each local branch's upstream, the stash list, ahead and behind,
and the repositories refused at open because their refs are not files. Nothing here
writes, and nothing here starts a process: every answer is gix's, read in process.
No view draws any of it yet.

Spec: `docs/prd/refs-and-status.md` (R1, R2). Evidence:
`docs/research/refs-and-status/gix-refs-and-status-api.md`.

## The snapshot

`Repository::refs(&cancel)` (`crates/cairn-git/src/refs.rs`) answers a `RefsRead`: a
`cairn_model::RefsSnapshot` and the `RefsCost` it paid. The snapshot is plain data
(`crates/cairn-model/src/refs.rs`):

- `refs` — every local branch, remote-tracking ref and tag, as `Ref`s, in
  `git for-each-ref`'s order (bytewise by full name, which puts `refs/heads/` before
  `refs/remotes/` before `refs/tags/`). A `Ref` carries its full name, its `RefKind`, its
  `RefTarget`, the ref at the end of its chain when it is symbolic (`%(symref)`), and —
  for a local branch — its `Upstream`. `RefsSnapshot::find` is a binary search on that order.
- `RefTarget` is what the ref names, as `%(objectname)` and `%(objecttype)` print it:
  `Commit(id)`; `Tag { object, commit }`, the outermost tag object of a chain and the
  commit the chain ends at (`None` when it ends at a tree or a blob); or `Other(id)`, a
  tree or blob named directly. For a symbolic ref it is what the target names.
- `head` — `HeadState::Branch(name)`, `Detached(id)` or `Unborn(name)`, as
  `git symbolic-ref HEAD` and `git rev-parse HEAD` answer: the branch is the end of
  `HEAD`'s symbolic chain, followed to git's depth, and unborn when that end does not
  exist (gix's `head()` follows one level, and would call `HEAD` → a dangling symbolic
  branch a branch). In a linked worktree it is that worktree's own `HEAD`.
- `stashes` — the stash list, newest first (below).
- `unreadable` — refs skipped because they could not be read.

The query reads the refs unpeeled through gix's `local_branches()`, `remote_branches()`
and `tags()` iterators (loose and packed merged, a loose ref over its packed self, an
invalid name or a `.lock` file skipped as git skips it), then resolves each ref itself:
a symbolic chain is followed by name to git's depth — at most five refs read, the ref
itself included, so four symbolic hops — and the ref at its end is named; the object at
the end is told apart by its header, and a tag is peeled through a packed ref's `^` line where it
has one, as git reads it, or object by object to the end of its chain. `cancel` is
polled once per ref the store yields, once per local branch before its upstream is
resolved, and once per stash reflog line and stash entry; a cancelled query is
`Error::RefsCancelled`, and polls nothing after.
`RefsCost` counts the ref entries read, the objects looked up, the reflog lines read and
the time taken. The query fails as a whole (`Error::Refs`) only when the store cannot be
listed or `HEAD` cannot be read.

A refresh (`docs/systems/history-graph.md`, "Refresh") reads the snapshot again — on
focus, after a fetch, on the Refresh action — and compares it with the one the history
was walked from: `RefsSnapshot::walks_as` says whether the two would draw the same
history — the same refs naming the same objects, a symbolic ref's target among them, the
same `HEAD` and the same stash list — leaving out an upstream's configuration and the
unreadable count, which the graph does not draw (`a_walk_is_the_same_unless_what_it_draws_changed`).
Comparing snapshots sees a symbolic ref retargeted and a tag object replaced on the same
commit, which comparing peeled ids did not. It sees only what the snapshot holds, so a fetch
that moves only a ref outside those namespaces (`refs/notes/`, `refs/pull/`,
`refs/replace/`) reopens nothing. (The network lane's own before-and-after comparison,
`Repository::ref_tips`, is gone with phase 06.)

`RefsSnapshot::matching` is the sidebar's filter (R8.3): the refs whose name past its
namespace, and the stashes whose message, hold the text, case ignored as the Changes tab's
file filter ignores it (`src/text_filter.rs`, shared by both)
(`the_sidebar_filter_keeps_the_names_that_hold_its_text`). `RefsSnapshot::sidebar_rows`
lays the sidebar's rows out from what it keeps — sections, folders split at `/`, what is
open — in one pass over every ref, run on the repository thread in a lane of its own
(`docs/systems/sidebar.md`).

## The five parity rules

gix agrees with `git for-each-ref` once it is read under five rules, each pinned against
git run in the same fixture at test time (`crates/cairn-git/tests/refs.rs`):

1. **A symbolic ref is never peeled into its target's name.** gix's peeled iteration
   replaces `refs/remotes/origin/HEAD` with a second `refs/remotes/origin/main`; the
   snapshot reads unpeeled and lists `origin/HEAD` as itself, naming `origin/main`.
2. **A dangling symbolic ref is hidden**, at any depth, as git hides it — not counted as
   unreadable. A chain past git's depth is hidden by git too, silently; it is counted as
   unreadable here.
3. **The stash reflog is read oldest first, then reversed** (`refs/stash.rs`): gix's
   newest-first reader reads through a 4 KiB window and stops for good at a longer line.
4. **`remote = .` is resolved by hand**: gix answers no upstream for a local one.
5. **Every upstream is resolved by hand from the configuration** (`refs/upstream.rs`),
   as git's `set_merge` and `query_refspecs` resolve it, because gix takes the last
   `branch.<name>.merge` where git takes the first, expands and maps a short `merge`
   (`main`) given a named remote where git maps nothing, and of two refspecs both mapping
   the merge does not take the first. The rule: the last `branch.<name>.remote` and the
   first `branch.<name>.merge`, neither present, no upstream; with `remote = .`, the merge
   resolved as git resolves a name it is given (`refs/`, `refs/tags/`, `refs/heads/`,
   `refs/remotes/`, `refs/remotes/<name>/HEAD`), kept as written when it names no ref or
   more than one (or, with `core.warnAmbiguousRefs` off, the first it names); with any
   other remote, the merge as written matched against `remote.<remote>.fetch` in
   configuration order — a negative refspec, or one with no destination, skipped; a
   pattern matching by its prefix and suffix with the middle carried into the
   destination's `*` — the first match's destination the upstream, no match no upstream.
   The upstream is `Upstream::Exists { name, commit }` when that ref resolves, and
   `Upstream::Gone { name }` — git's `[gone]` — when it does not.

`GIT_NAMESPACE` is not honoured: gix reads it from Cairn's own environment at open, so
`SharedRepository::discover_as` clears the namespace on the repository every handle is
made from, and the refs shown are the ones the `git` Cairn runs (which is never given
the variable) sees. gix restores it whenever its configuration is re-read in place
(`config_snapshot_mut`), which nothing in Cairn does.

**Skipped and counted.** A ref whose content is not a ref, or whose symbolic chain runs
past git's depth, is skipped and counted in `unreadable`; git skips it too, warning
`ignoring broken ref`. A ref naming an object that is not there is skipped and counted
the same way. That one is a deliberate divergence, the user's decision: git's own
`for-each-ref` refuses to list anything at all (`fatal: missing object`), which would
leave a window with no refs; `a_ref_naming_a_missing_object_is_skipped_and_counted`
pins both halves, and fails if git ever starts listing such a ref.

**Residuals.** A ref name git skips as invalid (`bad..name`) is skipped silently, as gix
skips it, and not counted in `unreadable`, though git warns about it — an open product
question. A remote defined by a file git reads in place of configuration
(`$GIT_DIR/remotes/<name>`, `$GIT_DIR/branches/<name>`) has fetch refspecs git reads and
the upstream rule does not, so a branch tracking one has no upstream here (fetch refuses
such a remote on sight). The first of git's name-resolution rules, the name itself, is
tried only for a name under `refs/`, so a local `merge` naming a root ref such as `HEAD`
is taken as written. A ref name that is not UTF-8 is held lossily (`RefName` is a
`String`).

## The stash list

`StashEntry { index, message, commit, base }`: `stash@{index}` (`0` the newest), the
reflog's message as `git stash list`'s `%gs` prints it, the stash commit, and the commit
it was made on — the stash commit's first parent. The `refs/stash` reflog is read whole,
oldest first, and reversed (rule 3), and numbered as `git stash list` numbers it (both
measured): a line that does not parse is skipped, counted, and takes no number, so the
entries older than it move up a place; an entry whose commit cannot be read is skipped
and counted but keeps its number.

## Ahead and behind

`Repository::ahead_behind(&snapshot, &cancel)` (`crates/cairn-git/src/ahead_behind.rs`)
answers an `AheadBehindRead`: for each local branch of the snapshot whose upstream
exists and identifies a commit, in the snapshot's order, a `cairn_model::AheadBehind {
ahead, behind }` — what `git rev-list --left-right --count <branch>...<upstream>`
prints — and the commits read and time taken. A branch with no upstream or a gone one
has no entry; a branch equal to its upstream is `0, 0` without walking. It is a query of
its own, asked over a snapshot already answered, so a long divergence never holds up the
refs.

Each count is a gix walk from one side that hides the other
(`history::walk::hiding`, `gix::traverse::commit::Simple::hide`), over the commits git's
walk sees — a shallow clone's boundary grafted, as the history walk grafts it. Every
object read goes through `Polled`, which fails once `cancel` says so: the frontier gix
paints before a hiding walk's first commit is one long call, and a refused read is what
stops it. So no commit-graph is used (a commit read from one never reaches `Polled`),
though git would use one. `cancel` is also polled before each branch and after each
commit counted; a cancelled query is `Error::AheadBehindCancelled { branches,
commits_read }` — the branches answered and the commits read before it stopped — and what
it had counted is not answered.

## Refused at open

`SharedRepository::discover_as`, after gix opens the repository and before the snapshot or
anything Cairn runs reads a ref (gix's open itself reads `HEAD`, a placeholder in a
reftable repository that it reads without failing), reads the repository format as git
does — from the common directory's `config`, no include followed, a missing file read as
format 0 with no extension, as git and gix open it (`crates/cairn-git/src/ref_storage.rs`)
— and refuses:

- `Error::RefStorageUnsupported { path, storage }` when `extensions.refStorage` names
  anything but `files`. gix 0.87 reads no such setting; it would open a reftable
  repository and fail on the first ref it read.
- `Error::RefStorageNeedsFormatVersion1 { path, storage }` when
  `core.repositoryFormatVersion` is 0 and `extensions.refStorage` is set at all, `files`
  or `reftable`, as git refuses it ("repo version is 0, but v1-only extension found");
  the version is decided before the storage.

The window draws either reason as it draws every failed open.

## Pins

- C1, against git in the same fixture: `the_snapshot_is_what_git_lists_whatever_the_kind_of_ref_and_head`
  (a packed chain of tags and a loose one of three, peeled object by object),
  `the_snapshot_is_gits_over_a_braided_history_in_either_hash`,
  `a_linked_worktree_lists_what_git_lists_there_with_its_own_head`,
  `the_stash_list_is_git_stash_lists_with_forty_entries_and_a_long_message`,
  `a_damaged_stash_reflog_is_numbered_as_git_numbers_it`,
  `each_upstream_is_what_git_resolves`,
  `a_symbolic_chain_resolves_to_gits_depth_and_names_its_end`,
  `head_through_a_symbolic_branch_is_what_git_says`,
  `git_namespace_in_cairns_environment_is_not_honoured`,
  `a_ref_naming_a_missing_object_is_skipped_and_counted`,
  `a_refresh_sees_a_symbolic_ref_retargeted_and_a_tag_object_replaced`,
  `a_refs_query_stops_at_whichever_poll_is_cancelled`,
  `a_refs_query_reports_the_cost_it_paid`; and in the crate,
  `a_refspec_maps_the_merge_literally_as_git_maps_it`. Cairn's side reads the
  developer's global configuration and git's side does not, so each fixture sets
  again, locally, the keys the snapshot reads that a global file could set
  (`pin_configuration_read`).
- C3: `ahead_and_behind_are_what_rev_list_counts` (a merge on each side),
  `ahead_and_behind_are_what_rev_list_counts_when_dates_run_backwards`,
  `ahead_and_behind_are_what_rev_list_counts_in_a_shallow_clone`, and
  `a_cancelled_ahead_behind_stops_its_walk`, which cancels inside the frontier paint
  and requires it to stop reading there, and cancels between two branches.
- C2: `a_reftable_repository_is_refused_at_open_and_a_files_one_opens` (a linked worktree
  of the reftable repository too) and `the_ref_storage_setting_is_read_as_git_reads_it`
  (both version-0 cases and a missing `config`, against git refusing or opening the same
  fixture), each skipped — wholly, or git's half — where the host's git cannot make a
  reftable repository and required where it can: `CAIRN_REQUIRE_REFTABLE`, set by
  `scripts/gate.sh`'s `require_reftable_where_possible`, pinned by
  `the_reftable_refusal_is_required_wherever_it_can_run` (matcher
  `required_skip_violations`, self-test
  `the_required_skip_matcher_catches_the_shapes_it_claims`); and in the crate
  `the_storage_is_read_from_the_common_directorys_own_file` and
  `an_included_file_does_not_set_the_storage`.
- Cost: `measures_the_refs_snapshot_and_ahead_behind`, `#[ignore]`d, driven by
  `CAIRN_BENCH_REPO`.
