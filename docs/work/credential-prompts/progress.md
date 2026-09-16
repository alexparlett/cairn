# Progress — credential-prompts

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-09-16 — phase 01: the git subprocess backend landed

Packet mode, committed directly to `feature/credential-prompts` (three focused
commits: backend, guard, startup check). The environment is built from a
spelled-out roster (`PATH`, `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`,
`SSH_AUTH_SOCK`, `TMPDIR`, `LANG`, `LC_ALL`, `LC_MESSAGES`) plus
`GIT_TERMINAL_PROMPT=0`; the parent is never asked for any other name, which
is how a `GIT_ASKPASS` set for something else stays out (L5). The one
construction path is structural, not just tested: `GitEnvironment::command`
is the only place a `Command` is built.

Decisions taken in-phase, none needing the user:

- **Proxy variables are not inherited.** `http_proxy`/`https_proxy`/`no_proxy`
  would be a deliberate roster addition; the PRD scopes proxy configuration out,
  and git's `http.proxy` config still works. Filed as a follow-up for the user.
- **Locale variables are inherited.** git's stderr is shown to the user
  verbatim, so it should be in their language. Nothing machine-readable that
  Cairn parses depends on locale.
- **The startup check refuses to open the repository** rather than warning and
  carrying on read-only: D1 says fail loudly, and a read-only Cairn that cannot
  say why its write buttons are missing is the silent degradation D1 forbids.
- **`Invalidated` lives in `cairn-git::ops`, not `cairn-model`**: only the
  worker (which already depends on `cairn-git`) reads it, and it describes the
  engine's own caches.

Guards seen red before landing, then reverted:

- `only_the_ops_module_mutates_a_repository`: a `Command::new("git")` appended
  to `crates/cairn-git/src/repository.rs` — failed with
  `crates/cairn-git/src/repository.rs:224 spawns a git subprocess outside
  crates/cairn-git/src/ops`.
- `every_git_invocation_disables_the_terminal_prompt`, four shapes: a
  `command.env("GIT_TERMINAL_PROMPT", "1")` in `cli.rs` (caught as a variable
  set beside the roster); a `std::process::Command::new(path)` in `binary.rs`
  (caught as naming `Command` outside the two backend files); the `ALWAYS`
  entry changed to `"1"` in `environment.rs` (caught as the table no longer
  carrying the entry); a `Command` held in `cairn-app/src/repository_path.rs`
  (caught as naming `Command`).

QA: see the adjudication entry above (the log is newest-first).

## 2026-09-16 — phase 01 QA adjudicated and fixed

Reviewers spawned fresh over `main...HEAD`: `qa-checklist`,
`destructive-ops-reviewer`, `test-coverage-auditor`, and — because the diff
touches the enforcement layer — `gate-integrity-reviewer`. All raw findings went
to a fresh `qa-confirm`: 25 confirmed (8 duplicate groups merged), 2 dismissed,
1 escalated to the user, 2 deferred to phase 03 by name.

Confirmed and fixed in this phase:

- Three reproduced holes in the new guard (gate-integrity): `cli.rs` was exempt
  from naming `Command`, so an alias `use std::process::Command as Cmd` there
  passed; the `("GIT_TERMINAL_PROMPT", "0")` check ran over the whole file
  including tests rather than the `ALWAYS` table; and the one-literal count was
  keyed to one file, so a child module `ops/environment/bypass.rs` could build
  `GitEnvironment { entries: Default::default() }`. The guard now exempts
  nothing, slices the `ALWAYS` table, looks for the named literal and any `impl`
  block for the type in every production file, asserts `envs` as well as
  `env_clear`, and refuses a `&mut self` method on the type. Each was seen red
  again after the fix and reverted.
- The raw runner was fully public (destructive-ops, BLOCKING): `cairn-app`
  could have run `git.command().args(["push", "--force"]).run()` with no
  `Confirmed` and no guard firing. `GitBinary::command`, `GitCommand` and
  `Output` are now `pub(crate)`; the tests that need them moved inside the
  crate (`ops/cli.rs`, with `ops/stub_git.rs`).
- The roster omitted `DBUS_SESSION_BUS_ADDRESS` and `XDG_RUNTIME_DIR`, which
  `git-credential-libsecret` needs to reach the keyring — exactly the user L7
  protects — and `LANGUAGE`, which gettext reads first. Added, with reasons;
  the enumeration tests spell out the new set.
- `Performed` had public fields, so a future operation could write an
  acknowledged prompt by hand. Fields are private; `Performed::new` and
  `Performed::destructive(.., &Confirmed, ..)` are the constructors.
- The startup check had no test. `worker::open_with(path, GitEnvironment)` is
  the seam; `a_missing_git_is_refused_naming_the_version_and_nothing_is_served`
  pins the message and that the repository is not served behind the refusal.
- Tests added for: `in_repository` and the working directory, stdin being
  `/dev/null` (Linux), `Error::GitNotStarted`, every single `Invalidated` flag
  counting, and `Performed::description`.
- Docs: root `CLAUDE.md` repo map no longer calls `ops/` planned; the invariant
  entry states its `src/` scope and exactly what the guard decides; the
  `cli.rs` header no longer claims closed stdin stops `ssh` (it prompts on
  `/dev/tty`; that is `SSH_ASKPASS_REQUIRE=force`, phase 02); the contract
  states the same-tick residual for `packed-refs` and no longer overstates pack
  unloading; `docs/systems/history-graph.md` names the startup check on the
  `worker::open` path; the reviewer definitions carry the obligations the
  invariant assigns them (`destructive-ops-reviewer` check 9, `qa-checklist`
  item 7).

Dismissed, with reasons:

- "`GitBinary::discover()` hardcoding PATH would pass its only test" — the
  function is a one-line delegation, and the only way to test it is to change
  this process's PATH, which needs `std::env::set_var`: `unsafe`, forbidden.
- "The installed-git test depends on the machine" — by design and documented;
  CI is `ubuntu-latest`, whose git is far above 2.30.
- "B2 should extend to `GitFailed`/`GitNotStarted`" — B2 covers missing or too
  old, and the three variants that decide those all name the version; a git
  whose `--version` fails to exec is neither.
- "A cancelled prompt needs a distinguishable error and stderr must not be
  matched" — already stated in the `ops` output policy; a sentence was added
  under Errors pointing phase 02 at the askpass channel.

Escalated to the user (batched in the phase report): further roster and pin
decisions — `SSL_CERT_FILE`/`SSL_CERT_DIR`/`GIT_SSL_CAINFO`, `KRB5CCNAME`,
`DISPLAY`/`WAYLAND_DISPLAY`, `GNUPGHOME`, proxy variables, and pinning
`GIT_EDITOR` in `ALWAYS`. Each is a deliberate leak or pin under L5 and a
policy question; nothing in phases 02-03 needs any of them.

Deferred to phase 03, by name: the worker discards the discovered `GitBinary`
(holding it unread fails `-D warnings` until an operation reads it); and
`run()` is `output()`-only — fetch progress needs a streamed stderr and a
cancel that can kill the child, which R4.3 owns.

## 2026-09-14 — three open questions closed with the user

Reviewing the packet's open questions closed the three that were genuinely the
user's to take, before any code was written: minimum git version (2.30, a support
policy), the channel's threat model (protects against other users, not same-user
processes — accepted with the limit written down rather than papered over), and
`zeroize` as an accepted dependency (hand-rolled zeroing can be optimised away,
so doing it by hand would claim a protection it might not provide).

Recorded as L9, L10 and L11. O4 and O5 remain, and both are settled by test or
lookup in phase 03 rather than by decision.

## 2026-09-14 — packet planned

Filed from the design decisions locked the same day (`docs/design/cairn.md` D1,
D2). The mechanism was verified against git 2.55.0's own manual on the
development machine rather than recalled: `GIT_ASKPASS` takes the prompt on
`argv` and reads the secret from the program's stdout, and `GIT_TERMINAL_PROMPT`
is what prevents a tty fallback. Those two facts set the whole shape — a separate
helper binary, and a secret that never enters the main process.

Two things were deliberately NOT verified and are carried as open questions: the
precedence between `GIT_ASKPASS`, `core.askPass` and a configured
`credential.helper` (O4), and the OpenSSH version that introduced
`SSH_ASKPASS_REQUIRE` (O5). O4 can invalidate the packet's central product rule,
so it is settled by test, not by reading.

No implementation has started.
