# UX/design review: activity popover, lock removal, Show Lost Commits, chrome, shortcuts, status lines

Reviewer: independent, read-only. Worktree `.claude/worktrees/staging-and-commit`. Judged on design, not coverage.
Fork baseline (fork-staging-and-commit.md section 7): Activity Manager = a popover opened by clicking the toolbar status box; left a list of operations (name, status `running`/`succeeded`, time, x to cancel); right the selected operation's `$ git ...` lines and output. Later growth: hook output, results/statuses, sections for automatic fetches, a verbose-output preference. Not persisted. "No quotation of a confirmation the user accepted exists anywhere in it." Reflog = a mode of the graph, dimmed rows, View menu toggle, chord in the commit list; no entry list, recovery is ordinary commit actions.

Sources read: activity_popover.rs, activity.rs, status_text.rs, status_box.rs, shortcuts.rs, lost_commits.rs, accelerators.rs, commit_row.rs (header/lost rows), local_lane.rs (name/what/awaited, WriteEnding), local_writes.rs, local_changes_actions.rs (acting_line), create_branch.rs (waiting), commit_box_pane.rs, remove_lock.rs, error.rs, PRD R12, progress.md (round-by-round), state.md "Decided in the building".

## Summary verdict

The popover's skeleton (left list, right `$ git` lines, x to cancel, hangs from the status box) is Fork's and sound. Everything around it was grown by successive patches: a second "log" of lock files, three parallel wording tables per write, an eight-word status vocabulary, a heuristic "Show All", and four different texts for the same lock-file situation. The worst user-facing defects are in the text: the status words and messages leak engine internals, the message pane duplicates and then truncates git's stderr, and the same fact is said four ways.

---

## HIGH

### H1. A user-cancelled write is reported as "may have taken effect"; cancel has two words depending on what was cancelled
- Where: activity.rs:73-103 (`Outcome::MayHaveTakenEffect` doc: "Cancelled, or Cairn lost hold of its git"; `status()` -> "may have taken effect"), local_lane.rs:688-692 (`Error::GitCancelled | Error::GitUnwatched` -> `MayHaveTakenEffect`), vs `Outcome::Cancelled` -> "cancelled" used only by fetch (activity.rs:fetch_ended).
- User sees: press x on a running commit -> entry says "may have taken effect" (looks like a fault) with a message that is the engine's error string. Press x on a fetch -> "cancelled". The same gesture yields two vocabularies.
- Root cause: `WriteEnding` has no Cancelled variant; the engine's `GitCancelled` (user asked) and `GitUnwatched` (Cairn lost the process) are collapsed into one ending, then the UI patched it with a status word that tries to cover both.
- Clean design: `WriteEnding::Cancelled` for GitCancelled (status "cancelled", and a message only if locks were left); `GitUnwatched` stays a failure ("failed", with the reason). Fork has only running/succeeded (+ failure); one word per fact.

### H2. The message pane repeats git's stderr, then truncates it at three lines, which hides the actionable tail
- Where: activity_popover.rs `message` label `.max_lines(3).text_overflow(Ellipsis)`; error.rs:230 `GitFailed` display = "git {args} failed ({status}): {stderr}{PresentLocks}"; local_lane.rs:~667 uses `error.to_string()` as the message.
- User sees: Failed entry -> a header message "git commit -q -F - failed (exit status: 1): <the whole stderr>...", cut with "...", then the same stderr again, uncut, in the `$ git` lines below. The stranded-lock sentence (the only thing the user can act on, "lock files remain under the git directory... remove: /path") is appended at the END of the message, i.e. exactly what a 3-line cut removes. Nothing lets the user expand the message (Show All exists only for the prompt).
- Root cause: the engine error's `Display` (a developer string including the whole stderr) is reused as the UI message; the 3-line cut (user decision B/C "as built") was added for the symptom of long messages instead of giving the entry a short outcome sentence. Also exit text formats disagree: "(exit status: 1)" (error.rs) vs "(exit status 1)" (activity.rs command_lines).
- Clean design: one short outcome line per entry ("Commit failed: <first fatal/error line>", same `why_it_failed` the banner uses), and git's stderr only in the lines pane (Fork's design: output on the right). Lock files as a separate named line/button, not a sentence at the tail.

### H3. The same lock-file fact is told in four different voices, and the banner tells the user to do by hand what the popover offers a button for
- Where: (1) error.rs `StrandedLocks` "; lock files remain under the git directory, which later writes will fail on while they are there (stale if no git is running here): <paths>"; (2) status_text.rs:`locks_line` "Lock files remain under the git directory and will fail later writes while they are there — stale if no other git is running here, and then safe to remove: <paths>" (a permanent window banner); (3) status_text.rs:`fetch_line` Cancelled-with-locks "Fetch of X cancelled; lock files remain under the git directory and will fail later writes while they are there — stale if no other git is running here, and then safe to remove: <paths>"; (4) activity.rs `LOCKS_AT_OPEN_NAME` "Lock files found as the repository opened" (status "found") plus the `Remove index.lock...` offer.
- User sees: a long, hedged, run-on banner across the top of the window that never mentions Remove index.lock... (which is in the popover), with the same sentence in the entry message, and a fourth entry "found" listing paths. "safe to remove" in the banner invites `rm` by hand while the app has a guarded removal with re-check.
- Root cause: lock handling was added per surface as each QA round found a gap (issue #19, R3.8, R4.9, then H); no single "stale lock" concept in the UI. The offer exists only for `index.lock` while the banners name all lock files.
- Clean design: one notice component ("`index.lock` is left behind -- Remove..." or "Lock files left behind: ..."), with one wording, shown in one place. Fork shows nothing like this (its Git Error modal just prints git's text). If kept, the banner is the only entry point and it carries the button; the popover entry "found" and the hedged paragraph go.

### H4. "Remove index.lock..." is a feature Fork does not have, and its apparatus is the largest source of states in the popover
- Where: activity.rs `lock_named`, `lock_note`, `lock_asked`, `lock_ready`, `removing`, `LockOffer::{Ready{note}, Blocked}`, `ActivityKey::Opened`, `locks_at_open`, `GIT_RUNNING_NOTE`, `use_lock_confirmation`; remove_lock.rs (identity re-check: time/size/dev/inode); Request::LockConsequence.
- User sees: a button that is sometimes there, sometimes greyed with "Cairn is running git in this repository", sometimes live with a note beside it after a refused press, which (once removed) vanishes from every other entry. Offer appears per entry according to "the ending named the lock" (names_index_lock), not according to whether a lock exists now.
- Root cause: the offer is keyed to a past event (an ending that named the lock) rather than to the current state (does `<gitdir>/index.lock` exist?). That mismatch forced: read-at-press consequence, blocked state, a per-entry note, "taken away from every entry on success", and a separate lock-at-open entry. Decided "in the building" and not in Fork's evidence (state.md "Decided in the building, for the user's review").
- Clean design: either drop the feature (Fork: the user removes the lock, git's message names it) or make it a state of the repository, not of entries: one "index.lock present -- Remove..." affordance driven by the current listing, enabled when no Cairn git runs, no per-entry copies, no lock-at-open entry.

### H5. Eight status words, three of which are engine jargon
- Where: activity.rs `Outcome::status()`: "running", "succeeded", "failed", "not run", "may have taken effect", "partly done", "cancelled", "found".
- User sees: "not run" for a refused write ("Stage 2 files -- not run" when the actual state is "nothing was written because..."), "partly done" for a discard that skipped files (which the under-lists line calls "did not take every file"), "found" for a log entry that is not an operation at all.
- Root cause: statuses mirror `WriteEnding`'s Rust variants (Stale/Refused/NotRun/Incomplete/MayHaveTakenEffect) instead of what the user needs: did it happen? Fork: running / succeeded (+failed).
- Clean design: running / succeeded / failed / cancelled. "Nothing was changed" or "Partly done: 2 of 5 files kept" go in the message, not the status chip. "found" disappears with H4.

---

## MEDIUM

### M1. Three parallel wording tables per write, and they disagree
- Where: local_lane.rs `what()` ("staging 2 files", "discarding files" -- no count, "commit", "creating a branch, discarding changes"), `name()` ("Stage 2 files", "Discard 3 files" from Consequence::name, "Create branch 'x'"), `awaited()` ("staging to finish", "the discard to finish", "the commit to finish", "the lock's removal to finish"); plus commit_box_pane.rs ("Committing (waiting)", "Committing is unavailable while ... is in progress."), local_changes_actions.rs:acting_line ("Staging 1 file…", "Staging 1 file (queued)", "(1 more queued)"), status_text.rs:closing_line ("Finishing commit… Closing again leaves it unfinished.").
- User sees: "Waiting for the commit to finish…" (Create Branch dialog) vs "Committing (waiting)" (commit box) vs "Staging 1 file (queued)" (under lists) vs "Finishing amend…" (closing banner). Four phrasings of "this is queued behind that". The commit_box.rs doc comment still promises "Waiting for staging 2 files", which the code no longer says (stale).
- Root cause: every surface was built with its own phrase function; each phase added one more (`what` -> `name` -> `awaited`).
- Clean design: one `Operation { name, in_progress_verb }` and derived forms; a single "waiting" phrase shape ("Waiting for <name>…").

### M2. The under-lists line for a partial discard says the same thing three times, one of them ungrammatical
- Where: local_changes_actions.rs:acting_line `Incomplete` arm: "{what} did not take every file — {kept} kept as they were: {message}"; error.rs:403 DiscardIncomplete display "the discard did not take every file: {kept} kept as {it|they} were{failure}".
- User sees: "Discarding files did not take every file — a.rs kept as they were: the discard did not take every file: a.rs kept as it were". "kept as it were" is wrong English for the singular ("as it was"), and the sentence repeats itself.
- Root cause: the wrapper builds its own sentence and also appends the engine message that already says the same.
- Clean design: show the engine message once, with the right verb agreement ("kept as it was").

### M3. "Stale", "Refused" and "NotRun" are shown as "nothing was written", but the same line style is not applied to every failure, and "nothing was written" appears twice
- Where: local_changes_actions.rs:acting_line `Stale/Refused/NotRun` -> "{what}: nothing was written. {message}" while error.rs messages already end "...; nothing was written" (CheckoutRefused, CommitRefused: "{why}; nothing was committed", AmendChanged: "nothing was amended", LockChanged: "nothing was removed").
- User sees: "Staging 1 file: nothing was written. a.rs changed since you read it; nothing was written." Duplicated. Three different "nothing was X" endings across operations.
- Root cause: both layers add reassurance.
- Clean design: engine says why; the UI adds nothing, or the engine never adds it.

### M4. "Show All" decided by a character-count estimate; the expanded prompt is a special first row in a virtualized list with a measured dynamic height
- Where: activity_popover.rs `PROMPT_CHARS_PER_LINE = (WIDTH - LIST_WIDTH - 16) / 6.5`, `longer_than_its_cut`, `Lines.prompt: Option<(String, State<f32>)>`, `ItemSize::Dynamic` closure, `on_sized` writing back a height, `item.index - 1` shift in `line_row`.
- User sees: a prompt that wraps to 4 lines in a narrow window may have no "Show All" (estimate says short), or a short prompt offers it; the toggle appears/disappears with the window width because the estimate uses the constant WIDTH, not the real width (the panel narrows in narrow windows -- `hung()`), so it is wrong precisely when narrowed. Scroll offsets shift when the prompt row appears. The prompt text is displayed above "Show All" and then moves into the scrolling list.
- Root cause: a 4-line cut + "Show All" was the answer to "the prompt can be long and I cannot use ScrollView" (user decision K). The prompt is not a git output line, but it is made one of the scroll rows.
- Clean design: the prompt is bounded text in its own fixed region (a short scrolling box of fixed height, or simply the full text above the lines pane with the lines pane taking the remainder). Fork has no prompt at all (see M5).

### M5. "Confirmed:" prompt quotation is an invention with no Fork precedent
- Where: activity_popover.rs `CONFIRMED_CAPTION`, activity.rs `prompt`; module doc says "Cairn's addition, L14". Fork evidence: "No quotation of a confirmation the user accepted exists anywhere in it."
- User sees: a "Confirmed:" block containing the entire dialog text again ("Amend abc? It replaces ... You can't undo this action."), including text that is a warning for a decision already made, plus the "can't undo" tone. It is recorded in the PRD (R12.1) but with a design rationale only in terms of the "operation log can quote them afterwards" (CLAUDE.md), not a user need. For refused/NotRun operations it says "Confirmed:" for something that did not happen.
- Clean design: either omit (Fork) or show a single line, not the prompt text, e.g. "Confirmed by you at 14:03". The Show All/Show Less control (M4) exists only because this is long.

### M6. Time shown as "HH:MM:SS UTC"
- Where: activity.rs `started_text`.
- User sees: "started 14:03:22 UTC" on a machine in UTC+1; the history's dates do the same. Recorded as user decision D and filed. Still a visible mismatch; classified here as recorded-but-misleading.
- Clean design: local time, or relative ("2 min ago") in the list. Acknowledged blocker is a timezone dependency.

### M7. "Show Replaced Commit" silently turns a global mode on
- Where: activity.rs `show_replaced` -> `crate::lost_commits::toggle` if off; `find_commit` before the toggle ("After the find: the reopen asks it again").
- User sees: popover closes, History reopens with "Show Lost Commits" ticked (a session mode that stays on), the commit selected. The checkbox is the only sign. There is no way back to "off" except unticking, and the user is never told the mode was changed. The call order (find, then toggle, then re-find on reopen) is a workaround for the reopen invalidating the find.
- Root cause: "way back" modeled as an action on an entry that mutates a unrelated global view mode and relies on reopen re-running the find.
- Clean design: Fork's design -- recovery is the dimmed commit in the graph; the entry may say the id ("Replaced abc1234") and let the user turn the mode on; or make it explicit ("Show Lost Commits and select abc1234") and restore the mode on dismissal.

### M8. Popover feature set exceeds Fork's: caps, "let go" notice, command-ending lines, quoting, lock states
- Where: ACTIVITY_ENTRIES 200 / ACTIVITY_LINES 10,000 / ACTIVITY_BYTES 4 MiB, `LINES_LET_GO` "Its output was let go to make room for newer operations.", `shell_quoted` (POSIX quoting), command_lines trailing "(exit status 1)" / "(ended by signal 9)" / "(how it ended is not known)" / "(not started)" / ", cancelled", per-line cut marker, a streamed-vs-commands two-phase lines swap, a11y modal dialog.
- Assessment: byte/line bounds are a legitimate protection for a long-lived UI; but the user-facing consequences are exposed: "let go" is internal vocabulary; "(how it ended is not known)" / "(not started)" parenthetical states are developer states; and the lines pane switches from streamed output to the command log when the commands arrive (activity.rs `ran` clears streamed), i.e. what the user was reading changes under them.
- Clean design: keep bounds, state them as "Older output was removed." (or nothing); lines pane stays a single append-only list.

### M9. Popover can only be opened with a pointer press on a bare element, and cannot be opened or closed from the keyboard except Escape
- Where: window.rs status box wrapped in `rect().on_press(...)`; shortcuts.rs `keys_inert` returns true while open; accelerator table has no Action for the log. Fork likewise has a click only (evidence), so not a deviation; but with keys_inert the popover is a modal that no key can navigate (no up/down in the list, no Enter on x or Remove).
- Clean design: either keep a pointer-only affordance with a focusable button role, or move focus into the popover and give the entries a list's keys. LOW-MEDIUM (consistent with Fork).

### M10. `keys_inert` as one boolean for "any dialog, any prompt, the Git Error dialog, the popover": chords disabled with no visible reason
- Where: shortcuts.rs:30-35, :act, lost_commits.rs `history_action` (also checks keys_inert), local_changes_actions.rs `dialog_open`.
- User sees: F5 / Ctrl+Shift+. etc. do nothing while a popover is open; the popover gives no sign (it is not visually a modal: no scrim). The click-outside-to-close layer is a transparent full-window rect, so a click outside both closes it and is swallowed.
- Root cause: "the window hears every key before any dialog can" -- the inertness is a global side-condition read from four states in each handler (`keys_inert(view)` is invoked in three separate files).
- Clean design: one input-capture owner (a modal stack); handlers do not each ask.

---

## LOW

- L1. activity_popover.rs: `entry_row` second line is "{status} · {started}" and the right-pane header repeats "{status} · started {started} · took {took}": same fields twice; the list could just show the time.
- L2. `CANCEL_OPERATION_CAPTION = "×"` is a bare glyph with no tooltip/aria label (Fork's matches, but its accessibility is not).
- L3. The "Show All" toggle state is keyed by entry place (index), so when a newer entry arrives while the popover is open, the expanded prompt collapses silently (selected place shifts).
- L4. Blocked note is the engine's `LockRefusal::GitRunning(1)` string duplicated as `GIT_RUNNING_NOTE` ("Cairn is running git in this repository") in the UI crate (comment says "the engine's own words"): two copies that can drift, and it does not say what to do ("wait for the commit to finish").
- L5. Names of failed-to-start writes fall back to "A write" (activity.rs:write_ended), and `Asked::unknown` "another write to finish" -- vague fallback for an unreachable case.
- L6. `fetch_line` "Fetching origin: <git progress line>" in the title bar banner, plus an Activity entry with the same lines: fine and Fork-like. But "Fetch of origin not started: <reason>" (refusal banner) vs decision J (no entry for it): two fetch-failure states, one banner, one entry.
- L7. Accelerators: Linux "Ctrl+Alt+1/2" for Commit/Changes tabs (Fork's Windows mapping) collides with workspace/desktop shortcuts on several Linux desktops (e.g. Ctrl+Alt+digit); Linux stage-all "Ctrl+Alt+Shift+S" is a four-key chord. Fork's Windows set was copied; no evidence it suits Linux. Bare Backspace/Delete discard is gated by confirmation; fine.
- L8. Six scopes (`Window`, `Detail`, `LocalChanges`, `LocalChangesLists`, `CommitBox`, `History`) plus `FieldScope`; `Action` has 20 entries of which 5 diff toggles have no chord and 4 are "press" pseudo-actions in a key table. The table is an unusually heavy abstraction for what Fork (menu-driven) does; each new action costs two matches in two files plus `shortcuts::act` arm. Not user-visible; maintainability only.
- L9. Dimmed lost-commit rows: text uses the placeholder grey (the same colour as column headers), so a lost commit looks "disabled" rather than "unreachable"; the only explanation is the checkbox label. Contrast of placeholder grey on the row background is lower than normal text. Matches Fork's "dimmed" and recorded as Fork-settled.
- L10. Show Lost Commits checkbox in the history header: Fork has a View menu item; Cairn has no menu, so a header checkbox is a reasonable substitution (decision B, with the chord in the tooltip). The state is session-only (off at launch) with no indication in the title bar when on. Acceptable.
- L11. Locks banner and closing banner stack in the window top (`window.rs` banners) with no dismiss; the lock banner persists until the next write ends without locks.

---

## GOOD

- The popover shell follows Fork's evidence closely: hangs from the status box, left list with name/status/time, x to cancel only while cancellable, right pane `$ git ...` lines with stderr, nothing persisted across launches, session-only.
- Names are imperative like Fork's ("Fetch origin", "Create branch 'x'", "Stage 2 files").
- Virtualized lists, bounded memory, and the URL-userinfo scrub are done carefully and are invisible to the user.
- `$ git` lines quoted so they paste into a terminal (decision L) is small and correct.
- The status box (status_box.rs) is Fork's: name with `*`, branch glyph, `18↓1↑`, git's own words for detached/unborn; clean and small.
- Show Lost Commits follows Fork (mode of the graph, same lanes, dimmed text, chord Ctrl/Cmd+Shift+. heard in the commit list, View-menu equivalent as a header checkbox).
- The accelerator table is a single data table, modifiers named in one file, per-platform rows, and a guard.
- The destructive "Remove stale lock" confirmation text is honest and specific (path, age, size, "another program may still own it").
- Cutting a very long single line with a marker (as the diff view does) is consistent.

## Patch-over-root-cause map (for the owner)

| Symptom patched | Patch | Root cause | Clean design |
|---|---|---|---|
| User cancel looked like failure | "may have taken effect" status | no `Cancelled` WriteEnding | `WriteEnding::Cancelled` |
| Long engine message | 3-line cut | engine Display reused as UI text | short outcome line + lines pane |
| Long prompt | 4-line cut + Show All + estimate + dynamic first row | prompt quoted in a log Fork doesn't quote | drop or one line |
| Stranded locks | banner + message sentence + entry + button + blocked state | offer tied to past endings | state-driven single affordance |
| Wait phrasing | `what`/`name`/`awaited` + box strings | per-surface wording | one operation type |
| Lines pane mixed | streamed vs commands swap | two sources for one pane | one append-only lines list |
