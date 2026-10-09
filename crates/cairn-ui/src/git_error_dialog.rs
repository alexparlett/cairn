//! Fork's `Git Error` dialog for a commit that failed (staging-and-commit R10.5): the command
//! and git's output — a hook's lines among it, ANSI sequences already stripped by the window —
//! with `Skip pre-commit hooks and commit` offered only where git would run a `pre-commit` or
//! `commit-msg` hook (R6.6), and `Close`. For an amend the skip is the commit box's
//! `AmendSkip`, which draws the prompt the amend was confirmed with and builds the skipped
//! amend's token from it; this file builds none. Escape and a press outside close it; nothing is the
//! default, as in Fork (Tracker #2173). The draft is the window's, untouched by either answer.
//!
//! The output can be as long as a hook makes it, so it is drawn through a virtualizing view —
//! one row per line, only the rows in view built — never a `ScrollView`, and opens at its end,
//! where a failing hook says why (Fork's Windows 2.15).

use std::rc::Rc;

use freya::prelude::*;

use cairn_model::{Confirmed, Consequence};

use crate::commit_box::{AmendSkip, SKIP_HOOKS_CAPTION};
use crate::diff_palette::DIFF_FONT_FAMILY;

/// Fork's words.
pub const GIT_ERROR_TITLE: &str = "Git Error";
pub const GIT_ERROR_TEXT: &str = "An unexpected error occurred while performing the git request.";
pub const ERROR_DETAILS: &str = "Error Details:";
pub const CLOSE_CAPTION: &str = "Close";

const DIALOG_WIDTH: f32 = 680.;
/// One line of output.
pub const OUTPUT_ROW_HEIGHT: f32 = 16.;
/// The output's box: room for some fifteen lines, the rest a scroll away.
const OUTPUT_HEIGHT: f32 = 240.;

pub struct GitErrorDialog {
    serial: u64,
    command: String,
    lines: Rc<Vec<String>>,
    skip: bool,
    on_skip: EventHandler<()>,
    /// A failed amend's skip: the consequence it was confirmed with, and where the token goes.
    skip_amend: Option<(Rc<Consequence>, EventHandler<Confirmed>)>,
    on_close: EventHandler<()>,
    key: DiffKey,
}

impl GitErrorDialog {
    /// The failure numbered `serial` — another failure is another dialog — of `command`, whose
    /// output is `lines`.
    pub fn new(serial: u64, command: impl Into<String>, lines: Rc<Vec<String>>) -> Self {
        Self {
            serial,
            command: command.into(),
            lines,
            skip: false,
            on_skip: EventHandler::new(|()| {}),
            skip_amend: None,
            on_close: EventHandler::new(|()| {}),
            key: DiffKey::None,
        }
    }

    /// Whether the skip is offered: only where a hook `--no-verify` skips exists (R10.5).
    pub fn skip(mut self, offered: bool) -> Self {
        self.skip = offered;
        self
    }

    /// The skip was pressed: the same commit again, its hooks skipped this once.
    pub fn on_skip(mut self, on_skip: impl Into<EventHandler<()>>) -> Self {
        self.on_skip = on_skip.into();
        self
    }

    /// A failed amend's skip, in place of the commit's (the user's decision of 2026-10-09):
    /// one press amends at once without hooks, under a token built from `consequence` — the one
    /// the amend was confirmed with — by the commit box's `AmendSkip`, which draws its prompt.
    /// Offered whatever [`Self::skip`] says.
    pub fn skip_amend(
        mut self,
        consequence: Rc<Consequence>,
        on_confirmed: impl Into<EventHandler<Confirmed>>,
    ) -> Self {
        self.skip_amend = Some((consequence, on_confirmed.into()));
        self
    }

    /// Close, Escape, or a press outside.
    pub fn on_close(mut self, on_close: impl Into<EventHandler<()>>) -> Self {
        self.on_close = on_close.into();
        self
    }
}

impl PartialEq for GitErrorDialog {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
            && self.skip == other.skip
            && self.skip_amend.is_some() == other.skip_amend.is_some()
            && self.key == other.key
    }
}

impl std::fmt::Debug for GitErrorDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitErrorDialog")
            .field("serial", &self.serial)
            .field("command", &self.command)
            .field("lines", &self.lines.len())
            .field("skip", &self.skip)
            .finish_non_exhaustive()
    }
}

impl KeyExt for GitErrorDialog {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for GitErrorDialog {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        // Opened at the end of the output, where a failing hook says why.
        let controller = use_scroll_controller(|| ScrollConfig {
            default_vertical_position: ScrollPosition::End,
            ..ScrollConfig::default()
        });
        let rows = self.lines.len();
        let answered = use_state(|| false);
        let once = move |then: EventHandler<()>| {
            move || {
                let mut answered = answered;
                if !*answered.peek() {
                    answered.set(true);
                    then.call(());
                }
            }
        };
        let close = once(self.on_close.clone());
        let skip = once(self.on_skip.clone());

        let output = VirtualScrollView::new_with_data_controlled(
            OutputLines(self.lines.clone()),
            output_row,
            controller,
        )
        .length(rows)
        .item_size(OUTPUT_ROW_HEIGHT)
        .width(Size::fill())
        .height(Size::px(OUTPUT_HEIGHT));

        let mut buttons = PopupButtons::new();
        let amend_skip = self.skip_amend.as_ref().map(|(consequence, on_confirmed)| {
            let close = self.on_close.clone();
            let on_confirmed = on_confirmed.clone();
            AmendSkip::new(self.serial, consequence.clone()).on_confirmed(
                move |token: Confirmed| {
                    on_confirmed.call(token);
                    close.call(());
                },
            )
        });
        if self.skip && amend_skip.is_none() {
            buttons = buttons.child(
                Button::new()
                    .on_press(move |_| skip())
                    .child(SKIP_HOOKS_CAPTION),
            );
        }
        let closing = close.clone();
        buttons = buttons.child(
            Button::new()
                .on_press(move |_| closing())
                .child(CLOSE_CAPTION),
        );

        Popup::new()
            .width(Size::px(DIALOG_WIDTH))
            .on_close_request(move |()| close())
            .child(
                rect()
                    .width(Size::fill())
                    .a11y_modal(true)
                    .a11y_role(AccessibilityRole::AlertDialog)
                    .child(PopupTitle::new(GIT_ERROR_TITLE.to_owned()))
                    .child(
                        PopupContent::new()
                            .child(label().text(GIT_ERROR_TEXT).font_size(14.))
                            .child(
                                label()
                                    .text(ERROR_DETAILS)
                                    .font_size(12.)
                                    .color(colours.text_secondary),
                            )
                            .child(
                                label()
                                    .text(self.command.clone())
                                    .font_family(DIFF_FONT_FAMILY)
                                    .font_size(12.)
                                    .width(Size::fill()),
                            )
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .border(Border::new().fill(colours.border).width(1.))
                                    .font_family(DIFF_FONT_FAMILY)
                                    .font_size(12.)
                                    .child(output),
                            )
                            .maybe_child(amend_skip),
                    )
                    .child(buttons),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(DiffKey::U64(self.serial))
    }
}

/// The output, compared by identity: one failure's lines never change.
#[derive(Clone)]
struct OutputLines(Rc<Vec<String>>);

impl PartialEq for OutputLines {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

fn output_row(item: VirtualItem, lines: &OutputLines) -> Element {
    rect()
        .key(item.index)
        .min_width(Size::fill())
        .height(Size::px(item.size))
        .padding(Gaps::new(0., 6., 0., 6.))
        .child(
            label()
                .text(lines.0.get(item.index).cloned().unwrap_or_default())
                .max_lines(1),
        )
        .into()
}
