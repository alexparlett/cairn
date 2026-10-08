# The write path as built, and what a local write lane needs

- **Date:** 2026-10-07
- **Commissioned by:** staging-and-commit planning
- **Read against:** `feature/plan-staging-and-commit` at `36c0d5c` ("refs-and-status: walk and
  label every ref, and list what git status says, as Fork draws it (#83)"), which is
  `origin/main`.
- **Method:** read in full `crates/cairn-git/src/ops/{mod,authority,fetch,stranded_locks}.rs`,
  `crates/cairn-model/src/{confirm,command_log}.rs`, `crates/cairn-git/src/process/{mod,cli,registry,command_log}.rs`
  and the parts of `process/runner.rs`, `process/pipes.rs`, `process/group.rs` and
  `process/environment.rs` that build, feed, end and report an invocation;
  `crates/cairn-app/src/worker/{mod,network_lane,routing,refresh_lane}.rs`, the thread and
  submit half of `worker/pool.rs`, `session.rs`'s `apply`, `refresh.rs`, `closing.rs`,
  `fetch_state.rs` and `local_changes_state.rs`'s `status_arrived`; the guard
  `destructive_operations_are_sealed_behind_the_confirmation_token`
  (`crates/cairn-guards/tests/invariants.rs`) and the gitoxide-mutation rosters in
  `crates/cairn-guards/src/lib.rs`; `.claude/agents/destructive-ops-reviewer.md`. Docs:
  `docs/design/{concurrency,processes,engine}.md`, `docs/systems/git-processes.md`
  ("Outcomes", "The registry", "The command log", "The network lane", "Closing"),
  `docs/systems/credentials.md` (the token), `docs/systems/local-changes.md`,
  `docs/prd/process-manager.md` (R7, Out of scope), `docs/work/daily-loop/{roadmap,brainstorm}.md`
  (packet 5's brief, L2–L5, O3), and `docs/research/process-manager/consumer-invocations.md`
  (the per-verb lock and invalidation table, rows P1–P13). Ran `gh issue view` for #45,
  #44, #19, #41, #18, #46, #48, #16, and for #47, #43, #66, #65, #62, which turned up as
  adjacent. Ran four small probes against real git 2.56.0 and the floor build 2.30.9
  (`~/.cache/cairn/git-floor/git-2.30.9`) in a scratch directory with `HOME` pointed at it
  (section 9). Nothing in the repository was changed but this file.
- **Conventions:** anchors are paths and symbols; line numbers appear only as evidence. A
  statement marked **(i)** is inferred from the code or from git's documented behaviour, not
  read in source or reproduced.

---

## 1. Summary

The write path exists end to end for exactly one verb, `ops::fetch`, and almost every piece
a local write needs is already in the engine: the write seal (`WriteAuthority`), the write
environment, a runner that feeds stdin on its own thread and closes it, streams stderr line
by line, keeps a 256 KiB tail, cancels by `SIGTERM` to the process group then `SIGKILL`,
lists lock files after a cancelled or failed write, books every invocation in a per-repository
registry and command log, and ends everything on close. The engine half of a local verb is
therefore "a new named function in `ops/`", not runner work.

What is missing is almost all on the application side, and a little in the model:

- **No local write lane.** `Lane` has one variant (`Network`); `Operation` has one
  (`Fetch`); the repository thread's `Threads::perform` destructures the operation with an
  irrefutable `let Operation::Fetch { remote } = &operation;`, and every type around it —
  `FetchControl`, `Refusal`, `Request::CancelFetch`, the `Update::Fetch*` variants,
  `FetchStatus` — is fetch-specific.
- **Operations are forwarded by the repository thread**, so a write queues behind whatever
  history page that thread is walking (stated in `FetchControl`'s `Stage` docs).
- **`Performed` and its `Invalidated` are thrown away.** The network lane maps the outcome to
  `()`; the window refreshes refs, ahead/behind and status after every operation's end,
  whatever it declared.
- **A running status is never superseded** (refs-and-status R10.3 as amended), so a refresh
  asked after a write can draw a status that started before it, then the follow-up.
- **The confirmation seal is weaker than its prose.** `Confirmed` derives `Clone`, its
  constructor is `pub` and callable from any crate, and the guard's "some `ops/` file names
  `Confirmed`" is satisfied today by `ops/mod.rs` itself (the `use` and
  `Performed::destructive`), so it would stay green if no operation took the token at all.
- **Prompts are only shown while a fetch is in flight**; a local write whose child asks for a
  secret (an LFS smudge on discard, an SSH-signing key) is refused by the window.
- **No operation log exists.** `Performed` is built only on success, has no time, outcome or
  target, and is stored nowhere. The command log is per invocation and records argv but not
  stdin.
- **The close ends a write without asking** (#45 item 3), and lock lists never reach the
  user on a failure or a close (#44).

Eight open issues meet this packet; four of them (#45, #44, #47, #66) explicitly name "the
first local write" as where they are decided.

---

## 2. `cairn-git/src/ops/` and `cairn_model::Confirmed`

### 2.1 `WriteAuthority` (`ops/authority.rs`)

- `pub(crate) struct WriteAuthority { _sealed: () }`, `#[derive(Debug)]` only — not `Clone`,
  `Copy` or `Default`. One constructor, `pub(in crate::ops) fn new()`.
- It is a token rather than a visibility on the builder because `process/` cannot write
  `pub(in crate::ops)` on its own items (a restricted visibility must name an ancestor).
- `GitBinary::write_invocation(WriteAuthority)` builds a `GitCommand<'_, Write>`; the
  `Write` kind holds the authority for the invocation's life (`process/cli.rs`, `Write {
  _authority, token: Option<AskpassToken> }`).
- Five `compile_fail` doctests in `ops/mod.rs` pin that nothing outside the crate can name
  the authority, build a write or a read, or turn the environment into a `Command`, each
  being the passing scaffold plus one line.
- Erosion twin: `the_runner_is_named_only_by_ops_and_reads` (only `ops/` constructs it, builds
  a literal or implements it; `process/` and `ops/` alone name `write_invocation`).
- Consequence for this packet: **any new file under `ops/` can build a write**; there is no
  per-verb roster. A new `ops/stage.rs`, `ops/commit.rs`, `ops/stash.rs` needs no guard
  change to compile.
- `ops/authority.rs`'s test module already runs real local writes through the write builder
  (they need the authority, so they live there): `commit` without a message fails promptly
  with a hanging configured editor (`a_commit_without_a_message_fails_promptly_instead_of_opening_an_editor`),
  `rebase -i` likewise, a `commit -a` cancelled inside a sleeping `pre-commit` hook leaves no
  `index.lock` and commits nothing (`a_commit_cancelled_inside_a_sleeping_hook_leaves_no_index_lock`),
  a failed write names a present `index.lock` (`a_failed_write_names_a_present_index_lock`),
  and `mktree` fed 100,000 entries on stdin (`a_real_read_of_over_five_mib_of_records_arrives_whole_and_in_order`).

### 2.2 `ops::fetch` (`ops/fetch.rs`), the template a local verb would copy

- `fetch(git, repo, remote, token) -> Result<FetchInProgress, Error>`: runs
  `refspec_policy::check` first (a `git config` read; refusal is `Error::FetchRefused`, a
  failed read `Error::RemoteConfig`, no process starts), then
  `git.write_invocation(WriteAuthority::new()).in_repository(repo).args(["fetch",
  "--progress", "--no-prune-tags", "--end-of-options"]).arg(remote)`, `.authorized_by(token)`
  when given, `.start()`.
- Returns as soon as git is running. `FetchInProgress::canceller() -> FetchCancel` (a
  `Clone + Send` wrapper over the crate-private `KillHandle`) and `finish(progress) ->
  Result<Performed, Error>`, which drives the invocation with a `CancelSignal::new()` nobody
  holds (cancel is the kill handle's alone), drops stdout, hands each stderr line to
  `progress`, and on success returns `Performed::new("fetched <remote>",
  Invalidated::refs().and(Invalidated::objects()))`.
- Explicitly **not destructive, takes no `Confirmed`** (module docs: what prune deletes is
  remote-tracking refs, by the user's own `fetch.prune`; tags never pruned).
- Shape lesson: the public surface is a named operation type with a separate canceller, so
  the runner's `Invocation`/`KillHandle` never leaves `ops/` (a stated review obligation
  in the root `CLAUDE.md`).

### 2.3 `Performed` and `Invalidated` (`ops/mod.rs`)

- `Invalidated { refs, index, objects, working_tree }`, `Copy`, with `NOTHING`, one
  constructor per flag, `and`, `anything`. Tests pin that no flag is lost.
- `Performed { description: String, acknowledged: Option<String>, invalidated }`, fields
  private, `#[derive(Debug, Clone, PartialEq, Eq)]`. Two `pub(crate)` constructors:
  `Performed::new(description, invalidated)` and `Performed::destructive(description,
  &Confirmed, invalidated)`, which copies `confirmed.acknowledged()` — "the prompt is taken
  from the token, never typed". Public getters only.
- What it does **not** carry: a time, the repository, the target (paths, stash id, commit
  ids before/after), the outcome (only successes produce one: `finish` returns `Err(Error)`
  otherwise), or a link to the command-log records the operation produced.
- Where it goes: nowhere. `serve_network_lane` calls `fetch_outcome(remote,
  outcome.map(|_| ()), prompting)` (`worker/network_lane.rs:218`), discarding it. No type in
  `cairn-app` or `cairn-ui` names `Performed` or `Invalidated` (grep).

### 2.4 The placeholder seal (`ops::describe_destructive`)

- `pub fn describe_destructive(repo: &Repository, confirmed: Confirmed) -> Performed`
  (`ops/mod.rs:323`) performs no I/O, returns a `Performed::destructive("no-op against
  <git dir>", &confirmed, Invalidated::NOTHING)`. Its doc: "replaced by the first real
  destructive operation". One test, `a_destructive_operation_records_what_the_user_agreed_to`.
- It is a public function of `cairn_git::ops` reachable from `cairn-app` today.

### 2.5 `cairn_model::Confirmed` (`crates/cairn-model/src/confirm.rs`)

- `pub struct Confirmed { acknowledged: String }`, `#[derive(Debug, Clone, PartialEq, Eq)]`
  (`confirm.rs:8`).
- `pub fn by_user(acknowledged_prompt: impl Into<String>) -> Self` and `pub fn
  acknowledged(&self) -> &str`. The doc says to call `by_user` "at the acknowledgement site —
  the handler for the confirm button — never in the engine, and never with text the user was
  not shown". Nothing enforces where it is called.
- How a destructive op is meant to consume it: by value (`describe_destructive(repo,
  confirmed: Confirmed)`), then `Performed::destructive(.., &confirmed, ..)` copies the text
  into the record.
- What the type actually guarantees: that a `String` was supplied. Because it is `Clone`, one
  acknowledgement can be duplicated and spent on several operations; because `by_user` is
  `pub` in a crate every layer links, any code — `cairn-git` itself included — can mint one
  from a literal. "By value" therefore means nothing about single use. The prompt's honesty,
  where it was built and whether it is what was rendered are `destructive-ops-reviewer`'s
  checks 2–4, by review.

### 2.6 The guard `destructive_operations_are_sealed_behind_the_confirmation_token`

`crates/cairn-guards/tests/invariants.rs:4031-4059`. What it checks, completely:

1. `cairn-model/src/confirm.rs` exists and its code (comments stripped) contains the text
   `acknowledged: String`.
2. That file contains exactly two occurrences of `pub fn ` (`by_user`, `acknowledged`).
3. Some file under `crates/cairn-git/src/ops` contains the text `Confirmed` in code.

What it does not check: that `acknowledged` is private (a `pub acknowledged: String` still
contains the substring), that `Confirmed` has no `Clone`/`Default`/`From` impl, where
`by_user` is called, or that any particular operation takes the token by value. Item 3 is
satisfied by `ops/mod.rs`'s `use cairn_model::Confirmed;` and `Performed::destructive`'s
`&Confirmed` parameter, so it stays green even after `describe_destructive` is deleted and
whether or not any verb takes the token. There is no roster of destructive operations; which
operations ARE destructive is check 1 of `destructive-ops-reviewer`, by judgement.

### 2.7 `stranded_locks` (`ops/stranded_locks.rs`)

- `pub(crate) fn stranded_locks(git_dir, common_dir) -> Vec<PathBuf>`: every `*.lock`
  directly in the git directory (and the common directory for a linked worktree), all of
  `refs/` recursively, and `objects/pack`, `objects/info`, `objects/info/commit-graphs`.
  Sorted, deduplicated, never fails.
- Read-only by design: "removing a lock another process may still hold is a decision for a
  confirmed operation that does not exist yet" (#19 item 2).
- A linked worktree's `index.lock` lives in that worktree's own git directory, which is
  searched at top level, so the lock a local write meets most is covered.
- A listing "cannot tell a lock the cancel stranded from one a git in a terminal holds this
  instant".

---

## 3. `cairn-git/src/process/`: the runner as a write path

### 3.1 Building a write

`GitCommand<'a, K>` (`process/cli.rs`) holds program, environment, kind, the repository's
location (`--git-dir=` and `--work-tree=` ahead of the verb), arguments, current directory,
`GitDirs` for lock search, optional stdin bytes, and the repository's `Arc<Processes>`.

- `in_repository(repo)` sets all of the location, the directory (work tree, or git dir if
  bare), `GitDirs { git_dir, common_dir }` and the registry. Every repository is named to git
  explicitly; git's `safe.directory` check is made at Cairn's open, not by the child
  (`repository_location`'s docs).
- `arg`, `args`, `input(bytes)`, and on `GitCommand<'_, Write>` only, `authorized_by(&token)`
  (a read has no method and no field for a token).
- `start()` → `Invocation<K>`; `collected()` is test-only.

### 3.2 stdin: supported, tested, never used by a product verb

- `GitCommand::input(bytes: impl Into<Vec<u8>>)` — the whole input in memory. `start_with`
  pipes stdin only when input was given, `Stdio::null()` otherwise.
- `runner::watch` writes it on a thread of its own (`cairn-git-stdin`), started before the
  stdout/stderr readers. The `ChildStdin` is held in an `Arc<Mutex<Option<_>>>` until that
  thread has started, with a comment that names this packet's verb: a closure that never runs
  would drop the pipe and git would read "the end of an EMPTY input — `reset
  --pathspec-from-file=-` given nothing unstages everything"; on a thread-start failure the
  process is ended BEFORE stdin closes (`runner.rs`, around line 183;
  `a_stdin_thread_that_cannot_start_ends_the_process_before_its_input_closes`).
- `pipes::feed` treats `BrokenPipe` (git closing stdin early) as no error; any other write
  error calls `Group::input_failed`, which ends the process before the pipe closes, and the
  outcome is `Error::GitUnwatched` (`a_failed_stdin_write_ends_the_process_and_a_closed_stdin_does_not`,
  `a_failed_stdin_write_signals_before_it_closes_the_pipe`).
- Pinned: 64 MiB in while stdout and stderr are busy
  (`sixty_four_mib_of_stdin_beside_busy_stdout_and_stderr_completes`), `hash-object --stdin`
  end of input and id parity, `mktree` over 100,000 entries.
- So `git apply --cached -` (patch on stdin), `git commit -F -` (message on stdin) and
  `--pathspec-from-file=- --pathspec-file-nul` (long path lists) need **no runner change**.
  The roadmap's "`git apply --cached` needs a runner change" (packet 5 brief, written before
  process-manager) is already satisfied.

### 3.3 Running, output and exit codes

`Invocation<K>` offers `finish(cancel, stdout_fn, progress_fn)`, `finish_within(cancel,
ceiling, ..)`, `records(cancel, record_fn, progress_fn)` and `collect(cancel, ceiling,
progress_fn)`, all generic over the kind — a write may be collected under a ceiling today.

`Invocation::drive` decides, after the reap, in this order (`docs/systems/git-processes.md`,
"Outcomes"):

1. Ceiling crossed → `Error::GitOutputTooLarge { arguments, ceiling, stranded_locks }`,
   **even if git then exits 0** (#45 item 2).
2. Stdin write failed, a pipe thread could not start, `try_wait` failed →
   `Error::GitUnwatched { arguments, source, stranded_locks }`.
3. Asked to end and not beaten by a clean exit → `Kind::cancelled`: a write is
   `Error::GitCancelled { arguments, stranded_locks }` (locks listed **after** the reap).
   A cancel beaten by exit 0 is success. A cancelled write may have taken effect (#45 item 1).
4. Non-zero exit → `Error::GitFailed { arguments, status, stderr, present_locks }`, stderr the
   retained tail, `present_locks` listed for a write.
5. Exit 0 → `Ok(Output)` carrying the retained stderr tail and (for `collect`) stdout.

There is no outcome between "failed" and "succeeded": every non-zero exit is `GitFailed`.
A verb whose exit 1 is a state — `git stash pop`/`apply` stopping on conflicts, keeping the
stash — will arrive as `GitFailed`, and the output policy forbids reading stderr to tell the
two apart ("classify a failure by its exit code and the repository's state", `processes.md`,
"The environment"). `Output::stderr()` and `Output::stdout()` carry `#[expect(dead_code)]`
outside tests with reasons that are stale ("the first operation to will be status";
"nothing yet reads the tail a success retained"): a successful commit's hook output, or a
warning git prints on success, is retained and then dropped.

### 3.4 stderr

- Split on `\n` and `\r`; each non-blank line handed to the `progress` callback as it
  arrives (this is how hook output would stream); the last 256 KiB (`pipes::TAIL_BYTES`) kept
  for the error and the log. Shown verbatim, never parsed.

### 3.5 Cancelling a write

- Three routes, all ending the group the same way (`SIGTERM` to the group, `SIGKILL` after
  `TERMINATION_GRACE` = 2 s): the cancel signal polled every `TICK` (20 ms), a `KillHandle`
  (`kill()` never blocks), or dropping the invocation (ended and reaped on a reaper thread).
- The group reaches hooks, filter drivers and credential helpers git started; a process that
  left the group (detached `gc --auto`, the fsmonitor daemon, an ssh ControlPersist master)
  outlives it. `git credential-cache--daemon` stays in the group and dies with a cancel.
- No timeout on any invocation, by design (`processes.md`, "Cancellation"): "the window shows
  how long an operation has run and offers its cancel instead". The window shows no elapsed
  time today (no `elapsed` in `status_text.rs`, `fetch_state.rs` or `window.rs`).

### 3.6 Registry and close (`process/registry.rs`)

- `Processes { running, left: Condvar, next, closing: AtomicBool, log }` per
  `SharedRepository`, shared with every worker handle.
- A `Registration` enters at spawn, leaves (and writes its one log record) when the runner
  concludes the invocation, on whatever thread.
- `end_all(bound)` (`SharedRepository::end_invocations`) sets `closing`, asks every group to
  end, waits up to `bound` on the condvar. Anything entering afterwards is asked to end at
  once. `CLOSE_BOUND` = 3 s, asserted above grace + `DRAIN_BOUND`.
- It ends **every** invocation, a commit or an apply included, with no distinction of kind.

### 3.7 The command log (`process/command_log.rs`, `cairn_model::CommandRecord`)

- One `CommandRecord { arguments, directory, started, duration, exit, cancelled, stderr }` per
  invocation run `in_repository`; bounded by `LOG_ENTRIES` (1000) and `LOG_BYTES` (4 MiB),
  oldest dropped; a record over the byte bound is trimmed and says so.
- Records the **arguments after the program** (the `--git-dir`/`--work-tree` location is
  left out on purpose) and **not stdin**. Consequences for this packet: a commit message given
  as `-m <msg>` would be in the log verbatim; given on stdin (`-F -`) it is not; a staged
  patch is never in it; a pathspec list given as arguments is (up to `ARG_MAX`), one given
  on stdin is not.
- `CommandRecord` derives `Debug` and keeps stderr, which #46 is about.
- Answered as values: `Request::CommandLog` → `Update::CommandLog { records }` on the
  repository thread; `session::apply` ignores it (`Update::CommandLog { .. } => {}`); no view
  (#41).

### 3.8 The askpass token on a write

- `authorized_by(&AskpassToken)` clones the token into the `Write` kind; the environment
  profile `Profile::Write { token }` adds `CAIRN_ASKPASS_TOKEN` only then
  (`an_authorised_write_carries_its_token_and_only_that_one`).
- A write without a token still points `GIT_ASKPASS`/`SSH_ASKPASS` at Cairn's helper, which
  fails closed (`docs/systems/credentials.md`, "The pieces"). `Channel::begin()` issues one
  `Operation` whose token covers every prompt of one invocation and is retired on refusal.
- The network lane begins one channel operation per fetch (`network_lane.rs:190`). Nothing
  says whether a local write gets one.

### 3.9 The write environment

`ALWAYS` = `GIT_TERMINAL_PROMPT=0`, `SSH_ASKPASS_REQUIRE=force`, `GIT_EDITOR=false`,
`GIT_SEQUENCE_EDITOR=false`, plus `GIT_ASKPASS`/`SSH_ASKPASS`; a read adds `READ_ONLY`
(`GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`). A write gets neither of the read pins —
correct for a write, and also means a write in a partial clone may lazy-fetch (i).

`INHERITED` is the roster in `process/environment.rs`: `PATH`, `HOME`, `XDG_CONFIG_HOME`,
`XDG_CACHE_HOME`, `DBUS_SESSION_BUS_ADDRESS`, `XDG_RUNTIME_DIR`, `SSH_AUTH_SOCK`, `TMPDIR`,
the locale variables, the proxy, CA and Kerberos variables (#18). Everything else is
stripped, every `GIT_*` but `GIT_SSL_CAINFO`/`GIT_SSL_CAPATH` included. This was decided for
fetch; for a commit it decides what the user's **hooks** see. Inferences to verify in the
packet: a hook framework that relies on `VIRTUAL_ENV`, `NVM_DIR`, `JAVA_HOME`, `SHELL`,
`USER`/`LOGNAME` or `TERM` runs differently from the user's terminal (i); `commit.gpgSign`
with gpg-agent needs `GNUPGHOME` when non-default and a display for a GUI pinentry
(`DISPLAY`/`WAYLAND_DISPLAY`), both still open on #18 (i); SSH signing (`gpg.format=ssh`)
reaches the agent through `SSH_AUTH_SOCK`, and an unagented passphrase would go to
`SSH_ASKPASS` (i); git's `EMAIL` fallback for an unset `user.email` is not inherited (i).

### 3.10 What in `cairn-git` assumes fetch is the only write

- `ops/mod.rs` docs: "the public surface is named operations ([`fetch`] today)", the
  cache-contract paragraph ends on fetch, and `describe_destructive` is a placeholder.
- `Output::stdout`/`stderr`/`records` dead-code reasons name fetch.
- `Kind::cancelled` for `Write` always searches locks; correct for every write.
- Nothing else in the runner is verb-specific. The process module was built to serve "a
  mutation fed on stdin, a hooked mutation that may run for minutes" (`process-manager` PRD).

---

## 4. `cairn-app/src/worker/`: how an operation runs today

### 4.1 Threads and routing

Five threads per repository (`worker/pool.rs`): `cairn-repository` (history lane, refs, the
filters, the command log, and the **front door for operations**), `cairn-diff`,
`cairn-refresh` (status, ahead/behind), `cairn-network` (the network lane), `cairn-askpass`.

`RepositoryHandle::submit` (UI thread, never blocks) numbers a query in its lanes and routes
it by `routing::route`. Operations (`Request::Fetch`, `CancelFetch`, `Close`, `CommandLog`,
`Retire`) have no lanes (`Request::lanes` returns `&[]`) and so no epoch. `Request::Fetch`
routes to `RepositoryJob::Fetch` on the **repository thread's queue**, whose `serve` loop
calls `threads.perform(Operation::Fetch { remote }, outbox)` when it dequeues it
(`pool.rs:851`). `CancelFetch` alone bypasses every queue (`Routed::CancelFetch =>
self.control.cancel()`).

So an operation waits behind any history page or find the repository thread is walking.
`FetchControl`'s `Stage::CancelledBeforeStarting` exists precisely because "`Request::Fetch`
queues behind whatever page the repository thread is walking, while `CancelFetch` reaches
this control directly".

### 4.2 The network lane (`worker/network_lane.rs`)

- `enum Operation { Fetch { remote } }`, `enum Lane { Network }`, `Operation::lane()` an
  exhaustive match ("no arm may default, so a new operation does not compile until it has
  one"). Pinned by `a_fetch_runs_in_the_network_lane`.
- `Threads::perform` (`pool.rs`) matches the lane, then `let Operation::Fetch { remote } =
  &operation;` — irrefutable today; a second operation in the network lane stops it
  compiling.
- `FetchControl(Arc<Mutex<Stage>>)`, `Stage = Idle | CancelledBeforeStarting | Starting {
  remote, cancelled } | Running { remote, cancel: FetchCancel }`. `arm` claims the single slot
  or returns `Refusal { in_flight, running }` (Display: "a fetch of X is already running" /
  "waiting to start"), sent as `Update::FetchRefused`. One fetch at a time, refused rather
  than queued: there is no queue of distinct operations in the lane beyond the channel.
- `serve_network_lane(git, shared, channel, prompting, control, operations, outbox)`: takes
  one `shared.to_worker()` handle for its life; per fetch, `channel.begin()` for a token,
  `fetch(..)`, `control.install(remote, canceller)` BEFORE sending `Update::FetchStarted`,
  streams `Update::FetchProgress { line }` per stderr line (one update per line, #25),
  `control.clear()`, drops the token, sends `fetch_outcome(..)`: `FetchFinished`,
  `FetchCancelled { stranded_locks }` or `FetchFailed { message }` (message =
  `error.to_string()`, plus why nothing could prompt when the channel is unusable).
- Known races (#47): a Cancel pressed between `clear()` and the outcome is kept as
  `CancelledBeforeStarting` and cancels the **next** fetch; a fetch already forwarded when a
  close comes is spawned and then ended by the registry.

### 4.3 How an outcome reaches the window and triggers refresh

`session::apply` (`crates/cairn-app/src/session.rs`):

- `FetchStarted` / `FetchProgress` update `FetchStatus`; `FetchFinished`, `FetchCancelled`,
  `FetchFailed` each `withdraw` any prompt, set the status, and call
  `refresh_after_an_operation(worker)`, which submits `Request::Refresh` unless the window is
  closing. The refresh is unconditional — every ending, success, cancel or failure — which is
  what covers "a cancelled or failed fetch may have moved refs" without reading any
  declaration.
- The other refresh triggers (refs-and-status R10.1): the window gaining focus
  (`refresh::on_focus_gained`, a `use_side_effect` on `Platform::is_app_focused`), the
  Refresh action in the accelerator table (`shortcuts.rs`, F5 / ⌘R), and the window's first
  open. Nothing watches the file system (R10.2, #65).
- A refresh is `Routed::Refresh`: refs to the repository thread (refs lane epoch), status to
  the refresh thread under the status lane's **current** epoch (no refresh bumps it), and
  ahead/behind forwarded by the repository thread after the refs. `Update::Refs { snapshot,
  reopen }` reopens the history when the refs the walk draws moved (this is how the cache
  contract's `refs` flag is honoured: the open `HistorySession` is dropped by
  `Page::Open`). `Update::Status { changes }` redraws Local Changes; the chosen path is
  "asked again as it is listed now" (`docs/systems/local-changes.md`, "A refresh follows the
  path"), so the working-tree diff is re-read after any status.
- **Status is not superseded** (`worker/refresh_lane.rs` docs; R10.3 as amended, the user's
  decision of 2026-10-07): a running `git status` finishes and is drawn, and every refresh
  asked while it ran becomes one follow-up. A status that started before a write ends is
  therefore drawn after it — pre-write lists — and the post-write one follows it. On a
  stat-dirty rust-lang/rust that window is about 736–759 ms (`docs/research/refs-and-status/measured.md`).

### 4.4 Progress and errors in the window

- `FetchStatus` (`fetch_state.rs`): `Idle | Starting | Running { line } | Cancelling |
  Finished | Cancelled { stranded_locks } | Failed { message }`; one banner; the Fetch button
  hidden while in flight; `Cancelling…` drawn at once.
- A failure's banner draws only git's first `fatal:`/`error:` line
  (`status_text::why_it_failed`), so `present_locks` inside the message is never seen (#44).
- `Update::Prompt` is shown only `if fetch.read().is_in_flight()`, otherwise refused
  (`session.rs:160`): a prompt from any other operation's child is refused by the window.
- No confirmation dialog component exists in `cairn-ui` (the one modal is
  `credential_prompt.rs`, which has a yes/no mode for ssh's host-key question). No operation
  log drawer exists.

### 4.5 Closing (`closing.rs`, `Threads::drop`)

- First close request: `submit(Request::Close)` (stops every epoch, an atomic store, then
  queues the close) and `CloseDecision::KeepOpen`. The repository thread's `Threads::drop`
  stops the diff and refresh threads, drops the network queue, calls
  `end_invocations(CLOSE_BOUND)`, stops the acceptor. The window closes on the update
  stream's end; a second request after `CLOSE_PATIENCE` (5 s) closes it regardless (#48).
- A queued operation on the repository thread's queue is never forwarded once the epochs are
  stopped (`serve` stops); one already in a lane's queue is spawned and ended as it enters
  the registry (#47).
- The hook refuses nothing and says nothing about what is running (#43, #45 item 3).

### 4.6 What in `cairn-app` assumes fetch is the only write

`Operation`/`Lane` (one variant each); `Threads::perform`'s irrefutable `let`;
`Threads { network: Option<Sender<Operation>> }` ("one lane so far"); `FetchControl`,
`Stage`, `Refusal`'s fetch wording; `Request::Fetch`/`CancelFetch`, `Routed::CancelFetch`;
`Update::FetchStarted/Progress/Finished/Cancelled/Failed/Refused`; `FetchStatus`/`PromptView`;
the prompt gate on `fetch.is_in_flight()`; `refresh_after_an_operation`'s "a fetch, today";
`fetch_outcome`'s prompting message; the close step that names the network lane as the one
the window waits on for a late `git`.

---

## 5. Write lanes: designed vs built (`docs/design/concurrency.md`, "Operations")

| Designed | Built |
| --- | --- |
| Operations carry no epoch; cancelled by the user, by killing the process | Built for fetch |
| "When it finishes, the worker acts on what it reports invalidated" | Not built: `Performed` discarded; the window refreshes everything after every ending |
| Two write lanes per repository, each one at a time and in order: **Network** (fetch, push, pull's transfer) and **Local** (index, working tree, local refs: staging, commit, checkout, reset, stash, branch, tag) | Network only (`Lane::Network`); Local "lands beside this one with the first local write" (`network_lane.rs` docs; PRD process-manager R7.3) |
| The two lanes run beside each other; the index is written only from the local lane; refs and `packed-refs` contention left to git's own locks and retry | One lane, so nothing to run beside. No code writes the index |
| Reads run beside both; a read computed during a write may see a transient state and the write's invalidation corrects it | Reads do run beside the network lane. Correction is by unconditional refresh, and status is never superseded (4.3) |
| A user operation started while its lane is busy **queues, and the window says it is queued** | Not built: a second fetch is refused (`Refusal`); the "waiting to start" text is the refusal of a second copy, not a queue of different operations |
| A second copy of one already running or queued is refused with that reason | Built for fetch (R7.2) |
| Work Cairn starts itself (an automatic fetch, a refresh-index per #66) is skipped while its lane is busy, and never prompts | Nothing Cairn-started exists |
| Lane keyed by repository (the common directory), so two worktrees share their local lane; "whether worktrees want it split is open in the spine" | Lanes live in `Threads`, one per opened repository (one window, one repository). Nothing is keyed by common directory; two windows on two worktrees of one repository would each have their own (i) |
| Rejected: one write lane for everything; a read/write lock over the repository | Consistent: there is no lock |

Gaps the design does not address but the code makes concrete: operations reach their lane
through the repository thread's queue (4.1), so "a stage click never waits behind a push" is
true but "never waits behind a history page" is not; and the design's "queued, and says so"
has no window vocabulary.

---

## 6. The cache-invalidation contract after a git write

From `ops/mod.rs` ("The cache-invalidation contract", checked against gix 0.87.1) and
`docs/design/engine.md` ("Two implementations of git, kept in agreement"):

- **refs** — gix re-reads loose refs per lookup and reloads `packed-refs` when its mtime
  changes (same-tick rewrite is the residual). Stale: anything that resolved a ref and kept
  it — an open `HistorySession` and `HistoryCursor`. Honoured by reopening the history from
  `HEAD`. As built, by `Update::Refs { reopen }` after a refresh, which compares what the walk
  draws. A commit, amend, stash push/pop/drop all move a ref the snapshot holds (the branch,
  `HEAD`, `refs/stash`), so the refresh path covers them (i).
- **index** — gix shares one index snapshot between the `SharedRepository` and every worker
  handle and re-reads it when the file's mtime changes. "A write inside the same timestamp tick
  as the previous read is the residual gix cannot see, so an operation that writes the index
  and then reads it back in one request must reopen a handle." Nothing in `cairn-git` caches
  an answer derived from the index. Status is git's (`reads::status`), so it is always fresh;
  what gix still reads from the index is the working-tree diff's index side and conflict
  stages (`diff/working_tree.rs`) and `diff_freshness`'s `StagedInputs` (the `.gitattributes`
  and `.gitmodules` the index holds, re-read when the index file or `HEAD` moved, compared by
  value). The diff thread's `diff_freshness` treats any stamp within `SETTLING` (2 s) of the
  query's start as unsettled and keeps nothing read under it, which bounds the tick residual
  for kept answers.
- **objects** — gix refreshes the pack directory on a lookup miss
  (`RefreshMode::AfterAllIndicesLoaded`); nothing to do; declared so a future refresh-mode
  change knows what it breaks.
- **working_tree** — nothing in gix caches it; re-run status and diff.

What the packet inherits: the contract is written and per-flag, but no consumer reads it.
The window's single response — refresh refs, ahead/behind and status, after every ending —
over-serves a stage (which needs status alone: a refs read and ahead/behind walk per click)
and is late for one (the non-superseded status). Whether to route a narrower refresh from
`Invalidated`, and how the window keeps from drawing a status that began before the write it
answers, are open (section 11).

Per-verb declarations, from `docs/research/process-manager/consumer-invocations.md` rows
P1–P12 (inferences there marked (i)): stage files → `index`, `objects`; unstage → `index`;
stage/unstage hunk or lines (`git apply --cached [-R] -`) → `index`; discard hunk/lines
(`git apply -R -`) → `working_tree` (and an index refresh (i)); discard file (`git restore`
/ `checkout --`) → `working_tree`; clean → `working_tree`; commit/amend → `refs`, `index`,
`objects`, and `working_tree` if a hook rewrote files; stash push → all four; stash
apply/pop → all four; stash drop → `refs`.

---

## 7. GitHub issues this packet meets

### #45 — Decide the write-verb policies before the first local write (open, question)

Three runner behaviours harmless for fetch, sent by the user on 2026-10-02 "to the first
local-write packet":

1. **A cancelled write may have taken effect.** Proposal: make "compare the state the verb
   touches before and after, when it ends cancelled or unwatched" a `destructive-ops-reviewer`
   obligation for every write verb, and reword the window's cancelled message. Recommended:
   both.
2. **A crossed ceiling outranks a write's clean exit** (`Invocation::drive`). Options: (a)
   forbid a ceiling on a write — `collect` on `Invocation<Read>` only, type-level; (b) clean
   exit outranks the ceiling with a "cut" outcome; (c) state it. Recommended: (a).
3. **Closing ends a write without asking.** Options: (a) the close hook refuses while a
   local write is in flight and says so; (b) asks, naming it; (c) closes then says what it
   cost. Recommended: (a) for local writes, today's behaviour for the network lane. Couples
   with #44.

What it asks the first local write to decide: all three.

### #44 — Show the lock files a failed or closed write leaves behind (open, enhancement)

1. A failed write's `present_locks` reaches `Update::FetchFailed` inside the message, which
   the banner cuts to the first `fatal:`/`error:` line. Proposal: carry the list structurally
   and draw it. "Small and has no design cost."
2. A close that `SIGKILL`s a write strands locks reported to a closing window. Options: (a)
   keep the residual; (b) keep the window open to show them. Recommended: (a) while fetch is
   the only verb, "revisit (b) with the first local write, where `index.lock` is the lock a
   user will actually meet, together with … #45".

What it asks: whether the local lane's outcome carries locks structurally (a generalisation
of `FetchCancelled { stranded_locks }`), and the close-with-locks behaviour.

### #19 — Cancel is SIGKILL: SIGTERM-first, and surface a stranded lock (open, question)

Half shipped (comment of the credential-prompts packet): `nix` with `signal` + `process`,
`SIGTERM` then `SIGKILL` after 2 s, stranded locks listed after a cancel. **Left open:** a
failed write whose stderr says `cannot lock ref` is not read for the lock it names (and the
output policy now forbids reading stderr, so the structural `present_locks` is the
replacement), and the **confirmed lock-removal operation** does not exist — `ops` taking
`Confirmed`, since deleting a lock another process holds is destructive.

What it asks: whether this packet, whose verbs are the ones that meet `index.lock`, builds
"Remove stale lock" behind `Confirmed`.

### #41 — Show the command log (open, enhancement)

Engine and worker halves exist; no view. Proposal: a Fork-shaped panel asking for the log
when opened and after each operation ends; `VirtualScrollView`; stderr collapsed by default;
decide #46 before drawing a tail.

What it asks this packet: the brief says "the operation log becomes visible here". Whether
that is #41's command log, a separate operation log fed by `Performed`, or one view of both
is undecided anywhere in the docs (`docs/design/ui.md` names "an operation log drawer,
quoting the prompt the user acknowledged for each destructive operation"; `processes.md`
names the command log "for a view that shows the user what Cairn ran", Fork-style).

### #18 — The rest of the inherited roster, and pinning GIT_EDITOR (open, question)

Shipped: proxies, CA bundles, Kerberos (credential-prompts), and `GIT_EDITOR=false`,
`GIT_SEQUENCE_EDITOR=false` in `ALWAYS` (process-manager). **Still open, for the user:**
`DISPLAY`, `WAYLAND_DISPLAY`, `GNUPGHOME`. The original text: `GNUPGHOME` is "irrelevant to
fetch; relevant the day a signed commit or tag is made through this backend" — which is this
packet's commit.

What it asks: what a signed commit (`commit.gpgSign`, `gpg.format=ssh`) needs to work, and
by extension whether hooks need anything off the roster (3.9).

### #46 — Scrub credentials from the retained stderr? (open, question)

The 256 KiB tail rides on errors and in the command log for the session; a URL with userinfo
is a credential that is not a `Secret`; `CommandRecord` derives `Debug`. Options: (a) scrub
`scheme://user[:pass]@` in `cairn-git` (redact, never classify); (b) keep the tail out of the
log; (c) keep and collapse in #41's view. Recommended: (a), decided with #41 and before that
view draws a tail.

What it asks this packet: only if it draws the command log (or hook output in an error) —
hook output from a commit is exactly the long stderr that would be shown.

### #48 — A hung probe and a forced close (open, question)

1. `git --version` has no deadline; a hung probe blocks every repository. Option (b): a probe
   deadline written into `processes.md` as the one bounded wait.
2. A close forced after `CLOSE_PATIENCE` orphans any `git` registered late. Option (b): before
   the forced close, `SIGKILL` every group in the registry through a try-lock call from the UI
   thread. Recommended: (b) for both.

What it asks: with local writes, a forced close can orphan a commit or an apply holding
`index.lock` — the stakes of option 2 rise; and #45's "refuse to close during a local write"
interacts with the patience close (does the second request still force it?).

### #16 — Push through the backend, sealed behind `Confirmed` (open, enhancement)

`remote-sync`'s; built "exactly as `ops::fetch` is", taking `Confirmed` for force,
force-with-lease and delete. Relevance here: it is the first network-lane verb after fetch,
so the generalisation of `Operation`/`FetchControl` this packet does for the local lane
should leave room for it; and it is the precedent for "which forms of one verb are
destructive" (amend vs commit, stash drop vs apply).

### Adjacent issues found while reading

- **#47** — tie a cancel to the operation it was meant for (`arm` hands back an id carried by
  the cancel) and add a `Closed` stage so a lane spawns nothing for a closing repository. Its
  recommendation names "a second verb in the lane"; a local lane with a cancellable commit
  meets the same races.
- **#43** — say the window is closing. Its second half (the network lane's `ref_tips` scans
  before and after a fetch) describes code that no longer exists: `serve_network_lane` reads
  no refs (refs-and-status moved that to the refresh). Only the banner half stands.
- **#66** — refresh a stale index (`git update-index --refresh`, or `status` as a write) "in
  the local write lane under `ops/`", depending on the lane and #45; recommends automatic,
  only when no other write is queued, refusing rather than waiting on an existing lock. The
  first Cairn-initiated local write.
- **#65** — file-system watching (open); decides whether a write made outside Cairn is seen.
- **#62**, **#70** — ignored and untracked listing in Local Changes; `.gitignore` editing's
  natural neighbours.

---

## 8. What already pre-empts decisions for staging and commit

In code:

- **Patch construction is done.** `cairn_model::Selection` (removed lines by old number,
  added by new — survives every projection), `emit_patch(file, text, selection) -> Patch`
  at `PATCH_CONTEXT` = 3 always, from the exact `TextDiff` (the input type cannot carry
  whitespace-ignoring ranges), and the reference `apply_patch`. C1–C3
  (`crates/cairn-git/tests/diff/patches.rs`) put emitted patches through real `git apply
  --cached --whitespace=nowarn`, with and without `--reverse`, after `--check`, into a scratch
  index and object directory, comparing resulting trees. The window keeps `ShownDiff`, which
  holds the `FileDiff` and its `TextDiff` (`ShownDiff::text()`), so a selection can be made
  against what is drawn.
- **The invocation is chosen**, in the test fixture: `git apply --check --cached` then `git
  apply --cached`, "the pair `git add -p` itself runs". `git apply` without `--reject` is
  all-or-nothing (verified, section 9), so the `--check` pass is not needed for atomicity.
- **The runner comment pre-empts pathspec-on-stdin for unstaging**: `reset
  --pathspec-from-file=-`.
- **The working-tree read is in git's form** (clean filter run), so a patch built from it
  applies to the index; and `git apply -R` against the working tree reads it through the clean
  filter and writes it back through smudge (verified on 2.30.9 and 2.56.0, section 9), so the
  same patch discards correctly in a CRLF or filtered file.
- **`GIT_EDITOR=false`** is pinned: `git commit` without a message fails promptly. A commit
  must pass its message (`-F -` or `-m`).
- **The stash list is read** (`refs/stash.rs`): `StashEntry { index, message, commit, base }`,
  `stash@{0}` newest, numbered as git numbers them. A drop addressed as `stash@{n}` acts on
  whatever is at `n` when git runs (i); the entry's `commit` is there to check against.
- **gix's newest-first reflog reader stops at the first line longer than 4 KiB**
  (`refs/stash.rs` docs: the log is read whole and reversed instead). The reflog view meets
  the same reader.
- **`stranded_locks` is read-only on purpose**, deferring lock removal to a confirmed op.

In docs and decisions:

- **L2** (patch-capable model), **L3** (staging through `git apply --cached`), **L4** (the
  reflog view ships with the first commit-level destructive operation — amend and stash drop),
  **L5/O3** (auto-stash before destructive working-tree operations: "decide deliberately… Do
  not inherit a default"; `ui.md` puts **"stash first"** in the discard dialog as UI).
- **The destructive dialog's text IS the `Confirmed` prompt**, saying what is lost, how much,
  and whether it is recoverable, in that order (`ui.md`).
- **Fork's staging affordance** (hover Stage/Discard, drag-selection narrows, Discard on
  unstaged chunks only) vs the mockup's header actions and selection gutter: "undecided on
  purpose" (roadmap brief).
- **No retry of a write** (`processes.md`, "Failure"): "a retry Cairn invented could re-run a
  mutation the user confirmed once"; a present `index.lock` is named, not waited on.
- **Write verbs and versions at the 2.30 floor** (checked against the 2.30.9 build): `git
  restore`, `git add --pathspec-from-file=<file> --pathspec-file-nul`, `git stash push
  --pathspec-from-file` exist; **`git stash push --staged` does not** (it is newer), so a
  "stash only what is staged" would degrade below its version (memory: git floor stays 2.30).
- **`consumer-invocations.md` synthesis (b)**: index writers are mutually exclusive per
  worktree and git does not wait for `index.lock` (verified: exit 128 at once, section 9);
  hooks mutate under a running commit, so Cairn must not schedule its own index work, or
  trust an index read, while a commit is inside its hooks; a repository mid-sequencer accepts
  only that sequencer's verbs.
- **`.gitignore` editing**: no `git` verb writes it, so D1 ("every mutation goes through the
  `git` binary") has no answer for it. `consumer-invocations.md` P13: "whether it counts as
  an `ops` mutation is unstated". No guard sees a plain file write: the gitoxide-mutation
  matcher catches `.write(` as a method in `cairn-git` but not `std::fs::write` as a path
  call (`GITOXIDE_MUTATION_METHODS` docs), and nothing scans `cairn-app` for file writes.

---

## 9. Probes run

Scratch repositories under the session scratchpad, `HOME` pointed at the scratch directory
and `GIT_CONFIG_NOSYSTEM=1`, so no user configuration took part.

| Probe | git 2.56.0 | git 2.30.9 |
| --- | --- | --- |
| `git commit -q -F -` with a `pre-commit` hook that tries to `read` a line from stdin | hook's stdin is empty (git gives hooks no stdin); the message reached the commit | same |
| Same commit, message `s\n\n# c\n` | `# c` kept in the message: with `-F`, git's default cleanup is whitespace-only, not strip | same |
| `git add f` with a stale `.git/index.lock` present | fails at once, exit 128, "Unable to create '…/index.lock': File exists. Another git process seems to be running…" | not run |
| `git apply --cached` of a two-hunk patch whose second hunk does not match | exit 1, "patch does not apply", **nothing staged** (all-or-nothing) | not run |
| `git apply -R` of `git diff-files -p` output to the working tree, `core.autocrlf=true`, CRLF file | discard exit 0, the file restored with its CRLF endings, status clean | same, with a clean/smudge filter on top: restored in working-tree form |
| `git stash push -h` lists `--staged` | not run | no (and `--pathspec-from-file` is listed; `git restore` and `git add --pathspec-from-file … --pathspec-file-nul` exist too) |

---

## 10. What a local write lane would have to add

Engine (`cairn-git`):

1. One named operation per verb under `ops/`, each built with `write_invocation(WriteAuthority::new())
   .in_repository(repo)`, `--end-of-options` or `--` before user-supplied paths, NUL
   pathspecs on stdin for long lists, the patch or message on stdin; returning an
   `…InProgress` with a canceller and a `finish` that yields `Performed` declaring its flags.
   The destructive ones (discard file/hunk/lines, clean, amend, stash drop, and whichever
   others the packet rules destructive) take `Confirmed` by value and build
   `Performed::destructive`. `describe_destructive` deleted.
2. An outcome vocabulary for a verb that can stop in a state (stash apply/pop on conflict):
   decided from the repository's state after the exit (e.g. the stash still listed, unmerged
   entries in status), never from stderr.
3. Possibly: `collect`/`finish_within` restricted to `Invocation<Read>` (#45 item 2).
4. Possibly: a confirmed `remove_stale_lock` (#19).
5. A reflog read for the reflog view (gix; whole-log read as `refs/stash.rs` does).

Application (`cairn-app`):

1. `Lane::Local` and its thread, its own `to_worker()` handle, with `Operation` variants per
   verb and `Operation::lane` naming each; `Threads::perform` generalised past its
   irrefutable `let`.
2. A control per lane generalising `FetchControl`: what is running, what is queued (the
   design says a different operation queues and the window says so), a refusal for a second
   copy, a cancel tied to the operation's identity (#47), a `Closed` stage.
3. A route for operations that does not queue behind a history page (today they ride the
   repository thread's queue).
4. Generic operation updates (`OperationStarted { id, what }`, progress lines, ended with
   outcome, locks carried structurally — #44), or one family per verb; and the window state
   and banner that draw them, with elapsed time if "no timeout, show how long" is honoured.
5. A decision on askpass for local writes, and the prompt gate in `session::apply`
   generalised from `fetch.is_in_flight()`.
6. A refresh after every local ending (as fetch does), with a rule that keeps the window from
   drawing a status that started before the write ended, and optionally a narrower refresh
   from `Invalidated`.
7. The close's behaviour with a local write in flight (#45 item 3, #44 item 2, #48 item 2).
8. A store for the operation log (`Performed`s, plus failures and cancellations) and its
   drawer; the confirmation dialog component, whose rendered text is what `Confirmed::by_user`
   receives.
9. Keeping Cairn's own refreshes and any Cairn-initiated write (#66) from touching the index
   while a commit is inside its hooks.

Guards and review:

1. Strengthen or restate `destructive_operations_are_sealed_behind_the_confirmation_token`
   so it decides something once the placeholder is gone (2.6).
2. Every new `ops/` verb is in `destructive-ops-reviewer`'s scope; #45 item 1 proposes a new
   check (before/after comparison on a cancelled or unwatched write).
3. A change to `cairn-git/src/ops/` requires a test in the same commit (root `CLAUDE.md`).

---

## 11. Open questions for the packet

Each is a decision the packet must make, with the evidence that frames it.

1. **Does a local write route through the repository thread or straight to its lane?**
   Today operations queue behind a history page or find (`pool.rs`, `RepositoryJob::Fetch`;
   `Stage::CancelledBeforeStarting` docs). Diffs and status were moved off that thread for
   exactly this reason (`routing.rs` docs). A stage click behind a deep find is the cost of
   keeping it.

2. **What does the local lane do with a second operation: queue, refuse, or coalesce?**
   Designed: queue and say so; refuse a second copy (`concurrency.md`). Built: refuse only.
   Stage clicks in quick succession are distinct operations on the same index; whether a
   queued stage built from a diff that the previous stage made stale is still applied (its
   `git apply --cached` would fail "does not apply", all-or-nothing, section 9) or dropped
   is a UX decision.

3. **Is the lane keyed per repository (common directory) or per worktree?** Designed per
   common directory, "open in the spine"; built per opened repository. Only matters once two
   worktrees of one repository are open (the repository-manager shape is also open).

4. **Which operations are destructive and take `Confirmed`?** `consumer-invocations.md`
   marks discard (hunk, lines, file), clean, amend and stash drop as destructive; stage,
   unstage, commit, stash push and stash apply as not. Undecided: stash pop (drops the stash
   only on success — recoverable from its id until gc (i)), unstage of an intent-to-add or
   new file, staging over an index state the user built by hand, discard of a staged change
   (Fork refuses it by design), `.gitignore` edits.

5. **Does the seal get stronger before it is first relied on?** `Confirmed` is `Clone`;
   `by_user` is callable from any crate; the guard's `ops/` check is vacuous today (2.5,
   2.6). Options include dropping `Clone`, a roster of destructive operations the guard
   requires to take `Confirmed` by value, and a guard on where `by_user` may be called (the
   render crates' confirm handler) — or stating each as a `destructive-ops-reviewer`
   obligation.

6. **O3 / L5: auto-stash before a destructive working-tree operation?** The reflog does
   nothing for an uncommitted edit; `ui.md` puts "stash first" in the discard dialog as an
   option rather than a default. Must be decided, not inherited (brainstorm L5). If chosen,
   stash push is the mechanism and its own failure modes (hooks do not run; untracked files
   need `--include-untracked`; `--staged` absent below its version) apply.

7. **#45 item 1: a cancelled or unwatched write.** Adopt the before/after obligation and
   reword "cancelled" to "may have taken effect"? For local writes the unconditional
   post-ending refresh already shows the truth; the message is what is open.

8. **#45 item 2: ceilings on writes.** Make `collect`/`finish_within` read-only at the type
   (recommended (a)), or state it. No planned local verb needs stdout except possibly `clean
   -n` (which may be a read, see 15).

9. **#45 item 3, #44 item 2, #48 item 2: closing with a local write in flight.** Refuse and
   say so, ask, or close and report; whether the patience close still forces; whether a close
   that `SIGKILL`s an apply keeps the window open to name `index.lock`; whether a forced close
   kills every registered group first.

10. **#44 item 1 and #19: locks in the window.** Carry `present_locks`/`stranded_locks`
    structurally on every local outcome and draw them; and whether this packet builds a
    confirmed "remove stale lock" (`index.lock` is the lock users meet).

11. **Askpass on a local write.** Does the local lane begin a channel operation (token) per
    invocation, and does the window show a prompt for it? Children of local writes that may
    ask: an LFS smudge fetching objects during discard/restore (i), an SSH-signing key without
    an agent (i), a hook that runs `git fetch` (i). Today the window refuses every prompt
    unless a fetch is in flight (`session.rs:160`).

12. **Commit signing and the hook environment (#18).** Inherit `GNUPGHOME`, `DISPLAY`,
    `WAYLAND_DISPLAY` (and `GPG_TTY` is meaningless without a tty)? Is the roster right for
    hooks (`VIRTUAL_ENV`, `SHELL`, `USER`, `TERM`…), or is a commit's environment a different
    roster? Every entry is a deliberate leak (destructive-ops check 9). Evidence: 3.9.

13. **Commit message transport and cleanup.** `-F -` keeps the message out of argv and so out
    of the command log, and git gives hooks no stdin (verified), so the message survives a
    `pre-commit` hook; `-m` puts it in the log verbatim. With `-F`, git's default cleanup is
    whitespace-only, so `#` lines are kept (verified): pass `--cleanup=strip`, follow the
    user's `commit.cleanup`, or match Fork (unresearched)? Sign-off (`--signoff`) is in the
    mockup.

14. **Hook output: progress, and what a success shows.** The runner streams stderr lines; the
    window has no generic progress surface, and a successful commit's retained stderr is
    dropped (`Output::stderr` is dead outside tests). A long `pre-commit` needs "still
    running", elapsed time and cancel (`processes.md`: no timeout, show how long). Ties to
    #46 if the tail is drawn.

15. **`git clean`: what the prompt lists, and is a dry run a read?** The untracked list is
    already in `LocalChanges` from status (one entry per file, or a collapsed directory per
    `status.showUntrackedFiles`); `git clean -n` would be a fourth porcelain read mode needing
    the user's acceptance and a guard row (`the_porcelain_reads_are_the_three_named_queries`).
    Directories (`-d`), nested repositories (`-ff`) and ignored files (`-x`, cf. #62) decide
    what "clean" may remove and what the prompt must count — computed before the prompt
    and acted on unchanged (destructive-ops check 4).

16. **Which verbs exactly.** Unstage: `git restore --staged` (2.23+) vs `git reset -q --`;
    unstage of lines: `git apply --cached -R -`; discard of lines: `git apply -R -`; discard
    file: `git restore --worktree` vs `checkout --`; stage file: `git add` with
    `--pathspec-from-file=- --pathspec-file-nul` for long lists; `--check` pass or not. Each
    with `--end-of-options`/`--` and a test at the floor.

17. **Stash apply/pop conflicts.** An exit-1 stop is a state, not a failure, and stderr may not
    be read: what repository state decides "stopped with conflicts" (unmerged entries, the
    stash still in the list), and what the window offers then? Also: address a drop/pop by
    `stash@{n}` checked against `StashEntry::commit` just before, since the index shifts when
    anything pushes a stash (i).

18. **Refresh after a local write.** Keep the unconditional full refresh (refs + ahead/behind
    + status) or narrow it by `Invalidated` (a stage needs only status)? And how the window
    avoids drawing a status that began before the write ended (status is never superseded,
    4.3) — e.g. numbering statuses against a write generation, or letting a write's refresh
    supersede a running status (a reversal of R10.3 as amended for this case only).

19. **Hooks mutate under a running commit.** While a commit is inside its hooks, does Cairn
    suspend its own refreshes (focus, F5) and any queued index write, so it neither draws a
    hook's intermediate index nor collides with it on `index.lock`?

20. **The operation log.** What it records (successes only, as `Performed` is built today, or
    every ending), what it keys on (time, repository, target, ids before/after so an amend
    names the commit it replaced), where it lives (memory only, like the command log, or
    persisted — "the operation log can quote them afterwards" across a restart?), and whether
    it is #41's command log, a separate drawer (`ui.md`), or one view joining both. The
    command log records argv and not stdin, so a patch or a message reaches neither unless
    the operation log keeps it.

21. **The reflog view (L4).** Which reflogs (`HEAD`, the current branch, every branch, the
    stash), what actions it offers (show only, or restore — which would be a write: checkout,
    branch, reset, each its own `Confirmed` question), and the gix reflog reader's 4 KiB
    line window (read whole, as the stash list does).

22. **`.gitignore` editing.** A direct file write (no git verb exists), so: does it live in
    `ops/` (declaring `working_tree`, logged as an operation) as the one mutation not made by
    `git`, amending D1; does it also offer `.git/info/exclude`; is overwriting a user's edit
    made outside Cairn possible (re-read and compare before writing); and does a guard learn
    that file writes belong in `ops/` (none sees `std::fs::write` today).

23. **#66, the first Cairn-initiated local write.** Whether this packet builds the index
    refresh (automatic only when no other write is queued, refusing on an existing lock, per
    the issue), now that the lane exists; it is "work Cairn starts on its own", which the
    design says is skipped while its lane is busy and never prompts.

24. **The cancel's identity (#47).** Build the local lane's cancel tied to an operation id
    from the start (the issue's recommendation "also suits a second verb in the lane"), and
    retrofit fetch's, or leave fetch's races as filed.
