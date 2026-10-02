# Precedent study — how established clients spawn and manage `git`

Evidence record, saved in full. Commissioned 2026-10-02 by the process-manager packet planning. Historical: never retro-edited.

## Method and what was read

Source was read, not summarised from blogs, wherever source exists. Each
repository was shallow-cloned (`--depth 1`, sparse) on 2026-10-02 and read at
the commit below. File paths are relative to each repository root. Line
numbers are deliberately omitted (they rot); every claim cites a file and a
symbol. Anything not confirmed from source or a primary page is marked
**UNVERIFIED**.

| Target | Repository | Commit read (date) |
| --- | --- | --- |
| dugite | `github.com/desktop/dugite` | `417dab855025` (2026-08-13) |
| GitHub Desktop | `github.com/desktop/desktop` | `3754e26d1f02` (2026-10-02) |
| VS Code git extension | `github.com/microsoft/vscode`, `extensions/git/src/` | `d2fe6d3bf903` (2026-10-02) |
| Jujutsu | `github.com/jj-vcs/jj`, `lib/src/` | `0cb02a837f28` (2026-10-01) |
| GitButler | `github.com/gitbutlerapp/gitbutler`, `crates/` | `5d3db3007943` (2026-10-02) |
| lazygit | `github.com/jesseduffield/lazygit`, `pkg/` | `ff375b124d14` (2026-09-30) |
| Magit | `github.com/magit/magit`, `lisp/` | `8802df2ebcde` (2026-10-02) |
| git itself (for lock and editor semantics) | `github.com/git/git` | `c46c1e37724f` (2026-10-01) |
| process-wrap | `github.com/watchexec/process-wrap` | `ca45003a831a` (2026-09-23), crate 10.0.1 |
| shared_child | `github.com/oconnor663/shared_child.rs` | `7c446307057` (2026-09-03), crate 1.1.2 |
| duct | `github.com/oconnor663/duct.rs` | `efdf771498eb` (2026-09-20), crate 1.1.2 |
| nix | `~/.cargo/registry/src/*/nix-0.31.3` (the version Cairn's workspace pins) | registry copy |
| Rust std | toolchain 1.97.1 `library/std/src` (the pinned toolchain's `rust-src`) | — |

Sublime Merge and Fork are closed source; only their public pages were read.
Local `git --version` at time of writing: 2.56.0.

---

## 1. GitHub Desktop + dugite

dugite is the process layer (`lib/exec.ts`, `lib/spawn.ts`,
`lib/git-environment.ts`, `lib/errors.ts`); Desktop wraps it in
`app/src/lib/git/core.ts` (`git()`), `app/src/lib/git/spawn.ts` (`spawnGit`),
the credential "trampoline" (`app/src/lib/trampoline/`) and, recently, a hook
proxy (`app/src/lib/hooks/`).

**Execution model.** Two shapes. `exec(args, path, options)` is one-shot over
Node's `execFile`: buffers stdout and stderr in full, resolves with
`{stdout, stderr, exitCode}`; rejects only when the process could not start
(string `err.code` such as `ENOENT`), never on a non-zero exit. `spawn(args,
path, opts)` hands back the raw `ChildProcess` for streaming (used for
progress-reporting operations through `processCallback`). Desktop's `git()`
adds: an operation `name` for tracing (`GitPerf.measure`), per-call
`successExitCodes` (default `{0}`) and `expectedErrors` (a set of parsed
`GitError` kinds the caller will handle). No long-lived batch process anywhere
in `app/src/lib/git/`: many-path operations use `--stdin -z` instead
(`update-index.ts`, `checkout-index.ts`).

**Cancellation.** dugite's `exec` accepts an `AbortSignal` and `killSignal`
(default `SIGTERM`), passed to Node, which signals the direct child only. In
Desktop, no `app/src/lib/git/*.ts` or `app/src/lib/stores/*.ts` call site
passes a signal or calls `.kill(` (grep, this commit): fetch, pull, push and
clone are **not user-cancellable**. The only abort is for hooks (below).

**Stdin.** `exec` writes `options.stdin` and immediately `end()`s it;
`ignoreClosedInputStream` swallows `EPIPE`/`EOF`/`ECONNRESET` on stdin so a
child that exits without reading (e.g. a bad patch to `apply`) surfaces as its
real exit code and stderr, not as an unhandled stream error (rationale cites
`desktop/desktop#4027`). Paths for staging go in on stdin, NUL-separated
(`update-index -z --stdin`).

**Output limits.** `maxBuffer` defaults to Infinity in dugite; Desktop sets
it to `kStringMaxLength` for string-encoded calls and Infinity for buffer
calls, and on overflow Node kills the child and Desktop rethrows
`ERR_CHILD_PROCESS_STDIO_MAXBUFFER` naming the operation. Separately, Desktop
keeps a **256 KiB** combined stdout+stderr "terminal output" ring
(`pushTerminalChunk`, `terminalCapacity`) used for error messages and for a
live terminal view (`onTerminalOutputAvailable`); it logs at most the last
1 KiB on an unexpected failure.

**Environment.** dugite `setupEnvironment` starts from `process.env`
(inherits everything) and sets only what the **bundled** git needs:
`GIT_EXEC_PATH`, `GIT_CONFIG_SYSTEM` (its own system gitconfig on macOS and
Linux), `GIT_TEMPLATE_DIR`, `PREFIX`, `GIT_SSL_CAINFO` (Linux, bundled CA
bundle). Desktop's `git()` adds `TERM=dumb` (comment: so git does not treat a
terminal-launched Desktop as a smart terminal, citing git's `editor.c`), and
the trampoline env: `GIT_ASKPASS=''`, a credential helper injected through
`GIT_CONFIG_PARAMETERS` (`'credential.helper=' 'credential.helper=desktop'`,
chosen over `GIT_CONFIG_COUNT/KEY/VALUE` because of a Python hook-manager bug,
`desktop/desktop#18945`, and over `-c` so that filters such as LFS inherit it),
`GIT_USER_AGENT`, an SSH env, and a per-invocation `DESKTOP_TRAMPOLINE_TOKEN`.
`envForAuthentication()` sets `GIT_TERMINAL_PROMPT=0`. **No locale pinning**:
`LC_ALL`/`LANG`/`LANGUAGE` appear nowhere in `app/src` or dugite `lib/`
(grep). Whether the bundled git is built without gettext — which would make
that moot — is **UNVERIFIED**. `GIT_OPTIONAL_LOCKS` is not set; instead
`getStatus` passes `--no-optional-locks` (`app/src/lib/git/status.ts`).

**Concurrency / index.lock.** No queue and no lock. Per-repository boolean
guards in `AppStore`: `withIsCommitting` and `withPushPullFetch` **drop** a
second request while one is in flight ("Don't allow concurrent network
operations") rather than queue it. No retry on lock contention: a
`LockFileAlreadyExists` parse produces the message "A lock file already exists
in the repository, which blocks this operation from completing."
(`getDescriptionForError` in `core.ts`); `ConfigLockFileAlreadyExists` is
parsed and its path extracted (`parseConfigLockFilePathFromError`).

**Error classification.** `GitErrorRegexes` in dugite `lib/errors.ts`: about
60 regexes over English stderr (then stdout) mapped to a `GitError` enum —
auth failures (HTTPS vs SSH), remote hang-ups, host down, conflicts per
operation, non-fast-forward, lock files, dubious ownership, and nine
GitHub-server-specific `GH00x` rejections. `parseError` returns the **first**
match in object-key order. Desktop then decides: exit code in
`successExitCodes`, or parsed error in `expectedErrors`, returns the result;
anything else is logged and thrown as `GitError` carrying args and terminal
output.

**Observability.** No user-visible command log. Developer-side: `GitPerf`
timings keyed by `"<name>: git <args>"`, `log.error` on unexpected exits,
`GIT_TRACE` opt-in from `localStorage` (`authentication.ts`).

**Hooks and editors.** Non-interactive continuation uses `GIT_EDITOR=':'`
(`rebase.ts`, `cherry-pick.ts`); config edits use `GIT_EDITOR='printf %s'`
(`config.ts`). Interactive rebase supplies the todo through
`-c sequence.editor=cat "<todo file>" >` with `GIT_SEQUENCE_EDITOR` unset.
Hooks: when enabled, `withHooksEnv` copies a `process-proxy` binary into a
temp dir once per hook name, points git at it with
`GIT_CONFIG_PARAMETERS='core.hooksPath=<tmp>'`, and serves the proxy over a
token-checked localhost socket; the proxy then runs the user's real hook via
`git hook run <name> [--to-stdin=<file>]` **inside the user's login-shell
environment** (`get-shell-env.ts`), streams its stderr back, reports
`onHookProgress({status: 'started', abort})`, and lets the UI abort it
(`hooks-proxy.ts`). It filters which `GIT_*` variables reach the hook and
strips its own (`GIT_CONFIG_PARAMETERS`, `GIT_ASKPASS`, `GIT_SSH_COMMAND`,
...), and ignores failures of hooks whose exit git does not act on
(`post-commit`, `post-checkout`, ...).

**Known issues seen in source.** stdin EPIPE (above); Windows `PATH` case
sensitivity in the hook proxy; the `GIT_CONFIG_COUNT` Python incompatibility.

## 2. VS Code built-in git extension

Files: `extensions/git/src/git.ts` (`Git.spawn`, `Git._exec`, `Git.stream`,
module-level `exec`, `cpErrorHandler`, `getGitErrorCode`),
`repository.ts` (`Repository.run`, `retryRun`, `whenIdleAndFocused`),
`operation.ts` (`OperationManager`, the `Operation` table),
`decorators.ts`, `askpass.ts`, `gitEditor.ts`.

**Execution model.** `Git.spawn` is the single `cp.spawn` site. `_exec` is
one-shot (collect both streams fully, resolve on `exit` + both `close`s);
`stream` returns the live child for incremental parsing (status, `show`,
`hash-object`). No long-lived batch process; `cat-file -s` is one-shot.
Argument lists for `add`/`rm` are chunked at `MAX_CLI_LENGTH = 30000`
characters.

**Cancellation.** A VS Code `CancellationToken` races the result:
`child.kill()` (Node default `SIGTERM`, direct child only) and rejects with
`CancellationError`. `getStatus` additionally **kills git itself once it has
parsed `limit` entries (default 10 000)** and returns `didHitLimit: true` — a
deliberate output cap enforced by killing the producer.

**Stdin.** Default `stdio: ['ignore', pipe, pipe]`; with `options.input`
it writes and ends stdin. Commit messages go in on stdin
(`commit --quiet --allow-empty-message --file -`), never through an editor or
argv.

**Output limits.** None in `_exec` (unbounded `Buffer[]`). The status limit
above is the only cap.

**Environment.** Inherits `process.env`, then layers the extension env
(askpass, editor) and per-call env, then **forces** `VSCODE_GIT_COMMAND`,
`LANGUAGE=en`, `LC_ALL=en_US.UTF-8`, `LANG=en_US.UTF-8`, `GIT_PAGER=cat`.
Askpass (`askpass.ts`): `GIT_ASKPASS` → a shell script that runs VS Code's
Node with `askpass-main.js` and talks IPC; `SSH_ASKPASS` + 
`SSH_ASKPASS_REQUIRE=force`. `GIT_TERMINAL_PROMPT` is **not** set anywhere in
the extension (grep). `getStatus` sets `GIT_OPTIONAL_LOCKS=0`; nothing else
does. (What git does when `en_US.UTF-8` is not generated on the host —
presumably falls back to untranslated messages — is **UNVERIFIED**.)

**Concurrency / index.lock.** No mutual exclusion between git processes.
`OperationManager` records running operations by kind, each with flags
`blocking` (disables UI commands while running: commit, pull, push, sync),
`readOnly`, `remote`, `retry`, `showProgress`. `Repository.run` wraps every
operation: start → `retryRun` → refresh model state if not read-only → end.
`retryRun` retries **up to 10 attempts with quadratic backoff
(`attempt² × 50 ms`)** when the parsed code is `RepositoryIsLocked`
(stderr "Another git process seems to be running..."), and, only for
operations flagged `retry` (fetch, pull, sync), on `CantLockRef` and
`CantRebaseMultipleBranches`. Background status refresh is debounced (1 s),
`@throttle`d (one in flight, at most one queued — `_throttle` in
`decorators.ts`), and waits in `whenIdleAndFocused` until no non-read-only
operation is running and the window is focused. `@sequentialize` chains calls
of one method.

**Error classification.** Non-zero exit always rejects with `GitError`;
`getGitErrorCode` maps ~17 English stderr regexes to `GitErrorCodes`
(locked, auth failed, not a repo, dubious ownership, dirty tree, branch
exists, ...). `ENOENT` on spawn is reported by `cpErrorHandler`.

**Observability.** A **user-visible "Git" output channel**
(`window.createOutputChannel('Git', { log: true })`): every command is logged
as `> git <args> [<ms>ms]` with `(cancelled)` when killed, stderr always, and
stdout only for subcommands listed in the `git.commandsToLog` setting.
`OperationManager` traces start/end at trace level.

**Hooks and editors.** Hooks run inside git, normally. The editor is opt-in
(`git.useEditorAsCommitInput`): `GIT_EDITOR` points at `git-editor.sh`, which
runs VS Code's Node with `git-editor-main.js` over IPC; the extension opens
the message file as an editor tab and answers when the tab closes.
`GIT_EDITOR=true` is used where no edit is wanted.

## 3. Jujutsu — the git subprocess backend

File: `lib/src/git_subprocess.rs` (`GitSubprocessContext`), options in
`lib/src/git.rs` (`GitSubprocessOptions`), `lib/src/subprocess_util.rs`.
Minimum git `2.42.0` (comment lists why: `fetch --no-write-fetch-head`,
`fetch --porcelain`, `worktree add --orphan`).

**Execution model.** One-shot `std::process::Command` per verb (fetch, push,
branch prune, remote show, worktree add/repair/prune). `create_command`
prefixes every call with `-c core.fsmonitor=false` (macOS fsmonitor daemon
interference, `jj-vcs/jj#6440`), `-c submodule.recurse=false` (`#7565`),
`--git-dir <path>`.

**Streaming and pipes.** `wait_with_progress` uses `thread::scope`: one scoped
thread `read_to_end`s stdout while the calling thread reads stderr
incrementally, then joins and `wait`s — both pipes are always drained, so a
full pipe cannot deadlock. stderr is split on `\r` **or** `\n`
(`read_until_cr_or_lf`); progress lines (`Receiving objects:`, `Resolving
deltas:`, `remote: Counting objects:`, `remote: Compressing objects:`) parse
`(n/total)` into `GitProgress`; other `remote: ` lines go to a sideband
callback (e.g. "create a pull request" URLs); once a line starts with an error
prefix (`error: `, `fatal: `, `usage: `, `unknown option: `) the rest of
stderr is read raw for the caller to classify. Callback errors are swallowed
(`.ok()`) so the UI cannot break the read loop.

**Cancellation.** None in the library: no kill, no timeout. As a CLI, jj
relies on the terminal delivering `SIGINT` to the whole foreground process
group.

**Stdin.** `Stdio::null()` always (and `drop(child.stdin.take())`).
Prompts are deliberately left to the inherited **console/tty**: on Windows,
`suppress_console_window` sets `CREATE_NO_WINDOW` only when jj has no console,
because an invisible console makes ssh prompt where nobody can answer
("leaving `jj` waiting forever").

**Environment.** Inherited, plus caller-supplied
`GitSubprocessOptions::environment` (documented as the way to set
`GIT_ASKPASS` or `GIT_TRACE` "without unsafe code and process-wide state").
Locale: `env_remove("LC_ALL")`, `env_remove("LANGUAGE")`,
`LC_MESSAGES=C` — comment: not `LC_ALL=C` "because it would change the
encoding", and "C.UTF-8 locale isn't always available". No
`GIT_TERMINAL_PROMPT`, no `GIT_OPTIONAL_LOCKS` (jj's own operation log, not
git's index, is its concurrency story).

**Error classification.** Machine output first: `fetch --porcelain` and
`push --porcelain` are parsed for per-ref outcomes. stderr is classified by
**first-line prefix matching** (`parse_no_such_remote`,
`parse_no_remote_ref`, `parse_no_remote_tracking_branch`,
`parse_unknown_option` → "requires git >= 2.42"); unknown failures become
`External(stderr)`. Typed `thiserror` enum `GitSubprocessError` (`Spawn`,
`SpawnInPath`, `Wait`, `UnsupportedGitOption`, `External`).

**Hooks.** `push --no-verify`: jj does not run hooks (comment cites
`#3577`, `#405`).

**Observability.** `tracing::debug!(cmd = ?git_cmd, ...)` per spawn.

## 4. GitButler (gix + git CLI hybrid)

Files: `crates/gitbutler-git/src/executor/mod.rs` (`GitExecutor`),
`executor/tokio/mod.rs` (`TokioExecutor`), `executor/tokio/unix.rs`
(askpass socket), `src/repository.rs` (fetch/push/clone with the auth
harness), `crates/but-core/src/sync.rs` + `crates/but-ctx/src/access.rs`
(repository locks), `crates/gitbutler-repo/src/hooks.rs`.

**Execution model.** Only network verbs go through the git CLI. `GitExecutor`
is an `unsafe trait` (the unsafety is about askpass socket invariants, not
process spawning). `execute` prepends `-c protocol.version=2 --no-pager` and
sets `GIT_TERMINAL_PROMPT=0` and **`LC_ALL=C`** ("Force English. We need this
for parsing output."). `TokioExecutor::execute_raw` is one-shot
`tokio::process::Command::output()`, returning `(exit_code, stdout, stderr)`
(lossy UTF-8, trimmed); `Err` only if the process could not run.

**Cancellation.** `cmd.kill_on_drop(true)`: dropping the future `SIGKILL`s
the direct child (tokio semantics) — the executor docs say children "should be
killed if the future is dropped". No grace period, so a killed git cannot
remove its lock files (see git §, below).

**Credentials.** `execute_with_indirect_askpass` creates a per-call unix
socket (mode 0600, random name in `temp_dir`), sets `GITBUTLER_ASKPASS_PIPE`,
`GITBUTLER_ASKPASS_SECRET`, `SSH_ASKPASS`, `SSH_ASKPASS_REQUIRE=force`, and
`select!`s between the child finishing and a socket connection. Each
connection is authenticated three ways: peer PID → executable path via
`sysinfo` (a full process-table refresh, "pretty expensive"), `stat`
device+inode must equal the askpass binary's, and the shared secret line.
`GIT_SSH_COMMAND` is built from env/config/`GIT_SSH`, defaulting to
`ssh -o StrictHostKeyChecking=accept-new -o KbdInteractiveAuthentication=no`.
`debug_log_sanitised_git_cmd` strips the askpass variables before
`tracing::debug!` of the command.

**Concurrency.** In-process, per git-dir **fair `parking_lot::RwLock`**:
`exclusive_worktree_access` (writers, prioritised) vs `shared_worktree_access`
(readers), held via guards; plus a best-effort inter-process file lock, and a
whole-project inter-process lock held while a UI observes the project
(`try_exclusive_access`). A debug-build `panic_if_locked_in_debug` catches
re-entrant locking.

**Error classification.** Ad-hoc substring checks on lowercased stderr
(`couldn't find remote ref`, `permission denied`) → typed errors; failure
logs `tracing::error!` with stdout and stderr.

**Hooks.** GitButler creates commits itself, so it **re-implements hook
execution**: `commit-msg`/`pre-commit`/`post-commit` via the `git2_hooks`
crate, `pre-push` hand-spawned through `gix::command::prepare` with refspec
lines on stdin (tolerating `BrokenPipe` because "hooks may exit before reading
stdin"). The pre-push code carries the comment "THIS IS WRONG: but is correct
in the common case" about deducing the local ref; `pre_commit_with_tree`
backs up and restores the index around the hook under its own lock file.
This is direct evidence for the cost Cairn's D1 avoids by letting git run
hooks.

## 5. lazygit

Files: `pkg/commands/oscommands/cmd_obj.go` (`CmdObj`),
`cmd_obj_runner.go` (`cmdObjRunner`), `pkg/commands/git_cmd_obj_builder.go`,
`pkg/commands/git_cmd_obj_runner.go` (`gitCmdObjRunner`),
`pkg/commands/git_commands/git_command_builder.go`, `file_loader.go`,
`rebase.go`, `sync.go`, `pkg/tasks/tasks.go`, `pkg/app/daemon/daemon.go`,
`pkg/gui/background.go`, `pkg/gui/types/common.go`.

**Execution model.** A `CmdObj` builder (args, env, wd, stdin, flags
`DontLog`, `StreamOutput`, `SuppressOutputUnlessError`, `UsePty`,
`IgnoreEmptyError`, optional mutex, credential strategy) run by one runner
with four shapes: `Run`, `RunWithOutput` (combined), `RunWithOutputs`
(separate), `RunAndProcessLines` (line callback that can stop early by
**closing the stdout pipe** so git dies of `SIGPIPE`). The git builder pins
every command's working directory and env to the repository it was created
for, because lazygit `chdir`s on repository switch while old background work
may still be running.

**Cancellation.** Main-view preview tasks (`pkg/tasks/tasks.go`):
when the selection changes, `cmd.Terminate()` (`SIGTERM`,
`TerminateProcessGracefully`) and close the pipe; the line scanner runs in its
own goroutine because "on windows if running git through a shim, we sometimes
kill the parent process without killing its children, meaning the scanner
blocks forever" — they accept a leaked goroutine over a frozen view. Task
manager `Close` gives up after 3 s ("cannot kill child process"). Credential
"fail" strategy terminates by closing the pty. No user cancel for
push/pull/fetch; no process groups (`Setpgid`/`Setsid` absent).

**Back-pressure as a feature.** Preview tasks read lines **on demand** as the
user scrolls (`readRequests`), so a huge `git log`/`git diff` simply blocks on
a full pipe instead of being buffered; lines are truncated at
`bufio.MaxScanTokenSize` (`ScanLinesAndTruncateWhenLongerThanBuffer`).

**Credentials.** Not askpass: commands with a credential strategy run **in a
pty**, with `LANG=C LC_ALL=C LC_MESSAGES=C`, and the output is scanned byte by
byte against prompt regexes (`Password:`, `Username for '...':`,
`Enter passphrase for key '...':`, PIN, `2FA Token`); a match pauses the UI
task, prompts, and writes the answer to the pty. Background fetch uses
`FailOnCredentialRequest` + `SuppressOutputUnlessError` + `DontLog`; a few
commands set `GIT_TERMINAL_PROMPT=0` (`commit.go`, `branch.go`).

**Environment.** Every git command gets **`GIT_OPTIONAL_LOCKS=0`**
(`OptionalLocksEnvVar`; rationale: never contend for `index.lock`, neither
with each other nor "with git commands the user runs in a terminal"). The one
opt-out is the **foreground** files refresh, which removes the variable so
`git status` may persist its refreshed stat cache ("keeps subsequent status
calls fast"); background refreshes keep it suppressed. Locale pinning only on
pty/credential and rebase commands.

**Lock retry.** `gitCmdObjRunner.retryOnLockError`: up to **7 attempts**,
delay starting at **20 ms and doubling** (a bit over 1 s total), when output
or error contains `index.lock` or `cannot lock ref` (bare fragment so linked
worktrees and submodules match). Clones the `CmdObj` per attempt.
`RunAndProcessLines` is not retried. Upstream issue `jesseduffield/lazygit#5906`
(2026-08-07, v0.64.0, macOS large repo: checkout vs status colliding on
`index.lock`) is consistent with this code being the response, but the link
between the two is **UNVERIFIED** (no PR read).

**Mutexes.** `CmdObj.WithMutex` exists, but at this commit no production code
calls it; `Mutexes` holds only `SubprocessMutex` (suspend the TUI to run an
interactive subprocess) and `PtyMutex`. An older `SyncMutex` serialising
push/pull/fetch is remembered from earlier lazygit versions but is **not
present** now and its history is **UNVERIFIED**. Background refreshes can be
paused (`PauseBackgroundRefreshes`, counter-based).

**Editors.** lazygit sets `GIT_EDITOR`/`GIT_SEQUENCE_EDITOR` to **its own
executable** with `LAZYGIT_DAEMON_KIND=<n>` and an encoded instruction
(`daemon.go`): the re-executed lazygit rewrites the todo (move up/down, drop,
fixup, write todo) or exits immediately, then git continues. Pull uses
`GIT_SEQUENCE_EDITOR=:`.

**Observability.** A **visible command log panel** (`guiIO.logCommandFn`)
for every command not marked `DontLog`; logrus timings with wall and CPU time.

## 6. Magit

Files: `lisp/magit-process.el`, `lisp/magit-git.el`.

**Execution model.** Synchronous (`magit-call-process`, `magit-process-git`,
`magit-process-file` → Emacs `process-file`/`call-process`) for reads and
quick writes, which **block Emacs**; asynchronous (`magit-start-process`,
`magit-run-git-async`) for long or interactive commands, with a sentinel that
refreshes the originating buffer and the status buffer on exit. Every
invocation is prefixed with `magit-git-global-arguments`: `--no-pager
--literal-pathspecs -c core.preloadIndex=true -c log.showSignature=false
-c color.ui=false -c color.diff=false -c diff.noPrefix=false`.

**Prompts.** Async processes use a **pty** by default
(`magit-process-connection-type`; pipes when input is supplied, because a pty
sets `icrnl` and would alter the input). `magit-process-filter` drops
everything before the last `\r` (progress redraw), then matches yes/no,
username and password regexes (`magit-process-password-prompt-regexps`,
including git-credential-manager `Token:`, Yubikey and PIN prompts) and
answers through the minibuffer, optionally from `auth-source`. `C-g` in such a
prompt kills the process.

**Cancellation.** In the process buffer, `magit-process-kill`: first
invocation (after confirmation) `interrupt-process` (`SIGINT`), a second
invocation `kill-process` (`SIGKILL`) — a user-driven two-step escalation.
(Whether Emacs delivers these to the pty's process group or only the child is
**UNVERIFIED** here.)

**Environment.** `INSIDE_EMACS=<ver>,magit` prepended; on Windows
`i18n.logOutputEncoding=UTF-8`; no locale, no `GIT_OPTIONAL_LOCKS`, no
`GIT_TERMINAL_PROMPT` (prompts are handled through the pty).

**Editors.** `with-editor`: `magit-run-git-with-editor` exports `GIT_EDITOR`
(and a hook editor variable) so git opens the message or todo in the running
Emacs and waits.

**Errors.** One-line summary extracted by `magit-process-error-message-regexps`
(`^(?:error|fatal|git): (.*)$`, `^Cannot rebase:`), after
`magit-process-remove-bogus-errors` strips noise such as the editor "Canceled
by user".

**Observability.** The canonical precedent for a visible log: the **process
buffer** (`$`) holds one section per invocation (command, output, exit), kept
to `magit-process-log-max` (32) sections — when exceeded the older half is
dropped, but sections of still-running processes are never removed;
`magit-process-popup-time` can pop it after N seconds;
`magit-toggle-git-debug` and `magit-toggle-subprocess-record` add more.

## 7. Sublime Merge and Fork (closed source; public pages only)

**Sublime Merge.** The launch post (sublimetext.com blog, "Sublime Merge -
Git, Done Sublime", 2018-09-20) says: "We have a custom implementation of Git
for reading repositories" and that they defer to git for anything that
mutates the repository (staging, committing, checking out). This is the same
read/write split as Cairn's D1. A `git_binary` preference selects bundled
(Windows, macOS) or system git (per search results summarising
sublimetext.com docs; the docs page itself was not fetched — **UNVERIFIED**
in detail). How it logs commands, handles `index.lock`, or times out hooks:
**UNVERIFIED**, nothing public found.

**Fork.** Release notes (git-fork.com/releasenotes, as summarised by a fetch
tool — spot-check before quoting): bundles its own git instance (1.0.52,
2017) and lets the user choose a custom one (1.0.49); "Do not block UI during
fetch, pull, and push operations" (1.0.33, 2016); **cancel Fetch/Pull/Push/
Clone, automatic background fetch and an Activity Manager** (1.0.67, 2018);
show pre-commit hook output interactively (1.0.69); show all dialog commands'
output in the Activity Manager (1.0.88); "Do not start automatic fetch if
other fetch is running" (2.27, 2023); background fetch interval 10 min
(2.36); option for verbose git output (2.62, 2026-01). Issue
`fork-dev/Tracker#1633` (2022-06-29): "Commit and Push" immediately after
focusing the window fails with `index.lock` exists; the reporter attributes
it to the focus-triggered automatic fetch/refresh not being waited for, and
retrying succeeds; no maintainer diagnosis visible. Whether Fork reads with
libgit2 or its own reader, and how it kills cancelled processes:
**UNVERIFIED**.

## 8. Rust process crates, against a std + `nix`, no-`unsafe` design

**Rust std (1.97.1).** `std::os::unix::process::CommandExt::process_group(pgid)`
is **safe and stable since 1.64**; std applies it with
`posix_spawnattr_setpgroup` on the `posix_spawn` path or `setpgid(0, pgid)` in
the fork path (`library/std/src/sys/process/unix/unix.rs`) — it works on
macOS and Linux and needs no `pre_exec`. `pre_exec` is `unsafe fn`. std resets
`SIGPIPE` to `SIG_DFL` in the child (same file), so a child git that writes
to a closed pipe dies of `SIGPIPE` as it would from a shell, while the Rust
parent (which ignores `SIGPIPE`) sees `BrokenPipe` on a write to a dead
child's stdin. `Child::kill` sends `SIGKILL`; `Child::wait`/`kill` take
`&mut self`.

**nix 0.31.3** (already a workspace dependency, features `signal`,
`process`). Safe wrappers: `sys::signal::kill(pid, sig)` and
**`sys::signal::killpg(pgrp, sig)`**; `sys::wait::waitpid`; `sys::wait::waitid
(Id::{Pid,PGid,PIDFd}, flags)` with `WNOWAIT` — but `waitid` is gated to
Linux/Android/FreeBSD/Haiku, **not macOS**. `sys::prctl::set_pdeathsig` and
`set_child_subreaper` are safe functions, but `set_pdeathsig` must run **in
the child** (i.e. inside `pre_exec`), so it is out of reach without `unsafe`;
`set_child_subreaper` runs in the parent (Linux only) and makes orphaned
grandchildren reparent to Cairn rather than init.

**process-wrap 10.0.1** (watchexec; successor of `command-group`).
`ProcessGroup::leader()` uses std's safe `process_group`; signalling uses
`nix::killpg`; but its group wait loops `libc::waitpid(-pgid, ...)` inside
`unsafe` (to recover the raw status), `ProcessSession` uses `pre_exec(setsid)`
(unsafe), `ResetSigmask` uses `pre_exec` (unsafe), and the Windows job-object
backend is `unsafe` Win32. Note that `waitpid(-pgid)` reaps only the caller's
own children in that group; git's grandchildren (ssh, hooks, credential
helpers) are not the caller's children and are reaped by their own parent or
by init/a subreaper.

**shared_child 1.1.2.** Solves "kill while another thread waits":
`std::process::Child` needs `&mut` for both, because `waitpid` frees the PID
and a concurrent signal could hit a recycled PID. shared_child waits with
`libc::waitid(P_PID, ..., WEXITED | WNOWAIT)` — blocks until exit **without
reaping**, so `kill` stays safe — then reaps under a lock. Uses `unsafe`
libc calls; the timeout feature handles `SIGCHLD` via the `sigchld` crate.

**duct 1.1.2.** `#![forbid(unsafe_code)]` itself, built on shared_child
(which is not). Notable mechanisms: `Handle::kill` kills only direct children
— its docs call out that grandchildren holding inherited pipes keep IO threads
(and so `wait`) blocked, and its own example warns that bash would hang
because the `sleep` grandchild survives the kill; dropped-but-running children
go onto a global `LEAKED_CHILDREN` list reaped opportunistically on the next
spawn, copying CPython `subprocess`'s strategy.

## 9. git's own semantics that the above depend on

- **Lock acquisition does not wait.** `hold_lock_file_for_update` and
  `repo_hold_lock_file_for_update` call the `_timeout` variant with **0**
  (`lockfile.h`), and `repo_hold_locked_index` uses the latter
  (`repository.c`): a second writer on `index.lock` fails immediately.
  Configurable retries exist only for refs (`core.filesRefLockTimeout`,
  default 100 ms), `packed-refs` (`core.packedRefsTimeout`, 1000 ms) and
  config (`core.configLockTimeout`, 1000 ms) (`Documentation/config/core.adoc`).
- **Lock files are removed on catchable signals only.** `tempfile.c`
  registers `remove_tempfiles_on_signal` via `sigchain_push_common`, which
  covers `SIGINT`, `SIGHUP`, `SIGTERM`, `SIGQUIT`, `SIGPIPE` (`sigchain.c`).
  `SIGKILL` leaves `index.lock` (and ref locks) behind — the classic stale
  lock.
- **`GIT_OPTIONAL_LOCKS=0`** (= `git --no-optional-locks`): "complete any
  requested operation without performing any optional sub-operations that
  require taking a lock", e.g. `git status` not refreshing the index; intended
  for background processes (`Documentation/git.adoc`).
- **Editor fallback** (`editor.c`, `git_editor`): `GIT_EDITOR`, then
  `core.editor`, then `VISUAL` (only if `TERM` is not dumb), then `EDITOR`; if
  none and `TERM` is dumb or unset, git errors ("Terminal is dumb, but EDITOR
  unset") instead of launching `vi`. Setting `GIT_EDITOR` explicitly is the
  only way to be certain no terminal editor is attempted.
- **Automatic maintenance** may run after a write (`gc.auto`,
  `gc.autoDetach`, `maintenance.autoDetach`): detached by default, so a
  client may see the verb exit while maintenance continues in a process that
  has left the parent's control (whether it leaves the process group depends
  on git's daemonize path — **UNVERIFIED** here).

---

## Cross-cutting comparison

| | Desktop/dugite | VS Code | jj | GitButler | lazygit | Magit |
| --- | --- | --- | --- | --- | --- | --- |
| Shapes | one-shot + raw spawn | one-shot + stream | one-shot, stderr streamed | one-shot | one-shot, lines, stream, pty | sync + async |
| Long-lived batch (`cat-file --batch`) | no | no | no | no | no | no |
| Cancel | hooks only (abort) | `SIGTERM` child | none (tty `SIGINT`) | `SIGKILL` on drop | `SIGTERM`/close pipe (views) | `SIGINT` then `SIGKILL` |
| Process group | no | no | no | no | no | (pty) |
| Output cap | 256 KiB ring; maxBuffer | status: kill after 10k entries | none | none | on-demand reading; 64 KiB line cap | 32 log sections |
| Locale | none | `LC_ALL=en_US.UTF-8`, `LANGUAGE=en` | `LC_MESSAGES=C`, unset `LC_ALL`/`LANGUAGE` | `LC_ALL=C` | `LC_ALL=C` on pty/rebase only | none |
| Optional locks | status only (`--no-optional-locks`) | status only | n/a | n/a | **all but foreground status** | no |
| Terminal prompt | `GIT_TERMINAL_PROMPT=0` | askpass, no TPROMPT | inherits tty | `=0` | pty scraping | pty scraping |
| Lock retry | no (message) | 10×, `n²·50 ms` | n/a | n/a | 7×, 20 ms doubling | no |
| Serialisation | drop duplicate per kind | none; UI-block flags; status waits for idle | n/a | fair RwLock per gitdir | none (pause bg refresh) | none |
| Error parse | ~60 stderr regexes | ~17 stderr regexes | porcelain + first-line prefix | substring | stderr string | `error:`/`fatal:`/`git:` line regex |
| Visible log | no | Output channel | no (tracing) | no (tracing) | command log panel | process buffer |
| Editor | `:` / `cat todo >` | IPC script (opt-in) | n/a (`--no-verify`) | n/a (own hooks) | self as editor (daemon) | with-editor |

---

## Synthesis

### Where clients converge

1. **One-shot processes, buffered unless there is a reason to stream.** No
   surveyed GUI keeps a long-lived `git cat-file --batch`; each read or write
   is a fresh process. Streaming is reserved for progress (fetch/push/clone),
   large listings (status, log, diff) and prompts.
2. **Both pipes are always drained concurrently** (Node's event loop; jj's
   scoped thread; lazygit's goroutines). Nobody reads one stream to EOF before
   the other.
3. **stdin is closed or null by default**, opened only to feed data (commit
   message `--file -`, NUL-separated paths `--stdin -z`, hook refspecs), and a
   write to a child that exited early is tolerated rather than treated as the
   error (dugite `ignoreClosedInputStream`, GitButler's `BrokenPipe` arm).
4. **Long argument lists go through stdin or are chunked** (Desktop
   `update-index -z --stdin`; VS Code 30 000-character chunks).
5. **Something non-interactive always answers git's questions**: askpass
   (VS Code, GitButler, Desktop's trampoline), a pty scraper (lazygit,
   Magit), or `GIT_TERMINAL_PROMPT=0`; and `GIT_EDITOR` is set explicitly
   whenever git might open an editor.
6. **Background work must not cost the user a lock.** Everyone who refreshes
   status in the background suppresses optional locks for it; lazygit
   generalises this to every command.
7. **Errors are classified from English stderr** by a small regex table,
   with the exit code deciding success first and per-call "expected"
   outcomes (Desktop's `successExitCodes`/`expectedErrors`).
8. **Background network work never prompts** (lazygit
   `FailOnCredentialRequest`; Desktop `isBackgroundTask`) and is not started
   while a foreground one runs (Fork 2.27; Desktop's dropped duplicate).

### Where they diverge

- **Lock contention: avoid, retry, or serialise.** VS Code and lazygit retry
  on the lock message with bounded backoff; Desktop reports and stops;
  GitButler prevents it in-process with a readers/writer lock; nobody deletes
  a stale `index.lock` automatically.
- **Locale.** From nothing (Desktop, Magit) through `LC_MESSAGES=C` with
  `LC_ALL`/`LANGUAGE` removed (jj, explicitly to keep the encoding) to
  `LC_ALL=C` (GitButler) or a forced `en_US.UTF-8` (VS Code). jj's comment is
  the only one that reasons about the side effect.
- **Cancellation strength.** From none (Desktop network ops, jj) through
  `SIGTERM` to the direct child (VS Code, lazygit) to immediate `SIGKILL`
  (GitButler `kill_on_drop`) to a user-driven `SIGINT`-then-`SIGKILL`
  (Magit). **None of the clients signals a process group**; grandchildren
  (ssh, credential helpers, hooks, LFS) are left to die of a closed pipe or
  not at all — lazygit's comment about shims on Windows and duct's docs on
  surviving grandchildren are the documented failure.
- **Prompts.** Askpass IPC (VS Code, GitButler, Desktop, Cairn) versus pty
  scraping with locale-pinned regexes (lazygit, Magit). Scraping is
  locale-fragile and needs a pty; askpass is structural.
- **Hooks.** Let git run them (most), proxy them for progress and abort
  (Desktop's `core.hooksPath` trick), skip them (jj `--no-verify`), or
  reimplement them (GitButler — with a self-declared wrong case).
- **Visibility.** A first-class command log (VS Code output channel, lazygit
  panel, Magit process buffer, Fork Activity Manager) versus developer
  tracing only (Desktop, jj, GitButler).

### Lessons for a Rust design with no `unsafe` and one-thread-per-role workers

1. **Signal the group, from std and nix alone.** Spawn every git with
   `CommandExt::process_group(0)` (safe, stable, works on macOS via
   `posix_spawnattr_setpgroup`) and cancel with `nix::sys::signal::killpg`
   (safe). This reaches hooks, ssh and credential helpers that a direct-child
   signal leaves running, which is the gap every surveyed client has. Keep the
   existing **`SIGTERM` first, `SIGKILL` after a grace** escalation: git
   removes its lock files on `SIGTERM`, `SIGINT`, `SIGHUP`, `SIGQUIT`,
   `SIGPIPE` and **not** on `SIGKILL`, so GitButler-style immediate kill is
   the recipe for stale `index.lock`. Residual: a grandchild that calls
   `setsid` (detached auto-maintenance, an ssh `ControlPersist` master) escapes
   the group — mostly desirable, but means "killed" is not "nothing left
   running". Not reachable without `unsafe`: `setsid`/`PR_SET_PDEATHSIG` in
   the child, so children of a crashed Cairn are orphaned; Linux-only partial
   mitigation is `prctl::set_child_subreaper` in the parent.
2. **Kill-while-waiting without `unsafe`.** shared_child's `waitid(WNOWAIT)`
   is the clean answer but needs `unsafe` libc, and nix's safe `waitid` is not
   available on macOS. Cairn's current poll-with-`try_wait` under a short lock
   (the shape in `crates/cairn-git/src/ops/cli.rs`) is the portable,
   safe-only alternative; once the group leader is reaped, signal by PGID
   rather than PID only while the leader is known unreaped, or accept the
   group-ID reuse window (**UNVERIFIED** how wide that window is in practice;
   a PGID stays valid while any member lives).
3. **One reader thread per pipe, always.** Drain stdout and stderr on
   separate threads (jj's scoped-thread shape fits one-thread-per-role); never
   `output()` a verb that can be large or slow. Bound what is *retained*, not
   what is *read*: a ring of the last N KiB for the error and the log
   (Desktop: 256 KiB), with progress lines (split on `\r` and `\n`) forwarded
   and discarded. Where the consumer is a viewport, lazygit shows the
   alternative bound: read on demand and let a full pipe stall git.
4. **Pin the environment by construction, which Cairn already does with
   `env_clear` + roster.** Precedent says add: `GIT_OPTIONAL_LOCKS=0` by
   default with an explicit per-verb opt-in for the one foreground status that
   should refresh the index (lazygit's exact policy); `GIT_EDITOR` always set
   to something that cannot open a terminal editor (`:` or a helper);
   `GIT_PAGER=cat` or `--no-pager`; `TERM=dumb`; `-c color.ui=false`. For
   locale, follow jj: leave `LC_ALL` and `LANGUAGE` off the roster and set
   `LC_MESSAGES=C`, so messages are English without changing the encoding
   (`LANGUAGE` overrides `LC_MESSAGES` in gettext, which is why jj removes
   it). Prefer machine output (`--porcelain`, `-z`, exit codes) over stderr
   text wherever git offers it, as jj does for fetch and push.
5. **Serialise mutations per repository; let reads run.** Git gives no wait
   on `index.lock`, so two Cairn writers racing is a self-inflicted failure.
   A per-repository single writer (GitButler's model, which maps naturally to
   one mutation worker per repository) removes internal contention; reads via
   gix need no lock, and any `git` read subprocess runs with optional locks
   off. External contention (the user's terminal, an editor plugin) remains:
   VS Code and lazygit answer it with a bounded retry on the lock message
   (10× quadratic from 50 ms; 7× doubling from 20 ms). For Cairn the retry
   must be restricted to failures that git reports **before** mutating —
   "Unable to create '...index.lock'" is; "cannot lock ref" on a multi-ref
   operation may not be, which is why VS Code retries it only for fetch, pull
   and sync. A destructive verb consuming a `Confirmed` token should not
   silently re-run. Never delete a lock file on the user's behalf; say whose
   it might be.
6. **Coalesce, do not queue, background work.** Desktop drops a duplicate
   network operation; Fork skips an automatic fetch while one runs; VS Code
   throttles status to one running plus one pending and waits for idle. A
   background status or fetch should never queue behind, or race ahead of, a
   user's mutation — Fork `#1633` is that race.
7. **Classify errors into caller-actionable enums with a small, tested
   table**, locale pinned, first by exit code and structured output, then by
   stderr prefix (jj) or regex (dugite, VS Code). Keep the raw tail for
   display. Per-call "expected" outcomes (Desktop's `expectedErrors`) map
   cleanly onto per-verb `thiserror` enums.
8. **Make the command log first-class.** Four of the six (plus Fork) show
   the user what ran: argv, duration, exit, cancelled-or-not, stderr. GitButler
   strips its askpass variables before logging; Cairn's equivalent is that the
   log records argv and Cairn-chosen variable names, never the askpass token or
   any `Secret`. Retention must be bounded (Magit: 32 sections, never dropping
   a running one).
9. **Editors and hooks stay git's, with a seam for progress.** The cheapest
   proven route for interactive rebase is to hand git a prepared todo
   (`-c sequence.editor=` writing a file, Desktop) or to re-exec one's own
   helper as the editor (lazygit's daemon mode, VS Code's IPC script) — Cairn
   already ships a helper binary and a channel for askpass, which is the same
   shape. Hooks can be long: streaming their stderr and allowing abort
   (Desktop's proxy, Fork 1.0.69) matters more than a timeout; Desktop also
   shows that hooks may need the user's login-shell environment, which a
   cleared environment will not have — a roster decision, not a process one.
   GitButler's reimplemented hooks are the counter-example D1 exists to avoid.
10. **Background verbs never prompt.** A background fetch must run with a
    "fail on prompt" askpass mode (lazygit, Desktop) so a missing credential
    becomes an error, not a dialog the user did not ask for.

### Unverified, collected

- Whether dugite's bundled git is built without gettext (so Desktop's lack of
  locale pinning is safe there).
- VS Code's behaviour on hosts without an `en_US.UTF-8` locale.
- The causal link between lazygit `#5906` and its current lock policy; the
  history of lazygit's former `SyncMutex`.
- Whether Emacs `interrupt-process`/`kill-process` on a pty-backed Magit
  process reaches the process group.
- Sublime Merge's `git_binary` details, command log, lock and hook handling;
  Fork's read backend and kill mechanism. Fork release-note entries were read
  through a summarising fetch and should be spot-checked before quoting.
- Whether git's detached auto-maintenance leaves the parent's process group.
- How wide the PGID-reuse window is after the group leader is reaped.
