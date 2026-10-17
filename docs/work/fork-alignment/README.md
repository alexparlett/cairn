# fork-alignment

Draws Cairn's window as Fork for Windows draws its own:
- the caption row and menu bar, the toolbar and its status box;
- the sidebar, the history, the detail pane and Local Changes at Fork's density, in Fork Dark's
  tones;
- every context menu, and Fork's keys.

It builds the chrome's shell now. Actions that do not exist yet are greyed in the toolbar and absent
from the menus, so the later packets only fill it in.

Spec: `docs/prd/fork-alignment.md` (the one copy of the acceptance criteria). Design:
`docs/design/ui.md`. Evidence: `docs/research/ui-review/`. Base: `main` once `history-topo-order`
(roadmap 5c) has merged. Integration branch: `feature/fork-alignment`.

| Phase | File | Builds | Criteria |
| --- | --- | --- | --- |
| 01 | `phase-01-style.md` | `cairn_ui::style`: metrics, Fork Dark's tones, Lucide icons, the launch theme, the focused and unfocused selection, and their guards | C1-C4 |
| 02 | `phase-02-caption-and-menus.md` | the menu component, the Linux caption row, the menu bar, macOS's menu row, zoom, About Cairn | C5, C6, C20 (About) |
| 03 | `phase-03-toolbar-and-regions.md` | the toolbar, the status box in it, the badges, the banners under it, the even split, hiding the details and Ctrl+Shift+D, the minimum size | C7-C9, C18 (Ctrl+Shift+D) |
| 04 | `phase-04-history.md` | no header, 23 px rows, avatars, local dates (jiff), `HEAD`'s reach greyed, a stash on its base's lane, Fork's chips and geometry, Show Lost Commits in View | C10-C13 |
| 05 | `phase-05-sidebar.md` | the sidebar's header, icons, indent, leaf names, disclosure, order and wording, and its ref menus | C14, C21 (sidebar) |
| 06 | `phase-06-detail-and-diff.md` | the tab strip, the Commit tab's author block, the summary's avatar, the diff bar and bands | C15 |
| 07 | `phase-07-local-changes.md` | the bar over both lists, the headings, the one-field commit box, the split Commit button | C16 |
| 08 | `phase-08-menus-keys-dialogs.md` | every context menu's items, Copy and Copy Full, the new chords, focus at every stop, the dialogs' look | C17-C21 |
| 09 | `phase-09-qa.md` | the merge bar: whole-packet `/qa`, every criterion, the bench measurements, teardown | C22 and all |
