//! A menu's panel: its rows drawn from the data, the one row highlighted — by the pointer or by
//! ↑ and ↓, one highlight for both — and the keys a menu holds while it is open.

use std::rc::Rc;

use freya::prelude::*;

use super::{Groups, Item, ItemKind};
use crate::accelerators::{self, MenuKey, Os};
use crate::style::{metrics, tones};

/// Which way ← and → move along the menu bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Previous,
    Next,
}

/// The rows of one menu, holding the keyboard while it is shown.
pub struct Panel<T> {
    groups: Rc<Groups<T>>,
    /// The groups laid out once, as the panel is made: what each render reads by index.
    layout: Rc<Layout>,
    /// What held focus as the menu opened, given it back on a choice or Escape.
    return_focus: AccessibilityId,
    highlight_first: bool,
    take_focus: bool,
    on_choose: EventHandler<T>,
    /// The menu bar's: ← and → open the menu beside this one.
    on_sideways: Option<EventHandler<Side>>,
    /// A submenu's: ← and Escape close it, back to the row that opened it.
    on_back: Option<EventHandler<()>>,
    /// Closes the whole menu on Escape, which then goes no further.
    on_dismiss: Option<EventHandler<()>>,
    key: DiffKey,
}

impl<T> Panel<T> {
    pub fn new(groups: Groups<T>, return_focus: AccessibilityId) -> Self {
        Self {
            layout: Rc::new(Layout::of(&groups)),
            groups: Rc::new(groups),
            return_focus,
            highlight_first: false,
            take_focus: true,
            on_choose: EventHandler::new(|_: T| {}),
            on_sideways: None,
            on_back: None,
            on_dismiss: None,
            key: DiffKey::None,
        }
    }

    /// Opens with the first item that can be chosen highlighted.
    pub fn highlight_first(mut self, first: bool) -> Self {
        self.highlight_first = first;
        self
    }

    /// Takes the keyboard as it opens (a submenu opened by the pointer does not).
    pub fn take_focus(mut self, take: bool) -> Self {
        self.take_focus = take;
        self
    }

    pub fn on_choose(mut self, on_choose: impl Into<EventHandler<T>>) -> Self {
        self.on_choose = on_choose.into();
        self
    }

    pub fn on_sideways(mut self, on_sideways: impl Into<EventHandler<Side>>) -> Self {
        self.on_sideways = Some(on_sideways.into());
        self
    }

    pub fn on_dismiss(mut self, on_dismiss: impl Into<EventHandler<()>>) -> Self {
        self.on_dismiss = Some(on_dismiss.into());
        self
    }

    fn on_back(mut self, on_back: impl Into<EventHandler<()>>) -> Self {
        self.on_back = Some(on_back.into());
        self
    }
}

// By the data it draws: `EventHandler` never compares equal.
impl<T: PartialEq> PartialEq for Panel<T> {
    fn eq(&self, other: &Self) -> bool {
        self.groups == other.groups
            && self.return_focus == other.return_focus
            && self.highlight_first == other.highlight_first
            && self.take_focus == other.take_focus
            && self.on_sideways.is_some() == other.on_sideways.is_some()
            && self.on_back.is_some() == other.on_back.is_some()
            && self.on_dismiss.is_some() == other.on_dismiss.is_some()
            && self.key == other.key
    }
}

impl<T> std::fmt::Debug for Panel<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Panel")
            .field("groups", &self.groups.len())
            .finish_non_exhaustive()
    }
}

impl<T> KeyExt for Panel<T> {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// A menu's groups laid out once, when its panel is made, so a render, a key press and a row
/// built read an item by its index rather than walking the groups.
#[derive(Debug, Default)]
struct Layout {
    /// Each item's group and place in it, in order.
    places: Vec<(usize, usize)>,
    /// Whether each item can be chosen.
    enabled: Vec<bool>,
    /// Whether each item opens a group after the first ([`group_starts`]).
    starts: Vec<bool>,
    /// The lines a short menu draws, items and rules.
    lines: Vec<Line>,
    /// Whether any item is ticked, which opens the ✓ gutter for the whole menu.
    ticks: bool,
}

impl Layout {
    fn of<T>(groups: &Groups<T>) -> Self {
        let places = groups
            .iter()
            .enumerate()
            .flat_map(|(g, group)| (0..group.len()).map(move |n| (g, n)))
            .collect();
        Self {
            places,
            enabled: groups.iter().flatten().map(Item::is_enabled).collect(),
            starts: group_starts(groups),
            lines: lines_of(groups),
            ticks: groups.iter().flatten().any(|item| item.ticked),
        }
    }

    /// Item `n` of `groups`, the groups it was laid out from.
    fn item<'g, T>(&self, groups: &'g Groups<T>, n: usize) -> Option<&'g Item<T>> {
        let (g, at) = *self.places.get(n)?;
        groups.get(g)?.get(at)
    }

    /// Whether the menu is too long to draw whole.
    fn is_long(&self) -> bool {
        self.lines.len() > LONG_MENU_LINES
    }
}

/// The most lines — items and rules — a menu draws whole; a longer one shows this many rows
/// and scrolls, building only the rows its viewport shows.
pub(crate) const LONG_MENU_LINES: usize = 24;

/// One line of a menu as it is laid out: an item, by its place among every item, or the rule
/// between two groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    Item(usize),
    Rule,
}

/// The lines of `groups` in order: each group's items, a rule between every two groups.
fn lines_of<T>(groups: &Groups<T>) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut n = 0;
    for (g, group) in groups.iter().enumerate() {
        if g > 0 {
            lines.push(Line::Rule);
        }
        for _ in group {
            lines.push(Line::Item(n));
            n += 1;
        }
    }
    lines
}

/// Which items open a group after the first: a long menu draws the rule between two groups
/// along the top of the second group's first row, so every row is one height and the view
/// finds any of them without summing the rows above it.
fn group_starts<T>(groups: &Groups<T>) -> Vec<bool> {
    groups
        .iter()
        .enumerate()
        .flat_map(|(g, group)| (0..group.len()).map(move |n| g > 0 && n == 0))
        .collect()
}

/// The item that can be chosen `side` of `from` (the first or the last from none), wrapping
/// round; `None` when no item can be.
fn step(enabled: &[bool], from: Option<usize>, down: bool) -> Option<usize> {
    let count = enabled.len();
    if count == 0 {
        return None;
    }
    let mut at = from;
    for _ in 0..count {
        let next = match (at, down) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(n), true) => (n + 1) % count,
            (Some(n), false) => (n + count - 1) % count,
        };
        if enabled[next] {
            return Some(next);
        }
        at = Some(next);
    }
    None
}

impl<T: Clone + PartialEq + 'static> Component for Panel<T> {
    fn render(&self) -> impl IntoElement {
        let id = use_a11y();
        let mut controller = use_scroll_controller(ScrollConfig::default);
        let layout = self.layout.clone();
        let first = self
            .highlight_first
            .then(|| step(&layout.enabled, None, true))
            .flatten();
        let mut highlighted = use_state(move || first);
        // The submenu open, by its row, and whether it holds the keyboard.
        let mut open = use_state(|| None::<usize>);
        let mut sub_focused = use_state(|| false);
        // The greyed row under the pointer, whose reason is drawn under it: Freya's own tooltip
        // hides while a context menu is open, so a menu draws its reasons itself.
        let mut pointed_greyed = use_state(|| None::<usize>);
        let ticks = layout.ticks;
        let return_focus = self.return_focus;

        let choose = {
            let groups = self.groups.clone();
            let layout = layout.clone();
            let on_choose = self.on_choose.clone();
            move |n: usize| {
                let Some(item) = layout.item(&groups, n) else {
                    return;
                };
                if !item.is_enabled() {
                    return;
                }
                match &item.kind {
                    ItemKind::Choose(choice) => {
                        return_focus.request_focus();
                        on_choose.call(choice.clone());
                    }
                    ItemKind::Submenu(_) => {
                        highlighted.set(Some(n));
                        open.set(Some(n));
                        sub_focused.set(true);
                    }
                }
            }
        };

        let keys = {
            let layout = layout.clone();
            let groups = self.groups.clone();
            let mut choose = choose.clone();
            let on_sideways = self.on_sideways.clone();
            let on_back = self.on_back.clone();
            let on_dismiss = self.on_dismiss.clone();
            move |e: Event<KeyboardEventData>| {
                let is_submenu = |n: Option<usize>| {
                    n.and_then(|n| layout.item(&groups, n))
                        .is_some_and(|item| matches!(item.kind, ItemKind::Submenu(_)))
                };
                match accelerators::menu_key(&e) {
                    key @ (MenuKey::Down | MenuKey::Up) => {
                        let next = step(&layout.enabled, *highlighted.peek(), key == MenuKey::Down);
                        highlighted.set(next);
                        open.set(None);
                        // A long menu scrolls the item reached into view.
                        if let Some(at) = next
                            && layout.is_long()
                        {
                            controller.scroll_to_offset(
                                at as f32 * metrics::MENU_ITEM,
                                metrics::MENU_ITEM,
                                Direction::Vertical,
                            );
                        }
                    }
                    MenuKey::Choose => {
                        if let Some(n) = *highlighted.peek() {
                            choose(n);
                        }
                    }
                    MenuKey::Forward => {
                        let at = *highlighted.peek();
                        if is_submenu(at) {
                            open.set(at);
                            sub_focused.set(true);
                        } else if let Some(on_sideways) = &on_sideways {
                            on_sideways.call(Side::Next);
                        }
                    }
                    MenuKey::Back => {
                        if let Some(on_back) = &on_back {
                            on_back.call(());
                        } else if let Some(on_sideways) = &on_sideways {
                            on_sideways.call(Side::Previous);
                        }
                    }
                    MenuKey::Close => {
                        if let Some(on_back) = &on_back {
                            on_back.call(());
                        } else if let Some(on_dismiss) = &on_dismiss {
                            return_focus.request_focus();
                            on_dismiss.call(());
                        } else {
                            // The menu's own Escape closes it; focus goes back first.
                            return_focus.request_focus();
                            e.stop_propagation();
                            return;
                        }
                    }
                    // A modifier held is heard by the window, which keeps what is held.
                    MenuKey::Unclaimed => return,
                    MenuKey::Other => {}
                }
                // The menu holds the keyboard: nothing behind it hears the key.
                e.stop_propagation();
                e.prevent_default();
            }
        };

        let lit = *highlighted.read();
        let opened = *open.read();
        let explained = *pointed_greyed.read();
        let long = layout.is_long();

        // One item's row with its handlers. `inline`: whether the row carries its own submenu
        // and greyed reason, as a short menu's rows do; a long menu's rows are clipped by the
        // view that scrolls them, so it draws both itself, beside the view.
        let build = {
            let groups = self.groups.clone();
            let layout = layout.clone();
            let on_choose = self.on_choose.clone();
            let choose = choose.clone();
            Rc::new(move |at: usize, inline: bool| -> Element {
                let Some(item) = layout.item(&groups, at) else {
                    return rect().height(Size::px(metrics::MENU_ITEM)).into();
                };
                let mut choosing = choose.clone();
                let submenu = inline
                    .then(|| {
                        submenu_of(
                            item,
                            opened == Some(at),
                            return_focus,
                            &on_choose,
                            id,
                            open,
                            sub_focused,
                        )
                    })
                    .flatten();
                let is_submenu = matches!(item.kind, ItemKind::Submenu(_));
                let enabled_here = item.is_enabled();
                row(
                    item,
                    lit == Some(at),
                    ticks,
                    submenu,
                    inline && explained == Some(at),
                )
                .on_pointer_enter(move |_| {
                    pointed_greyed.set_if_modified((!enabled_here).then_some(at));
                    if enabled_here {
                        highlighted.set(Some(at));
                        if is_submenu {
                            open.set(Some(at));
                            sub_focused.set(false);
                        } else {
                            open.set(None);
                        }
                    } else {
                        highlighted.set(None);
                        open.set(None);
                    }
                })
                .on_pointer_leave(move |_| {
                    if *pointed_greyed.peek() == Some(at) {
                        pointed_greyed.set(None);
                    }
                })
                .maybe(enabled_here, |row| {
                    row.on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        choosing(at);
                    })
                })
                .into()
            })
        };

        let panel = rect()
            .a11y_id(id)
            .a11y_role(AccessibilityRole::Menu)
            .a11y_focusable(true)
            .a11y_auto_focus(self.take_focus)
            .font_size(metrics::BODY)
            .on_key_down(keys);
        if !long {
            let mut children: Vec<Element> = Vec::new();
            for line in layout.lines.iter() {
                children.push(match *line {
                    Line::Rule => separator(),
                    Line::Item(at) => build(at, true),
                });
            }
            return panel
                .min_width(Size::px(metrics::MENU_MIN_WIDTH))
                .width(Size::fill_minimum())
                .content(Content::fit())
                .children(children);
        }

        // A long menu: a fixed width, its rows virtualised, the open submenu and the greyed
        // reason drawn beside the view at their row's place.
        let (_, scrolled): (i32, i32) = controller.into();
        let top_of = |at: usize| at as f32 * metrics::MENU_ITEM + scrolled as f32;
        let beside: Option<Element> = opened
            .and_then(|at| {
                let item = layout.item(&self.groups, at)?;
                let panel = submenu_of(
                    item,
                    true,
                    return_focus,
                    &self.on_choose,
                    id,
                    open,
                    sub_focused,
                )?;
                Some(
                    rect()
                        .position(Position::new_absolute().top(top_of(at) - 4.).right(-4.))
                        .width(Size::px(0.))
                        .height(Size::px(0.))
                        .child(
                            rect()
                                .content(Content::fit())
                                .child(MenuContainer::new().child(panel)),
                        )
                        .into(),
                )
            })
            .or_else(|| {
                let at = explained?;
                let item = layout.item(&self.groups, at)?;
                let reason = item.reason.clone()?;
                Some(
                    rect()
                        .position(
                            Position::new_absolute()
                                .top(top_of(at) + metrics::MENU_ITEM)
                                .left(metrics::MENU_INSET),
                        )
                        .width(Size::px(0.))
                        .height(Size::px(0.))
                        .child(reason_box(reason))
                        .into(),
                )
            });
        let rows = layout.starts.len();
        let height = rows.min(LONG_MENU_LINES) as f32 * metrics::MENU_ITEM;
        let data = LongRows {
            layout: layout.clone(),
            build: build.clone(),
            lit,
            opened,
            explained,
        };
        panel
            .width(Size::px(metrics::MENU_LONG_WIDTH))
            .height(Size::px(height))
            .child(
                VirtualScrollView::new_with_data_controlled(data, build_line, controller)
                    .length(rows)
                    .item_size(metrics::MENU_ITEM)
                    .scroll_with_arrows(false)
                    .expanded(),
            )
            .maybe_child(beside)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// What a long menu's virtualising view draws from: its lines and the row builder, with what
/// decides which rows are drawn again.
#[derive(Clone)]
struct LongRows {
    layout: Rc<Layout>,
    build: Rc<dyn Fn(usize, bool) -> Element>,
    lit: Option<usize>,
    opened: Option<usize>,
    explained: Option<usize>,
}

impl PartialEq for LongRows {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.layout, &other.layout)
            && self.lit == other.lit
            && self.opened == other.opened
            && self.explained == other.explained
    }
}

fn build_line(item: VirtualItem, data: &LongRows) -> Element {
    let opens_group = data.layout.starts.get(item.index).copied().unwrap_or(false);
    rect()
        .key(item.index)
        .width(Size::fill())
        .height(Size::px(item.size))
        .maybe(opens_group, |row| {
            row.border(
                Border::new()
                    .fill(tones::MENU_SEPARATOR)
                    .width(BorderWidth {
                        top: 1.,
                        right: 0.,
                        bottom: 0.,
                        left: 0.,
                    })
                    .alignment(BorderAlignment::Inner),
            )
        })
        .child((data.build)(item.index, false))
        .into()
}

/// The submenu `item` opens, while it is `open`: a panel of its own, to its row's right.
fn submenu_of<T: Clone + PartialEq + 'static>(
    item: &Item<T>,
    open: bool,
    return_focus: AccessibilityId,
    on_choose: &EventHandler<T>,
    id: AccessibilityId,
    mut opened: State<Option<usize>>,
    mut sub_focused: State<bool>,
) -> Option<Element> {
    match (&item.kind, open) {
        (ItemKind::Submenu(groups), true) => Some(
            Panel::new(groups.clone(), return_focus)
                .highlight_first(*sub_focused.peek())
                .take_focus(*sub_focused.peek())
                .on_choose(on_choose.clone())
                .on_back(move |()| {
                    opened.set(None);
                    sub_focused.set(false);
                    id.request_focus();
                })
                .into(),
        ),
        (ItemKind::Submenu(_) | ItemKind::Choose(_), _) => None,
    }
}

/// A greyed item's reason, in the tooltip's look.
fn reason_box(reason: String) -> Rect {
    rect()
        .layer(Layer::Overlay)
        .interactive(Interactive::No)
        .width(Size::px(metrics::MENU_REASON_WIDTH))
        .padding(Gaps::new(4., 10., 4., 10.))
        .background(tones::MENU)
        .border(
            Border::new()
                .width(1.)
                .alignment(BorderAlignment::Inner)
                .fill(tones::MENU_BORDER),
        )
        .child(label().text(reason).width(Size::fill()).color(tones::TEXT))
}

/// The rule between two groups, as wide as the menu's widest row: drawn by the panel's own
/// child, which `Content::fit` widens to its widest sibling, since a child of its own filling
/// what that child offers would stretch the menu across the window.
fn separator() -> Element {
    rect()
        .width(Size::fill_minimum())
        .height(Size::px(1.))
        .margin(Gaps::new(3., 0., 3., 0.))
        .background(tones::MENU_SEPARATOR)
        .into()
}

/// One item's row: the ✓ gutter when the menu has a ticked item, the label, and the chord or a
/// submenu's chevron right-aligned; highlighted in Fork Dark's hover band with white text; a
/// greyed one's reason drawn under it, in the tooltip's look, while the pointer is on it
/// (`explained`).
fn row<T>(
    item: &Item<T>,
    lit: bool,
    ticks: bool,
    submenu: Option<Element>,
    explained: bool,
) -> Rect {
    let enabled = item.is_enabled();
    let text = if !enabled {
        tones::DISABLED_TEXT
    } else if lit {
        tones::SELECTED_TEXT
    } else {
        tones::TEXT
    };
    let caption = label().text(item.label.clone()).max_lines(1).color(text);
    let reason = item.reason.clone().filter(|_| explained).map(|reason| {
        rect()
            .position(
                Position::new_absolute()
                    .top(metrics::MENU_ITEM)
                    .left(metrics::MENU_INSET),
            )
            .width(Size::px(0.))
            .height(Size::px(0.))
            .child(reason_box(reason))
    });
    let trailing = match &item.kind {
        ItemKind::Submenu(_) => Some("›".to_owned()),
        ItemKind::Choose(_) => item
            .chord
            .and_then(|action| accelerators::chord_name(action, Os::current())),
    };
    rect()
        .width(Size::fill_minimum())
        .height(Size::px(metrics::MENU_ITEM))
        .horizontal()
        .cross_align(Alignment::Center)
        .padding(Gaps::new(0., metrics::MENU_INSET, 0., metrics::MENU_INSET))
        .maybe(lit, |row| row.background(tones::MENU_HOVER))
        .child(
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .maybe(ticks, |leading| {
                    leading.child(
                        rect()
                            .width(Size::px(metrics::MENU_TICK_GUTTER))
                            .maybe_child(item.ticked.then(|| label().text("✓").color(text))),
                    )
                })
                .child(caption),
        )
        // The chord's room is held by a copy of it nobody sees, after the gap, so the menu is
        // measured wide enough for it; the chord itself is placed against the row's right edge
        // once the menu's width is known. A chord placed by the row's own alignment would be
        // set against the room the window offered before the menu was measured, past its edge.
        .maybe_child(trailing.clone().map(|trailing| {
            rect()
                .horizontal()
                .child(rect().width(Size::px(metrics::MENU_CHORD_GAP)))
                .child(label().text(trailing).max_lines(1).color(tones::UNSEEN))
        }))
        .maybe_child(trailing.map(|trailing| {
            rect()
                .position(Position::new_absolute().top(0.).right(0.))
                .height(Size::px(metrics::MENU_ITEM))
                .main_align(Alignment::Center)
                .child(label().text(trailing).max_lines(1).color(if enabled {
                    tones::MENU_CHORD
                } else {
                    text
                }))
        }))
        .maybe_child(reason)
        .maybe_child(submenu.map(|panel| {
            rect()
                .position(Position::new_absolute().top(-4.).right(-4.))
                .width(Size::px(0.))
                .height(Size::px(0.))
                .child(
                    rect()
                        .content(Content::fit())
                        .child(MenuContainer::new().child(panel)),
                )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ↑ and ↓ step over the items that cannot be chosen, wrap round, and start from the first
    /// or the last with nothing highlighted. Caught by: a greyed item highlighted, a walk that
    /// stops at an end, or ↑ from nothing starting at the top.
    #[test]
    fn a_step_skips_greyed_items_and_wraps() {
        let enabled = [false, true, false, true, true];
        assert_eq!(step(&enabled, None, true), Some(1));
        assert_eq!(step(&enabled, None, false), Some(4));
        assert_eq!(step(&enabled, Some(1), true), Some(3));
        assert_eq!(step(&enabled, Some(4), true), Some(1));
        assert_eq!(step(&enabled, Some(1), false), Some(4));
        assert_eq!(step(&[false, false], None, true), None);
        assert_eq!(step(&[], None, true), None);
    }
}
