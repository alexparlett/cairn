//! The history list: virtualised, keyed by row identity, keyboard reachable.
//!
//! **Virtualised, and the claim is checkable.** Rows are built by a closure
//! Freya's `VirtualScrollView` calls once per item inside its viewport and for
//! no other item, so the number of row elements a frame builds is a function of
//! the window's height and never of the history's length. This file adds no
//! second path that walks the whole vector: the only thing read per render that
//! is proportional to the list is `len()`.
//!
//! **Keyed by `RowId`, never by position.** Two reasons, and the second is the
//! one that bites: a row arriving ABOVE another — which is what the working-tree
//! row will do — must not renumber everything below it; and a `Canvas`'s render
//! callback compares equal to every other callback in the pinned Freya
//! revision, so a row reused at a viewport slot under a positional key would
//! keep the graph of the row that was there before it. A `RowId` key makes that
//! a replacement rather than a reuse.
//!
//! **Paging is asked for by the list and decided by the caller.** Every row
//! within `PREFETCH_ROWS` of the end calls `on_reach_end` when it becomes
//! visible — every one of them, not just the first, because a single trigger
//! row is a single point of failure on a window tall enough to show it from the
//! start. So `on_reach_end` fires repeatedly by design, and whether it turns
//! into a request — whether the history is already complete, whether one is in
//! flight — is the caller's policy, because the caller is the one that knows.
//! Freya has no "scrolled near the end" callback; `on_visible` on a row is the
//! mechanism its own infinite-list example uses.

use cairn_model::{HistoryRow, RowId};
use freya::prelude::*;

use crate::graph_geometry::ROW_HEIGHT;

/// How many rows from the end of the loaded history the list asks for more.
///
/// Roughly a screen at a typical window height, so a page is requested about a
/// screen before the reader would see the end of what is loaded.
pub const PREFETCH_ROWS: usize = 24;

/// Whether the row at `index` asks for the next page when it becomes visible.
///
/// Every row in the last [`PREFETCH_ROWS`] does, not only the boundary one: a
/// single trigger row that happens to be on screen from the first frame, or
/// that is skipped past by a fast scroll, is a list that quietly stops loading.
/// Repeated calls are cheap because the caller already has to decide whether a
/// request is wanted at all.
fn asks_for_more(index: usize, length: usize) -> bool {
    index + PREFETCH_ROWS >= length
}

/// How far `PageUp` and `PageDown` move the selection.
const PAGE_JUMP: usize = 10;

/// What the caller is asked to draw, for one row.
///
/// The row is handed over whole rather than pre-decomposed, because deciding
/// what a row IS — matching on its `RowContent` — is the caller's obligation
/// (PRD A9) and this crate must not make that decision on its behalf.
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
    /// `rows` is a handle, not a vector: the list reads it inside its own
    /// render and inside the item builder, so a page arriving re-renders the
    /// list and rebuilds the VISIBLE rows, and nothing anywhere copies the
    /// history to get it on screen.
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

    /// Which row is selected, by identity. Survives more rows arriving, and
    /// would survive rows arriving above it.
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
    /// loaded. May be called more than once for the same end; the caller
    /// decides what to do about that.
    pub fn on_reach_end(mut self, on_reach_end: impl Into<EventHandler<()>>) -> Self {
        self.on_reach_end = on_reach_end.into();
        self
    }
}

// Hand-written for two reasons. `EventHandler` and `Callback` compare unequal
// to everything, so deriving this would make the list re-render on every render
// of its parent; and they are closures over handles that never change identity,
// so comparing the DATA is the honest comparison. The list still re-renders
// when its rows change, because it reads them and so is subscribed to them.
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
    /// How many rows are loaded, which is what decides whether a row is near
    /// enough to the end to ask for more.
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
        // Where the selection was last seen. A hint, not the truth: every use
        // of it is checked against the row actually at that index, so a stale
        // one costs a scan and never a wrong answer.
        let cursor = use_state(|| 0usize);

        // Reading the length here is what subscribes THIS component to the row
        // vector, so a page arriving re-renders the list even though its props
        // did not change. It is also the only per-render read proportional to
        // the history, and it is O(1).
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
            // Focused when the window opens. The list is the only thing in the
            // window a keyboard can act on, and R4.4 asks for selection to be
            // keyboard reachable — reachable behind a mouse click first is not
            // reachable, for the reader who has no mouse.
            .a11y_auto_focus(true)
            .a11y_role(AccessibilityRole::List)
            .on_key_down(self.keyboard(cursor, controller))
            // A visible focus ring, and only for keyboard focus: a reader who
            // arrived by clicking can already see where they are.
            .maybe(focus() == Focus::Keyboard, |el| {
                el.border(Border::new().fill(border).width(1.))
            })
            .child(
                VirtualScrollView::new_with_data_controlled(data, build_row, controller)
                    .length(length)
                    .item_size(ROW_HEIGHT)
                    // The arrows move the SELECTION, not the viewport, so the
                    // built-in arrow scrolling is turned off rather than
                    // fighting with it.
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
    /// `End` move the selection and reveal it; everything else is left alone,
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
            // called out to. `on_select` today only writes the selection, but a
            // handler that reloaded the list in response would re-enter this
            // borrow and panic on the UI thread, and this workspace forbids a
            // panic on a path a user can reach.
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
            // The row being selected usually does not exist as an element yet,
            // so it has no rectangle to reveal against; a virtualised list
            // reveals by OFFSET instead, which for a uniform row height is the
            // index times the height.
            controller.scroll_to_offset(next as f32 * ROW_HEIGHT, ROW_HEIGHT, Direction::Vertical);
        }
    }
}

/// Where `key` moves a selection currently at `current`, in a list whose last
/// row is `last`. `None` for a key this list does not own, so a shortcut
/// belonging to something else still reaches it.
///
/// Pulled out of the handler because it is the whole of R4.4's arithmetic and
/// the whole of what can be wrong about it: an inverted arrow, an `End` that
/// goes to the top, a page jump that does not clamp. None of that is decidable
/// from inside a closure over a reactive handle.
fn moved_to(key: &Key, current: Option<usize>, last: usize) -> Option<usize> {
    // Nothing selected yet: every key this list owns starts at the top, which
    // is where a reader's eye already is.
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

/// Build one visible row. Called by `VirtualScrollView` for the items inside
/// its viewport and for nothing else.
fn build_row(item: VirtualItem, data: &ListData) -> Element {
    let rows = data.rows.read();
    let Some(row) = rows.get(item.index) else {
        // The length the view was told and the vector it reads can disagree for
        // one frame. An empty row of the right height keeps the geometry honest
        // instead of panicking on an index.
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
/// The hint is where the selection was last seen. Rows only ever append today,
/// so it is right almost always; when it is wrong — a row arrived above, the
/// list was reloaded — the scan behind it is what keeps the answer correct
/// rather than merely fast.
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
    /// row anyway. This is the case that decides whether selection survives
    /// rows arriving — if the hint were trusted, it would not.
    #[test]
    fn a_wrong_hint_still_finds_the_row() {
        let rows = history(8);
        assert_eq!(index_of(&rows, RowId::Commit(oid(5)), 0), Some(5));
        assert_eq!(index_of(&rows, RowId::Commit(oid(5)), 99), Some(5));
        assert_eq!(index_of(&rows, RowId::Commit(oid(0)), 7), Some(0));
    }

    /// A selection whose row is gone reports gone rather than reporting the row
    /// that happens to sit where it used to.
    #[test]
    fn a_row_that_is_not_there_is_not_found() {
        let rows = history(4);
        assert_eq!(index_of(&rows, RowId::Commit(oid(9)), 2), None);
        assert_eq!(index_of(&[], RowId::Commit(oid(0)), 0), None);
    }

    /// R4.4 across a page boundary: a selection made on page one still names
    /// the same row after page two arrives, and still at the same index — which
    /// is the part that would break if identity were positional and rows were
    /// prepended, and the part that a 30-commit test cannot decide because it
    /// never has a second page.
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

    /// The case the module doc calls the one that bites, and the reason
    /// selection is an identity rather than an index: a row arriving ABOVE the
    /// selection — which is exactly what the working-tree row will do — moves
    /// every index below it, and the selection must follow the row rather than
    /// the number. The hint kept from before the arrival is now wrong, which is
    /// what makes this decide the fallback scan and not just the hint.
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

    /// R4.4's arithmetic, one arm at a time and in both directions. The
    /// mutations this kills are the ones a screenshot would not: the arrows
    /// swapped, `End` going to the top, a page jump of one.
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

    /// Neither end of the list can be walked off. A selection that ran past the
    /// last row would index nothing and select nothing; one that wrapped to the
    /// bottom on `ArrowUp` would be a reader losing their place.
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

    /// With nothing selected, every key this list owns selects something rather
    /// than doing nothing: a reader who has only a keyboard must be able to
    /// select a first row at all, which is the first word of "keyboard
    /// reachable".
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

    /// A key the list does not own is left alone, so a shortcut belonging to
    /// something else still reaches it rather than being swallowed here.
    #[test]
    fn a_key_the_list_does_not_own_moves_nothing() {
        assert_eq!(moved_to(&Key::Named(NamedKey::Tab), Some(3), 99), None);
        assert_eq!(moved_to(&Key::Named(NamedKey::Escape), Some(3), 99), None);
        assert_eq!(moved_to(&Key::Character("j".into()), Some(3), 99), None);
    }

    /// Rows ask for more a screen before the end, not AT the end: asking only
    /// once the last loaded row is on screen is a visible stall. And the whole
    /// last screen asks, not one boundary row, so a window tall enough to show
    /// the boundary from the first frame is not a list that stops loading.
    #[test]
    fn the_last_screen_of_rows_asks_for_more_and_the_rest_do_not() {
        let length = 1_000;
        assert!(!asks_for_more(0, length));
        assert!(!asks_for_more(length - PREFETCH_ROWS - 1, length));
        assert!(asks_for_more(length - PREFETCH_ROWS, length));
        assert!(asks_for_more(length - 1, length));

        // A history shorter than the prefetch asks from its first row, which is
        // what fills a viewport the first page did not.
        assert!(asks_for_more(0, 3));
    }
}
