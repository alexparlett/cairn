# Progress — process-manager

Running log, newest first. Historical record: entries are never retro-edited.
Correct course in a new entry.

## 2026-10-02 — phase 01: the seal and the environment

Packet mode, committed directly to `feature/process-manager`.

**What landed** (`f1d1d5b`, `73c9dd4`; QA fixes `ea52ef7`, `e52abf4`):
- `GitBinary`, `GitEnvironment`, `Askpass` and the runner moved from `ops/` into
  the crate-private `process/`, unchanged in behaviour.
- Invocations are now `GitCommand<'_, Read>` or `GitCommand<'_, Write>`, and a
  write consumes a `WriteAuthority` that only `ops/` can construct. Fetch is a
  write. The version probe is a read.
- `ALWAYS` pins `GIT_EDITOR=false` and `GIT_SEQUENCE_EDITOR=false`. A read adds
  `GIT_OPTIONAL_LOCKS=0` and has nowhere to carry a token.
- `reads/` exists, empty and documented.
- Three guard twins are new or rewritten:
  - `only_the_process_module_builds_or_runs_a_process` (new);
  - `the_runner_is_named_only_by_ops_and_reads` (new);
  - `only_the_ops_module_mutates_a_repository`, which gains a gitoxide mutation
    roster enumerated from vendored gix 0.87.1, with file:line per entry.
- The environment twin moved to `process/environment.rs` and gained the new
  pins.
- `docs/systems/git-processes.md` is new.

**Acceptance and the tests that pin each criterion:**
- G1: the compile_fail doctests in `ops/mod.rs`.
- G2: the twins above and their matcher self-tests, plus
  `the_unguarded_routes_to_a_process_now_fail_a_twin`.
- G3: the builder tests in `environment.rs`, and the stub-git tests in
  `process/cli.rs`, `ops/authority.rs` and `ops/fetch.rs`.
- G4: `a_status_read_leaves_a_stale_index_byte_identical`.
- G5: `a_commit_without_a_message_fails_promptly_instead_of_opening_an_editor`,
  plus its `rebase -i` twin.

Removing each environment pin turns its own G3, G4 or G5 test red.
GIT_EDITOR=false broke no fetch test, so the stopping rule did not fire.

**Checks run by hand:**
- The four routes found unguarded in `runner-and-worker-as-built.md` section 3,
  a `reads/` file constructing a `WriteAuthority`, and a gix index write were
  each written as real scratch files. Every one failed the guard suite.
- Every compile_fail doctest was turned into a plain block. Each fails on
  privacy alone, and still does with its method widened to `pub`, because the
  types they take or return are crate-private.

**QA:** qa-checklist, gate-integrity-reviewer, destructive-ops-reviewer and
test-coverage-auditor ran fresh over `main...HEAD`, and a fresh qa-confirm
adjudicated.

Confirmed and fixed:
- **Two doctests failed on their arguments.** The `write_invocation()` and
  `command(.., None)` doctests failed on arity and type, not privacy. Every
  argument is now `unreachable!()`.
- **Nothing pinned `command`'s visibility.** The guard now pins
  `GitEnvironment::command` as `pub(super)`.
- **The doctest pin was too loose.** It now requires each refused block to be
  exactly the scaffold plus its one line.
- **Roster entries could be dropped unnoticed** (`ProcessKill`, `ChildStdin`,
  `ChildStdout`). The self-test cases are now spelled out apart from the
  rosters, with a length check.
- **The "process/ must show these shapes" claim was overstated.** It now lists
  the shapes actually required, and `.wait()` and `ChildStderr` were added to
  them.
- **The guard scope was misstated.** "cairn-git" is now
  "crates/cairn-git/src".
- **`scanned` counted exempt files.** The `process/` files exempt from
  `spawns_git` no longer count.
- **CLAUDE.md's D1 bullet stated planned work as present.** It now says the
  changes query "will be the first".
- **"Plumbing" admitted writers.** It is now "query plumbing", the plumbing
  writers are named, and `ls-files`, `rev-parse` and `cat-file` are no longer
  claimed as backed by C3.
- **The G4 control differed from the read in more than profile.** It now
  differs in profile alone.
- **The runner hand-off residual was unstated.** A built invocation or
  `Running` handed out of `ops/` or `reads/` and driven elsewhere by inference
  is not seen by the guard. This is now stated in CLAUDE.md, git-processes.md
  and qa-checklist item 7, owned by qa-checklist. Raising it to a guard check
  is a possible future choice for the user.

Dismissed, with reasons:
- **C2:** an unsealed `Kind` inside `process/`. `Kind` and `Profile` cannot be
  named outside `process/`, which builds the `Command` by design. The "any verb"
  half is already check 10's stated residual.
- **D5:** line numbers in the gix roster comments. They cite an immutable
  published version that the comment names, so they cannot rot silently.
- **D6:** future editor verbs (revert, cherry-pick, amend, `tag -a`) will fail
  under `GIT_EDITOR=false`. That is the intended G5 behaviour. Those packets
  pass `-m`/`--no-edit`, and git's 'false' message should be translated in the
  UI.
- **D7:** `printed_environment` drops the shell's own variables. That is needed
  for an exact comparison, and `the_parent_is_asked_about_the_inherited_roster_and_nothing_else`
  pins inheritance.
- **E1:** gix sub-crate roster completeness. qa-confirm spot-checked gix-ref,
  gix-index, gix-odb, gix-pack, gix-lock, gix-tempfile, gix-fs, gix-worktree,
  gix-merge and gix-note, and found no missing disk writer.

Probed, then resolved: A2, the `read_invocation()` doctest, still fails when
widened. It fails on privacy of the return type `GitCommand<'_, Read>`, which is
honest. The method's own visibility is pinned by `declares_publicly`.

Escalated to the user: D3, the partial-clone lazy fetch. It is in `state.md`
under Open questions.

**Pending user review, not decided:**
- **The gitoxide guard's method-call bans.** `.write(`, `.write_to(`,
  `.write_stream(` and `.notes(` are banned outside `ops/` in
  `crates/cairn-git/src`.
  - Why: they are the only shapes that catch `open_index()?.write(..)`, a tree
    editor's `.write()`, and note writes.
  - Cost: non-ops engine code cannot call `io::Write::write` or
    `RwLock::write` (`write_all` is fine), and a notes read would have to live
    in `ops/`.
- **The `PROCESS_CALL_EXCEPTIONS` row** excusing `.status()` under
  `crates/cairn-app/src`, which is `HistoryProgress::status`. The row fails once
  it is no longer needed.

**Environment note:** /tmp (tmpfs) filled up during QA, about 30G of it from
other projects' Claude session directories, which were left alone. Running the
gate with TMPDIR pointing inside the worktree breaks the askpass tests: the
socket path exceeds SUN_LEN. The full gate was run once /tmp had room again.

## 2026-10-02 — planned

The packet was planned with `/feature-plan` while `diff-engine` was in flight, when its
changes query hit rename parity (its 2026-09-30 progress entry). Four recon records are saved under
`docs/research/process-manager/`, and the two that commissioned the packet were
copied from the `diff-engine` branch into `docs/research/diff-engine/` so `main`
can cite them.

- L1-L4 were locked by the user. L5-L15 were presented as defaults and stood.
- The design landed in the same PR:
  - `docs/design/processes.md` (new);
  - D1 rewritten in `engine.md` so that `git` answers a read where gix diverges;
  - the write lanes in `concurrency.md`;
  - the spine's map, summary and open list.
- The PRD is `docs/prd/process-manager.md`, with five phases. No code has
  changed.
