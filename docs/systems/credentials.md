# Credentials

How Cairn asks a user for a secret today, and what it does not do. As-built:
everything here is code that exists, with the test that pins each behaviour
named beside it. The commitment it is being built against is
`docs/prd/credential-prompts.md` (in flight); the decisions are D1 and D2 in
`docs/design/cairn.md` and L1-L11 in the packet's `brainstorm.md`; the evidence
is `docs/research/credential-prompts/git-credential-delegation.md`.

**What works end to end today: nothing the user can reach.** The helper, the
channel and the environment exist and are tested against each other, but no
operation opens a channel, no dialog exists, and no `git` verb that could prompt
is run. A `git` Cairn starts today (`git --version`, at startup) is pointed at
the helper and would fail closed if it asked. Fetch — the first operation that
can prompt — is the next phase's, and so is the wiring below marked "not yet".

## The shape

```
git / ssh ──runs──▶ cairn-askpass ──unix socket──▶ Channel (in Cairn)
   ▲  argv[1]: prompt      │ stdout: secret + "\n"      │ Prompt::answer(&Secret)
   │                       ▼                            ▼
   └── environment: GIT_ASKPASS, SSH_ASKPASS, SSH_ASKPASS_REQUIRE=force,
                    GIT_TERMINAL_PROMPT=0, CAIRN_ASKPASS_SOCKET, CAIRN_ASKPASS_TOKEN
```

Cairn implements no authentication and stores no credential (D2, L1). git
consults the user's `credential.helper` first and only runs the askpass program
when nothing answered; ssh-agent keys never ask. The helper is a separate binary
because git's contract is a process: prompt on `argv[1]`, secret on stdout
(L2). A secret's whole life is: the dialog (not yet), the channel, the helper's
stdout, git's stdin. It never enters `cairn-git` and never enters application
state.

## The pieces

- **`cairn-askpass`** (`crates/cairn-askpass/src/main.rs`). Invoked by git or
  ssh with the prompt as `argv[1]`. Reads `CAIRN_ASKPASS_SOCKET` and
  `CAIRN_ASKPASS_TOKEN` from its environment, never from `argv` (`/proc` makes
  `argv` world-readable, L6), asks the channel, writes the answer to stdout with
  one trailing newline, exits 0. On every failure it writes nothing to stdout,
  one constant-ish line to stderr that names no prompt, token or secret, and
  exits 1; git then reports it could not read a credential. It links
  `cairn-model` and `zeroize` and nothing else
  (`cargo tree -p cairn-askpass`; the allowlist row in
  `crates/cairn-guards/tests/invariants.rs` pins it). Pinned by
  `crates/cairn-askpass/tests/helper.rs`, which runs the built binary: the
  success path
  (`the_helper_prints_the_answer_with_a_trailing_newline_and_nothing_else`), and
  each failure path — no prompt, no variables, no listener, a stale socket, a
  refused prompt, a dropped prompt, a dead or unknown token, wrong permissions,
  not a socket — each named `..._fails_closed` or `..._is_refused...`.
- **`cairn_askpass::Channel`** (`crates/cairn-askpass/src/channel.rs`), the
  application's end. `Channel::open($XDG_RUNTIME_DIR)` creates
  `cairn-<pid>-<random>/` with mode `0700` (created with the mode, not chmod'd
  after) and binds `askpass` inside it, then chmods it `0600` and refuses to
  serve if either mode did not take
  (`what_the_channel_creates_is_owner_only_and_gone_on_drop`,
  `without_a_runtime_directory_there_is_no_channel`). Not the abstract
  namespace, which carries no permissions. `begin()` issues an `Operation`
  whose token is good for every prompt of one git invocation and dies with it;
  `accept()` blocks for one helper and yields a `Prompt`, which is answered
  with a `&Secret` or refused — and a refusal, or a drop without an answer,
  retires the token so git's next ask is refused rather than asked again
  (`a_refused_prompt_fails_closed_and_retires_the_token`,
  `a_prompt_dropped_unanswered_is_a_refusal`). A connection that is not a
  helper is refused and reported, and serving continues
  (`a_connection_that_is_not_a_helper_is_refused_and_serving_continues`).
  The token is per operation, not per ask, because an HTTPS credential is two
  asks (username, then password) with one environment
  (`one_operation_answers_more_than_one_prompt`).
- **The threat model, as stated in the crate docs (L10):** the channel protects
  against other users on the machine, not against other processes running as
  the same user. Same-user isolation is not achievable — such a process can
  read Cairn's environment through `/proc` — and not worth pursuing, because it
  could read `~/.git-credentials` or query the ssh-agent directly. Rejected:
  `SO_PEERCRED` and an inherited descriptor, for the reasons given there.
- **`cairn_model::Secret`** (`crates/cairn-model/src/secret.rs`). No `Debug`,
  `Display`, `Clone` or serialisation; built by moving a buffer in, so the
  allocation that held the typed characters is the one zeroed on drop
  (`zeroize`, L11); read through `expose_secret` only. What the compiler
  refuses is pinned by the type's `compile_fail` doctests, which is why the
  full gate runs `cargo test --doc`; the drop half by
  `a_secret_is_zeroed_on_drop_and_holds_the_only_copy`. What safe code cannot
  observe — that the freed bytes are zero — rests on reading `zeroize`'s
  `Vec` impl (it zeroes the length, clears, then zeroes the spare capacity,
  through volatile writes); the packet's `progress.md` records that reading.
- **`cairn_model::AskpassToken`** and the variable names `SOCKET_VARIABLE`,
  `TOKEN_VARIABLE`, `HELPER_PROGRAM` (`crates/cairn-model/src/askpass.rs`):
  the contract the helper and the environment builder share without seeing
  each other. A token is an authorisation to be asked, not a credential; its
  `Debug` shows nothing anyway.
- **The environment** (`crates/cairn-git/src/ops/environment.rs`, and
  `ops/askpass.rs`). `GitEnvironment::new(parent, &Askpass)` sets, on every
  invocation, `GIT_TERMINAL_PROMPT=0` and `SSH_ASKPASS_REQUIRE=force` (the
  `ALWAYS` table), `GIT_ASKPASS` and `SSH_ASKPASS` to the helper, and
  `CAIRN_ASKPASS_SOCKET` when the `Askpass` names a socket; there is no
  environment without an `Askpass`. `GitEnvironment::command` applies
  `CAIRN_ASKPASS_TOKEN` per invocation, from `GitCommand::authorized_by`,
  because the token is the invocation's. Pinned by
  `the_environment_is_exactly_the_deliberate_entries` (the whole set, spelled
  out), `the_token_is_set_on_the_invocation_and_only_when_given`, and the stub
  `git` in `ops/cli.rs` that prints what it was given
  (`the_child_sees_the_built_environment_and_nothing_inherited`,
  `an_authorised_invocation_carries_its_token_and_only_that_one`).
- **The application** (`crates/cairn-app/src/worker/pool.rs`). `worker::open`
  names the helper beside its own executable (`cairn-askpass`, or the bare name
  for git to search `PATH` if where the executable is cannot be known) and, as
  yet, no socket: no channel is opened, so every prompt fails closed.

## The invariant this leaves behind

"No credential value is logged, Debug-printed, serialised, or stored in
application state" — root `CLAUDE.md`, Invariants, with its twin
`no_credential_value_is_logged_printed_serialised_or_stored` and the review
obligations it states. The `every_git_invocation_disables_the_terminal_prompt`
guard now also pins the askpass entries.

## Not yet, and not here

Not built: the dialog that answers a `Prompt` (which must name the remote and
URL asking), the worker thread that accepts on the channel and hands prompts to
the UI, opening the channel and passing its socket into the environment, fetch
itself, and a cancel that kills the git process. Not in the packet at all:
push, clone, submodule credentials, proxies, GPG signing, and any Cairn-owned
credential storage, which D2 rules out permanently.

Known limits, described rather than pinned: the helper has no read timeout —
it waits as long as the user takes, and a Cairn that dies closes the socket,
which is what ends the wait; `$XDG_RUNTIME_DIR` is required, so a session
without one (macOS today) gets no channel and clean failures rather than a
fallback directory, a policy for the user to set; and a helper answering
`ssh`'s host-key confirmation (`SSH_ASKPASS_REQUIRE=force` routes that to the
helper too) receives whatever the application answers — the application does
not yet distinguish it from a passphrase prompt.
