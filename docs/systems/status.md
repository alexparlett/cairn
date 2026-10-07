# Status

How the engine answers what has changed in a working tree: which paths are staged,
changed, in conflict or untracked, and the states in which git lists nothing. The
answer is git's own — `git status --porcelain=v2 -z`, run as a read — parsed into plain
data. Nothing here writes. What draws it: the title bar's star for a repository with a
change (`docs/systems/history-graph.md`, "The title bar"), the count beside the sidebar's
Local Changes (`docs/systems/sidebar.md`), and Local Changes' two lists, laid out over it on
the refresh thread (`docs/systems/local-changes.md`); a refresh reads it on that thread
(`history-graph.md`, "Refresh").

Spec: `docs/prd/refs-and-status.md` (R3). Design: `docs/design/engine.md` (D1, the
fourth read git answers). Evidence:
`docs/research/refs-and-status/status-agreement-spike.md`.

## The answer

`Repository::status(&git, &cancel)` (`crates/cairn-git/src/status.rs`) answers a
`cairn_model::WorkingTreeStatus` (`crates/cairn-model/src/status.rs`):

- `Listed(entries)` — what git listed, in its order: changed tracked paths, then
  conflicts, then untracked paths; empty for a clean tree. Each `StatusEntry` is
  - `Changed(ChangedEntry)` — a path, its `StagedChange` (`HEAD` against the index:
    added, modified, deleted, type changed, or renamed or copied with its source and
    similarity), its `UnstagedChange` (the index against the working tree: modified,
    deleted, type changed, intent-to-add, or an intent-to-add entry git paired as a
    rename or copy of a tracked path), and, for a submodule on any side, its
    `SubmoduleState` (new commits, modified content, untracked content);
  - `Conflicted(ConflictedEntry)` — a path with unmerged index entries and its
    `ConflictKind`, one of git's seven, each decided by which of the three stages the
    path has entries in (`ConflictKind::from_stages`) and printed as git's two letters
    (`ConflictKind::code`);
  - `Untracked(path)` — one per file; a nested repository is listed as its directory,
    with its trailing `/`, as git lists it in every mode.
- `IndexUnreadable(UnreadableIndex::Sparse)` — the index is a sparse index and the
  git in use is older than 2.32, which cannot read one. Never an empty or partial list.
- `NoWorkingTree` — a bare repository; no `git` runs.

Every path is the bytes git printed (`RepoPath`): `-z` quotes nothing and ends each
record with a NUL, so a name with a newline, a tab, a leading space or bytes that are
not UTF-8 arrives whole, and the parser splits on nothing else.

## The read

`crate::reads::status` (`crates/cairn-git/src/reads/status.rs`) runs, as a read
invocation (`GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`, no askpass token):

```text
git --git-dir=<dir> --work-tree=<top> status --porcelain=v2 -z
```

and, only where that answer listed an untracked directory as one `dir/` record, once
more with `--untracked-files=all`, whose answer replaces the first whole. Nothing else
is ever passed: no `--ignored` (an ignored file is never listed), no `-M`,
`--no-renames` or `--find-renames` (renames and copies are what `status.renames`,
`status.renameLimit` and `diff.renames` make them), no `--ignore-submodules` (a
submodule is reported as its `submodule.<name>.ignore` and `diff.ignoreSubmodules`
say). Pinned by `the_reads_are_status_porcelain_v2_and_all_untracked_only_after_a_collapse`
and `a_first_answer_with_no_collapsed_directory_is_the_answer`, over a stub `git` that
records its argv, and `a_status_read_runs_with_the_read_environment`.

**Why two reads.** Untracked files are listed one per file, as Fork lists them, unless
the user set `status.showUntrackedFiles=no`. That setting has to be read as git reads
it — git 2.56 takes `false` and `true` for it where 2.30 refuses both as a bad
configuration, and gix's reading of a linked worktree's `includeIf` and of the system
file is not git's — so git reads it: the first read passes no `--untracked-files`, and
under `no` git lists no untracked path, which is the answer. Only a collapsed directory
shows the user did not say `no` and git used its `normal` mode, and only then is the
second read worth its cost. A nested repository is listed as `dir/` under every mode,
so a tree holding one is always read twice (#78). No `git config` read is added, so a read
here runs `git status` and nothing else.

**Parsing.** Porcelain v2's `1` (an ordinary change), `2` (a rename or copy, whose
source is the next NUL-terminated record), `u` (unmerged) and `?` (untracked) records
are read; a `#` header is skipped, as git's documentation tells a parser to (none is
asked for); an `!` record, an unknown kind, a field missing, a mode that is not six
octal digits, an object id that is not 40 or 64 hex digits, an empty path, a score that
is not `R` or `C` and a percentage, or a `2` record whose two sides and score disagree
(git keeps one rename per entry) is `Error::UnexpectedGitOutput`, and nothing of that
answer is used. Each letter is held to the side it may appear on: `A` on the unstaged
side is an intent-to-add entry, and `R` or `C` there is the one pairing git's status
makes between the index and the working tree. Pinned in `reads/status.rs`
(`each_letter_of_an_ordinary_record_is_its_own_change`,
`a_path_is_its_bytes_whatever_they_hold`,
`a_rename_or_copy_takes_the_next_record_as_its_source_on_its_own_side`,
`each_unmerged_code_is_its_own_kind`, `each_submodule_letter_is_its_own_flag`,
`what_git_does_not_print_here_is_refused_and_a_header_skipped`).

**Failure.** Classified by exit status and the repository's state, never by stderr,
which is prose in the user's language. A failed read on a git older than 2.32 whose
index gix reads as sparse is `IndexUnreadable(Sparse)` — `read-cache.c` learnt the
`sdir` extension in v2.32.0 — and every other failure is `Error::GitFailed`
(`a_failure_is_the_unreadable_state_only_on_an_old_git_over_a_sparse_index`,
`a_failed_read_that_is_not_a_sparse_index_is_an_error`).

**Cancellation.** The cancel is polled before each read and by the runner while git
runs: a superseded read ends git's process group — whatever git started with it, such
as a `core.fsmonitor` hook — and answers `Error::StatusCancelled`, leaving nothing in
the repository's registry, whichever of the two reads it was in
(`a_superseded_status_read_ends_its_process_group_and_leaves_nothing_running`,
`a_status_read_superseded_during_its_second_read_ends_it`,
`a_status_read_superseded_before_it_starts_runs_nothing`). In the application, only a
close cancels a status: a refresh leaves one that is running to finish and asks one more
after it (refs-and-status R10.3; `docs/systems/history-graph.md`, "Refresh").

## What a status read writes and runs

Under `GIT_OPTIONAL_LOCKS=0` git writes no index — not the refreshed stat information,
not the untracked cache, not the fsmonitor token — and takes no lock
(`a_status_read_leaves_the_index_byte_identical`, which also shows that git with locks
allowed rewrites that index; every fixture of `crates/cairn-git/tests/status.rs` holds
the superproject's and each submodule's index byte-identical).
`a_status_read_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor` holds the
whole git directory byte-identical, every mtime unchanged but the shared index's, under
an untracked cache, an fsmonitor hook answering a token and a split index, all of which a
locked status would rewrite; and with a caching textconv, an external diff, a driver's
`command`, a smudge filter and a `post-index-change` hook configured it runs none of
them — only the fsmonitor hook and the clean filter, and nothing but `git status`.
Residuals, accepted as parity with the user's own `git status` (but the last, which is the
read policy's):

- under a split index git advances `sharedindex.*`'s mtime, and under a sparse index
  the mtime of the loose tree objects it expands, bytes unchanged in both;
- it runs the repository's `core.fsmonitor` hook or daemon, the clean filter of each
  stat-dirty file it rehashes, and `git status` inside each submodule it looks into,
  with that repository's own hook and filters — the programs D1 allows a read;
- because a read never writes the refreshed stat back, a tree whose every file's stat
  changed is rehashed in full on every read until something refreshes the index, which
  nothing in Cairn does (#66);
- staged rename detection compares blobs, and a read never lazily fetches one
  (`GIT_NO_LAZY_FETCH=1`, `crate::reads`): in a partial clone, a staged rename whose
  blob only the promisor holds fails the WHOLE read on git 2.44 and later —
  `Error::GitFailed`, nothing listed, no pack written — where the user's own `git status`
  would fetch the blob and answer; git before 2.44 ignores the variable and fetches,
  writing a pack (`in_a_partial_clone_a_status_read_fails_rather_than_fetching`, which
  pins both).

## Parity, and where it is checked

`crates/cairn-git/tests/status.rs` compares every answer with oracles that are not
`git status`: `git diff --cached --name-status` and `git diff --name-status` under the
rename detection the fixture configures (with `diff.renames` off for the oracle alone,
since `-C` on top of `diff.renames=copies` is "find copies harder"), `git ls-files
--others --exclude-standard`, `git ls-files -u` read through
`ConflictKind::from_stages`, and each submodule's own state masked by the `ignore`
level the fixture set. `git diff` is not the oracle for a submodule: on 2.32.7 and
2.56.0, though not on 2.30.9, it leaves out one whose only change is untracked content,
which status reports. The fixtures
cover every kind of entry, all seven conflicts from one real merge, renames and copies
under each `status.renames` value and under `diff.renames`, forty inexact renames,
intent-to-add entries and their pairs, type and mode changes, submodules under each
`ignore` value, `diff.ignoreSubmodules` and `status.showUntrackedFiles=no`, untracked
files under each `status.showUntrackedFiles` value, awkward names, an unborn branch and
a sparse index. `scripts/git-floor.sh` runs the file under git 2.30.9 and 2.32.7 as
well; on 2.30.9 the sparse index answers `IndexUnreadable`, its fixture built by the
first git on `PATH` that can write one.

Cost is measured by the `#[ignore]`d `measures_the_status_read` in the same file, on the
repository `CAIRN_BENCH_REPO` names (read only) and a scratch clone of it.
