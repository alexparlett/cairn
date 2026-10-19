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

/// The items of `groups` in order, flattened: each one's group and place in it.
fn flattened<T>(groups: &Groups<T>) -> Vec<&Item<T>> {
    groups.iter().flatten().collect()
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
        let items = flattened(&self.groups);
        let enabled: Rc<Vec<bool>> = Rc::new(items.iter().map(|item| item.is_enabled()).collect());
        let first = self
            .highlight_first
            .then(|| step(&enabled, None, true))
            .flatten();
        let mut highlighted = use_state(move || first);
        // The submenu open, by its row, and whether it holds the keyboard.
        let mut open = use_state(|| None::<usize>);
        let mut sub_focused = use_state(|| false);
        // The greyed row under the pointer, whose reason is drawn under it: Freya's own tooltip
        // hides while a context menu is open, so a menu draws its reasons itself.
        let mut pointed_greyed = use_state(|| None::<usize>);
        let ticks = items.iter().any(|item| item.ticked);
        let return_focus = self.return_focus;

        let choose = {
            let groups = self.groups.clone();
            let on_choose = self.on_choose.clone();
            move |n: usize| {
                let Some(item) = groups.iter().flatten().nth(n) else {
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
            let enabled = enabled.clone();
            let groups = self.groups.clone();
            let mut choose = choose.clone();
            let on_sideways = self.on_sideways.clone();
            let on_back = self.on_back.clone();
            let on_dismiss = self.on_dismiss.clone();
            move |e: Event<KeyboardEventData>| {
                let is_submenu = |n: Option<usize>| {
                    n.and_then(|n| groups.iter().flatten().nth(n))
                        .is_some_and(|item| matches!(item.kind, ItemKind::Submenu(_)))
                };
                match accelerators::menu_key(&e) {
                    key @ (MenuKey::Down | MenuKey::Up) => {
                        let next = step(&enabled, *highlighted.peek(), key == MenuKey::Down);
                        highlighted.set(next);
                        open.set(None);
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
        let mut children: Vec<Element> = Vec::new();
        let mut n = 0usize;
        for (g, group) in self.groups.iter().enumerate() {
            if g > 0 {
                children.push(separator());
            }
            for item in group {
                let at = n;
                n += 1;
                let mut choosing = choose.clone();
                let submenu = match (&item.kind, opened == Some(at)) {
                    (ItemKind::Submenu(groups), true) => Some(
                        Panel::new(groups.clone(), return_focus)
                            .highlight_first(*sub_focused.peek())
                            .take_focus(*sub_focused.peek())
                            .on_choose(self.on_choose.clone())
                            .on_back(move |()| {
                                open.set(None);
                                sub_focused.set(false);
                                id.request_focus();
                            })
                            .into(),
                    ),
                    (ItemKind::Submenu(_) | ItemKind::Choose(_), _) => None,
                };
                let is_submenu = matches!(item.kind, ItemKind::Submenu(_));
                let enabled_here = item.is_enabled();
                children.push(
                    row(item, lit == Some(at), ticks, submenu, explained == Some(at))
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
                        .into(),
                );
            }
        }

        rect()
            .a11y_id(id)
            .a11y_role(AccessibilityRole::Menu)
            .a11y_focusable(true)
            .a11y_auto_focus(self.take_focus)
            .min_width(Size::px(metrics::MENU_MIN_WIDTH))
            .width(Size::fill_minimum())
            .content(Content::fit())
            .font_size(metrics::BODY)
            .on_key_down(keys)
            .children(children)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// The rule between two groups.
fn separator() -> Element {
    rect()
        .width(Size::fill_minimum())
        .padding(Gaps::new(3., 0., 3., 0.))
        .child(
            rect()
                .width(Size::fill())
                .height(Size::px(1.))
                .background(tones::MENU_SEPARATOR),
        )
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
            .child(
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
                    .child(label().text(reason).width(Size::fill()).color(tones::TEXT)),
            )
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
        // The chord at the far end of a row as wide as the menu's widest, never wider: a
        // flexible gap would stretch the menu across the window.
        .main_align(Alignment::SpaceBetween)
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
        .maybe_child(trailing.map(|trailing| {
            label()
                .text(trailing)
                .max_lines(1)
                .margin(Gaps::new(0., 0., 0., metrics::MENU_CHORD_GAP))
                .color(if enabled { tones::MENU_CHORD } else { text })
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
