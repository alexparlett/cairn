# Brainstorm — repository-tabs

The decisions locked with the user on 2026-10-24, each with the alternatives rejected and the
evidence behind it. Evidence: `docs/research/repository-tabs/` — FT (`fork-tabs-evidence.md`), WS
(`window-state-audit.md`), WG (`worker-and-guards-audit.md`), FR (`freya-tab-strip.md`).

The user's ask: "after the fork alignment we need to do one more thing before continuing, refactor
to support multiple repositories as different tabs just like fork does." So the packet runs next
after `fork-alignment` (5d), before the briefs that followed it and before 6, 7 and 8.

Decisions marked *(user)* were chosen by the user from options put to them; those marked
*(planner)* are defaults the planner set inside a user decision, stated here so the user can
revisit them, and every one Fork's evidence leaves open is an [observe] requirement in the PRD.

## T1. Scope: tabs, opening, closing and restoring, remembered across launches *(user)*

Several repositories behind Fork's tab strip; opened from the command line, File › Open
Repository… (Ctrl+O) and a Repository Manager tab (Recent and Open); New Tab and "+", Close Tab,
Restore Closed Tab, Ctrl+Tab. The first piece of a store on disk comes forward: the open tabs and
the Recent list, restored on relaunch, as Fork restores its tabs on every launch (FT §10).

*Rejected:*
- tabs with no persistence, every launch from the command line: unlike Fork, and the Manager
  would hold only this session's repositories;
- the whole 5h brief (Clone, Init, the Manager's summary card) in this packet: the largest
  packet, and none of it is needed to work across repositories;
- the refactor alone, the paths on the command line as tabs: no way to open or close a tab in the
  window.

The roadmap's `repository-manager` brief keeps Clone, Init, the summary card and the Repositories
list.

## T2. A tab not shown stays resident and is not mounted *(user)*

Every tab keeps its worker threads, its `SharedRepository` and its state as values the window
holds; only the active tab is drawn. Switching back is instant, as in Fork. Cost: seven idle
threads per tab (WG §1.2), and its loaded rows; nothing is evicted (issue #4 stays the eviction
story).

*Rejected:*
- closing a background tab's repository and reopening it on return: a second-long reopen on a
  large repository at every switch;
- resident with an eviction rule in this packet: eviction is #4's, for one tab as for ten;
- mounting every tab's view and hiding the inactive ones (not put to the user, ruled out by FR
  §2): a hidden view is still visited and laid out every frame, and still hears the window's
  global key and pointer events.

## T3. The layout and the diff's toggles are the window's; what a person looks at is the tab's *(user)*

Window-wide: the splitters, the sidebar shown and its width, Hide Commit Details, the diff's
toggles (the context still starting at each repository's `diff.context` until changed), zoom, the
theme. Per tab: the main view, selections, filters, the detail tab, Show Lost Commits, the commit
draft, scrolls. This matches the spine's "worker threads per repository with view settings
app-wide" and Fork's vendor notes (its context and its View Mode are global, FT §12). The full
field table is the PRD's R2.1, from WS §2.

*Planner's, inside it:*
- Show Lost Commits is per tab: toggling it reopens that repository's walk (WS §2.1, row 29), so a
  window-wide toggle would reopen every repository's history at once.
- Create Branch's "Check out after create" is the window's: a preference, not a view.
- The activity log is per tab, and the status box shows the shown tab's: it is the status box of
  that repository. Fork's Activity Manager scope is an [observe] question (FT §17 q46).

*Rejected:* everything per tab; leaving each ambiguous field open until Fork is observed.

## T4. Fork's unrecorded behaviour is observed first, in the packet's first phase *(user)*

The Windows VM is not in the cloud session that planned this. The architecture is locked now. The
first phase, run on the user's machine, opens with `/observe-fork` over FT §17's strip questions.
It may amend the PRD's [observe] requirements, with the user's approval, before anything is drawn.
Meanwhile each unrecorded point has a stated default in the PRD.

*Rejected:* deciding every gap now and never observing (twice in staging-and-commit a ratified
decision rested on a false Fork claim); pausing planning until it is observed.

## T5. The strip's defaults where Fork is unrecorded *(planner, [observe])*

- a × on the active tab and the hovered one, and a middle press closing a tab;
- dragging a tab along the strip reorders it;
- a tab's context menu: Close Tab and Close Other Tabs;
- equal widths down to a minimum, then a button listing every tab, so no tab is out of reach and
  no name is cut without its whole name a press away (`ui.md`: no text a person needs is cut). A
  horizontally scrolled strip is ruled out: `ScrollView` is barred on render paths by the
  virtualization guard (FR §1);
- the mark at a tab's left drawn when its status lists a change (untracked included, as Local
  Changes' count does), absent when clean. Fork's vendor notes say its marker ignores untracked
  files (FT §6), and every tab in the survey showed a grey dot of unknown meaning;
- a tab with a waiting dialog marked by another shape.

## T6. Tab names: Fork's parent folder in parentheses, worked out again on every change *(planner)*

From FT §5 (VERIFIED twice): two `t`s become `t (uirev)` and `t (uirev2)`. Cairn extends the rule
to as many folders up as tell them apart, and works names out again on every open and close, so a
tab left alone is plain again. Fork keeps the stale qualified name (FT §5), and fork-alignment's
brainstorm already listed "stale tab names" among the Fork bugs not to copy.

## T7. The folder picker is `rfd`, made direct *(user)*

`rfd` 0.17.2 is already in `Cargo.lock` through `freya-winit`'s crash dialog; a direct
dependency adds no package (FR §5, measured). On Linux it asks the XDG portal through `libdbus`,
loaded at run time, and falls back to running `zenity`. That process starts outside Cairn's
`process/` module and is not `git`: a review point, not a guard failure. It has macOS's native
panel too (D5). Its async call runs on a thread of its own; Cairn asks it from `worker/`.

*Rejected:* `ashpd` (one new package, Linux only, so macOS would need another route later); no
picker, a typed path (a folder dropped on the window would not work on native Wayland, FR §6).

## T8. Focus refreshes the shown tab whole and the others' status alone *(user)*

Each background tab reads its status on its own refresh thread, so its mark is current; Fork's
marker refreshes on focus (FT §6, vendor). A tab made active refreshes whole.

*Rejected:* only the active tab, background marks showing what they last knew; every tab
refreshing whole on every focus (ten history reopens and ten `git status`es per focus).

## T9. Closing the last tab leaves a Repository Manager tab; a closed tab's write finishes *(user)*

File › Exit or the window's close quits. A tab closed while its write runs leaves the strip at
once; its worker finishes the write, never killing it, as the window's close always has, and
quitting waits on it with the closing line naming the repository.

*Rejected:* closing the last tab quits Cairn.

## T10. Opening a repository already open shows its tab *(user)*

One tab per repository root keeps one local write lane per index. Two worktrees of one repository
are two roots and two tabs, each with its own lane, until the `worktrees` packet keys the lane by
the common directory (`concurrency.md` already designs it so). That is a stated residual in the
PRD's Out of scope.

*Rejected:* a second tab on the same repository, two write lanes on one index.

## T11. A credential prompt comes forward; every other dialog waits on its tab *(user)*

The prompt comes forward over any tab, titled with its repository's name, since its `git` is
blocked on the answer. A Git Error, a "Couldn't <name>" and a confirmation wait on their own tab,
which is marked until shown. One modal state stays the rule. Prompt ids become unique across the
application: today each acceptor counts from one (WG §2).

*Rejected:* every dialog coming forward and switching tabs; every dialog waiting, even a prompt,
which leaves a fetch hanging until the person looks at its tab.

## T12. The store: TOML through serde, under `$XDG_STATE_HOME/cairn/` *(user: the format; planner: the place)*

The user chose TOML through `serde` over Cairn's own line format and over JSON. `serde` and both
`toml` versions are already in `Cargo.lock` (`toml` 0.9 through `cairn-guards`, 1.1 as a build
dependency of Freya's skia bindings), so making them direct is an allowlist decision with no new
package; the `preferences` packet may take the same format. The place is the state directory, not
the configuration one: open tabs and Recent are state, not settings. Written whole, to a
temporary file renamed into place, on an application thread; a bad file opens nothing and is
never fatal. The store's writes are a `FILESYSTEM_MUTATION_EXCEPTIONS` row of their own.

*Rejected:* a plain-lines format of Cairn's own; `serde_json`.

## T13. The packet is `repository-tabs`, roadmap 5e *(user)*

It runs next after 5d. The briefs that followed 5d keep their names, renumbered: `file-tree-view`
5f, `quick-launch` 5g, `preferences` 5h, `repository-manager` 5i (slimmed to Clone, Init, the
Manager's summary card and its Repositories list).

*Rejected:* reusing 5h `repository-manager` moved forward, with its remainder split out.

## T14. The planning docs land on `main` directly *(user)*

They are planning docs alone; they go to `main` through their own pull request, beside
fork-alignment's still-open packet pull request (#154). The packet's integration branch is cut
from `main` once #154 has merged, since every file the PRD names is as `feature/fork-alignment`
has it.
