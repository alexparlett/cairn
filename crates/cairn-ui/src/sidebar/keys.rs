//! The sidebar's keys while it has focus, Fork's (`docs/research/ui-review/fork-sidebar-2026-10-20.md`
//! §4): ↑ and ↓ walk every row of the list — each heading, folder, ref and stash — and stop at
//! its ends, never reaching the filter, All Commits or Local Changes; a ref or a stash walked to
//! is chosen at once, as a press chooses it, its commit selected in the history, while a heading
//! or a folder walked to only holds the walk. ← and → are a tree's: ← closes an open heading or
//! folder and otherwise goes to the row it stands in; → opens a closed one, and on an open one
//! goes to its first row. Return does nothing, as in Fork.
//!
//! Where the walk stands is the sidebar's own state, set by a press and by these keys. It holds
//! while the entry chosen is the one it chose (a row chosen in the history lets it go) and while
//! the rows are the ones it stood among — but for the rows laid out again for a heading or folder
//! these keys or a press opened or closed, where it holds while that heading or folder still
//! stands at its place.

use cairn_model::SidebarRow;
use freya::prelude::*;

use super::{SIDEBAR_ROW_HEIGHT, SidebarRefs, SidebarTarget};
use crate::accelerators;

/// The walk as the sidebar keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walk {
    /// The rows it stands among, by the address of their list — compared, never read — or
    /// `None` while the rows are being laid out again around the row it stands on.
    rows: Option<usize>,
    /// The row it stands on, by its place among them.
    index: usize,
    /// That row, by which a walk kept across a new layout knows it is still there.
    row: SidebarRow,
    /// The entry chosen when it was set: a different one lets the walk go.
    chosen: Option<SidebarTarget>,
}

impl Walk {
    /// The walk standing on `index` of `shown`'s rows, with `chosen` the entry chosen.
    pub fn at(
        shown: &SidebarRefs,
        index: usize,
        row: SidebarRow,
        chosen: Option<SidebarTarget>,
    ) -> Self {
        Self {
            rows: Some(rows_address(shown)),
            index,
            row,
            chosen,
        }
    }

    /// The walk standing on `row`, at `index`, while the rows are laid out again around it: it
    /// holds over any rows while that heading or folder stands at its place, open or closed.
    pub fn keeping(index: usize, row: SidebarRow, chosen: Option<SidebarTarget>) -> Self {
        Self {
            rows: None,
            index,
            row,
            chosen,
        }
    }
}

fn rows_address(shown: &SidebarRefs) -> usize {
    std::sync::Arc::as_ptr(&shown.rows) as usize
}

/// The row the walk stands on now, if it still holds.
pub fn place_now(
    walk: Option<&Walk>,
    chosen: Option<&SidebarTarget>,
    shown: Option<&SidebarRefs>,
) -> Option<usize> {
    let shown = shown?;
    let walk = walk?;
    let same_rows = match walk.rows {
        Some(rows) => rows == rows_address(shown),
        None => shown
            .rows
            .get(walk.index)
            .is_some_and(|row| same_place(*row, walk.row)),
    };
    (same_rows && walk.chosen.as_ref() == chosen && walk.index < shown.rows.len())
        .then_some(walk.index)
}

/// Whether two rows are the same heading, folder or entry, whether open or closed.
fn same_place(one: SidebarRow, other: SidebarRow) -> bool {
    match (one, other) {
        (SidebarRow::Section { section, .. }, SidebarRow::Section { section: was, .. }) => {
            section == was
        }
        (
            SidebarRow::Folder { first, depth, .. },
            SidebarRow::Folder {
                first: was_first,
                depth: was_depth,
                ..
            },
        ) => first == was_first && depth == was_depth,
        (
            SidebarRow::Section { .. }
            | SidebarRow::Folder { .. }
            | SidebarRow::Ref { .. }
            | SidebarRow::DetachedHead
            | SidebarRow::Stash { .. },
            _,
        ) => one == other,
    }
}

/// How many levels in `row` stands: a heading 0, what stands straight under it 1, and one more
/// per folder it is in.
pub fn level(row: SidebarRow) -> u32 {
    match row {
        SidebarRow::Section { .. } => 0,
        SidebarRow::Folder { depth, .. } | SidebarRow::Ref { depth, .. } => u32::from(depth) + 1,
        SidebarRow::DetachedHead | SidebarRow::Stash { .. } => 1,
    }
}

/// Whether `row` is a heading or a folder, and open.
fn disclosure(row: SidebarRow) -> Option<bool> {
    match row {
        SidebarRow::Section { open, .. } | SidebarRow::Folder { open, .. } => Some(open),
        SidebarRow::Ref { .. } | SidebarRow::DetachedHead | SidebarRow::Stash { .. } => None,
    }
}

/// What a key does to the walk standing on `at` of `rows` — or on nothing yet, where ↑ and ↓
/// start on the first row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarStep {
    /// The walk moves to this row.
    To(usize),
    /// The heading or folder the walk stands on is opened or closed, the walk staying on it.
    Toggle(usize),
}

/// What `key` does to the walk at `at` over `rows`; `None` for a key the walk does not own, and
/// for ← or → where there is nowhere to go.
pub fn walked_to(key: &Key, at: Option<usize>, rows: &[SidebarRow]) -> Option<SidebarStep> {
    let last = rows.len().checked_sub(1)?;
    let Some(at) = at else {
        return match key {
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowUp) => Some(SidebarStep::To(0)),
            _ => None,
        };
    };
    let row = *rows.get(at)?;
    match key {
        Key::Named(NamedKey::ArrowDown) => Some(SidebarStep::To(at.saturating_add(1).min(last))),
        Key::Named(NamedKey::ArrowUp) => Some(SidebarStep::To(at.saturating_sub(1))),
        Key::Named(NamedKey::ArrowLeft) => match disclosure(row) {
            Some(true) => Some(SidebarStep::Toggle(at)),
            Some(false) | None => {
                let own = level(row);
                rows.get(..at)?
                    .iter()
                    .rposition(|above| level(*above) < own)
                    .map(SidebarStep::To)
            }
        },
        Key::Named(NamedKey::ArrowRight) => match disclosure(row) {
            Some(false) => Some(SidebarStep::Toggle(at)),
            Some(true) => rows
                .get(at + 1)
                .filter(|below| level(**below) > level(row))
                .map(|_| SidebarStep::To(at + 1)),
            None => None,
        },
        _ => None,
    }
}

/// Whether a row walked to is chosen at once, as a press chooses it: a ref, a stash or a
/// detached `HEAD`. A heading or a folder walked to only holds the walk.
pub fn chosen_when_walked(row: SidebarRow) -> bool {
    match row {
        SidebarRow::Ref { .. } | SidebarRow::DetachedHead | SidebarRow::Stash { .. } => true,
        SidebarRow::Section { .. } | SidebarRow::Folder { .. } => false,
    }
}

/// What the key handler reads.
pub struct Keys {
    pub walk: State<Option<Walk>>,
    pub chosen: Option<SidebarTarget>,
    pub shown: Option<SidebarRefs>,
    pub on_row: EventHandler<SidebarRow>,
    pub controller: ScrollController,
}

/// The sidebar's key handler: keys it does not own — every chord of the window's, and Return —
/// are left unhandled.
pub fn handler(keys: Keys) -> impl FnMut(Event<KeyboardEventData>) + 'static {
    let Keys {
        mut walk,
        chosen,
        shown,
        on_row,
        mut controller,
    } = keys;
    move |e: Event<KeyboardEventData>| {
        if accelerators::is_chord(&e, &[]) {
            return;
        }
        let Some(shown) = shown.as_ref() else {
            return;
        };
        let at = place_now(walk.peek().as_ref(), chosen.as_ref(), Some(shown))
            // Nowhere yet: the chosen entry's row, looked for once per key press, each row
            // compared in place.
            .or_else(|| {
                let chosen = chosen.as_ref()?;
                shown
                    .rows
                    .iter()
                    .position(|row| chosen.is(*row, &shown.refs))
            });
        let Some(step) = walked_to(&e.key, at, &shown.rows) else {
            return;
        };
        e.stop_propagation();
        match step {
            SidebarStep::Toggle(index) => {
                if let Some(row) = shown.rows.get(index).copied() {
                    walk.set(Some(Walk::keeping(index, row, chosen.clone())));
                    on_row.call(row);
                }
            }
            SidebarStep::To(index) => {
                let Some(row) = shown.rows.get(index).copied() else {
                    return;
                };
                let chooses = chosen_when_walked(row);
                let now_chosen = if chooses {
                    SidebarTarget::of(row, &shown.refs)
                } else {
                    chosen.clone()
                };
                walk.set(Some(Walk::at(shown, index, row, now_chosen)));
                controller.scroll_to_offset(
                    index as f32 * SIDEBAR_ROW_HEIGHT,
                    SIDEBAR_ROW_HEIGHT,
                    Direction::Vertical,
                );
                if chooses {
                    on_row.call(row);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cairn_model::SidebarSection;

    use super::*;

    fn rows() -> Vec<SidebarRow> {
        vec![
            SidebarRow::Section {
                section: SidebarSection::Branches,
                open: true,
            },
            SidebarRow::Folder {
                first: 0,
                depth: 0,
                open: true,
            },
            SidebarRow::Ref { index: 0, depth: 1 },
            SidebarRow::Folder {
                first: 1,
                depth: 0,
                open: false,
            },
            SidebarRow::Ref { index: 2, depth: 0 },
            SidebarRow::Section {
                section: SidebarSection::Tags,
                open: false,
            },
            SidebarRow::Section {
                section: SidebarSection::Stashes,
                open: true,
            },
            SidebarRow::Stash { index: 0 },
        ]
    }

    fn key(named: NamedKey) -> Key {
        Key::Named(named)
    }

    /// ↓ and ↑ walk every row — headings, folders, refs, stashes — one step, stopping at either
    /// end, and start on the first row; any other key is not the walk's, Return among them.
    /// Caught by: a heading or folder skipped, a step past an end, or Return taken.
    #[test]
    fn the_arrows_walk_every_row_and_stop_at_the_ends() {
        let rows = rows();
        let down = key(NamedKey::ArrowDown);
        let up = key(NamedKey::ArrowUp);
        assert_eq!(walked_to(&down, None, &rows), Some(SidebarStep::To(0)));
        let mut at = 0;
        let mut visited = vec![at];
        while let Some(SidebarStep::To(next)) = walked_to(&down, Some(at), &rows) {
            if next == at {
                break;
            }
            visited.push(next);
            at = next;
        }
        assert_eq!(visited, (0..rows.len()).collect::<Vec<_>>());
        assert_eq!(walked_to(&up, Some(0), &rows), Some(SidebarStep::To(0)));
        assert_eq!(walked_to(&up, Some(3), &rows), Some(SidebarStep::To(2)));
        assert_eq!(walked_to(&key(NamedKey::Enter), Some(3), &rows), None);
        assert_eq!(walked_to(&down, None, &[]), None, "nothing to walk");
    }

    /// ← closes an open heading or folder and from anything else goes to the row it stands in;
    /// → opens a closed one and from an open one goes to its first row; neither does anything on
    /// a leaf with nowhere to go. Caught by: ← going up from an open folder instead of closing
    /// it, → stepping into a closed one, or a parent found at the wrong level.
    #[test]
    fn left_and_right_open_close_and_climb_as_a_tree() {
        let rows = rows();
        let left = key(NamedKey::ArrowLeft);
        let right = key(NamedKey::ArrowRight);
        assert_eq!(
            walked_to(&left, Some(1), &rows),
            Some(SidebarStep::Toggle(1))
        );
        assert_eq!(walked_to(&left, Some(2), &rows), Some(SidebarStep::To(1)));
        assert_eq!(walked_to(&left, Some(3), &rows), Some(SidebarStep::To(0)));
        assert_eq!(walked_to(&left, Some(4), &rows), Some(SidebarStep::To(0)));
        assert_eq!(walked_to(&left, Some(7), &rows), Some(SidebarStep::To(6)));
        assert_eq!(walked_to(&left, Some(5), &rows), None, "a closed heading");
        assert_eq!(
            walked_to(&right, Some(3), &rows),
            Some(SidebarStep::Toggle(3))
        );
        assert_eq!(
            walked_to(&right, Some(5), &rows),
            Some(SidebarStep::Toggle(5))
        );
        assert_eq!(walked_to(&right, Some(1), &rows), Some(SidebarStep::To(2)));
        assert_eq!(walked_to(&right, Some(0), &rows), Some(SidebarStep::To(1)));
        assert_eq!(walked_to(&right, Some(2), &rows), None, "a leaf");
        let empty_open = [SidebarRow::Section {
            section: SidebarSection::Remotes,
            open: true,
        }];
        assert_eq!(walked_to(&right, Some(0), &empty_open), None);
    }
}
