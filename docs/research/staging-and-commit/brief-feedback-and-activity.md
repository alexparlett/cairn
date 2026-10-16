# Feedback and activity: design brief

Concern: where every progress indication, refusal, failure and leftover-lock message appears,
and how it is worded. Read-only research, 2026-10-10, over the worktree
`.claude/worktrees/staging-and-commit` and fresh Fork evidence.

Labels: **VERIFIED** (a vendor note, a vendor comment or a screenshot says it), **INFERRED**
(reasoning over verified material), **UNKNOWN** (looked for and not found).
`Tracker #N` = `https://github.com/fork-dev/Tracker/issues/N`, `TrackerWin #N` =
`https://github.com/fork-dev/TrackerWin/issues/N`. "Mac RN x" / "Win RN x" = entries on
<https://git-fork.com/releasenotes> / <https://git-fork.com/releasenoteswin>.

**The headline finding for this concern.** The existing Fork evidence missed this:
**Fork does offer to remove `index.lock`.** Mac RN 1.0.79 (31 May 2019): "Propose to remove
index.lock file if repository is locked". Win RN 1.63 (10 Jun 2021): "Propose to delete
lock.index if repo is locked". A user in Oct 2026 describes when it appears: when an operation
fails, "a 'file exists' error with an option to remove the file" (USER, Tracker #2663,
<https://github.com/fork-dev/Tracker/issues/2663#issuecomment-6043580921>). So Fork ties the
offer to a **failure caused by the lock**. It is not a standing indicator, and it is not tied
to entries in a log. Separately, Fork **retries** before it reports a locked repository (VENDOR
Tracker #1303; Win RN 1.84 retries commit, TrackerWin #1861).

---

## 1. Fork's behaviour, by kind of feedback

### 1.1 Progress (an operation is running)

| Claim | Evidence | Confidence |
| --- | --- | --- |
| The toolbar's central status control is where progress lives. A busy indicator spins in it while git works, including while `git status` refreshes. | VENDOR TrackerWin #2114: "Does the busy indicator on the status control spin?" (<https://github.com/fork-dev/TrackerWin/issues/2114#issuecomment-1875729362>); Mac RN 1.0.18 "Added progress bar in the repository status button." | VERIFIED |
| The control names the running operation. | VENDOR TrackerWin #125 (cited in `fork-staging-and-commit.md` §8); blog Mac 1.0.79 "Improved status control" | VERIFIED (the exact text drawn is UNKNOWN) |
| Network and long operations show a progress bar: clone, fetch, pull, push, long checkout, LFS. | Mac RN "Show progress and status for clone, fetch, pull and push operations." (Oct 2016); "Show progress during long checkout operations" (Mac Dec 2018, Win 1.29) | VERIFIED |
| The commit button is disabled while a commit runs. Fork shows no separate "committing" line. | Win RN 1.29 "Disable commit button while commit is in progress" | VERIFIED for the disabling; INFERRED that there is no line |
| Windows dialogs (stash, discard) show a spinner inside the dialog while their command runs. | USHOT blog Win 1.48; TrackerWin #1191, #2780 (evidence §8) | VERIFIED |
| **Queued operations.** Fork shows no queue. Users asked for operations to be serialised (Tracker #1633, 2022), and the vendor reports "an improvement in Fork 2.30" without saying what it is. | Tracker #1633 (<https://github.com/fork-dev/Tracker/issues/1633#issuecomment-1606741627>) | VERIFIED that no queue was visible in 2022; what 2.30+ draws is UNKNOWN |
| Nothing is drawn under the file lists. Every Local Changes capture in the evidence shows lists, a diff and the commit box, with no message strip. | `fork-staging-and-commit.md` §1, §8; ux-local-changes H2 | INFERRED (no capture shows one) |

### 1.2 Success

| Claim | Evidence | Confidence |
| --- | --- | --- |
| Success shows no message: the status control goes back to idle, and the lists and graph refresh. | Every capture; TrackerWin #1340, where a user complains that "There is no clear indication that the operation ended with the success" | VERIFIED |
| In the Activity Manager, success is the word in the right pane's header, `succeeded <date> <time>`, plus (Win 1.68) a success icon. | VSHOT TrackerWin #1340 (<https://github.com/fork-dev/TrackerWin/issues/1340#issuecomment-955700882>), screenshot read: left row `Fetch origin  14:25`, right header `Fetch origin … succeeded 31 Oct 2021 14:25:08`, then `$ git fetch --prune origin --verbose` and its output | VERIFIED |

### 1.3 Failure

| Claim | Evidence | Confidence |
| --- | --- | --- |
| Any failing git request a person started opens **one modal**, `Git Error`: "An unexpected error occurred while performing the git request.", `Error Details:`, a box with the output, and `Close`. Escape closes it. | Mac RN 1.0.5 "Show git error details dialog on git error."; USHOT Tracker #2645 (Jun 2026, Mac) and TrackerWin #2825 (Jul 2026, Win), both read | VERIFIED |
| The modal is also used for errors that are **not git's**: `[Internal bug] Unhandled error exit code 1` (Tracker #2645) and `[bug] Invalid ref dates: …` (TrackerWin #2825, which the vendor calls "an internal assert"). The title and fixed sentence do not change. | the two screenshots | VERIFIED |
| The details box wraps text and scrolls to the end, where a hook says why it failed. | Win RN 2.2 "Wrap text in error dialog"; Win RN 2.15 "Auto-scroll error messages to end in error dialog" | VERIFIED |
| The modal adds buttons for some failures: `Skip pre-commit hooks and commit` (hook failure); on Windows, a Credential Manager button. **Removing `index.lock`** is offered on a lock failure (see 1.6). | evidence §5, §8; Win RN 2.0 | VERIFIED; the lock button's wording is UNKNOWN |
| The failure is also logged: a **warning triangle** before the time in the Activity Manager's row, and the full output in its right pane. The vendor sends people there for "full output" when the modal says too little. | VSHOT TrackerWin #1340 (Mac list: ⚠ beside failed `Fetch origin` rows); VENDOR Tracker #2645 "Check full output in activity manager" | VERIFIED |
| The status word for a failure in the right pane's header. | none found | UNKNOWN; "failed" is the natural pair to "succeeded" (INFERRED) |
| Automatic fetches that fail are logged with ⚠ and do not raise the modal. | Mac screenshot in TrackerWin #1340: hourly `Fetch origin` rows, several with ⚠; no user report of a modal per auto-fetch | INFERRED |

### 1.4 Refusal (Fork declines before running git)

| Claim | Evidence | Confidence |
| --- | --- | --- |
| Fork prevents rather than refuses: it disables controls (Commit with nothing staged, Commit while a commit runs). Staged changes cannot be discarded because the action is not offered. | evidence headlines 1, 2; Win RN 1.29 | VERIFIED |
| Validation inside a dialog (Create Branch's name) is shown inside that dialog. | `fork-create-branch-evidence.md` (Cairn record) | VERIFIED there |
| A refusal Fork detects at run time is shown in the `Git Error` modal, as its internal errors are. | the internal-error screenshots above | INFERRED |

### 1.5 Cancel

| Claim | Evidence | Confidence |
| --- | --- | --- |
| A running operation is cancelled with an × on its row in the Activity Manager. | VSHOT blog Mac 1.0.67; Win RN 1.16 "Allow to cancel active operations" | VERIFIED |
| Cancel was first offered for Fetch/Pull/Push/Clone, then made immediate. | Mac RN 1.0.67 "Ability to cancel Fetch/Pull/Push/Clone operations"; Mac RN 1.0.69 "Cancel long operations immediately" | VERIFIED |
| Whether a commit can be cancelled, and what word a cancelled entry shows. | none found | UNKNOWN |

### 1.6 Locks

| Claim | Evidence | Confidence |
| --- | --- | --- |
| Fork once failed silently on a locked index (Tracker #212, 2018). A user asked it to "report / propose to remove" the file, and the vendor confirmed the bug and fixed it in 1.0.72. | Tracker #212 (<https://github.com/fork-dev/Tracker/issues/212#issuecomment-365200351>) | VERIFIED |
| Fork **proposes removing `index.lock`** when the repository is locked: Mac 1.0.79 (2019), Win 1.63 (2021). | release notes above | VERIFIED |
| The proposal comes with the failure ("file exists" error with an option to remove), and only where the operation reports it. The same user saw it when staging a whole file, not on other routes. | USER Tracker #2663 (Oct 2026) | VERIFIED as a user report; the exact UI (a button in `Git Error`, or a separate alert) is UNKNOWN |
| Before reporting a lock, Fork checks several times whether it is still held. Commit retries were added in Win 1.84. | VENDOR Tracker #1303 (<https://github.com/fork-dev/Tracker/issues/1303#issuecomment-820405476>); TrackerWin #1861 | VERIFIED |
| Other lock files (`refs/…lock`, `packed-refs.lock`) are not special-cased. git's own message ("remove the file manually to continue") appears in the `Git Error` modal. | Tracker #2246 (a ref lock): vendor advice only, no Fork affordance | VERIFIED for that case |
| Fork shows nothing about a lock it finds when the repository opens, and has no standing lock indicator. | no note, issue or capture mentions one | UNKNOWN (absence is INFERRED) |
| The vendor's stance on removing a lock: "If you are sure it's not [running], deleting the file is the correct solution." | VENDOR Tracker #1446 | VERIFIED |

### 1.7 The Activity Manager, as Fork draws it

- Opened by clicking the repository name or status control on the toolbar (Mac RN 1.0.84 / Win
  RN 1.39; VENDOR Tracker #2645, #1966). A user describes it as "the icon with the 4 lines at the top
  center" (Tracker #2220). VERIFIED.
- The left list has one row per operation: the name (`Fetch origin`, `Create branch 'x'`), ⚠
  when it failed, and the **local** time `HH:MM`. The list shows **no status word**. VERIFIED
  (screenshots in TrackerWin #1340).
- The right pane's header holds the name, the status word and the local date and time
  (`succeeded 31 Oct 2021 14:25:08`). Below it, `$ git …` lines and the output. **No duration**: it
  was asked for and is still open (TrackerWin #2179). VERIFIED.
- Output: hook output (Mac 1.0.69, Tracker #303); the output of every dialog command (Mac
  1.0.88); results (Mac 2.18, Win 1.68/1.75); sections that keep auto-fetch from crowding the
  list (Mac 2.48, Win 2.3); highlighting. VERIFIED.
- Nothing persists after quitting (Tracker #2232). No quotation of a confirmation the user
  accepted, no "way back" buttons. Reset was suggested and the vendor pointed to reflog
  (TrackerWin #1810). VERIFIED as an absence from every capture (INFERRED for "none").
- How the log is bounded, and what it says when it drops output. UNKNOWN.

---

## 2. Cairn today: every place a message appears

### 2.1 The places

| # | Place | Code | What it holds |
| --- | --- | --- | --- |
| P1 | **Banners** under the title bar, one line each, cut with an ellipsis, stacked, with no dismiss | `window.rs` `banner`; `status_text.rs` | fetch progress and endings, fetch refusal, closing, lock files |
| P2 | **The title-bar status box** | `cairn-ui/src/status_box.rs` | name `*`, branch, `18↓1↑`, detached or unborn. **No progress at all**: Fork's progress home is unused |
| P3 | **The strip under Local Changes' lists**, 12 px, up to 3 lines with an ellipsis | `local_changes_actions.rs::acting_line` | refusals of a click, the pre-dialog read, running and queued writes, and how **any** write last ended (a commit's too) |
| P4 | **The commit box**: its busy line, its `note`, its `stopped` line | `commit_box_pane.rs`, `cairn-ui/src/commit_box.rs` | "Committing" with elapsed time and Cancel, "(waiting)", Amend read failures, "unavailable while a merge is in progress" |
| P5 | **The `Git Error` modal** | `cairn-ui/src/git_error_dialog.rs` | a failed commit or amend (with skip); every non-Done ending of Create Branch, including refusals with an empty command |
| P6 | **Create Branch's dialog**: its refusal row (cut at 2 lines) and its wait line | `create_branch.rs` | name validation, "Waiting for …" |
| P7 | **The activity popover**: list row, right pane header, message (cut at 3 lines), "Confirmed:" prompt (cut at 4 lines, with Show All), `$ git` lines, the let-go notice, Show Replaced Commit, Remove index.lock… with its note | `activity.rs`, `cairn-ui/src/activity_popover.rs` | every write and fetch, and a lock-at-open entry |
| P8 | **Confirmation dialogs** (Remove stale lock, Discard changes) | `confirm_dialog.rs` | prompts, in scope only as the target of the lock offer |

### 2.2 Status words (P7): eight

`running`, `succeeded`, `failed`, `not run`, `may have taken effect`, `partly done`, `cancelled`,
`found` (`activity.rs` `Outcome::status`). The list row shows `"{status} · {started}"` and the
header repeats `"{status} · started {started} · took {took}"`. Time is shown as `HH:MM:SS UTC`
(decision D).

### 2.3 One fact, said several ways

**"An operation is running or queued": six phrasings in four places, from three wording
tables** (`LocalWrite::what` lower-case gerund, `::name` imperative, `::awaited`), plus literals.

| Where | Running | Queued |
| --- | --- | --- |
| P3 strip | `Staging 1 file…`, `Staging 1 file… (2 more queued)` | `Staging 1 file (queued)` |
| P4 commit box | `Committing` / `Amending` + `0:07` + Cancel | `Committing (waiting)` (drawn as `Committing (waiting)…`); the doc comment still promises "Waiting for staging 2 files" |
| P6 Create Branch | n/a | `Waiting for the commit to finish…`, `Waiting for staging to finish…`, `Waiting for the branch to be created…`, `Waiting for another write to finish…` |
| P1 closing | `Finishing commit… Closing again leaves it unfinished.` (built from `what`) | n/a |
| P1 fetch | `Fetching origin…`, `Fetching origin: Receiving objects: 40%`, `Cancelling fetch of origin…` | n/a |
| P7 | `Stage 1 file` / `running` | not shown |

Gaps between the tables: `what` says `discarding files` with no count, while `name` says
`Discard 3 files`. `what` says `commit` (a noun), and the strip capitalises it to `Commit…`.

**"Nothing was written": five reassurance tails, and the strip adds a sixth.**

- engine `Display` (`cairn-git/src/error.rs`): `…; nothing was written` (Refused, NoPaths,
  ChangedSinceRead, CheckoutRefused, CommitCancelledBeforeRunning), `…; nothing was committed`,
  `…; nothing was discarded`, `…; nothing was amended`, `…; nothing was removed`.
- the strip (`acting_line`) wraps Stale/Refused/NotRun as `"{What}: nothing was written.
  {message}"`. The result reads: `Staging lines of a.rs: nothing was written. a.rs changed
  after its diff was read; nothing was written`.
- The same ending appears again in P7 (`not run` and the same message) and, for Create Branch,
  in P5 under "An unexpected error occurred…" with an empty command line.

**"A write failed in git": three renderings.**

- P5: `$ git commit -q -F -` and the output lines (commit and amend only).
- P3: `Commit failed: <first fatal:/error: line>` (`why_it_failed`), under the **file lists**, on
  the other side of the window from the commit box.
- P7 message: `git commit -q -F - failed (exit status: 1): <whole stderr>; lock files are present
  under the git directory — another git process is running here, or one was stopped before it
  could remove them: <paths>`, cut at 3 lines. Then the same stderr again in the `$ git` lines,
  ending `(exit status 1)`. Note the colon in one exit status and not in the other.

**"Lock files exist": five sentences, in three places, with two different hedges.**

1. engine `PresentLocks` (on a failure): `; lock files are present under the git directory —
   another git process is running here, or one was stopped before it could remove them: <paths>`
2. engine `StrandedLocks` (on a cancel or a lost process): `; lock files remain under the git directory,
   which later writes will fail on while they are there (stale if no git is running here):
   <paths>`
3. `status_text::locks_line` (P1, stays up until the next write ends): `Lock files remain under
   the git directory and will fail later writes while they are there — stale if no other git is
   running here, and then safe to remove: <paths>`. It is cut at one line, so the paths, the part
   a person acts on, are what the ellipsis hides.
4. `status_text::fetch_line` (P1): `Fetch of origin cancelled; lock files remain under the git
   directory and will fail later writes … safe to remove: <paths>`
5. P7 entry `Lock files found as the repository opened` / `found`, each path as a line, and
   `Remove index.lock…` where `index.lock` is among them.

The banner says "safe to remove" and never mentions the button. The button is in the
popover, on whichever entries' endings named the lock.

**"Remove index.lock…" has six states** (`activity.rs`): offered (`LockOffer::Ready{note:
None}`); refused at the press, with a note beside it (`index.lock was not removed: <why>`);
blocked while any Cairn git runs (`Cairn is running git in this repository`, `GIT_RUNNING_NOTE`,
a copy of the engine's `LockRefusal::GitRunning(1)` text); reading the consequence
(`lock_asked`); ready to confirm (`lock_ready` → `Remove stale lock` dialog); removed (taken
from every entry). The offer follows **past endings** (`lock_named` from `names_index_lock` at
the ending), not whether the file exists now. After a removal by hand the offer stays, and
pressing it is refused with `there is no index.lock`.

**Cancelling.** A cancelled fetch shows `cancelled`. A cancelled commit shows `may have taken
effect`, with the message `git commit … was cancelled; lock files remain …`, and the strip
under the lists says `Commit may have taken effect in part: …`. One gesture (× or Cancel)
produces two vocabularies, because `WriteEnding` has no Cancelled variant: `GitCancelled` and
`GitUnwatched` are folded into `MayHaveTakenEffect` (`worker/local_lane.rs`, `WriteEnding::of`).

**Other P7 text:** `Its output was let go to make room for newer operations.` (decision M);
command endings `(exit status N)`, `(ended by signal N)`, `(not started)`, `(how it ended is not
known)`, `(…, cancelled)`; `Confirmed:` + the whole prompt, cut at 4 lines, with `Show All` /
`Show Less` chosen by a character-count estimate taken against a constant width; `Show
Replaced Commit`, which also switches on Show Lost Commits for the session; `Nothing has run in
this window yet.`; a write that never started is named `A write` when it was never seen being
asked.

**Other P1 text:** `Fetched origin` stays until the next fetch, because no code path returns
`FetchStatus` to `Idle` (INFERRED from `fetch_state.rs` and `session.rs`). `Fetch of origin
failed: <first fatal:/error: line>`. `Fetch of origin not started: a fetch of upstream is already
running`, a refusal of a click shown as a banner.

**Other P3 text:** `Reading what the discard would lose…`, a read before the confirmation dialog.
`Nothing was discarded: <why>`. `<What> did not take every file — a.rs kept as they were: the
discard did not take every file: a.rs kept as it were` (repeats itself, and gets "it were" wrong).

### 2.4 Summary of today

- **Progress** has four homes (P1 fetch, P3 writes, P4 commit, P6 wait) and none of them is
  Fork's (P2).
- **Failure** has four homes (P5 for commit and branch, P3 for every write, P1 for fetch, P7 for all).
- **Refusal** has five homes (P3, P4 note, P5 for Create Branch, P6 row, P1 for fetch).
- **Lock** has three homes (P1, P7 entry message, P7 offer) and five sentences.
- **Success** is mostly silent, which is right, except the `Fetched origin` banner, which stays up.

---

## 3. The proposed clean model

### 3.1 One home and one wording per kind of feedback

| Kind | Its one home | Wording pattern | Also recorded in |
| --- | --- | --- | --- |
| **Progress**: an operation running | **The status box (P2)**: a spinner and `<Name>` as its second line, replacing the branch line while something runs. A fetch adds git's progress line (`Fetch origin — Receiving objects: 40%`). | the operation's one **name** (Fork's imperative: `Stage 2 files`, `Commit`, `Fetch origin`). The spinner already means "in progress", so no gerund and no `…`. | P7 entry `running` |
| **Queued**: waiting behind another | the status box: `<Running name>` + ` · 1 waiting` | **`Waiting for <Name>`** is the only sentence form, used where a control is disabled because of the wait (commit box, Create Branch: `Waiting for Stage 2 files`) | not a separate P7 state: a queued write gets its entry when it starts (as today) |
| **Progress at the control** (Fork disables Commit) | the commit box keeps `Commit` disabled, with **elapsed time and Cancel** (the only cancellable local write) | `Committing 0:07 [Cancel]` / `Waiting for Stage 2 files` | the status box shows it too |
| **A read before a dialog** (what a discard loses, what Amend replaces) | inside the control that asked: Discard opens its dialog at once with its button disabled and `Counting what this discards…` in the body (Fork's Windows in-dialog spinner); Amend keeps the commit box's `Reading what Amend would replace…` | `<Counting/Reading> …` | none |
| **Success** | nothing. The refresh is the message. | none | P7 `succeeded` |
| **Failure**: git ran and failed | **the `Git Error` modal (P5)** for every operation the person started (stage, discard, commit, amend, create branch, fetch, remove lock) | Fork's title and sentence; `Error Details:`, then `$ git <args>` and git's output, scrolled to the end; extra buttons in context (Skip hooks; **Remove index.lock…** when git's output says it could not create `index.lock`) | P7 `failed` + ⚠, the full lines |
| **Refusal known before the click** (staged discard, submodule, conflict, a fetch while one runs, Commit during a merge) | **the control is disabled, and its reason is the tooltip or menu-item reason** (the menu already does this). No message after a click that should not have been possible. | `<Action> isn't available: <short reason>.` One template, one sentence, and no explanation of the implementation (ux-local-changes M2) | none |
| **Refusal found at run time** (stale patch, file changed since confirmed, HEAD moved since amend confirmed, closing) | **the same modal**, shown as an operation that did not happen. See choice C2. | title `Couldn't <name>` (or Fork's `Git Error`); body: the engine's one sentence, which names the path and the reason **once**, and **one** fixed tail: `Nothing was changed.` | P7 `failed` + ⚠, the sentence as its one line, no `$ git` lines |
| **Partly done** (a discard that kept some files) | the same modal | `Discarded 3 of 5 files. Kept as they were: a.rs, b.rs.` + the reason | P7 `failed` |
| **Cancel** (× in P7, Cancel in the commit box) | **no modal**: the person asked for it. The commit box's busy line reads `Cancelling…` until git is reaped. | P7 status `cancelled`. A message only when there is something to say: `Cancelled while git was running. The lists and history show anything it had already done.` | P7 |
| **Lost hold of git** (`GitUnwatched`) | a failure: the modal | `Lost track of git while it ran. The lists and history show anything it had already done.` | P7 `failed` |
| **A leftover `index.lock`** | **one repository state, one banner**: present exactly while `<gitdir>/index.lock` exists at the last check, carrying the button | see 3.3 | the modal's `Remove index.lock…` button when a write fails on it |
| **Other lock files** (ref, `packed-refs`, `HEAD`) | **no state of their own**. git names them in its own words when a later write fails on them, in the modal (as Fork). A cancel that stranded one says so in its P7 entry. | git's text | P7 |
| **Closing while a write runs** (R4.9) | the one remaining banner, because it is about the window rather than an operation | `Closing after <Name> finishes. Close again to quit now.` | none |

The rules behind the table:

1. **One name per operation.** `LocalWrite::name()` (imperative, Fork's) is the only wording
   table. The status box, `Waiting for <Name>`, the closing banner, the modal title and P7 all use
   it. `what()`, `awaited()` and `Asked.what/awaited` go.
2. **The engine says why, once. The UI never adds a reassurance of its own.** The engine's
   refusal sentence carries no "nothing was …" tail. The view adds exactly one tail,
   `Nothing was changed.`, which is true of every refusal ending. (The engine could say it
   instead, but not both.)
3. **The engine's `Display` is never shown as text.** A `WriteEnding` carries a **short
   sentence** for people (`why`) and, separately, the command, the output and the lock paths as
   data. P7's message line and the modal body are built from those fields. This removes the 3-line
   cut (decision C) because nothing long goes there.
4. **No strip under the lists (P3) and no banner for operations (P1)**, except the lock state
   and closing. Both are about the repository or the window, not about an operation.
5. **Status words: `running`, `succeeded`, `failed`, `cancelled`.** Fork's two, plus the two
   pairs Cairn can actually produce. A queued write is not an entry yet. "Not run", "partly done",
   "may have taken effect" and "found" become a `failed` or `cancelled` entry with a one-line
   message.

### 3.2 The activity popover, limited to Fork's contents

| Element | Fork | Proposal |
| --- | --- | --- |
| List row | name · ⚠ if failed · `HH:MM` local | **as Fork**: name, a glyph for failed (⚠) and cancelled (a distinct shape, since Cairn's palette tells kinds apart by shape), time. No status word in the row. |
| Header | name, status word, date and time | `<Name>`, then `<status> <time>`. Duration only if the user keeps it (Fork lacks it; TrackerWin #2179 asks for it). |
| Message line | Fork shows "results" (Mac 2.18) | **one sentence**, only for failed/cancelled (`why`), never cut, because it is one sentence by construction. Lock paths are not appended to it. |
| `$ git` lines | yes | yes, one append-only list (streamed output and command log in one stream; no swap when the command log arrives). One exit form: `exit code 1`, `killed by signal 9`, or `cancelled`. Drop `(not started)` and `(how it ended is not known)`, which are developer states. |
| × cancel | yes | yes |
| `Confirmed:` prompt | **no** | see choice C4 |
| Show Replaced Commit | **no** | see choice C5 |
| Remove index.lock… | **no** (the offer is in the failure alert) | **removed from the popover**: it lives in the lock state (3.3) and in the modal |
| Lock-at-open entry `found` | **no** | **removed**: the lock state covers it |
| Let-go notice | UNKNOWN | evict **whole oldest entries** to stay under the byte bound. Only an entry larger than the bound by itself is tail-cut, with one first line, `Earlier output not kept.` The phrase "let go" goes. |

### 3.3 The leftover lock as one state of the repository

- **What decides it:** whether `<gitdir>/index.lock` exists **now**. A `stat` is added to every
  refresh: on open, on focus, after every write ends, and on F5. It runs on the refresh thread
  (one syscall). There is no history of which ending named it.
- **When it is shown:** the file exists, **and** no git of Cairn's is running or queued in this
  repository (while one runs, the lock is most likely its own; showing it would be a false
  alarm in every successful stage).
- **Where:** one banner under the title bar, the only operation-independent banner besides closing:
  `index.lock is in this repository, so git can't change the index. If no other git program is running, it was left behind.   [Remove index.lock…]`
  It stays until a refresh no longer finds the file, whether it was removed by Cairn, by hand, or
  by the git that held it.
- **Reactive entry, as Fork does it:** when a write fails and git's output says it could not
  create `index.lock`, the `Git Error` modal carries the same `Remove index.lock…` button beside
  `Close`. Both entry points start one flow: read the consequence (path, age, size) → the `Remove
  stale lock` confirmation (decision G's prompt, unchanged) → the removal → a refresh. The banner
  disappears because the file is gone, not because of bookkeeping.
- **States that are removed:** blocked (the banner is hidden while Cairn runs git; the removal
  still re-checks in `ops/`); per-entry notes; "taken away from every entry"; the lock-at-open
  entry; `found`. A refusal at the press (the file changed or vanished since it was read) is
  the run-time refusal of an operation named `Remove index.lock`, shown in the modal like any
  other.
- **Other locks:** shown only in git's words, when git fails on them. The engine still lists
  them (R3.8) for P7's cancelled entry.

### 3.4 Storyboards

**(a) A stage that succeeds**
1. Local Changes: `src/a.rs` is selected in Unstaged. The person presses Return.
2. Within 250 ms (see choice C1), the status box's second line becomes `⟳ Stage 1 file`. Nothing
   else changes: no strip, and no banner.
3. git ends with status 0. The refresh runs (the status box spinner keeps going while it does, as
   in Fork). `a.rs` moves to Staged, and the selection goes to the nearest remaining row.
4. The status box shows `⎇ main  2↑` again. Nothing else is said.
5. If the person opens the popover: the top row is `Stage 1 file   14:03`, and the right pane reads
   `Stage 1 file`, `succeeded 14:03:22`, then `$ git update-index …` (or whatever ran), with no
   message line.

**(b) A stage refused because the file changed**
1. The person hovers a hunk of `a.rs` in the diff and presses `Stage`. Meanwhile an editor saves
   `a.rs`.
2. The status box shows `⟳ Stage lines of a.rs`.
3. The engine's re-hash finds that `a.rs` differs from the diff that was drawn, and writes
   nothing (R3.7).
4. A modal opens: title `Couldn't stage lines of a.rs`, body `a.rs changed after its diff was
   read. Nothing was changed.`, and a `Close` button (no `Error Details:`, because git did not
   fail). Behind it, the refresh has already redrawn the diff with the new content.
5. Escape or Close dismisses it. Popover: `Stage lines of a.rs ⚠ 14:05`, `failed 14:05:10`, and the
   sentence. The strip under the lists does not exist.

**(c) A commit a hook fails**
1. Subject typed and 2 files staged. The person presses Ctrl+Enter.
2. The commit box's button is disabled, its busy line reads `Committing 0:01 [Cancel]`, and the
   status box shows `⟳ Commit`.
3. `pre-commit` exits 1.
4. The `Git Error` modal opens: `An unexpected error occurred while performing the git request.`,
   then `Error Details:`, then `$ git commit -q -F -` and the hook's lines, scrolled to the end. The
   buttons are `[Skip pre-commit hooks and commit] [Close]`. The draft is kept.
5. The status box returns to the branch. Popover: `Commit ⚠ 14:07`, `failed 14:07:31`, the
   message line `The pre-commit hook failed (exit code 1).` (from `why`), and the full lines,
   ending `exit code 1`.
6. Nothing appears under the file lists.

**(d) Cancelling a running commit**
1. The commit is running behind a slow hook: `Committing 0:14 [Cancel]`, status box `⟳ Commit`.
2. The person presses Cancel (or × on the popover's `Commit` row).
3. The busy line reads `Cancelling…` with Cancel disabled, until git's process group is reaped.
4. **No modal.** The commit box returns, with the draft kept unless the refresh shows the commit
   was made (as today's `Done` handling, decided by the refresh).
5. Popover: `Commit ◌ 14:09` (the cancelled glyph), `cancelled 14:09:02`, the message `Cancelled
   while git was running. The lists and history show anything it had already done.`, and the lines,
   ending `cancelled`.
6. If git left `index.lock`, the refresh after the cancel finds it, and **the lock banner** of
   (e) appears. The entry carries no lock sentence.

**(e) A write that leaves `index.lock` behind, then the next launch**
1. A stage is running. The person closes the window: the banner reads `Closing after Stage 3
   files finishes. Close again to quit now.` They close again, and the window goes with git
   orphaned (R4.9, decision 12). git is killed by the session ending, and `index.lock` remains.
2. Next launch: the repository opens, and the first refresh `stat`s `<gitdir>/index.lock`: it
   exists, and no Cairn git runs.
3. Under the title bar: `index.lock is in this repository, so git can't change the index. If no
   other git program is running, it was left behind. [Remove index.lock…]`. The popover is empty
   (`Nothing has run in this window yet.`).
4. If the person ignores it and presses Stage: the status box shows `⟳ Stage 1 file`. git fails
   with `fatal: Unable to create '…/index.lock': File exists.` (after the retry of choice C6, if
   adopted). The `Git Error` modal opens with git's text and `[Remove index.lock…] [Close]`.
5. Pressing `Remove index.lock…` (in the banner or the modal) shows `Remove stale lock`: `Remove
   /r/.git/index.lock? It was last changed 3 hours ago and holds 0 bytes. Another program may
   still own it: removing a lock a running git holds can corrupt the index. You can't undo this
   action.` [Cancel] [Remove index.lock], with focus on Cancel.
6. Remove: the status box shows `⟳ Remove index.lock`, then a refresh. The file is gone, so the
   banner goes. Popover: `Remove index.lock 14:12`, `succeeded`.
7. If the lock is removed in a terminal instead, the banner goes on the next refresh (focus
   regained), with no stale offer left anywhere.

---

## 4. Choices where Fork is unknown or a decision is needed

**C1. A spinner that flashes for fast writes.** A stage takes about 90 ms.
- (a) Show at once. The status box flickers on every stage. Cost: none.
- (b) Show only after 250 ms. Most stages show nothing at all. Cost: a timer per operation on the
  UI side (an animation tick, not a wait).
- (c) Never show for local index writes. Cost: a slow stage on a monorepo looks frozen.
- **Recommend (b).** Fork's status control also spins during refresh, so a delayed spinner is
  close to what users see there, and it keeps Cairn's slow-repository promise.

**C2. Run-time refusals: modal or inline.**
- (a) The same `Git Error` modal, unchanged: Fork's precedent for non-git errors (Tracker #2645,
  TrackerWin #2825). The person reads "An unexpected error occurred while performing the git
  request." for a designed safety refusal, which is misleading.
- (b) The same modal component with a different title and no details box: `Couldn't <name>` and
  one sentence. This is Fork's shape with honest words. Cost: a title parameter on
  `GitErrorDialog`.
- (c) Inline at the control: the commit box's note, or a transient line by the diff. Cost: one
  home per control again (today's problem), and a stage from the keyboard in the list has no
  control to put it beside.
- **Recommend (b).** It gives one home and does not use the word "unexpected" for a designed
  refusal.

**C3. Should a failed local write raise a modal at all, or only P7 + ⚠?** Fork raises it for
anything the person started. Raising it for local writes that fail rarely (stale patch, lock) is
consistent. **Recommend: modal for anything the person started.** This makes the strip
unnecessary.

**C4. The `Confirmed:` quotation (decision K, R12.1).** Fork has none. CLAUDE.md's architecture
says "the operation log can quote them afterwards", so this is a stated project intent, not a
patch.
- (a) Drop it: Fork-exact; the token's prompt is still recorded for an engine-side log.
- (b) Keep it whole, wrapped, in the right pane above the lines, never cut and with no toggle.
  Prompts are bounded by construction (Discard names its first three files), so the cut, the
  width estimate and Show All all go. Show it only for operations that ran (not refused ones).
- (c) Keep today's 4-line cut and Show All.
- **Recommend (b).** It keeps the intent, deletes the machinery (M4/M5), and stops
  `Confirmed:` appearing on an operation that never happened.

**C5. `Show Replaced Commit` (decision F).** Fork has nothing like it and points to reflog.
- (a) Drop it. The amend's prompt (choice C4 b) already says "The old commit stays in Show Lost
  Commits."
- (b) Keep it, renamed to say what it does: `Show in Lost Commits`. It turns the mode on visibly
  and selects the commit.
- (c) Keep it as is.
- **Recommend (b), or (a) if the popover is to be Fork-exact.** Today's version silently changes
  a session mode.

**C6. Retry before reporting a locked index.** Fork retries (Tracker #1303; Win 1.84 for commit).
- (a) No retry: today's behaviour.
- (b) For a write whose git fails on `index.lock: File exists` while no Cairn git runs, wait
  about 1 s and retry once or twice on the local lane, then fail. Cost: the engine has to recognise
  git's lock message, and a write takes longer to fail.
- **Recommend (b).** Most "File exists" errors come from a short-lived git in another tool (an
  editor's git integration, a terminal), and Fork learned this the hard way.

**C7. Whether to show the lock banner while another program's git may be running** (a `git
commit` in a terminal with its editor open holds `index.lock` for minutes).
- (a) Show whenever the file exists and Cairn runs no git: honest, but noisy during terminal
  commits.
- (b) Also require the file to be older than about 10 s. This removes noise from short-lived git
  in other tools.
- (c) Fork-exact: no banner, only the reactive button in the modal.
- **Recommend (b).** The prompt already warns that another program may own the lock. If the user
  wants Fork-exact, choose (c) and keep the modal button only. That also deletes the
  refresh-time `stat`.

**C8. Time in P7 (decision D).** Fork shows local `HH:MM` in the row and the local date and time
in the header. Local time needs a timezone dependency, which is the user's call. **Recommend:**
keep UTC until #D's issue lands, but show `HH:MM` in the row (no seconds, no `UTC` suffix) and
`HH:MM:SS UTC` in the header only.

**C9. Duration in the header (part of decisions B/C).** Fork lacks it, and users ask for it
(TrackerWin #2179). Keeping it costs one field. **Recommend: keep it**, as a stated user need.

**C10. Where fetch progress goes.** Today it is a banner. **Recommend: in the status box**
(Fork: "progress bar in the repository status button"), with git's progress line after the name.
`Fetched origin` disappears, since success is silent. A manual fetch that fails opens the modal (Fork). A
fetch while one runs is prevented: the Fetch button is disabled with the reason as its tooltip.
That makes decision J moot.

**User decisions of 2026-10-09 (R12.1 A–N, R12.4 G–H) that this model would revisit:**

| Decision | What it says | What changes, and why |
| --- | --- | --- |
| A | the popover hangs from the status box | **kept** |
| B | the status words and right pane's order "as built" | **revisited**: 8 words become 4 (`running/succeeded/failed/cancelled`); no status word in the row (Fork); the message is one sentence |
| C | the message's three-line cut | **revisited**: unnecessary once the message is a sentence rather than the engine's `Display` with stderr and lock paths |
| D | `HH:MM:SS UTC` | kept, or softened (C8) |
| E | imperative names | **kept, and promoted** to the only wording table |
| F | "Show Replaced Commit" | **revisited** (C5) |
| G | the lock confirmation's title and prompt | **kept** |
| H | offer computed at the press; blocked state with a note; refusal note; lock-at-open entry | **revisited**: the lock becomes a repository state (3.3). Blocked becomes hidden-while-Cairn-runs-git, notes become a modal, and the at-open entry goes. Reason: an offer keyed to past endings needs every one of those patches, while a state keyed to the file needs none. |
| I | keys inert while the popover is open | kept (outside this concern) |
| J | a refused fetch gets no entry | moot (C10: prevented, not refused) |
| K | `Confirmed:` 4-line cut + Show All | **revisited** (C4) |
| L | POSIX-quoted `$ git` lines | **kept** |
| M | "Its output was let go…" | **revisited**: evict whole entries; `Earlier output not kept.` only for one entry over the bound by itself |
| N | a refused write keeps its own name | **kept** (falls out of rule 1) |

Requirements to amend with them: **R8.6** ("drawn where the user acted": the strip is replaced by
status box + modal + disabled controls); **R4.7** (a cancelled write reports `cancelled`, and only
an unwatched one reports failure; the "may have taken effect" sense moves into the cancel
sentence); **R3.8** (locks stay data on every ending, but are no longer appended to a sentence);
**R4.9** (closing wording now uses the name); **R12.1/R12.4** as above. Also `docs/design/ui.md`
should record the deviations kept: the lock banner (if C7 a/b), the `Couldn't <name>` title (C2 b),
and the cancel glyph.

---

## 5. What the redesign removes from today's code (roughly)

**Places and states**
- The strip under the lists: `local_changes_actions.rs::acting_line`, `Acting.said` / `quiet` /
  the `is_reading` state as a line, its draw in `local_changes_pane.rs` (the `acting` label,
  `max_lines(3)`), and `READING_DISCARD` as a strip text (it moves into the Discard dialog).
- Banners: `status_text::fetch_line` (every arm: progress moves to the status box, success is
  silent, failure is the modal), `refusal_line`, `locks_line`. `closing_line` is reworded to use
  `name`. `window.rs` keeps two banners (lock state, closing) instead of five kinds.
- `Outcome::{NotRun, MayHaveTakenEffect, PartlyDone, Found}` and their status strings.
  `WriteEnding` gains `Cancelled` and splits `GitUnwatched` into `Failed`.
- Lock offer apparatus in `activity.rs`: `ActivityKey::Opened`, `locks_at_open`,
  `LOCKS_AT_OPEN_NAME`, `GIT_RUNNING_NOTE`, `Activity.lock_named` / `lock_note`,
  `ActivityLog.lock_asked` / `lock_ready` / `removing`, `lock_asked` / `lock_answered` /
  `removal_asked`, the "take it from every entry" loop in `write_ended`, `LockOffer::{Ready{note},
  Blocked}`, the popover's lock row (`activity_popover.rs`, `REMOVE_LOCK_CAPTION` there).
  `worker/local_lane.rs::names_index_lock` at the ending. `LocalWrites.locks` as a "last
  listed" list. Added in their place: one `index_lock: Option<LockSeen>` in the refresh answer,
  and one flow from two buttons.
- Show All machinery: `PROMPT_CHARS_PER_LINE`, `longer_than_its_cut`, `Lines.prompt` with its
  measured `State<f32>`, the `ItemSize::Dynamic` closure, `item.index - 1`, `SHOW_ALL_CAPTION`,
  `SHOW_LESS_CAPTION` (and `CONFIRMED_CAPTION` too, if C4 a).
- `LINES_LET_GO` and `let_go`. The streamed-vs-commands two-phase store in `Activity` (one
  append-only list).
- Command endings `(not started)`, `(how it ended is not known)`, and the
  `exit status: N` / `exit status N` disagreement.

**Wording functions and strings**
- `LocalWrite::what`, `LocalWrite::awaited`, `Asked.what`, `Asked.awaited`, `UNKNOWN_WRITE`,
  `Asked::unknown`'s `"a write"` / `"A write"`, `status_text::capitalised` (used only to turn
  `what` into a sentence), `Done.description` if it is shown nowhere else.
- `commit_box_pane.rs`'s `format!("{verb} (waiting)")` → `Waiting for <Name>`, and the stale
  `Busy.what` doc. `create_branch.rs::waiting`'s `Waiting for {awaited}…` → `Waiting for <Name>`.
- The engine's reassurance tails: `; nothing was written` (×5), `; nothing was committed`,
  `; nothing was discarded`, `; nothing was amended`, `; nothing was removed`, and the strip's
  `: nothing was written.`
- `StrandedLocks` / `PresentLocks` as `Display` suffixes (the data stays in the error). The
  `GitFailed` `Display` stops being UI text: `WriteEnding` carries `why` and the fields.
- `status_text::why_it_failed` stays: it is the one place that picks git's `fatal:` / `error:`
  line, used for the P7 sentence and the modal title context.
- `DiscardIncomplete`'s `kept as {it|they} were` and the strip's wrapper around it, replaced by
  one sentence.
- `create_branch.rs::write_ended`'s mapping of `Stale | Refused | MayHaveTakenEffect |
  Incomplete | NotRun` into a `Git Error` with an empty command: it routes through the one
  modal's refusal form (C2 b).
- `GIT_ERROR_TEXT` stays for git failures. A title parameter is added for refusals.

**Rough size.** About 600–900 lines removed across `activity.rs`, `activity_popover.rs`,
`status_text.rs`, `local_changes_actions.rs`, `local_lane.rs`'s wording tables and
`WriteEnding::of`, `local_writes.rs`, `create_branch.rs`, and the engine's lock suffixes. About
250–400 lines added: the status box's progress line, the one modal's refusal form, the lock
state and banner, `WriteEnding::Cancelled` and `why`, and the retry of C6 if adopted. Most of the
work is in tests that pin today's strings (`status_text.rs` tests, the popover tests, and the
strip's tests).

---

### Sources (new in this brief)
- <https://git-fork.com/releasenotes>: Mac 1.0.5, 1.0.18, 1.0.67, 1.0.69, 1.0.79, 2.18
- <https://git-fork.com/releasenoteswin>: Win 1.16, 1.29, 1.63, 1.68, 1.75, 2.2, 2.15
- Tracker #212, #1303, #1446, #1633, #2220, #2246, #2645, #2663; TrackerWin #1175, #1340, #1810,
  #1861, #1966, #2114, #2179, #2825. Screenshots read: TrackerWin #1340 (Mac list with ⚠; Win
  1.68 header), Tracker #2645 (Mac `Git Error`), TrackerWin #2825 (Win `Git Error`).
- GitHub issue search over the trackers was rate-limited (shared quota), so the lists came from
  the web issue search pages. The exact UI of Fork's index.lock proposal (button label,
  alert or `Git Error` button) remains UNKNOWN. A few minutes on the owner's Fork would settle it:
  create `.git/index.lock`, then stage a file.
