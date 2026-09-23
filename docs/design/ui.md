# UI design

Intent, not as-built. The mockups are `docs/design/mockups/cairn-ui.html` — five
screens, example data, dark-first because the application is. Where a mockup and
a design document differ, the document is the design: the detail pane and the
diff view in particular are designed in `diff.md`, and the mockup's fixed-height
pane, tab count, `+N` file counts, banded hunk header and checkbox column are not
part of it.

Modelled on Fork, deliberately: it is the client the user reaches for, and its
layout is the thing worth keeping. What changes is driven by the design, not by
taste — every deviation below names the document that drives it.

## The layout model, kept from Fork

Three regions, always:

```
┌──────────────────────────────────────────────────────────────┐
│ toolbar: Fetch · Pull · Push · Stash │ repo · branch │ actions │
├───────────┬──────────────────────────────────────────────────┤
│ sidebar   │ main: history graph  OR  local changes           │
│ refs      ├──────────────────────────────────────────────────┤
│           │ detail: commit / changes  OR  diff               │
└───────────┴──────────────────────────────────────────────────┘
```

Kept, and why:

- **"Local Changes (N)" is the first sidebar entry**, with its count. It is the
  daily entry point; the number is the first thing a person wants to know.
- **The graph is the main view** when a repository opens (`history-graph.md`).
- **Staging is one screen**: unstaged and staged trees, the diff of the selected
  file, and the commit box, without navigating away.
- **Stashes appear inline in the commit list** at the commit they sit on, rather
  than in a separate panel. Fork's feature list calls this out for a reason.
- **Repository tabs** across the top. The manager behind them is open
  (`cairn.md`, "Still open"); tabs are the least-committing surface to show.
- **A detail pane below the graph**, behind a draggable splitter, with Commit and
  Changes tabs (`diff.md`).

## What changes, and why

| Change | Driven by |
| --- | --- |
| The destructive-operation dialog says what is lost, how much, and whether it is recoverable — in that order — and its text IS the `Confirmed` prompt. Fork's dialogs are generic. | `cairn.md`, what Cairn is; `engine.md`, the confirmation seal |
| An **operation log** drawer, quoting the prompt the user acknowledged for each destructive operation. Fork has nothing like it. | `engine.md`, the confirmation seal |
| The discard dialog offers **stash first** as an option — surfacing the open auto-stash question as UI rather than deciding it silently. | `cairn.md`, "Still open" |
| **Worktrees are a sidebar section**, and a branch checked out in another worktree carries a chip and a disabled checkout. Fork has no worktree UI. | `worktrees.md` |
| Branch and commit context menus carry **forge links**: create pull request, open on the forge, copy permalink. After a push, the toast offers *Create pull request* directly. | `forge-links.md` |
| **Conflict resolution is three-way and region-level**, with *edit in your editor* as the escape hatch. Fork resolves per file and opens a resolver for the rest. | `conflicts.md` |
| The toolbar drops Fork's Appearance / Workspace / Feedback and adds *Open in terminal / editor* and a command palette. | `feature-inventory.md`, Tier 7 |
| No Accounts section in any sidebar. | `credentials.md` — Cairn holds no credential |
| A visible **loading state** distinct from an empty repository. | `concurrency.md` — every query is assumed slow |
| Linux-native chrome: no traffic lights, client-side decorations with controls at the right, keyboard-first throughout. | `platform.md` |

## Staging gestures

Fork puts no actions on the hunk header. It outlines a hovered chunk and floats
Stage and Discard over it; a drag-selection narrows them to the selected lines;
there is no checkbox column and no button on the `@@` row; and Discard appears on
unstaged chunks only, because Fork refuses to discard staged changes at all
(`docs/research/diff-engine/fork-detail-and-diff-ui.md`). The mockup instead shows
hunk-header actions with a line-selection gutter. Which of the two Cairn uses is
open (below); either way a gesture produces a selection, and the patch is built
from the exact diff, never the displayed one (`diff.md`).

## Palette and type

Dark-first, because the application launches with Freya's `dark_theme`. Cool
graphite neutrals with a slight blue bias, so the ground reads as chosen rather
than default. One accent — a desaturated steel blue — for HEAD, selection and the
primary action, chosen because red and green are already spoken for by diffs and
must stay semantic. Lane colours are a six-step set that stays distinguishable
under common colour-vision deficiencies, and lane identity never depends on
colour alone: the column position carries it.

Type is IBM Plex — Sans for the interface, Mono for ids, paths and diffs. One
family, so the UI and its data read as one instrument. `tabular-nums` wherever
digits align.

A light theme is a real obligation, not a toggle: a diff palette that works on a
dark ground does not survive inversion, so it needs its own design (below).

## The screens

1. **History** — the default view. Graph with a feature branch off `main`, a
   stash inline, ref badges, the detail pane. The example data is Cairn's own
   log plus two invented feature commits, and is marked as such.
2. **Local changes** — the staging screen: the diff with the mockup's candidate
   staging gesture, and the commit box with subject, body and amend.
3. **Discard** — the destructive dialog over the history view, with the
   operation log drawer open beneath it. This is the screen that is Cairn's,
   not Fork's.
4. **Conflict** — the three-way resolver, mid-merge, one of two conflicts.
5. **Worktrees and forge links** — the sidebar section, a branch context menu
   with the forge items, a branch held by another worktree, and the post-push
   toast.

## Open

- The staging gesture: Fork's hover outline with floating Stage and Discard, or
  hunk-header actions with a selection gutter. Decided before hunk staging is
  built.
- A light theme, with its own diff palette.
- The menu bar on both platforms (`platform.md`).
