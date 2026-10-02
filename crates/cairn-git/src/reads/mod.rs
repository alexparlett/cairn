//! Reads that `git` answers: one named function per read, and nothing else.
//!
//! Every repository read goes through gitoxide, in process, unless gix's
//! answer differs from git's (D1, `docs/design/engine.md`). Where showing what
//! git shows means asking git — the changes query, whose rename and copy
//! detection is where the two disagree — the read is a function here, built
//! with [`crate::ops::GitBinary`]'s read builder, and the runner is reached from
//! nowhere else but `ops/`. Empty until `diff-engine` adds its first function;
//! the module exists so that the guard suite can name it as one of the runner's
//! two callers.
//!
//! # What a read may run
//!
//! **Query plumbing, or `git status`, and nothing else.** A read runs with
//! `GIT_OPTIONAL_LOCKS=0`, so that looking at a repository never refreshes its
//! index behind the user's back or holds `index.lock` while their own
//! `git commit` needs it. But only `status` honours that variable: porcelain
//! `git diff` against the working tree refreshes the index whenever
//! `diff.autoRefreshIndex` finds stat-only changes, and `git describe --dirty`
//! refreshes it too, each taking `index.lock` to write it whatever the
//! variable says. The diff plumbing — `diff-tree`, `diff-index`,
//! `diff-files` — never writes the index (evidence:
//! `docs/research/process-manager/platform-and-git-behaviour.md`, C3). So a
//! read that wants a diff runs `diff-tree` or `diff-index`, never `diff`; and
//! a refresh, if one is ever wanted, is a write, built in `ops/`. Any other
//! query plumbing a read adds (`ls-files`, `rev-parse`, `cat-file`, ...) brings
//! its own evidence that it writes nothing, because C3 does not cover it.
//!
//! Plumbing is not the same as a query: `update-ref`, `update-index` (its
//! `--refresh` included), `read-tree`, `write-tree`, `hash-object -w` and
//! `commit-tree` are plumbing writers, and each is a write, built in `ops/`.
//!
//! **A read never lazily fetches — on git 2.44 or later.** In a partial
//! clone, asking for an object only the promisor remote holds fetches it,
//! writing a pack and reaching the network. A read runs with
//! `GIT_NO_LAZY_FETCH=1`, so git answers that the object is missing instead
//! (decided by the user on 2026-10-02, keeping the 2.30 floor). Git older than
//! 2.44 ignores the variable: there, a read in a partial clone may still
//! lazy-fetch. Carrying no askpass token, it fails closed only where the
//! promisor needs a prompt; one a configured credential helper or the ssh
//! agent answers fetches. That is a constraint each read here designs around — a read that
//! may touch an object a partial clone lacks must treat both answers, the
//! object missing and the fetch that failed, as what they are — not one the
//! environment removes. Pinned against real git by
//! `a_read_in_a_partial_clone_does_not_fetch_a_missing_object`.
//!
//! A read can carry no askpass token — its invocation has nowhere to hold one —
//! so a read that reached a credential prompt fails closed rather than asking
//! the user. It parses only output `git` does not translate (`-z` records,
//! `--raw`, porcelain v2), and classifies a failure by exit status and the
//! repository's state, never by matching stderr, which is prose in the user's
//! language.
//!
//! Who pins what: the read's environment is built in `process/` and spelled
//! out by its tests; that only this module and `ops/` name the runner is
//! `the_runner_is_named_only_by_ops_and_reads`; that a read cannot build a
//! write is the compiler's, because only `ops/` can construct the
//! `WriteAuthority` a write needs. That each function here runs query
//! plumbing or `status` is a review obligation: a token scan cannot tell `diff-tree` from
//! `diff` in an argument list built at run time.
