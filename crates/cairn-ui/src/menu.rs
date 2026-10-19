//! One menu component, drawn from data: every context menu and every menu of the menu bar is a
//! list of groups of [`Item`]s, and this module alone draws them — groups parted by a rule, each
//! item's chord right-aligned in the chord tone, read from the accelerator table
//! ([`accelerators::chord_name`]) and never typed, a greyed item with its reason as its tooltip,
//! a ticked item's ✓ in a gutter the whole menu opens for it, and a submenu flown out to its
//! row's right. No view builds its own menu rows.
//!
//! Its keys are Fork's (`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`): ↑ and ↓ walk
//! the items that can be chosen, Return chooses, → opens a submenu (or, in the menu bar, the next
//! menu) and ← closes it (or opens the previous menu), and Escape or a press outside closes the
//! menu. A menu holds the keyboard while it is open — no chord of the window or a list reaches
//! past it — and hands focus back to where it was when it closes with Escape or a choice. A
//! context menu opens with its first item highlighted; a menu of the bar, as Fork's, with none
//! until ↓ is pressed.

use freya::prelude::*;

use crate::accelerators::Action;

mod bar;
mod panel;

pub use bar::{BarMenu, MenuBar};
pub(crate) use panel::Panel;

/// One item of a menu: what it reads, the action whose chord it shows, why it cannot be chosen
/// now (a greyed item), whether it is ticked, and what choosing it does.
#[derive(Debug, Clone, PartialEq)]
pub struct Item<T> {
    pub label: String,
    /// The action whose first chord the item shows, right-aligned; `None`, or an action with no
    /// chord on this platform, shows none.
    pub chord: Option<Action>,
    /// Why the item is greyed: `None` while it can be chosen. A greyed item always says why.
    pub reason: Option<String>,
    /// A ✓ before the label while its state is on, the label unchanged (Fork's View menu).
    pub ticked: bool,
    pub kind: ItemKind<T>,
}

/// What an item does when chosen.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind<T> {
    /// Reports its choice.
    Choose(T),
    /// Opens a menu of its own, to its right.
    Submenu(Groups<T>),
}

/// A menu: its groups in order, each a run of items, a rule between every two.
pub type Groups<T> = Vec<Vec<Item<T>>>;

impl<T> Item<T> {
    /// An item that reports `choice` when chosen.
    pub fn new(label: impl Into<String>, choice: T) -> Self {
        Self {
            label: label.into(),
            chord: None,
            reason: None,
            ticked: false,
            kind: ItemKind::Choose(choice),
        }
    }

    /// An item that opens `groups` as a submenu.
    pub fn submenu(label: impl Into<String>, groups: Groups<T>) -> Self {
        Self {
            label: label.into(),
            chord: None,
            reason: None,
            ticked: false,
            kind: ItemKind::Submenu(groups),
        }
    }

    /// Shows `action`'s chord.
    pub fn chord(mut self, action: Action) -> Self {
        self.chord = Some(action);
        self
    }

    /// Greyed, saying why, when `reason` says one; as it was when it is `None`.
    pub fn greyed(mut self, reason: Option<String>) -> Self {
        self.reason = reason;
        self
    }

    /// Ticked while `on`.
    pub fn ticked(mut self, on: bool) -> Self {
        self.ticked = on;
        self
    }

    /// Whether it can be chosen now.
    pub fn is_enabled(&self) -> bool {
        self.reason.is_none()
    }
}

/// A context menu over `groups`, for `ContextMenu::open` or `ContextMenu::open_from_down`, its
/// first item highlighted: a choice closes the menu, hands focus back to what held it as the
/// menu opened, and is reported to `on_choose`. Called as the menu opens, where it reads what
/// holds focus.
pub fn context_menu<T: Clone + PartialEq + 'static>(
    groups: Groups<T>,
    on_choose: impl Into<EventHandler<T>>,
) -> Menu {
    let on_choose = on_choose.into();
    let return_focus = *Platform::get().focused_accessibility_id.peek();
    Menu::new().child(
        Panel::new(groups, return_focus)
            .highlight_first(true)
            .on_dismiss(|()| ContextMenu::close())
            .on_choose(move |choice: T| {
                ContextMenu::close();
                on_choose.call(choice);
            }),
    )
}
