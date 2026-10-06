# Status agreement: gix 0.87.1 against git status, measured

Research for the `refs-and-status` packet's open question O2 — does working-tree
status come from `gix` (0.87.1, `status` feature) or from `git status --porcelain=v2
-z`? — recorded 2026-10-05. It decides nothing: it measures agreement and cost, and
says where each difference comes from. Evidence record, saved in full; historical,
never retro-edited.

## Headlines

- **gix and git agree on 26 of the 38 fixtures in every mode**, including every
  plain change, type change, exec bit (with `core.fileMode` true and false),
  `core.autocrlf`/`eol`/`text` renormalisation, a clean filter driver, git-lfs's
  long-running filter, racy git, skip-worktree/assume-unchanged, split index, index
  v4, `index.skipHash`, untracked cache, the fsmonitor daemon (when it is accurate),
  `core.ignoreCase`, unborn and detached HEAD, sha256, a linked worktree, and
  awkward file names. On the rust-lang/rust scratch copy they agree line for line in
  every cost scenario (up to 9,990 untracked and 1,000 staged or unstaged lines).
- **They DIFFER on 12 fixtures, by nine causes** (each a user-visible divergence,
  so a critical bug by the house rule if gix's answer were shown):
  1. **Rename limit**: gix pairs no inexact staged rename once sources x
     destinations exceeds 1,000 (40 x 40 here); git pairs all 40. The unsquared
     limit `../diff-engine/rename-parity-spike.md` found in the diff lane, hit again
     by status.
  2. **`status.renames` and `status.renameLimit` are not read**: gix 0.87.1 declares
     both keys under the `merge` section
     (`gix-0.87.1/src/config/tree/sections/status.rs:14-22`), so status reads
     `merge.renames`/`merge.renameLimit`. `status.renames=false` is ignored (gix
     pairs, git does not); `merge.renames=false` turns gix's status rename detection
     off (git pairs, gix does not); `status.renames=copies` is ignored.
  3. **Copies**: with `diff.renames=copies` gix finds the copy but drops the `M` of
     the modified copy source.
  4. **Intent-to-add renames**: git pairs a deleted file with an `add -N` entry of
     the same content (`.R`); gix reports `.D` + `.A`, and gix's own index-worktree
     rename tracking pairs deletions with *untracked* files instead — the opposite
     of git — so no gix setting reproduces git.
  5. **Conflicts**: every conflict code agrees (UU AA UD DU AU UA DD), but gix
     *also* lists as untracked each conflicted path with no stage-2 entry
     (`dr-theirs.txt` stages 1+3, `du.txt` 1+3, `rr-theirs.txt` 3).
  6. **Sparse checkout**: in cone mode gix omits an untracked file inside an
     excluded directory that git lists; with a sparse index gix's HEAD-to-index
     half fails outright (`Cannot diff indices that contain sparse entries`).
  7. **fsmonitor hook**: with a hook that lies, git hides the change (it trusts the
     hook), gix reports it (it never runs the hook).
  8. **Submodules under `-uno`**: gix still reports a submodule dirty for untracked
     files inside it (`S..U`); git passes `-uno` down and does not.
  9. **Ignored files with `-uall --ignored`**: git lists each file inside an
     ignored directory; gix lists the collapsed directory.
- **Neither tool wrote the index in any fixture.** git wrote elsewhere under
  `GIT_OPTIONAL_LOCKS=0`, on every run: it advanced the **mtime** of
  `.git/sharedindex.<sha>` under `core.splitIndex`, and of the loose **tree
  objects** under a sparse index, bytes unchanged in both. gix wrote nothing in any
  fixture.
- **gix runs programs too.** On a stat-dirty file under a filter it ran the clean
  driver itself (`sh -c '<clean>'`) and started `git-lfs filter-process` — outside
  any runner of Cairn's, with the application's inherited environment. It spawned
  no `git` and never ran the fsmonitor hook.
- **Cost on rust-lang/rust (62,892 entries), warm median of 7, 16 threads:**
  clean tree with untracked off — gix 25.1 ms on a held handle (40.5 ms with a
  fresh open) against git 5.6 ms; clean with untracked `normal` — gix 34.5 against
  git 27.2 ms; 10,000 untracked — gix 52.2 against git 34.1 (16.0 with a fresh
  untracked cache); every file touched, content unchanged — **gix 135 ms against
  git 736 ms, on every run**, since neither writes the refreshed stat back. gix's
  floor is its HEAD-to-index half, about 22 ms every time (it builds an index from
  the HEAD tree instead of using the cache-tree extension); git's is the spawn plus
  a cache-tree it can trust.

## Method

**Machine.** AMD Ryzen 7 9800X3D (8 cores, 16 threads), 60 GiB RAM, Linux
7.2.8-2-cachyos, `git version 2.56.0` (distribution package), and the floor build
`git version 2.30.9` from `~/.cache/cairn/git-floor`. Rust 1.97.1, release profile.
Fixtures and the scratch copy lived on tmpfs (the session scratchpad); one
clean-tree check ran on the bench repository itself, on btrfs.

**The spike** (Appendix A) is a cargo binary depending on
`gix = { version = "=0.87.1", features = ["blame","blob-diff","revision","status","max-performance","parallel","sha256"] }`
— Cairn's exact feature set — resolved offline with a copy of Cairn's `Cargo.lock`.
It opens a repository with Cairn's trust options
(`ThreadSafeRepository::open_opts` with `Options::default_for_level(Trust::Full)
.with(Trust::Full)`; not `open_path_as_is`, since it is handed a worktree path) and
has three verbs:

- `gix <repo>` runs `Repository::status(Discard)` — all three halves: HEAD tree to
  index (`Item::TreeIndex`), index to worktree and the directory walk
  (`Item::IndexWorktree`) — with `untracked_files(None | Collapsed | Files)` for
  `-uno`/`-unormal`/`-uall`, and for `--ignored`
  `dirwalk_options(emit_ignored(Some(CollapseDirectory | Matching)))` plus
  `emit_collapsed(Some(OnStatusMismatch))`. Submodules as configured (gix's
  default); rename tracking as configured for HEAD-to-index, none for
  index-to-worktree (gix's default; git status pairs only intent-to-add entries).
- `git <repo>` runs `git status --porcelain=v2 -z --untracked-files=<mode>
  [--ignored]` with `LC_ALL=C GIT_OPTIONAL_LOCKS=0 GIT_NO_LAZY_FETCH=1
  GIT_TERMINAL_PROMPT=0`, as Cairn's reads run, and parses the records.
- `bench <repo>` times both (see Cost).

Both normalise to one line per fact, sorted: `staged <X> <path>[ <orig>]` (HEAD to
index), `unstaged <Y> <path>[ <orig>][ S<c><m><u>]` (index to worktree, with git's
submodule field), `unmerged <XY> <path>`, `untracked ? <path>[/]`,
`ignored ! <path>[/]`. gix maps `Addition`/`Deletion`/`Modification`/`Rewrite` to
A / D / M-or-T (T when the entry kind — file, symlink, gitlink — changes) / R-or-C;
`Change::Removed|Type|Modification` to D/T/M; `SubmoduleModification` to M with `S`
+ C (checked-out head differs from the index) + M (any tracked change) + U (any
untracked); `IntentToAdd` to `unstaged A` (git's `.A`); `Conflict` summaries to
git's seven codes; a directory-walk entry to `?`/`!`, with `/` when its disk kind is
a directory or repository. gix's `NeedsUpdate` items never reach a caller of the
iterator: it keeps them aside for an index write-back
(`gix-0.87.1/src/status/iter/mod.rs`, `maybe_keep_index_change`).

**Fixtures** (Appendix B, `fixtures.sh`) are built with real git under an isolated
global configuration (`HOME` a scratch directory, `GIT_CONFIG_NOSYSTEM=1`).
`compare.sh` runs both tools in five modes — `-uno`, `-unormal`, `-uall`,
`-unormal --ignored`, `-uall --ignored` — and diffs the normalised output.
**Write detection:** before gix, between gix and git, and after git, it snapshots
every file and directory under the repository's common git directory (path, size,
mtime to the nanosecond, SHA-1 of the bytes), so a write anywhere — index, shared
index, objects, `.git/modules`, a linked worktree's directory — shows. The whole
suite was then run a second time with git 2.30.9 first on `PATH` for the git side.

## Agreement, fixture by fixture

Verdicts are for git 2.56.0. With git 2.30.9 the set of differing fixtures and every
differing line is the same, except that 2.30.9 cannot read the sparse index at all
(`index uses sdir extension, which we do not understand`), being older than it.
"All modes" means all five.

| Task item | Fixture | What it exercises | Verdict |
| --- | --- | --- | --- |
| 1 | `f01_basic` | unstaged M; staged A; staged D; unstaged D; staged + unstaged on one file | AGREE, all modes |
| 1 | `f01_rename_staged` | staged exact rename; staged rename with an edit | AGREE |
| 1 | `f01_many_renames` | 40 staged renames, each with an edit | **DIFFER** |
| 1 | `f01_worktree_rename` | unstaged deletion + untracked file of the same content | AGREE (`.D` + `?`; neither pairs) |
| 1 | `f01_copies` | `status.renames=copies`; copy of a modified and of an unmodified file | **DIFFER** |
| 1 | `f01_copies_diff_renames` | the same with `diff.renames=copies` | **DIFFER** |
| 1 | `f01_merge_renames_false` | `merge.renames=false`, a staged rename | **DIFFER** |
| 1 | `f01_status_renames_false` | `status.renames=false`, a staged rename | **DIFFER** |
| 2 | `f02_untracked` | nested untracked dirs; empty and nested-empty dirs; a dir holding only ignored files; nested repositories with and without a commit; untracked symlink; untracked file and dir inside a tracked dir; mixed untracked and ignored dir | AGREE, all modes (`-unormal` collapses to `dir/` alike; empty dirs shown by neither; nested repos as `dir/` by both) |
| 3 | `f03_ignored` | `.gitignore` with negation, `dir/` rule, rooted rule, re-include under an excluded dir; nested `.gitignore`; `.git/info/exclude`; `core.excludesFile`; a tracked file matching an ignore rule, modified | AGREE in four modes; **DIFFER** in `-uall --ignored` |
| 4 | `f04_conflicts` | one merge producing UU, AA, UD (x2), DU (x2), AU, UA, DD | codes AGREE; **DIFFER** (extra untracked) whenever untracked is on |
| 5 | `f05_ita` | `add -N` file; `add -N` empty file; deleted file + `add -N` of its renamed copy | **DIFFER** |
| 6 | `f06_typechange` | file to symlink, unstaged and staged; `chmod +x` with `core.fileMode=true` | AGREE |
| 6 | `f06_filemode_false` | `chmod +x` with `core.fileMode=false`, alone and with an edit | AGREE |
| 7 | `f07_autocrlf` | `core.autocrlf=true`, worktree CRLF whose clean form equals the LF blob | AGREE — both say `M` |
| 7 | `f07_eol_attr` | `eol=crlf`: rewritten CRLF (size changed); LF file with only the mtime changed | AGREE |
| 7 | `f07_text_renorm` | CRLF blobs, then `*.txt text` committed; one file touched, one not | AGREE — both say `M` for both |
| 8 | `f08_filter` | `filter.upper.clean = tr a-z A-Z`: a touched file; `Hello` on disk against blob `HELLO`; a real change | AGREE (only the real change is `M`) |
| 9 | `f09_racy` | same-size edit, mtime restored to the entry's, index mtime set to the same instant | AGREE (both `M`) |
| 9 | `f09_racy_trustctime_false` | the same with `core.trustctime=false` and the index 10 s newer | AGREE (both clean: the stat-identical edit is invisible to both) |
| 9 | `f09_racy_checkstat_minimal` | `core.checkStat=minimal`, same-size edit | AGREE (both `M`) |
| 10 | `f10_skip` | skip-worktree file edited and another deleted; assume-unchanged file edited | AGREE (neither reports any) |
| 11 | `f11_sparse_cone` | cone mode; edits inside the cone and at top level; untracked file in an excluded dir | **DIFFER** |
| 11 | `f11_sparse_sparse_index` | the same with `--sparse-index` (`index.sparse=true`) | **DIFFER** (gix errors); git wrote |
| 12 | `f12_split` | `core.splitIndex=true` + `--split-index`; staged and unstaged edits | AGREE; git wrote |
| 12 | `f12_untracked_cache` | `core.untrackedCache=true`, cache written, then new files and dirs | AGREE |
| 12 | `f12_index_v4` | `--index-version 4` | AGREE |
| 12 | `f12_skip_hash` | `index.skipHash=true` | AGREE |
| 13 | `f13_fsmonitor_hook_lies` | `core.fsmonitor` = a protocol-v2 hook that reports nothing; FSMN extension written | **DIFFER** |
| 13 | `f13_fsmonitor_daemon` | `core.fsmonitor=true`, builtin daemon running and accurate | AGREE |
| 14 | `f14_submodules` | clean; modified inside; new commit; untracked inside; `ignore=dirty`, `=untracked`, `=all`; deinitialised | AGREE except **DIFFER** under `-uno` |
| 15 | `f15_icase` | `core.ignoreCase=true` on a case-sensitive fs, `File.txt` renamed `file.txt` on disk | AGREE (both ` D File.txt`, neither lists `file.txt`) |
| 16 | `f16_unborn` | no commit; staged files, one also edited; one untracked | AGREE |
| 16 | `f16_detached` | detached HEAD with staged and unstaged changes | AGREE |
| 16 | `f16_sha256` | `--object-format=sha256` repository with a staged rename | AGREE |
| 17 | `f17_linked_worktree` | status inside a `git worktree add` checkout | AGREE |
| – | `f18_names` | spaces, Unicode, leading `-`, backslash, quote, newline and tab in names | AGREE |
| 8 | `f19_lfs` | git-lfs (`filter.lfs.process`): touched tracked file; changed file | AGREE |

### The differences, with the lines

Lines are `gix only:` (gix said it, git did not) and `git only:`.

**`f01_many_renames`** — 120 lines differ, the same in every mode. git pairs all 40;
gix reports 40 `staged A g<n>.txt` and 40 `staged D f<n>.txt` instead of
`staged R g<n>.txt f<n>.txt`:

```
gix only: staged A g1.txt
...       (39 more staged A, 40 staged D)
git only: staged R g1.txt f1.txt
...       (39 more)
```

40 x 40 = 1,600 exceeds gix's default `Rewrites::limit` of 1,000, which gix compares
against the *unsquared* product (`gix-diff-0.67.1/src/rewrites/tracker.rs:395-396`,
as `../diff-engine/rename-parity-spike.md` found), so the fuzzy stage is skipped;
git's limit is effectively 1,000 squared. Status renames through `gix::status` take
their limit from `merge.renameLimit` (below), so the correction that spike measured —
pass gix `limit = renameLimit²` — would have to come through
`tree_index_track_renames(TrackRenames::Given(..))`, built from git's keys by the
caller.

**`f01_copies`** (`status.renames=copies`), every mode:

```
gix only: staged A copy.txt
git only: staged C copy.txt src.txt
```

**`f01_copies_diff_renames`** (`diff.renames=copies`) — gix finds the copy, loses the
source's modification:

```
git only: staged M src.txt
```

**`f01_merge_renames_false`** and **`f01_status_renames_false`**, every mode:

```
merge.renames=false:   gix only: staged A moved.txt
                       gix only: staged D big.txt
                       git only: staged R moved.txt big.txt
status.renames=false:  gix only: staged R moved.txt big.txt
                       git only: staged A moved.txt
                       git only: staged D big.txt
```

Cause of the three configuration cases: `gix::config::tree::Status::RENAMES` and
`Status::RENAME_LIMIT` are declared with `&config::Tree::MERGE`
(`gix-0.87.1/src/config/tree/sections/status.rs:14-22`), and
`Repository::tree_index_status` with `TrackRenames::AsConfigured` reads those keys
first and `diff.renames` second (`gix-0.87.1/src/status/tree_index.rs`). In the
copies case gix reads `diff.renames=copies` correctly, so the lost `M` is a separate
defect in the rewrite tracker's handling of a copy source; its line was not traced
(OPEN).

**`f03_ignored`**, `-uall --ignored` only:

```
gix only: ignored ! build/
gix only: ignored ! logs/
git only: ignored ! build/keep.txt
git only: ignored ! build/obj/o.o
git only: ignored ! logs/1.txt
```

git's traditional `--ignored` lists the files inside an ignored directory when
`-uall` is given; gix collapses the ignored directory even with
`EmissionMode::Matching`. Setting `for_deletion(IgnoredDirectoriesCanHideNestedRepositories)`
was tried and changed nothing. Everything else about ignore rules agreed: negation,
re-include under an excluded directory (re-included by neither), rooted rules,
nested `.gitignore`, `info/exclude`, `core.excludesFile`, and a tracked file
matching a rule.

**`f04_conflicts`**, whenever untracked files are listed (`-uno` agrees):

```
gix only: untracked ? dr-theirs.txt      (index stages 1, 3)
gix only: untracked ? du.txt             (index stages 1, 3)
gix only: untracked ? rr-theirs.txt      (index stage 3)
```

Every `unmerged` line agrees. The directory walk classifies a path by
`State::entry_by_path` (`gix-dir-0.29.1/src/walk/classify.rs:423`), which answers
only a stage-0 or stage-2 entry (`gix-index-0.55.0/src/access/mod.rs:353`), so a
path whose conflict has no "ours" stage reads as untracked while its file is on
disk.

**`f05_ita`**, every mode:

```
gix only: unstaged A y.txt
gix only: unstaged D x.txt
git only: unstaged R y.txt x.txt
```

git pairs index-to-worktree renames only where the destination is an intent-to-add
entry. gix's `index_worktree_rewrites` pairs a deleted entry with an *untracked*
walk entry: probed with it on, gix still reports `A y.txt`/`D x.txt` here, and
reports `unstaged R w2.txt w.txt` in `f01_worktree_rename`, where git reports `.D`
and `?`. The plain intent-to-add lines (`.A new.txt`, `.A empty.txt`) agree.

**`f11_sparse_cone`**, whenever untracked is on:

```
git only: untracked ? b/new.txt
```

gix marks a directory whose every tracked entry is skip-worktree as
`Property::TrackedExcluded` (`gix-dir-0.29.1/src/walk/classify.rs`, the
`TrackedExcluded` assignment after the `prefixed_entries_range` check) and does not
report what is in it; git lists the untracked file (printing its sparse-expansion
hint on stderr).

**`f11_sparse_sparse_index`**:

```
gix only: ERROR Cannot diff indices that contain sparse entries
git only: untracked ? b/new.txt          (when untracked is on)
```

`gix_diff::index` refuses a sparse index
(`gix-diff-0.67.1/src/index/function.rs:41-42`; its doc says sparse indices "must be
'unsparsed' before"), so gix's HEAD-to-index half errors; its index-to-worktree
lines (`a/x.txt`, `top.txt`) still agree. git 2.30.9 cannot read this index at all.

**`f13_fsmonitor_hook_lies`**, every mode:

```
gix only: unstaged M a.txt
```

git ran the hook, believed it and hid the edit (it still listed the untracked file,
there being no untracked cache). gix 0.87.1 has no fsmonitor client: it runs no hook
and asks no daemon. It skips entries carrying `FSMONITOR_VALID`
(`gix-status-0.34.1/src/index_as_worktree/function.rs:282`), but nothing set that
flag on read here. With an accurate daemon (`f13_fsmonitor_daemon`) the two agree,
because the truth and the daemon's answer coincide.

**`f14_submodules`**, `-uno` only:

```
gix only: unstaged M sub_untr S..U
```

git passes `-uno` into the submodule; gix's `BuiltinSubmoduleStatus`
(`gix-0.87.1/src/status/index_worktree.rs`, `fn status`) computes a submodule's
status from its `ignore` setting alone, not the caller's untracked mode. Every other
submodule state agreed — `S.M.`, `SC..`, `S..U`, each `ignore` value, and a
deinitialised submodule (reported by neither).

### Agreements worth noting

- `f07_autocrlf`, `f07_eol_attr`, `f07_text_renorm`: where the size differs, **both**
  say `M` without consulting the content (git's stat check reports a size mismatch
  as a change, and gix matches it), although the clean form equals the blob. A diff
  of such a file is empty. Where only the mtime changed, both run the conversion
  and both say clean.
- `f09_racy_trustctime_false`: a same-size edit with the mtime restored is invisible
  to both once ctime is not trusted and the index is newer — git's own blind spot,
  matched.
- `f12_split`, `f12_index_v4`, `f12_skip_hash`, `f12_untracked_cache`: gix reads every
  index format tested, ignores the untracked cache, and reaches the same answer.

## Writes

Every fixture, every mode, both tools, snapshot of the whole common git directory:

| Tool | Fixtures with a write | What |
| --- | --- | --- |
| gix | none | — |
| git, `GIT_OPTIONAL_LOCKS=0` | `f12_split` | `.git/sharedindex.<sha>`: mtime advanced on **every** run, bytes unchanged (git freshens a shared index it uses, so it is not expired) |
| git, `GIT_OPTIONAL_LOCKS=0` | `f11_sparse_sparse_index` | the loose tree objects of `a/`, `b/`, `c/`: mtime advanced on every run, bytes unchanged (object freshening as the sparse index is expanded) |

The index itself was written by neither tool in any fixture. Under git 2.30.9 the
shared-index freshening is the same. The fsmonitor daemon was started during
fixture setup, so its socket and cookie directory predate the measured runs; cookie
files created and deleted inside one run cannot be seen by a before/after snapshot.

The bench repository `~/Development/bench/rust` (HEAD `c999cef531e`) was only read:
`find .git` (path, size, mtime) before the first measurement and after the last is
identical, `.git/index` (SHA-1 `9e8267d3…`, mtime `1789663102.530893169`) unchanged,
and `GIT_OPTIONAL_LOCKS=0 git status --porcelain` empty before and after.

## Programs a status runs

- **gix spawned no `git`**: a logging `git` first on `PATH` recorded nothing across a
  gix status, with or without `GIT_CONFIG_NOSYSTEM`.
- **gix ran the clean filter driver itself.** With the `filter.upper.clean` command
  logging each invocation, gix's status ran it twice and git's four times
  (`f08_filter`); with git-lfs, a logging `git-lfs` first on `PATH` recorded gix
  starting `git-lfs filter-process` twice and git once (`f19_lfs`). These children
  are started by `gix-filter` inside the gix call — not through `cairn-git`'s
  `process/` runner, so in no repository's registry or command log, not killable by
  its cancel, and with the application process's inherited environment rather than
  a `GitEnvironment` (no `GIT_TERMINAL_PROMPT=0`, no askpass pins). git's filter
  children get the environment Cairn built for the read.
- **Only git ran anything for fsmonitor**: the configured hook
  (`f13_fsmonitor_hook_lies`) and the daemon's IPC (`f13_fsmonitor_daemon`).

## Cost

**Subject.** A scratch copy of `~/Development/bench/rust` (rust-lang/rust at
`c999cef531e`; 62,892 index entries, 4,625 tracked directories, about 406 MB of
working tree) made with `cp -a` into the scratchpad (tmpfs, 28 GB free after), its
index refreshed once (`git update-index --refresh`) because a copy changes every
inode and ctime. Before each scenario the copy is **settled** — 1.5 s of sleep, a
refresh and a locked `git status` — so no entry is racily clean unless the scenario
says so.

**Method.** `status-spike bench`, in one process: for each of gix-held (one
`Repository` reused, as Cairn's worker holds one; gix keeps the parsed index between
calls while the file is unchanged), gix-fresh (open, then status) and git (spawn
`git status --porcelain=v2 -z`, read, parse), one warm-up then 7 timed runs; median
and [min–max] in milliseconds. Every gix run collects and normalises every item;
every git run parses every record; line counts are asserted equal across runs.
Before each scenario's timings the two tools' normalised outputs are diffed: **they
agreed in every scenario** (line counts in the tables). Also timed: gix's two halves
alone (`into_index_worktree_iter`; `Repository::tree_index_status`) and
`open + index_or_empty`, to show where gix's time goes. gix uses all 16 logical
cores unless limited. Harness: Appendix C.

### Scratch copy (tmpfs)

| Scenario | lines | gix held | gix fresh open | git spawn + parse |
| --- | --- | --- | --- | --- |
| a. clean, `-uno` | 0 | 25.1 [24.6–26.4] | 40.5 [40.3–41.3] | 5.6 [5.4–6.1] |
| a. clean, `-unormal` | 0 | 34.5 [33.5–35.4] | 49.6 [49.4–50.5] | 27.2 [25.9–29.7] |
| b. 1,000 files modified (unstaged), `-uno` | 1,000 | 25.1 [24.4–27.4] | 40.8 [40.0–41.6] | 7.3 [7.1–8.1] |
| b. 1,000 files modified, `-unormal` | 1,000 | 38.0 [34.7–41.6] | 52.0 [50.9–52.5] | 26.3 [26.1–26.9] |
| e. those 1,000 restored and refreshed within the second (racily clean), `-uno` | 0 | 25.8 [25.0–26.9] | 41.7 [40.8–43.4] | 17.5 [17.1–18.2] |
| e. racily clean, `-unormal` | 0 | 36.0 [35.0–37.5] | 51.2 [50.2–53.7] | 36.6 [36.3–38.5] |
| f. 1,000 files modified **and staged**, `-uno` | 1,000 | 25.2 [24.8–26.6] | 42.3 [41.8–43.3] | 19.0 [18.3–20.3] |
| f. 1,000 staged, `-unormal` | 1,000 | 42.3 [41.1–43.3] | 56.8 [55.7–57.1] | 47.6 [46.9–50.3] |
| c. 10,000 untracked files in 1,000 tracked dirs, `-uno` | 0 | 24.8 [24.5–25.5] | 40.7 [40.0–41.7] | 5.5 [5.4–6.1] |
| c. 10,000 untracked, `-unormal` | 9,990 | 52.2 [51.1–53.3] | 69.0 [66.8–71.1] | 34.1 [33.3–37.8] |
| d. every tracked file touched (stat dirty, content same), `-uno` | 0 | 135.4 [128.7–155.8] | 158.3 [150.5–172.5] | 736.2 [734.0–738.8] |
| d. every file touched, `-unormal` | 0 | 137.2 [133.4–156.3] | 151.4 [133.2–164.1] | 765.6 [759.2–771.5] |
| a2. clean again after settling, `-uno` | 0 | 26.1 [25.4–27.3] | 40.6 [40.0–41.3] | 5.7 [5.2–6.3] |
| a2. clean again, `-unormal` | 0 | 38.1 [37.2–40.1] | 53.2 [52.8–54.1] | 26.8 [26.1–28.0] |

(9,990 rather than 10,000: one chosen directory is ignored, and both tools omit its
ten files.)

Scenario d is the state a client that never writes the index lives in after a
checkout, a build that touches files, or a copy: every entry stays stat-dirty, and
because `GIT_OPTIONAL_LOCKS=0` stops git writing the refreshed stat back (and Cairn
would not call gix's `Outcome::write_changes`), **every** status pays the full
re-hash — 736 ms for git, 135 ms for gix hashing in parallel. Scenario e is the same
effect at 1,000 entries: git +12 ms per run, gix +1 ms. Scenario f shows git's
HEAD-to-index cost once the cache-tree is invalidated along 1,000 staged paths
(5.6 → 19.0 ms); gix's does not move.

### git's own accelerators (scratch copy)

The untracked cache and the fsmonitor daemon are read by git and ignored by gix, so
gix's numbers barely move.

| Scenario | lines | gix held | gix fresh | git |
| --- | --- | --- | --- | --- |
| c-uc. 10,000 untracked, `core.untrackedCache=true`, UNTR freshly written by a locked status after the files existed, `-unormal` | 9,990 | 52.9 [52.0–54.7] | 69.7 [69.0–70.5] | 16.0 [15.8–16.8] |
| c-uc. the same, `-uno` | 0 | 25.1 [24.9–26.8] | 41.5 [40.7–42.2] | 6.0 [5.8–7.0] |
| a-uc. clean, untracked cache fresh, `-unormal` | 0 | 35.4 [34.4–37.2] | 53.4 [52.0–54.5] | 15.4 [14.3–15.5] |
| a-uc. clean, untracked cache fresh, `-uno` | 0 | 24.6 [24.4–25.5] | 41.5 [40.9–43.0] | 6.0 [5.8–6.6] |
| a-fsm. clean, `core.fsmonitor=true` (builtin daemon), token written, `-uno` | 0 | 25.2 [24.3–26.3] | 40.7 [40.3–41.0] | 3.5 [3.5–3.7] |
| a-fsm. clean, daemon, `-unormal` | 0 | 38.5 [34.4–40.2] | 51.2 [50.8–52.3] | 23.5 [21.9–26.1] |
| b-fsm. 1,000 modified since the token was written, daemon, `-uno` | 1,000 | 25.4 [24.7–26.7] | 41.7 [40.8–41.9] | 5.5 [5.4–5.6] |
| b-fsm. 1,000 modified, daemon, `-unormal` | 1,000 | 35.2 [34.9–36.1] | 52.3 [51.0–53.4] | 28.5 [25.5–30.0] |

Both caches help git only as far as the last *locked* `git status` wrote them;
Cairn's reads never refresh them, so the untracked cache decays as directories
change, and the daemon token in the index stays as old as the user's last own
`git status`.

### Where gix's time goes (scratch copy, medians)

| Scenario | index-to-worktree half alone | HEAD-to-index half alone | open + index load |
| --- | --- | --- | --- |
| a. clean, `-uno` | 6.0 | 22.3 | 12.3 |
| a. clean, `-unormal` | 32.1 | 22.7 | 12.3 |
| c. 10,000 untracked, `-unormal` | 47.6 | 22.8 | 12.3 |
| d. every file touched, `-uno` | 133.9 | 20.6 | 12.3 |
| f. 1,000 staged, `-uno` | 8.5 | 25.9 | 12.9 |

The halves run on separate threads, so a whole status is near the larger half plus
overhead. The HEAD-to-index half costs about 22 ms on every call whatever changed:
gix builds a whole index from the HEAD tree (`index_from_tree`,
`gix-0.87.1/src/status/tree_index.rs`) and diffs the two, where git trusts the
index's cache-tree extension for every directory it still covers.

### Thread count (scratch copy, gix held, `-uno`)

| `thread_limit` | clean | every file touched |
| --- | --- | --- |
| 1 | 53.4 [52.3–54.6] | 822.6 [820.5–829.1] |
| 4 | 23.5 [23.0–25.7] | 212.8 [211.6–224.1] |
| default (all 16) | 25.1 [24.6–26.4] | 135.4 [128.7–155.8] |

git in the same runs: 5.8 and 5.5 ms clean, 736 and 743 ms touched. Single-threaded,
gix's re-hash is slower than git's.

### The bench repository itself (btrfs), clean, read only

| Scenario | gix held | gix fresh | git |
| --- | --- | --- | --- |
| clean, `-uno` | 25.4 [25.1–31.1] | 39.2 [39.0–42.0] | 6.1 [5.5–7.2] |
| clean, `-unormal` | 38.2 [38.0–39.1] | 52.9 [52.1–53.4] | 28.8 [28.2–29.3] |

Split: index-to-worktree 5.9 / 35.4 ms, HEAD-to-index 22.5 / 22.6 ms, open + index
load 12.3 ms — within a few milliseconds of the tmpfs copy.

## What the measurements bear on (not a decision)

- **Agreement.** gix's status as 0.87.1 ships it answers differently from git's on
  ordinary repositories in five ways no configuration hides — many inexact staged
  renames, intent-to-add renames, conflicted paths without stage 2, sparse checkouts
  (cone and sparse index), `-uall --ignored` — and in five more under
  configuration (`status.renames`, `merge.renames`, copies, a lying or stale
  fsmonitor hook) or mode (submodules under `-uno`). Some could be corrected around
  gix in Cairn's own code (drop untracked entries whose path has unmerged index
  entries; pass `TrackRenames::Given` built from git's own keys with the squared
  limit; never ask `-uno` of a submodule); the sparse-index refusal, the ita pairing
  and the copy source's lost `M` could not without reimplementing that half. Where
  gix is "more right" (the lying hook) it still differs from the user's
  `git status`.
- **Cost.** On a clean or lightly dirty tree git is faster with untracked off (5–7
  against 25 ms) and close with untracked on (27 against 35 ms; 16 against 53 with
  a fresh untracked cache). With many stale stat entries — a state a never-writing
  client stays in — gix is about 5x faster per call (135 against 736 ms) on this
  16-thread machine, and slower than git when held to one thread.
- **Side effects.** git under `GIT_OPTIONAL_LOCKS=0` writes no index but does
  advance mtimes (a split index's shared file; tree objects under a sparse index),
  and honours the user's fsmonitor and untracked cache. gix writes nothing but runs
  filter drivers (clean commands, git-lfs's process filter) itself, outside
  `process/`.

## OPEN

- **The copy source's lost `M`** (`f01_copies_diff_renames`): observed; the cause
  was not traced to a line of `gix-diff`'s tracker.
- **macOS and Windows**: case-insensitive filesystems, `core.precomposeUnicode`,
  `core.fscache`, `core.symlinks=false` — not measured (Linux host only).
  `f15_icase` sets `core.ignoreCase` on a case-sensitive filesystem, which is not
  the same thing.
- **A filter that fails** (a non-`required` driver exiting non-zero, a missing
  `git-lfs`): not exercised; whether gix falls back to the unfiltered content as git
  does is unknown.
- **Submodule edge cases**: nested submodules, a submodule in conflict,
  `diff.ignoreSubmodules`, a gitfile pointing into a moved superproject — not
  exercised.
- **`status.showUntrackedFiles` with no flag**: both tools were always given the
  mode explicitly.
- **Larger rename sets on the bench** (git's own `renameLimit` cut-off and its
  warning) — only the 40-file fixture measured the limit.
- **gix's `NeedsUpdate` count** in scenario d: the iterator consumes those items
  (`maybe_keep_index_change`), so the bench did not count them.
- **Cold-cache cost**: every timing is warm (warm-up first, page cache hot).
- **The fsmonitor daemon's per-run cookie writes**: transient within a run, not
  captured by before/after snapshots.

## Appendix A: the spike (`status-spike`)

`Cargo.toml`:

```toml
[package]
name = "status-spike"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
gix = { version = "=0.87.1", features = ["blame", "blob-diff", "revision", "status", "max-performance", "parallel", "sha256"] }

[profile.release]
debug = false
```

`src/main.rs` (the `SPIKE_THREADS`, `SPIKE_IW_RENAMES` and `SPIKE_FOR_DELETION`
environment variables are the probes the text names; unset, they change nothing):

```rust
//! Status agreement spike: gix 0.87.1 `status` against `git status --porcelain=v2 -z`,
//! both normalised to `<dimension> <code> <path>[ <orig-path>]`, one line each, sorted.
//!
//! Usage:
//!   status-spike gix   <repo> [--untracked no|normal|all] [--ignored]
//!   status-spike git   <repo> [--untracked no|normal|all] [--ignored]
//!   status-spike bench <repo> [--untracked ...] [--ignored] [--runs N] [--git-config k=v]...
//!
//! `bench` times (after one warm-up each) gix on a held handle, gix with a fresh open per
//! run, and the git spawn+parse, and prints the median and the min/max of N runs.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use gix::bstr::ByteSlice;
use gix::dir::entry::{Kind as DiskKind, Status as DirStatus};
use gix::dir::walk::{CollapsedEntriesEmissionMode, EmissionMode};
use gix::status::plumbing::index_as_worktree::{Change, Conflict, EntryStatus};
use gix::status::{UntrackedFiles, index_worktree};

#[derive(Clone, Copy, PartialEq)]
enum Untracked {
    No,
    Normal,
    All,
}

struct Opts {
    untracked: Untracked,
    ignored: bool,
    runs: usize,
    git_config: Vec<String>,
}

fn parse_opts(args: &[String]) -> Opts {
    let mut o = Opts {
        untracked: Untracked::Normal,
        ignored: false,
        runs: 7,
        git_config: Vec::new(),
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--untracked" => {
                i += 1;
                o.untracked = match args[i].as_str() {
                    "no" => Untracked::No,
                    "normal" => Untracked::Normal,
                    "all" => Untracked::All,
                    other => panic!("bad --untracked {other}"),
                }
            }
            "--ignored" => o.ignored = true,
            "--runs" => {
                i += 1;
                o.runs = args[i].parse().unwrap();
            }
            "--git-config" => {
                i += 1;
                o.git_config.push(args[i].clone());
            }
            other => panic!("unknown arg {other}"),
        }
        i += 1;
    }
    o
}

fn open(path: &Path) -> gix::Repository {
    use gix::sec::trust::DefaultForLevel as _;
    let trust = gix::sec::Trust::Full;
    let options = gix::open::Options::default_for_level(trust)
        .with(trust);
    gix::ThreadSafeRepository::open_opts(path, options)
        .expect("open")
        .to_thread_local()
}

fn kind(mode: gix::index::entry::Mode) -> char {
    if mode == gix::index::entry::Mode::SYMLINK {
        'l'
    } else if mode == gix::index::entry::Mode::COMMIT {
        'c'
    } else {
        'f'
    }
}

fn dir_suffix(entry: &gix::dir::Entry) -> &'static str {
    match entry.disk_kind {
        Some(DiskKind::Directory) | Some(DiskKind::Repository) => "/",
        _ => "",
    }
}

fn conflict_code(c: Conflict) -> &'static str {
    match c {
        Conflict::BothDeleted => "DD",
        Conflict::AddedByUs => "AU",
        Conflict::DeletedByThem => "UD",
        Conflict::AddedByThem => "UA",
        Conflict::DeletedByUs => "DU",
        Conflict::BothAdded => "AA",
        Conflict::BothModified => "UU",
    }
}

fn submodule_flags(s: &gix::submodule::Status) -> String {
    let c = if s.checked_out_head_id != s.index_id { 'C' } else { '.' };
    let mut m = '.';
    let mut u = '.';
    for item in s.changes.iter().flatten() {
        match item {
            gix::status::Item::TreeIndex(_) => m = 'M',
            gix::status::Item::IndexWorktree(index_worktree::Item::Modification { .. })
            | gix::status::Item::IndexWorktree(index_worktree::Item::Rewrite { .. }) => m = 'M',
            gix::status::Item::IndexWorktree(index_worktree::Item::DirectoryContents { entry, .. }) => {
                if entry.status == DirStatus::Untracked {
                    u = 'U'
                }
            }
        }
    }
    format!("S{c}{m}{u}")
}

#[derive(Default)]
struct GixCounts {
    needs_update: usize,
    other_dir_entries: usize,
}

fn gix_status(repo: &gix::Repository, o: &Opts, counts: &mut GixCounts) -> Vec<String> {
    let mode = match o.untracked {
        Untracked::No => UntrackedFiles::None,
        Untracked::Normal => UntrackedFiles::Collapsed,
        Untracked::All => UntrackedFiles::Files,
    };
    let mut platform = repo.status(gix::progress::Discard).expect("platform").untracked_files(mode);
    if let Some(n) = std::env::var("SPIKE_THREADS").ok().and_then(|n| n.parse().ok()) {
        platform = platform.index_worktree_options_mut(|o| o.thread_limit = Some(n));
    }
    if std::env::var_os("SPIKE_IW_RENAMES").is_some() {
        // Probe only: index-to-worktree rename tracking, which git status does not do
        // (it pairs only intent-to-add entries).
        platform = platform.index_worktree_rewrites(gix::diff::Rewrites::default());
    }
    if o.ignored {
        let ig = if o.untracked == Untracked::All {
            EmissionMode::Matching
        } else {
            EmissionMode::CollapseDirectory
        };
        if o.untracked == Untracked::No {
            // git `--ignored -uno` still lists ignored paths; gix needs a dirwalk for that.
            platform = platform.untracked_files(UntrackedFiles::Collapsed);
        }
        platform = platform.dirwalk_options(|d| {
            d.emit_ignored(Some(ig))
                .emit_collapsed(Some(CollapsedEntriesEmissionMode::OnStatusMismatch))
                .for_deletion(std::env::var_os("SPIKE_FOR_DELETION").map(|_| {
                    gix::dir::walk::ForDeletionMode::IgnoredDirectoriesCanHideNestedRepositories
                }))
        });
    }
    let mut out = Vec::new();
    for item in platform.into_iter(None).expect("iter") {
        let item = match item {
            Ok(i) => i,
            Err(e) => {
                out.push(format!("ERROR {e}"));
                continue;
            }
        };
        match item {
            gix::status::Item::TreeIndex(change) => {
                use gix::diff::index::ChangeRef::*;
                match change {
                    Addition { location, .. } => out.push(format!("staged A {location}")),
                    Deletion { location, .. } => out.push(format!("staged D {location}")),
                    Modification {
                        location,
                        previous_entry_mode,
                        entry_mode,
                        ..
                    } => {
                        let code = if kind(previous_entry_mode) != kind(entry_mode) { 'T' } else { 'M' };
                        out.push(format!("staged {code} {location}"))
                    }
                    Rewrite {
                        source_location,
                        location,
                        copy,
                        ..
                    } => out.push(format!(
                        "staged {} {location} {source_location}",
                        if copy { 'C' } else { 'R' }
                    )),
                }
            }
            gix::status::Item::IndexWorktree(iw) => match iw {
                index_worktree::Item::Modification { rela_path, status, .. } => match status {
                    EntryStatus::Conflict { summary, .. } => {
                        out.push(format!("unmerged {} {rela_path}", conflict_code(summary)))
                    }
                    EntryStatus::Change(Change::Removed) => out.push(format!("unstaged D {rela_path}")),
                    EntryStatus::Change(Change::Type { .. }) => out.push(format!("unstaged T {rela_path}")),
                    EntryStatus::Change(Change::Modification { .. }) => {
                        out.push(format!("unstaged M {rela_path}"))
                    }
                    EntryStatus::Change(Change::SubmoduleModification(s)) => {
                        out.push(format!("unstaged M {rela_path} {}", submodule_flags(&s)))
                    }
                    EntryStatus::NeedsUpdate(_) => counts.needs_update += 1,
                    EntryStatus::IntentToAdd => out.push(format!("unstaged A {rela_path}")),
                },
                index_worktree::Item::DirectoryContents { entry, .. } => match entry.status {
                    DirStatus::Untracked => {
                        out.push(format!("untracked ? {}{}", entry.rela_path, dir_suffix(&entry)))
                    }
                    DirStatus::Ignored(_) => {
                        out.push(format!("ignored ! {}{}", entry.rela_path, dir_suffix(&entry)))
                    }
                    DirStatus::Tracked | DirStatus::Pruned => counts.other_dir_entries += 1,
                },
                index_worktree::Item::Rewrite {
                    source,
                    dirwalk_entry,
                    copy,
                    ..
                } => out.push(format!(
                    "unstaged {} {} {}",
                    if copy { 'C' } else { 'R' },
                    dirwalk_entry.rela_path,
                    source.rela_path()
                )),
            },
        }
    }
    // git prints a submodule line's sub field even when only the worktree changed; keep
    // the flags only on unstaged lines, as both sides do here.
    out.sort();
    out
}

fn git_command(repo: &Path, o: &Opts) -> Command {
    let mut c = Command::new("git");
    c.env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("-C")
        .arg(repo);
    for kv in &o.git_config {
        c.arg("-c").arg(kv);
    }
    c.arg("status").arg("--porcelain=v2").arg("-z");
    c.arg(match o.untracked {
        Untracked::No => "--untracked-files=no",
        Untracked::Normal => "--untracked-files=normal",
        Untracked::All => "--untracked-files=all",
    });
    if o.ignored {
        c.arg("--ignored");
    }
    c
}

fn git_status(repo: &Path, o: &Opts) -> Vec<String> {
    let output = git_command(repo, o).output().expect("spawn git");
    if !output.status.success() {
        return vec![format!(
            "ERROR git exited {}: {}",
            output.status,
            output.stderr.to_str_lossy().trim()
        )];
    }
    parse_v2(&output.stdout)
}

fn parse_v2(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut records = bytes.split(|b| *b == 0);
    while let Some(rec) = records.next() {
        if rec.is_empty() {
            continue;
        }
        let rec = rec.to_str_lossy();
        let kind = &rec[..1];
        match kind {
            "#" => {}
            "?" => out.push(format!("untracked ? {}", &rec[2..])),
            "!" => out.push(format!("ignored ! {}", &rec[2..])),
            "1" => {
                let f: Vec<&str> = rec.splitn(9, ' ').collect();
                let (xy, sub, path) = (f[1], f[2], f[8]);
                push_xy(&mut out, xy, sub, path, None);
            }
            "2" => {
                let f: Vec<&str> = rec.splitn(10, ' ').collect();
                let (xy, sub, path) = (f[1], f[2], f[9]);
                let orig = records.next().map(|b| b.to_str_lossy().into_owned()).unwrap_or_default();
                push_xy(&mut out, xy, sub, path, Some(&orig));
            }
            "u" => {
                let f: Vec<&str> = rec.splitn(11, ' ').collect();
                out.push(format!("unmerged {} {}", f[1], f[10]));
            }
            _ => out.push(format!("UNPARSED {rec}")),
        }
    }
    out.sort();
    out
}

fn push_xy(out: &mut Vec<String>, xy: &str, sub: &str, path: &str, orig: Option<&str>) {
    let mut c = xy.chars();
    let x = c.next().unwrap();
    let y = c.next().unwrap();
    if x != '.' {
        match (x, orig) {
            ('R' | 'C', Some(o)) => out.push(format!("staged {x} {path} {o}")),
            _ => out.push(format!("staged {x} {path}")),
        }
    }
    if y != '.' {
        let subf = if sub.starts_with('S') { format!(" {sub}") } else { String::new() };
        match (y, orig) {
            ('R' | 'C', Some(o)) => out.push(format!("unstaged {y} {path} {o}{subf}")),
            _ => out.push(format!("unstaged {y} {path}{subf}")),
        }
    }
}

/// gix's index-to-worktree half alone (no HEAD-tree-to-index diff), counting items, to
/// split where the time goes.
fn gix_index_worktree_only(repo: &gix::Repository, o: &Opts) -> usize {
    let mode = match o.untracked {
        Untracked::No => UntrackedFiles::None,
        Untracked::Normal => UntrackedFiles::Collapsed,
        Untracked::All => UntrackedFiles::Files,
    };
    repo.status(gix::progress::Discard)
        .expect("platform")
        .untracked_files(mode)
        .into_index_worktree_iter(None)
        .expect("iter")
        .filter(|i| i.as_ref().map_or(true, |i| i.summary().is_some()))
        .count()
}

/// The HEAD-tree-to-index half alone, as gix computes it (an index built from the tree,
/// then diffed), counting changes.
fn gix_tree_index_only(repo: &gix::Repository) -> usize {
    let tree = repo.head_tree_id_or_empty().expect("head tree");
    let index = repo.index_or_empty().expect("index");
    let mut n = 0;
    repo.tree_index_status(
        &tree,
        &index,
        None,
        gix::status::tree_index::TrackRenames::AsConfigured,
        |_, _, _| {
            n += 1;
            Ok::<_, std::convert::Infallible>(std::ops::ControlFlow::Continue(()))
        },
    )
    .expect("tree-index");
    n
}

fn time_runs(runs: usize, mut f: impl FnMut()) -> (Duration, Duration, Duration) {
    f();
    let mut v = Vec::new();
    for _ in 0..runs {
        let t = Instant::now();
        f();
        v.push(t.elapsed());
    }
    median(v)
}

fn median(mut v: Vec<Duration>) -> (Duration, Duration, Duration) {
    v.sort();
    (v[v.len() / 2], v[0], v[v.len() - 1])
}

fn ms(d: Duration) -> String {
    format!("{:.1}", d.as_secs_f64() * 1000.0)
}

fn bench(repo_path: &Path, o: &Opts) {
    let runs = o.runs;
    // gix, held handle (as Cairn's worker holds one)
    let repo = open(repo_path);
    let mut c = GixCounts::default();
    let lines = gix_status(&repo, o, &mut c).len(); // warm-up
    let mut held = Vec::new();
    for _ in 0..runs {
        let t = Instant::now();
        let mut c2 = GixCounts::default();
        let n = gix_status(&repo, o, &mut c2).len();
        held.push(t.elapsed());
        assert_eq!(n, lines);
    }
    drop(repo);
    // gix, fresh open per run
    let mut fresh = Vec::new();
    {
        let r = open(repo_path);
        gix_status(&r, o, &mut GixCounts::default());
    }
    for _ in 0..runs {
        let t = Instant::now();
        let r = open(repo_path);
        let _ = gix_status(&r, o, &mut GixCounts::default()).len();
        fresh.push(t.elapsed());
    }
    // git spawn + parse
    let glines = git_status(repo_path, o).len();
    let mut git = Vec::new();
    for _ in 0..runs {
        let t = Instant::now();
        let n = git_status(repo_path, o).len();
        git.push(t.elapsed());
        assert_eq!(n, glines);
    }
    let repo = open(repo_path);
    let (iw, _, _) = time_runs(runs, || {
        gix_index_worktree_only(&repo, o);
    });
    let (ti, _, _) = time_runs(runs, || {
        gix_tree_index_only(&repo);
    });
    let (op, _, _) = time_runs(runs, || {
        let r = open(repo_path);
        r.index_or_empty().expect("index");
    });
    let (hm, hmin, hmax) = median(held);
    let (fm, fmin, fmax) = median(fresh);
    let (gm, gmin, gmax) = median(git);
    println!(
        "gix_held_ms={} [{}-{}] gix_fresh_ms={} [{}-{}] git_ms={} [{}-{}] gix_lines={} git_lines={} runs={} | split: gix_index_worktree_only_ms={} gix_tree_index_only_ms={} gix_open_and_index_load_ms={}",
        ms(hm),
        ms(hmin),
        ms(hmax),
        ms(fm),
        ms(fmin),
        ms(fmax),
        ms(gm),
        ms(gmin),
        ms(gmax),
        lines,
        glines,
        runs,
        ms(iw),
        ms(ti),
        ms(op)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args[1].as_str();
    let repo = PathBuf::from(&args[2]);
    let o = parse_opts(&args[3..]);
    match cmd {
        "gix" => {
            let r = open(&repo);
            let mut c = GixCounts::default();
            for l in gix_status(&r, &o, &mut c) {
                println!("{l}");
            }
            if c.needs_update > 0 {
                eprintln!("gix: {} entries NeedsUpdate (stat-dirty, content clean)", c.needs_update);
            }
        }
        "git" => {
            for l in git_status(&repo, &o) {
                println!("{l}");
            }
        }
        "bench" => bench(&repo, &o),
        other => panic!("unknown command {other}"),
    }
}
```

## Appendix B: the agreement harness

`compare.sh`:

```bash
#!/usr/bin/env bash
# compare.sh <repo-worktree> — run gix and git status over the repository in five
# untracked/ignored modes, print AGREE or the differing lines per mode, and report any
# write either tool made to the git directory (common dir, every file: bytes + mtime).
set -u
SPIKE=$(dirname "$(readlink -f "$0")")/target/release/status-spike
repo=$1
common=$(/usr/bin/git -C "$repo" rev-parse --path-format=absolute --git-common-dir)
snap() { (cd "$common" && find . -type f -o -type s -o -type d | sort | while read -r f; do
            if [ -f "$f" ]; then printf '%s %s %s\n' "$f" "$(stat -c '%s %.9Y' "$f")" "$(sha1sum < "$f" | cut -c1-12)";
            else printf '%s %s\n' "$f" "$(stat -c '%F' "$f")"; fi; done); }
tmp=$(mktemp -d)
gix_wrote=""; git_wrote=""
for mode in "--untracked no" "--untracked normal" "--untracked all" "--untracked normal --ignored" "--untracked all --ignored"; do
  snap > "$tmp/s0"
  "$SPIKE" gix "$repo" $mode > "$tmp/gix" 2> "$tmp/gix.err"
  snap > "$tmp/s1"
  "$SPIKE" git "$repo" $mode > "$tmp/git" 2> "$tmp/git.err"
  snap > "$tmp/s2"
  if ! diff -q "$tmp/s0" "$tmp/s1" >/dev/null; then gix_wrote+="[$mode] $(diff "$tmp/s0" "$tmp/s1" | grep '^[<>]' | tr '\n' ';')"; fi
  if ! diff -q "$tmp/s1" "$tmp/s2" >/dev/null; then git_wrote+="[$mode] $(diff "$tmp/s1" "$tmp/s2" | grep '^[<>]' | tr '\n' ';')"; fi
  if diff -q "$tmp/gix" "$tmp/git" >/dev/null; then
    echo "  [$mode] AGREE ($(wc -l < "$tmp/git") lines)"
  else
    diff "$tmp/gix" "$tmp/git" | grep '^[<>]' | sed 's/^</    gix only:/; s/^>/    git only:/' > "$tmp/d"
    if [ -f "$tmp/dprev" ] && cmp -s "$tmp/d" "$tmp/dprev"; then
      echo "  [$mode] DIFFER (the same lines as the mode above)"
    else
      echo "  [$mode] DIFFER ($(wc -l < "$tmp/d") lines differ)"
      head -12 "$tmp/d"; n=$(wc -l < "$tmp/d"); [ "$n" -gt 12 ] && echo "    ... $((n-12)) more"
    fi
    cp "$tmp/d" "$tmp/dprev"
  fi
  [ -s "$tmp/gix.err" ] && sed 's/^/    gix stderr: /' "$tmp/gix.err" | head -5
  [ -s "$tmp/git.err" ] && sed 's/^/    git stderr: /' "$tmp/git.err" | head -5
done
echo "  git (normal) output:"; "$SPIKE" git "$repo" --untracked normal | sed 's/^/    /'
echo "  writes: gix=${gix_wrote:-none} | git=${git_wrote:-none}"
rm -rf "$tmp"
```

`fixtures.sh`:

```bash
#!/usr/bin/env bash
# fixtures.sh [name...] — build each fixture repository with real git under $FX and run
# compare.sh over it. Global config is isolated: HOME points at a scratch home.
set -u
HERE=$(dirname "$(readlink -f "$0")")
S=$(dirname "$HERE")
export FX=$S/fx
export HOME=$S/home
export GIT_CONFIG_NOSYSTEM=1
export GIT_EDITOR=true
unset XDG_CONFIG_HOME
mkdir -p "$HOME" "$FX"
cat > "$HOME/.gitconfig" <<EOF
[user]
	name = Spike
	email = spike@example.invalid
[init]
	defaultBranch = main
[protocol "file"]
	allow = always
[advice]
	detachedHead = false
EOF

new() { rm -rf "$FX/$1"; git init -q "$FX/$1"; cd "$FX/$1"; }
lines() { for i in $(seq 1 ${2:-20}); do echo "$1 line $i"; done; }
commit() { git add -A && git commit -qm "${1:-c}"; }
run() { echo "== $1"; "$HERE/compare.sh" "${2:-$FX/$1}"; }

f01_basic() {
  new f01_basic
  for f in a b c d e; do lines $f > $f.txt; done; commit
  echo extra >> a.txt                          # unstaged M
  lines f > f.txt; git add f.txt               # staged A
  git rm -q c.txt                              # staged D
  rm d.txt                                     # unstaged D
  echo one >> e.txt; git add e.txt; echo two >> e.txt   # MM
  run f01_basic
}
f01_rename_staged() {
  new f01_rename_staged
  lines big 40 > big.txt; lines other 40 > other.txt; commit
  git mv big.txt moved.txt                     # staged R, exact
  git mv other.txt other2.txt; echo tail >> other2.txt; git add other2.txt   # staged R, similar
  run f01_rename_staged
}
f01_worktree_rename() {
  new f01_worktree_rename
  lines w 40 > w.txt; commit
  mv w.txt w2.txt                              # unstaged D + untracked, same content
  run f01_worktree_rename
}
f01_copies() {
  new f01_copies
  lines src 40 > src.txt; lines quiet 40 > quiet.txt; commit
  git config status.renames copies
  cp src.txt copy.txt; echo changed >> src.txt; git add -A     # copy from a modified source
  cp quiet.txt quietcopy.txt; git add quietcopy.txt             # copy from an unmodified source
  run f01_copies
  new f01_copies_diff_renames
  lines src 40 > src.txt; lines quiet 40 > quiet.txt; commit
  git config diff.renames copies
  cp src.txt copy.txt; echo changed >> src.txt; git add -A
  cp quiet.txt quietcopy.txt; git add quietcopy.txt
  run f01_copies_diff_renames
  new f01_merge_renames_false
  lines big 40 > big.txt; commit
  git config merge.renames false               # a merge setting; git status ignores it
  git mv big.txt moved.txt
  run f01_merge_renames_false
  new f01_status_renames_false
  lines big 40 > big.txt; commit
  git config status.renames false              # git status stops pairing renames
  git mv big.txt moved.txt
  run f01_status_renames_false
}
f02_untracked() {
  new f02_untracked
  mkdir tracked; echo t > tracked/t.txt; echo '*.log' > .gitignore; commit
  mkdir -p u1 u2/sub/deep empty onlyign mix tracked/newdir
  echo a > u1/a.txt; echo b > u2/sub/deep/b.txt; echo c > u2/c.txt
  echo x > onlyign/x.log
  echo a > mix/a.txt; echo b > mix/b.log
  echo n > tracked/new.txt; echo n > tracked/newdir/f.txt
  ln -s tracked/t.txt link-untracked
  git init -q nested; (cd nested && echo n > n.txt && git add n.txt && git commit -qm n)
  git init -q nested_empty
  mkdir -p deepempty/a/b
  run f02_untracked
}
f03_ignored() {
  new f03_ignored
  printf '*.log\n!keep.log\nbuild/\n/rootonly.txt\nlogs/\n!build/keep.txt\n' > .gitignore
  mkdir sub; printf '*.tmp\n!important.tmp\n' > sub/.gitignore
  echo t > tracked.log; git add -f tracked.log; commit
  echo modified >> tracked.log                 # tracked file matching an ignore rule
  echo secret > secret.txt; echo 'secret.txt' >> .git/info/exclude
  echo '*.bak' > "$HOME/excludes"; git config core.excludesFile "$HOME/excludes"; echo b > x.bak
  echo l > a.log; echo k > keep.log; echo r > rootonly.txt
  mkdir -p deeper; echo r > deeper/rootonly.txt
  mkdir -p build/obj; echo o > build/obj/o.o; echo k > build/keep.txt
  mkdir logs; echo l > logs/1.txt
  echo t > sub/x.tmp; echo i > sub/important.tmp
  mkdir -p untr; echo u > untr/u.txt; echo l > untr/u.log
  run f03_ignored
}
f04_conflicts() {
  new f04_conflicts
  lines uu > uu.txt; lines ud > ud.txt; lines du > du.txt; lines rd > rd.txt; lines dr > dr.txt
  lines rr 30 > rr.txt; commit base
  git checkout -qb theirs
  echo theirs >> uu.txt; lines aa-theirs > aa.txt; git rm -q ud.txt; echo theirs >> du.txt
  git rm -q rd.txt; git mv dr.txt dr-theirs.txt; git mv rr.txt rr-theirs.txt; commit theirs
  git checkout -q main
  echo ours >> uu.txt; lines aa-ours > aa.txt; echo ours >> ud.txt; git rm -q du.txt
  git mv rd.txt rd-ours.txt; git rm -q dr.txt; git mv rr.txt rr-ours.txt; commit ours
  git merge -q theirs >/dev/null 2>&1
  run f04_conflicts
}
f05_ita() {
  new f05_ita
  lines x 40 > x.txt; commit
  echo n > new.txt; git add -N new.txt         # intent-to-add
  : > empty.txt; git add -N empty.txt          # intent-to-add, empty file
  mv x.txt y.txt; git add -N y.txt             # worktree rename to an ita entry
  run f05_ita
}
f06_typechange() {
  new f06_typechange
  echo l > l.txt; echo s > s.txt; echo x > x.sh; echo t > target; commit
  rm l.txt; ln -s target l.txt                 # unstaged T
  rm s.txt; ln -s target s.txt; git add s.txt  # staged T
  chmod +x x.sh                                # exec bit, fileMode true
  run f06_typechange
  new f06_filemode_false
  echo x > x.sh; echo y > y.sh; commit; git config core.fileMode false
  chmod +x x.sh                                # exec bit, fileMode false
  chmod +x y.sh; echo more >> y.sh             # exec bit + content
  run f06_filemode_false
}
f07_eol() {
  new f07_autocrlf
  printf 'a\nb\n' > lf.txt; commit; git config core.autocrlf true
  printf 'a\r\nb\r\n' > lf.txt                 # stat dirty, normalises equal
  run f07_autocrlf
  new f07_eol_attr
  printf '*.crlf eol=crlf\n' > .gitattributes; printf 'a\nb\n' > f.crlf; commit
  printf 'a\r\nb\r\n' > f.crlf                 # already CRLF on checkout; rewrite: stat dirty, normalises equal
  printf 'a\nb\n' > g.crlf; git add g.crlf; git commit -qm g
  touch -d '2001-01-01' g.crlf                 # LF on disk where checkout would write CRLF; stat dirty
  run f07_eol_attr
  new f07_text_renorm
  printf 'a\r\nb\r\n' > crlf.txt; printf 'a\r\nb\r\n' > crlf2.txt; commit   # CRLF stored in the blob
  printf '*.txt text\n' > .gitattributes; git add .gitattributes; git commit -qm attrs
  touch -d '2001-01-01' crlf.txt               # stat dirty; clean would now give LF != blob
  run f07_text_renorm
}
f08_filter() {
  new f08_filter
  git config filter.upper.clean 'tr a-z A-Z'; git config filter.upper.smudge cat
  printf '*.up filter=upper\n' > .gitattributes
  echo hello > a.up; echo hello > b.up; commit           # blobs are HELLO
  touch -d '2001-01-01' a.up                             # stat dirty, cleaned content equal
  echo Hello > b.up                                      # content differs only before cleaning
  echo hello > c.up; git add c.up; git commit -qm c; echo hellx > c.up   # differs after cleaning too
  git config filter.upper.clean "echo \"\$(date +%s.%N) \$PPID\" >> $FX/f08_filter.log; tr a-z A-Z"
  : > $FX/f08_filter.log; "$HERE/target/release/status-spike" gix "$FX/f08_filter" > /dev/null
  echo "  clean-filter runs by gix alone: $(wc -l < $FX/f08_filter.log)"
  : > $FX/f08_filter.log; "$HERE/target/release/status-spike" git "$FX/f08_filter" > /dev/null
  echo "  clean-filter runs by git alone: $(wc -l < $FX/f08_filter.log)"
  run f08_filter
}
f09_racy() {
  new f09_racy
  echo aaaa > r.txt; echo cccc > c.txt; echo dddd > d.txt; commit
  ts=$(stat -c '%y' r.txt)
  echo bbbb > r.txt; touch -d "$ts" r.txt                 # same size, same mtime as the entry
  touch -d "$ts" .git/index                               # index written in the same instant: racy
  run f09_racy
  new f09_racy_trustctime_false
  echo aaaa > r.txt; commit; git config core.trustctime false
  ts=$(stat -c '%y' r.txt)
  echo bbbb > r.txt; touch -d "$ts" r.txt                 # stat-identical but for ctime
  touch -d '+10 seconds' .git/index                       # not racy: index newer than the file
  run f09_racy_trustctime_false
  new f09_racy_checkstat_minimal
  echo aaaa > r.txt; commit; git config core.checkStat minimal
  echo bbbb > r.txt; touch -d '2001-01-01' .git/index; touch -d '2001-01-01' r.txt
  run f09_racy_checkstat_minimal
}
f10_skip() {
  new f10_skip
  echo s > sw.txt; echo a > au.txt; echo g > gone.txt; commit
  git update-index --skip-worktree sw.txt gone.txt; git update-index --assume-unchanged au.txt
  echo mod >> sw.txt; echo mod >> au.txt; rm gone.txt
  run f10_skip
}
f11_sparse() {
  for mode in cone sparse_index; do
    new f11_sparse_$mode
    mkdir a b c; for d in a b c; do lines $d > $d/x.txt; lines $d > $d/y.txt; done; echo top > top.txt; commit
    if [ $mode = cone ]; then git sparse-checkout set --cone a; else git sparse-checkout set --cone --sparse-index a; fi
    echo mod >> a/x.txt; echo mod >> top.txt
    mkdir -p b; echo new > b/new.txt              # untracked file in an excluded directory
    run f11_sparse_$mode
  done
}
f12_split_untracked_cache() {
  new f12_split
  for f in a b c; do lines $f > $f.txt; done; commit
  git config core.splitIndex true; git update-index --split-index
  echo m >> a.txt; lines d > d.txt; git add d.txt; echo m >> b.txt; git add b.txt; echo n > u.txt
  run f12_split
  new f12_untracked_cache
  mkdir d; for f in a b c; do lines $f > d/$f.txt; done; commit
  git config core.untrackedCache true; git update-index --untracked-cache; git status >/dev/null
  echo n > d/new.txt; mkdir e; echo n > e/n.txt; echo m >> d/a.txt
  run f12_untracked_cache
}
f13_fsmonitor() {
  new f13_fsmonitor_hook_lies
  for f in a b; do lines $f > $f.txt; done; commit
  cat > .git/lying-fsmonitor <<'EOF'
#!/bin/sh
# fsmonitor hook protocol v2: print a token, NUL, then the changed paths. Reports nothing.
printf 'lying-token\0'
EOF
  chmod +x .git/lying-fsmonitor
  git config core.fsmonitor "$PWD/.git/lying-fsmonitor"; git config core.fsmonitorHookVersion 2
  git status >/dev/null; git status > /dev/null   # with locks: writes the FSMN extension
  echo changed >> a.txt; echo n > untracked.txt
  run f13_fsmonitor_hook_lies
  if git fsmonitor--daemon --help >/dev/null 2>&1 || git fsmonitor--daemon status >/dev/null 2>&1; [ $? -le 1 ]; then
    new f13_fsmonitor_daemon
    for f in a b; do lines $f > $f.txt; done; commit
    git config core.fsmonitor true
    git status >/dev/null; sleep 1; git status >/dev/null   # starts the daemon, writes FSMN
    echo changed >> a.txt; echo n > untracked.txt; sleep 1
    run f13_fsmonitor_daemon
    git fsmonitor--daemon status; git fsmonitor--daemon stop
  fi
}
f14_submodules() {
  rm -rf "$FX/f14_origin"; git init -q "$FX/f14_origin"
  (cd "$FX/f14_origin" && lines s > s.txt && git add s.txt && git commit -qm s)
  new f14_submodules
  echo top > top.txt; commit
  for n in clean mod newc untr ign_dirty ign_untracked ign_all uninit; do
    git submodule add -q "$FX/f14_origin" sub_$n >/dev/null 2>&1
  done
  git config -f .gitmodules submodule.sub_ign_dirty.ignore dirty
  git config -f .gitmodules submodule.sub_ign_untracked.ignore untracked
  git config -f .gitmodules submodule.sub_ign_all.ignore all
  commit subs
  git submodule deinit -q -f sub_uninit
  echo m >> sub_mod/s.txt
  (cd sub_newc && echo n > n.txt && git add n.txt && git commit -qm n)
  echo u > sub_untr/u.txt
  echo m >> sub_ign_dirty/s.txt; echo u > sub_ign_dirty/u.txt
  echo u > sub_ign_untracked/u.txt; echo m >> sub_ign_untracked/s.txt
  (cd sub_ign_all && echo n > n.txt && git add n.txt && git commit -qm n)
  run f14_submodules
}
f15_icase() {
  new f15_icase
  echo f > File.txt; commit; git config core.ignoreCase true
  mv File.txt file.txt
  run f15_icase
}
f16_unborn_detached() {
  new f16_unborn
  echo a > a.txt; mkdir d; echo b > d/b.txt; git add a.txt d; echo u > u.txt; echo m >> a.txt
  run f16_unborn
  new f16_detached
  echo a > a.txt; commit; echo b > b.txt; commit; git checkout -q --detach HEAD~1
  echo m >> a.txt; echo n > n.txt; git add n.txt
  run f16_detached
}
f17_linked_worktree() {
  rm -rf "$FX/f17_linked"; new f17_main
  echo a > a.txt; echo b > b.txt; commit
  git worktree add -q "$FX/f17_linked" -b wt
  cd "$FX/f17_linked"; echo m >> a.txt; rm b.txt; echo n > n.txt; echo s > s.txt; git add s.txt
  run f17_linked_worktree "$FX/f17_linked"
}

f01_many_renames() {
  new f01_many_renames
  for i in $(seq 1 40); do lines "file$i" 30 > f$i.txt; done; commit
  for i in $(seq 1 40); do git mv f$i.txt g$i.txt; echo "edit $i" >> g$i.txt; done; git add -A   # 40 inexact renames
  run f01_many_renames
}
f12_index_formats() {
  new f12_index_v4
  for f in a b c; do lines $f > $f.txt; done; mkdir d; lines d > d/long-name-for-prefix-compression.txt; commit
  git update-index --index-version 4; echo m >> a.txt; lines e > d/e.txt; git add d/e.txt; echo u > u.txt
  run f12_index_v4
  new f12_skip_hash
  git config index.skipHash true
  for f in a b c; do lines $f > $f.txt; done; commit; echo m >> a.txt; lines e > e.txt; git add e.txt
  run f12_skip_hash
}
f16_sha256() {
  rm -rf "$FX/f16_sha256"; git init -q --object-format=sha256 "$FX/f16_sha256"; cd "$FX/f16_sha256"
  lines a > a.txt; lines b > b.txt; commit; echo m >> a.txt; git mv b.txt c.txt; echo u > u.txt
  run f16_sha256
}
f18_names() {
  new f18_names
  echo s > 'with space.txt'; echo u > 'ünïcödé.txt'; echo d > ./-dash.txt; printf 'n\n' > "$(printf 'new\nline.txt')"
  echo b > 'back\slash.txt'; echo q > 'quo"te.txt'; commit
  for f in 'with space.txt' 'ünïcödé.txt' ./-dash.txt "$(printf 'new\nline.txt')" 'back\slash.txt' 'quo"te.txt'; do echo m >> "$f"; done
  echo n > 'tab	name.txt'; mkdir 'dir with space'; echo n > 'dir with space/x'
  run f18_names
}

f19_lfs() {
  new f19_lfs
  git lfs install --local >/dev/null; git lfs track '*.bin' >/dev/null
  head -c 4096 /dev/urandom > a.bin; head -c 4096 /dev/urandom > b.bin; head -c 4096 /dev/urandom > c.bin; commit lfs
  touch -d '2001-01-01' a.bin                                  # stat dirty, content unchanged
  head -c 4096 /dev/urandom > b.bin                            # content changed, same size
  mkdir -p "$FX/lfswrap"; printf '#!/bin/sh\necho "$*" >> %s/lfswrap/log\nexec /usr/bin/git-lfs "$@"\n' "$FX" > "$FX/lfswrap/git-lfs"; chmod +x "$FX/lfswrap/git-lfs"
  : > "$FX/lfswrap/log"; PATH="$FX/lfswrap:$PATH" "$HERE/target/release/status-spike" gix "$FX/f19_lfs" > /dev/null
  echo "  git-lfs runs by gix alone: $(wc -l < "$FX/lfswrap/log") ($(sort -u "$FX/lfswrap/log" | tr '\n' ';'))"
  : > "$FX/lfswrap/log"; PATH="$FX/lfswrap:$PATH" "$HERE/target/release/status-spike" git "$FX/f19_lfs" > /dev/null
  echo "  git-lfs runs by git alone: $(wc -l < "$FX/lfswrap/log") ($(sort -u "$FX/lfswrap/log" | tr '\n' ';'))"
  run f19_lfs
}

ALL="f01_basic f01_rename_staged f01_many_renames f01_worktree_rename f01_copies f02_untracked f03_ignored f04_conflicts f05_ita f06_typechange f07_eol f08_filter f09_racy f10_skip f11_sparse f12_split_untracked_cache f12_index_formats f13_fsmonitor f14_submodules f15_icase f16_unborn_detached f16_sha256 f17_linked_worktree f18_names f19_lfs"
for f in ${@:-$ALL}; do (set +e; $f); done
```

## Appendix C: the cost harness

`bench.sh`:

```bash
#!/usr/bin/env bash
# bench.sh <rust-copy> — cost of gix status against git status on a scratch copy of the
# rust-lang/rust bench repository. Each scenario: agreement check (normalised diff), then
# `status-spike bench` (warm-up + 7 runs, median [min-max]) with untracked=no and =normal.
set -u
HERE=$(dirname "$(readlink -f "$0")")
S=$(dirname "$HERE")
export HOME=$S/home GIT_CONFIG_NOSYSTEM=1
unset XDG_CONFIG_HOME
B=$HERE/target/release/status-spike
R=$1
cd "$R"

agree() {
  for m in "--untracked no" "--untracked normal"; do
    "$B" gix "$R" $m > "$TMPDIR"/gix.out 2>/dev/null; "$B" git "$R" $m > "$TMPDIR"/git.out
    if cmp -s "$TMPDIR"/gix.out "$TMPDIR"/git.out; then echo "  agree [$m]: AGREE ($(wc -l < "$TMPDIR"/git.out) lines)";
    else echo "  agree [$m]: DIFFER"; diff "$TMPDIR"/gix.out "$TMPDIR"/git.out | grep '^[<>]' | head -6; fi
  done
}
measure() {
  echo "== $1"
  agree
  for m in "--untracked no" "--untracked normal"; do
    echo "  [$m] $("$B" bench "$R" $m --runs 7 "${@:2}")"
  done
}

# settle: let every file's mtime fall a full second behind the index, then let a locked git
# status rewrite the index, so no entry is racily clean (git rewrites when one is).
settle() { sleep 1.5; git update-index -q --refresh; git status >/dev/null; }
settle
measure "a: clean tree"

mapfile -t FILES < <(git ls-files -- '*.rs' '*.md' '*.toml' | awk 'NR % 40 == 0' | head -1000)
for f in "${FILES[@]}"; do echo "// spike" >> "$f"; done
measure "b: ${#FILES[@]} modified files"
git checkout -q -- "${FILES[@]}"; git update-index -q --refresh
measure "e: the same ${#FILES[@]} files restored and refreshed in the same second (racily clean entries)"
settle

mapfile -t DIRS < <(git ls-files | sed -n 's|/[^/]*$||p' | sort -u | awk 'NR % 4 == 0' | head -1000)
for d in "${DIRS[@]}"; do for i in 0 1 2 3 4 5 6 7 8 9; do echo u > "$d/spike-untracked-$i.txt"; done; done
measure "c: $(( ${#DIRS[@]} * 10 )) untracked files in ${#DIRS[@]} tracked directories"
# untracked cache, populated by one locked git status after the files exist (the user's own
# last `git status`); Cairn's reads never write it, so this is its best case.
git config core.untrackedCache true; git update-index --untracked-cache; git status >/dev/null; git status >/dev/null
measure "c-uc: as c, with core.untrackedCache=true and a freshly written UNTR extension"
for d in "${DIRS[@]}"; do rm -f "$d"/spike-untracked-*.txt; done
git status >/dev/null
measure "a-uc: clean tree, core.untrackedCache=true, fresh UNTR"
git config --unset core.untrackedCache; git update-index --no-untracked-cache; settle
if [ "${FSMONITOR:-1}" = 1 ]; then
  git config core.fsmonitor true; git status >/dev/null; sleep 2; git status >/dev/null; git status >/dev/null
  measure "a-fsm: clean tree, core.fsmonitor=true (builtin daemon), token written"
  for f in "${FILES[@]}"; do echo "// spike" >> "$f"; done; sleep 2
  measure "b-fsm: ${#FILES[@]} modified files, core.fsmonitor=true, token from before the edits"
  git checkout -q -- "${FILES[@]}"; git status >/dev/null
  git fsmonitor--daemon stop; git config --unset core.fsmonitor; git update-index --no-fsmonitor; settle
fi

git ls-files -z | xargs -0 touch -c --
measure "d: every tracked file touched (stat dirty, content unchanged)"
settle
measure "a2: clean tree again, after refresh"
```

`bench-staged.sh` (scenario f):

```bash
#!/usr/bin/env bash
# bench-staged.sh <rust-copy> — 1000 staged modifications (the cache-tree invalidated along
# their directories), agreement and cost, then restore.
set -u
HERE=$(dirname "$(readlink -f "$0")"); S=$(dirname "$HERE")
export HOME=$S/home GIT_CONFIG_NOSYSTEM=1
B=$HERE/target/release/status-spike; R=$(readlink -f "$1"); cd "$R"
mapfile -t FILES < <(git ls-files -- '*.rs' '*.md' '*.toml' | awk 'NR % 40 == 0' | head -1000)
for f in "${FILES[@]}"; do echo "// spike" >> "$f"; done
git add -- "${FILES[@]}"; sleep 1.5; git status >/dev/null
echo "== f: ${#FILES[@]} staged modifications"
for m in no normal; do
  "$B" gix "$R" --untracked $m > "$S/g1"; "$B" git "$R" --untracked $m > "$S/g2"
  cmp -s "$S/g1" "$S/g2" && echo "  agree [--untracked $m]: AGREE ($(wc -l < "$S/g2") lines)" || { echo "  agree [--untracked $m]: DIFFER"; diff "$S/g1" "$S/g2" | head; }
  echo "  [--untracked $m] $("$B" bench "$R" --untracked $m --runs 7)"
done
git reset -q -- "${FILES[@]}"; git checkout -q -- "${FILES[@]}"; sleep 1.5; git update-index -q --refresh; git status >/dev/null
echo "  restored: $(git status --porcelain | wc -l) status lines"
```
