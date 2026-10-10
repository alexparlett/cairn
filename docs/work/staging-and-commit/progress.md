# Progress — staging-and-commit

Running log, newest first. Dismissed QA findings are logged here with their
reasons, per phase.

## 2026-10-10 — the user's decisions on phase 14's open items

Recorded before phase 15 (packet mode): DO#1 — Create Branch's Discard deletes an untracked
nested repository in the way, with its history, silently, as Fork's does (PRD R11.3; residual in
`docs/systems/staging.md`; `an_untracked_nested_repository_in_the_way_is_deleted_as_forks_discard_deletes_it`
on 2.56.0, 2.30.9 and 2.32.7; git-floor's `diff_engine` floor 217); "revisit" on state.md's
teardown list. 11 — its activity entry describes what happened rather than quoting a prompt never
drawn (PRD R12.1, phase 20's doc). 16b — "An am session is in progress. Finish or abort it first."
(`cairn_ui::discard_refusal`, PRD R11.3). 16a (the sentence) is phase 15's frame.

## 2026-10-10 — phase 14's QA fixes (packet mode)

The coordinator's adjudication (`qa-p14/adjudication.md`): FIX NOW 1, 2, 3, 4, 6, 8, 9, 10; CARRY
5b (phase 15), 7, 13, 16c-e (phase 19); FILE 14, 15 (state.md's list); USER DO#1, 11, 16a, 16b.
Dismissed by the adjudication: 5a (the ⚠ is drawn and pinned), 12 (no write can be asked ahead
of Discard's ask while the dialog is up).

- **#1** pinned: `an_older_names_discard_answer_never_builds_the_token`; the mutation dropping
  `discarding_for`'s name filter fails it. The optional belt (clearing `discarding` on a new
  ask) left out, so the test keeps pinning the filter.
- **#2, #3** test first: `a_token_for_another_name_or_choice_is_let_go_of` and
  `nothing_is_asked_while_a_creates_git_error_is_up` failed on the code (a token for another
  name spent; a discard asked under the Git Error), then passed with `discard_confirmed`'s
  re-check and both handlers' Git Error guard.
- **#4** test first: `an_operation_begun_during_the_recheck_is_refused_before_git_runs` (a stub
  `git` writing `MERGE_HEAD` when asked `check-ref-format`, then the real git) failed — the
  forced checkout ran and removed the merge — then passed with the check moved last.
- **#8, #9**: `the_windows_keys_are_inert_while_create_branch_is_up` gained the Git Error alone
  (the dialog cancelled while its write ran; the write's own read-again counted out), and
  `a_cancel_under_the_git_error_leaves_the_dialog_as_it_was`; the mutations `is_up` without the
  error and `cancel` without its early return each fail one.
- **#6, #10** docs: the reviewer's check 3 and CLAUDE.md's residual for B2; the `--no-track`
  claims moved onto the literal argv.

## 2026-10-10 — phase 14 built, Create Branch as Fork runs it (packet mode)

What shipped is state.md's "Phase 14". Notes for the record:

- **Decided here**: the consequence is keyed by the name it was asked for, and asked again as
  the name changes while Discard is chosen, so the token a press builds is always for the name
  shown; the dialog stays open, its button disabled, while its write runs, and closes when it is
  done (a second press asks nothing); a name re-check that is no longer free refuses with
  `CheckoutMoved::Name`; `Cancel` is ignored while a create's Git Error is over the dialog.
- **Found on the way**: Freya's `Popup` consumes Escape and fires global key listeners in
  document order (`freya-core/tests/global_key_order.rs` in the vendored fork), so a Git Error
  drawn after the dialog never heard Escape; it is placed first in the window's tree, wrapped one
  overlay higher (`Layer::Overlay`) so it still paints above. Pinned by
  `a_failed_create_opens_the_git_error_over_the_dialog_left_as_it_was`.
- **Fork's command as the oracle**: `a_discarding_checkout_leaves_what_forks_command_leaves` runs
  Fork's own `git checkout --no-track -b <name> <commit> --force` on an identical fixture and
  compares the status, every working-tree file's bytes and the branch's upstream.

## 2026-10-10 — phase 14 stopped on its rule, then decided

The forced checkout probed on the host and both floors (evidence
`docs/research/staging-and-commit/create-branch-discard-probe-2026-10-10.md`): exit 0 in every case;
staged, unstaged, a staged new file and an untracked file in the way gone, other untracked files
kept; a merge's conflict discarded and the merge abandoned (`MERGE_HEAD`, `MERGE_MSG` removed); a
stash-apply conflict discarded; a submodule's moved checkout, its inside edit and a staged change of
its commit left in place (the last listed again as unstaged) — the stopping rule. The user's answer
("Agree take them"): no refusal over a submodule or a conflicted path, as Fork; an operation in
progress refused before git runs. The branch-name probe dismissed the review's M3.

## 2026-10-10 — the user's ratification after phase 13's QA

The user answered "Those are fine" to the coordinator's recommendations: (1) the token hand-back
as (a′) ratified; (2) R6.10 and C32 as amended ratified, the stripspace read's
`--strip-comments` and `--comment-lines` accepted; (3) option B for the token-free amend's window
— a post-run re-check of publication and the reflog, returned as data (phase 18, step 2.7), and
reported in the activity entry (phase 20), never prevented. Written into the PRD (its opening,
R1.1, R1.5, R6.10, C32 and a new C35), phase 18's and phase 20's docs; 2(b) is off state.md's
open and teardown lists.

## 2026-10-10 — phase 13's QA fixes (packet mode)

The coordinator's adjudication (`qa-p13/adjudication.md`): FIX NOW 1, 2(a), 3, 5, 6, 7, 8, 11;
CARRY 4 and 9 (phase 18), 12 (teardown); FILE 10 and the gc note (state.md's list); USER 2(b).
Dismissed by the adjudication: 13, 14, 15, 16.

- **#1** test first: `a_commit_git_made_then_failed_after_is_reported_made`, git wrapped to exit
  128 after its success, failed on the code (`GitFailed`), then passed with
  `Error::MadeButGitFailed { verb, failure }` (read as after a cancel); a confirmed amend's token is
  spent. The lane ends it `Failed` with git's command and words; its wording carried to phase 19.
- **#2(a)** stated in `staging.md`'s residuals and `ops/commit.rs`'s module doc; `CommitUnconfirmed`
  for an amend now says git may have amended what `HEAD` had become
  (`an_unconfirmed_amend_says_git_may_have_amended_what_head_became`).
- **#3** `a_failed_amend_whose_head_moved_hands_no_token_back`; M3 (the hand-back's `HEAD` check
  replaced by `true`) fails it on 2.56.0, 2.30.9 and 2.32.7, and is restored.
- **#5, #6** destructive-ops-reviewer's check 13 (token-free paths, the predicate, the window, the
  hand-back) and "four accepted exceptions".
- **#7, #8, #11** docs; the three systems sentences naming phase 18 restated as current fact, the
  intent moved into phase-18-commit-box.md's step 6.

## 2026-10-10 — phase 13, the coordinator's decisions applied (packet mode)

- **Token hand-back (a′)**: built as ruled — `Error::AmendNotMade { failure, unspent }`, git's
  failure with `HEAD` unmoved after the reap; the holder row and its pin
  (`the_engine_error_holds_a_token_only_as_an_unspent_amend`, self-test
  `the_engine_error_token_matcher_catches_the_shapes_it_claims`); the PRD's R1.1 records the
  reading. Real-git pin on the host and both floors:
  `a_hook_failing_a_confirmed_amend_hands_the_token_back_and_the_skip_amends`.
- **Stripspace parity**: git's rules from `git help commit` (`--cleanup`), `git help config`
  (`core.commentChar`, `commit.cleanup`, `commit.status`) and git's `cleanup_message`,
  `wt_status_locate_end` and `adjust_comment_line_char` as the oracle confirms them: under `auto`
  the picked character starts no line, so nothing is a comment and no scissors line is found —
  whitespace only. The oracle is `git -c commit.status=false commit` with `GIT_EDITOR=true`,
  leaving out the template (help lines, status) a person removes by hand under `whitespace` and
  `verbatim`. 18 settings × the host and both floors agree; no version difference.
  `where_gits_editor_is_not_the_strip_of_comments_it_is_pinned` and
  `merge_msg_is_cleaned_and_committed_as_gits_editor_leaves_it` replaced.

## 2026-10-10 — phase 13 built, the commit engine asks git (packet mode)

Built on `feature/staging-and-commit` from d81f3ef. What shipped is state.md's "Phase 13". Notes
for the record:

- **Evidence gathered first, on 2.56.0, 2.30.9 and 2.32.7.** `git commit` with an editor leaves
  exactly what `git stripspace --strip-comments` leaves under the default cleanup with `#` or `;`,
  and under `scissors` where the merge's own message holds no comment-character line; it keeps
  more under `whitespace`, `verbatim`, `scissors` with such a line, and `core.commentChar=auto`
  (git's commit then picks another comment character). `git commit -F -` during a single
  cherry-pick keeps the picked author and removes `CHERRY_PICK_HEAD`; during a single revert it
  removes `REVERT_HEAD`; git keeps no `sequencer/` for one commit and does for two. `git config
  --type=bool --get` parses every value of a key, so it fails on `always` anywhere above the last
  value; git's own reading takes `always` case-insensitively (`ALWAYS` logs tags). A failing
  `pre-commit` leaves `.git/index` rewritten and two loose tree objects written — the stopping
  rule's evidence.
- **Decided here**: carried #10 — a commit or amend reported made must leave `HEAD` the commit it
  made (first parent the old `HEAD`; an amend, the replaced commit's parents, the same object
  allowed after git's exit 0); `CommitCancelledBeforeRunning` is `Cancelled` (the user cancelled
  it), not `NotRun`; an amend at the press is a `LocalWrite` so it queues, runs and cancels as a
  commit does; the config file renamed `reads/config.rs`.
- **Removed**: `crates/cairn-git/src/commit_hooks.rs`, `crates/cairn-git/src/reads/hooks_path.rs`,
  `crates/cairn-model/src/commit_hooks.rs` and their pins
  (`the_hooks_path_read_writes_nothing_and_runs_nothing`,
  `the_hooks_directory_is_gits_own_answer_made_absolute`,
  `a_hook_counts_where_access_would_let_its_owner_run_it`,
  `the_skip_is_offered_exactly_where_either_hook_would_run`); `Consequence::Amend`'s subject,
  `replaces`, `force_push_warning`, `needs_force_push`, `amend_replaces`, `amend_force_push`;
  `amend.rs`'s gix `reflog` read and `subject`; `commit.rs`'s gix `utf8_messages`;
  `WriteEnding::MayHaveTakenEffect` and `Outcome::MayHaveTakenEffect`.

## 2026-10-10 — phase 12's QA fixes (packet mode)

The coordinator's adjudication confirmed 13 findings (3 dismissed: #3, #9, #14). Fixed, test
first where a bug:

- **#1** escapes stripped after the scrub could rebuild a credential (`https\x07://u:T@h`,
  `https:\x1b[0m//u:T@h`): `strip_ansi` moved into `cairn_model` beside the `Scrubber`, which now
  strips a line's terminal controls before it scrubs, and `pipes::piece_end` never cuts a piece
  inside an escape. **Deviation from phase 12's spec item 5** ("strip_ansi moves to
  shown_output.rs, its one owner"): an engineering decision, nothing a person sees changes — the
  window still strips as it draws, now idempotently — and it closes a credential path the residual
  had stated. Pins: `an_escape_inside_a_url_does_not_hide_it` (model; failed first),
  `an_escape_inside_a_url_draws_no_token` (app), `an_escape_straddling_the_piece_limit_hides_no_url`
  (pipes; fails with the piece cut at the character alone). The residual is gone from CLAUDE.md and
  `git-processes.md`.
- **#2** `WriteEnding::of`'s `command`: the runner already named its errors from scrubbed
  arguments (`cli::describe`), so the test passed as written; the arguments are now one scrubbed
  copy, `cli::scrubbed_arguments`, that the record books and every runner error names. Pins:
  `git_output_is_read_once_as_whole_scrubbed_lines_on_both_streams` (asserts `GitFailed`'s
  arguments), `arguments_are_described_as_typed_and_scrubbed`, and
  `an_ending_carries_the_engines_scrubbed_text` (asserts `command`).
- **#4 and #5**: a piece ending inside an authority removes all of it and carries, whether or not
  an `@` was seen, and a carried run filling a piece is removed and carried again; a whole line's
  authority ends at the line's end, its host kept (`From https://github.com` drawn whole). Pins:
  `a_userinfo_cut_by_pieces_is_never_drawn` and `a_url_ending_a_line_keeps_its_host` (both failed
  first), and the pipes test over `https://u:SEC@RET@host/y` at every offset across the piece limit.
- **#6** `stdouts_last_line_without_a_newline_is_handed_on_kept_and_recorded`; replacing the
  splitter's `finish` with a drop fails it.
- **#7** the hook test's docstring says its lines travel git's stderr.
- **#11** `.claude/agents/qa-checklist.md` item 12, the scrubbing residuals.
- **#12** the CLAUDE.md invariant scoped to git's diagnostic output; reads' answers stay as git
  gave them, for parity.
- **#13** the passing scaffold names `cairn_model::ScrubbedLine`.

Carried: #10 to phase 13, #15 and #16 to phase 20 (written into those docs); #8 in state.md's
teardown list. Note: `git-floor`'s `--lib diff:: reads:: ops::` run sits at zero slack — phase 12
removed three of commit's splitter tests and added two commit tests, so it is at its floor of 149
exactly and any further loss fails it.

## 2026-10-10 — phase 12 of the rebuild: git's output read once (packet mode)

Test first: `a_character_straddling_the_piece_limit_arrives_whole` written against `pipes.rs` as
it was failed ("é cut 1 bytes in read as U+FFFD"), then passed once one line type cut pieces
where a character ends. Then the rest of the phase as `state.md` lists it ("Phase 12 of the
rebuild"): one `pipes::Lines` for stdout and stderr, each line scrubbed as it is split, the tail
whole lines with a plain `older_dropped`, `ops/commit.rs` on `Invocation::lines`, and
`cairn_model::ScrubbedLines` the one form git's text crosses into cairn-app in, every
per-consumer scrub gone. C31 pinned by `a_character_straddling_the_piece_limit_arrives_whole`,
`a_urls_userinfo_never_survives_the_split`, `lines_end_at_either_terminator_across_reads`,
`git_output_is_read_once_as_whole_scrubbed_lines_on_both_streams` (the runner, both streams, a
URL across two writes, the tail and the record), `a_hooks_output_reaches_the_commit_as_whole_scrubbed_lines`
and `gits_stdout_reaches_the_commits_output_and_failure_as_lines` (a commit through real git, which
also keep `git-floor`'s `--lib diff:: reads:: ops::` count at its floor of 149 after commit's own
splitter tests moved to `process::pipes`), the model's scrub tests and compile_fail
doctests, `an_errors_shown_text_carries_no_userinfo` and the app's
`an_ending_carries_the_engines_scrubbed_text`. No measurement. Full gate green; QA is the
coordinator's.

## 2026-10-10 — the open questions answered (packet mode, before phase 12's code)

The user answered "Approve" to each of the planning agent's six recommendations, relayed by the
coordinator: (1) a confirmed amend's skip by option (a), the unspent token handed back; (2) no
"k of n selected"; (3) the amend button reads "Amend Last Commit"; (4) "Remove Stale Lock";
(5) Fork's All / User / Background tabs filed at teardown; (6) R2.1 describes
`TextDiff::inverted`, C21 gains no staged-rename row (a measurement issue at teardown), and Escape
as a literal key is an allowed convention, its sites named. Recorded in the PRD (its opening and
each requirement touched: R1.1, R2.1, R7.2, R8.1, R10.5, R10.6, R12.1, R12.4, the deviations
list, C14, C21, C25, C30, "Not done"), `docs/design/ui.md`, `state.md` ("Open for the user", all
resolved; the teardown list) and phases 13, 15, 17, 18, 19 and 20. Nothing is built for them
here.

## 2026-10-10 — the review, the design pass, the user's redesign decisions; the rebuild planned

**The review.** At 2bf8394, with the merge bar's fixes in, the user asked whether the packet had
grown by patches. Five fresh reviewers on another model read the whole packet — the commit box
and dialogs, the activity popover and chrome, Local Changes, the engine, the app and UI code —
and found that it had, from about eight design causes: confirmation built around the token, not
the person (three ways to confirm an amend); prompts that list the engine's cases; messages with
no home; capped text; invented features that breed states (the lock offer, Show All, the line
under the lists); two sources of truth for the selection; the engine's re-checks, after-the-cut
scrubbing and re-implementations of git; and app state copied rather than generalised. About 17
of 24 engine fix commits trace to three of those choices. Four git-parity bugs were found:
`MERGE_MSG`'s comment lines committed; stderr cut inside a character in `process/pipes.rs`; the
branch name checked with `--branch`, which resolves `@{-N}`; `core.logAllRefUpdates` read
through gix. All five reviewers also named what is sound (the seams, the token, Cancel focused,
the five routes, the gesture, the key policy, the narrow rosters). Reports:
`docs/research/staging-and-commit/review-*.md`.

**The design pass.** The user held the merge and ordered a design pass. Four briefs, each from
Fork's tracker and release notes, git reproduced and Cairn as built (`brief-*.md`), became one
page — eight design rules, a storyboard per flow, every choice starting on its recommendation —
which also showed that three earlier decisions rested on Fork claims Fork's own record
contradicts: files drawn together (Fork shows one file, Tracker #261, TrackerWin #786), Alt for
Stage All (Fork's key is Shift, TrackerWin #2429) and `MERGE_MSG`'s comment lines (Fork's open
bug, Tracker #180) (`redesign-page-2026-10-10.html`).

**The decisions.** The user approved rules 1-8 and answered every choice
(`redesign-decisions-2026-10-10.md`), changing B2 (Create Branch's Discard asks no second
question, as Fork), B3 ("Create and Checkout") and F7 (UTC kept everywhere). The user then
checked Fork for Windows 2.21.1 in a VM (`fork-observed-2026-10-10.md`), which changed B1 to
Fork's own command, `git checkout --no-track -b <name> <commit> --force`, and applied three
things as Fork under rule 1: the diff shows the file clicked first, Ctrl+S (not Return) stages
from the diff, and each activity row carries Fork's result line. Kept as the user's deviations:
L4 (a line selection survives an unchanged refresh), C1 (the amend dialog when pushed or no
reflog), F6 and G (the lock banner, and its removal confirmed). Last, the user decided how Create
Branch's Discard is confirmed: "Clicking the button when I've already selected discard is the
confirmation. Asking again will just annoy the user." — the dialog joins the confirmation-surface
roster, its `Consequence` fixed and generic, its re-check `HEAD`, the commit and the name, and the
prediction of what git deletes goes.

**Written down.** The evidence saved in full under `docs/research/staging-and-commit/`; the PRD
amended in place, dated, with R4.10, R6.10, R6.11, R9.6 and R14 added, the three superseded
decisions recorded and C25-C34 added; `docs/design/ui.md`'s deviation table and staging sections
and `docs/design/engine.md`'s D1 and seal rewritten; phases 12-20 planned, and the merge bar
renamed `phase-21-qa.md`. The merge bar's W1 and W3 are resolved by the redesign; F2 and F7 still
wait on the user, and five new questions are listed in `state.md` ("Open for the user").

## 2026-10-09 — phase 12, the merge bar: QA adjudicated, fixes applied (packet mode)

The merge-bar QA's two adjudications (engine half and app half, at 8002e2f) listed nine items to
fix now, one probe, four user decisions and three items to file. Every fix-now item is fixed, each
pinned; the four decisions (W1, W3, F2, F7) wait on the user and nothing is built for them.

- **C22**: `docs/design/engine.md`'s D1 names Create Branch's writes (`git checkout -q -b`, `git
  checkout -q -f -b`, beside `git branch`) and its three reads (`git check-ref-format --branch`,
  the `--numstat` pair, `git ls-files --others --exclude-standard -z`), each citing R11.3 (and R1.2
  for the count); `docs/systems/git-processes.md`'s tree lists `ops/branch.rs`, `ops/checkout.rs`,
  `ops/remove_lock.rs` and the three `reads/` files.
- **C14's carried item** (phase 09's QA item 8):
  `a_failing_hook_fails_a_commit_the_skip_commits_past_it_and_the_next_runs_it_again` — a third
  commit, hooks on, fails with the hook's words and leaves `main` where the skip put it.
- **C2's symlink half**: `a_parent_symlinked_after_the_confirmation_is_never_followed` — `d`
  replaced by a link to an outside directory whose `a` holds the confirmed bytes after the
  confirmation; the run ends `ChangedSinceConfirmed { "d/a" }`, the outside file and the link
  intact. It passed first time: no bug.
- **C19's tabs half**: `a_commits_diff_hovered_in_either_tab_draws_no_staging_action` (window
  tests) — a changed row hovered in the Changes tab, and in a file opened in place in the Commit
  tab, floats no Stage, Unstage or Discard; handing the Changes tab a gesture fails it.
- **W2**: a destructive write's prompt is copied as the window asks it (`LocalWrite::prompt`, kept on
  `local_writes::Asked`) and the popover quotes it however the write ended
  (`a_destructive_write_that_did_not_succeed_still_quotes_its_prompt`: an amend that may have
  taken effect, a discarding checkout git failed, a removal never run; fails with the asked prompt
  dropped).
- **Gate-integrity 1**: `CONFIRMED_HOLDERS` is keyed by file and type, `LocalWrite` alone excused;
  a second holder in the file, a row whose type holds no token, and a renamed type each fail
  (self-test cases in `the_confirmation_seal_matchers_catch_the_shapes_they_claim`).
- **Gate-integrity 3**: `accelerator_pin_violations` — each `pin_violations(&x)` in the table's pin
  inside an `assert!`/`assert_eq!`/`assert_ne!` (`unasserted_calls`, self-test
  `the_unasserted_call_matcher_catches_the_shapes_it_claims`), `pin_violations` reading
  `LOCAL_CHANGES_BARE_KEYS`, and that const declared once, exactly Enter, Backspace and Delete
  (`the_pin_placement_check_catches_the_shapes_it_claims` gains six shapes). Not expressible, so
  stated in CLAUDE.md and the gate-integrity reviewer's check 11: an example asserted the wrong way
  round. R4.8's pin needs no assertion check — its required lines bind the `Absent` pattern.
- **Gate-integrity 2 and 4**: the destructive-ops reviewer's checks 11 (a callback stashing its
  token) and 12 (a type holding a `LocalWrite`), linked from CLAUDE.md; `docs/qa-gate.md`'s row
  names `only_the_ops_module_changes_the_filesystem` and the bounded-helper check; `RemovedKind`
  in CLAUDE.md's list of a `Consequence`'s parts.
- **Responsiveness 1 and 2**: CLAUDE.md's UI-thread-exempt arms name `CheckBranchName`,
  `CheckoutConsequence` and `LockConsequence`; the Show All size callback's scroll-depth work is
  stated in CLAUDE.md's virtualization residuals and listed to file.
- **W4**: `ops/stage.rs`'s header says what a wrong unstage leaves once the file was edited since
  (a dangling blob), behaviour unchanged.
- **F1**: `docs/systems/history-graph.md`'s long-reflog limit rewritten to the phase-11
  measurements; the phase-10 carry ticked. **F4**: one "To file at teardown" list in state.md.
  **F5**: phase 11's QA dismissals, below.

**Probe 7** (Show All's measured height read with `peek` and ignored by `Lines::eq`): with the
estimate forced to 8 px, `show_all_makes_the_whole_confirmed_prompt_reachable` still passed — the
adjudication's sign for CONFIRMED — but the control, the measured height forced to 8 px too, fails
it ("a line at 165 drawn over the prompt ending at 296"), so the measure does reach the list: the
lines' `VirtualScrollView` lays out again after the prompt measures (instrumented: the size
callback is asked again with the measured height, the popover not rendered again). A prompt of
narrow glyphs, whose estimate runs long, sits on its measured height too, and now pins that
direction (`a_prompt_shown_whole_sits_on_its_measured_height_not_its_estimate`, failing with the
estimate alone, "git's first line 72 px below a prompt 60 px tall"). Dismissed; no code change. The
probe's edit was reverted.

**C21, measured on the tip** (`writes_check`, release, warm; each run on a fresh `--no-hardlinks`
tmpfs clone of the bench at `c999cef531e`, `maintenance.auto=false`, `gc.auto=0`, no commit-graph,
no alternates, deleted after; the bench untouched — `find .git -newer <marker>` empty and `.git`'s
mtime still 2026-10-08 20:35:34). The first run measured everything, then died of SIGBUS as it put
the clone back: the check rewrote the index in place while a worker still had it mapped. The check
now replaces each file as git does — a sibling written with the file's mode and renamed over it —
and three clean runs follow. Press to the refreshed lists drawn, median of seven:

| Verb | Bar (ms) | Run 2 | Run 3 | Run 4 |
|---|---|---|---|---|
| Stage | 93 | 68.9 | 69.0 | 69.5 |
| Unstage | 93 | 69.0 | 68.8 | 68.9 |
| Discard | 76 | 51.3 | 51.4 | 51.4 |
| Commit | 87 | 52.3 | 69.4 | 69.3 |

Every bar met. The commit's median moved by one status read between runs (52.3 against 69.4 ms,
52.5 in phase 11), still under its bar. Every frame of every phase under 16.7 ms of UI-thread work;
the slowest while a hook ran, popover closed / open:

| Hook | Run 2 | Run 3 | Run 4 |
|---|---|---|---|
| A line every 50 ms for 2 s | 1.08 / 1.84 | 0.99 / 1.79 | 1.28 / 2.10 |
| 20,000 lines, each flushed | 4.02 / 4.96 | 6.06 / 5.26 | 4.02 / 5.23 |
| Five lines of 200 KiB | 4.42 / 1.64 | 4.85 / 2.21 | 4.29 / 1.74 |

The gesture hovered over a 10,000-line diff, slowest frame 0.33-0.46 ms; dragged, 1.15-1.38 ms;
1,000 files drawn together in 26.4-28.5 ms, slowest frame 9.6-11.8 ms; Show Lost Commits' first
page 17.5-17.9 ms after the toggle. Responsiveness 3 (the closed popover still subscribing the
window's root) is within budget on these numbers and listed to file.

**Dismissed at the merge bar** (the adjudications' DISMISSED sections):

- F6, `staging_gesture.rs`'s `format!("Discard {count} {lines}…")`: the floating action's caption,
  ratified by the user on 2026-10-09; the dialog's text and button come from the `Consequence`.
- The destructive-ops notes N1 and N2 (git add drops conflict stages; no-verify only from the Git
  Error dialog): they match git or lose nothing, by the reviewer's own account.
- The gate-integrity notes (the filesystem matcher reads spellings; `Drop for OutputReceipt`'s
  responsiveness row): already stated residuals.
- The dismissal audit: no earlier dismissal failed; #33's reason has narrowed (numstat counts use
  `+=`) and still holds.
- Responsiveness 4 (`LockConsequence` queued behind a slow write): the button is drawn blocked
  while a `git` of Cairn's runs, so the press never waits behind a hook.
- C11's gap (`main.rs`'s wiring untested): the residual CLAUDE.md already states and accepts.
- C15's gaps: "failing first" is a history fact, recorded in this log; the three filters are one constructor,
  pinned by `tests/text_field.rs`.
- C17's Tab order: the criterion asks that Tab stays inside, which `tab_stays_inside_the_dialog`
  proves.
- C24's conflicted diff: drawn as a notice alone, which has no gesture, and
  `a_conflicted_path_draws_its_notice_and_asks_nothing` fails any route that drew a diff.
- Probe 7, above.

Not adjudicated: N3 (an intent-to-add file's discard prompted as "emptied"), listed to check.

**Phase 11's QA, dismissed** (recorded here at the merge bar, F5; from that phase's two
adjudications):

- qa-checklist #2, a fetch progress line's token tail in the title bar: the network lane scrubs
  every progress line with one `Scrubber` across the whole fetch, so a URL cut across two reads is
  scrubbed before it leaves the lane; the window's pass is a second.
- responsiveness #6, the root re-rendered per activity write: bounded by `ACTIVITY_ENTRIES`, only
  while the popover is open, and not new — the commit box's Git Error dialog read the same state.
- qa-checklist #5, the popover's deep `PartialEq`: required, since `shown()` builds a new `Rc` each
  render; bounded by 200 entries, the lines compared by pointer first.
- responsiveness #7, finished records scrubbed again on the UI thread: a deliberate second pass,
  once per operation's end over its own records, linear.
- TC13, no test that command lines are scrubbed: a commit's arguments are fixed literals and a
  branch's carry no `://`, so the scrub is defence in depth a test decides nothing reachable for.
- TC14, the commit-graph date route: decided by the fixture's distinct dates; the route deleted is a
  cost only, measured.

## 2026-10-09 — phase 11, the user's decisions on the popover (packet mode)

The user took every recommendation (A-N); each is applied and recorded in the PRD (R12.1, R12.4),
`docs/systems/git-processes.md` ("The activity popover") and state.md.

- **A**: the popover hangs from the status box. Freya 0.5's vendored source gives a laid-out
  place through `on_sized` (`SizedEventData::area`, global; the same mechanism `Attached` uses,
  clamped there to `Platform::root_size`), so the window keeps the box's left edge and bottom
  (`ActivityLog::set_anchor`, written only when it moved) and the panel hangs under it, an arrow
  — a square turned 45° — pointing up, both kept inside the window (`activity_popover::hung`).
- **G**: "Remove stale lock", the button unchanged, the prompt exactly the user's
  (`Consequence::prompt`, its test in the same commit); phase 01's QA item 23 settled by it.
- **H**: the consequence is read on the local lane at the press (`Request::LockConsequence` →
  `Update::LockConsequence`), the offer the ending's naming of the lock; while a `git` of Cairn's
  runs the button is drawn unpressable, saying why; a refusal at the press is said beside it. The
  locks found at open are an entry of their own. DO3+DO5 resolved by it.
- **I**: `shortcuts::keys_inert` (a prompt, a confirmation, the Git Error dialog, or the popover)
  stops the window's chords, Local Changes' intents, the history list's presses and Show Lost
  Commits; the panel is `a11y_modal`; Escape closes it.
- **K**: Show All moves the whole prompt into the lines' virtualizing view as its first row,
  sized by its own layout, so it scrolls with the pane without a `ScrollView`; Show Less cuts it
  again.
- **L**: `activity::shell_quoted`, checked by `sh` reading the line back.
- **M**: `activity::LINES_LET_GO` in place of lines the byte bound let go of.
- **N**: the name is cheaply available — the window's queued write — so a write ended before it
  started keeps it; no fallback was needed.
- Create Branch's wait reads "Waiting for the branch to be created…" (`LocalWrite::awaited`, which
  replaced `noun`, returns the whole clause).
- B, C, D, E, F, J and the minor wordings and formats are kept as built; D's local time is filed at
  teardown (state.md) as one issue with the history's date column.
- In passing: the lock files line under the lists read "while they              are there", a
  source line broken without `\`; fixed and pinned
  (`the_lock_files_line_names_every_lock_in_one_sentence`).

Decided in the building, for the user's review (listed in state.md): the lock-at-open entry's
status "found"; "Show Less"; when Show All is offered (a character estimate); the blocked note's
words; the refusal beside a still-pressable button; the arrow's shape; the panel narrowed in a
narrow window; a successful removal clearing every entry's offer.

## 2026-10-09 — phase 11 QA, the adjudicated fixes (packet mode)

The two adjudications (engine and app halves) listed 22 items to fix now. Every one is fixed
but DO3+DO5 (the lock offer computed once at the write's end, its age stale), which the
coordinator holds for the user's decisions G and H. Four are carried to file at teardown (state.md).

- **Engine**: the pushed check's graph route pinned to hand over for a tip the graph lacks (TC1,
  a `filter_map` mutation now fails both new tests); the lock re-check's time-only and
  directory arms (TC2, a comparison without the time now fails); the scrubber carries a cut at or
  inside `://` and reads a quote as part of a userinfo, every byte of a line tried as the cut
  (TC3, TC4); where kept output was cut is said by the engine — the runner's `Retained`, a
  record's `stderr_cut`, `Error::GitFailed`'s `stderr_cut` offsets, a commit's joined output —
  and the lanes scrub by it, the length guess gone (TC5); the filesystem matcher sees
  `File::options`, `File::create_buffered`, `fs::{self as f}`, braced globs and `File` renamed,
  every roster entry has a case spelled out, and `ops/` and each exception are pinned to their
  kinds of write (GI1-GI6, TC6); `LockRefusal::Unreadable` (QC4); the registry's limit stated
  (DO4); the missing tests TC8-TC12; and the reviewer files' gaps (GI8, GI9).
- **App**: the activity log counts bytes as lines go in and out, lets go of a finished entry's
  streamed output, trims a running entry's oldest lines rather than clearing it, and builds
  the selected entry's lines only for the open popover (item 1); the carry of diffs drawn together
  looks each path up by hash — 9.6 ms at 50,000 selected and 5,000 drawn, release (item 2);
  popover lines cut at `LINE_CUT_BYTES` with the diff view's marker (item 3); window_check's hook
  phases (item 4); the docs cite constants (item 5); and each operation is named in Fork's
  imperative form (`LocalWrite::name`, `Consequence::name`), Fork-settled.

**Measured** (`writes_check`, release, warm, a fresh `--no-hardlinks` tmpfs clone of the bench at
`c999cef531e`, `maintenance.auto=false`, `gc.auto=0`, no commit-graph; three runs; deleted after).
C21, press to the refreshed lists drawn, median of seven: stage 68.8 / 69.3 / 69.6 ms (bar 93),
unstage 69.1 / 69.3 / 69.0 (bar 93), discard 51.5 / 51.6 / 51.4 (bar 76), commit 52.5 / 52.5 /
52.5 (bar 87). Every frame under 16.7 ms of UI-thread work. The slowest frame while a hook ran,
across the three hooks, each with the popover closed and open:

| Hook | Popover closed (ms) | Popover open (ms) |
|---|---|---|
| A line every 50 ms for 2 s | 1.12 / 1.57 / 0.98 | 2.08 / 1.96 / 1.66 |
| 20,000 lines, each flushed (14,500-19,900 updates applied) | 4.46 / 4.33 / 5.37 | 5.40 / 5.39 / 5.09 |
| Five lines of 200 KiB | 4.17 / 4.32 / 4.15 | 1.47 / 1.67 / 1.71 |

1,000 files drawn together in 28.7 ms, the slowest frame 12.2 ms; Show Lost Commits' first page
17.5-18.2 ms after the toggle. C21's frame claim holds for these three hooks; a hook that writes
faster than one pipe read per line, or for longer, is not measured.

## 2026-10-09 — phase 11, the activity popover and the measured bar (packet mode; built, QA pending)

Built on `feature/staging-and-commit` from 4794754, in packet mode. Everything R12, R13, C2's lock
half, C20's popover half, C21 and C22 settle is built; the popover's placements and words that
neither the PRD nor Fork's evidence settle are built provisionally and held for the user (NEEDS
USER SIGN-OFF, relayed by the coordinator).

- **The popover (R12.1, R12.3)**: `cairn_ui::ActivityPopover`, opened by the title bar's status
  box — Fork's left list of operations (name, status, start, × on a running commit, amend or
  fetch) and the selected one on the right: status, start and duration, what its ending said,
  the prompt it confirmed, an amend's way back (Show Lost Commits turned on, the replaced
  commit found as a ref's press finds its row, `ref_find::find_commit`), `Remove index.lock…`,
  then `$ git ...` and its stderr. Both lists virtualized; at most 200 operations, 10,000 lines
  one, 4 MiB of lines together; session only. Each lane hands the window its own operation's
  records (`Repository::command_mark`/`commands_since`, the thread and the build order kept
  beside each record) in `Update::OperationRan`, just before the ending.
- **Scrubbing (R12.2)**: `cairn_model::Scrubber` removes every `scheme://` URL's userinfo, a URL
  cut at a line's end on both sides of the cut; the lanes scrub records, progress, messages and
  output, and the window scrubs again as it keeps them (`shown_output`) — the Git Error dialog's
  streamed and kept lines, the line under the lists and the popover's (dialog-level
  `the_git_error_draws_no_token_a_hook_printed`).
- **`Remove index.lock…` (R12.4, C2's lock half)**: `ops::remove_lock_consequence` /
  `ops::remove_index_lock` (on `DESTRUCTIVE_OPERATIONS`), `LocalWrite::RemoveLock`, offered by
  the lane only where an ending names the lock and `Repository::running_invocations` is zero; the
  re-check refuses a lock made again (new inode at the same time and size), rewritten or gone.
- **The filesystem-mutation guard (R12.5)**: `only_the_ops_module_changes_the_filesystem`,
  matcher `mutates_the_filesystem`, self-test, exceptions roster (askpass socket directory,
  the ownership probe), each row failing when unneeded; checked by adding an aliased
  `remove_file` to a render file, which fails it.
- **Carries done**: WriteOutput one update a pipe read with a 1 MiB budget the window gives back
  (`output_flow`) — and `OutputTail`'s one line kept whole past its byte bound is at most a
  runner's piece (256 KiB of stderr, 64 KiB of stdout), since the runner sends a longer line in
  pieces, so it is bounded already; a multibyte character at
  `ops/commit.rs`'s cuts no longer reads as U+FFFD; `begin_for`'s failure said in the ending; Show
  Lost Commits' tips read once and dated from the commit-graph (decision A); the pushed check
  answered from the commit-graph with a generation cutoff; files drawn together keep their
  diffs across a selection change (#4'); 50,000 paths stay off argv (pinned).
- **Re-carried, measured**: the discard count's per-path reads (15.4 ms a path on the clone:
  1,000 files 15.4 s before the dialog, cancellable) — batching them changes what a prompt counts
  for an LFS pointer, a file past the limits and under a configured `diff.algorithm` (numstat
  counts what the drawn diff does not), so it is not made here; the amend read's split (0.14 ms
  consequence, 2.6 ms staged list per status at a remote tip; 1.2 s per status only for a
  detached `HEAD` far behind with no commit-graph); #87 (two lock listings a write: 0.02 ms on
  the clone's packed refs, 3.4 ms at 10,000 loose refs) — no change.

**Measured** (release, warm, a plain `--no-hardlinks` clone of the bench at `c999cef531e` in
`/tmp` (tmpfs), no alternates, no commit-graph unless said, `maintenance.auto=false` and
`gc.auto=0` set in the clone so a commit's auto-maintenance cannot write a graph mid-run; deleted
after). C21, press to the refreshed lists drawn, median of seven after one to warm, three runs
of `writes_check`: stage 85.9 / 69.1 / 68.7 ms (bar 93; baseline git + status 42.9), unstage
85.5 / 68.8 / 68.6 (bar 93; 42.8), discard (from the confirmation's press) 68.0 / 51.3 / 51.4
(bar 76; 25.8), commit 69.1 / 52.6 / 52.4 (bar 87; 37.4) — the first run a frame slower
throughout; an earlier run, its time read one frame late, had commit at 86.2. Every bar met. The
engine's share (`cairn_write_costs`): `stage_lines` 30.0 ms, `unstage_lines` 30.2, `discard_lines`
13.7 (its consequence 13.4 before the dialog), `commit` 9.2-11.4; git's own in the same run
17.7 / 17.7 / 0.8 / 12.1 and a status read 33.8. Frames: every frame under 16.7 ms of UI-thread
work — while the writes landed, slowest 1.4 ms; a commit whose hook writes a line every 50 ms for
2 s, 132 frames, slowest 1.25 ms; the gesture hovered across a 10,000-line diff, slowest 0.5 ms,
and dragged down it past its edge, slowest 1.35 ms; 1,000 files selected and drawn together in
28.5 ms, slowest frame 12.0 ms; Show Lost Commits toggled on, its first page drawn 18.2 ms after
the press. Show Lost Commits' first page and the pushed check: `docs/systems/history-graph.md`
and `docs/systems/staging.md`.

**Bench**: see the phase's bench check in state.md; the clone was made with
`GIT_OPTIONAL_LOCKS=0`.

## 2026-10-09 — phase 10, the review of the decisions' round (packet mode)

A fresh review of 1eba929..32b64cb found it ready but for one safety item. Each fix was built
test-first.

- **Focus on open** (decision E's safety half): with the primary first on Linux, the order no
  longer kept a risky answer off the first place, so focus is now set, never left to the
  toolkit. What Freya does, read in the vendored fork (`freya-components/src/popup.rs`,
  `freya-core/src/accessibility/tree.rs`): `Popup` gives focus to nothing as it opens; a
  frame marked `a11y_modal` pulls focus onto the frame itself, not a button, only when focus
  is outside it, and keeps Tab inside it — so Tab's first stop in a modal is its first
  focusable button; and Freya's `Button` cannot take focus as a dialog opens. Before the fix a
  Return on opening the host-key prompt or the Git Error dialog did nothing at all, on Linux
  (checked against the old code: focus behind the prompt, which is not modal, or on the Git
  Error dialog's frame), so neither accepted nor skipped, but neither was on the safe answer,
  and a Tab's first stop was the risky one on Linux. Now the confirmation's
  focusable answer is shared (`cairn_ui::answer_button`): ssh's host-key question opens on
  Cancel, the Git Error dialog on Close, the confirmation on Cancel as before, and Create
  Branch and a credential field's prompt in their field; Return or Space on opening answers
  safely on both platforms (`a_risky_dialog_opens_on_its_safe_answer_on_both_platforms`,
  which failed first; dropping Close's focus fails it again).
- **The wait's words** (decision D, as the user's mockup read): each write has a plain noun
  for the line (`LocalWrite::noun`): "Waiting for staging to finish…" (lines, files or all),
  "Waiting for unstaging to finish…" (lines, files or all), "Waiting for the discard to
  finish…" (lines or files), "Waiting for the commit to finish…", "Waiting for the amend to
  finish…", "Waiting for the branch to finish…" (a branch created), "Waiting for the checkout
  to finish…" (a branch created and checked out, keeping or discarding the changes), and,
  for a write the window never saw asked, "Waiting for another write to finish…". A fetch
  runs on the network lane and never holds the name's check, so it has no line.
- **Optional, done**: Local Changes' right-click order pinned
  (`a_right_click_outside_the_selection_chooses_its_row_before_the_menu_opens`; dropping the
  choice fails it).

## 2026-10-09 — phase 10, the user's decisions on QA's six held items (packet mode)

The user decided the six items QA held, accepting the recommendations in the coordinator's
mockups (2026-10-09, relayed by the coordinator). Each was built test-first: every new test
failed on the code before it.

- **A — the Discard confirmation** (the user's decision): titled "Discard changes", its button
  "Discard Changes and Check Out", its prompt naming the changed files as the files prompt
  does and, in a sentence of its own, each folder and nested repository deleted by its path —
  "Deleted because the branch has a file there: folder d/ (4 untracked files) and repository
  vendor/lib/ with its history." — the first three named and the rest counted ("and N more");
  a discard with nothing but such deletions asks "Do you want to create branch x at abc1234 and
  check it out?". The interim words are gone. The `Consequence` already carried each path
  (`LostChange::path`). Full-literal tests:
  `a_directory_or_a_nested_repository_in_the_way_is_named_lost`. The changed files' own words
  ("1 new file deleted (3 lines)") are L8's, kept; the mockup's "1 staged (3 lines)" was read
  as an example, not a change to them.
- **B — refusals stay inside the dialog** (the user's decision): beside the buttons with ⚠, as
  built; a name's refusal never opens the Git Error dialog. git's refusal of the write itself
  (a checkout that would overwrite, a race) still opens it, as R11.3 says.
- **C — a right-click selects its row** (the user's decision, Fork for Windows): in the history
  list, unless it lands on the selected row or either row of a compared pair, which keeps the
  whole selection (`a_right_click_selects_its_row_and_keeps_a_selection_it_lands_in`). Local
  Changes' lists already chose a row outside the selection and kept a selection the press
  landed in (`each_rows_menu_offers_forks_items_and_no_discard_where_none_is_allowed`,
  `a_drag_the_menu_and_a_double_press_stage_what_is_selected`).
- **D — the wait said** (the user's decision): while the name's check waits behind a write on
  the local lane, the dialog says "Waiting for <write> to finish…" beside the buttons, naming
  the running write (or the first queued) — first in the status line's words, then, after
  the review, by a plain noun as the mockup read (above); nothing otherwise
  (`a_check_waiting_behind_a_write_is_said_beside_the_buttons`,
  `a_name_check_waiting_behind_a_write_says_which_write`).
- **E — button order per platform** (the user's decision): on Linux the primary first and
  Cancel (or Close) last, as Fork for Windows; on macOS Cancel first and the primary last, as
  Fork for macOS; `cairn_ui::button_order::ordered`, used by the confirmation, the Git Error
  dialog, Create Branch and the credential prompt, each drawing in this build's platform
  unless a test names another (`.platform(Os)`); a confirmation's focus stays on Cancel on both
  (`every_dialogs_buttons_follow_the_platforms_order`,
  `a_confirmation_starts_on_cancel_on_both_platforms`). `docs/design/ui.md` and R7.4 record it.
- **F — `@{` refused** (the user's decision): before git is asked, "A branch name can't
  contain '@{'" (`a_name_holding_at_brace_is_refused_in_cairns_words`).

The PRD (R7.4, R7.4a, R11.3), `docs/design/ui.md` and the systems docs record each. The
`git-floor` diff_engine floor rose to 201 with F's test.

## 2026-10-09 — phase 10, QA fixes (packet mode)

Five fresh reviewers and a fresh qa-confirm adjudicated phase 10 (the coordinator's
`qa10/adjudication.md`). Every FIX NOW item is done test-first: each regression test failed on the
code before its fix, and each guard fix was proven by the mutation the reviewers ran.

- **1-3 (CRITICAL), Discard's untracked losses**: the count now reads every untracked path from
  `git ls-files --others --exclude-standard -z` (`reads::untracked_paths`), whatever
  `status.showUntrackedFiles` says (3), and looks up each path's every proper prefix in the
  commit's tree: a directory where the commit holds a file is removed with every file under it
  (1, and the variant of a tracked `x` replaced by an untracked `x/`), and a nested repository
  where the commit holds anything is removed whole (2) — each a loss of its own,
  `ChangeLoss::Removed { kind: RemovedKind, files, bytes }`, compared by the re-check too.
  Tests in `crates/cairn-git/tests/diff/branch.rs`, green on the floors too:
  `untracked_files_under_a_directory_the_commit_holds_as_a_file_are_named_lost`,
  `a_nested_repository_the_commit_holds_a_file_at_is_named_lost`,
  `an_untracked_file_in_the_way_is_named_whatever_status_shows`; the model's prompt test
  `a_directory_or_a_nested_repository_in_the_way_is_named_lost`. **The words are interim,
  awaiting the user (held item A)**: "N directory/directories in the way removed (N untracked
  file(s), size)" and "N nested repository/repositories removed (N file(s), size)", in the
  discard's existing style, and the "kept" sentence counts only the untracked files not lost.
- **4**: `ops/checkout.rs` and `docs/systems/staging.md` say ignored files and ignored
  directories are overwritten or removed by any checkout, as git's own does, and not counted.
- **6 (CRITICAL)**: the chord-name module is held to the modifier matcher too, its
  `Modifiers::<CONST>` uses alone blanked (`names_a_modifier_in_code`,
  `modifier_constants_blanked`, `CHORD_NAME_CONSTANTS`); the event's `modifiers` field,
  `.ctrl()` and `NamedKey::Control` there now fail (the reviewers' mutation), self-tested.
- **7**: a literal that is exactly a bare modifier word (`Alt`, `Shift`, `Super`, `Meta`,
  `Option`, ...; `MODIFIER_WORDS`) fails outside the chord-name module, self-tested both ways.
- **8**: `scripts/git-floor.sh`'s floors raised to one under today's counts (lib 140, diff_engine
  200 with item 15's test, status 17) and a `lost_commits` run added (3): its oracle builds
  reflogs with the git in use.
- **9**: `qa-checklist` item 11 triggers on `accelerators/chord_names.rs` and carries the residual
  of a name put together there from single-key literals.
- **10**: the two guard assert messages with embedded spaces mended.
- **11**: the reflog tips are de-duplicated against the refs' through a `HashSet`.
- **12**: the long-reflog residual widened (`state.md`, `docs/systems/history-graph.md`): the
  local branches' count, the ids across every log, the repeat per ref-moving refresh, item 11;
  carried to phase 11's measurements.
- **13**: a name check superseded before its turn on the local lane starts no `git`
  (`a_name_check_superseded_before_its_turn_runs_no_git`).
- **15**: a name a branch's directory holds — `foo` beside `foo/bar`, `baz/qux` beside `baz` —
  is refused before git runs, in git's words, inline where other refusals show today (held item
  B) (`a_name_clashing_with_a_branchs_directory_is_refused_before_git_runs`).
- **17**: "Local changes:" pinned over an untracked-only tree (not drawn) and over an untracked
  file sorting before a changed one (drawn)
  (`local_changes_are_offered_over_a_change_and_never_over_untracked_files_alone`; both mutants
  fail it).
- **19, Fork-settled, built**: Fork's New Branch chord, Ctrl+Shift+B (⇧⌘B on macOS), heard
  anywhere (`Scope::Window`, as Fork's menu item is), opening Create Branch at `HEAD`'s commit
  as the last refresh read it, and doing nothing before the refs are read, on an unborn `HEAD`
  or over a dialog already up (`the_new_branch_chord_opens_create_branch_at_head`). Clash check:
  no other action of the table holds it on either platform
  (`chords_are_distinct_and_every_bare_one_is_a_function_key_or_local_changes_own`); this host's
  desktop (Hyprland) binds `SUPER + SHIFT + B` and no `CTRL + SHIFT + B`; other desktops' and
  macOS's defaults were not checked on a machine. The subtitle "Use '/' as a path separator to
  create folders" under the title; ⚠ — Fork's warning triangle, `RefGlyph::Gone` — before a
  refusal.
- **20**: the refusal is said in the buttons' row, left of them
  (`a_refusal_is_said_beside_the_buttons_behind_the_warning_glyph`).
- **21**: the derived user-visible choices are carried in `state.md` as awaiting the user.
- **22**: `state.md`'s stale `expect(dead_code)` line removed; C20's dimming is attributed
  alike in the PRD, `state.md` and here: Fork-settled on Fork's evidence, applied under the
  user's Fork-first rule.

**Dismissed** (qa-confirm's verdicts, logged with their reasons):

- **5**, the re-check run on a signal that cannot be cancelled: the local lane is serial, so
  nothing of Cairn's moves the tree between the count and the write; edits from outside are
  R1.4's stated residual.
- **18**, the reflog unit test covering entries only: the integration test
  (`crates/cairn-git/tests/lost_commits.rs`) kills the mutant.
- **23**, "Return presses, Escape cancels" not being in the evidence: it is a PRD requirement
  (R11.3), and Escape cancelling is C17's convention.

**Held for the user, not decided** (today's behaviour kept): (A) the Discard confirmation's
wording, title and button, the new losses' interim words among them; (B) a refusal inline or in
the Git Error dialog; (C) whether a right-click selects the row; (D) a "checking…" state while
the name's check waits behind a write; (E) the buttons' order on Linux; (F) names holding `@{`.

## 2026-10-09 — phase 10, the check box, its tooltip and Create Branch built (packet mode)

The user decided the check box's place and tooltip and chose Fork's full Create Branch dialog
(2026-10-09, relayed by the coordinator; evidence filed as
`docs/research/staging-and-commit/fork-create-branch-evidence.md`). Built test-first:

- **B — the check box**: right-aligned at the end of the "Graph and subject" heading's cell, laid
  over the cell so every heading stays over its column (`every_heading_sits_over_its_column`
  unchanged and green) and no row loses width; "Show Lost Commits", ticked while on, a press
  flipping it as the chord does, the list taking the keyboard back after the reopen
  (`show_lost_commits_is_a_check_box_at_the_end_of_the_first_headings_cell`,
  `the_show_lost_commits_box_flips_the_toggle_as_its_chord_does`). The Amend toggle's check box is
  now `cairn_ui::check_box`, shared three ways.
- **B — the tooltip**: the chord, rendered from the accelerator table by
  `accelerators::chord_name` in Fork's own spelling (Linux `Ctrl+Shift+.`, macOS `⌘⇧.`,
  `keyboard-shortcuts-*.md`). The modifier invariant is amended (root `CLAUDE.md`): one rostered
  file (`CHORD_NAMES`: `crates/cairn-ui/src/accelerators/chord_names.rs`) may spell chords, every
  literal there one held key's name alone (`MODIFIER_NAMES`, matcher `hand_typed_chords`), no
  element built there, the row failing once unneeded; every other render file still fails on a
  chord spelled (`the_chord_name_roster_and_its_matcher_catch_the_shapes_they_claim`).
- **Create Branch**, Fork's anatomy (PRD R11.3 amended, dated): `New Branch…` on every commit
  row, lost ones too (stash rows none); the commit read only; the name asked of the engine as it
  is typed (`git check-ref-format --branch`, a read in `reads/`, then the ref by gix) and refused
  inline — Fork's "Branch <name> already exists" or git's reason; "Check out after create" sticky
  for the session (decision 1; across restarts, issue #89); "Local changes:" over staged,
  unstaged or conflicted changes, each opening on "Don't change" — `git checkout -q -b <name>
  <oid> --`, git's refusal in the Git Error dialog with the name kept (Fork RN Win 1.82) — or
  "Discard" (decision 3): `Consequence::CheckoutDiscarding` (every staged and unstaged change's
  lines as git's numstat counts them, every untracked file the commit's tree overwrites by its
  size, untracked files kept said), counted on the local lane, confirmed in the confirmation
  dialog, re-checked before `git checkout -q -f -b` runs, refused before any prompt during an
  operation in progress, over a conflict or a submodule's change; on `DESTRUCTIVE_OPERATIONS`; R1.5
  and R3.6 amended for the exception; the packet qa-checklist's "nothing discards a staged change"
  names it. No "Stash and reapply" until 5b (decision 2: PRD product rules, `docs/design/ui.md`,
  5b's brief).
- **Engineering choices, no user-visible effect**: the name is checked on the local lane in a lane
  of its own (`QueryLane::BranchName`) and the discard counted in another (`CheckoutCount`), so
  Local Changes' `StopCounting` never ends it; a stale answer for another text is never trusted
  (`new_branch_opens_the_dialog_and_creates_only_a_name_the_engine_said_is_free`); the discard
  refuses an operation in progress outright rather than let `checkout -f` leave its state behind.
- **Derived, not separately decided — for the user to veto**: the discard confirmation's title is
  Local Changes' "Discard changes" and its button "Discard Changes and Check Out"; its prompt
  follows L8's form ("Do you want to create branch 'x' at 1a2b3c4, check it out and discard the
  changes in …? … You can't undo this action."); an engine refusal of Discard is said in the
  dialog beside the buttons, where a name's refusal goes; buttons in Cairn's existing order,
  Cancel then the primary; a right-click on a row opens the menu without selecting the row.
- **Not built (not decided)**: Fork's New Branch chord (⇧⌘B / Ctrl+Shift+B); Fork's subtitle and
  warning glyph.

Issue filed: <https://github.com/alexparlett/cairn/issues/89> (the settings store).

## 2026-10-09 — phase 10, the user's decisions applied (packet mode; stopped again on B)

The user decided phase 10's open choices (2026-10-09, relayed by the coordinator):

- **B — the Show Lost Commits control**: a checkbox at the right end of the history's
  column-heading strip ("Graph and subject | Author | Commit | Date (UTC)"), labelled "Show Lost
  Commits", ticked while on, its tooltip naming the chord from the accelerator table (never a
  literal), a click flipping it exactly as the chord does. This is R11.1's "a control in the
  history's toolbar area"; no PRD change. Fork has only a View-menu item, and Cairn has no menu
  bar. **Not built: stopped with NEEDS USER SIGN-OFF on two points the decision leaves open.**
  (1) The strip's headings sit over their columns, pinned by `every_heading_sits_over_its_column`.
  A control after "Date (UTC)" either pushes the Author, Commit and Date headings left of their
  columns or needs a matching empty column in every row. (2) No text a person reads names a chord
  today: the accelerator table holds data and resolution only, and the modifier guard refuses a
  chord spelled in any render file. A tooltip naming the chord needs a renderer of chord names for
  people, and the invariant and its twins amended to admit it.
- **A — the long reflog's first page**: accepted as a stated residual
  (`docs/systems/history-graph.md`, known limits, with the measurements), and carried to phase 11:
  read each tip id once (~41 ms) and use commit-graph dates where a graph is present, measured on
  the bench.
- **D**: the toggle is off at launch and kept for the session only (Fork's persistence is
  unrecorded). Already as built.
- **Create Branch view: on hold.** The user chose Fork's full Create Branch dialog. Its checkout,
  stash and discard semantics are being researched for mockups. Nothing built; `ops::create_branch`
  and `LocalWrite::CreateBranch` stay.

Fork-settled, applied the same day:

- **A lost row's text alone is dimmed**: subject, author, short id and date in the theme's
  `text_placeholder` (about 50% grey), the graph's lanes, edges and node and the chips in their own
  colours (Fork: VSHOT Tracker #351 GIF, 2018; USHOT Tracker #351 screenshot). This replaces the
  whole-row opacity of 0.5 (`LOST_OPACITY` removed). Test-first:
  `a_lost_rows_text_alone_is_dimmed` (`crates/cairn-ui/tests/commit_row.rs`) was red on the
  opacity build.
- **Focus through the toggle**: the history list takes the keyboard again as the reopened walk's
  first page draws it, so the chord works again with no click. The window test
  `the_show_lost_commits_chord_reopens_the_history_with_the_toggle_flipped` now presses the chord
  twice with no click between. It passed as soon as the click was removed: the list's
  `a11y_auto_focus` already takes focus when it is drawn again. Earlier the test pressed the
  chord before the page arrived, while the list was not drawn, which looked like lost focus. It
  now pins the behaviour, and the moment before the page arrives is stated as a residual.

## 2026-10-09 — phase 10, Show Lost Commits (packet mode; stopped for the user's sign-off)

Built on `feature/staging-and-commit` from daa16ec, in packet mode. Everything R11 and C20 settle
is built; the two views they do not settle — the history's toolbar control (R11.1) and `Create
Branch Here…`'s menu and name entry (R11.3) — wait on the user (NEEDS USER SIGN-OFF, relayed by
the coordinator), with everything under them built.

- **The walk (R11.1, R11.2, as amended by decision D)**: `HistoryRequest::with_lost_commits` seeds
  from the old and the new id of every entry of `HEAD`'s and each local branch's reflog. Each log
  is read whole through gix's store into a buffer and parsed by git's own `show_one_reflog_ent`
  rules (`history/reflogs.rs`) — never gix's newest-first reader (stops at a line over 4 KiB) nor
  gix's line parser (refuses lines git reads). The null id, a missing object and a non-commit are
  skipped, as `handle_one_reflog_commit` skips them.
- **The marking**: decided as the walk goes (`history/reach.rs`), never a walk of its own before the
  first page. Engineering decision: a parent dated after its reached child can come off the walk
  first, through a lost commit, and be taken for lost; it is corrected when the child comes off —
  a row not yet carried reads its state when carried, a row carried already is named reached by
  the next page (`RowsPage::reached`) and `History::append` clears its bit. git's own `rev-list
  --not` gives the same answer on that fixture (`a_parent_dated_after_its_reached_child_is_not_drawn_as_lost`).
  The tracking stops once no reflog tip is left unreached and none is lost.
- **C20's Show Lost Commits half**: `crates/cairn-git/tests/lost_commits.rs`, against `git
  rev-list <ids from the files> --not --branches --remotes --tags HEAD` and git's own `--reflog`
  reading — the two agree, so gix's reading never disagreed with git's on any fixture (the stopping
  rule). Held and cold, pages 1/2/5/64, windows 1/3/1,024; on git 2.56.0, 2.30.9 and 2.32.7.
- **The amend's reflog and the walk are one entry** (state.md's carry from phase 01 item 13 and
  phase 05): `whether_the_reflog_is_written_is_what_git_then_does` holds every arm to Show Lost
  Commits — drawn when git logged the amend or a ref reaches the replaced commit (a detached amend
  leaves `main` on it), dimmed exactly when none does.
- **`Create Branch Here…`'s engine and lane (R11.3)**: `ops::create_branch` (`git branch --
  <name> <id>`), its argv pinned against the recording stub, its effect and git's refusals against
  real git on every floor; `LocalWrite::CreateBranch` reads everything again however it ends.
- **The toggle (R11.4, R7.3)**: `View::show_lost`, off as the window opens and kept for the session
  (engineering default: nothing in Cairn persists past the window); the chord heard on the history
  list (`HistoryList::on_action`), a reopen like any other; a refresh's reopen walks as the toggle
  stands. A lost row was first drawn at half opacity; replaced the same day by Fork's dimming
  (below).

**Measured** (release, warm, median of seven; a plain `--no-hardlinks` clone of the bench at
`c999cef531e` in `/tmp` (tmpfs), no alternates, no commit-graph; deleted after): the first page of
64 rows from every ref, snapshot read — the clone's own reflog plus an amend and a reset-away commit
made in it: 7.24 ms off / 7.49 ms on (3 rows lost), and on a later run 2.15 / 2.37 ms (the machine's
baseline moved between runs; each pair is back to back). A synthetic `HEAD` log of 1,000 entries
naming commits spread across the history: 2.72 off / 7.95 ms on; of 10,000: 2.14 off / 49.8 ms on —
8.4 ms reading the logs and looking up each id, 39 ms the walk's open reading each tip's date. git's
own `rev-list --max-count=64 --reflog --branches --remotes --tags HEAD` there: 57-80 ms (3-5 ms
without `--reflog`). The whole walk (345,449 rows) with that log: 2,184 ms on / 2,079 ms off.
Against the stopping rule's bar (twice refs-and-status's 7.27 ms): within it on the bench and with
1,000 entries; past it only with 10,000 — raised with the user, who accepted it as a residual
(decision A, below).

**Bench**: `find ~/Development/bench/rust/.git -newer <marker>` (marker made at
2026-10-09T14:20:12+01:00, before the clone) printed nothing before and after, and `-newermt
2026-10-09T14:20:00` nothing; `.git`'s own mtime stayed 2026-10-08 20:35:34. The clone was made with
`GIT_OPTIONAL_LOCKS=0`.

## 2026-10-09 — phase 09 closed: the user's decisions on the four open items

The user decided the four items still open from phase 09 (2026-10-09, relayed by the
coordinator), each kept as built. Fork's evidence, gathered for the decisions, is a new record:
`docs/research/staging-and-commit/fork-merge-and-amend-evidence.md`, with two open questions the
owner can check on Fork.

1. **The subject is required** (Fork: Tracker #1490; VENDOR TrackerWin #637). PRD R10.1 amended
   with a dated note.
2. **A merge in progress commits with nothing staged, concluding the merge** (Fork: Tracker #90,
   fixed in 1.0.57, October 2017). PRD R10.1's "disabled while nothing is staged" amended with
   the merge exception and a dated note.
3. **`MERGE_MSG` fills an empty draft once per merge; a cleared message stays empty** (Fork does
   not document a cleared message; the vendor's rule in Tracker #61 — an auto-filled message is
   replaced only "until you edited the message" — points this way, inferred). PRD R10.8 amended
   with a dated note.
4. **Amend is still offered when amend's staged list cannot be read**, the box saying the lists
   show what is staged against `HEAD` (Fork: no evidence; Fork would likely lazy-fetch, which
   Cairn's reads never do). Recorded in PRD R10.3 and `docs/systems/local-changes.md`.

`docs/design/ui.md` "The commit box" states all four. No code changed: each is as built at
18fea48, whose full gate is green. Phase 09 is done.

## 2026-10-09 — phase 09 QA, adjudicated and fixed; the user's decision on the Amend button

The coordinator's QA adjudication and the user's decision (2026-10-09, relayed by the
coordinator). The coordinator withdrew its phase 09 instruction that the commit box never build a
token and `CONFIRMATION_SURFACES` stay one row — the coordinator's own, contradicting PRD R1.1 —
so the box follows R1.1 as written: it is the second confirmation surface.

**The user's decision (2026-10-09): the Amend button works Fork's way**, exactly as R10.6 and
`docs/design/ui.md` "The commit box" say (no PRD change): ticking Amend fills `HEAD`'s message,
Staged lists `HEAD`'s files, the button reads `Amend <short>` above "Replaces …", and that visible
line is the confirmation's prompt; clicking the button amends at once for a commit no remote has,
and Ctrl+Enter / ⌘Return from either field does exactly what clicking does; a commit a remote has
asks the confirmation dialog first, by click or chord; a hook that fails an amend is skipped by one
press, which amends at once without hooks under the line confirmed, with no second dialog; and the
engine still refuses an amend whose `HEAD` moved or was pushed since.

Built, each test-first (the behaviour tests red on 1f44edc):

- **The confirmation surface (R1.1)**: `crates/cairn-ui/src/commit_box.rs` joins
  `CONFIRMATION_SURFACES` (the guard's doc rewritten). `AmendButton` is one component drawing the
  button and the line it confirms; it builds the token from the consequence it draws, once per
  consequence (`confirm_in_place`, a serial kept in the box), and refuses a consequence a remote
  has. The commit chord in either field calls the same `confirm_in_place`. `ConfirmButton` is gone
  from `confirm_dialog.rs`. Tests: `the_commit_chord_amends_an_unpublished_commit_as_the_button_does`
  (red before: the chord opened the dialog), `the_amend_button_refuses_a_pushed_consequence_and_builds_nothing_unready`.
- **The skip of a failed amend — the mechanism**: `AskedCommit` keeps the consequence its token
  was built from (`confirmed_with`, an `Rc` of the token's own `consequence()`), and the Git Error
  dialog draws the commit box's `AmendSkip` (`GitErrorDialog::skip_amend`) with that
  consequence's prompt — the line the person confirmed, a remote's force push before it where there
  was one — whose one press builds the skipped amend's token from that same consequence and asks
  `LocalWrite::Amend { skip_hooks: true }` at once. The token is built in `commit_box.rs`, a rostered
  surface; `git_error_dialog.rs` builds none and stays off the roster (it holds the callback as
  `EventHandler<Confirmed>`, a `TOKEN_CALLBACKS` spelling). Guard change: the one roster row added.
  Tests: `a_failed_amends_skip_amends_at_once_under_the_line_confirmed` (red before: the skip
  reopened the dialog), `a_failed_amends_skip_confirms_the_prompt_it_draws_once`.
- **QA 1**: a commit made clears the draft only where it still holds the message the commit took
  (`a_draft_typed_while_a_commit_runs_outlives_its_ending`, red before).
- **QA 4, 5, 6, 7** (coverage): the skip test above; `the_commit_chord_asks_nothing_while_the_box_is_not_ready`;
  `a_newer_amend_read_names_its_own_head`; `a_cancel_while_the_commit_is_queued_asks_nothing`.
- **QA 9**: every write asked bumps the amending lane, so a stage never waits behind a stale amend
  read (`a_write_supersedes_the_amend_read_and_nothing_else`); the read split carried to phase 11.
- **QA 10**: root `CLAUDE.md`'s "Off the UI thread but in its way" names the box's reads per
  refresh and the amend's read per status, and `submit`'s Write arm names its bump.
- **QA 12**: the skip is offered for an amend only when the consequence it was confirmed with is
  held (always, as asked), so it never offers what it cannot do.
- **QA 13, 14**: `local-changes.md` says the skip is offered for any git failure while a hook
  exists, and the activity line is worded as built.

Withdrawn from the items batched for the user (settled by the decision): item 1 (where the
amend's token is built) and item 2 (no chord confirms an amend). Still open with the user, as
built and unchanged: amend offered when amend's staged list cannot be read; the subject required;
a merge commits with nothing staged; `MERGE_MSG` fills an empty draft once per merge.

Carried (state.md): QA 11 — the Git Error dialog's lines scrubbed (phase 11's step 2), the read
split and its measurements, one `WriteOutput` update per line and one oversized line kept whole;
QA 8 — a real-git fail, skip and next-commit test on the merge bar's C14 checklist (phase 12).

## 2026-10-09 — phase 09, the commit box (packet mode)

Built on `feature/staging-and-commit`; QA is the coordinator's. Commits 4f25cf4 (model: an
amend's two texts apart, `needs_force_push`, `amended`; `LocalChanges::amending`,
`StagedAgainst`), a4ca81d (git: `WorkingTreeDiff::Amending`, `Repository::amend_parent`),
2c49c61 (ui: `CommitBox`, `GitErrorDialog`, `ConfirmButton`, `text_field_recalling`,
`accelerators::recall_step`), a7f5734 (app: the box's state and pane, its reads and an amend's
on the local lane, `WriteEnding::Failed`'s `command` and `output`, amend's lists drawn, the
window's tests) and the docs commit after them.

- **The box (R10.1, R10.2, R10.7)**: under the diff; the draft two `State<String>`s kept by
  `LocalChangesView::commit` for the window's life, bound to fields built by the one key policy
  (`text_field_in` / `text_field_recalling`, `FieldScope::CommitBox`); Fork's counter, the ruler at
  72, `Commit N Files`; Recent Commit Messages from the `≡` menu and a bare ↑/↓ in an empty or
  recalled subject. No stopping rule: the linked `Input` keeps the draft across Amend — the value
  is the window's `State`, and a value written underneath it syncs into the editor
  (`input.rs`, the `committed_text` check) — and Fork's thresholds are a label beside the field.
- **Amend (R10.3, R10.6)**: ticking it sets the draft aside and asks `Request::Amending` over the
  status drawn; the answer (`Update::Amending`) carries the consequence, `HEAD`'s message and
  amend's lists, drawn as a status's are; each status arriving while ticked asks again; unticking
  restores the draft exactly and asks `StopAmending`. Staged's paths diff against `HEAD`'s parent
  and unstage back to it (`UnstageTarget::Commit`/`Nothing`). Amend is disabled on an unborn
  branch and while an operation is in progress.
- **Running and failing (R10.4, R10.5)**: `Committing (waiting)…`, then `Committing…` with its
  elapsed time and a Cancel for the running commit only; a git failure opens the Git Error dialog
  over the streamed output (ANSI stripped, bounded, virtualized), the skip only where a hook
  exists and never twice; `Request::CancelWrite`'s and `LocalWrites::running`'s `expect(dead_code)`
  are gone, and so is `LocalWrite`'s.
- **An operation in progress (R10.8)**: `MERGE_MSG` fills an empty draft once per merge, as git
  wrote it; a rebase, `git am`, a cherry-pick or a revert disables the box and names it.
- **Phase 08's #7**: `a_chunk_drawn_at_context_ten_stages_exactly_as_drawn` (real git, through the
  lane). Local Changes' window tests run 860 px high: the box takes some 150 px under the diff, and
  `several_paths_selected_draw_their_diffs_together` needs the second file in view.
- Gate: `scripts/gate.sh --fast` green after each step; the full gate before the docs commit.

Decisions, and items batched for the user's ratification (none is a stopping rule):

1. **Withdrawn — settled by the user's decision of 2026-10-09 (see the QA entry above).** Was:
   **For the user's ratification — where the amend's token is built.** The carry (phase 06's QA:
   "the commit box hands the window a `Confirming`, never calls `by_user`; `CONFIRMATION_SURFACES`
   stays one row") and L12/R10.6 (an unpublished amend confirmed by its own button, no dialog) are
   both kept: the amend button drawn in place is `cairn_ui::ConfirmButton`, which lives in
   `confirm_dialog.rs` beside the dialog and shares its one `token` function, so the one roster
   row covers both and the commit box files never name `by_user`; a published amend's press hands
   the window a `Confirming` (the dialog). Alternative: roster `commit_box.rs` as a second surface
   (R1's original wording). Recommendation: ratify — one file builds every token. PRD R1.2/L12's
   "the commit box is the second confirmation surface" wording is the user's to amend.
2. **Withdrawn — settled by the user's decision of 2026-10-09: the chord amends as the button
   does.** Was: **For the user's ratification — no chord confirms an amend.** The commit chord heard while
   amending, and an amend's hook-failure skip, open the confirmation dialog rather than amending
   (a token is built only by a press on a confirmation surface). Alternative: the chord amends in
   place as the button does. Recommendation: keep.
3. **For the user's ratification — Fork's rules the PRD does not state**: the subject is required
   (Fork, Tracker #1490); a merge in progress commits with nothing staged; `MERGE_MSG` fills an
   empty draft once per merge (a draft emptied by hand is not filled again).
4. **For the user's ratification — amend's staged list unread** (phase 05's QA item 6): the box
   says why, the lists stay the status's against `HEAD`, and the amend is still offered. Alternative:
   disable the amend. Recommendation: keep (the list is the box's to show, not the amend's to need).
5. Decision — the amend button waits while a newer amend read is on its way (each stage while
   ticked re-reads), so a press confirms only the latest consequence; the old one stays drawn.
6. Decision — the Git Error dialog opens for a failure git reported (`Failed` with its command);
   a refusal, a stale amend, a cancel or a commit not run is said under the lists, as before.
7. Not done (optional in the carry): skipping the amend re-check's walk when the tips it read have
   not moved — re-carried to phase 11 with the pushed check's measurement.

## 2026-10-09 — phase 08 QA, adjudicated and fixed; the user's decisions applied

Four fresh reviewers and a fresh qa-confirm adjudicated phase 08; the user decided the batched
items (2026-10-09, relayed by the coordinator). Commits after b442dc6 below.

Fixed:

- **1 (critical) — a stale line selection applied to a re-read diff.** The files drawn together
  kept one number per ask, so a selection made over a re-ask's old diffs named the new diffs'
  lines once their page arrived; and an act built before a redraw trusted the number it was
  drawn under. Now the files drawn together are renumbered whenever a page replaces a diff that
  was drawn, every `GestureAct` carries its number, and `on_gesture` refuses one no longer drawn
  — single path and stacked alike. Test-first:
  `a_selection_over_files_drawn_together_is_nothing_once_a_page_replaces_its_diff` failed on the
  old code (a `DiscardLinesConsequence` sent against the re-read diff), and
  `an_act_made_under_an_answer_no_longer_drawn_asks_nothing` fails with the check removed.
- **2** — `ShownDiff` holds `Arc<FileDiff>` (`shared_diff`; `the_answer_drawn_is_shared_not_copied`),
  and the lines writes and the lines discard carry the shared answer: no copy per action.
- **8** — a release and a press read the drag in place, copying three fields, never the phase.
- **11** — `part_of_a_new_files_lines_is_discarded_as_lines`. **12** —
  `after_an_action_a_moved_pointer_brings_the_actions_back`.
- **13 (probe)** — flipping the budget's `>=` to `>` survived the real-git test; it gained a path
  costing exactly the budget, which now catches the flip. **14 (probe)** — the real-git test read
  untracked paths only; `paths_drawn_together_are_each_read_as_their_own_side` reads an unstaged,
  a staged and an untracked path through the lane's own stage and commit.
- **#3 residual** written into the root `CLAUDE.md`'s virtualization obligations.

The user's decisions (2026-10-09):

- **5** — with no lines selected, the chords over files drawn together act only on the files
  read and drawn (`together_read_paths`;
  `the_chords_over_files_drawn_together_take_only_the_files_drawn`); and a discard of several
  files names them — the first three and how many more — in `Consequence::prompt`, for every
  caller (the lists' route too), the token's text the dialog's
  (`a_discard_of_several_files_names_the_first_three_and_counts_the_rest`; the engine's and the
  lane's literal prompts updated).
- **6** — the floating discard reads `Discard 2 Lines…`; the dialog's button stays `Discard 2
  Lines`.
- **Ratified** — the line budget for files drawn together; no Load Diff, mode row or previous and
  next change when drawn together; actions pinned at the list's top; a press without a drag or
  Escape clears a selection and focus loss cancels a drag. Recorded as PRD product rules and
  `ui.md` "What changes, and why" rows. **4'** (keep each file's answer across selection changes)
  carried to phase 11; the flash documented as interim.

Dismissed:

- **9** — the floating count and the dialog's count differ: both are `Selection::len()` over the
  same `Selection` (`staging_gesture.rs`, `consequence.rs`, `discard.rs`); a mode change is counted
  apart, and git's end-of-file marker is no line number.
- **16** — a `refresh_tests` flake: 5/5 alone and 3/3 with the whole `worker::` module, unmutated;
  the failure was a `WAIT` deadline under the mutation run's load.

Carried: see `state.md` (phase 11: #3 and #4'; phase 09: #7; phase 12, for the user: #15).

## 2026-10-09 — phase 08, the diff's staging gesture (packet mode)

Built on `feature/staging-and-commit`; QA is the coordinator's. Commits 3bfb9e9 (model: which
drawn chunk a row is in and which lines a drag selects, `row_selection.rs`), dc8a682 (ui: the
gesture layer, `LineDrag`, `ModeRow`, and `StackedDiff` for files drawn together), 59fd0ef
(app: the gesture's acts, the lines' discard consequence on the local lane, the chords narrowed,
and the paths selected drawn together through `Request::Together`), and the docs commit after.

- **No stopping rule met.** (1) Fork's gesture is built on the linked Freya alone (`caa46f8`,
  each API read in the vendored fork): global pointer listeners (`on_global_pointer_move`,
  `_down`, `_press`) on the view's root, `PointerEventData::global_location`,
  `Position::new_absolute` for the layer, `Interactive::No` for the outline and tint, and
  `async-io`'s timer through phase 06's `EdgeScroll` — no second dependency. One finding: a
  node's layer is its parent's plus one (`freya-core/src/data.rs`), so the rows, built deep in
  the virtualising view, stood above a layer added at the view's root and took its presses;
  the layer is lifted by `GESTURE_LAYER` (64), well under `Layer::Overlay`. (2) A chunk drawn at
  a non-default context maps to exactly the exact changes the drawn hunk groups — Fork's chunk
  is the hunk drawn at the current context (Finding 23: the active chunk is outlined with its
  header row; Finding 14: in entire-file mode the buttons apply to the whole file, the file
  being one chunk), so the outline is the selection and no second reading arises
  (`a_chunk_at_context_ten_takes_every_change_it_draws_and_no_other`, C19's QA risk 1).
- **Outside the recycled rows** (R9.1): one layer over the list — at most an outline, a tint and
  three actions — read from the rows' numbers and the scroll, re-rendered on pointer moves and
  scrolls alone. The selection is read from the layout when the drag ends (`selection_in`,
  costing the changed lines selected) and never copied per frame; an action copies it once.
- **A selection belongs to the answer drawn**: the view is handed a number for it
  (`DiffState::working_drawn`, kept per working-tree answer; `together_drawn`, per ask), and a
  `LineDrag` made under another number is nothing — so a refresh or a write's re-read never
  leaves a selection over other lines.
- **After an action** the actions hide until the diff is drawn again or the pointer moves (Fork's
  Tracker #480 and its fix), so a second press cannot act on rows already taken (QA risk 3).
- **The user's decision on phase 07's QA item 4(e) — several paths drawn together — is built**:
  `Request::Together` reads each selected path's working-tree diff on the diff thread, in the
  lists' order, a page at a time under the ask's number; `cairn_ui::StackedDiff` draws them on
  the Commit tab's `Expansion`, the gesture over each file's rows (a drag kept to its file); the
  path chosen stays chosen, unasked, meanwhile. PRD R8.1's note, `local-changes.md` and the root
  `CLAUDE.md` amended; `ui.md` already described the end state.
- **Every line of a new file** (phase 03's `WholeFileOnly`, phase 07's carry) is discarded as the
  file: the view checks it as the engine does and asks `Request::DiscardConsequence` for the
  path, whose count already takes its cancel; a lines discard asks
  `Request::DiscardLinesConsequence` (new, the discard-count lane), read only while still the
  newest — `discard_lines_consequence` reads one path and takes no cancel.
- **The mode row** (R9.4) selects through `Selection::select_mode` (phase 02's carry); a
  mode-only discard is allowed and named by the engine (phase 03's carry).
- **A staged rename's source row** (phase 02's QA item 8, the lines half): it draws the rename's
  staged diff, so its lines unstage at the new path
  (`a_renames_source_row_unstages_its_lines_at_the_new_path`).
- **Keys and dialogs** (phases 06 and 07): the diff's chords stay focused-only, act on a drag's
  lines when there is one, and nothing acts while a confirmation or a prompt is open
  (`nothing_acts_on_lines_while_a_confirmation_is_open`); every lines discard goes through
  `ConfirmDialog` with the engine's `Consequence::DiscardLines`, which carries its selection and
  patch.
- Test helpers that press list rows now look only in the lists' column (`in_lists`), since the
  files drawn together draw their paths too.
- `LocalWrite`'s `expect(dead_code)` names phase 09 alone.

Items for the user's ratification (none was a stopping rule; each is built as stated):

1. **The files drawn together are read under Expand All's line budget** (50,000 lines, a path
   costing one and both its sides' lines); the paths past it are not drawn, each saying to
   choose it alone. Fork's own bound is not recorded. Alternative: no bound (a 50,000-path
   selection would read every diff). Recommendation: keep.
2. **Drawn together, a file offers no Load Diff and no mode row, and previous and next change
   are inert**; each is reached by choosing the file alone. Alternative: build them over the
   stacked view (Load Diff per file, the mode row as a row of the list, stepping across files).
   Recommendation: keep for this packet; file an issue if wanted.
3. **The floating actions stand at the list's top while the chunk's top is scrolled above it**
   (Fork places them at the chunk's top right), so a tall chunk's actions stay in reach; a
   selection's actions stand at its top.
4. **A press without a drag lets a selection go, and so does Escape**; losing focus mid-drag
   lets the drag go (as the lists' drag does) rather than selecting to where it was.
5. **With nothing selected, the chords over files drawn together act on every path drawn**
   (whole files), as the lists' chords act on the selection.
6. **The selection's captions are R9.2's words** — `Stage 2 Lines`, `Unstage 1 Line`, `Discard 2
   Lines` — so the discard's has no ellipsis where the chunk's `Discard Changes…` does.

Carried forward: see `state.md`.

## 2026-10-09 — phase 07 QA, adjudicated and fixed; the user's decisions applied

Four fresh reviewers and a fresh qa-confirm adjudicated phase 07; the user decided the open
items (2026-10-09, relayed by the coordinator). Commits b49d4b4, a918810, be847b8, 4c1c891,
01942eb and this one.

Fixed:

1. **(CRITICAL) A discard's count held the local lane and the close, uncancellable.**
   `ops::discard_files_consequence` now takes a `cancel`, polled before each path and handed to
   each path's `git diff-files` read (which it ends); a cancelled count is
   `Error::ConsequenceCancelled` and answers nothing. The count is numbered in a lane of its own
   (`QueryLane::DiscardCount`): a newer ask, `Request::StopCounting` (asked as Local Changes
   unmounts, `use_drop`) and a close end it. Each path is counted once by a set rather than a
   quadratic scan. Tests: `a_cancelled_count_stops_between_paths_and_runs_no_further_read`,
   `a_path_named_twice_is_counted_once` (ops, same commit),
   `a_newer_ask_or_a_stop_ends_a_discards_count_and_its_read` (lane: the superseded count's held
   read ended), `leaving_local_changes_ends_a_discards_count`; root `CLAUDE.md`'s residuals say
   the count is per path, holds the lane, and what ends it.
2. 50,000 paths ALL selected, with `.held` set, still build one viewport
   (`a_status_of_50000_paths_all_selected_builds_one_viewport`).
3. Root `CLAUDE.md`'s cost of a large selection corrected: Shift+↑/↓ re-spans on every key repeat,
   a ⌘/Ctrl-press clones twice and inserts; 5-15 ms at 50,000, per press, never per frame.
5. **A refresh left the selection holding gone paths.** The follow now moves the selection with
   the path it chooses (`Follow::ChooseFirst`) or lets go (`Follow::LetGo`); a toggle that empties
   the selection keeps it its list's, so nothing is acted on (`acted_rows` falls back to the path
   chosen only with no selection made there). `a_refresh_or_an_emptying_toggle_leaves_nothing_selected_unseen`
   failed first on both halves.
6. **(CRITICAL) The behind-the-modal test could not fail.** Rewritten with the recording
   submitter, driving every intent, `choose` and `on_the_diff` behind the open confirmation;
   checked against M7 (`dialog_open` → `false && ..`): it now fails, five writes asked.
7. **R8.3's call site.** `the_selection_moves_to_the_row_that_takes_the_acted_rows_place`: c of
   a, b, c, d leaves d; cx of the filter's cx, dx, ex leaves dx. Checked against M6
   (`nearest_remaining(len, 0 * first, ..)`): it now fails (a.rs).
8. The drag's other ends: `a_press_heard_mid_drag_or_focus_lost_ends_the_drag_without_a_drop`.
   **It found a gap**: a press on the filter field after a lost release did not end the drag —
   Freya's `Input` cancels the global pointer-down (and its own release) as it takes a press —
   so the next release over the other list dropped. The lists' root now also hears the
   platform's mouse-down, which fires before any pointer-down handler and which a field does
   not cancel. Residual, stated in `local-changes.md`: a press on another view's text field (the
   sidebar's filter) after a lost release is still not heard.
9. The release-outside case now crosses Staged before it is released over the filter.
11. **The user's decision: Stage All / Unstage All take the rows a filter shows** (every row
   with none on): `LocalWrite::StageAll`/`UnstageAll` carry `shown: Option<Vec<u32>>`, the
   filter's indices, and the lane gathers those rows; nothing is asked while the filter's rows
   are on their way. `stage_all_and_unstage_all_take_the_rows_the_filter_shows` failed first (a
   hidden row and a hidden conflicted row were staged); lane unit test
   `an_all_gathers_the_rows_the_filter_shows_or_every_row`. PRD R8.2 noted.

The user's decisions recorded (2026-10-09, the user's):

- 4(a) R8.3's rule for a selection with gaps (the row in the first acted row's place, else the
  nearest above) RATIFIED — PRD R8.3 note, `local-changes.md`, `docs/design/ui.md`.
- 4(b) a double press acts on its own row RATIFIED — C18 amended, PRD R8.2 note.
- 4(c) "Staged changes can't be discarded: unstage them first." RATIFIED — PRD R8.4 note, a
  deviation row in `docs/design/ui.md`'s "What changes, and why", `local-changes.md`.
- 4(d) Stage All stays in Unstaged's heading with the Alt press; xfwm, openbox and Plasma 5 grab
  Alt+button-1, where the press fails safe — `docs/design/ui.md`, PRD R8.2 note, `local-changes.md`.
- 4(e) the multi-selection diff MATCHES FORK (the selected files' diffs drawn together), built in
  phase 08 on the Commit tab's layout of files opened in place (`cairn_ui::Expansion`); phase 07
  draws the path last pressed in meanwhile — PRD R8.1 note, `docs/design/ui.md`,
  `local-changes.md`, `state.md` (a phase 08 requirement).
- 10 conflicted-row staging LEFT AS FORK: no extra wording before a conflicted row is staged; the
  one-way resolution inside Cairn is a stated residual in `local-changes.md`.
- 11 above.

Dismissed, with reasons:

- 12 "no test of `whole_file_paths`": it is in ed168c2,
  `a_whole_file_action_names_each_rows_path_and_a_renames_source`.
- 13 `Err(String)` across the worker boundary: the engine's errors stay typed
  (`Error::Refused` and the rest, `ops/discard.rs`); the application formats one only to draw it
  and branches on nothing but the cancelled count, which it matches typed.
- 14 a rename row's discard counts two files: the prompt names both honestly, and that is
  `whole_file_paths`' intended rule (the rename moves back whole).
- 15 no end-to-end gesture → git test: each link is tested against its real boundary (the
  component's intents, the window's requests, the lane through the real worker, the engine
  against real git); recording requests at the application is the seam's design.

Carried: to phase 08 the Fork-matching multi-selection diff (4e); to phase 11 batching the
count's per-path reads into one multi-path `git diff-files` read in `reads/` (measure first), and
the argv size of stage, unstage and restore at 50,000 paths (destructive-ops).

## 2026-10-09 — phase 07, Local Changes acts on files (packet mode)

Built on `feature/staging-and-commit`; QA is the coordinator's. Commits ed168c2 (model:
`LocalChanges::whole_file_paths`), 4b60da0 (ui: the lists' selection, routes, drag, menu, the
exact diff's bar, the edge scroll's lost release), f059a13 (git: a nested repository's row
staged, and the engine's `Absent` arm owned by its caller), 988cee5 (app: the actions, the
discard's consequence on the local lane, the confirmation, the line under the lists).

- **No new dependency, no Fork route unbuildable** (stopping rule 1 not met): Freya's
  `EventsCombos` gives the double press, `ContextMenu`/`MenuButton` the menu (each item closes
  it, `ContextMenu::close`), `on_mouse_up` the drop — a press is reported only on the element
  the button went down on, so a drop zone hears the release, as Freya's own `DropZone` does —
  and `freya::clipboard::Clipboard` Copy Path; each read in the vendored fork at `caa46f8`.
- **The selection after a stage (R8.3) for a multi-selection** — stopping rule 2 weighed, and
  decided rather than stopped on, for the user's ratification (item 1 below): the row that
  slides into the FIRST acted row's place, else the nearest row above it
  (`cairn_ui::nearest_remaining`). For a contiguous selection that is Fork's documented
  "nearest remaining" with no other reading; it differs only for a selection with gaps.
- **Where a discard's consequence is asked** (phase 04's carry): on the local lane, as a job of
  its own (`Request::DiscardConsequence`, `LocalJob::Consequence`), so it counts what the writes
  asked before it left (`a_discards_consequence_is_counted_after_the_writes_asked_before_it`:
  a stage held in its `git add`, the consequence answered only after it ended, and refusing the
  path it staged). The answer opens `Confirming::new` from the pane (whose submit it needs);
  one arriving with Local Changes not shown, or for an earlier ask, is dropped.
- **Paths git status never listed** (phase 03's QA item 4): the caller owns it — the window
  asks only for paths of rows the lists drawn list, each found again by a search
  (`a_discard_names_only_paths_the_lists_drawn_still_list`); `staging.md`'s residual and the
  engine test's doc (`a_file_git_clean_leaves_is_named_as_kept`, the `Absent` arm) say so.
- **A collapsed untracked-directory row** is, since status lists untracked files one per file,
  only ever a nested repository (`dir/`): staged by its row it becomes a gitlink at its commit
  and nothing else (`a_nested_repositorys_row_stages_exactly_what_git_add_adds_for_it`, read
  back with `git ls-files --stage`); its discard is refused before any dialog (engine,
  phase 03).
- **Keys** (phase 06's carry): `Scope::LocalChanges` heard on each list's and the diff's own
  `on_key_down`, `Scope::LocalChangesLists` (Shift+↑/↓) on the lists; presses resolved through
  `HeldKeys::press()` (`ExtendSelection` toggles, `SelectRange` ranges). The table gained one
  chord: `StageOrUnstageAll`'s ⌥/Alt **press**, for the heading button (R8.2's "⌥-held");
  every table test and the text-field tests now skip a press where they read keys.
- **While a dialog is open** (phase 06's QA item 4): the dialog's modal holds the keys, and
  `local_changes_actions::intent` and `on_the_diff` act on nothing while a confirmation or a
  credential prompt is up (`local_changes_acts_on_nothing_while_a_confirmation_is_open`).
  **A credential prompt over a confirmation** (item 5, decided): the confirmation is set aside —
  kept, unanswered, not drawn — while the prompt is up, and drawn again after
  (`a_credential_prompt_sets_an_open_confirmation_aside_until_it_is_answered`). **Backspace in
  the filter field** (item 21): `backspace_in_the_filter_edits_the_filter_and_discards_nothing`.
  **The pointer leaving the window** (item 16): `EdgeScroll` (and the lists' drag) end on a
  press heard while a drag is on, and on focus lost
  (`a_release_the_window_never_heard_ends_the_drag_at_the_next_press_or_focus_lost`).
- **R8.5**: the working-tree query is asked with `ignore_whitespace: false` whatever the shared
  setting (`diff_actions::working_options`, and `DiffState::settings_changed` takes the working
  options apart); `DiffHeader::exact` draws the toggle off and disabled and no hiding notice.
- **R8.6**: one line under the lists (`local_changes_actions::acting_line`): why the last
  action asked nothing, a discard being counted, the write running and those queued, or a
  write's ending that did not do what was asked (a stale patch naming its path).
- `expect(dead_code)` removed from `Confirming::new`, `local_writes::ask`, `LocalWrites::{queued,
  running, last}` and `OperationId::next`; `LocalWrite`'s now names phases 08 and 09.

Decisions, and items batched for the user's ratification (none was a stopping rule):

1. **For the user's ratification — R8.3 for a selection with gaps**: the selection moves to the
   row that takes the first acted row's place, else the nearest above it. Alternatives: the row
   after the LAST acted row; or the remaining row nearest the selection's middle. Recommendation:
   keep (the eye stays where the selection began; a contiguous run walks down the list as
   Fork's does).
2. **For the user's ratification — a multi-selection's diff**: the diff shows the path last
   pressed in (or, toggled out, another the selection holds), where Fork draws the selection's
   files together. A combined view is new UI, not in R8.
3. **For the user's ratification — Stage All's placement and press**: the double chevron sits in
   Unstaged's heading (Fork for Windows' place; Linux follows the Windows rows), and the heading
   button's ⌥ press is Alt on Linux (Fork for Windows' own is unrecorded), which some window
   managers take to move a window; the chevron, the menu and the chord remain.
4. **For the user's ratification — what the view says for a staged-side discard**: Fork does
   nothing; Cairn says "Staged changes can't be discarded: unstage them first." (the chord on the
   Staged list or a staged diff), asking nothing.
5. **A double press acts on its row**: its first press makes the row the selection, so a double
   press never stages a multi-selection; C18's "each route stages and unstages a
   multi-selection" holds for the other four routes, each tested both ways, and the double
   press is tested on its row.
6. The menu names no chord beside its items (a chord spelled in a label would be a modifier
   named in a component) and nothing opens it from the keyboard.

Carried forward: see `state.md` (the amend diff against `HEAD^` to phase 09; a discard of every
line of a new file to phase 08, whose gesture is the only route that selects lines).

## 2026-10-09 — phase 06 QA, adjudicated and fixed; the user's decisions applied

Five fresh reviewers and a fresh `qa-confirm` (the coordinator's; probes P1-P3 ran as window
tests). Fixed, each test-first — the new test red on the code before the fix, or the named
mutation or bypass caught:

1. **Critical, items 1-3 as one change — the dialog kept a replaced confirmation's handlers,
   stayed deaf after a chained one, and copied the consequence on every render.** `ChoiceButton`'s
   equality ignored its handler and the dialog was unkeyed, so B set over A drew B and handed A's
   continuation a token naming A's path (P1); `answered` survived into a confirmation opened by
   the last one's answer, leaving Escape, Cancel and confirm dead under inert window chords (P2);
   and the window cloned the `Consequence` and compared it whole each render. Each confirmation
   now has a serial (`Confirming::serial`): the dialog is equal by it and keyed by it (the window's
   `.key` and its own `render_key`), so another confirmation remounts afresh; `Confirming` shares
   its consequence (`Rc`), and the dialog renders its words once as it mounts. Window tests
   `a_confirmation_replaced_in_place_hands_its_own_token_to_its_own_continuation` and
   `a_confirmation_opened_by_the_last_ones_answer_answers_afresh` were red on c74bfff ("the
   replaced confirmation got a token"; "Escape did nothing"). The "identity is stable" comment is
   gone.
2. **Item 7 — the accelerator pin's guard read only the pin's own attributes.** New
   `pin_placement_violations`: declared once, directly in the one module whose attributes are
   exactly `#[cfg(test)]`, its own attributes exactly `#[test]` (or none, for R4.8's pinned
   function), its body still holding its rule examples. R4.8's pin had the same gap (a
   `cfg(any())` module, a nested module) and takes the same check. Self-test
   `the_pin_placement_check_catches_the_shapes_it_claims` (module compiled away, `cfg(not(test))`,
   ignored, nested in a `cfg(any())` module, body emptied, an example dropped, moved into an
   `impl`, renamed) and three new R4.8 shapes.
3. **Item 8 — a field built without `Input` escaped the text-field guard.** `TEXT_FIELD_IDENTS`
   adds `use_editable`, `UseEditable`, `EditableConfig`, `text_edit`, `SelectableText`,
   `CodeEditor`, each self-tested; `TEXT_FIELD_EXCEPTIONS` excuses the accelerator table's
   `text_edit` (the editor's bindings), required to still match. Bypass reproduced: a
   `freya::text_edit::use_editable` in a render file fails the guard.
4. **Item 9** — root `CLAUDE.md` names `async-io` beside `nix` as a dependency whose features
   `deny.toml` pins.
5. **Item 10 — `EventHandler<Confirmed>` was trusted by spelling.** New
   `token_callback_shadow_violations`: no production file declares, aliases or imports-as
   `EventHandler`, `Fn`, `FnMut` or `FnOnce` (self-tested both ways); the residual — a callback
   stashing its token in a captured cell — is stated in root `CLAUDE.md`.
6. **Item 11 — `text_field_in` took any scope.** It takes a `FieldScope` (the commit box alone);
   `a_fields_own_scope_holds_no_bare_chord` holds every field scope free of a bare chord.
7. **Item 14, the user's decision — `is_chord` swallowed another view's chords.**
   `is_chord(event, own)` reads the window's, the detail pane's and the view's own scopes only;
   `another_views_chord_is_the_history_lists_arrow`, `..._file_lists_arrow`,
   `..._commit_tabs_arrow` and `..._diff_views_arrow` were each red before (Shift+↓ moved nothing)
   and green after; `a_chord_of_the_window_the_pane_or_the_views_own_scope_is_a_chord` replaces
   `a_chord_of_any_scope_is_a_chord`.
8. **Item 15** — `edge_step` answers 0 for a viewport with no height
   (`a_viewport_with_no_height_scrolls_nothing`, red before: a pointer at -1000 over `0..0`
   scrolled -32).
9. **Item 18** — root `CLAUDE.md`'s UI-thread residuals name the edge-scroll timer task.
10. **Items 19 and 20 — mutations K and L survived.** The text-field tests' view around the field
    resolves every scope, the window's included, and the window records every raw key: removing
    the unclaimed arm's `stop_propagation` (K) now fails the filter test, and removing the own
    arm's `prevent_default` (L) the commit-box test.

The user's decisions (2026-10-09, relayed by the coordinator):

- **6 — ratified**: the token-holder exemption for a callback's argument, with item 10's
  rename refusal and the stated residual. Root `CLAUDE.md` records it.
- **12 — ratified**: "every text field takes the one key policy", with item 8's widened list.
  Root `CLAUDE.md` records it.
- **13 — R7.3 amended**: "no list or staging chord fires while the commit box has focus; the
  window's chords still do" (as from the filter fields). PRD R7.3 carries a dated note;
  `Scope::CommitBox`'s doc and state.md say so.
- **14 — fix `is_chord`** (item 7 above). Range-select in the history stays a later call.

Carried (state.md): to phase 07, items 4, 5, 16 and 21; to phase 09, how amend builds its token
(through `ConfirmDialog`, the box handing the window a `Confirming`), a token-less force-push
warning if informational, and the widened guard already covering the commit box.

Dismissed:

- **17** (per-key work; the idle context-menu viewer): an observation, bounded — `field_key` is
  one table resolution per key press, `shortcuts::act` returns early in O(1) under a dialog, and
  the viewer's pointer-move write touches its own scope alone.

## 2026-10-09 — phase 06, render foundations (packet mode)

Built on `feature/staging-and-commit`; QA is the coordinator's. Commits f044c95 (the table's
lists and scopes, the text-field key policy, the confirmation dialog and the context-menu host)
and the edge auto-scroll with `async-io` after it.

- **C15 failed first, as the stopping rule requires**:
  `a_focused_filter_field_hands_the_windows_chords_and_held_keys_to_the_window` was written
  before any fix and was red on 2112f18 on both halves — the Refresh chord unheard with the
  sidebar's filter focused (0 refreshes asked), and the extending chord held while the filter
  had focus never reaching `HeldKeys` (a plain selection). Green after the policy.
- **The accelerator table (R7.2, R7.3, L22)**: `chords(action, os) -> Chords`, a list in the
  table's order, replaced `chord(..) -> Option<Chord>`; every caller and test moved. New
  actions and scopes: `StageOrUnstage`, `StageOrUnstageAll`, `Discard` in `LocalChanges`;
  `ExtendSelectionUp/Down` (Shift+↑/↓) in `LocalChangesLists`; `Commit` in `CommitBox`;
  `ShowLostCommits` in `History`; `SelectRange` (Shift+press) beside `ExtendSelection`, which
  now also names a path toggled in Local Changes' lists. No new chord collides with an
  existing Cairn chord (no stopping rule). The pin, renamed
  `chords_are_distinct_and_every_bare_one_is_a_function_key_or_local_changes_own`, checks every
  chord of every list and then shows its rule failing on a bare Enter, Backspace or Delete moved
  to each other scope, a bare letter in Local Changes, a bare press, a chord listed twice for one
  action and a chord shared by two actions. `the_accelerator_table_holds_data_and_resolution_only`
  now also requires the list signature, no single `chord`, and the pin as a plain `#[test]`.
- **The key policy (R7.1)**: `accelerators::field_key` resolves a key for a focused field
  (`FieldKey::Own`, `Unclaimed`, `Edit { bubbles }`); `cairn_ui::text_field` and
  `text_field_in` build every field with it (the three filters and the credential prompt).
  A window chord and a lone Control/Alt/Command pass to the window untyped; a primary+letter
  that is no `EditBindings` binding types nothing; the field's own scope's chord is claimed;
  every other key is kept from the views around (Shift alone still bubbles). New guard
  `every_text_field_takes_the_shared_key_policy` (matcher `builds_a_text_field`, self-test).
  `Chord::press_hold` now sends the modifier's own key, as a keyboard does.
- **The dialog (R7.4)**: `cairn_ui::ConfirmDialog`, on `CONFIRMATION_SURFACES`; the window keeps
  `View::confirming: State<Option<Confirming>>` (`crates/cairn-app/src/confirming.rs`) and
  `shortcuts::act` is inert while it is open. **The context-menu host (R7.5)**:
  `ContextMenuViewer` at the window's root. **Edge auto-scroll (R7.6)**:
  `cairn_ui::{use_edge_scroll, EdgeScroll, edge_step}`, paced by `async_io::Timer`.
- **Dependency**: `async-io = "2.6.0"` in `cairn-ui` (L5, the user's decision): the allowlist
  row, the workspace manifest's reason, and `deny.toml`'s reason with a `[[bans.features]]`
  pin of no features (`exact`, `allow = []`); `Cargo.lock` gained only the edge from `cairn-ui`
  — no crate, no feature (no stopping rule).
- Each new test was checked against the mutation it claims (the dialog without `a11y_modal`,
  without its Cancel focus; the window without the confirmation check or the menu host; the
  field policy replaced by Freya's default; the timer looping once).

Decisions, and items batched for the user's ratification (none is a stopping rule):

1. **For the user's ratification — the token-holder guard excuses a callback's argument.**
   `CONFIRMED_HOLDERS`' rule refused `ConfirmDialog`'s `EventHandler<Confirmed>` and the
   window's `Rc<dyn Fn(Confirmed)>`, which hand a token on and keep none. Rather than roster
   both files (which would excuse any later field there), the matcher blanks exactly the four
   `TOKEN_CALLBACKS` spellings first; a callback that returns a token, takes it beside other
   arguments, or is named otherwise still fails (self-tested). Root `CLAUDE.md` states it.
2. **For the user's ratification — a new invariant line and twin**: "every text field takes
   the one key policy" in root `CLAUDE.md`'s modifier invariant, with
   `every_text_field_takes_the_shared_key_policy`. Its one cost: `window_check.rs`'s private
   test enum `Input` was renamed `Stimulus`, since the guard reads the name.
3. **For the user's ratification — window chords still pass from the commit box.** R7.3 says
   "no chord but commit's fires while the commit box holds focus"; C16 names the stage, unstage,
   discard and Show Lost Commits chords. Built as C16 and R7.1 read together: none of R7.3's
   other actions resolves in `CommitBox`, but a window chord (F5, the tab chords) still reaches
   the window from the box as from any field. If R7.3 means every chord, the box's
   `text_field_in` would claim window chords too — a one-line change in `field_key_on`.
4. **Shift+↑/↓ no longer move the selection in the history, the Changes tab's files, the Commit
   tab or the diff**: they are a chord now (`is_chord`), and those views leave chords alone, as
   they do Ctrl+↓. Before, Shift was ignored and Shift+↓ moved as ↓ did.
5. **`deny.toml` pins async-io's features** (none) beyond the phase's ask, so a later crate
   turning on `tracing` is a decision.
6. The dialog's title is the caller's (`Confirming::new("Discard changes", ..)`, Fork's Windows
   title); `Consequence` renders no title. Its answers are focusable `rect`s, not Freya's
   `Button`, whose focus cannot be given on open.
7. Dismissed: `NamedKey::Super` in the lone-modifier list — deprecated in keyboard-types; Meta
   covers it.

## 2026-10-09 — phase 05, the user's decisions A-F applied

The user decided the six batched items (relayed by the coordinator, 2026-10-09):

- **A — ratified**: `git rm --cached -f -q` out of a root commit's amend. R3.4's note now
  records the ratification (and R6.3 names `-f` too).
- **B — ratified with the fix**: the user accepted QA's verdict that asking the upstream alone
  was a defect (fixed in 44ccdb1); R6.4's amendment records the ratification.
- **C — ratified**: commit and amend are refused while `git am` is in progress. R6.9, R10.8 and
  C24 now list it.
- **D — seed Show Lost Commits from every reflog entry's old and new ids; keep the prompt's
  wording.** R11.1 and C20 amended (the walk's tips are each entry's old and new ids, as `git
  rev-list --reflog` and `git fsck` read a reflog; C20's fixture adds an amend whose log it
  created itself); "The old commit stays in Show Lost Commits." stays; the 30-day
  `gc.reflogExpireUnreachable` expiry is a stated residual in R11.1. A phase 10 requirement that
  blocks its QA (state.md).
- **E — keep `MERGE_MSG`'s `# Conflicts:` lines visible**: the box prefills the file as git
  wrote it and the user deletes them by hand; under `-F` they are committed if left. R10.8 and
  C24 amended; recorded for phase 09 (state.md).
- **F — ratified**: `git commit -q`, and no `--literal-pathspecs` on a commit. R6.1's note
  records the ratification.

Phase 05 is done: QA adjudicated, its fixes and these decisions applied, the full gate green
at c07c076 (this entry's change is documentation alone; `gate.sh --fast` and `qa-stop.sh` run
over it).

## 2026-10-09 — phase 05 QA, adjudicated and fixed

Four fresh reviewers and a fresh `qa-confirm` (the coordinator's). Fixed, each test-first —
the new test red on the code before the fix, or the named mutation caught:

1. **An upstream behind `HEAD` answered "unpublished" without asking other remotes** (= batched
   B, a defect against `Publication::Unpublished`'s "no remote-tracking ref reaches it"): the
   pushed check now falls through to `HEAD --not --remotes` when the upstream does not hold
   `HEAD`. `the_dialog_is_asked_exactly_when_a_remote_has_head` gains the case (an upstream
   behind, another remote branch at `HEAD`, checked against `git branch -r --contains`), red
   before the fix. R6.4's method sentence amended for the user's ratification. (`fix(git)`.)
2. **No test checked the argv `git commit` actually ran**: a recording `git` now pins both
   verbs' argv, stdin, environment and the command log
   (`commit_and_amend_run_git_commit_with_the_message_on_stdin_and_nowhere_else`); the `-m
   <message>` mutation of `run` fails it. (`test(git)`.)
5. **The CLAUDE.md lane-lock residual** omitted `install`'s kill and the amend walk's poll
   under the lock; both named. (`docs(docs)`.)
6. **Amend's staged list in a partial clone**: fails closed on git 2.44+, fetches below it —
   stated as status's residual is, and pinned on the host and both floors
   (`in_a_partial_clone_amends_staged_list_fails_rather_than_fetching`). (`test(git)`.)
7. **The reflog rule was never tested one log at a time**: a detached `HEAD` with only its own
   log, and a branch with only its own, under `false`; dropping either term of the rule fails
   the test (both mutations run). (`test(git)`.)
8. **The lane's hook-skip mapping was never exercised**:
   `a_failing_hook_fails_a_commit_and_the_skip_commits_past_it`; inverting the mapping, or
   fixing it either way, fails it. (`test(app)`.)
9. **The re-check was never tested against a reflog change**:
   `an_amend_refuses_when_the_reflog_it_promised_is_gone_since_it_was_confirmed`; a re-check
   comparing `HEAD` alone fails it. (`test(git)`.)
10. **R6.1's amendment** now says "for the user's ratification" and reads `-q` as by analogy
    with decision 12; batched as F below. (`docs(docs)`.)

Carried (state.md): 3 and 11 to phase 11; 4 to phases 09 and 11; 6's and 8's view halves to
phase 09; 12 (= D) to phase 10, pending the user.

Dismissed, with its reason:

- 13 — **a fetch's askpass title could carry a URL with credentials**: the network lane names
  the token after `Operation::Fetch { remote }`, the default remote's NAME; fetch never takes a
  URL.

The bench repository: its `.git` directory's mtime moved at 2026-10-08 20:35:34 — an entry
created and removed inside it, most likely a lock from a `git` run there without
`GIT_OPTIONAL_LOCKS=0`; no file inside changed, and the agent is unknown. The coordinator's
earlier "untouched" check had used a time format `bfs` rejects, with its stderr hidden. (This
phase's own reads of the bench, on 2026-10-09, ran with `GIT_OPTIONAL_LOCKS=0`; its
`find -newer <marker>` checks were empty.)

Batched for the user, added to A-E below:

- **F.** Ratify R6.1's amendment: `git commit -q`, by analogy with decision 12, and no
  `--literal-pathspecs` on a commit.

B is no longer batched: QA found it a defect, fixed above (item 1), and R6.4 is amended for
ratification.

## 2026-10-09 — phase 05, the commit engine (packet mode)

Built on `feature/staging-and-commit`; full gate green; QA pending (the coordinator dispatches
the reviewers). No stopping rule was met:

- **`-F -` against `git commit -F <file>`**: byte-identical under every `commit.cleanup` value
  and none, `core.commentChar` unset and `;`, with `#`/`;` lines, CRLF, trailing spaces and
  blank lines, a scissors line and non-ASCII text, by commit and by amend, on git 2.30.9,
  2.32.7 and 2.56.0 (checked by hand first, then pinned:
  `a_message_is_stored_as_git_commit_f_stores_it_under_every_cleanup`).
- **The floors amend as the host does**: every test of `crates/cairn-git/tests/diff/commit.rs`
  passes on 2.30.9 and 2.32.7 (amend, root amend, merge, rebase, `git am`, cherry-pick and its
  sequence, revert, the reflog arms, the publication cases).
- **The pushed check stays bounded on rust-lang/rust** (a plain `git clone --no-hardlinks` of
  the bench at `c999cef531e` in `/tmp`, its 13 remote-tracking refs fetched from the bench's
  own, deleted after; release build, warm, the `the_pushed_check_on_a_large_repository`
  reporter, three rounds each):

  | `HEAD` | Publication | Time |
  | --- | --- | --- |
  | `main` at the tip, upstream `origin/main` | upstream | 0.2-2.7 ms |
  | `main` at the tip, no upstream | some remote | 0.2-2.2 ms |
  | a new commit on the tip, no upstream | unpublished | 91-99 ms |
  | a new commit, upstream behind it | unpublished | 0.2-2.4 ms |
  | detached at `main~2000` | some remote | 1.10-1.12 s |
  | detached at `main~30000` (2013) | some remote | 1.30-1.31 s |

  git's own `rev-list -1 HEAD --not --remotes` in the last state: 1.14 s without a
  commit-graph, 0.08-0.10 s with it (`for-each-ref --contains` 0.01 s). The walk reads no
  commit-graph so it stays cancellable at every object read; bounded by the history, as git's
  is without a graph, and on a worker. Judged bounded, not stopped for; the cost and the
  commit-graph option are carried to phase 11.
- The bench was read only: `find ~/Development/bench/rust/.git -newer <marker>` empty before and
  after the clone and the fetch from it.

What shipped, by acceptance criterion (engine halves; the views are phases 06-09):

- **C13**: the message test above; `a_non_utf8_commit_encoding_is_refused_before_git_runs`;
  `a_failing_pre_commit_hook_fails_the_commit_with_its_output_and_the_skip_commits` (output on
  the failure and streamed, nothing committed, no lock, the skip commits; `.git/hooks`,
  `core.hooksPath`, a non-executable hook not counted);
  `amends_staged_list_is_the_index_against_heads_parent` (against `git diff --cached
  --name-status HEAD^`, a rename paired) and the root commit's against the empty tree;
  `a_root_commits_amend_works_and_unstages_with_rm_cached`;
  `amend_is_unavailable_on_an_unborn_branch`; `with_no_identity_gits_own_error_is_the_outcome`;
  `recent_messages_are_git_logs_last_ten`.
- **C14**: `the_dialog_is_asked_exactly_when_a_remote_has_head`, `the_amend_button_and_prompt_name_head`.
- **C24**: `a_merge_in_progress_commits_the_merge_and_refuses_an_amend`,
  `a_rebase_am_cherry_pick_or_revert_in_progress_refuses_commit_and_amend` (every state made by
  real git; `git status`'s words checked beside the engine's answer).
- **C2 (amend)**: `an_amend_refuses_when_head_moved_or_was_published_since_it_was_confirmed`.
- **R6.4's reflog** (the user's decision on phase 01's item 13):
  `whether_the_reflog_is_written_is_what_git_then_does`, each arm against the entry git then
  writes. Found doing it: git's own `git reflog` lists each entry's NEW id, so with the logs
  removed and the default setting, the amend writes an entry whose OLD id is the replaced
  commit, which `git reflog show --format=%H` (C20's seed) never lists. Carried to phase 10.
- **C12 (identity)**: `a_commit_is_by_the_identity_in_cairns_environment`.
- **C10-C12's commit halves on real `git commit`** (`worker/local_lane_tests.rs`, a commit held
  in its `pre-commit` hook): `a_commit_keeps_refreshes_back_and_a_stage_asked_meanwhile_waits_for_it`,
  `a_refresh_asked_before_a_commit_draws_nothing_while_it_runs`,
  `a_cancel_names_its_commit_and_never_reaches_the_one_queued_behind_it`,
  `a_close_ends_a_fetch_at_once_while_it_waits_on_a_commit`,
  `a_close_during_a_commit_waits_for_it_and_ends_nothing`,
  `a_signed_commits_prompt_is_titled_by_the_commit_while_a_fetch_runs` (a real `gpg.format=ssh`
  commit, its signing program a stand-in `ssh-keygen` that asks through `SSH_ASKPASS` — no real
  key, agent or `~/.gnupg` touched), and `an_amend_through_the_lane_quotes_its_prompt_and_keeps_refreshes_back`.
  Each ran five times green.

The carries from phases 03 and 04: `hooks_path` re-exported; `UnstageTo::Commit`/`Nothing`
exercised through amend; the root amend's refusal met (`-f`, below); `HeldCommit` deleted; the
`deferred` override deleted (item 7), every commit's ending reading everything
(`a_commits_ending_reads_everything_whatever_it_did`); the fixed 500/700 ms windows and the
`< 100 ms` bound replaced (item 9) by the repository thread's answer to a later
`Request::CommandLog`, a held process's mark, or a held `git add` the asks must return before —
the one time waited out is the close's patience, which is what that test tests; a prompt titled
by its own operation (item 13: `Channel::begin_for`, `Prompt::operation`, `Update::Prompt`'s
`asking`); `install()`'s kill under the lane's lock documented as a `try_lock` and a `killpg`;
`-q` on `git clean` with a test that `restore` and `apply` are silent on success
(`the_destructive_verbs_say_nothing_on_success_so_an_orphan_finishes`); the banner. Re-carried:
`Request::CancelWrite`'s `expect(dead_code)` (phase 09 first constructs it).

Decisions taken here (each stated in the PRD as amended or in `docs/systems/staging.md`):

1. **`git rm --cached -f -q`** out of a root commit's amend: without `-f`, git refuses a path
   whose staged content differs from both the file and `HEAD` — exactly what an amend's list
   shows edited. `-f` drops the staged blob from the index as `git reset` would; `--cached`
   keeps the file. R3.4 amended, for the user's ratification.
2. **`git commit -q`**, and no `--literal-pathspecs` on a commit (git exports it to the hooks,
   where a `pre-commit` hook's globbed pathspec would match nothing). R6.1 amended.
3. **`git am` refuses a commit and an amend** as a rebase does; R6.9 names a merge, a rebase, a
   cherry-pick and a revert, and `git status` reports an `am` session as one of them. Batched.
4. **The operation in progress is read as git's `wt_status` reads it**, not through gix's
   `Repository::state`, whose order differs and which misses a cherry-pick sequence whose
   stopped pick was committed (the API checked against the vendored gix 0.87.1).
5. **An upstream that is a local branch is no remote** (`branch.<b>.remote = .`): the pushed
   check falls back to every remote-tracking ref, as for none.
6. **A cancel before git runs stops the commit's checks** (an amend's walk can be long) and
   ends `NotRun`; once git runs, the lane's `install` path the phase doc names.
7. **The commit's output reaches the window as `Update::WriteOutput`** per line, as a fetch's
   progress does; and a failure carries stdout's tail ahead of stderr's, since git says why on
   stdout for "nothing to commit" and an amend that would be empty.

Batched for the user (no stopping rule; none blocks the packet):

- **A.** Ratify decision 1 (`-f` on the root amend's unstage) — or prefer refusing that path
  with git's words.
- **B.** R6.4 as written asks only the upstream when there is one: with the upstream behind
  `HEAD` and another remote branch holding it, the amend reads as unpublished and no dialog is
  asked. Recommendation: fall through to `HEAD --not --remotes` when the upstream does not hold
  it (a second walk only in that case).
- **C.** Decision 3 (`git am` refused). Recommendation: keep.
- **D.** Phase 10's seed for Show Lost Commits (the reflog finding above). Recommendation: seed
  from each entry's old and new ids, as `git rev-list --reflog` does, so `Reflog::Written`
  always means findable.
- **E.** R10.8's prefill keeps git's `# Conflicts:` comment lines from `MERGE_MSG`, which a `-F`
  commit under the default cleanup stores. Recommendation: the box strips git's comment lines
  from the prefill as git's own editor cleanup would (phase 09).

## 2026-10-09 — phase 04 QA, adjudicated and fixed; the user's decisions 12 and 14 applied

Five fresh reviewers (`responsiveness-reviewer`, `destructive-ops-reviewer`,
`gate-integrity-reviewer`, `test-coverage-auditor`, `qa-checklist`) and a fresh
`qa-confirm`. Fixed, each test-first (the new test, or the guard with the bypass planted,
red on the code before the fix):

1. **The inherited roster was cut at the first `];` in its raw text**, so a comment or a
   string holding one hid a following `GIT_AUTHOR_DATE`, and the twin passed (planted, and
   it did). `commented_table` now finds the closing with comments and strings blanked; the
   self-test spells both shapes. (`fix(guards)`.)
2. **A close waited on the local lane before ending a fetch**, so a fetch reached the network
   for as long as a commit's hooks ran. `Threads::drop` cancels the fetch through
   `FetchControl` first (`a_close_ends_a_fetch_at_once_while_it_waits_on_a_commit`, which
   waited the stream out before the fix). (`fix(app)`.)
3. **The open's lock listing walked `refs/` on the repository thread before its first
   answer.** The local lane lists them before any write as `Update::LocksAtOpen`, and the
   walk polls the lane's closing before each directory (`stranded_locks_until`,
   `a_cancelled_search_answers_nothing_and_an_uncancelled_one_everything`;
   `SharedRepository::lock_files` takes a `Cancel`). (`fix(app)`.)
4. **`CONFIRMED_HOLDERS` read the engine alone**, so the application's `LocalWrite` held a
   `Confirmed` with no row. The scan reads every crate's production code, requires the
   worker among it, and rosters `crates/cairn-app/src/worker/local_lane.rs`; emptying the
   roster turns the twin red, and the self-test refuses an unrostered application holder.
   Root CLAUDE.md updated. (`fix(guards)`.)
5. **The lane was marked closing only in `Threads::drop`.** `submit`'s close arm marks it too
   (`a_close_marks_the_local_lane_closing_as_it_is_submitted`); the CLAUDE.md residual for the
   close arm says so. (`fix(app)`.)
6. **R4.6's thread-side gates were never reached by a test.**
   `a_refresh_asked_before_a_commit_draws_nothing_while_it_runs` queues a refresh's
   ahead/behind behind a held status as the commit starts; dropping the refresh thread's
   ahead/behind gate turns it red. The repository thread's gate on refs is not driven (its
   refs are read before a test can start a commit behind them), stated in
   `docs/systems/git-processes.md`. (`test(app)`.)
10. **The roster's doc counted four identity variables**; it names all five. (`docs(git)`.)

The user's decisions (2026-10-09, relayed by the coordinator):

- **12 — the second close during a write: keep R4.9 and harden it.** A second close past
  the patience still closes the window and never ends the write. `docs/design/processes.md`
  ("Lifecycle") now says so instead of "ends it as a cancel would", with the orphan residual
  (an orphaned `git` dies of `SIGPIPE` at its next line of output, which `-q` keeps `git
  clean` from writing), stated as built in `docs/systems/git-processes.md` ("Closing"). The
  hardening is phase 05's (state.md).
- **14 — reads inherit the same roster as writes: keep one roster.** No code change; the
  reason — it is what the user's own `git` gives a filter or hook from a shell — is beside
  `INHERITED` in `crates/cairn-git/src/process/environment.rs`.

Filed: #87 (every local write walks all of `refs/` twice, `ops/local_write.rs`'s
`locks_now`/`locks_around`, phase 03 code — for phase 11's measurement) and #88 (`gate.sh
--fast` does not build the askpass helper though `built_helper`'s comment says the gate
does).

Dismissed, with reasons:

- 15 — **the identity variables are inherited**: the user's decision (R5.2, L26); the date
  variables are pinned out twice, in the builder's tests and the twin.
- 16 — **a write is dropped if `cairn-local` fails to spawn**: fail-closed by design, as for
  the diff and refresh threads (`Threads::start`); a dropped `Confirmed` means nothing
  destructive runs.
- 17 — **the UI thread takes `LaneState`'s mutex**: stated in the root CLAUDE.md residuals;
  every hold is bounded (an assignment and at most one unbounded send), with no lock taken
  inside it that the UI thread holds.
- 18 — **`refs/` walked twice per write**: phase 03's code, unchanged here; filed as #87.
- 19 — **the gate failed on a missing askpass helper**: a cold target; `--fast` runs `--lib
  --bins` only; filed as #88.

## 2026-10-08 — phase 04, the local write lane (packet mode)

Built on `feature/staging-and-commit`; gate green; QA pending (the coordinator dispatches the
fresh reviewers implementation-plan.md names: `responsiveness-reviewer`,
`destructive-ops-reviewer`, `gate-integrity-reviewer`, `test-coverage-auditor`).

What shipped (`docs/systems/git-processes.md`, "The local write lane"):

- **The lane** (R4.1-R4.3): `crates/cairn-app/src/worker/local_lane.rs`, the `cairn-local`
  thread. `Request::Write { id, write: LocalWrite }` routed by `submit` straight to its
  queue; FIFO; `OperationId` taken by the window as it asks (`OperationId::next`,
  `local_writes::ask`); `Update::WriteStarted` / `WriteEnded { id, ending, read_again }`;
  `Request::CancelWrite { id }` reaching `LaneState` directly, ending only a running
  commit with that id. `LocalWrite` carries the six phase-03 verbs, a discard its
  `Confirmed`; `UnstageTarget` is `UnstageTo` in the window's words.
- **Freshness** (R4.4-R4.6): `LaneState`'s write clock ticks at a write's start and end
  under the lock the lane announces under; the refresh thread reads no status while a
  write runs and sends one only if the clock did not tick while it was read (same lock).
  A write's ending names what to read again (`ReadAgain::Status` →
  `Request::RefreshStatus`, new, status alone; `ReadAgain::Everything` →
  `Request::Refresh`); the window asks exactly that. While a commit runs, a refresh is
  kept back at `submit` and at the repository thread's refs, the refresh thread's status
  and ahead/behind; the commit's ending says `Everything`.
- **Endings** (R4.7, R4.9): `WriteEnding::{Done, Stale, Refused, Failed,
  MayHaveTakenEffect, Incomplete, NotRun}`, each with its lock files. Closing marks the
  lane closing, joins it (the write running finishes; ones queued end `NotRun`), then ends
  the rest; the window says "Finishing <write>…" (`Closing::when_requested`,
  `status_text::closing_line`). `Update::Opened` carries `SharedRepository::lock_files`
  (new), and the window names the locks last listed (`status_text::locks_line`).
- **Prompts and the roster** (R5): one askpass token per write; the window shows a prompt
  while a fetch or a write is in flight, titled by the write when no fetch runs. R5.2's nine
  joined `INHERITED`, each with its reason, and the environment twin reads the roster
  (`INHERITED_PINS`, `INHERITED_NEVER`). R5.3's residual was already stated in
  `docs/design/processes.md`; filed as #86 and cited there.
- Window state: `crates/cairn-app/src/local_writes.rs` (`LocalWrites`), `View::writes`.

Tests: C10 — `writes_asked_faster_than_they_run_run_in_order_each_with_its_own_ending`,
`a_status_begun_before_a_write_ended_is_never_drawn`,
`a_commit_keeps_refreshes_back_and_a_stage_asked_meanwhile_waits_for_it`,
`a_cancel_names_its_commit_and_never_reaches_the_one_queued_behind_it`,
`a_writes_ending_reads_again_what_it_says_and_no_more`; C11 —
`a_close_during_a_commit_waits_for_it_and_ends_nothing`,
`a_lock_left_behind_is_named_as_the_repository_opens_and_by_the_write_it_fails`,
`the_window_is_told_once_as_the_first_close_is_asked`,
`the_window_names_the_write_it_waits_on_its_prompt_and_the_locks_found`; C12 —
`a_prompt_a_stages_hook_raises_is_shown_and_answered` (a real `git add` whose
`post-index-change` hook asks through the helper),
`a_prompt_a_commit_raises_is_shown_and_answered`,
`a_prompt_is_shown_while_a_write_runs_and_its_ending_takes_it_down`, and the twin's
roster check with its self-test. Mutations that redden them, checked by hand: sending a
status whatever the clock (the stale-status test), never keeping a refresh back (the
commit test), not joining the lane on close (the close test).

Decisions and deviations:

- **The commit-dependent halves run against `LocalWrite::HeldCommit`**, a variant compiled
  for tests alone: `ops::fetch` through a stub `git` whose `fetch` is held, asks or ends as
  the test says — the one long-running, cancellable write the engine has before phase 05,
  which the lane treats as a commit (`is_commit`, `ReadAgain::Everything`). Phase 05 adds
  `Commit`/`Amend` to `LocalWrite` and re-runs those tests against `git commit`.
- **`Request` is no longer `Clone`**: a destructive write carries its `Confirmed`, spent
  once. Tests keep `Clone` through a test-only impl on `LocalWrite` that refuses to copy a
  destructive write.
- **A status skipped or dropped is always replaced**: a write's ending always names status
  at least (every local verb's `Invalidated` has the index or the working tree), and the
  window asks it unless it is closing. So the stopping rule (a dropped status no later read
  replaces) did not trigger.
- **Fetch's types and behaviour are unchanged**: the local lane is a sibling, not reached
  through `Threads::perform`; the one new fetch behaviour arises only beside a running local
  write (a fetch's ending leaves a prompt up while a write runs). Not a stopping-rule change.
- **No roster variable carries a secret**: `XAUTHORITY` is a path to the cookie, never it.
- **The open's lock listing is on `Update::Opened`**, made on the repository thread before
  its first answer, so a stream's order is fixed (a separate update from another thread
  would land anywhere in a test reading in order). It walks `refs/` once per open; not
  measured on the bench repository (phase 11 may).
- **A cancel for a queued commit does nothing** (R4.3: only while it runs); a Cancel the
  commit box draws (phase 09) must be for the running one.

Batched for the user (no stopping rule; recorded for the packet's end):

1. **The second close during a write.** `docs/design/processes.md`, "Lifecycle", says a
   second close past the patience "ends it as a cancel would"; PRD R4.9 says it "closes
   anyway, as today". Built per the PRD: the window closes and the write's `git` is left to
   run on, orphaned — it finishes, or its next write to a pipe nobody reads ends it — and a
   lock it leaves is named the next time the repository opens. Ending it with `SIGTERM`
   first needs a non-waiting "end everything" the UI thread can call (the registry's
   `end_all` waits). Recommendation: accept the PRD's behaviour and amend the design's
   sentence; or file the non-waiting end as a follow-up.
2. **A prompt carries no owner.** With a fetch and a local write in flight at once, the
   ending of one leaves a prompt up that may have been the other's; it stays until answered
   or cancelled, and the acceptor serves nothing else meanwhile. Tagging a prompt with its
   operation needs `cairn-askpass`'s `Prompt` to say which token asked. Recommendation:
   accept for now (rare: two writes asking at once); file if phase 11 shows it.

Carried forward:

- **Phase 05**: `LocalWrite::Commit`/`Amend` (`is_commit` true, `ReadAgain::Everything`,
  `perform` installing the cancel with `LaneState::install`, whose and
  `Request::CancelWrite`'s `expect(dead_code)` then go); re-run the `HeldCommit` tests
  against `git commit` with a slow hook (C10, C11, C12's halves).
- **Phase 07**: asking writes (`local_writes::ask`, whose `expect(dead_code)` then goes),
  drawing a write queued where the user acted and its outcome (`LocalWrites::queued`,
  `last`); where the window asks `discard_*_consequence` before a dialog (they run `git`,
  so a worker's call; the local lane orders them after the writes ahead, which a
  consequence computed while a stage is queued would otherwise race).
- **Phase 11**: the activity popover reads `WriteEnding` and `Done`; the open's lock listing
  measured on the bench repository.

## 2026-10-08 — phase 03 QA, adjudicated and fixed; the user's four decisions applied

Four fresh reviewers, adjudicated by a fresh `qa-confirm`
(`scratchpad/qa03/adjudication.md`). Confirmed findings fixed in focused commits, behaviour
changes test-first (each new test seen to fail on the code before its fix):

- 1 (critical): a deleted tracked `d/a` with an untracked file `d` where its directory
  was read as absent, so `git restore` would unlink `d`; a symlinked `d` was hashed outside
  the working tree. Every directory on a path is now looked at without following links, and
  an obstructed path is refused before any prompt (`Refusal::Obstructed`) and by the
  re-check (`a_file_where_a_deleted_files_directory_was_is_never_destroyed`,
  `a_symlinked_parent_is_never_followed_out_of_the_working_tree`).
- 2 (engine half): the executable bit is recorded in every discard's `Consequence` and
  re-checked, so a `chmod` after the confirmation refuses (the mode moves in both C2 tests).
- 3, 4: a discard of files reads every confirmed file again after the run and answers
  `Error::DiscardIncomplete { performed, kept, failure }` when it did not take every one —
  `git clean` failing part way after `git restore` ran, or leaving an ignored file
  (`a_discard_that_fails_part_way_says_what_it_did_and_what_is_left`,
  `a_file_git_clean_leaves_is_named_as_kept`). Item 4's refusal of paths status never
  listed is carried to phase 07 (state.md).
- 5: `Consequence::DiscardLines` carries the patch emitted from the confirmed diff, and
  `discard_lines` applies exactly it and takes no diff
  (`a_discard_applies_the_patch_it_was_confirmed_with`; the model's
  `the_confirmed_patch_is_part_of_the_value`).
- 6: the git-form comparison is decisive — a filter attribute added after the
  confirmation moves git's form with the bytes unchanged; with the comparison removed the
  test fails (checked by hand).
- 7: `hashing_a_file_runs_only_the_clean_filter_and_writes_nothing` and
  `the_hooks_path_read_writes_nothing_and_runs_nothing`, named in the root `CLAUDE.md`'s
  list of reads held to the clean filter and fsmonitor.
- 8: git-floor's floors raised to 128, 166 and 16 (counts 129, 167, 17).
- 9: `reads/hash_object.rs`'s anchor fixed. 10: git-floor's skip count is seven.

The user's decisions of 2026-10-08 (relayed by the coordinator), applied — batched items
2, 3, 5 and 6 of the phase 03 entry closed:

- **Decision 2** (batched 2): R3.9's amendment ratified, reworded to "the bytes and the
  executable bit"; `docs/design/engine.md` matches.
- **Decision 3** (batched 3): C2, R3.5 and C8 amended to what is built — a file added
  beside a confirmed one is never taken and the discard proceeds, a path behind a file or a
  symlink is refused, `-d` is never passed.
- **Decision 5** (batched 5): an intent-to-add file's discard keeps `git restore`'s
  behaviour, worded honestly: `FileLoss::Emptied`, "1 new file emptied (20 lines)"
  (`an_intent_to_add_file_is_named_as_emptied`,
  `an_intent_to_add_files_discard_empties_it_and_says_so`); R1.2 amended.
- **Decision 6** (batched 6): a whole file's mode change is named — `FileLoss::Modified`
  carries `mode`, "1 modified (2 lines and the mode change (100644 to 100755))", and alone
  never "0 lines" (`a_whole_files_mode_change_is_named_never_counted_as_no_lines`,
  `a_whole_files_mode_change_is_named_and_put_back`); R1.2 amended.

Still batched for the user: 4 (pinning `apply.ignoreWhitespace` anyway) and 7 (a discard
of files reads one diff per tracked file to count its lines).

Dismissed, with the adjudicator's reasons:

- 11 (`IndexNow` freshness): `IndexNow::read` calls `open_index()`, which in gix 0.87.1
  (`src/repository/index.rs`) builds a fresh `File::at` each call — no cache, no mtime gate.
- 12 (`hash-object` lazily fetching through `.gitattributes` on git before 2.44 in a sparse
  partial clone): the accepted residual the root `CLAUDE.md`'s environment invariant already
  states; the 2.30 floor is the user's decision.
- `write_verbs`' 24 tests in 0.04 s looked too fast: 0.21 s single-threaded, a `git` spawn
  takes under a millisecond on this host, and no test returns early. Not a defect.

## 2026-10-08 — phase 03, C21's margin decided

The user chose C21's margin on 2026-10-08, relayed by the coordinator: a flat 50 ms per
verb, added to git's own time plus one status read (option A of the phase 03 report). From
`measured-baseline.md`'s highest sums the bars are about 93 ms to stage (42.9 + 50), 93 ms
to unstage (42.8 + 50), 76 ms to discard (25.8 + 50) and 87 ms to commit (37.4 + 50).
PRD C21 and R13.2 amended. Batched item 1 is closed; items 2-7 stay batched.
`measured-baseline.md` is evidence and not retro-edited (docs/CLAUDE.md); its section 4
already set out the 50 ms option, and the PRD records the decision.

## 2026-10-08 — phase 03, the write verbs (packet mode; stopped for C21's margin)

Built on `feature/staging-and-commit`; QA pending (the coordinator dispatches the fresh
reviewers). Stopped at R13.2's stopping rule: git's baseline exists
(`docs/research/staging-and-commit/measured-baseline.md`), and C21's margin is the user's.

What shipped (`docs/systems/staging.md`):

- `ops::stage_lines`, `unstage_lines` (`git apply --cached --whitespace=nowarn -`),
  `discard_lines` (`git apply --whitespace=nowarn -`), `stage_files` (`git add`),
  `unstage_files` with `UnstageTo::{Head, Commit(id), Nothing}` (`git reset -q`, `git
  reset -q <id>`, `git rm --cached -q`), `discard_files` (`git restore --worktree`, then
  `git clean -f --` in batches) — every one a write with `--literal-pathspecs` before the
  verb, many paths in a NUL pathspec file on stdin, the locks listed before and after on
  `Performed::locks` (R3.8), taking `Option<&AskpassToken>` for phase 04.
- The stale check (R3.7): the index read fresh with gix (`IndexNow`, once per call), the
  working tree's git form by `reads::hash_object` (`git hash-object --path=<p> -- <p>`,
  never `-w`) and its bytes hashed in process; `reads::hooks_path` (`git rev-parse
  --git-path hooks`) landed for phase 05, module-private until then.
- `discard_lines_consequence` and `discard_files_consequence` build each `Consequence`;
  `discard_lines` and `discard_files` take `Confirmed` by value, re-check before the first
  write and replaced `describe_destructive` on the roster in the same change.
- `Consequence::DiscardLines` gained `on_disk` (the bytes' hash) and `mode` (named in the
  prompt with both modes); `FileLoss`'s working-tree id is the bytes' hash.
- New errors: `Error::Refused { path, why: Refusal }`, `NoPaths`, `ChangedSinceRead`,
  `ChangedSinceConfirmed`, `ReadIndex`, `ReadWorkingTree`.
- C3 now runs through the verbs; `tests/diff/write_verbs.rs` holds C2's discard halves,
  C5, C6, C8's engine half and C9's effect; the recording `git` (`ops/recording_stub.rs`)
  holds C9's argv. `scripts/git-floor.sh` runs `ops::` too, floors 126/158/16.

Measurements and decisions:

- **`git clean`'s argv bound** (R3.5): `ops::CLEAN_ARGUMENT_BYTES`, 64 KiB per invocation,
  each path counted as its bytes, its NUL and its 8-byte pointer. Measured on this host:
  `getconf ARG_MAX` 2,097,152; `git --literal-pathspecs clean -n -- <names>` took 1,536 KiB
  of 100-byte names and the kernel refused 1,900 KiB (`E2BIG`, the pointers and the
  environment counted); one argument of 128 KiB is refused (`MAX_ARG_STRLEN`). 64 KiB is
  half Linux's smallest `ARG_MAX` (32 pages), a sixteenth of macOS's 1 MiB. A list past it
  runs as several `git clean`s after one re-check
  (`a_long_list_is_deleted_in_batches_after_one_recheck`: 700 paths, 2 invocations).
- **`apply.ignoreWhitespace`**: measured on 2.30.9, 2.32.7 and 2.56.0 over 400
  selections of a whitespace-heavy diff, staged with and without `-c
  apply.ignoreWhitespace=change` (and `=false`): every staged blob identical, every apply
  succeeded. The setting only lets a patch land on context that moved by whitespace (a
  stale index: exit 0 with it, exit 1 without), which the stale check refuses first. Not
  pinned; C5 holds the case; PRD's filed list amended.
- **`hash-object` reproduces the diff's git form** of a CRLF file under `core.autocrlf`
  and of a rot13-filtered file, with `--path` and with several paths at once, on 2.30.9,
  2.32.7 and 2.56.0, and writes nothing — so that stopping rule did not fire. A symlink
  is hashed in process (as its target), since `hash-object` reads the file it points to.
- **No verb behaved differently on 2.30.9** in what Cairn passes: `--end-of-options`
  before a commit after `--pathspec-from-file` is refused by 2.30's `reset` ("must come
  before non-option arguments") where 2.56 takes it, so the commit is passed as its hex
  id with no `--end-of-options` (an id cannot be an option); every verb as built passes
  the same tests on 2.30.9, 2.32.7 and 2.56.0.
- **Hashing for the re-check** (phase 01's QA item 25): every discard compares the
  file's bytes hashed with no filter, so a line ending changed after the confirmation
  refuses; a discard of lines also compares git's form (R3.7). PRD R3.9 amended — for the
  user's review.
- **The mode in a discard of lines** (QA item 19): `Consequence::DiscardLines.mode`, the
  prompt "discard 2 lines and the mode change (100644 to 100755)", the label "Discard 2
  Lines and Mode Change" / "Discard Mode Change"; a selection of the mode alone with no
  mode change is refused as nothing selected (QA item 18's zero-line prompt).
- **Every line of a new file discarded** is refused as `WholeFileOnly` and left to
  `discard_files`, whose prompt says "1 untracked file deleted (N bytes). You can't undo
  this action." (phase 02's carry-forward).
- **The rename source row** (phase 02's QA item 8): its staged diff is the rename, and
  its lines unstage at the new path, the source's entry untouched; its own whole-file
  unstage resets the source alone; both paths named unstage the rename whole.
- **R3.8 as built**: a success carries the locks before and after; a failure carries
  git's own report of those present after it (`GitFailed.present_locks`,
  `GitCancelled`/`GitUnwatched`'s `stranded_locks`), which names any lock that was there
  before and still is.
- The bench repository's `.git` was unchanged by everything: `find .git -newer <marker>`
  listed nothing before the clone, after it and after every baseline run.

Batched for the user's review — not decided:

1. **C21's margin** (the stopping rule): proposed below in the phase's report.
2. **R3.9's amendment**: a discard of files re-checks the bytes on disk hashed in process,
   not `git hash-object` (which cannot hash a symlink as git stores it and would not see a
   line ending changed under `core.autocrlf`).
3. **C2's "an untracked file added to a deletion's directory"**: Cairn never deletes a
   directory (status lists files one per file; the one directory it lists whole, a nested
   repository, is refused), so such a file is never taken and the deletion of the
   confirmed files proceeds (`a_file_added_beside_a_confirmed_deletion_is_never_taken`) —
   not refused, as C2's wording has it. A confirmed file replaced by a directory is refused.
4. **`apply.ignoreWhitespace`**: not pinned, as measured; pinning `-c
   apply.ignoreWhitespace=false` anyway would close the window between the stale check
   and `git apply` for a whitespace-only move.
5. **An intent-to-add file's discard of files** runs `git restore --worktree`, which
   leaves the file EMPTY (git's own behaviour, verified on 2.30.9 and 2.56.0) and the
   intent-to-add entry in place; the prompt counts its lines as modified. Deleting it
   instead (as an untracked file) would be two writes.
6. **A mode change in a discard of whole files** is restored without the prompt naming
   it (`FileLoss::Modified` counts lines only).
7. **A discard of files reads each tracked file's unstaged diff to count its lines** —
   exact, but one read per file selected.

## 2026-10-08 — phase 02 QA, adjudicated and fixed

Four fresh reviewers, adjudicated by a fresh `qa-confirm`. Confirmed findings fixed in
focused commits, behaviour changes test-first (each new test seen to fail before its fix):

- 1: `action_patch` returns the empty patch for an empty selection before anything else;
  an empty untracked file's stage, unstage and discard, and a `ModeChangeOnly` type
  change, wrote headers or `deleted file mode` before
  (`a_selection_of_nothing_makes_no_patch_by_any_action`).
- 2: the whole-index pairing read runs only for a path absent from `HEAD` or the index
  (`only_a_path_that_can_be_in_a_pair_asks_the_whole_index`, which counts the reads in
  the command log; it failed with the read running for a copy's modified source).
- 3: `scripts/git-floor.sh`'s floors raised to one under the runs' counts (84 and 134).
- 4: C3 decides a deletion from the case and checks the patch's header against it; an
  unborn branch's addition unstaged whole leaves no entry.
- 5: the partial-clone residual of the staged pairing stated in `docs/systems/diff.md` and
  pinned on both arms (`in_a_partial_clone_a_staged_pairing_fails_rather_than_fetching`).
- 6: the write-nothing pin asks a staged `git mv` with an edit, so both pairing reads
  run under it, and requires that they did.
- 7: C3's awkward names gain CR, DEL, BEL, BS, VT and FF.
- Optional, done: C3 gains a staged copy under `diff.renames=copies`, unstaged as content
  at the copy's path with real git (`lines_of_a_staged_copy_unstage_as_content_at_its_path`).

Carried forward to phases 03 and 07 in state.md: 8, 9 and the destructive carry list.

Batched for the user's review at the end of the packet — not decided:

- 10: PRD R2.1's text against the insertion-then-removal refinement of
  `TextDiff::inverted` (no reviewer found the refinement wrong).
- 11: whether C21 gains a selection-to-diff row for a staged rename (the cost of the
  pairing read under `diff.renameLimit=0`).

Dismissed, with the adjudicator's reasons:

- 12 (the `ContentReadsDisagree` retry is unbounded): it is not —
  `worker/diff_lane.rs`'s `READ_ATTEMPTS` is 3, and `asking_again` stops at 3 and on
  cancel.
- 13 (C4 derives its expected mode from `drawn.file.inverted()`): the `assert_ne`
  beside it is decisive whatever the expected value says; style only.

## 2026-10-08 — phase 02, the patch engine (packet mode)

Built on `feature/staging-and-commit`; full gate green (git-floor included); QA
pending — the coordinator dispatches the fresh reviewers. What shipped:
`TextDiff::inverted`, `Selection::inverted`, `ChangedFile::inverted` (R2.1);
`Selection`'s mode item (R2.4); C-quoted path lines (R2.5); `action_patch`, what
stage, unstage and discard emit, refusing part of a whole-file-only change (R2.2,
R2.3); the staged side of the working-tree query pairing renames and copies as `git
diff --cached` does (R2.6), unstaging lines of a rename as content at its new path.
C3, C4 and C7 pass against real git on 2.56.0, 2.30.9 and 2.32.7
(`crates/cairn-git/tests/diff/staging.rs`, `staged_renames.rs`); the intent-to-add
question is settled (state.md). No stopping rule fired: every floor git applied
every case as the host's did; the staged pairing is query plumbing (`diff-index`),
no porcelain read; no case needed `-R`.

Decisions taken in the phase (none is a stopping rule; the first is batched for the
user's review, as a refinement of R2.1's wording):

- **An inverted replacement is two changes, its insertion then its removal.** R2.1
  says "each change's two spans swapped". Swapped as one change, the forward rule
  leaves what stays of a partial selection inside one replacement AHEAD of what it
  restores (`B b`), where git's mirrored patch applied with `-R` — E1b, and what git's
  own `reset -p`/`checkout -p` edits do — leaves `b B`; the two oracles C3 names then
  disagree on every mixed selection. With the split, the inversion, the reference
  applier, the mirrored rule as the model states it and real `git apply -R` all agree
  on every case and seed; the unsplit inversion fails both the model's property test
  and C3 (checked by mutation). Visible cost: a whole inverted replacement is written
  `+old` before `-new`, which git applies identically. Still one forward rule, still
  no `-R`.
- **The C3 mirrored oracle is grounded in real git**: written in the test from the
  forward diff (it inverts nothing), applied in reverse both by real `git apply -R`
  on a scratch file outside any repository and by the reference applier, which must
  agree; the reference applier then reads Cairn's own patch, and git applies it.
- **The staged pairing reads the whole index once** (`git diff-index --cached --raw
  -z -M|-C -l<n> --diff-filter=RC --ita-invisible-in-index`), keeping only the
  records naming the path as they stream, then reads the pair's lines across both
  paths with the same detection; a record that moved between the two reads is
  `ContentReadsDisagree`. `--ita-invisible-in-index` was found needed: plumbing
  otherwise pairs a deleted empty file with an intent-to-add one, which `git diff
  --cached` never shows (C7's fixture holds that case; dropping the flag fails C7).
  `--diff-filter=RC` also drops an unmerged entry's `U` record, which the raw parser
  would refuse.
- **A staged rename's source path answers the rename record too**, since that
  record is the only thing `git diff --cached` shows for it; a copy's source keeps
  its own record.
- **The mode item is selected by nothing but `select_mode`**: whole-file round
  trips (`tests/diff/patches.rs`, diff-engine's C1-C3) now select it explicitly.
- **Path lines carry git's trailing tab** after a `---`/`+++` label holding a
  space (`diff.c`), so the emitted headers equal `git diff`'s byte for byte, which
  C3 compares for every awkward name.
- The root `CLAUDE.md` was not edited (state.md, carried forward).

## 2026-10-08 — phase 01, the user's three decisions applied

The user decided the three items the adjudication left with them (relayed by the
packet coordinator):

- **Item 6, the empty roster**: assert it non-empty. The guard now asserts
  `!DESTRUCTIVE_OPERATIONS.is_empty()`, its doc comment says so, and emptying the
  roster was seen to fail the guard. The placeholder row satisfies it until phase
  03.
- **Item 13, the amend prompt**: conditional wording. `Consequence::Amend` carries
  `reflog: Reflog`; the prompt says "The old commit stays in Show Lost Commits."
  only for `Reflog::Written`, and "The old commit can't be recovered afterwards:
  this repository keeps no reflog." for `NotWritten`, with full-literal tests for
  both arms. What decides it was checked against git 2.56: an amend writes the entry
  under `core.logAllRefUpdates=true` (the non-bare default), writes none under
  `false` set from the start, and still appends under `false` set after the logs
  exist; a bare repository leaves the setting unset, and git's default there is
  `false`. R10.6 and R6.4 are amended with a dated note; the engine's computation
  is phase 05's (state.md).
- **Item 34a, R1.2's bytes**: R1.2 amended to Fork's wording — lines per modified
  path and bytes per untracked file (L8) — with a dated note; no code change.

## 2026-10-08 — phase 01 QA, adjudicated and fixed

Eight fresh reviewers, adjudicated by a fresh `qa-confirm`. Confirmed findings
fixed in focused commits (`fix(model)`, `fix(git)`, `fix(guards)`, `docs(docs)`);
each guard fix was mutation-tested — the hole reopened in the real tree, the guard
seen to fail, the tree restored:

- 1: the token-file check reads every function (`function_signatures`), every
  impl header with attributes stepped over (`impl_headers`), refuses `const`,
  `static`, `macro_rules!` and `mod` items, and requires one `by_user` and one
  literal. The Secret guard's same idiom is filed as #85.
- 2: `CONFIRMED_HOLDERS` (empty) refuses an engine type that keeps a token in a field.
- 3: `renames_type` starts a statement after `}` and `{` as well as `;`.
- 4: the ceiling `drive` takes is `Kind::Ceiling`, `Infallible` for a write; a
  generic helper passing `Some(1)` no longer compiles (checked).
- 5: the R4.8 check reads code (comments out), refuses `cfg` or `ignore` on the
  pin, with a self-test (`the_bounded_output_helper_check_catches_the_shapes_it_claims`).
- 7: `RemoveLock` carries `modified`, `read_at`, `bytes`, `device`, `inode`.
- 8: `DiscardLines` carries the `Selection`; the "every target from
  `confirmed.consequence()`" contract is a residual in `CLAUDE.md` and in
  `destructive-ops-reviewer`'s check 4.
- 9: `consequence.rs`, `confirm.rs` and every `CONFIRMATION_SURFACES` file route to
  `destructive-ops-reviewer` (its scope gate, `docs/qa-gate.md`, checklist item 7).
- 10: fixture strings no longer trip `.claude/hooks/qa-stop.sh`.
- 11: state.md's phase 01 row corrected.
- 12: paths and subjects escaped as git C-quotes a path, plus line separators and
  bidirectional controls, with tests.
- 14: the surface scan must have read `cairn-ui/src` and `cairn-app/src`.
- 15: each roster self-test fixture breaks one rule and asserts that rule's
  message; disabling the by-value check fails it (checked).
- 16: `consequence.rs` is held to no `Default`/`From`/`TryFrom`/`FromStr`/
  `Deserialize`/`Decode` and one impl block.
- 17, 31, 32: residuals stated in `CLAUDE.md` and the reviewers' definitions.
- 20: the prompt tests pin literal text per variant, sum several untracked files'
  sizes, use an asymmetric selection, and cover singular hour/day, GiB/TiB, the
  label's fallback and a lock from the future.
- 24: a file deleted in the working tree is named "deleted file restored".
- 28: `.github/workflows/enforcement-review.yml` and the mockup reworded.
- 29: `CONFIRMED_RECORD` is keyed on `Performed`'s inherent impl, not the name.
- 39: `Deserialize` and `Decode` have negative self-test cases.

Carried forward to the owning phases in state.md: 18 and 19 and 25 (phase 03), 21
(phase 09), 23 (phase 11). With the user: 6, 13, 34a.

Dismissed, with the adjudicator's reasons:

- 22 (an amend re-check needs the publication): the shape already carries
  `published`; comparing it is phase 09's re-check.
- 26 (a refused operation leaves no record): R1.6 binds `Performed`, which records
  completed operations; the refusal outcome is R1.4's, phase 03's.
- 27 (added and removed lines both counted as discarded): matches Fork.
- 30 (`<Consequence>::Variant { .. }` escapes `names_a_path_into`): a qualified
  path in a struct expression is unstable, and every variant is a struct variant.
- 33 (unchecked addition could overflow): the counts are bounded by an in-memory
  diff, and only the engine builds the values.
- 34b (merging discard files and delete untracked was unapproved): R1.5 lists them
  as one requirement.
- 35 (the copy doctest cannot flip): harmless; it fails for the reason it names.
- 36 (widening the helpers to every kind is not caught): the check requires the
  declaration inside `impl Invocation<Read>`.
- 37 (a `&mut self` helper on a write escapes the pin): the declared-once check
  catches any second declaration.
- 38 (a scratch copy reported `describe_destructive` off the roster): not
  reproducible on a fresh copy or with a fresh target directory.
- 24b (a collapsed untracked directory is not modelled): the status read expands
  it with `--untracked-files=all`.

## 2026-10-08 — phase 01, the seal (packet mode)

Built on `feature/staging-and-commit`. `Confirmed` lost `Clone`, carries an
engine-computed `Consequence` and the prompt rendered from it, and has one
constructor that takes the `Consequence` alone; `Performed::destructive` spends the
token by value and records its prompt (R1.6); the bounded-output helpers moved to
`impl Invocation<Read>` (R4.8, #45 item 2). The guard was rewritten around two
rosters and the forging and route checks; the root `CLAUDE.md` invariant names
them and their residuals (C22).

Decisions taken in the phase (none is a stopping rule):

- **One `DiscardFiles` variant for discarding files and deleting untracked files**,
  not two: Fork confirms a mixed selection in one dialog with one prompt (L8), and
  one prompt must be one `Confirmed`. Four variants, not the phase doc's five.
- **R4.8's "compile_fail pin" is an in-crate type-check pin**, not a doctest: a
  doctest cannot name the crate-private `Invocation`, and stable Rust has no
  in-crate `compile_fail`. `the_bounded_output_helpers_exist_on_a_read_alone`
  (`process/runner.rs`) is a function that compiles only while a write's calls of
  the helpers' names resolve to a fallback trait (answering `Absent`) and a read's
  to the real helpers; checked by hand that widening the impl to every kind fails
  to compile (E0308 on both write calls). The runner guard requires the pin and
  the helpers' place.
- **The placeholder `describe_destructive` is the destructive roster's one row**:
  the rule "every function naming `Confirmed` is rostered" covers it, so the roster
  is not empty, and the row proves the by-value check on real code until phase 03
  replaces it.
- **The `Consequence` cannot be spelled outside the engine and the model** in
  production code (a forged one would otherwise reach a surface's `by_user`); the
  render crates hold one and ask it for its words. Phase 09 adds a method for the
  amend dialog's decision rather than matching variants.
- **Test code may build tokens** (test modules, `#[cfg(test)]` module files,
  `tests/`): it cannot ship, and phase 03's operations need tokens in their tests.
  A helper under any other `cfg`, or named for tests in production code, fails.
- `Error::GitOutputTooLarge` lost `stranded_locks`, and
  `a_write_the_runner_ends_lists_the_locks_present` its ceiling half: no write can
  cross a ceiling now.
- Each of the six `compile_fail` doctests in `confirm.rs` was checked by hand to
  fail for its own reason (E0599 `clone`, E0382 move, E0308 text, E0451 private
  fields, E0599 `default`, E0277 `From<String>`), the scaffold compiling.

## 2026-10-07 — planned

Planned with `/feature-plan` on `feature/plan-staging-and-commit`. Seven evidence
records under `docs/research/staging-and-commit/`; the user locked L1-L21 over five
rounds, asking for mechanism detail on the patch engine, which became
`patch-mechanics-spike.md` (git 2.30.9 and 2.56.0 agree on every case). The brief
was split: stash and `.gitignore` go to a new packet 5b, `stash-and-ignore`.
Program O3 closed: no backup before a discard, as Fork.

Recon found, and the plan answers: `Confirmed` derives `Clone` and its
constructor is callable from any crate (R1); the runner already feeds stdin, so
no runner change is needed for `apply` or `commit -F -`; a status begun before a
write is drawn after it today (R4.4); a focused text field likely hides the
window's chords and held modifiers today (R7.1, C15 fails first).

Side effect, reported to the user at the time: the git-verbs recon agent's GPG
experiment reached the user's real `gpg-agent` through its socket, wrote two test
private keys into `~/.gnupg/private-keys-v1.d/` and restarted the agent twice; the
agent was refused removing them, and the user was given the commands. No further
GPG experiments were run.
