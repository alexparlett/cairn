# Credentials

How Cairn asks a user for a secret today, and what it does not do. As-built:
everything here is code that exists, with the test that pins each behaviour
named beside it. The commitment it was built against is
`docs/prd/credential-prompts.md` (shipped, frozen); the decisions are D1 and D2
in `docs/design/cairn.md` and the packet's locked decisions L1-L11, kept under
"Decisions the packet locked" below now that its work directory is gone; the
evidence is `docs/research/credential-prompts/git-credential-delegation.md`,
whose 2026-09-17 addenda settle the two questions the design left open (O4,
O5) and record what `zeroize` actually does on drop.

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
  through volatile writes); the evidence record's third addendum records that
  reading.
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
  remote, token)` starts `git fetch --progress --no-prune --no-prune-tags
  --end-of-options <remote>` and returns a `FetchInProgress`; `finish(progress)` streams each redraw of git's
  progress meter to the callback and yields a `Performed` declaring `refs` and
  `objects` invalid; `canceller()` is a `Send` handle that ends the process
  from any thread without blocking that thread, after which `finish` reports
  `Error::GitCancelled`. **Not destructive, and takes no `Confirmed` on
  purpose**: a fetch moves only remote-tracking refs, all in the reflog; the
  module docs say so. The runner underneath (`GitCommand::stream` in
  `ops/cli.rs`) reads the pipe on a thread of its own, because git's children
  — `ssh`, `git-remote-https`, the helper — inherit it and outlive a killed
  git; a cancel returns once git itself is reaped, and the reader ends when
  the last child lets the pipe go. Pinned by the stub-git tests
  `a_streamed_invocation_hands_stderr_on_a_redraw_at_a_time` and
  `a_kill_from_another_thread_ends_a_hung_invocation_and_reaps_it`, and end
  to end by `crates/cairn-git/tests/fetch.rs` (below). A clean exit is
  reported as the success it was even when a cancel raced it
  (`a_kill_after_a_clean_exit_reports_the_success`).

  **A cancel is `SIGTERM` first** (issue #19), sent through `nix`'s safe
  `kill(2)` wrapper — the one thing that crate is linked for — because git
  removes the lock files it holds on `SIGTERM` and cannot on `SIGKILL`, which
  is all `std` can send. The thread waiting in `finish` polls the process
  every `EXIT_POLL` and escalates to `SIGKILL` once `TERMINATION_GRACE` (two
  seconds: git's handler exits in milliseconds, so that is margin for a
  loaded machine, and short enough that "cancelled" still arrives while the
  user is looking) has passed with git still running, from whichever loop
  it is waiting in; the same poll sends the `SIGTERM` itself if the
  cancelling thread could not take the lock, so a cancel is never lost, and
  no signal goes to a child that has already been reaped, whose pid the
  system may have handed on. Pinned over stub gits that report the signal
  they got: `a_cancel_sends_sigterm_first_and_a_process_that_exits_on_it_is_not_killed`
  (a `SIGKILL` runs no trap, so "terminated" on stderr is `SIGTERM` and
  nothing else, and the wait ends inside the grace period),
  `a_process_that_ignores_sigterm_is_killed_once_the_grace_period_has_passed`
  (the stub outlives the grace period, so it ignored the first signal, under
  a deadline so a runner that never escalates fails rather than hangs),
  `a_cancel_that_lands_after_stderr_closed_is_still_escalated_to_sigkill`,
  `a_kill_that_misses_the_lock_is_finished_by_the_waiter` and
  `a_cancel_after_the_reap_signals_nothing`; and over real git by the
  under-two-seconds bound on every end-to-end cancel, which only a git that
  acted on the `SIGTERM` meets.

  **What a cancel finds is reported.** Once git is reaped, `finish` lists
  every `*.lock` under the git directory and, for a linked worktree, the
  common directory — the top level, all of `refs/`, and the three places
  under `objects/` where git locks a file it rewrites whole (the multi-pack
  index, the commit graph and its chain, which `fetch.writeCommitGraph`
  takes), not the packs themselves (`ops/stranded_locks.rs`, pinned by
  `every_lock_file_git_could_strand_is_found_and_nothing_else` and
  `a_linked_worktree_is_searched_in_both_of_its_directories`) — and carries
  the paths on `Error::GitCancelled::stranded_locks`; the message names them
  (`a_cancellation_names_what_it_stranded_and_is_silent_when_nothing_was`).
  A listing, not a verdict: it cannot tell a lock this cancel stranded from
  one left by an earlier crash or one a git in a terminal holds this
  instant, so the words around the paths say what was found and that
  removing one is safe only when no other git is running there. End to end,
  `a_cancel_names_the_lock_files_left_under_the_git_directory_and_only_those`
  in `tests/fetch.rs` cancels a real git hung on a remote that never answers,
  with locks planted as a crash would leave them, and reads back exactly
  those — then none once they are gone; that the search runs after the reap
  rather than before is stated in `FetchInProgress::finish` and not pinned,
  since no fixture can hold a real git mid ref-write at the instant of a
  cancel. The worker hands the paths on as `Update::FetchCancelled::stranded_locks`
  (`a_cancelled_fetch_carries_the_lock_files_it_stranded`, and the planted
  lock in `cancelling_a_fetch_that_waits_on_a_prompt_ends_it_as_cancelled`),
  the session into `FetchStatus::Cancelled`
  (`a_cancelled_fetch_hands_the_lock_files_it_found_to_the_banner`), and the
  banner names every one with when acting on it is safe
  (`a_cancel_that_left_a_lock_behind_names_every_lock_file` in
  `crates/cairn-app/src/status_text.rs`). The press itself is said at once:
  Cancel puts the view in `FetchStatus::Cancelling` and takes the button
  away, since the worker's answer may be the whole grace period away, and a
  `FetchStarted` that lands after the press does not undo it
  (`a_cancel_is_said_at_once_and_survives_a_late_start` in
  `crates/cairn-app/src/fetch_state.rs`, and the window test
  `the_fetch_button_fetches_the_default_remote_and_becomes_cancel_while_running`). Listing only: removing a lock
  another process may still hold is a decision for a confirmed operation
  that does not exist yet, and a FAILED fetch whose stderr says `cannot lock
  ref` is not yet read for the lock it names — both remain on issue #19.
  "Not destructive" is kept true against the user's configuration
  by the two `--no-prune` flags: `fetch.prune`, `fetch.pruneTags` and their
  `remote.<name>.*` forms would have a plain fetch delete remote-tracking refs
  and even local tags, and Cairn's never does — a user who set them gets
  stale refs left standing rather than removed without a word, and pruning
  as a named operation is issue #17's remainder. Pinned by
  `a_fetch_never_prunes_however_the_repository_is_configured`
  (`tests/fetch.rs`, with plain git as the control that the configuration
  would have pruned) and, for the arguments themselves,
  `the_arguments_forbid_pruning_and_end_the_options_before_the_remote` (which
  is the only pin on `--no-prune-tags`: git prunes tags only when pruning at
  all, so `--no-prune` alone decides the behaviour and the second flag is
  belt and braces, as the module docs say). What the flags cannot cover,
  stated in the module docs and left on issue #17, is a configured refspec
  that overwrites a local ref: a destination under `refs/heads/*` (a
  `--mirror` clone, or a bare repository taking the remote's branches as its
  own, where there is no reflog by default), or a forced tag refspec in any
  clone, since git never reflogs a tag. The operation inspects no refspec.
- **The cache-invalidation contract** (`crates/cairn-git/src/ops/mod.rs`,
  module docs; `ops::Invalidated`, `ops::Performed`). D1 puts two
  implementations of git semantics in one process, so after a `git` subprocess
  writes, the gitoxide handle the worker holds may be looking at a repository
  that no longer exists. Every operation therefore declares what it
  invalidated — `refs`, `index`, `objects`, `working_tree` — on the
  `Performed` it returns, and the module docs state, per flag and checked
  against gix 0.87.1 as linked, what a worker must do to honour it and the
  same-timestamp-tick residual gix cannot see. Fetch declares `refs` and
  `objects`. What the worker does with a declaration today is narrower than
  the contract: it decides whether to reload the history by comparing
  `ref_tips` before and after, and reads nothing else off the `Performed`
  (issue #25). Pinned by `every_single_flag_counts_as_something`,
  `declarations_combine_without_losing_a_flag`,
  `a_destructive_operation_records_what_the_user_agreed_to` and
  `an_unconfirmed_operation_carries_no_prompt`.
- **`Repository::remotes`** (`crates/cairn-git/src/remotes.rs`). The
  configured remotes as `cairn_model::RemoteSummary`, the default first, read
  through gitoxide, a password embedded in a URL left out and a remote without
  a URL listed without one; what the window's fetch button names
  (`the_default_is_first_a_password_is_left_out_and_a_missing_url_is_none`).
  **`Repository::ref_tips`** (`src/refs.rs`) is every ref's id as the handle
  sees it now, compared before and after a fetch; that it follows what git
  writes — a ref made, a ref moved — is `ref_tips_follow_the_refs_git_writes`
  in `tests/fetch.rs`, over a fixture, since a CI checkout is detached with
  no local branch and its own refs decide nothing.
- **The dialog** (`crates/cairn-ui/src/credential_prompt.rs`,
  `CredentialPrompt`). Names the remote the running operation was asked for,
  states what is wanted from where, and shows the prompt exactly as git or ssh
  gave it — the URL in it is the one being authenticated against. A username
  is typed in the open, a password or passphrase masked, and ssh's host-key
  check is a question with "Yes, connect" and no text field, answered `yes` —
  beside a sentence (`ACCEPTING_IS_PERMANENT`) saying that accepting writes
  the key to `known_hosts` and ssh will not ask about that host again, since
  the question itself does not say what a yes costs.
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
  `Update::FetchProgress`, and ends with `FetchFinished`, `FetchCancelled` or
  `FetchFailed`, each carrying `refreshed` — whether a ref moved, told by
  comparing `ref_tips` before and after, on every outcome, since a fetch that
  failed or was killed may have moved some. `Request::CancelFetch` never
  queues: the handle reaches the fetch's `FetchControl` directly (one fetch
  at a time; a cancel that lands before git runs is kept and applied the
  moment it does; `FetchStarted` goes out only once the kill handle is
  installed). Operations carry no epoch, so a scroll cannot supersede a fetch
  nor a fetch a scroll. The token is retired before the outcome goes out, so
  a helper orphaned by a killed git is refused at the channel rather than
  accepted afterwards. On shutdown the
  repository thread kills any fetch, closes the operations queue and wakes the
  acceptor by connecting to its own socket (the one thing that returns a
  blocking `accept`), on a clean exit and on unwinding alike; a prompt still
  waiting is refused. The wake is retried until the acceptor acknowledges it
  has left its loop, or until `STOP_DEADLINE` passes when it cannot — it is
  held on the window's answer to a prompt until that answering end goes, and
  acknowledges on the way out, never on the way in — so a connection that
  fails to land does not leave the thread blocked with nothing coming
  (`AcceptorStop::stop`, `askpass.rs`; pinned by
  `a_stop_returns_only_once_the_acceptor_has_left_its_loop`,
  `a_stop_gives_up_on_an_acceptor_held_by_a_prompt_and_is_acknowledged_once_it_leaves`
  and
  `a_stop_is_visible_from_a_clone_and_gives_up_on_a_socket_nobody_listens_on`).
  Stated rather than pinned, since a thread that fails to spawn cannot be
  injected: when the acceptor thread cannot be started, `Threads::start`
  acknowledges the stop itself, so a shutdown does not wait the deadline on
  a loop nobody entered — a review obligation for whoever touches that arm.
  Pinned by `crates/cairn-app/src/worker/fetch_tests.rs`
  against real git and the built helper, with a loopback remote that answers
  `401` to everything:
  `a_fetch_reports_progress_finishes_and_the_history_reloads_from_the_new_refs`
  (the cache contract's first real exercise: the worker's gix handle serves the
  commits a `git` subprocess just wrote; the remote is the Cairn checkout's
  `HEAD`, fetched into a bare fixture's `main`, because a CI checkout has no
  `main` and may have no branch at all),
  `a_prompt_reaches_the_window_as_a_value_and_its_answer_reaches_git`,
  `refusing_a_prompt_fails_the_fetch_once_and_the_worker_carries_on`,
  `cancelling_a_fetch_that_waits_on_a_prompt_ends_it_as_cancelled`,
  `letting_go_of_the_repository_ends_the_acceptor_and_removes_its_socket` and
  `a_prompt_waiting_at_shutdown_is_refused`.
- **The window** (`crates/cairn-app/src/window.rs`, `session.rs`, `main.rs`,
  `fetch_state.rs`). A "Fetch <remote>" button for the default remote, gone
  from the press itself (`FetchStatus::Starting`) and "Cancel" until the fetch
  ends; a one-line banner with git's latest progress or the outcome — for a
  failure, git's first `fatal:` line, else its first `error:` line, else the
  message's first line, because a fetch's stderr is progress first and the
  reason after it (`status_text::why_it_failed`, pinned by
  `a_failure_says_why_it_failed_and_not_how_far_git_got`); and the dialog
  whenever a `PromptView` is set,
  answered through the worker's `Replier` callback and taken down.
  `session::apply` is where an `Update` becomes view state: a fetch that moved
  a ref clears the rows, resets the progress and asks for the history from
  `HEAD` again (one that moved nothing leaves the reader's place alone); a
  fetch ending with a dialog still up refuses that prompt, which releases the
  helper; a prompt arriving with no fetch in flight is refused rather than
  shown. The update task holds the answering end weakly, so the window's last
  reference is what lets the acceptor go. Pinned by
  `a_prompt_draws_the_dialog_and_its_answer_leaves_as_a_secret_for_that_prompt`,
  `cancelling_the_dialog_refuses_that_prompt`,
  `the_fetch_button_fetches_the_default_remote_and_becomes_cancel_while_running`,
  `a_failed_fetch_is_said_in_the_window_and_the_button_comes_back`, and in
  `session.rs`
  `a_fetch_that_moved_refs_clears_the_rows_and_asks_for_the_history_again`,
  `a_fetch_that_moved_nothing_leaves_the_rows_alone`,
  `a_cancelled_or_failed_fetch_that_moved_refs_reloads_too`,
  `a_fetch_ending_takes_the_dialog_down_and_refuses_its_prompt` and
  `a_prompt_with_no_fetch_in_flight_is_refused_not_shown`.

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

The ssh cases skip where `sshd` cannot run, with the reason on stderr —
which `cargo test` hides for a passing test, so a skip would read `ok`.
`CAIRN_REQUIRE_SSH_FIXTURE` set in the environment turns the skip into a
failure, and it is set where the criteria must decide: CI installs
`openssh-server` and sets it unconditionally (`.github/workflows/ci.yml`),
and `scripts/gate.sh`'s `test-full` step sets it wherever an `sshd` can be
found (on `PATH`, `/usr/sbin` or `/usr/local/sbin` — the fixture looks in the
same places) and otherwise says that the criteria skipped here, under the
step and again on the `gate: PASS` line (issue #20). That CI sets it, that
the gate's step probes, and that the two look in the same directories is
pinned by `the_ssh_criteria_are_required_wherever_they_can_run` in
`crates/cairn-guards/tests/invariants.rs`. The fixture needs no privilege and no system service: it starts
its own `sshd` on a loopback port with keys it generates, and kills it after.

## Building the helper

`cargo run -p cairn-app` builds only the application; the helper is a
separate binary, and cargo builds a dependency's library, never its binaries.
The worker looks for `cairn-askpass` beside its own executable
(`target/<profile>/`), where `cargo build --workspace` (or
`cargo build -p cairn-askpass`) puts it. Without it, the application still
opens and fetches wherever a credential helper or agent answers, and a fetch
that needed a prompt fails with a message naming the build command. The gate
and `cargo test --workspace` build it before any test runs. `cairn-app`'s
tests fail with the same instruction when it is absent rather than building
it, since that crate's tests may not run a `Command`; `cairn-git`'s fetch
tests build it themselves (`tests/remotes/askpass.rs`, `--release` when the
profile directory is).

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

Not in the packet at all: push (issue #16), clone, submodule credentials,
GPG signing, and any Cairn-owned credential storage, which D2 rules out
permanently. Proxy, CA-bundle and Kerberos variables are inherited since
issue #18 decided them (below, under L5); what that issue still holds open
is the display and signing variables and pinning `GIT_EDITOR`. Not built:
remembering anything between fetches (D2), a remote picker (the button
fetches the default remote; issue #23), and a fetch of more than one remote
at a time.

Known limits, described rather than pinned:

- The typed characters live in Freya's `Input` — its editing rope and history
  and the `State<String>` behind it — until the dialog closes, and nothing
  zeroes those. The `String` handed out of the dialog is moved into the
  `Secret`, so from there on the value is in one zeroed buffer; before it, the
  toolkit's. A zeroing password editor is a Freya change, not a Cairn one
  (tracked with the other hygiene leftovers in issue #26).
- The helper has no read timeout: it waits as long as the user takes, and a
  Cairn that dies closes the socket, which ends the wait.
- `$XDG_RUNTIME_DIR` is required, so a session without one (macOS today) gets
  no channel and named failures rather than a fallback directory, a policy for
  the user to set (issue #24).
- A cancel ends `git` (`SIGTERM`, then `SIGKILL` after two seconds) and
  returns; its children (`ssh`, the helper) end on their own when the
  dialog's refusal releases them or their pipes break. The runner's reader
  thread lives until then. A `SIGKILL` that lands while git is updating
  refs can still leave a `*.lock`; the cancel names every lock file it
  finds, and removing one is the user's by hand once they know no other git
  is running — nothing in Cairn removes a lock yet, since the process
  holding it might not be Cairn's, and a failed fetch that trips over one
  does not yet name it (issue #19).
- On process exit (the window closing) the socket directory under
  `$XDG_RUNTIME_DIR` is left behind if the threads did not get to unwind: a
  runtime directory is a tmpfs cleared at logout, and the names are per pid
  and random, so nothing collides. Not probed with a display (issue #27).
- A stalled network is only interrupted by the cancel; nothing times a fetch
  out.
- What a large fetch costs the window — one update per progress redraw, a
  synchronous clear of the rows on a refresh, two whole-ref walks per fetch —
  is bounded by nothing yet (issue #25).

## Decisions the packet locked

The packet's brainstorm and progress log were deleted at teardown (git has
them); these are the decisions from them that a later change on this backend
must not reopen by accident, each with the reason that locked it.

- **L1** Cairn implements no authentication and stores no credential; the
  user's `credential.helper` and ssh-agent already do (D2). Rejected: a Cairn
  keychain integration — a second place for secrets to leak that breaks a
  setup that already works.
- **L2** The askpass helper is a separate binary, because git's contract is a
  process (prompt on `argv`, secret on stdout); the upside is that the secret
  never enters the main process's address space.
- **L3** `GIT_TERMINAL_PROMPT=0` on every invocation: the difference between
  "authentication failed" and "the app froze".
- **L4** The subprocess backend and the helper landed together: a helper with
  no verb to serve is untestable.
- **L5** The environment handed to `git` is built explicitly, never inherited
  wholesale; every passed variable is a deliberate entry with its reason
  beside it in `environment.rs`. Rejected: passing the parent environment
  through and overriding a few keys. Issue #18 decided the transport entries
  the packet left undecided — the proxy variables libcurl reads
  (`http_proxy`, `https_proxy`/`HTTPS_PROXY`, `all_proxy`/`ALL_PROXY`,
  `no_proxy`/`NO_PROXY`; not `HTTP_PROXY`, which curl ignores), the CA bundle
  (OpenSSL's `SSL_CERT_FILE` and `SSL_CERT_DIR`, and git's own
  `GIT_SSL_CAINFO`, `GIT_SSL_CAPATH`, the only `GIT_*` names on the roster;
  not `CURL_CA_BUNDLE`, which the curl tool reads and libcurl does not)
  and Kerberos (`KRB5CCNAME`, `KRB5_CONFIG`) — each with its reason beside it
  and all pinned by `the_environment_is_exactly_the_deliberate_entries`. Still
  open on #18: `DISPLAY`/`WAYLAND_DISPLAY`, `GNUPGHOME`, and pinning
  `GIT_EDITOR` to fail closed when a verb that opens an editor lands. Issue
  #22 is whether a user-set askpass program is an exception.
- **L6** A secret never travels on `argv` (`/proc` makes it world-readable);
  the socket and token reach the helper through its environment.
- **L7** A working setup is not degraded — a user with libsecret, osxkeychain
  or a loaded agent sees no new dialog — and B4 is the regression test that
  protects it, resting on git's own precedence (O4 addendum), not a Cairn
  workaround.
- **L8** Push is not in this packet: destructive, needs `Confirmed` and the
  `destructive-ops-reviewer` (issue #16).
- **L9** Minimum git is 2.30 (`GitVersion::MINIMUM`): Debian bullseye's, a
  support policy and so the user's to raise. Rejected: 2.11 (the technical
  floor, tested forever for no user) and 2.45 (excludes bookworm).
- **L10** The channel is a unix socket under `$XDG_RUNTIME_DIR`, `0700`
  directory and `0600` socket, not the abstract namespace, and its threat
  model is stated at the strength it holds: other users, not same-user
  processes. Rejected: `SO_PEERCRED` (defeatable by a determined same-user
  process, complexity for a boundary that cannot hold) and an inherited
  descriptor (depends on git preserving fds across the askpass invocation,
  unverified).
- **L11** `zeroize` is the accepted dependency for the secret type, because
  hand-rolled zeroing can be optimised away and a type doing it by hand would
  claim a protection it may not provide.

Taken inside the phases, none reopening the above:

- The token is scoped to one git invocation, not one ask: an HTTPS credential
  is two asks with one environment, and the token dies when the `Operation`
  drops or a prompt under it is refused.
- `Secret` and `zeroize` live in `cairn-model`, because the secret crosses
  from the dialog to the worker, which is what that crate is for; the
  alternative had a render crate depending on a crate that opens sockets.
- The helper crate is a library and a binary so both ends of one wire format
  cannot drift; it has no `thiserror`, keeping the linked surface at
  `cairn-model` and `zeroize`. Its tokens are 32 bytes of `/dev/urandom` as
  hex rather than a `rand` dependency.
- `Askpass` is a required input to `GitEnvironment::new`, with the socket
  optional, so nothing ever falls back to a tty; a Cairn with no channel fails
  every prompt closed. `GitEnvironment::new` takes a lookup closure rather than
  reading the process environment, because `std::env::set_var` is `unsafe` in
  the 2024 edition and `unsafe` is forbidden, so a test could set nothing.
- Locale variables are inherited because git's stderr is shown verbatim;
  proxy, CA-bundle and Kerberos variables because their absence is a
  "cannot connect" nobody can diagnose from Cairn (issue #18).
- The startup check refuses to open a repository with no usable `git` rather
  than carrying on read-only: a Cairn that cannot say why its write buttons
  are missing is the silent degradation D1 forbids.
- `Invalidated` lives in `cairn-git::ops`, not `cairn-model`: only the worker
  reads it, and it describes the engine's own caches.
- Three threads per repository (repository, operations, acceptor): a fetch
  blocked on the helper blocked on the acceptor would deadlock on one thread.
  Operations carry no epoch.
- The secret crosses the worker boundary as `worker::Reply` over its own
  channel, handed out as a bare `Rc<dyn Fn(Reply)>` so no struct holds it;
  named `Reply` because the credential guard reads spellings and the
  channel's `Error::Answer` made a type named `Answer` a container.
- The dialog hands out a `String`, not a `Secret`: an `EventHandler<Secret>`
  field would make the component a guarded holder.
- A missing helper or runtime directory is a reason, not a refusal: fetch
  still runs wherever a helper or agent answers (L7), and a failure says why
  nothing could have asked.
- The helper has no read timeout, and `$XDG_RUNTIME_DIR` has no fallback
  (issue #24).

## Lessons for the next change on this backend

Things the packet learned the expensive way, kept here because the only other
record was the deleted progress log.

- Test fixtures must generate their credentials, never contain them; the
  packet's history was scanned at the merge bar and nothing was ever
  committed.
- The stub-`git` tests (`crates/cairn-git/tests/git_binary.rs`,
  `crates/cairn-git/src/ops/stub_git.rs`) retry on `ETXTBSY`: a fork in a
  parallel test inherits a still-open write descriptor to a stub for
  microseconds. The stub exists twice because the runner is crate-private.
- A test that mutates a committed file to see a guard go red must snapshot
  the file and restore by copy: `git checkout <file>` restores the INDEX
  version and silently discards uncommitted work.
- `cargo test --workspace --all-targets` never runs doctests; the gate's
  `test-doc` step does, and stable `rustdoc` checks that a `compile_fail`
  block fails, not why, so each such block in `secret.rs` differs from a
  passing twin by exactly one line.
- Tests in `cairn-app/src` may not name `Command` (the terminal-prompt guard
  scans the crate's `src/`, test modules included), so fixtures there are
  `std::fs` repositories and a loopback `TcpListener`; tests that need a real
  remote live in `crates/cairn-git/tests/`.
- `ssh` reads its per-user configuration from the passwd home directory,
  never `$HOME`, so the ssh fixture hands its configuration over through
  `core.sshCommand`, and `sshd` must be invoked by absolute path.
- The credential guard reads spellings and errs toward catching: a struct
  field naming a type that names `Secret` is a holder wherever it is, and a
  type sharing its name with a variant of another type is caught too. Name
  new secret-carrying types distinctly.
- Clippy's `allow-expect-in-tests` does not cover helper functions in
  `tests/*.rs` outside a `#[test]` fn.
