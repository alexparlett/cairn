# Repository engine

Intent, not as-built; `docs/systems/` describes what exists. Spine:
`docs/design/cairn.md`, where this is decision **D1**.

## The split: gitoxide reads, `git` writes

Every repository read goes through `gix`, in process. Every mutation goes through
the `git` binary, invoked from `crates/cairn-git/src/ops/` and nowhere else. The
`git` CLI never runs on a read path; the one process a read may start is the
user's own clean filter driver, described under "Reads see git's form" below.

Writes go to `git` because of hooks, not coverage. A client that does not run
`pre-commit` and `commit-msg` is broken for a large share of users, and gix runs
neither. The same holds for clean/smudge filters on write (so Git LFS works),
`core.fsmonitor`, sparse checkout, submodule recursion and index locking — the
places where a reimplementation corrupts a repository quietly. Coverage is a
second, independent reason: gix has commit creation, three-way tree merge
(`merge_trees`, `merge_commits`, `virtual_merge_base`), merge bases, tag,
`delete_local_branches`, index read and write, blame, status, dirwalk and fetch,
but no push (its `gix::push` module is only `push.default` config parsing), no
branch-switch checkout, no reset, rebase, cherry-pick, revert or stash, and no
hunk-staging helper. Verified against the gix 0.87.1 source:
`docs/research/backend-split/gix-write-path-coverage.md`.

Reads go to gitoxide because reads are what a GUI does constantly, and its
performance work aims squarely at large repositories. Keeping the CLI off the read
path is the whole reason the split pays.

What it costs: a process spawn per mutation (milliseconds, against a
user-initiated action), and parsing git's output, mitigated by preferring `-z`
and porcelain v2 formats. Cairn locates `git` and checks its version at startup,
and fails loudly rather than degrading silently. Because gitoxide carries only
reads, no backend spike is needed to validate the bet.

## Behind the seam

The whole engine sits behind `cairn-model` types. `gix` types never appear in a
public signature, so replacing the backend for one operation is a change inside
`crates/cairn-git/`, invisible to every component. That is why the guard suite
defends the seam rather than the backend choice: the choice stays reversible
exactly as long as the seam holds.

## Two implementations of git, kept in agreement

With reads in gitoxide and writes in `git`, two implementations of git semantics
live in one application, and they can disagree. Three obligations follow.

1. **Every mutation says what it invalidated.** After a `git` subprocess writes,
   the gix handle may hold a stale index, stale refs or stale packs. Each
   operation in `ops/` reports what it invalidated (`Invalidated`, per flag, on
   its `Performed`), and the worker boundary acts on it (`concurrency.md`). Get
   this wrong and the UI shows the pre-write state, which reads to the user as
   "the operation failed". The contract is documented in
   `crates/cairn-git/src/ops/mod.rs`.
2. **Reads see git's form.** The bytes in the object database are not always
   what git would show, and a working-tree file is not always what git would
   store. A read converts working-tree content to git's form the way `git diff`
   does: through gix's filter pipeline (`gix-filter`), running the clean filter
   driver the path's attributes name and the user's config defines — git-lfs,
   git-crypt, nbstripout. Refusing to run it would show those users a diff
   `git diff` does not, and would hand staging a patch built from content their
   filter exists to change. Smudge is not applied on a read: everything is
   compared in git's form, so an LFS-tracked file in a commit shows as its
   pointer, modelled as a state to display rather than as content (`diff.md`).
3. **Edge cases can diverge.** gix's status against git's under sparse checkout,
   `core.fsmonitor`, or unusual attribute configuration is a real defect class:
   Cairn shows one answer and the user's next `git` command acts on another.

Running someone else's filter driver on a read has three residuals, stated rather
than implied. The driver runs with Cairn's own inherited environment plus the
repository's paths, because gix builds that process and not
`ops::GitEnvironment`. Its stderr is Cairn's, inherited. And `textconv` never
runs on a read, so a file with a textconv driver shows as binary where `git diff`
shows text. Spec: `docs/prd/diff-engine.md` R3; evidence:
`docs/research/diff-engine/gix-diff-api.md`.

## Destructive operations are sealed

Every destructive operation takes `cairn_model::Confirmed` by value, and the
token's only constructor records the prompt the user acknowledged. "Remember to
ask first" is exactly the kind of rule that holds for a year and then quietly
does not; a value the function demands moves it from review to the compiler, and
lets the operation log quote the prompt afterwards. The limit is honest: the type
guarantees a prompt happened, not that it was true, which is what
`destructive-ops-reviewer` exists for. The prompt says what will be lost, how
much, and whether it can be recovered — in that order (`ui.md`).
