# Progress — repository-tabs

Running log, newest first. Each phase's dismissed QA findings are logged here with their reasons,
and each [observe] amendment with the user's approval, for the final QA phase to audit.

## 2026-10-24 — planned

Planned with `/feature-plan`, from the user's ask to support several repositories as tabs, as
Fork does, after `fork-alignment` and before the rest of the roadmap.

Evidence in `docs/research/repository-tabs/`, from four recon agents over `feature/fork-alignment`
(b539d9d):
- `fork-tabs-evidence.md`: everything the Fork surveys recorded of its tabs, graded, and the 50
  questions no evidence answers;
- `window-state-audit.md`: every field of the window's state, per repository or per window;
- `worker-and-guards-audit.md`: what one open creates, what is process-wide, closing, idle cost,
  memory, and every guard row a split touches;
- `freya-tab-strip.md`: no tabs component; keys, unmounting and tasks; no hidden mounting worth
  having; `rfd` already in the lockfile; no file drop on native Wayland.

The Windows VM was not in the cloud session that planned this, so no Fork question was observed;
the user chose to observe in phase 01 (T4).

The user locked T1-T4 and T7-T14 from the options put to them, choosing the recommendation each
time but on the store's format (TOML through `serde`, over a plain-lines format of Cairn's own)
and the planning docs' base (straight to `main`). T5 and T6, and the planner's choices inside T3
and T12, are recorded as the planner's in `brainstorm.md`.

Written: `docs/prd/repository-tabs.md`, `docs/design/repository-tabs.md` (new), and the design
docs it touches (`cairn.md`'s map and "Still open", `ui.md`, `concurrency.md`, `credentials.md`,
`feature-inventory.md`); the daily-loop roadmap gained 5e and renumbered 5e-5h to 5f-5i.

A fresh coverage audit found 23 points, all fixed in the planning docs but one: C8 and C3 split
across the phases that can build them; the credential prompt drawn by its tab's holder, so its
`Replier` never leaves the hook; the window's root-scope hooks and every list's scroll made the
shown tab's; confirmations answered after a switch waiting on their tab (T11 into R9.3); each
tab's seeded diff context; the living docs fixed in the phase that makes them false, not at
teardown; the picker on the render side (`worker/` names no Freya); R1.1's questions widened and
every requirement they bear on settleable; a closed tab's failed write and its prompt; one root
opened once though the root is known only after discovery; an application stream for the zone
and the `git` found; Recent's folder checks on the store's thread; the roadmap's 5i, 8 and 5h
entries; design-doc tense; non-UTF-8 paths; code names; `DirBuilder` for the store's mode. Not
fixed, on purpose: `docs/prd/fork-alignment.md`'s Out of scope still sends tabs to
`repository-manager`; that PRD is stamped shipped on `feature/fork-alignment` (#154), a frozen
record, so editing `main`'s copy would only conflict with its stamp. Left to phase 04, for the
same reason: `docs/design/ui.md`'s closing-banner sentence, which #154 rewrites, gains the
repository's name there.
