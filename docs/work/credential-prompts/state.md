# State — credential-prompts

The cross-session cheat sheet. Every session updates this before ending.

**Status: phases 01, 02 and 03 landed on `feature/credential-prompts`; phase
04 (QA) not started.** Fetch works end to end: the window's button, the
operations thread, git's own progress, a cancel that kills git, the acceptor
thread, the dialog, and the reply back to git — B3, B4 and B5 pinned over
HTTP and SSH against real remotes. As-built description:
`docs/systems/credentials.md`. Push is not built (L8).

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

None. O4 and O5 were settled by test in phase 03 and recorded as addenda to
the evidence record: a configured `credential.helper` answers before
`GIT_ASKPASS` is consulted and the askpass fills only what it left (so L7
holds as designed and the stopping rule did not trigger); `GIT_ASKPASS`
outranks `core.askPass`; `SSH_ASKPASS_REQUIRE` is OpenSSH 8.4 and `force`
routes passphrases and host-key confirmations to the helper whatever the
terminal or `DISPLAY`. Below, as it stood before: O1-O3 were closed by L9-L11, and phase 01
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
| `ops::Invalidated` | `cairn-git` | What a mutation left stale in a gix handle (`refs`, `index`, `objects`, `working_tree`); every `Performed` carries one. The contract for each flag is in the `ops` module docs. Fetch declares `refs` + `objects`; the operations thread compares `Repository::ref_tips` before and after and tells the window `refreshed`, on every outcome, and the window re-asks for the history. |
| `ops::Performed` | `cairn-git` | The record of a mutation: private fields, read through `description()`, `acknowledged()`, `invalidated()`; built by `Performed::new` or, for a destructive one, `Performed::destructive(.., &Confirmed, ..)`, so an acknowledged prompt can only come from a token. |
| `Error::{GitNotFound, GitTooOld, GitVersionUnreadable, GitNotStarted, GitFailed}` | `cairn-git` | The backend's failures, each naming what the caller must handle; the first three name the required version in their message. |
| `Error::{GitCancelled, Refs}` | `cairn-git` | Phase 03: a fetch the user cancelled (killed, reaped, not a failure; a clean exit is never reported as cancelled, whatever the flag says), and the refs being unreadable for `ref_tips`. |
| `ops::fetch(&GitBinary, &Repository, remote, Option<&AskpassToken>) -> FetchInProgress` | `cairn-git` | Starts `git fetch --progress --end-of-options <remote>`. Not destructive; no `Confirmed`, by design. `finish(progress) -> Performed` streams each progress redraw and declares `refs` + `objects` invalid; `canceller() -> FetchCancel` (`Send`, `Clone`) kills git from any thread, after which `finish` is `GitCancelled`. |
| `GitCommand::stream() -> Running` (crate-private) | `cairn-git` | The runner's second form: stdout discarded, stderr read on a thread of its own so a killed git's lingering children cannot hold the wait; `Running::finish`, `Running::killer() -> ProcessKill`. |
| `Repository::remotes() -> Vec<RemoteSummary>` | `cairn-git` | The configured remotes through gitoxide, default first; a remote with a missing or unparsable URL is listed without one; a password embedded in a URL is left out. Cannot fail. |
| `Repository::ref_tips() -> Result<BTreeMap<RefName, Oid>, Error>` | `cairn-git` | Every ref and its id, as the handle sees them now; what the worker compares around a fetch. |
| `RemoteSummary`, `PromptKind`, `prompt_subject` | `cairn-model` | A remote's name and fetch URL; what a prompt asks for (`Username`, `Password`, `Passphrase`, `Confirmation`, `Other`, from git's and OpenSSH's spellings; `ACCEPTED` = `yes`); the first quoted span of a prompt. |
| `CredentialPrompt` | `cairn-ui` | The dialog: `new(remote, text)`, `on_submit(EventHandler<String>)` (the input's buffer, moved), `on_cancel`. Masks all but a username; a confirmation gets buttons, no field. |
| `Request::{ListRemotes, Fetch { remote }, CancelFetch}`, `Request::is_query` | `cairn-app` | Operations carry no epoch (`submit` bumps only for a query). |
| `Update::{Remotes, FetchStarted, FetchProgress { line }, FetchFinished/FetchCancelled/FetchFailed { refreshed, .. }, Prompt { id, text }}` | `cairn-app` | All epochless. `refreshed` on every ending says a ref actually moved (tips compared before and after): the window clears and re-asks for the history only then. `FetchStarted` is sent once the kill handle is installed. |
| `worker::Reply::{Provide { prompt, secret }, Refuse { prompt }}`, `worker::PromptId`, `Replier = Rc<dyn Fn(Reply)>` | `cairn-app` | The window's answer to a prompt, over its own channel; `Reply` derives nothing and is never a struct field. Named `Reply` because the guard reads spellings and the channel's `Error` has a variant `Answer`. |
| `worker::open -> (RepositoryHandle, Updates, Replier)` | `cairn-app` | Now three threads per repository (`cairn-repository`, `cairn-operations`, `cairn-askpass`), each with its own sender; the stream ends when all have gone. `open_with(path, Startup)` is the test seam. `submit(CancelFetch)` reaches the fetch directly through `FetchControl` (arm/cancel/install: one fetch at a time, a cancel before git runs is kept), never the jobs queue. |
| `session::apply(update, View, &Worker { submit, refuse })` | `cairn-app` | The one place an `Update` becomes view state, tested through the headless runner: a fetch ending takes the dialog down and refuses its prompt; `refreshed` clears and re-asks; a prompt with no fetch in flight is refused, not shown. `main.rs` holds the answering end weakly for it. |
| `FetchStatus::Starting` | `cairn-app` | Set by the button press itself, so a second press has no button; `Running` follows `FetchStarted`. |
| `worker::startup::{Startup, Backend}` | `cairn-app` | On the repository thread, before anything: `Channel::open($XDG_RUNTIME_DIR)`, the environment around its socket, `GitBinary::discover_with`. `Backend::prompting: Result<(), String>` names why no prompt can be answered (no runtime dir, helper not built) — fetch still runs; a failure appends the reason. |
| `fetch_state::{FetchStatus, PromptView}`, `window::View` | `cairn-app` | The view state the window is drawn from; `status_text::fetch_line` renders the fetch's sentence. |
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
| 03 fetch end to end | landed | `scripts/gate.sh` PASS (full) | run, adjudicated (33 confirmed, 6 dismissed, 2 escalated) and fixed; see progress.md |
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
- The `worker::Request`/`Update` enums derive `Debug`, so the secret's answer
  path is `worker::Reply` over its own channel (phase 03). The credential
  guard reads SPELLINGS: any struct field naming a type that names `Secret`
  is a holder, transitively, and a type sharing its name with a variant of
  another type is caught too (`Error::Answer` in the channel made an enum
  named `Answer` a container). Name new secret-carrying types distinctly.
- The runner's `Output::stdout`/`records` still carry
  `#[cfg_attr(not(test), expect(dead_code, ..))]`, now reasoned "fetch reads
  nothing from stdout"; the `expect` fails the build once a caller appears.
- Tests in `cairn-app/src` may not name `Command` (the terminal-prompt guard
  scans the crate's `src/`, test modules included), so fixtures there are
  `std::fs` repositories and a loopback `TcpListener`; the tests that need a
  real remote (`git http-backend`, `sshd`) live in `crates/cairn-git/tests/`,
  where `cairn-git` has a TEST_ONLY_ALLOWLIST row for `cairn-askpass`.
- `ssh` reads `~/.ssh/config` from the passwd home directory, never `$HOME`;
  the ssh fixture hands its configuration over through `core.sshCommand`.
- The helper binary is found at `target/<profile>/cairn-askpass`, two
  directories up from a test executable; `cargo test --workspace` and the
  gate build it before any test runs. `cargo test -p cairn-app` alone does
  not (its test names the build command); `cargo test -p cairn-git` builds
  it itself (`tests/remotes/askpass.rs`, with `--release` when the profile is).
- The ssh fixture tests skip (a line on stderr, hidden by cargo's capture on a
  pass) where `sshd` cannot run, unless `CAIRN_REQUIRE_SSH_FIXTURE` is set,
  when they fail instead. CI does not set it yet — whether it should provision
  `sshd` is the user's (phase 03 report).
- The credential guard reads spellings: a struct field naming a type that
  names `Secret` is a holder wherever it is — `session::Worker` learned this
  with a `&Weak<dyn Fn(Reply)>` field and became two plain callbacks.
- `std::env::set_var` is `unsafe` in the 2024 edition and `unsafe` is forbidden,
  so nothing can set a variable in-process for a test: `GitEnvironment::new`
  takes a lookup closure instead, and that is also why there is one constructor
  rather than a test-only second one.
