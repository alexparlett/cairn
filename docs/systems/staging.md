# Staging, discarding and committing

How the engine stages, unstages, discards, commits and amends today: the write verbs
in `crates/cairn-git/src/ops/`, the stale check before every patch, the
confirmation each discard and an amend are sealed behind, and what the commit box
reads beside them. As-built: everything here is code that exists, with the test
that pins each rule named beside it. The commitment is
`docs/prd/staging-and-commit.md` (R1, R3, R6; in flight); the patches the verbs carry
are the model's (`docs/systems/diff.md`, "Stage, unstage and discard"); how every
`git` process is built and run, and the local write lane each verb runs on, is
`docs/systems/git-processes.md`.

**What exists:** the engine half and the lane. The verbs below, the `Consequence`
builders and the reads they stand on; the application runs every verb on its local
write lane (`docs/systems/git-processes.md`, "The local write lane"). Local Changes and
the commit box ask for the staging, discarding and committing verbs
(`docs/systems/local-changes.md`), and Create Branch for the branch verbs
(`docs/systems/history-graph.md`, "Create Branch").

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
| `unstage_files` (`UnstageTo::Nothing`) | `rm --cached -f -q --pathspec-from-file=- --pathspec-file-nul` | the paths | no |
| `discard_files` | `restore --worktree --pathspec-from-file=- --pathspec-file-nul`, then `clean -f -q -- <paths>` | the tracked paths; nothing | yes |
| `create_branch` | `branch -- <name> <commit>`, the commit by its full id | nothing | no |
| `create_branch_and_checkout` | `checkout -q -b <name> <commit> --` | nothing | no |
| `create_branch_discarding` | `checkout -q -f -b <name> <commit> --`, the name and commit the confirmation names | nothing | yes |

Pinned against a `git` that records its argv, environment and stdin and then
runs the real one (`ops/recording_stub.rs`):
`the_patch_verbs_run_as_r3_names_them`, `the_file_verbs_run_as_r3_names_them`,
`unstaging_resets_or_removes_from_a_pathspec_file` (`ops/stage.rs`),
`a_discard_of_lines_runs_as_r3_names_it` and
`a_discard_of_files_runs_as_r3_names_it` (`ops/discard.rs`) and
`a_branch_is_created_by_git_branch_after_a_double_dash` (`ops/branch.rs`) and
`the_checkouts_run_as_r11_names_them` (`ops/checkout.rs`) — each a write's
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
  has no parent, `git rm --cached -f -q`: without `-f` git refuses a path whose
  staged content differs from both the file and `HEAD` — exactly the path an
  amend's staged list shows edited — and with it the staged content goes and the
  file stays, as `git reset` would leave them
  (`unstaging_out_of_an_amend_puts_back_the_parents_entry_or_none`). A staged
  rename unstages whole by naming both its paths; one path unstages its side
  alone (`a_staged_rename_unstages_whole_by_both_paths_and_one_path_alone_by_its_own`).
- **Lines of a rename** unstage at the path the change has now, whichever of its
  rows asked: where `status.renames` is off and `diff.renames` on, the source's
  row's staged diff is the rename, and its lines go back at the new path, the
  source's entry untouched (`a_rename_sources_row_unstages_its_lines_at_the_new_path`).
- **Staging a conflicted path** is `git add`, which marks it resolved (R3.11).
- **A branch put on a commit** — Create Branch, from `New Branch…` on any commit row
  (R11.3; `docs/systems/history-graph.md`, "Create Branch") — is `git branch`
  with the name after `--`, so a name beginning with `-` is a name git judges, never an
  option. git refuses a name that is taken or not a valid branch name, and its words are
  the failure's (`Error::GitFailed`'s stderr), first line for first line what the user's own
  `git branch` says, nothing written; the branch it makes is the one `git branch` makes,
  logged with the same message (`create_branch_here_makes_the_branch_git_branch_makes`,
  `a_name_git_refuses_is_refused_with_its_reason_and_nothing_written`,
  `crates/cairn-git/tests/diff/branch.rs`). It invalidates the refs, so the lane reads
  everything again after it, however it ended. Whether git takes a name is asked first, by
  `Repository::branch_name` — `git check-ref-format --branch` (`reads::branch_name`), git's
  reason kept for a name it refuses — a name holding `@{` refused before git is asked, "A
  branch name can't contain '@{'" (the user's decision F, 2026-10-09;
  `a_name_holding_at_brace_is_refused_in_cairns_words`) — then the ref, each proper prefix of it and the namespace
  under it looked up by gix, for a name a branch has or one a branch's directory holds, refused
  in git's words (`a_name_clashing_with_a_branchs_directory_is_refused_before_git_runs`) — and
  the read writes nothing
  (`a_branch_name_is_checked_by_gits_rules_and_the_check_writes_nothing`).
- **A branch checked out as it is created**, keeping the changes ("Don't change"), is `git
  checkout -q -b <name> <commit> --`: the changes carried over, or git refusing where one would
  be overwritten — its words the failure's — and then no branch is made
  (`a_kept_checkout_carries_the_changes_or_is_refused_by_git_writing_nothing`).
- **A branch checked out discarding the changes** ("Discard", the user's decision 3) is the one
  operation that discards a staged change — R3.6's stated exception. Its
  `Consequence::CheckoutDiscarding` (`ops::checkout_discarding_consequence`) names the branch,
  the commit, `HEAD`, every tracked path with a staged or an unstaged change (a rename's source
  too), each with its kind, its index entry and its bytes on disk, and its staged and unstaged
  lines together as git's numstat counts them (`reads::change_lines`: `git diff-index --cached
  --numstat` and `git diff-files --numstat`); what `checkout -f` does to the untracked files,
  read from `git ls-files --others --exclude-standard` (`reads::untracked`, so
  `status.showUntrackedFiles=no` hides nothing) and each path's every prefix looked up in the
  commit's tree: a file at the same path is overwritten, by its size; a directory where the
  commit holds a file is removed with every untracked file under it, and a nested repository
  where the commit holds anything is removed whole (`ChangeLoss::Removed`, its files and bytes
  counted on disk); and how many untracked files stay. The prompt (the user's decision A,
  2026-10-09) names the changed files as the files prompt does, then, in a sentence of its own,
  each directory and nested repository deleted by its path — "Deleted because the branch has a
  file there: folder d/ (4 untracked files) and repository vendor/lib/ with its history" — the
  first three named and the rest counted; its title is "Discard changes", its button "Discard
  Changes and Check Out" (`a_directory_or_a_nested_repository_in_the_way_is_named_lost`). It is
  refused before any prompt during a merge, rebase, `git am`, cherry-pick or revert, for a
  conflicted path and for a submodule's change (`a_discarding_checkout_is_refused_where_it_cannot_count_the_loss`),
  computed again and compared before git runs, any difference refusing with the path it moved
  at (`a_discarding_checkout_refuses_what_changed_since_its_confirmation`), and its run loses
  exactly what it named (`a_discarding_checkout_names_every_loss_and_then_loses_exactly_those`,
  `an_untracked_file_in_the_way_is_named_overwritten`,
  `an_untracked_file_in_the_way_is_named_whatever_status_shows`,
  `untracked_files_under_a_directory_the_commit_holds_as_a_file_are_named_lost`,
  `a_nested_repository_the_commit_holds_a_file_at_is_named_lost`). An ignored file at a path
  the commit holds, and an ignored directory where it holds a file, are overwritten or removed
  by any checkout, kept or discarding, as the user's own `git checkout` does: not counted.
- **Every path is literal**: a file named `*.txt` is staged alone, and `git clean`
  of `st*` leaves `stx` (`every_path_is_read_literally_never_as_a_pattern`).
- **Every verb is silent on success**, so one left running by a second close, its
  pipes unread, finishes rather than dying of `SIGPIPE` at its next line (the
  user's decision 12): `git clean` is given `-q`, without which it names each file
  it removes, and `git restore --worktree` and `git apply` print nothing
  (`the_destructive_verbs_say_nothing_on_success_so_an_orphan_finishes`, on every
  floor).

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

- **What is compared.** The index entry (gix), and the working-tree file's bytes
  hashed as they are on disk, with no filter, streamed by gix's hasher (a symlink
  as its target), and its executable bit — so any byte changed since the
  confirmation refuses, a line ending under `core.autocrlf` included, which git's
  form would not show, and so does a `chmod` (PRD R3.9, ratified by the user). A
  discard of lines compares the file's git form too, since that is what its patch
  was built against — a filter attribute added after the confirmation moves it
  while the bytes stay
  (`a_discard_of_lines_refuses_whatever_moved_after_the_confirmation`,
  `a_discard_of_files_refuses_whatever_moved_after_the_confirmation`: an edit,
  the mode alone, the line endings alone, the git form alone, the entry
  restaged, an untracked file edited, removed, replaced by a directory or
  staged).
- **Every directory on a path is a real one.** Each is looked at without
  following a link; a path behind a file or a symlink is refused before any
  prompt (`Refusal::Obstructed`) and by the re-check after one, since `git
  restore` would unlink a file standing where a deleted file's directory was, and
  hashing through a symlinked directory reads a file outside the working tree
  (`a_file_where_a_deleted_files_directory_was_is_never_destroyed`,
  `a_symlinked_parent_is_never_followed_out_of_the_working_tree`). A builder reads a file's bytes before and
  after its slower reads and answers `Error::ContentReadsDisagree` when they
  differ, so a prompt never counts one version and confirms another.
- **A discard of lines applies the patch it was confirmed with.** The builder emits
  it from the diff the user selected in and the `Consequence` carries it; the
  operation takes no diff, so a later diff of the same blobs whose lines align
  otherwise cannot move the selection onto other lines
  (`a_discard_applies_the_patch_it_was_confirmed_with`).
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
- **A mode change** is named with both modes, as the diff draws them — selected
  for a discard of lines ("discard 2 lines and the mode change (100644 to
  100755)") or discarded with a whole file ("1 modified (2 lines and the mode
  change (100644 to 100755))", and alone never "0 lines") — and put back
  (`a_mode_change_selected_for_discard_is_named_and_put_back`,
  `a_whole_files_mode_change_is_named_and_put_back`).
- **Files.** Tracked files are restored from the index by `git restore
  --worktree` (a file deleted in the working tree comes back); untracked files
  are deleted by `git clean -f -q --` with exactly the paths `git status` listed,
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
  intent-to-add file's index blob is the empty one, so `git restore` leaves it
  empty with its entry in place, as the user's own `git restore` does, and the
  prompt names it so — "1 new file emptied (20 lines)" (`FileLoss::Emptied`,
  `an_intent_to_add_files_discard_empties_it_and_says_so`).
- The `Performed` records the prompt the user accepted (R1.6).

## Commit and amend

`ops::commit`, `ops::amend_unconfirmed` and `ops::amend` (`crates/cairn-git/src/ops/commit.rs`)
run `git commit -q -F -` and `git commit -q --amend -F -`, the message on stdin byte for
byte, as a write with the operation's askpass token (R6.1, R6.2, R6.5) — and never
`--literal-pathspecs`: a commit takes no pathspec, and git would export the mode to
every hook it runs, where a `pre-commit` hook's own `git diff -- '*.rs'` would then
match nothing. `-q` leaves out the summary git prints once the commit is made.

| Verb | `git` | stdin | Destructive |
| --- | --- | --- | --- |
| `commit` | `commit -q [--no-verify] -F -` | the message | no |
| `amend_unconfirmed` | `commit -q --amend [--no-verify] -F -`, only where git logs it and no remote has `HEAD` | the message | no |
| `amend` | `commit -q --amend [--no-verify] -F -` | the message | yes |

- **The message is git's to clean.** No `--cleanup` is passed, so `commit.cleanup`
  and git's default for `-F` (`whitespace`: `#` lines kept) decide what is
  stored — byte for byte what `git commit -F <file>` stores from the same bytes,
  under every value and none, with `core.commentChar` unset and `;`, and a draft
  of `#` and `;` lines, CRLF endings, trailing spaces and blank lines, a scissors
  line and non-ASCII text, by commit and by amend, on the host's git and both
  floors (`a_message_is_stored_as_git_commit_f_stores_it_under_every_cleanup`,
  `crates/cairn-git/tests/diff/commit.rs`). The message never reaches `argv`, so
  neither the process table nor the command log holds it.
- **A non-UTF-8 `i18n.commitEncoding`** is refused before git runs
  (`Error::CommitRefused`, `CommitRefusal::CommitEncoding`): the setting as `git config`
  answers it (`reads::commit_encoding`, R6.11 — a linked worktree's `includeIf`, the system
  file and trust read as git reads them, never by gix:
  `a_linked_worktrees_conditional_include_sets_the_commits_settings`), its name read as git
  reads it — `UTF-8` or `UTF8` in any case
  (`a_non_utf8_commit_encoding_is_refused_before_git_runs`,
  `utf8_is_named_as_git_names_it`).
- **Hooks run as git runs them.** `Hooks::Skip` — `--no-verify`, which skips
  `pre-commit` and `commit-msg` and nothing else — is passed only for the skip a failed
  commit or amend offers, which it offers on every failure (R10.5 as C4 amended it): Cairn
  keeps no model of which hooks git would run, so a `core.hooksPath` git resolves, or a hook
  declared in configuration, is git's to find
  (`a_failing_pre_commit_hook_fails_the_commit_with_its_output_and_the_skip_commits`). A
  failing hook fails
  the commit with git's output — the runner's tail of stdout's and stderr's lines
  in the order they arrived, whole and scrubbed (`Invocation::lines`, R4.10),
  since git says "nothing to commit" and "would make it empty" on stdout — and
  nothing is committed, the change still staged and no lock left.
- **The identity is git's** (R6.8): nothing is passed and nothing read, so a commit
  is by the inherited identity variables over the configuration, as a terminal's
  is (`a_commit_is_by_the_identity_in_cairns_environment`), and with none, git's
  own error is the failure (`with_no_identity_gits_own_error_is_the_outcome`).
- **While it runs** a commit is the one write that can be cancelled (R4.3): its
  `CommitWatch` is polled before git starts — a commit cancelled there writes
  nothing (`Error::CommitCancelledBeforeRunning`,
  `a_commit_cancelled_before_git_runs_writes_nothing`) — is handed an
  `ops::CommitCancel` as git starts, which ends git's process group as a fetch's
  cancel does, and is given every line git or a hook writes as it arrives.
- **Made is read from `HEAD`** (R4.7; phase 12's QA item #10). After git is reaped `HEAD` is
  read again: a commit was made when `HEAD` is a commit whose first parent is the `HEAD` it
  ran on (none, on an unborn branch); an amend, when `HEAD`'s parents are the replaced
  commit's, as the object names them (a shallow boundary's included). So a commit or an amend
  cancelled after git made it — its `post-commit` hook running — is reported made, and one
  cancelled before is `Error::GitCancelled`
  (`a_cancelled_commit_or_amend_is_made_exactly_when_head_says_so`); git's exit 0 with
  `HEAD` anything else — something moved it while git ran — is
  `Error::CommitUnconfirmed`, never made
  (`a_commit_git_says_it_made_with_head_elsewhere_is_unconfirmed`, beside "HEAD moved in
  between", `an_amend_refuses_when_head_moved_or_was_published_since_it_was_confirmed`); an
  amend that makes the very object it replaced — one second, the same tree and message — is
  made (`an_amend_that_makes_the_same_commit_is_made`). One git made and then failed after —
  a `die` past the ref update, such as "repository has been updated, but unable to write
  new_index file" — is `Error::MadeButGitFailed`, made, git's words kept, and a confirmed
  amend's token is spent with it (`a_commit_git_made_then_failed_after_is_reported_made`, git
  wrapped to exit 128 after its success, on the host and both floors); after a failure `HEAD`
  is read as after a cancel, an amend's `HEAD` unmoved not made. Where `HEAD` cannot be read,
  git's exit decides.
- **What it invalidates:** the refs, the index (a hook may stage) and the objects.

**The operation in progress** (R6.9, L25) is `Repository::operation_in_progress`
(`crates/cairn-git/src/operation_in_progress.rs`), the files git's `wt_status`
reads — `MERGE_HEAD`, `rebase-apply/` (`applying` in it for `git am`), `rebase-merge/`,
`CHERRY_PICK_HEAD`, `REVERT_HEAD` and the sequencer's next command — rather than gix's
`Repository::state`, which checks in another order and never reads the sequencer, so a
cherry-pick sequence whose stopped pick was committed would read as nothing. A merge, a
single cherry-pick and a single revert are concluded by the commit, as `git commit` concludes
them: a merge's commit is the merge commit, its parents `HEAD` and `MERGE_HEAD`
(`a_merge_in_progress_commits_the_merge_and_refuses_an_amend`); a single pick's keeps the
picked commit's author and git removes `CHERRY_PICK_HEAD`, a single revert's is the
committer's and git removes `REVERT_HEAD`, each equal to what `git commit -F` makes
(`a_single_cherry_pick_is_concluded_by_commit_keeping_its_author`,
`a_single_revert_is_concluded_by_commit`; git keeps no sequencer state for one commit). A
rebase, `git am` and a sequence of picks or reverts (`sequencer/`) refuse a commit and an
amend before git runs, naming git's own command to continue or abort it
(`OperationInProgress::git_command`;
`a_rebase_am_or_sequence_in_progress_refuses_commit_and_amend`), and an amend is refused
during any operation, as git refuses it. A detached `HEAD` is no operation: the refs
snapshot says so, and a commit there is made on no branch, refused by nothing
(`a_commit_on_a_detached_head_is_made`). Each state is made by real git and checked against
what `git status` says.

**The message git prepared** for the commit that concludes a merge, a cherry-pick or a revert
is `Repository::prepared_message`: `MERGE_MSG` as git's own editor session would leave it under
the repository's settings (R6.10, C32; `reads::as_the_editor_leaves`), `commit.cleanup` and
the comment character read with `git config` in query form (`reads::commit_cleanup`,
`reads::comment_char_is_auto`) and the cleaning done by `git stripspace`, the message on stdin,
run in the repository so git reads the comment character itself: unset, `default` or `strip`,
`--strip-comments`; `whitespace`, no option; `verbatim`, the message as written; `scissors`, cut
at git's scissors line (`wt_status_locate_end`, the comment string asked of `git stripspace
--comment-lines`, so `core.commentString` counts where git reads it) and then no option; and
under `core.commentChar=auto`, no option for every mode but `verbatim`, since the character
git's commit picks starts no line of the message. Every setting — each cleanup, unset and the
five, under `#`, `;` and `auto` — leaves exactly what `git commit` with an editor leaves, and
the box's `git commit -F -` of it stores the same, on git 2.30.9, 2.32.7 and the host's
(`merge_msg_is_shown_and_committed_as_gits_editor_leaves_it_under_every_setting`, with
`each_setting_shows_a_message_of_its_own` holding the settings apart). git's template around
the message in an editor session — the help lines and the status, which under `whitespace`
and `verbatim` a person removes by hand — is not the message's, and the oracle leaves it out
(`commit.status=false`). It writes nothing and runs nothing
(`the_stripspace_read_writes_nothing_and_runs_nothing`).

**What the commit box reads** beside the verbs, each on a worker:
`Repository::operation_in_progress` and, while one a commit concludes is in progress,
`Repository::prepared_message`; `Repository::recent_messages`, the messages of the last `RECENT_MESSAGES` (ten)
commits `HEAD` reaches in `git log`'s order, each as written
(`recent_messages_are_git_logs_last_ten`); and `Repository::amend_staged`, amend's
staged list (R6.3): the index against `HEAD`'s parent — the empty tree for a root
commit — through `reads::staged_since`, `git diff-index --cached --raw` over the
whole index with the user's rename detection, what `git diff --cached
--name-status HEAD^` lists (`amends_staged_list_is_the_index_against_heads_parent`);
`Repository::amend_parent`, the commit that list is against — `None` for a root commit or a
shallow clone's boundary — which an unstage out of the amend puts entries back to; and one
file of that list's diff, `WorkingTreeDiff::Amending`, the staged side's own plumbing asked
against the parent, as `git diff --cached HEAD^ -- <path>` shows it
(`a_file_of_amends_staged_list_reads_as_git_diff_cached_against_heads_parent`). How the box
uses them is `docs/systems/local-changes.md`, "The commit box".
Amend is unavailable on an unborn branch (`amend_is_unavailable_on_an_unborn_branch`);
a root commit amends (`a_root_commits_amend_works_and_unstages_with_rm_cached`).

### Amend, at the press and sealed

What an amend costs is read when Amend is pressed, in the job that then runs it (R6.4, rules
2 and 3 of the redesign), never on a refresh: `ops::amend_unconfirmed` reads it and, where git
logs the amend and no remote has `HEAD` — recoverable from Show Lost Commits — runs it at once,
taking no token and recording no prompt (`an_amend_git_logs_and_no_remote_has_runs_at_once`,
the replaced commit then in the reflog git wrote and drawn lost by Show Lost Commits); any other
it answers without running git (`AmendAnswer::NeedsConfirming`, `HEAD` and the index
untouched: `an_amend_a_remote_has_or_git_logs_nowhere_is_answered_not_run`, a remote's ref, a
`false` with no log, and a `false` set only through a linked worktree's `includeIf`), for the
confirmation dialog, whose token `amend` then takes. Which is which is
`Consequence::needs_confirming`. `amend_unconfirmed` is off the destructive-operation roster:
what it runs loses nothing.

`amend` takes a `Confirmed` by value and is a row of the destructive-operation
roster. Its `Consequence::Amend` is computed by `ops::amend_consequence`
(`crates/cairn-git/src/ops/amend.rs`), from the repository now, and holds what the amend's
freshness rests on and nothing drawn for display alone — no subject
(`the_amends_consequence_names_head_and_why_it_is_confirmed`):

- **`HEAD`'s id.**
- **Whether a remote already has it** (`Publication`, R6.4). With an upstream that
  is a remote-tracking ref and exists, whether it reaches `HEAD` — its ahead count
  is zero — answered by the walk `HEAD --not <upstream>`, which stops at the first
  commit it yields; when it does not, or with none — a detached `HEAD`, no upstream,
  a gone one, one that is a local branch — the same walk hidden by every
  remote-tracking ref, `HEAD --not --remotes`, so `Unpublished` means no
  remote-tracking ref reaches it. Each is cancellable at every object read
  (`the_dialog_is_asked_exactly_when_a_remote_has_head`: an upstream at, ahead of
  and behind `HEAD`, behind it with another remote branch holding it, a branch with no upstream at a remote's commit and past it, a
  fork whose remote branch was deleted, a local upstream, a detached `HEAD`).
- **Whether git will write the reflog entry** (`Reflog`, R6.4 and R10.6 as amended):
  when `core.logAllRefUpdates` is `true` or `always` — or, unset, unless the
  repository is bare, which a linked worktree of a bare repository is not — or,
  whatever it is set to, when `HEAD`'s or the branch's log already exists, since git
  appends to a log it finds. The setting is `git config`'s answer
  (`reads::log_all_ref_updates`, R6.11): its last value as written, `always` in any case taken
  first, as git's own reading does, and any other parsed by git's boolean rules — so an
  `always` in the user's global configuration beneath the `true` `git init` writes is read as
  git reads it, which `git config --type=bool` would refuse; a bare key or an empty value is
  asked of git's boolean parse, which tells them apart
  (`the_commits_settings_are_read_as_git_reads_them`). Each arm checked against the entry git
  then writes (`whether_the_reflog_is_written_is_what_git_then_does`: the default, `false`
  with no log and with one, `always`, and a bare repository's worktree with the setting unset
  and `false`).

Its prompt is the fixed sentences of R10.6 — "<id> is already on <remote ref>. Amending it
rewrites history others may have." (a remote's ref by its short name, or "a remote" where no
upstream holds it and another remote-tracking ref does) and "<id> can't be recovered after
this: this repository keeps no reflog.", both where both hold — its button "Amend".

Refused before any of it: an unborn branch and any operation in progress, a merge
among them. Before git runs, `amend` computes the `Consequence` again and refuses
with `Error::AmendChangedSinceConfirmed`, writing nothing, when it differs from the
confirmed one — `HEAD` moved, a remote came to hold it, the reflog setting changed
(R1.4, `an_amend_refuses_when_head_moved_or_was_published_since_it_was_confirmed`,
`an_amend_refuses_when_the_reflog_it_promised_is_gone_since_it_was_confirmed`);
its `Performed` quotes the accepted prompt (R1.6,
`an_amend_records_its_prompt_and_a_commit_invalidates_what_it_moves`). An amend git refused
before amending — a hook failed, `HEAD` read unmoved after the reap — hands its token back
unspent, `Error::AmendNotMade { failure, unspent }` (R1.1's option (a), as (a′)): git's index
refresh and the tree objects it writes before a hook runs are its own bookkeeping, outside
what the consequence names, and the skip's run re-checks it; a refusal before git ran or a
cancel hands none back (`a_hook_failing_a_confirmed_amend_hands_the_token_back_and_the_skip_amends`).
The local lane lets the returned token go, and the window's skip builds a new token from the
same consequence (`AmendSkip`).

**What the walk costs** on rust-lang/rust (340,228 commits, the bench clone at
`c999cef531e`, 13 remote-tracking refs, release build, warm; the
`the_pushed_check_on_a_large_repository` reporter): an upstream at `HEAD`, a branch
at a remote's tip, and a new commit whose upstream is behind it, each under 3 ms; a
new commit with no upstream, 91-99 ms; a detached `HEAD` at a commit 2,000 or
30,000 first parents back — `HEAD` reached only after painting from every remote
tip — 1.1-1.3 s, which is git's own walk without a commit-graph (`git -c
core.commitGraph=false rev-list -1 HEAD --not --remotes`, 1.14 s; 0.08 s with the
graph, which the walk does not read so it can be cancelled at every object). Where the
repository has a commit-graph holding `HEAD` and every remote tip the check reads, the graph
answers alone (`crate::history::walk::reaches_through_graph`, phase 11): a walk down from the
remote tips through the graph's parents that never descends below `HEAD`'s generation, as git's
own `repo_is_descendant_of` cuts it, cancellable at every commit — 0.5 ms for that detached
`HEAD` 20,000 commits behind on the bench clone with a graph written, against 1.2 s by the
object walk and git's 10 ms
(`the_pushed_check_answers_from_a_commit_graph_as_git_does`). Without a graph — the bench
itself has none — the object walk still costs 1.2 s there, cancellable, and an amend pays it
at the press, again in a confirmed amend's re-check, and, while Amend is ticked, once per
status the window reads
(phase 09's QA item 9, measured and not split: 0.14 ms for the consequence and 2.6 ms for
amend's staged list with `HEAD` at a remote tip).

**`Remove index.lock…`** (R12.4): `ops::remove_lock_consequence` and the destructive
`ops::remove_index_lock` (`src/ops/remove_lock.rs`), the one mutation not made by git —
`docs/systems/git-processes.md`, "The activity popover".

## Residuals

- Between a re-check and git's run is a window no check closes — the user's own
  `git checkout -p` has it too.
- `discard_files` is two writes, and `git clean` deletes what it can before it
  exits non-zero on the rest. So every confirmed file is read again after the run,
  and a discard that did not take every one answers `Error::DiscardIncomplete`: the
  `Performed` record (quoting the accepted prompt), git's failure if one failed, and
  the confirmed paths still exactly as they were — found by reading each again, never
  by parsing git's prose. That also names a file `git clean -f` leaves alone, an
  ignored one (`a_discard_that_fails_part_way_says_what_it_did_and_what_is_left`,
  `a_file_git_clean_leaves_is_named_as_kept`).
- `discard_files_consequence` counts any path absent from the index as untracked
  (`IndexSide::Absent`), so an ignored file, or a path git never listed, is offered as
  one. The engine trusts its caller here, and the caller owns it: Local Changes asks only
  for paths of rows `git status` listed in the lists it draws, each found again there by a
  search, so a selection naming a path a refresh took away asks nothing for it
  (`local_changes_actions`; `a_discard_names_only_paths_the_lists_drawn_still_list` in
  `cairn-app`, and the engine's arm by `a_file_git_clean_leaves_is_named_as_kept`).
- A discard of files counts each tracked file's lines by reading its unstaged diff
  as the diff view does, one read per file: exact, and proportional to the files
  selected.
- git exports `--literal-pathspecs` to the hooks it runs under these verbs
  (`GIT_LITERAL_PATHSPECS=1` in a `post-index-change` or `post-checkout` hook), so a
  hook's own globbed pathspec matches literally (R3). A commit is not run with it.
- Between an amend's re-check and git's run is the same window; `git commit
  --amend` amends whatever `HEAD` is when it runs.
- An amend at the press without a token (`ops::amend_unconfirmed`) has the window too, and
  there it decides whether the amend was recoverable: the cost is read, then git runs the
  `pre-commit` and `commit-msg` hooks, then updates the ref. A push of `HEAD`, or a checkout,
  from a terminal while a slow hook runs publishes the commit being replaced, or moves what
  git amends, and Cairn still reports "amended <short>" with no token taken. The replaced
  commit stays in the reflog git writes, so Show Lost Commits finds it; where `HEAD` was moved,
  the amend reads `Error::CommitUnconfirmed`, whose text says git may have amended what `HEAD`
  had become. No check after the run asks whether a remote came to hold the replaced commit
  meanwhile (the user's to decide, phase 13's QA item 2(b)).
- The pushed check knows only what was last fetched, as `git branch -r --contains`
  does.
- `Reflog::Written` says git will append the amend's entry, whose old id is the
  replaced commit; with a log the amend itself created, that old id is the only place
  the replaced commit is named. Show Lost Commits seeds from every entry's old and new
  ids, as `git rev-list --reflog` does (`docs/systems/history-graph.md`, "Show Lost
  Commits"), reading the same two logs the check above looks for — `HEAD`'s in the
  worktree's git directory, the branch's in the common one — so the prompt's "The old
  commit stays in Show Lost Commits." holds exactly when git logs the amend: on every arm
  of `whether_the_reflog_is_written_is_what_git_then_does` the replaced commit is drawn
  when git logged the move or a ref still reaches it, and dimmed exactly when none does. git
  expires an unreachable commit's entry after `gc.reflogExpireUnreachable` (30 days by
  default), and from then on Show Lost Commits no longer has it.
- In a partial clone, amend's staged list pairs a staged inexact rename against
  `HEAD^` by comparing blobs, and a blob only the promisor holds is never fetched by
  the read: on git 2.44 and later the whole list fails (`Error::GitFailed`, nothing
  listed) where the user's own `git diff --cached HEAD^` would fetch and answer; git
  before 2.44 ignores `GIT_NO_LAZY_FETCH` and fetches, writing a pack — the same
  residual as status's
  (`in_a_partial_clone_amends_staged_list_fails_rather_than_fetching`,
  `crates/cairn-git/tests/status.rs`).
- A commit or amend left running by a second close can still die of `SIGPIPE` at a
  line its hook writes.
- A hook owned by another user counts when the group's or others' execute bit is
  set, whether or not this user is in the file's group: the process's groups are
  not read.
