# State — credential-prompts

The cross-session cheat sheet. Every session updates this before ending.

**Status: phase 01 landed on `feature/credential-prompts`; phases 02-04 not
started.** The `git` subprocess backend exists and the application checks for
git 2.30 at startup. No askpass helper and no fetch yet.

## Locked decisions

L1-L11 in `brainstorm.md`; the design-level frame is D1 and D2 in
`docs/design/cairn.md`. The ones that most constrain implementation:

- Cairn implements no authentication and stores no credential (L1).
- The askpass helper is a separate binary because git's contract is a process:
  prompt on `argv`, secret on stdout (L2).
- A user whose credential helper or ssh-agent already works must see no new
  dialog (L7) — and B4 is the regression test that protects it.
- Minimum git is **2.30** (L9). Raising it is a support-policy change and the
  user's call, not a phase's.
- The channel protects against other users, NOT against same-user processes, and
  says so in its own docs (L10). Do not widen that claim.
- `zeroize` is an accepted dependency for the secret type (L11), added in phase 02
  alongside its allowlist row.

## Open questions

O4 and O5 in `brainstorm.md`; O1-O3 were closed by L9-L11, and phase 01
implemented O1's answer as `GitVersion::MINIMUM` (2.30.0), pinned by
`crates/cairn-git/tests/git_binary.rs` (discovery, through the public API) and
`ops/cli.rs`'s stub tests (what a found `git` is then handed). O4 (askpass vs. credential-helper
precedence) is the one that can invalidate L7, so phase 03 must settle it with a
test rather than a reading.

## New modules and interfaces introduced so far

As phases land, record here: the type or function, its crate, and the one-line
contract.

| Symbol | Crate | Contract |
| --- | --- | --- |
| `ops::GitEnvironment` | `cairn-git` | The environment every `git` subprocess runs with. One constructor, `new(parent)`, which asks `parent` for a spelled-out inherited roster and applies the `ALWAYS` table (`GIT_TERMINAL_PROMPT=0`). Its `command()` is the only place a `std::process::Command` is built; it calls `env_clear()` first. Phase 02 adds the askpass variables to `ALWAYS`. |
| `ops::GitCommand` (crate-private) | `cairn-git` | One invocation: `arg`/`args`/`in_repository`, then `run()` with stdin closed. Non-zero exit is `Error::GitFailed` carrying arguments, `ExitStatus` and stderr. Crate-private so nothing outside `ops` can run a raw verb around the confirmation seal; the public surface is named operations. |
| `ops::Output` (crate-private) | `cairn-git` | What a successful invocation wrote: `stdout()` bytes, `stdout_text()`, `stderr()`, and `records()` for `-z` output. |
| `ops::GitBinary` | `cairn-git` | A found and version-checked `git`. `discover()` reads this process's PATH; `discover_with(environment)` searches the environment's own PATH entry (tests and the worker use it). `command()` (crate-private) starts an invocation. |
| `ops::GitVersion` | `cairn-git` | `major.minor.patch`, `parse()` over `git --version` output, `MINIMUM` = 2.30.0 (L9). |
| `ops::Invalidated` | `cairn-git` | What a mutation left stale in a gix handle (`refs`, `index`, `objects`, `working_tree`); every `Performed` carries one. The contract for each flag is in the `ops` module docs, and names the worker in `cairn-app` as where it is to be honoured — nothing there reads it yet, because no operation reaches the worker yet; fetch (phase 03) is the first. |
| `ops::Performed` | `cairn-git` | The record of a mutation: private fields, read through `description()`, `acknowledged()`, `invalidated()`; built by `Performed::new` or, for a destructive one, `Performed::destructive(.., &Confirmed, ..)`, so an acknowledged prompt can only come from a token. |
| `Error::{GitNotFound, GitTooOld, GitVersionUnreadable, GitNotStarted, GitFailed}` | `cairn-git` | The backend's failures, each naming what the caller must handle; the first three name the required version in their message. |
| `worker::open` / `open_with` (startup check) | `cairn-app` | The worker thread runs `GitBinary::discover_with` before opening the repository and reports a refusal as `Update::Failed`, so the window shows the required version; `open_with(path, GitEnvironment)` is the seam a test hands an environment through, since nothing may set this process's variables. |

Guard added: `every_git_invocation_disables_the_terminal_prompt` with matcher
self-test `the_process_environment_matcher_catches_the_shapes_it_claims`
(`crates/cairn-guards/tests/invariants.rs`); invariant entry in the root
`CLAUDE.md`. Matchers `constructs_process_command`,
`configures_process_environment`, `constructs_struct`,
`constructs_named_struct` and `implements_type` live in
`crates/cairn-guards/src/lib.rs`.

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 git backend | landed | `scripts/gate.sh` PASS (full) | run; see progress.md for the adjudication |
| 02 askpass helper | not started | — | — |
| 03 fetch end to end | not started | — | — |
| 04 QA | not started | — | — |

## Environment notes

- Development machine has git 2.55.0. The evidence record was gathered against
  it; O1 sets the actual required floor, which will be lower.
- `freya` 0.5-rc and `gix` 0.87.1 are pre-1.0. Verify APIs against the vendored
  source under `~/.cargo/registry/src/`, never from memory.
- `scripts/gate.sh` is the bar. Never an ad-hoc `&&` chain, never piped through
  `tail`.
- Commit explicit paths, never `git add -A`.
- **Test fixtures must generate their credentials, never contain them.** This
  packet is the one most likely to commit a secret by accident.
- A new crate, dependency or invariant needs its row in
  `crates/cairn-guards/tests/invariants.rs` in the same commit, or the gate fails.
- The repository has a remote (`origin`, github.com:alexparlett/cairn); the
  integration branch `feature/credential-prompts` tracks it. Packet mode: phases
  commit directly to the integration branch, and the user merges every PR.
- The stub-`git` tests (`crates/cairn-git/tests/git_binary.rs` and
  `crates/cairn-git/src/ops/stub_git.rs`) retry on `ETXTBSY`: a fork in a
  parallel test inherits a still-open write descriptor to a stub for
  microseconds. A property of the harness, absorbed in the test. The stub helper
  exists twice because the runner is crate-private and an integration test
  cannot reach it.
- The runner's `pub(crate)` methods carry
  `#[cfg_attr(not(test), expect(dead_code, ..))]` until fetch lands: the
  `expect` fails the build the moment a caller appears, so the attribute cannot
  outlive its reason.
- `std::env::set_var` is `unsafe` in the 2024 edition and `unsafe` is forbidden,
  so nothing can set a variable in-process for a test: `GitEnvironment::new`
  takes a lookup closure instead, and that is also why there is one constructor
  rather than a test-only second one.
