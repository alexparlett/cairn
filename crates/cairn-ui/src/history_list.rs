//! The history list: virtualised, keyed by row identity, keyboard reachable.
//!
//! **Virtualised.** Rows are built by a closure `VirtualScrollView` calls once
//! per item inside its viewport and for no other item, so a frame's work is a
//! function of the window's height and never of the history's length. The only
//! per-render read proportional to the list is `len()`.
//!
//! **Keyed by `RowId`, never by position.** A row arriving ABOVE another — what
//! the working-tree row will do — must not renumber everything below it; and a
//! row reused at a viewport slot under a positional key would keep the graph of
//! the row that was there before it (see `graph_cell`).
//!
//! **Paging is asked for by the list and decided by the caller.** `on_reach_end`
//! fires repeatedly by design (see `asks_for_more`); whether it becomes a
//! request is the caller's policy. Freya has no "scrolled near the end"
//! callback, so `on_visible` on a row is the mechanism, as in its own example.

use cairn_model::{HistoryRow, RowId};
use freya::prelude::*;

use crate::graph_geometry::ROW_HEIGHT;

/// How many rows from the end of the loaded history the list asks for more.
///
/// Roughly a screen at a typical window height, so a page is requested about a
/// screen before the reader reaches the end of what is loaded.
pub const PREFETCH_ROWS: usize = 24;

/// Whether the row at `index` asks for the next page when it becomes visible.
///
/// Every row in the last [`PREFETCH_ROWS`] does, not only the boundary one: a
/// single trigger row on screen from the first frame, or skipped past by a fast
/// scroll, is a list that quietly stops loading.
fn asks_for_more(index: usize, length: usize) -> bool {
    index + PREFETCH_ROWS >= length
}

/// How far `PageUp` and `PageDown` move the selection.
const PAGE_JUMP: usize = 10;

/// What the caller is asked to draw, for one row.
///
/// The row is handed over whole: deciding what a row IS — matching on its
/// `RowContent` — is the caller's obligation (PRD A9).
#[derive(Debug, Clone, PartialEq)]
pub struct RowRender {
    pub row: HistoryRow,
    pub selected: bool,
    /// Width of the graph column for the whole list, in lanes.
    pub lanes: usize,
}

/// The virtualised history list.
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
    /// A list over `rows`, drawing each one through `row`.
    ///
    /// `rows` is a handle, not a vector: the list reads it inside its own render
    /// and inside the item builder, so a page arriving rebuilds the VISIBLE rows
    /// and nothing copies the history to get it on screen.
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

    /// How many lanes of graph every row reserves. One number for the whole
    /// list, or the subjects do not line up into a column.
    pub fn lanes(mut self, lanes: usize) -> Self {
        self.lanes = lanes;
        self
    }

    /// Which row is selected, by identity: survives rows arriving above it.
    pub fn selected(mut self, selected: Option<RowId>) -> Self {
        self.selected = selected;
        self
    }

    /// Called when the reader selects a row, by pointer or by keyboard.
    pub fn on_select(mut self, on_select: impl Into<EventHandler<RowId>>) -> Self {
        self.on_select = on_select.into();
        self
    }

    /// Called when the reader is within [`PREFETCH_ROWS`] of the end of what is
    /// loaded. May be called more than once for the same end.
    pub fn on_reach_end(mut self, on_reach_end: impl Into<EventHandler<()>>) -> Self {
        self.on_reach_end = on_reach_end.into();
        self
    }
}

// Hand-written because `EventHandler` and `Callback` compare unequal to
// everything, so deriving this would re-render the list on every render of its
// parent. They are closures over handles that never change identity, so
// comparing the DATA is the honest comparison.
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

/// What the item builder needs, carried where `VirtualScrollView` can compare
/// it: data captured inside the builder closure is invisible to diffing.
#[derive(Clone)]
struct ListData {
    rows: State<Vec<HistoryRow>>,
    lanes: usize,
    selected: Option<RowId>,
    /// How many rows are loaded: what decides whether a row asks for more.
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
        // Where the selection was last seen. A hint, not the truth: it is
        // checked against the row at that index, so a stale one costs a scan
        // and never a wrong answer.
        let cursor = use_state(|| 0usize);

        // Reading the length is what subscribes THIS component to the row
        // vector, so a page arriving re-renders the list even though its props
        // did not change. O(1), and the only read proportional to the history.
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
            // Focused when the window opens: R4.4 asks for selection to be
            // keyboard reachable, and reachable only behind a mouse click is
            // not reachable for a reader with no mouse.
            .a11y_auto_focus(true)
            .a11y_role(AccessibilityRole::List)
            .on_key_down(self.keyboard(cursor, controller))
            // Only for keyboard focus: a reader who arrived by clicking can
            // already see where they are.
            .maybe(focus() == Focus::Keyboard, |el| {
                el.border(Border::new().fill(border).width(1.))
            })
            .child(
                VirtualScrollView::new_with_data_controlled(data, build_row, controller)
                    .length(length)
                    .item_size(ROW_HEIGHT)
                    // The arrows move the SELECTION, not the viewport, so the
                    // built-in arrow scrolling would fight this list.
                    .scroll_with_arrows(false)
                    .expanded(),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

impl HistoryList {
    /// The keyboard half of R4.4. Arrow keys, `PageUp`/`PageDown`, `Home` and
    /// `End` move the selection and reveal it; every other key is left alone,
    /// so a shortcut this list does not own still reaches whatever does.
    fn keyboard(
        &self,
        mut cursor: State<usize>,
        mut controller: ScrollController,
    ) -> impl FnMut(Event<KeyboardEventData>) + 'static {
        let rows = self.rows;
        let selected = self.selected;
        let on_select = self.on_select.clone();

        move |e: Event<KeyboardEventData>| {
            // The read guard is taken, used and DROPPED before anything is
            // called out to: a handler that reloaded the list in response would
            // otherwise re-enter this borrow and panic on the UI thread.
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
            // The row being selected usually has no element yet, so nothing to
            // reveal against: a virtualised list reveals by OFFSET, which for a
            // uniform row height is the index times the height.
            controller.scroll_to_offset(next as f32 * ROW_HEIGHT, ROW_HEIGHT, Direction::Vertical);
        }
    }
}

/// Where `key` moves a selection currently at `current`, in a list whose last
/// row is `last`. `None` for a key this list does not own, so a shortcut
/// belonging to something else still reaches it.
///
/// A free function because this is the whole of R4.4's arithmetic and of what
/// can be wrong about it, none of which is decidable from inside a closure over
/// a reactive handle.
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

/// Build one visible row. Called by `VirtualScrollView` for the items inside its
/// viewport and for nothing else.
fn build_row(item: VirtualItem, data: &ListData) -> Element {
    let rows = data.rows.read();
    let Some(row) = rows.get(item.index) else {
        // The length the view was told and the vector it reads can disagree for
        // one frame; an empty row of the right height keeps the geometry honest.
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
///
/// The hint is where the selection was last seen. Rows only append today, so it
/// is almost always right; when it is wrong — a row arrived above, the list was
/// reloaded — the scan behind it is what keeps the answer correct.
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

    /// The hint is an optimisation and never an answer: a wrong one finds the
    /// row anyway. Trusting the hint is the mutation this catches.
    #[test]
    fn a_wrong_hint_still_finds_the_row() {
        let rows = history(8);
        assert_eq!(index_of(&rows, RowId::Commit(oid(5)), 0), Some(5));
        assert_eq!(index_of(&rows, RowId::Commit(oid(5)), 99), Some(5));
        assert_eq!(index_of(&rows, RowId::Commit(oid(0)), 7), Some(0));
    }

    /// A selection whose row is gone reports gone, not the row now sitting
    /// where it used to.
    #[test]
    fn a_row_that_is_not_there_is_not_found() {
        let rows = history(4);
        assert_eq!(index_of(&rows, RowId::Commit(oid(9)), 2), None);
        assert_eq!(index_of(&[], RowId::Commit(oid(0)), 0), None);
    }

    /// R4.4 across a page boundary: a selection made on page one still names the
    /// same row after page two arrives, and still at the same index. A
    /// 30-commit fixture cannot decide this — it never has a second page.
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

    /// Why selection is an identity and not an index: a row arriving ABOVE the
    /// selection — what the working-tree row will do — moves every index below
    /// it, and the selection must follow the row rather than the number. The
    /// hint is now wrong, so this decides the fallback scan and not just the
    /// hint.
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

    /// R4.4's arithmetic, one arm at a time and in both directions. Mutations
    /// caught: the arrows swapped, `End` going to the top, a page jump of one.
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

        // A page is more than a row and less than the list, in both
        // directions — which is what a page jump of 1 or of `last` would fail.
        let page_down = moved_to(&Key::Named(NamedKey::PageDown), Some(50), last);
        let page_up = moved_to(&Key::Named(NamedKey::PageUp), Some(50), last);
        assert_eq!(page_down, Some(50 + PAGE_JUMP));
        assert_eq!(page_up, Some(50 - PAGE_JUMP));
        assert!(page_down > Some(51) && page_down < Some(last));
        assert!(page_up < Some(49) && page_up > Some(0));
    }

    /// Neither end can be walked off. A selection running past the last row
    /// would select nothing; one wrapping to the bottom on `ArrowUp` would be a
    /// reader losing their place.
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

    /// With nothing selected, every key this list owns selects something: a
    /// reader with only a keyboard must be able to select a first row at all.
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

    /// A key the list does not own is left alone rather than swallowed, so a
    /// shortcut belonging to something else still reaches it.
    #[test]
    fn a_key_the_list_does_not_own_moves_nothing() {
        assert_eq!(moved_to(&Key::Named(NamedKey::Tab), Some(3), 99), None);
        assert_eq!(moved_to(&Key::Named(NamedKey::Escape), Some(3), 99), None);
        assert_eq!(moved_to(&Key::Character("j".into()), Some(3), 99), None);
    }

    /// Rows ask a screen before the end, not AT the end, which would be a
    /// visible stall; and the whole last screen asks, not one boundary row, so a
    /// window tall enough to show the boundary still loads.
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
