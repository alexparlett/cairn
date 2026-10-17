# State — fork-alignment

The cross-session cheat sheet. Every session updates it before ending.

**Status: planned, 2026-10-16. Not started.** It waits on `history-topo-order` (roadmap 5c)
merging to `main`, which phase 04 relies on. Phases 01-03 don't touch the walk and could start
from `main` sooner, but only if the user says so.

## Locked decisions (brainstorm.md has the evidence and the rejections)

- **S1-S5.** The packet's shape:
  - depth: layout, behaviour and density;
  - the shell now, actions later;
  - it runs next, before remote-sync;
  - the file tree, Quick Launch, Preferences and the repository manager are packets of their own;
  - topological order is packet 5c, run first.
- **L1. Caption row.** Fork's caption row on Linux: client-side decorations with menus, title and
  controls. macOS keeps its native frame, the menus a row inside the window.
- **L2. Unbuilt actions.**
  - The toolbar greys a planned action, its tooltip "Comes with <feature>.".
  - Menus list only what works.
  - What no packet plans is absent everywhere.
- **L3. Palette.**
  - Fork Dark's tones and its focused (#236BD2) and unfocused (#525252) selection.
  - The lanes keep Cairn's Okabe-Ito eight.
- **L4. Font and icons.**
  - No embedded UI font: Freya's fallback sans at Fork's sizes, 12 px body.
  - Lucide through `icons-lucide`, pinned in `deny.toml`.
- **L5. Local time.**
  - Through jiff, already in `Cargo.lock` by way of gix-date.
  - The zone is read on a worker and handed to `cairn-ui`, which never reads it itself.
- **L6. Three deviations reversed.**
  - Show Lost Commits moves to the View menu, with a strip while it is on.
  - Stage All moves to the bar over both lists; a visible Unstage All stays on the Staged heading.
  - A stash is drawn on its base's lane.
- **L7. No tree.** View Mode offers List alone.
- **L8. Not in this packet:** tabs, the manager, Preferences and Quick Launch.
- **L9. Ctrl+Shift+D** is Hide Commit Details on Linux. Discard keeps Backspace and Delete there.
- **L10. Dependencies.** New crates `freya-icons` (Lucide, ISC recorded in `deny.toml`) and
  `freya-borderless-plugin`, both from the pinned Freya source. No `muda`: macOS draws the menus
  as a row inside the window.

## Interfaces introduced so far

None yet. Phases record here what they add:
- module paths;
- public types;
- new `cairn-model` vocabulary;
- accelerator actions;
- guard names;
- `deny.toml` pins.

## Validation status

| Phase | Status | Gate | QA |
| --- | --- | --- | --- |
| 01 | not started | — | — |
| 02 | not started | — | — |
| 03 | not started | — | — |
| 04 | not started | — | — |
| 05 | not started | — | — |
| 06 | not started | — | — |
| 07 | not started | — | — |
| 08 | not started | — | — |
| 09 | not started | — | — |
