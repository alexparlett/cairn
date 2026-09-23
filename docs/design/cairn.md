# Cairn — design spine

Intent, not as-built: the whole of Cairn as it is meant to be. What exists today
is in the root `CLAUDE.md` status paragraph, and how each system works once it
does is in `docs/systems/`. This file is the summary and the map; each feature's
design is its own document, listed below.

## What Cairn is

A native git client for people who already know git and want to *see* it. The
audience is the developer who reaches for Fork or Sourcetree not to avoid the
command line but because a graph, a diff and a staging area are genuinely better
rendered than printed.

What that has to mean in practice:

1. **The history graph is readable at scale** — an actual graph, legible across
   many long-lived branches, and interactive while it loads (`history-graph.md`).
2. **Staging is hunk- and line-level and fast.** Selecting, splitting and
   discarding hunks is the operation a GUI wins at; it must be faster than
   `git add -p`, not just prettier (`diff.md`).
3. **Destructive operations tell the truth.** Before a force push, a hard reset, a
   branch delete or a rebase, Cairn says what will be lost, how much, and whether
   it can be recovered — in that order, in words, before the button (`engine.md`,
   `ui.md`).
4. **Worktrees are first-class.** No mainstream client serves them well, and
   anyone running several branches at once lives in them (`worktrees.md`).

And under all of it, **speed is a claim, so it is measured.** Choosing gitoxide
makes speed possible, not certain. Every read surface carries a stated bar against
a named real repository, with the numbers recorded; an unmeasured performance
claim decays invisibly until someone with a big repository finds it.

## What Cairn is not

- Not a git tutorial. It assumes the user knows what a rebase is.
- Not a platform client. Reviewing pull requests, reading issues and CI status
  stay out; *opening* the right forge URL is in, and is not the same thing at all
  (`forge-links.md`).
- Not cross-platform-first. Linux is the target, macOS follows where the toolkit
  makes it cheap, and Windows is not a goal (`platform.md`).
- Not an editor. Diffs are read-only, and conflict resolution picks between
  alternatives rather than accepting free text (`conflicts.md`).

## How it is built

One hard seam. `cairn-git` answers questions and performs operations;
`cairn-ui` renders answers; `cairn-model` is the plain-data vocabulary between
them, and neither side leaks its own types across. Reads go through gitoxide,
writes through the `git` binary, and every destructive write demands a
confirmation token carrying the words the user saw (`engine.md`). Cairn holds no
credential; git's helpers do, and prompts reach the window through Cairn's own
askpass helper (`credentials.md`). The UI thread never waits on a repository:
work runs on a few routed worker threads per repository, and a superseded query
is cancelled, not rendered (`concurrency.md`).

## The first version worth having

The bar for "I would use this instead of Fork" is the daily loop: graph, diff,
stage by hunk and line, commit, branch, and fetch, pull and push — plus creating a
pull request after a push (`forge-links.md`). Anything less is a viewer you would
leave every day, and a client that cannot push is not a client. Rebase,
interactive rebase, conflict resolution, submodules and LFS are the second lap.

Rejected: a read-only explorer first — it ships sooner and would validate the
gitoxide read bet with real numbers, but it replaces nothing, and the history
graph validates the read bet anyway. Also rejected: read plus commit with no
remote, which defers the credential work into a milestone that still sends you
back to Fork daily. The build order is `docs/work/daily-loop/roadmap.md`.

## The map

| Document | What it designs |
| --- | --- |
| `engine.md` | Reads through gitoxide, writes through `git`, keeping the two in agreement, the confirmation seal |
| `credentials.md` | No stored credential; prompts through Cairn's askpass helper; where a secret may exist |
| `concurrency.md` | Worker threads per repository, lanes, epochs and cancellation |
| `history-graph.md` | The graph view, incremental lane assignment, the held walk |
| `diff.md` | The patch-capable diff model, the detail pane, the diff view |
| `conflicts.md` | Structured three-way resolution |
| `worktrees.md` | Worktrees as a first-class surface |
| `forge-links.md` | Forge URLs in, forge APIs out |
| `platform.md` | Linux first, the macOS disciplines, the toolkit |
| `ui.md` | The layout, kept from Fork, and every deviation from it |
| `feature-inventory.md` | The whole feature surface, tiered by risk, and what is out of scope |

**Decision index.** The decisions locked with the user, which other documents
cite by number; each lives in the document that designs it.

| Id | Decision | Document |
| --- | --- | --- |
| D1 | gitoxide reads, `git` subprocess writes | `engine.md` |
| D2 | Credentials are delegated to git entirely | `credentials.md` |
| D3 | A few routed worker threads per repository | `concurrency.md` |
| D4 | Graph lanes are assigned incrementally, in the engine | `history-graph.md` |
| D5 | Linux first; macOS kept reachable by two disciplines | `platform.md` |
| D6 | Conflicts are resolved structurally, not textually | `conflicts.md` |
| D7 | The first version worth having is the daily loop | this file |
| D8 | Worktrees are first-class | `worktrees.md` |
| D9 | Forge links are in scope; forge APIs are not | `forge-links.md` |

## Still open

- **Repository manager shape.** Tabs, a sidebar of repositories, or separate
  windows. It decides how much state is per-repository versus global, so it wants
  answering before a second repository can be open at once. Worker threads per
  repository with view settings app-wide fit all three shapes.
- **Interactive rebase.** The operation Fork is most valued for and the one with
  the largest UI surface: its own program, not a packet.
- **Whether Cairn auto-stashes before destructive working-tree operations.** The
  reflog covers destroyed *commits*; nothing covers a discarded uncommitted edit,
  so for that class a confirmation dialog is the only barrier. An automatic stash
  would be a real differentiator and fits the confirmation seal. It must be
  decided before any discard operation ships (`feature-inventory.md`, "Recovery").
- **Syntax highlighting in diffs**, and whether the diff and conflict views share
  a component (`diff.md`).
- **The menu bar on both platforms** (`platform.md`).
- **A light theme** (`ui.md`).
