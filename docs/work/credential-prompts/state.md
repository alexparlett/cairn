# State — credential-prompts

The cross-session cheat sheet. Every session updates this before ending.

**Status: phases 01 and 02 landed on `feature/credential-prompts`; phases 03-04
not started.** The `git` subprocess backend, the askpass helper, its channel
and the secret type exist and are tested against each other; every `git` Cairn
starts is pointed at the helper. Nothing opens a channel yet and there is no
dialog and no fetch: a prompt today fails closed. As-built description:
`docs/systems/credentials.md`.

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
`ops/cli.rs`'s stub tests (what a found `git` is then handed). Phase 02
implemented O2's answer (L10) as `cairn_askpass::Channel` — the socket, its
modes and the per-operation token, pinned by
`crates/cairn-askpass/tests/helper.rs` — and O3's (L11) as `cairn_model::Secret`
over `zeroize::Zeroizing`. O4 (askpass vs. credential-helper precedence) is
the one that can invalidate L7, so phase 03 must settle it with a test rather
than a reading. O5 (the OpenSSH floor for `SSH_ASKPASS_REQUIRE`) is still phase
03's; the variable is set regardless.

## New modules and interfaces introduced so far

As phases land, record here: the type or function, its crate, and the one-line
contract.

| Symbol | Crate | Contract |
| --- | --- | --- |
| `ops::GitEnvironment` | `cairn-git` | The environment every `git` subprocess runs with. One constructor, `new(parent, &Askpass)`, which asks `parent` for a spelled-out inherited roster, applies the `ALWAYS` table (`GIT_TERMINAL_PROMPT=0`, `SSH_ASKPASS_REQUIRE=force`), and names the helper (`GIT_ASKPASS`, `SSH_ASKPASS`, `CAIRN_ASKPASS_SOCKET` when the `Askpass` has a socket). Its `command(program, token)` is the only place a `std::process::Command` is built; it calls `env_clear()` first and applies `CAIRN_ASKPASS_TOKEN` per invocation. |
| `ops::Askpass` | `cairn-git` | Where git and ssh are sent for a secret: `new(program, Option<socket>)`. `None` is a Cairn with no channel — the helper is still named, so nothing falls back to a tty, and every prompt fails closed. The worker builds one with the helper beside the executable and no socket (phase 03 supplies the socket). |
| `ops::GitCommand` (crate-private) | `cairn-git` | One invocation: `arg`/`args`/`in_repository`/`authorized_by(&AskpassToken)`, then `run()` with stdin closed. Non-zero exit is `Error::GitFailed` carrying arguments, `ExitStatus` and stderr. Crate-private so nothing outside `ops` can run a raw verb around the confirmation seal; the public surface is named operations. |
| `Secret` | `cairn-model` | The one type holding a credential: `new(Vec<u8>)`/`from_string(String)` move the buffer in (no copy), `expose_secret() -> &[u8]` is the one accessor, `Zeroize`/`ZeroizeOnDrop` via a `Zeroizing<Vec<u8>>` field. No `Debug`, `Display`, `Clone`, serialisation, or derive of any kind; `compile_fail` doctests pin that (the full gate runs `cargo test --doc` for them). |
| `AskpassToken`, `SOCKET_VARIABLE`, `TOKEN_VARIABLE`, `HELPER_PROGRAM` | `cairn-model` | The contract between the helper and the environment builder: the per-operation token (`new`, `as_str`, redacted `Debug`), the two variable names, and the helper's file name `cairn-askpass`. |
| `Channel` | `cairn-askpass` | The application's end: `open(&runtime_dir)` creates `cairn-<pid>-<random>/askpass` at `0700`/`0600` and refuses if the modes did not take; `socket_path()`; `begin() -> Operation`; `accept() -> Prompt` (blocking; `Error::Malformed`/`UnknownToken` for a connection it will not serve). Removes socket and directory on drop. |
| `Operation` | `cairn-askpass` | One git invocation's token (`token()`); dropping it retires the token. |
| `Prompt` | `cairn-askpass` | One helper's question: `text()`, `answer(self, &Secret)`, `refuse(self)`; a refusal or a drop without an answer retires the operation's token so git's next ask is refused. |
| `ask(socket, &token, prompt) -> Result<Secret, Refusal>` | `cairn-askpass` | The helper's end: checks the socket and its directory are owner-only, connects, asks, waits without a timeout. `Refusal` names paths and causes, never a prompt, token or secret. |
| `cairn-askpass` (binary) | `cairn-askpass` | `argv[1]` prompt, `CAIRN_ASKPASS_SOCKET`/`CAIRN_ASKPASS_TOKEN` from the environment, answer + newline on stdout, exit 0; else nothing on stdout, one line on stderr, exit 1. Links `cairn-model` and `zeroize` only. |
| `ops::Output` (crate-private) | `cairn-git` | What a successful invocation wrote: `stdout()` bytes, `stdout_text()`, `stderr()`, and `records()` for `-z` output. |
| `ops::GitBinary` | `cairn-git` | A found and version-checked `git`. `discover()` reads this process's PATH; `discover_with(environment)` searches the environment's own PATH entry (tests and the worker use it). `command()` (crate-private) starts an invocation. |
| `ops::GitVersion` | `cairn-git` | `major.minor.patch`, `parse()` over `git --version` output, `MINIMUM` = 2.30.0 (L9). |
| `ops::Invalidated` | `cairn-git` | What a mutation left stale in a gix handle (`refs`, `index`, `objects`, `working_tree`); every `Performed` carries one. The contract for each flag is in the `ops` module docs, and names the worker in `cairn-app` as where it is to be honoured — nothing there reads it yet, because no operation reaches the worker yet; fetch (phase 03) is the first. |
| `ops::Performed` | `cairn-git` | The record of a mutation: private fields, read through `description()`, `acknowledged()`, `invalidated()`; built by `Performed::new` or, for a destructive one, `Performed::destructive(.., &Confirmed, ..)`, so an acknowledged prompt can only come from a token. |
| `Error::{GitNotFound, GitTooOld, GitVersionUnreadable, GitNotStarted, GitFailed}` | `cairn-git` | The backend's failures, each naming what the caller must handle; the first three name the required version in their message. |
| `worker::open` / `open_with` (startup check) | `cairn-app` | The worker thread runs `GitBinary::discover_with` before opening the repository and reports a refusal as `Update::Failed`, so the window shows the required version; `open_with(path, GitEnvironment)` is the seam a test hands an environment through, since nothing may set this process's variables. |

Guards added in phase 02: `no_credential_value_is_logged_printed_serialised_or_stored`
with matcher self-test `the_credential_matcher_catches_the_shapes_it_claims`;
matchers `type_declarations`, `types_containing`, `structs_with_a_field_naming`,
`derives_or_implements` and `renders_in_a_macro` in
`crates/cairn-guards/src/lib.rs`; the rosters `SECRET_READERS` (three files) and
`SECRET_HOLDERS` (empty) in `invariants.rs`. The terminal-prompt guard now also
pins `SSH_ASKPASS_REQUIRE` in `ALWAYS` and the askpass literals in the
constructor. `crates/cairn-askpass/src` is on `PRODUCT_SOURCE_DIRS`, and the
crate has a `FORBIDDEN_IDENTS` row that also forbids `tracing` and `log`.

Guard added in phase 01: `every_git_invocation_disables_the_terminal_prompt` with matcher
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
| 02 askpass helper | landed | `scripts/gate.sh` PASS (full, including the `test-doc` step) | run and fixed; see progress.md for the adjudication |
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
- Tests that mutate a committed file to see a guard go red must snapshot the
  file (to the scratchpad) and restore by copy — `git checkout <file>` restores
  the INDEX version and silently discards uncommitted work, which cost phase 02
  a rewrite of `environment.rs` once.
- `cargo test --workspace --all-targets` never runs doctests; the gate's
  `test-full` step runs `cargo test --workspace --doc` as its second command.
  Stable `rustdoc` checks that a `compile_fail` block fails, not WHY, so each
  such block in `secret.rs` differs from a passing twin by exactly one line.
- Clippy's `allow-expect-in-tests` does not cover helper functions in
  `tests/*.rs` that are outside a `#[test]` fn; use `unwrap_or_else(panic!)`.
- The `worker::Request`/`Update` enums derive `Debug`. A variant carrying a
  `Secret` (phase 03's answer path) would fail the credential guard as a
  container deriving `Debug`; phase 03 must carry the secret some other way
  (a dedicated channel, or an enum with no `Debug`) rather than adding a
  variant to those.
- The runner's `pub(crate)` methods carry
  `#[cfg_attr(not(test), expect(dead_code, ..))]` until fetch lands: the
  `expect` fails the build the moment a caller appears, so the attribute cannot
  outlive its reason.
- `std::env::set_var` is `unsafe` in the 2024 edition and `unsafe` is forbidden,
  so nothing can set a variable in-process for a test: `GitEnvironment::new`
  takes a lookup closure instead, and that is also why there is one constructor
  rather than a test-only second one.
