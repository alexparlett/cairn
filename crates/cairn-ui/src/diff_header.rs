//! The bar over a diff (PRD R6.2, R6.3, R6.7), laid out as Fork lays it out (Finding 10):
//! previous and next change on the left, the path in the middle with its file name
//! emphasised, the view's toggles on the right. No statistics and no file actions.
//!
//! Every toggle is a button and none has a chord — Fork binds none (user decision,
//! `docs/research/diff-engine/fork-shortcuts.md`); previous and next change also answer the
//! detail pane's chords. Side-by-side is drawn disabled here and arrives in phase 07. With
//! whitespace ignored the bar says when that hides a change — only then (R6.7), a deliberate
//! deviation from Fork, which hides them silently.

use cairn_model::{ChangeStatus, ChangedFile};
use freya::prelude::*;

use crate::diff_palette::{DIFF_FONT_FAMILY, HEADER_BAR};
use crate::diff_settings::DiffSettings;

/// The bar's height: the detail pane strip's.
pub const DIFF_HEADER_HEIGHT: f32 = 30.0;

pub const PREVIOUS_CHANGE_CAPTION: &str = "↑";
pub const NEXT_CHANGE_CAPTION: &str = "↓";
pub const IGNORE_WHITESPACE_CAPTION: &str = "Ignore whitespace";
pub const FEWER_LINES_CAPTION: &str = "Fewer lines";
pub const MORE_LINES_CAPTION: &str = "More lines";
pub const ENTIRE_FILE_CAPTION: &str = "Entire file";
pub const SIDE_BY_SIDE_CAPTION: &str = "Side-by-side";
/// Said in the bar while ignoring whitespace hides a change that is really there.
pub const HIDDEN_CHANGES_NOTICE: &str = "Ignoring whitespace hides some changes";

const PATH_FONT_SIZE: f32 = 12.0;
const CAPTION_FONT_SIZE: f32 = 12.0;

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
        let (primary, muted, warning) = {
            let sheet = colours.read();
            let sheet = sheet.colors();
            (sheet.text_primary, sheet.text_placeholder, sheet.warning)
        };
        let settings = self.settings;
        let action = |which: HeaderAction| {
            let on_action = self.on_action.clone();
            move |_: Event<PressEventData>| on_action.call(which)
        };
        let toggle = |caption: &'static str, active: bool, enabled: bool, which: HeaderAction| {
            let button = Button::new()
                .compact()
                .enabled(enabled)
                .on_press(action(which))
                .child(label().text(caption).font_size(CAPTION_FONT_SIZE));
            if active { button.filled() } else { button }
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
                    .font_size(PATH_FONT_SIZE)
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
            .child(
                TooltipContainer::new(Tooltip::new_text("Previous change")).child(toggle(
                    PREVIOUS_CHANGE_CAPTION,
                    false,
                    true,
                    HeaderAction::PreviousChange,
                )),
            )
            .child(
                TooltipContainer::new(Tooltip::new_text("Next change")).child(toggle(
                    NEXT_CHANGE_CAPTION,
                    false,
                    true,
                    HeaderAction::NextChange,
                )),
            )
            .child(shown_path)
            .maybe_child(self.hiding.then(|| {
                label()
                    .text(HIDDEN_CHANGES_NOTICE)
                    .max_lines(1)
                    .font_size(CAPTION_FONT_SIZE)
                    .color(warning)
            }))
            .child(toggle(
                IGNORE_WHITESPACE_CAPTION,
                settings.ignore_whitespace(),
                true,
                HeaderAction::IgnoreWhitespace,
            ))
            .child(toggle(
                FEWER_LINES_CAPTION,
                false,
                settings.can_show_fewer_lines(),
                HeaderAction::FewerLines,
            ))
            .child(toggle(
                MORE_LINES_CAPTION,
                false,
                settings.can_show_more_lines(),
                HeaderAction::MoreLines,
            ))
            .child(toggle(
                ENTIRE_FILE_CAPTION,
                settings.entire_file(),
                true,
                HeaderAction::EntireFile,
            ))
            .child(
                Button::new().compact().enabled(false).child(
                    label()
                        .text(SIDE_BY_SIDE_CAPTION)
                        .font_size(CAPTION_FONT_SIZE),
                ),
            )
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
