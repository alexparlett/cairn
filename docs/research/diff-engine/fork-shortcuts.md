# Fork's keyboard shortcuts for the diff and commit-detail actions

Evidence record. Commissioned 2026-10-03 by the diff-engine packet before phase
06 made the diff actions live, because phase 05's accelerator table had invented
chords for actions Fork documents none for. Historical: never retro-edited.
Extends `fork-detail-and-diff-ui.md` (its Finding 18, Windows tab chords, stays
OPEN).

## Sources

- Vendor lists: `fork-dev/Docs` `keyboard-shortcuts-mac.md` and
  `keyboard-shortcuts-windows.md` (last docs commit 2023-10-04), matching the
  vendor's posts on Tracker #309 (Jun 2018) and TrackerWin #333 (Jun 2019), read
  in full.
- Release notes: every entry of git-fork.com/releasenotes (Mac, to 2.70, 4 Sep
  2026) and /releasenoteswin (Windows, to 2.23, 25 Sep 2026), searched for
  shortcut, ⌘, Ctrl, Alt, ⌥, keyboard, whitespace, side-by-side, context, entire
  file, next file, chunk.
- All fork.dev blog posts; ~20 issue searches per tracker plus Tracker #1127,
  #1850, #1249, #671 and TrackerWin #23, #2799, #2393, #930, #2833.
- Secondary: KeyCombiner (a copy of the vendor lists); terminal.guide
  (2026-01-20) is unreliable — it contradicts the vendor lists.

## Per action

| Action | Fork Mac | Fork Windows | Source | Confidence |
|---|---|---|---|---|
| Previous/next change (hunk) | ⌘↑/↓ | Ctrl+↑/↓ | Mac 2.67 release note (15 May 2026); vendor on Tracker #1850 (2026-05-15); Windows 2.20 (5 Jun 2026) | High |
| Commit / Changes tab | ⌘⌥1 / ⌘⌥2 (⌘⌥3 File Tree) | not found | vendor on Tracker #1127 (2020-09-08); Mac 1.0.80 (14 Jun 2019) | Mac high; Windows unestablished |
| Second commit into the selection | ⌘-click | Ctrl-click | vendor FAQ (`faq.md`); Tracker #33 (2017); Mac 1.0.37, Windows 1.20 | High |
| Previous/next file | none — plain ↑/↓ in the focused file list; Tab / Shift-Tab moves focus list↔details (2.67) | same; ↑/↓ also works with Space quick look open; Tab cycling (2.20) | absent from lists and notes; TrackerWin #333 comment (2020-05-15) | High that no chord exists |
| Toggle side-by-side | none (header button) | none | absent everywhere | High |
| Toggle ignore whitespace | none (button / context menu) | none | absent everywhere | High |
| More/fewer context lines | none | none | absent; never requested in the trackers | High |
| Entire file | none (button) | none | absent | High |
| Collapse the detail pane | ⌘D | Ctrl+Shift+D | Mac 2.66 (10 Apr 2026); Windows 2.19 (24 Apr 2026) | High |

## Conflicts and hazards

- Fork scopes chords to the focused view: Windows Ctrl+Shift+D both collapses
  the details and discards in the Changes view; Mac ⌘D both collapses and opens
  the external diff. Copying either needs view-scoped chords.
- Fork's stage-all is ⌘⌥⇧S / Ctrl+Alt+Shift+S — one stray Shift from an
  invented Ctrl+Alt+S side-by-side toggle, and it changes the index.
- Fork's author on Tracker #1249 (2021-01-19): on Windows, Alt combinations are
  for accelerators and system commands.
- AltGr: on Windows AltGr is Ctrl+Alt; TrackerWin #23 shows Fork's Ctrl+Alt+O
  swallowing Polish "ó" (fixed 1.30–1.31). In xkb `pl`, AltGr+E/S/I give ę/ś/→ —
  exactly Ctrl+Alt+E/S/I. In German layouts `[` and `]` are AltGr+8/9, so
  Ctrl+Alt+[ / ] are awkward or unreachable. Unverified: how Freya/winit report
  AltGr on Linux (xkb treats it as a separate level-3 modifier).
- GNOME (`org.gnome.desktop.wm.keybindings`, gsettings-desktop-schemas 50.1):
  Ctrl+Alt+arrows switch workspace (Shift moves the window); Ctrl+Alt+Tab/Esc;
  Ctrl+Alt+Delete, Ctrl+Alt+F1–F12. KDE KWin: Ctrl+Meta+arrows, Ctrl+F1–F4,
  Meta+Alt+arrows; Konsole Ctrl+Alt+T. None clashes with Alt+↑/↓, Ctrl+↑/↓,
  Ctrl+Alt+1/2, Ctrl+Alt+S/I/E or Ctrl+Alt+[ / ]. Ctrl+Alt+↑/↓ must never be used
  for file navigation.
- macOS (support.apple.com/en-us/102650): ⌥⌘I show/hide inspector (convention);
  ⌥⌘S Finder sidebar; ⌥⌘D Dock. ⌘↑/↓ is start/end of document in text — a diff
  binding must be scoped to diff focus so text editing keeps it (Ctrl+↑/↓ likewise
  on Linux).

## Recommendation as reported

Keep Fork's three bound actions (⌘↑/↓ / Ctrl+↑/↓ scoped to the diff; ⌘⌥1/2 and
Ctrl+Alt+1/2 — the Linux row derived from Fork Mac, Windows unknown — with ⌘⌥3
reserved for a File Tree tab; ⌘/Ctrl-click). Previous/next file: ↑/↓ in the
focused file list with Tab / Shift-Tab between list and diff, dropping
Alt+↑/↓. The four diff toggles: no chord, as Fork. Collapse: none.

## Unestablished

Fork Windows' tab chords; whether Fork's Quick Launch (⌘P / Ctrl+P) lists the
diff toggles; tooltips beyond previous/next change; Freya/winit's AltGr report on
Linux (test any Ctrl+Alt+letter chord with the `pl` layout before shipping one);
menu-only chords absent from docs and notes (judged unlikely).

## Decided from it (2026-10-03, by the user)

Follow Fork: keep the three Fork-bound actions, scope change navigation to diff
focus, drop the five invented chords — previous/next file by ↑/↓ in the focused
file list with Tab moving focus; no chord for side-by-side, ignore whitespace,
context ± or entire file; no collapse chord.
