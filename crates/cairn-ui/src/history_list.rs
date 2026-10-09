//! The virtualised history list.

use std::sync::Arc;

use cairn_model::{History, HistoryRow, Oid, RefsSnapshot, RowContent, RowEdges, RowId};
use freya::prelude::*;

use crate::accelerators::{self, Action, HeldKeys, Scope};
use crate::commit_row::label_room;
use crate::graph_geometry::ROW_HEIGHT;
use crate::ref_chips::{Chip, chips_of_row};

pub const PREFETCH_ROWS: usize = 24;

/// The commit rows' context menu item, Fork's (the user's decision, 2026-10-09).
pub const NEW_BRANCH_CAPTION: &str = "New Branch…";

/// Every row in the last [`PREFETCH_ROWS`] asks, not only the boundary one, which a fast scroll can skip.
fn asks_for_more(index: usize, length: usize) -> bool {
    index + PREFETCH_ROWS >= length
}

const PAGE_JUMP: usize = 10;

/// What a row draws, read out of the history for that row alone: its content, its edges and
/// its chips, and nothing else of it.
#[derive(Debug, Clone, PartialEq)]
pub struct RowRender {
    pub content: RowContent,
    /// The row's lane and every line crossing it, derived from the rows above it for this
    /// row alone.
    pub graph: RowEdges,
    /// The chips the row draws, laid out for the room the list has, once as the row is built
    /// ([`row_chips`]; a stash's row its `stash@{n}`).
    pub chips: Vec<Chip>,
    /// Whether this is `HEAD`'s commit, drawn bold.
    pub head: bool,
    /// Whether no ref reaches the row's commit: Show Lost Commits draws it dimmed
    /// (staging-and-commit R11.1).
    pub lost: bool,
    pub selected: bool,
    /// Width of the graph column for the whole list, in lanes.
    pub lanes: usize,
}

pub struct HistoryList {
    rows: State<History>,
    refs: Option<Arc<RefsSnapshot>>,
    lanes: usize,
    selected: Option<RowId>,
    also_selected: Option<RowId>,
    held: Option<Readable<HeldKeys>>,
    on_select: EventHandler<RowId>,
    on_extend: EventHandler<(RowId, usize)>,
    on_reach_end: EventHandler<()>,
    on_action: EventHandler<Action>,
    on_new_branch: EventHandler<Oid>,
    row: Callback<RowRender, Element>,
    controller: Option<ScrollController>,
    cursor: Option<State<usize>>,
    key: DiffKey,
}

impl HistoryList {
    pub fn new(rows: State<History>, row: impl Fn(RowRender) -> Element + 'static) -> Self {
        Self {
            rows,
            refs: None,
            lanes: 1,
            selected: None,
            also_selected: None,
            held: None,
            on_select: EventHandler::new(|_| {}),
            on_extend: EventHandler::new(|_| {}),
            on_reach_end: EventHandler::new(|()| {}),
            on_action: EventHandler::new(|_: Action| {}),
            on_new_branch: EventHandler::new(|_: Oid| {}),
            row: Callback::new(row),
            controller: None,
            cursor: None,
            key: DiffKey::None,
        }
    }

    /// Scrolls the list through `controller` rather than one of its own, so its caller can
    /// reveal a row the list did not choose (a parent link's, through [`reveal_row`]).
    pub fn controller(mut self, controller: ScrollController) -> Self {
        self.controller = Some(controller);
        self
    }

    /// The refs snapshot a branch's upstream is read from, for compact labels (R5.2), and the
    /// branch `HEAD` is on, whose chip leads its row's.
    /// Keeps where the selection sits in `cursor` rather than a state of its own, so its
    /// caller — which selects rows the list did not choose, a pressed ref's or a parent link's —
    /// can say where the row it chose is, and the next arrow key finds it there at once
    /// instead of searching the loaded rows. A hint: checked against the row at that index
    /// before use.
    pub fn cursor(mut self, cursor: State<usize>) -> Self {
        self.cursor = Some(cursor);
        self
    }

    pub fn refs(mut self, refs: Option<Arc<RefsSnapshot>>) -> Self {
        self.refs = refs;
        self
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

    /// The second of two rows selected to be compared (R7), drawn selected beside the first.
    pub fn also_selected(mut self, also_selected: Option<RowId>) -> Self {
        self.also_selected = also_selected;
        self
    }

    /// The keys the window hears held, which a press is resolved against: a press while the
    /// table's `ExtendSelection` chord is held (⌘-click, Ctrl-click) is reported through
    /// [`Self::on_extend`] rather than [`Self::on_select`].
    pub fn held(mut self, held: impl Into<Readable<HeldKeys>>) -> Self {
        self.held = Some(held.into());
        self
    }

    pub fn on_select(mut self, on_select: impl Into<EventHandler<RowId>>) -> Self {
        self.on_select = on_select.into();
        self
    }

    /// A row pressed with the selection-extending chord held, with its index in the rows: the
    /// second commit of a comparison (R7.1). What it selects is the caller's.
    pub fn on_extend(mut self, on_extend: impl Into<EventHandler<(RowId, usize)>>) -> Self {
        self.on_extend = on_extend.into();
        self
    }

    /// Called within [`PREFETCH_ROWS`] of the end of what is loaded, possibly
    /// more than once for the same end.
    pub fn on_reach_end(mut self, on_reach_end: impl Into<EventHandler<()>>) -> Self {
        self.on_reach_end = on_reach_end.into();
        self
    }

    /// A chord of the history's own scope pressed while the list has focus
    /// ([`Scope::History`]: Show Lost Commits, staging-and-commit R7.3), resolved through the
    /// accelerator table. What it does is the caller's.
    pub fn on_action(mut self, on_action: impl Into<EventHandler<Action>>) -> Self {
        self.on_action = on_action.into();
        self
    }

    /// "New Branch…" chosen from a commit row's context menu — every commit's row, a lost
    /// one's too; a stash's row has no menu — reporting that commit (staging-and-commit R11.3,
    /// the user's decision, 2026-10-09). What it opens is the caller's.
    pub fn on_new_branch(mut self, on_new_branch: impl Into<EventHandler<Oid>>) -> Self {
        self.on_new_branch = on_new_branch.into();
        self
    }
}

// Hand-written: `EventHandler` and `Callback` never compare equal, and their identity is stable.
impl PartialEq for HistoryList {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
            && same_refs(&self.refs, &other.refs)
            && self.lanes == other.lanes
            && self.selected == other.selected
            && self.also_selected == other.also_selected
            && self.controller == other.controller
            && self.cursor == other.cursor
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
    rows: State<History>,
    refs: Option<Arc<RefsSnapshot>>,
    /// The room a row's chips and subject share, from the list's measured width.
    room: f32,
    lanes: usize,
    selected: Option<RowId>,
    also_selected: Option<RowId>,
    held: Option<Readable<HeldKeys>>,
    length: usize,
    row: Callback<RowRender, Element>,
    on_select: EventHandler<RowId>,
    on_extend: EventHandler<(RowId, usize)>,
    on_reach_end: EventHandler<()>,
    on_new_branch: EventHandler<Oid>,
    list_id: AccessibilityId,
    cursor: State<usize>,
}

impl PartialEq for ListData {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
            && same_refs(&self.refs, &other.refs)
            && self.room == other.room
            && self.lanes == other.lanes
            && self.selected == other.selected
            && self.also_selected == other.also_selected
            && self.length == other.length
            && self.list_id == other.list_id
    }
}

impl Component for HistoryList {
    fn render(&self) -> impl IntoElement {
        let list_id = use_a11y();
        let focus = use_focus(list_id);
        // Resolved unconditionally, so the hook count holds whichever controller is used.
        let own = use_scroll_controller(ScrollConfig::default);
        let controller = self.controller.unwrap_or(own);
        // A hint, checked against the row at that index before use; the caller's when given,
        // resolved unconditionally so the hook count holds either way.
        let own_cursor = use_state(|| 0usize);
        let cursor = self.cursor.unwrap_or(own_cursor);
        // Measured, so chips stop being built at the column's edge (R5.3); none are built
        // before the first measurement.
        let mut width = use_state(|| 0.0f32);

        // Reading the length subscribes this component to the row vector.
        let length = self.rows.read().len();

        let data = ListData {
            rows: self.rows,
            refs: self.refs.clone(),
            room: label_room(*width.read(), self.lanes),
            lanes: self.lanes,
            selected: self.selected,
            also_selected: self.also_selected,
            held: self.held.clone(),
            length,
            row: self.row.clone(),
            on_select: self.on_select.clone(),
            on_extend: self.on_extend.clone(),
            on_reach_end: self.on_reach_end.clone(),
            on_new_branch: self.on_new_branch.clone(),
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
            .on_sized(move |e: Event<SizedEventData>| width.set_if_modified(e.area.width()))
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
        let on_action = self.on_action.clone();

        move |e: Event<KeyboardEventData>| {
            // The history's own chords: heard here, where its scope is.
            if let Some(action) = accelerators::resolve_key(&e, Scope::History) {
                e.stop_propagation();
                on_action.call(action);
                return;
            }
            // A chord is an accelerator's, whoever hears it: Ctrl+↓ is "next change", not
            // "next commit", even while the pane that hears it does not have focus.
            if accelerators::is_chord(&e, &[Scope::History]) {
                return;
            }
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
                held.id(next).map(|id| (next, id))
            };
            let Some((next, id)) = moved else {
                return;
            };

            e.stop_propagation();
            cursor.set(next);
            on_select.call(id);
            reveal_row(&mut controller, next);
        }
    }
}

/// Scrolls a history list driven by `controller` until row `index` is in view. By offset:
/// the row usually has no element yet.
pub fn reveal_row(controller: &mut ScrollController, index: usize) {
    controller.scroll_to_offset(index as f32 * ROW_HEIGHT, ROW_HEIGHT, Direction::Vertical);
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
    let Some(row) = rows.row(item.index) else {
        // Length and history can disagree for one frame; draw an empty row of the right height.
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
    let on_extend = data.on_extend.clone();
    let held = data.held.clone();
    let on_reach_end = data.on_reach_end.clone();
    let on_new_branch = data.on_new_branch.clone();
    let asks_for_more = asks_for_more(index, data.length);

    let drawn = data.row.call(render_of(row, data));

    rect()
        // Keyed by identity: a positional key lets a reused slot paint the previous row's graph.
        .key(id)
        .width(Size::fill())
        .height(Size::px(item.size))
        .on_press(move |_| {
            list_id.request_focus();
            let extending = held
                .as_ref()
                .is_some_and(|held| held.peek().press() == Some(Action::ExtendSelection));
            if extending {
                on_extend.call((id, index));
            } else {
                cursor.set(index);
                on_select.call(id);
            }
        })
        .maybe(asks_for_more, |el| {
            el.on_visible(move |_| on_reach_end.call(()))
        })
        // Every commit's row offers Fork's "New Branch…"; a stash's row offers nothing.
        .on_pointer_down(move |e: Event<PointerEventData>| {
            let commit = match id {
                RowId::Commit(commit) => commit,
                RowId::Stash(_) => return,
            };
            if e.button() == Some(MouseButton::Right) {
                let on_new_branch = on_new_branch.clone();
                ContextMenu::open_from_down(
                    Menu::new().child(
                        MenuButton::new()
                            .on_press(move |_: Event<PressEventData>| {
                                ContextMenu::close();
                                on_new_branch.call(commit);
                            })
                            .child(NEW_BRANCH_CAPTION),
                    ),
                );
            }
        })
        .child(drawn)
        .into()
}

/// What `row` draws: its content, copied out of the history's stores, its edges and its
/// chips. At most a snapshot interval of rows above it is read: bounded by the interval,
/// never by the history. Rows the assigner laid out always have a snapshot within it; a row
/// without one draws its node alone. Its chips are laid out here, once per row built, for
/// the room the list has — at most the labels that fit are read (`ref_chips`) — and the row
/// only draws them.
fn render_of(row: HistoryRow<'_>, data: &ListData) -> RowRender {
    let id = row.id();
    let chips = chips_of_row(row, data.refs.as_deref(), data.room);
    RowRender {
        content: row.content(),
        graph: row.edges().unwrap_or_else(|| RowEdges {
            lane: row.lane(),
            edges: Vec::new(),
        }),
        chips,
        head: row.labels().is_head(),
        lost: row.is_lost(),
        selected: data.selected == Some(id) || data.also_selected == Some(id),
        lanes: data.lanes,
    }
}

/// The same snapshot, by identity: a refresh hands the list a new one.
fn same_refs(one: &Option<Arc<RefsSnapshot>>, other: &Option<Arc<RefsSnapshot>>) -> bool {
    match (one, other) {
        (Some(one), Some(other)) => Arc::ptr_eq(one, other),
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

/// Where `id` sits in `rows`, checking `hint` first.
fn index_of(rows: &History, id: RowId, hint: usize) -> Option<usize> {
    if rows.id(hint) == Some(id) {
        return Some(hint);
    }
    rows.position(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{GraphRow, Lane, Oid, PagedCommit, RowsPage};

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

    /// A row for each of `ids`, held as one history.
    fn rows_of(ids: impl IntoIterator<Item = u8>) -> History {
        let mut page = RowsPage::new();
        for n in ids {
            page.push(
                GraphRow::new(oid(n), Lane::new(0), Vec::new()),
                PagedCommit {
                    parents: 1,
                    subject: "commit",
                    author: "A",
                    author_time: 0,
                },
            );
        }
        let mut history = History::new();
        match history.append(page) {
            Ok(()) => history,
            Err(full) => panic!("{full}"),
        }
    }

    fn history(len: u8) -> History {
        rows_of(0..len)
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
        assert_eq!(index_of(&History::new(), RowId::Commit(oid(0)), 0), None);
    }

    #[test]
    fn a_selection_survives_a_page_arriving_under_it() {
        let first_page = history(64);
        let chosen = RowId::Commit(oid(40));
        let at = index_of(&first_page, chosen, 0);
        assert_eq!(at, Some(40));

        let both_pages = rows_of(0..200);
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

        let both = rows_of((0..1).chain(0..8));

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

    /// The QA brief: a row's labels are laid out once per row built, never per frame — idle
    /// frames lay out nothing, a scroll lays out the rows it builds. Caught by: chips laid out
    /// in the row's own render, or a list that rebuilds its rows every frame.
    #[test]
    fn a_rows_chips_are_laid_out_once_per_row_built_and_never_per_frame() {
        use crate::graph_geometry::ROW_HEIGHT;
        use crate::ref_chips::LAYOUTS;
        use cairn_model::{Label, RefKind};
        use freya_testing::TestingRunner;

        let mut page = RowsPage::new();
        for n in 0..200u8 {
            page.push_labelled(
                GraphRow::new(oid(n), Lane::new(0), Vec::new()),
                PagedCommit {
                    parents: 1,
                    subject: "commit",
                    author: "A",
                    author_time: 0,
                },
                false,
                &[Label {
                    name: "refs/tags/v1",
                    kind: RefKind::Tag,
                    current: false,
                }],
            );
        }
        let mut history = History::new();
        history.append(page).unwrap();
        let refs = std::sync::Arc::new(cairn_model::RefsSnapshot {
            refs: Vec::new(),
            head: cairn_model::HeadState::Detached(oid(0)),
            stashes: Vec::new(),
            unreadable: 0,
        });
        let (mut test, mut tick) = TestingRunner::new(
            || -> Element {
                let rows = use_consume::<State<History>>();
                let refs = use_consume::<std::sync::Arc<cairn_model::RefsSnapshot>>();
                // What else the window draws: written while the list's own props stay put.
                let tick = use_consume::<State<u32>>();
                rect()
                    .expanded()
                    .child(label().text(tick.read().to_string()))
                    .child(
                        HistoryList::new(rows, |render: RowRender| {
                            crate::CommitRow::new(
                                crate::history_list::tests::summary(&render),
                                render.graph,
                                render.lanes,
                            )
                            .chips(render.chips)
                            .into()
                        })
                        .refs(Some(refs)),
                    )
                    .into()
            },
            (900., 260.).into(),
            move |runner| {
                runner.provide_root_context(move || State::create(history));
                runner.provide_root_context(move || refs);
                runner.provide_root_context(|| State::create(0u32))
            },
            1.,
        );
        test.sync_and_update();
        test.sync_and_update();
        let settled = LAYOUTS.with(std::cell::Cell::get);
        assert!(settled > 0, "no row laid out its chips");
        for n in 1..=20u32 {
            tick.set(n);
            test.sync_and_update();
        }
        assert_eq!(
            LAYOUTS.with(std::cell::Cell::get),
            settled,
            "frames that redrew the window but not the list laid chips out again"
        );
        test.scroll((100., 100.), (0., -(40. * ROW_HEIGHT as f64)));
        let scrolled = LAYOUTS.with(std::cell::Cell::get) - settled;
        let viewport = (260. / ROW_HEIGHT).ceil() as usize;
        assert!(
            scrolled > 0 && scrolled <= 3 * (viewport + 2),
            "a scroll of 40 rows laid out {scrolled} rows' chips for a {viewport}-row viewport"
        );
    }

    pub(super) fn summary(render: &RowRender) -> cairn_model::CommitSummary {
        match &render.content {
            RowContent::Commit(commit) => commit.clone(),
            RowContent::Stash(stash) => stash.as_commit(),
        }
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
