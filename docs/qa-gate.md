<!-- The QA contract for every agent runtime working in this repo. Root CLAUDE.md
summarizes it; this file is the authority. Keep the two consistent in the same
change. -->

# The QA gate

One repository QA contract, layered so every check runs at the cheapest boundary
that can catch its class of defect.

| Layer | What | When | Blocks? |
| --- | --- | --- | --- |
| Instant debris gate | `.claude/hooks/qa-stop.sh`: added-line scan for debug debris, focused/ignored tests, conflict markers, and the crate-layering seal | end of every agent turn | yes |
| Pre-push floor | `.githooks/pre-push`: `cargo fmt --check`, `cargo check`, the guard suite | before every push | yes |
| Day loop | `scripts/gate.sh --fast`: format, lint, guards, fast tests (no network-dependent checks: the day loop must work offline) | while iterating | no |
| **Pre-merge gate** | `scripts/gate.sh`: format, lint, typecheck, guards, dependency policy, full test suite | **before any merge to main; the merge bar** | **yes** |
| CI | `.github/workflows/ci.yml`: each merge-bar check is a named `scripts/gate.sh --step` invocation; after setup, later checks run despite earlier check failures unless the job is cancelled | every PR, every push to main, and every push to a `feature/**` packet integration branch | yes, once branch protection requires the `gate` check |
| Judgment review | `/qa`: qa-checklist agent + domain reviewers + `qa-confirm` adjudication | end of every contribution | advisory |

Design rules, keep these when extending the gate:

- **Fail toward more tests.** Any future test-selection optimization must widen to
  the full suite for changes it cannot reason about, and always-run any test a
  static import graph cannot see. Silent caps must speak: if a gate bounds its
  coverage, it prints what it dropped.
- **Flakes get exactly one sanctioned retry**, matched by an exact failure
  signature, always loud in the log. Blanket retries hide real regressions.
- **Every prose rule gets an enforcement twin** (guard test, ratchet, hook, or
  gate step) in the same change that introduces the rule. When architecture
  changes, update the applicable reviewer and guard in that same change.
- **A green test is not a decisive test, and the difference is where defects
  hide.** Recurring shapes worth checking for: a fixture whose negative case is
  unreachable for an unrelated reason; two fixtures so uniform that the property
  under test holds by construction rather than by the code; a predicate pinned in
  its own module while its CALL SITE is unpinned; a pin reading the same constant
  on both sides, so a coherent retune passes; a control built from a different
  query/path than the one under test; and an enumeration-free pin whose FIXTURE is
  enumeration-shaped, so it silently stops exercising what it claims to cover.
  The obligation: if you cannot state the mutation that fails the test, you have
  not shown it decides anything. Twin: the `test-coverage-auditor`'s contract.
- **Add a new specialist reviewer only when** a concern is large enough to need
  focused judgment AND is not already protected by a deterministic test.

## Review diff scope

Use a user-provided base when one exists. Otherwise review the UNION of committed
task branch changes (`main...HEAD`, when off `main`), staged and unstaged tracked
changes against `HEAD`, and every untracked file. An empty `main...HEAD` diff never
makes a dirty task branch out of scope. Establish this scope once in the
coordinator and pass the same file set and patch to every reviewer.

## Reviewer dispatch table

When a diff matches more than one row, dispatch all matching reviewers in
PARALLEL, each spawned fresh (never the implementer reviewing its own work). To
run the checklist alone, `/qa-checklist` forks the
`qa-checklist` agent in a fresh context and returns its report. The
adversarial-confirm stage dispatches `qa-confirm` fresh with the diff scope and
every raw finding; it adjudicates (confirm/dismiss/escalate), it never re-reviews.

| Diff surface | Reviewer | Deterministic twin |
| --- | --- | --- |
| Anything (end of contribution) | `qa-checklist` | `scripts/gate.sh` |
| Tests added/changed, or behavior changed without tests | `test-coverage-auditor` | the suite itself |
| The enforcement layer itself: guard checks, hooks, anything under `scripts/`, CI workflows, reviewer/skill definitions, `.claude/`, `.githooks/`, `crates/cairn-guards/`, this file | `gate-integrity-reviewer` | direct guard-rule tests where they exist; the rest is this review |
| Docs (any tense) — creation, moves, claims about code or plans | `qa-checklist` (its docs tier: tense discipline per `docs/CLAUDE.md` — intent never stated as built, `systems/` describes only current code, PRDs stamped at teardown, anchors resolve) | review |
| Anything under `crates/cairn-git/src/ops/`, or any new call site that reaches one | `destructive-ops-reviewer` | `destructive_operations_are_sealed_behind_the_confirmation_token` and `only_the_ops_module_mutates_a_repository` pin the seal; whether the prompt is HONEST is the review |
| `crates/cairn-ui/`, `crates/cairn-app/`, or anything that changes what runs per frame or per repository query | `responsiveness-reviewer` | `the_ui_thread_never_waits_on_repository_work` pins which FILES may reach a repository or name a waiting primitive — only `crates/cairn-app/src/worker/` — and the debris hook echoes its engine-reach half. It cannot decide which THREAD a function runs on, so "does this code block the UI thread?" stays the reviewer's question in full, including for the `worker/` functions the UI thread calls; so do a busy poll loop and page size. Virtualization is now PARTLY pinned: `a_history_sized_list_renders_through_a_virtualizing_view` decides which scroll view a render file reaches for (no plain `ScrollView` outside its empty exceptions roster; some file must use `VirtualScrollView` over `HistoryRow`s), and `only_a_viewport_of_rows_is_built_however_long_the_history` renders `HistoryList` headlessly and pins that it builds one viewport of rows, at the top and scrolled deep, at 1,000 and 100,000 rows, which leaves the reviewer what neither can express — whether an iteration is over a history at all, and whether per-frame work grows with scroll depth while the built-row count stays flat |

Future reviewers: name them here as (planned) when you know a surface will need
one, so packets converge on the same name — and add each WITH the packet that
lands its surface, per the specialist rule above. Write it from
`.claude/agents/domain-reviewer.template.md`; the structure is what makes
reviewers comparable and dispatchable.

## Packet phase QA

Inside a feature packet, each implementation phase's QA is orchestrated by the
implementing session at phase end: `/qa` spawns the reviewers fresh per the table
above, and the adversarial-confirm step goes to `qa-confirm`, spawned fresh — the
implementer never adjudicates findings against its own work. Every dismissed
finding is recorded in the packet's `progress.md` with its reason. The packet's
FINAL QA phase is the one standalone fresh session: it reviews the whole packet
diff, runs the full packet checklist, audits the dismissal log (a dismissal whose
reason no longer holds is a finding), and is the merge bar before the packet PR.

## Enforcement-layer parity

`.claude/hooks/qa-stop.sh` restates the crate-layering seal, and the half of the
worker partition that a line scan can express (nothing outside
`crates/cairn-app/src/worker/` names `gix` or `cairn_git`), both of which
`crates/cairn-guards/tests/invariants.rs` owns. The guard suite is the authority;
the hook is a millisecond echo with a coarser matcher, and it deliberately does
NOT try to echo the waiting-primitive half — telling `handle.join()` from
`root.join("crates")` needs the matcher, not awk. Change one, change the other in
the same commit — a `gate-integrity-reviewer` obligation, since no check compares
the two.
