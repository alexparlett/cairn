# Implementation plan — staging-and-commit

## Shape

```
cairn-ui     Local Changes ── Unstaged / Staged (multi-select, header buttons, drag, menu)
                  │             └─ diff ── hovered chunk: Stage │ Discard… │ Unstage
                  │                        drag-selected lines, mode row
                  ├─ commit box ── subject (counter, recent) │ description (ruler) │ Amend │ Commit N Files
                  ├─ confirmation dialog ◄── Consequence (prompt rendered from it)
                  ├─ history toolbar ── Show Lost Commits ── dimmed rows ── Create Branch Here…
                  └─ toolbar status ── activity popover (operations, prompts, git, stderr)
                  │
cairn-app    local write lane (own thread, FIFO, op ids, cancel a commit)
             write counter ── a status begun before the last write ended is dropped
             refresh by Invalidated │ quiet while a commit runs │ close waits for a write
                  │
cairn-git    ops/   apply --cached │ apply │ add │ reset │ restore │ clean │ commit -F - │ branch
                    each destructive one: Confirmed by value, Consequence re-checked first
                    remove index.lock: the one std::fs removal, guarded to ops/
             reads/ hash-object (no -w) │ rev-parse --git-path hooks
             diff/  staged side pairs renames as git diff --cached
                  │
cairn-model  Confirmed + Consequence │ inversions of TextDiff, Selection, ChangedFile
             Selection's mode item │ C-quoted paths in emit_patch
```

The type names above are suggestions for the phases to settle; the PRD fixes what
they carry, not what they are called.

## Phase order and why

1. **01 seal** first: every destructive verb after it is written against the
   token's final shape, and the guards that police the rosters exist before
   anything is put on them. It also makes the runner's bounded-output helpers
   read-only at the type (R4.8), a `process/` change best made before new writes
   multiply.
2. **02 patch engine** before any verb: the verbs only carry patches the model
   emits, and C3's round trips are the riskiest engine work in the packet.
3. **03 write verbs** on 01's seal and 02's patches: every file and patch verb,
   the stale check, the lock outcomes, and git's own baseline numbers (R13.2), so
   C21's margin is written before any Cairn number exists.
4. **04 local lane** once there are verbs to carry: the lane is designed against
   real operations, the write counter against the real status lane.
5. **05 commit engine** after the lane, because a commit is the operation whose
   hooks make the lane's quiet period (R4.6) and cancel (R4.3) real.
6. **06 render foundations** before any view acts: the key policy, the
   accelerator table's amendment, the dialog and the menu host are shared by 07-11.
7. **07-09 UI** in dependency order: Local Changes' file actions (the smallest
   surface over 03 and 06), then the diff's gesture (which needs 07's selection
   and 02's mode item), then the commit box (which needs 05 and 06's dialog).
8. **10 Show Lost Commits** after commit and amend exist, since an amend is what
   it recovers.
9. **11 activity popover** last among features, because every operation before
   it feeds it; Cairn's measured numbers and the window check close the build.
10. **12 QA** as its own fresh session, the merge bar.

## Review dispatch per phase

The packet's one copy of the rules. Every phase ends by orchestrating its own
QA: `/qa` over the phase diff with these reviewers spawned fresh, and
`qa-confirm` (fresh) adjudicating. `qa-checklist` is always on and is not
repeated in the table.

| Phase | Reviewers, beyond `qa-checklist` |
| --- | --- |
| 01 | `destructive-ops-reviewer` — the seal itself, the `Consequence` and the pre-run re-check contract; `gate-integrity-reviewer` — two new rosters, their guards and self-tests, the strengthened Confirmed twin; `test-coverage-auditor` — the `compile_fail` pins decide what they claim |
| 02 | `test-coverage-auditor` — C3's oracles are git and the mirrored derivation, never a golden file; `destructive-ops-reviewer` — the staged diff's rename change is a read in `diff/`/`reads/` (its check 10) |
| 03 | `destructive-ops-reviewer` — every verb, its argv, what it can destroy and whether its prompt says so (its whole checklist); `gate-integrity-reviewer` — two new reads and any roster they extend (#54); `test-coverage-auditor` |
| 04 | `responsiveness-reviewer` — the lane, the write counter, what the UI thread calls to submit; `destructive-ops-reviewer` — closing mid-write, prompts during writes, the inherited roster (its check 9); `gate-integrity-reviewer` — the environment twin's roster; `test-coverage-auditor` |
| 05 | `destructive-ops-reviewer` — commit, amend, `--no-verify`, the pushed check; `test-coverage-auditor` |
| 06 | `gate-integrity-reviewer` — the accelerator pin's amendment must fail toward more coverage; `responsiveness-reviewer`; `test-coverage-auditor` |
| 07 | `responsiveness-reviewer`, `test-coverage-auditor`; `destructive-ops-reviewer` — the discard dialog's honesty |
| 08 | `responsiveness-reviewer` — hover and drag over a virtualized list, per-frame work; `test-coverage-auditor` |
| 09 | `responsiveness-reviewer`, `test-coverage-auditor`; `destructive-ops-reviewer` — amend's confirmation surface and the skip-hooks route |
| 10 | `responsiveness-reviewer` — the walk's extra tips and the dimming pass; `destructive-ops-reviewer` — `Create Branch Here…`; `test-coverage-auditor` |
| 11 | `responsiveness-reviewer`, `test-coverage-auditor`; `destructive-ops-reviewer` — `Remove index.lock…` and the scrubbed stderr; `gate-integrity-reviewer` — the filesystem-mutation guard (R12.5), its matcher, self-test and exceptions roster |
| 12 | all of the above, over the whole packet diff |

## Invariants in play

- **Destructive operations take `cairn_model::Confirmed` by value** — strengthened
  here (R1): no `Clone`, a constructor roster, a destructive-operation roster, a
  `Consequence` re-checked before each run. The twin
  `destructive_operations_are_sealed_behind_the_confirmation_token` must stop
  being satisfiable by `ops/mod.rs` alone.
- **Only `cairn-git/src/ops/` mutates a repository.** Every verb is a write
  invocation built in `ops/`; every read added (R3.9) is a named function in
  `reads/`, built with `read_invocation`. `the_porcelain_reads_are_the_three_named_queries`
  reads the `"diff"`, `"config"` and `"stash"` literals only; the new reads are
  query plumbing it does not see, which `destructive-ops-reviewer` reads verb by
  verb (#54 would make it a guard). One mutation is not a `git` invocation:
  `Remove index.lock…` deletes `<gitdir>/index.lock` through `std::fs` in `ops/`
  (R12.4, L23), and a new guard holds every filesystem-mutating call to `ops/`
  (R12.5).
- **Every `git` subprocess runs with an environment Cairn built.** R5.2 extends
  `INHERITED`; `every_git_invocation_disables_the_terminal_prompt` reads the
  file, and the roster's rightness is `destructive-ops-reviewer`'s check 9.
- **The UI thread never waits on repository work.** The lane lives under
  `crates/cairn-app/src/worker/`; the render side only submits. The auto-scroll
  timer (R7.6) runs on the toolkit's executor — no waiting primitive on a render
  path.
- **No component names a literal modifier.** Every chord of R7.3, each action's
  scope and the bare-key scope live in `crates/cairn-ui/src/accelerators.rs`, an
  action now mapping to a list of chords per platform (L22), which amends the
  invariant's "at most one chord per platform"; the multi-select presses
  resolve through `HeldKeys`, never a pointer event's modifiers.
- **No unbounded list renders without virtualization.** The gesture's overlay is
  drawn outside the rows; the activity popover's list and Show Lost Commits'
  rows are virtualized; the `ScrollView` exceptions roster stays empty.
- **`DiffContent`, `UnifiedRow` and `SideBySideRow` are read by naming every
  variant.** A mode row (R9.4) is a new kind of row; every reader the compiler
  names is fixed by naming it.
- **No credential value is logged, printed or stored.** Prompts during local writes
  reuse the askpass channel; the scrubbed stderr (R12.2) is display only.
- **`cairn-model` changes carry tests in the same commit**, and so do `ops/` and
  `cairn-guards` changes.
- **Every dependency addition is a user decision.** `async-io` is named in
  `cairn-ui` by L5 and needs its allowlist row and `deny.toml` reason.

## New enforcement this packet must leave behind

1. **The destructive-operation roster** (C1): every rostered operation in `ops/`
   takes `Confirmed` by value, and every `ops/` function that takes one is
   rostered.
2. **The confirmation-surface roster** (C1): `Confirmed`'s constructor is named
   only in the dialog component and the commit box.
3. **`Confirmed`'s shape** (C1): `compile_fail` doctests for `.clone()` and for
   construction outside `cairn-model`'s visibility, with a passing twin.
4. **The chord lists and the bare-key scope** (C16): the table maps an action to a
   list of distinct chords per platform, the root `CLAUDE.md` modifier invariant
   ("at most one chord per platform") and its twin amended to read lists (L22);
   the table's pin admits bare Enter, Backspace and Delete in the one scope only.
5. **A viewport twin** for Local Changes' diff with the gesture drawn (C19).
6. **The filesystem-mutation guard** (R12.5, C20, C22): no production file
   outside `crates/cairn-git/src/ops/` removes, writes, renames, creates or changes
   the permissions of a file or directory, beyond an exceptions roster for what
   writes outside any repository (the askpass channel's socket directory), each
   row failing when no longer needed; a nonzero-files assertion and a matcher
   self-test (L23).

Each goes into `CLAUDE.md` with its guard name in the same commit as the guard,
with the residual it cannot express stated.

## Docs obligations by phase

| Phase | Doc |
| --- | --- |
| 01 | the root `CLAUDE.md` Confirmed invariant (its twin, the two rosters, residuals); `docs/systems/git-processes.md` where it describes the seal and the bounded-output helpers |
| 02 | `docs/systems/diff.md`: inversion, the mode item, quoting, partial untracked files, the staged side's renames |
| 03 | `docs/systems/git-processes.md`: every write verb and the two reads; the root `CLAUDE.md` D1 paragraph (C22); a new `docs/systems/staging.md` for the verbs and the stale check, or a section of `local-changes.md` (phase's call) |
| 04 | `docs/systems/git-processes.md`: the local lane, prompts during writes, the roster, closing mid-write; the root `CLAUDE.md` responsiveness residuals for anything new the UI thread calls |
| 05 | `docs/systems/git-processes.md` and the staging doc: commit, amend, hooks |
| 06 | `docs/systems/diff.md`'s accelerator table contract (a list of chords per action, each action's scope); the root `CLAUDE.md` modifier invariant (a list of chords per platform, the bare-key scope) |
| 07-09 | `docs/systems/local-changes.md`: actions, the gesture, the commit box; the root `CLAUDE.md` status paragraph and repo map rows |
| 10 | `docs/systems/history-graph.md`: Show Lost Commits |
| 11 | the activity popover in `docs/systems/git-processes.md` (beside the command log) or its own section; the root `CLAUDE.md` D1 paragraph (the one deletion made without `git`) and the "only `ops/` mutates" invariant (the filesystem-mutation guard, its twin and residuals); measured numbers in `progress.md` |
| 12 | verify all of the above, stamp the PRD, update the roadmap and spine pointers, tear down |

## Technical notes

Verify every gix and Freya API against the vendored source before writing it (the
root `CLAUDE.md` version-sensitive rule). `freya-ui-apis.md` cites the files at
rev `caa46f8`; a phase re-reads them.

- **The seal.** `crates/cairn-model/src/confirm.rs` today: `#[derive(Debug, Clone,
  PartialEq, Eq)]`, `pub fn by_user`. A `cairn-model` constructor cannot know its
  caller's crate, so the constructor roster is a guard over call sites, not a
  visibility; the `Consequence` is built by the engine and cannot be forged into a
  matching shape without the engine's read, which is what the pre-run re-check
  catches. `Performed::destructive` already takes `&Confirmed`.
- **Inversion.** `TextDiff::new(old, new, changes)`; a `ChangedRange` has
  `removed` and `added` spans; `emit_patch` writes `new file mode` for `Added`
  even partially and turns a partial `Deleted` into a modification (E3, E4).
  `apply_patch_in_reverse` exists for the mirrored oracle.
- **Quoting.** git's `quote_c_style`: `"` and `\` escaped, control characters as
  `\a \b \t \n \v \f \r` or octal, bytes ≥ 0x80 as octal under `core.quotePath`
  (the default); `git apply` reads either, but refuses a raw tab (E5).
- **Staged renames.** The working-tree query's staged side passes `--no-renames`
  today (`staging-surface-as-built.md`); `git diff --cached` follows `diff.renames`
  (default `true` since git 2.9). Match it, and pin it under each value (C7).
- **Pathspecs.** Literal mode is git's global `--literal-pathspecs` option on
  `argv`, before the verb, as `reads/patches.rs` and `reads/working_tree.rs` pass
  it — never `GIT_LITERAL_PATHSPECS`, which would be a per-invocation variable
  outside `process/environment.rs` that the environment twin refuses. git exports
  it to the hooks it runs, a residual R3 states.
- **Verbs at the floor.** `restore` (2.23), `--pathspec-from-file` (2.25 for `add`,
  `reset`, `restore` and `checkout`; 2.26 for `rm` and `stash`) and
  `--pathspec-file-nul` exist at 2.30.9; `git clean` has no pathspec file, so its
  paths go on `argv`, batched past a bound phase 03 measures;
  `git apply --allow-empty` (2.35) and `--3way` with `--cached` (2.32) are not
  used (`git-write-verbs.md`).
- **The stale check.** Index entry by gix (`Repository::index_or_empty`, then the
  entry's id) — verify the API; the working-tree hash by `git hash-object
  --path=<p> -- <p>` without `-w`. Compare against the blob id of the drawn new
  side computed in `cairn-git` (gix's hasher), never in `cairn-model`.
- **Locks.** A cancelled or ended write already reports `present_locks` and
  `stranded_locks` (`process/cli.rs`, `ops/mod.rs`'s `stranded_locks` module,
  `write-path-as-built.md` §2); carry them on every local outcome.
- **The lane.** `Lane` and `Operation` have one variant each and
  `Threads::perform` matches `Fetch` directly (`write-path-as-built.md` §3); the
  fetch-only types (`FetchControl`, `Refusal`, `CancelFetch`, `Update::Fetch*`,
  `FetchStatus`) generalise or gain siblings — push (#16) will be the next to need
  them.
- **Prompts.** `session.rs` refuses a prompt unless a fetch is in flight; R5.1
  lifts that for any write holding a token.
- **Commit.** `-F -` with an empty stdin for hooks (verified); `--cleanup` unset;
  hooks see `GIT_EDITOR=:`. Signing failures arrive after the hooks ran.
- **Text fields.** `Input` with `.multiline(true)` is the description
  (`freya-components/src/input.rs`); writing its value from code clears its undo
  history, so the draft is Cairn's state; pasting into the single-line subject
  can insert newlines, which the subject strips.
- **Dialog.** `Popup` does not trap focus; the fork's `a11y_modal` exists but is
  unused — the wrapper applies it, and the window's global key handler must
  consult "a dialog is open" before acting.
- **Menus.** `ContextMenu` panics without a mounted `ContextMenuViewer`.
- **Pointer.** No modifiers on pointer events: Ctrl/⌘- and Shift-click resolve
  through `HeldKeys` and new `Trigger::Press` rows. `Button` takes focus on click
  even with `.focusable(false)`: the floating actions are plain pressables.
- **Reflog.** Read each log whole and parse it, as the stash list does;
  gix's `log_iter().rev()` stops at a line over 4 KiB. Reftable repositories are
  refused at open already.
