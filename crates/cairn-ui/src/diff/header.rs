//! The bar over a diff, laid out as Fork lays it out (`docs/research/ui-review/
//! fork-window-survey-2026-10-16.md` §6, `fork-window-survey-2-2026-10-16.md` §1b): Go to
//! previous change and Go to next change on the left, the file's document glyph and its path
//! centred with its folder in the secondary tone, and the view's buttons on the right — Ignore
//! whitespaces, Decrease and Increase number of visible lines, Show entire file and Side by side
//! diff. Fork's Show hidden symbols and Word wrap are not drawn (#33, #34). No statistics and no
//! file actions.
//!
//! **The buttons are Fork's**: a Lucide glyph each (`style::icons`), drawn in the accent while
//! its toggle is on and never filled, and Fork's tooltip, verbatim — its words, and the chord the
//! accelerator table lists for the button's action after a comma, as Fork writes "Go to previous
//! change, Ctrl+↑" ([`bar_tooltip`]). Each button's name, which assistive technology reads, is
//! the tooltip's words, and a toggle says whether it is on, so no meaning rests on the glyph.
//!
//! Side-by-side is lit while the shared setting is on. With whitespace ignored the bar says
//! when that hides a change — only then, a deliberate deviation from Fork, which hides them
//! silently.

use cairn_model::{ChangeStatus, ChangedFile};
use freya::prelude::*;

use crate::accelerators::{self, Action, Os};
use crate::diff::palette::HEADER_BAR;
use crate::diff::settings::DiffSettings;
use crate::style::{icons, metrics, tones};

/// The bar's height: Fork's 17 px bar and the rule under it (`fork-window-survey-2-2026-10-16.md`
/// §1b, uw5).
pub const DIFF_HEADER_HEIGHT: f32 = metrics::DIFF_BAR;

/// Fork's tooltip for previous change, before its chord.
pub const PREVIOUS_CHANGE_LABEL: &str = "Go to previous change";
/// Fork's tooltip for next change, before its chord.
pub const NEXT_CHANGE_LABEL: &str = "Go to next change";
/// Fork's tooltip.
pub const IGNORE_WHITESPACE_LABEL: &str = "Ignore whitespaces";
/// Fork's tooltip.
pub const FEWER_LINES_LABEL: &str = "Decrease number of visible lines";
/// Fork's tooltip.
pub const MORE_LINES_LABEL: &str = "Increase number of visible lines";
/// Fork's tooltip.
pub const ENTIRE_FILE_LABEL: &str = "Show entire file";
/// Fork's tooltip.
pub const SIDE_BY_SIDE_LABEL: &str = "Side by side diff";
/// Said in the bar while ignoring whitespace hides a change that is really there.
pub const HIDDEN_CHANGES_NOTICE: &str = "Ignoring whitespace hides some changes";

/// What a press in the bar asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    PreviousChange,
    NextChange,
    IgnoreWhitespace,
    FewerLines,
    MoreLines,
    EntireFile,
    SideBySide,
}

impl HeaderAction {
    /// Every button of the bar, in Fork's order.
    pub const ALL: [HeaderAction; 7] = [
        HeaderAction::PreviousChange,
        HeaderAction::NextChange,
        HeaderAction::IgnoreWhitespace,
        HeaderAction::FewerLines,
        HeaderAction::MoreLines,
        HeaderAction::EntireFile,
        HeaderAction::SideBySide,
    ];

    /// The accelerator table's action the button does, which its press is acted on as.
    pub fn action(self) -> Action {
        match self {
            Self::PreviousChange => Action::PreviousChange,
            Self::NextChange => Action::NextChange,
            Self::IgnoreWhitespace => Action::ToggleIgnoreWhitespace,
            Self::FewerLines => Action::FewerLines,
            Self::MoreLines => Action::MoreLines,
            Self::EntireFile => Action::EntireFile,
            Self::SideBySide => Action::ToggleSideBySide,
        }
    }

    /// The button's name: Fork's tooltip without its chord.
    pub fn label(self) -> &'static str {
        match self {
            Self::PreviousChange => PREVIOUS_CHANGE_LABEL,
            Self::NextChange => NEXT_CHANGE_LABEL,
            Self::IgnoreWhitespace => IGNORE_WHITESPACE_LABEL,
            Self::FewerLines => FEWER_LINES_LABEL,
            Self::MoreLines => MORE_LINES_LABEL,
            Self::EntireFile => ENTIRE_FILE_LABEL,
            Self::SideBySide => SIDE_BY_SIDE_LABEL,
        }
    }

    fn glyph(self) -> fn(f32, Color) -> Element {
        match self {
            Self::PreviousChange => icons::chevron_up,
            Self::NextChange => icons::chevron_down,
            Self::IgnoreWhitespace => icons::whitespace,
            Self::FewerLines => icons::fewer_lines,
            Self::MoreLines => icons::more_lines,
            Self::EntireFile => icons::entire_file,
            Self::SideBySide => icons::side_by_side,
        }
    }
}

/// The tooltip of `button` on `platform`, as Fork writes it: its words, then — where the
/// accelerator table lists a chord for its action — a comma and the chord.
pub fn bar_tooltip(button: HeaderAction, platform: Os) -> String {
    match accelerators::chord_name(button.action(), platform) {
        Some(chord) => format!("{}, {chord}", button.label()),
        None => button.label().to_owned(),
    }
}

/// The bar over one file's diff.
pub struct DiffHeader {
    file: ChangedFile,
    settings: DiffSettings,
    hiding: bool,
    exact: bool,
    said: Option<&'static str>,
    on_action: EventHandler<HeaderAction>,
    key: DiffKey,
}

impl DiffHeader {
    pub fn new(file: ChangedFile, settings: DiffSettings) -> Self {
        Self {
            file,
            settings,
            hiding: false,
            exact: false,
            said: None,
            on_action: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// Whether ignoring whitespace hides a change in the diff shown: the notice is drawn
    /// only when it does, never just because the toggle is on.
    pub fn hiding(mut self, hiding: bool) -> Self {
        self.hiding = hiding;
        self
    }

    /// The diff under the bar is always the exact one, whatever the shared setting says —
    /// Local Changes', where what is staged is what is drawn: ignore whitespace is drawn off
    /// and disabled, the shared setting left as it was, and no notice says whitespace hides a
    /// change.
    pub fn exact(mut self, exact: bool) -> Self {
        self.exact = exact;
        self
    }

    /// A line the bar says beside the path — Local Changes' "The file changed — line selection
    /// cleared." — for as long as the caller hands it.
    pub fn said(mut self, said: Option<&'static str>) -> Self {
        self.said = said;
        self
    }

    /// A button was pressed. Whether the press changes anything is the caller's to decide;
    /// a disabled button reports nothing.
    pub fn on_action(mut self, on_action: impl Into<EventHandler<HeaderAction>>) -> Self {
        self.on_action = on_action.into();
        self
    }
}

// Hand-written: `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for DiffHeader {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file
            && self.settings == other.settings
            && self.hiding == other.hiding
            && self.exact == other.exact
            && self.said == other.said
            && self.key == other.key
    }
}

impl std::fmt::Debug for DiffHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffHeader")
            .field("path", &self.file.new_path)
            .field("settings", &self.settings)
            .field("hiding", &self.hiding)
            .finish_non_exhaustive()
    }
}

impl KeyExt for DiffHeader {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// The directory and the file name of a path as bytes git stores, for drawing apart.
fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(slash) => path.split_at(slash + 1),
        None => ("", path),
    }
}

impl Component for DiffHeader {
    fn render(&self) -> impl IntoElement {
        // The warning keeps the theme's tone, which Fork Dark's table does not name.
        let warning = get_theme_or_default().read().colors().warning;
        let settings = self.settings;
        let exact = self.exact;
        // Whether each button is a toggle and whether it is on, and whether it can be pressed.
        let button = |which: HeaderAction| -> Element {
            let (toggle, enabled) = match which {
                HeaderAction::PreviousChange | HeaderAction::NextChange => (None, true),
                HeaderAction::IgnoreWhitespace => {
                    (Some(settings.ignore_whitespace() && !exact), !exact)
                }
                // Greyed only while the entire file is shown, as Fork's are (cm27): at one
                // line of context Decrease stays pressable and does nothing (cm38).
                HeaderAction::FewerLines | HeaderAction::MoreLines => {
                    (None, !settings.entire_file())
                }
                HeaderAction::EntireFile => (Some(settings.entire_file()), true),
                HeaderAction::SideBySide => (Some(settings.side_by_side()), true),
            };
            BarButton {
                which,
                toggle,
                enabled,
                on_action: self.on_action.clone(),
            }
            .into()
        };

        let path = self.file.new_path.display().into_owned();
        let (directory, name) = split_path(&path);
        let shown_path = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(4.)
            .child(icons::file(metrics::DIFF_BAR_ICON, tones::DIFF_BAR_ICON))
            .child(
                paragraph()
                    .max_lines(1)
                    .font_size(metrics::BODY)
                    .text_overflow(TextOverflow::Ellipsis)
                    .span(Span::new(directory.to_owned()).color(tones::DETAIL_LABEL))
                    .span(Span::new(name.to_owned()).color(tones::TEXT)),
            );
        let shown_path: Element = match self.file.status {
            ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => TooltipContainer::new(
                Tooltip::new_text(format!("{} → {}", self.file.old_path.display(), path)),
            )
            .child(shown_path)
            .into(),
            ChangeStatus::Added
            | ChangeStatus::Deleted
            | ChangeStatus::Modified
            | ChangeStatus::TypeChanged => shown_path.into(),
        };
        let said = |text: &'static str| {
            label()
                .text(text)
                .max_lines(1)
                .font_size(metrics::BODY)
                .color(warning)
        };

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(DIFF_HEADER_HEIGHT))
            .cross_align(Alignment::Center)
            .padding(Gaps::new(0., 4., 0., 4.))
            .background(HEADER_BAR)
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
            .spacing(metrics::DIFF_BAR_GAP)
            .child(button(HeaderAction::PreviousChange))
            .child(button(HeaderAction::NextChange))
            .child(
                rect()
                    .horizontal()
                    .width(Size::flex(1.))
                    .main_align(Alignment::Center)
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new(0., 8., 0., 8.))
                    .child(shown_path),
            )
            .maybe_child((self.hiding && !exact).then(|| said(HIDDEN_CHANGES_NOTICE)))
            .maybe_child(self.said.map(said))
            .children(HeaderAction::ALL.into_iter().skip(2).map(button))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// One button of the bar: its glyph in Fork's blue while its toggle is on and in the bar's
/// tone otherwise, a square behind it under the pointer, its name for assistive technology and
/// Fork's tooltip. A disabled button is greyed and reports nothing.
#[derive(Clone)]
struct BarButton {
    which: HeaderAction,
    toggle: Option<bool>,
    enabled: bool,
    on_action: EventHandler<HeaderAction>,
}

impl PartialEq for BarButton {
    fn eq(&self, other: &Self) -> bool {
        self.which == other.which && self.toggle == other.toggle && self.enabled == other.enabled
    }
}

impl Component for BarButton {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let (which, toggle, enabled) = (self.which, self.toggle, self.enabled);
        let colour = if !enabled {
            tones::DISABLED_TEXT
        } else if toggle == Some(true) {
            tones::DIFF_BAR_ON
        } else {
            tones::DIFF_BAR_ICON
        };
        let on_action = self.on_action.clone();
        let name = which.label();
        TooltipContainer::new(Tooltip::new_text(bar_tooltip(which, Os::current()))).child(
            rect()
                .a11y_id(a11y_id)
                // A Tab stop while it can be pressed, so the five buttons with no chord are
                // reached from the keyboard; Space or Enter presses it.
                .a11y_focusable(enabled)
                .a11y_role(AccessibilityRole::Button)
                .a11y_builder(move |node| {
                    node.set_value(name);
                    if let Some(on) = toggle {
                        node.set_toggled(Toggled::from(on));
                    }
                    if !enabled {
                        node.set_disabled();
                    }
                })
                .width(Size::px(metrics::DIFF_BAR_BUTTON))
                .height(Size::px(metrics::DIFF_BAR_BUTTON))
                .center()
                .maybe(enabled && *hovered.read(), |el| {
                    el.background(tones::DIFF_BAR_HOVER)
                })
                .maybe(focus() == Focus::Keyboard, |el| {
                    el.border(
                        Border::new()
                            .fill(tones::ACCENT)
                            .width(1.)
                            .alignment(BorderAlignment::Inner),
                    )
                })
                .on_pointer_enter(move |_| hovered.set(true))
                .on_pointer_leave(move |_| hovered.set_if_modified(false))
                .on_press(move |_| {
                    if enabled {
                        on_action.call(which);
                    }
                })
                .child(which.glyph()(metrics::DIFF_BAR_ICON, colour)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each button does the accelerator table's action for it, which its press is acted on as
    /// and its tooltip's chord is read for. Caught by: two buttons' actions swapped.
    #[test]
    fn each_button_does_its_own_action() {
        assert_eq!(
            HeaderAction::ALL.map(HeaderAction::action),
            [
                Action::PreviousChange,
                Action::NextChange,
                Action::ToggleIgnoreWhitespace,
                Action::FewerLines,
                Action::MoreLines,
                Action::EntireFile,
                Action::ToggleSideBySide,
            ]
        );
    }

    #[test]
    fn a_path_is_drawn_as_its_directory_and_its_name() {
        assert_eq!(split_path("src/ui/view.rs"), ("src/ui/", "view.rs"));
        assert_eq!(split_path("README"), ("", "README"));
    }
}
