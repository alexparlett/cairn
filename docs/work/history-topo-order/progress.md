# Progress — history-topo-order

Running log, newest first. Each phase's dismissed QA findings are logged here with their
reasons, for the final QA phase to audit.

## 2026-10-18 — phase 01 built (packet mode)

Built on `feature/history-topo-order`: `HistoryOrder::DateOrder` (git's incremental date-order
walk, `crates/cairn-git/src/history/date_order.rs`), stashes as lazily decided synthetic tips,
cancellation and the 100 ms count; and, for every walk, the shallow file, `info/grafts` and
`refs/replace` read as git reads them, the commit-graph only where git would (`walk::commit_graph`).
Bench: the walk equals git's `--date-order` row for row; first page from every ref 1.19-1.20 s
against git's 1.25-1.26 s without a commit-graph, 95 ms against 82-84 ms with one; 170 B per
commit (`docs/research/history-topo-order/measured.md`). Tie-break swap and six other mutations
fail the tests.

QA: `qa-checklist`, `test-coverage-auditor`, `gate-integrity-reviewer` and
`destructive-ops-reviewer` (dispatched because the amend's pushed check reads the walk), each
spawned fresh over `5da3291...9ff096c`; findings adjudicated by a fresh `qa-confirm`.

Confirmed and fixed:
- A replace ref whose replacement is missing read as a missing object, so a hiding walk could
  answer "not pushed" where git dies: now an error (`Grafted::try_find`), pinned by
  `a_replacement_that_is_not_there_fails_the_walk` (git itself checked to fail there too).
- `Replacements::read` departed from `register_replace_ref`: a symbolic replace ref is now
  resolved, a name's tail read for its first id only, an unreadable ref and two refs for one
  object refused, as git refuses them.
- Coverage: `core.useReplaceRefs=false`; Show Lost Commits' reflog dates and graph eligibility
  under substitutions; each substitution fixture with and without a commit-graph; the shallow
  file beating a graft; commit details with graft-file parents; the commit-time and graph-order
  routes and ahead/behind against git under each substitution
  (`every_route_sees_the_substitutions_git_sees`); `REPORT_EVERY` pinned to 100 ms and the
  public open's cadence (`the_public_walk_reports_its_count_every_100_ms`). Each new test was
  checked to fail under the mutation it names.
- `CLAUDE.md` and `docs/qa-gate.md` now say `git-floor` runs the date-order tests.
- The oracle now grafts only stashes whose commit no ref reaches, with a fixture where a branch is
  built on a stash (see the question below). The floor for `--test date_order` is 6 (7 tests).

Dismissed:
- GATE-2 (the inline unit tests run only at the host git): no criterion asks for them at the floor;
  the floor row is `tests/date_order.rs`, which `git-floor` runs at both floors.
- QA-5 (`date_order.rs` near 1,300 lines, no ratchet): no rule requires a ratchet; about 735 lines
  are production code, the rest its tests.

Escalated, PENDING the user's review (not approved):
- The amend's pushed check follows git's replaced and grafted view, as `git branch -r --contains`
  does, not the objects the remote holds: a local replace ref or graft cutting a remote's history
  short of `HEAD` makes a published commit read as unpublished, and an amend git logs then runs
  without asking. Kept, matching git, and stated in `docs/systems/staging.md`.
- The Commit tab ignores replace refs (gitoxide 0.87.1 reads `core.useReplaceRefs` inverted and
  applies none): a git-parity divergence older than this phase, now visible beside a graph that
  applies them. Stated in `docs/systems/diff.md`; to be filed as an issue or fixed, the user's
  call.

Decisions batched for the user, PENDING review (not approved):
- `gix_revwalk::Graph` is not the commit store: it borrows its commit-graph for its own lifetime,
  so a walk held across pages that opens its own graph could not keep one. The walk reads into
  gix_revwalk's `IdMap` of `graph::Commit`s itself.
- A stash a ref reaches (a branch built on it) is that commit's row with its real parents, index
  commit included, where the oracle's global `git replace --graft` would show one parent. This is
  what the user's own `git log --date-order --branches --remotes --tags HEAD` shows, with or
  without the stash given on stdin; the oracle now follows git there.
- `GIT_GRAFT_FILE`, `GIT_NO_REPLACE_OBJECTS` and `GIT_REPLACE_REF_BASE` in Cairn's environment are
  not honoured.
- A replace-ref or graft-file read failure surfaces as `Error::Walk`.
- Not filed or closed, for the user: the gitoxide replace-refs issue; #128, whose rule this phase
  applies at every graph read it names (left open).

## 2026-10-17 — planned

Planned with `/feature-plan`, on the bug `fork-alignment`'s planning found: with tied commit
times Cairn draws a parent above its child.

Evidence in `docs/research/history-topo-order/`:
- git's order and algorithm, cited from git's source at v2.30.9 and v2.56.0, which the user
  approved fetching;
- gitoxide's walks;
- Cairn's walk as built.

The user locked L1-L7 (`brainstorm.md`); L8 records two corrections a coverage audit found:
- date order;
- Cairn's own walk;
- accept the opening pass, visibly;
- a history thread;
- the oracle;
- the git source fetched for citations;
- a reopen keeping the old rows.

`docs/design/history-graph.md` and `concurrency.md` were rewritten to the end state.
