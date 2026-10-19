//! About Cairn: the name, the version, the licence and the `git` found as the application
//! started, with Close — focused as it opens, so Return or Space closes it, as Escape does.
//! Fork's About box carries its name, version and copyright with only a ✕, and Escape leaves it
//! open (`docs/research/ui-review/fork-menu-toggles-2026-10-19.md`); Cairn's closes as every one
//! of its dialogs does.

use freya::prelude::*;

use super::answer_button::{AnswerColours, answer};
use crate::style::{metrics, tones};

/// The dialog's title: the product's name.
pub const ABOUT_TITLE: &str = "Cairn";
/// What the dialog says while no `git` has been found.
pub const NO_GIT_FOUND: &str = "No git has been found.";

const DIALOG_WIDTH: f32 = 400.;

/// "Version 0.1.0", as Fork's About writes its own.
pub fn version_line(version: &str) -> String {
    format!("Version {version}")
}

/// The licence Cairn is offered under.
pub fn licence_line(licence: &str) -> String {
    format!("Licensed under {licence}")
}

/// About Cairn: `version`, `licence`, and `git` — the `git` found, as the window was told of it,
/// or `None` while none has been.
pub struct AboutDialog {
    version: String,
    licence: String,
    git: Option<String>,
    on_close: EventHandler<()>,
    key: DiffKey,
}

impl AboutDialog {
    pub fn new(
        version: impl Into<String>,
        licence: impl Into<String>,
        git: Option<String>,
    ) -> Self {
        Self {
            version: version.into(),
            licence: licence.into(),
            git,
            on_close: EventHandler::new(|()| {}),
            key: DiffKey::None,
        }
    }

    pub fn on_close(mut self, on_close: impl Into<EventHandler<()>>) -> Self {
        self.on_close = on_close.into();
        self
    }
}

// By what it draws: `EventHandler` never compares equal.
impl PartialEq for AboutDialog {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.licence == other.licence
            && self.git == other.git
            && self.key == other.key
    }
}

impl std::fmt::Debug for AboutDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AboutDialog")
            .field("version", &self.version)
            .field("git", &self.git)
            .finish_non_exhaustive()
    }
}

impl KeyExt for AboutDialog {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for AboutDialog {
    fn render(&self) -> impl IntoElement {
        let close_id = use_a11y();
        let closing = self.on_close.clone();
        let close = answer(
            0,
            close_id,
            "Close".to_owned(),
            true,
            &AnswerColours::of_theme(),
            move || closing.call(()),
        );
        let on_close = self.on_close.clone();
        let line = |text: String| {
            label()
                .text(text)
                .width(Size::fill())
                .font_size(metrics::BODY)
        };
        Popup::new()
            .width(Size::px(DIALOG_WIDTH))
            .on_close_request(move |_| on_close.call(()))
            .child(
                PopupContent::new()
                    .child(
                        label()
                            .text(ABOUT_TITLE)
                            .font_size(metrics::HEADING)
                            .font_weight(FontWeight::BOLD)
                            .color(tones::TEXT),
                    )
                    .child(line(version_line(&self.version)).color(tones::TEXT))
                    .child(line(licence_line(&self.licence)).color(tones::DETAIL_LABEL))
                    .child(
                        line(self.git.clone().unwrap_or_else(|| NO_GIT_FOUND.to_owned()))
                            .color(tones::DETAIL_LABEL),
                    ),
            )
            .child(PopupButtons::new().child(close))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
