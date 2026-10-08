# Concurrency and cancellation

Intent, not as-built; `docs/systems/history-graph.md` describes the worker that
exists. Spine: `docs/design/cairn.md`, where this is decision **D3**.

## The UI thread never waits

A repository is somebody's 10-year monorepo, so every query is assumed slow. The
UI thread never waits on repository work: `cairn-git` is synchronous at its
boundary and decides nothing about where work runs; `cairn-app` runs it on worker
threads and hands answers back as values. Every view has a loading state distinct
from an empty answer.

## A few routed threads per repository

`gix::Repository` is not `Send`; gitoxide's model is a `ThreadSafeRepository`
(shared object database and pack indices) that each thread converts with
`.to_thread_local()` to get its own caches. Cairn holds one
`ThreadSafeRepository` per open repository and a small number of worker threads,
each taking a thread-local handle once: cache reuse where it is expensive, no
contention where it is not. Rejected: one thread per repository, which queues
every query behind the slowest, and one `Repository` behind a lock, which
gitoxide's design exists to avoid.

Work is routed to a thread by an explicit table, not handed to whichever thread is
free, because some work is pinned. A scroll keeps one gitoxide walk alive
(`history-graph.md`); that walk borrows the repository and is not `Send`, so every
page of it runs on the thread that owns the handle. History has its thread; diffs
— commit, comparison and working-tree — have another, so a long history page never
queues a diff behind it. The refs snapshot is read on the history thread, which
reopens the walk from it. Status — a `git` read that rehashes every stat-dirty
file — and ahead/behind — two walks a branch, as long as the branch has
diverged — run on a third thread, so neither a page nor a diff queues behind a
refresh.

## Lanes and epochs

Every query belongs to a **lane** — history, changes, file diff, the changed-file
filter, refs, ahead/behind, status and the sidebar's filter — and carries an
epoch numbered per lane. Finding a ref's commit pages the held walk, so it is
history-lane work, and a scroll and a find supersede each other. A new query supersedes older ones in its own
lane only, except that a changes query also supersedes the file-diff lane;
nothing else crosses. One counter for everything would let a scroll cancel a
selection, or a refresh a diff. A refresh — on focus, after an operation, or
asked for — supersedes the refresh before it in the refs and ahead/behind lanes.
Status is not superseded: a refresh leaves a running `git status` to finish, its
answer drawn, and every refresh asked while it runs becomes one follow-up status
after it, however many there were. Ending a status to start another would, on a
tree whose status takes longer than the gap between refreshes, never let one
finish; only closing the repository ends a running one.

The epoch is the cancel signal itself, not just a discard filter: the engine
polls it, so superseding a query stops its walk rather than discarding its
answer, and a superseded answer is never drawn. An answer also names the target
it answers, so a fast click never draws the previous commit's files under the new
commit's header. That is the part that is painful to retrofit, and what
`responsiveness-reviewer`'s cancellation check exists to protect.

A read that `git` answers (`engine.md`) is a query like any other: it belongs to
its lane, and superseding it ends its process (`processes.md`). Specs:
`docs/prd/diff-engine.md` R4 and `docs/prd/refs-and-status.md` R11 for the lanes,
`docs/prd/process-manager.md` R4.1
for a read's process.

## Operations

An operation carries no epoch, so a scroll, a stage and a fetch cannot supersede
one another. Each carries an operation id instead, and a cancel names the
operation it is for, so it reaches that one and no other, and ends it by killing
its process. When an operation
finishes, the worker acts on what it reports invalidated (`engine.md`) and no
more: a stage, unstage or discard refreshes status; a commit or an amend refreshes
refs, status and the history; creating a branch refreshes refs and the history.

Operations run in two **write lanes** per repository — distinct from the query
lanes above, which order reads — each one at a time and in order:

- **Network** — fetch, push, and the transfer half of pull. These touch the
  local repository only to update remote-tracking refs and add objects.
- **Local** — everything that writes the index, the working tree or a local ref:
  staging, discarding, commit and amend, checkout, reset, stash, branch and tag.

The two lanes run beside each other, so a two-minute push or a slow `pre-push`
hook never holds up a stage click. They can, because where they meet — a ref and
`packed-refs` — git takes its own locks with its own retry, while the index, which
git never waits on, is written only from the local lane. Reads run beside both:
gix takes no lock, and a `git` read runs with optional locks off.

The local lane is a thread of its own, which a request reaches directly rather
than through the repository thread, where a stage would wait behind a deep find.
Its writes run first in, first out. A write asked while another runs is queued,
drawn as queued where the user acted, and never refused for being second, so a run
of stage presses lands in the order pressed. A queued patch is checked against the
content it was built from when its turn comes, not when it was asked
(`diff.md`, "Selections and patches"), and a patch the writes ahead of it made
stale is dropped with a note naming its path.

A read that runs while a write does may see a moment between two states, so the
lane keeps a write counter, and a status read that began before the latest write
ended is discarded rather than drawn: the refresh after the write is the one that
shows its result. While a commit runs, Cairn starts no refresh of its own — on
focus, on the Refresh chord, or after another operation — and runs no queued index
write; they run when it ends, so its hooks see the index they were given and no
write of Cairn's meets the commit's own `index.lock`.

Of the local writes only a commit — or an amend — is cancellable, and only while
it runs, because its hooks are the one local write that may run for minutes; killing it in its
hooks leaves no `index.lock`. A cancelled or unwatched write reports that it may have
taken effect, and the refresh after it shows what did. Closing the window while a
local write runs waits for it and says which ("Finishing commit…"), never killing
it; a second close after the window's patience closes anyway, and a lock that
leaves behind is named the next time the repository opens (`processes.md`,
"Lifecycle").

When the user starts a network operation while its lane is busy, it queues, and
the window says it is queued. When it is a second copy of one already running or
queued — a second fetch — it is refused with that reason, never silently dropped.
Work Cairn starts on its own, such as an automatic fetch, is skipped while its
write lane is busy rather than queued behind the user's, and it never prompts: its
askpass fails closed, so a missing credential becomes an error the user can
retry, not a dialog they did not ask for.

A write lane is keyed by repository — the common directory every worktree of it shares
— so two worktrees of one repository share their local lane. The index and `HEAD`
belong to one worktree, so a key per worktree would be enough for them; refs,
objects and config are shared, and the local lane writes both. Whether worktrees
want it split is open in the spine.

Rejected: one write lane for every operation, which cannot race but puts every stage
click behind the slowest push. Rejected: a read/write lock over the repository,
which makes reads wait on writes they do not conflict with. Rejected: refusing a
local write while the lane is busy, which fails rapid stage clicks, and routing
local writes through the repository thread. Evidence:
`docs/research/process-manager/consumer-invocations.md` (what each operation
locks), `docs/research/process-manager/precedent-study.md` (how other clients
serialise) and `docs/research/staging-and-commit/write-path-as-built.md`. Specs:
`docs/prd/process-manager.md` R7, `docs/prd/staging-and-commit.md` R4.
