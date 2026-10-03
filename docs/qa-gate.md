<!-- The QA contract for every agent runtime working in this repo. Root CLAUDE.md
summarizes it; this file is the authority. Keep the two consistent in the same
change. -->

# The QA gate

One repository QA contract, layered so every check runs at the cheapest boundary
that can catch its class of defect.

| Layer | What | When | Blocks? |
| --- | --- | --- | --- |
| Instant debris gate | `.claude/hooks/qa-stop.sh`: added-line scan for debug debris, focused/ignored tests, conflict markers, and the crate-layering seal — over uncommitted lines AND lines committed on the branch but not yet in `main`, so committed debris stays in view until it is fixed. Committed lines skip two rules a kept measurement reporter legitimately trips (`eprintln!`, `#[ignore = "reason"]`); a bare `#[ignore]` is flagged in both. Behaviour pinned by `crates/cairn-guards/tests/debris_hook.rs`, which runs the hook against scratch repositories | end of every agent turn | yes |
| Session-link check | `.githooks/commit-msg`: refuses a commit message that links a Claude Code session (the repository is public). Pinned by `crates/cairn-guards/tests/session_link_hook.rs` | every commit | yes |
| Pre-push floor | `.githooks/pre-push`: the session-link check over every outgoing commit's message, `cargo fmt --check`, `cargo check`, the guard suite | before every push | yes |
| Day loop | `scripts/gate.sh --fast`: format, lint, guards, fast tests (no network-dependent checks: the day loop must work offline) | while iterating | no |
| **Pre-merge gate** | `scripts/gate.sh`: format, lint, typecheck, guards, dependency policy, full test suite, doctests (`test-doc`, where the secret type's compile-fail pins live), and `git-floor` — every `--step` but the day loop's `test-fast`, held there by `the_local_full_gate_runs_every_step_but_the_day_loops`. `git-floor` (`scripts/git-floor.sh`) builds git 2.30.9 and 2.32.7 from source by pinned commit into a cache on its first run, which needs the network, a C compiler, make and zlib's headers — a machine without them fails the step, told what to install, and never skips it — and then runs cairn-git's real-git diff tests on each, failing if a filtered run lists fewer tests than its floor and printing every test each run skipped | **before any merge to main; the merge bar** | **yes** |
| CI | `.github/workflows/ci.yml`: each merge-bar check is a named `scripts/gate.sh --step` invocation; after setup, later checks run despite earlier check failures unless the job is cancelled. A second job, `git floor`, runs `--step git-floor` (`scripts/git-floor.sh`): cairn-git's real-git diff tests against the oldest gits Cairn supports — the `GitBinary::MINIMUM` floor and 2.32 — built from source by pinned commit, as the local full gate runs it; a job of its own because it needs none of the gate's graphics stack and `CAIRN_REQUIRE_NO_LAZY_FETCH` must not reach it, which is why both `CAIRN_REQUIRE_*` variables are set in the `gate` job's own `env:` (pinned there by `the_partial_clone_pin_is_required_in_ci` and `the_ssh_criteria_are_required_wherever_they_can_run`). `ci_runs_every_merge_bar_gate_step` holds CI to running every step | every PR, every push to main, and every push to a `feature/**` packet integration branch | yes, once branch protection requires the `gate` check (and `git floor`, for the floor) |
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
| Anything under `crates/cairn-git/src/ops/`, `crates/cairn-git/src/process/` or `crates/cairn-git/src/reads/`, or any new call site that reaches one | `destructive-ops-reviewer` | `destructive_operations_are_sealed_behind_the_confirmation_token` and `only_the_ops_module_mutates_a_repository` (a `git` subprocess, or a name on its gitoxide mutation roster, outside `ops/`) pin the seal; `every_git_invocation_disables_the_terminal_prompt` pins that a `git` process is built in one place, with the explicit environment; `only_the_process_module_builds_or_runs_a_process` that nothing outside `process/` drives one; `the_runner_is_named_only_by_ops_and_reads` that only `ops/` and `reads/` reach the runner and only `ops/` builds a write; `the_retired_runner_is_gone` that the runner `process-manager` replaced stays gone and production `process/` starts a process by one `.spawn()` call (method-call syntax only; what a path call or `nix` start escapes is `qa-checklist`'s, item 7). Whether the prompt is HONEST is the review, and so is whether the environment's inherited roster is right — each entry is a deliberate leak of the user's environment to `git`, and a missing one breaks a credential helper that worked — and whether a read really runs plumbing or `status`, and whether a new read could touch an object a partial clone lacks, which git older than 2.44 lazy-fetches despite `GIT_NO_LAZY_FETCH=1` |
| Anything that names `cairn_model::Secret` or `expose_secret`, anything under `crates/cairn-askpass/`, or a new type that holds a credential | `qa-checklist` (its item 10) | `no_credential_value_is_logged_printed_serialised_or_stored` pins the type's shape, that no container of it derives or hand-implements a rendering trait, that the accessor stays inside the `SECRET_READERS` roster and out of every rendering macro, and that no struct keeps one; `cairn-model`'s compile-fail doctests pin what the compiler refuses. The matchers read spellings, so a `type` alias, a generic wrapper instantiated with the type at a use site, and whether a rendered PROMPT could carry a secret are the review |
| `crates/cairn-ui/`, `crates/cairn-app/`, or anything that changes what runs per frame or per repository query | `responsiveness-reviewer` | `the_ui_thread_never_waits_on_repository_work` pins which FILES may reach a repository or name a waiting primitive — only `crates/cairn-app/src/worker/` — and the debris hook echoes its engine-reach half. It cannot decide which THREAD a function runs on, so "does this code block the UI thread?" stays the reviewer's question in full, including for the `worker/` functions the UI thread calls (`RepositoryHandle::submit` — through `into_submitter` and the window's close hook `Closing::requested` — `worker::open` and its `Replier`, `Updates::next`, `Wake::poll`, `Discovery::start`) and for the close's shape (the hook only asks; `end_invocations` waits on the repository thread; the window closes on the stream's end, or a second request after `worker::CLOSE_PATIENCE`; the stream's end depends on the window refusing a prompt left open, `session::apply`'s `withdraw`; the `main.rs` wiring, which no test drives, including the `Closing::is_requested` flag it passes to `session::apply`); so do a busy poll loop and page size. Virtualization is now PARTLY pinned: `a_history_sized_list_renders_through_a_virtualizing_view` decides which scroll view a render file reaches for (no plain `ScrollView` outside its empty exceptions roster; some file must use `VirtualScrollView` over `HistoryRow`s), and `only_a_viewport_of_rows_is_built_however_long_the_history` renders `HistoryList` headlessly and pins that it builds one viewport of rows, at the top and scrolled deep, at 1,000 and 100,000 rows, which leaves the reviewer what neither can express — whether an iteration is over a history at all, and whether per-frame work grows with scroll depth while the built-row count stays flat |

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

`.claude/hooks/qa-stop.sh` restates the crate-layering seal (including the
`cairn-askpass` row, which also forbids a logging crate), and the half of the
worker partition that a line scan can express (nothing outside
`crates/cairn-app/src/worker/` names `gix` or `cairn_git`), both of which
`crates/cairn-guards/tests/invariants.rs` owns. The guard suite is the authority;
the hook is a millisecond echo with a coarser matcher, and it deliberately does
NOT try to echo the waiting-primitive half — telling `handle.join()` from
`root.join("crates")` needs the matcher, not awk. Change one, change the other in
the same commit — a `gate-integrity-reviewer` obligation, since no check compares
the two.
