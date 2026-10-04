# Git processes

How Cairn builds and runs a `git` process today, and what keeps that in one
place. As-built: everything here is code that exists, with the test that pins
each rule named beside it. The commitment it was built against is
`docs/prd/process-manager.md` (shipped, frozen); the intent is
`docs/design/processes.md`, with D1 in `docs/design/engine.md`. What the askpass
helper does with the environment described here, and fetch end to end, are
`docs/systems/credentials.md`.

**What exists:** one crate-private module that builds every `git` process, an
invocation typed as a read or a write, the two environments those kinds get,
and the runner: each process leads its own process group, each pipe it uses has
a thread, stdout is handed over as it arrives or collected under a ceiling,
stderr is kept as a bounded tail, and an invocation ends by a cancel signal, a
kill handle or a drop — `SIGTERM` to the group, then `SIGKILL` after two
seconds. The version probe and fetch run on it like everything else; the
paths they ran on before are gone, and a guard keeps them gone. Each open
repository keeps a registry of the invocations running in it, which closing
it ends and waits on, and a bounded log of every one that is over. In the
application, `git` is found once, as it starts; a fetch runs in the network
lane, which refuses a second; closing the window closes its repository; and
the worker answers the log as values, which no view draws yet (issue #41).

Where a residual below says **accepted by the user on 2026-10-02**, the user
reviewed it when the packet shipped and kept the behaviour as stated; where it
cites an issue, the user chose to have it fixed later, and the issue holds the
options.

## The layout

```
crates/cairn-git/src/
  process/      crate-private; the only place a process is built, spawned, waited on or read
    binary.rs       GitBinary, GitVersion — discovery, the 2.30 floor, the read and write builders
    environment.rs  GitEnvironment — the base, ALWAYS, INHERITED, READ_ONLY, Profile
    askpass.rs      Askpass — where git and ssh are sent for a secret
    cli.rs          GitCommand<K>, Read, Write, Kind, GitDirs, Output; `start`, the runner's entry
    runner.rs       Invocation<K>: driving a started process to its end, and its outcome
    group.rs        the process group: when it may be signalled, ending it, KillHandle,
                    TERMINATION_GRACE
    pipes.rs        the pipe threads: stdout chunks, stderr lines, stdin; the tail, the records
    registry.rs     Processes — a repository's running invocations, end_all, CLOSE_BOUND;
                    Registration, an invocation's booking from spawn to end
    command_log.rs  CommandLog — the records, LOG_ENTRIES, LOG_BYTES
    stub_git.rs     (tests) a stub `git` on a PATH the test controls
  ops/          every mutation; constructs WriteAuthority; re-exports what the app needs
    authority.rs    WriteAuthority, and the tests that need one (real `git` writes among them)
    fetch.rs        fetch, built as a write
    refspec_policy.rs  the refspecs fetch refuses (docs/systems/credentials.md)
    stranded_locks.rs  every `*.lock` under a git directory; the runner reports them for a write
  reads/        each read `git` answers, one named function each; its tests run a read
                built from a `GitBinary` copy, on a thread, stopped by an epoch
    changes.rs      changes — `git diff-tree -r -z --raw`, the changes query (docs/systems/diff.md)
    patches.rs      patches — `git diff-tree -p`, a file's changed ranges and function context
                    for the content query, one file or a whole comparison (docs/systems/diff.md)
    attributes.rs   diff_attributes — `git check-attr --stdin -z diff`, whether a path's diff
                    driver names its own algorithm (docs/systems/diff.md)
```

`process` is a private module (`mod process;` in `lib.rs`). The application
reaches `GitBinary`, `GitVersion`, `GitEnvironment` and `Askpass` through
`cairn_git::ops`, which re-exports them because the application owns startup
and the helper's channel, and `cairn_git::CLOSE_BOUND` is re-exported at the crate
root. It cannot reach the runner: nothing that builds or runs a process is `pub`.

On the application's side, in `crates/cairn-app/src/`:

```
  worker/discovery.rs     Discovery — git found once per application, on cairn-discovery
  worker/startup.rs       Startup, Backend — a repository's channel, and git pointed at it
  worker/network_lane.rs  the network lane's loop: Operation, Lane, FetchControl, Refusal
  worker/pool.rs          the repository thread: serves the history lane, Request::Close and
                          Request::CommandLog, spawns the cairn-diff and cairn-network
                          threads, Threads::drop
  worker/routing.rs       the routing table: which thread serves each request, applied as it
                          is submitted
  worker/epoch.rs         epochs numbered per query lane, each a read's Cancel
  worker/diff_lane.rs     the diff thread's loop: the changes and file-diff lanes, whose reads
                          a superseding query ends (docs/systems/diff.md, "In the application")
  worker/request.rs       Request and Update, the boundary's messages
  closing.rs              Closing — the window's close hook, which asks and never waits
```

## Discovery

`git` is found and its version checked once per application, as it starts
(PRD R6.1). `main` calls `worker::Discovery::start()` before the window
exists; it spawns a `cairn-discovery` thread that runs
`GitBinary::discover_with` against the launching environment — `PATH` and
the inherited roster, pointed at the helper with no channel yet, since the
probe asks nobody anything — and keeps the answer, or the refusal's text, in
a `OnceLock`. Each repository's thread asks for it before it looks for the
repository, and waits if it is not in yet; if the discovery thread could not
be started, the first to ask finds it, once. A missing or too-old `git`
reaches every repository that asks as the same `Update::Failed`, naming the
version Cairn needs, and nothing is served behind it.

The probe has no deadline, as no invocation does: a `git --version` that never
answers leaves every repository waiting on `Discovery::git`, unable to open,
and a worker that never reaches `serve` never sees a close (issue #48).

The answer is a path and a version, and each repository needs it pointed at
its own askpass channel, which does not exist until the repository opens:
`GitBinary::with_environment` copies the path and version onto the
environment built around that channel's socket, with no second search and no
second probe. The program stays the one found, by its absolute path.

Pinned: `git_is_found_once_per_application_not_once_per_repository`
(`worker/lifecycle_tests.rs`: three repositories opened on one discovery,
started as `main` starts it and raced by the first open, probe the stub once;
two opened with a discovery each probe it twice, so the count moves),
`a_git_refused_at_discovery_is_refused_to_every_repository_that_asks`
(`worker/lifecycle_tests.rs`),
`a_missing_git_is_refused_naming_the_version_and_nothing_is_served`
(`worker/pool.rs`), and
`a_found_git_takes_a_new_environment_without_being_searched_for_or_probed_again`
(`crates/cairn-git/tests/git_binary.rs`).

## The seal

An invocation is a `GitCommand<'_, K>`, and `K` says what kind:

- **`GitBinary::read_invocation()`** builds a `GitCommand<'_, Read>`. `Read` is a
  unit struct, so a read has no field that could hold an askpass token.
- **`GitBinary::write_invocation(WriteAuthority)`** builds a
  `GitCommand<'_, Write>`. `Write` keeps the authority for the invocation's life,
  plus the token when `authorized_by` is called — a method that exists on a
  write and nowhere else.
- **`WriteAuthority`** (`ops/authority.rs`) is `pub(crate)`, with a private field
  and one constructor, `pub(in crate::ops) fn new()`. So the compiler refuses a
  write anywhere in the crate but `ops/`: in `reads/`, in `history/`, and in
  `process/` itself, which cannot even build a `Write` without one. It is a
  token rather than a visibility on the builder because `process/` cannot write
  `pub(in crate::ops)` on an item of its own: a restricted visibility must name
  an ancestor of the item.
- **`GitCommand::new`** and **`GitEnvironment::command`** are `pub(super)`:
  visible to `process/` alone. The version probe is the one invocation built
  outside the two builders. It runs before there is a `GitBinary`, and it is a
  read.

Fetch is built as a write (`ops/fetch.rs`): `FetchInProgress` holds its
`Invocation<Write>`, `FetchCancel` its `KillHandle`, and `finish` drives it
with a cancel signal nobody holds, since its cancel is the handle's, and
drops what it writes to stdout. The probe is built as a read
(`process/binary.rs`) and collected under a 4 KiB ceiling (`PROBE_CEILING`):
one line is all `git --version` prints.

From outside the crate nothing is nameable. The `compile_fail` doctests in
`crates/cairn-git/src/ops/mod.rs` each add one line to a passing scaffold:
naming a `WriteAuthority`, calling its constructor, calling either builder, and
calling `GitEnvironment::command`. Every argument is `unreachable!()`, so no
block can fail on its arguments. Stable `rustdoc` checks that a block fails,
not why, so each was checked by hand, by turning it into a plain block: each
fails on privacy alone (E0603 for the type, E0624 for the methods). With a
builder or `command` widened to `pub`, its block still fails on privacy, of
the crate-private type it returns or takes (`GitCommand`, `Profile`). So the
doctests decide that the public surface offers no way in. The methods' own
visibility is pinned by the guard, which also requires each refused block to
be exactly the scaffold plus its line.

## The environment

`GitEnvironment::new(parent, &Askpass)` builds the **base** every invocation
gets:

- the `INHERITED` roster, copied from the parent when present;
- the `ALWAYS` table: `GIT_TERMINAL_PROMPT=0`, `SSH_ASKPASS_REQUIRE=force`,
  `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false`;
- `GIT_ASKPASS` and `SSH_ASKPASS` naming the helper;
- `CAIRN_ASKPASS_SOCKET` when the `Askpass` names a socket.

`GitEnvironment::command(program, Profile)` is the only place a
`std::process::Command` is built. It clears the inherited environment, applies
the base, and then applies what the invocation's kind adds:

- **A read** (`Profile::Read`) adds the `READ_ONLY` table, `GIT_OPTIONAL_LOCKS=0`
  and `GIT_NO_LAZY_FETCH=1`, and never a token, because the variant has no
  field for one.
- **A write** (`Profile::Write { token }`) adds `CAIRN_ASKPASS_TOKEN` when the
  operation was given a token, and nothing else.

The kind chooses the profile (`Kind::profile` in `cli.rs`). The caller never
does.

`GitEnvironment` and `GitBinary`, which holds one, derive `Debug`, so `{:?}`
on either renders the inherited values, the proxy URLs among them, which may
carry the user's proxy credentials. Nothing prints either; the application's
`worker::Discovery` has a `Debug` of its own that renders the path and version
only (`a_discovery_renders_what_was_found_and_nothing_of_the_environment`).
A value-free `Debug` on the engine types is issue #49.

Why each variable is there, with its evidence, is beside it in
`environment.rs`. Two need a word here:

- **The editor is pinned to `false`, not `:`.** A verb that wants an editor fails
  instead of waiting on one nobody can see. `:` would silently accept whatever
  message git proposed. Both editor variables are set, because a user's
  `sequence.editor` outranks `GIT_EDITOR` for the rebase todo list.
- **`GIT_OPTIONAL_LOCKS=0` covers `git status` and nothing else.** Porcelain
  `diff` and `describe --dirty` refresh the index anyway. That is why a read in
  `reads/` runs query plumbing or `status` only — and, as the one porcelain
  exception the user accepted, `git diff --no-index -- /dev/null <path>` for an
  untracked file's working-tree diff, `<path>` work-tree-relative (no absolute,
  `.` or `..` component, refused before git runs) and `./-` for `-`, which reads
  no index, with its presentation
  settings pinned to git's defaults by `-c` — as the module's own docs say
  (`reads/mod.rs`, "What a read may run"); `destructive-ops-reviewer` check 10
  names it.
- **A read may run the repository's `core.fsmonitor` hook and, on a read of the
  working tree, the path's clean filter driver — no other program.**
  `diff-tree`, `diff-index`, `diff-files` and `check-attr` run the hook as they
  read the index of a repository with a working tree (reproduced on 2.30.9
  through 2.56.0), as the user's own `git diff` does; no flag of theirs turns it
  off, and the user decided to allow it as parity (`reads/mod.rs`, "What a read
  may run"; `the_content_query_writes_nothing_and_runs_nothing`). Under
  `core.fsmonitor=true` the same four start git's own fsmonitor daemon instead,
  if none is running (git 2.36 and later; reproduced on 2.56.0; `diff
  --no-index` starts none), as the user's `git status` does — accepted as parity
  by the user on 2026-10-04. git starts it in a session of its own, so it is
  outside the read's process group, outside the repository's registry of
  running invocations and outside `SharedRepository::end_invocations`: it
  outlives the read and the application, and is not Cairn's to end. It writes
  its socket and cookie directory in the git directory, the one change a read
  leaves there
  (`a_read_under_the_builtin_fsmonitor_writes_only_the_daemons_own_files`).
  A planted repository naming a hook is refused at open ("Where an invocation
  runs").
  `diff-files` and `diff --no-index` run the clean filter driver the path's
  attributes name, as a child of the read's `git`, so with the read's
  environment above — `GIT_ASKPASS`, `SSH_ASKPASS` and, where the application
  listens, `CAIRN_ASKPASS_SOCKET` among it, so a driver can reach the socket
  but, with no token, fails closed — plus what git sets for a filter
  (`GIT_EXEC_PATH`, `GIT_PREFIX`, `GIT_CONFIG_PARAMETERS`, the exec directory
  first on `PATH`, and `GIT_DIR` and `GIT_WORK_TREE` when the repository is
  named to git — full trust, "Where an invocation runs" — and not when it is
  left to discovery), its stderr the read's bounded tail; and for
  a submodule `diff-files` runs `git status` inside it, with that repository's
  own hook and filters (`reads/working_tree.rs`;
  `a_working_tree_query_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor`,
  `a_clean_filter_drivers_form_is_what_is_diffed_and_it_runs_under_git`).
- **`GIT_NO_LAZY_FETCH=1` needs git 2.44; the floor is 2.30.** In a partial
  clone, a read that asks for an object only the promisor remote holds would
  fetch it — a pack written, the network reached. With the variable, git
  answers that the object is missing instead. Git older than 2.44 ignores it,
  so there a read in a partial clone may still lazy-fetch. Carrying no askpass
  token, it fails closed only where the promisor needs a prompt; one a
  configured credential helper or the ssh agent answers fetches. The user decided on
  2026-10-02 to keep the floor and set the variable; the reads `diff-engine`
  adds design around the older-git case (`reads/mod.rs`).

Pinned:

- **Variable by variable, from the builder.** `environment.rs`:
  `the_environment_is_exactly_the_deliberate_entries`,
  `a_read_is_the_base_with_optional_locks_and_lazy_fetch_off_and_no_token`,
  `a_write_is_the_base_with_its_token_only_when_given` and
  `the_always_table_wins_whatever_the_parent_says`.
- **From what a stub `git` printed.** Each test spells out the exact set the stub
  must have seen:
  - a read: `a_read_sees_exactly_the_read_environment_and_nothing_inherited`;
  - the version probe: `the_version_probe_runs_with_the_read_environment`;
  - a write: `a_write_sees_exactly_the_write_environment_and_nothing_inherited`;
  - a write with its token: `an_authorised_write_carries_its_token_and_only_that_one`;
  - fetch, with and without a token: `a_fetch_runs_with_the_write_environment_and_its_token`.
- **The effect, against real `git`** (`ops/authority.rs`):
  - `a_status_read_leaves_a_stale_index_byte_identical`. The index's stat
    information is stale and the working tree is dirty. The read leaves the
    index byte for byte as it was, and the same `status` run as a write
    rewrites it — the control that proves the fixture was stale.
  - `a_read_in_a_partial_clone_does_not_fetch_a_missing_object`. In a
    `--filter=blob:none` clone, a read of a blob only the promisor holds fails
    and writes no pack; the same `cat-file` as a write fetches it into a new
    pack — the control that proves the clone was partial. On a git older than
    2.44 it says it was skipped and decides nothing.
  - `a_commit_without_a_message_fails_promptly_instead_of_opening_an_editor`
    and `an_interactive_rebase_fails_promptly_instead_of_opening_the_sequence_editor`.
    Each configures an editor that records it ran and then hangs. Each verb
    fails within the deadline, the editor never runs, and `HEAD` does not move.

## Where an invocation runs

`GitCommand::in_repository(repo)` runs the invocation in the repository's
working tree, or the git directory of a bare one, and **names the repository
to git** ahead of the verb: `--git-dir=<git dir>` and, when there is one,
`--work-tree=<working tree>`, both absolute (`repository_location` in
`process/cli.rs`). Left to its own discovery from that directory, git can
read a different repository from the one Cairn opened: a working tree whose
git directory lives elsewhere (`core.worktree`) and which sits inside
another repository's working tree is discovered as that enclosing
repository, and under `safe.bareRepository=explicit` git refuses to discover
a bare repository at all. `GIT_DIR` and `GIT_WORK_TREE` from the launching
environment are never inherited, so the options are the only place either
comes from.

An explicitly named git directory is one git does not check the ownership
of: `safe.directory` guards discovery only (reproduced with git 2.56 under
`GIT_TEST_ASSUME_DIFFERENT_OWNER=1`, where `git log` refuses with "dubious
ownership" and the same command given `--git-dir` answers). So git's own
check is made by Cairn as the repository is opened, and a repository git
would refuse is refused there — `Error::DubiousOwnership`, before anything
in it is read or run (user decision, 2026-10-04: refuse at open where git
would refuse). The rule is `ensure_valid_ownership` in `setup.c` as the
version of `git` in use has it (`crates/cairn-git/src/ownership.rs`, whose
module documentation is the version table, read at every tag from v2.30.0
to v2.56.0): no check before 2.30.3 and its sister releases; the working
tree's top alone, then — from 2.30.5, 2.36.2 and 2.37.1 — every path git
checks (the `.git` file, when the working tree reaches its git directory
through one; the working tree's top; the git directory, for a `.git` file
the directory it names); the owner by `lstat` against the effective uid,
read from `/proc/self/status` on Linux (the owner of a file the process
creates elsewhere), with `SUDO_UID` standing in for root alone;
`GIT_TEST_ASSUME_DIFFERENT_OWNER` read as git reads it; and `safe.directory`
matched as git matches it — `*`, the empty value's reset, `~/`, `%(prefix)/`
from 2.34, the command line from 2.38, `<dir>/*` from 2.45.3 and 2.46.0,
normalised (`real_path`, `.` the starting directory, relative entries
ignored) from 2.46.1, `:(optional)` from 2.52 — over the configuration git
protects, read by `crate::bare_discovery::protected_values` (no include
followed on 2.38.x; `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` from 2.32).
What passes is opened with full trust and named to git. Pinned against git
itself by `a_repository_opens_exactly_where_git_opens_it_whatever_safe_directory_says`
(`crates/cairn-git/tests/diff/ownership.rs`, every shape under every
spelling of the setting with `GIT_TEST_ASSUME_DIFFERENT_OWNER=1`, so it
runs against the floors' gits in `git-floor` too); through every open's
wiring by `a_repository_someone_else_owns_is_refused_at_open_unless_safe_directory_names_it`
(an identity whose effective uid is not the owner's) and
`a_gitfile_rewritten_between_the_check_and_the_open_is_refused` (the git
directory opened must be the one judged, or `Error::RepositoryReplaced`), in
`repository.rs`; and each band by the unit tests in `ownership.rs`. The real
second owner needs root, so that case is a privileged review step:
`a_linked_worktree_whose_git_dir_is_someone_elses_is_refused_as_git_refuses_it`,
`#[ignore]`d in `crates/cairn-git/tests/diff/ownership.rs`, run as root.

Naming the git directory is also the explicit spelling git never refuses, so
the one check git makes on a repository it found by searching — a bare one,
under `safe.bareRepository = explicit` — is made when Cairn opens it, or a
bare repository planted inside a cloned working tree would be read anyway,
and the programs its configuration names (`core.fsmonitor` on every read,
`core.sshCommand` and `credential.helper` on a fetch) run with it.
`SharedRepository::discover_for(path, git, environment)` — the application's
open, given the `git` found as it started and the launching environment —
refuses such a repository with `Error::BareRepositoryFoundBySearching` (and
a value git dies on with `Error::InvalidConfig`) before gix opens it, exactly
where that version of git refuses it from the same directory
(`crates/cairn-git/src/bare_discovery.rs`, read from git's `setup.c` at
v2.38.0 through v2.56.0): the search stops at a directory that is itself a
git directory rather than one holding a `.git`; the setting is read only from
the system file, the global ones (includes followed, `includeIf "gitdir:"`
not) and the command line's `GIT_CONFIG_COUNT` (its count read as git's
`strtoul` reads it — leading whitespace and a sign accepted, an empty value
zero entries, "bogus count" and "too many entries" where git says them) and
`GIT_CONFIG_PARAMETERS`, never the repository's own, and every value is
checked, git dying on any but `explicit` and `all`; and what git calls
implicit opens — nothing before 2.38 (the setting does not exist), nothing
from 2.38 to 2.43, a directory named `.git` in 2.44, and from 2.45 a linked
worktree's or a submodule's git directory too. `SharedRepository::discover`
applies git 2.45's rule with the process's own environment. A repository
that passes is named to git as before.

The search that is checked is the search that opens. `bare_discovery::find`
walks once, from the physical directory upwards as git does (and, as git and
gix do by default, not into another filesystem), and hands back where it
stopped — the `.git` of a working tree, or a git directory found as itself —
and gix opens exactly that path (`ThreadSafeRepository::open_opts` with the
path taken as it is, the options and trust gix's own discovery derives from
its owner), never searching again. Two searches agree only while they take
the same steps: gix's today switches to the physical path as git's does, but
one that followed a link logically would climb from `docs/guide -> ../guide`
into a bare repository planted as `docs/` while the check passed the working
tree above it. So the paths a repository reports are physical — a repository
opened through a link reports the directory the link leads to, as
`git rev-parse` does. Reading the system file runs nothing: it is
`GIT_CONFIG_SYSTEM` or `/etc/gitconfig`, gix's `Source::System`, never
`Source::GitInstallation`, whose path gix-path finds by running the `git` on
`PATH` (`git config -lz --show-origin`) outside `GitEnvironment` — which
gix's own open never asks for either.

The command log and an error report the verb and its arguments, not the
location: a repository's log is its own, and the record's directory says
where it ran.

Pinned: `a_trusted_repository_is_named_to_git_ahead_of_the_verb` (a stub
`git` prints its arguments) and
`a_repository_trusted_less_than_fully_is_left_to_gits_discovery`, in
`process/cli.rs`; against real git,
`a_repository_whose_working_tree_sits_inside_another_is_the_one_asked` and
`a_bare_repository_is_answered_under_safe_bare_repository_explicit`
(`crates/cairn-git/tests/diff/changes.rs`), and
`a_fetch_lands_in_the_repository_opened_when_its_working_tree_sits_inside_another`
(`crates/cairn-git/tests/fetch.rs`), each of which fails with the options
removed. The open's check is pinned against the git in use by
`a_bare_repository_found_by_searching_opens_exactly_where_git_opens_it` —
every shape (the planted repository and a directory inside it, a `.git`
directory entered, a linked worktree's git directory, the worktree, the
working tree) under every way the setting is given or not, the repositories'
own configuration saying `explicit` throughout, Cairn opening the same git
directory `git rev-parse --absolute-git-dir` does, or refusing where it
refuses; a link inside a planted bare repository to a directory of the
working tree, and a link from outside to it, among the shapes, and four
`GIT_CONFIG_COUNT` spellings git accepts among the settings — and
`opening_reads_the_system_file_without_running_a_process` (the test binary
run again with a recording `git` first on `PATH` and no
`GIT_CONFIG_NOSYSTEM`, the recorder shown to record) and
`a_planted_bare_repository_is_refused_at_open_and_runs_nothing` (the planted
`core.fsmonitor` runs on a read without the setting, as it does under git,
and never under it; skipped before 2.38), in
`crates/cairn-git/tests/diff/bare_discovery.rs`; the version bands by
`which_bare_repositories_are_implicit_follows_the_version_of_git` and the
command line's parsing by
`command_line_parameters_are_read_as_git_reads_them` and
`the_entry_count_is_read_as_gits_strtoul_reads_it`, in
`bare_discovery.rs`; and the application's open by
`a_planted_bare_repository_is_refused_as_the_launchs_git_refuses_it`
(`crates/cairn-app/src/worker/pool.rs`). Residual, stated rather than
implied: no fixture can make a
repository its own user does not own, so a real second owner is decided by
the privileged run above, and every other case through
`GIT_TEST_ASSUME_DIFFERENT_OWNER` or an injected identity; gix, given a
repository Cairn decided git opens, checks the working tree's owner again by
its own rule and, where that rule refuses what git's admits (`safe.directory`
on the command line, `.`, a normalised entry, a git with no check), opens it
with reduced trust — shown as git shows it, but read by gix with the
repository's own configuration filtered and left to git's own discovery
for the `git` Cairn runs in it, which opens it as git does; `%(prefix)/` is
expanded against the directory above the `bin/` of the `git` found, and to
nothing where no `git` is known (`SharedRepository::discover`). The
`safe.bareRepository` check reads the system file at `GIT_CONFIG_SYSTEM` or
`/etc/gitconfig`, not at the path compiled into the `git` Cairn found — the
same file for a distribution's git (prefix `/usr`), another for a git built
with another `sysconfdir` (a custom prefix, Homebrew's, Apple's), whose file
only running a process could find; does not follow an
`includeIf "hasconfig:"` in a global file, which git can match there; does
not check the other keys of `GIT_CONFIG_PARAMETERS`, and reads either
variable only where the search stops at a bare repository, while git refuses
every command when it cannot parse one; and reads a git built
`WITH_BREAKING_CHANGES` before 3.0 as defaulting to `all`. Each would show a
divergence only in the user's own protected configuration or environment,
never one a repository can plant. Not residual: whether the repository
opened is the one checked — it is opened from the path the check searched
to, and the shapes test fails when it is opened through a second, logical
search instead, or when the one search is made logical.

## The runner

`GitCommand::start()` spawns the process and hands back an `Invocation<K>`,
still running. The caller drives it on its own thread with one of three calls,
each taking the cancel signal it polls:

- `finish(cancel, stdout, progress)` hands stdout to a closure as it arrives;
- `records(cancel, record, progress)` splits stdout into the NUL-terminated
  records of a `-z` format and hands each on as soon as it is whole;
- `collect(cancel, ceiling, progress)` returns the whole of stdout, or
  `Error::GitOutputTooLarge { ceiling }` once git writes more — the process is
  ended at once and nothing it wrote is returned.

`GitCommand::input(bytes)` gives the process those bytes on stdin, written and
then closed; without it stdin is `/dev/null`. `Invocation::kill_handle()` hands
out a `KillHandle`. Each call returns an `Output` holding the retained stderr
(and stdout, for `collect`). On an `Err` from `records`, the records already
handed on are a prefix of an answer that did not complete, for the caller to
discard; a last record without its NUL is handed on only on success
(`a_cancelled_read_hands_on_no_partial_last_record`).

`start` runs on the caller's thread and may wait — it spawns, and a pipe thread
that cannot start makes it end the process there — so it is a worker's call.
What the UI thread may call is `KillHandle::kill` and the drop, neither of
which waits, but for the drop's no-thread fallback below.

### Spawning and the pipes

- **Its own process group.** `start` sets `CommandExt::process_group(0)`, so
  the process leads a new group whose id is its pid, and everything git starts
  — hooks, `ssh`, helpers — is in it. Pinned by
  `every_invocation_leads_a_process_group_of_its_own`, which reads the stub's
  group from `/proc` and finds the stub and its `sleep` in it, and this test
  process not.
- **A thread per pipe** (`pipes.rs`, started and named in `runner.rs`):
  `cairn-git-stdout`, `cairn-git-stderr`, and `cairn-git-stdin` when there is
  input. No thread reads one pipe while
  another is read or written on it, so no output volume on one stream can stall
  another at any pipe capacity — 64 KiB, two pages at a user's pipe limit, or
  512 bytes on macOS. Readers send `Event`s to the driving thread on a channel
  bounded at `EVENTS_BOUND`, so a caller slower than git holds git back rather
  than buffering without limit. Pinned by
  `five_mib_of_records_beside_continuous_stderr_arrive_whole_and_in_order`
  (G6's volume of records while another member of the group writes many
  pipes' worth of stderr) and
  `sixty_four_mib_of_stdin_beside_busy_stdout_and_stderr_completes`; each runs
  under a deadline, so a deadlock fails rather than hangs. With real `git`:
  `sixty_four_mib_through_hash_object_gives_gits_own_id` (end of input, and the
  id gitoxide computes for the same bytes) and
  `a_real_read_of_over_five_mib_of_records_arrives_whole_and_in_order`
  (a generated tree, made by `mktree` fed on stdin and read back by
  `ls-tree -z`).
- **stdin first, and held until its thread runs.** The writer thread starts
  before the readers, and the pipe stays with the runner until that thread has
  started: a thread that cannot start would otherwise drop it, and git would
  read the end of an EMPTY input as the whole of it —
  `reset --pathspec-from-file=-` given nothing unstages everything. On that
  failure the process is ended first and the pipe dropped after
  (`a_stdin_thread_that_cannot_start_ends_the_process_before_its_input_closes`).
  A write failure other than git closing its end (`EPIPE`, git's own choice)
  likewise ends the process BEFORE the pipe closes, and the outcome is
  `Error::GitUnwatched`
  (`a_failed_stdin_write_ends_the_process_and_a_closed_stdin_does_not`,
  `a_failed_stdin_write_signals_before_it_closes_the_pipe`).
- **A thread that cannot start** is `Error::GitUnwatched`, and the process is
  ended and reaped on the calling thread first, never left running with nobody
  waiting on it (`a_thread_that_cannot_start_ends_and_reaps_the_process`, with
  a thread starter that fails at the stdout and the stderr reader).
- **stderr** is split into lines at `\r` and `\n`, and each read's lines cross
  to the driver as one event, so what git wrote before it exited is a handful
  of events however many lines it was. Every non-blank line goes to `progress`,
  and the last `TAIL_BYTES` (256 KiB) is retained, starting at a line's start
  where one falls inside the window; while the process runs the tail holds at
  most twice that and the line being added, because it cuts in batches. A line
  with no terminator is cut into pieces of that size. Pinned by
  `a_mib_of_stderr_is_forwarded_whole_and_retained_as_a_bounded_tail`,
  `a_final_burst_of_stderr_is_kept_whole_though_a_holder_keeps_the_pipe` (a
  burst written just before git fails, with a slow progress callback and a pipe
  still held: its last line arrives) and the unit tests in `pipes.rs`.
- **Thread hygiene.** Every thread is counted while it runs;
  `every_thread_an_invocation_starts_ends_with_it` sees the three names started
  and the count back at zero once the invocation is over. The one thread that
  may outlive its invocation is a reader left on a pipe that a process git left
  behind still holds; it ends when that process lets the pipe go, or at its
  next send, since the receiver is gone, which closes the pipe on a writer
  that keeps writing
  (`a_reader_left_on_a_pipe_still_written_ends_when_its_invocation_lets_go`).
  A stdin writer whose reader is such a process and never reads is the same
  case.

### When it is over

`Driver::run` (`runner.rs`) ends the invocation when its leader has been reaped
and its pipes are drained — or `DRAIN_BOUND` (250 ms) after the exit, when
something else still holds a pipe.

- **The common case costs no tick.** git exiting closes its pipes, the readers
  end, the channel disconnects, and the driver reaps with `try_wait`, backing
  off from 50 µs to `TICK` (20 ms) because the kernel makes the process a
  zombie a moment after its files close.
- **A pipe still held** after git exits is noticed on the tick, which runs
  `try_wait` while the pipes are open. The driver then waits for what is
  already queued, and no longer than `DRAIN_BOUND` from the exit, the tick
  included. Pinned by
  `an_exit_with_a_grandchild_holding_the_pipes_returns_within_the_drain_bound`:
  the stub leaves a `sleep` holding both pipes, its last output still arrives,
  the return comes within the bound of the stub's last line and not before the
  drain, and the `sleep` — never cancelled — is left alone.

The values are pinned by `the_fixed_bounds_have_the_values_the_packet_recorded`:
the 2 s grace, the 256 KiB tail, the 250 ms drain bound.

**What the runner costs** (PRD G19): `git diff-tree -r -M -z --raw` on the
rust-lang/rust bench commit `5a3292f163d` against its first parent, through a
read invocation and through a bare `std::process::Command`, release build,
warm, on an AMD Ryzen 7 9800X3D with git 2.56.0: within noise of each other,
0.38 ms apart at the median in one run and none in another, against the 2 ms
the criterion allows; on the empty-diff floor, about 12 µs. The reporter is
`g19_reports_the_runners_overhead_over_a_bare_command`, `#[ignore]`d and driven
by `CAIRN_BENCH_REPO`.

### Cancelling

Three ways, all ending the process the same way:

- **The cancel signal** passed to `finish`, `records` or `collect`, polled every
  tick (`a_superseded_cancel_signal_ends_the_whole_group_and_reaps_it`).
- **A `KillHandle`** (`group.rs`): `Send`, `Clone`, and never blocking. It
  records the request, then only TRIES for the lock; a miss is not a lost
  cancel, because the driving thread sends the signal itself when it sees the
  request (`a_kill_handle_ends_the_whole_group_and_reaps_it`,
  `a_kill_that_misses_the_lock_returns_at_once_and_the_driver_finishes_it`,
  `the_kill_handle_is_send_and_clone`).
- **Dropping** an unfinished invocation asks for the end without waiting
  (`SIGTERM` if the lock is free) and drives it to the reap on a
  `cairn-git-reaper` thread — the one wait being the fallback below, when no
  such thread can start, which takes the group's lock, held only for a few
  non-blocking system calls
  (`dropping_an_unfinished_invocation_ends_and_reaps_its_group_without_blocking`
  times the drop and watches the group go, and
  `dropping_never_waits_for_a_process_that_outlasts_sigterm` times it against a
  process that takes the whole grace to end). If even that thread cannot start,
  the group is sent `SIGKILL` at once and the leader reaped if it already can
  be; one that outlives that is a zombie until Cairn exits, and a write's
  locks a `SIGKILL` strands go unreported, since nothing is left to report
  them (`a_drop_with_no_thread_to_reap_on_kills_the_group_at_once`). The same
  hand-off happens when a caller's callback panics: the driver, unwinding,
  gives the invocation to a reaper
  (`a_panicking_callback_still_ends_and_reaps_the_process`).

**Ending** is `SIGTERM` to the group (`killpg`), then `SIGKILL` to it once
`TERMINATION_GRACE` (2 s) has passed. A cancelled invocation waits for its
group: the leader reaped and its pipes closed, within the grace plus a tick.
When a pipe's holder has left the group, neither signal reaches it, and the
pipes are abandoned `DRAIN_BOUND` after the `SIGKILL`, so that cancel takes the
grace plus the bound
(`a_cancel_whose_pipe_holder_left_the_group_returns_after_the_grace_and_the_bound`).
A cancel that arrives after the leader has exited while a pipe is still held
signals the group too — whatever is left of what git started — and can take
the same; an uncancelled invocation in that state waits only the bound
(accepted by the user on 2026-10-02).
Pinned by `a_cancel_sends_sigterm_first_and_a_process_that_acts_on_it_is_not_killed`
(a trap that reports `SIGTERM`, ending inside the grace),
`a_process_that_ignores_sigterm_is_killed_after_the_grace` and
`a_cancel_that_lands_after_the_pipes_closed_is_still_escalated_to_sigkill`
(the driver escalates from its `try_wait` backoff too, not only while it
receives); each G8 test then
reads `/proc` and finds no member of the group alive or unreaped. With real
`git`, `a_commit_cancelled_inside_a_sleeping_hook_leaves_no_index_lock` cancels
a `commit -a` while its `pre-commit` hook sleeps, having seen `index.lock` held,
and finds the lock gone, the hook ended and nothing committed.

**When the group may be signalled.** Only while a member is believed alive —
the leader not yet reaped, or one of the pipes the readers hold still open —
and never once the invocation is over. The check and the signal are made under
the one lock the reap also takes, and the open pipes are read after that reap,
so nothing signals a reaped pid unless an open pipe still names the group
(`a_kill_after_the_invocation_is_over_signals_nothing`; and after a drain-bound
return, with the leftover process still holding the pipes, the G10 test's kill
signals nothing). A group id is not reused while any member lives, which
closes all but a stated race: an open pipe does not prove its holder is still
in the group, the count lags the pipe (a reader still handing on its last read
counts its pipe open after the writer closed it), and if every member exits
between the check and the signal the id could in principle be reused. The escalation widens that window: once the
leader is reaped and only an open pipe keeps the group believed alive, the
`SIGKILL` goes out up to the whole grace after the leader's pid was freed
(accepted by the user on 2026-10-02: the alternatives were not reaping until
the end — `waitid` with `WNOWAIT`, which nix 0.31 lacks on Apple and which
would wait on a pid std's `Child` owns — and skipping the `SIGKILL` once only
pipe evidence remains). One more
window is stated rather than closed: git may exit 0 between the reap's
`try_wait` and the `killpg`, and is then counted as signalled while running —
a completed invocation reported as cancelled (accepted by the user on
2026-10-02, for writes as for reads).

### Outcomes

`Invocation::drive` decides, after the reap, in this order:

1. A crossed ceiling is `Error::GitOutputTooLarge { arguments, ceiling,
   stranded_locks }`, whatever the exit status: a write collected under a
   ceiling that git then completes is reported as this error though its change
   stands — harmless while no write is collected; whether a ceiling may
   outrank a write's clean exit, or a write may have one at all, is issue #45
   (`a_collect_over_its_ceiling_is_refused_whole_and_the_process_ended`;
   `output_exactly_at_the_ceiling_is_collected_and_one_byte_over_is_refused`).
2. A failed stdin write, a pipe thread that could not start, or a `try_wait`
   that itself failed is `Error::GitUnwatched { arguments, source,
   stranded_locks }`. In the first two the process was ended; in the last,
   what became of it is not known.
   For a write, both list the lock files present after the reap, as a
   cancelled write does; for a read, nothing.
3. A cancel that lost the race to a clean exit — status 0 with no signal sent
   while the leader was running — is the success it was
   (`a_cancel_after_a_clean_exit_is_reported_as_success`). "No signal sent
   while running" is what the runner can see: a git that exits 0 after the
   last `try_wait` but before the `SIGTERM` lands is counted signalled, and
   reported cancelled though it completed — for fetch, a completed fetch the
   window calls cancelled, whose moved refs the worker still finds by
   comparing them before and after. Any other invocation
   asked to end is cancelled, whatever its status — a git that had already
   failed on its own when the request landed included, so the outcome carries
   neither its status nor its stderr, which its command-log record still
   holds (accepted by the user on 2026-10-02)
   (`a_cancelled_process_that_exits_zero_after_the_signal_is_reported_cancelled`,
   `a_cancelled_process_that_exits_nonzero_is_reported_cancelled`). What that
   means is the kind's (`Kind::cancelled` in `cli.rs`):
   - **a read** is `Error::GitReadCancelled { arguments }` and nothing more;
   - **a write** is `Error::GitCancelled { arguments, stranded_locks }`, the
     `*.lock` files under its git and common directories (recorded by
     `in_repository`) listed after the reap, so what git removed on its way out
     is not reported (`a_cancelled_write_lists_the_locks_present_after_the_reap`:
     a stale lock is listed and the `index.lock` git held is not;
     `a_cancelled_write_lists_its_locks_only_once_it_is_reaped`: a process that
     takes 300 ms to remove its lock on `SIGTERM` has it not listed). The
     listing is what was there at that moment. A cancelled write may still
     have taken effect, in part or whole — the signal can land after git made
     its change and before it exited — so an operation that must know
     compares the repository's state before and after, as fetch does with its
     refs. Whether that comparison becomes a review obligation for every write
     verb, and the cancelled message says a write may have taken effect, is
     for the first local write (issue #45).
4. A non-zero exit is `Error::GitFailed { arguments, status, stderr,
   present_locks }`, `stderr` the retained tail; for a write, `present_locks`
   lists the lock files present, which is what a write fails on and git never
   waits for (`a_failed_write_names_a_present_index_lock`). For fetch the list
   reaches the error's message, but the window's banner draws only git's
   first `fatal:`/`error:` line, so it does not reach the user (issue #44). Nothing retries,
   and nothing removes a lock. A read's is always empty
   (`a_failure_carries_the_arguments_the_status_and_stderr`,
   `a_real_read_fails_with_its_diagnostic_and_cancels_as_a_read`).

## The registry

Each `SharedRepository` holds one `Processes` (`process/registry.rs`), shared
with every worker handle `to_worker` makes from it. An invocation built with
`in_repository` carries a `Registration` for it, and that registration is the
one place the invocation's life is booked:

- **It enters the registry when its process is spawned** (`watch`, right
  after the group is made). A spawn that fails enters nothing and is logged as
  never started.
- **It leaves when the runner concludes the invocation**, on whichever thread
  that is — the caller's `finish`, `records` or `collect`, the reaper thread a
  drop hands it to, or the calling thread when a pipe thread could not start —
  and it writes the invocation's one log record as it leaves, log first, so
  whoever sees the registry empty sees the log complete. The registration is
  consumed by that, so a second record cannot be written; a registration
  dropped unfinished, which no path does, still writes one, exit unknown.
- **A drop with no thread to reap on** leaves at once, reaped or not: it sends
  `SIGKILL` and reaps only if it already can, and nothing is left that would
  reap it later.

An invocation run in no repository — the version probe, and the tests that
give none — is booked nowhere.

`SharedRepository::end_invocations(bound)` (`Processes::end_all`) is what
closing a repository runs, with `cairn_git::CLOSE_BOUND` ("Closing", below): every
invocation in the registry is asked to end the way a cancel asks (`SIGTERM`
to its group if the lock is free, the rest by the thread driving it), and the
call waits on a condition variable, up to `bound`, for the registry to empty.
It returns how many were still running when it stopped waiting. From then on
the repository is closing: an invocation that enters afterwards is asked to
end the moment it does. It waits, so it is a worker's call: the application
calls it on the repository thread.

What a close costs: a write that outlasts the grace is `SIGKILL`ed, which can
strand its lock files. Its cancellation still lists them, to whoever drives
it, but on a close nobody may be left to show them, and the next write in
that repository fails on them with git's own "File exists" message (issue
#44).

`CLOSE_BOUND` is 3 s: the 2 s grace and the 250 ms drain bound, which is the
longest a cancel can take, with three-quarters of a second to spare; a
compile-time assertion keeps it above their sum.

Pinned (`process/registry.rs`):
`a_finished_invocation_is_logged_once_with_every_field` (in the registry from
the spawn, not after the end),
`ending_every_invocation_ends_them_all_and_waits_for_their_reaps` (two driven
and one dropped, ended and reaped well inside the bound, each recorded once as
cancelled), `ending_every_invocation_waits_no_longer_than_its_bound` (a git
that ignores `SIGTERM` outlives a 100 ms bound, which says so, and is reaped
within `CLOSE_BOUND`), `an_invocation_started_after_the_end_is_ended_at_once`
and `every_handle_on_a_repository_shares_its_log`.

## The command log

A record is a `cairn_model::CommandRecord`: the arguments after the program,
lossily decoded; the directory it ran in; when it started, by the wall clock;
how long until it was over; how it ended (`CommandExit`: a code, a signal,
never started, or unknown); whether it was cancelled; and the retained
stderr tail. There is no field for the environment, so the askpass token an
invocation carried has nowhere to land, and none that could hold a `Secret`;
`the_record_holds_exactly_what_r8_1_lists` destructures it exhaustively, so a
new field stops it compiling. Its arguments are what Cairn passed — fetch
passes the remote name the application gives it.

`cancelled` is the rule the caller's outcome reads (`Ended::cancelled` in
`runner.rs`): asked to end and not beaten by a clean exit. A crossed ceiling,
a lost status and a pipe thread that could not start are the runner's own
ends and are not cancellations; a drop is.

The log (`process/command_log.rs`) is in memory, oldest first, and bounded
two ways, dropping its oldest records to stay under both:

- **`LOG_ENTRIES`, 1000 records** — what bounds it in the common case, where
  git says little: a long session's worth of fetches and of the reads
  `diff-engine` adds, one per selection.
- **`LOG_BYTES`, 4 MiB** of arguments, directories and stderr — what bounds it
  when git says a lot: sixteen full 256 KiB tails. It is above what one record
  holds at the platforms' default `ARG_MAX` (2 MiB on Linux, 1 MiB on macOS),
  so a record is trimmed only past those: its stderr keeps its end, then its
  arguments their start, with a last argument counting what went.

`SharedRepository::command_log()` answers it, and the worker hands it to
whoever asks: `Request::CommandLog` is answered on the repository thread by
`Update::CommandLog { records }`, the records oldest first. Nothing in the
application asks yet — no view draws the log (PRD R8.3; issue #41) — so
the request is its tests' alone. Pinned, G17's worker half:
`the_command_log_is_answered_through_the_worker_with_the_fetch_in_it`
(`worker/lifecycle_tests.rs`: empty before anything has run; after a fetch
through the boundary, one record with fetch's arguments, the repository's
directory, exit code 0, not cancelled, and the stub's stderr — the probe,
run in no repository, is not in it).

Residual, `qa-checklist`'s: the retained stderr is git's own text, kept for
the session where before it travelled only on an error. git can quote a
remote's URL there, and a URL configured with userinfo
(`https://user:token@host`) is a credential that is not a `Secret`. git
anonymises the URL in the messages checked (`From <url>`), but that every
message of every git from 2.30 does is not verified; scrubbing userinfo from
the tail would be a design change, and is not made; whether to make it, before
the log's view (issue #41) draws a tail, is issue #46. The arguments carry what
Cairn passed, which for fetch is a remote's name.

Pinned, G17's engine half, one record per exit path (`process/registry.rs`):
`a_finished_invocation_is_logged_once_with_every_field`,
`a_failed_invocation_is_logged_once_with_its_status`,
`a_cancelled_invocation_is_logged_once_as_cancelled` (by the handle and by the
signal), `a_dropped_invocation_is_logged_once_by_its_reaper`,
`a_drop_with_no_reaper_thread_is_logged_once`,
`an_invocation_that_never_started_is_logged_once`,
`what_the_runner_ends_is_logged_once_and_not_as_a_cancel`,
`a_cancel_beaten_by_a_clean_exit_is_logged_as_the_success` and
`an_invocation_in_no_repository_is_not_logged_in_one`; the bounds by
`the_log_keeps_the_newest_log_entries_records`,
`the_log_holds_no_more_than_log_bytes` and
`a_record_larger_than_the_bound_is_trimmed_to_fit_and_says_so`
(`process/command_log.rs`), and
`the_log_and_close_bounds_have_the_values_the_packet_recorded`
(`process/registry.rs`); and the token
by `a_fetch_with_a_token_is_logged_once_without_the_token_or_the_environment`
(`ops/fetch.rs`), whose stub writes the environment it was given to a file —
the token and every value looked for among it — while its record holds none
of them.

## The network lane

A `git` operation runs in a write lane (PRD R7, `docs/design/concurrency.md`,
"Operations"); one lane exists, the network lane, on the `cairn-network`
thread (its loop in `worker/network_lane.rs`, spawned by `Threads::start` in
`worker/pool.rs`), where fetch runs — what was the operations thread. The lane is chosen per operation: `Operation::lane` names one for each
operation by an exhaustive match, and the repository thread's
`Threads::perform` routes by it, so the local lane lands beside this one with
the first local write rather than being assumed away
(`a_fetch_runs_in_the_network_lane`).

One fetch at a time. `FetchControl::arm` claims the lane for a fetch of a
remote, or refuses it naming the fetch already in flight and how far it has
got — "a fetch of origin is already running", or "already waiting to start"
while it is queued — and a refusal goes to the window as
`Update::FetchRefused { remote, reason }`, never dropped (R7.2). The window
keeps it in its own state, beside the fetch in flight, which the refusal
leaves as it was, and draws it as a banner — "Fetch of origin not started:
…" — until the Fetch button is next pressed. The window hides that button
while a fetch is in flight, so a refusal is what a second fetch meets when
anything else asks for one.

A cancel is not tied to the fetch it was pressed for. The lane clears
`FetchControl` once a fetch is over and before its outcome reaches the window;
a Cancel pressed in that moment is kept as a cancel that overtook its fetch's
start, and the next fetch, whenever it is asked for, starts cancelled (issue
#47).

Pinned, G15:
`a_second_fetch_while_one_runs_is_refused_with_a_reason` (through the
boundary, with a stub fetch that hangs), `a_second_fetch_is_refused_naming_the_fetch_in_flight`
(`network_lane.rs`), `a_refused_fetch_is_kept_for_the_window_and_the_fetch_in_flight_is_untouched`
(`session.rs`), `a_refused_fetch_is_drawn_with_its_reason_until_the_next_press`
(`window.rs`, headless) and `a_refusal_names_the_fetch_refused_and_the_reason`
(`status_text.rs`).

## Closing

Closing the window closes its repository (PRD R6.3), and nothing that waits
runs on the UI thread:

1. The window's close hook (`WindowConfig::with_on_close`, `main.rs`) calls
   `Closing::requested` (`closing.rs`). The first time, it submits
   `Request::Close` and keeps the window open. `RepositoryHandle::submit`
   stops the epochs — an atomic store, so a page being walked is abandoned at
   its next poll and a diff's read is ended at the runner's next tick — and
   queues the close, which `serve` breaks on.
2. On the repository thread, `Threads::drop` tells the diff thread to stop
   (the window's handles hold its queue open, so it is told rather than left
   to see the queue close), closes the network lane's queue
   and calls `SharedRepository::end_invocations(CLOSE_BOUND)`: every `git` in
   the registry is ended the way a cancel ends it, and the thread waits up to
   `CLOSE_BOUND` for their reaps. The registry is the one authority here — a
   fetch in flight is ended by it, not by its cancel. Then it stops the
   acceptor, and the thread exits.
3. A fetch asked for just before the close goes one of two ways. If it is
   still on the repository thread's queue, `serve` has stopped (the epochs
   were stopped as the close was submitted) and it is never forwarded or
   started. If it has already been forwarded to the network lane, the lane
   takes it up after the close and spawns it — it reaches the network for
   that moment (issue #47) — and it is ended as it enters the registry: `end_all` marks the repository closing before it looks,
   and an invocation that enters afterwards is asked to end at once. A
   process does run for a moment before it is registered — between its
   spawn and its booking — and the closing mark is what covers that moment,
   not simultaneity. One that enters after `end_all` has stopped waiting is
   not in that wait; the network lane, which drives it to its reap, is what
   the window then waits on (step 4).
4. Each worker thread lets its update sender go as it exits, the network lane
   only once its fetch is reaped, so the stream's end means every `git` Cairn
   started in the repository is over — except a process git itself detached
   from the group, such as the auto-maintenance a fetch may start, or the
   fsmonitor daemon a read starts under `core.fsmonitor=true`, which outlive
   any group kill (`docs/design/processes.md`) — and the channel,
   and with it the askpass socket, is gone. A prompt still open holds the
   acceptor until the window refuses it, which the window does when the
   ended fetch's outcome arrives (`session::apply`, `withdraw`); a change to
   that would make every close with a prompt open wait for the patience
   below. The task driving the stream sees it end, and `Closing::worker_gone`
   says the window asked; the task then closes the window past the hook
   (`Platform::close_current_window`). Meanwhile the window draws nothing
   new — no banner says it is closing (issue #43) — and an outcome the close
   produced does not reload the history (`Closing::is_requested`,
   `session::Worker::closing`).

A window whose repository never opened, or whose worker has already gone,
closes at once. A second close request before `worker::CLOSE_PATIENCE` (5 s,
past `CLOSE_BOUND` and the acceptor's one-second stop deadline, asserted at
compile time) changes nothing; one after it closes the window anyway, so a
worker that has stopped answering cannot keep it open for good — at the cost,
then, of whatever that worker had not yet ended: the process exits, nothing
is left to drive a late `git` to its `SIGKILL` or reap it, and such a `git`
runs on, orphaned, holding whatever locks it holds (issue #48).

What a close does not bound: the network lane, after its fetch is reaped,
still reads the refs once more (`ref_tips`, which has no cancel and peels
every ref) before it lets its sender go, and it read them once before the
fetch started, so a repository with a great many refs and a cold cache can
hold the stream's end — and the window — past `CLOSE_PATIENCE`. The window
stays open and draws nothing until the second request closes it. The
constants above bound the reaps, not the scans (issue #43; the scans
themselves are #25's).

A close also ends any `git` in flight, a write included, without asking: the
window refuses nothing and the user is told nothing about what was running.
Fetch is the only verb today and is not destructive; the first local write
verb must decide whether the close hook may refuse while a write is in flight,
or say what the close cost (issue #45).

What a close costs is the registry's: a write that outlasts the grace is
`SIGKILL`ed and may strand its lock files, and its `FetchCancelled` lists
them on a stream about to end, to a window about to close, so nobody may be
left to read them (above, "The registry"; issue #44).

Pinned, G14: `closing_a_repository_ends_and_reaps_every_git_in_it_within_the_bound`
(`worker/lifecycle_tests.rs`: a stub fetch leading its group with a
grandchild holding its pipes; after `Request::Close` the stream ends inside
`CLOSE_BOUND`, nothing in the group is alive, the leader is reaped, and the
askpass socket is gone — deleting the `end_invocations` call fails it) and
`a_fetch_closed_as_it_starts_is_still_ended` (the fetch forwarded to the lane
before the close — the repository thread answering the command log in
between proves it — ends once, as a cancel, inside the bound; deleting the
`end_invocations` call fails it too), and
`a_close_stops_the_epochs_as_it_is_submitted_and_is_queued`; on the window's side,
`the_first_request_asks_the_worker_and_its_stream_ending_closes_the_window`,
`a_window_with_no_repository_closes_at_once` and
`a_window_whose_worker_has_gone_closes_at_once_and_only_when_asked`
(`closing.rs`). That the toolkit runs the hook and the window then closes is
G20, which only a display can show: checked by hand on a desktop session
when this landed (Linux, Hyprland, the window closed through the desktop's
own close action, under both the Wayland and the X11 backend), not by a test.

## The guards

All in `crates/cairn-guards/tests/invariants.rs`. Each asserts a nonzero file
count, and each has a matcher self-test:

| Twin | What it decides |
| --- | --- |
| `every_git_invocation_disables_the_terminal_prompt` | Outside `process/environment.rs`, no product file names or builds a `Command`, sets a process environment variable, or builds or implements `GitEnvironment`. Inside it: one `Command`, one literal, `env_clear` and `envs`, and the `ALWAYS` table's four pins. The `READ_ONLY` table carries each of `READ_ONLY_PINS` — `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1` — and is applied. The askpass names are set. |
| `only_the_process_module_builds_or_runs_a_process` | Outside `process/`, no product file names `Stdio`, `Child` or its pipes, `CommandExt` or `nix`. It calls none of `.spawn()`, `.output()`, `.status()`, `.wait()`, `.try_wait()` or `.wait_with_output()`, and does not call `GitEnvironment::command`. `GitEnvironment::command` stays `pub(super)`. `process/` itself must show the runner's own shapes — `.spawn()`, `.try_wait()`, `CommandExt`, `Stdio`, `Child`, `ChildStdout`, `ChildStderr`, `nix` and `.command(..)` — so the matcher is proven to read real code; `.output()` and `.wait()` were swapped out for `CommandExt` and `ChildStdout` when the old runner went, and stay banned outside `process/`. The one exception row in `PROCESS_CALL_EXCEPTIONS`, `.status()` in `cairn-app` (`history_state::Progress::status`, the history view's load state), fails once it is no longer needed; the row was accepted by the user on 2026-10-02. |
| `the_runner_is_named_only_by_ops_and_reads` | In `crates/cairn-git/src`, the runner's names (`GitCommand`, `read_invocation`, `Running`, `ProcessKill`, `Invocation`, `KillHandle`) are allowed in `process/`, `ops/` and `reads/` only. The write builder and `WriteAuthority` are allowed in `process/` and `ops/`. Constructing, writing a literal of or implementing `WriteAuthority` is allowed in `ops/` only. Nothing is declared or re-exported `pub`, and `process` stays private. The authority keeps its shape, and the doctests stay. |
| `the_retired_runner_is_gone` | Nowhere in `crates/cairn-git/src`, `process/` and test modules included, is `Running` or `ProcessKill` named, `run` or `stream` declared in an `impl` block of `GitCommand`, or `.stream(..)`, `GitCommand::run(..)` or `GitCommand::stream(..)` called (PRD R3.7, G18). Production `process/` calls `.spawn()` on exactly one line (today in `GitCommand::start_with`), and never `.output()`, `.status()` or `.exec()`, so a second runner that starts its own process by those method calls fails whatever it is called; a path call (`Command::spawn(&mut c)`) or a `nix` start (`fork`, `exec*`, `posix_spawn*`) is not counted, a residual stated in the root `CLAUDE.md`. Proven to read real code by finding `GitCommand`'s impl declaring `start`; self-test `the_retired_runner_matcher_catches_the_shapes_it_claims`. |
| `only_the_ops_module_mutates_a_repository` | No product file outside `ops/` and `process/` spawns `git` by its literal name. No file of `crates/cairn-git/src` outside `ops/` names gitoxide's mutation API. That roster was enumerated from the vendored gix 0.87.1 source and sits, with each entry's file and line, in `crates/cairn-guards/src/lib.rs`. It includes method calls — `.write(`, `.write_to(` and `.write_stream(` (`GITOXIDE_MUTATION_METHODS`) and `.notes(` (in `GITOXIDE_MUTATION_CALLS`) — the only shapes that catch an index's or a tree editor's `.write()` and a note write. The cost: engine code outside `ops/` cannot call `io::Write::write` or `RwLock::write` (`write_all` is fine), and a notes read would have to live in `ops/`. Accepted by the user on 2026-10-02. |

`the_unguarded_routes_to_a_process_now_fail_a_twin` pins the routes that
`docs/research/process-manager/runner-and-worker-as-built.md` section 3 found
open, each now failing a twin:

- `git.command().run()`, under its new name and its old one;
- `.stream()` with its kill;
- `GitCommand::new`;
- `git.environment().command(..).output()`;
- a `reads/` file constructing a `WriteAuthority`.

Outside the guard suite, `nix`'s features are pinned in `deny.toml` (a
`[[bans.features]]` row, `exact`: `process` and `signal`), which
`scripts/gate.sh --step deps` enforces, because the dependency allowlist reads
names, not features; and `reads/` and `process/` are in
`destructive-ops-reviewer`'s dispatch row in `docs/qa-gate.md`. Both accepted
by the user on 2026-10-02.

What the guards cannot decide is stated in the root `CLAUDE.md` beside each
invariant. That a read runs query plumbing, `status` or `diff --no-index` is
`destructive-ops-reviewer`'s check 10. These are `qa-checklist`'s item 7:

- a process or a gix write reached through an alias, a trait object or a macro;
- a retired entry point declared through a `type` alias of the builder, as a
  free function or by a macro, which the retired-runner twin sees only if it
  starts a process; two spawns on one line, which it counts once; and a
  second path built on `start` that drives an `Invocation` by rules of its
  own, which it does not see at all;
- a built or started invocation (`GitCommand`, `Invocation`) or a
  kill handle (`KillHandle`) handed out of `ops/` or `reads/` and driven
  elsewhere by inference. The runner guard reads names, so whether `ops/` and
  `reads/` hand out only named operation types (as fetch does) is review.
