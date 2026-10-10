# UX / design review: commit box, Git Error, confirmation, Create Branch, remove-lock

Reviewer: independent, read-only. Worktree: staging-and-commit. Judged: the DESIGN, not coverage.
Sources read: commit_box.rs, confirm_dialog.rs, git_error_dialog.rs, create_branch_dialog.rs,
button_order.rs, answer_button.rs, commit_box_pane.rs, create_branch.rs, confirming.rs,
local_changes_actions.rs (acting_line), consequence.rs (all prompt/action/name rendering),
ops/amend.rs, ops/commit.rs (flags), Fork evidence (3 files), PRD R10/R11, state.md.

## Headline

The recurring disease is one root cause shown three ways:

1. **The `Confirmed` type seal forced a "confirmation surface" to exist for amend**, but the UX
   decision (Fork has no amend confirmation) was "don't interrupt". The result is not one
   confirmation but THREE surfaces for one operation (an in-place button+line, a dialog, and a
   prompt pasted into the Git Error dialog), selected by a runtime fact (`needs_force_push`),
   with a line-cap (`max_lines(3)`) that can hide the one sentence that matters.
2. **Prompts are assembled by enumerating the repository's edge cases** (modified / emptied /
   restored / deleted / overwritten / removed-folder / removed-repo / binary / mode change /
   kept-untracked) instead of saying one short, uniform thing; wording then needs conditional
   branches and still has gaps (W3 pending: folder sizes, ignored files).
3. **Failure reporting has no single home**: a refusal/stale/failure appears in the commit box
   note, under the Local Changes lists, in the Create Branch dialog, or in a Git Error dialog
   that says "An unexpected error occurred while performing the git request." even when no git
   request was made.

Counts: HIGH 8, MEDIUM 11, LOW 7. Good items listed at the end.

---

## HIGH

### H1. Amend has three confirmation surfaces chosen by a data fact (root cause of the owner's example)
- Where: `commit_box.rs` `CommitButton::{AmendInPlace, AmendAsking}`, `AmendButton`,
  `AmendSkip`; `commit_box_pane.rs` `pressed`/`amend_confirmed`/`open_amend_dialog`;
  `consequence.rs` `needs_force_push`.
- User experiences: the SAME button "Amend abc1234" either amends instantly (no remote has
  HEAD), or reads "Amend abc1234…" (ellipsis) and opens a dialog titled "Amend commit" (a remote
  has it), or, after a hook failure, a THIRD prompt is pasted as grey text inside the Git Error
  dialog above a button also labelled with a different verb ("Skip pre-commit hooks and
  commit" — which here AMENDS and, if HEAD is published, silently re-confirms the force-push
  warning the user only ever read as body text).
- Root cause: R1.1 says destructive ops need a `Confirmed` built by a "confirmation surface";
  the UX decision says an unpublished amend must not interrupt. Instead of deciding what is
  actually destructive, the packet made the amend BUTTON a token-builder, then needed a
  refusal path in that button (`needs_force_push` -> disabled), a dialog path, and a skip path.
  The chord (Ctrl+Enter from either field) also confirms in place, so the "confirmation" is a
  passive caption the user may never read.
- Clean design: decide once what is destructive about an amend. It is destructive in exactly
  two cases: HEAD is published (history rewrite) or no reflog will keep the old commit
  (unrecoverable). Otherwise it is a recoverable commit-like operation (Show Lost Commits).
  So: (a) ONE surface: ConfirmDialog, shown only in those two cases, same title/button/prompt
  as every other destructive confirm; (b) the ordinary case is a plain button with no token
  theatre (give the engine a `Recoverable` amend that needs no `Confirmed`, or let the commit
  press itself be the token source when the consequence is recoverable — a type-level decision,
  not three UI components); (c) the hook-failure skip is just "Skip hooks and amend" in the
  Git Error footer and re-uses whatever (a)/(b) decided.

### H2. The Amend line repeats what is already on screen and is capped at 3 lines (owner's example) — and the same cap is on EVERY line of the box
- Where: `consequence.rs:493-503` `amend_replaces` -> "Replaces abc1234 'Subject'. The old
  commit stays in Show Lost Commits." / "…can't be recovered afterwards: this repository keeps
  no reflog."; `commit_box.rs` `line()` `.max_lines(3)` + `TextOverflow::Ellipsis` used for the
  stopped line, the replaces line, and the failure notes; `AmendButton` repeats `max_lines(3)`.
- User experiences: id repeated in button AND line (and a third time in the dialog's
  "abc1234 is already on …" sentence); subject repeated though the subject field was just
  filled with HEAD's message. A long subject pushes "can't be recovered" past line 3 and it is
  cut with an ellipsis — the only sentence that matters. The same `line()` also cuts
  `Amend's staged list could not be read…: {git's why}` and
  `What Amend would replace could not be read: {why}`, i.e. git's own error text, with no way
  to see the rest.
- Root cause: the line carries IDENTITY (id + subject) that the button/field/history already
  carry, plus a reassurance sentence; identity is unbounded, so a cap was added. A cap on
  text that can contain the safety-critical clause is a patch.
- Clean design: the line states ONLY the exception and never repeats identity:
  nothing when the old commit stays findable (the normal case; Show Lost Commits is already a
  toolbar toggle), and one fixed-length sentence "The old commit can't be recovered: this
  repository keeps no reflog." when not. The button already says "Amend abc1234". No subject,
  no cap needed. Error notes: a fixed head + a "Details" affordance or the existing Git Error
  dialog, never an ellipsised git message.

### H3. "Git Error" dialog is used for failures that are not git errors, and says so wrongly
- Where: `git_error_dialog.rs:29-31` GIT_ERROR_TEXT; `create_branch.rs:~400-420` maps
  `Stale | Refused | MayHaveTakenEffect | Incomplete | NotRun` to a Git Error dialog with an
  EMPTY command line and the message as the only output line.
- User experiences: after confirming Create Branch -> Discard, if the state moved since the
  confirmation (Stale), a dialog titled "Git Error": "An unexpected error
  occurred while performing the git request." / "Error Details:" / (blank) / the real
  explanation. Nothing was requested of git; the message is a safety refusal and is designed
  behaviour, not "unexpected". Contrast: the same ending for a staging write is said under the
  lists ("Staging: nothing was written. …"), and for commit/amend (`commit_box_pane::write_ended`)
  it is silently dropped from the box (the arm is empty) and surfaces only under the Local
  Changes lists.
- Root cause: Fork has one error modal for git failures; Cairn added refusal kinds (Stale,
  Refused, NotRun…) with no designed home and routed them to whichever surface existed in each
  feature.
- Clean design: one outcome vocabulary, two homes. A git failure -> Git Error dialog (Fork's).
  A Cairn refusal ("changed since you confirmed", "operation in progress", "not run: closing")
  -> a normal in-context message next to the control the user used (commit box note; dialog's
  own refusal row), worded as a refusal, never inside the Git Error modal.

### H4. Create Branch -> Discard: Cancel on the confirmation throws away the whole dialog; the primary button hides the destructiveness
- Where: `create_branch.rs` `confirm_arrived` (`state.open = None; kept_name = …`);
  `create_branch_dialog.rs` primary button `CREATE_AND_CHECKOUT_CAPTION` regardless of choice.
- User experiences: picks a commit row -> New Branch… -> types name -> ticks checkout -> selects
  "Discard" -> presses "Create and Checkout" -> the dialog VANISHES and a second dialog,
  titled "Discard changes", opens. Cancel there returns to the history with nothing open: the
  target commit, the checkout tick's context, and the "Discard" radio are gone; only the name
  is offered back on the next opening (and only if they open it again at the same commit).
  Return in the name field with "Discard" selected also goes straight to this flow. The first
  dialog's button reads like the safe variant ("Create and Checkout") with no sign it will
  delete files.
- Root cause: the confirmation seal needs a `Consequence` computed by the engine, so the dialog
  is closed to ask it, rather than the Create Branch dialog hosting the confirmation (or
  re-opening).
- Clean design: keep Create Branch open underneath; the confirmation is a layer over it and
  Cancel returns to it (`cancel` = back one step). And when "Discard" is selected the primary
  button says what it will do ("Discard Changes and Check Out…", i.e. the same words the
  confirmation uses), so the second dialog is a confirmation of an action the user can read.

### H5. Prompts enumerate every edge case by hand; terms drift between the four destructive prompts
- Where: `consequence.rs` `discard_files_prompt` (modified/emptied/restored/untracked + mode
  change(s) + binary), `checkout_discarding_prompt` (modified/new-file deleted/restored/
  overwritten/removed folder/repository/kept untracked, three conditional endings).
- Example (Create Branch Discard, its own doc comment): "Do you want to create branch 'topic'
  at 1a2b3c4, check it out and discard the changes in 3 files (a.rs, b.rs and c.rs)? 2 modified
  (14 lines), 1 new file deleted (3 lines), 1 untracked file overwritten (2.1 KiB). Deleted
  because the branch has a file there: folder d/ (4 untracked files) and repository vendor/lib/
  with its history. Other untracked files are kept. You can't undo this action." — five
  sentences, the longest ~45 words, with the most destructive part (a whole directory / a
  nested repository with its history) in the THIRD sentence after "3 files", with NO size for
  it (W3, still undecided) and nothing about ignored files git -f also overwrites.
- Term drift for the same effect on a file: "discard" (lines/files), "deleted" (untracked file,
  new file), "restored" (deleted file), "emptied" (intent-to-add), "overwritten" (untracked
  file under checkout), "Remove" (lock), "Replaces" (amend), "modified (3 lines)" whose "lines"
  is changed lines, plus raw octal "(100644 to 100755)" in prose. A user cannot learn one
  meaning.
- Root cause: the prompt is a report of the engine's classification of cases rather than a
  statement of the consequence in the user's terms.
- Clean design: one sentence frame for every destructive confirm: "<Verb> <what>. <worst
  consequence>. You can't undo this." with ONE destructive verb family (Discard / Delete) and a
  fixed noun grammar. The enumeration of cases goes in a collapsible list ("Show files") under
  the dialog for those who want it, not in the sentence. Whole-directory/repo deletion is the
  headline when present, with size. (Fork: "Do you want to discard all your changes in the
  selected files? You can't undo this action." — two short sentences.)

### H6. Pre-filled MERGE_MSG keeps git's `# Conflicts:` comment lines VISIBLE and COMMITS them if left
- Where: PRD R10.8; `commit_box_state.rs` merge fill; `ops/commit.rs` passes no `--cleanup`
  (`-F -` default => comment lines are kept).
- User experiences: after a conflicted merge the description reads "Merge branch 'x'\n\n#
  Conflicts:\n#\ta.txt…"; if the user does not notice and press Delete on those lines, the merge
  commit message contains them. `git commit` (editor) and `git merge --continue` strip them;
  Cairn's box is the replacement for the editor, so the user's mental model is git's.
- Root cause: "message exactly as git wrote it" was applied to an editor-template file, not to a
  final message (MERGE_MSG is a template; its comments are for the person editing).
- Clean design: when pre-filling from MERGE_MSG, drop `#` comment lines (what git's cleanup=
  strip would do in an editor session) — or commit that pre-fill with `--cleanup=strip`. Either
  removes the trap; the PRD's "left visible for the user to delete by hand" is a user decision
  but violates "git parity is critical" (memory) and gives the user a chore for no benefit.

### H7. A third button implementation and three focus rules across the dialogs
- Where: `answer_button.rs` ChoiceButton (Confirm, Git Error Close), Freya `Button` (Git Error
  Skip, Create Branch Create/Cancel), `commit_box.rs` `control` (box buttons, AmendSkip, Cancel).
- User experiences: in the SAME Git Error dialog, "Close" is a bordered rect with a focus ring
  that holds initial focus and "Skip pre-commit hooks and commit" is a Freya Button with a
  different look and no initial focus; for a failed AMEND the skip is NOT in the footer at all
  but in the dialog body, under the prompt text, as a `control` button — footer has only
  "Close". Create Branch's Cancel is a Freya Button, Confirm's Cancel is a ChoiceButton.
  Focus: Confirm -> Cancel; Git Error -> Close; Create Branch -> the name field. Button order
  is platform-dependent (Linux primary first, macOS last) which is Fork's, fine, but the
  skip-on-amend placement breaks the order rule entirely.
- Root cause: `AmendSkip` had to live in `commit_box.rs` (a roster'd "confirmation surface") so
  it is a body child, not a footer action.
- Clean design: one dialog button component; a destructive/secondary action always sits in the
  footer in the platform order, whatever component builds its token.

### H8. The in-place amend "confirmation" confirms on the commit chord
- Where: `commit_box.rs` `chord` EventHandler -> `confirm_in_place`.
- User experiences: Ctrl+Enter in the subject/description (the habit for "commit") amends HEAD
  immediately when no remote has it, including when the line says "can't be recovered:
  this repository keeps no reflog". The line is read only if looked at; the shortcut path never
  shows it as a decision.
- Root cause: same as H1.
- Clean design: the unrecoverable case (no reflog) gets a real dialog; the rest is recoverable
  and needs no confirmation at all. Then the chord and the button are identical and honest.

---

## MEDIUM

### M1. "Amend" checkbox disabled with no reason; the reason exists only for the other operations
- `commit_box_pane.rs`: `amend_enabled = born && operation.is_none()`. Unborn HEAD or a merge
  disables the toggle silently, while rebase/am/cherry-pick/revert get the line "Committing is
  unavailable while … is in progress." A merge in progress: commit works, amend greyed, no word.
- Clean: every disabled control carries its reason (tooltip/line); one `why_unavailable` string
  per operation state.

### M2. Four button states for amend, two of them transient text-only
- `CommitButton::{ReadingAmend, AmendUnreadable(String)}`: the button is a disabled grey
  "Amend" (caption) with the real status as an a11y name (`READING_AMEND`, `why`), invisible to a
  sighted user; the unreadable reason is shown in the note (cut by H2's cap).
- Root cause: the consequence is computed asynchronously on every status arrival while Amend is
  ticked (pushed-check walk), so the UI has "reading" and "unreadable" modes. Each refresh
  re-disables the button (`is_reading_amend`) — a press can be lost mid-read.
- Clean: compute lazily at press time (the engine already re-checks at write), show nothing
  while idle; or read once on the tick and again only on HEAD/remote change, not per status.

### M3. Two different Amend dialogs/texts for the pushed case, and "force push" advice with no push in the app
- `consequence.rs` `amend_force_push`: "abc1234 is already on origin/main. Sharing the amended
  commit needs a force push." then "Replaces abc1234 'S'. The old commit stays in Show Lost
  Commits." — three occurrences of the id in three sentences; Cairn has no push. The user is told
  how to proceed with an operation the app cannot do, and is not told the actual consequence
  (others' clones diverge). Two wording variants (Upstream-named vs "a remote"), times two
  reflog variants = four amend prompts.
- Clean: one fixed pushed sentence: "abc1234 is already on origin/main. Amending rewrites it;
  you will need to force-push." (ids once) + the recoverability exception only when true.

### M4. Overloaded reuse of the "Discard changes" title for Create Branch
- `create_branch.rs` `DISCARD_BEFORE_CHECKOUT_TITLE = local_changes_actions::DISCARD_TITLE`
  ("Discard changes"). The prompt opens "Do you want to create branch …", the button says
  "Discard Changes and Check Out", the title says "Discard changes". Three framings for one
  operation, picked to match Fork's file-discard dialog, not the operation.

### M5. Remove index.lock prompt carries the discard boilerplate
- `consequence.rs:319-326`: "Remove /abs/path/.git/index.lock? It was last changed 4 minutes
  ago and holds 0 bytes. Another program may still own it: removing a lock a running git holds
  can corrupt the index. You can't undo this action."
- Nothing of value is lost by removing a stale lock; "You can't undo this action" is
  uniform boilerplate (it reads alarming and is untrue in the sense the user cares about). The
  real risk (a live git) is stated second. Absolute path is long; the button already says
  "Remove 'index.lock'".
- Clean: "Remove index.lock? No git Cairn started is running, but another program might be using
  it; removing a live lock can corrupt the index." Drop the "can't undo" tail and byte count.

### M6. Git Error dialog: output is virtualized, but the dialog has fixed 240px height, and amend's prompt pushes it
- `git_error_dialog.rs` OUTPUT_HEIGHT 240 + AmendSkip's prompt (up to the 4-sentence amend
  prompt, uncapped here) stacked below: dialog height now depends on whether an amend failed.
  Fork's modal is "small and resets its size" (a known Fork complaint) — not improved.

### M7. Inconsistent refusal/where-it-appears, per feature
- Commit box: `note` (Amend reads), the `stopped` line, ending silently dropped (H3).
  Local Changes: `acting_line` (a long switch: "nothing was written.", "failed:", "may have
  taken effect in part:", "did not take every file — … kept as they were:"). Create Branch:
  `refusal` row left of the buttons (cut at `max_lines(2)`, so git's check-ref-format words can
  be ellipsised) plus Git Error. Four patterns, each with its own line caps.

### M8. Dialog vs in-place rule is data-dependent and therefore unpredictable
- Discard: always dialog. Create Branch: dialog only for "Discard", otherwise no confirmation.
  Amend: in place when unpublished, dialog when published, body text in Git Error on skip.
  Remove lock: dialog from the activity popover. The user cannot predict whether an action
  confirms.
- Clean: one rule — "irreversible or rewrites shared history => dialog, always the same
  component; recoverable => no confirmation".

### M9. Create Branch "Local changes:" shows two of Fork's three radios and a deviation is stated only in code docs
- Documented (decision 2), but the dialog gives no sign "Stash and reapply" exists in Fork; also
  `has_changes` decides via `entries.first()` being Changed/Conflicted ("git status lists those
  before any untracked path") — a dependence on status ordering, not a query. Untracked-only
  trees never show the group, though `checkout -b` can still be refused over untracked files
  (then Git Error). LOW-MEDIUM; a hack worth a named `status.has_tracked_changes()`.

### M10. "Overwritten", "Removed" and "kept_untracked" exist because checkout -f semantics were mirrored literally
- `ChangeLoss::{Overwritten,Removed}`, `RemovedKind::{Directory,Repository}`, `kept_untracked`
  with three conditional wordings (`(0,_) / (_,0) / _`) — a whole prompt-grammar for a case that
  rarely occurs because the user would normally use "Don't change" (git refuses) or commit/stash
  first. Fork's Discard simply discards (Tracker #419: without confirmation). Root: Discard
  was implemented as `checkout -f` (UNVERIFIED in Fork's own evidence) instead of a tracked-
  changes-only discard, creating untracked-file loss that then had to be explained.
- Clean: discard only tracked changes (`git checkout -b` after `git restore`-style discard, or
  `reset --hard` equivalent without touching untracked); then no "Overwritten/Removed/kept" at
  all — git's refusal for untracked collisions goes in the Git Error as for "Don't change".

### M11. Elapsed/busy line wording drifts from its own doc
- `commit_box.rs` `Busy.what` doc: "Committing", "Amending", "Waiting for staging 2 files"; the
  pane builds "Committing (waiting)" and then `format!("{}…")` -> "Committing (waiting)…".
  The Create Branch dialog separately says "Waiting for {awaited}…". Two phrasing schemes for
  "queued behind a write".

---

## LOW

- L1. `Busy` doc comment is stale vs. implementation (above).
- L2. Counter: Fork's characters-left logic with a hard limit at 70 (red) and soft 50 — fine, but
  the `a11y_alt` reads "{n} characters left" even when negative ("-3 characters left").
- L3. `commit_caption(0)` "Commit" shown disabled with nothing staged but enabled during a merge:
  caption does not say "Commit merge"/"Conclude merge" though that is what it does (Fork: unknown).
- L4. MERGE_MSG "once per merge, a cleared message stays empty" is a flag added from INFERRED
  vendor behaviour (Fork evidence §2 says "not documented"); correct decision-labelling, but it
  is an invented rule where Fork's behaviour is unknown.
- L5. Recent Commit Messages: menu shows subject only of each; two commits with the same subject
  are indistinguishable (no id/date). Fork shows the same, OK, but ↑/↓ recall rules (empty or
  last-recalled only) are a state machine the user cannot see.
- L6. `ConfirmDialog` width is a fixed 520 and no maximum height or scroll is set in
  `confirm_dialog.rs` for a prompt that can run five sentences and name long quoted paths.
- L7. "Create and Checkout" vs title "Create Branch" vs Fork's two older variants: follows the
  Windows capture; order/labels per platform otherwise. Fine.

---

## GOOD (fair accounting)

- Focus on Cancel in every destructive confirmation (a considered deviation, Fork #1080), and
  Escape / outside-press cancel; answered-once guard so one acknowledgement = one token.
- Prompts and button labels are rendered from the `Consequence` value (no typed-apart text);
  untrusted path text is escaped (control chars, bidi) — genuinely good safety design.
- Button order per platform is data (`button_order.rs`), single function, tested.
- The Git Error dialog follows Fork's wording; output virtualized, opens at end where hooks say
  why; skip offered only where a hook exists; amend skip carries the original consequence.
- Hook-failure skip is "this once" (`--no-verify` never a preference); draft survives.
- Create Branch's name checking is inline, pre-git, with git's own reasons, never in the Git
  Error dialog (decision B); a failed create keeps the name (Fork 1.82 parity).
- "Commit N Files" / counter / ruler / Recent Commit Messages match Fork's evidenced behaviour;
  merge-in-progress commits with nothing staged on Fork's evidence (Tracker #90).
- Credential prompt during writes is titled by its own operation (tested), not "git".

---

## Where behaviour is INVENTED or differs from Fork without evidence
- In-place amend line/button (Fork has no amend confirmation; Windows warns when pushed, text
  UNVERIFIED) — invented, decision L12.
- "Replaces <id> '<subject>'" and Show Lost Commits sentences — Cairn-only.
- MERGE_MSG comment lines visible and committable; once-per-merge flag; Amend disabled during
  merge (Fork's behaviour not documented).
- Create Branch "Discard" with its own confirmation (Fork performs it silently, #419) — a
  considered safety deviation, but the prompt grammar it created (H5, M10) is Cairn's.
- AmendSkip in the Git Error body; Stale/Refused in the Git Error modal.

## Priority fix order (recommendation)
1. Decide amend's destructive criteria; collapse to one surface (H1, H2, H8, M3).
2. Give failures one vocabulary and a home (H3, M7); stop reusing the Git Error modal for refusals.
3. Make Create Branch's Discard a layer, not a replacement (H4); drop checkout -f semantics if
   possible (M10).
4. Rewrite the destructive prompts to one sentence frame with an optional expandable list (H5, M5).
5. Strip MERGE_MSG comment lines (H6).
