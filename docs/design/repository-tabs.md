# Repository tabs

Intent, not as-built. Spine: `docs/design/cairn.md`. The window chrome around the strip is
`ui.md`; the threads behind each tab are `concurrency.md`. Evidence:
`docs/research/repository-tabs/` (Fork's tabs as recorded, the window's state and the worker as
they stood before tabs, and Freya's parts for a strip).

## One window, several repositories

Cairn opens several repositories in one window, one tab each, as Fork does: a strip of tabs under
the toolbar, the window's whole body below it showing the active tab's repository. A person who
works across a service, its client and a shared library switches between them in one place, and
every tab keeps its own history, selection, Local Changes and commit draft while another is shown.

Rejected: a window per repository. Freya can open several windows, but on Linux they share no
parent, each would draw its own caption row, menus and toolbar, and Fork does not work this way.
Also rejected: a sidebar of repositories, which takes width from the refs sidebar every view needs.

## What a tab owns and what the window owns

The rule: **the layout and the diff's toggles are the window's; what a person is looking at is the
tab's.**

| The window's, one value shared by every tab | Each tab's own |
| --- | --- |
| The splitters: the sidebar's width, the history and detail split, the Changes tab's list width, Local Changes' list width and its Unstaged and Staged split | All Commits or Local Changes in the main region |
| Whether the sidebar is shown, and Hide Commit Details | The history's selection, its compared pair, its scroll and cursor |
| The diff's toggles: side by side, ignore whitespace, entire file, and the context once the person has changed it | The detail pane's tab, Commit or Changes |
| Zoom, the theme, the time zone, the `git` About names | The sidebar's filter, disclosure and the ref being found |
| Create Branch's "Check out after create" | Show Lost Commits |
| The held keys a press is resolved against | The Changes tab's filter, the diff shown, and the scroll of every list the tab draws |
| The one modal state | Local Changes' filter, selection and lists, and the commit box's draft and Amend |
| | The operations it ran and runs: its writes, its fetch, its activity entries, the endings waiting to be shown |

The diff's context starts, in each tab, at that repository's own `diff.context`, which git reads
per repository; once the person changes the context in any tab, the chosen value is the window's
and every tab draws it. A toggle changed in one tab redraws that tab's diff at once and every
other tab's when it is next shown. Show Lost Commits is the tab's, not the window's, because
turning it on reopens that repository's walk: a window-wide toggle would reopen every
repository's history at once.

Fork's vendor notes call its diff context and its file list's View Mode global
(`docs/research/repository-tabs/fork-tabs-evidence.md` §12).

## A tab not shown

Every tab keeps its repository open — its worker threads, its `SharedRepository`, its held walk —
and its state as plain values the window holds, and only the active tab is drawn. Switching back is
instant: nothing is read again but what a refresh reads. A tab drawn by no component runs none of
the view's timers and hears none of the window's keys; its update stream is still applied to its
state as answers arrive.

Rejected: closing a background tab's repository and reopening it on return, which costs a
second-long reopen on a large repository every switch, unlike Fork. Rejected: keeping every tab's
view mounted and hidden, since a hidden view is still visited and laid out each frame and still
hears the window's global key events (`freya-tab-strip.md` §2). Nothing is evicted from a
background tab: what a history keeps once scrolled is issue #4's, for every tab alike.

Each tab costs seven idle threads, blocked on their queues, and nothing else while nothing happens:
Cairn watches no file and polls nothing (`worker-and-guards-audit.md` §4).

When the window gains focus, the active tab refreshes everything a refresh reads (`concurrency.md`),
and every other tab reads its working tree's status alone, on its own refresh thread, so its tab's
mark of uncommitted changes is current. A tab made active refreshes everything.

## The strip

The strip is a row under the toolbar and above the banners, one tab per open repository and the
Repository Manager when one is open, as Fork's (`fork-tabs-evidence.md` §1):

- tabs share the strip's width equally, a new tab narrowing the others, and a "+" at its right end
  opens a new tab;
- the active tab is drawn in the surface tone of the sidebar under it, the others in the strip's
  darker tone;
- each tab carries a mark at its left and its name centred;
- a tab is pressed to show it; its ×, drawn on the active tab and on the tab under the pointer,
  closes it, and so does a middle press;
- a tab is dragged along the strip to reorder it, and its context menu offers Close Tab and Close
  Other Tabs;
- keyboard Tab never reaches the strip: tabs are switched by Ctrl+Tab and Ctrl+Shift+Tab, in the
  strip's order, wrapping at its ends.

**A tab's name** is its repository's folder. When two open tabs share a name, each is named
with the folder that holds it in parentheses — `t (uirev)` and `t (uirev2)` — and, when those
match too, with as many folders up as tell them apart. Names are worked out again on every open and
every close, so a tab left alone under its name goes back to the plain folder; Fork keeps the
qualified name until it is relaunched, which Cairn does not copy. The status box, the sidebar's
header and the desktop title use the tab's name.

**A tab's mark** says whether its working tree has an uncommitted change: drawn when its status
lists one, absent when the tree is clean. A tab with a dialog waiting to be shown (below) carries a
different shape in its place, so the two are told apart by shape, never by colour alone.

**When the tabs no longer fit** at the strip's minimum tab width, the strip ends in a button
listing every tab by its whole name; a tab is never scrolled out of reach and its name is never
cut without the whole name one press away.

Wherever Fork's own strip does otherwise than this section, the difference is a row of `ui.md`'s
deviations; Fork's strip as recorded is `fork-tabs-evidence.md`.

## The Repository Manager tab

New Tab — File › New Tab, Ctrl+T, or the "+" — opens a Repository Manager tab at the end of the
strip, Fork's: its left a heading and the Recent list, the repositories Cairn opened most recently
first; its right, for the one selected, its name, its path and an Open button. Open, a double press
or Enter on a row opens that repository in place of the Manager tab, or shows its tab when it is
already open. A recent repository whose folder is gone or no longer a repository says so on its
row, its Open greyed with the reason, and its menu removes it from Recent.

While the Manager tab is shown, the menu bar holds File, Window and Help alone, the toolbar's
repository buttons are greyed, and the status box says no repository is open. The Manager holds
one Open Repository… button besides its list.

Outside this design: Fork's Repositories list — the repositories under its Preferences' source folder —
its summary card of counts, remotes, branches and tags, Clone and Init. Each is a feature of its
own (`feature-inventory.md`, Tiers 5 and 6).

## Opening

A repository opens in a tab of its own from:

- the command line: each path named opens a tab, the last one named shown;
- File › Open Repository…, Ctrl+O, which asks the desktop's folder picker for a folder;
- the Repository Manager's Recent list;
- Restore Closed Tab (below).

A path inside a repository opens the repository that contains it, as `git` finds it. Opening a
repository already open shows its tab: one tab per repository root, so one local write lane writes
each index. The root is known once the new tab's repository has been found, so a tab that turns
out to hold a root already open closes before it is drawn, and of two opens of one root at once
the second to be found goes. Two worktrees of one repository are two roots and two tabs; their write lanes are one
lane, keyed by the common directory they share (`concurrency.md`).

A repository that cannot be opened — gone, not a repository, refused by git's ownership check, a
ref storage Cairn refuses — opens its tab anyway, showing why in the tab's body, as a single
failed open does, with Close. The folder picker is the desktop's own, through the XDG portal on
Linux and the native panel on macOS, so it needs nothing per platform of Cairn's (D5,
`platform.md`).

## Closing

Close Tab — File › Close Tab, Ctrl+W, the tab's ×, a middle press — closes the active tab's
repository and shows the tab its left neighbour, or its right when it had none. The tab leaves at
once. A write running in it finishes, never killed, on that repository's own threads, as closing
the window's repository always has (`concurrency.md`); the repository's other work is ended as a
window's close ends it, a credential prompt it had open is refused, and nothing the tab held is
freed on the UI thread. Should that write fail, its Git Error has no tab to wait on, so it comes
forward over the tab shown, titled with the closed repository's name, with Close alone.

Closing the last repository leaves the window open on a Repository Manager tab. File › Exit, or the
window's own close, closes every tab's repository: the window goes once each has ended, waiting on
the writes still running, and the closing line under the toolbar names the repository and the write
it waits on.

Restore Closed Tab — File › Restore Closed Tab, Ctrl+Shift+T — opens the most recently closed
repository again, at the place its tab had, as a fresh open: its view starts where a new tab
starts. The window remembers the repositories closed in this session, newest first.

## Dialogs and background tabs

The window keeps one modal state: one dialog drawn, the keys inert beneath it (`ui.md`, "Messages").
Across tabs:

- **A credential prompt comes forward over whatever tab is shown**, titled with the name of the
  tab whose `git` asks, since that `git` is blocked on the answer. Two tabs' prompts are shown one
  after the other.
- **Every other dialog waits on its own tab** — a Git Error, a "Couldn't <name>", a confirmation
  whose answer arrived after the person left the tab, as `Remove index.lock…`'s does — and the tab
  carries its waiting mark until it is shown, when the dialog opens. A dialog that opens at a press
  opens on the tab pressed, which is the one shown.
- The status box and the activity popover are the shown tab's: its running operation, its entries.
  A background tab's operation runs on and ends into its own tab's state.

## Remembering the tabs

Cairn remembers the tabs open when it quits, in their order, and which was shown, and opens them
again at the next launch; and it keeps the Recent list, capped, newest first. Paths on the command
line open after the remembered tabs, each switching to its tab when already open; with no path
and nothing remembered, the working directory's repository opens if it is in one, and otherwise a
Repository Manager tab.

They are kept in one file of Cairn's own state, outside every repository, under
`$XDG_STATE_HOME/cairn/` (`~/.local/state/cairn/` when unset), in TOML. It is read once as the
application starts and written whole each time the tabs or Recent change, to a temporary file
renamed into its place, so a crash leaves the old file or the new one, never half of either. A
file that is missing, unreadable or not one Cairn wrote opens no tab and is never fatal; the next
write replaces it. The file holds paths alone, never anything read from a repository; a path
that is not valid UTF-8 cannot be written in TOML, so its tab works but is not remembered.

Two Cairn processes each keep their own tabs, and the one that writes last decides what the next
launch opens. A second launch handing its paths to the running window is a feature of its own.

Preferences — the diff's settings across sessions, the splitters, the theme — are a separate
store of their own, designed with Preferences; tabs and Recent are state, not settings.
