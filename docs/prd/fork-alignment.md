---
status: in-flight
packet: fork-alignment
opened: 2026-10-16
---

# PRD — Fork alignment

Cairn's window drawn as Fork for Windows draws its own: the caption row with the menus, the
toolbar with the status box in its middle, the sidebar, the history, the detail pane and Local
Changes at Fork's density, in Fork Dark's tones, with Fork's menus and context menus and Fork's
keys. The chrome's shell is built now. Actions that do not exist yet are greyed in the toolbar and
absent from the menus until the packets that build them land.

The decisions and their rejected alternatives are in `docs/work/fork-alignment/brainstorm.md`
(S1-S5, L1-L10). The evidence is in `docs/research/ui-review/`, cited here by the brainstorm's short
names:
- FS1 and FS2: the two Fork for Windows 2.23.2 surveys, every fact VERIFIED against a screenshot;
- CS: Cairn's window survey;
- CI: Cairn's UI as built;
- EV: the earlier evidence and the ui.md claims it contradicts;
- FA: the Freya APIs.

The design is `docs/design/ui.md`, which this packet's planning rewrote to describe the end state.

**Depends on:** `history-topo-order` (the daily-loop roadmap's 5c), merged to `main`. The history's
greyed rows (R5.6) and its stash lane (R5.7) are marked during a walk that emits every child before
its parents.

## What this packet delivers

- One style module: every size, tone and icon of the chrome.
- Fork's caption row and menu bar on Linux; on macOS the native frame over the same menus as a
  row inside the window.
- Fork's toolbar, with the status box in it.
- The window's regions sized and split as Fork's.
- The history, the sidebar, the detail pane, the diff bar and Local Changes at Fork's density,
  wording and order.
- Every context menu, with Fork's groups, separators and chord text.
- Fork's View chords, with focus visible at every stop.
- The dialogs in Fork Dark's look, and About Cairn.
- Dates in local time.

## Requirements

### R1 — One style module (L3, L4)

1. `cairn_ui::style` holds every size, tone and icon the chrome draws:
   - `style/metrics.rs`: type sizes, row heights, paddings, the sidebar's width and the bars'
     heights;
   - `style/tones.rs`: the colour tokens;
   - `style/icons.rs`: every icon.

   The diff's colours stay in `diff/palette.rs` and the lanes' in `history/lane_palette.rs`, each
   reading its tones from `style::tones` where Fork Dark gives one.
2. The metrics start at Fork's, measured at 1024×768 (FS1):

   | Metric | Value |
   | --- | --- |
   | Body text | 12 px |
   | Toolbar labels | 11 px |
   | Caption row | 26 px |
   | Toolbar | 41 px |
   | History row | 23 px |
   | Sidebar row | 22 px |
   | Local Changes row | 20 px |
   | Menu item | 23 px |
   | Sidebar default width | 300 px |
   | Indent per sidebar level | 12 px |
   | Diff row (Cairn's measured choice, kept) | 17 px |

   No component falls back to Freya's 16 px default: menus, buttons and tooltips are given the body
   size.
3. The tones are Fork Dark's (FS2 §1, the colour table in FS2).

   Surfaces:

   | Surface | Tone |
   | --- | --- |
   | Caption row, menu bar, toolbar | #1A1A1A |
   | Sidebar | #2C2C2D |
   | History | #212121 |
   | Detail pane | #333333 |
   | Detail tab strip, list headings | #2B2B2B |
   | Diff body | #282828 |
   | Rules and splitters | #525252 |

   Text:

   | Text | Tone |
   | --- | --- |
   | Body | #EAE8E8 |
   | Toolbar labels | #B2B2B2 |
   | Detail labels | #C4C0C0 |
   | Rows `HEAD` cannot reach | #A9A9A9 |
   | Disabled | #707070 on #343434 |

   Selection, menus and dialogs:

   | Element | Tone |
   | --- | --- |
   | Selection, its list focused | #236BD2, white bold text |
   | Selection, its list unfocused | #525252 |
   | Menu | #2C2C2C, a #525252 border |
   | Menu item, hovered | #424242 |
   | Menu separator | #525252 |
   | Menu chord text | #9A9797 |
   | Dialog | #2B2B2B, a 1 px #3BACED border |
   | Text field, focused | #0078D7 border |
   | Button | #373737, a #525252 border |

   Diff bands, and the chips Fork tones by kind:

   | Element | Tone |
   | --- | --- |
   | Added line | #3A5C3F |
   | Removed line | #633F3E |
   | Changed word | #9F4247 |
   | Hunk header text | #959595 |
   | Chip: tag | #3E3F4E, a #636281 border |
   | Chip: stash | #545454, a #929292 border |
   | Chip text | #F0F0F0 |

   The lanes keep Cairn's eight Okabe-Ito colours. A branch's and a remote's chip take their row's
   lane, as Fork's do (FS2 §6: the tint follows the lane, not the branch): bordered in the lane's
   colour and filled with a dark shade of it, as Fork fills its first lane's #5D3D14 and its
   second's #5B4C0E. A chip's icon segment is unfilled.
4. The theme the window launches with is built from Freya's `dark_theme` with these tones set
   (FA §8). No component reads Freya's stock tones for anything this table names.
5. Icons are Lucide, through Freya's `icons-lucide` feature (FA §4), named only in
   `style/icons.rs`. The feature compiles in one new crate, `freya-icons`, from the pinned Freya
   source (brainstorm L10). `deny.toml` pins the feature with the rest of `freya`'s, so turning on
   another icon set is a dependency decision, and records that the icons are Lucide's, under the
   ISC licence at `freya-icons/external/lucide/LICENSE`, which cargo-deny reads as the crate's
   MIT.
6. IBM Plex Mono stays the face for ids, paths and diffs. No UI font is embedded; the body face is
   Freya's fallback list.

### R2 — The caption row and the menu bar (L1, L2, L8)

1. **On Linux, Cairn draws its own caption row** (`with_decorations(false)`, FA §5), one row of
   26 px:
   - left: the menus;
   - centre: the title "Cairn";
   - right: minimize, maximize or restore, and close (Freya's `TitlebarButton`).
2. The caption row behaves as a frame does:
   - pressing and dragging its empty space moves the window (`window_drag`);
   - a double press maximizes or restores it;
   - a secondary press opens the compositor's window menu (`show_window_menu`);
   - every edge and corner of the window resizes it (`BorderlessPlugin`: Freya's `borderless`
     feature and its one new crate, `freya-borderless-plugin`, from the pinned Freya source,
     brainstorm L10).
3. **On macOS, the native frame**, with the same menus and items drawn as a row of their own at
   the top of the window. No caption row is drawn, and no native menu bar is attached: that needs
   `muda` (FA §1), which the user declined (brainstorm L10).
4. The OS-level window title is "<repository> — Cairn", for the taskbar and the window switcher.
5. **The menus**, in Fork's order with only what works (FS1 §9, L2), each chord read from the
   accelerator table (`accelerators::chord_name`):
   - **File**: "Exit".
   - **View**:
     - "Show Uncommitted Changes" Ctrl+1 and "Show All Commits" Ctrl+2;
     - "Hide Commit Details" Ctrl+Shift+D;
     - "Show HEAD" Ctrl+0;
     - "Show Lost Commits (Reflog)" Ctrl+Shift+., heard anywhere in the window.

     How Fork marks those two items while their state is on (a changed label, a tick) was not
     observed; phase 02 observes it with `/observe-fork` before building it.
   - **Repository**:
     - "Refresh" F5;
     - "Fetch '<remote>'" Ctrl+Shift+F, greyed while a fetch runs;
     - "New Branch…" Ctrl+Shift+B.
   - **Window**: "Zoom In" Ctrl+= and "Zoom Out" Ctrl+-. The zoom scales the whole window through
     Freya's scale factor, in steps, for the session.
   - **Help**: "About Cairn".

   The groups are separated as Fork separates them.
6. A menu opens on a press, moves along the bar while open on hover, and walks its items with
   ↑/↓. Return chooses an item, and Escape or a press outside closes the menu. An item a press
   could not act on is greyed, with its reason as its tooltip.

### R3 — The toolbar and the status box (L2)

1. **Under the caption row, the toolbar, 41 px.** Each button is an icon over its label, in Fork's
   order and groups (FS1 §1-2):
   - **sidebar toggle**: hides and shows the sidebar;
   - **Quick Launch**: greyed ("Comes with Quick Launch.");
   - **Fetch**: fetches the default remote at once, as today;
   - **Pull**: greyed ("Comes with pull and push.");
   - **Push**: greyed ("Comes with pull and push.");
   - **Stash** and its separate ⌄: greyed ("Comes with stashing.");
   - **the status box**;
   - **Branch**: New Branch… at `HEAD`. Its separate ⌄ opens a menu of "New Branch…" alone, since
     New Worktree… comes with worktrees;
   - **Appearance** with its ⌄: greyed ("Comes with Preferences.").

   Fork's Open in, Console and Work are not drawn: no packet plans them yet (L2).
2. A greyed button keeps its place, and its tooltip is the sentence above. An enabled button has
   Fork's tooltip where Fork shows one — its name, with its chord on a second line read from the
   accelerator table, as Fork's Fetch and Push have (FS1 §2) — and none where Fork showed none: the
   sidebar toggle, the status box and Branch.
3. **Ahead and behind are badges**, as Fork draws them (FS1 §2): the commits ahead on Push's icon,
   the commits behind on Pull's, each absent at zero, drawn on the greyed buttons too. A gone
   upstream reads "upstream gone" in the status box's second line.
4. **The status box** sits in the toolbar's middle, ~300 px at 1024 px wide, shrinking first as the
   window narrows:
   - its first line is the repository's name, bold;
   - its second line is the branch glyph with the current branch, a detached or unborn `HEAD`
     shown in git's words;
   - its list glyph at the left opens the activity popover (FS2 §2);
   - while an operation runs, the box shows it as it does today.

   The title row's commit count, "*" and counts are gone: the sidebar's "Local Changes (N)" carries
   the change count, as in Fork.

### R4 — The window's regions (L1, FS1 §1, FS2 §3)

1. The banners — the leftover lock, a close waiting on a write, and the Show Lost Commits strip
   (R5.9) — stand under the toolbar, across the whole width, above the sidebar and the main region,
   as Fork's notification banner does.
2. **The history and the detail pane share the main region's height as a proportion**, half each
   at first, both growing with the window; the splitter moves the proportion. Expand All is never
   cut off at any window height the minimum allows.
3. **Hiding the details:** Ctrl+Shift+D, the View menu, and the detail pane's own icon at the
   right of its tab strip collapse the pane to its tab strip, and bring it back. The icon is
   Fork's, tooltip "Hide commit details" (FS2 §2). It replaces the "Collapse" button. Fork's
   pop-out icon is not drawn.
4. **The window has a minimum size** at which every region keeps its content without clipping:
   the toolbar's buttons, the status box at its narrowest, the history's columns and the detail
   pane's bar. Phase 03 measures it and records it in `docs/systems/`. Fork clips instead below
   about 600 px (FS1 §11), and Cairn does not copy that.
5. As the window narrows, the history's subject column shrinks first. Author, id and date keep
   their widths, as in Fork (FS1 §11).

### R5 — The history (L5, L6, FS1 §4, FS2 §1, §6)

1. **No header row.** The list starts at its first row. Its columns are:
   - the graph;
   - the chips, then the subject;
   - the author, after an avatar;
   - the 7-character id;
   - the date.
2. **Rows are 23 px.** The focused and unfocused selection follow R1.3, the selected row's text
   white and bold while focused. `HEAD`'s row is bold, as today.
3. **Avatars are initials:** a rounded square of the author's initials, its colour chosen from the
   name out of a fixed set. Avatars appear in the history, the Commit tab and the Changes summary,
   and no network is called. `docs/design/diff.md`'s "no avatars" becomes "no network".
4. **Dates are local:** "16 Oct 2026 12:12" in the history (Fork's `d Mon yyyy HH:mm`) and "16 Oct
   2026 12:12:33 +01:00" in the Commit tab. The activity popover's start times are local too.
   - `jiff` is a direct dependency of `cairn-app` and of `cairn-ui` only. Each gets an allowlist
     row in `layer_dependencies_are_allowlisted` and a `deny.toml` feature pin; no crate is added
     to `Cargo.lock` (brainstorm L5).
   - The system's zone is read once on a worker (`jiff::tz::TimeZone::try_system`, which reports a
     zone it cannot read; `system` would fall back to UTC silently) and handed to the render layer
     as a value.
   - `cairn-ui` never reads the system's zone itself, held by
     `the_render_layer_is_handed_its_time_zone`.
   - An unreadable zone falls back to UTC and says so once, in the Commit tab's date: "(UTC)".
   - Closes #103.
5. **Chips as Fork's** (FS1 §4, FS2 §1a):
   - a rounded rectangle 17 px tall, toned as R1.3 says: a branch's and a remote's by its lane, a
     tag's and a stash's by kind;
   - a remote's or tag's chip opens with its icon segment;
   - the current branch is "✓ name", bold;
   - a branch level with its upstream is a cloud-only chip, then the branch's;
   - a stash is "stash@{n}" with git's message, "On main: WIP on parser";
   - `HEAD` has no chip;
   - a remote's chip comes before a tag's.
6. **Rows `HEAD` cannot reach are greyed** (FS1 §4; closes #67): their subject, author, id and date
   are in the unreachable tone. Their lanes, edges and chips keep their colours.
   - The walk marks each row as reachable from `HEAD` as it goes: `HEAD` is reachable, and so is
     each parent of a reachable row. The mark is final when a row is emitted, because the
     history-topo-order walk emits every child before its parents.
   - The mark is `cairn-model` vocabulary on the row, with tests in the same commit.
   - Show Lost Commits' dimming is unchanged.
7. **A stash's row is drawn on its base commit's lane**, above it, as Fork for Windows draws it
   (L6, FS1 §4 uv7):
   - `LaneAssigner::push_stash` gives a stash the lane a commit whose one parent is the base would
     take, leaving the base's lane waiting for the base;
   - a stash whose base's lane is taken by a later commit takes a new lane, as such a commit would;
   - drawn-lane tests pin both;
   - `docs/design/history-graph.md` is rewritten to say so.
8. **The graph's geometry follows Fork's:**
   - the lane pitch and node size measured from FS1's uv7;
   - joins between lanes drawn as rounded curves, not diagonals;
   - a merge's node a larger hollow circle.

   The drawn-lane twins hold the edges unchanged; only the geometry changes.
9. **Show Lost Commits** is the View menu's item and its chord (L6). While it is on:
   - a strip under the toolbar reads "Showing commits only the reflog keeps." with a "Hide Lost
     Commits" button;
   - the item is ticked.

   The history's header check box goes with the header.

### R6 — The sidebar (FS1 §3, FS2 §2)

1. 300 px by default, resizable as today.
2. **A header row** holds the repository's name, bold, and a gear at the right whose menu holds
   "Copy Repository Path" (FS2 §2). Rename Repository and its settings are not drawn (L2).
3. **"Local Changes (N)" and "All Commits"** open with Fork's icons. The selected one is a rounded
   grey band. Below them, the filter field, placeholder "Filter", with a magnifier. Fork's
   refs/search switch is not drawn: no packet plans commit search.
4. **Sections, in Fork's order and disclosure:**
   - Branches, Remotes, Tags and Stashes, each a bold heading with a chevron and no count;
   - folders and `origin` closed by default, Tags closed, the rest open.

   Submodules waits on #69, which lists them.
5. **Rows:**
   - each level indents 12 px;
   - a folder shows its leaf name only, its icon an open folder;
   - a branch has the branch glyph, which the current branch replaces with ✓, the name bold;
   - each remote is grouped under its name with a cloud glyph;
   - tags are listed newest first;
   - a stash reads its message after git's `On <branch>: ` ("WIP on parser"), with the archive
     glyph;
   - ahead is right-aligned "1↑", as Fork draws it; how Fork draws a behind count was not
     observed, and phase 05 observes it before building it.

   Fork's hover icons (pin, show only, hide) are not drawn: filtering is #2.
6. Each ref row's context menu holds its working items (R9.2).

### R7 — The detail pane and the diff (FS1 §5-6, FS2 §2)

1. **The tab strip:** Commit and Changes (File Tree is #31). At its right, the hide-details icon
   (R4.3).
2. **The Commit tab:**
   - the "AUTHOR" caps label;
   - a 42 px avatar beside the name, the email and the date with seconds and its offset;
   - "COMMITTER" and its block only where the committer differs from the author. FS1 §5 saw only
     the agreeing case, with no block; phase 06 observes a differing committer with
     `/observe-fork` before building it, and which fields count as differing is that observation's;
   - REFS (absent when no ref names the commit), SHA (the full id) and PARENTS (links);
   - a rule, the message, a rule, Expand All and the files.

   A file opened in place draws its diff at the diff's row height (R1.2), not the list's.
3. **The Changes tab's summary line** opens with the author's avatar.
4. **The diff bar**, in Fork's order:
   - ^ "Go to previous change" and ⌄ "Go to next change", with their chords on the tooltip;
   - the path, centred, its folder in the secondary tone;
   - "Ignore whitespaces", "Decrease number of visible lines", "Increase number of visible lines",
     "Show entire file" and "Side by side diff".

   Each is a Lucide glyph with that tooltip, verbatim (FS1 §6). Fork's "Show hidden symbols" and
   "Word wrap" are not drawn (#33, #34).
5. **The diff's bands and hunk header** take R1.3's tones.

### R8 — Local Changes and the commit box (L6, L7, FS1 §7, FS2 §2)

1. **One bar across both lists' top**, as Fork for Windows' (L6):
   - the filter, a magnifier with no placeholder;
   - at its right, ⌄⌄ "Stage All";
   - the eye "Show diff side-by-side";
   - View Mode, whose menu offers List alone, enabled, until the file-tree packet (L7).
2. **"Unstaged" and "Staged" headings**, bold, each with its bordered "Stage" or "Unstage" button,
   greyed while nothing is selected in its list. The Staged heading keeps a visible ⌄⌄ "Unstage
   All", Cairn's deviation (L6). Each heading's own double chevron in Unstaged goes.
3. **The commit box** (FS1 §7, uv12c):
   - one bordered field of two parts: "Commit subject", with the Recent Commit Messages glyph at its
     right end, over a rule and "Description";
   - under it, the "Amend" check box at the left, and at the right a split button: "Commit N
     Files", or "Amend Last Commit" while Amend is ticked, with a separate ⌄;
   - the ⌄ is a greyed control, its tooltip "Comes with pull and push.": Fork's ⌄ holds Commit
     Options and Commit and Push (FS2 §2), and menus list only what works (L2), so it opens
     nothing until remote-sync gives it items;
   - the character counter appears only once the subject has text (FS2 §2).

   The rest of the box is unchanged.
4. Rows are 20 px. The badges, natural order, selection, drag and every staging route are
   unchanged (`docs/systems/local-changes.md`).

### R9 — Context menus (L2, FS1 §8)

1. **One menu component** for every context menu and dropdown (`cairn_ui::menu`), with:
   - groups separated by rules;
   - each item's chord right-aligned in the chord tone;
   - a greyed item's reason as its tooltip;
   - a submenu opened by → or a hover;
   - the first item focused on open, keys as R2.6.

   Each menu's items are data the component draws: no view builds its own menu rows.
2. **Each row's working items**, in Fork's groups and order:

   | Row | Items |
   | --- | --- |
   | Commit row | "New Branch…" Ctrl+Shift+B — "Copy Commit SHA" Ctrl+C · "Copy Commit Info" Ctrl+Shift+C |
   | Local branch, current or not | "New Branch…" Ctrl+Shift+B — "Copy Branch Name" Ctrl+C |
   | Remote branch | "New Branch…" Ctrl+Shift+B — "Copy Branch Name" Ctrl+C |
   | Tag | "New Branch…" Ctrl+Shift+B — "Copy Tag Name" Ctrl+C |
   | Unstaged file | as today, with Fork's groups and chords, plus "Copy Full Path" Ctrl+Shift+C beside "Copy Path" Ctrl+C |
   | Staged file | "Unstage" Enter — "Unstage All" — "Copy Path" Ctrl+C · "Copy Full Path" Ctrl+Shift+C |
   | A commit's file (Commit tab and Changes tab) | "Copy Path" Ctrl+C · "Copy Full Path" Ctrl+Shift+C |
   | The sidebar's gear | "Copy Repository Path" |

   A stash row has no menu until stashing gives it items. "Copy Commit Info" copies Fork's form:
   the short id and the subject. Phase 08 checks the form in the VM first and records it.
3. A right press selects the row it lands on, or keeps a selection it lands inside, as today. On a
   ref in the sidebar it also selects that ref's commit in the history, as Fork's does (FS1 §8).

### R10 — Keys and focus (FS2 §3)

1. **New actions in the accelerator table, Linux first:**
   - `ShowLocalChanges` Ctrl+1: Local Changes, focus in Unstaged;
   - `ShowAllCommits` Ctrl+2;
   - `ShowHead` Ctrl+0: select `HEAD`'s row and bring it into view;
   - `ToggleDetails` Ctrl+Shift+D;
   - `Fetch` Ctrl+Shift+F;
   - `Copy` Ctrl+C: copies what the focused view's first copy item names;
   - `CopyFull` Ctrl+Shift+C: its second;
   - `ZoomIn` Ctrl+=, `ZoomOut` Ctrl+-.

   Ctrl+Shift+D leaves Discard on Linux, which keeps Backspace and Delete (brainstorm L9). On
   macOS, discard keeps ⇧⌘D and `ToggleDetails` has no chord, the menu item alone, until Fork for
   macOS is observed (⌥⌘D is macOS's own Dock toggle). The other keys take ⌘ for Ctrl on macOS,
   marked in the table as unverified against Fork for macOS. `Copy`
   and `CopyFull` are one action each, interpreted by the focused view, so no chord is listed
   twice (the table's pin). The detail tabs keep their chords (#57).
2. **Every Tab stop shows focus:** a focused list by its selection turning from the unfocused to
   the focused tone, as Fork's does; every other control by a 1 px ring in the focused-field tone.
   No stop shows nothing (CS (f3)).
3. Local Changes is reachable from the keyboard (Ctrl+1, and the sidebar's rows by ↑/↓ and Return
   once the sidebar has focus).

### R11 — Dialogs and wording (FS2 §1e, CI)

1. Every dialog takes R1.3's look, and its primary button is filled. That covers the confirmation,
   the Git Error and "Couldn't <name>", Create Branch, the credential prompt and About Cairn. Today
   only two of the four existing dialogs fill it.
2. **About Cairn:** the name, the version, the licence and the git found at startup with its
   version; Close.
3. No message tells the person to do what the window cannot do: the engine error that says to
   "reopen the repository" is reworded to what the person can do.

### R12 — Measured and bounded

1. Every virtualization twin holds. The new lists are each rendered headlessly with a viewport's
   rows built, the same at the top and deep:
   - the menus with many items (the activity popover's operations are already held);
   - a sidebar of 50,000 refs at 22 px.
2. The window checks over the bench repository (`crates/cairn-app/src/window_check.rs`) show no
   frame over 16.7 ms when scrolling the history, opening a commit, or switching between All
   Commits and Local Changes. The numbers are recorded in
   `docs/research/ui-review/measured.md`.

## Product rules

1. Fork for Windows is the standard on Linux, and the observed behaviour is the evidence. A claim
   about Fork reaches this packet's code only when FS1, FS2 or an earlier first-hand observation
   records it. A behaviour not observed is observed with `/observe-fork` before it is built.
2. Every difference from Fork is a row of `docs/design/ui.md`'s "What changes, and why", with what
   drives it. Fork's bugs listed in the brainstorm are not copied.
3. What the shell draws for an action that does not exist follows L2 exactly: greyed in the
   toolbar with "Comes with <feature>.", absent from menus, absent everywhere if no packet plans
   it.
4. No layout change crosses the seam. Every new piece of information a view draws is a value the
   engine or the worker answers, never one a view reads for itself:
   - the reachable mark, `cairn-model` vocabulary on the row;
   - the zone, a `jiff` value the worker reads and the window hands down;
   - the repository path for Copy Full Path, which the window already holds.
5. Every size, tone and icon comes from `cairn_ui::style` (R1), held by
   `chrome_sizes_and_tones_come_from_the_style_module` and
   `every_icon_is_named_through_the_icon_module`.

## Acceptance criteria

The packet's one authoritative copy. Each is met by the named test, written by the phase in
brackets.

| # | Criterion | Decided by |
| --- | --- | --- |
| C1 | No render file outside `cairn_ui::style` (its exceptions roster aside) passes a literal to a font size or builds a colour from a literal. The matcher is proven on the shapes it claims. | `chrome_sizes_and_tones_come_from_the_style_module`, self-test `the_style_literal_matcher_catches_the_shapes_it_claims` [01] |
| C2 | Only `style/icons.rs` names Freya's icon module; `deny.toml` pins `freya`'s features exactly, `icons-lucide` and `borderless` among them, and records Lucide's ISC licence; the lockfile's new crates are `freya-icons` and `freya-borderless-plugin` alone. | `every_icon_is_named_through_the_icon_module` and its self-test; `gate.sh --step deps` [01] |
| C3 | The metrics and tones are the PRD's tables. | `the_style_module_holds_forks_metrics_and_fork_darks_tones` [01] |
| C4 | A selected row is the focused tone with white bold text while its list has focus, and the unfocused tone when not: the history, the sidebar, both Local Changes lists and the Changes tab's files. | `a_selection_is_blue_while_its_list_has_focus_and_grey_when_not` (cairn-ui tests) [01] |
| C5 | On Linux the window has no OS decorations and draws the caption row: menus, title, three controls. A drag on its empty space asks the window to move, and a double press toggles maximized. | `the_caption_row_draws_the_menus_the_title_and_the_controls`, `a_drag_on_the_caption_row_moves_the_window` [02] |
| C6 | The menus hold exactly R2.5's items in its order, each chord read from the accelerator table, and none an item that does nothing; they open, move and close by R2.6's keys and hover; the OS-level title names the repository; Zoom In and Out scale the window in steps. | `the_menu_bar_holds_only_what_works_in_forks_order`, `the_menus_walk_by_key_and_hover`, `the_window_title_names_the_repository`, `zoom_scales_the_window_in_steps` [02] |
| C7 | The toolbar holds R3.1's buttons in order. The greyed ones are disabled with their sentence as tooltip. The badges show ahead and behind and are absent at zero. | `the_toolbar_is_forks_with_the_unbuilt_buttons_greyed`, `ahead_and_behind_are_badges_on_push_and_pull` [03] |
| C8 | The status box draws the repository over the branch, opens the popover from its glyph, and shows a running operation as before. | `the_status_box_names_the_repository_over_the_branch` (and the existing `status_box` tests, kept) [03] |
| C9 | The history and the detail pane split the height by proportion and grow together. Expand All is visible at the minimum size and at 1024×1200. Ctrl+Shift+D hides and restores the pane. The banners stand under the toolbar, and the subject column shrinks first. | `the_detail_pane_grows_with_the_window`, `hiding_the_details_leaves_the_tab_strip`, `the_banners_stand_under_the_toolbar`, `the_subject_column_shrinks_first` (window tests) [03] |
| C10 | The history has no header, 23 px rows, avatars, Fork's chips (R5.5) and local dates in Fork's format. `cairn-ui` never reads the system's zone. Ctrl+0 selects `HEAD`'s row and brings it into view. | `a_history_row_is_forks`, `a_rows_chips_are_forks`, `the_render_layer_is_handed_its_time_zone`, `show_head_selects_heads_row` [04] |
| C11 | Rows `HEAD` cannot reach are marked by the walk and drawn greyed. The mark agrees with `git rev-list HEAD` on fixture repositories with a merge, a side branch and a stash. | `the_walk_marks_what_head_reaches_as_git_rev_list_does` (cairn-git, real git) [04] |
| C12 | A stash takes its base's lane when that lane is waiting for the base, and a new lane when it is not. The graph's joins are curves and a merge's node is the larger hollow circle (R5.8). | `a_stash_is_drawn_on_its_base_commits_lane` (`LaneAssigner`'s unit tests in cairn-model and `crates/cairn-ui/tests/drawn_lanes.rs`), the `graph_geometry` tests [04] |
| C13 | Show Lost Commits is the View item and its chord, the strip stands while it is on, and the header check box is gone. | `show_lost_commits_is_the_view_menus_and_names_itself_while_on` [04] |
| C14 | The sidebar's header, icons, indentation, leaf names, disclosure defaults, tag order and stash wording are Fork's. A right press on a ref selects its commit; the sidebar's rows walk by ↑/↓ and Return opens Local Changes or All Commits. | `the_sidebar_is_forks`, `a_right_press_on_a_ref_selects_its_commit`, `the_sidebar_walks_by_key_and_opens_with_return` (cairn-ui tests) [05] |
| C15 | The Commit tab shows COMMITTER only where it differs, as observed, and an opened file's diff at the diff's row height. The Changes summary opens with the avatar. The diff bar holds R7.4's buttons with their tooltips, verbatim. | `the_committer_is_shown_only_where_it_differs`, `the_changes_summary_opens_with_the_avatar`, `the_diff_bar_is_forks_with_its_tooltips` [06] |
| C16 | Local Changes' shared bar, headings, one-field commit box, split Commit button with its greyed ⌄, and 20 px rows are R8's. Every staging route still passes its existing tests. | `local_changes_bar_and_headings_are_forks`, `the_commit_box_is_one_field_and_a_split_button`; the `local_changes_actions` and `commit_box` suites [07] |
| C17 | Every context menu holds exactly R9.2's items, separated and with chords, through the one component, and Copy and Copy Full put the right text on the clipboard. | `every_context_menu_holds_its_rows_working_items`, `copy_and_copy_full_copy_what_the_focused_view_names` [08] |
| C18 | R10.1's actions resolve in their scopes; Ctrl+Shift+D is ToggleDetails on Linux and Discard is Backspace and Delete there. The table's pin still holds: no chord twice, bare keys only where admitted. | the accelerator table's unit tests and its pin [03, 08] |
| C19 | Every Tab stop shows focus. Ctrl+1 reaches Local Changes with focus in Unstaged. | `every_tab_stop_shows_focus`, `ctrl_1_opens_local_changes_with_unstaged_focused` [08] |
| C20 | Every dialog's primary button is filled and its look is R1.3's. About Cairn names the version and the git found. No message asks the person to reopen the repository. | `every_dialogs_primary_button_is_filled` (the `dialog_buttons` tests), `about_cairn_names_the_version_and_the_git`, `no_message_asks_to_reopen_the_repository` [02, 08] |
| C21 | The menus, and a sidebar of 50,000 refs at 22 px, build one viewport. The existing virtualization twins hold. | `a_long_menu_builds_one_viewport`, `a_sidebar_of_50000_refs_builds_one_viewport` (kept) [05, 08] |
| C22 | No frame over 16.7 ms on the bench in R12.2's three gestures, the numbers in `docs/research/ui-review/measured.md`. | the `#[ignore]`d window checks, read on the bench machine [09] |

## Out of scope

Each is a packet's, an issue's, or not planned:
- **Packets of their own** (daily-loop roadmap briefs, brainstorm S4):
  - the file tree and Combined List (`file-tree-view`);
  - Quick Launch (`quick-launch`);
  - Preferences and the settings store, the light theme, Appearance's theme and layout, and
    remembering the diff settings and the sidebar's disclosure (`preferences`, #89, #29, #72);
  - repository tabs, the Repository Manager, New Tab, Open, Clone and Init
    (`repository-manager`);
  - topological order (`history-topo-order`, which runs first).
- **Other packets' actions:** Pull, Push, Stash, Checkout, Merge, Rebase, Tag, Rename, Delete,
  Tracking, New Worktree, and Fork's Fetch dialog (#23).
- **Issues:** Collapse All Merges (#1); branch filter, pin, hide and show-only (#2); commit search;
  the File Tree tab (#31); Quick Look (#35); hidden symbols (#33); word wrap (#34); Reset File to
  (#100); Submodules (#69); the detail pane to the right (#30).
- **macOS:** the native menu bar (`muda`, declined; brainstorm L10), filed as an issue at
  teardown.
- **Not planned:** Open in, Console and opening files or folders, which start programs other than
  git and need a design under the process guards; gravatar; Git Flow; LFS; workspaces; the
  detached detail window; Fork's notification banner for `.gitignore`.
