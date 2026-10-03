//! The bar over a diff (PRD R6.2, R6.3, R6.7), laid out as Fork lays it out (Finding 10):
//! previous and next change on the left, the path in the middle with its file name
//! emphasised, the view's toggles on the right. No statistics and no file actions.
//!
//! **The buttons are Fork's** (Finding 10; the user's decision, 2026-10-03): a glyph each
//! (`toggle_glyphs`), drawn in the accent colour — the theme's `text_highlight` — while its
//! toggle is on, never filled; and each button's name, which assistive technology reads and
//! its tooltip shows, is Fork's tooltip where one is recorded ([`IGNORE_WHITESPACE_LABEL`],
//! [`FEWER_LINES_LABEL`], [`MORE_LINES_LABEL`], [`ENTIRE_FILE_LABEL`]) and Cairn's own
//! otherwise ([`PREVIOUS_CHANGE_LABEL`], [`NEXT_CHANGE_LABEL`], [`SIDE_BY_SIDE_LABEL`]), so
//! no meaning rests on the glyph alone.
//!
//! Every toggle is a button and none has a chord — Fork binds none (user decision,
//! `docs/research/diff-engine/fork-shortcuts.md`); previous and next change also answer the
//! detail pane's chords. Side-by-side is drawn disabled here and arrives in phase 07. With
//! whitespace ignored the bar says when that hides a change — only then (R6.7), a deliberate
//! deviation from Fork, which hides them silently.

use cairn_model::{ChangeStatus, ChangedFile};
use freya::prelude::*;

use crate::diff_palette::{DIFF_FONT_FAMILY, DIFF_FONT_SIZE, HEADER_BAR};
use crate::diff_settings::DiffSettings;
use crate::toggle_glyphs::Glyph;

/// The bar's height: the detail pane strip's. Fork's own bar height is not established by
/// the research; Cairn's is chosen to line up with the strip above it.
pub const DIFF_HEADER_HEIGHT: f32 = 30.0;

/// Previous change's name and tooltip. Fork's tooltip is not recorded; Cairn's own.
pub const PREVIOUS_CHANGE_LABEL: &str = "Previous change";
/// Next change's name and tooltip. Fork's tooltip is not recorded; Cairn's own.
pub const NEXT_CHANGE_LABEL: &str = "Next change";
/// Fork's tooltip (Finding 10: the vendor's GIF, Mac 1.0.69).
pub const IGNORE_WHITESPACE_LABEL: &str = "Ignore whitespaces";
/// Fork's tooltip (Finding 10: named in Tracker #911 and TrackerWin #1046).
pub const FEWER_LINES_LABEL: &str = "Decrease number of visible lines";
/// Fork's tooltip (Finding 10: the vendor's GIF, Mac 1.0.69).
pub const MORE_LINES_LABEL: &str = "Increase number of visible lines";
/// Fork's tooltip (Finding 10: the vendor's screenshot, TrackerWin #792).
pub const ENTIRE_FILE_LABEL: &str = "Show entire file";
/// Side-by-side's name and tooltip. Fork's header toggle's tooltip is not recorded (its quick
/// look's eye button reads "Show diff side-by-side (spacebar)", another control); Cairn's
/// own, in the words Fork's release notes use for the view.
pub const SIDE_BY_SIDE_LABEL: &str = "Side-by-side diff";
/// Said in the bar while ignoring whitespace hides a change that is really there.
pub const HIDDEN_CHANGES_NOTICE: &str = "Ignoring whitespace hides some changes";

const NOTICE_FONT_SIZE: f32 = 12.0;

/// What a press in the bar asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderAction {
    PreviousChange,
    NextChange,
    IgnoreWhitespace,
    FewerLines,
    MoreLines,
    EntireFile,
}

/// The bar over one file's diff.
pub struct DiffHeader {
    file: ChangedFile,
    settings: DiffSettings,
    hiding: bool,
    on_action: EventHandler<HeaderAction>,
    key: DiffKey,
}

impl DiffHeader {
    pub fn new(file: ChangedFile, settings: DiffSettings) -> Self {
        Self {
            file,
            settings,
            hiding: false,
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
        let colours = get_theme_or_default();
        let (primary, muted, warning, accent, disabled) = {
            let sheet = colours.read();
            let sheet = sheet.colors();
            (
                sheet.text_primary,
                sheet.text_placeholder,
                sheet.warning,
                sheet.text_highlight,
                sheet.disabled,
            )
        };
        let settings = self.settings;
        // One button of the bar: its glyph in the accent while its toggle is on, its name for
        // assistive technology and in its tooltip. A disabled button reports nothing.
        let button = |glyph: Glyph,
                      name: &'static str,
                      active: bool,
                      enabled: bool,
                      which: Option<HeaderAction>| {
            let colour = if !enabled {
                disabled
            } else if active {
                accent
            } else {
                primary
            };
            let on_action = self.on_action.clone();
            TooltipContainer::new(Tooltip::new_text(name)).child(
                Button::new()
                    .compact()
                    .enabled(enabled)
                    .on_press(move |_: Event<PressEventData>| {
                        if let Some(which) = which {
                            on_action.call(which);
                        }
                    })
                    // The button takes its name from its first child's value.
                    .child(
                        glyph
                            .draw(colour)
                            .a11y_builder(move |node| node.set_value(name)),
                    ),
            )
        };

        let path = self.file.new_path.display().into_owned();
        let (directory, name) = split_path(&path);
        let shown_path = rect()
            .horizontal()
            .width(Size::flex(1.))
            .cross_align(Alignment::Center)
            .padding(Gaps::new(0., 8., 0., 8.))
            .child(
                paragraph()
                    .max_lines(1)
                    .font_family(DIFF_FONT_FAMILY)
                    .font_size(DIFF_FONT_SIZE)
                    .text_overflow(TextOverflow::Ellipsis)
                    .span(Span::new(directory.to_owned()).color(muted))
                    .span(Span::new(name.to_owned()).color(primary)),
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

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(DIFF_HEADER_HEIGHT))
            .cross_align(Alignment::Center)
            .padding(Gaps::new(0., 8., 0., 8.))
            .spacing(4.)
            .background(HEADER_BAR)
            .child(button(
                Glyph::PreviousChange,
                PREVIOUS_CHANGE_LABEL,
                false,
                true,
                Some(HeaderAction::PreviousChange),
            ))
            .child(button(
                Glyph::NextChange,
                NEXT_CHANGE_LABEL,
                false,
                true,
                Some(HeaderAction::NextChange),
            ))
            .child(shown_path)
            .maybe_child(self.hiding.then(|| {
                label()
                    .text(HIDDEN_CHANGES_NOTICE)
                    .max_lines(1)
                    .font_size(NOTICE_FONT_SIZE)
                    .color(warning)
            }))
            .child(button(
                Glyph::IgnoreWhitespace,
                IGNORE_WHITESPACE_LABEL,
                settings.ignore_whitespace(),
                true,
                Some(HeaderAction::IgnoreWhitespace),
            ))
            .child(button(
                Glyph::FewerLines,
                FEWER_LINES_LABEL,
                false,
                settings.can_show_fewer_lines(),
                Some(HeaderAction::FewerLines),
            ))
            .child(button(
                Glyph::MoreLines,
                MORE_LINES_LABEL,
                false,
                settings.can_show_more_lines(),
                Some(HeaderAction::MoreLines),
            ))
            .child(button(
                Glyph::EntireFile,
                ENTIRE_FILE_LABEL,
                settings.entire_file(),
                true,
                Some(HeaderAction::EntireFile),
            ))
            // Side-by-side lands in phase 07.
            .child(button(
                Glyph::SideBySide,
                SIDE_BY_SIDE_LABEL,
                false,
                false,
                None,
            ))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_drawn_as_its_directory_and_its_name() {
        assert_eq!(split_path("src/ui/view.rs"), ("src/ui/", "view.rs"));
        assert_eq!(split_path("README"), ("", "README"));
    }
}
