# Implementation plan — diff-engine

## Shape

```
cairn-ui     detail pane ── Commit tab │ Changes tab
                  │            file list, diff rows, header toggles
                  ▼
cairn-app    session state ── selection ──► worker lanes ──► cairn-diff thread
                  ▲                          history │ changes │ file diff
                  │                                     (own epoch each)
cairn-git    changes query   ── trees, renames, modes
             content query   ── gix resource cache (to-git) ──► gix computes
             worktree query  ── filter pipeline, drivers included
                  │
cairn-model  file diff: both versions' lines + exact changed ranges
                  ├── projections: hunks at N, entire file, unified, side by side
                  └── patch emitter: (diff, selection) ──► unified patch
```

One exact answer per file, computed once by gix. Everything a user sees, and the
patch packet 5 will stage with, is a projection of that answer.

## Phase order and why

Strictly sequential, and the order is dictated by what each phase can be tested
against.

1. **01 model** first, because the emitter's shape decides the engine's return
   types. It is pure, so its tests need no repository.
2. **02 engine, commits** proves the model against real git: the round trip is
   only meaningful over diffs the engine produced from real commits.
3. **03 engine, working tree** after 02, because it is the same content query
   with a filtered left or right side, and it carries D1's amendment.
4. **04 worker** once there is something worth carrying, and before any view, so
   the lanes are designed against two real query kinds rather than one.
5. **05-08 UI** in the order the views depend on each other: the pane and the
   file list, then the unified rows those lists open, then the Changes tab and
   side-by-side over the same rows, then expansion in place and compare, which
   need everything before them.
6. **09 QA** as its own fresh session, the merge bar.

## Review dispatch per phase

The packet's one copy of the rules. Every phase ends by orchestrating its own QA:
`/qa` over the phase diff with these reviewers spawned fresh, and `qa-confirm`
(fresh) adjudicating. `qa-checklist` is always on and is not repeated in the
table.

| Phase | Reviewers, beyond `qa-checklist` |
| --- | --- |
| 01 | `test-coverage-auditor` — the emitter's tests are the packet's foundation, and a vacuous one here is invisible until packet 5 |
| 02 | `test-coverage-auditor` |
| 03 | `test-coverage-auditor`; `destructive-ops-reviewer` — this phase makes a read run a user-configured program, and what runs with which environment is its judgement even though no `ops/` file changes |
| 04 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 05 | `responsiveness-reviewer`, `gate-integrity-reviewer` (a new invariant and its guard), `test-coverage-auditor` |
| 06 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 07 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 08 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 09 | all of the above, over the whole packet diff |

## Invariants in play

- **`cairn-model` is plain data on its own allowlist.** The model grows by more
  than any packet has added to it; nothing may follow it in, and every change to
  that crate needs its test in the same commit.
- **The crate seal.** No gix type in a `cairn-git` public signature, no `gix` or
  `cairn_git` named in `cairn-ui`, and the engine never names the toolkit. The
  diff model is what crosses.
- **Only `cairn-git/src/ops/` mutates a repository.** Every query this packet adds
  is a read, including the ones that run a filter driver, and the working-tree
  query must not write a refreshed index. Test code that runs `git apply` into a
  scratch index is fixture work, not a mutation path — confirm the guard's scope
  rather than assuming it.
- **Every `git` subprocess runs with an environment Cairn built.** This packet
  adds none. It does add a process Cairn causes without building its environment:
  the user's clean filter driver, run by gix. That residual is stated in D1's
  amendment and in `CLAUDE.md`, and is phase 03's to write.
- **The UI thread never waits on repository work.** The lanes and the diff thread
  live under `crates/cairn-app/src/worker/`; nothing outside it may name the
  engine or a waiting primitive.
- **No unbounded list renders without virtualization.** Three new lists arrive —
  changed files, unified rows, side-by-side rows. The exceptions roster stays
  empty; a plain `ScrollView` anywhere on a render path is a user decision, not a
  phase's.
- **No `unsafe`, no `unwrap`, no `expect` in shipping code.** A diff view reads
  arbitrary bytes from arbitrary repositories, which is where a lazy `expect`
  becomes a crash report.

## New enforcement this packet must leave behind

1. **No component names a literal modifier** (PRD R8.3, brainstorm L14). Twin: a
   guard over `crates/cairn-ui` and `crates/cairn-app` render files for the
   toolkit's modifier spellings, with a matcher self-test. Lands in phase 05 with
   the accelerator table, in the same commit as the `CLAUDE.md` invariant.
2. **One viewport of diff rows however long the file** (PRD C9). Twin: a headless
   `freya-testing` test over 1,000 and 100,000-line files, at the top and scrolled
   deep, unified in phase 06 and side-by-side in phase 07 — the behavioural twin
   of `only_a_viewport_of_rows_is_built_however_long_the_history`.

Both go into `CLAUDE.md`'s Invariants with their guard names, in the same commit
as the guard, and the residual each cannot express is stated there rather than
implied.

## Docs obligations by phase

Living docs are updated in the change that makes them true or false, never later.

| Phase | Doc |
| --- | --- |
| 01 | create `docs/systems/diff.md` describing the model as built; add its row to `docs/systems/README.md`; update the `cairn-model` row in the root `CLAUDE.md` repo map |
| 02-03 | extend `docs/systems/diff.md` with the engine as built; phase 03 amends D1's paragraph in the root `CLAUDE.md` to match `docs/design/engine.md` |
| 04 | update `docs/systems/history-graph.md` where it describes one epoch counter and one worker, and the root `CLAUDE.md` architecture bullet (`docs/design/concurrency.md` is intent and already states the lanes) |
| 05-08 | extend `docs/systems/diff.md` with the pane and the view; update the root `CLAUDE.md` status paragraph when the application draws a diff |
| 09 | verify all of the above against the code, stamp the PRD, tear the packet down |

## Technical notes

Every gix symbol named here was verified against the vendored 0.87.1 source in
`docs/research/diff-engine/gix-diff-api.md`. Re-read that source before writing
against it: the root `CLAUDE.md` version-sensitive API rule applies to gix and
Freya both, and a memory-coded API is a review finding.

- **Trees:** `Tree::changes()?.for_each_to_obtain_tree_with_cache(..)` with a
  reused blob platform, rather than `diff_tree_to_tree`, which builds a cache per
  call. Changes do not arrive in path order, and rewrites arrive after the walk,
  so the answer is sorted before it is returned. `gix::diff::Options` reads
  `diff.renames` and `diff.renameLimit`; the rewrites outcome reports how many
  similarity checks the limit skipped, which is what R2.2 surfaces.
- **Content:** `Repository::diff_resource_cache(Mode::ToGit, WorktreeRoots { .. })`,
  `set_resource` per side, and `prepare_diff`; keep
  `skip_internal_diff_if_external_is_configured` false so a configured external
  diff program is never spawned. The outcome's own interner strips terminators,
  so tokenise with `sources::byte_lines` instead and keep them.
- **Diff:** `Diff::compute` then `postprocess_lines` (git's indent heuristic), and
  `Diff::hunks()` for changed ranges in line indices. `InternedInput::update_before`
  and `update_after` take any hashable token, which is how the whitespace-ignoring
  pass and the intra-line word pass reuse the same machinery.
- **Worker:** the lanes replace `Request::is_query` and the single `Epochs`
  counter in `crates/cairn-app/src/worker/`. `serve`'s match over requests is
  total and has no wildcard arm: the three non-history arms end in `continue`, the
  two history arms evaluate to a row count, and the paging code after the match is
  therefore the history path. A second query kind has to undo that shape
  deliberately rather than slip past it.
- **Freya:** `VirtualScrollView` with a fixed item size, as `HistoryList` uses it;
  `paragraph().highlights(..)` for intra-line ranges, one colour per paragraph;
  `ResizableContainer` for the splitter; `LaunchConfig::with_font` for Plex Mono.
  There is no plain two-tab strip, but there are three candidates to weigh before
  hand-rolling one: `SegmentedButton`, `FloatingTab`, and the fork's docking
  module (`DockingArea`, `DockPanel`, `TabBarContext`), which is probably more
  machinery than a two-tab pane wants. Verify all of this in the **fork** checkout
  under `~/.cargo/git/checkouts/freya-*/`, at the pinned rev: the registry's
  `freya-0.5.0-rc.6` is a different version and is not what links.
