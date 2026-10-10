# Repository engine

Intent, not as-built; `docs/systems/` describes what exists. Spine:
`docs/design/cairn.md`, where this is decision **D1**.

## The split: gitoxide reads, `git` writes

Every repository read goes through `gix`, in process, unless gix's answer to it
differs from git's. Every mutation goes through the `git` binary, invoked from
`crates/cairn-git/src/ops/` and nowhere else, with one exception: git has no verb
that removes a lock, so removing a stale `index.lock` the user confirmed is a
filesystem deletion of exactly that file, made in `ops/` after re-checking the
lock's age and identity against what was confirmed. A guard holds every
filesystem-mutating call to `ops/`, so the exception cannot widen unseen. A read runs the `git` CLI only where
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

The working tree's writes are each a named operation in `ops/`, run as a write
invocation with literal pathspecs — git's global `--literal-pathspecs` option
before the verb, as the reads that name paths pass it, never the
`GIT_LITERAL_PATHSPECS` variable, since an invocation's environment is built in
one place (`processes.md`) — and `--` or `--end-of-options` before any path, many
paths passed on stdin by `--pathspec-from-file=- --pathspec-file-nul` where the
verb takes it. git exports literal mode to the hooks it runs under these verbs
(`post-index-change`, and `post-checkout` where a verb runs it), so a hook's own
globbed pathspec matches literally: a residual of the option, stated rather than
implied.

- **Lines and mode changes**: `git apply --cached` to stage or unstage and
  `git apply` to discard, the patch on stdin and `--whitespace=nowarn` always
  (`diff.md`, "Selections and patches").
- **Whole files**: `git add` to stage; `git reset -q --` to unstage, which works on
  an unborn branch, `git reset -q -- <old> <new>` for a staged rename,
  `git reset -q HEAD^ --` out of an amend and `git rm --cached -f -q --` out of a root
  commit's amend, which has no `HEAD^` — forced, since without `-f` git refuses the
  very path an amend's staged list shows edited, one whose staged content differs
  from both the file and `HEAD`, where an unstage drops that content as `git reset`
  would; `git restore --worktree --` to discard. A
  submodule is never discarded: `git restore` exits 0 and leaves its commit where
  it was, only `git submodule update` moves it back, and no prompt can count what
  is dirty inside it. A conflicted path is staged with `git add`, which marks it
  resolved, and takes neither a patch of lines nor a discard; resolving its content
  is the conflict view's (`conflicts.md`).
- **Untracked files**: `git clean -f -q --` with the exact paths status listed —
  never a pattern, and never the list `git clean -n` prints, which is localised
  and quoted — and never `-d`: status lists untracked files one per file, and the
  one directory it lists whole, a nested repository, is refused, because deleting
  it deletes history the prompt cannot count. `-q` because a `git clean` left
  running unwatched by a second close dies at the first line it writes to a pipe
  nobody reads (`processes.md`, "Lifecycle"). `git
  clean` takes no pathspec file, so its paths go on `argv`, split across several
  invocations under one re-check of what was confirmed when the list is long.
- **Commits**: `git commit -q -F -` and `git commit -q --amend -F -`, the message
  on stdin so it is never on `argv` or in the command log, and no
  `--literal-pathspecs`, which git would export to every hook. No `--cleanup` is
  passed, so the user's `commit.cleanup` decides exactly as it does for their own
  `git commit -F`; `--no-verify` is passed only from the skip a failed commit or
  amend offers (`ui.md`, "The commit box"); the author is git's own identity — the one the
  user's terminal would commit with, since the identity variables are inherited
  (`processes.md`, "The environment") — and git's own error is shown when it has
  none. With a merge, a single cherry-pick or a single revert in progress the commit
  concludes it, as `git commit` does; during a rebase, `git am` or a sequence of picks
  or reverts Cairn does not commit, since continuing one is that operation's own — each
  read from the files git's own status reads, the sequencer's included. A non-UTF-8
  `i18n.commitEncoding`, as `git config` answers it, is refused with its reason, since
  transcoding the message would need a dependency. An amend's cost is read when it is
  pressed, in the job that runs it: one git logs and no remote has runs at once, being
  recoverable; one a remote has, or git logs nowhere — `core.logAllRefUpdates` as `git
  config` answers it — waits for the confirmation seal.
- **Creating a branch**: `git branch -- <name> <oid>`, which also recovers a lost
  commit; checked out as it is created, `git checkout -q -b <name> <oid> --`, which
  carries the working tree's changes over or is refused by git, writing nothing,
  where one would be overwritten; or, the user choosing to discard them, Fork's own
  command, `git checkout -q --no-track -f -b <name> <oid> --` — destructive, the one
  write that discards a staged change, and with it any untracked file in the way, as
  git decides: its `Consequence` is a fixed sentence that predicts nothing, and its
  re-check is that `HEAD`, the commit and the name are what they were. git leaves a
  submodule's change in place and discards a conflicted path, as Fork's command does; an
  operation in progress — a merge, a rebase, a cherry-pick, a revert, `git am` — is
  refused before git runs, since the forced checkout would abandon it without a word
  (evidence `docs/research/staging-and-commit/create-branch-discard-probe-2026-10-10.md`).
  The name is
  `-b`'s value, which git reads as the name whatever it begins with, and the commit
  its full id, the `--` after it saying it is no path. Spec:
  `docs/prd/staging-and-commit.md` R11.3.
- **A stale lock**: no verb exists, so `Remove index.lock…` deletes exactly
  `<gitdir>/index.lock` through the filesystem, in `ops/`, once the lock is
  re-checked as the one whose age the user confirmed — the single mutation not
  made by `git`.

Fetch, in the network lane, sits beside them (`credentials.md`). Spec:
`docs/prd/staging-and-commit.md` R3, R6. Evidence, each verb run on git 2.30.9,
2.32.7 and 2.56.0: `docs/research/staging-and-commit/git-write-verbs.md`.

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
`--type=bool --get <key>`, `--get <key>` or `--get-all <key>`, query form only, never a
setter (`reads::fetch_settings`), the second porcelain mode a read runs, accepted by
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
that overrides the user's rename or submodule settings (`reads::status`).
Untracked files are listed one per file, yet git itself reads the user's
`status.showUntrackedFiles`: a first read passes no `--untracked-files`, so git
applies the setting — `no` lists none, `normal` collapses an untracked directory
to one entry — and only where that answer holds a collapsed directory does a
second read, with `--untracked-files=all`, list its files. Cairn reads no
configuration of its own to decide, so the setting is honoured from wherever git
finds it — an include, a worktree's own file, the command line — as `git status`
honours it; the price is a tree holding an untracked nested repository, which git
lists as a directory either way, read twice every time. Like every read of the
working tree, a status read starts what the user's own `git status` starts: the
repository's `core.fsmonitor` hook or daemon, the clean filter driver of each
file whose stat changed, and, inside each submodule, `git status` with that
repository's own hook and filters ("Reads see git's form"). In a partial clone, a
staged rename whose blob only the promisor holds fails the whole read on a git
that honours `GIT_NO_LAZY_FETCH` (2.44 and later) — nothing listed, where the
user's own `git status` would fetch the blob and answer — and an older git, which
ignores the variable, fetches it, writing a pack and reaching the network: the
floor's residual, accepted with the 2.30 floor (as built: `docs/systems/status.md`). It is
the cheaper of the two as well, except on a tree whose every file's stat changed:
a read never writes the refreshed index back, so git rehashes every file each
time until something refreshes the index, where gix's in-process hashing is
several times faster. Under a split index git touches `sharedindex.*`'s mtime and under a sparse
index loose tree objects' mtimes, as the user's own `git status` does; neither
changes a byte. Evidence: `docs/research/refs-and-status/status-agreement-spike.md`
and `docs/research/refs-and-status/gix-refs-and-status-api.md`.

The fifth case is what a stash changed. A stash made with its untracked files keeps
them in a commit of their own, apart from its tracked changes, and with the user's
`stash.showIncludeUntracked` set, `git stash show` diffs the commit the stash was made
on against both at once — so rename and copy detection pairs a tracked file deleted
beside an untracked file of its content as one rename, where two diffs of the halves
print a deletion and an addition. No plumbing can diff one tree against two without
writing a tree or an index, so git answers: `git stash show --raw -z --no-abbrev
--no-color --no-ext-diff --no-textconv --no-relative --end-of-options <stash commit>`
(`reads::stash_changes`), the third porcelain mode a read runs, accepted by the user. In
raw form it prints no patch, so no textconv or external diff can run, it takes no lock
and reads no index, and git reads the setting itself, so a git that does not know it
lists what the user's own `git stash show` lists there.

Refs stay with gix, because gix agrees with `git for-each-ref` once it is read
with care: a symbolic ref is never peeled into its target's name, a dangling one
is hidden, a reflog — the stash's, and `HEAD`'s and each branch's for Show Lost
Commits — is read whole so that a long message cannot end it, and
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

A write also needs answers only git can give, and asks them as reads, each query
plumbing. Before a discard of lines, `git hash-object --path=<p> -- <p>`, without
`-w`, hashes the working-tree file in git's form — through the clean filter the
diff ran, writing no object — to check it is still the side that was drawn and
confirmed (`diff.md`, "Selections and patches"). Every discard, of lines or of
files, also compares the file's bytes and its executable bit as they are on disk,
the bytes hashed in process with no filter — a symlink as its target, as git stores
one — so an edit git's form does not show, a line ending alone, or a `chmod` still
refuses it (`docs/prd/staging-and-commit.md`
R3.9). Creating a branch asks one more: whether git takes a name for a new branch,
exactly as `git branch` and `git checkout -b` take it: `git check-ref-format --branch`,
which resolves a name in `@{-N}`'s form to another branch's exactly as both verbs do, where
`check-ref-format refs/heads/<name>` would take `-x` and `HEAD`, which both refuse; a name
holding `@{` is refused before git is asked, and a name a branch has, or one a branch's
folder holds, is looked up — so the dialog refuses, before anything runs, exactly the
names git would (R11.3). A commit asks two: `git
stripspace`, given a merge's, cherry-pick's or revert's message on stdin, so the draft is
what git's own editor session would leave under the repository's `commit.cleanup` and
comment character — comment lines gone under `strip`, kept under `whitespace`, the message as
written under `verbatim`, cut at git's scissors line under `scissors`, and no line a comment
under `core.commentChar=auto` — git reading the comment character itself, and what is shown
is what is committed (R6.10); and the
settings the commit and an amend depend on, `core.logAllRefUpdates` and
`i18n.commitEncoding`, through the same `git config` query fetch's refspec check
makes, since gix's reading of a linked worktree's `includeIf`, the system file and
trust is not git's (R6.11). None is argued from a measured disagreement with gix:
each is a question only git can answer — git's form of a working-tree file through
the user's filters, git's own rules for a branch's name, git's cleaning of a message
and git's reading of its configuration — so asking git is the only way to ask it.
Where hooks live is not asked at all: a failed commit always offers to skip them,
since git finds hooks no file check sees. Those inside an operation run on the local
lane and end with it rather than by a query's epoch; a branch name's check is a
query of its own, superseded by the next name typed. Spec:
`docs/prd/staging-and-commit.md` R3.9, R6.10, R6.11, R11.3.

Each read git answers is a named function in `reads/` and runs under a read's
environment — no optional locks, no askpass token. One a query asks is cancelled by
its query's epoch like any gix walk (`processes.md`, `concurrency.md`). A new one
is a decision — argued from a measured disagreement where gix has an answer, never
a convenience. Evidence: `docs/research/diff-engine/rename-parity-spike.md`.

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
the top of the working tree (`./-` for `-`) — one of the four porcelain modes
a read runs (the others are `git config` in query form, `git stash show` in raw
form and `git stripspace`, "Where git answers a read"), accepted because it reads no index and so has none to refresh — and the lines
Cairn holds for the working-tree side are rebuilt from git's own patch over the
old side, checked against the object id git names for that content. A staged
diff (`git diff-index --cached`) reads only objects; a staged rename or copy is
paired as the user's `git diff --cached` pairs it, by a first `git diff-index
--cached --raw --diff-filter=RC` over the whole index with their rename detection,
since plumbing reads no `diff.renames` and a pathspec of one path pairs nothing
(Spec: `docs/prd/staging-and-commit.md` R2.6). gix reads the index, fresh
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

Every destructive operation takes `cairn_model::Confirmed` by value, and the token
is bound to a `Consequence`: an engine-computed value naming what the operation
will destroy — the paths, the lines and bytes of each, the blob ids the loss was
computed against, a commit's id, a lock file's path and age — from which the
prompt is rendered, never typed beside it. The token is neither `Clone` nor `Copy`,
so one confirmation buys one operation, and only the confirmation surfaces on a
roster the guard holds construct it: the confirmation dialog, and the Create Branch
dialog, where choosing Discard and pressing Create and Checkout is the
acknowledgement, as in Fork (`ui.md`). Destructive means what cannot be undone or
what rewrites history someone else may have; what can be recovered asks nothing
and takes no token. The destructive operations are a roster of the guard's too —
discarding lines, discarding files, deleting untracked files, an amend a remote has
or git keeps no reflog for, Create Branch's Discard, and removing a stale
`index.lock` — and an operation on it without the token, or a roster entry with no
such operation, fails.

Immediately before it runs, each destructive operation re-reads the state its
`Consequence` names and refuses, writing nothing, when anything moved: a file
edited since the prompt, a file added to a directory about to be deleted, `HEAD`
moved before an amend, a lock removed and made again. A stale confirmation is an
outcome of its own, "changed since you confirmed", never a failure of git's. The
time between the prompt and the run is where a race would otherwise make the
prompt's words untrue — a stash dropped by an index another stash now holds, a
deletion list stale since it was counted, a hunk landing at an offset.

"Remember to ask first" is exactly the kind of rule that holds for a year and then
quietly does not; a value the function demands moves it from review to the
compiler, and lets the activity popover quote the prompt afterwards, since every
destructive operation's `Performed` records it. The limit is honest: the type
guarantees a prompt happened and that what it named has not moved, not that its
words were true, which is what `destructive-ops-reviewer` exists for. What the
prompt says, and in what order, is `ui.md`'s.

Staging, unstaging, committing and creating a branch are not destructive.
Unstaging a staged version the working tree no longer holds leaves that blob
unreachable; Fork does not confirm it and neither does Cairn, a residual that
stays `destructive-ops-reviewer`'s. No destructive operation takes a backup first:
what a confirmation protects is said in `feature-inventory.md`, "Recovery". Spec:
`docs/prd/staging-and-commit.md` R1.
