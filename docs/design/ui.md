# UI design

Intent, not as-built. The mockups this document describes are
`docs/design/mockups/cairn-ui.html` — five screens, example data, dark-first
because the application is. Nothing here exists in code; the root `CLAUDE.md`
status paragraph is the authority on what does.

Modelled on Fork, deliberately: it is the client the user reaches for, and its
layout is the thing worth keeping. What changes is driven by decisions already
taken in `cairn.md`, not by taste — every deviation below names its decision.

## The layout model, kept from Fork

Three regions, always:

```
┌──────────────────────────────────────────────────────────────┐
│ toolbar: Fetch · Pull · Push · Stash │ repo · branch │ actions │
├───────────┬──────────────────────────────────────────────────┤
│ sidebar   │ main: history graph  OR  local changes           │
│ refs      ├──────────────────────────────────────────────────┤
│           │ detail: commit / changes / file tree  OR  diff   │
└───────────┴──────────────────────────────────────────────────┘
```

Kept, and why:

- **"Local Changes (N)" is the first sidebar entry**, with its count. It is the
  daily entry point; the number is the first thing a person wants to know.
- **The graph is the main view** when a repository opens (PRD `history-graph`,
  product rules). Columns: lanes, subject with ref badges, author, short id, date.
- **Staging is one screen**: unstaged and staged trees, the diff of the selected
  file, and the commit box, without navigating away. Hunk actions live on the
  hunk header, in the diff.
- **Stashes appear inline in the commit list** at the commit they sit on, rather
  than in a separate panel. Fork's feature list calls this out for a reason.
- **Repository tabs** across the top. The manager behind them is parked
  (`cairn.md`, "Still open"); tabs are the least-committing surface to show.
- **A detail pane with Commit / Changes / File Tree tabs** below the graph.

## What changes, and which decision drives it

| Change | Decision |
| --- | --- |
| The destructive-operation dialog says what is lost, how much, and whether it is recoverable — in that order — and its text IS the `Confirmed` prompt. Fork's dialogs are generic. | Spine pillar 3; the `Confirmed` seal |
| An **operation log** drawer, quoting the prompt the user acknowledged for each destructive operation. Fork has nothing like it. | `ops::Performed` |
| The discard dialog offers **stash first** as an option — surfacing the open auto-stash question as UI rather than deciding it silently. | `cairn.md` "Still open"; `daily-loop` O3 |
| **Worktrees are a sidebar section**, and a branch checked out in another worktree carries a chip and a disabled checkout. Fork has no worktree UI. | D8 |
| Branch and commit context menus carry **forge links**: create pull request, open on the forge, copy permalink. After a push, the toast offers *Create pull request* directly. | D9 |
| **Conflict resolution is three-way and region-level**: ours / result / theirs, with *use ours / use theirs / use both* per conflict region, and *edit in your editor* as the escape hatch. Fork resolves per file and opens a resolver for the rest. | D6 |
| The toolbar drops Fork's Appearance / Workspace / Feedback and adds *Open in terminal / editor* and a command palette. | Tier 7 of the inventory |
| No Accounts section in any sidebar. | D2 — Cairn holds no credentials |
| A visible **loading state** distinct from an empty repository. | PRD `history-graph` R4.3 |
| Linux-native chrome: no traffic lights, client-side decorations with controls at the right, keyboard-first throughout. | D5 |

## Palette and type

Dark-first, because the application launches with Freya's `dark_theme`. Cool
graphite neutrals with a slight blue bias, so the ground reads as chosen rather
than default. One accent — a desaturated steel blue — for HEAD, selection and the
primary action, chosen because red and green are already spoken for by diffs and
must stay semantic. Lane colours are a six-step set that stays distinguishable
under common colour-vision deficiencies, and lane identity never depends on
colour alone: the column position carries it (`history-graph` product rules).

Type is IBM Plex — Sans for the interface, Mono for ids, paths and diffs. One
family, so the UI and its data read as one instrument. `tabular-nums` wherever
digits align.

Light theme is not designed yet. It is a real obligation, not a toggle: a diff
palette that works on a dark ground does not survive inversion. Open item.

## The screens

1. **History** — the default view. Graph with a feature branch off `main`, a
   stash inline, ref badges, the detail pane. The example data is Cairn's own
   log plus two invented feature commits, and is marked as such.
2. **Local changes** — the staging screen. Unified diff with hunk-header actions
   and a line-selection gutter, so hunk *and* line staging are visible as one
   mechanism. The commit box with subject, body and amend.
3. **Discard** — the destructive dialog over the history view, with the
   operation log drawer open beneath it. This is the screen that is Cairn's,
   not Fork's.
4. **Conflict** — the three-way resolver, mid-merge, one of two conflicts.
5. **Worktrees and forge links** — the sidebar section, a branch context menu
   with the forge items, a branch held by another worktree, and the post-push
   toast.

## Open

- Light theme (above).
- The menu bar on both platforms — a Freya capability question (D5).
- Whether the detail pane sits below the graph (Fork) or to its right. Below
  wins on a laptop; right wins on a wide monitor. Probably a user preference,
  which means a layout the components must not assume.
- Side-by-side diff, which Fork offers. The `diff-engine` packet's O1.
