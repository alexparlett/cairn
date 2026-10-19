//! One button of the toolbar, Fork's (`docs/research/ui-review/fork-window-survey-2026-10-16.md`,
//! section 2): an icon over its label, a light box behind it under the pointer, and — on Push and
//! Pull — a grey count badge at the icon's top right. A greyed button keeps its place, draws its
//! icon and label in the disabled tone and reports nothing, whatever presses it; its tooltip
//! says why. A button with a menu has its ⌄ as a separate press beside it.

use freya::prelude::*;

use crate::style::{metrics, tones};

/// What a button's tooltip holds: Fork's name and chord on two lines, one sentence, or none.
#[derive(Debug, Clone, PartialEq)]
pub enum Tip {
    None,
    /// Why the button cannot act now, or what will bring it.
    Sentence(String),
    /// The button's name over its chord, as Fork's Fetch and Push read.
    Named {
        name: String,
        chord: Option<String>,
    },
}

/// A toolbar button: its icon, drawn in the tone it is given, its label, and whether it acts.
pub(crate) struct ToolButton {
    pub icon: fn(f32, Color) -> Element,
    /// `None` for the sidebar toggle, which Fork draws as an icon alone.
    pub label: Option<&'static str>,
    /// Assistive technology's name for the button, and what a test presses it by.
    pub name: &'static str,
    pub enabled: bool,
    pub tip: Tip,
    /// A count drawn at the icon's top right; none at zero.
    pub badge: usize,
    /// A small ⌄ beside the icon, above the label: Fork's Appearance, whose whole button opens
    /// its menu.
    pub icon_menu: bool,
    pub on_press: EventHandler<()>,
}

impl ToolButton {
    pub fn element(self) -> Element {
        let (icon_tone, label_tone) = if self.enabled {
            (tones::TEXT, tones::TOOLBAR_TEXT)
        } else {
            (tones::DISABLED_TEXT, tones::DISABLED_TEXT)
        };
        let enabled = self.enabled;
        let on_press = self.on_press.clone();
        let name = self.name;
        let icon =
            rect()
                .horizontal()
                .cross_align(Alignment::center())
                .child(
                    rect()
                        .width(Size::px(metrics::TOOLBAR_ICON))
                        .height(Size::px(metrics::TOOLBAR_ICON))
                        .child((self.icon)(metrics::TOOLBAR_ICON, icon_tone))
                        .maybe_child((self.badge > 0).then(|| badge(self.badge))),
                )
                .maybe_child(self.icon_menu.then(|| {
                    crate::style::icons::chevron_down(metrics::TOOLBAR_CHEVRON, label_tone)
                }));
        let button = Hovered {
            enabled,
            child: rect()
                .a11y_role(AccessibilityRole::Button)
                .a11y_builder(move |node| node.set_value(name))
                .min_width(Size::px(metrics::TOOLBAR_BUTTON_MIN_WIDTH))
                .height(Size::fill())
                .padding(Gaps::new(
                    3.,
                    metrics::TOOLBAR_BUTTON_PADDING,
                    2.,
                    metrics::TOOLBAR_BUTTON_PADDING,
                ))
                .cross_align(Alignment::center())
                .main_align(Alignment::center())
                .spacing(2.)
                .on_press(move |_| {
                    if enabled {
                        on_press.call(());
                    }
                })
                .child(icon)
                .maybe_child(self.label.map(|text| {
                    label()
                        .text(text)
                        .max_lines(1)
                        .font_size(metrics::TOOLBAR_LABEL)
                        .color(label_tone)
                }))
                .into(),
        };
        with_tip(self.tip, button.into())
    }
}

/// The ⌄ beside Stash and Branch: a press of its own, greyed with its button.
pub(crate) fn chevron(
    name: &'static str,
    enabled: bool,
    tip: Tip,
    on_press: EventHandler<()>,
) -> Element {
    let tone = if enabled {
        tones::TOOLBAR_TEXT
    } else {
        tones::DISABLED_TEXT
    };
    let button = Hovered {
        enabled,
        child: rect()
            .a11y_role(AccessibilityRole::Button)
            .a11y_builder(move |node| node.set_value(name))
            .height(Size::fill())
            .padding(Gaps::new(0., 2., 4., 2.))
            .main_align(Alignment::End)
            .on_press(move |_| {
                if enabled {
                    on_press.call(());
                }
            })
            .child(crate::style::icons::chevron_down(
                metrics::TOOLBAR_CHEVRON,
                tone,
            ))
            .into(),
    };
    with_tip(tip, button.into())
}

fn with_tip(tip: Tip, child: Element) -> Element {
    match tip {
        Tip::None => child,
        Tip::Sentence(sentence) => TooltipContainer::new(Tooltip::new_text(sentence))
            .child(child)
            .into(),
        Tip::Named { name, chord } => TooltipContainer::new(
            Tooltip::new()
                .child(label().text(name).max_lines(1))
                .maybe_child(chord.map(|chord| label().text(chord).max_lines(1))),
        )
        .child(child)
        .into(),
    }
}

/// A count at the icon's top right: a grey circle with white figures.
fn badge(count: usize) -> Rect {
    rect()
        .position(
            Position::new_absolute()
                .top(-metrics::TOOLBAR_BADGE / 3.)
                .left(metrics::TOOLBAR_ICON - metrics::TOOLBAR_BADGE / 2.),
        )
        .min_width(Size::px(metrics::TOOLBAR_BADGE))
        .height(Size::px(metrics::TOOLBAR_BADGE))
        .padding(Gaps::new(0., 3., 0., 3.))
        .corner_radius(metrics::TOOLBAR_BADGE / 2.)
        .background(tones::BADGE)
        .center()
        .child(
            label()
                .text(count.to_string())
                .max_lines(1)
                .font_size(metrics::TOOLBAR_BADGE_TEXT)
                .color(tones::BADGE_TEXT),
        )
}

/// A button's light box under the pointer, Fork's hover, drawn only while it can act.
#[derive(PartialEq)]
struct Hovered {
    enabled: bool,
    child: Element,
}

impl Component for Hovered {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let lit = self.enabled && *hovered.read();
        rect()
            .height(Size::fill())
            .corner_radius(4.)
            .maybe(lit, |el| el.background(tones::MENU_HOVER))
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set_if_modified(false))
            .child(self.child.clone())
    }
}
