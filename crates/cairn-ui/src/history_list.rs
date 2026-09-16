//! The virtualised history list.

use cairn_model::{HistoryRow, RowId};
use freya::prelude::*;

use crate::graph_geometry::ROW_HEIGHT;

pub const PREFETCH_ROWS: usize = 24;

/// Every row in the last [`PREFETCH_ROWS`] asks, not only the boundary one, which a fast scroll can skip.
fn asks_for_more(index: usize, length: usize) -> bool {
    index + PREFETCH_ROWS >= length
}

const PAGE_JUMP: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct RowRender {
    pub row: HistoryRow,
    pub selected: bool,
    /// Width of the graph column for the whole list, in lanes.
    pub lanes: usize,
}

pub struct HistoryList {
    rows: State<Vec<HistoryRow>>,
    lanes: usize,
    selected: Option<RowId>,
    on_select: EventHandler<RowId>,
    on_reach_end: EventHandler<()>,
    row: Callback<RowRender, Element>,
    key: DiffKey,
}

impl HistoryList {
    pub fn new(rows: State<Vec<HistoryRow>>, row: impl Fn(RowRender) -> Element + 'static) -> Self {
        Self {
            rows,
            lanes: 1,
            selected: None,
            on_select: EventHandler::new(|_| {}),
            on_reach_end: EventHandler::new(|()| {}),
            row: Callback::new(row),
            key: DiffKey::None,
        }
    }

    pub fn lanes(mut self, lanes: usize) -> Self {
        self.lanes = lanes;
        self
    }

    /// Which row is selected, by identity: survives rows arriving above it.
    pub fn selected(mut self, selected: Option<RowId>) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_select(mut self, on_select: impl Into<EventHandler<RowId>>) -> Self {
        self.on_select = on_select.into();
        self
    }

    /// Called within [`PREFETCH_ROWS`] of the end of what is loaded, possibly
    /// more than once for the same end.
    pub fn on_reach_end(mut self, on_reach_end: impl Into<EventHandler<()>>) -> Self {
        self.on_reach_end = on_reach_end.into();
        self
    }
}

// Hand-written: `EventHandler` and `Callback` never compare equal, and their identity is stable.
impl PartialEq for HistoryList {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
            && self.lanes == other.lanes
            && self.selected == other.selected
            && self.key == other.key
    }
}

impl std::fmt::Debug for HistoryList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryList")
            .field("lanes", &self.lanes)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl KeyExt for HistoryList {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// Data captured inside the builder closure is invisible to `VirtualScrollView`'s diffing.
#[derive(Clone)]
struct ListData {
    rows: State<Vec<HistoryRow>>,
    lanes: usize,
    selected: Option<RowId>,
    length: usize,
    row: Callback<RowRender, Element>,
    on_select: EventHandler<RowId>,
    on_reach_end: EventHandler<()>,
    list_id: AccessibilityId,
    cursor: State<usize>,
}

impl PartialEq for ListData {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
            && self.lanes == other.lanes
            && self.selected == other.selected
            && self.length == other.length
            && self.list_id == other.list_id
    }
}

impl Component for HistoryList {
    fn render(&self) -> impl IntoElement {
        let list_id = use_a11y();
        let focus = use_focus(list_id);
        let controller = use_scroll_controller(ScrollConfig::default);
        // A hint, checked against the row at that index before use.
        let cursor = use_state(|| 0usize);

        // Reading the length subscribes this component to the row vector.
        let length = self.rows.read().len();

        let data = ListData {
            rows: self.rows,
            lanes: self.lanes,
            selected: self.selected,
            length,
            row: self.row.clone(),
            on_select: self.on_select.clone(),
            on_reach_end: self.on_reach_end.clone(),
            list_id,
            cursor,
        };

        let border = get_theme_or_default().read().colors().border_focus;

        rect()
            .expanded()
            .a11y_id(list_id)
            .a11y_focusable(true)
            .a11y_auto_focus(true)
            .a11y_role(AccessibilityRole::List)
            .on_key_down(self.keyboard(cursor, controller))
            .maybe(focus() == Focus::Keyboard, |el| {
                el.border(Border::new().fill(border).width(1.))
            })
            .child(
                VirtualScrollView::new_with_data_controlled(data, build_row, controller)
                    .length(length)
                    .item_size(ROW_HEIGHT)
                    // The arrows move the selection, not the viewport.
                    .scroll_with_arrows(false)
                    .expanded(),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

impl HistoryList {
    /// Keys the list does not own are left unhandled.
    fn keyboard(
        &self,
        mut cursor: State<usize>,
        mut controller: ScrollController,
    ) -> impl FnMut(Event<KeyboardEventData>) + 'static {
        let rows = self.rows;
        let selected = self.selected;
        let on_select = self.on_select.clone();

        move |e: Event<KeyboardEventData>| {
            // Drop the read guard before calling out: a handler that reloads the list would panic.
            let moved = {
                let held = rows.read();
                let Some(last) = held.len().checked_sub(1) else {
                    return;
                };
                let current = selected.and_then(|id| index_of(&held, id, *cursor.peek()));
                let Some(next) = moved_to(&e.key, current, last) else {
                    return;
                };
                held.get(next).map(|row| (next, row.id()))
            };
            let Some((next, id)) = moved else {
                return;
            };

            e.stop_propagation();
            cursor.set(next);
            on_select.call(id);
            // The selected row usually has no element yet, so reveal by offset.
            controller.scroll_to_offset(next as f32 * ROW_HEIGHT, ROW_HEIGHT, Direction::Vertical);
        }
    }
}

/// Where `key` moves a selection at `current`; `None` for a key this list does not own.
fn moved_to(key: &Key, current: Option<usize>, last: usize) -> Option<usize> {
    // Nothing selected yet: every key this list owns starts at the top.
    let from = |step: fn(usize, usize) -> usize| current.map_or(0, |at| step(at, last));

    match key {
        Key::Named(NamedKey::ArrowDown) => Some(from(|at, last| (at + 1).min(last))),
        Key::Named(NamedKey::ArrowUp) => Some(from(|at, _| at.saturating_sub(1))),
        Key::Named(NamedKey::PageDown) => Some(from(|at, last| (at + PAGE_JUMP).min(last))),
        Key::Named(NamedKey::PageUp) => Some(from(|at, _| at.saturating_sub(PAGE_JUMP))),
        Key::Named(NamedKey::Home) => Some(0),
        Key::Named(NamedKey::End) => Some(last),
        _ => None,
    }
}

fn build_row(item: VirtualItem, data: &ListData) -> Element {
    let rows = data.rows.read();
    let Some(row) = rows.get(item.index) else {
        // Length and vector can disagree for one frame; draw an empty row of the right height.
        return rect()
            .width(Size::fill())
            .height(Size::px(item.size))
            .into();
    };

    let id = row.id();
    let index = item.index;
    let list_id = data.list_id;
    let mut cursor = data.cursor;
    let on_select = data.on_select.clone();
    let on_reach_end = data.on_reach_end.clone();
    let asks_for_more = asks_for_more(index, data.length);

    let drawn = data.row.call(RowRender {
        row: row.clone(),
        selected: data.selected == Some(id),
        lanes: data.lanes,
    });

    rect()
        // Keyed by identity: a positional key lets a reused slot paint the previous row's graph.
        .key(id)
        .width(Size::fill())
        .height(Size::px(item.size))
        .on_press(move |_| {
            list_id.request_focus();
            cursor.set(index);
            on_select.call(id);
        })
        .maybe(asks_for_more, |el| {
            el.on_visible(move |_| on_reach_end.call(()))
        })
        .child(drawn)
        .into()
}

/// Where `id` sits in `rows`, checking `hint` first.
fn index_of(rows: &[HistoryRow], id: RowId, hint: usize) -> Option<usize> {
    if rows.get(hint).is_some_and(|row| row.id() == id) {
        return Some(hint);
    }
    rows.iter().position(|row| row.id() == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{CommitSummary, EdgeSegment, GraphRow, Lane, Oid, RowContent};

    fn oid(n: u8) -> Oid {
        let mut hex = String::new();
        for _ in 0..20 {
            hex.push_str(&format!("{n:02x}"));
        }
        match Oid::parse(&hex) {
            Ok(id) => id,
            Err(_) => unreachable!("20 bytes of hex is a valid SHA-1"),
        }
    }

    fn history(len: u8) -> Vec<HistoryRow> {
        (0..len)
            .map(|n| HistoryRow {
                content: RowContent::Commit(CommitSummary {
                    id: oid(n),
                    parents: Vec::new(),
                    summary: format!("commit {n}"),
                    author_name: "A".to_owned(),
                    author_email: "a@example.com".to_owned(),
                    author_time: 0,
                }),
                graph: GraphRow {
                    id: oid(n),
                    lane: Lane::new(0),
                    edges: vec![EdgeSegment::passing(Lane::new(0))],
                },
            })
            .collect()
    }

    #[test]
    fn a_right_hint_is_taken_without_scanning() {
        let rows = history(8);
        assert_eq!(index_of(&rows, RowId::Commit(oid(5)), 5), Some(5));
    }

    /// Caught by: trusting the hint.
    #[test]
    fn a_wrong_hint_still_finds_the_row() {
        let rows = history(8);
        assert_eq!(index_of(&rows, RowId::Commit(oid(5)), 0), Some(5));
        assert_eq!(index_of(&rows, RowId::Commit(oid(5)), 99), Some(5));
        assert_eq!(index_of(&rows, RowId::Commit(oid(0)), 7), Some(0));
    }

    #[test]
    fn a_row_that_is_not_there_is_not_found() {
        let rows = history(4);
        assert_eq!(index_of(&rows, RowId::Commit(oid(9)), 2), None);
        assert_eq!(index_of(&[], RowId::Commit(oid(0)), 0), None);
    }

    #[test]
    fn a_selection_survives_a_page_arriving_under_it() {
        let first_page = history(64);
        let chosen = RowId::Commit(oid(40));
        let at = index_of(&first_page, chosen, 0);
        assert_eq!(at, Some(40));

        let mut both_pages = first_page;
        both_pages.extend(history(200).into_iter().skip(64));
        assert_eq!(
            index_of(&both_pages, chosen, 40),
            Some(40),
            "the selection moved when the second page arrived"
        );
        assert!(both_pages.len() > 64, "the second page did not arrive");
    }

    /// The hint is wrong here, so this decides the fallback scan.
    #[test]
    fn a_selection_survives_a_row_arriving_above_it() {
        let before = history(8);
        let chosen = RowId::Commit(oid(5));
        assert_eq!(index_of(&before, chosen, 5), Some(5));

        let mut both = history(1);
        both.extend(before);

        assert_eq!(
            index_of(&both, chosen, 5),
            Some(6),
            "the selection stayed on the index instead of following the row"
        );
        assert_eq!(both.len(), 9, "the row above did not arrive");
    }

    /// Caught by: swapped arrows, `End` going to the top, a page jump of one.
    #[test]
    fn every_key_the_list_owns_moves_the_selection_its_own_way() {
        let last = 99;
        let down = Key::Named(NamedKey::ArrowDown);
        let up = Key::Named(NamedKey::ArrowUp);

        assert_eq!(moved_to(&down, Some(10), last), Some(11));
        assert_eq!(moved_to(&up, Some(10), last), Some(9));
        assert_ne!(
            moved_to(&down, Some(10), last),
            moved_to(&up, Some(10), last),
            "the arrow keys move the selection the same way"
        );

        assert_eq!(
            moved_to(&Key::Named(NamedKey::Home), Some(50), last),
            Some(0)
        );
        assert_eq!(
            moved_to(&Key::Named(NamedKey::End), Some(50), last),
            Some(last)
        );
        assert_ne!(
            moved_to(&Key::Named(NamedKey::End), Some(50), last),
            moved_to(&Key::Named(NamedKey::Home), Some(50), last),
            "Home and End go to the same place"
        );

        // More than a row, less than the list: a jump of 1 or of `last` fails here.
        let page_down = moved_to(&Key::Named(NamedKey::PageDown), Some(50), last);
        let page_up = moved_to(&Key::Named(NamedKey::PageUp), Some(50), last);
        assert_eq!(page_down, Some(50 + PAGE_JUMP));
        assert_eq!(page_up, Some(50 - PAGE_JUMP));
        assert!(page_down > Some(51) && page_down < Some(last));
        assert!(page_up < Some(49) && page_up > Some(0));
    }

    #[test]
    fn the_selection_stops_at_both_ends_rather_than_running_off_or_wrapping() {
        let last = 5;
        assert_eq!(
            moved_to(&Key::Named(NamedKey::ArrowDown), Some(last), last),
            Some(last)
        );
        assert_eq!(
            moved_to(&Key::Named(NamedKey::ArrowUp), Some(0), last),
            Some(0)
        );
        assert_eq!(
            moved_to(&Key::Named(NamedKey::PageDown), Some(last), last),
            Some(last)
        );
        assert_eq!(
            moved_to(&Key::Named(NamedKey::PageUp), Some(0), last),
            Some(0)
        );
        // A one-row list is the degenerate case of both.
        assert_eq!(moved_to(&Key::Named(NamedKey::End), Some(0), 0), Some(0));
        assert_eq!(
            moved_to(&Key::Named(NamedKey::ArrowDown), Some(0), 0),
            Some(0)
        );
    }

    #[test]
    fn the_first_key_press_selects_something() {
        for key in [
            NamedKey::ArrowDown,
            NamedKey::ArrowUp,
            NamedKey::PageDown,
            NamedKey::PageUp,
            NamedKey::Home,
        ] {
            assert_eq!(
                moved_to(&Key::Named(key), None, 99),
                Some(0),
                "{key:?} left a keyboard-only reader with nothing selected"
            );
        }
        assert_eq!(moved_to(&Key::Named(NamedKey::End), None, 99), Some(99));
    }

    #[test]
    fn a_key_the_list_does_not_own_moves_nothing() {
        assert_eq!(moved_to(&Key::Named(NamedKey::Tab), Some(3), 99), None);
        assert_eq!(moved_to(&Key::Named(NamedKey::Escape), Some(3), 99), None);
        assert_eq!(moved_to(&Key::Character("j".into()), Some(3), 99), None);
    }

    /// Caught by: asking only at the end, or only from one boundary row.
    #[test]
    fn the_last_screen_of_rows_asks_for_more_and_the_rest_do_not() {
        let length = 1_000;
        assert!(!asks_for_more(0, length));
        assert!(!asks_for_more(length - PREFETCH_ROWS - 1, length));
        assert!(asks_for_more(length - PREFETCH_ROWS, length));
        assert!(asks_for_more(length - 1, length));

        // A history shorter than the prefetch asks from its first row.
        assert!(asks_for_more(0, 3));
    }
}
