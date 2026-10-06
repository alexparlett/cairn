# Repository engine

Intent, not as-built; `docs/systems/` describes what exists. Spine:
`docs/design/cairn.md`, where this is decision **D1**.

## The split: gitoxide reads, `git` writes

Every repository read goes through `gix`, in process, unless gix's answer to it
differs from git's. Every mutation goes through the `git` binary, invoked from
`crates/cairn-git/src/ops/` and nowhere else. A read runs the `git` CLI only where
showing what git shows means asking git, and each such read is a named function
in `crates/cairn-git/src/reads/`. The changes query — which paths a commit or a
comparison changed, with their renames and copies — is one, because rename and
copy detection is where gix and git disagree; which lines of a changed file
changed is another, because line diffing is too ("Where git answers a read",
below); and so is one path's working-tree diff, because only git's own read of
the working tree is git's form of it ("Reads see git's form", below); and so is
the working tree's status, which gix answers differently from git wherever
status is hard. Besides
`git` itself, the programs a read may start are the user's own clean filter
driver, which git runs on a read of the working tree, and the repository's
`core.fsmonitor`, which git consults as it reads the index of a repository with
a working tree — each exactly as the user's own `git diff` runs it. The
fsmonitor is either a hook, a program git runs as a child of the read, or,
under `core.fsmonitor=true`, git's own fsmonitor daemon, which the first read
to consult it starts if none is running (the user's decision,
accepted as parity): the daemon writes its socket and cookie directory,
`.git/fsmonitor--daemon.ipc` and `.git/fsmonitor--daemon/`, in the git
directory — the one thing a read leaves there; no object, ref, index or config
is written — and runs in a session of its own, so it outlives the read and the
application and is not Cairn's to end. How every `git` process is built, run
and ended is `processes.md`.

Writes go to `git` because of hooks, not coverage. A client that does not run
`pre-commit` and `commit-msg` is broken for a large share of users, and gix runs
neither. The same holds for clean/smudge filters on write (so Git LFS works),
`core.fsmonitor`, sparse checkout, submodule recursion and index locking — the
places where a reimplementation corrupts a repository quietly. Coverage is a
second, independent reason: gix has commit creation, three-way tree merge
(`merge_trees`, `merge_commits`, `virtual_merge_base`), merge bases, tag,
`delete_local_branches`, index read and write, blame, status, dirwalk and fetch,
but no push (its `gix::push` module is only `push.default` config parsing), no
branch-switch checkout (`gix_worktree_state::checkout` is reachable only from the
clone path), no reset, rebase, cherry-pick, revert or stash, and no
hunk-staging helper. Verified against the gix 0.87.1 source:
`docs/research/backend-split/gix-write-path-coverage.md`.

Reads go to gitoxide because reads are what a GUI does constantly, and its
performance work aims squarely at large repositories. Keeping the CLI off the read
path wherever gix agrees with git is the whole reason the split pays.

What it costs: a process spawn per mutation (milliseconds, against a
user-initiated action) and per read git answers (below), and parsing git's output, mitigated by preferring `-z`
and porcelain v2 formats. Cairn locates `git` and checks its version at startup,
and fails loudly rather than degrading silently. Because gitoxide carries only
reads, no backend spike is needed to validate the bet.

## Where git answers a read

Showing the user something git would not show them is a defect, not a gap to
document, so where gix and git disagree on a read the user can see, git answers
it. Rename and copy detection is the case that forced the rule: on the largest
rollup measured, gix paired 231 renames where git paired 2,774, because gix
compares `diff.renameLimit` against a different quantity than git does and has
no basename stage, and even where gix searches exhaustively its similarity
measure and its pairing pick different pairs from git's. So the changes query
runs `git diff-tree`, at git's own cost — a few tens of milliseconds a
selection, under a hundred on the worst subject for renames and about 130 when
copies are detected too.

Line diffing is the second case. A diff algorithm leaves choices open — where an
inserted block sits among equal lines, which of two equally short scripts to
print — and gix's diff makes some of them differently from git's even under the
same algorithm and indent heuristic, lacks patience altogether, and ignores the
algorithm a diff driver names; on this repository's own history it placed some
files' hunks somewhere `git diff -U3` does not
(`docs/research/diff-engine/content-parity-spike.md`). Comparing lines with their
whitespace removed diverges the same way, and the text git prints after a hunk
header's `@@` — the enclosing function, by the path's `xfuncname` — is git's
alone. So a file's changed ranges, its whitespace-ignoring ranges and each hunk's
function context come from `git diff-tree -p`, asked at the context the view
shows, with the algorithm the user's `git diff` would use and every printed line
checked against the lines gix read; one call per comparison answers every file
at once. gix keeps what it agrees with git on, or what git has no answer for:
history, reading both versions of a file and deciding what is not text (binary,
too large, LFS, submodule) before anything is diffed, intra-line highlights, the
patch emitter and the model they feed.

The third case is configuration a write is about to act on. Before a fetch
starts, its refspec check decides from the remote's configuration whether the
fetch would write local branches or prune local tags, and that decision is only
sound if it reads what the fetch's own git will: gix 0.87 evaluates a linked
worktree's `includeIf "gitdir:..."` against the common directory where git uses
the worktree's own git directory, reads the system file from its own path, and
decides trust by an owner rule of its own, and each of those once let through a
fetch git then made. So git answers: `git config --includes --null` with
`--type=bool --get <key>` or `--get-all <key>`, query form only, never a setter
(`reads::fetch_settings`), the second porcelain mode a read runs, accepted by
the user; the check fails closed when the read fails.

The fourth case is status: which paths are staged, changed, untracked or in
conflict. Status is the read most exposed to everything that makes git's answer
hard — rename detection between `HEAD` and the index, `core.fsmonitor`, sparse
checkout, a split or sparse index, conflicted stages, intent-to-add, submodules
— and gix's answer differs from git's across them, several times silently:
staged renames with edits reported as adds and deletes once there are enough of
them, `status.renames` read from the wrong section, a staged deletion nobody made
under a split index beside a sparse one. gix's status also starts the user's
clean filter itself,
from Cairn's own process and environment, outside `processes.md`'s one place. So
git answers: `git status --porcelain=v2 -z`, run as a read, with nothing passed
that overrides the user's rename or submodule settings (`reads::status`). It is
the cheaper of the two as well, except on a tree whose every file's stat changed:
a read never writes the refreshed index back, so git rehashes every file each
time until something refreshes the index, where gix's in-process hashing is
several times faster. Under a split index git touches `sharedindex.*`'s mtime and under a sparse
index loose tree objects' mtimes, as the user's own `git status` does; neither
changes a byte. Evidence: `docs/research/refs-and-status/status-agreement-spike.md`
and `docs/research/refs-and-status/gix-refs-and-status-api.md`.

Refs stay with gix, because gix agrees with `git for-each-ref` once it is read
with care: a symbolic ref is never peeled into its target's name, a dangling one
is hidden, the stash reflog is read so that a long message cannot end it, and
every branch's upstream is resolved by hand from the configuration as git
resolves it — the last remote, the first merge, a local upstream (`remote = .`)
resolved as a ref name, a named remote's merge matched literally against its
fetch refspecs in order — where gix answers none for a local upstream and
otherwise for a second merge, a short merge or two matching refspecs. A ref
naming a missing object is skipped and counted, where git refuses to list
anything. gix cannot read a reftable repository at all, and opens one only to
fail at `HEAD`, so such a repository is refused at open with the reason, and so is
a format-version-0 repository that names a ref storage, as git refuses it. Evidence:
`docs/research/refs-and-status/gix-refs-and-status-api.md`.

Each such read is a named function in `reads/`, runs under a read's environment
— no optional locks, no askpass token — and is cancelled by its query's epoch
like any gix walk (`processes.md`, `concurrency.md`). A new one is a decision,
argued from a measured disagreement, never a convenience. Evidence:
`docs/research/diff-engine/rename-parity-spike.md`.

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
   its `Performed`), and the worker acts on it (`concurrency.md`). Get this wrong
   and the UI shows the pre-write state, which reads to the user as "the
   operation failed". The contract is documented in
   `crates/cairn-git/src/ops/mod.rs` and summarised in
   `docs/systems/credentials.md`.
2. **Filters affect reads, not only writes** — the next section.
3. **Edge cases can diverge.** A gix read against git's under sparse checkout,
   `core.fsmonitor`, or unusual attribute configuration is a real defect class:
   Cairn shows one answer and the user's next `git` command acts on another.
   Status is the clearest case, and git answers it.

## Reads see git's form

The bytes in the object database are not always what git would show, and a
working-tree file is not always what git would store.

**Clean, on the way in.** A read converts working-tree content to git's form the
way `git diff` does, running the clean filter driver the path's attributes name
and the user's config defines — git-lfs, git-crypt, nbstripout — with line-ending
conversion, `ident` and a working-tree encoding. The driver may be a `clean`
command run per file or a long-running `process` — what `git lfs install`
configures — which git starts once per read and sends only `command=clean`. Refusing to run it would show
those users a diff `git diff` does not, and would hand staging a patch built from
content their filter exists to change. git does the converting: a working-tree
read is `git diff-files` (the index against the working tree) or, for a file git
does not track, `git diff --no-index -- /dev/null <path>`, the path relative to
the top of the working tree (`./-` for `-`) — one of the two porcelain modes
a read runs (the other is `git config` in query form, "Where git answers a
read"), accepted because it reads no index and so has none to refresh — and the lines
Cairn holds for the working-tree side are rebuilt from git's own patch over the
old side, checked against the object id git names for that content. A staged
diff (`git diff-index --cached`) reads only objects. gix reads the index, fresh
for every query, for what git's answer does not say alone, and every blob git
names.

**Smudge, on the way out, is not applied.** Everything is compared in git's form,
so an LFS-tracked file in a commit shows as its pointer, modelled as a state to
display rather than as content (`diff.md`). Showing smudged content — an LFS
file's real bytes — is the half this leaves unanswered.

Running someone else's filter driver on a read has residuals, stated rather than
implied:

- **What runs it, and with what.** git starts the driver, as a child of the read's
  own `git` process, so it runs with the environment Cairn built for that read —
  the inherited roster and the `ALWAYS` table of `ops::GitEnvironment`, with
  `GIT_ASKPASS` and `SSH_ASKPASS` naming Cairn's helper and, while the
  application listens for it, `CAIRN_ASKPASS_SOCKET`, and the read's
  `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1` and no askpass token — so a
  driver can reach the helper's socket, but without a token it fails closed —
  plus what git sets for a filter (`GIT_EXEC_PATH`, `GIT_PREFIX`,
  `GIT_CONFIG_PARAMETERS`, and `PATH` with git's exec directory first; and
  `GIT_DIR` and `GIT_WORK_TREE`, since Cairn names every repository it opens
  to git). The roster still hands it the user's `PATH`, `HOME` and the rest; that
  is what makes git-lfs work, and it is the driver's to use.
- **What it writes is its own.** A driver may keep a store of its own — git-lfs's
  clean copies the file into `.git/lfs/objects`, as it does under the user's
  `git diff` — which no read of Cairn's can prevent; git itself writes nothing.
- **Its stderr is git's**, kept as the read's bounded tail and shown with a
  failure. A driver that fails and is `required` fails the read, with git's
  diagnostic. One that is not required makes git fall back to the unfiltered
  content with a warning on stderr; Cairn shows the diff git shows, without that
  warning, since stderr is prose it never parses.
- **A submodule's checkout is looked into by git**: `diff-files` runs `git status`
  inside it to say whether it is dirty, which may run that repository's own
  fsmonitor and clean filters, as the user's `git diff` does.
- **`textconv` never runs on a read**, so a file with a textconv driver shows as
  binary where `git diff` shows text.

Spec: `docs/prd/diff-engine.md` R3; as built: `docs/systems/diff.md`; evidence:
`docs/research/diff-engine/content-parity-spike.md` section 3.

## The confirmation seal

Every destructive operation takes `cairn_model::Confirmed` by value, and the
token's only constructor records the prompt the user acknowledged. "Remember to
ask first" is exactly the kind of rule that holds for a year and then quietly
does not; a value the function demands moves it from review to the compiler, and
lets the operation log quote the prompt afterwards. The limit is honest: the type
guarantees a prompt happened, not that it was true, which is what
`destructive-ops-reviewer` exists for. What the prompt says, and in what order, is
`ui.md`'s.
