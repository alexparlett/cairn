# Brainstorm — fork-alignment

Locked decisions and the alternatives the user rejected, each with the evidence it
rests on. Planning session 2026-10-16. Evidence, all in `docs/research/ui-review/`:

| Short name | File |
| --- | --- |
| FS1 | `fork-window-survey-2026-10-16.md` — Fork for Windows 2.23.2, light theme, the whole window, every menu and context menu, Preferences, the start page, a narrow window |
| FS2 | `fork-window-survey-2-2026-10-16.md` — Fork Dark's sampled colours, toolbar dropdowns, Quick Launch, keyboard, double presses, equal timestamps |
| CS | `cairn-window-survey-2026-10-16.md` — Cairn's real window at 1024×768 over the same fixture |
| CI | `cairn-ui-as-built.md` — Cairn's UI as built, per surface, every string and constant |
| EV | `fork-evidence-inventory.md` — every Fork fact on record before this session, graded, and the ui.md claims it contradicts |
| FA | `freya-chrome-apis.md` — the Freya APIs the chrome needs, read in the pinned fork |
| FX | `fixture-recipe.md` — the repository both windows were compared over |

Both windows were compared over the fixture, at 1024×768. A side-by-side page of the
screenshots was shown to the user (a private artifact; the Fork screenshots stay in
`~/VMs/fork-win10/shots/`, `uv*` and `uw*`, and are not committed).

## Scope

**S1. Depth: layout, behaviour and density.** Regions, their contents and order,
interactions, wording, menus, and sizing — row heights, paddings, font sizes, icon style.
The palette changes only where the evidence argues (L3). *Rejected:* layout and behaviour
only (density stays); everything including Fork's theme wholesale.

**S2. The chrome's shell now, its actions later.** The toolbar, the menu bar and full
context menus are built where Fork has them; operations that do not exist yet arrive with
the packets that build them (L2). *Rejected:* align the existing surfaces only; build the
operations too (which would absorb remote-sync, branch-ops and stash-and-ignore into a
program).

**S3. Order: next.** Before remote-sync, so later packets put their buttons and menu items
into chrome that already matches Fork. *Rejected:* after D7, which builds more UI to be
reworked.

**S4. Its own packets, each later:** a file tree (L7), Quick Launch, Preferences with the
settings store, repository tabs with the Repository Manager (L8). The user: "New packets for
each please". Each is a brief in the daily-loop roadmap.

**S5. Topological order is its own packet, run first.** With equal commit times Cairn draws
a parent above its child (CS (a), the dashed edges); `git log --graph` and Fork never do
(FS2 §6, uw59-uw65, with Fork's "Sort commits: By date" set). That is a git-parity bug in
the engine's walk, not a UI question, and its fix needs research (git's incremental
topo-order over generation numbers, whether gix's commit-graph reading serves it) and bench
measurements, since a page must stay O(page). *Rejected:* after fork-alignment; a phase
inside this packet, which would mix engine and UI review in one packet. fork-alignment's
base is `main` once that packet has merged.

## Locked decisions

**L1. Fork's caption row on Linux.** Cairn draws its own caption row, as Fork for Windows
does: the menus at the left, the title centred, the window's controls at the right, one row
(FS1 §1: ~26 px). Mechanism (FA §5): `with_decorations(false)`, `window_drag()`, Freya's
`TitlebarButton` (`titlebar` feature, no new crate) and `BorderlessPlugin` for the resize
edges (a new crate, L10), `show_window_menu` for the window menu. Cairn then owns dragging, a double press to
maximize, the resize edges and the window menu, and on KDE and tiling compositors replaces
the frame the user chose. macOS keeps its native frame, with the same menus drawn as a row
inside the window (L10: no native menu bar). *Rejected (the recommendation):* the OS
frame kept with the menus in a row of their own, compositor-neutral, one row taller.

**L2. Unbuilt actions.** The toolbar draws every button a roadmap packet or brief owns, in
Fork's order, greyed until it works, its tooltip naming the feature it waits for; the menu
bar and the context menus list only what works, in Fork's order and groups, so a later
packet inserts its items into a group that already exists. What no packet plans is left out
everywhere: Git Flow, Git LFS, Bisect, Statistics, Treemap, Performance Benchmark,
Workspaces (Fork's Work ⌄), Accounts, Configure SSH Keys, Fork's feedback smiley, Fork
Activation and Fork's social links. (Check for Updates and Open in / Console are planned by
`feature-inventory.md` Tier 7 but owned by no packet, so they are absent too, by the same
rule; Open in and Console need a design under the process guards first.) *Rejected:* grey everywhere (a branch
row's menu would be ten grey items of thirteen); working items only everywhere (the toolbar
shifts as packets land).

**L3. Palette: Fork Dark's tones and selection, Cairn's lanes.** The evidence the user's
"unless the evidence argues" asked for: Cairn's selection is Freya's #2d2d2d on #141414
(CI, CS (b)), so a selected row barely shows and the focused list never shows, where Fork's
selection is #236BD2 with white bold text while its list has focus and #525252 when not
(FS2 §1a, uw3, uw4) — behaviour, not decoration. Taken from Fork Dark (FS2 §1): the
neutrals (caption, menu bar and toolbar #1A1A1A, sidebar #2C2C2D, history #212121, detail
#333333, its tab strip #2B2B2B, rules #525252), text #EAE8E8 with secondary tones, the
focused and unfocused selection, menus (#2C2C2C, hover #424242, separators #525252), dialogs
(#2B2B2B, a 1 px accent border, the focused field #0078D7), the diff's bands (added #3A5C3F,
removed #633F3E, a changed word #9F4247) and the tag's and stash's chips (filled and bordered
by kind). Kept: Cairn's eight Okabe-Ito lane colours, which stay distinguishable under
colour-vision deficiencies, where Fork's are orange and yellow; a branch's and a remote's chip
take their row's lane, as Fork's do (FS2 §6: the tint follows the lane, not the branch). A light theme waits on the
Preferences packet (a persisted choice; winit cannot read the desktop's preference on
Wayland or X11, FA §5). *Rejected:* Fork Dark exactly, lanes included; the selection alone.

**L4. The system's sans at Fork's sizes, and Lucide.** No UI font is embedded: Fork uses the
platform's UI font (Segoe UI, ~12 px, FS1 §2, §4), Freya cannot ask the desktop for its
font and falls back to Ubuntu, Adwaita Sans or Noto Sans (FA §9), and that fallback is the
nearest thing to the platform's font. Every text size comes from one metrics module, 12 px
body as in Fork, so no component falls back to Freya's 16 px. IBM Plex Mono stays for ids,
paths and diffs. ui.md's promise of IBM Plex Sans goes. Icons are Lucide (ISC), compiled by
Freya's `icons-lucide` feature from the `freya-icons` crate of the same pinned Freya
checkout (FA §4) — a new crate in the lockfile, approved in L10 — pinned in `deny.toml`, every
icon named through one module. *Rejected:*
embedding IBM Plex Sans; hand-painted canvas icons.

**L5. Local time through jiff.** Fork shows "16 Oct 2026 12:12", local, with no header
(FS1 §4); once the history's header goes, Cairn's UTC dates would be unlabelled. `jiff`
0.2.37 is already compiled into Cairn through `gix-date`, with `tz-system` and the zoneinfo
database enabled (`cargo tree -e features -i jiff`), so local time adds no crate to
`Cargo.lock`, only direct-dependency rows. The zone is read once off the UI thread and
handed to the render layer as a value; `cairn-ui` never reads the system's zone itself (it
never touches the filesystem). Closes #103, the activity popover's times included.
*Rejected:* UTC with a header to say so.

**L6. Three earlier deviations reversed, on first-hand evidence.**
- *Show Lost Commits* leaves the history's header (which goes, FS1 §4: Fork draws none) for
  Fork's View › "Show Lost Commits (Reflog)" and Ctrl+Shift+. (FS1 §9); the deviation's
  reason, that the reflog must be visible to be usable, is met by the menu item and a strip
  naming the mode while it is on.
- *Stage All* moves to Fork for Windows' bar across both lists (FS1 §7, uv12: filter, ⌄⌄
  Stage All, side-by-side, View Mode). A visible Unstage All stays on the Staged heading,
  where Fork shows it only in the menu: the deviation's reason, every action reachable
  (`platform.md`), still holds. EV flag 13: ui.md described Fork's chevron as in the
  Unstaged heading, which it is not.
- *A stash's row* is drawn on its base commit's lane, above it, as Fork for Windows draws
  it (FS1 §4, uv7), instead of on a short lane of its own: the lane assigner treats a stash
  as a child whose one parent is its base. The walk is unchanged.

**L7. No file tree in this packet.** Fork's Changes tab and Local Changes default to a tree
(View Mode: Tree, List, Combined List; FS2 §2). A tree changes what a selection is, the
selection rule Local Changes runs after every action and refresh (`next_selection`), the
drag between the lists and the virtualized lists' rows; it is its own packet. The View Mode
button is drawn, its menu offering List alone, enabled. *Rejected:* a tree in this packet,
the default, about two more phases.

**L8. Tabs, the Repository Manager, Preferences and Quick Launch are packets of their own**
(S4). In this packet the File and Window menus offer only what works; the toolbar's Quick
Launch and Appearance are greyed under L2, owned by those briefs.

**L9. Ctrl+Shift+D hides the commit details on Linux.** Fork for Windows' View menu binds it to
"Hide Commit Details", and pressing it collapses the pane (FS1 §9 uv25, FS2 §3). Cairn bound it
to Discard on Linux beside Backspace and Delete; the accelerator table allows no chord twice, so
one must give it up. Discard keeps Backspace and Delete on Linux, the key Fork's own file menu
shows ("Discard changes..." Delete, FS1 §8); macOS keeps ⇧⌘D for discard. *Rejected:* Discard
keeps it and Hide Commit Details has no chord; observing Fork's Local Changes first. On macOS
`ToggleDetails` has no chord until Fork for macOS is observed (⌥⌘D is the Dock's).

**L10. Three dependency decisions the plan had understated** (the coverage audit found them;
FA §1, §4, §5):
- `freya-icons`, the crate `icons-lucide` compiles in from the pinned Freya source, is
  approved; `deny.toml` records that its icons are Lucide's under ISC, which cargo-deny reads
  as the crate's MIT. *Rejected:* inline SVG copies under a roster guard; hand-painted icons.
- `freya-borderless-plugin` (Freya's `borderless` feature) is approved for the caption row's
  resize edges. *Rejected:* Cairn drawing its own bands over winit's `drag_resize_window`.
- No `muda`: macOS draws the menus as a row inside the window under its native frame, and the
  native menu bar is an issue for when macOS is worked on. The user first chose `muda` on
  every target, which makes the Linux build need GTK 3 and libxdo headers, then returned to
  this. *Rejected:* `muda` on every target; `muda` behind a macOS-only dependency table.

## Applied without a decision, because Fork's behaviour is now seen first-hand

The memory rule: where Fork's behaviour is known and Cairn's design does not argue against
it, apply it. Each is in the PRD with its evidence. The toolbar's order and its badges on
Pull and Push (FS1 §1-2); the status box in the toolbar's centre, the repository's name over
the branch, its glyph opening the activity popover (FS1 §2, FS2 §2); the sidebar's header
row with the repository's name and a gear, its icons, a 12 px indent per level, leaf names
in folders, Tags closed by default and newest first, a stash's message as git wrote it after
its `On <branch>: ` (FS1 §3); the history without a header, 23 px rows, initials avatars,
the rows `HEAD` cannot reach greyed (FS1 §4, #67); the history and the detail pane splitting
the height evenly and growing together, and Ctrl+Shift+D hiding the details (FS1 §1, FS2
§3); COMMITTER only where it differs from the author (FS1 §5); the diff bar's glyphs, order
and tooltips (FS1 §6); the Local Changes headings' bordered Stage and Unstage, the one
bordered commit field, Amend at the left, a split "Commit N Files ⌄" at the right whose menu
waits on push (FS1 §7, FS2 §2); menus with separators and right-aligned chords (FS1 §8,
FS2 §1d); Ctrl+1, Ctrl+2, Ctrl+0 and Ctrl+Shift+D (FS2 §3); New Branch's chord at `HEAD`,
which Cairn already does (FS2 §4 — CS (f1) took it for a bug).

Fork's bugs, not copied (FS1 §8, §11; FS2 §2, §6): Checkout… offered on the branch already
checked out; Fast-Forward offered while the branch is ahead; Interactively Rebase onto
itself enabled; the window clipping below ~600 px rather than reflowing (Cairn sets a
minimum size instead); the Activity Manager's clipped time; the Repository Manager's wrong
counts; stale tab names.

Cairn's own defects found here and fixed by this packet: the detail pane fixed at 260 px
with Expand All cut off (CS (b)); no key reaching Local Changes (CS (e)); Tab stops with no
visible focus (CS (f3)); the Commit tab's in-place diff at 24 px rows rather than the diff's
17 (CI); primary buttons filled in two of the four dialogs (CI); the engine error telling the
person to "reopen the repository", which the window cannot do (CI).
