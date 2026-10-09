# Fork on a merge with nothing staged, a cleared merge message, and an amend over missing objects

Evidence record. Date: 2026-10-09. Commissioned by: staging-and-commit phase 09's QA, for the
user's decisions on the commit box's four open items. Historical: never retro-edited. A
companion to `fork-staging-and-commit.md` (2026-10-07), which it adds to and does not repeat.

## Method and sources

Documentary, as the companion's: `gh` searches over `fork-dev/Tracker` and `fork-dev/TrackerWin`,
and both release-notes pages. Fork has no Linux build, so nothing was installed or clicked.
Labels as the companion's: VENDOR (Fork's developers), USER, RN (release notes), INFERRED
(reasoned, not documented), UNVERIFIED.

## 1. A merge in progress with nothing staged: Commit is enabled, and concludes the merge

- USER Tracker #90 (2017-09-05 to 2017-10-03), "[Bug] Commit button is disabled when merge with
  no file changed" (<https://github.com/fork-dev/Tracker/issues/90>), quoting git's "All
  conflicts fixed but you are still merging. (use "git commit" to conclude merge)"; milestone
  1.0.57.
- VENDOR Tracker #110, "[Prerelease] Fork 1.0.57" (2017-09-29), lists "[BUG] Commit button is
  disabled when merge with no file changed #90"; the vendor's "fixed #90" on 2017-10-02
  (<https://github.com/fork-dev/Tracker/issues/110#issuecomment-333596630>); the reporter
  confirmed on 2017-10-03.
- Windows: UNVERIFIED (Fork for Windows shipped in 2018, after the fix); INFERRED the same. No
  regression report since 2017.
- The subject rule is unaffected: the subject is required (Tracker #1490; VENDOR TrackerWin #637,
  2020). Empty commits outside a merge are refused (VENDOR TrackerWin #2288).

## 2. A pre-filled merge message the user clears: not documented

- RN Mac 1.0.16 (2016-08-05): "Use original git commit message when merge/revert/cherrypick is in
  progress."
- VENDOR Tracker #155 (2017-11-08): the message is loaded from `MERGE_MSG`
  (<https://github.com/fork-dev/Tracker/issues/155#issuecomment-342845294>).
- Withdrawn when the merge state ends: RN Mac 1.0.78 "Merge commit message stays after resolving";
  RN Win 1.33 "Commit message remains after merge conflicts" (Tracker #61).
- VENDOR Tracker #61 (2019-05-31): an auto-filled message should not persist "until you edited the
  message" (<https://github.com/fork-dev/Tracker/issues/61#issuecomment-497618184>). INFERRED: an
  edited message, a cleared one included, is kept rather than loaded again.
- A pre-fill overwriting the user's edits has been treated as a bug: Tracker #548 (2019); Tracker
  #1109, fixed in Mac 1.0.97 (2020); TrackerWin #338, fixed in Win 1.54.

## 3. Amend when the amended commit's objects are missing (a partial clone): no evidence

- VENDOR Tracker #1829 (open): Fork has no partial-clone support in its UI — "99.99% of users
  don't need that. Use command line"
  (<https://github.com/fork-dev/Tracker/issues/1829#issuecomment-1937817388>).
- Tracker #2042 (2024): Fork 2.38 reads objects in-process ("Bt error: Failed to find object").
  INFERRED: Fork's in-process reads fail on a missing object, while the commands it runs through
  git would fetch it lazily; nothing says Fork sets `GIT_NO_LAZY_FETCH`.
- Amend lists the previous commit's files among the staged (Mac 1.0.44, VENDOR Tracker #10); the
  mechanism is unstated. TrackerWin #2637 (2025): amend mode lags in large repositories.

## Open questions the owner can check on Fork for macOS

1. A cleared merge message: start a merge that conflicts, clear the pre-filled message, move focus
   away and back, and press F5 — does the message stay empty, or is `MERGE_MSG` loaded again?
2. A merge with nothing staged on Fork for Windows: is Commit enabled, concluding the merge, as
   Fork for macOS has done since 1.0.57?
