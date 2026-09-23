# Cairn — design spine

Intent, not as-built. What exists today is in the root `CLAUDE.md` status
paragraph; what each system actually does, once it does anything, goes in
`docs/systems/`.

## What Cairn is

A native git client for people who already know git and want to *see* it. The
audience is the developer who reaches for Fork or Sourcetree not to avoid the
command line but because a graph, a diff, and a staging area are genuinely better
rendered than printed.

What that has to mean in practice:

1. **The history graph is readable at scale.** Not a list of commits with a
   decorative gutter — an actual graph that stays legible across a repository with
   many long-lived branches, and stays interactive while it loads.
2. **Staging is hunk-level and fast.** Selecting, splitting and discarding hunks
   is the operation a GUI wins at. It must be faster than `git add -p`, not just
   prettier.
3. **Destructive operations tell the truth.** Before a force push, a hard reset,
   a branch delete or a rebase, Cairn says what will be lost, how much, and
   whether it can be recovered — in that order, in words, before the button.
4. **Worktrees are first-class** (D8). No mainstream client serves them well, and
   anyone running several branches at once — or several agents — lives in them.

The full surface, tiered by risk and with the out-of-scope list, is
`docs/design/feature-inventory.md`. The interface itself — Fork's layout kept,
each deviation tied to a decision here — is `docs/design/ui.md`, with mockups in
`docs/design/mockups/`.

## What Cairn is not

- Not a git tutorial. It assumes the user knows what a rebase is.
- Not a platform client. Reviewing pull requests, reading issues and showing CI
  status stay out. *Opening* the right forge URL is in, and is not the same thing
  at all — D9 draws the line.
- Not cross-platform-first. Linux is the target; macOS follows where Freya makes
  it free. Windows is not a goal and no design should be compromised for it.
- Not an editor. Diffs are read-only, and conflict resolution is *structured*
  rather than free-text (D6).

## The bets

**Freya for the UI.** A Rust-native, Skia-backed, declarative toolkit: no web
runtime, no FFI layer between the view and the data, and a component model that
holds up as the UI grows. The cost is maturity — Freya 0.5 is a release candidate
whose API replaced the previous one wholesale. Accepted deliberately: the
alternative toolkits either bring a browser or bring C++.

**gitoxide for reads, the `git` binary for writes.** Decision D1. gitoxide is a
pure-Rust engine whose performance work aims squarely at large repositories, which is the case a GUI lives or dies on — and reads are
what a GUI does constantly. Writes go to `git` itself, because a mutation must
run the user's hooks, filters and credential helpers, and honour their config;
those are exactly the places where a reimplementation corrupts a repository
quietly. The seam does the rest of the work: the whole engine sits behind
`cairn-model` types, so swapping a backend for one operation is a change inside
`crates/cairn-git/`, invisible to every component. That is why the guard suite
defends the seam and not the backend choice.

**Speed is a claim, so it gets measured.** Choosing gitoxide does not deliver
speed; it makes it possible. Every read surface carries a stated bar against a
named real repository, with the numbers recorded — `docs/systems/history-graph.md`
shows the shape. An unmeasured performance claim decays invisibly until someone with a big repository finds it.

**The confirmation token as a type, not a convention.** `cairn_model::Confirmed`
exists because "remember to ask first" is exactly the kind of rule that holds for
a year and then quietly does not. Making it a value a destructive function
demands moves the rule from review to the compiler. Its limit is honest: the type
guarantees a prompt happened, not that the prompt was true — which is why
`destructive-ops-reviewer` exists and what it spends itself on.

## Decisions

Locked with the user. Each states the rule, the reason it beat the alternative,
and the cost accepted. Evidence, where a decision needed it, is in
`docs/research/`; how each is built today is in `docs/systems/`.

### D1 — gitoxide reads, `git` subprocess writes

Every repository read goes through `gix`. Every mutation goes through the `git`
binary, invoked from `crates/cairn-git/src/ops/`.

The deciding argument is hooks, not coverage. A client that does not run
`pre-commit` and `commit-msg` is broken for a large share of users, and gix runs
neither. The same reasoning covers clean/smudge filters (so Git LFS works),
`core.fsmonitor`, sparse checkout, submodule recursion, and index locking.

Coverage is a second, independent reason. Verified against the gix 0.87.1
source — method and queries in `docs/research/backend-split/gix-write-path-coverage.md`: gix has
commit creation, real three-way tree merge (`merge_trees`,
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
is strongest, so no backend spike is needed to validate it.

**The cost this decision creates.** Two implementations of git semantics live in
one application, and they can disagree. Three specific obligations follow, and
they are requirements rather than observations:

1. **Cache coherence is part of every mutation.** After a `git` subprocess writes,
   the gix handle may hold a stale index, stale refs or stale packs. Every
   operation in `ops/` states what it invalidates, and the worker boundary (D3)
   enforces it. Get this wrong and the UI shows the pre-write state, which reads
   to the user as "the operation failed".
2. **Filters affect reads, not only writes.** `.gitattributes` smudge filters mean
   the bytes in the object database are not what git would show. An LFS-tracked
   file read through gix without filter support renders as a pointer file rather
   than content. gix can do this — `gix-filter`, behind the `attributes` feature —
   but it is a thing to wire up and verify, not something D1 grants for free.
3. **Divergence on edge cases is a real defect class.** gix's status against git's
   under sparse checkout, `core.fsmonitor`, or unusual attribute configuration.
   The failure mode is that Cairn shows one answer and the user's next `git`
   command acts on another.

None of this reopens D1. It means the seam owes a cache-invalidation contract:
every operation in `ops/` reports what it invalidated (`Invalidated`, per flag,
on its `Performed`), documented in `crates/cairn-git/src/ops/mod.rs` and
summarised in `docs/systems/credentials.md`.

**A working-tree read runs the user's clean filter driver.** Converting a
working-tree file to git's form, which any diff of uncommitted work needs, runs
the clean filter driver that the path's attributes name and the user's config
defines — git-lfs, git-crypt, nbstripout — through gix, exactly as `git diff`
does. So a read never runs `git`, but one read can start a process: D1's "never
on a read path" means the CLI, not every process. Refusing to run the driver would
show those users a diff `git diff` does not, and would hand staging a patch built
from content their filter exists to change.

This answers the **clean** half of obligation 2 above and not the smudge half:
reads see everything in git's form, so an LFS-tracked file in a commit diff shows
as its pointer, modelled as a state to display rather than as content.

Three residuals of running someone else's program are stated rather than implied.
The driver runs with Cairn's own inherited environment plus the repository's
paths, because gix builds that process and not `ops::GitEnvironment`. Its stderr
is Cairn's, inherited. And `textconv` never runs on a read, so a file with a
textconv driver shows as binary where `git diff` shows text. Spec:
`docs/prd/diff-engine.md` R3; evidence: `docs/research/diff-engine/gix-diff-api.md`.

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

This makes "Cairn never handles a secret" nearly literal: the value exists
inside the helper process and on git's stdin, and passes through the application
exactly once — the dialog hands it to a worker thread, which writes it to the
helper's socket and drops it. It is never application state; the `Secret` type and
its guard are what keep that passage from becoming state. Spec:
`docs/prd/credential-prompts.md`. The environment roster, the threat model and what
a cancel can leave: `docs/systems/credentials.md`.

**Cairn's helper is the askpass while Cairn runs git.** A user who has set
`GIT_ASKPASS`, `SSH_ASKPASS` or `core.askPass` for something else finds it
replaced by Cairn's helper for the duration of a Cairn-run `git`: the environment
is built, never inherited, and a prompt must reach the running window rather than
a program with no window to reach. This is a decision, not collateral. What it
does not touch: `credential.helper` and the ssh-agent, which git consults before
it ever asks, so a setup that answers without prompting keeps answering. Rejected: honouring the
user's askpass over Cairn's dialog, because a program chosen for a terminal
may itself expect one, and a fetch that hangs on it is the failure this whole
decision exists to prevent. The way back in, when someone needs it, is an
"auth provider" setting that lets the user pick their own askpass program over
Cairn's dialog explicitly — issue #22 holds that setting.

### D3 — A few routed worker threads per repository

`gix::Repository` is not `Send`; gitoxide's model is a `ThreadSafeRepository`
(shared object database and pack indices) that each thread converts with
`.to_thread_local()` to get its own caches. So Cairn holds one
`ThreadSafeRepository` per open repository and a small number of worker threads,
each taking a thread-local handle once. Cache reuse where it is expensive,
no contention where it is not — the shape gitoxide was designed around. Rejected:
one thread per repository, which serialises every query behind the slowest, and
one `Repository` shared behind a lock, which gitoxide's design exists to avoid.

Work is routed to a thread by an explicit table, not handed to whichever thread is
free, because some work is pinned: a scroll keeps one gitoxide walk alive, that
walk borrows the repository and is not `Send`, so every page of it runs on the
thread that owns the handle. History has its thread; diffs — commit, comparison
and working-tree — have another, so a long history page never queues a diff
behind it.

Every query belongs to a **lane** — history, changes, file diff — and carries an
epoch numbered per lane, so a superseded query is abandoned rather than rendered.
A new query supersedes older ones in its own lane only, except that a changes
query also supersedes the file-diff lane; nothing else crosses. One counter for
everything would let a scroll cancel a selection. The epoch is the cancel signal
itself, not just a discard filter: superseding a query stops its walk. That is
the part that is painful to retrofit, and it is what `responsiveness-reviewer`'s
cancellation check exists to protect. An operation such as fetch carries no
epoch; it is cancelled by killing its process.

Every query is also assumed slow: a repository is somebody's 10-year monorepo, so
the view always has a loading state distinct from an empty answer. Spec:
`docs/prd/diff-engine.md` R4; as built: `docs/systems/history-graph.md`.

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

Precisely: the assigner owns a bounded window of rows so that a line to a
late-arriving parent has something to repaint, which makes retained state
proportional to that window times the lanes across it — and lane *width* on real
repositories is single digits. Spec: `docs/prd/history-graph.md` R1; evidence:
`docs/research/history-graph/scroll-memory-model.md`; as built:
`docs/systems/history-graph.md`.

It belongs in `cairn-git`, not `cairn-ui`: the lane is part of the answer, so it
is `cairn-model` vocabulary. A component that computed lanes would need the whole
history in memory, which is the failure this design exists to avoid.

gix offers no `--topo-order` equivalent. `Sorting` is `BreadthFirst`,
`ByCommitTime` or `ByCommitTimeCutoff`, and commit-time order can emit a parent
before its child under clock skew, which is common in rebased and imported
history. The assigner must therefore be correct under out-of-order arrival rather
than assume the walk guarantees child-before-parent; the window above is what lets
it repaint, and skew deeper than the window is a stated blind spot
(`docs/systems/history-graph.md`). Evidence: `docs/research/history-graph/gix-revwalk-ordering.md`.

### D5 — macOS is deferred, with two disciplines kept now

Linux is the target. macOS follows where Freya makes it cheap, and no Linux
design is compromised for it. Two habits keep it possible at near-zero cost:

1. Platform surface stays in `cairn-app`. `cairn-model` and `cairn-git` are
   portable; keep them that way.
2. Keyboard shortcuts resolve through one accelerator table mapping a logical
   action to a per-platform chord — never a literal `Ctrl` inside a component.

D1 and D2 already remove the credential-store question. The remaining unknown is
the menu bar, which is a Freya capability question to answer before promising
anything.

### D6 — Conflicts are resolved structurally, not textually

A three-way view with per-region *take ours / take theirs / take both*, plus "open
in your editor" as the escape hatch for the messy remainder.

Rejected: simply opening the user's editor. Fork headlines a built-in resolver,
and delegating is materially worse at the single most painful moment in git. The distinction that
makes this compatible with "not an editor": **structured resolution picks between
existing alternatives; an editor accepts arbitrary text.** Cairn does the first and
hands off the second.

Cost accepted: a conflict view is real work, and the escape hatch means the
external merge-tool integration (Tier 7) is a dependency rather than a nicety.
Rejected: full in-app editing — best possible UX, and an open-ended commitment to
becoming an editor.

### D7 — The first version worth having is the daily loop

The bar for "I would use this instead of Fork" is: graph, diff, stage by hunk and
line, commit, branch, and fetch/pull/push. Anything less is a viewer you would
leave every day, and a client that cannot push is not a client.

This sets the program: `docs/work/daily-loop/roadmap.md` sequences the packets
that reach it. Explicitly NOT in that first bar: rebase, interactive rebase,
conflict resolution, submodules, LFS. Those are the second lap.

Rejected: a read-only explorer first — it ships sooner and would validate the
gitoxide read bet with real numbers, but it replaces nothing, and the read bet
gets validated by the history graph anyway. Also rejected: read plus commit with no
remote, which defers the credential work into a milestone that still sends you
back to Fork daily.

### D8 — Worktrees are first-class

List, create, switch and remove worktrees, and show which worktree a branch is
checked out in. Cheap on the read side, unserved by every mainstream client, and
directly load-bearing for the way this repository is developed — `CLAUDE.md` puts
every packet in its own worktree.

The failure it prevents matters even at the read-only tier: checking out a branch
that is already checked out in another worktree. A client that does not know about
worktrees offers that operation and then fails confusingly.

Rejected: read-only awareness (avoids the worst failure, but leaves the workflow
unserved) and ignoring them (actively unhelpful for the intended user).

### D9 — Forge links are in scope; forge APIs are not

The line is mechanical rather than a judgement call: **anything that is "open the
correct forge URL" is in scope. Anything that needs an API token is out.**

In: create a pull request for the current branch against its upstream default,
open a commit / branch / tag / file in the browser, copy a permalink to a selected
line, open a compare view between two refs. All of it is one mechanism — read the
branch and its upstream, read the remote URL, identify the forge, construct a URL,
hand it to the system opener. No token, no network call from Cairn, no state to
keep in sync, and nothing to get out of date.

Out: reviewing pull requests, reading or filing issues, CI status, and creating
repositories on a platform.

A half-implemented forge panel is worse than no panel, but a link is not a panel.
"Create pull request on origin" is one of the user's most-used Fork context-menu
commands — which also means D7's milestone cannot be met without it.

**CI status deserves its own reason for staying out**, because it is the most
tempting thing on the far side of the line: it needs a per-forge API token, which
would make Cairn a credential holder, and D2's whole premise is that it never is.
The inconsistency is with a decision already taken, not merely with a scope
preference — so "the PR link worked out fine" is not an argument for it.

Cost accepted: a forge table that must be kept as DATA rather than code, so adding
a forge is an entry. Self-hosted GitLab, Gitea and Forgejo cannot be identified
from a hostname, so an unrecognised remote gets no menu item rather than a guessed
URL and a 404.

## Still open

- **Repository manager shape.** Tabs, a sidebar of repositories, or separate
  windows. Decides how much state is per-repository versus global, so it wants
  answering before a second repository can be open at once. Worker threads per
  repository with view settings app-wide fit all three shapes; that constrains
  the answer without giving it.
- **Interactive rebase.** The operation Fork is most valued for and the one with
  the largest UI surface. Its own program, not a packet.
- **Whether Cairn auto-stashes before destructive working-tree operations.** The
  reflog covers destroyed *commits*; nothing covers a discarded uncommitted edit,
  so for that class a confirmation dialog is the only barrier there is. An
  automatic stash would be a real differentiator and fits the `Confirmed` design.
  It must be decided before any discard operation ships, not inherited.
  Background: `docs/design/feature-inventory.md`, "Recovery".
- **Syntax highlighting in diffs**, and whether the diff view and a future
  conflict view share a component. The rest of how a diff renders is in
  `docs/design/ui.md`, "The detail pane and the diff".
- **Freya's menu bar story on both platforms** (see D5).
