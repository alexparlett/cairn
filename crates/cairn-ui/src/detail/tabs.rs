//! The detail pane's strip: its tabs, and the icon that hides the pane's body. Fork draws
//! the tabs as text with no count on either; the strip stays when the pane collapses, so
//! the pane can be opened again from where it went. The icon at the strip's right is Fork's
//! "Hide commit details" (`docs/research/ui-review/fork-window-survey-2-2026-10-16.md`, section
//! 2), lit in the accent while the pane is hidden, as Fork's is (section 3); Fork's pop-out icon
//! beside it is not drawn.

use freya::prelude::*;

use crate::style::{icons, metrics, tones};

/// Height of the strip, which is what a collapsed pane keeps: Fork's 31 px between two rules
/// (`style::metrics::DETAIL_STRIP`).
pub const DETAIL_STRIP_HEIGHT: f32 = metrics::DETAIL_STRIP;

/// The detail pane's tabs. Commit is the default; a File Tree tab is issue #31.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailTab {
    #[default]
    Commit,
    Changes,
}

impl DetailTab {
    pub const ALL: [DetailTab; 2] = [DetailTab::Commit, DetailTab::Changes];

    pub fn caption(self) -> &'static str {
        match self {
            Self::Commit => "Commit",
            Self::Changes => "Changes",
        }
    }
}

/// The tooltip of the icon that hides the pane's body and brings it back, Fork's words, and its
/// name for assistive technology.
pub const HIDE_DETAILS_TOOLTIP: &str = "Hide commit details";

pub struct DetailTabs {
    selected: DetailTab,
    collapsed: bool,
    unavailable: Option<DetailTab>,
    on_tab: EventHandler<DetailTab>,
    on_collapse: EventHandler<bool>,
    key: DiffKey,
}

impl DetailTabs {
    pub fn new(selected: DetailTab) -> Self {
        Self {
            selected,
            collapsed: false,
            unavailable: None,
            on_tab: EventHandler::new(|_| {}),
            on_collapse: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// A tab that cannot be shown now, drawn disabled and pressed for nothing: the Commit tab
    /// while two commits are selected, as Fork's Windows build disables it.
    pub fn unavailable(mut self, unavailable: Option<DetailTab>) -> Self {
        self.unavailable = unavailable;
        self
    }

    /// A tab was pressed, whether or not it is the one shown.
    pub fn on_tab(mut self, on_tab: impl Into<EventHandler<DetailTab>>) -> Self {
        self.on_tab = on_tab.into();
        self
    }

    /// The collapse control was pressed: `true` asks to collapse, `false` to open.
    pub fn on_collapse(mut self, on_collapse: impl Into<EventHandler<bool>>) -> Self {
        self.on_collapse = on_collapse.into();
        self
    }
}

// Hand-written: `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for DetailTabs {
    fn eq(&self, other: &Self) -> bool {
        self.selected == other.selected
            && self.collapsed == other.collapsed
            && self.unavailable == other.unavailable
            && self.key == other.key
    }
}

impl std::fmt::Debug for DetailTabs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetailTabs")
            .field("selected", &self.selected)
            .field("collapsed", &self.collapsed)
            .finish_non_exhaustive()
    }
}

impl KeyExt for DetailTabs {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for DetailTabs {
    fn render(&self) -> impl IntoElement {
        // Fork Dark's strip: the chosen tab's caption in its blue, the others in the body's.
        let (border, accent, surface) = (tones::RULE, tones::ACCENT, tones::HEADING_BAR);

        let collapsed = self.collapsed;
        let on_collapse = self.on_collapse.clone();

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(DETAIL_STRIP_HEIGHT))
            .cross_align(Alignment::End)
            .padding(Gaps::new(0., 8., 0., 0.))
            .background(surface)
            .border(
                Border::new()
                    .fill(border)
                    .width(BorderWidth {
                        top: 1.,
                        right: 0.,
                        bottom: 1.,
                        left: 0.,
                    })
                    .alignment(BorderAlignment::Inner),
            )
            .children(DetailTab::ALL.into_iter().map(|tab| -> Element {
                let shown = tab == self.selected && !collapsed;
                let available = self.unavailable != Some(tab);
                let on_tab = self.on_tab.clone();
                rect()
                    .key(tab.caption())
                    .height(Size::fill())
                    .min_width(Size::px(metrics::DETAIL_TAB))
                    .main_align(Alignment::Center)
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new(0., 12., 0., 12.))
                    .a11y_role(AccessibilityRole::Tab)
                    .maybe(shown, |el| {
                        el.border(
                            Border::new()
                                .fill(accent)
                                .width(BorderWidth {
                                    top: 0.,
                                    right: 0.,
                                    bottom: 2.,
                                    left: 0.,
                                })
                                .alignment(BorderAlignment::Inner),
                        )
                    })
                    .on_press(move |_| {
                        if available {
                            on_tab.call(tab);
                        }
                    })
                    .child(
                        label()
                            .text(tab.caption())
                            .max_lines(1)
                            .font_size(metrics::BODY)
                            .color(if !available {
                                tones::DISABLED_TEXT
                            } else if shown {
                                accent
                            } else {
                                tones::TEXT
                            }),
                    )
                    .into()
            }))
            .child(rect().width(Size::flex(1.)))
            .child(
                rect()
                    .height(Size::fill())
                    .main_align(Alignment::Center)
                    .child(
                        TooltipContainer::new(Tooltip::new_text(HIDE_DETAILS_TOOLTIP)).child(
                            rect()
                                .a11y_role(AccessibilityRole::Button)
                                .a11y_builder(|node| node.set_value(HIDE_DETAILS_TOOLTIP))
                                .padding(4.)
                                .on_press(move |_| on_collapse.call(!collapsed))
                                .child(icons::bottom_pane(
                                    metrics::STRIP_ICON,
                                    if collapsed { accent } else { tones::TEXT },
                                )),
                        ),
                    ),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
