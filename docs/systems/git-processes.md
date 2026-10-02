# Git processes

How Cairn builds and runs a `git` process today, and what keeps that in one
place. As-built: everything here is code that exists, with the test that pins
each rule named beside it. The commitment it is being built against is
`docs/prd/process-manager.md` (in flight); the intent is
`docs/design/processes.md`, with D1 in `docs/design/engine.md`. What the askpass
helper does with the environment described here, and fetch end to end, are
`docs/systems/credentials.md`.

**What exists:** one crate-private module that builds every `git` process, an
invocation typed as a read or a write, and the two environments those kinds
get. The runner inside the module is the one the credential-prompts packet
built, moved unchanged: it runs a process to completion or streams its stderr,
and a cancel is `SIGTERM` to `git` alone, then `SIGKILL` after two seconds.
Process groups, a stdout reader, stdin, bounded stderr, the registry and the
command log are not built yet.

## The layout

```
crates/cairn-git/src/
  process/      crate-private; the only place a process is built, spawned, waited on or read
    binary.rs       GitBinary, GitVersion — discovery, the 2.30 floor, the read and write builders
    environment.rs  GitEnvironment — the base, ALWAYS, INHERITED, READ_ONLY, Profile
    askpass.rs      Askpass — where git and ssh are sent for a secret
    cli.rs          the runner: GitCommand<K>, Read, Write, Running, ProcessKill, Output
    stub_git.rs     (tests) a stub `git` on a PATH the test controls
  ops/          every mutation; constructs WriteAuthority; re-exports what the app needs
    authority.rs    WriteAuthority, and the tests that need one
    fetch.rs        fetch, built as a write
  reads/        each read `git` answers, one named function each — empty until diff-engine
```

`process` is a private module (`mod process;` in `lib.rs`). The application
reaches `GitBinary`, `GitVersion`, `GitEnvironment` and `Askpass` through
`cairn_git::ops`, which re-exports them because the application owns startup
and the helper's channel. It cannot reach the runner: nothing in it is `pub`.

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

Fetch is built as a write (`ops/fetch.rs`). The probe is built as a read
(`process/binary.rs`).

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

- **A read** (`Profile::Read`) adds the `READ_ONLY` table, `GIT_OPTIONAL_LOCKS=0`,
  and never a token, because the variant has no field for one.
- **A write** (`Profile::Write { token }`) adds `CAIRN_ASKPASS_TOKEN` when the
  operation was given a token, and nothing else.

The kind chooses the profile (`Kind::profile` in `cli.rs`). The caller never
does.

Why each variable is there, with its evidence, is beside it in
`environment.rs`. Two need a word here:

- **The editor is pinned to `false`, not `:`.** A verb that wants an editor fails
  instead of waiting on one nobody can see. `:` would silently accept whatever
  message git proposed. Both editor variables are set, because a user's
  `sequence.editor` outranks `GIT_EDITOR` for the rebase todo list.
- **`GIT_OPTIONAL_LOCKS=0` covers `git status` and nothing else.** Porcelain
  `diff` and `describe --dirty` refresh the index anyway. That is why a read in
  `reads/` runs query plumbing or `status` only, as the module's own docs say where
  `diff-engine` will read them.

Pinned:

- **Variable by variable, from the builder.** `environment.rs`:
  `the_environment_is_exactly_the_deliberate_entries`,
  `a_read_is_the_base_with_optional_locks_off_and_no_token`,
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
  - `a_commit_without_a_message_fails_promptly_instead_of_opening_an_editor`
    and `an_interactive_rebase_fails_promptly_instead_of_opening_the_sequence_editor`.
    Each configures an editor that records it ran and then hangs. Each verb
    fails within the deadline, the editor never runs, and `HEAD` does not move.

## The guards

All in `crates/cairn-guards/tests/invariants.rs`. Each asserts a nonzero file
count, and each has a matcher self-test:

| Twin | What it decides |
| --- | --- |
| `every_git_invocation_disables_the_terminal_prompt` | Outside `process/environment.rs`, no product file names or builds a `Command`, sets a process environment variable, or builds or implements `GitEnvironment`. Inside it: one `Command`, one literal, `env_clear` and `envs`, and the `ALWAYS` table's four pins. The `READ_ONLY` table carries `GIT_OPTIONAL_LOCKS=0` and is applied. The askpass names are set. |
| `only_the_process_module_builds_or_runs_a_process` | Outside `process/`, no product file names `Stdio`, `Child` or its pipes, `CommandExt` or `nix`. It calls none of `.spawn()`, `.output()`, `.status()`, `.wait()`, `.try_wait()` or `.wait_with_output()`, and does not call `GitEnvironment::command`. `GitEnvironment::command` stays `pub(super)`. `process/` itself must show `.spawn()`, `.output()`, `.wait()`, `.try_wait()`, `Stdio`, `Child`, `ChildStderr`, `nix` and `.command(..)`, so the matcher is proven to read real code. The one exception row, `.status()` in `cairn-app` (`HistoryProgress::status`), fails once it is no longer needed. |
| `the_runner_is_named_only_by_ops_and_reads` | In `crates/cairn-git/src`, the runner's names are allowed in `process/`, `ops/` and `reads/` only. The write builder and `WriteAuthority` are allowed in `process/` and `ops/`. Constructing, writing a literal of or implementing `WriteAuthority` is allowed in `ops/` only. Nothing is declared or re-exported `pub`, and `process` stays private. The authority keeps its shape, and the doctests stay. |
| `only_the_ops_module_mutates_a_repository` | No product file outside `ops/` and `process/` spawns `git` by its literal name. No file of `crates/cairn-git/src` outside `ops/` names gitoxide's mutation API. That roster was enumerated from the vendored gix 0.87.1 source and sits, with each entry's file and line, in `crates/cairn-guards/src/lib.rs`. |

`the_unguarded_routes_to_a_process_now_fail_a_twin` pins the routes that
`docs/research/process-manager/runner-and-worker-as-built.md` section 3 found
open, each now failing a twin:

- `git.command().run()`, under its new name and its old one;
- `.stream()` with its kill;
- `GitCommand::new`;
- `git.environment().command(..).output()`;
- a `reads/` file constructing a `WriteAuthority`.

What the guards cannot decide is stated in the root `CLAUDE.md` beside each
invariant. That a read runs query plumbing or `status` is
`destructive-ops-reviewer`'s check 10. These are `qa-checklist`'s item 7:

- a process or a gix write reached through an alias, a trait object or a macro;
- a built invocation or `Running` handed out of `ops/` or `reads/` and driven
  elsewhere by inference. The runner guard reads names, so whether `ops/` and
  `reads/` hand out only named operation types (as fetch does) is review.
