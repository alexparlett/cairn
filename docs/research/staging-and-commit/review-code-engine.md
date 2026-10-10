# Engine review: staging-and-commit (cairn-model + cairn-git), read-only

Scope: `git diff origin/main...HEAD -- crates/cairn-model crates/cairn-git` (82 files, ~18.7k lines added; 173 commits on the branch, 24 of them `fix`/`refactor` in these two crates), plus `docs/work/staging-and-commit/progress.md` for the fix rounds.

## Verdict (honest)

The engine is NOT a pile of hacks. The core seams are good (see "What is good"). The owner's suspicion is right in three places, though, and they are structural rather than cosmetic:

1. The **`Consequence` / re-check design** is a hand-maintained parallel of "what I observed" and "how I re-observe it". Every QA round that touched a destructive op (chmod, symlink parent, obstructed path, intent-to-add, git-form hash) added a field to each of ~6 variants and a clause to a hand-written comparator. That is the single biggest source of churn and will keep producing bugs.
2. **Create Branch "Discard"** (`checkout -f`) is an open-ended re-implementation of git's checkout loss semantics. Phase 10 QA found three CRITICAL gaps in it, each fixed by adding another loss kind; it still documents an undisclosed loss class.
3. **Text scrubbing / "where was it cut"** is three mechanisms (a stateful credential scrubber with a "try every byte as the cut" mode, `Retained.cut`, and `stderr_cut: Vec<usize>`), compensating for the fact that scrubbing happens after truncation, on arbitrary slices. The same UTF-8-cut bug fixed for a commit's stdout is still live in the stderr splitter.

Counts: 3 HIGH, 8 MEDIUM, 7 LOW, plus 9 good-design items.

---

## Findings

### H1 (HIGH). Re-check is two hand-written mirrors of one observation; the "witness" triple is copy-pasted into six variants
- Where: `crates/cairn-model/src/consequence.rs:33-250` (`DiscardLines{index,working_tree,on_disk,executable}`, `FileLoss::Modified/Emptied/Untracked`, `ChangeLoss::Changed/Overwritten` each carry some subset of {index blob, working-tree hash, executable bit}); `crates/cairn-git/src/ops/discard.rs:148-181` (`discard_lines` re-check) and `:469-515` (`as_confirmed`, one match arm per `FileLoss`); `ops/fresh_state.rs:49-98` (`IndexSide::is_old_side` and `IndexSide::holds`, near-identical, plus a third inline copy in `ops/stage.rs:112-123` `unstage_lines`).
- Evidence: commit `ab6478d` (chmod after confirmation) had to add `executable` to each variant (consequence.rs +42, discard.rs, fresh_state.rs); `6835fbb` (parent turned into file/symlink) added `OnDisk::Obstructed` and arms in several places; `1031f27` (the confirmed patch carried in the Consequence); `4c9614b`, `e5a4cd2`. Phase 03 QA items 1, 2, 3, 4, 5, 6 were all "the re-check missed X".
- Symptom: a destructive op's re-check silently disagreed with what the Consequence recorded (chmod undone silently; file unlinked where a dir was).
- Patch: add the missing field and a clause in the comparator.
- Root cause: the Consequence mixes two things: (a) the *witness* of repository state the op will re-verify, and (b) *description* for the prompt (line counts, modes, byte sizes). The witness has no type, so each variant spells it differently and each op re-implements "compare witness" by hand, differently from how the builder derived it.
- Clean design: one `PathWitness { index: IndexSide-like (Absent | Entry{id,mode,intent_to_add}), disk: OnDiskKind{Absent|File{id,exec}|Symlink{id}|Dir|...}, git_form: Option<Oid> }` in `cairn-model`, produced by ONE function in `fresh_state.rs` (`witness(repo, path)`), stored in the Consequence, and re-checked by `witness(repo,path) == stored`. The prompt reads description fields only. Then `is_old_side`/`holds`/`as_confirmed`/the inline unstage match collapse to equality, and "add chmod/symlink/etc." is one place.
- Cost: medium-high (~1-1.5k lines touched across consequence.rs, discard.rs, checkout.rs, tests); no behaviour change; the existing tests (`write_verbs.rs`, `branch.rs`) are the safety net.

### H2 (HIGH). Create Branch "Discard" predicts `git checkout -f`'s losses by re-implementing unpack-trees; known unmodelled loss
- Where: `crates/cairn-git/src/ops/checkout.rs:77-130` (consequence), `:268-405` (`untracked_losses`, `held`, `content_under`), `:441-482` (`first_difference`); `crates/cairn-model/src/consequence.rs:100-185` (`ChangeLoss`, `RemovedKind`, `ChangedKind`); `reads/untracked.rs`, `reads/change_lines.rs`.
- Evidence: phase 10 QA items 1-3 (CRITICAL) in `progress.md:361-380`: untracked files under a dir the commit holds as a file, nested repositories, `showUntrackedFiles=no` hiding files. Each fixed by a new loss kind (`Removed{Directory|Repository}`, `ls-files --others`). `checkout.rs:21-26` states what the seal still does not count: ignored files/dirs at a path the commit holds a file at (git deletes them, Cairn's prompt does not say so), and a "directory named lost" is re-checked only by file count and bytes (an edit that keeps a size is invisible). Another residual: `lines` sums staged+unstaged `--numstat`, so a hunk staged then edited again is counted twice.
- Root cause: the product promise is "tells you what it will cost before it costs it", but git offers no dry-run of `checkout -f`, so Cairn models D/F conflicts, nested repos, sparse/ignored behaviour itself. This is an unbounded surface (case-insensitive FS, sparse checkout, symlinks in the way, `core.protectNTFS`, submodules are only refused).
- Clean design options: (a) compose from already-modelled primitives: Discard (existing `DiscardFiles` consequence, plus a staged reset) then plain `git checkout -b` without `-f`, which makes git itself the safety net: it refuses, writing nothing, wherever something unmodelled would be overwritten. The prompt then names only what Cairn actually deletes. (b) Keep `-f` but state the prompt as a conservative over-approximation ("every change to tracked files, and untracked files at paths the target commit contains") and make the ignored-file gap an explicit line in the prompt. (a) removes ~300 lines and the CRITICAL class. It does change the user-decided "one exception: staged changes are discarded" (needs `git reset` of the index step in the same op) so it is a product decision.
- Cost: (a) medium; touches model prompt + checkout.rs + tests; needs the user's sign-off since decision 3 (2026-10-09) fixed the `checkout -f` behaviour.

### H3 (HIGH). Scrubbing happens after truncation, so it grew a state machine plus three "where was it cut" encodings; one live bug in the parallel splitter
- Where: `crates/cairn-model/src/scrub.rs` (Scrubber with `Carried::{Nothing,Scheme{seen},Authority,Cut}`), `crates/cairn-git/src/process/pipes.rs:106-139` (`Lines`: splits stderr at `\n` or `\r` and, at `TAIL_BYTES`, in pieces) and `:152-195` (`Tail`/`Retained.cut`), `ops/commit.rs:358-452` (a second, separate line splitter `Lines` for commit stdout, with `whole_characters`, `STDOUT_TAIL`, `joined_output`), `error.rs:236-243` (`stderr_cut: Vec<usize>`), `process/runner.rs:460` (`ended.tail.cut.then_some(0).into_iter().collect()`).
- Evidence: commits `79bdce7` (never cut a commit's output inside a character), `dc454b0`, `dd073f7` (say where git's kept output was cut), `64cd56d` (scrub a URL cut at or inside `://`; "every byte of a line is now tried as the cut, both ways"), phase 11 QA TC3/TC4/TC5.
- Live defect (verified by reading): `pipes.rs:121-126` pushes a "piece" when `pending.len() >= TAIL_BYTES` and decodes it with `from_utf8_lossy`, with no char-boundary care. That is exactly the bug `79bdce7` fixed in `commit.rs`'s own stdout splitter (`whole_characters`). It remains for every other git invocation's stderr (fetch, clean, restore, apply). Also `pipes::Lines` splits on `\r`, commit's `Lines` does not: two line definitions.
- `stderr_cut: Vec<usize>` is a stringly offset list whose only non-zero use is the commit path (offset of stderr's cut tail after stdout's, `commit.rs:301-318`); everywhere else it is `[0]` or `[]`, built with `.then_some(0).into_iter().collect()`.
- Root cause: credentials are scrubbed in the view, after the runner has already cut the text at arbitrary byte offsets. The scrubber then has to guess the unseen prefix ("Cut" mode assumes every byte could be inside `://`).
- Clean design: scrub (and split into characters) once, at the runner, on whole lines before any truncation: one `Lines` type in `process/pipes.rs` (char-boundary aware), used by stdout and stderr; the `Tail` keeps already-scrubbed complete lines and drops from the front by line, so nothing starts mid-line and `cut` is a plain bool ("older lines dropped"). Commit's stdout uses the same type. `Scrubber::after_cut` and `stderr_cut: Vec<usize>` disappear; the scrubber needs only the cross-chunk carry for a line longer than the piece limit. Note `cairn-model` already has the scrubber; the runner is in `cairn-git` which depends on `cairn-model`, so this is allowed.
- Cost: medium (pipes.rs, commit.rs, runner.rs, error.rs, app's `shown_output.rs`; tests exist).

### M1 (MEDIUM). `Consequence` is a closed union that every operation has to pattern-match defensively; four `NotWhatWasConfirmed` refusals and four "changed since confirmed" errors
- Where: `confirm.rs:70-93` (`Confirmed` holds untyped `Consequence`); every op: `discard.rs:166-170,375-385`, `checkout.rs:128-137`, `commit.rs:149-157` (`amend`), `remove_lock.rs:108-128`; `error.rs` has `Refusal::NotWhatWasConfirmed`, `CheckoutRefusal::NotWhatWasConfirmed`, `CommitRefusal::NotWhatWasConfirmed`, `LockRefusal::NotWhatWasConfirmed` (16 occurrences in cairn-git/cairn-app) and `ChangedSinceConfirmed{path}`, `AmendChangedSinceConfirmed`, `LockChangedSinceConfirmed{path}`.
- Also: Amend-only helper methods hang off the general enum and return `Option` for every other variant (`force_push_warning`, `amended`, `replaces`, `needs_force_push`, `consequence.rs:336-390`); `name()` and `action()` copy the same RemoveLock arm.
- Root cause: operation type and token type are decoupled; "wrong variant" is a runtime state that cannot occur if the caller is right.
- Clean design: per-operation structs (`DiscardLines`, `DiscardFiles`, `Amend`, `RemoveLock`, `CheckoutDiscarding`) each implementing a sealed `Describes` trait (`prompt/action/name`), `Confirmed<C: Describes>`; the UI's dialog state keeps one `enum AnyConsequence`. Wrong-variant branches, the 4 refusal variants, and the Amend `Option` accessors vanish at compile time. The confirmation guard's roster logic would need updating.
- Cost: medium-high because the guard suite (`destructive_operations_are_sealed_behind_the_confirmation_token`) and `cairn-app` dialog state are keyed to the current shape; the benefit is moderate (it removes dead branches, not bug classes). Do H1 first.

### M2 (MEDIUM). Two re-check philosophies side by side
- Amend (`commit.rs:160-166`) and checkout (`checkout.rs:152-157`) recompute the whole Consequence and compare by `==` (checkout then derives the name of the first difference with `first_difference`, which has a catch-all `_ => Some("HEAD")` arm at `:476` and a zip/longer-list heuristic). Discard lines/files, remove-lock compare selected fields by hand. Both are legitimate (full recompute is expensive for N discards) but there is no shared contract; the `==` route silently includes display fields (`subject`) as freshness conditions, and the by-hand route silently excludes them.
- Clean design: H1's witness. Recompute-and-compare is then `witness == witness` everywhere, and display fields are never part of equality. `first_difference` could report from the witness path instead of zipping lists.

### M3 (MEDIUM). Create Branch name check re-implements git's lock failure, plus a Cairn-worded special case for `@{`
- Where: `crates/cairn-git/src/branch_names.rs:18-85`, `reads/branch_name.rs` (`Result<Result<String,String>,Error>`, stringly), `crates/cairn-model/src/branch_name.rs`.
- Evidence: `350541e`, `7ffeb72` (D/F lock clashes added after QA item 15), `73232e0` ('@{' refused in Cairn's words; the user's decision F).
- Root cause: `git check-ref-format --branch` *resolves* `@{-N}` to a previous branch name and so is the wrong oracle for `git checkout -b <name>` (which takes the literal). The `@{` refusal exists to hide that mismatch; the D/F check hand-rolls what `git update-ref`'s lock would say, and words the reason "in git's words" by string-formatting `'{above}' exists; cannot create '{reference}'`.
- Clean design: validate `refs/heads/<name>` with `git check-ref-format` (no `--branch`) so there is no resolution; keep the existence/D-F lookup via gix but return a typed `NameRefusal{ Invalid(String), Taken, DirectoryHolds(RefName), UnderneathIt(RefName) }`; the UI words it. And inline-check is advisory only: the checkout itself (`git checkout -b`, atomic) remains the authority, which it already is for the kept path.
- Cost: low-medium (~150 lines + tests). Divergence note: the project treats divergence from git as critical; the `@{` rejection rejects names git's `branch` would refuse anyway, so this is low risk but the wording is Cairn's.

### M4 (MEDIUM). Git reimplemented, not asked: operation-in-progress, hook executability, reflog parser, logAllRefUpdates
- `operation_in_progress.rs`: reimplements `wt_status_get_state` from marker files plus a hand-parsed `sequencer/todo`, with an invented precedence ("a refusing operation answers ahead of a merge"). Test `a_refusing_operation_answers_ahead_of_a_merge` documents choices git does not make.
- `commit_hooks.rs`: `runs()` re-derives `access(X_OK)` from mode bits; the code states its own wrong case (group bit counted regardless of group membership, for a file owned by someone else). Offers/withholds the "skip hooks" button.
- `history/reflogs.rs`: a full reimplementation of `show_one_reflog_ent` and `strtoumax` quirks (`nonzero_timestamp`), because gix's reader stops at a long line. Impressive and well-tested, but it is git's parser maintained by hand, reading only HEAD's and local branches' reflogs where it advertises `git rev-list --reflog` (which reads all refs' reflogs).
- `ops/amend.rs:177-199` `reflog()`: re-derives `should_autocreate_reflog`/`log_ref_setup` from gix's config snapshot. The project itself documents that gix's config reading is not git's for linked-worktree `includeIf`, the system file and trust (`reads::fetch_settings`). So there are two mechanisms for "what is config": `git config` for fetch, gix config here and in `commit.rs:utf8_messages`, `diff/git_config.rs`. A wrong answer here makes the amend prompt promise "The old commit stays in Show Lost Commits" when git writes no reflog.
- Clean design: ask git where it has a query form (`git config --includes --get core.logAllRefUpdates` through the existing `reads::fetch_settings`-style named read; `git rev-parse --git-path`; `git var`/`rev-parse` for state when possible), and keep hand-parsers only where git exposes nothing (state detection, reflog bytes). At minimum one config-reading route.
- Cost: low for the config route, higher for the rest; most are LOW individually.

### M5 (MEDIUM). Amend's staged list is a fake `WorkingTreeStatus`
- Where: `crates/cairn-model/src/local_changes.rs` (`LocalChanges::amending`, `amended_entries`, `StagedAgainst`), `crates/cairn-git/src/diff/amend.rs`.
- The amend view takes the real status, then rewrites each `StatusEntry`'s `staged` from a different diff (HEAD's parent vs index), manufacturing `SubmoduleState{false,false,false}` for submodules and dropping entries when it empties them. Conflicted/untracked entries are special-cased. `StagedAgainst` is a flag read to decide Stage/Unstage targets elsewhere.
- Root cause: the Local Changes model's only constructor input is `WorkingTreeStatus`; amend needed a second source of the Staged list, and was bent into the first.
- Clean design: `LocalChanges::from_parts(staged: Vec<ChangeRow>, unstaged: Vec<ChangeRow>, against)` with two adapters, one from status, one from "amend's staged + status's unstaged". Removes `amended_entries` and the fake submodule state.
- Cost: medium; `local_changes.rs` is +285 lines and well-tested.

### M6 (MEDIUM). `Reach` is an approximation of `rev-list --not` with a retroactive correction path
- `history/reach.rs` + `walk.rs`: lost/not-lost decided as the date-ordered walk goes; clock skew produces a row first handed on as lost then "reached after all" via `take_reached`, so a row's dimming can change after it has been drawn. This is a patch over a streaming assumption (children precede parents) that git itself does not make (it uses generation numbers / full marking).
- Parallel mechanism: the amend's published check uses `walk::hiding` / `reaches_through_graph` for the same question ("does any ref reach this commit").
- Clean design: for the lost-commit question, compute reachability once per ref change from the commit-graph generation (the primitive already written for the amend check: `reaches_through_graph`), and per row ask it; or run the real `git rev-list --not` once off-thread. The approximation is a deliberate perf trade for monorepos (page 1 in 17.5 ms measured), so this is MEDIUM not HIGH, but the divergence path (dim flips after draw) should be named in the UI contract, and it is a second reachability engine.

### M7 (MEDIUM). Commit-output capture is a bespoke second runner path
- `ops/commit.rs` owns its own line splitter, 64 KiB tail, UTF-8 repair, join of stdout+stderr and a `Polled` adapter, beside the runner's `drive`, `finish`, `finish_by_read`, `finish_within`, `collect` (five ways to run a process, `process/runner.rs:332-512`; the `helpers_by_kind` test uses an autoref-fallback trick to prove methods are absent on writes). This is the same family as H3, and is why `79bdce7` and `dd073f7` were commit-specific fixes. Fold into the runner's one line stream (H3).

### M8 (MEDIUM). `OnDisk` is a state enum with two error-ish states and caller-by-caller handling
- `fresh_state.rs:148-192`: `Changing` and `Obstructed` are failure modes, not states of a path. Callers handle them in different ways (`not_a_file` maps `Changing` to `ContentReadsDisagree`; `discard_lines_consequence` reads twice and compares; checkout ignores). `Obstructed` was added by `6835fbb` after a CRITICAL QA finding. Return `Result<OnDisk, DiskRefusal>` so unrepresentable-as-a-file is an error at the read, not a value every consumer must remember to refuse. LOW-MEDIUM; bundle with H1.

### L1 (LOW). `TextDiff::inverted` splits a replacement into two changes to fit the forward emit rule
- `diff_text.rs`: not an involution ("inverting twice gives the same sides and not necessarily the same changes"). Well argued against git's `reset -p` ordering and tested (`an_inverted_replacement_puts_its_insertion_ahead_of_its_removal`), so it is a deliberate accommodation, not a hack. The cleaner shape would be an `emit_patch(…, Direction)` that handles ordering itself; cost is high for little gain. Leave.

### L2 (LOW). Empty `Patch` is a sentinel for four different refusals
- `action_patch` returns `Patch::empty()` for nothing-selected, whole-file-only, not-text, binary; `ops/stage.rs:150-163` (`patch_of`) re-derives which refusal it was by testing `selection.is_empty()` first. `discard_lines_consequence` repeats a `WholeFileOnly` check itself (`discard.rs:116-123`). `action_patch` should return `Result<Patch, PatchRefusal>`.

### L3 (LOW). `UnstageTo::Nothing` = `git rm --cached -f -q`
- `stage.rs:200-230`. A root-commit amend has no parent to reset to, so a different verb with `-f` is used (commit `4c9614b`). It is a special verb for one state, but it is documented, tested against real git, and git offers no single verb. A cleaner form is `git read-tree --empty`-style only if the whole index is meant; not for per-path. Accept.

### L4 (LOW). Command attribution by `(ThreadId, order)`
- `process/command_log.rs`, `registry.rs`: "what this operation ran" is "what this thread built since a mark". Correct only while an operation never hops threads. A correlation id carried on the invocation would not depend on that. Works today (the local lane is serial).

### L5 (LOW). Stringly nested results / duplicated trivial code
- `reads/branch_name.rs:28` `Result<Result<String,String>,Error>`; `Consequence::name` and `Consequence::action` duplicate the RemoveLock arm; `error.rs` grew 37 variants with five nested `*Refusal` enums.

### L6 (LOW). Three index-side comparators (`is_old_side`, `holds`, the inline match in `unstage_lines`); folded into H1.

### L7 (LOW). Net-new git versioning gaps are documented, not tested at edges
- `PATHSPEC_FILE` for `rm` needs git 2.26: inside floor 2.30; fine. `reflog` rule checked on 2.56 only in prose, floors run the test.

---

## Divergence from git (point 4)

No divergence was introduced *to dodge a problem* that I could find; the opposite: the engine is unusually strict about parity (writes delegate to git; patches are emitted to match `git diff` byte-for-byte including C-quoting, `--- a/x\t` tabs; hash-object `--path` for the git form; `--whitespace=nowarn`; `--literal-pathspecs`). Remaining divergences/risks:
- Create Branch Discard: ignored files deleted by `checkout -f` are not in the prompt (documented residual) — H2.
- `reflog()` and `utf8_messages` read config via gix (not git's route) — M4.
- Hook executability for foreign-owned files — M4.
- Lost-commit reachability can flip after draw on clock skew — M6.
- Cairn-worded `@{` refusal (user decision F) — M3.
- Reflog tips: HEAD and local branches only, while described as `rev-list --reflog` — M4.

## What is good (be fair)

1. **One patch rule, three diffs** (`action_patch.rs`): stage = forward patch of the unstaged diff; unstage/discard = the same rule over the inverted diff; no `-R`; whole-file-only rules centralised. Cleanly factored and exhaustively tested; oracle is real `git apply`.
2. **Mode change as its own selectable item** (`Selection{mode}`), consistent with `git add -p`.
3. **`Confirmed` seal** is real: private fields, one constructor taking a `Consequence` (not text), prompt rendered from the value, compile-fail doctests; operations derive targets only from the token; prompt tests pin literal text and escaping of hostile paths.
4. **`Kind`-typed invocations**: a write cannot hold a ceiling (`e7df04a`), bounded-output helpers exist on reads only, enforced by the compiler; locks listed before and after every local write (`Locks`).
5. **`fresh_state.rs`** streams the on-disk hash with no process, detects a file changing mid-read, and never follows a symlinked parent. The hash/git-form split (bytes vs `hash-object --path`) is right and explained.
6. **Discard of files**: re-read after the run, `DiscardIncomplete{performed, kept, failure}` reports exactly what happened rather than a bare failure; `clean_batches` bounds argv.
7. **Reads vs writes** discipline held across ~12 new verbs with `reads/*` named functions and guard pins; no process spawned outside `process/`.
8. **Process attribution fix `Retained{text,cut}`** is the right direction (engine states the fact rather than guessing from length); M/H3 only asks to finish the job.
9. **`reflogs.rs` and `reach.rs`** are carefully argued and well-tested for what they do; `reaches_through_graph` uses generation cut-off as git does.

## Fix rounds, summarised (engine side)
24 fix/refactor commits in scope, grouped by root cause:
- Re-check/witness incompleteness (H1): `d859f1d`, `ab6478d`, `6835fbb`, `1031f27`, `e5a4cd2`, `c24c4d8`, `19ad8df`.
- Predicting git's checkout (H2): `2f63745`, `7ffeb72`, `28d1afa`.
- Output cut/scrub (H3): `79bdce7`, `dc454b0`, `64cd56d`, `dd073f7`, `d017152`.
- Amend/published check: `44ccdb1`, `4c9614b`.
- Misc: `b49d4b4` (cancel), `297f15b`, `4e5282d`, `93b0594`, `d78e354` (wording).
Roughly 9 of 24 trace to H1, 3 to H2, 5 to H3; i.e. over 70% of engine fix rounds came from three design choices.

## Suggested order if fixing
1. H3 (small, isolates the one live bug in `pipes::Lines`, and deletes `stderr_cut: Vec<usize>`).
2. H1 (witness type) — biggest bug-class reduction.
3. H2 decision with the user (compose Discard + plain checkout vs keep predictor).
4. M1/M2 after H1; M3, M4 config-route as small independent changes.
