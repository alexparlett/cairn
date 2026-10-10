# Credentials

How Cairn asks a user for a secret today, and what it does not do. As-built:
everything here is code that exists, with the test that pins each behaviour
named beside it. The commitment it was built against is
`docs/prd/credential-prompts.md` (shipped, frozen); the decisions are D1 and D2
in `docs/design/engine.md` and `docs/design/credentials.md` and the packet's locked decisions L1-L11, kept under
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
- **The environment** (`crates/cairn-git/src/process/environment.rs`, and
  `process/askpass.rs`; the environment's home, its read and write profiles
  and the seal that chooses between them are `docs/systems/git-processes.md`).
  `GitEnvironment::new(parent, &Askpass)` sets, on every invocation,
  `GIT_TERMINAL_PROMPT=0` and `SSH_ASKPASS_REQUIRE=force` (the `ALWAYS` table,
  which also pins the editor), `GIT_ASKPASS` and `SSH_ASKPASS` to the helper,
  and `CAIRN_ASKPASS_SOCKET` when the `Askpass` names a socket; there is no
  environment without an `Askpass`. `CAIRN_ASKPASS_TOKEN` is applied per
  invocation, from `GitCommand::authorized_by`, because the token is the
  invocation's — and only on a write: a read has nowhere to hold one. Pinned
  by `the_environment_is_exactly_the_deliberate_entries` (the whole set,
  spelled out), `a_write_is_the_base_with_its_token_only_when_given`, and the
  stub `git` that prints what it was given
  (`a_read_sees_exactly_the_read_environment_and_nothing_inherited` in
  `process/cli.rs`, `an_authorised_write_carries_its_token_and_only_that_one`
  in `ops/authority.rs`, `a_fetch_runs_with_the_write_environment_and_its_token`
  in `ops/fetch.rs`).
  `SSH_ASKPASS_REQUIRE=force` needs OpenSSH 8.4 (2020-09); on an older one the
  variable is ignored and a passphrase goes to the terminal Cairn was launched
  from, or fails closed without one (O5 addendum). By decision D2 (amended
  2026-09-17, issue #22): a `GIT_ASKPASS`, `SSH_ASKPASS` or `core.askPass`
  the user set for something else is replaced while Cairn runs git, since
  `GIT_ASKPASS` outranks `core.askPass` and the environment is never
  inherited; `credential.helper` and the agent, which L7 protects, are
  untouched. An "auth provider" setting letting the user pick their own
  askpass program over Cairn's dialog is the future on issue #22.
- **`ops::fetch`** (`crates/cairn-git/src/ops/fetch.rs`). `fetch(&git, &repo,
  remote, token)` starts `git fetch --progress --no-prune-tags
  --end-of-options <remote>` and returns a `FetchInProgress`; `finish(progress)` streams each redraw of git's
  progress meter to the callback and yields a `Performed` declaring `refs` and
  `objects` invalid; `canceller()` is a `Send` handle that ends the process
  from any thread without blocking that thread, after which `finish` reports
  `Error::GitCancelled`. **Not destructive, and takes no `Confirmed` on
  purpose**: a fetch moves only remote-tracking refs, in the reflog wherever
  the repository keeps one (a bare repository logs nothing by default, and
  the old tip's commits survive until `gc` either way); the module docs say
  so. Fetch is a write invocation on the runner every `git` runs on, so how
  its process is spawned, read, cancelled and reaped is
  `docs/systems/git-processes.md`, not restated here: git leads a process
  group of its own, so a cancel reaches `ssh`, `git-remote-https` and the
  helper with it; a cancel is `SIGTERM` to that group, then `SIGKILL` after
  `TERMINATION_GRACE` (two seconds), because git removes the lock files it
  holds on `SIGTERM` and cannot on `SIGKILL` (issue #19); and a clean exit
  that a cancel raced is reported as the success it was — where the runner
  saw it exit before the signal; one that exits in the microseconds between
  the last look and the `SIGTERM` is reported cancelled. A failed fetch
  names the lock files present under the git directory, as every failed
  write does (`a_failed_fetch_names_the_lock_files_present_and_only_those`
  in `tests/fetch.rs`). What fetch adds is only its arguments and its
  outcome. Pinned end to end by
  `crates/cairn-git/tests/fetch.rs` (below), every one of whose cancels is
  bounded under two seconds, which only a git that acted on the `SIGTERM`
  meets.

  **What a cancel finds is reported.** Once git is reaped, the runner lists,
  because a fetch is a write,
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
  rather than before is pinned on the runner, against a stub that takes
  300 ms to remove its lock (`a_cancelled_write_lists_its_locks_only_once_it_is_reaped`
  in `ops/authority.rs`), since no fixture can hold a real git mid ref-write
  at the instant of a cancel. The worker hands the paths on as `Update::FetchCancelled::stranded_locks`
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
  that does not exist yet (issue #19); a FAILED fetch names the lock files
  present in its error, which the banner does not yet draw (issue #44).
  "Not destructive" is kept true against the user's configuration
  by one flag and one refusal (issue #17, decided 2026-09-17). `fetch.prune`
  and `remote.<name>.prune` are honoured exactly as `git fetch` honours them
  — nothing is passed for prune, so git reads that configuration itself,
  fresh, and a remote-tracking ref the remote has already deleted goes; its
  commits survive until `gc`, and the ref was never local work. That is the
  one deletion a fetch performs, by the user's own setting. `--no-prune-tags`
  is always passed, so `fetch.pruneTags` and `remote.<name>.pruneTags` are
  ignored and no local tag is ever pruned (tags have no reflog); a user who
  set them gets stale tags left standing rather than removed without a word.
  Pinned with plain git as the control on every case, so a passing assertion
  is Cairn agreeing with git rather than a configuration nobody read:
  `fetch_prune_on_prunes_a_stale_remote_tracking_ref_as_git_does`,
  `fetch_prune_unset_keeps_a_stale_remote_tracking_ref_as_git_does`,
  `remote_prune_overrides_fetch_prune_both_ways_as_git_does` and
  `a_local_tag_survives_whatever_prune_tags_says` (where the control deletes
  the tag) in `tests/fetch.rs`; the arguments themselves by
  `the_arguments_leave_prune_to_git_forbid_pruning_tags_and_end_the_options`.
  What the flag cannot cover is a configured refspec that writes where a
  fetch from a button never may: `ops/refspec_policy.rs` asks git for the
  remote's configuration before the fetch starts (`reads::fetch_settings`:
  `git config --includes --null --type=bool --get <key>` for
  `remote.<name>.mirror`, `remote.<name>.prune` and `fetch.prune`, and
  `git config --includes --null --get-all remote.<name>.fetch` — the second
  porcelain read, query form only, accepted by the user on 2026-10-04; see
  `docs/systems/git-processes.md`, "What a read may run"). The same binary,
  the same built environment and the same `--git-dir`/`--work-tree` as the
  fetch, afresh on every fetch, so what the check decides on is what the
  fetch's own git reads: a refspec added in a terminal since Cairn started,
  a linked worktree's `includeIf "gitdir:..."` — which git evaluates against
  the worktree's own git directory and gix against the common directory
  (`the_refspec_check_reads_a_linked_worktrees_conditional_include_as_git_does`)
  — the system file at git's own `sysconfdir`, a repository whose git
  directory is another user's
  (`the_refspec_check_sees_the_remote_of_a_repository_gix_trusts_less_than_git`,
  which needs a user namespace with a second uid its root may give a file
  to; required by the gate wherever `unshare --map-root-user --map-auto
  chown 1:1` succeeds, `CAIRN_REQUIRE_SECOND_OWNER` — the user's decision,
  2026-10-04, that the probe tests exactly what the test needs. Residual:
  it does not run on GitHub's Ubuntu 24.04 runners, whose AppArmor
  `unprivileged_userns` profile maps the second uid and then refuses the
  namespace's root the `chown`; the gate's PASS line says it SKIPPED and
  why. It runs, and is required, wherever the probe passes), and never
  Cairn's own `GIT_CONFIG_*`, which the built environment does not carry
  (`the_refspec_check_ignores_config_from_cairns_own_environment`, over both
  `GIT_CONFIG_COUNT` and `GIT_CONFIG_GLOBAL`). git parses its own booleans,
  and the last value wins, as `remote.c` and `builtin/fetch.c` read them.
  The check FAILS CLOSED: a read that fails — no `git`, a value git will not
  parse as a boolean, a configuration file it cannot read, an answer that is
  not one — and a configured refspec that does not parse are
  `Error::RemoteConfig`, and no fetch starts
  (`an_unreadable_remote_configuration_is_reported_and_starts_nothing`;
  `a_failed_or_cancelled_read_is_an_error_never_an_unset_key` in
  `reads/config.rs`), except that a read ended because the
  repository is closing is the fetch cancelled before it started,
  `Error::GitCancelled` with no lock files, as a close of a running fetch
  reports it (`a_fetch_closed_as_it_starts_is_still_ended`, in `cairn-app`);
  its reads write nothing
  (`the_refspec_checks_reads_write_nothing`). It refuses, as
  `Error::FetchRefused` quoting the setting, a remote with
  `remote.<name>.mirror` (a push setting in git, refused on sight as what a
  mirror clone carries, and said so), a `+refs/*:refs/*` or any
  `refs/heads/` destination (local branches overwritten, with no reflog in a
  bare repository) — and, while pruning is on, one whose own refspecs write
  `refs/tags/`, since `--no-prune-tags` withholds only the tag refspec git
  would add; that refusal names the refspec and the prune setting that
  decided, `remote.<name>.prune` over `fetch.prune` as git reads them
  (`fetch.pruneTags` and `remote.<name>.pruneTags` are not read:
  `--no-prune-tags` overrides both). A remote git would define from a file
  rather than configuration — `$GIT_DIR/remotes/<name>` or
  `$GIT_DIR/branches/<name>`, which no query prints and whose `branches/`
  form fetches into `refs/heads/<name>` — is refused on sight with the file
  quoted, whatever the configuration also says
  (`a_remote_defined_by_a_file_git_reads_in_place_of_configuration_is_refused`).
  An unqualified destination is read as git's `get_local_ref` reads it
  (`heads/`, `tags/`, `remotes/` get `refs/` in front, any other name is a
  branch, an unqualified glob writes nothing), checked against git 2.55.
  Pinned by `a_refspec_that_writes_local_branches_is_refused_before_git_runs`
  (opened before the refspec is written) and
  `a_tag_refspec_is_refused_under_prune_and_fetched_without_it`, both over a
  recording stub `git` that hands `config` to the real git and would have
  said so had a fetch been started
  (`the_recording_stub_reports_a_fetch_that_was_allowed_to_start` is the
  control); the destination reading and the decision over what git read are
  the unit tests in `refspec_policy.rs`. An empty configured refspec
  (`fetch =`) is git's `HEAD` with no destination, which fetches into
  `FETCH_HEAD` and writes no ref (git 2.30.9 and 2.56.0 alike), so it is
  neither refused nor unparsed
  (`an_empty_refspec_is_fetched_into_fetch_head_as_git_fetches_it`, with
  plain git as the control; `an_empty_refspec_writes_nothing_and_is_not_refused`).
  Two residuals of the check's shape: its four `git config` reads and the
  fetch are separate processes, so a configuration write landing in the
  milliseconds between them can have the check decide on one generation of
  the configuration and the fetch act on the next; and the user's own
  cancel of a fetch cannot stop the four reads, which run before the fetch
  has a process for it to kill (their cancel signal is held by nobody) —
  closing the repository does end them, as the fetch cancelled before it
  started. The
  refusal reaches the window as a failed fetch whose one line is the
  engine's sentence. Pruning that says what will go, as a confirmed
  operation, and a setting for it, are issue #17's remainder.
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
  the contract: the window asks for a refresh when the fetch ends, which reads
  the refs again and reopens the history when what it draws changed
  (`docs/systems/history-graph.md`, "Refresh"), and nothing reads the
  `Performed` itself (issue #25). Pinned by `every_single_flag_counts_as_something`,
  `declarations_combine_without_losing_a_flag`,
  `a_destructive_operation_records_what_the_user_agreed_to` and
  `an_unconfirmed_operation_carries_no_prompt`.
- **`Repository::remotes`** (`crates/cairn-git/src/remotes.rs`). The
  configured remotes as `cairn_model::RemoteSummary`, the default first, read
  through gitoxide, a password embedded in a URL left out and a remote without
  a URL listed without one; what the window's fetch button names
  (`the_default_is_first_a_password_is_left_out_and_a_missing_url_is_none`).
  The refresh after a fetch reads the refs snapshot through the handle the
  worker already holds (`Repository::refs`, `docs/systems/refs.md`); that it
  follows what git writes — a ref made, a ref moved — is
  `a_refs_read_follows_the_refs_git_writes` in `tests/fetch.rs`, over a
  fixture, since a CI checkout is detached with no local branch and its own
  refs decide nothing.
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
- **The application** (`crates/cairn-app/src/worker/`). Several threads per open
  repository, each with its own sender, so the update stream ends only when all
  have gone. `git` itself is found once per application, as it starts
  (`discovery.rs`; `docs/systems/git-processes.md`). `startup.rs`: on the
  repository thread, once the repository is open, the channel is opened under
  `$XDG_RUNTIME_DIR` and that `git` pointed at its socket
  (`GitBinary::with_environment`) — a missing runtime directory or a helper that is not
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
  dropping it. `network_lane.rs`: the network lane runs the fetch with a
  token from `Channel::begin`, forwards every progress line as
  `Update::FetchProgress`, and ends with `FetchFinished`, `FetchCancelled` or
  `FetchFailed`; on every one the window asks for a refresh, since a fetch that
  failed or was killed may have moved some refs, and the refresh decides whether
  the history reopens (`a_fetch_reports_progress_finishes_and_the_refresh_after_it_reopens_from_the_new_refs`,
  `every_fetch_ending_asks_for_a_refresh_and_touches_no_row`). `Request::CancelFetch` never
  queues: the handle reaches the fetch's `FetchControl` directly (one fetch
  at a time, a second refused with a reason the window draws; a cancel that
  lands before git runs is kept and applied the
  moment it does; `FetchStarted` goes out only once the kill handle is
  installed). Operations carry no epoch, so a scroll cannot supersede a fetch
  nor a fetch a scroll. The token is retired before the outcome goes out, so
  a helper orphaned by a killed git is refused at the channel rather than
  accepted afterwards. On shutdown — the window's `Request::Close`, or every
  handle gone — the repository thread closes the network lane's queue, ends
  and reaps every `git` running in the repository through its registry
  (`docs/systems/git-processes.md`, "Closing"), and wakes the
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
  helper — unless a local write runs, whose prompt it may be; a prompt arriving
  with neither a fetch nor a local write in flight is refused rather than
  shown (a local write's prompts: `docs/systems/git-processes.md`, "The local
  write lane"). The update task holds the answering end weakly, so the window's last
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
- A cancel ends `git` and its process group (`SIGTERM`, then `SIGKILL`
  after two seconds) — `ssh` and the helper with it — and returns once the
  group is reaped (`docs/systems/git-processes.md`). A `SIGKILL` that lands while git is updating
  refs can still leave a `*.lock`; the cancel names every lock file it
  finds, and removing one is the user's by hand once they know no other git
  is running — nothing in Cairn removes a lock yet, since the process
  holding it might not be Cairn's (issue #19). A failed fetch names the lock
  files present in its error (`GitFailed::present_locks`), but the banner
  draws only git's first `fatal:`/`error:` line, so the list does not reach
  the user (issue #44).
- Closing the window closes the repository first: every `git` in it is
  ended and reaped, the acceptor stopped and the socket directory removed,
  and only then does the window go (`docs/systems/git-processes.md`,
  "Closing"). Checked by hand on a desktop session with the credential
  dialog up: no `git`, no helper, no socket directory and no lock left. A
  window forced closed after `worker::CLOSE_PATIENCE`, or a crash, can
  still leave the directory under `$XDG_RUNTIME_DIR`: a runtime directory is
  a tmpfs cleared at logout, and the names are per pid and random, so
  nothing collides.
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
  and all pinned by `the_environment_is_exactly_the_deliberate_entries`. Of
  what #18 left open, staging-and-commit decided the rest: `GNUPGHOME`,
  `DISPLAY`, `WAYLAND_DISPLAY` and `XAUTHORITY` joined the roster for a signing
  pinentry, beside the identity variables (L11, L26; `docs/systems/git-processes.md`,
  "The environment"), and `GIT_EDITOR` is pinned to `false`. Issue
  #22 decided a user-set askpass program is not an exception (D2, amended),
  and holds the future setting that would make one.
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
- Three threads per repository (repository, network lane, acceptor): a fetch
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
  `crates/cairn-git/src/process/stub_git.rs`) retry on `ETXTBSY`: a fork in a
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
