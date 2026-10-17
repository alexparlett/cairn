# QA checklist — fork-alignment

Packet-specific checks beyond the repo-wide gate and `docs/qa-gate.md`. The acceptance criteria
are in `docs/prd/fork-alignment.md` (C1-C22), not copied here. Each phase runs the items its file
names; phase 09 runs them all.

1. **Fork evidence, cited.** Every string, order, size and tone a phase puts on screen as Fork's is
   in FS1 or FS2, or in an earlier first-hand file, and the phase's commit or `docs/systems/` text
   cites it. Something not observed was observed with `/observe-fork` before it was built, never
   taken from memory or a tracker. `docs/systems/` and the code are compared with the evidence,
   not with the PRD's paraphrase of it.
2. **L2, exactly.**
   - Every toolbar button a packet owns is drawn, greyed with its "Comes with <feature>." tooltip.
   - No menu or context menu holds an item that does nothing.
   - Nothing unplanned appears anywhere: the brainstorm's L2 list is absent from the code.
3. **Deviations recorded.** Every difference from Fork the phase leaves is a row of `ui.md`'s "What
   changes, and why". Fork's listed bugs are not copied.
4. **The seam.** A view draws nothing it reads for itself. The reachable mark, the zone and the
   repository path reach `cairn-ui` as values. `cairn-ui` gains no dependency but jiff and Freya's
   features. `cairn-app`'s render files name no waiting primitive.
5. **Style, one place.** No size, tone or icon is written outside `cairn_ui::style`, beyond the
   guard's exceptions roster. Each roster row's reason still holds. A component given no size does
   not fall back to Freya's 16 px: check the menus, tooltips and buttons by eye in the real window.
6. **Focus.** In the real window, every Tab stop of every view the phase touched shows where focus
   is. The focused and unfocused selection tones follow focus between the lists.
7. **The real window.** Run the release build over the fixture (`docs/research/ui-review/fixture-recipe.md`)
   at 1024×768 and at the minimum size. Compare each surface the phase touched with FS1 and FS2's
   screenshots (`~/VMs/fork-win10/shots/uv*`, `uw*`), and record what differs and why in the
   phase's progress entry. Drive it with `wtype` and Hyprland's `send_shortcut`, as CS did, and
   say what needed the pointer.
8. **Virtualization.** Every new or changed list still builds one viewport, the twins named in the
   root `CLAUDE.md` kept, and new ones written for new lists (menus, the sidebar at its new row
   height).
9. **No regression behind the look.** Every existing behavioural suite stays green:
   - staging routes and their selection;
   - the commit box;
   - Create Branch;
   - the confirmation seal;
   - Show Lost Commits;
   - the activity popover;
   - the status box.

   A restyle that changes a test's expectation changes it to Fork's evidence and says so.
