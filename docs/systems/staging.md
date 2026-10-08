# Staging and discarding

How the engine stages, unstages and discards today: the write verbs in
`crates/cairn-git/src/ops/`, the stale check before every patch, and the
confirmation each discard is sealed behind. As-built: everything here is code that
exists, with the test that pins each rule named beside it. The commitment is
`docs/prd/staging-and-commit.md` (R1, R3; in flight); the patches the verbs carry
are the model's (`docs/systems/diff.md`, "Stage, unstage and discard"); how every
`git` process is built and run is `docs/systems/git-processes.md`.

**What exists:** the engine half. Six verbs, two `Consequence` builders and the
reads they stand on. Nothing in the application calls them yet: the local write
lane is staging-and-commit phase 04, and the views that act are phases 07-09. So
no window stages, unstages or discards today.

## The verbs

Every verb is a named operation in `cairn_git::ops`, built as a write
invocation in one place (`ops/local_write.rs`'s `run`), run with git's global
`--literal-pathspecs` before the verb — never the `GIT_LITERAL_PATHSPECS`
variable — its input on stdin, and the lock files under the git directories
listed before it starts and after it is over, on its `Performed`
(`Performed::locks`, R3.8). A local write is not cancelled (R4.3), and takes an
askpass token where its caller has one.

| Verb | `git` after `--literal-pathspecs` | stdin | Destructive |
| --- | --- | --- | --- |
| `stage_lines` | `apply --cached --whitespace=nowarn -` | the model's patch of the unstaged or untracked diff | no |
| `unstage_lines` | `apply --cached --whitespace=nowarn -` | the model's patch of the staged diff, inverted | no |
| `discard_lines` | `apply --whitespace=nowarn -` | the model's patch of the unstaged or untracked diff, inverted | yes |
| `stage_files` | `add --pathspec-from-file=- --pathspec-file-nul` | the paths, NUL-terminated | no |
| `unstage_files` (`UnstageTo::Head`) | `reset -q --pathspec-from-file=- --pathspec-file-nul` | the paths | no |
| `unstage_files` (`UnstageTo::Commit(id)`) | `reset -q --pathspec-from-file=- --pathspec-file-nul <id>` | the paths | no |
| `unstage_files` (`UnstageTo::Nothing`) | `rm --cached -q --pathspec-from-file=- --pathspec-file-nul` | the paths | no |
| `discard_files` | `restore --worktree --pathspec-from-file=- --pathspec-file-nul`, then `clean -f -- <paths>` | the tracked paths; nothing | yes |

Pinned against a `git` that records its argv, environment and stdin and then
runs the real one (`ops/recording_stub.rs`):
`the_patch_verbs_run_as_r3_names_them`, `the_file_verbs_run_as_r3_names_them`,
`unstaging_resets_or_removes_from_a_pathspec_file` (`ops/stage.rs`),
`a_discard_of_lines_runs_as_r3_names_it` and
`a_discard_of_files_runs_as_r3_names_it` (`ops/discard.rs`) — each a write's
environment with no read pin and no `GIT_LITERAL_PATHSPECS` (C9). What each does
is pinned against real git on the host's git and, through
`scripts/git-floor.sh`, on 2.30.9 and 2.32.7: C3 runs every case of
`crates/cairn-git/tests/diff/staging.rs` through `stage_lines`,
`unstage_lines` and `discard_lines`, and
`crates/cairn-git/tests/diff/write_verbs.rs` holds the rest.

- **Never `-R`, `--recount`, `--3way`, `--unidiff-zero`, `-C` or
  `--allow-overlap`.** The patch is the model's, built from the exact diff the
  user selected in, its counts and context exact; a patch that does not apply as
  it is must not be made to.
- **`--whitespace=nowarn`** outranks the user's `apply.whitespace`, which would
  strip trailing whitespace from what is staged (`fix`) or refuse it (`error`)
  (`trailing_whitespace_is_staged_byte_for_byte_whatever_apply_whitespace_says`,
  which proves git's default changes it first).
- **`apply.ignoreWhitespace` is left to the user.** Measured on 2.30.9, 2.32.7 and
  2.56.0 over 400 selections of a diff among lines that differ only in
  whitespace: `-c apply.ignoreWhitespace=change` changed nothing a fresh patch
  stages. It lets a patch land on context that moved by whitespace alone, and the
  stale check refuses such a patch before git runs
  (`apply_ignore_whitespace_stages_exactly_the_selected_lines`).
- **Unstaging** is `git reset -q`, which works on an unborn branch where `git
  restore --staged` does not
  (`unstaging_on_an_unborn_branch_leaves_the_file_untracked`); out of an amend,
  against `HEAD`'s parent, given by its id; out of a root commit's amend, which
  has no parent, `git rm --cached -q`, which git refuses where the staged content
  differs from both the file and `HEAD`
  (`unstaging_out_of_an_amend_puts_back_the_parents_entry_or_none`). A staged
  rename unstages whole by naming both its paths; one path unstages its side
  alone (`a_staged_rename_unstages_whole_by_both_paths_and_one_path_alone_by_its_own`).
- **Lines of a rename** unstage at the path the change has now, whichever of its
  rows asked: where `status.renames` is off and `diff.renames` on, the source's
  row's staged diff is the rename, and its lines go back at the new path, the
  source's entry untouched (`a_rename_sources_row_unstages_its_lines_at_the_new_path`).
- **Staging a conflicted path** is `git add`, which marks it resolved (R3.11).
- **Every path is literal**: a file named `*.txt` is staged alone, and `git clean`
  of `st*` leaves `stx` (`every_path_is_read_literally_never_as_a_pattern`).

## The stale check

Before git runs, every patch verb reads the index file again with gix
(`ops/fresh_state.rs`, `IndexNow`) and refuses with `Error::ChangedSinceRead`,
naming the path and writing nothing, unless the entry is the side the patch was
built from: for a stage, the diff's old id and mode — or, for a new file, no
entry or an intent-to-add one; for an unstage, the staged diff's new id and mode
at its path. A discard of lines checks the index the same way and the working-tree
file twice: in git's form, through `git hash-object --path=<p> -- <p>`
(`reads/hash_object.rs`, a read, never `-w`), which must be the drawn side's id
(R3.7), and as its bytes are, below. Without the check, `git apply` lands a stale
patch at an offset on lines the user never selected: C6's tests prove that git
would, then that the verb refuses
(`a_stale_stage_writes_nothing_where_git_apply_would_land_it_at_an_offset`,
`a_stale_unstage_writes_nothing`,
`a_stale_discard_writes_nothing_where_git_apply_would_land_it_at_an_offset`).

`hash-object --path` reproduces the id git's diff gives a working-tree side under
`core.autocrlf` and a clean filter, on all three gits, and writes nothing
(`hashing_a_file_gives_git_diffs_id_and_writes_nothing`, which holds the git
directory byte-identical). A symlink is hashed by Cairn as its target, as git
stores one, since `hash-object` would read the file it points to.

## Discards, sealed

`discard_lines` and `discard_files` take a `cairn_model::Confirmed` by value and
are the destructive-operation roster's rows. Each `Consequence` is computed by the
engine, from the repository now, by `discard_lines_consequence` and
`discard_files_consequence`; the operation derives every target from it, and
re-reads the repository the moment before it runs, refusing with
`Error::ChangedSinceConfirmed` — writing nothing at all, not even the files that
did not move — when anything it names moved (R1.4).

- **What is compared.** The index entry (gix), and the working-tree file hashed
  as its bytes are on disk, with no filter, streamed by gix's hasher (a symlink
  as its target) — so any byte changed since the confirmation refuses, a line
  ending under `core.autocrlf` included, which git's form would not show (phase
  01's QA item 25). A discard of lines compares the file's git form too, since
  that is what its patch was built against
  (`a_discard_of_lines_refuses_whatever_moved_after_the_confirmation`,
  `a_discard_of_files_refuses_whatever_moved_after_the_confirmation`: an edit,
  the line endings alone, the entry restaged, an untracked file edited, removed,
  replaced by a directory or staged). A builder reads a file's bytes before and
  after its slower reads and answers `Error::ContentReadsDisagree` when they
  differ, so a prompt never counts one version and confirms another.
- **The diff a discard of lines is handed** must be the one confirmed — its path
  and both ids the `Consequence`'s — or it is refused
  (`Refusal::NotWhatWasConfirmed`): the patch comes from that diff, never a re-read.
- **Refused before any prompt** (`Error::Refused`, with a `Refusal`): a selection
  of nothing, or the mode alone where there is no mode change (no "Discard 0
  Lines"); no file at all (`Error::NoPaths`); part of a change only a file verb
  takes; every line of a new file, whose discard deletes it and so is the file
  verb's, with a prompt that says the file is deleted and can't be undone
  (`every_line_of_an_untracked_file_is_discarded_by_the_file_verb_which_says_so`);
  a conflicted path (R3.11); a submodule (R3.10, `a_submodule_is_never_discarded`);
  a repository nested in the working tree, by either spelling, and any other
  directory (`a_nested_repository_is_refused_before_any_confirmation`); and a path
  with no unstaged change, since staged changes are never discarded
  (`staged_changes_are_never_discarded`, R3.6).
- **A mode change selected for discard** is named with both modes ("discard 2
  lines and the mode change (100644 to 100755)"), and put back
  (`a_mode_change_selected_for_discard_is_named_and_put_back`).
- **Files.** Tracked files are restored from the index by `git restore
  --worktree` (a file deleted in the working tree comes back); untracked files
  are deleted by `git clean -f --` with exactly the paths `git status` listed,
  each a file. git lists untracked files one per file (`docs/systems/status.md`),
  and the one directory it still lists whole is a nested repository, which is
  refused, so `-d` is never passed and a file added beside a confirmed deletion is
  never taken (`a_file_added_beside_a_confirmed_deletion_is_never_taken`;
  `a_discard_of_files_restores_the_tracked_and_deletes_exactly_the_untracked`
  reads the command log for the one `clean`). `git clean` reads no pathspec file,
  so its paths go on `argv`, split into invocations of at most
  `ops::CLEAN_ARGUMENT_BYTES` (64 KiB, each path counted with its NUL and its
  pointer) — all after the one re-check made before the first
  (`a_long_list_is_deleted_in_batches_after_one_recheck`,
  `a_list_past_the_bound_is_split_and_every_path_kept_in_order`). An
  intent-to-add path is a tracked file whose index blob is empty: `git restore`
  leaves it empty, as the user's own `git restore` does, and the prompt counts its
  lines as modified.
- The `Performed` records the prompt the user accepted (R1.6).

## Residuals

- Between a re-check and git's run is a window no check closes — the user's own
  `git checkout -p` has it too.
- `discard_files` is two writes: where `git clean` fails after `git restore`
  succeeded, the restore has happened, and the failure says which verb failed.
- A discard of files counts each tracked file's lines by reading its unstaged diff
  as the diff view does, one read per file: exact, and proportional to the files
  selected.
- A mode change in a discard of whole files is restored without being named; the
  prompt counts lines only.
- git exports `--literal-pathspecs` to the hooks it runs under these verbs
  (`GIT_LITERAL_PATHSPECS=1` in a `post-index-change` or `post-checkout` hook), so a
  hook's own globbed pathspec matches literally (R3).
