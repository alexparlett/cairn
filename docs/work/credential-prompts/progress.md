# Progress — credential-prompts

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-09-17 — phase 04: packet QA, the merge bar

Packet mode. Five reviewers spawned fresh and in parallel over the WHOLE packet
diff `main...HEAD` — `qa-checklist`, `destructive-ops-reviewer`,
`responsiveness-reviewer`, `gate-integrity-reviewer`, `test-coverage-auditor`
(three hit their turn limit and were resumed to a full report). All 34 raw
findings went to a fresh `qa-confirm`: 19 confirmed, 12 dismissed, 3 escalated,
with two duplicate pairs merged.

Secret-in-history check, first because it is the one that cannot be fixed later:
`git log -p main..HEAD` scanned for credential, key and token values, plus a
sweep for private-key blocks, provider token prefixes and literals assigned to
secret-shaped names, and a review of every path ever touched. **Nothing was ever
committed.** Every fixture generates its credentials per run.

Guards seen red, each by violating it and reverting: dropping the
`GIT_TERMINAL_PROMPT` tuple failed
`every_git_invocation_disables_the_terminal_prompt`; deriving `Debug` on
`worker::Reply` failed `no_credential_value_is_logged_printed_serialised_or_stored`;
spawning `git` from `refs.rs` failed `only_the_ops_module_mutates_a_repository`.

Fixed in focused commits:

- `fix(app)`: a Cancel pressed before the repository thread dequeued the fetch
  landed on `Stage::Idle` and was discarded, so the fetch the user had already
  cancelled ran to completion. A cancel with nothing armed now parks in its own
  stage that the next `arm` claims.
- `fix(app)`: the failure banner took the first line of `Error::GitFailed`,
  which for a fetch is a progress redraw, not the reason; it now prefers git's
  own `fatal:`/`error:` diagnostics.
- `fix(ui)`: the host-key dialog said nothing about accepting being permanent,
  though a yes writes the key to `known_hosts` for good.
- `fix(guards)`: `PRODUCT_SOURCE_DIRS` was the one hand-maintained roster with no
  closure test, so a crate added later would sit outside both the mutation and
  terminal-prompt guards while they went on passing; also dropped an assertion
  that could not fail and corrected a self-test message that inverted what the
  struct matcher counts.
- `test(git)`: every refusal test refused the FIRST prompt, so git never had a
  credential rejected — the one case its own machinery retries. B5 now answers
  the username and refuses the password.

Dismissed, with reasons (from `qa-confirm`):

- "Teardown obligations not discharged" — teardown is not this phase; it is the
  orchestrator's step after the merge bar.
- "Two `Debug` impls render git's prompt text" — they render the QUESTION; a
  `Secret` has no `Debug` and cannot be derived into one, so no answer reaches
  them.
- "The failure-message path is unasserted for secrets" — it is, twice:
  `fetch.rs` runs `no_secret_in(&error.to_string(), ..)` on both refusal tests,
  and that string is exactly what the banner renders.
- The per-byte `format!` in the token, files without an in-file test module, one
  `current_exe()` readlink at startup, `HistoryList::index_of`'s scan (unchanged
  by this packet), roster growth being "a code edit", and `GitEnvironment`'s
  derived `Debug` carrying the askpass token (authorisation, not a credential,
  and formatted nowhere on a shipping path) — each filed by its own reviewer as
  a note, and each judged not a defect.

Confirmed but NOT fixed here, because each is an architecture or policy change
rather than a QA repair — carried to teardown as issues: the progress-line
volume on a large fetch, the synchronous history clear on a refresh, the two
unbounded `ref_tips` walks per fetch, the retained stderr and the parked reader
thread per cancel, `Performed` being built and discarded, the 64 KiB request
truncation, the un-zeroed toolkit buffers behind the dialog, the hook/guard
seal lists having no comparator, B6's `argv` arm having no behavioural test, and
`fetch_tests`'s deadline-free waits.

Dismissal audit over the whole packet: every earlier dismissal still holds. The
one that deferred a question — phase 02's "`cargo run -p cairn-app` does not
build the helper", left to phase 03 — was discharged: the app degrades with a
message naming the build command, `credentials.md` has a section on it, and the
root `CLAUDE.md` says so beside the run command. Verified by building
`-p cairn-app` with the helper deleted.

O1-O5 all answered and recorded; O4's addendum shows L7 holds by git's own
precedence, not by accident, so the stopping rule did not trigger.
`docs/systems/credentials.md` exists and names push only as not built.

## 2026-09-17 — phase 03 QA adjudicated and fixed

Reviewers spawned fresh over `b48b5a5...HEAD` (packet context `main...HEAD`):
`qa-checklist`, `responsiveness-reviewer`, `destructive-ops-reviewer`,
`test-coverage-auditor`, and `gate-integrity-reviewer` (the diff touches the
guard rosters); each ran twice because the first set were dispatched in the
background and their reports were thought lost — both sets arrived and were
merged. All 43 raw findings went to a fresh `qa-confirm`: 33 confirmed, 6
dismissed, 2 escalated, 1 needing a display to probe.

Confirmed and fixed, in focused commits:

- `fix(guards)`: `code_only` opened a char literal on ANY apostrophe, so a
  `&'static str` followed by a string containing `'` and `//` blanked the
  rest of the file for every matcher — `window.rs` was unseen from its first
  prompt fixture on (CRITICAL; reproduced by the reviewer with `.lock()` and a
  `Reply`-holding struct appended there passing). Fixed with the lifetime
  logic `code_without_strings` already had, plus a self-test; escaped
  newlines in strings keep the line count. `cairn_askpass` joins the
  render-path seal (guard roster, hook, CLAUDE.md) since `accept` blocks.
- `fix(git)`: `reap` checked the cancelled flag before the exit status, so a
  cancel racing a clean exit reported `GitCancelled` and hid moved refs; the
  test that pinned it asserted the wrong outcome and now observes a zombie
  before killing. The kill and fetch docs no longer claim git cleans its
  locks after `SIGKILL`, nor that a fetch is non-destructive under every
  configuration. `Repository::remotes` cannot fail and leaves an embedded
  password out (`Error::Remotes` was never constructed; deleted);
  `ref_tips` added; `LC_CTYPE` joins the locale roster; the drain fix got a
  deterministic test (fails 3 of 3 without the drain); the `/proc` scan a
  positive control; `unknown_tokens() == 0` assertions that could not fail
  were removed; the ssh skips fail under `CAIRN_REQUIRE_SSH_FIXTURE` and use
  `eprintln!` like their siblings.
- `fix(app)`: `refreshed` was always true (every fetch cleared the rows and
  lost the reader's place) and only on success (a failed or killed fetch that
  moved refs left the graph stale) — now the operations thread compares ref
  tips before and after, on every outcome. `FetchStarted` was sent before the
  kill handle existed (a cancel in that window was a no-op); a cancel queued
  behind page walks and a second press queued an uncancellable fetch —
  `FetchControl` arms one fetch at a time, keeps an early cancel, and the
  handle cancels directly; the button is gone from the press itself. The
  token is retired before the outcome (a helper orphaned by a killed git is
  refused, not accepted after `withdraw` ran); an acceptor that could not
  start is a named reason; `FetchProgress` no longer clones the remote per
  line; the failure banner is one line. Applying an update moved from
  `main.rs` to `session.rs` with tests (withdraw, reload, refuse-when-idle),
  and the task holds the answering end weakly. The first shape of that
  module held `&Weak<dyn Fn(Reply)>` in a struct and the credential guard
  refused it — correctly — so it became two plain callbacks.
- `test(app,ui)`: the boundary B3 test compared the header to a prefix
  (an empty password passed); it compares to the exact Basic credential.
  Dialog tests for an unrecognised prompt and a press outside.
- Docs: CLAUDE.md's threads and epoch sentences, its `TEST_ONLY_ALLOWLIST`
  and alias sentences, a `cairn-askpass` repo-map row; `history-graph.md`'s
  "every submit supersedes"; `credentials.md`'s helper-build and ssh-skip
  claims and the changed shapes; state.md's premature QA row and stale
  `Invalidated` row.

Dismissed, with reasons (from `qa-confirm`):

- Startup does `git --version` and a socket bind before the first page —
  milliseconds, not repository-sized.
- `reap` held the child lock across `wait()` — was a bounded note on the
  repository thread; moot now that the wait polls `try_wait` and the kill
  only tries for the lock.
- `fetch_tests.rs` is held to production guard rules though it is a test
  module — stricter than documented is not a gap; the asymmetry with
  `tests/remotes` (out of the product `src/` scope by design) is now stated
  in state.md.
- A daemonising grandchild holding stderr — probed, git's own daemons and
  ssh's masters redirect; not reproducible.
- `Input::on_submit`'s toolkit copy of the typed text — the argument is
  discarded; the answer is the component's own buffer, moved.

Escalated to the user (batched in the phase report): `SIGTERM` before
`SIGKILL` needs a signalling dependency; whether a fetch under a
local-branch refspec or prune configuration should take `Confirmed`; whether
CI should provision `sshd` and set `CAIRN_REQUIRE_SSH_FIXTURE`. Needs a
display to probe: whether closing the window with a dialog up leaves the
socket directory behind (recorded as a known limit meanwhile).

## 2026-09-17 — phase 03: fetch end to end landed

Packet mode, committed directly to `feature/credential-prompts` in six
commits (research addenda, model, git, ui, app, an askpass fix) plus docs.
Full gate PASS. QA adjudication is the entry above this one once it exists.

O4 and O5 settled first, by test, before any code: see the two addenda in
`docs/research/credential-prompts/git-credential-delegation.md`. O4 came out
the way L7 needs — a configured `credential.helper` answers before the
askpass is consulted — so the stopping rule did not trigger.

Decisions taken in-phase, none reopening a locked decision:

- **Fetch takes no `Confirmed`**, said in its module docs: it moves only
  remote-tracking refs, all in the reflog.
- **The runner reads stderr on a thread.** `finish` on the calling thread
  waited for the pipe to close, and a SIGKILLed git's children (`ssh`, the
  helper, `git-remote-https`) inherit the pipe and outlive it; a cancel would
  have waited for them. The reader is detached and `finish` returns once git
  itself is reaped. "`cairn-git` knows nothing about threads" is about where
  blocking work is scheduled, not a ban on a pipe reader; recorded in
  `credentials.md`.
- **Three threads per repository, not one.** Fetch on its own thread so
  paging keeps working; the acceptor on its own because a fetch blocked on the
  helper blocked on the acceptor would deadlock on one thread. Each has its own
  sender; the stream ends when all have gone. Operations carry no epoch.
- **The secret crosses as `worker::Reply` over its own channel**, handed out
  by `open` as a bare `Rc<dyn Fn(Reply)>` so no struct holds it. First named
  `Answer`; renamed because the credential guard reads spellings and
  `cairn_askpass::Error::Answer` made it a container of `Error` and `Refusal`.
  The guard erring toward catching is the right direction; the note is in
  state.md.
- **The dialog hands out a `String`, not a `Secret`.** An `EventHandler<Secret>`
  field would make the component a guarded holder; the window wraps the moved
  buffer in `Secret::from_string` at once. Freya's `Input` keeps its own
  unzeroed copies regardless — a toolkit limit, written down.
- **A missing helper or runtime directory is a reason, not a refusal.**
  Fetch still runs (a helper or agent may answer, L7); a failure appends why
  nothing could have asked. Working assumption from the orchestrator, pending
  the user.
- **`cargo run -p cairn-app` still does not build the helper.** Cargo builds a
  dependency's library only. Documented in CLAUDE.md and `credentials.md`;
  the worker looks beside its executable and names the build command. A
  build.rs that runs cargo, or bindeps, are the user's call (batched).
- **The remote picker is the default remote.** `Repository::remotes` lists
  them all, default first; the button fetches the first. A picker is UI work
  for a later packet.
- **The ssh fixture goes through `core.sshCommand`**, since ssh ignores
  `$HOME` for its config; `sshd` must be invoked by absolute path.
- **A phase-02 flake fixed**: the channel closed an over-long request with
  bytes unread, which reset the connection and lost the helper its answer
  (`an_oversized_prompt_is_bounded_rather_than_hung`, two runs in three). The
  rest is now drained, bounded, before answering.

Assumptions recorded for the user (from phase 02's escalations, taken as the
orchestrator directed): a user-set `SSH_ASKPASS`/`core.askPass` is replaced
while Cairn runs git (L5; accepted collateral, in `credentials.md`, not the
PRD); a missing helper degrades prompting with a clear message rather than
refusing the app.

## 2026-09-17 — phase 02 QA adjudicated and fixed

Reviewers spawned fresh over `ea72de9...HEAD` (packet context `main...HEAD`):
`qa-checklist`, `destructive-ops-reviewer`, `gate-integrity-reviewer` (the
phase changes the enforcement layer), `test-coverage-auditor`. All raw findings
went to a fresh `qa-confirm`: 27 confirmed (5 duplicate groups merged), 6
dismissed, 2 escalated to the user.

Confirmed and fixed, in focused commits:

- Guard holes the gate-integrity reviewer reproduced on a scratch copy: the
  stored-state rule read only the direct type (a struct keeping the enum that
  carries a secret passed); "one accessor" was a count of `-> &[u8]`, so a
  `Deref`, an `impl From<Secret>` or an `into_bytes` routed around the reader
  roster; `use Secret as ..` and a `type` alias passed; `format_args!` and
  `log::log!` were missing from the rendering macros; a path-qualified impl
  target (`impl Debug for self::Held`) and a `where` clause with a
  parenthesised bound escaped the matchers; the readers floor was satisfied by
  the definition itself; the socket/token spellings in the terminal-prompt
  guard were satisfied by the import line; the passing twin of the
  compile-fail doctests was unpinned. Each fixed, added to the self-test, and
  seen red on real code (a struct keeping a secret-carrying enum, an
  `impl From<Secret>` in `protocol.rs`, a `use .. as`, a `type` alias, a
  `Deref` in `secret.rs` itself). The `dbg!` fixture in the self-test tripped
  the Stop hook on every turn of this branch; it is spelled in two pieces now.
- Hook parity: `.claude/hooks/qa-stop.sh` gained the `cairn-askpass` seal row
  (engine, toolkit, and logging crates) with two hook tests.
- The helper wrote the secret and its newline in two `write_all`s, which
  parks a newline-free secret in std's 1 KiB line buffer, never zeroed; it now
  writes once from a zeroed buffer. The "one allocation" claim in this log's
  previous entry was overstated for the helper — there are two, both zeroed.
- The helper checked modes, never ownership; it now refuses a socket or
  directory owned by another user (Linux, via `/proc/self`). Not testable
  without a second user; the positive arm runs in every helper test.
- The wrong-permissions test could only fail by hanging; it keeps an acceptor
  ready so a helper that connected is answered and fails the assertion. Every
  refusal asserts its stderr names neither the prompt nor the token (the B6
  arm for the helper's own stderr). New tests: a symlinked socket, an
  oversized prompt (bounded, not hung), a bind that fails after mkdir (nothing
  left behind), and where the worker points `GIT_ASKPASS`.
- Two test names claimed what safe code cannot observe (zeroing on drop, no
  reallocation) and now say what they pin; `Secret::len`/`is_empty` are
  documented as the deliberate non-secret they are.
- Docs: `credentials.md` no longer states O4's precedence as fact, and names
  the failure tests rather than a pattern that did not hold; the CLAUDE.md
  residual list gains the hoisted-local shape and loses the alias (now
  guarded); the gate gets a separate `test-doc` step so a red `--all-targets`
  run no longer hides the doctest result.

Dismissed, with reasons:

- "`cargo run -p cairn-app` does not build `cairn-askpass`" — true, and the
  app opens no channel this phase; a missing helper fails closed. Phase 03,
  where the app first needs the helper, decides the build wiring.
- "XDG_RUNTIME_DIR required / host-key confirmation routed to the helper / O5
  open" — all already stated under Known limits in `credentials.md`.
- "The `ALWAYS` comment on `SSH_ASKPASS_REQUIRE` overstates" — the comment
  already defers the OpenSSH floor to O5 by name.
- "The helper cannot tell host-key confirmation from a passphrase" — real, no
  ssh operation exists this phase, and the dialog that must handle it is
  phase 03's (carried forward below by name).
- "`the_variable_names_are_the_ones_the_helper_reads` is a constant-vs-literal
  pin" — the auditor's own verdict was "not a gap"; the cross-side pin is the
  end-to-end helper test.

Escalated to the user (batched in the phase report): whether a user-set
`SSH_ASKPASS` (a wallet-backed askpass with no agent) should be honoured rather
than replaced — L5 says replace, L7/B4 name only `credential.helper` and
ssh-agent as the protected setups; and whether a missing helper should be
refused at startup rather than surfacing as git's "terminal prompts disabled"
on the first prompt.

Carried forward to phase 03 by name: the dialog must show the prompt text (or
the channel gains a prompt kind) so ssh's host-key confirmation is not typed
into a password field; `worker::Request`/`Update` derive `Debug`, so the
secret's path from dialog to worker cannot be a variant of either; the app must
build and locate the helper for the B3 fixture; `Channel::accept` blocks and
needs its own thread plus a way to unblock it on shutdown.

## 2026-09-17 — phase 02: the askpass helper, its channel and the secret type landed

Packet mode, committed directly to `feature/credential-prompts` in four commits
(model, askpass, git, guards) plus docs. Full gate PASS, including the doctest
run the gate now makes. QA adjudication is the entry above this one.

Decisions taken in-phase, none reopening a locked decision:

- **The token is scoped to one operation, not one ask.** L10 says "single-use
  token". An HTTPS credential is two asks (`Username for`, then `Password for`),
  each a fresh helper process with the same environment, so a token consumed by
  the first ask would fail every HTTPS credential. "Single use" is implemented
  as: one token per git invocation, issued when the operation begins, dead when
  the `Operation` drops or the moment a prompt under it is refused — so a
  cancel cannot be followed by git asking again (R2.5). Written in the crate
  docs and pinned by `one_operation_answers_more_than_one_prompt` and
  `a_refused_prompt_fails_closed_and_retires_the_token`. Flagged for the user
  as an interpretation, not a stopping rule.
- **`Secret` lives in `cairn-model`, and so does `zeroize`.** The secret
  crosses from the UI (the dialog, phase 03) to the worker, which is exactly
  what `cairn-model` is for; the alternative — the helper crate as the UI's
  dependency — would have a render crate depend on a crate that opens sockets.
  The repo-map row now reads "depends on nothing but `zeroize`".
- **The helper crate is a library and a binary.** Both ends of one wire format
  in one crate, so they cannot drift; the binary links `cairn-model` and
  `zeroize` only (`cargo tree -p cairn-askpass`). No `thiserror` in the helper —
  its error types are hand-written to keep the linked surface at two crates.
- **`Askpass` is a required input to `GitEnvironment::new`**, with the socket
  optional. There is no environment without a helper named, so nothing ever
  falls back to a tty; a Cairn with no channel fails every prompt closed. The
  token is applied by `command()`, per invocation, because it is the
  invocation's property — and the terminal-prompt guard's "exactly one
  `Self {..}` literal" rule is what moved `Askpass` into `ops/askpass.rs`.
- **The app opens no channel yet.** `worker::open` names the helper beside the
  executable and passes no socket; phase 03 opens the channel on the worker
  thread before discovering git and passes `channel.socket_path()` in. Stated
  in `docs/systems/credentials.md` under "Not yet".
- **`$XDG_RUNTIME_DIR` is required, with no fallback.** L10 names it; a
  fallback directory (macOS has no runtime dir) is a policy for the user.
- **No read timeout in the helper.** A user takes as long as they take; a
  Cairn that dies closes the socket, which ends the wait. The cancel that
  kills a hung git is R4.3's, phase 03.
- **Tokens come from `/dev/urandom`**, 32 bytes as hex, rather than a `rand`
  dependency.

Zeroize verification, what was actually confirmed against the vendored
`zeroize-1.9.0/src/lib.rs`: `Zeroizing<Z>::drop` calls `Z::zeroize`;
`impl Zeroize for Vec<Z>` zeroes the initialised elements through
`volatile_set` (`ptr::write_volatile` per element, then an
`optimization_barrier`), calls `clear()`, then zeroes the spare capacity the
same way — so the whole allocation, not just the length, is scrubbed; it
documents that it cannot scrub what an earlier REALLOCATION left behind.
`Secret` therefore has exactly one field, `Zeroizing<Vec<u8>>`, and both
constructors move a buffer in without copying
(`a_secret_is_zeroed_on_drop_and_holds_the_only_copy` pins the pointer
identity, and that `Secret: ZeroizeOnDrop`). The helper reads the socket into a
buffer sized to the response limit up front and drains the status line in
place, so its one allocation is the one zeroed. What safe code cannot observe:
that the freed bytes are zero — reading freed memory is `unsafe`, and `unsafe`
is forbidden — so that half of B7 rests on the reading above, not a test.

Guards seen red before landing, then restored (snapshot-and-copy, not
`git checkout`, after the lesson in state.md):

- `no_credential_value_is_logged_printed_serialised_or_stored`, nine shapes:
  `#[derive(Debug)] struct Held { s: Secret }` appended to
  `cairn-askpass/src/channel.rs` (caught: "gives `Held`, which holds a
  credential, an impl that renders"); a hand-written `impl Debug for Held`
  (same message); `struct Inner(Secret); #[derive(Clone)] struct Outer { inner:
  Inner }` in `cairn-app/src/worker/request.rs` (caught on `Outer`, the
  container of a container); `format!("{:?}", s.expose_secret())` in the
  helper's `main.rs`, a roster file (caught: "names `expose_secret` inside a
  macro that renders its arguments"); `tracing::info!(pw = ?s.expose_secret())`
  there (same); `s.expose_secret()` in `cairn-git/src/ops/cli.rs` (caught:
  outside `SECRET_READERS`); `pub struct Pending { secret: Secret }` in
  `request.rs` (caught: "application state keeping a credential");
  `#[derive(Clone)]` on `Secret` itself (caught: "derives something"); the
  field changed to `Vec<u8>` (caught: "no longer a `Zeroizing<..>`").
- `every_git_invocation_disables_the_terminal_prompt`, three shapes:
  `("SSH_ASKPASS_REQUIRE", "never")` in `ALWAYS` (caught: the table "no longer
  carries"); the `"GIT_ASKPASS"` literal misspelled in the constructor (caught:
  "no longer sets"); and, live rather than staged, `Askpass::new`'s
  `Self { .. }` beside `GitEnvironment::new`'s (caught: "built in exactly one
  place"), which is what moved `Askpass` to its own module.
- The `compile_fail` doctests: confirmed that stable `rustdoc` ignores the
  `,E0277` error-code suffix (an `E9999` suffix still passed), so the codes
  were removed rather than kept as a pin that does not hold, and a passing twin
  snippet was added so each refused block differs from working code by one
  line.

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
