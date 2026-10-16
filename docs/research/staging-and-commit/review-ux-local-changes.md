# UX / design review — Local Changes surface (staging-and-commit packet)

Reviewer: independent, read-only. Worktree `.claude/worktrees/staging-and-commit`. Judged: design and wording, not test coverage.
Method: read the app/UI sources named in the brief, the PRD R6-R9 and its dated decisions, `fork-staging-and-commit.md`, the ui.md deviation table, `docs/systems/local-changes.md`, and progress.md QA rounds for phases 07/08.

## Verdict in one paragraph

The core of this surface is clean and Fork-faithful: five stage routes, nearest-remaining selection, one drop zone per list, Fork's badges, Fork's floating Stage/Discard/Unstage, discard that always confirms and never exists on the staged side. Most Fork deviations ARE recorded (ui.md rows, each with a user decision). The weak spots are not in the gestures; they are in (1) the **status/error text channel under the lists**, which is invented (no Fork evidence, no ui.md row), assembled from three vocabularies and double-prefixes engine errors; (2) **two sources of truth for "what is selected"** (the list selection and the "chosen" diff path) which produced a string of fallback branches in QA rounds; (3) **files-drawn-together**, whose line-budget bound leaks into what Return/Backspace do (a different set of files depending on where focus is); and (4) a handful of **answer-number / identity checks** that silently discard user work (a line selection) on any refresh. Counts: 4 HIGH, 8 MEDIUM, 7 LOW; 9 things that are good.

---

## HIGH

### H1. Engine errors are double-prefixed: "nothing was written" twice (sometimes "nothing" three times)
- Where: `crates/cairn-app/src/local_changes_actions.rs:186-192` (`acting_line`: `format!("{what}: nothing was written. {message}")`) over `WriteEnding::Stale/Refused/NotRun` whose `message` is the engine error's `Display` (`crates/cairn-git/src/error.rs:367,380,386,412`, all ending "...; nothing was written" / "nothing was discarded").
- User sees (red, under the lists): `Discarding files: nothing was written. src/a.rs changed since you confirmed; nothing was discarded` and `Staging lines of a.rs: nothing was written. a.rs changed after its diff was read; nothing was written`. Also "Discarding files" is the gerund of a *running* line reused as a *noun* for a finished failure.
- Root cause: the wrapper and the engine each add the reassurance; the wrapper was written first (R8.6 "a stale patch's refusal names its path") and the engine errors were later given their own tail. The unit test only asserts the path is contained, so it passes with the duplication.
- Clean design: the engine message is the whole sentence (it already names path and outcome). The view adds only the subject when the message doesn't carry it, never a second "nothing was ...". One rule for the verb form: "Couldn't stage 2 files: <engine message>".

### H2. The strip under the lists is invented (no Fork evidence, no deviation row) and mixes four kinds of message
- Where: `local_changes_pane.rs:462-478,497-518` + `local_changes_actions.rs:155-215` (`acting_line`); R8.6 in the PRD.
- Fork (research §8): progress is the toolbar status control; every failing git request is one `Git Error` modal. There is no inline strip, no "queued" concept. `docs/design/ui.md` "What changes, and why" has no row for it (grep: no "under the lists"/"queued").
- User sees one grey/red 12px line, max 3 lines with ellipsis, that is, depending on moment: (a) why the last click did nothing ("Staged changes can't be discarded..."), (b) a progress label ("Reading what the discard would lose…", "Staging 1 file… (1 more queued)", "Staging 1 file (queued)"), (c) the last write's failure (stays until the *next write ends*, not until the next action, and cannot be dismissed), (d) git failures of OTHER surfaces — `last` is any write, so a failed commit shows "Commit failed: ..." under the file lists while the commit box (under the diff, other side of the window) opens the Git Error dialog for the same failure; a refused commit ("Commit: nothing was written. ...") appears under the file lists far from the commit box.
- Root cause: the PRD said "drawn where the user acted" and the one existing slot (under the lists) became the sink for refusals, progress and every write's failure, regardless of which control produced them.
- Clean design: refusals of a click = disabled control with tooltip, or a transient inline reason at the control; progress = the toolbar status box / activity popover that already exists (phase 11) — a stage takes ~90 ms and flashes "Staging 1 file…" for a few frames; failures = the Git Error modal Fork uses (the commit path already does). Delete the `queued`/`running` text entirely or fold it into the status box. If the strip stays: it must be recorded in ui.md, and scoped to messages about *this view's* actions.

### H3. Same keystroke stages a different set of files depending on focus (files drawn together + line budget)
- Where: `local_changes_actions.rs:on_the_diff` (`Acted::Paths(together_read_paths)`, ~lines 262-272) vs `intent`/`acted_rows` (the list's `Acted::Selection`); `diff_state/together.rs:37`; ui.md row "Several files selected ... under a line budget ... stage and discard keys act only on the files drawn" (user-ratified).
- User experience: select 40 files, press Return with focus in the list: all 40 stage. Click into the diff and press Return: only the files "read and drawn" stage — files past the budget or still being read are silently left. Fork has no bound and stages the selection. The ratified wording "act only on the files drawn" hides that this is a silent partial act. Backspace (discard) is the same, and its dialog names only the drawn subset, so the user cannot tell 10 were left out.
- Root cause: the stack-of-diffs view needed a bound (concurrency.md) and the bound was allowed to define the *acted set*. Rendering budget and action scope are conflated.
- Clean design: the chords act on the selection wherever heard (as Fork), the selection being the single scope. If the diff pane can't show all, say so in the pane; never narrow the act. (If narrowing is wanted for safety, refuse with a visible reason rather than partially act.)

### H4. Two sources of truth for "the selection": `ListSelection` and the "chosen" diff path — a trail of fallback branches
- Where: `local_changes_actions.rs`: `acted_rows` (falls back to `diff.working_choice()` when the selection is in the other list/empty), `toggle` (`(false, Some(chosen)) => ListSelection::of(list, chosen)`), `range` (anchor falls back to the chosen path), `follow_the_lists`/`Follow::{Keep,ReAsk,ChooseFirst,LetGo,Nothing}` (local_changes_pane.rs:236-325), `ListSelection::toggled` ("the last path toggled out leaves the list's selection empty, and still the list's: nothing is selected there"), `together_wanted` ("the path chosen stays chosen meanwhile, unasked"). progress.md phase 07 QA item 5: "a path it held that the status took away must not come back selected unseen", fixed by adding `ChooseFirst/LetGo` + the empty-but-still-the-list's state.
- User experience: mostly invisible until it isn't: a Stage button that is enabled "when the selection is empty but a path is chosen", header buttons/chords that act on a row that is not drawn selected after an emptying toggle (handled by yet another special case), selection jumping to the *first* path when the chosen one vanishes on refresh but to the *nearest* after a stage (two "what next" rules: R8.3 vs `Follow::ChooseFirst`).
- Root cause: the diff pane's "chosen path" and the list's "selection" are separate pieces of state that must be reconciled by hand after every refresh/toggle/stage.
- Clean design: one state: an ordered selection with a "primary" (anchor/last). The diff follows the primary; the actions read the selection; refresh prunes the selection with one rule ("nearest remaining", same as after a stage). Then `acted_rows`' fallback, `toggle`'s synthesis, `Follow::*` and the "empty but still the list's" state mostly disappear.

---

## MEDIUM

### M1. Capped / truncated text where the reason is the point
- `local_changes_menu.rs:119-126`: the disabled-Discard reason in the context menu is `.width(300).max_lines(2)` at 12px. `NoDiscard::Submodule` text ("X is a submodule: its changes can't be discarded, since what they would lose inside it can't be counted." ≈ 105 chars + path) needs ~3 lines at 300px => the reason ends in an ellipsis in the one place it is offered. Conflicted text is ~2 lines; fits only for short paths.
- `local_changes_pane.rs:497-518` `acting`: `max_lines(3)` + ellipsis in a pane that is the list column (min 200px, default narrower than the diff). `Incomplete` ("did not take every file — a, b, c kept as they were: <msg>") enumerates paths and will be cut — the "which files" part, the part that matters. 
- Clean: reason text short enough to fit ("Submodule changes can't be discarded.") with the long form in a tooltip/dialog; never enumerate paths in an inline line.

### M2. Refusal text is long because it explains the implementation, and is in a different voice from its neighbours
- `local_changes_menu.rs:41-52`: "...since what they would lose inside it can't be counted." (explains the engine's consequence-counting; a user reads this as jargon), vs "Staged changes can't be discarded: unstage them first." (instruction), vs "<path> has a conflict: it can't be discarded. Stage it to mark it resolved." (instruction), vs "Nothing was discarded: <engine why>" (consequence). Four voices for one family of refusals; only the staged one is in the ratified decision.
- Clean: one template: "<what> can't be discarded: <one short reason>. <one short way forward>."

### M3. A mixed selection is refused as a whole, naming only the first offender
- `local_changes_menu.rs:62-80` `no_discard` returns the first submodule/conflict found; `local_changes_actions.rs:discard` then refuses the entire discard. Select 30 modified files + 1 submodule -> Backspace -> the line under the lists names the submodule and nothing happens; the user must find and ctrl-click it off, in a list that may be scrolled.
- Fork asks twice (once per kind). Not a recorded deviation (ui.md row covers "offers no discard" for a submodule row, not "blocks the others").
- Clean: discard the discardable rows and say what was skipped, or disable with a count ("1 of 31 can't be discarded").

### M4. Selection is silently dropped by any refresh (focus gain / any write)
- `diff_state/working.rs:196-198`: `working_drawn()` is "another [number] each time an answer is kept", and a line selection "made under another number is nothing" (`staging_gesture.rs` `phase_under`). A refresh runs on window focus gain and after every write.
- User experience: drag-select 6 lines, Alt-Tab to the editor, come back: selection gone (and any visible tint), Return now stages the whole file. Safe for correctness (the phase 08 QA item 1 reason) but silent, and it turns a focus-return into a data-loss-of-gesture. Fork evidence is silent on refresh-while-selected.
- Root cause: identity of the answer is a counter, not its content. Clean: keep the selection when the re-read diff text is identical; drop (and say nothing is lost) only when the lines actually changed.

### M5. Files drawn together: stale-looking message with jargon, and features that vanish
- `diff_state/together.rs:37`: "Not shown: the files above it used the diff's line budget. Choose it alone to see its diff." "line budget" is an internal term (it comes from Expand All); the user has no budget concept. Plus ui.md rows: no Load Diff, no mode row, no previous/next change when drawn together (user-ratified), so features appear/disappear with selection size. Fork's bound is "not recorded" (ui.md says so) — this is Cairn-invented behaviour driven by an internal performance rule.
- Clean: "Too much to show here — select this file alone to see its diff." And ideally lazily virtualise rather than refuse (the diff list is already virtualised; the cost is reading, not drawing).

### M6. The header fakes a file to say "N files"
- `local_changes_pane.rs:together_side`: `RepoPath::from(format!("{count} files"))` fed to `header_file(None, ..)` which fabricates a `ChangedFile { status: Modified, old_path == new_path, ... }`, so the diff bar draws a Modified file named "3 files" (with whatever badge/rename logic the bar has). (Not verified: what the bar draws for that fabricated Modified status; the smell is the fabrication itself.)
- Root cause: the bar has one constructor (a `ChangedFile`); a "group" header was shoehorned in. Clean: a bar variant for a group (title only, no file semantics), as the Commit-tab compare header ("names both and a swap") already does.

### M7. Stage All / Unstage All discoverability is asymmetric; on Linux Unstage All has no visible control
- `local_changes.rs` ListSection render: the double chevron exists only in Unstaged (`list == Unstaged`); in Staged the only mouse route to Unstage All is Alt+click on the "Unstage" button, which `docs/systems/local-changes.md` itself says "never reaches Cairn" on xfwm/openbox/Plasma 5. So Linux users get Unstage All only via the context menu or chord. Recorded as a user decision (fails safe), and Fork's Mac flips the chevron — but the stated Linux reality means one of two symmetric operations is menu-only.
- Clean: a Staged-side counterpart (Fork Mac flips the chevron) so both lists have a visible, non-modifier route; the "button changes caption under a held key" mechanism is the hidden-state part (the label morphs as Alt is held; on a WM that eats Alt it never does).

### M8. Four spellings of every write, none shared
- `worker/local_lane.rs:290-448`: `what()` gerund lowercase ("staging 1 file") capitalised by `status_text::capitalised` for the strip; `name()` imperative ("Stage 1 file") for the popover; `awaited()` ("the commit to finish"); `Done.description` past tense ("staged 1 file"). One concept, four hand-maintained matches over the same enum, with `"discarding files"` losing its count while `"Discard 3 files"` keeps it.
- Clean: one noun phrase per write + the verb forms generated, or just the one imperative name ("Stage 1 file") plus a status ("Running", "Waiting", "Failed").

---

## LOW

- L1. `local_changes_actions.rs:222-227` `dialog_open` / `Acting::quiet()`: a refusal "stays until the next action" — but a *refusal text* that stays after the user has long since clicked elsewhere (selection changes call `quiet`, hovers/scrolls/filters don't). Minor; part of H2.
- L2. Discard wording ladder: floating `Discard Changes…` (chunk), `Discard 2 Lines…` (selection), dialog title `Discard changes`, button `Discard 2 Lines` / `Discard Changes in N Files`. Each individually evidenced (Fork Mac/Win); together they are a mix of Fork-Mac title case and Fork-Win sentence case ("Discard changes" title + "Discard Changes…" menu item). Pick one platform's convention.
- L3. `local_changes_menu.rs`: Staged-list menu = Unstage, Unstage All, Copy Path. Fork's Staged menu contents are UNVERIFIED (research §1), so this is invented-but-minimal; fine, but should be said in the deviations list.
- L4. `Follow::ChooseFirst`: when a path vanishes because of an *external* change (e.g. committed from a terminal), the selection jumps to the **first** row of Unstaged; after Cairn's own stage it goes to the *nearest*. Fork selects "one when files appear" (#2333); the vanish case is unevidenced.
- L5. `READING_STATUS`, `NO_PATH_CHOSEN` ("Choose a path to see its diff."), `NO_LOCAL_CHANGES`, `NO_PATH_MATCHES` ("No path matches the filter."), `SPARSE_INDEX` ("...the git in use cannot read (git 2.32 and later can)."): "path" vs the Commit tab's "file"; sparse-index text is the engine's limit leaking as a user message (an unavoidable consequence of the git 2.30 floor, but the parenthetical is a changelog, not help).
- L6. The `status_text.rs` file named in the brief holds NO Local Changes text: it is the history/fetch/close text. The Local Changes strip text lives in `local_changes_actions.rs::acting_line`, borrowing `status_text::capitalised` and `why_it_failed` — odd dependency direction, evidence the strip is bolted on. (`closing_line` "Finishing commit… Closing again leaves it unfinished." is a user-decided string.)
- L7. Double-click acts on its own row only, collapsing a multi-selection to it (user-ratified 4(b)). Fork's behaviour unrecorded; it means double-click is the one stage route that ignores the selection.

---

## Where Cairn differs from Fork — recorded vs not

Recorded with reasons (ui.md "What changes, and why"; fine): Cancel-default discard dialog; Ignore Whitespace off; counts-by-kind prompt; dialog names first three files; stacked diffs under budget; no Load Diff/mode row/prev-next when stacked; floating actions pinned at list top; press-without-drag clears selection; staged-discard refusal message; submodule row no discard; nested repo refused before dialog; Create Branch discard confirmed.

NOT recorded / invented:
1. The under-the-lists strip and the "(queued)" / "(1 more queued)" progress vocabulary (H2). Fork: status box + Git Error modal.
2. A mixed selection with a submodule/conflict is refused whole (M3). Fork asks twice.
3. Stage All's Staged counterpart missing on Linux in practice (M7).
4. Staged list menu contents (L3). Fork: unverified.
5. Selection dropped on refresh by answer number (M4). Fork: unrecorded.
6. Double-click ignoring the multi-selection (L7) is a user decision, but its Fork parity is unrecorded.

Fork behaviour UNKNOWN where Cairn picked something: nearest-with-gaps rule (R8.3, ratified), the Alt press on Linux (ratified), line budget bound (ratified), actions pinned at list top (ratified).

## Patch-over-root-cause list (summary)

| Symptom patched | Root cause | Clean design |
| --- | --- | --- |
| Selection returning "unseen" after a refresh; emptied toggle acting on chosen row; `Follow::*` | Two states: selection vs chosen path | One selection with a primary (H4) |
| Chord on diff acts on "files drawn" only | Render budget conflated with action scope | Chords act on the selection (H3) |
| Stale line selection applied to re-read diff (QA item 1) | Selection tied to row numbers of "an answer" | Selection tied to content; keep if equal (M4) |
| "(queued)" text, stale/refused ending wrappers | R8.6 invented a per-view progress channel | Use status box + modal (H2) |
| "N files" fake ChangedFile | Bar takes only a file | Group header variant (M6) |
| Alt+click Stage All fails on 3 WMs; button label morphs | Hidden modifier state as the only mouse route to Unstage All | Visible control both lists (M7) |

## What is GOOD (be fair)

1. The five stage routes all converge on one function (`stage_or_unstage`) and one R8.3 rule (`next_after`/`nearest_remaining`) — a real clean design, Fork-evidenced (Tracker #514).
2. Discard has a single funnel (`discard` -> `DiscardConsequence` -> `Confirming` -> token): no route skips the dialog; staged/submodule/conflict refusals share one function, `no_discard`, used by the menu, chord and gesture alike.
3. Dropping on a list zone (not a row), tracked by the lists, with edge auto-scroll and drag cancel on focus loss/Escape: a thoughtful, well-documented drag design, with the Linux window-manager Alt problem found and written down honestly.
4. Hover-is-not-selecting (Fork #103) is honoured: chords act on the whole file unless lines are selected.
5. Stage All / Unstage All with a filter take exactly the rows the filter shows (user decision) — avoids staging unseen rows.
6. Staged-discard says why instead of doing nothing (a justified, recorded improvement on Fork).
7. Badges as shapes + colour reinforcement; flat lists with virtualisation and end-room for the scrollbar.
8. Actions hide after a press until the diff is redrawn (Fork #480), preventing double-acting.
9. Docs are unusually honest: `local-changes.md` "Where it is not Fork's, and other limits" lists residuals (lost-release drag, Fork's Mac chevron flip, Alt grab) instead of hiding them.

## Not examined (out of budget / out of surface)

Commit box, confirmation dialog copy (`Consequence::prompt`), activity popover text, Create Branch — other surfaces. I did not run the application; all findings are from source and docs. M1 is estimated from character counts (12px font, 300px / list-column widths), not measured.
