As-built descriptions, one per system that exists; see docs/CLAUDE.md for the
contract. Each is kept current in the same change that invalidates it.

- `history-graph.md` — how the history view reads, lays out and draws a
  repository: the worker boundary, the history session, the lane assigner and
  the virtualized list.
- `credentials.md` — the `git` subprocess backend and its environment, the
  askpass helper and its channel, the secret type, fetch end to end, and the
  decisions the `credential-prompts` packet locked.
- `git-processes.md` — where every `git` process is built, the read/write seal,
  the environment each kind of invocation runs with, the runner, each
  repository's registry and command log, where a repository is refused at open,
  and, in the application, `git` found once, the network lane and closing.
- `diff.md` — how Cairn describes a change to a file: the one exact answer, the
  hunk, row and patch projections of it, presentation-independent line identity,
  the patch emitter with its reference applier, the engine queries that fill it
  from a repository (what a commit or two commits changed, one file's change, one
  path's working-tree diff), the diff thread and its lanes, the detail pane, the
  diff view, and the accelerator table.
- `refs.md` — how the engine reads a repository's refs: the refs snapshot and the
  five rules that make gix's answer git's, each branch's upstream, the stash list,
  ahead and behind, and the repositories refused at open because their refs are not
  files.
- `sidebar.md` — the window's sidebar: Local Changes and All Commits, the filter,
  the sections of refs and their folders laid out on a worker, and pressing an
  entry — its row selected, or found by paging the history's walk.
- `status.md` — the working tree's status as `git status` answers it: what it lists, the
  read that asks it, and the oracles that check it.
- `local-changes.md` — the Local Changes view: Unstaged and Staged laid out over a status on
  a worker, the filter, a path chosen and its working-tree diff asked and drawn for that path
  alone, and how the path chosen follows each refresh.
