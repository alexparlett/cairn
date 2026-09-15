# Cairn

Cairn is a native git client for Linux (and, where the toolkit allows, macOS),
aiming at what Fork and Sourcetree do well — a readable history graph, a diff you
can stage by hunk, and destructive operations that tell you what they will cost
before they cost it — without the Electron tax. The load-bearing bet is a hard
seam: **the UI describes what it wants, never how a repository is read, and every
repository access is a value-returning call the view layer cannot make itself.**
Testable form: `cairn-ui` compiles with neither `gix` nor `cairn-git` in its
dependency graph, and nothing outside `cairn-git::ops` can mutate a repository.

Status today: the workspace, the seam, the gate and the guard suite exist and are
green. The first repository read exists — `cairn-git`'s bounded, resumable history
query, feeding the lane assigner in `cairn-model` — and it is now wired to the
window through the worker boundary in `crates/cairn-app/src/worker/`, so the
application opens the repository it is run in and lists its commits off the UI
thread. The list is not virtualized and draws no lanes yet, and no command mutates
a repository. Entries marked (planned) below name the canonical home something
WILL have so docs and implementation converge on the same names — never cite one
as if it exists.

## Repo map

| Path | What lives there |
| --- | --- |
| `docs/` | `qa-gate.md` (QA contract), `design/` intent, `prd/` per-packet specs, `systems/` as-built, `work/` in-flight dirs, `research/` evidence (deferred work goes to GitHub issues; `backlog/` is the no-remote fallback) — findings promote research → brainstorm → design/prd → systems (contract: `docs/CLAUDE.md`) |
| `crates/cairn-model/` | The vocabulary crossing the seam: `Oid`, `RefName`, `CommitSummary`, the `Confirmed` token. Plain data, plus the pure layout algorithm that produces some of it (`LaneAssigner`). Depends on nothing — not `gix`, not `freya`, not the other crates. |
| `crates/cairn-git/` | The repository engine: gitoxide-backed reads, and under `src/ops/` every write (planned — only the confirmation-seal placeholder exists today), most of them delegating to the `git` binary per design decision D1. Speaks `cairn-model` types at its boundary; `gix` types never appear in a public signature. Must never depend on `freya` or `cairn-ui`. |
| `crates/cairn-ui/` | Freya components. Render `cairn-model` values, report intent through `EventHandler` props. Must never depend on `gix` or `cairn-git`, and must never touch the filesystem. |
| `crates/cairn-app/` | The binary. Owns the window, the worker threads, and the wiring between engine and UI — the only crate where the two layers meet. |
| `crates/cairn-guards/` | Test-only. The deterministic enforcement twins for the Invariants below; nothing depends on it. |
| `scripts/`, `.githooks/`, `.github/` | The enforcement layer (contract: `docs/qa-gate.md`). |

Directories with their own CLAUDE.md carry local conventions; read it when you work
there.

## Commands

- `scripts/gate.sh` — the pre-merge gate: format, lint, typecheck, guards,
  dependency policy, full test suite. Exit-code safe; run it before calling a
  change done instead of an ad-hoc `&&` chain (piping test output through
  `tail`/`head` masks the exit code).
- `scripts/gate.sh --fast` — day-loop subset. Never the merge bar; deliberately
  skips network-dependent checks so the day loop stays usable offline.
- `scripts/gate.sh --step <name>` — one named gate component. CI uses this
  interface so CI and local runs share the same command implementation.
- `cargo run -p cairn-app` — run the app. `cargo run -p cairn-app --release` for
  anything where frame time or a large repository is the point; the dev profile
  builds dependencies at `opt-level = 3` but Cairn's own crates at 1.
- Toolchain is pinned in `rust-toolchain.toml`; `cargo deny` is the one tool the
  gate needs that rustup does not ship (`cargo install cargo-deny --locked`).

**Version-sensitive API rule:** for any fast-moving dependency, verify APIs you are
not certain of against current docs before writing them. Never code such an API from
memory, and flag memory-coded usage in review. Here that means **`freya` (0.5 is a
release candidate on a builder API that replaced the old `rsx!` macro — anything
you remember about Freya from `rsx!` examples is wrong) and `gix` (pre-1.0, and its
feature flags gate whole modules: `default-features = false` silently produced an
empty `gix_hash::Kind` once already)**. Read the vendored source under
`~/.cargo/registry/src/` when the docs are thin; it is the version actually linked.

## Default task workflow

- Branch per standalone task: `feature/<slug>` or `fix/<slug>` off `main`; keep
  `main` green. Packet phases follow the mode-specific flow below instead.
- Feature packets get ONE long-lived integration branch, `feature/<packet-slug>`,
  created in its own worktree when the packet's first implementation phase starts.
  **User-mode branch rule** (the default when the user starts one phase): create a
  runtime-owned phase branch from `feature/<packet-slug>` before editing, then raise
  a templated pull request back into that integration branch; never merge it.
  **Packet-mode exception:** only an explicitly declared packet coordinator (the
  `/orchestrate-packet` skill) and the phase agents it dispatches may commit phase
  work directly to `feature/<packet-slug>`; packet mode raises no per-phase PRs.
  **EVERY pull request — phase PRs into integration and packet or planning-doc PRs
  into `main` — is merged by the USER, never by an agent**, after a human has read
  the code. `.claude/settings.json` denies the common spellings of merging and
  pushing to `main`, but it binds Claude Code sessions only and cannot enumerate
  every route: treat the rule as the authority and the deny list as a backstop.
  Packet planning docs land on `main` via their own PR, so every session shares the
  current plan.
- For parallel or long-running tasks, use a separate git worktree per task so
  sessions cannot trample each other; a packet's branch lives in its own worktree.
- Read the relevant local CLAUDE.md and existing implementation before modifying.
- Make ALL changes the objective needs: code, data, tests, docs. No unrelated
  refactors. A change to `cairn-model`, to anything under `cairn-git/src/ops/`, or
  to `cairn-guards` requires a test in the same commit — those are the seam, the
  destructive surface, and the enforcement layer, and each is a place where a
  silent regression is expensive and invisible.
- Big or multi-session work goes through a feature packet: run `/feature-plan` to
  design it with the user first.
- Done means: `scripts/gate.sh` passes locally and `/qa` has reviewed the diff.

## Architecture (the load-bearing ideas)

- **The engine is authoritative; the view is a projection.** `cairn-git` answers
  questions and performs operations; `cairn-ui` renders answers. A component that
  needs new information asks for a new `cairn-model` type and a new engine call —
  it never reaches for a repository, a path, or a subprocess. Enforced by the
  dependency allowlist and the crate-seal guard.
- **`cairn-model` is the whole contract between them, and it is plain data.**
  Neither side may leak its own vocabulary across: no `gix::ObjectId` in a
  component, no `freya` type in the engine. That is what keeps the backend
  replaceable — the decision to bet on gitoxide is reversible exactly as long as
  this holds.
- **Reads go through gitoxide; writes go through the `git` binary.** Decision D1
  in `docs/design/cairn.md`: a mutation must run the user's hooks, filters and
  credential helpers and honour their config, and gix runs none of them. Reads
  never spawn a process — that is the whole reason the split pays. Consequence
  for free: Cairn stores no credentials, because git's helpers do (D2).
- **Every repository mutation lives in `cairn-git::ops`, and the destructive ones
  are sealed behind `cairn_model::Confirmed`.** The token's only constructor
  records the prompt text the user acknowledged, so a code path cannot reach a
  force push or a hard reset without having put words in front of a human — and
  the operation log can quote them afterwards.
- **The UI thread is never allowed to wait on a repository.** `cairn-git` is
  synchronous and knows nothing about threads; `cairn-app` decides where the
  blocking work runs and hands results back as values (decision D3: one
  `cairn_git::SharedRepository` — gitoxide's `ThreadSafeRepository` — per
  repository, a worker taking its thread-local handle once, every request
  carrying an epoch so a superseded query is abandoned rather than rendered). The
  epoch IS the cancel signal the engine polls, so superseding a query stops its
  walk rather than discarding its answer. A repository is somebody's 10-year
  monorepo: any design that assumes a query is fast is wrong.
- **A scroll keeps its walk open.** gitoxide's walk cannot be resumed from a
  value, so a cursor resumes by replaying — which makes page *k* cost `k x limit`
  and does not reach the sizes the history view promises. `cairn-git` therefore
  offers a `HistorySession` that holds the walk for the life of a scroll, making
  paging O(limit); it borrows the repository and is not `Send`, so it lives on
  the worker that owns that handle and never crosses a thread. The cursor
  remains, as the cold-restart path.

## Invariants, YOU MUST keep these

Meta-invariants (keep these; they are what makes the rest durable):

- Every invariant in any CLAUDE.md gets a deterministic enforcement twin (guard
  test, ratchet, hook, or gate step) in the same change that introduces it. A rule
  without a guard is a suggestion.
- Enforcement lives at the STRONGEST tier that can express it — type-level/
  construction seal, then compiler/linter config, then a guard check, then a gate
  step — one authority per invariant. Residual gaps a check cannot express are
  STATED as review obligations, never left implied.
- Every dependency addition is a decision to surface to the user, not a default
  move; dependency policy is a blocking gate step (`gate.sh --step deps`), and
  `deny.toml` records the reason for every exception.
- Never commit secrets or `.env`; never hand-edit generated files (if one is
  introduced, add a PreToolUse deny hook in the same change, see
  `.claude/hooks/README.md`).

Project invariants:

- **Each crate depends only on its allowlist.** Twin: `layer_dependencies_are_allowlisted`
  in `crates/cairn-guards/tests/invariants.rs`. A crate with no row there fails,
  so adding a layer cannot happen by accident.
- **`cairn-ui` and `cairn-model` never name `gix` or `cairn_git`; `cairn-git`
  never names `freya` or `cairn_ui`.** Manifests alone would miss a re-export, so
  the twin reads source: `layers_never_name_the_crates_they_are_sealed_from`,
  matching aliased imports and qualified paths, with the debris hook echoing the
  same rule in milliseconds.
- **Only `cairn-git/src/ops/` mutates a repository**, whether through gitoxide or
  a `git` subprocess. Twin: `only_the_ops_module_mutates_a_repository`.
- **Destructive operations take `cairn_model::Confirmed` by value, and the token
  carries the prompt the user saw.** Primary enforcement is the type: the field is
  private and there is exactly one constructor. Twin against erosion:
  `destructive_operations_are_sealed_behind_the_confirmation_token`. Residual
  review obligation the type cannot express — whether the prompt text is *honest*
  about the consequence — belongs to the `destructive-ops-reviewer`.
- **No `unsafe`, anywhere.** Twin: `unsafe_code = "forbid"` in the workspace lint
  table (compiler tier, so it cannot be locally overridden).
- **Shipping code never panics on a path a user can reach**: `unwrap`, `expect`,
  `todo!`, `unimplemented!` and `dbg!` are denied by the workspace clippy table,
  with tests exempted via `clippy.toml`. A panic in a git client can cost someone
  a working tree.
- **CI runs every merge-bar gate step.** Twin: `ci_runs_every_merge_bar_gate_step`
  compares `gate.sh`'s dispatch arms against the workflow, so a step added locally
  cannot quietly skip CI.
- **The UI thread never waits on repository work.** `cairn-app` is partitioned by
  FILE: `crates/cairn-app/src/worker/` runs repository work and may block; every
  other file in the crate renders, and may name neither `cairn_git` nor any
  waiting primitive — the types (`Receiver`, `Mutex`, `Condvar`, `JoinHandle`),
  the channel constructors (`channel`, `unbounded`, ...), and the nullary waiting
  calls (`recv()`, `join()`, `lock()`, `wait()`), plus `sleep`, `park`,
  `block_on`. Naming the constructor is what catches a receiver held by
  inference. Twin: `the_ui_thread_never_waits_on_repository_work`, matching
  aliased imports and calls whose parentheses wrapped, ignoring string literals,
  and asserting a nonzero file count per directory on BOTH sides.

  **Residual obligations the guard structurally cannot express** — stated here
  rather than implied, and owned by `responsiveness-reviewer`: a file partition
  cannot decide which THREAD a function runs on, so the handful of `worker/`
  functions the UI thread itself calls (`RepositoryHandle::submit`,
  `Updates::next`, `Wake::poll`, all in `crates/cairn-app/src/worker/`) are
  exempt from the matcher while running on the UI thread, and that they never
  block is a review judgement. (The spinning spellings — `try_recv`, `try_iter`,
  `try_lock`, `spin_loop`, `yield_now` — ARE on the roster, so a busy poll loop
  on a render path is caught; one written inside `worker/` is not.) Also the
  reviewer's: whether a page is small enough that the work between yields is
  short, and whether a list is virtualized.

Not yet mechanically pinned — state these when they come up, and add the twin with
the change that makes them load-bearing:

- No unbounded list renders without virtualization.

## Conventions

- Rust 2024 edition, toolchain pinned in `rust-toolchain.toml`. `cargo fmt` with
  the repo `rustfmt.toml` (100 columns). Clippy at `-D warnings` over
  `--workspace --all-targets --all-features`; the workspace lint table in the root
  `Cargo.toml` is the single place lint levels are set.
- Tiny dependency set. Adding a dependency is a user decision.
- Conventional Commits with a scope AND a body (1-4 sentences of why, not what).
  Scopes track the crates: `model`, `git`, `ui`, `app`, `guards`, `gate`, `docs`.
- Name modules for behavior, never for layer: no `helpers`, no `utils`, no `misc`.
  `cairn-git/src/repository.rs`, not `cairn-git/src/core.rs`.
- Errors are `thiserror` enums whose variants name what the CALLER must handle;
  never re-export a dependency's error type across the seam.
- Keyboard shortcuts resolve through one accelerator table mapping a logical
  action to a per-platform chord. Never a literal `Ctrl` inside a component — it
  is the cheap half of keeping macOS reachable (decision D5).
- Docs follow the anchor rule: cite stable paths, exported symbols, and pinned
  tests; never literal counts or line numbers that rot.

## Modularity

Module-first is the default for ALL new code. The deciding question: does this code
need the coordinator's private mutable state? If no, it is a sibling module, every
time. Entry-point files are firewalls that assemble modules, not homes.

- Extract on the rule of three, not before. Never abstract for one use or a
  hypothetical future need.
- Data-as-code is exempt from size pressure: content tables are correctly big.
- Fix bugs test-first: reproduce with a failing test on the real code path, then
  the smallest change that turns it green. Detail: the `extract-and-test` skill.
- Optional but recommended once files grow: a file line-count ratchet — every
  walked file gets a pinned ceiling, growth past it fails, extraction lowers the
  ceiling in the same change, and a raise is a reviewed user decision.

## Testing & verification

Layers, cheapest boundary first (full contract: `docs/qa-gate.md`):

1. Stop hook: instant debris scan every turn (`.claude/hooks/qa-stop.sh`).
2. `.githooks/pre-push`: format check, `cargo check`, guard suite.
3. `scripts/gate.sh --fast` while iterating; `scripts/gate.sh` is the merge bar.
4. CI (`.github/workflows/ci.yml`): the same checks as named `scripts/gate.sh
   --step` invocations on every PR and push to `main`.
5. `/qa` at end of contribution: dispatches the `qa-checklist` agent plus the
   domain reviewers matching the diff surface. Spawn reviewers FRESH; never have
   the implementer review its own work.

Engine tests run against real repositories, not mocks: `cairn-git` opens the Cairn
checkout itself in its unit tests, and fixture repositories are built by running
real git operations. A fake object database proves nothing about gitoxide.
Component tests use `freya-testing`'s headless runner (planned; no component test
exists yet).

## Working style by model capability

- Baseline tier: small verifiable steps, checkpoint with the user, one
  investigation subagent at a time.
- Frontier tier: work autonomously end to end; front-load the spec; fan out
  parallel subagents across independent files or subsystems (these models
  under-spawn by default); before declaring done, have a FRESH subagent review the
  diff for COVERAGE (report every gap with confidence and severity), not filtering.
- State rule scope literally: models follow instructions literally and will not
  generalize a rule across cases unless told; say "every" or "all" when you mean it.
- Never gate the Invariants, safety, or correctness on which model you are. Anchor
  every autonomous step on a check you can actually run, never on "looks done."

## Pointers

- `docs/design/cairn.md` — the design spine: what Cairn is for, what it is not,
  and the locked decisions D1-D9 that the architecture above implements.
- `docs/design/feature-inventory.md` — the full feature surface, tiered by risk,
  with the out-of-scope list and its reasons. Intent, not as-built.
- `docs/work/daily-loop/roadmap.md` — the build order to the D7 milestone: eight
  packets, two filed and six as briefs.
- `docs/qa-gate.md` — the QA layer contract and reviewer dispatch table.
- `docs/CLAUDE.md` — the docs layer contract (tenses, promotion, teardown).
- `docs/work/<packet>/` — in-flight packet dirs, created by `/feature-plan`, torn
  down when the work merges.
- `docs/systems/` — as-built descriptions, written when a system exists (planned;
  empty today, which is accurate).
