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
    let return_focus = *Platform::get().focused_accessibility_id.peek();
    context_menu_over(groups, return_focus, on_choose)
}

/// [`context_menu`] opened over the view `owner` names — a list that asked for focus in the
/// same press that opens the menu, before the toolkit moved it there — which a choice or Escape
/// gives focus back to.
pub fn context_menu_over<T: Clone + PartialEq + 'static>(
    groups: Groups<T>,
    owner: AccessibilityId,
    on_choose: impl Into<EventHandler<T>>,
) -> Menu {
    let on_choose = on_choose.into();
    let return_focus = owner;
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

/// The choice of the item in `groups` that shows `action`'s chord and can be chosen now — what
/// a list does when that chord is pressed with it focused, so the chord a menu shows and the key
/// are one source (Fork's Copy Commit SHA on Ctrl+C, Copy Full Path on Ctrl+Shift+C). `None`
/// when no item shows it, or the one that does is greyed. A submenu's items are not looked in:
/// a chord is the row's own.
pub fn chosen_by<T: Clone>(groups: &Groups<T>, action: Action) -> Option<T> {
    groups
        .iter()
        .flatten()
        .find(|item| item.chord == Some(action))
        .filter(|item| item.is_enabled())
        .and_then(|item| match &item.kind {
            ItemKind::Choose(choice) => Some(choice.clone()),
            ItemKind::Submenu(_) => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chord picks the item that shows it, in any group, and nothing when that item is
    /// greyed, is a submenu, or no item shows it. Caught by: a lookup that reads only the
    /// first group, takes a greyed item, or answers the first item whatever its chord.
    #[test]
    fn a_chord_chooses_the_item_that_shows_it_when_it_can_be_chosen() {
        let groups: Groups<u8> = vec![
            vec![Item::new("New Branch…", 1).chord(Action::NewBranch)],
            vec![
                Item::new("Copy Path", 2).chord(Action::Copy),
                Item::new("Copy Full Path", 3).chord(Action::CopyFull),
            ],
        ];
        assert_eq!(chosen_by(&groups, Action::Copy), Some(2));
        assert_eq!(chosen_by(&groups, Action::CopyFull), Some(3));
        assert_eq!(chosen_by(&groups, Action::NewBranch), Some(1));
        assert_eq!(chosen_by(&groups, Action::Refresh), None);
        let greyed: Groups<u8> = vec![vec![
            Item::new("Copy Path", 2)
                .chord(Action::Copy)
                .greyed(Some("why".into())),
        ]];
        assert_eq!(chosen_by(&greyed, Action::Copy), None);
        let submenu: Groups<u8> = vec![vec![
            Item::submenu("Copy", groups.clone()).chord(Action::Copy),
        ]];
        assert_eq!(chosen_by(&submenu, Action::Copy), None);
    }
}
