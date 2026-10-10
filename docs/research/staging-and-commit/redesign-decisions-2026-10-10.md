# The staging redesign: the user's decisions (2026-10-10)

Evidence, saved in full and never edited after. Three parts, in the order they were given:

1. The user's answers to the design page (`redesign-page-2026-10-10.html`, saved beside this
   file), as the user pasted them from the page's Copy answers, verbatim, with the note the
   coordinator wrote under them and the change made after the Fork check
   (`fork-observed-2026-10-10.md`).
2. The design rules 1-8 the user approved, verbatim from the page.
3. The user's decision on how Create Branch's Discard is confirmed, relayed by the coordinator
   in the user's words.

The inputs behind the page: the five review reports (`review-*.md`) and the four design briefs
(`brief-*.md`) in this directory.

---

## 1. The answers, verbatim

# The user's answers to the staging redesign (2026-10-10)

Given in chat, verbatim from the design page's Copy answers. Fork investigation to follow from the VM.

Rules 1-8: all approved.

L1 A (primary file's diff, "k of n selected") · L2 A (Shift on Linux, ⌥ on macOS, plus ⇈ in Staged) ·
L3 A (selection shrinks to shown rows) · L4 A (kept while diff text unchanged, else cleared with one notice) ·
L5 A (row that took its place)

C1 A (dialog at the press, only when pushed or no reflog) · C2 A (git stripspace --strip-comments at pre-fill;
new read approved by choosing it) · C3 A (Commit concludes a single cherry-pick/revert) · C4 A (skip on every
failed commit or amend) · C5 A (keep Cancel) · C6 A ("Amend 3f2a1c9") · C7 A (pre-fill only an untouched empty
draft) · C8 A (one line: "HEAD is detached: this commit will be on no branch.")

D1 A (discard what can be, one line says what's left) · D2 A (one sentence frame + Show files) · D3 A (Title Case)

B1 A3 (tracked discard, then plain git checkout -b, all or nothing via stash create/apply --index)
B2 CHANGED: no confirmation for Create Branch's Discard, as Fork did in 2018
B3 CHANGED: "Create and Checkout" (Fork's caption)
B4 A (Stash and reapply shown greyed: "Comes with stashing.")

F1 A (spinner after 250 ms) · F2 A ("Couldn't <name>", one sentence, no Error Details) · F3 A (confirmed prompt
whole, wrapped, only for operations that ran) · F4 A ("Show in Lost Commits") · F5 A (retry ~1 s once or twice) ·
F6 A (lock banner: exists, no Cairn git, older than ~10 s) · F7 CHANGED: keep "HH:MM:SS UTC" everywhere ·
F8 A (keep duration) · F9 A (fetch progress in the status box, Fetch greyed while running)

Driving the VM: the user is happy either way (GUI driving, including typing in its terminal, or a permission rule).

Note for the rebuild: B2 (no confirmation) means Create Branch's Discard builds no `Confirmed`; it is still
destructive (rule 2 says an unrecoverable operation confirms) — the user chose Fork's 2018 behaviour knowingly;
record it as their deviation from rule 2, and the destructive roster/`create_branch_discarding` must change with it.

## After the Fork VM check (2026-10-10)
B1 CHANGED to Fork's: Create Branch's Discard runs Fork's command, `git checkout --no-track -b <name> <commit> --force`
(observed in Fork for Windows 2.21.1's Activity Manager), so untracked files in the way are discarded too — "we'd want
untracked files discarded too". With B2 (no confirmation) and B3 ("Create and Checkout"), this is Fork's flow exactly.
Applied as Fork under rule 1 without asking (Fork observed): diff shows the file clicked first; Ctrl+S (not Return)
stages from the diff; each activity row carries Fork's result line. Kept as the user's deviations: L4, C1, F6/G.

---

## 2. The design rules, as approved ("Rules 1-8: all approved")

1. **Fork first, checked at the source.** Where Fork's behaviour is known, Cairn does the same.
   Every "as Fork does" cites the Fork issue or release note it comes from.
2. **One confirmation rule.** An operation you can't undo, or one that rewrites history someone
   else may have, always gets the same confirmation dialog: Cancel has focus, and Escape goes
   back one step. An operation you can recover from asks nothing. The button and the keyboard
   chord always do the same thing.
3. **Ask the cost at the press.** Cairn works out what an operation will destroy when you press
   its button, not on every refresh beforehand. No "reading…" button states.
4. **One home for each kind of message.** Progress shows in the title-bar status box. A git
   failure opens Fork's Git Error dialog. A refusal Cairn finds while running opens the same
   dialog, titled "Couldn't <name>". A refusal Cairn knows in advance shows as a greyed control
   with its reason beside it. Success says nothing. A cancel opens no dialog. There is no line
   under the file lists.
5. **One name per operation.** Fork's imperative name ("Stage 2 files", "Commit", "Create
   branch 'x'") everywhere it appears. Four status words: running, succeeded, failed, cancelled.
6. **Short text, never cut.** Never cut off text you need to read; make it short instead. Never
   repeat what's already on screen. Per-file detail goes behind "Show files".
7. **Let git decide.** Run git and show its refusal, rather than predicting what git would do.
   The message you see is the message that gets committed.
8. **One selection, one scope.** You select files in one list. The diff shows one file. Every
   route — chord, button, drag, menu — acts on what's selected, wherever focus is.

The page also recorded that three earlier decisions rested on Fork claims its tracker
contradicts: several files' diffs drawn together (Fork shows one file: fork-dev/Tracker #261,
TrackerWin #786); Alt for Stage All (Fork's key is Shift: TrackerWin #2429); and `MERGE_MSG`'s
`# Conflicts:` lines committed (Fork's open bug since 2017, Tracker #180, not a design).

---

## 3. Create Branch's Discard: how it is confirmed (2026-10-10)

The user's decision, in the user's words, as the coordinator relayed it: "Clicking the button
when I've already selected discard is the confirmation. Asking again will just annoy the user."

As the coordinator wrote it down with the decision: the Create Branch dialog becomes a
confirmation surface on the `CONFIRMATION_SURFACES` roster: with Discard selected, pressing
Create and Checkout, or Return in the dialog, IS the acknowledgement that builds the
`Confirmed`. No second dialog, no prediction of what git deletes. The `Consequence` is fixed and
generic, naming no files: it discards local changes and any untracked files in the way, then
checks out <name> at <commit>. The operation stays on `DESTRUCTIVE_OPERATIONS`. It runs Fork's
command, `git checkout --no-track -b <name> <commit> --force`, and its re-check covers only that
HEAD, the commit and the name are unchanged. The ⚠ beside Discard stays, as Fork has it. The
`checkout_discarding_consequence` prediction, `reads/untracked.rs` (if nothing else uses it) and
the extra loss kinds go. Rule 2 holds: the confirmation is the deliberate radio choice plus the
press, as in Fork.
