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
//! **Plumbing, or `git status`, and nothing else.** A read runs with
//! `GIT_OPTIONAL_LOCKS=0`, so that looking at a repository never refreshes its
//! index behind the user's back or holds `index.lock` while their own
//! `git commit` needs it. But only `status` honours that variable: porcelain
//! `git diff` against the working tree refreshes the index whenever
//! `diff.autoRefreshIndex` finds stat-only changes, and `git describe --dirty`
//! refreshes it too, each taking `index.lock` to write it whatever the
//! variable says. Plumbing — `diff-tree`, `diff-index`, `diff-files`,
//! `ls-files`, `rev-parse`, `cat-file` — never writes the index. So a read
//! that wants a diff runs `diff-tree` or `diff-index`, never `diff`; and a
//! refresh, if one is ever wanted, is a write, built in `ops/`. Evidence:
//! `docs/research/process-manager/platform-and-git-behaviour.md`, C3.
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
//! `WriteAuthority` a write needs. That each function here runs plumbing or
//! `status` is a review obligation: a token scan cannot tell `diff-tree` from
//! `diff` in an argument list built at run time.
