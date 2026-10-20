//! The bar over Local Changes' two lists, as Fork for Windows draws it
//! (`docs/research/ui-review/fork-window-survey-2026-10-16.md` §7, uv12a;
//! `fork-window-survey-2-2026-10-16.md` §1c, uw6a): the filter — a magnifier and the field, no
//! placeholder — and at its right Stage All's double chevron, the eye, and View Mode, whose menu
//! offers the list alone, the one mode Cairn draws.
//!
//! **What it reports.** Stage All as the lists' own intent on Unstaged
//! (`ListIntent::Act(Unstaged, StageOrUnstageAll)`), greyed while Unstaged shows no row; and
//! nothing for View Mode's one item, already the mode drawn. The eye is greyed, saying what
//! brings it: Fork's opens Quick Look, a window over Fork's own drawing the selected file's diff
//! side by side (`fork-local-changes-2026-10-20.md`, lc9), which Cairn has not built (#35).

use freya::prelude::*;

use crate::accelerators::Action;
use crate::bar_button::BarButton;
use crate::local_changes::list::ListIntent;
use crate::local_changes::menu::STAGE_ALL_CAPTION;
use crate::menu::{Item, context_menu};
use crate::style::{icons, metrics, tones};
use crate::text_field::text_field;
use cairn_model::ChangeList;

/// The eye's name: Fork's tooltip without the key Fork binds to it, the space bar.
pub const SIDE_BY_SIDE_TOOLTIP: &str = "Show diff side-by-side";
/// Why the eye is greyed.
pub const COMES_WITH_QUICK_LOOK: &str = "Comes with Quick Look.";
/// View Mode's tooltip and name, Fork's.
pub const VIEW_MODE_TOOLTIP: &str = "View Mode";
/// View Mode's one item, ticked: Fork's words for the list.
pub const VIEW_AS_LIST_CAPTION: &str = "View as List";

/// What the bar draws and where its presses go.
pub(crate) struct ListsBar {
    pub filter: Writable<String>,
    /// Whether Unstaged shows a row, so Stage All has something to take.
    pub stage_all: bool,
    pub on_intent: EventHandler<ListIntent>,
}

impl ListsBar {
    pub fn element(self) -> Element {
        let on_intent = self.on_intent;
        let stage_all = BarButton {
            glyph: icons::stage_all,
            name: STAGE_ALL_CAPTION,
            tooltip: STAGE_ALL_CAPTION.to_owned(),
            toggle: None,
            enabled: self.stage_all,
            on_press: EventHandler::new(move |()| {
                on_intent.call(ListIntent::Act(
                    ChangeList::Unstaged,
                    Action::StageOrUnstageAll,
                ));
            }),
        };
        let quick_look = BarButton {
            glyph: icons::eye,
            name: SIDE_BY_SIDE_TOOLTIP,
            tooltip: COMES_WITH_QUICK_LOOK.to_owned(),
            toggle: None,
            enabled: false,
            on_press: EventHandler::new(|()| {}),
        };
        let view_mode = BarButton {
            glyph: icons::view_mode,
            name: VIEW_MODE_TOOLTIP,
            tooltip: VIEW_MODE_TOOLTIP.to_owned(),
            toggle: None,
            enabled: true,
            on_press: EventHandler::new(|()| {
                // The list is the mode drawn, so choosing it changes nothing.
                ContextMenu::open(context_menu(
                    vec![vec![Item::new(VIEW_AS_LIST_CAPTION, ()).ticked(true)]],
                    |()| {},
                ));
            }),
        };
        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(metrics::LISTS_BAR))
            .cross_align(Alignment::Center)
            .padding(Gaps::new(0., 4., 0., 0.))
            .spacing(metrics::DIFF_BAR_GAP)
            .background(tones::HISTORY)
            .border(
                Border::new()
                    .fill(tones::RULE)
                    .width(BorderWidth {
                        top: 0.,
                        right: 0.,
                        bottom: 1.,
                        left: 0.,
                    })
                    .alignment(BorderAlignment::Inner),
            )
            .child(
                rect().width(Size::flex(1.)).child(
                    text_field(self.filter)
                        .flat()
                        .background(tones::HISTORY)
                        .hover_background(tones::HISTORY)
                        .focus_background(tones::HISTORY)
                        .leading(icons::magnifier(
                            metrics::DIFF_BAR_ICON,
                            tones::DIFF_BAR_ICON,
                        ))
                        .height(Size::px(metrics::LISTS_BAR - 1.))
                        .theme_layout(InputLayoutThemePartial {
                            inner_margin: Some(Preference::Specific(Gaps::new(0., 6., 0., 6.))),
                            ..Default::default()
                        })
                        .width(Size::fill()),
                ),
            )
            .child(stage_all)
            .child(quick_look)
            .child(view_mode)
            .into()
    }
}
