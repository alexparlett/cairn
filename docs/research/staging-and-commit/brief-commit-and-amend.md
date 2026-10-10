# Design brief: the commit box and amend (redesign)

Date: 2026-10-10. Read-only research over the `staging-and-commit` worktree; nothing in the
repository was changed. Inputs: the review (`review/ux-commit-and-dialogs.md` H1–H8, M1–M3,
M6, M8, M11, L2–L5; `code-engine.md` M1/M4/M5; `code-app-ui.md` M3; `ux-activity-and-chrome.md`
H1 and the wording sprawl), the Fork records in `docs/research/staging-and-commit/`
(`fork-staging-and-commit.md` §5, §7, §8; `fork-merge-and-amend-evidence.md`), PRD R6, R10
(and the locked decisions L9, L12, L25 in `docs/work/staging-and-commit/brainstorm.md`), and
the code: `crates/cairn-ui/src/commit_box.rs`, `crates/cairn-app/src/commit_box_pane.rs`,
`crates/cairn-app/src/commit_box_state.rs`, `crates/cairn-model/src/consequence.rs`,
`crates/cairn-model/src/operation_in_progress.rs`, `crates/cairn-ui/src/git_error_dialog.rs`.

Fresh research this round: both release-notes pages (Mac
<https://git-fork.com/releasenotes>, Windows <https://git-fork.com/releasenoteswin>), the blog
index (<https://fork.dev/blog/posts/> — nothing on the commit box), GitHub's issue search on
`fork-dev/Tracker` and `fork-dev/TrackerWin` (HTML search; the API was rate-limited), and
`git help commit|merge|cherry-pick|config` from git 2.56.0. A scratch-repository experiment on
`MERGE_MSG` cleanup was refused by the permission system, so git's run-time behaviour below is
from its manual and source knowledge and is marked so.

Confidence labels: **VERIFIED** (a vendor statement, a release note, a screenshot or git's
manual says it), **INFERRED** (reasoned from verified pieces), **UNKNOWN**.

---

## 1. Fork's behaviour, step by step

### 1.1 Writing the message

1. The box sits under the diff in Local Changes: `Commit subject` (one line) with a
   characters-left counter and a `≡` icon at its right; `Description` below; `Amend` check box
   bottom-left; the commit button bottom-right. VERIFIED (`fork-staging-and-commit.md` §5,
   screenshots in Tracker #901, #1371).
2. The counter shows characters left before a soft limit of 50 (grey), goes negative past it,
   and turns red past 70; it never blocks. VERIFIED (vendor, Tracker #633; Win blog 1.17 shows
   `-38` in red). The limit is configurable (Mac 1.0.89, Win 1.43). VERIFIED.
3. `≡` opens `Recent Commit Messages`: the latest 10 commits on the current branch; choosing
   one fills subject and description. ↑/↓ in the subject cycle them. VERIFIED (Tracker #720,
   #587; Win RN 1.38). When exactly ↑/↓ recall (only in an empty subject?) is UNKNOWN.
4. The draft survives switching views (Win 1.54 fixed "Amend message disappears after
   switching between Changes/All Commits"). VERIFIED.
5. Lines starting with `#` that the person types are **committed**: Fork runs
   `git commit --file=<temp>` (Tracker #2485), whose default cleanup keeps commentary, and Fork
   overrides `core.commentChar=^`. VERIFIED — Tracker #180, open since 2017
   (<https://github.com/fork-dev/Tracker/issues/180>); the vendor in 2021: "I was incorrectly
   sure this is how it works by default."

### 1.2 Committing

6. Button: `Commit N Files`; plain `Commit`, disabled, with nothing staged; disabled with an
   empty subject. VERIFIED (Mac RN 1.0.9; Tracker #901, #1490). ⌘Return / Ctrl+Enter presses
   it. VERIFIED (vendor shortcut list).
7. While it runs: the commit runs as a background activity; the button is disabled (Win RN
   1.29 "Don't freeze UI on long commit"); the toolbar status box spins and names it; hook
   output streams into the Activity Manager. VERIFIED (TrackerWin #125; Mac RN 1.0.69, 1.0.81).
8. Cancel: the Activity Manager's × cancels a running operation (Mac RN 1.0.80 "Cancel long
   operations immediately"). VERIFIED for operations in general; whether the commit box itself
   has a Cancel is UNKNOWN — no capture shows one. INFERRED: no Cancel in the box.
9. On success the draft is cleared and Amend unticks (Tracker #997). VERIFIED. No message is
   shown (INFERRED from every capture).
10. Detached `HEAD`: Fork warns when committing (Mac RN 1.0.73/1.0.85; Tracker #487, vendor
    "already available in 1.0.73"). VERIFIED that a warning exists; its form and wording are
    UNKNOWN.

### 1.3 A hook fails

11. The `Git Error` modal: `An unexpected error occurred while performing the git request.`,
    `Error Details:` with the command and output, buttons `Skip pre-commit hooks and commit`
    and `Close`; Esc closes; nothing is the default. VERIFIED (screenshot Tracker #776; vendor
    Tracker #2173).
12. The skip is offered only where Fork detects a `pre-commit` or `commit-msg` hook — in 2019
    only under `.git/hooks`, not `core.hooksPath` (Tracker #776). VERIFIED for 2019; a Husky
    user in Tracker #948 reports the failure dialog offering only Close
    (<https://github.com/fork-dev/Tracker/issues/948>) — INFERRED that `core.hooksPath` hooks
    still get no skip.
13. `--no-verify` up front is refused, repeatedly ("This is a rarely used feature, I think we
    don't need a UI for it", vendor, Tracker #948, 2020). VERIFIED.
14. The draft is kept after a hook failure (Mac RN 1.0.83 fixed "Commit message not cleared
    after skipping commit hook"); a skip in amend mode amends (Mac RN 2.7 fixed "Amend checkbox
    remains enabled after skipping pre-commit hook"). VERIFIED. The skip's caption in amend mode
    is UNKNOWN; INFERRED it is the same caption.
15. ANSI codes print raw; the modal is small (Tracker #1218, #1636). VERIFIED.

### 1.4 Amend

16. Ticking `Amend` loads the last commit's message and lists its files among the staged ones
    (Mac RN 1.0.44 "Show changes from the latest commit in amend mode"). VERIFIED.
17. The button is enabled at once in amend mode; an amend with nothing new is possible (open
    request for a warning, Tracker #2499). VERIFIED. **The button's caption in amend mode is
    UNKNOWN** (no capture in the record).
18. Mac asks nothing before amending. VERIFIED by absence: no release note, no issue (the
    Tracker searches "amend pushed" and "amend warning" return nothing relevant). Windows
    since 1.91 "Show warning when amending already pushed commit". VERIFIED. Its wording is
    "commit is already pushed" per a false-positive report whose author says amending "works
    just fine if you ignore the warning" (<https://github.com/fork-dev/TrackerWin/issues/2561>,
    open, no vendor reply). INFERRED: a warning the person can proceed past; whether a modal
    or an inline note is UNKNOWN.
19. Amend is allowed while an interactive rebase stops on a commit (Mac RN 1.0.72, Win RN
    1.26 "Allow to amend commits during interactive rebase") and is not offered on a conflict
    during one (Win RN 1.94 fixed "Amend mode mistakingly enabled on a conflict during IR").
    VERIFIED. During a merge: UNKNOWN.

### 1.5 A merge, rebase, cherry-pick or revert in progress

20. A merge in progress: the box pre-fills git's message (`MERGE_MSG`; Mac RN 1.0.16, vendor
    Tracker #155) and Commit is enabled with nothing staged, concluding the merge (Tracker #90,
    1.0.57). VERIFIED. A global merge-conflict bar with Abort shows in both views (vendor,
    Tracker #222: "the merge conflict bar is global"). VERIFIED that it exists; its text is
    UNKNOWN.
21. The pre-fill is replaced only until the person edits it (vendor, Tracker #61); it goes when
    the merge ends (Mac 1.0.78, Win 1.33). VERIFIED. A cleared message: UNKNOWN.
22. `MERGE_MSG`'s `# Conflicts:` lines: shown and, if left, committed (Tracker #180 thread:
    with Fork's `^` commentChar, cherry-pick writes `^ Conflicts:` which is stripped, but
    rebase writes `#` lines that "end up polluting my commit messages", 2024). VERIFIED that
    `#` conflict lines leak in rebases; for a merge, INFERRED the same (`#` lines kept under
    `--file`). An open defect since 2017, not a design.
23. A cherry-pick or revert in progress pre-fills the original message and commits (Mac RN
    1.0.16 "Use original git commit message when merge/revert/cherrypick is in progress"; Win
    1.49 "Improve layout for cherry pick conflicts"). VERIFIED that it pre-fills; INFERRED the
    commit button concludes the pick.
24. A rebase in progress: the message field is disabled and the actions are `Continue
    Rebase`, `Abort`, `Skip` (Tracker #180 comment, 2024: "Fork disables the commit message
    field when resolving a conflict during a rebase"; TrackerWin #2646). VERIFIED. The message
    shown is the commit's being replayed (Mac 2.38 / Win 1.92 fixed "Commit message is not
    updated during rebase"). VERIFIED.

### 1.6 Where Fork says things

25. Git failed → the `Git Error` modal (one modal for every failing git request, §8). Running
    → the toolbar status box and the Activity Manager. Success → nothing. VERIFIED.
    A refusal that is not git's: no evidence of a separate surface (UNKNOWN).

---

## 2. Git's behaviour where it bears

- **Cleanup** (`git help commit`, `--cleanup`): `default` is "Same as strip if the message is
  to be edited. Otherwise whitespace." `strip` removes commentary; `whitespace` keeps it.
  `commit.cleanup` changes the default. VERIFIED. So `git commit -F -` (Cairn) and
  `--file=<temp>` (Fork) keep `#` lines; the editor (`git commit`, `git merge --continue`)
  strips them. VERIFIED from the manual.
- **What the conflict lines are**: `git help config` (`core.commentChar`) calls them "the
  conflicts comments added to the commit message by cherry-pick, merge, rebase and revert".
  VERIFIED: git intends them as editor commentary, never as message text. They start with
  `core.commentChar`/`core.commentString` (default `#`), not always `#`. VERIFIED. With
  `--cleanup=scissors` a scissors line is appended to `MERGE_MSG` on a conflict (`git help
  merge`). VERIFIED.
- **`git stripspace --strip-comments`** applies exactly git's `strip` cleanup to stdin,
  honouring the comment character. VERIFIED (git-stripspace is documented for this; not run
  here).
- **Concluding**: "Use git commit or git merge --continue to seal the deal" (`git help merge`).
  VERIFIED. `git commit` also concludes a single cherry-pick or revert (removes
  `CHERRY_PICK_HEAD`/`REVERT_HEAD`) and, for a cherry-pick, keeps the picked commit's author.
  INFERRED from git's source (`builtin/commit.c` reads the author from `CHERRY_PICK_HEAD`);
  needs a real-git test before relying on it. A multi-commit sequence (`.git/sequencer`) still
  needs `git cherry-pick --continue` afterwards. INFERRED.
- **Amend during an operation**: git refuses `--amend` in a merge ("You are in the middle of a
  merge -- cannot amend.") and in a cherry-pick. INFERRED from git's source; Cairn already
  refuses before git runs. During a rebase stopped on `edit`, amend is the normal path.
  VERIFIED (that is what `edit` is for).
- **Amend and the reflog**: `commit --amend` moves the branch and `HEAD`; with
  `core.logAllRefUpdates` true (the default with a working tree) the old tip stays in both
  reflogs. VERIFIED (PRD R10.6, checked with git 2.56 by phase 01). git never warns about a
  published commit; the next push is rejected as non-fast-forward. VERIFIED.
- **Hooks**: `--no-verify` bypasses `pre-commit` and `commit-msg` only (`git help commit`).
  VERIFIED. Hooks may also be declared in configuration (`hook.<name>.command` /
  `hook.<name>.event`, in `git help config` of 2.56) — not only as files. VERIFIED: a
  file-existence check cannot see every hook git runs on new gits.
- **A cancelled commit**: git holds `index.lock` while `pre-commit` runs; on SIGTERM git's
  lockfile handlers remove it, on SIGKILL it stays. Cairn already sends SIGTERM then SIGKILL
  after 2 s (`process/registry.rs`). Whether the commit landed is decided by whether `HEAD`
  moved — a fact, readable after the reap. INFERRED.

---

## 3. Cairn today, step by step, and where it diverges

| # | What the person sees today | Divergence / defect |
|---|---|---|
| 1 | Subject + counter + `≡`, Description with ruler, `Amend`, `Commit N Files`. | Matches Fork. Counter's accessible name reads "-3 characters left" (L2). |
| 2 | `≡` menu, ↑/↓ recall while subject empty or holding the last recalled. | Matches Fork as far as known. |
| 3 | Commit runs: a busy line "Committing" / "Committing (waiting)…" + elapsed + Cancel. | Fork has no busy line or Cancel in the box (INFERRED); the wording "(waiting)…" is one of four "queued" phrasings (M11). |
| 4 | Cancel → the activity entry says "may have taken effect"; the box returns idle. | No `Cancelled` ending; a user's cancel reads as a fault (`ux-activity` H1). |
| 5 | Ticking Amend: fills an empty draft with HEAD's message; Local Changes swaps its data source (`status_arrived_amending` → `Request::Amending`). Each status re-reads the amend consequence, so the button goes "Amend" (disabled, a11y "Reading what Amend would replace…") between reads (M2). | Matches Fork on the message and lists; the per-status consequence read is Cairn's own and can swallow a press. |
| 6 | Amend unpublished: button `Amend abc1234` above "Replaces abc1234 'Subject'. The old commit stays in Show Lost Commits." (capped at 3 lines). Press or Ctrl+Enter amends — the press is the "confirmation". | Fork shows nothing. The line repeats id and subject, and the cap can cut "can't be recovered" (H2). The chord "confirms" a caption nobody read (H8). |
| 7 | Amend published: `Amend abc1234…` opens a dialog "Amend commit": "abc1234 is already on origin/main. Sharing the amended commit needs a force push. Replaces abc1234 '…'. …" | Windows Fork warns (VERIFIED), Mac does not. Id three times; advice about a push Cairn cannot do (M3). |
| 8 | Hook fails (commit) → Git Error with a Freya `Button` "Skip pre-commit hooks and commit" beside a `ChoiceButton` "Close". | Fork's dialog; two button components (H7). Skip offered from Cairn's own `X_OK` re-derivation (`code-engine` M4), blind to config hooks. |
| 9 | Hook fails (amend) → Git Error whose **body** holds the amend line again plus an `AmendSkip` button that builds a new token; footer has only Close. | Third surface for one confirmation (H1); breaks button order (H7); dialog height varies (M6). |
| 10 | Refused before git runs (Stale/Refused/NotRun/Incomplete) → `write_ended` drops it; it surfaces under the Local Changes lists. | No home at the control used (H3, M7). |
| 11 | Merge in progress: an empty draft is filled once per merge with `MERGE_MSG` **including `# Conflicts:` lines**, committed if left (R10.8, decision E). Amend greyed with no reason (M1). | Matches Fork's open defect (Tracker #180), diverges from git's editor flow, which strips them (H6). |
| 12 | Rebase / am / cherry-pick / revert: box disabled, "Committing is unavailable while a cherry-pick is in progress." | Fork commits a cherry-pick/revert with its original message (RN 1.0.16) and git's `git commit` concludes one — a divergence for those two. Rebase/am: Fork offers Continue/Abort/Skip that Cairn has no verbs for. |
| 13 | Success: draft cleared if it was the one taken, Amend unticked, `StopAmending` sent. | Matches Fork. |

---

## 4. Proposed clean flow (storyboard)

### Rules this flow follows (stated once)

- **R-A. One confirmation rule.** An operation that rewrites history someone else has, or
  leaves nothing to recover from, asks the one confirmation dialog; anything recoverable asks
  nothing. For amend: published `HEAD` → dialog; no reflog will be written → dialog;
  otherwise → none, and nothing on screen says so (Show Lost Commits is the recovery, and is
  already a menu item).
- **R-B. Ask at the press, not before.** What an amend costs is read when Amend (the button)
  is pressed, inside the same job that then runs it — never per refresh. No "reading" state.
- **R-C. Two homes for an ending.** git ran and failed → `Git Error` (Fork's, unchanged text).
  Cairn refused before or instead of git → one line under the commit button (the control
  used), worded as a refusal, replaced by the next press. Cancelled → nothing in the box; the
  activity entry reads `cancelled`. Made → nothing.
- **R-D. No caps on text a person needs; short text instead.** Every line in the box is one
  fixed sentence plus at most one name; git's own text only ever appears in `Git Error`, which
  scrolls.
- **R-E. The message committed is the message shown.** Pre-fills are cleaned the way git's
  editor would clean them before they are shown; what the person then types is committed under
  git's `-F` rules, as Fork does.
- **R-F. One unconsumed token.** A destructive operation that fails before writing anything
  hands its `Confirmed` back in the error; the skip reuses it, so a confirmation is asked once
  per intent.

### Storyboard

**S1. Idle, nothing staged.** On screen: `Commit subject` field, counter `50` (grey), `≡`;
`Description` field with ruler; `☐ Amend` bottom-left; `Commit` (disabled) bottom-right. No
other text.

**S2. Staging and typing.** Button reads `Commit 3 Files`, enabled once the subject is not
blank. Counter counts down; past 50 it shows `-4` grey; past 70 red. Accessible name: "4
characters over" when negative, "12 characters left" otherwise. `≡` opens `Recent Commit
Messages` (subjects of the latest 10 on the branch); choosing one fills both fields. ↑/↓ as
today.

**S3. Commit pressed (or ⌘Return / Ctrl+Enter — identical in every state).** The button reads
`Committing…` and is disabled; right of it, `Cancel`. Fields stay editable (the draft typed now
is the next message). The status box names the operation; the activity popover streams the
hooks' output. If another write is ahead of it: `Waiting for staging…` in the button's place —
the same "Waiting for <operation>…" phrase every queued write uses, with `Cancel` (a queued
commit cancels without starting).

**S4a. Made.** Fields clear (only if unchanged since the press), Amend unticks, the history
gains the row. Nothing else.

**S4b. Cancel pressed.** Button returns to `Commit 3 Files`; draft kept. The activity entry
reads `cancelled`. If `HEAD` had already moved to the new commit when git was ended, the
outcome is S4a instead (decided by reading `HEAD` after the reap, not by guessing). If a lock
file was stranded, the existing lock offer handles it (out of this flow).

**S4c. A hook (or anything git ran) fails.** `Git Error` opens, Fork's: title `Git Error`,
`An unexpected error occurred while performing the git request.`, `Error Details:`, the
command and git's output (ANSI stripped, virtualized, opened at the end, fixed size whatever
the case). Footer in platform order: `Skip pre-commit hooks and commit` and `Close` (focus on
`Close`, Esc closes). Same two buttons, same component, whether it was a commit or an amend.
Draft kept.

**S5. Skip pressed.** The same message is committed with `--no-verify`, this once — back to
S3. For an amend, it amends (S8 onward) reusing the confirmation the person already gave, if
any (R-F); no prompt text appears in this dialog.

**S6. Refused before git runs** (e.g. `HEAD` moved since a confirmation, an operation began,
a non-UTF-8 `i18n.commitEncoding`). One line under the button, e.g. `Not committed: a rebase
started.` / `Not amended: HEAD moved since you confirmed.` / `Not committed: i18n.commitEncoding
is Shift_JIS; Cairn writes UTF-8 only.` Draft kept. The line goes at the next press or edit.

**S7. Amend ticked.** An empty draft fills with `HEAD`'s message (a typed draft is set aside
and returns when Amend is unticked, as today). The staged list shows the last commit's files
too (as Fork). Button reads `Amend abc1234` (enabled with a subject, even with nothing new
staged — Fork). No line under it.

**S8. Amend pressed (or chord).** Button reads `Amending…` with `Cancel`. The job reads what
amending costs, then:
- recoverable (unpublished, reflog written) → git runs; S4a/S4b/S4c as for a commit.
- published or no reflog → the job ends without running and the confirmation dialog opens
  (S9). Button returns to `Amend abc1234`.

**S9. The amend confirmation** (the one dialog component every destructive operation uses).
Title `Amend Commit`. Text, one or two fixed sentences, the id once:
- published: `abc1234 is already on origin/main. Amending it rewrites history others may
  have.`
- no reflog: `abc1234 can't be recovered after this: this repository keeps no reflog.`
- both: both sentences.
Footer: `Amend` and `Cancel` in platform order, focus on `Cancel`. `Amend` runs it (S3's
running state); `Cancel` returns to S7 unchanged.

**S10. Merge in progress.** The draft, if empty and untouched since the merge began, fills
with `MERGE_MSG` cleaned by git's own strip (`git stripspace --strip-comments`, so the
`# Conflicts:` block and any scissors section are gone and the comment character is git's).
Button reads `Commit` (or `Commit 2 Files`), enabled with nothing staged, and concludes the
merge. `Amend` is disabled; its reason sits under the box as the operation line:
`Merging. Committing concludes the merge; Amend is unavailable.` — one line, the only one.

**S11. Cherry-pick or revert in progress (single).** As S10: the draft fills with the cleaned
`MERGE_MSG`; `Commit` concludes it; the line reads `Cherry-picking a1b2c3d. Committing
concludes it; Amend is unavailable.` (pending the option in §5.3).

**S12. Rebase, `git am`, or a cherry-pick/revert sequence in progress.** Fields and buttons
disabled; the operation line reads git's own instruction: `A rebase is in progress. Continue
or abort it with git rebase --continue / --abort.` (Cairn has no rebase verbs yet; when it
gets them, Fork's `Continue Rebase` / `Abort` / `Skip` replace the sentence.)

---

## 5. Choices where Fork is unknown or Fork is wrong

### 5.1 Where the amend warning lives (Fork: Windows warns, form UNKNOWN; Mac silent)
- **A. Dialog at press, only when published or reflog-less (recommended).** Person: amends
  freely in the common case; one dialog exactly when it matters, also on the chord. Cost: the
  press first reads the cost (a hidden walk bounded by unpublished commits) — tens of ms on
  the bench; one extra round trip before git runs.
- **B. Inline note while Amend is ticked** (Windows-like, if Fork's is inline). Person: sees
  the warning before pressing, but a press still goes through, so it confirms nothing — the
  token theatre returns, and the note must be refreshed per status (today's M2).
- **C. Mac Fork: no warning at all.** Person: a published amend is discovered at push time.
  Cost: drops a safety Cairn's users already have and the `Confirmed` seal for amend.
- Recommendation **A**: it is R-A applied uniformly, and Windows Fork evidently warns.

### 5.2 `MERGE_MSG`'s comment lines (Fork leaks them — an open defect, Tracker #180)
- **A. Clean at pre-fill with `git stripspace --strip-comments` (recommended).** Person: sees
  `Merge branch 'x'` only; what is shown is committed; their own `#123` lines are kept, as in
  Fork. Cost: one new read in `reads/` (a porcelain-free, repository-free command, but a new
  guard roster entry); the list of conflicted files is no longer in the draft (it is in the
  history and Local Changes anyway).
- **B. Keep them visible, commit with `--cleanup=strip`.** Person: sees lines that silently
  vanish; any line they type starting with `#` vanishes too. Cost: WYSIWYG lost; diverges from
  Fork for every commit.
- **C. Today (visible, committed).** Person: a chore and a trap. Diverges from git's editor.
- Recommendation **A** (reverses decision E of 2026-10-09 — a user decision to revisit).

### 5.3 Cherry-pick / revert in progress (Fork commits them; Cairn refuses)
- **A. Commit concludes a single pick/revert, as Fork and `git commit` do (recommended).**
  Person: resolves, commits, done; author kept for a pick. Cost: real-git tests that `-F -`
  keeps the picked author and clears `CHERRY_PICK_HEAD`; the engine must tell a single pick
  from a sequence (`.git/sequencer`).
- **B. Keep refusing; the line quotes git's continue command.** Person: must use a terminal.
  Cost: none now; a divergence from Fork and git that stays.
- Recommendation **A**; B for sequences, rebases and `am` until those verbs exist.

### 5.4 When to offer `Skip pre-commit hooks and commit`
- **A. On every failed commit or amend (recommended).** Person: the skip is there whenever a
  hook might be why; where no hook ran, pressing it fails the same way, harmlessly. Cost:
  offered on failures hooks did not cause (e.g. no identity) — Fork-inexact.
- **B. Only where a hook is detected, by `git rev-parse --git-path hooks` + file exists (no
  `X_OK` reimplementation).** Person: Fork's behaviour; misses config-declared hooks
  (`hook.<name>.command`), the Husky gap users complain of in Tracker #948. Cost: today's check
  minus its executability rule.
- Recommendation **A**: it removes Cairn's own hook model, and the failure mode of B is the
  one Fork's users report.

### 5.5 Cancel in the box (Fork: in the Activity Manager only, INFERRED)
- **A. Keep `Cancel` beside `Committing…` (recommended).** Person: stops a slow hook where
  they pressed. Cost: a control Fork does not have; R10.4 already requires it.
- **B. Only the activity popover's ×.** Fork-exact, but a hook that hangs (R5.3's `/dev/tty`
  case) is cancelled only by finding the popover.
- Recommendation **A**.

### 5.6 The amend button's caption (Fork: UNKNOWN)
- **A. `Amend abc1234` (recommended).** The id appears once, here; nothing else repeats it.
- **B. `Amend`.** Shorter; the person reads the id from the history's top row.
- **C. `Amend Last Commit`.** Plain words; no id to check against the dialog.
- Recommendation **A** — it is the identity the dialog's sentence refers to.

### 5.7 A cleared pre-fill (Fork: UNKNOWN)
- **A. A pre-fill fills only an empty, untouched draft; any edit — clearing included — makes
  the draft the person's (recommended).** One rule for `HEAD`'s message, `MERGE_MSG` and
  (later) `commit.template`; matches the vendor's rule in Tracker #61. Cost: one "touched"
  bit per draft instead of today's per-merge `merge_filled` flag.
- **B. Refill whenever empty.** Simpler state; a cleared draft refills at the next refresh.
- Recommendation **A**.

### 5.8 Detached `HEAD` (Fork warns, form UNKNOWN)
- **A. The operation line reads `HEAD is detached: this commit will be on no branch.`
  (recommended).** No press is blocked; recoverable (Show Lost Commits), so R-A says no dialog.
- **B. Confirmation dialog.** Contradicts R-A.
- Recommendation **A**.

---

## 6. What the redesign removes from today's code (roughly)

- `cairn-ui/src/commit_box.rs`: `CommitButton::{AmendInPlace, AmendAsking, ReadingAmend,
  AmendUnreadable}` → `Commit{files}` and `Amend{short}`; `AmendButton`, `AmendSkip`,
  `confirm_in_place` and the box's `confirmed` state; `on_confirmed`; the `note` line and the
  `line()` helper's `max_lines(3)`/ellipsis; `READING_AMEND`; the busy line as a separate row
  (folded into the button row). The commit box leaves the `Confirmed` constructor roster
  (R1.1): the dialog is the only surface.
- `cairn-app/src/commit_box_pane.rs`: `amend_arrived`, `status_arrived_amending`,
  `open_amend_dialog`, `amend_confirmed`, `skip_amend_hooks`, the `needs_force_push` branch,
  the "(waiting)" formatting, `AMEND_TITLE` (the dialog title becomes the consequence's own).
- `cairn-app/src/commit_box_state.rs`: `amendable`, `reading_amend`, `amend_asked`,
  `fill_with_head` + `merge_filled` (replaced by one `touched` bit), `GitError.failed.
  confirmed_with` (the token comes back in the error instead).
- `cairn-app` worker/session: `Request::Amending`, `Request::StopAmending`, `QueryLane::
  Amending` and its hand-written supersession; `session.rs`'s status fork on the amend flag
  (amend's staged list becomes an overlay on the one status, `code-app-ui` M3).
- `cairn-model/src/consequence.rs`: `amend_replaces` (the "Replaces …" line and both
  Show-Lost-Commits sentences), `replaces()`, `force_push_warning()`, `needs_force_push()`; the
  four amend prompt variants collapse to two fixed sentences; `Consequence::Amend` keeps
  `commit`, `published`, `reflog` (no `subject`, so the freshness check stops comparing a
  display field — `code-engine` M2).
- `cairn-git`: `commit_hooks.rs`'s executability model (option 5.4 A removes the hook check
  altogether); the per-status amend consequence read becomes a step of the amend job;
  `CommitRefusal` for cherry-pick/revert narrows to sequences, rebase and am (option 5.3 A).
- `cairn-ui/src/git_error_dialog.rs`: `skip_amend` and the body-placed skip; one button
  component for both footer buttons.
- `WriteEnding`: gains `Cancelled`; `MayHaveTakenEffect` stops covering a user's cancel.
- PRD/decision text to revisit with the user: R10.6 (line and in-place confirmation), R10.8
  and decision E (comment lines), L12, L25 (cherry-pick/revert), R1.1's roster, R6.6.
