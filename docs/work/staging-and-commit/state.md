# State — staging-and-commit

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 01 (the seal) done on `feature/staging-and-commit` in packet
mode: QA adjudicated, every confirmed fix and the user's three decisions of
2026-10-08 applied, full gate green. Phase 02 (the patch engine) done in packet
mode: QA adjudicated, confirmed fixes applied, full gate green; items 10-11 batched for
the user's review at the end of the packet. Phase 03 (the write verbs) built in packet
mode: QA adjudicated, confirmed fixes and the user's four decisions (2, 3, 5, 6) applied,
C21's margin decided (a flat 50 ms) and amended, full gate green. Phase 04 (the local
lane) done in packet mode: QA adjudicated, confirmed fixes and the user's decisions 12 and
14 (2026-10-09) applied, full gate green. Phase 05 (the commit engine) done in packet
mode: QA adjudicated, fixes and the user's decisions A-F (2026-10-09) applied, full gate green
at c07c076. Phase 06 (render foundations) done in packet mode: QA adjudicated, fixes and the
user's decisions 6, 12, 13 and 14 (2026-10-09) applied, full gate green. Phase 07 (Local
Changes acts on files) done in packet mode: QA adjudicated, fixes and the user's decisions of
2026-10-09 applied, full gate green. Phase 08 (the diff's staging gesture, and a
multi-selection's diffs drawn together) done in packet mode: QA adjudicated, fixes and the
user's decisions (2026-10-09) applied, full gate green. Phase 09 (the commit box) done in
packet mode: QA adjudicated, fixes and the user's decisions (2026-10-09) applied, full gate green
at 18fea48. Phase 10 (Show Lost Commits) done in packet mode: Show Lost Commits with its check
box, and Fork's Create Branch dialog with its sealed Discard, as the user decided (2026-10-09);
QA adjudicated, fixes and the user's decisions A-F (2026-10-09) applied, full gate green.
Phase 11 (the activity popover and the measured bar) done in packet mode: QA adjudicated, every
fix-now item fixed, and the user's decisions A-N on the popover (2026-10-09) applied — DO3+DO5
resolved by H; full gate green. Phase 12 (the merge bar): QA adjudicated, every fix-now item and
the probe's verdict applied, C21 re-measured on the tip; its four user decisions (W1, W3, F2,
F7) since resolved, listed under "Phase 12, the merge bar" below; everything to file at teardown gathered in one
list there. Then (2026-10-10) an independent review, a design pass and the user's redesign
decisions: design pass done, rebuild planned — phases 12-20 rebuild what the redesign changes and
the merge bar is now phase 21 (`phase-21-qa.md`, the doc the first merge bar ran under as phase
12); see "The redesign" below. The user's answers to the six open questions recorded
(2026-10-10). Phase 12 of the rebuild (git's output read once) built in packet mode, full gate
green; its QA is the coordinator's. Next: phase 12's QA, then phase 13
(`phase-13-commit-asks-git.md`).**

## The redesign (2026-10-10)

- **Why.** Five fresh reviewers found the UI grown by patching each QA finding where it showed
  (`docs/research/staging-and-commit/review-*.md`); four design briefs (`brief-*.md`) and a page
  of rules, storyboards and choices (`redesign-page-2026-10-10.html`) followed; the user checked
  Fork for Windows 2.21.1 first-hand (`fork-observed-2026-10-10.md`) and decided
  (`redesign-decisions-2026-10-10.md`): rules 1-8 approved; every choice answered; B1 changed to
  Fork's forced checkout; Create Branch's Discard confirmed by the dialog's press. The PRD is
  amended in place (dated 2026-10-10) with C25-C34 added; `docs/design/ui.md` and
  `docs/design/engine.md` are rewritten to it.
- **Superseded, on false Fork claims**: files drawn together (R8.1, phase 07's 4(e), phase 08's
  ratification); Alt for Stage All (R8.2's 4(d)); `MERGE_MSG`'s comment lines committed
  (decision E).
- **The merge bar's pending decisions**: W1 (the amend line's cap) and W3 (Create Branch's
  prompt) are resolved by the redesign — the line goes (C1, C6) and the prompt predicts nothing
  (B1, B2). F2 (R2.1's wording against `TextDiff::inverted`, and a staged-rename row for C21) and
  F7 (Escape as a literal) — resolved 2026-10-10 by the user's answers (6, below).
- **Phases**: 12 output once; 13 the commit engine asks git; 14 Create Branch; 15 the discard
  prompts; 16 one selection; 17 one file's diff; 18 the commit box; 19 one home for each
  message; 20 the Activity Manager; 21 the merge bar.

### Open for the user — all resolved 2026-10-10

The user answered "Approve" to each of the planning agent's recommendations (recorded in the
PRD's opening, "The planning agent's open questions, answered 2026-10-10", and in each
requirement it touches; phases 13, 15, 17, 18, 19 and 20 amended to match):

- ~~**A confirmed amend's skip**~~ — **resolved (1)**: option (a), the failed run hands the
  unspent token back and Skip retries without asking again; the `CONFIRMED_HOLDERS` rows for the
  engine's error (phase 13) and the window's Git Error state (phase 18).
- ~~**"k of n selected"**~~ — **resolved (2)**: no count in the diff's bar; Fork shows none
  (`lc2`) and the list's highlighted rows show the selection. L1 otherwise stands.
- ~~**The amend button's caption**~~ — **resolved (3)**: Fork's "Amend Last Commit" (`amd2`),
  replacing C6's "Amend 3f2a1c9".
- ~~**Title Case against decision G**~~ — **resolved (4)**: "Remove Stale Lock"; G's prompt
  unchanged.
- ~~**Fork's All / User / Background tabs**~~ — **resolved (5)**: filed as an issue at teardown
  (in the one list below); Cairn has no background operations yet.
- ~~**F2 and F7**~~ — **resolved (6)**: R2.1 amended to describe `TextDiff::inverted` (a
  replacement inverts into its insertion then its removal, as `git reset -p` leaves it); C21
  gains no staged-rename row, a measurement issue filed at teardown; Escape matched as a literal
  key is an allowed convention, its sites named in PRD R7.2.

## Phase 12 of the rebuild, git's output read once (2026-10-10)

- **Built** (`phase-12-output-once.md`, C31): the live bug first — a stderr line longer than the
  piece limit with a character straddling it arrived as U+FFFD (`pipes.rs`'s lossy piece);
  `a_character_straddling_the_piece_limit_arrives_whole` failed on it, then passed. One line type
  (`pipes::Lines`) for stdout and stderr, split at `\n` and `\r` on both (decided once: git's
  progress is stderr's and is split as before; a commit's stdout now splits at `\r` too), pieces
  of `PIECE_BYTES` cut where a character ends; each line scrubbed as it is split
  (`Scrubber::line`, `Scrubber::piece` carrying only across a line's pieces); the tail
  (`pipes::Tail`) whole lines, `ScrubbedLines::older_dropped` a plain bool. `ops/commit.rs` drives
  `Invocation::lines`, so a commit's stdout and stderr reach the failure and the command log as
  the runner's lines, in arrival order. git's text crosses into cairn-app only as
  `cairn_model::ScrubbedLines` (`GitFailed::stderr`, `CommandRecord::stderr`, `CommitWatch::output`,
  fetch's progress, `Update::WriteOutput`, `Update::FetchProgress`, `WriteEnding::Failed::output`);
  an error's text as `Error::shown`; arguments scrubbed once for the record and every runner error
  (`cli::scrubbed_arguments`). The type holds "no render file takes git's raw text" (compile_fail doctests in
  `scrub.rs`); no guard written, said in CLAUDE.md as a new invariant with its residuals.
- **Removed**: `pipes.rs`'s lossy piece decode and its second line definition (commit's `Lines`);
  `Scrubber::after_cut`, `Carried::Cut` and the cross-line carry; `scrub_userinfo`;
  `Error::GitFailed::stderr_cut` and the runner's `.then_some(0)`; `pipes::Retained`;
  `CommandRecord::stderr_cut`; `ops/commit.rs`'s `Lines`, `whole_characters`, `STDOUT_TAIL`,
  `joined_output` and `Polled` (a `&dyn Cancel` is a `Cancel` now, `cancel.rs`);
  `Invocation::finish_by_read` (`finish` hands a read's lines on together); `local_lane::scrubbed_error`
  and `ran_since`'s scrub; `network_lane`'s scrubber; `session`'s scrubs and `shown_output::
  shown_ending`; `shown_output::{ShownLines, shown_line, scrubbed, scrubbed_at}`; `commit_box_state`'s
  and `activity`'s `ShownLines`; the app tests pinning per-consumer scrubbing
  (`no_line_of_an_entry_carries_a_token`, `a_failures_cut_output_is_scrubbed_where_the_engine_says_it_was_cut`),
  moved to the type and the engine. `strip_ansi` moved to `shown_output.rs`.
- **Seen by a person, as the phase settles**: a hook redrawing with `\r` on stdout shows each
  redraw as a line; a commit's `$ git commit` entry in the popover keeps its stdout lines after it
  ends (they were stderr-only); a commit's kept failure output is in arrival order rather than
  stdout's tail ahead of stderr's. Nothing else drawn changes.
- **QA (2026-10-10), fixed**: #1 escapes stripped by the scrubber before it scrubs
  (`cairn_model::strip_ansi` moved beside it; a piece never cut inside an escape), the residual
  removed; #2 one scrubbed copy of the arguments for the record and every runner error; #4/#5 a
  piece's authority removed and carried whole, a line's authority ending at the line's end (its
  host kept); #6 stdout's unterminated last line pinned; #7 the hook test's docstring; #11
  `qa-checklist`'s item 12; #12 the invariant scoped to git's diagnostic output; #13 the
  scaffold names `ScrubbedLine`. Carried: #10 to phase 13, #15 and #16 to phase 20 (written into
  those docs); #8 in the teardown list. Dismissed: #3, #9, #14.
- **Carried**: `review-code-app-ui.md` H3 (b) one bounded tail and (c) the receipt protocol — phase
  20, as planned (`OutputFlow`, `OutputTail` and `ActivityLog` still bound separately, now over
  scrubbed lines); `WriteEnding`'s messages stay `String`s rendered in the lane from
  `Error::shown` — phase 19 rewrites the messages; #46 to be commented at teardown with what this closes (the engine keeps git's stderr
  scrubbed now).

## Phase 12, the merge bar (2026-10-09)

- **Fixed, each pinned** (progress.md, phase 12): C22 (`docs/design/engine.md` D1 names Create
  Branch's writes and three reads; `docs/systems/git-processes.md`'s tree lists them); C14's carried
  item; C2's symlink half; C19's tabs half; W2 (the prompt kept for every ending); gate-integrity 1
  (`CONFIRMED_HOLDERS` by file and type), 2 (the reviewer's checks 11 and 12, the qa-gate row), 3
  (the pin's examples inside assertions, the bare keys pinned exactly; the polarity residual
  stated), 4 (`RemovedKind`); responsiveness 1 (the three submit arms named) and 2 (stated in
  CLAUDE.md's residuals); W4 (`ops/stage.rs`'s comment); F1; F5.
- **Probe 7** (Show All's measured height): the probe passed, but its controls show the measure
  does reach the list in both directions — dismissed, and the other direction pinned
  (`a_prompt_shown_whole_sits_on_its_measured_height_not_its_estimate`).
- **The user's decisions pending** (W1 and W3 resolved by the redesign, F2 and F7 by the user's
  answers of 2026-10-10; kept as the record):
  W1 — the amend button's three-line cap can cut "can't be recovered" off a long subject; W3 —
  Create Branch's Discard prompt gives no size for a folder or nested repository and says nothing
  of ignored files; F2 — PRD R2.1's "spans swapped" against `TextDiff::inverted`'s
  insertion-then-removal split, and whether C21 gains a staged-rename row (phase 02's QA items 10
  and 11); F7 — Escape matched as a literal `NamedKey::Escape` (`diff_view.rs`, `stacked_diff.rs`,
  `local_changes_drag.rs`, `activity_popover.rs`).

### To file at teardown — the one list

Gathered from phase 11's carry, the PRD's "Not done" paragraph, the phase carries below and the
merge bar's QA; each is filed with the `file-issue` skill at teardown. An existing issue is
commented, not filed again.

- **From the PRD's "Not done"** (`docs/prd/staging-and-commit.md`): a backup before discard (L2);
  Fork's per-repository sign-off setting and the commit box showing author and signing;
  pre-filling an empty draft from `commit.template`; wrapping a paragraph at the ruler; a
  configurable subject limit; transcoding to a non-UTF-8 `i18n.commitEncoding`; a hook reading
  `/dev/tty` (L11's residual, R5.3); discarding a submodule's changes (L24); pinning
  `apply.ignoreWhitespace` on every `apply`; a persisted operation log; a Commit-tab "restore this
  file" from a lost commit; the reflog as a list of entries. Comment, not file: a forced close
  orphaning a process that holds `index.lock` (#48 item 2); the index refresh (#66); fetch's cancel
  tied to its operation (#47's remainder).
- **Local time** (the user's decision D, 2026-10-09): the popover's start times and the history's
  date column in UTC; local time needs a timezone dependency, the user's — one issue for both.
- **DO1**: the commit-graph reach treats generation 0 as a level, where git reads it as "not
  computed"; harden by answering `None` when the floor or a visited generation is 0.
- **DO2** (pre-existing): neither the graph route nor the object walk honours `refs/replace` or
  `info/grafts`, where git turns the commit-graph off when they are present.
- **DO6**: `reflogs::reflog_tips` takes an id the commit-graph holds as present without an object
  lookup, so a commit pruned after the graph was written is listed as lost and fails to open.
- **Fork's All / User / Background tabs** in the Activity Manager (the user's answer of
  2026-10-10, 5): observed `am1`; Cairn has no background operations yet.
- **A staged rename's measurement** for C21 (the user's answer of 2026-10-10, 6): stage, unstage
  and discard a hunk of a staged rename on the bench clone, beside C21's four.
- **A `lines` invocation unwound mid-run loses stdout from its log record** (phase 12's QA #8):
  the hand-off drop keeps only queued `Event::Lines` (stderr), and the reaper drives
  `StdoutSink::Bytes`, so a pending stdout line and queued stdout chunks never reach the command
  log — only on an unwind or an unfinished drop; a kill and a reap keep them. Fix: the reaper
  keeps a `Lines` sink for an invocation `Invocation::lines` drives.
- **GI7** (pre-existing): `code_without_test_modules` blanks the block after a `#[cfg(test)]` on a
  field, variant or arm, which would hide production code from every guard using it.
- **F3**: skipping the amend re-check's walk when the tips it read have not moved (phase 05's QA
  item 4, optional, carried to phases 09 and 11 and dropped there unrecorded); without a
  commit-graph the walk runs twice per amend. Performance.
- **Batching the discard count's per-path reads** (15.4 ms a path on the bench clone); a batched
  `numstat` would change what a prompt counts for an LFS pointer, a file past the limits and under
  `diff.algorithm`.
- **The engine's half of #46** (comment): the command log and `Error::GitFailed` keep git's stderr
  unscrubbed; every drawn line is scrubbed. (Phase 12 scrubs at the runner, before anything is
  kept; comment #46 with what it closes.)
- **Responsiveness 2**: while Show All is pressed, Freya's `VirtualScrollView` asks the size
  callback for every row above the viewport on each layout — bounded by `ACTIVITY_LINES` + 1, and
  stated in CLAUDE.md; laying the whole prompt out above the list would keep `ItemSize::Fixed`.
- **Responsiveness 3**: `activity::popover` reads the activity log on every window render, the
  popover closed included, so each streamed line re-renders the window's root; reading `is_open`
  in a child component would leave a closed popover unsubscribed. Within budget (C21, phase 12).
- ~~**F7, Escape as a literal**~~ — resolved 2026-10-10: an allowed convention, its sites named
  in PRD R7.2.
- **Responsiveness 2 and 3** above are taken by phase 20 (Show All goes; `is_open` read in a
  child); drop them from the list if it lands them.
- ~~**W3's residuals**~~ — resolved by the redesign (2026-10-10): Create Branch's Discard
  predicts nothing and its fixed prompt says untracked files in the way go (phase 14).
- **The review's refactors the rebuild does not take** (2026-10-10; each phase doc marks its
  own): `review-code-engine.md` H1 (one witness of a path's state), M1 (per-operation
  consequence types), M4's operation-in-progress and reflog parsers, M5 (amend's staged list as
  a fake status), M6 (lost-commit reach), L1-L7; `review-code-app-ui.md` H2 (one pointer-drag
  primitive), H4 (the write ledger), M1 (declarative lanes), M2's rest (one consequence ask),
  L1-L4 — one issue per family, unless a phase took it.
- **Fork's Activity Manager tabs** (All / User / Background), unless the user wants them built.
- **N3** (not adjudicated): discarding an intent-to-add file is prompted as "emptied"; check
  against real git (`add -N`, then discard) whether the prompt over- or understates.
- **GitHub, by the coordinator**: close #41; comment #46 and #87.


## Locked decisions

L1-L26 in `brainstorm.md`; the spec is `docs/prd/staging-and-commit.md`. The redesign of
2026-10-10 amends several below — L7 (bare Enter from the lists only; Shift for Stage All on
Linux), L12 (an amend confirms in the dialog, only when a remote has `HEAD` or no reflog; the
commit box builds no token), L13 (Create Branch's Discard is Fork's forced checkout, the
dialog's press its confirmation), L14 (Fork's Activity Manager), L15 (the lock as a state of
the repository) and L25 (a single cherry-pick or revert concluded; `MERGE_MSG` cleaned by
`git stripspace`) — and the PRD carries each amendment, dated. The ones that most constrain
implementation:

- **Stash and `.gitignore` are packet 5b's** (L1). No Clean command; deleting
  untracked files is discard on untracked rows (L8).
- **No backup before a discard** (L2, closes program O3). The confirmation is the
  only barrier, so it names exactly what is lost.
- **`Confirmed` carries an engine-computed `Consequence`**, has no `Clone`, is
  built only by the dialog and the commit box, and every destructive operation
  re-checks its `Consequence` before it runs (L3).
- **The local write lane**: its own thread, FIFO, op ids, a write counter over
  status, refresh by `Invalidated`, quiet while a commit runs (L4); close waits
  for a write (L15).
- **Patches**: invert the diff and reuse the forward emitter, no `-R` (L17a);
  untracked files partially; deletions, binary, LFS, oversize, submodules, type
  changes and conflicted paths whole, and a submodule never discarded (L24); a mode change its own item; C-quoted paths; a stale check before
  every apply (L17); `--whitespace=nowarn` always (L16); staged renames paired as
  git pairs them (L18).
- **Fork's UI**: the hover-and-drag gesture with `async-io` auto-scroll (L5); no
  Ignore Whitespace in Local Changes (L6); five file routes, Fork's keys with bare
  Enter/Backspace/Delete in one scope (L7); Fork's discard rules, Cancel focused
  (L8); Fork's commit box (L9); amend confirmed by its button, a dialog only when
  pushed (L12); Show Lost Commits with `Create Branch Here…` (L13); the Activity
  popover with prompts, session only (L14).
- **Commit** is `git commit -F -`, no `--cleanup` (L10); every local write
  carries an askpass token, and four display/GPG variables join the inherited
  roster (L11), with the five identity variables beside them and no date
  variable (L26).
- **An action maps to a list of chords** per platform, amending the modifier
  invariant's "at most one" (L22).
- **One mutation without `git`**: `Remove index.lock…` deletes exactly
  `<gitdir>/index.lock` in `ops/`, and a guard holds every filesystem-mutating
  call to `ops/` (L23).
- **Conflicts and a merge follow Fork**: a conflicted row stages whole (`git add`)
  with no lines or discard; a merge pre-fills `MERGE_MSG` and commits the merge,
  Amend disabled; rebase, cherry-pick or revert disable the box (L25).

## Open questions

- ~~C21's margin~~ — decided by the user on 2026-10-08: a flat 50 ms per verb over
  git's own time plus one status read (≈93 ms stage, ≈93 ms unstage, ≈76 ms discard,
  ≈87 ms commit); PRD C21 and R13.2 amended. Phase 11 measures Cairn against these.
- Fork's chunk-discard dialog default button and how Fork stages lines of an
  untracked file — the user may check on their own Fork; Cairn's choices (Cancel
  focused, a partial new-file patch) stand regardless.
- ~~Whether a `new file mode` patch applies against an intent-to-add entry~~ —
  settled in phase 02: it does, on git 2.30.9, 2.32.7 and 2.56.0, leaving a real
  entry with the lines selected (`a_new_file_patch_applies_over_an_intent_to_add_entry`,
  `crates/cairn-git/tests/diff/staging.rs`), so an intent-to-add path stages through
  the same rule as an untracked one.

## New modules and interfaces

Phase 01:

- **`cairn_model::Confirmed`** (`crates/cairn-model/src/confirm.rs`): two private
  fields, `consequence: Consequence` and `prompt: String`; derives `Debug`,
  `PartialEq`, `Eq` only — no `Clone`, `Copy` or `Default`. One constructor,
  `Confirmed::by_user(consequence: Consequence) -> Self`, which renders the prompt
  from the consequence; readers `consequence(&self) -> &Consequence` (for the
  operation's re-check) and `prompt(&self) -> &str` (for the log). Six
  `compile_fail` doctests beside a passing scaffold.
- **`cairn_model::Consequence`** (`crates/cairn-model/src/consequence.rs`), plain
  data, `Clone`, built by the engine with struct literals: `DiscardLines { path,
  index: Option<Oid>, working_tree: Oid, selection: Selection }` (the selection of
  the unstaged diff the patch is built from; the counts are its own); `DiscardFiles { files:
  Vec<DiscardedFile> }`, each `DiscardedFile { path, loss: FileLoss }`, `FileLoss`
  `Modified { index: Oid, working_tree: Option<Oid>, lines: Option<usize> }` or
  `Untracked { working_tree: Oid, bytes: u64 }`; `Amend { commit: Oid, subject,
  published: Publication, reflog: Reflog }`, `Publication` `Unpublished`,
  `Upstream(RefName)` or `SomeRemote`, `Reflog` `Written` or `NotWritten` (the
  prompt promises "stays in Show Lost Commits" only for `Written`, and otherwise
  says the old commit can't be recovered because the repository keeps no reflog —
  R10.6 as amended); `RemoveLock { path: PathBuf, modified: SystemTime, read_at:
  SystemTime, bytes: u64, device: u64, inode: u64 }` (the age rendered is
  `read_at - modified`, so the renderer reads no clock; mtime, size, device and
  inode are what the re-check compares). Every path and subject is escaped as git
  C-quotes a path (`quote_c_style`, `core.quotePath=false`), plus line separators
  and bidirectional controls; a file deleted in the working tree is named
  "deleted file restored". An operation derives every target from
  `confirmed.consequence()`, never from a parameter beside it.
  `Consequence::prompt()` renders the prompt (L8's words for a discard, R10.6's for
  an amend) and `Consequence::action()` the button's label (`Discard 2 Lines`,
  `Discard Changes in 3 Files`, `Amend <short>`, `Remove index.lock`). **Discarding
  files and deleting untracked files share `DiscardFiles`** — a deviation from the
  phase doc's five variants, because Fork confirms a mixed selection in one dialog
  with one prompt (L8), and one prompt is one confirmation; phase 03's operation
  for it restores the tracked files and deletes the untracked ones under one
  re-check. An untracked directory row is not modelled (status lists untracked
  files one per file; a nested repository is refused before any consequence);
  phase 03 extends the type, with a test, if it needs one. A mode change selected
  for discard is not modelled either (phase 02/03).
- **`ops::Performed::destructive(description, confirmed: Confirmed, invalidated)`**
  now takes the token by value and records `confirmed.prompt()` (R1.6): the record
  spends it. `ops::describe_destructive` remains the placeholder; phase 03 deletes
  it and its roster row.
- **The bounded-output helpers** `Invocation::collect` and `finish_within` are on
  `impl Invocation<Read>` alone (R4.8); a write is driven with `finish` or
  `records`. `Invocation::drive` takes `ceiling: Option<K::Ceiling>`, where
  `Kind::Ceiling` is `usize` for a read and `Infallible` for a write. `Error::GitOutputTooLarge` lost its `stranded_locks` field (only a
  read reaches it). The `#[cfg(test)]` `GitCommand::collected` gathers through
  `finish`, so tests still collect a write's output.
- **The guard's rosters** (`crates/cairn-guards/tests/invariants.rs`, in
  `destructive_operations_are_sealed_behind_the_confirmation_token`):
  `DESTRUCTIVE_OPERATIONS` (file, function) — phase 03 adds discard lines, discard
  files, phase 05 amend, phase 11 remove-lock, and 03 removes the
  `describe_destructive` row; `CONFIRMED_RECORD` (file, `Performed`,
  `destructive`: excused only inside `Performed`'s inherent impl);
  `CONFIRMED_HOLDERS` (files of the engine whose types may keep a `Confirmed` in a
  field) — empty, so a phase-04 lane request carrying a token lives in
  `cairn-app`, never in a `cairn-git` type;
  `CONFIRMATION_SURFACES` (files) — empty; phase 06 adds the dialog component,
  phase 09 the commit box. `CONSEQUENCE_BUILDERS` (`cairn-model`, `cairn-git`):
  production code anywhere else may hold a `Consequence` and call its methods but
  never spell `Consequence::` or its parts' names — **so phase 09, which must know
  whether an amend is published to decide on the dialog, adds a method to
  `Consequence` (for example `needs_force_push()`) rather than matching on
  `Publication` in `cairn-app`.**

Phase 02 (`docs/systems/diff.md`, "Stage, unstage and discard" and "A staged
rename or copy is paired as `git diff --cached` pairs it"):

- **`cairn_model::action_patch(PatchAction, &FileDiff, &Selection) -> Patch`**
  (`crates/cairn-model/src/action_patch.rs`), `PatchAction` `Stage | Unstage |
  Discard`: what each verb of R3.1-R3.3 carries. Give it the diff the selection was
  made on — the staged `FileDiff` for `Unstage`, the unstaged or untracked one
  otherwise — exactly as the engine answered it. Stage is `emit_patch` as drawn;
  unstage and discard are `emit_patch` over the inversion; apply with `git apply
  --cached --whitespace=nowarn -` (stage, unstage) or `git apply --whitespace=nowarn
  -` (discard), never `-R`. Empty patch = nothing to apply (nothing selected, part
  of a whole-file-only change, or a state that is not text): phase 03's verb must
  refuse an empty patch rather than run `git apply` on nothing, and route whole-file
  changes to its file verbs.
- **Inversions**: `TextDiff::inverted` (a replacement inverts into its insertion
  then its removal — see Decisions in progress.md), `Selection::inverted`,
  `ChangedFile::inverted` (a copy inverts to its destination deleted).
- **`Selection`'s mode item**: `select_mode`, `unselect_mode`, `holds_mode`;
  `with_every_change` selects lines only; `len` counts lines only; `is_empty` is
  false for the mode alone. A whole-file stage by patch must select the mode too.
- **Quoting**: `emit_patch` writes every path line as git's `quote_c_style`
  (`crates/cairn-model/src/c_quote.rs`, crate-private), with git's trailing tab on a
  `---`/`+++` label holding a space.
- **`crate::reads::staged_pairing`** (`crates/cairn-git/src/reads/working_tree.rs`)
  and `WorkingTreeQuery::paired`: the staged side of `Repository::working_tree_diff`
  now answers a staged rename or copy as git pairs it (status `Renamed`/`Copied`,
  both paths), one more read process under rename detection. A paired read that
  moved between its two reads is `ContentReadsDisagree` (the diff lane asks again).
- **Test harness for later phases**: `crates/cairn-git/tests/diff/staging.rs` builds
  each C3 case and checks an apply against three oracles; phase 03's verbs can
  reuse its cases (`edited`, `Case`) and oracles (`forward_rule`, `mirrored_rule`,
  `left_out`) to test through the real operations.

Phase 03 (`docs/systems/staging.md`):

- **The verbs** (`cairn_git::ops`, each `-> Result<Performed, Error>`, each taking
  `token: Option<&AskpassToken>` last, for phase 04's lane): `stage_lines(git, repo, diff,
  &Selection, token)` and `unstage_lines(..)` (give them the diff the selection was made on:
  unstaged/untracked for stage, staged for unstage); `stage_files(git, repo, &[RepoPath],
  token)`; `unstage_files(git, repo, &[RepoPath], &UnstageTo, token)` with `UnstageTo::Head`
  (`git reset -q`), `UnstageTo::Commit(Oid)` (out of an amend: `HEAD`'s parent) and
  `UnstageTo::Nothing` (out of a root commit's amend: `git rm --cached -q`, which git refuses
  where the staged content differs from both the file and `HEAD` — phase 05's to meet);
  `discard_lines(git, repo, Confirmed, token)` (it applies the patch the `Consequence`
  carries — no diff is passed) and `discard_files(git, repo, Confirmed, token)`, which
  answers `Error::DiscardIncomplete { performed, kept, failure }` when it did not take
  every confirmed file (the lane must log `performed`, whose prompt the user accepted).
- **The builders**: `discard_lines_consequence(git, repo, &FileDiff, Selection)` and
  `discard_files_consequence(git, repo, &[RepoPath])` — computed on a worker (they run
  `git`), each refusing before any prompt with `Error::Refused { path, why: Refusal }`
  (`NothingSelected`, `WholeFileOnly`, `Conflicted`, `Submodule`, `NestedRepository`,
  `NotAFile`, `Obstructed`, `NoUnstagedChange`, `NotWhatWasConfirmed`) or `Error::NoPaths`. A selection of
  every line of a new file is `WholeFileOnly`: phase 08 routes it to `discard_files`.
- **Outcomes to draw**: `Error::ChangedSinceRead { path }` (a stale patch, R3.7) and
  `Error::ChangedSinceConfirmed { path }` (R1.4), never git's failure; `Performed::locks()`
  (`Locks { before, after }`, R3.8); `Invalidated`: stage/unstage → index, discard lines →
  working tree, discard files → index and working tree.
- `ops::CLEAN_ARGUMENT_BYTES` (64 KiB): `git clean`'s paths per invocation.
- `crate::reads::hash_object` (used by the stale check) and `reads/hooks_path.rs`'s
  `hooks_path` — **phase 05**: re-export it from `reads/mod.rs` (it is module-private and
  `expect(dead_code)` until then).
- `Consequence::DiscardLines` gained `on_disk: Oid`, `executable: bool`, `mode:
  Option<(FileMode, FileMode)>` and `patch: Patch`; `FileLoss::Modified` gained
  `executable` and `mode`, `FileLoss::Untracked` `executable`, and `FileLoss::Emptied` is new
  (an intent-to-add file, which the discard leaves empty); every working-tree id is the bytes
  on disk hashed with no filter.

Phase 04 (`docs/systems/git-processes.md`, "The local write lane"):

- **`crate::worker::LocalWrite`** (`crates/cairn-app/src/worker/local_lane.rs`): `StageLines {
  diff: Box<FileDiff>, selection }`, `UnstageLines { .. }`, `StageFiles { paths }`,
  `UnstageFiles { paths, to: UnstageTarget }` (`Head`, `Commit(Oid)`, `Nothing`),
  `DiscardLines(Confirmed)`, `DiscardFiles(Confirmed)`, and `#[cfg(test)] HeldCommit {
  remote }`; `what()` names it ("staging 1 file", "commit"). **Phase 05 adds `Commit` and
  `Amend`**: `is_commit` true, `read_again` `Everything`, and `perform` calls
  `lane.install(id, Box::new(move || cancel.cancel()))` once git runs — the tests that drive
  `HeldCommit` are the ones to re-run against `git commit`.
- **Requests**: `Request::Write { id: OperationId, write }`, `Request::CancelWrite { id }`,
  `Request::RefreshStatus` (status alone). `Request` is no longer `Clone` (a test-only
  `Clone` refuses a destructive write). Ask a write with `local_writes::ask(&mut writes,
  submit, write)`, which takes the id and keeps it queued.
- **Updates**: `WriteStarted { id }`, `WriteEnded { id, ending: WriteEnding, read_again:
  ReadAgain }`; `LocksAtOpen { locks, index_lock }` (the lock files as the repository opened, listed by
  the lane before any write, and whether `<gitdir>/index.lock` is among them).
  `WriteEnding::{Done(Done), Stale { path, message }, Refused, Failed { message, locks },
  MayHaveTakenEffect { message, locks }, Incomplete { done, kept, message }, NotRun }`;
  `Done { description, acknowledged, locks_before, locks_after }`.
- **Window state**: `View::writes: State<LocalWrites>` (`crates/cairn-app/src/local_writes.rs`):
  `queued()`, `running()`, `last()`, `locks()`, `closing_on()`; `Closing::when_requested`.
- **`cairn_git::SharedRepository::lock_files(&impl Cancel) -> Option<Vec<PathBuf>>`**: every
  `*.lock` under the git directories, `None` once cancelled.
- **The environment twin** reads `INHERITED`: `INHERITED_PINS` (R5.2's nine, each with a
  comment of its own) and `INHERITED_NEVER` (no `*_DATE`).

Phase 05 (`docs/systems/staging.md`, "Commit and amend"; `docs/systems/git-processes.md`, "The
local write lane"):

- **`cairn_git::ops::commit(git, repo, message: &str, hooks: Hooks, token, watch:
  CommitWatch<'_>) -> Result<Performed, Error>`** and **`ops::amend(git, repo, confirmed:
  Confirmed, message, hooks, token, watch)`** (`crates/cairn-git/src/ops/commit.rs`): `git
  commit -q [--amend] [--no-verify] -F -`, the message on stdin, no `--literal-pathspecs`.
  `Hooks::{Run, Skip}` (`Skip` is `--no-verify`, the hook failure's skip only). `CommitWatch {
  cancel: &dyn Cancel, running: &mut dyn FnMut(CommitCancel), output: &mut dyn FnMut(&str) }`:
  `cancel` is polled before git starts (`Error::CommitCancelledBeforeRunning`), `running` is
  handed the `CommitCancel` as git starts, `output` every stdout and stderr line. A failure is
  `Error::GitFailed` whose `stderr` is stdout's tail then stderr's (git says "nothing to
  commit" on stdout). Refusals before git runs: `Error::CommitRefused { why: CommitRefusal }`
  — `InProgress(OperationInProgress)`, `NothingToAmend`, `CommitEncoding { encoding }`,
  `NotWhatWasConfirmed`. Amend re-computes its `Consequence` and answers
  `Error::AmendChangedSinceConfirmed` when it differs. Invalidates refs, index and objects.
- **`ops::amend_consequence(repo, cancel) -> Result<Consequence, Error>`**
  (`crates/cairn-git/src/ops/amend.rs`): `Consequence::Amend { commit, subject, published,
  reflog }` — the publication from the upstream (a remote-tracking ref that exists) or, when
  it does not hold `HEAD` or there is none, every remote-tracking ref (`walk::hiding`, now many hidden tips), the reflog as
  R6.4/R10.6 say. Rostered in `DESTRUCTIVE_OPERATIONS`.
- **Reads** (each `Repository` method): `operation_in_progress() -> Option<OperationInProgress>`
  (git's `wt_status` files, not gix's `state()`); `commit_hooks(git, cancel) -> CommitHooks`
  (`reads::hooks_path`, now re-exported, plus `access`-like bits); `recent_messages(cancel) ->
  Vec<String>` (`RECENT_MESSAGES` = 10, `git log`'s order); `amend_staged(git, cancel) ->
  Vec<ChangedFile>` (`reads::staged_since`: the index against `HEAD^` or the empty tree, the
  user's rename detection; `Error::UnbornHead` with no `HEAD`).
- **Model**: `cairn_model::OperationInProgress { Merge { message: Option<String> }, Rebase,
  ApplyingPatches, CherryPick, Revert }` with `refuses_commit`, `refuses_amend`, `name`;
  `cairn_model::CommitHooks { pre_commit, commit_msg }` with `skippable`.
- **Changed verbs**: `UnstageTo::Nothing` is `git rm --cached -f -q` (an amend's edited path);
  `git clean -f -q --`.
- **Askpass**: `Channel::begin_for(name)` names a token's operation; `Prompt::operation()`.
  `Update::Prompt { id, text, asking: Option<String> }`; `PromptView.asking`; the dialog is
  titled by it ("git" when `None`). `FetchStatus::remote_in_flight` is gone.
- **Lane**: `LocalWrite::Commit { message, skip_hooks }`, `LocalWrite::Amend { confirmed,
  message, skip_hooks }` (`what()` "commit", "amend"; both read everything again however they
  end). `Update::WriteOutput { id, line }` (no view draws it yet). `LocalWrite::HeldCommit` and
  the `deferred` override are gone; `LaneState::end(announce: impl FnOnce())`. A pre-git cancel
  ends `NotRun`; `CommitRefused` ends `Refused`; `AmendChangedSinceConfirmed` ends `Stale` at
  `HEAD`. The closing banner: "Finishing commit… Closing again leaves it unfinished."

Phase 06 (`docs/systems/diff.md`, "The accelerator table" and its two subsections):

- **`cairn_ui::accelerators::chords(action, os) -> Chords`** (a list: `iter()`, `first()`,
  `is_empty()`), replacing `chord`. Scopes: `Window`, `Detail`, `LocalChanges` (a focused file
  list or diff in Local Changes — the one scope with bare Enter/Backspace/Delete),
  `LocalChangesLists`, `CommitBox`, `History`; `Scope::ALL`. Actions added: `SelectRange`
  (Shift+press), `ExtendSelectionUp/Down` (Shift+↑/↓, `LocalChangesLists`), `StageOrUnstage`
  (Enter, Ctrl+Shift+S / Return, ⌘S), `StageOrUnstageAll` (Ctrl+Alt+Shift+S / ⌥⇧⌘S), `Discard`
  (Backspace, Delete, Ctrl+Shift+D / ⌫, ⇧⌘D), all three `LocalChanges`; `Commit` (Ctrl+Enter /
  ⌘Return, `CommitBox`); `ShowLostCommits` (Ctrl+Shift+. / ⌘⇧., `History`). `ExtendSelection`
  (command+press) is also Local Changes' toggle. `HeldKeys::press()` answers `SelectRange` for a
  Shift-press. `shortcuts::act` has a no-op arm for each new action: the views that hear those
  scopes act. **`is_chord(event, own: &[Scope])`**: a view leaves alone the window's and the
  detail pane's chords and those of the scopes it is in (history list `&[Scope::History]`, Local
  Changes' lists `&[LocalChanges, LocalChangesLists]`, the rest `&[]`). R7.3 as amended by the
  user (2026-10-09, decision 13): no list or staging chord fires while the commit box has focus;
  the window's chords still do, as from the filter fields.
- **`cairn_ui::text_field(value) -> Input`** and **`text_field_in(value, own: FieldScope,
  on_action: impl Into<EventHandler<Action>>) -> Input`** (`FieldScope::CommitBox`, the one scope
  a field can be heard in — none with a bare chord): the only way to build a field (guard
  `every_text_field_takes_the_shared_key_policy`). Chain `Input`'s builders after (`.multiline`,
  `.placeholder`, `.on_submit`), never `.on_pre_key_down`.
- **`cairn_ui::ConfirmDialog::new(serial, title, Rc<Consequence>)
  .on_confirm(EventHandler<Confirmed>).on_cancel(..)`** (`CANCEL_CAPTION`); on
  `CONFIRMATION_SURFACES`. Equal and keyed by `serial`, so another confirmation is another dialog;
  its words are rendered once, as it mounts.
- **`crate::confirming::Confirming::new(title, consequence, then: impl Fn(Confirmed))`** set into
  **`View::confirming: State<Option<Confirming>>`** opens it (`Confirming::serial()`, unique per
  confirmation; `consequence() -> &Rc<Consequence>`, cheap to clone); the window draws it, makes chords
  inert, and clears it on either answer. `Confirming::new` is `expect(dead_code)` outside tests
  until phase 07 asks.
- **`ContextMenuViewer`** at the window's root: open a menu with
  `ContextMenu::open_from_down(Menu::new().child(MenuButton::new()...))` from `on_secondary_down`;
  an item's handler calls `ContextMenu::close()` (Freya does not close on choose).
- **`cairn_ui::use_edge_scroll(controller) -> EdgeScroll`**: wrap the list's container with
  `edge.on(rect)` and call `edge.begin()` where a drag starts; it ends on release.
  `edge_step`, `EDGE_BAND`, `EDGE_TICK`, `MOST_PER_TICK`.
- Guards: `TOKEN_CALLBACKS` (a type may name `Confirmed` only as a callback's argument, in four
  exact spellings) with `token_callback_shadow_violations`; `ACCELERATOR_PIN` with
  `pin_placement_violations` (shared with R4.8's pin); `TEXT_FIELD_POLICY`, `TEXT_FIELD_IDENTS`
  (eight names) and `TEXT_FIELD_EXCEPTIONS`.

Phase 07 (`docs/systems/local-changes.md`, "Acting on files"):

- **`cairn_ui::ListSelection`** (`crates/cairn-ui/src/list_selection.rs`): the paths selected in
  one list, sorted by bytes, and the anchor — `of`, `spanning`, `toggled`, `holds`, `paths`,
  `anchor`, `list`; **`cairn_ui::nearest_remaining(len, first, acted)`** (R8.3). The window keeps
  it as `LocalChangesView::selection`.
- **`cairn_ui::ListIntent`**: `Toggle`, `Range`, `Double`, `Act(list, Action)`, `Drop { from,
  path }`, `CopyPaths`; `LocalChangesList::{selection, held, on_intent}` beside `on_choose`.
  **`cairn_ui::NoDiscard`** and **`no_discard(changes, list, paths)`** (why a selection offers no
  discard); the menu captions (`STAGE_CAPTION`, …, `DISCARD_CAPTION`, `COPY_PATH_CAPTION`).
  `DiffHeader::exact(bool)`. `DRAG_THRESHOLD`.
- **The table**: `StageOrUnstageAll` gained a press chord, ⌥/Alt+press (the heading button held).
- **`LocalChanges::whole_file_paths(list, rows)`**: each row's path and a rename's source.
- **Worker**: `Request::DiscardConsequence { asked: OperationId, paths }` → local lane
  (`LocalJob::Consequence`), answered `Update::DiscardConsequence { asked, outcome:
  Result<Consequence, String> }`; `LocalWrite::StageAll { changes: Arc<LocalChanges> }` and
  `UnstageAll { changes, to }` (paths gathered on the lane); `worker::UnstageTarget` re-exported.
- **`crate::local_changes_actions`** (`crates/cairn-app/src/local_changes_actions.rs`): `choose`,
  `intent`, `on_the_diff`, `confirm_arrived`, `acting_line`, `Acting` (kept as
  `LocalChangesView::acting`), `DISCARD_TITLE`, `READING_DISCARD`; `diff_actions::working_options`
  (always exact); `DiffState::settings_changed(options, in_place, working, asking)`.
- **The window** draws a credential prompt in place of an open confirmation, which it keeps.
- **`EdgeScroll`** ends on a press heard while dragging and on focus lost.
- From phase 07's QA: **`ops::discard_files_consequence(git, repo, paths, cancel: &impl Cancel)`**
  (phase 08 passes its own cancel), `Error::ConsequenceCancelled`; `QueryLane::DiscardCount`,
  `Request::StopCounting` (asked as Local Changes unmounts); `LocalWrite::StageAll { changes,
  shown }` / `UnstageAll { changes, shown, to }` — `shown` the filter's row indices, `None` with
  no filter (the user's decision, 2026-10-09); an emptied `ListSelection` keeps its list
  (`list()` is `Some`), so nothing is acted on; the follow resets the selection when it moves or
  lets go of the path chosen.

Phase 08 (`docs/systems/local-changes.md`, "Several paths, drawn together" and "The diff's
staging gesture"):

- **`cairn_model`**: `UnifiedLayout::hunk_at`/`hunk_rows`/`hunk_selection`/`selection_in` and the
  side-by-side twins (`selection_in` taking a `SideColumn`), in `row_selection.rs` and
  `diff_rows.rs`; exact rows only (`None` for rows drawn ignoring whitespace).
- **`cairn_ui::Gesture::new(side: GestureSide, drawn: u64, lines: State<LineDrag>, on_act)`**,
  handed to `DiffView::gesture` or `StackedDiff::gesture`; `GestureAct { file, verb:
  GestureVerb, selection }` (`file` 0 for one diff, the place among the paths drawn together
  otherwise); `LineDrag::selected(drawn) -> Option<(usize, &Selection)>`; `ModeRow::of(file,
  side, on_act)`; captions `STAGE_CHUNK_CAPTION`, `DISCARD_CHUNK_CAPTION`,
  `UNSTAGE_CHUNK_CAPTION`, `lines_caption`, `mode_caption`. **`cairn_ui::StackedDiff::new(files:
  Readable<Expansion>, paths: Readable<Vec<RepoPath>>, scroll)`**.
- **Worker**: `Request::DiscardLinesConsequence { asked, diff, selection }` (discard-count lane,
  answered by `Update::DiscardConsequence`); `Request::Together(TogetherQuery { asked, files:
  Arc<Vec<TogetherFile>>, options })` in the file-diff lane, answered by `Update::Together {
  asked, files: Vec<(usize, TogetherOutcome)>, ended: Option<TogetherEnded> }`.
- **Window**: `LocalChangesView::lines`; `DiffState::working_drawn`, `together`,
  `together_drawn`, `together_diff`, `show_together`, `let_go_of_together`,
  `together_needs_asking`, `reask_together` (`diff_state/together.rs`);
  `local_changes_actions::on_gesture`, `Discarding::{Files, Lines}`; `local_changes_pane::
  together_wanted`, `draw_together`.
- From phase 08's QA: `GestureAct::drawn` (an act refused once its answer is no longer drawn);
  `ModeRow::of(file, side, drawn, on_act)`; `DiffState::together_read_paths`;
  `ShownDiff::shared_diff() -> Arc<FileDiff>`, and `LocalWrite::{StageLines, UnstageLines}` and
  `Request::DiscardLinesConsequence` carry `Arc<FileDiff>`; a discard of several files' prompt
  names the first three and how many more.

Phase 09 (`docs/systems/local-changes.md`, "The commit box"):

- **`cairn_model`**: `Consequence::force_push_warning`, `replaces`, `needs_force_push`, `amended`
  (the prompt is `force_push_warning` then `replaces`, joined); `LocalChanges::amending(status,
  staged, parent)` and `LocalChanges::staged_against() -> StagedAgainst { Head, HeadParent(Option<Oid>) }`.
- **`cairn_git`**: `Repository::amend_parent() -> Result<Option<Oid>, Error>`;
  `WorkingTreeDiff::Amending` (the index against `HEAD`'s parent, the empty tree for a root).
- **`cairn_ui`**: `CommitBox::new(subject, description).amend(..).button(CommitButton, ready)
  .busy(Option<Busy>).stopped(..).note(..).recent(..)` with `on_amend`, `on_commit`,
  `on_confirmed` (`EventHandler<Confirmed>`), `on_cancel`, `on_recall`, `recall`;
  `CommitButton::{Commit, AmendInPlace, AmendAsking, ReadingAmend, AmendUnreadable}`;
  `AmendButton::new(serial, Rc<Consequence>, confirmed: State<Option<u64>>).ready(..).on_confirmed(..)`
  and `AmendSkip::new(serial, Rc<Consequence>).on_confirmed(..)` — `commit_box.rs` is on
  `CONFIRMATION_SURFACES` (R1.1); `GitErrorDialog::new(serial, command, lines).skip(..).on_skip(..)
  .skip_amend(Rc<Consequence>, EventHandler<Confirmed>)`; `SKIP_HOOKS_CAPTION` (in `commit_box.rs`); `text_field_recalling(.., recall: Callback<RecallStep, bool>)`;
  `accelerators::{RecallStep, recall_step, recall_step_on}`; `commit_caption`, `subject_count`.
- **Worker**: `Request::CommitReads` (`Update::CommitReads(Box<CommitReads>)`: `operation`,
  `hooks`, `recent`), `Request::Amending { status }` (`Update::Amending { status, read:
  Box<AmendRead> }`: `consequence`, `message`, `lists`), `Request::StopAmending`;
  `QueryLane::CommitBox`, `QueryLane::Amending` (both on the local lane);
  `WorkingSide::Amending`; `WriteEnding::Failed { message, locks, command, output }`;
  `Retired::amending(status, lists)`.
- **Window**: `LocalChangesView::commit: CommitBoxView { subject, description, state }`
  (`commit_box_state.rs`: `CommitBox`, `AskedCommit`, `Amendable`, `GitError`, `OutputTail`,
  `split_message`, `compose_message`, `strip_ansi`; `AskedCommit::confirmed_with`, the consequence an
  amend's token was built from, which its skip confirms again); `commit_box_pane.rs` (`CommitBoxPane`,
  `toggle_amend`, `pressed`, `amend_confirmed`, `cancel`, `recall`, `recall_step`, `skip_hooks`, the
  `*_arrived` and `write_*` hooks `session::apply` calls, `git_error`, `AMEND_TITLE`);
  `local_changes_actions::unstage_target`; `local_changes_tests::{in_lists_reads, click_heading}`.

Phase 10 (`docs/systems/history-graph.md`, "Show Lost Commits"; `docs/systems/staging.md`, the
verb table):

- **`cairn_git::HistoryRequest::with_lost_commits()`**: a walk from refs also starts from the old
  and new id of every entry of `HEAD`'s reflog and each local branch's (the snapshot's
  `RefKind::LocalBranch` refs), read as the walk opens under its first page's cancel
  (`src/history/reflogs.rs`, git's `show_one_reflog_ent` rules, each log read whole); the
  commits no ref reaches are marked as the walk goes (`src/history/reach.rs`, `Reach`). A cursor
  carries the tips read and which the refs named (`Marking`).
- **`cairn_model`**: `RowsPage::push_lost(graph, commit)`, `RowsPage::reached(row, id)` (a row an
  earlier page carried as lost, reached after all — clock skew), `RowsPage::is_lost(index)`,
  `HistoryRow::is_lost()`; the `LOST` bit in `StoredRow::flags` (rows stay 72 bytes);
  `Chunks::get_mut`.
- **`cairn_git::ops::create_branch(git, repo, name: &str, commit: Oid, token) -> Result<Performed,
  Error>`** (`src/ops/branch.rs`): `git --literal-pathspecs branch -- <name> <id>`, invalidates the
  refs, not destructive; git's refusal is `Error::GitFailed` with its stderr. Reusable by
  branch-ops' create branch (roadmap section 7).
- **`cairn_ui`**: `HistoryList::on_action(EventHandler<Action>)` — the history's own scope's chords
  (`Scope::History`), resolved before anything else in its key handler; `RowRender::lost`;
  `CommitRow::lost(bool)`: its four texts in the theme's `text_placeholder`, graph and chips in
  their own colours (Fork-settled on Fork's evidence, applied under the user's Fork-first rule, 2026-10-09);
  `HistoryHeader::show_lost_commits(on, on_toggle)`, `SHOW_LOST_COMMITS_CAPTION`;
  `accelerators::chord_name(action, os)` (`accelerators/chord_names.rs`); `check_box(caption,
  ticked, enabled, on_toggle)` (shared with Amend); `HistoryList::on_new_branch(EventHandler<(Oid,
  String)>)`, `NEW_BRANCH_CAPTION`; `CreateBranchDialog::new(serial, at, subject, name).ready()
  .refusal().checkout().local_changes().on_checkout().on_local().on_create().on_cancel()`,
  `LocalChoice::{Keep, Discard}` and Fork's captions, `CREATE_BRANCH_SUBTITLE`,
  `.waiting(Option<String>)`; `accelerators::Action::NewBranch` (Ctrl+Shift+B, ⇧⌘B,
  `Scope::Window`); `button_order::ordered(platform, actions, cancel)` and `.platform(Os)` on
  `ConfirmDialog`, `GitErrorDialog`, `CreateBranchDialog` and `CredentialPrompt`.
- **Create Branch's engine**: `cairn_model::BranchName::{Free, Taken, Refused { reason }}` and
  `refusal(name)`; `Consequence::CheckoutDiscarding { branch, at, head, changes: Vec<LostChange>,
  kept_untracked }`, `LostChange`, `ChangeLoss::{Changed { kind, index, working_tree, executable,
  lines }, Overwritten { .. }, Removed { kind: RemovedKind, files, bytes }}`, `ChangedKind`,
  `RemovedKind::{Directory, Repository}`; `Repository::branch_name(git, name, cancel)` (a name a
  branch's directory holds refused in git's words);
  `ops::create_branch_and_checkout`, `ops::checkout_discarding_consequence`,
  `ops::create_branch_discarding` (on `DESTRUCTIVE_OPERATIONS`); `reads::branch_name`,
  `reads::change_lines`, `reads::untracked_paths`; `Error::CheckoutRefused { why: CheckoutRefusal }`.
- **Create Branch's worker and window**: `Request::CheckBranchName { name }` (`QueryLane::BranchName`),
  `Request::CheckoutConsequence { asked, name, at }` (`QueryLane::CheckoutCount`), answered by
  `Update::BranchName` and `Update::CheckoutConsequence`; `LocalWrite::CreateBranchAndCheckout`,
  `LocalWrite::CreateBranchDiscarding(Confirmed)` (`CreateBranch`'s `expect(dead_code)` gone);
  `View::branch: CreateBranchView { state, name }` (`create_branch.rs`: `open`, `name_checked`,
  `open_at_head`, `create`, `consequence_arrived`, `write_ended`, `dialogs`).
- **Worker**: `Request::OpenHistory { rows, lost }`, `Routed::OpenHistory { rows, lost }`,
  `Page::Open { rows, walk, lost }`, `HistoryLane::replace_walk(walk, lost)` (the lane keeps
  `lost` for the walk it opens); `LocalWrite::CreateBranch { name, at }` (`what()` "creating branch
  <name>", reads everything again), asked by Create Branch's dialog.
- **Window**: `View::show_lost: State<bool>` (off as the window opens, kept for the session);
  `crate::lost_commits::{history_action, toggle}`; `session::reopen_history(rows, progress, lost,
  submit)` is now `pub` and takes the toggle — a refresh's reopen passes `*view.show_lost.peek()`.

Phase 11 (`docs/systems/git-processes.md`, "The registry" and "The activity popover"):

- **`cairn_model::Scrubber`** (`new`, `after_cut`, `line`) and `scrub_userinfo`
  (`crates/cairn-model/src/scrub.rs`); the window's `crate::shown_output` (`ShownLines`,
  `shown_lines`, `shown_line`, `scrubbed`, `shown_ending`, `MAY_BE_CUT_BYTES`).
- **`cairn_git`**: `Repository::command_mark() -> CommandMark`, `commands_since(mark)`,
  `running_invocations()`; `ops::remove_lock_consequence(repo)`, `ops::remove_index_lock(repo,
  Confirmed)` (`src/ops/remove_lock.rs`, on `DESTRUCTIVE_OPERATIONS`); `Error::LockRefused { why:
  LockRefusal }` (`NoLock`, `NotAFile`, `GitRunning(n)`, `NotWhatWasConfirmed`),
  `Error::LockChangedSinceConfirmed { path }`, `Error::LockNotRemoved`; `CommitWatch::output`
  now `&mut dyn FnMut(&[&str])` (one call a pipe read), `Invocation::finish_by_read`;
  `history::walk::{open_dated, graph_date, reaches_through_graph}`.
- **Worker**: `LocalWrite::RemoveLock(Confirmed)`, `LocalWrite::replaces()`,
  `is_cancellable()`; `Update::WriteOutput { id, lines, receipt: OutputReceipt }`
  (`worker/output_flow.rs`); `Update::OperationRan { by: RanBy::{Write(id), Fetch}, commands,
  lock_named }`, sent before each ending; `local_lane::{ran_since, names_index_lock}`;
  `Request::LockConsequence { asked: OperationId }` (the local lane, routed `Routed::LockConsequence`)
  answered by `Update::LockConsequence { asked, outcome: Result<Consequence, String> }` — the
  removal's consequence read at the press (the user's decision H).
- **Window**: `View::activity: State<ActivityLog>` (`crate::activity`: `ActivityKey::{Write,
  Fetch, Opened}`, `Outcome` (with `Found`), `ACTIVITY_ENTRIES`, `ACTIVITY_LINES`, `ACTIVITY_BYTES`,
  `popover`, `use_lock_confirmation`, `shell_quoted`, `REMOVE_LOCK_TITLE` ("Remove stale lock"),
  `LOCKS_AT_OPEN_NAME`, `GIT_RUNNING_NOTE`, `LINES_LET_GO`; `ActivityLog::{set_anchor, lock_asked,
  lock_answered, take_lock_ready, removal_asked, locks_at_open}`); `Asked::{name, awaited,
  replaces, cancellable}`; `shortcuts::keys_inert`; `ref_find::find_commit`.
- **`cairn_ui::ActivityPopover`** (`ActivityEntry` with `lock: Option<LockOffer>`,
  `LockOffer::{Ready { note }, Blocked(why)}`, `ActivityLine::{Ran, Output}`, `.anchor(..)`,
  `activity_popover::hung`, captions `ACTIVITY_TITLE`, `SHOW_REPLACED_CAPTION`,
  `REMOVE_LOCK_CAPTION`, `CANCEL_OPERATION_CAPTION`, `CONFIRMED_CAPTION`, `NO_ACTIVITY`,
  `SHOW_ALL_CAPTION`, `SHOW_LESS_CAPTION`); `Expansion::take_all`.
- **Guards**: `only_the_ops_module_changes_the_filesystem`, `mutates_the_filesystem`,
  `FILESYSTEM_MUTATION_IDENTS`, `FILESYSTEM_MUTATION_FS_FUNCTIONS`,
  `FILESYSTEM_MUTATION_EXCEPTIONS`, `TEST_ONLY_CFGS`.
- **Reporters**: `window_check::writes_check` (C21 in the window), `cairn_write_costs` and
  `discard_count_costs` (`crates/cairn-git/tests/diff/write_baseline.rs`).

## Carried forward from phase 11 (owned by the phase named)

- **Resolved by the user's decision H (2026-10-09)**: DO3+DO5 — the offer is now the ending's
  naming of the lock (`local_lane::names_index_lock`), and the `Consequence` is read on the local
  lane at the press (`Request::LockConsequence`), so the age is current and a `git` running as the
  write ended loses nothing; while one runs the button says why rather than vanishing.
- **The user's decisions on the popover (2026-10-09), applied**: A — hung from the status box, its
  left edge under the box's, an arrow pointing at it, inside the window's width; B, C — the right
  pane's order and the status words as built; D — "HH:MM:SS UTC" kept (local time filed below);
  E — Fork's imperative names (done in phase 11's fixes); F — "Show Replaced Commit" kept; G — the
  confirmation titled "Remove stale lock", its button "Remove index.lock", its prompt "Remove
  <path>? It was last changed N ago and holds N bytes. Another program may still own it: removing a
  lock a running git holds can corrupt the index. You can't undo this action."; H — a lock found at
  open an entry of its own, "Lock files found as the repository opened", and the consequence read at
  the press; I — keys inert while it is open, Escape closing it; J — a fetch refused because another
  runs gets no entry; K — the confirmed prompt's four-line cut with "Show All", expanding in place
  and scrolling with the pane; L — each `$ git` argument quoted as a POSIX shell needs it; M — "Its
  output was let go to make room for newer operations." where the byte bound let lines go; N — a
  write refused before it started keeps its own name (cheaply available: the window's queued write,
  no fallback needed); and Create Branch's "Waiting for the branch to be created…". The empty-state
  line, ×, the duration format, the command endings, the newest entry selected, the bounds and the
  error message's 3-line cut stay as built.
- **Decided in the building, for the user's review** (phase 11, round 3): the lock-at-open entry's
  status word "found" and its lines the lock paths; "Show Less" as the collapse caption; "Show All"
  offered where the prompt's characters exceed four lines at the pane's width (an estimate, not a
  layout); the blocked note "Cairn is running git in this repository"; a refusal at the press said
  beside the still-pressable button; the arrow a square turned 45°; the panel narrowed for a window
  narrower than it; a successful removal taking the offer from every entry.
- **To file at teardown** (phase 11's QA): local time, DO1, DO2, DO6 and GI7 — moved into the one
  list under "Phase 12, the merge bar" above.

- **Phase 12 / the user** (the issues among these are in the one list above): batching the discard count's per-path reads (measured 15.4 ms a path
  on the bench clone; a batched `numstat` read would change what a prompt counts for an LFS
  pointer, a file past the limits and under `diff.algorithm`); the amend read's split (measured:
  0.14 ms consequence, 2.6 ms staged list per status at a remote tip; 1.2 s only for a detached
  `HEAD` far behind every remote tip with no commit-graph); the engine's half of #46 (the command
  log and `Error::GitFailed` keep git's stderr unscrubbed; every drawn line is scrubbed); closing
  #41 and commenting #46/#87 on GitHub, left to the coordinator.

## Carried forward from phase 10 (owned by the phase named)

- **Decided by the user (2026-10-09), built**: (B) Show Lost Commits' check box at the right end
  of the "Graph and subject" heading's cell, its tooltip the chord from the accelerator table
  (the modifier invariant amended: `CHORD_NAMES`, `accelerators::chord_name`); (A) the
  10,000-entry first page a stated residual, carried below; (D) off at launch, session only;
  Fork-settled: a lost row's text alone dimmed, the list keeping the keyboard through the
  toggle's reopen. Create Branch: Fork's full dialog from `New Branch…` on every commit row (PRD
  R11.3 amended), "Check out after create" sticky for the session (decision 1; across restarts
  is issue #89, cited in the roadmap's after-D7 list), "Local changes:" with "Don't change" and
  a sealed "Discard" (decision 3; R1.5 and R3.6's stated exception, `DESTRUCTIVE_OPERATIONS`
  row), no "Stash and reapply" until packet 5b (decision 2, carried into 5b's brief); checkout
  enters this packet only through the dialog (roadmap, branch-ops).
- **Fork-settled at QA, built**: Fork's New Branch chord, Ctrl+Shift+B (⇧⌘B on macOS), at
  `HEAD`, heard anywhere (`fork-create-branch-evidence.md` §1); the dialog's subtitle ("Use '/'
  as a path separator to create folders"); a refusal said in the buttons' row, left of them,
  behind Fork's warning triangle (§2: TrackerWin #2472, Tracker #1911).
- **Decided by the user (2026-10-09), QA's held items A-F, built**: (A) the Discard
  confirmation titled "Discard changes", its button "Discard Changes and Check Out", its prompt
  naming each folder and nested repository deleted by its path in a sentence of its own, the
  first three named and the rest counted (`cairn_model::consequence`); (B) a name's refusal
  inside the dialog beside the buttons behind ⚠, never the Git Error dialog; (C) a right-click
  selects its row, keeping a selection it lands in — the history's list (Local Changes' lists
  already did); (D) "Waiting for the <noun> to finish…" beside the buttons while the name's check
  waits behind a write; (E) every dialog's buttons in the platform's order, the primary first
  on Linux and last on macOS, focus on Cancel for a confirmation on both
  (`cairn_ui::button_order`); (F) a name holding `@{` refused before git is asked, "A branch
  name can't contain '@{'". The derived choices QA's item 21 carried are settled by these.
- **Phase 11 (the user's decision A, 2026-10-09)**: read each Show Lost Commits tip id once
  (today a lookup and then the walk's date read, ~41 ms of the 49.8 ms at 10,000 entries) and take
  the dates from a commit-graph where one is present, measured on the bench clone with
  `measures_the_first_page_with_show_lost_commits`. Also C21's "Show Lost Commits' first frame
  recorded" in `window_check`. Phase 10's numbers: +0.2-0.3 ms on the bench clone with three lost
  commits; 7.95 ms with a 1,000-entry `HEAD` reflog; 49.8 ms with 10,000 (git's own `rev-list
  --reflog`: 57-80 ms) against the bar of twice refs-and-status's 7.27 ms.
- **Residuals, stated in `docs/systems/history-graph.md`'s known limits**: a reflog that changes
  while no drawn ref moves (`git reflog expire`) is not read again until the next reopen; only
  `HEAD`'s and the local branches' logs (no remote-tracking, leftover or other worktrees' logs);
  a chord pressed while the reopened walk's first page is on its way is not heard (the list is
  not drawn then); the long-reflog first page (decision A) — its cost grows with the local
  branches' count and the ids across all their logs, not `HEAD`'s alone, and is paid again on
  every refresh that moves a ref while the toggle is on (each reopen reads every log again);
  the tips are de-duplicated against the refs' through a hash set (QA item 11), so the merge is
  linear. Carried to phase 11's measurements: a bench run with many local branches' logs, and the
  repeat per ref-moving refresh — done: measured in phase 11 (200 branches each with a 50-entry
  log, 50.6 ms with no commit-graph and 8.9 ms with one; each reopen pays it again), and
  `docs/systems/history-graph.md`'s known limit brought up to it at the merge bar (F1).

## Carried forward from phase 09 (owned by the phase named)

- **Phase 11**: measure on the bench what the box asks of the local lane — its reads on every
  refresh's refs while Local Changes is shown (a `git rev-parse` and a ten-commit walk), and an
  amend's read per status while Amend is ticked (the pushed check's walk and a `git diff-index
  --cached` against `HEAD`'s parent, per stage) — and, from phase 05's QA item 4 (optional, not
  done here), skip the re-check's walk when the tips it read have not moved (not done in phase
  11; to file at teardown, F3, in the one list above).
- **Phase 11**: the activity popover draws every write's `Update::WriteOutput`; the commit box
  keeps only its own commit's, for the Git Error dialog.
- **Phase 11, from phase 09's QA** (adjudicated 2026-10-09): item 11 — the Git Error dialog draws
  hook and git stderr unscrubbed (`commit_box_state.rs`, `OutputTail::push` and
  `CommitBox::failed`): scrub the streamed `WriteOutput` lines and the engine's kept output before
  `CommitBoxState` keeps them, with a dialog-level test (written into phase 11's step 2); item 9 —
  split the amend's read (the `Consequence` on refs arrival and on the tick; `amend_staged` per
  status) and measure both; item 11 — `Update::WriteOutput` is one update per line, and
  `OutputTail` keeps one oversized line whole (bound it).
- **Phase 12 — done, from phase 09's QA** (item 8): a real-git end-to-end test of a commit a hook
  fails, its skip, and the next commit running its hooks
  (`a_failing_hook_fails_a_commit_the_skip_commits_past_it_and_the_next_runs_it_again`).
- **Decided by the user (2026-10-09), each kept as built** (progress.md, phase 09 closed): the
  subject required; a merge committing with nothing staged; `MERGE_MSG` filling an empty draft
  once per merge; amend offered when amend's staged list cannot be read — PRD R10.1, R10.3 and
  R10.8 amended, `docs/design/ui.md` and `local-changes.md` updated, evidence in
  `docs/research/staging-and-commit/fork-merge-and-amend-evidence.md`. Items 1-2 (where the
  amend's token is built; no chord confirms an amend) were settled by the user's decision on the
  Amend button the same day.

## Carried forward from phase 08 (owned by the phase named)

- **Phase 11**: measure the gesture on the bench in `window_check` — hover and drag over a large
  diff (the layer re-renders per pointer move and scroll), and a selection of many paths drawn
  together (one pass over the files per render of `StackedDiff`, `widest`); C21's frame budget.
- **Phase 11, from phase 08's QA** (adjudicated 2026-10-09): #3 — each page of the files drawn
  together rebuilds their `Expansion` from the page's first file to the end (O(paths × pages),
  bounded by the selection), and `widest` runs every frame of a sideways resize: measure, and
  bound if it shows; #4' (the user's decision) — adding a path to a selection drawn together
  re-reads every path: keep each file's answer across selection changes, with measurements (the
  flash is documented as interim in `local-changes.md`).
- **Phase 09 — done** (#7): `a_chunk_drawn_at_context_ten_stages_exactly_as_drawn`.
- **Phase 12 / the user, at the merge bar** (#15; F7 resolved 2026-10-10 as an allowed convention, PRD R7.2; in the one
  list above): Escape is matched as a literal
  `NamedKey::Escape` (`diff_view.rs`, `stacked_diff.rs`, `local_changes_drag.rs`) rather than as
  an accelerator-table `Action` — a convention note, not a guard breach.

## Carried forward from phase 07 (owned by the phase named)

- **Phase 08 — done (built in phase 08)**: a requirement (the user's decision on phase 07's QA item 4(e), 2026-10-09):
  with several files selected in Local Changes, the diff draws the selected files' diffs
  together, as Fork does, reusing the Commit tab's layout of files opened in place
  (`cairn_ui::Expansion`, `CommitTab`'s list); phase 07 draws the path last pressed in
  meanwhile (PRD R8.1's note; `docs/design/ui.md`; `local-changes.md`, "Where it is not
  Fork's"). Phase 08's gesture then acts on the file under it.
- **Phase 08 — done**: a discard of every line of a new file goes to `discard_files` (phase 03's
  `Refusal::WholeFileOnly`): the line gesture is the only route that selects lines, so the
  gesture routes it — `local_changes_actions` asks `Request::DiscardConsequence` for the path,
  as the file routes do. The diff's chords act on the whole file today
  (`local_changes_actions::on_the_diff`); phase 08 narrows them to a drag-selection's lines.
  The drag selection over the diff begins its `EdgeScroll` on the diff's container (unchanged
  from phase 06's carry); `EdgeScroll` now ends on a lost release itself.
- **Phase 09 — done**: `WorkingTreeDiff::Amending` and amend's lists drawn in Local Changes while
  Amend is ticked; an unstage from them asks `UnstageTarget::Commit`/`Nothing`
  (`amends_staged_list_is_drawn_diffed_against_heads_parent_and_unstaged_to_it`).
- **Phase 11, from phase 07's QA**: batch the discard count's per-path reads into one
  multi-path `git diff-files` read in `reads/` — measure first; and the argv size of stage,
  unstage and restore at 50,000 paths (`destructive-ops-reviewer`; `git clean` is already
  batched by `CLEAN_ARGUMENT_BYTES`, the others take a pathspec file).
- **Phase 11**: the activity popover reads the same `LocalWrites` the line under the lists does;
  the cost of a 50,000-path selection's actions on the UI thread (a clone per path on a range
  press, a search per path on an action) is named in root `CLAUDE.md` and is the window check's
  to measure if a reviewer asks.

## Carried forward from phase 06 (owned by the phase named)

Phase 07's items here are done (the keys heard on the lists and diff and resolved through
`HeldKeys::press()`, `Confirming::new` opening the discard, menu items closing themselves, the
drag with one drop zone per list and the target's `EdgeScroll`; QA items 4, 5, 16 and 21 — see
progress.md's phase 07 entry).


- **Phase 07**: hear `Scope::LocalChanges` on the Unstaged and Staged lists' and the diff's own
  `on_key_down` (the filter above the lists keeps its keys, so it never reaches them), and
  `LocalChangesLists` for Shift+↑/↓; resolve presses with `HeldKeys::press()` —
  `ExtendSelection` toggles a path, `SelectRange` selects a range; open a discard through
  `Confirming::new("Discard changes", consequence, move |token| local_writes::ask(..,
  LocalWrite::DiscardFiles(token)))` and drop `Confirming::new`'s `expect(dead_code)`; the
  context menu's items close it themselves; the drag between lists uses one drop zone per list
  and the list's `EdgeScroll`.
- **Phase 07, from phase 06's QA** (adjudicated 2026-10-09):
  - item 4: Local Changes hears its chords on focused keys only (the lists' and the diff's own
    `on_key_down`) or checks `view.confirming`; a window test presses Enter, Backspace and Delete
    in Local Changes while a confirmation is open and nothing stages or discards;
  - item 5: decide how a credential prompt stacks over an open confirmation (today the prompt
    cannot be typed into until the confirmation is answered — the modal holds focus; it fails
    safe);
  - item 16 (07 or 08, whichever first wires a drag): handle the pointer leaving the window — a
    release outside it leaves `EdgeScroll` dragging;
  - item 21: a window test pressing Backspace in the filter field while Local Changes is shown,
    and nothing discards.
- **Phase 08 — done**: the drag selection over the diff's rows begins with `EdgeScroll::begin` on
  the diff's container (`EdgeScroll::on`), never on a row.
- **Phase 09 — done**: the subject and description built with `text_field_in` /
  `text_field_recalling` (`FieldScope::CommitBox`, the description multiline, no `on_submit`).
- **Phase 09 — done, as the user decided (2026-10-09) and R1.1 says**: the commit box is the second
  confirmation surface (`AmendButton`, `AmendSkip` in `commit_box.rs`); a published amend hands the
  window a `Confirming`; the force push is a part of the dialog's prompt, never an informational
  component. (The phase 06 carry's "never calls `by_user`, one row" was the coordinator's
  instruction, withdrawn.)
- **Phase 10 — done**: `Scope::History` heard in the history list's `on_key_down`
  (`HistoryList::on_action`; `the_show_lost_commits_chord_is_reported_and_moves_nothing`,
  `the_show_lost_commits_chord_reopens_the_history_with_the_toggle_flipped`).

## Carried forward from phase 05 (owned by the phase named)

- **Phase 09 — done**: `WorkingTreeDiff::Amending`
  (`a_file_of_amends_staged_list_reads_as_git_diff_cached_against_heads_parent`).
- **Phase 09 — done**: `Request::CancelWrite`'s and `LocalWrite`'s `expect(dead_code)` are gone, the
  Git Error dialog draws the output, the `MERGE_MSG` prefill is tested
  (`a_merge_fills_the_draft_with_merge_msg_as_git_wrote_it_and_disables_amend`),
  `Consequence::needs_force_push` and the amend prompt's parts exist. As carried: Draw
  `Update::WriteOutput` (the Git Error dialog: the command and git's output, ANSI stripped, from
  the failure's `GitFailed` — stdout's tail then stderr's). R10.8's `MERGE_MSG` prefill:
  `OperationInProgress::Merge.message` is git's file as it stands, with git's own `# Conflicts:`
  comment lines, which `-F` under the default cleanup (`whitespace`) keeps in the commit —
  where the user's `git commit` (editor, `strip`) drops them. **Decided by the user (2026-10-09,
  E): keep them visible** — the box prefills `MERGE_MSG` as git wrote it, comment lines
  included, and the user deletes them by hand; under `-F` they are committed if left (R10.8 as
  amended). Test that the prefill is the file's text unchanged. Still: `Consequence::needs_force_push()` (phase
  01's note) and rendering the amend prompt's parts separately (phase 01 QA item 21).
- **Phase 10 — done** (`show_lost_commits_draws_what_git_reads_from_every_reflog_entry_and_dims_what_no_ref_reaches`,
  its fixture's amend whose log it created; `with_lost_commits`): a requirement that blocks phase
  10's QA (the user's decision D, 2026-10-09):
  seed Show Lost Commits' walk from every reflog entry's OLD and NEW ids, for `HEAD` and each
  local branch, as `git rev-list --reflog` and `git fsck` read a reflog (R11.1 and C20 as
  amended). `Reflog::Written` means git appends the amend's entry, whose old id is the replaced
  commit; with a log the amend itself created (measured in phase 05: default config, logs
  removed, amend), the replaced commit is ONLY an old id, and the new-ids-only seed misses it.
  C20's fixture gains that case. The amend prompt keeps "The old commit stays in Show Lost
  Commits."; `gc.reflogExpireUnreachable`'s 30-day expiry stays a stated residual.
- **Phase 11**: the pushed check's cost (`docs/systems/staging.md`, "What the walk costs"):
  under 3 ms to ~100 ms where `HEAD` is near a remote tip, 1.1-1.3 s on rust-lang/rust for a
  detached `HEAD` far behind every remote tip (git's own walk without a commit-graph, 1.14 s;
  0.08 s with it), and an amend pays it twice (consequence, re-check before git runs). Consider
  the commit-graph's generation numbers, keeping the walk cancellable.
- **Residual, stated in `docs/systems/staging.md`**: a hook owned by another user counts on the
  group's bit without reading this user's groups; a commit left orphaned by a second close can
  still die at a line its hook writes; in a partial clone amend's staged list fails closed on
  git 2.44+ (fetches below it) over a staged inexact rename of a blob only the promisor holds.

From phase 05's QA (adjudicated 2026-10-09):

- **Phase 09 — done** (item 4): the amending lane and `Reading what Amend would replace…`; the
  optional walk skip re-carried to phase 11.
- **Phase 09 — done** (item 6): `amends_staged_list_unread_is_said_and_the_status_lists_stay`.
- **Phase 09 — done** (item 8):
  `a_failed_hooks_skip_commits_once_without_hooks_and_the_next_runs_them`.
- **Phase 10 — done** (item 12, = D, decided by the user 2026-10-09): the reflog seed above.
- **Phase 11** (item 3): `Update::WriteOutput` is one unbounded update per line
  (`worker/local_lane.rs`'s `Watch::commit`; `session.rs` discards it; `pool.rs`'s
  `Updates::next` drains the whole backlog in one poll) — send one update per pipe chunk
  (`Vec<String>`) with a bound on what is queued; touches `process/runner.rs`'s per-line stderr
  callback.
- **Phase 11** (item 4): measure the pushed check on the bench; decide a commit-graph
  generation cutoff (gix-commitgraph lookup and generation, the cancel polled per step).
- **Phase 11** (item 11): `ops/commit.rs`'s `Lines::push`/`finish` cut at `STDOUT_TAIL` bytes
  and lossily decode, so a multibyte character at a cut reads as U+FFFD.

## Carried forward from phase 04 (owned by the phase named)

- **Phase 05** (done, except `Request::CancelWrite`'s `expect(dead_code)`, re-carried to phase
  09, which first constructs it outside tests): `Commit`/`Amend` in `LocalWrite`;
  `LaneState::install`'s `expect(dead_code)` gone; C10/C11/C12's commit-dependent tests re-run
  against `git commit` with a slow hook.
- **Phase 05, from phase 04's QA** (adjudicated 2026-10-09; each done in phase 05 — item 7 by
  deleting the override, a commit's ending reading everything whatever it did, pinned by
  `a_commits_ending_reads_everything_whatever_it_did`; item 9 except the close-patience wait,
  which is the time under test; item 13 by naming each token's operation; `-q` on `git clean`,
  `restore` and `apply` checked silent; the banner):
  - item 7: the `deferred` override in `local_lane.rs`'s `run` (a refresh kept back makes the
    ending `ReadAgain::Everything`) is unobservable while the only commit already reads
    everything — pin it against a real commit (an amend that a refresh was kept back for),
    or delete the plumbing;
  - item 8: delete `LocalWrite::HeldCommit` and re-run C10-C12's commit halves on `git
    commit`, the cancel installed with `LaneState::install`;
  - item 9: replace the tests' fixed quiet windows (500/700 ms in `worker/local_lane_tests.rs`)
    with waiting on a later request's answer, and the `< 100 ms` asking bound in
    `writes_asked_faster_than_they_run_run_in_order_each_with_its_own_ending`;
  - item 13: a prompt names its operation — map token to operation in the acceptor and
    carry it on `Update::Prompt`, titled by its own operation, before the first real commit
    (here), no later than phase 07; obligation (`responsiveness-reviewer`): `install()`'s
    kill under `LaneState`'s lock stays bounded;
  - the user's decision 12 (2026-10-09), its hardening: add `-q` to `git clean` so an
    orphaned discard finishes its batch (an `ops/` change, with a test in the same commit),
    and check `git restore` and `git apply` for the same `SIGPIPE` exposure; the closing
    banner says closing again leaves the write unfinished.
- **Phase 07** (done: every route asks through `local_writes::ask`; the line under the lists
  draws queued, running and ended; the consequence is asked on the local lane): ask writes
  through `local_writes::ask`; draw a write queued and its outcome (`LocalWrites::queued`,
  `last`); decide where `discard_*_consequence` is asked (a worker's call; the local lane
  orders it after the writes ahead of it).
- **Phase 09 — done**: `cancel_reaches_only_the_running_commit`.
- **Phase 11**: the activity popover reads `WriteEnding`/`Done`; measure the open's lock
  listing (`SharedRepository::lock_files`, a walk of `refs/`, now on the local lane) on the
  bench repository, and the two walks every local write makes (#87).
- **Phase 11, from phase 04's QA** (item 11): `channel.begin().ok()` in `local_lane.rs`'s `run`
  drops why no token could be had; the write's ending should say why no prompt could be
  answered, as `prompting` does for a channel never opened.
- **Decided by the user (2026-10-09)**: 12 — a second close keeps R4.9 (never ends the
  write), hardened in phase 05 (above), the design and as-built docs amended; 14 — reads
  keep the roster writes have (reason beside `INHERITED`).

## Carried forward from phase 03 (owned by the phase named)

- **Phase 04** (done: the lane runs every verb with its write's token; the consequences
  carried to phase 07): the lane calls the verbs above on its own thread; the verbs take the
  write's token; `discard_*_consequence` are reads the window asks for before a dialog.
- **Phase 05** (done: `hooks_path` re-exported; `UnstageTo::Commit`/`Nothing` exercised by
  amend's tests; the root amend's refusal met with `-f`; C13's root-commit case): `hooks_path`
  re-export; `UnstageTo::Commit`/`Nothing` for amend; the root amend's `git rm --cached`
  refusal; C13's root-commit case.
- **Phase 07** (done: the caller owns it, `staging.md`'s residual, pinned by
  `a_discard_names_only_paths_the_lists_drawn_still_list`; phase 03's QA item 4): `discard_files_consequence` accepts any path
  absent from the index as untracked, so an ignored file or a path inside a nested
  repository reaches `git clean -f`, which leaves it (the outcome now names it as kept).
  Phase 07 either refuses a path `git status` did not list before any prompt, or states in
  `docs/systems/staging.md` that the caller owns that, with a test of the `Absent` arm.
- **Phase 07** (done: the dialog opens on the engine's `Consequence`; the rename's row
  unstages `[new, old]`, a source row alone; the engine's refusals said before any dialog): the dialog's `Consequence` from `discard_files_consequence` (a nested
  repository, a submodule, a conflicted path and a staged-only path are refused by it
  before any dialog); the rename source row's whole-file unstage resets the source alone,
  both paths unstage the rename; an intent-to-add file's discard leaves it empty and says
  so (the user's decision 5).
- **Phase 08 — done**: `discard_lines_consequence` takes the `Selection` by value; a mode-only
  selection where the diff has a mode change is allowed and named.
- **Phase 11 / C22**: the root `CLAUDE.md` still has to name the one file deletion made
  without `git` (R12.4) when it exists.
- **Phase 12 / the user**: items 4 and 7 batched in progress.md's phase 03 entry (the
  rest decided by the user on 2026-10-08).

## Carried forward from phase 02 (owned by the phase named)

- **Phase 03** (done: `DiscardLines.mode`, named in the prompt): `Consequence::DiscardLines` counts `Selection::len`, which counts
  lines only, so a selection holding the mode change renders no word for it (and a
  mode-only one "Discard 0 Lines"); this is phase 01's item 19, now concrete. Phase
  03's discard verb decides how a mode selected for discard is confirmed, with a test.
- **Phase 03** (done: refused as `WholeFileOnly`, left to `discard_files`): a whole selection of an untracked or intent-to-add file's lines
  discards as a `deleted file mode` patch, which `git apply` honours by deleting the
  file — but L8 routes deleting an untracked file to `git clean` under
  `DiscardFiles`. The verb (or phase 07/08's gesture) must route an all-lines
  discard of an added file to the file verb, or confirm it as a deletion.
- **Phase 03 / C22** (done): the root `CLAUDE.md` (repo map's `cairn-model` and `cairn-git`
  rows, and D1's list of reads) does not yet name `action_patch`, the inversions or
  `reads::staged_pairing`; `docs/design/engine.md` and `docs/systems/diff.md` do. Not
  edited by the phase agent (an instruction file); for the user or C22's update.
- **Phase 08 — done**: drawing the mode row (R9.4) selects through `Selection::select_mode`.

From phase 02's QA (adjudicated 2026-10-08):

- **Phases 03, 07 and 08** (QA item 8; done — the engine's half, `a_rename_sources_row_unstages_its_lines_at_the_new_path`; phase 07's whole-file half, `a_whole_file_action_names_each_rows_path_and_a_renames_source`; phase 08's lines half, `a_renames_source_row_unstages_its_lines_at_the_new_path`): a rename's SOURCE path is paired too
  (`reads::working_tree::names`): where the user's `status.renames` differs from
  `diff.renames`, status lists the deleted source as a row of its own while its staged
  diff is the rename, and an unstage built from it acts at the NEW path. The verb and the
  gesture decide which row offers what, and on which path, with a test.
- **Phase 03 / C22** (QA item 9; done): the root `CLAUDE.md` repo map and D1's list of reads
  must name `action_patch`, the inversions and `reads::staged_pairing` (the C22 item
  above); phase 03's QA checks it.
- **Phase 03, the destructive verbs** (QA carry list; each done — the patch from the confirmed diff, checked against the `Consequence`; no `--recount`/`--3way`/`--unidiff-zero`; the file verb's prompt says the file is deleted and can't be undone):
  - emit the patch from the exact `FileDiff` the user acknowledged, never from a re-read;
  - no `--recount`, `--3way` or `--unidiff-zero` on any apply;
  - a whole discard of an untracked file must say it cannot be recovered;
  - an all-lines discard of an added file goes to the file verb (the item above).

## Carried forward from phase 01's QA (owned by the phase named)

- **Phase 03** (QA item 18; done: the builders refuse, `NoPaths` and `NothingSelected`): an empty `DiscardFiles` renders "the changes in 0
  files? ." and a zero-line `DiscardLines` "discard 0 lines"; "never empty" is a
  doc comment, not a check. The builder that computes either refuses empty input,
  with a test — or the type is made non-empty then.
- **Phase 03** (QA item 19; done: `DiscardLines.mode`): no variant expresses a mode change selected for
  discard (R2.4); add it with the operation, with a test.
- **Phase 03** (QA item 25; done: the bytes on disk, hashed with no filter, re-checked): the working-tree blob in a `Consequence` is hashed
  through clean filters, so an edit that changes only line endings after the
  confirmation matches and is lost. Decide the deletion re-check's hashing
  (consider `git hash-object --no-filters` for it) and test it.
- **Phase 09 — done** (QA item 21): `Consequence::force_push_warning` and `replaces`
  (`an_amends_parts_are_its_prompt_and_only_a_published_one_needs_the_dialog`); the box draws
  `replaces` under an in-place amend button whose token records exactly it, and the dialog the
  whole prompt.
- **Phase 11 — done** (QA item 23): settled by the user's decision G (2026-10-09) — the prompt
  says the index is what a running git's lock protects and that removing it cannot be undone
  (`removing_a_lock_names_it_its_age_and_the_risk`).

## The user's decisions on phase 01's QA (2026-10-08), applied

- **Item 6**: `DESTRUCTIVE_OPERATIONS` is never empty, and the guard asserts it.
  **Phase 03** (done) must replace the `describe_destructive` row with its real
  operations in the same change that deletes the placeholder, or the guard fails.
- **Item 13**: R10.6 amended — the amend prompt's recovery sentence is
  conditional on the reflog (`Consequence::Amend`'s `reflog: Reflog`). Phase 01
  defines the field and both renderings. **Phase 05** computes it in the engine
  (R6.4, amended): `Written` when git will write the entry — `core.logAllRefUpdates`
  `true` (the default with a working tree; `false` when bare) or `always`, or the
  ref's log file already existing, since git appends to an existing log (checked
  with git 2.56 in phase 01) — read as git reads the setting, with a test for each
  arm against real git, including a bare repository and a log that exists under
  `false` (done in phase 05: `whether_the_reflog_is_written_is_what_git_then_does`, a bare
  repository through its linked worktree, since a bare one has no work tree to amend in;
  see phase 05's carry to phase 10). Phase 10 — done: `whether_the_reflog_is_written_is_what_git_then_does`
  now holds every arm to Show Lost Commits' walk (the replaced commit drawn when git logged the
  amend or a ref reaches it, dimmed exactly when none does). Phase 10 should confirm that what Show Lost Commits walks (C20: the
  reflogs of `HEAD` and each local branch) is the entry this decides.
- **Item 34a**: R1.2 amended to Fork's wording, lines per modified path and bytes
  per untracked file (L8); no code change.

## Validation status

| Phase | Status |
| --- | --- |
| 01 seal | done — QA adjudicated, all confirmed fixes and the user's three decisions (items 6, 13, 34a) applied, full gate green |
| 02 patch engine | done — QA adjudicated, confirmed fixes applied, full gate green; items 10-11 batched for the user |
| 03 write verbs | done — QA adjudicated, fixes and the user's decisions applied, full gate green |
| 04 local lane | done — QA adjudicated, fixes and the user's decisions 12 and 14 applied, full gate green |
| 05 commit engine | done — QA adjudicated, fixes and the user's decisions A-F (2026-10-09) applied, full gate green at c07c076 |
| 06 render foundations | done — QA adjudicated, fixes and the user's decisions 6, 12, 13 and 14 (2026-10-09) applied, full gate green |
| 07 Local Changes actions | done — QA adjudicated, fixes and the user's decisions (2026-10-09) applied, full gate green |
| 08 diff gesture | done — QA adjudicated, fixes and the user's decisions (2026-10-09) applied, full gate green |
| 09 commit box | done — QA adjudicated, fixes and the user's decisions (2026-10-09) applied, full gate green at 18fea48 |
| 10 lost commits | done — QA adjudicated, fixes and the user's decisions A-F (2026-10-09) applied, full gate green |
| 11 activity and measured | done — QA adjudicated, every fix-now item fixed, the user's decisions A-N (2026-10-09) applied (DO3+DO5 resolved by H), full gate green |
| 12 QA (first merge bar, doc now `phase-21-qa.md`) | merge-bar QA adjudicated, fixes applied; W1 and W3 resolved by the redesign, F2 and F7 by the user's answers (2026-10-10); design pass done, rebuild planned (2026-10-10) |
| 12 output once | done — C31 pinned; QA adjudicated, fix-now items fixed, full gate green |
| 13 the commit engine asks git | planned — the confirmed amend's skip decided (option (a), 2026-10-10) |
| 14 Create Branch | planned |
| 15 the discard prompts | planned |
| 16 one selection | planned |
| 17 one file's diff | planned |
| 18 the commit box | planned |
| 19 one home for each message | planned — F7 decided (2026-10-10) |
| 20 the Activity Manager | planned |
| 21 QA | planned — the merge bar over the whole packet |
