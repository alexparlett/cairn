# Progress — diff-engine

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-10-03 — Phase 03 QA: every confirmed finding fixed, test-first

**The round:** 19 raw findings over phase 03 (working-tree diffs), 16 confirmed by
`qa-confirm`, QC3 dismissed with evidence. TC6 and DO1 were treated as critical under
the user's parity rule (a divergence from what git shows is a critical bug). Each fix
below was written test-first: the test shown RED on the code before it (or, for a
test-coverage finding, RED under the mutation the finding named), then GREEN, on git
2.30.9, 2.32.7 and 2.56.0. Every git behaviour a fix relies on was reproduced on those
three, not taken from memory.

- **GI1** — `a_sparse_index_is_unsupported_and_says_so` skipped whenever `sparse-checkout
  init` failed, on any git. RED: init sabotaged on 2.56.0, the test passed with
  `SKIPPED`. Now it skips only below 2.32 (`since`, moved to `tests/diff/mod.rs`, its
  third user) and asserts the setup succeeded; the sabotage fails it. The deprecated
  `init --cone --sparse-index` stays: `set --cone --sparse-index` writes no sparse
  index on 2.32.7 (no `sdir` extension), where `init` does on 2.32.7 and 2.56.0.
- **TC6** — a staged mode-only change of a file past the ceiling answered `TooLarge`
  (RED: `TooLarge { 4096, 8890 }`); `git diff --cached` shows `old mode`/`new mode`, and
  a commit answers `ModeChangeOnly`. The raw-only arm now answers `ModeChangeOnly` for a
  modification naming one non-null blob under two modes
  (`a_large_file_whose_stat_or_mode_alone_moved_is_what_git_diff_shows`, extended to the
  staged side and checked against `git diff --cached`).
- **DO1** — an untracked file named `-` read as standard input: an empty file added
  (RED: no hunks, where `git diff --no-index -- /dev/null ./-` shows its lines).
  Reproduced: git prints the operand verbatim — `./-` on the `-z` raw record and in
  the patch headers — and resolves the attributes of `./<p>` as those of `<p>`; but it
  passes the operand to a clean filter as its `%f`, so `./sub/x` hands a driver a name
  the user's `git diff --no-index -- /dev/null sub/x` does not. **Decided without
  asking, for parity:** only `-` is respelled `./-` (git's own advice in
  `diff-no-index.c`), every other path passed as itself; the record for `./-` is named
  `-` again (`no_index_operand`, `one_for`). Files `-` and `-x` added to
  `an_untracked_file_is_what_git_diff_no_index_shows`.
- **TC2** — `side.unwrap_or(self.ceiling)` was untested. New
  `a_working_tree_side_is_counted_at_the_limit_so_a_large_one_within_it_is_read`: a
  220,000-byte untracked file and a 220,000-byte unstaged edit of a 6-byte indexed
  file, default limits, each equal to git's. Mutation `unwrap_or(0)`: RED for both.
- **TC1** — the third stale-read check (the `-w` read names the same object) was
  untested. Filter invocations, reproduced: an exact `diff-files -p --full-index` runs
  the clean filter twice on all three gits, a `-w` one twice on 2.30.9 and 2.32.7 and
  four times on 2.56.0, a raw-only one never. New
  `a_whitespace_ignoring_read_of_other_content_is_the_error_a_caller_retries`: a filter
  whose output gains a trailing space on line 20 from the third read on (stable within
  one process), an edit on line 1; the `-w` read's printed lines agree, its id does
  not. Mutation `if false && …`: RED (`Ok(Some(..))`).
- **TC3** — the `.saturating_mul(2)` boundary. New
  `a_rewrite_of_every_line_within_the_limit_is_read_whole`: a staged rewrite of 40,000
  one-byte lines a side, each side just under half a 160,002-byte limit. Mutation
  `.saturating_mul(1)`: RED (`TooLarge { 160002, 160003 }`).
- **TC5** — new `line_endings_at_the_edges_read_as_git_diff_shows_them`, staged and
  unstaged: an edit beside an unterminated last line, a change to the final newline
  alone (both ways), a `-text` CRLF file edited mid-file, each `same_as_git`. Mutation
  of `PatchText::new_side` building every context line terminated: RED.
- **DO2, DO4, DO5** — `--no-index` reads whatever it is given. RED: an absolute path
  and `../<dir>/outside` answered the contents of a file outside the working tree;
  `./plain`, `dir/./inner`, `dir/../plain` answered; a named pipe gave no answer in
  30 s. Reproduced: git 2.56.0 waits on a FIFO for a writer, 2.30.9 and 2.32.7 print a
  gitlink record for it and fail (`cannot hash`), and every git diffs `/dev/null`
  against `<dir>/null` for a directory. Now `reads::work_tree_relative` refuses an
  empty or absolute path or a `.`/`..` component as `Error::NotAWorkTreePath` (a new
  variant) before anything runs, and a path that is neither a regular file nor a
  symlink by `symlink_metadata` is `Unsupported` before git starts
  (`an_untracked_path_outside_the_working_tree_or_not_a_file_is_refused_before_git_runs`,
  `only_a_work_tree_relative_path_is_read_untracked`). Check 10 of
  `destructive-ops-reviewer` reworded to what the code enforces.
- **DO3** — `filter.<driver>.process` (what `git lfs install` configures) runs on
  `diff-files` and `--no-index`. **A fixture, not a residual:** a pkt-line filter
  server in POSIX `sh` and `dd` (no dependency, no test binary).
  `a_long_running_filter_process_is_sent_only_clean_and_its_form_is_diffed`: a staged
  read starts it not at all; the working-tree reads send it `command=clean` and
  nothing else, and every answer is git's. A pin of behaviour already right, so no
  RED. Documented in `reads/mod.rs`, `reads/working_tree.rs`, `engine.md` ("Reads see
  git's form"), the root `CLAUDE.md` D1 bullet and `docs/systems/diff.md`.
- **TC4** — "naming the path" could not fail: `GitFailed`'s message carries the argv.
  RED: with git's stderr blanked in `working_tree_patch`, the old test still passed.
  Renamed `a_failing_clean_filter_is_gits_failure_with_its_diagnostic_or_what_git_shows`
  (it replaces `a_failing_clean_filter_is_an_error_naming_the_path_or_what_git_shows`,
  cited in an earlier entry): status 128 and a non-empty stderr, which the blanking
  fails; and a `required` filter whose program does not exist (exit 128 on all three)
  is an error, never no change. The `working_tree_patch` and
  `Repository::working_tree_diff` docs say "git's failure with its diagnostic".
- **GI2** — new guard `the_one_porcelain_read_is_diff_no_index_in_the_working_tree_read`
  (self-test `the_porcelain_read_matcher_catches_the_shapes_it_claims`, over the new
  `cairn_guards::production_string_literals`): in `reads/` production code the exact
  literal `"diff"` — plain, byte or raw — appears only in `reads/working_tree.rs`,
  once, with `"--no-index"` the next literal on its line, and `"/dev/null"` in that
  file. Its first run found four other `"diff"`s: two error labels in `reads/patches.rs`
  (relabelled `"a working-tree read"`) and the `diff` attribute's two lines in
  `reads/attributes.rs`, excused by the roster `DIFF_ATTRIBUTE_LINES` (file and exact
  line, each row required to still match). RED: a planted `["diff", "--no-index"]` in
  `reads/changes.rs` failed it. Scoped now, as asked: a verb built at run time
  (`format!`, `concat!`, bytes) is the stated review obligation (`reads/mod.rs`, root
  `CLAUDE.md`, check 10). **Candidate follow-up for the user:** a full roster guard
  over every verb a read runs.
- **GI3, GI4, QC1, QC2** — the mode is written exactly, `git diff --no-index --
  /dev/null <path>` with `<path>` work-tree-relative and `./-` for `-`, in the root
  `CLAUDE.md`, `reads/mod.rs`, `process/binary.rs` and `docs/qa-gate.md`; the driver's
  environment names `GIT_ASKPASS`, `SSH_ASKPASS` and `CAIRN_ASKPASS_SOCKET` (a driver
  can reach the socket, and with no token fails closed) in `CLAUDE.md`, `engine.md`
  and `git-processes.md`; and "`GIT_DIR`/`GIT_WORK_TREE` reach the driver" is now
  conditional on Cairn naming the repository (full trust) in `CLAUDE.md`, `engine.md`,
  `git-processes.md` and `diff.md` — reproduced: with `--git-dir`/`--work-tree` the
  driver sees both, left to discovery it sees neither, on all three gits.
- `scripts/git-floor.sh`'s floors raised to one under the new counts (60 and 83).

## 2026-10-03 — `git diff --no-index` accepted, and its presentation pinned

**The user decided (2026-10-03):** an untracked file is read with porcelain `git diff
--no-index -- /dev/null <path>`, accepted as a named exception to the reads rule
(query plumbing or `status` only). The phase 03 entry below asked for it.

What finishing it took:

- `crates/cairn-git/src/reads/mod.rs`, "What a read may run", names it as the single
  porcelain exception, why (no plumbing prints an untracked file in git's form;
  putting it in an index to ask is a write) and the evidence (whole-`.git`
  snapshots on 2.30.9, 2.32.7 and 2.56.0, and the tests that hold the git directory
  byte-identical after each such read).
- `.claude/agents/destructive-ops-reviewer.md` check 10 names that one mode as
  accepted, built only by `reads::working_tree_patch`, and says any other `git diff`
  — without `--no-index`, against anything but `/dev/null` and the one path, or
  built elsewhere — is still a finding. No guard enumerates read verbs (searched
  `crates/cairn-guards`), so none changed.
- **Porcelain reads presentation settings plumbing does not.** By experiment
  (`git diff --no-index -z --raw --no-abbrev -p --full-index -U3 --no-ext-diff
  --no-textconv --no-color`, an untracked file with a blank line, a tab, a trailing
  space and a path holding a space and `é`, the output's bytes hashed per key, on
  2.30.9, 2.32.7 and 2.56.0), of `diff.noprefix`, `diff.mnemonicPrefix`,
  `diff.srcPrefix`, `diff.dstPrefix`, `diff.context` (0 and 10),
  `diff.interHunkContext`, `diff.suppressBlankEmpty`, `color.ui`, `color.diff`,
  `color.diff.meta`, `diff.colorMoved`, `diff.wsErrorHighlight`, `diff.relative`,
  `core.quotePath`, `diff.external`, `diff.orderFile`, `diff.indentHeuristic`,
  `diff.algorithm`, `core.whitespace`, `diff.renames`, `core.abbrev`,
  `diff.statGraphWidth`, `diff.dirstat`, `pager.diff`, `core.pager`,
  `log.showSignature`, `diff.submodule`, `diff.ignoreSubmodules`,
  `diff.autoRefreshIndex`, `core.safecrlf` and `core.autocrlf`, the ones that
  changed the output were `diff.noprefix`, `diff.mnemonicPrefix`,
  `diff.srcPrefix`/`diff.dstPrefix` (2.56 only; they arrived in 2.45) and
  `core.quotePath` — and `core.autocrlf`, on stderr only (its warning). The rest are
  already decided by a flag (`-U`, `--no-color`, `--no-ext-diff`, `--no-textconv`,
  `--no-abbrev`, `--full-index`) or cannot act on one all-added file.
- **Neutralised** on the `--no-index` read with `-c`, as git's defaults
  (`NO_INDEX_PRESENTATION` in `reads/working_tree.rs`): `diff.noprefix=false`,
  `diff.mnemonicPrefix=false`, `diff.srcPrefix=a/`, `diff.dstPrefix=b/`,
  `core.quotePath=true`, `diff.interHunkContext=0`, `diff.relative=false`,
  `diff.orderFile=/dev/null`, `diff.suppressBlankEmpty=false` — the four that
  changed it, and the presentation keys that would once a file had context or a
  second hunk. With them set, the output hashed identically with these set hostile at
  once — the nine, `diff.context=0`, `color.ui` and `color.diff` `always`,
  `diff.colorMoved`, `diff.wsErrorHighlight`, `diff.external`, `core.abbrev=4` — on
  all three gits. **Kept** as parity: `core.autocrlf`,
  `core.eol`, `core.safecrlf`, the attributes and the filter drivers (they decide git's
  form of the file); `diff.algorithm` and `diff.indentHeuristic` (no flag is passed
  for an untracked file, whose one change is every line under any algorithm);
  `-U<n>` stays the view's context. Plumbing reads keep only
  `-c diff.suppressBlankEmpty=false`, as before.
- Pins: `an_untracked_answer_is_the_same_under_hostile_presentation_settings` (every
  neutralised key hostile, colour forced, `diff.context=0`, an external diff
  configured: the answer equals the calm one and the user's own `git diff
  --no-index`'s lines, the program did not run, and the logged read carries each
  default it pins) and `each_read_is_plumbing_that_names_exactly_its_path_and_runs_no_program`
  (every `-c` on the argument vector). Mutations: `--no-color` dropped — the
  integration test RED; the neutralising `-c`s dropped — both tests RED; restored,
  GREEN. The working-tree suite passes on 2.30.9, 2.32.7 and 2.56.0.
- `docs/systems/diff.md`, `docs/systems/git-processes.md`, root `CLAUDE.md` and
  `state.md` say the same; `scripts/git-floor.sh`'s `diff_engine` floor raised to 77.

## 2026-10-03 — phase 03: one path's working-tree diffs, through git

Packet mode, on `feature/diff-engine`. R3.1-R3.5 and C7 built; C15's amendment
written in `docs/design/engine.md` and the root `CLAUDE.md`. QA is due.

### What shipped

- `cairn_git::WorkingTreeDiff` (`Staged`, `Unstaged`, `Untracked`) and
  `Repository::working_tree_diff` / `DiffSession::working_tree_diff`, answering
  `Option<FileDiff>` — `None` exactly where the user's `git diff --cached`, `git diff`
  or `git diff --no-index /dev/null` prints nothing for the path
  (`crates/cairn-git/src/diff/working_tree.rs`).
- The read `reads::working_tree_patch` (`crates/cairn-git/src/reads/working_tree.rs`):
  `git -c diff.suppressBlankEmpty=false {diff-index --cached --no-renames | diff-files
  --no-renames | diff --no-index} -z --raw --no-abbrev -p --full-index -U<n>
  --no-ext-diff --no-textconv --no-color [-w] [--diff-algorithm] [--ignore-submodules]`,
  pathspec `:(literal)<path> :(exclude,glob)<escaped path>/**` (or `-- /dev/null
  <path>`), no `-a`.
- `PatchText::new_side` (the working-tree side rebuilt from the old side and git's
  patch), `new_index_id`, `submodule_targets`, `has_hunks`; `Parser::finish_listing`.
- `Invocation::finish_within` in `process/runner.rs`: a bounded stream whose output
  before a failed exit is already the caller's (for `--no-index`, which exits 1 with
  an answer). Two runner tests.
- `DiffContent::Submodule` gains `dirty` (`cairn-model`, with a test).
- `diff::submodules::working_tree_ignore`: porcelain's `diff.ignoreSubmodules`, which
  plumbing does not read, passed where a submodule has no `ignore` of its own.
- 19 integration tests in `crates/cairn-git/tests/diff/working_tree.rs`, 8 unit
  tests; `scripts/git-floor.sh`'s floors raised to 59 and 76 and the sparse-index
  skip on 2.30.9 named in its header.

### How Cairn holds git's form of the working tree — decided here

The phase doc said gix's filter pipeline; the user's later rules (driver run by git,
parity by construction) win. Cairn never reads a working-tree file for the diff:
git reads and converts it (`diff-files`, `diff --no-index`), and the side's lines
are rebuilt from git's patch over the old side — every unprinted line is the old
side's, every printed one git's. So the lines held ARE git's form, and the driver
runs once per git read, under git, with the read's environment. The rebuilt side is
then hashed (`gix::objs::compute_hash`, written nowhere) and must equal the id git
printed on the `index` line, which git computes from a second read of the file:
that is the stale-read guard for a file that changes while git reads it. No gix
filter pipeline, so no driver started by Cairn's own process and no inherited
environment.

### Evidence, by experiment (git 2.30.9, 2.32.7, 2.56.0 unless noted)

- No write: snapshots of every file under `.git` (`.git/modules` included) before
  and after `diff-files -p --full-index`, `diff-index --cached -p`, `diff
  --no-index`, on a repository with a clean filter, CRLF files, a stat-dirty file,
  submodules and their `git status`: identical on all three gits. `--full-index`
  hashes the working tree for its `index` line without writing the object; the
  empty tree on an unborn branch is git's own, nothing written.
- The filter: `diff-files` and `diff --no-index` run it (twice: the diff, then the
  hash); `diff-index --cached` does not. Its environment, recorded through Cairn:
  the read's (`GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`, `GIT_TERMINAL_PROMPT=0`,
  the editors, `GIT_ASKPASS`/`SSH_ASKPASS`, the roster) plus `GIT_DIR`,
  `GIT_WORK_TREE`, `GIT_EXEC_PATH`, `GIT_PREFIX`, `GIT_CONFIG_PARAMETERS` and git's
  exec directory first on `PATH`; no askpass token, nothing of the launching
  process's own.
- `text=auto` + a CRLF checkout: `diff-files` lists the file with a null id and
  prints no patch (stat-dirty shape); porcelain prints nothing.
- A failing clean filter: `required` → `fatal: <path>: clean filter '<x>' failed`,
  status 128, from `diff-files` and `--no-index`; not required, or a missing program →
  git warns on stderr, diffs the unfiltered content, status 0 (porcelain too).
- Untracked: `git diff --no-index /dev/null <file>` applies `text=auto` and the clean
  filter (a CRLF file reads LF, a filtered file reads in the filter's form), so "no
  git call needed" is refuted. It exits 1 with a difference AND with an unreadable
  file (no stdout); `--no-exit-code` changes nothing. It prints `--raw -z` records
  like plumbing.
- Intent-to-add: `diff-files` lists a new file with every line, as `git diff` does;
  `diff-index --cached` lists an EMPTY file added (`e69de29`) where `git diff
  --cached` lists nothing (porcelain's `ita_invisible_in_index`; plumbing refuses
  `--ita-invisible-in-index`, 2.30.9 and 2.56.0).
- Conflicted: `diff-files` prints a `::` combined record; `diff-index --cached` a `U`
  record and `* Unmerged path`; porcelain `diff --cc`.
- Submodules: plumbing ignores `diff.ignoreSubmodules` and `diff.submodule`, honours
  `submodule.<name>.ignore` (config and `.gitmodules`) and `--ignore-submodules=`;
  passing the global value where there is no own setting matched porcelain for a
  moved, a dirty and an untracked-content submodule under unset/none/untracked/
  dirty/all and under an own `none` beside a global `all` (2.30.9, 2.56.0). Dirty is
  git's `Subproject commit <id>-dirty`.
- `:(exclude,literal)<path>/` excludes a gitlink at `<path>` itself (git matches it as
  a directory); `:(exclude,glob)<path>/**` with `\ * ? [` escaped keeps it and still
  excludes a directory's contents (a path `a*b[c]?d\e` beside `aXb[c]?d\e`), all
  three gits.
- Sparse index: 2.32.7 and 2.56.0 write one (`sparse-checkout init --cone
  --sparse-index`, the `sdir` extension); 2.30.9 cannot.

### Mutations, each RED then restored GREEN

| Mutation | Test that went red |
| --- | --- |
| `--diff-algorithm` not passed | `every_discriminating_file_reads_as_git_diff_shows_it_staged_and_unstaged` |
| working-tree side read from disk (hash check also off) | CRLF, clean-filter, untracked, size and no-write tests |
| hash check off | `content_that_changes_between_gits_reads_is_the_error_a_caller_retries` |
| intent-to-add rule dropped | `an_intent_to_add_path_is_new_unstaged_and_nothing_staged` |
| `--ignore-submodules` not passed | `a_submodule_answers_its_commits_and_whether_it_is_dirty_as_git_diff_shows_it` |
| exclusion `:(exclude,literal)<path>/` | the submodule test (the finding above) |
| no exclusion | `a_staged_file_on_an_unborn_branch_and_beside_a_directory_is_that_file` |
| porcelain `diff` for unstaged | `a_working_tree_query_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor` |
| conflict check dropped | `a_conflicted_path_answers_conflicted` |
| sparse check dropped | `a_sparse_index_is_unsupported_and_says_so` |
| `-a` passed | `an_untracked_file_is_what_git_diff_no_index_shows` |
| no output ceiling | `the_size_ceiling_is_judged_on_gits_form_of_the_working_tree` |
| `-w` reading = exact | `staged_and_unstaged_edits_read_as_git_diff_shows_them` |
| staged asks `diff-files` | `staged_and_unstaged_edits_read_as_git_diff_shows_them` |
| `--no-index` status 1 with no record taken as no change | `a_failing_clean_filter_is_an_error_naming_the_path_or_what_git_shows` |
| a stat-only record (no section) refused | `a_stat_dirty_file_is_no_change` |
| `-dirty` ignored | the submodule test |
| rebuilt side drops its last line | 8 of the working-tree tests |
| `finish_within`: `>=` for `>`; the crossing chunk handed on | `a_bounded_stream_takes_exactly_its_ceiling_and_refuses_a_byte_more` |
| a working-tree modification of a large file answered from its raw record (the first cut of the code, found in review and fixed test-first) | `a_large_file_whose_stat_or_mode_alone_moved_is_what_git_diff_shows` (RED: a touched file answered `TooLarge` where `git diff` shows nothing) |

### Decided here, without the user

- **`git diff --no-index` for an untracked file — NEEDS THE USER'S SIGN-OFF.** It is
  porcelain, outside "query plumbing or `status`", the rule the root `CLAUDE.md`
  states and `destructive-ops-reviewer` check 10 enforces. There is no plumbing that
  prints an untracked file in git's form (adding it to an index is a write), and
  reading it any other way diverges from `git diff --no-index` on CRLF and filtered
  files, or runs the driver outside git (gix's pipeline, with Cairn's inherited
  environment). It reads no index, writes nothing (above) and runs only the clean
  filter. The code, `CLAUDE.md`, `docs/design/processes.md`,
  `docs/systems/git-processes.md` and `reads/mod.rs` now name it as the one porcelain
  mode a read runs; `.claude/agents/destructive-ops-reviewer.md` check 10 was NOT
  changed (enforcement layer) — if the user accepts, it should name the mode; if
  not, the alternative is gix's filter pipeline for untracked files only, with the
  inherited-environment residual back for them.
- Git decides what is binary on the working tree (no `-a`), on git's form; the
  working-tree side's size in a `Binary` answer is the file's size on disk.
- R2.6 is judged on git's form: a blob side by its header before git runs (raw-only
  read when one is past the ceiling — then `TooLarge` without the patch, except for a
  working-tree modification, whose patch is still asked for under the ceiling, since
  only it tells a stat or mode change alone from an edit); git's output bounded at
  twice what both sides within the ceiling could print; past it, `TooLarge` measured
  at the largest side known, the file on disk, or the ceiling's next byte.
- Conflicted, sparse index and bare repository are stand-in answers whose
  `ChangedFile` carries the path and no mode or id.
- Sparse index: unsupported for the whole repository (PRD R3.4), though plumbing would
  answer; a non-required failing filter is shown as git shows it, its stderr warning
  not (prose).
- `Untracked` answers `--no-index` for the named path whatever the index holds; one
  path pairs no rename (`git diff --cached -- <path>` does not either).

### Residuals and follow-ups

- Phase 04: the working-tree query blocks on one to three processes; it is a
  file-diff-lane call with its epoch. `ContentReadsDisagree` is retried — except that
  a clean filter whose output differs run to run never agrees with itself (a known
  limit in `docs/systems/diff.md`), so the retry needs a bound.
- Packet 4/5 (status): renames between two paths of the working tree, and which paths
  are untracked, are the changed-file list's; a two-path variant of the query may be
  wanted for a staged rename.
- A sparse index could be supported cheaply (git's plumbing answers it); gix's
  reading of one is what remains.
- The configuration (`diff.algorithm`, `diff.ignoreSubmodules`) is the handle's, read
  at open; only the index and attributes are fresh per query.

`scripts/gate.sh` PASS, all eight steps, `git-floor` on 2.30.9 (the sparse test
skipped, as its header now says) and 2.32.7.

## 2026-10-03 — S1 hardened: one search, git's count, no process

A fresh security review of S1 (`8574d5a`) found no bypass, and three things to
harden, all fixed in `32f7549`:

- **Two walks, one check (warning).** The check validated the directory its own
  walk found, then gix searched again and opened whatever it found; they agreed
  only because gix-discover 0.55 switches to the physical path as git does.
  `bare_discovery::find` now hands back where its one search stopped (the `.git`
  of a working tree, or a git directory found as itself), and
  `SharedRepository` opens exactly that path with gix's `open_opts`, the path
  taken as it is and the options and trust gix's discovery derives from its
  owner — the steps `discover_opts` takes after its own search. The search keeps
  gix's default of not crossing into another filesystem. Ordinary repositories
  open the same directory; the paths reported are physical (a repository opened
  through a link reports where the link leads, as `git rev-parse` does).
  `a_bare_repository_found_by_searching_opens_exactly_where_git_opens_it` now
  compares the git directory Cairn opens with `git rev-parse
  --absolute-git-dir`, not only whether each opens, over two new shapes: a bare
  repository planted as `docs/` holding `guide -> ../guide`, opened at
  `docs/guide`, and a link from outside the working tree to the same directory.
  Mutations: opening through a second, logical search after the check — RED,
  Cairn opening the planted `docs/` under every setting, `explicit` included,
  where git opens the working tree; the one search made logical — RED; the old
  shape (check, then `gix::ThreadSafeRepository::discover`) — green on gix 0.55,
  which is the agreement the warning was about.
- **`GIT_CONFIG_COUNT` parity (warning).** `str::parse::<usize>` refused what
  git's `strtoul` accepts. `entry_count` reads it as git does, each answer taken
  from git 2.56 by experiment: leading C whitespace and one sign accepted, an
  empty value or `-0` zero entries, whitespace or a sign alone "bogus count",
  anything after the number "bogus count", past `INT_MAX` "too many entries"
  (`-1` included; a negative within `INT_MAX` of 2^64 wraps to a small count, as
  on git), overflow saturating. git 2.30.9 ignores the variable (it arrived in
  2.31), and no git before 2.38 is checked. Both of git's errors surface as
  `Error::InvalidConfig` for `GIT_CONFIG_COUNT`, told apart by the internal
  `CountRefused`. Pinned by `the_entry_count_is_read_as_gits_strtoul_reads_it`
  and four accepted spellings in the shapes test against git itself (RED with
  `str::parse`).
- **A process outside `GitEnvironment` (note, confirmed).** `Source::GitInstallation`
  made gix-path run `git config -lz --show-origin --name-only` from `PATH` with
  the process's environment, once per process, wherever the setting was read
  without `GIT_CONFIG_NOSYSTEM`; gix's own open never asks for it
  (`git_binary: false`). A recording `git` on `PATH` saw it on the previous code
  and not on this. Dropped: the system file is `Source::System`
  (`GIT_CONFIG_SYSTEM` or `/etc/gitconfig`, the file this machine's git 2.56
  names) and runs nothing. Pinned by
  `opening_reads_the_system_file_without_running_a_process` (RED with
  `GitInstallation` restored). Residual, stated in `docs/systems/git-processes.md`:
  a git built with another `sysconfdir` has its system file unread, which only a
  process could find — the residual the previous commit already stated, less
  its "installation file of the `git` on `PATH`".

## 2026-10-03 — phase 02 QA, round 3: the content rework's findings fixed

The content-parity rework's QA: 21 raw findings, 16 confirmed by a fresh
`qa-confirm`, S3 dismissed. Three were escalated and the user decided them, with a
fourth decision on S2:

1. **S1, security — refuse at open, git's exact rule.** gix never reads
   `safe.bareRepository`, so a bare repository planted in a cloned working tree
   (`core.worktree=..`, `core.bare=false`, a `core.fsmonitor`) opened, and since D1
   names every repository to git with `--git-dir` — the spelling git never checks —
   `diff-tree` and `check-attr` ran the planted program where the user's own git
   refuses the repository; fetch was reachable too (`core.sshCommand`,
   `credential.helper`).
2. **S2 and Q5 — allow `core.fsmonitor` on a read, and document it.** `diff-tree`
   (raw and patch) and `check-attr` run the repository's hook as they read the
   index, on every git from 2.30.9 to 2.56.0, as the user's own `git diff` does.
3. **R2 — Expand All's memory is phase 08's.** Recorded in `state.md` (open
   questions): every changed blob and the whole parsed patch are held unbounded,
   and phase 08 bounds them with the line budget.
4. **G4 — file an issue**: a test that newly starts skipping on a floor git is
   reported but does not fail, and `VERSIONS`/`RUNS` in `scripts/git-floor.sh` are
   unpinned. Issue #51, with a per-version expected-skip roster proposed.

### What changed, commit by commit

- **S1** (`8574d5a`). `crates/cairn-git/src/bare_discovery.rs`: before gix opens
  anything, git's discovery walk is replayed from the physical path; a stop at a
  directory that is itself a git directory (not one holding a `.git`) is a bare
  repository found by searching, and it is refused with
  `Error::BareRepositoryFoundBySearching` where that git refuses it. The rule was
  read from `setup.c` at v2.37.0 (no setting), v2.38.0, v2.39.0 to v2.43.5, v2.44.0,
  v2.44.1, v2.45.0, v2.50.0 to v2.56.0 and `Documentation/BreakingChanges.adoc`, and
  reproduced with 2.30.9, 2.32.7, 2.39.5, 2.40.0 and 2.56.0: no refusal before
  2.38; every such repository refused under `explicit` from 2.38 to 2.43; a directory
  named `.git` allowed in 2.44; a path holding `/.git/worktrees/` or `/.git/modules/`
  allowed too from 2.45; `explicit` the default from git 3.0. The setting is read
  from the protected configuration only — system, XDG and global files (or
  `GIT_CONFIG_GLOBAL`), includes followed but `includeIf "gitdir:"` not, then
  `GIT_CONFIG_COUNT` and `GIT_CONFIG_PARAMETERS` (old and new style, parsed as
  `parse_config_env_list` does) — never the repository's own; every value is
  checked whenever the walk stops at a bare repository (git dies on any value but
  `explicit` or `all`, the bare key included, even for an implicit one), the last
  winning. `SharedRepository::discover_for(path, &GitBinary, environment)` is the
  application's open, given the git found at startup and the launching
  environment (`Startup::parent`); `discover` applies git 2.45's rule with the
  process environment. Repositories that pass are named with `--git-dir` as before;
  the separate-git-dir test stays green. Pins:
  `a_bare_repository_found_by_searching_opens_exactly_where_git_opens_it` (seven
  shapes under eight settings, the repositories' own configuration saying
  `explicit` throughout, Cairn opening exactly where `git rev-parse` does),
  `a_planted_bare_repository_is_refused_at_open_and_runs_nothing` (the planted
  hook runs on a read without the setting, as under git, and never under it;
  its refusal half skips before 2.38),
  `a_planted_bare_repository_is_refused_as_the_launchs_git_refuses_it` in the
  application, and five unit tests in `bare_discovery.rs`. Mutations, each RED:
  the check disabled; the command line ignored; the worktree exemption dropped;
  includes not followed; the application opening through `discover`.
- **S2 and Q5** (`530e2c8`). The "runs no program" claims in
  `reads/attributes.rs`, the `patches.rs` header, `reads/mod.rs` ("What a read may
  run", which now names the hook as the one program a read may run),
  `docs/design/engine.md`, `docs/systems/diff.md`, `docs/systems/git-processes.md`
  and the packet's QA checklist corrected.
  `the_content_query_writes_nothing_and_runs_nothing` configures `core.fsmonitor`
  as the accepted program — the git directory still byte-identical, every other
  program's mark absent, the hook's present — and requires `check-attr` in the
  command log on 2.40 and later (and absent before).
- **G1** (`a723bf6`). `gate_dispatch_arms` reads every line between
  `case "$SELECTED_STEP" in` and `esac` and is an `Err` for anything but a plain
  `name) run_x ;;` arm or the exact default arm; `ci_runs_every_merge_bar_gate_step`
  now reads the arms through it too. The reviewer's escape — git-floor's arm
  rewritten (`"$@"`, a trailing comment, an alternation) and dropped from the full
  run — passed both gate guards before and fails both now.
- **C6, C2, C1** (`d81de71`).
  `a_path_git_quotes_reads_as_git_diff_shows_it_alone_and_through_expand_all`
  (a space, a quote, a tab, a newline, a backslash, `é`, `[ab]`, a leading `-`, and
  a rename between two such paths; the changes query against
  `git diff --name-status -z`); RED when patches are keyed by the `diff --git`
  line's paths, which the older Expand All test passed.
  `a_renamed_files_driver_algorithm_is_its_old_paths` (in and out of `drv/` under
  `diff.drv.algorithm=minimal`, git's old-path rule shown on git itself); RED with
  the driver looked up by the new path, per file or in Expand All.
  `under_ignored_whitespace_a_context_line_not_the_new_sides_refuses_the_reading`;
  RED with the context line's new-side check deleted.
- **D1-D4** (`0c55d63`). The reads module names patch text among what it parses and
  why the `\ ` marker is safe (git prints it untranslated, and it is taken only
  right after a hunk's line); the patch command shows `--full-index`; the engine
  design cites the spike instead of "nine files"; the diff system doc cites test
  names and the spike instead of counts and timings.
- **C3, C5, C4** (`5bb2204`). `expand_all_runs_one_diff_tree_per_comparison` counts
  the `diff-tree` runs in the command log (RED, 72 against 1, with every file sent
  alone — the `if false &&` fallback the equal-answers test passed); that test's
  doc no longer claims what it cannot see. The discriminating fixture gained
  `drv/large.pl`, drawn until git shows myers and minimal disagree on it, and the
  configuration `diff.drv.algorithm=minimal` is shown to change the driver's files
  (RED when a minimal driver is ignored). The skip test holds a rename that kept its
  blob and a type change, each first shown to be git's own answer.
- **G3, G2** (`53f7fe6`).
  `the_full_gate_is_the_default_and_no_merge_bar_step_is_skipped`: `FAST` is set
  exactly to 0 at the top, before the arguments are read, and to 1 for `--fast`, and
  no merge-bar step's `*_CMD` is `skip` (RED for `FAST=1`, a skipped `git-floor`, a
  skipped `deps`); readers `gate_command_assignments` and `gate_function_commands`
  with their self-test. `job_env_entries` finds the job only under the top-level
  `jobs:`, reads entries at the env block's own indent, and skips block-scalar
  bodies; six new self-test shapes.
- **R1** (`2ca214b`). Expand All asks the files whose drivers name an algorithm in
  one `diff-tree -p` per distinct algorithm over their paths
  (`reads::Scope::Paths`, the algorithm passed), not one process per file; the
  answers are unchanged (every Expand All parity test green on 2.30.9, 2.32.7,
  2.39.5, 2.40.0, 2.56.0), and the run count is pinned (two, four with `-w`, on the
  driver fixture, where it was nineteen).

### Decided here, without the user

- The S1 rule follows the version of the git Cairn found, band by band, since the
  user's git refuses differently by version; one fixed rule would have diverged on
  2.38-2.44 or below 2.38.
- The protected configuration is read with gix-config from the files and the
  environment, not filtered from the repository's snapshot: the snapshot evaluates
  `includeIf "gitdir:"` against the repository, which git's protected read does not,
  and gix reads no `GIT_CONFIG_PARAMETERS`, so that is parsed here.
- The environment the check reads is the launching one (what the user's own git,
  run from the same place, reads), not the inherited roster Cairn's git runs with.
- Residuals, stated in `docs/systems/git-processes.md`: the system file is gix's
  guess, not the path compiled into the found git; an `includeIf "hasconfig:"` in a
  global file is not followed; other keys of `GIT_CONFIG_PARAMETERS` are not
  validated; a pre-3.0 git built `WITH_BREAKING_CHANGES` reads as defaulting to
  `all`. Each touches only the user's own protected configuration, never one a
  repository can plant.
- The no-write test asserts the fsmonitor hook DID run, so the doc's claim is
  pinned, not just tolerated.
- `skip` is refused for every step but the day loop's `test-fast`, and the gate's
  header comment now says so.

`scripts/gate.sh` PASS, all eight steps, `git-floor` on 2.30.9 and 2.32.7 skipping
only the three tests its header names (`git-floor.sh`).

## 2026-10-03 — content parity: a file's changed lines come from git

The user decided after the spike (`docs/research/diff-engine/content-parity-spike.md`,
"Decided from it"): the content query's changed ranges, its whitespace-ignoring
ranges and each hunk's function context come from `git diff-tree -p` run as a read;
gix keeps the blob reads, the size, binary and LFS pre-checks, intra-line highlights
and the patch emitter. **This amends packet decision L3** ("gix computes the diff;
Cairn groups it"), which stays as written in `brainstorm.md`, a historical record;
PRD R2.4, R2.8, R2.9, R6.4 and C6 are amended inline, each marked "(amended 2026-10,
content parity, see progress.md)". C6's "crafted fixtures with unambiguous edits" is
now parity on any edit, under every algorithm and over real history. F1 of the C6
audit is closed by it.

### What shipped

- `cairn_model::FunctionContext`, carried by `DisplayOverlay`
  (`with_function_context`, `function_context`): git's text after each hunk
  header's `@@`, keyed by the line the hunk starts at on the old side, with the
  context it was read at. Display-only, so the emitter's patches are unchanged.
- `crate::reads::patches` (`crates/cairn-git/src/reads/patches.rs`): `git
  --literal-pathspecs -c diff.suppressBlankEmpty=false diff-tree -r -z --raw
  --no-abbrev -p --full-index -U<n> --no-ext-diff --no-textconv --no-color -a [-w]
  [--diff-algorithm=X] <detection> --end-of-options <old> <new> -- <paths>`, parsed
  as maximal runs, every printed line checked against gix's lines. One file, or a
  whole comparison (Expand All).
- `crate::reads::diff_attributes` (`reads/attributes.rs`): `git check-attr --stdin
  -z diff`, run only when the git in use reads driver algorithms (2.40+) and the
  configuration names one git parses.
- `crate::diff::algorithm`: `diff.algorithm` read as porcelain does
  (`Error::InvalidConfig` for a value git refuses), the drivers' algorithms from 2.40.
- API: `ContentOptions.context` (the view's context, default three lines);
  `DiffSession::file_diff` and `Repository::file_diff` now take `&GitBinary`, the
  `&ChangesRequest` the file came from and `&impl Cancel`; new
  `DiffSession::file_diffs(&GitBinary, &ChangesRequest, &ChangeSet, ..)` for Expand
  All; `Error::ContentCancelled` and `Error::ContentReadsDisagree { path, detail }`
  (the stale-read guard, which a caller retries).
- Deleted: `crates/cairn-git/src/diff/whitespace.rs` (Cairn's own whitespace pass)
  and the gix line diff of the content query (`exact_changes`, `changed_ranges`).
  `GitCommand::input` lost its test-only `expect(dead_code)`: `check-attr` feeds it.

### Parity evidence (git 2.56.0; the same suites on 2.30.9, 2.32.7, 2.39.5, 2.40.0)

| Suite | Compared | Divergences |
| --- | --- | --- |
| Discriminating fixture, 9 configurations (unset; myers; minimal; patience; histogram; heuristic off; histogram + heuristic off; histogram with `diff.drv.algorithm=patience`; `diff.drv.algorithm=minimal`) at `-U3` | 720 files, 3,673 hunks, 3,052 with function context | 0 |
| This repository's history, last 50 non-merge commits (all there are), each of myers, minimal, patience, histogram | 596 files and 1,157-1,158 hunks per algorithm | 0 |
| Function context at `-U1`, `-U3`, `-U5`, `-U8`, default rule and a capturing `xfuncname` | 80 files per context | 0 |
| `git diff -w`, seeded whitespace-only, mixed and real edits, and the discriminating fixture | 34 + 80 files | 0 |
| Crafted history (C6) at `-U3` and `-U1` | every text file of every commit | 0 |
| Files git is not asked about (added, deleted, emptied, filled; under a driver and not) at `-U1`, `-U3` | 16, no process started | 0 |
| Expand All against per-file, with and without `-w`, seven fixtures | at least 300 files | 0 |

Each comparison is against porcelain `git diff` for the file's paths with the
detection that pairs them, so it reads `diff.algorithm`, the drivers and the indent
heuristic as the user does. The discriminating fixture is first shown to
discriminate: each algorithm's answer differs from myers', patience's from
histogram's, the heuristic's from its absence, and — on a git that reads one — a
driver's algorithm from `diff.algorithm`.

**Mutations, each run and reverted:** dropping the `--diff-algorithm` flag turns
the discriminating suite (minimal: 2 of 80 files), the history replay (minimal: 8
of 596) and Expand All red. Asking at `-U0` turns the discriminating, history,
function-context and `-w` suites red (72 of 80, 291 of 596, 72 of 80, 17 of 34
files). The parser's unit tests are the stale-read guard's: a removed, context or
added line that differs, a lost newline and a side that ends early each refuse the
reading, and `lines_git_printed_that_were_not_read_are_the_error_a_caller_retries`
maps that to `Error::ContentReadsDisagree`.

### C14, re-measured

Same machine and repository (AMD Ryzen 7 9800X3D, 60.4 GiB, NVMe, rust-lang/rust at
`c999cef531e`); `git version 2.56.0`; `cargo test --release`, warm, one run to warm
up then the median of seven; the reporter in `crates/cairn-git/tests/diff/bench.rs`.

| Subject | Cairn now (git ranges) | min / max | Cairn before (gix ranges) | git baseline | Bar | |
| --- | --- | --- | --- | --- | --- | --- |
| F7 `3b09522c34b`, loaded | **17.513 ms** | 16.801 / 22.088 | 9.024 ms | 9.8 ms | 100 ms | MET |
| F7, refused on the byte ceiling | 0.027 ms | 0.025 / 0.032 | 0.004 ms | — | — | — |
| F1 `6a6e8446b97`, refused | 0.007 ms | 0.004 / 0.029 | 0.002 ms | — | — | — |
| F1, Load Diff | **186.859 ms** | 185.959 / 195.281 | 128.4 ms | 163.0 ms | — | recorded |
| Expand All, S7 `f0845adb0c1` (1,017 text files, one `diff-tree -p`) | **61.090 ms** | 60.823 / 62.653 | — | — | — | recorded |

The changes query is unchanged (S7 7.808 ms, S1 33.611 ms, M1 81.494 ms; M1's 2,774
pairs all git's). F7's bar is met with five times its margin; the content query now
costs git's own diff plus the gix read Cairn keeps. Expand All's 61 ms covers gix
reading and splitting all 1,017 files' blobs, git's one patch over the commit
(the spike measured 31.7 ms for the process), parsing and checking it, and the
intra-line highlights.

### Decisions made here without asking

- **git is asked at the view's context, not at `-U1`.** The function context of a
  hunk depends on where it starts, so on the context, and cannot be derived from a
  `-U1` answer (a function line between `-U1`'s start and `-U3`'s changes it). One
  read at `-U<n>` (n ≥ 1, the entire file asking at one) answers the ranges — the
  same script at every context of one or more, which the spike measured and the
  replay re-checks — and the headers for exactly the hunks the view draws. A context
  change is a new content query (phase 06). `-U0` stays refused.
- **Driver detection asks git** (`check-attr`), rather than gix's attribute stack,
  so which paths a driver covers is git's answer by construction; it runs only in
  the rare case it can matter. For a path whose driver names an algorithm no flag
  is passed, as decided; for the rest `--diff-algorithm` is always the long form.
  Behaviour reproduced on 2.30.9, 2.32.7, 2.39.5 (driver algorithm ignored, porcelain
  and plumbing alike) and 2.40.0, 2.56.0 (honoured, by the OLD path's attribute).
- **Each file is answered as `git diff -- <path>` answers it**, not as a whole
  `git show` does: from 2.40 git applies a driver's algorithm by changing its own
  options, so in one multi-file output every later file inherits it (reproduced on
  2.40.0 and 2.56.0, porcelain and plumbing). Expand All passes the algorithm
  explicitly to its one call, which switches drivers off for it, and asks about each
  driver-algorithm file alone. Recorded as a known limit in `docs/systems/diff.md`.
- **git is not asked where it has one answer**: added, deleted and type-changed
  files, a side with no lines, and two sides with the same lines (a rename that kept
  its blob). A type change is one change of every line, since git's own patch for it
  is a deletion and an addition.
- **`-w` on older gits**: 2.30 through 2.40 list a whitespace-only file and print no
  patch for it, where 2.56 leaves it out of the raw records too. The read adds
  `--full-index` and matches such a file to a patch only by the blobs on its `index`
  line; absent or unprinted under `-w` is "no changes".
- **`--literal-pathspecs` is a global option**, before the verb (`diff-tree` refuses
  it after); verified with a path holding `*` beside one the glob would match, on
  every git above. The whole-comparison read does not pass it: its only pathspecs
  are the `:(exclude,literal)` magic of the changes query's submodule rule.
- `scripts/git-floor.sh`'s test-count floors raised to 49 and 52, one under the
  runs' new counts.

### Residuals and follow-ups

- **Phase 03** (working tree): the spike's mechanism carries over —
  `diff-files -p -U<n>` and `diff-index -p -U<n> [--cached] HEAD`, with the held
  lines git's clean-filtered, EOL-converted form; `diff-files` runs the clean filter
  (spike section 3), which L6 already allows. `reads::patches` takes trees today; a
  working-tree scope is its next arm.
- **Phase 04**: every content query now starts a process and blocks on it, so it is
  a diff-thread call with its epoch; `ContentOptions.context` is part of the cache
  key R4.5 allows; `Error::ContentReadsDisagree` is retried, `ContentCancelled` is
  not an error to show.
- **Phase 06**: a context change asks the engine again, since function context
  belongs to the context it was read at (`FunctionContext::of` answers `None` for a
  hunk git printed no header at). Under `-w`, git prints a context line from the NEW
  side, which may differ from the old in its whitespace; `UnifiedRows` draws context
  from the old side and has no rows over the whitespace-ignoring ranges yet — the
  parity test builds them over the new side. `diff.context` and
  `diff.interHunkContext` are not read (a known limit).
- **Phase 08**: `DiffSession::file_diffs` is Expand All's call; its one `diff-tree`
  diffs every file git would show, including ones Cairn refuses as too large, so a
  commit holding a very large text file pays git's time for it (cancellable). The
  line budget may want to pass a subset — today it takes the whole change set.
- CI's `git floor` job runs 2.30.9 and 2.32.7, so the before-2.40 branch of the
  driver rule is decided there and the 2.40+ branch on the gate's own git; 2.39.5 and
  2.40.0 were run by hand here, not by the gate.
- QA is due: `/qa` has not reviewed this change.

## 2026-10-03 — the C6 audit, the gate's integrity, and a shallow clone's history

The C6 audit and a gate-integrity review raised findings that a fresh `qa-confirm`
adjudicated. Fixed from commit `2e15d1c` up to this entry; `scripts/gate.sh` passes,
`git-floor` included.

### The C6 audit, F1-F7

- **F1 — escalated, not fixed: gix's hunks diverge from `git diff`'s.** The content
  query computes its ranges with gix (`diff_with_slider_heuristics` in
  `crates/cairn-git/src/diff/content.rs`), and the adjudicator found answers that
  differ from `git diff -U3`'s, which the user's standing rule makes a critical
  parity bug. A spike is measuring what moving the ranges to git would cost, and the
  user decides after it. Until then the algorithm is untouched and no test
  discriminates `diff.algorithm` or the indent heuristic (R2.4): it would fail today.
- **F2 — the load-anyway ceiling.** `past_the_load_anyway_ceiling_nothing_is_offered_or_read`,
  over the truncated object with `load_anyway_bytes` either side of its size: not
  offered without `load_anyway`, refused from the header with it, offered at exactly
  the size. Mutations `loadable: true` and a `u64::MAX` load-anyway ceiling each turn
  it red.
- **F3 — both limits at exactly their value.** A side of exactly `max_lines` lines is
  inside the limit (the unit test of `crossed_line_limit`), and a file of exactly
  `max_bytes` is drawn while one byte less of ceiling refuses it
  (`a_file_exactly_at_the_byte_ceiling_is_drawn`). Each `>` turned `>=` turns its test red.
- **F4, F5 — git's binary rules.** The attributes fixture gained the `binary` macro
  (`macro.dat`), a NUL at byte 7,999 (binary) and one at byte 8,000 in a file of short
  lines (not binary), each compared with git as before; and
  `a_file_past_big_file_threshold_is_binary_as_git_says` sets
  `core.bigFileThreshold=1k` beside a text file a few KiB long. All agreed with git.
- **F6 — a binary's sizes** are compared with `git cat-file -s` for every binary file
  of both tests; `old_size: 0, new_size: 0` turns both red.
- **F7 — a pure deletion inside a file.** `crafted()` gained `delete a line`, one
  middle line of `long.txt` removed, before the two commits tests reach as `HEAD~1`
  and `HEAD`; it runs through C1, C2 (which now requires a removal-only file among
  its shapes), C3 and C6. gix's answer for it agreed with `git diff -U3`.

### Gate integrity, G1-G6

- **G1.** The two `CAIRN_REQUIRE_*` guards matched the variable on any line of the
  workflow; they now read the `gate` job's own `env:` (`job_env_entries` in
  `crates/cairn-guards/src/lib.rs`), and
  `the_workflow_env_matcher_reads_only_the_jobs_own_block` shows a workflow-level,
  another job's, a step's, and a commented-out setting each failing. Moving both
  variables into a step and the other job turned both guards red.
- **G2, G3.** `scripts/git-floor.sh` lists each filtered run first and fails under a
  floor — decided here: one under the counts when set (29 for `--lib diff:: reads::`,
  30 tests; 44 for `--test diff_engine`, 45) — and runs with `--show-output`, printing
  each run's `SKIPPED` lines and restating them all at the end. Its header names the
  two skips the floor's gits take: the partial-clone rename search below 2.44 and the
  `safe.bareRepository` case below 2.38.
- **G4 — the user decided the floor runs in the local full gate.** `scripts/gate.sh`
  with no arguments now runs `git-floor` after `test-doc`, as it runs `deps`; `--fast`
  skips both. The first run fetches and builds; the builds are cached after. Where a
  build is needed the script first checks for a C compiler (`$CC`, now also passed to
  `make`), `make` and zlib's headers, and fails naming what is missing with the
  packages to install — a fresh Debian or Ubuntu needs `build-essential zlib1g-dev`,
  which is what the CI job installs — and never skips. Twin:
  `the_local_full_gate_runs_every_step_but_the_day_loops`, with the exemption roster
  `LOCAL_FULL_GATE_EXEMPT` (`test-fast`) and the self-test
  `the_gate_sequence_matcher_catches_the_shapes_it_claims`; it failed on the gate
  before `run_git_floor` was added. Root `CLAUDE.md` and `docs/qa-gate.md` say so.
  Run here from an empty cache, the full gate fetched and built both gits and their
  diff tests passed, each run listing its two expected skips.
- **G5.** git before 2.32 ignores `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_SYSTEM`, so the
  fixtures ran under the machine's `~/.gitconfig` on the floor's gits. `Repo::run`,
  the scratch index's runs (`tests/diff/scratch.rs`, decided here: same hazard,
  same tests) and `tests/fixtures/mod.rs` now set `HOME` and `XDG_CONFIG_HOME` to a
  checked-empty directory and `GIT_CONFIG_NOSYSTEM=1`. Shown on 2.30.9: a home whose
  `.gitconfig` signs commits with a failing program broke eight diff tests without
  the change and none with it.
- **G6 — dismissed:** the oracle cannot reach `log.diffMerges`.

### The history graph's shallow boundary — the user decided to fix it here

The history query handed a shallow clone's boundary commit over with the parents its
object names, where `git log --format=%P` prints none. The test written for it, against
`git log --format='%H %P'` at depths 1 to 4 of one merge-shaped history, found a second
divergence: at depth 4, where a boundary commit's parent is in the clone because a
sibling branch reaches it, gix's walk left that commit out entirely — gix 0.87.1's
`rev_walk` skips the next appearance of each cut-off parent id whichever commit names
it. Decided here: both routes now walk `gix::traverse::commit::Simple` directly over an
object source that serves a boundary commit with its `parent` lines removed, git's
graft applied where the object is read (`crates/cairn-git/src/history/walk.rs`); no
commit-graph is used in a shallow repository, as git's `commit_graph_compatible`
refuses one there; an invalid `core.commitGraph` is still an error and a graph that
will not open is walked without, as gix's walk decided both. The shallow file is read
by `crates/cairn-git/src/shallow.rs`, which `commit_details` now shares. A boundary
commit is a root to the lane assigner, so no lane waits below it; the test requires
the graph to be the one git's parents lay out, and the oldest row to carry no line
past it. Turning the graft off turns it red. As built: `docs/systems/history-graph.md`,
"A shallow clone is walked as git walks it".

## 2026-10-03 — phase 02's QA: what it found, what was decided, what was fixed

QA ran over `34bc907..HEAD`, the reworked phase. Its reviewers raised nineteen raw
findings plus a coverage gap around C6; a fresh `qa-confirm` adjudicated them, most
by experiment against git 2.56, and confirmed sixteen. Dismissed: **D3**, a ceiling on
the changes query's answer — a cap would hide files git lists, and the cancel already
bounds the time; **Q2**, a test-only option to run the floor's branches — it fails
loudly rather than proving anything, and is superseded by the CI job below; and
**Q3**, that gix opens the configuration with `includes: true` under full trust,
which the adjudicator found to be no defect. The C6 audit is still to do.

Two findings needed the user, who decided on 2026-10-03: **A3, honour
`log.showRoot`**, and **Q1, prove the 2.30 floor in CI**.

What was fixed, each with its test in the same commit (commits `430ab12` to the
docs commit after `cd0c065`):

- **D1 — the repository is named to git.** `in_repository` only set the directory,
  so git's discovery read the enclosing repository for a working tree inside
  another, and refused a bare repository under `safe.bareRepository=explicit`. Each
  invocation now passes `--git-dir` and `--work-tree`. Decided here, without the
  user: an explicitly named git directory skips git's `safe.directory` check
  (reproduced with `GIT_TEST_ASSUME_DIFFERENT_OWNER=1`), so the options are given
  only for a repository gix trusts fully, which is gix's reading of the same rule; a
  less trusted one is left to discovery and to git's check. The command log and
  errors record the verb's arguments, not the location — a log is the repository's
  own. Fetch now lands in the repository Cairn opened, which was the bug's other
  half.
- **A1 — a shallow clone's boundary commit lists no parents**, as `git log
  --format=%P` shows. **The history graph has the same divergence** — gix's walk
  hands `info.parent_ids` with the parent the clone lacks
  (`crates/cairn-git/src/history.rs`, `history/session.rs`), checked on a depth-1
  clone of this checkout — and is not fixed here: the graph is history-graph's,
  the fix spans three walk sites and the lane assigner's handling of a parent that
  never arrives, and it is reported for its own change.
- **A3 — `log.showRoot`.** False means no diff for a root commit (or a shallow
  boundary), as `git log` and `git show` print none. Decided here: it is read for
  every one-commit query, since `git log` refuses an invalid value whether or not
  the commit is a root, and never for a comparison, which is `git diff`'s.
- **A2 — `diff.ignoreSubmodules`.** Decided here, against the suggested rule: a
  list filtered after git answers still diverges — git hides the gitlinks before
  rename detection, so `git log` never counts them against `diff.renameLimit`, and
  an added gitlink can push a search `git log` runs past the limit (reproduced on
  2.30 and 2.56: `git log` shows `R094`, a filtered `diff-tree` a deletion and an
  addition). So git is asked not to queue them: `--ignore-submodules=all` when no
  submodule has a setting of its own, which is exact; otherwise, because that flag
  overrides a submodule's `none`, the first answer's hidden gitlinks are excluded by
  `:(exclude,literal)` pathspecs in a second run when detection is on, and dropped
  when it is off. The semantics — the working tree's `.gitmodules`, else the
  index's, else `HEAD`'s, never the shown commit's; none in a bare repository; the
  name the last to claim a path; `ignore` values git does not know skipped — were
  read from git's source at v2.30.0 and v2.56.0 and reproduced on both. Two
  residuals are stated in `docs/systems/diff.md`.
- **T1–T4.** C1 and C3 floor on files staged through a patch (a content query
  answering everything binary now fails them, and passed before); C2 requires a
  mode change beside a hunk, two hunks and CRLF context, from three new crafted
  commits (dropping the mode lines when hunks exist, and stripping `\r` from
  context, now fail it, and passed before); the cut-short test meets the square of
  the limit (`>` turned `>=` now fails it, and passed before); and the
  write-nothing test claims only what it decides, with the command pinned on the
  argument vector.
- **Q1 — the floor in CI.** A `git-floor` gate step (`scripts/git-floor.sh`)
  builds git 2.30.9 and 2.32.7 by pinned commit and runs the diff tests on each;
  2.32 because it alone reaches the 2.31–2.32 branch. Decided here: a gate step,
  so `ci_runs_every_merge_bar_gate_step` holds CI to running it, but outside the
  local full sequence, since it fetches and builds git; run by CI as its own job,
  with the two `CAIRN_REQUIRE_*` variables moved from the workflow to the gate
  job so the floor job's skips stay skips. The oracles needed spellings git 2.30
  reads (`-m --first-parent`, a relative `--git-common-dir`, the 400 default), and
  the `safe.bareRepository` case skips before 2.38. Verified locally against both
  gits built in the scratchpad from a local clone of git's source.
- **D2, Q4, Q5, T6, A4 — docs.** The partial-clone limit now covers tree-less
  clones and what older git does; the cancellation paragraph lost its history and
  its literal counts; `crafted()`'s doc lists the commits it makes; and `state.md`
  records that "cut short" needs a `cairn-model` counterpart before phase 04 or 05
  carries it across the seam.

## 2026-10-03 — phase 02 reworked: the changes query answered by `git diff-tree`

The packet resumed on the process manager (#50) and phase 02's changes query moved
from gix to git, per decision E. `Repository::changes` now takes the `GitBinary`
the application found at startup: gix reads the commits named and the two rename
keys, and `crate::reads::changes` runs `git diff-tree -r -z --raw --no-abbrev` as a
read invocation, parsing its `-z` records into `ChangedFile`s, which are then sorted
by the same total key as before. The gix tree walk went with it — `repair_copies`,
gix's `RenameDetection` counters, the per-change cancel poll and `Error::TreeDiff`.
The content query, the model and the round trips stand. As built:
`docs/systems/diff.md`, "The changes query". `scripts/gate.sh` passes.

PRD amended, each marked inline: R2.1 (the answer is `git diff-tree`'s), R2.2
(git's defaults, the search the user's own `git log` makes, cut short decided from
the answer), R2.9 (a superseded query ends its process, rename detection included)
and C14's rename clause (git's own pairs, no gap may be filed).

### How the configuration is honoured — decided here

- **`diff-tree` reads `diff.renameLimit` but not `diff.renames`.** Verified against
  git: `git help config` says `diff.renames` affects porcelain only, and git's
  `builtin/diff-tree.c` loads `git_diff_basic_config`, which holds
  `diff.renamelimit` while `diff.renames` is in `git_diff_ui_config`; a fixture
  with `diff.renameLimit=1` and plain `diff-tree -M` printed git's limit warning.
  So detection must be passed, and it is all passed: `-M` or `-C` with
  `-l<limit>`, or `--no-renames`.
- **Read in process, from gix's loaded configuration, parsed by git's rules** —
  the last value across files, the bare key as true, `copies`/`copy`, git's
  boolean words, any integer git accepts (base 0, `k`/`m`/`g`, the `int` range).
  Not by a `git config` process: `crate::reads` admits query plumbing and `status`
  only, and that list is a user-owned rule. Two residuals follow and are stated in
  `docs/systems/diff.md`: the keys are as of the repository's opening, like every
  key gix reads; and gix sees Cairn's own `GIT_CONFIG_*` environment, which the
  `git` process does not inherit — since the flags are passed explicitly, gix's
  view, which is the user's shell's, decides.
- **The default limit and what zero means depend on the git**, read from git's
  source at v2.30.0, v2.31.0, v2.32.0, v2.33.0 and v2.56.0: 400 before 2.33 and
  1,000 from it; a limit of zero or less is 32,767 before 2.33 and none from it.
- A value git refuses is `Error::InvalidConfig`: the user's own `git log` refuses
  to answer on it too.

### "Cut short", without reading stderr — decided here

git's warning is prose in the user's language, and there is no exit status or
flag for it. But git skips its exhaustive stage exactly when the sources it has
left times the destinations it has left exceeds the limit squared
(`too_many_rename_candidates`), and when it skips, those leftovers are the
answer's unpaired paths; when it does not, they are a superset of them. So the
inequality over the answer's own counts is exact, with sources counted as the git
in use counts them: unpaired deletions for renames from 2.31 (which culls what the
exact and basename stages paired), every deletion on 2.30, every deletion and
modified file for copies on every version. `RenameDetection::needed_limit` carries
git's own "set it to at least N". Pinned against the linking git's warning, read in
the C locale by the test only, at limits either side of each boundary.

### Tests

C1, C2, C3 and C6 pass unchanged on the git-produced lists. C5 is kept and widened:
`the_answer_is_what_git_log_shows_under_each_configuration` compares with porcelain
`git log --raw` — what the user sees — over 11 spellings of `diff.renames` against
6 of `diff.renameLimit`, every commit including the root;
`a_rename_limit_that_cuts_detection_short_is_reported_exactly_when_git_warns`;
`a_configuration_git_refuses_is_refused`. New: cancellation of a running
`diff-tree` by epoch (`a_changes_query_superseded_by_a_newer_epoch_stops_git_and_reports_it`,
an exhaustive 4,000 x 4,000 search ended mid-run, the command log saying it was
ended), a query superseded before it starts (no process), the read writing nothing
(`the_changes_query_writes_nothing`: the whole git directory byte-identical, the
working tree stat-dirty, `cachetextconv` configured, the textconv never run), a
partial clone (`in_a_partial_clone_a_rename_search_fails_rather_than_fetching`), and
a shallow clone's boundary commit (below). Each new rule was mutated by hand — the
bare key read as off, the culling rule dropped, the 2.33 default dropped, the
shallow check dropped — and a named test failed each time.

### Found while building, decided without asking

- **A shallow clone's boundary commit** named a parent the clone lacks, so
  `diff-tree` failed (`fatal: bad object`) where `git log` shows it as a root. The
  query now compares it with the empty tree
  (`a_shallow_clones_boundary_commit_is_compared_as_git_log_shows_it`). Its
  `CommitDetails::parents` still lists that parent: left for a follow-up.
- **In a blob-less partial clone a rename search fails** (`GitFailed`, nothing
  fetched), where `git log` would fetch: the read's no-lazy-fetch rule wins over
  parity. With detection off the same clone answers.
- **`diff.ignoreSubmodules`** is porcelain config `diff-tree` does not read, so a
  submodule change it would hide from `git log` is listed. Not handled.

### C14, re-measured

Same machine and repository as phase 02 (AMD Ryzen 7 9800X3D, 60.4 GiB, NVMe,
rust-lang/rust at `c999cef531e`); `git version 2.56.0` (the baseline measured
2.55.0); `cargo test --release` (release profile), warm: one run to warm up, then
the median of seven; `measures_the_diff_queries_against_a_named_repository` with
`CAIRN_BENCH_REPO`, which only reads the repository.

| Subject | Cairn now (git) | min / max | Cairn before (gix) | git baseline | Bar | |
| --- | --- | --- | --- | --- | --- | --- |
| S7 `f0845adb0c1`, 1,017 files | **8.205 ms** | 7.883 / 8.465 | 1.631 ms | 7.9 ms | 100 ms | MET |
| S1 `cf2dff2b1e3`, 27,592 exact renames | **35.816 ms** | 34.166 / 36.725 | 20.658 ms | 28.6 ms | 500 ms | MET |
| M1 `5a3292f163d`, 5,602 paths | **83.476 ms** | 80.153 / 84.458 | 2.845 ms | 78.5 ms | 500 ms | MET |

The cost is git's own plus a few milliseconds of parsing and sorting; S1's 27,592
records are the most of that. **M1's rename pairs: 2,774, equal to git's** — and
the reporter now asserts every pair, source, destination and score, against
`git diff-tree -M` on the same commit, not only the count. Not cut short at the
default limit of 1,000. The content query is unchanged: F7 loaded in 9.024 ms (bar
100 ms, MET), F1 refused in 0.002 ms and loaded anyway in 128.4 ms.

### For phase 03 and 04

- Phase 04's diff thread holds a `GitBinary` copy (the application's
  `Discovery`) and calls `Repository::changes` with its epoch; the engine API is
  synchronous and testable directly, as here.
- The rename keys are read at repository open: phase 04 decides when the worker
  reopens, and that is when a config change is seen.
- `process::registry::tests::a_drop_with_no_reaper_thread_is_logged_once` failed
  once in six runs of the crate's unit tests during this phase (the record showed
  `Signal(15)` where it allows `Signal(9)` or `Unknown`), then passed five times
  running; a timing race in the process manager's own test under load, not in
  this change. Worth an issue.

## 2026-09-30 — packet PAUSED: rename parity, decision E, a process manager first

Phase 02 stopped on its rename rule: on `5a3292f163d` gix pairs 231 renames where
git pairs 2,774. The user classed a wrong rename as a critical bug, so "accept and
file" was ruled out, and asked whether gix is the right backend at all.

Two evidence records settled it, both in `docs/research/diff-engine/`:
`rename-parity-spike.md` (measured on the bench repository) and
`git-process-survey.md` (how Cairn spawns git today). What they showed:

- The cause is two defects in gix's rename tracker — the limit compared unsquared
  against its own documented contract, and no basename stage — unchanged on
  gitoxide `main`.
- Passing gix a squared limit (B) is not viable: it still skips the worst subject,
  and where it completes its pairs differ from git's.
- Falling back to git only when gix reports it was cut short (D) is rare and
  cheap and merges cleanly, but closes only about 64% of disagreeing commits:
  gix's similarity measure and first-match pairing diverge from git's even when
  it runs a full search (0.32% of recent first-parent diffs, a lower bound).
- Cairn has a careful single-invocation runner, not a process manager: no stdout
  that is both captured and cancellable, no stdin, no timeouts, no tracking or
  kill-all, no epoch-driven cancel, and a guard gap that lets `diff/` reach the
  runner without tripping any twin.

**Decided by the user:** gix stays the read backend for history, content diffs
and the model. **Option E**: the changes query — which paths changed, their
statuses, modes, ids and rename and copy pairs — comes from `git diff-tree -M`
always, exact by construction, at git's own cost (about 8–37 ms per selection,
83 ms on the worst subject, inside every C14 bar). This amends D1 (now in
`docs/design/engine.md` on `main`, after #39). And **a process manager for
spawning git is designed and built first, as its own packet**, planned with
`/feature-plan`; phase 03's working-tree reads and packet 5's staging build on it
too.

Consequences for this packet, to be re-planned when it resumes:

- Phase 02's gix changes query (`crates/cairn-git/src/diff/changes.rs`, including
  `repair_copies` and `RenameDetection`) is superseded by a git-backed one; the
  content query, the model and the round-trip tests stand. C5 already compares
  against git, so it carries over; C14's changes-query numbers must be re-measured.
- Phase 02's QA, not yet run, is deferred to the reworked phase.
- The integration branch is behind `main` by #39 (D1's move to
  `docs/design/engine.md`). Bringing it up to date means a rebase and a force
  push of a shared branch, which is the user's call.

## 2026-09-18 — phase 02: the engine answers commit and comparison diffs

`cairn-git` has R2's two queries. `Repository::changes` walks two trees — two
commits, one commit against its first parent, or a root commit against the empty
tree — and answers `ChangeSet`: the changed files sorted by a total key, the
commit's details when one commit was named, and how rename detection went.
`Repository::file_diff` turns one of those files into a `FileDiff`, through gix's
resource cache in `Mode::ToGit`. `DiffSession` holds that cache for a run of
queries; the two `Repository` methods are one-shot sessions over it. As-built
prose: `docs/systems/diff.md`. `scripts/gate.sh` passes.

### C1, C2, C3, C5 and C6

All five pass, in `crates/cairn-git/tests/diff/`. Every apply runs real `git` with
`GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY` and `GIT_ALTERNATE_OBJECT_DIRECTORIES`
pointed at a scratch directory, so the repository under test is only ever read —
including this checkout, which C1 walks.

- **C1** compares TREES, never patch text: `git write-tree` after the apply
  against `<commit>^{tree}`. Over the crafted fixtures, both rewrite fixtures and
  every non-merge commit of the Cairn checkout.
- **C2** seeds twelve selections per file — every line, no lines, only additions,
  only removals, the first line of every change, the last line of every change and
  eight pseudo-random ones — and checks the staged blob against `apply_patch`
  **and** the header effects against the index: the mode staged, whether the
  source path survives, whether anything is staged at all.
- **C3** reverses the same patches onto the commit's tree and requires the
  parent's.
- **C5** compares against `git diff-tree -r --raw --no-abbrev` under the same
  config, field for field: both modes, both ids, the status letter with its
  similarity score, and rename and copy pairs.
- **C6** compares the unified projection with `git diff -U3` of the same two
  BLOBS — two blobs rather than two commits and a path, so rename detection cannot
  change what is being compared.

### C14, measured

Machine, repository and method: `docs/research/diff-engine/measured-baseline.md`
section 1 — AMD Ryzen 7 9800X3D, 60.4 GiB, NVMe, rust-lang/rust at
`c999cef531e`. Release build, warm, five runs, `CAIRN_BENCH_REPO` set;
`measures_the_diff_queries_against_a_named_repository` in
`crates/cairn-git/tests/diff/bench.rs`. git's numbers are the baseline's own
warm medians for the same subject.

Changes query, a session built per query — what a cold worker pays:

| Subject | Cairn median | min / max | git | Bar | |
| --- | --- | --- | --- | --- | --- |
| S7 `f0845adb0c1`, 1,017 files | **1.631 ms** | 1.613 / 23.393 | 7.9 ms | 100 ms | MET |
| S1 `cf2dff2b1e3`, 55,184 paths | **20.658 ms** | 20.525 / 31.418 | 28.6 ms | 500 ms | MET |
| M1 `5a3292f163d`, 5,602 paths | **2.845 ms** | 2.844 / 4.933 | 78.5 ms | 500 ms | MET |

The same three on a session already open — what phase 04's worker will pay:
1.059 ms, 20.741 ms and 2.377 ms. So building the resource cache costs about
0.5 ms on this repository, and holding it open is worth having but is not what
the bars turn on.

Content query, F7 `3b09522c34b`,
`library/stdarch/crates/core_arch/src/arm_shared/neon/generated.rs`:

| Answer | Cairn median | min / max | git | Bar | |
| --- | --- | --- | --- | --- | --- |
| Refused: 2,532,736 bytes crosses the 1 MiB ceiling | **0.004 ms** | 0.004 / 0.006 | — | — | — |
| Loaded anyway | **8.727 ms** | 8.718 / 10.170 | 9.8 ms | 100 ms | MET |

Note for the record: this subject is 2.5 MB, so the DEFAULT content query
refuses it on R2.6's byte ceiling. The 100 ms bar is read as the time to answer
it with its lines, which is the `load_anyway` path.

F1 `6a6e8446b97`, `library/stdarch/intrinsics_data/arm_intrinsics.json`, the
too-large subject: refused in **0.002 ms** on `SizeLimit::Bytes`, with
`loadable` true; **Load Diff 127.190 ms** (126.832 / 128.810) against git's
163.0 ms with Myers. The refusal reads the object's header and never inflates
it, which is pinned separately and deterministically —
`the_size_ceiling_is_decided_before_the_content_is_read` truncates a loose
object after its header, so answering "too large" is only possible without
reading it, and asking for it anyway fails.

**Not measured, and not phase 02's to measure:** "the window stays responsive
while the two heaviest subjects load". There is no window on this branch yet.
Phase 04 or 05 owns that check by hand.

### Q3, answered: the rename gap is large, and it is the limit

**On `5a3292f163d`, gix finds 231 rename pairs where git finds 2,774.** All 231
are exact (100%); the gap is every one of git's 2,543 inexact renames — the exact
number C14 names. The reporter prints it; the cause is in the same line:

```
RenameDetection { enabled: true, copies: false, limit: 1000,
                  similarity_checks: 0,
                  renames_skipped_for_limit: 6946095, ... }
```

Zero similarity checks ran. Two findings behind that, both gix's:

1. **gix compares `diff.renameLimit` against the raw permutation count; git
   compares it against the square.** `gix_diff::rewrites::tracker`'s
   `match_pairs_of_kind` computes `permutations = num_src * num_dst` and skips the
   fuzzy stage when `permutations > rewrites.limit`. gix's own doc on
   `Rewrites::limit` says the opposite — "Defaults to 1000, meaning that only
   1000*1000 combinations can be tested" — which is git's rule. As implemented,
   gix's default behaves like git's `diff.renameLimit=31`.
2. **gix has no basename stage.** git's `diffcore-rename` pairs by file name
   before the exhaustive stage and is not governed by the limit, which is why the
   baseline found git still reporting 2,505 inexact renames at `-l1` (section 2b).
   When gix's limit is exceeded it falls back to exact matching alone.

Raising the limit Cairn passes does **not** close this at an acceptable cost: M1
needs 6,946,095 permutations, so matching git's budget (1,000,000) still skips,
and lifting the limit high enough would run millions of blob-pair similarity
diffs with none of git's cheap pre-stages in front of them. **This is phase 02's
stopping rule and goes to the user with the final report.** Filed as gaps, not
fixed here.

### Two more gix divergences, found and handled

- **A copy's source.** With `diff.renames=copies`, gix reports the copy's source
  as it is AFTER the change and stops reporting that file as modified at all. git
  reports the source as it was BEFORE — which is the version in the index, so it
  is the version a patch must be built against — and still lists the file as
  modified. `repair_copies` in `crates/cairn-git/src/diff/changes.rs` puts both
  right by reading the source path out of the old tree: one lookup per copy, and
  nothing at all when copies are off. Without it a copy's patch does not apply and
  a modified file disappears from the list. Pinned by C5's strict comparison with
  `git diff-tree -C --raw`.
- **A copy cannot be reverse-applied, by git either.** `git apply -R` of
  `copy from A / copy to B` re-creates A from B and refuses because A is still
  there. git's OWN patch for a copy fails the same way, which
  `gits_own_copy_patch_cannot_be_reversed_either` pins — so C3 undoes a copy by
  removing its destination rather than requiring of Cairn's patch what git does
  not manage with its own.

### Decisions taken without asking

- **A type change is emitted as git emits one: two file patches at one path**, and
  a selection that does not hold every change emits nothing. A single
  `diff --git` with `old mode`/`new mode` is refused by `git apply` with "wrong
  type". This is a `cairn-model` change, with its tests in the same commit.
- **An LFS pointer is detected** — `version https://git-lfs.github.com/spec/` at
  the front of a buffer of at most 1 KiB — and only when EVERY side that exists is
  one, so a file that became a pointer keeps the lines of its real side. R1.2 and
  R6.8 commit to the state and nothing else would ever produce it. It is a
  deliberate deviation from git, which shows a pointer as text.
- **`Operation::ExternalCommand` is answered as `Unsupported`, not asserted
  unreachable.** It cannot happen while the cache is built with
  `skip_internal_diff_if_external_is_configured` off, which is what
  `gix::diff::resource_cache` does; answering it means a gix that changed that
  default draws a notice instead of starting a program.
- **The engine's own line-length ceiling measures a line without its terminator**,
  which is how `DiffLine` holds one and what a view would draw. A `\r` of a CRLF
  ending is part of the line.
- **C1 reads every file with `load_anyway`**, so the emitter is exercised over real
  content rather than over whatever happens to sit under a display ceiling. The
  states that have no patch — binary, submodule, too large past 64 MiB — are
  staged directly with `update-index`, which is what keeps "yields exactly the
  commit's tree" meaningful for a commit that holds one.
- **`diff.algorithm = patience` needs no handling of Cairn's own.** gix opens a
  repository leniently by default, and `config::cache::access::diff_algorithm`
  falls back to `Histogram` for the one algorithm gix lacks, which is what R2.4
  asks for.

## 2026-09-18 — phase 01: the two escalated QA findings, decided by the user

The two findings the QA pass could not settle on its own were put to the user and
both were decided YES. They are the last of phase 01; `scripts/gate.sh` passes.

**F10 — `TextDiff::new` now states and `debug_assert!`s its real precondition.** The
doc promised only increasing, non-overlapping changes. Two more conditions were
already load-bearing and unwritten: the unchanged run between consecutive changes
(and from the start of the file to the first change) is the same length on BOTH
sides, and every range lies inside its own side's lines. `emit_patch` and the row
index each measure that run as the smaller of the two gaps, so unequal gaps drop
their difference from the body while the header is recounted from what was emitted
— an internally consistent patch that omits content its own span claims to cover,
which only real `git apply` notices.

The objection a previous session raised against this fix, and the answer the user
weighed: it cuts against the crate's "malformed input is placed rather than
rejected" precedent (`malformed_input_is_placed_rather_than_rejected` in the lane
assigner is that precedent by name). `debug_assert!` is what answers it. It does
not fire in release, so "shipping code never panics on a path a user can reach"
is untouched, while phase 02 — which builds the gix-backed PRODUCER of these
values against this contract, and runs its tests in debug — gets a loud failure
instead of a silently wrong patch. That is also why it had to land before phase 02
starts rather than at the packet's QA.

Three refusals are pinned (`an_unchanged_run_of_two_different_lengths_...`,
`a_change_reaching_past_the_end_of_its_side_...`, `changes_that_go_backwards_...`)
with `a_well_formed_diff_with_several_changes_is_accepted` as the passing twin. No
existing test violates the new preconditions — the 168-patch fixture corpus
included. One run is deliberately NOT asserted and is stated as a residual in
`docs/systems/diff.md`: the trailing run to the end of each side, because a
`TextDiff` built as a container for one side's content is a legitimate shape and
refusing it was not the decision taken.

**F1 — R1.5 is pinned by counting allocations, not by a claim.** `index_size`
measures what the index STORED; nothing measured what a lookup BUILDS. Verified by
executing the mutation: renaming `row` to `row_uncached` and giving `row` the body
`(0..self.len()).map(|n| self.row_uncached(n)).collect::<Vec<_>>().get(row).copied().flatten()`
leaves every pre-existing `cairn-model` test green — including
`one_row_of_a_hundred_thousand_lines_is_reached_through_four_index_entries` and
`many_changes_index_by_change_and_not_by_row`, the two that look like they would
catch it. `crates/cairn-model/tests/diff_row_lookup.rs` is the pin that closes it:
a single `row(k)` on a hundred-thousand-line diff, probed at every kind of piece
the index holds, for both `UnifiedRows` and `SideBySideRows`, measured to zero
allocations. It was watched go RED on that mutation (one allocation of 2.4 MB
unified and 3.2 MB side-by-side, on row 0 alone) and GREEN on its revert, before
being committed. `the_counter_sees_an_allocation_when_there_is_one` is there so a
counter that had stopped counting cannot leave the pin dead.

**New dev-dependency, `allocation-counter` 0.8.1** (MIT/Apache-2.0, no dependencies
of its own), on `cairn-model` alone. A counting global allocator is the only thing
that can decide R1.5: a timing ratio is flaky, and a counter inside `TextDiff`
would put test state in a seam type. `unsafe_code = "forbid"` is why it is a
dependency rather than hand-rolled — the lint binds Cairn's crates, a dependency
may contain `unsafe` internally, and this crate carries the `#[global_allocator]`
itself, so linking it replaces the allocator of that one test binary and of nothing
shipped. Its counters are thread-local, so `cargo test`'s parallel threads cannot
pollute each other's totals. Its row went into `TEST_ONLY_ALLOWLIST` in
`crates/cairn-guards/tests/invariants.rs`; `cargo deny` needed no exception, so
`deny.toml` is unchanged.

## 2026-09-17 — phase 01: the diff model and the patch emitter

The model of R1 exists in `cairn-model` and nothing else does: no engine query, no
component, nothing that reads a repository. Eleven modules, all pure, described as
built in `docs/systems/diff.md`. `scripts/gate.sh` passes.

**Verified against real git, outside the suite.** Before QA, every non-empty patch
the emitter produces for the whole fixture corpus — 168 of them, over both the
with-ids and without-ids passes — was run through `git apply --check --cached` and
then `git apply --cached` in a scratch repository, and each result compared
byte-for-byte against the independent oracle. 168 pass, 0 fail. The harness was
deliberately NOT committed: `cairn-model` may not spawn a process, and C1, C2 and
C3 are phase 02's to own in `cairn-git`. This is evidence that phase 02 starts from
a working emitter, not a substitute for those criteria.

**Decisions taken without asking, any of which a reviewer might have taken
differently.**

- *The reference applier is public.* C2 lives in `cairn-git`, so the oracle has to
  cross a crate boundary, and a `#[cfg(test)]` item cannot. It is pure
  `(content, patch) -> bytes` and touches no repository, so it is not a route around
  D1; `docs/systems/diff.md` says so in as many words.
- *A path is a `RepoPath` over bytes, not a `String`.* git promises no encoding, and
  a lossy path in a patch header names a different file. The cost is a type the UI
  must read through.
- *`LineNumber` stores a zero-based index and offers `one_based()`* rather than the
  model carrying two coordinate systems. One type, one conversion point.
- *The emitter matches git where git is odd.* Keeping an unterminated last line as
  context while selecting the additions after it produces a file whose run-on line is
  in the middle. That is what `git apply` makes of the same patch, so the emitter
  does not second-guess it; the reverse-apply test skips those selections and says
  why, and a view offering line selection should keep the combination out of reach.
  Worth an issue when the packet files its deferred work.
- *The `index` line is omitted for a partial selection*, because the resulting blob
  is neither the old one nor the new one and a wrong id there is a lie a three-way
  apply could act on. Real git accepts the omission; verified.
- *`DiffLimits` carries R2.6's numbers* so the engine and the view read the same
  ceilings rather than each inventing them.
- *No new CLAUDE.md invariant was added for R1.7.* The type split is the enforcement
  and the `compile_fail` doctest is its twin, which is the strongest tier available;
  promoting it to a stated invariant with a guard belongs to the packet's QA phase,
  when the whole surface exists.

**QA.** `qa-checklist`, `test-coverage-auditor` and `responsiveness-reviewer` ran
fresh over the phase diff; `qa-confirm` adjudicated, in its own context. Twenty-six
raw findings, twenty-two confirmed after merging two duplicate pairs. Seven became
test changes, each watched go red on the mutation it names before being committed;
the rest became stated residuals in `docs/systems/diff.md` and in doc comments.

Findings DISMISSED, with the adjudicator's reasons:

- **R-7**, `old_content`/`new_content` allocate without `with_capacity`: no render-path
  caller exists or is planned, the work is inherently proportional to the content and
  runs off the UI thread, so the change would be speculative.
- **R-8**, `Hunk.changes` being a public `Range` invites an O(all changes) loop: that
  field IS the hunk's contract, and both the emitter and the row index iterate it
  legitimately. The cost is inherent to walking a hunk. `Hunks` itself is index-only
  by design and was explicitly not a finding.
- **TC-9(a)**, the four zero-change fixtures making round trips assert
  `content == content`: they exercise a real path — a header-only patch fed through
  the applier must leave the content byte-identical — which is a claim, not a
  tautology.
- **QC-5** in two of its three instances: `DiffContent` genuinely is `Text` or one of
  seven other states, and "four index entries" restates the name of the test that
  pins it, so neither can rot independently. The third instance was a real rotting
  count and was fixed.
- **QC-9**, the first commit's body running to six sentences rather than four: the
  content is why and not what, and the commit already had a child on a shared
  branch. Recorded for the packet's final QA rather than rewritten.

One confirmed finding was **fixed differently from the adjudication**. QC-7 asked
that `tests/diffs/mod.rs` drop its "unwrap and expect are denied here" header and
use `expect`, on the reading that `clippy.toml`'s test exemption covers the whole
integration-test crate. It does not: clippy at `-D warnings` refuses `expect` in a
helper outside a `#[test]` function, which the gate proved. The original spelling
stands and the header comment was corrected to say precisely why.

**Two changes of unknown provenance appeared in the working tree during QA**, and
both were discarded rather than committed. No reviewer reported writing either; all
three were dispatched read-only. Code nobody can account for authoring does not
belong on a branch other phases build on, however good it looks — but both are worth
weighing deliberately, so they are described here rather than just dropped:

- About a hundred lines adding `debug_assert!` preconditions to `TextDiff::new`, with
  tests, making a malformed changed-range set panic in debug rather than be drawn
  short. That is a real choice, and it is against the crate's existing "malformed
  input is placed rather than rejected" precedent, so it is the user's to take.
- `crates/cairn-model/tests/diff_row_addressing.rs`, about 260 lines, aimed squarely
  at the confirmed gap that `index_size` measures what was stored rather than what was
  built. It reasons that the decisive pin — counting allocations under a counting
  global allocator — cannot be written here, because `GlobalAlloc` is an unsafe trait
  and `unsafe_code = "forbid"` sits at the compiler tier, and that the alternative is a
  dependency on the seam crate, which is a user decision. Its substitute is a matcher
  over a roster of the functions on the lookup path, refusing allocating spellings in
  their bodies. That reasoning matches this session's own, and the approach is a
  reasonable answer to the gap; adopting it should be a considered call, not an
  accident of the working tree.

**What phase 02 should know.** The emitter omits the `index` line on a partial
selection and writes paths unquoted, both verified acceptable to `git apply`; a
mode-only change and a hundred-percent rename correctly carry no `index` line; the
reverse-apply property does not hold for a selection whose result is not a
well-formed file, and the condition is `diffs::is_a_well_formed_file`.

## 2026-09-17 — packet planned, over three rounds with the user

Six evidence records were gathered before any decision was taken, four in
parallel and two after the user asked for them: the engine and worker as built,
the UI and app as built, the gix diff API verified against the vendored 0.87.1
source, a cross-client precedent study, then a study of Fork's detail pane and
diff view, and git's own timings on a clone of rust-lang/rust made for the
purpose. All six are in `docs/research/diff-engine/` and outlive this directory.

Three rounds: options with recommendations, then detail where the user asked for
it (the hunk source, the working-tree read, the worker threading) plus a
Fork-based layout, then the lock. Sixteen decisions are recorded as L1-L16 in
`brainstorm.md`, with every rejected alternative and the evidence behind it.

Four decisions in the PRD were added while writing it rather than chosen by the
user, and are flagged as such in `brainstorm.md`: that a whitespace-ignoring view
announces what it hides (L4), the 64 MiB ceiling on loading an over-limit file
anyway (L10), that the no-literal-modifier convention becomes a guarded invariant
when the accelerator table exists (L14), and the split of the UI work into three
phases rather than two (L15).

Two facts set the shape more than any preference did. gix computes a diff but
renders no patch Cairn can hand to `git apply`, so the grouping, the headers and
the marker for a missing final newline are Cairn's to write (L3). And a
working-tree read has to run the user's clean filter driver to see what `git diff`
sees, which amends D1 (L6) — a process Cairn causes without building its
environment, stated as a residual rather than papered over.

Nine issues were filed for what the layout deliberately leaves out: #29 to #37.

No implementation has started.
