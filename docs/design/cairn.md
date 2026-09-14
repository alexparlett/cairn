# Cairn — design spine

Intent, not as-built. What exists today is in the root `CLAUDE.md` status
paragraph; what each system actually does, once it does anything, goes in
`docs/systems/`.

## What Cairn is

A native git client for people who already know git and want to *see* it. The
audience is the developer who reaches for Fork or Sourcetree not to avoid the
command line but because a graph, a diff, and a staging area are genuinely better
rendered than printed.

Three things that has to mean in practice:

1. **The history graph is readable at scale.** Not a list of commits with a
   decorative gutter — an actual graph that stays legible across a repository with
   many long-lived branches, and stays interactive while it loads.
2. **Staging is hunk-level and fast.** Selecting, splitting and discarding hunks
   is the operation a GUI wins at. It must be faster than `git add -p`, not just
   prettier.
3. **Destructive operations tell the truth.** Before a force push, a hard reset,
   a branch delete or a rebase, Cairn says what will be lost, how much, and
   whether it can be recovered — in that order, in words, before the button.

## What Cairn is not

- Not a git tutorial. It assumes the user knows what a rebase is.
- Not a platform client. Pull requests, issues and CI live in a browser; a
  half-implemented GitHub panel is worse than a link.
- Not cross-platform-first. Linux is the target; macOS follows where Freya makes
  it free. Windows is not a goal and no design should be compromised for it.
- Not an editor. Diffs are read-only; conflict resolution opens the user's editor.

## The bets

**Freya for the UI.** A Rust-native, Skia-backed, declarative toolkit: no web
runtime, no FFI layer between the view and the data, and a component model that
holds up as the UI grows. The cost is maturity — Freya 0.5 is a release candidate
whose API replaced the previous one wholesale. Accepted deliberately: the
alternative toolkits either bring a browser or bring C++.

**gitoxide for reads, the `git` binary for writes.** Locked 2026-09-14 (see
Decisions). gitoxide is a pure-Rust engine whose performance work aims squarely
at large repositories, which is the case a GUI lives or dies on — and reads are
what a GUI does constantly. Writes go to `git` itself, because a mutation must
run the user's hooks, filters and credential helpers, and honour their config;
those are exactly the places where a reimplementation corrupts a repository
quietly. The seam does the rest of the work: the whole engine sits behind
`cairn-model` types, so swapping a backend for one operation is a change inside
`crates/cairn-git/`, invisible to every component. That is why the guard suite
defends the seam and not the backend choice.

**The confirmation token as a type, not a convention.** `cairn_model::Confirmed`
exists because "remember to ask first" is exactly the kind of rule that holds for
a year and then quietly does not. Making it a value a destructive function
demands moves the rule from review to the compiler. Its limit is honest: the type
guarantees a prompt happened, not that the prompt was true — which is why
`destructive-ops-reviewer` exists and what it spends itself on.

## Decisions

Locked 2026-09-14 with the user. Each states the rule, the reason it beat the
alternative, and the cost accepted. Evidence for the two that needed it is in
`docs/research/`.

### D1 — gitoxide reads, `git` subprocess writes

Every repository read goes through `gix`. Every mutation goes through the `git`
binary, invoked from `crates/cairn-git/src/ops/`.

The deciding argument is hooks, not coverage. A client that does not run
`pre-commit` and `commit-msg` is broken for a large share of users, and gix runs
neither. The same reasoning covers clean/smudge filters (so Git LFS works),
`core.fsmonitor`, sparse checkout, submodule recursion, and index locking.

Coverage is a second, independent reason. Verified against the linked gix 0.87.1
source — method and queries in
`docs/research/backend-split/gix-write-path-coverage.md`: gix has commit creation, real three-way tree merge (`merge_trees`,
`merge_commits`, `virtual_merge_base`), merge bases, tag, `delete_local_branches`,
index read and write, blame, status, dirwalk and fetch. It has no push — the
`gix::push` module is only `push.default` config parsing — no branch-switch
checkout (`gix_worktree_state::checkout` is reachable only from the clone path),
no reset, rebase, cherry-pick, revert or stash, and no hunk-staging helper.

Cost accepted: a process spawn per mutation (milliseconds, against a
user-initiated action), and parsing git's output — mitigated by preferring `-z`
and porcelain v2 formats, and by never using the CLI on a read path. Cairn must
locate `git` and check its version at startup, and fail loudly rather than
degrade silently.

Consequence worth naming: this narrows the gitoxide bet to reads, where gitoxide
is strongest, and removes the need for the backend spike this document previously
carried as an open question.

### D2 — Credentials are delegated to git entirely

Cairn stores no credential, integrates no keychain, and implements no auth. Since
fetch and push run through `git` (D1), git invokes the user's configured
`credential.helper` and their SSH agent, and Cairn inherits whatever already works
for them — including `osxkeychain` on macOS.

The part Cairn must build is the interactive case, because git will otherwise
prompt on a terminal Cairn does not have. Verified against git 2.55.0's
documentation: `GIT_ASKPASS` names a program git calls with the prompt as an
argument and reads the secret from its stdout, and `GIT_TERMINAL_PROMPT=0` stops
git falling back to a tty. So Cairn ships a small askpass helper binary that
round-trips the prompt to the running UI, and sets `SSH_ASKPASS` with
`SSH_ASKPASS_REQUIRE=force` for key passphrases.

This makes "Cairn never handles a secret" nearly literal: the value exists only
inside the helper process and on git's stdin, never in application state.
Packet: `docs/prd/credential-prompts.md`.

### D3 — A worker pool per repository, not a thread per repository

`gix::Repository` is not `Send`; gitoxide's model is a `ThreadSafeRepository`
(shared object database and pack indices) that each thread converts with
`.to_thread_local()` to get its own caches. So Cairn holds one
`ThreadSafeRepository` per open repository and a small pool of worker threads,
each taking a thread-local handle once. Cache reuse where it is expensive,
no contention where it is not — the shape gitoxide was designed around.

Every request carries an epoch so a superseded query can be abandoned rather than
rendered. That is the part that is painful to retrofit, and it is what
`responsiveness-reviewer`'s cancellation check exists to protect.

### D4 — Graph lanes are assigned incrementally, in the engine

Walk newest-first; keep a vector of active lanes, each holding the commit id it is
waiting for. For each commit, the leftmost lane waiting for it is its column;
that lane is then replaced by the commit's first parent, and additional parents
take new or joined lanes. Each row carries its lane plus the edge segments
crossing it.

Amortised constant work per commit, state proportional to the number of open
lanes rather than to history length, and — because the walk is newest-first —
appending more commits never renumbers a lane already emitted. That stability is
what lets rows stream into a virtualised list.

It belongs in `cairn-git`, not `cairn-ui`: the lane is part of the answer, so it
is `cairn-model` vocabulary. A component that computed lanes would need the whole
history in memory, which is the failure this design exists to avoid.

Open sub-problem carried into the packet, not hand-waved: gix offers no
`--topo-order` equivalent. `Sorting` is `BreadthFirst`, `ByCommitTime` or
`ByCommitTimeCutoff`, and commit-time order can emit a parent before its child
under clock skew, which is common in rebased and imported history. The assigner
must therefore be correct under out-of-order arrival rather than assume the walk
guarantees child-before-parent. Evidence and the candidate approaches:
`docs/research/history-graph/gix-revwalk-ordering.md`. Packet:
`docs/prd/history-graph.md`.

### D5 — macOS is deferred, with two disciplines kept now

Linux is the target. macOS follows where Freya makes it cheap, and no Linux
design is compromised for it. Two habits keep it possible at near-zero cost:

1. Platform surface stays in `cairn-app`. `cairn-model` and `cairn-git` are
   portable today; keep them that way.
2. Keyboard shortcuts resolve through one accelerator table mapping a logical
   action to a per-platform chord — never a literal `Ctrl` inside a component.

D1 and D2 already remove the credential-store question. The remaining unknown is
the menu bar, which is a Freya capability question to answer before promising
anything.

## Still open

- **Repository manager shape.** Tabs, a sidebar of repositories, or separate
  windows. Decides how much state is per-repository versus global, so it wants
  answering before the worker pool in D3 has more than one consumer.
- **Interactive rebase.** The operation Fork is most valued for and the one with
  the largest UI surface. Its own program, not a packet.
- **How diffs are rendered.** Syntax highlighting, word-level intra-line diff, and
  whether the diff view and a future conflict view share a component.
- **Freya's menu bar story on both platforms** (see D5).
