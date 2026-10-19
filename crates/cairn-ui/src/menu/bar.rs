//! The menu bar: each menu's name, and the open menu's panel hung under it. As Fork's
//! (`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`): a press on a name opens its menu
//! with nothing highlighted, and a press on the open menu's name closes it; while one is open,
//! the pointer over another name opens that one in its place, and ← and → move to the menu
//! beside it, wrapping round; a name under the pointer, or whose menu is open, sits on Fork
//! Dark's hover box.

use std::rc::Rc;

use freya::prelude::*;

use super::Groups;
use super::panel::{Panel, Side};
use crate::style::{metrics, tones};

/// One menu of the bar: its name, and its items.
#[derive(Debug, Clone, PartialEq)]
pub struct BarMenu<T> {
    pub title: String,
    pub groups: Groups<T>,
}

impl<T> BarMenu<T> {
    pub fn new(title: impl Into<String>, groups: Groups<T>) -> Self {
        Self {
            title: title.into(),
            groups,
        }
    }
}

/// The names of `menus` in a row, the open one's panel under it; a choice is reported to
/// `on_choose` once the menu has closed.
pub struct MenuBar<T> {
    menus: Rc<Vec<BarMenu<T>>>,
    on_choose: EventHandler<T>,
    key: DiffKey,
}

impl<T> MenuBar<T> {
    pub fn new(menus: Vec<BarMenu<T>>) -> Self {
        Self {
            menus: Rc::new(menus),
            on_choose: EventHandler::new(|_: T| {}),
            key: DiffKey::None,
        }
    }

    pub fn on_choose(mut self, on_choose: impl Into<EventHandler<T>>) -> Self {
        self.on_choose = on_choose.into();
        self
    }
}

// By the menus it draws: `EventHandler` never compares equal.
impl<T: PartialEq> PartialEq for MenuBar<T> {
    fn eq(&self, other: &Self) -> bool {
        self.menus == other.menus && self.key == other.key
    }
}

impl<T> std::fmt::Debug for MenuBar<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuBar")
            .field("menus", &self.menus.len())
            .finish_non_exhaustive()
    }
}

impl<T> KeyExt for MenuBar<T> {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl<T: Clone + PartialEq + 'static> Component for MenuBar<T> {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| None::<usize>);
        let mut hovered = use_state(|| None::<usize>);
        // What held focus as the bar opened its first menu: every menu it opens then gives it
        // back, however many it moved through.
        let mut return_focus = use_state(|| *Platform::get().focused_accessibility_id.peek());
        // The window let go of: the open menu closes, as a context menu does.
        use_side_effect(move || {
            if !*Platform::get().is_app_focused.read() && open.peek().is_some() {
                open.set(None);
            }
        });
        let count = self.menus.len();
        let showing = *open.read();
        let pointed = *hovered.read();

        let captions = self.menus.iter().enumerate().map(|(at, menu)| {
            let on_choose = self.on_choose.clone();
            let panel = (showing == Some(at)).then(|| {
                let close = move |_| open.set(None);
                rect()
                    .position(Position::new_absolute().top(metrics::CAPTION_ROW).left(0.))
                    .width(Size::px(0.))
                    .height(Size::px(0.))
                    .child(
                        Menu::new().on_close(close).on_escape(close).child(
                            Panel::new(menu.groups.clone(), *return_focus.peek())
                                .key(DiffKey::U64(at as u64))
                                .on_choose(move |choice: T| {
                                    open.set(None);
                                    on_choose.call(choice);
                                })
                                .on_sideways(move |side: Side| {
                                    let next = match side {
                                        Side::Next => (at + 1) % count,
                                        Side::Previous => (at + count - 1) % count,
                                    };
                                    open.set(Some(next));
                                }),
                        ),
                    )
            });
            rect()
                .height(Size::fill())
                .padding(Gaps::new(
                    0.,
                    metrics::MENU_BAR_PADDING,
                    0.,
                    metrics::MENU_BAR_PADDING,
                ))
                .main_align(Alignment::Center)
                .maybe(showing == Some(at) || pointed == Some(at), |name| {
                    name.background(tones::MENU_HOVER)
                })
                // A press here is the bar's, never the start of a window drag.
                .on_pointer_down(|e: Event<PointerEventData>| e.stop_propagation())
                .on_pointer_enter(move |_| {
                    hovered.set(Some(at));
                    if open.peek().is_some_and(|showing| showing != at) {
                        open.set(Some(at));
                    }
                })
                .on_pointer_leave(move |_| {
                    if *hovered.peek() == Some(at) {
                        hovered.set(None);
                    }
                })
                .on_press(move |_| {
                    if *open.peek() == Some(at) {
                        open.set(None);
                    } else {
                        if open.peek().is_none() {
                            return_focus.set(*Platform::get().focused_accessibility_id.peek());
                        }
                        open.set(Some(at));
                    }
                })
                .child(
                    label()
                        .text(menu.title.clone())
                        .max_lines(1)
                        .font_size(metrics::BODY)
                        .color(tones::TEXT),
                )
                .maybe_child(panel)
                .into()
        });

        rect()
            .horizontal()
            .height(Size::fill())
            .children(captions.collect::<Vec<Element>>())
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
