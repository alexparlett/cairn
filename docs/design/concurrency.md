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
queues a diff behind it.

## Lanes and epochs

Every query belongs to a **lane** — history, changes, file diff — and carries an
epoch numbered per lane. A new query supersedes older ones in its own lane only,
except that a changes query also supersedes the file-diff lane; nothing else
crosses. One counter for everything would let a scroll cancel a selection.

The epoch is the cancel signal itself, not just a discard filter: the engine
polls it, so superseding a query stops its walk rather than discarding its
answer, and a superseded answer is never drawn. An answer also names the target
it answers, so a fast click never draws the previous commit's files under the new
commit's header. That is the part that is painful to retrofit, and what
`responsiveness-reviewer`'s cancellation check exists to protect.

An operation such as fetch carries no epoch, so a scroll and a fetch cannot
supersede each other; an operation is cancelled by killing its process. When it
finishes, the worker acts on what it reports invalidated (`engine.md`). Spec: `docs/prd/diff-engine.md` R4.
