# QA checklist — staging-and-commit

Packet-specific acceptance beyond the repo-wide gate. The acceptance criteria
C1-C34 live in `docs/prd/staging-and-commit.md` and are NOT copied here — verify
them there, against their pinned tests.

## Per-phase coverage of the PRD criteria

| Phase | PRD criteria it must satisfy |
| --- | --- |
| 01 | C1; C22 (the two rosters' twins) |
| 02 | C3, C4, C7 |
| 03 | C2 (discard and delete), C5, C6, C8 (engine), C9; C22 (D1's reads and verbs); git's baseline for C21 recorded |
| 04 | C10, C11, C12 — their commit-dependent halves (a stage waiting behind a running commit, no refresh during a commit, a close during a commit, the SSH-signing prompt) against a stub git's long-running write |
| 05 | C2 (amend), C13 (engine), C14 (engine), C24 (engine: the operation in progress, the merge commit, refusals); C10, C11 and C12's commit-dependent halves re-run against real `git commit` |
| 06 | C15, C16, C17; C22 (the chord lists and the bare-key scope) |
| 07 | C8 (views), C18, C24 (views: conflicted rows) |
| 08 | C19; C22 (the gesture's viewport twin) |
| 09 | C13 (views), C14 (views), C24 (views: the commit box during a merge, rebase, cherry-pick or revert) |
| 10 | C20 (Show Lost Commits, Create Branch: its dialog, its kept checkout and its sealed Discard) |
| 11 | C2 (lock), C20 (the activity popover, `Remove index.lock…`, the filesystem-mutation guard), C21; C22 (D1's one deletion and the filesystem-mutation guard's twin) |
| 12 | C31 |
| 13 | C13, C14 and C24 (engine), C32 (stripspace and config), C33 (engine) |
| 14 | C34; C32 (the branch name); C1 and C22 (the confirmation-surface roster) |
| 15 | C17; C8 (views) |
| 16 | C18, C25 (but the diff's half), C27 |
| 17 | C16 (the narrowed bare-key scope), C19, C25, C26 |
| 18 | C13, C14, C24 and C33 (views); C1 and C22 (the commit box off the roster) |
| 19 | C28, C30; C11 |
| 20 | C29; C21's window check re-run |
| 21 | all of C1-C34, re-verified over the whole packet diff; C21 re-measured (R13.3) |

## Packet-specific checks

Beyond the PRD, phase 21 (the merge bar) confirms:

- [ ] **Every patch oracle is git or an independent derivation.** C3 compares
      against the reference applier, the mirrored-rule derivation and what real
      `git diff` shows afterwards — never against a pasted earlier output of
      Cairn's.
- [ ] **No prompt is typed.** Every destructive prompt and button label is
      rendered from a `Consequence`; grep the render crates for prompt literals
      that name a count or a path.
- [ ] **Each `Consequence` is re-checked against the state it names**, not
      against a copy the window kept: the re-check reads the repository.
- [ ] **Nothing discards a staged change**, by any route — gesture, chord, menu,
      dialog — but the one stated exception (the user's decision 3, 2026-10-09, and the
      user's decisions of 2026-10-10; PRD R3.6, R1.5, R11.3): Create Branch's "Discard",
      `ops::create_branch_discarding`, Fork's `git checkout -q --no-track -f -b <name> <oid>
      --`, sealed behind its fixed `Consequence::CheckoutDiscarding`, its token built only by
      the Create Branch dialog's press with Discard chosen, on `DESTRUCTIVE_OPERATIONS`,
      re-checked (`HEAD`, the commit, the name) before it runs, and never the remembered
      choice. No other route reaches `checkout -f`, `reset --hard` or a staged change's
      discard.
- [ ] **Every operation that cannot be undone or rewrites shared history confirms, and
      nothing else does** (the redesign's rule 2): the discard dialog, the amend dialog
      exactly when a remote has `HEAD` or no reflog, Create Branch's Discard press, the lock's
      removal — and no dialog for a stage, an unstage, a commit or a recoverable amend. A
      button and its chord do the same thing in every state.
- [ ] **No text a person needs is cut** (rule 6): grep the render crates for `max_lines`
      and `TextOverflow::Ellipsis` and read each — none on a prompt, a reason, a refusal or an
      operation line.
- [ ] **Each message has one home** (rule 4): no line under the lists, no operation banner,
      no Git Error with an empty command; every ending of every write lands in exactly one of
      the status box, Git Error, "Couldn't <name>", or nothing.
- [ ] **One name per operation** (rule 5): grep for a second wording table of writes.
- [ ] **Every removal the phase docs of 12-20 list under "What goes" is gone**, and every
      review refactor they mark "follow-up issue at teardown" is in state.md's filing list.
- [ ] **The redesign's storyboards match the window**
      (`docs/research/staging-and-commit/redesign-page-2026-10-10.html`, as the user's answers
      and `fork-observed-2026-10-10.md` changed them).
- [ ] **Every write is built in `ops/` as a write invocation, every new read in
      `reads/` as a read invocation**; no read writes a byte of the index
      (`hash-object` without `-w`).
- [ ] **No patch carries `-R`, a whitespace-ignoring range or a context other than
      three lines**; every `apply` carries `--whitespace=nowarn`.
- [ ] **The message never reaches `argv` or the command log**; the operation log
      holds what the user typed only through what git echoed.
- [ ] **No credential reaches the activity popover**: a URL with userinfo in
      stderr is scrubbed; a prompt's answer never appears.
- [ ] **The Commit and Changes tabs draw no staging action** and no bare-key chord
      fires outside Local Changes' scope.
- [ ] **Every new list or overlay is virtualized or bounded by the viewport**; the
      `ScrollView` exceptions roster is still empty.
- [ ] **No new waiting primitive on a render path**: the auto-scroll timer, the status
      box's 250 ms timer and the dialog wait on nothing.
- [ ] **Every deviation from Fork is one the PRD and `docs/design/ui.md`'s
      "What changes, and why" table name** (the PRD's product rules list them all)
      and no other crept in.
- [ ] **Every out-of-scope item in the PRD is filed** as an issue at teardown, and
      packet 5b's brief in the roadmap still matches what this packet left for it.
