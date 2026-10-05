# Implementation plan — refs-and-status

## Shape

```
cairn-ui     sidebar ── Local Changes (N) │ All Commits │ filter │ Branches │ Remotes │ Tags │ Stashes
             title bar ── repo* · branch · ↓n ↑m
             history rows ── chips (branch ✓, remote glyph, tag) │ stash rows
             Local Changes ── Unstaged / Staged lists ──► diff view (diff-engine's)
                  │
cairn-app    refresh (focus │ after an operation │ Refresh chord)
                  ▼
             history thread ── refs ──► reopen history if the snapshot moved
                            ── history pages, and a ref's find (one lane)
             refresh thread ── status (git process) ── ahead/behind
             filter lanes   ── sidebar filter
                  │
cairn-git    refs snapshot (gix, four parity rules) ── ahead/behind (two hidden walks)
             reads::status ── git status --porcelain=v2 -z ── parsed
             history ── seeded from the snapshot, stash rows merged by date, labels
                  │
cairn-model  RefsSnapshot, Ref, HeadState, Upstream, Stash, AheadBehind,
             WorkingTreeStatus and its entries, a stash RowContent, row labels
```

The type names above are suggestions for phase 01 and 02 to settle; the PRD
fixes what they carry, not what they are called.

## Phase order and why

1. **01 refs** first: everything after it — the walk's seeds, the labels, the
   sidebar, the toolbar, the refresh's comparison — is a projection of the
   snapshot, so its shape and its parity with git come first.
2. **02 status** next, independent of 01 in code but after it in order so the
   model additions land one at a time; it carries D1's fourth git read and its
   floor tests, which are the packet's riskiest engine work.
3. **03 history from every ref** needs 01's snapshot; it changes `RowContent`, the
   one change whose blast radius the compiler and a guard both police, so it
   lands before any view draws a stash.
4. **04 worker and refresh** once three answers exist to carry, and before any
   view, so the lanes are designed against real queries.
5. **05-07 UI** in dependency order: labels on rows and the toolbar (smallest
   surface over 03 and 04), then the sidebar (which needs the find mechanism and
   the labels' vocabulary), then Local Changes (which needs the sidebar's entry
   and 02's status) and the window check over everything.
6. **08 QA** as its own fresh session, the merge bar.

## Review dispatch per phase

The packet's one copy of the rules. Every phase ends by orchestrating its own
QA: `/qa` over the phase diff with these reviewers spawned fresh, and
`qa-confirm` (fresh) adjudicating. `qa-checklist` is always on and is not
repeated in the table.

| Phase | Reviewers, beyond `qa-checklist` |
| --- | --- |
| 01 | `test-coverage-auditor` — parity tests whose oracle is git, not a golden file; `destructive-ops-reviewer` — open's refusal path changes what Cairn opens, and `ref_tips`' rebuild touches `ops/fetch.rs`' contract; `gate-integrity-reviewer` — C2's `CAIRN_REQUIRE_*` variable, its `scripts/gate.sh` probe and its guard twin |
| 02 | `test-coverage-auditor`; `destructive-ops-reviewer` — a new read in `reads/`, its verb and options, and that it writes nothing (its checks 9 and 10); `gate-integrity-reviewer` if a guard roster or a `CAIRN_REQUIRE_*` probe changes |
| 03 | `test-coverage-auditor`; `responsiveness-reviewer` — the walk's first page from every ref, and a new row kind in a virtualized list |
| 04 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 05 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 06 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 07 | `responsiveness-reviewer`, `test-coverage-auditor` |
| 08 | all of the above, over the whole packet diff |

## Invariants in play

- **`cairn-model` is plain data on its own allowlist**; every change to it needs
  its test in the same commit.
- **The crate seal.** No gix type in a `cairn-git` public signature; `cairn-ui`
  names neither `gix` nor `cairn_git`.
- **Only `cairn-git/src/ops/` mutates a repository.** Every query here is a read.
  gix's `Outcome::write_changes` is on the mutation roster; nothing calls it.
- **Every `git` subprocess runs with an environment Cairn built** and a read adds
  `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1`. `reads::status` is built as a
  read invocation, never anything else. The porcelain-reads guard
  (`the_porcelain_reads_are_the_two_named_queries`) checks only `"diff"` and
  `"config"` literals; `"status"` passes it, which is why
  `destructive-ops-reviewer` reads the verb and every option (issue #54 would
  make that a guard).
- **`RowContent` is read by naming every variant** (outside `cairn-model`). The
  stash variant is the first second production variant; every reader the
  compiler names is fixed by naming the new variant, never by a wildcard.
- **The UI thread never waits on repository work.** New lanes live under
  `crates/cairn-app/src/worker/`; the focus subscription and the refresh request
  are render-side and only submit. A reopen on focus makes `reload_if`'s
  UI-thread free of every loaded row frequent, so phase 04 retires those rows to
  a worker (#52, PRD R11.3).
- **No unbounded list renders without virtualization.** The sidebar's sections
  and Local Changes' two lists use `VirtualScrollView`; the `ScrollView`
  exceptions roster stays empty — a plain `ScrollView` is a user decision.
- **No component names a literal modifier.** The Refresh chord is an `Action` in
  `crates/cairn-ui/src/accelerators.rs`.
- **A test that skips where its host cannot serve it is required wherever the
  host can.** The reftable test (C2) skips on a git below 2.45; it needs a
  `CAIRN_REQUIRE_*` variable, a gate probe, and the twin's roster extended
  (`gate-integrity-reviewer`).
- **No `unsafe`, no `unwrap`, no `expect` in shipping code.** Status parses
  arbitrary bytes from git; a lazy `expect` there is a crash report.

## New enforcement this packet must leave behind

1. **The viewport twins** for each new list: the sidebar (C8, 50,000 refs) and
   Local Changes (C9, 50,000 paths), behavioural twins of
   `only_a_viewport_of_rows_is_built_however_long_the_history`, named in the root
   `CLAUDE.md` virtualization invariant beside the others.
2. **The reftable test's requirement pin** (C2): `CAIRN_REQUIRE_REFTABLE` (name to
   settle), probed in `scripts/gate.sh`'s `test-full` as the test probes, with a
   twin beside `the_fsmonitor_daemon_pin_is_required_wherever_it_can_run`.

Each goes into `CLAUDE.md` with its guard name in the same commit as the guard,
with the residual it cannot express stated.

## Docs obligations by phase

| Phase | Doc |
| --- | --- |
| 01 | create `docs/systems/refs.md` (the snapshot, the parity rules, ahead/behind, reftable refused); row in `docs/systems/README.md`; `docs/systems/git-processes.md` where it lists what open refuses |
| 02 | `docs/systems/git-processes.md` and `docs/systems/diff.md` where they list the reads git answers; create the status section of `docs/systems/refs.md` or a `status.md` (phase's call); the root `CLAUDE.md` D1 paragraph and repo map row for `cairn-git` (`reads::status`) — C13 |
| 03 | `docs/systems/history-graph.md`: seeded from every ref, labels, stash rows, the assigner's non-commit row |
| 04 | `docs/systems/history-graph.md` and `docs/systems/git-processes.md` where they describe `reload_if`; the root `CLAUDE.md` responsiveness residuals for anything new the UI thread calls |
| 05-07 | the sidebar, labels, toolbar and Local Changes in `docs/systems/history-graph.md` or a new `docs/systems/sidebar.md` (phase 06's call); the root `CLAUDE.md` status paragraph and repo map rows; the virtualization invariant's twins |
| 08 | verify all of the above, stamp the PRD, update the roadmap and spine pointers, tear down |

## Technical notes

Verify every gix and Freya API against the vendored source before writing it (the
root `CLAUDE.md` version-sensitive rule); `gix-refs-and-status-api.md` cites the
files, but a phase re-reads them.

- **Refs:** `Repository::references()?` then `local_branches()`,
  `remote_branches()`, `tags()`, `prefixed(..)`; follow symbolic refs by reading
  their target name, never `.peeled()`. Annotated tags: keep the object id, then
  peel. Stash: `find_reference("refs/stash")` → `log_iter()` → `all()` (oldest
  first), reversed — never `.rev()`. Upstream: `branch_remote_ref_name` and
  `branch_remote_tracking_ref_name`, with `remote = .` handled by hand. Worktree
  HEADs: `find_reference("worktrees/<id>/HEAD")` (`checked_out_branches` is
  `pub(crate)`). Retire or rebuild `Repository::ref_tips` on the snapshot (it
  peels tags and drops symbolic targets, `code-audit.md` section 5); the network
  lane is its only caller.
- **Reftable:** read `extensions.refStorage` from the repository's config at open,
  beside the dubious-ownership refusal; any value but `files` refuses.
- **Ahead/behind:** `rev_walk([branch]).with_hidden([upstream])` counted, and the
  reverse, polling the epoch; on the refresh thread with status (PRD R11.2).
- **Threads:** refs on the history thread (cheap; it reopens the walk); status and
  ahead/behind on a third, refresh, thread; a ref's find is a history-lane request
  that pages the held walk, so a scroll and a find supersede each other.
- **Status:** `git status --porcelain=v2 -z` with `--untracked-files=all` (or
  `no`, per L2), no `--ignored`, no `--branch`, nothing overriding renames or
  submodules. Porcelain v2 lines: `1` ordinary, `2` rename/copy (the source is the
  next NUL-separated field under `-z`), `u` unmerged (seven `XY` codes), `?`
  untracked; the submodule field `S<c><m><u>`. Parse bytes, not `str`.
- **History:** `HistoryRequest::from_commits` with the snapshot's commit-bearing
  seeds (tags on trees filtered — `walk_tips` checks only the hash kind). Stash
  rows are merged into the stream by commit time on the history thread, never
  after their base; each stash's base is a seed, so its edge lands; the
  assigner is keyed by `Oid` (`GraphRow`, `LaneAssigner::push`) and a stash has
  one — its own commit id — so the assigner can take it with its first parent
  as its only parent; confirm it does not then await the stash's index commit.
- **Focus:** `Platform::get().is_app_focused` (`freya-core/src/platform.rs` at
  rev `caa46f8`), a `State<bool>` set by `freya-winit`'s `WindowEvent::Focused`;
  `freya-testing` creates it `true`, so a test toggles it.
- **Retired:** `Update::into_retired` holds change sets and shown diffs today; a
  superseded refs snapshot or status of size joins it, and so do a reopen's
  replaced history rows (R11.3, #52).
