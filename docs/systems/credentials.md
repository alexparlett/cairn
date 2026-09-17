# Credentials

How Cairn asks a user for a secret today, and what it does not do. As-built:
everything here is code that exists, with the test that pins each behaviour
named beside it. The commitment it is being built against is
`docs/prd/credential-prompts.md` (in flight); the decisions are D1 and D2 in
`docs/design/cairn.md` and L1-L11 in the packet's `brainstorm.md`; the evidence
is `docs/research/credential-prompts/git-credential-delegation.md`, whose
2026-09-17 addenda settle the two questions the design left open (O4, O5).

**What works end to end today: fetch.** The window offers "Fetch <remote>" for
the repository's default remote; the fetch runs on its own thread, its progress
is git's own, it can be cancelled, and when it moves refs the history on screen
is asked for again. When git or ssh needs a credential, the helper asks the
running Cairn, a dialog names the remote and shows the prompt as git asked it,
and the answer goes back to git — or, if the user declines, the fetch fails
cleanly and nothing asks twice. A user whose credential helper or ssh-agent
already answers sees no dialog at all. Push, clone and everything else that
could prompt are not built.

## The shape

```
window ──Reply──▶ acceptor thread ──unix socket──▶ cairn-askpass ──stdout──▶ git / ssh
   ▲   (a value)     Prompt::answer(&Secret)          argv[1]: prompt            │
   │                                                                             │
   └──Update::Prompt { id, text }◀── accept() ◀──────── connects ◀──────────────┘
                     environment: GIT_ASKPASS, SSH_ASKPASS, SSH_ASKPASS_REQUIRE=force,
                                  GIT_TERMINAL_PROMPT=0, CAIRN_ASKPASS_SOCKET, CAIRN_ASKPASS_TOKEN
```

Cairn implements no authentication and stores no credential (D2, L1). git
consults the user's `credential.helper` first and runs the askpass program only
for what the helper left unanswered; `GIT_ASKPASS` takes precedence over
`core.askPass`; ssh-agent keys never ask — each settled by test in the O4
addendum, and the first is what makes L7 hold without a workaround. The helper
is a separate binary because git's contract is a process: prompt on `argv[1]`,
secret on stdout (L2). A secret's whole life is: the dialog's input, the
`Reply` the window sends, the channel, the helper's stdout, git's stdin. It
never enters `cairn-git` and never enters application state.

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
  each failure path: `without_a_prompt_the_helper_fails_closed`,
  `without_the_channel_in_its_environment_the_helper_fails_closed`,
  `with_no_cairn_listening_the_helper_fails_closed` (no socket, and a stale
  one), `a_refused_prompt_fails_closed_and_retires_the_token`,
  `a_prompt_dropped_unanswered_is_a_refusal`,
  `a_token_for_no_live_operation_is_refused`,
  `a_socket_with_the_wrong_permissions_is_refused_before_connecting` (modes,
  a symlink, not a socket), and `an_oversized_prompt_is_bounded_rather_than_hung`.
  Every refusal's stderr is asserted to name neither the prompt nor the token.
  The answer and its newline reach stdout as one write from a zeroed buffer —
  two writes would leave the first in std's line buffer, which nothing zeroes.
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
  `a_prompt_dropped_unanswered_is_a_refusal`). A request longer than the
  channel reads is cut at the limit and the rest drained before the answer is
  written, because closing with unread bytes in the socket resets the
  connection and loses the helper its answer. A connection that is not a
  helper is refused and reported, and serving continues
  (`a_connection_that_is_not_a_helper_is_refused_and_serving_continues`).
  The token is per operation, not per ask, because an HTTPS credential is two
  asks (username, then password) with one environment
  (`one_operation_answers_more_than_one_prompt`).
- **The threat model, as stated in the crate docs (L10):** the channel protects
  against other users on the machine, not against other processes running as
  the same user. The helper checks the socket and its directory are mode
  owner-only and, on Linux, owned by the user running it (read off
  `/proc/self`), so a runtime directory that is not the private one XDG
  promises still cannot hand a prompt to another user's socket. Same-user
  isolation is not achievable — such a process can read Cairn's environment
  through `/proc` — and not worth pursuing, because it could read
  `~/.git-credentials` or query the ssh-agent directly. Rejected: `SO_PEERCRED`
  and an inherited descriptor, for the reasons given there.
- **`cairn_model::Secret`** (`crates/cairn-model/src/secret.rs`). No `Debug`,
  `Display`, `Clone` or serialisation; built by moving a buffer in, so the
  allocation that held the typed characters is the one zeroed on drop
  (`zeroize`, L11); read through `expose_secret` only. What the compiler
  refuses is pinned by the type's `compile_fail` doctests, which is why the
  full gate runs `cargo test --doc`; the drop half by
  `a_secret_promises_to_zero_zeroes_on_request_and_holds_the_only_copy`, which
  says in its name what safe code can observe. What safe code cannot
  observe — that the freed bytes are zero — rests on reading `zeroize`'s
  `Vec` impl (it zeroes the length, clears, then zeroes the spare capacity,
  through volatile writes); the packet's `progress.md` records that reading.
- **`cairn_model::AskpassToken`** and the variable names `SOCKET_VARIABLE`,
  `TOKEN_VARIABLE`, `HELPER_PROGRAM` (`crates/cairn-model/src/askpass.rs`):
  the contract the helper and the environment builder share without seeing
  each other. A token is an authorisation to be asked, not a credential; its
  `Debug` shows nothing anyway.
- **`cairn_model::PromptKind` and `prompt_subject`**
  (`crates/cairn-model/src/prompt.rs`). What a prompt asks for, read off the
  spellings git and OpenSSH use — username, password, passphrase, ssh's
  host-key confirmation, or other — and the first quoted span, which is the
  URL, key path or host. Presentation only; no outcome depends on it
  (`the_spellings_git_and_ssh_use_are_told_apart`,
  `the_subject_is_the_first_quoted_span`).
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
  `SSH_ASKPASS_REQUIRE=force` needs OpenSSH 8.4 (2020-09); on an older one the
  variable is ignored and a passphrase goes to the terminal Cairn was launched
  from, or fails closed without one (O5 addendum). Accepted collateral of L5:
  a `GIT_ASKPASS`, `SSH_ASKPASS` or `core.askPass` the user set for something
  else is replaced while Cairn runs git, since `GIT_ASKPASS` outranks
  `core.askPass` and the environment is never inherited; `credential.helper`
  and the agent, which L7 protects, are untouched.
- **`ops::fetch`** (`crates/cairn-git/src/ops/fetch.rs`). `fetch(&git, &repo,
  remote, token)` starts `git fetch --progress --end-of-options <remote>` and
  returns a `FetchInProgress`; `finish(progress)` streams each redraw of git's
  progress meter to the callback and yields a `Performed` declaring `refs` and
  `objects` invalid; `canceller()` is a `Send` handle that kills the process
  from any thread, after which `finish` reports `Error::GitCancelled`. **Not
  destructive, and takes no `Confirmed` on purpose**: a fetch moves only
  remote-tracking refs, all in the reflog; the module docs say so. The runner
  underneath (`GitCommand::stream` in `ops/cli.rs`) reads the pipe on a thread
  of its own, because git's children — `ssh`, `git-remote-https`, the helper —
  inherit it and outlive a killed git; a cancel returns once git itself is
  reaped, and the reader ends when the last child lets the pipe go. Pinned by
  the stub-git tests `a_streamed_invocation_hands_stderr_on_a_redraw_at_a_time`
  and `a_kill_from_another_thread_ends_a_hung_invocation_and_reaps_it`, and
  end to end by `crates/cairn-git/tests/fetch.rs` (below).
- **`Repository::remotes`** (`crates/cairn-git/src/remotes.rs`). The
  configured remotes as `cairn_model::RemoteSummary`, the default first, read
  through gitoxide; what the window's fetch button names
  (`this_checkout_lists_its_origin_with_a_url`).
- **The dialog** (`crates/cairn-ui/src/credential_prompt.rs`,
  `CredentialPrompt`). Names the remote the running operation was asked for,
  states what is wanted from where, and shows the prompt exactly as git or ssh
  gave it — the URL in it is the one being authenticated against. A username
  is typed in the open, a password or passphrase masked, and ssh's host-key
  check is a question with "Yes, connect" and no text field, answered `yes`.
  Cancel, Escape and a press outside all decline. Pinned by
  `crates/cairn-ui/tests/credential_prompt.rs`
  (`a_password_prompt_names_the_remote_and_the_url_and_masks_what_is_typed`,
  `a_host_key_confirmation_has_no_text_field_and_is_answered_yes`,
  `cancel_and_escape_both_decline_without_submitting`, and two more). The
  typed text leaves through `on_submit` as the input's `String`, moved not
  copied, and the window wraps it in `Secret::from_string` at once.
- **The application** (`crates/cairn-app/src/worker/`). Three threads per open
  repository, each with its own sender, so the update stream ends only when all
  have gone. `startup.rs`: on the repository thread, before anything else, the
  channel is opened under `$XDG_RUNTIME_DIR`, the environment built around its
  socket, and `git` found — a missing runtime directory or a helper that is not
  built beside the executable is kept as a reason, not a refusal: fetching still
  works wherever a helper or agent answers (L7), and a fetch that then fails
  says why nothing could have asked
  (`a_runtime_directory_gives_the_environment_a_socket_and_prompting`,
  `without_a_runtime_directory_git_is_still_found_and_the_reason_is_named`,
  `a_missing_helper_is_named_with_how_to_build_it`,
  `a_failure_with_no_way_to_prompt_says_why_nothing_asked`). `askpass.rs`: the
  acceptor thread turns each helper connection into `Update::Prompt { id, text }`
  and waits on the window's `Reply` — `Provide { prompt, secret }` or
  `Refuse { prompt }` — over a channel of its own (never a `Request`, which
  derives `Debug`), handing the secret to `Prompt::answer` by reference and
  dropping it. `operations.rs`: the operations thread runs the fetch with a
  token from `Channel::begin`, forwards every progress line as
  `Update::FetchProgress`, and ends with `FetchFinished { refreshed }`,
  `FetchCancelled` or `FetchFailed`; `Request::CancelFetch` reaches its
  `FetchCancel` through the repository thread. Operations carry no epoch, so a
  scroll cannot supersede a fetch nor a fetch a scroll. On shutdown the
  repository thread kills any fetch, closes the operations queue and wakes the
  acceptor by connecting to its own socket (the one thing that returns a
  blocking `accept`), on a clean exit and on unwinding alike; a prompt still
  waiting is refused. Pinned by `crates/cairn-app/src/worker/fetch_tests.rs`
  against real git and the built helper, with a loopback remote that answers
  `401` to everything:
  `a_fetch_reports_progress_finishes_and_the_history_reloads_from_the_new_refs`
  (the cache contract's first real exercise: the worker's gix handle serves the
  commits a `git` subprocess just wrote),
  `a_prompt_reaches_the_window_as_a_value_and_its_answer_reaches_git`,
  `refusing_a_prompt_fails_the_fetch_once_and_the_worker_carries_on`,
  `cancelling_a_fetch_that_waits_on_a_prompt_ends_it_as_cancelled`,
  `letting_go_of_the_repository_ends_the_acceptor_and_removes_its_socket` and
  `a_prompt_waiting_at_shutdown_is_refused`.
- **The window** (`crates/cairn-app/src/window.rs`, `main.rs`,
  `fetch_state.rs`). A "Fetch <remote>" button for the default remote, which
  becomes "Cancel" while a fetch runs; a line under the title bar with git's
  latest progress or the outcome; and the dialog whenever `Update::Prompt`
  has set a `PromptView`, answered through the worker's `Replier` callback
  and taken down. A finished fetch that moved refs clears the rows, resets the
  progress and asks for the history from `HEAD` again; a fetch ending with a
  dialog still up refuses that prompt, which releases the helper. Pinned by
  `a_prompt_draws_the_dialog_and_its_answer_leaves_as_a_secret_for_that_prompt`,
  `cancelling_the_dialog_refuses_that_prompt`,
  `the_fetch_button_fetches_the_default_remote_and_becomes_cancel_while_running`
  and `a_failed_fetch_is_said_in_the_window_and_the_button_comes_back`.

## End to end, against real remotes

`crates/cairn-git/tests/fetch.rs` stands up the remotes the PRD's criteria
need, all std-only and all generating their credentials per run:
`tests/remotes/http.rs` is an HTTP/1.1 server in the test process fronting
`git http-backend` behind Basic auth, and `tests/remotes/ssh.rs` an
unprivileged `sshd` on a loopback port with a passphrase-protected key from
`ssh-keygen`, reached through the repository's `core.sshCommand` because ssh
reads its per-user configuration from the passwd home, not `$HOME`;
`tests/remotes/askpass.rs` answers a real `Channel` from a thread and locates
the built helper. The tests are the criteria:

- **B3** — `a_fetch_over_http_prompts_for_each_half_of_the_credential_and_succeeds`
  (username, then password, then the remote's history is there) and
  `a_fetch_over_ssh_prompts_for_the_key_passphrase_and_succeeds`;
  `an_unknown_host_key_is_confirmed_through_the_helper_before_the_passphrase`
  is the confirmation path.
- **B4** — `a_credential_helper_that_answers_means_no_prompt_at_all` and
  `an_agent_that_holds_the_key_means_no_prompt_at_all`. In both, the channel
  REFUSES everything it is asked and the test asserts the fetch succeeded, no
  prompt was seen, and (over HTTP) that the server saw a request authorised as
  the stub helper's user — so a Cairn that prompted anyway fails all three
  ways, and a fetch that skipped authentication fails the last.
- **B5** — `refusing_the_http_prompt_fails_the_fetch_cleanly_and_asks_nothing_again`
  and `refusing_the_passphrase_fails_the_ssh_fetch_cleanly`: one prompt, no
  second ask (the retired token turns a retry away at the channel and the
  test counts those too), a bounded wait, a message naming what could not be
  read, no secret in it, and no process left pointed at the socket.
- **R4.3** — `cancelling_a_fetch_that_is_waiting_on_a_prompt_kills_git_and_leaves_nothing_behind`.

The ssh cases skip by name, on stderr, where `sshd` cannot run; they never
pass silently.

## Building the helper

`cargo run -p cairn-app` builds only the application; the helper is a
separate binary, and cargo builds a dependency's library, never its binaries.
The worker looks for `cairn-askpass` beside its own executable
(`target/<profile>/`), where `cargo build --workspace` (or
`cargo build -p cairn-askpass`) puts it. Without it, the application still
opens and fetches wherever a credential helper or agent answers, and a fetch
that needed a prompt fails with a message naming the build command. The gate
and `cargo test --workspace` build it before any test runs; the tests that
need it fail with the same instruction when it is absent rather than building
it themselves, since this crate's tests may not run a `Command`.

## The invariant this leaves behind

"No credential value is logged, Debug-printed, serialised, or stored in
application state" — root `CLAUDE.md`, Invariants, with its twin
`no_credential_value_is_logged_printed_serialised_or_stored` and the review
obligations it states. The `every_git_invocation_disables_the_terminal_prompt`
guard also pins the askpass entries. Both guards read spellings: a type named
like a variant of another type (the channel's `Error::Answer` made a `Reply`
named `Answer` look like a container) is caught rather than missed, which is
the safe direction.

## Not here, and known limits

Not in the packet at all: push, clone, submodule credentials, proxies, GPG
signing, and any Cairn-owned credential storage, which D2 rules out
permanently. Not built: remembering anything between fetches (D2), a remote
picker (the button fetches the default remote), and a fetch of more than one
remote at a time.

Known limits, described rather than pinned:

- The typed characters live in Freya's `Input` — its editing rope and history
  and the `State<String>` behind it — until the dialog closes, and nothing
  zeroes those. The `String` handed out of the dialog is moved into the
  `Secret`, so from there on the value is in one zeroed buffer; before it, the
  toolkit's. A zeroing password editor is a Freya change, not a Cairn one.
- The helper has no read timeout: it waits as long as the user takes, and a
  Cairn that dies closes the socket, which ends the wait.
- `$XDG_RUNTIME_DIR` is required, so a session without one (macOS today) gets
  no channel and named failures rather than a fallback directory, a policy for
  the user to set.
- A cancel kills `git` and returns; its children (`ssh`, the helper) end on
  their own when the dialog's refusal releases them or their pipes break. The
  runner's reader thread lives until then.
- A stalled network is only interrupted by the cancel; nothing times a fetch
  out.
