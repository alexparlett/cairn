//! Fork's `Git Error` dialog, and the same dialog titled "Couldn't <name>" for a refusal found
//! while an operation ran — the one home of every ending that is not a success or a cancel.
//!
//! A git failure draws Fork's title and sentence (or the ending's own, for a commit git made
//! and then failed), `Error Details:`, the command and git's output — a hook's lines among it,
//! ANSI sequences already stripped by the window. A refusal draws its one sentence and no Error
//! Details, since no git failed; a discard that left files as they were says so in one
//! sentence, the files behind Show files. The footer, in the platform's order, holds Close and,
//! where they are offered, `Skip pre-commit hooks and commit` (a failed commit or amend) and
//! `Remove index.lock…` (a write git refused for the lock), one button component for all
//! ([`crate::dialogs::answer_button`]), focus on Close so a Return pressed by habit never skips
//! the hooks or removes a lock. Each answers once; what it runs is the window's. No prompt text
//! is drawn here, and no token is built. Escape and a press outside close it, as in Fork
//! (Tracker #2173).
//!
//! The output and the files can be as long as git makes them, so each is drawn through a
//! virtualizing view — one row per line, only the rows in view built — never a `ScrollView`; the
//! output opens at its end, where a failing hook says why, as Fork for Windows opens it.

use std::rc::Rc;

use freya::prelude::*;

use crate::accelerators::Os;
use crate::dialogs::answer_button::{AnswerColours, answer};
use crate::dialogs::button_order::ordered;

use crate::commit_box::SKIP_HOOKS_CAPTION;
use crate::diff::palette::DIFF_FONT_FAMILY;
use crate::style::{metrics, tones};

/// Fork's words.
pub const GIT_ERROR_TITLE: &str = "Git Error";
pub const GIT_ERROR_TEXT: &str = "An unexpected error occurred while performing the git request.";
pub const ERROR_DETAILS: &str = "Error Details:";
pub const CLOSE_CAPTION: &str = "Close";
/// The lock's removal offered beside Close where git could not create `index.lock`, as Fork
/// offers it, and on the leftover lock's banner.
pub const REMOVE_LOCK_CAPTION: &str = "Remove index.lock…";

/// The title of a refusal found while `name` ran: "Couldn't Discard Changes".
pub fn couldnt_title(name: &str) -> String {
    format!("Couldn't {name}")
}

const DIALOG_WIDTH: f32 = 680.;
/// One line of output.
pub const OUTPUT_ROW_HEIGHT: f32 = 16.;
/// The output's box: room for some fifteen lines, the rest a scroll away.
const OUTPUT_HEIGHT: f32 = 240.;
/// Show files' box: at most this many rows tall, the rest a scroll away.
const FILES_SHOWN: usize = 10;

pub struct GitErrorDialog {
    serial: u64,
    title: String,
    sentence: String,
    /// The command and git's output, under `Error Details:`: a git failure's alone.
    details: Option<(String, Rc<Vec<String>>)>,
    /// The files a partly done discard left as they were, behind Show files.
    files: Option<Rc<Vec<String>>>,
    skip: bool,
    on_skip: EventHandler<()>,
    remove_lock: bool,
    on_remove_lock: EventHandler<()>,
    on_close: EventHandler<()>,
    platform: Os,
    key: DiffKey,
}

impl GitErrorDialog {
    /// Fork's Git Error for the failure numbered `serial` — another failure is another dialog —
    /// of `command`, whose output is `lines`.
    pub fn new(serial: u64, command: impl Into<String>, lines: Rc<Vec<String>>) -> Self {
        Self {
            serial,
            title: GIT_ERROR_TITLE.to_owned(),
            sentence: GIT_ERROR_TEXT.to_owned(),
            details: Some((command.into(), lines)),
            files: None,
            skip: false,
            on_skip: EventHandler::new(|()| {}),
            remove_lock: false,
            on_remove_lock: EventHandler::new(|()| {}),
            on_close: EventHandler::new(|()| {}),
            platform: Os::current(),
            key: DiffKey::None,
        }
    }

    /// "Couldn't <name>": a refusal found while `name` ran, in one `sentence`, with no Error
    /// Details.
    pub fn couldnt(serial: u64, name: &str, sentence: impl Into<String>) -> Self {
        Self {
            title: couldnt_title(name),
            sentence: sentence.into(),
            details: None,
            ..Self::new(serial, String::new(), Rc::new(Vec::new()))
        }
    }

    /// The dialog's title in place of Fork's: "Couldn't <name>" for a refusal.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// The dialog's sentence in place of Fork's: a commit git made and then failed says so.
    pub fn sentence(mut self, sentence: impl Into<String>) -> Self {
        self.sentence = sentence.into();
        self
    }

    /// Error Details: what ran, and git's output.
    pub fn details(mut self, details: Option<(String, Rc<Vec<String>>)>) -> Self {
        self.details = details;
        self
    }

    /// The files left as they were, behind Show files (a partly done discard).
    pub fn files(mut self, files: Option<Rc<Vec<String>>>) -> Self {
        self.files = files;
        self
    }

    /// Whether the skip is offered: on every failure whose hooks were not skipped already.
    pub fn skip(mut self, offered: bool) -> Self {
        self.skip = offered;
        self
    }

    /// The skip was pressed: the same commit again, its hooks skipped this once.
    pub fn on_skip(mut self, on_skip: impl Into<EventHandler<()>>) -> Self {
        self.on_skip = on_skip.into();
        self
    }

    /// Whether `Remove index.lock…` is offered: git could not create the lock.
    pub fn remove_lock(mut self, offered: bool) -> Self {
        self.remove_lock = offered;
        self
    }

    /// `Remove index.lock…` was pressed: the dialog closes, and the window asks what the removal
    /// would cost, for its confirmation.
    pub fn on_remove_lock(mut self, on_remove_lock: impl Into<EventHandler<()>>) -> Self {
        self.on_remove_lock = on_remove_lock.into();
        self
    }

    /// Close, Escape, or a press outside.
    pub fn on_close(mut self, on_close: impl Into<EventHandler<()>>) -> Self {
        self.on_close = on_close.into();
        self
    }

    /// Which platform's button order to draw in: this build's own unless a test names another.
    pub fn platform(mut self, platform: Os) -> Self {
        self.platform = platform;
        self
    }
}

impl PartialEq for GitErrorDialog {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
            && self.title == other.title
            && self.sentence == other.sentence
            && self.skip == other.skip
            && self.remove_lock == other.remove_lock
            && self.platform == other.platform
            && self.key == other.key
    }
}

impl std::fmt::Debug for GitErrorDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitErrorDialog")
            .field("serial", &self.serial)
            .field("title", &self.title)
            .field("sentence", &self.sentence)
            .field(
                "details",
                &self
                    .details
                    .as_ref()
                    .map(|(command, lines)| (command, lines.len())),
            )
            .field("files", &self.files.as_ref().map(|files| files.len()))
            .field("skip", &self.skip)
            .field("remove_lock", &self.remove_lock)
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
        let close_id = use_a11y();
        // Opened at the end of the output, where a failing hook says why.
        let controller = use_scroll_controller(|| ScrollConfig {
            default_vertical_position: ScrollPosition::End,
            ..ScrollConfig::default()
        });
        let answered = use_state(|| false);
        let mut showing_files = use_state(|| false);
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
        let removing = once(self.on_remove_lock.clone());

        let details = self.details.as_ref().map(|(command, lines)| {
            let output = VirtualScrollView::new_with_data_controlled(
                OutputLines(lines.clone()),
                output_row,
                controller,
            )
            .length(lines.len())
            .item_size(OUTPUT_ROW_HEIGHT)
            .width(Size::fill())
            .height(Size::px(OUTPUT_HEIGHT));
            rect()
                .width(Size::fill())
                .spacing(6.)
                .child(
                    label()
                        .text(ERROR_DETAILS)
                        .font_size(metrics::BODY)
                        .color(tones::DETAIL_LABEL),
                )
                .child(
                    label()
                        .text(command.clone())
                        .font_family(DIFF_FONT_FAMILY)
                        .font_size(metrics::BODY)
                        .width(Size::fill()),
                )
                .child(
                    rect()
                        .width(Size::fill())
                        .border(Border::new().fill(tones::RULE).width(1.))
                        .font_family(DIFF_FONT_FAMILY)
                        .font_size(metrics::BODY)
                        .child(output),
                )
        });
        let files = self.files.as_ref().map(|files| {
            let shown = *showing_files.read();
            let list = shown.then(|| {
                VirtualScrollView::new_with_data(OutputLines(files.clone()), output_row)
                    .length(files.len())
                    .item_size(OUTPUT_ROW_HEIGHT)
                    .width(Size::fill())
                    .height(Size::px(
                        OUTPUT_ROW_HEIGHT * files.len().clamp(1, FILES_SHOWN) as f32,
                    ))
            });
            rect()
                .width(Size::fill())
                .spacing(4.)
                .child(
                    Button::new()
                        .compact()
                        .on_press(move |_| {
                            let now = *showing_files.peek();
                            showing_files.set(!now);
                        })
                        .child(crate::dialogs::confirm::SHOW_FILES_CAPTION),
                )
                .maybe_child(list.map(|list| {
                    rect()
                        .width(Size::fill())
                        .border(Border::new().fill(tones::RULE).width(1.))
                        .font_family(DIFF_FONT_FAMILY)
                        .font_size(metrics::BODY)
                        .child(list)
                }))
        });

        let skip_id = use_a11y();
        let lock_id = use_a11y();
        let colours_of_answers = AnswerColours::of_theme();
        let mut actions: Vec<Element> = Vec::new();
        if self.remove_lock {
            actions.push(answer(
                self.serial,
                lock_id,
                REMOVE_LOCK_CAPTION.to_owned(),
                false,
                &colours_of_answers,
                removing,
            ));
        }
        if self.skip {
            actions.push(answer(
                self.serial,
                skip_id,
                SKIP_HOOKS_CAPTION.to_owned(),
                false,
                &colours_of_answers,
                skip,
            ));
        }
        let closing = close.clone();
        // In the platform's order, Close in Cancel's place.
        let buttons = PopupButtons::new().children(ordered(
            self.platform,
            actions,
            // Opened on Close, the safe answer, so a Return or Space pressed by habit never
            // skips the hooks nor removes a lock.
            answer(
                self.serial,
                close_id,
                CLOSE_CAPTION.to_owned(),
                true,
                &colours_of_answers,
                closing,
            ),
        ));

        Popup::new()
            .width(Size::px(DIALOG_WIDTH))
            .on_close_request(move |()| close())
            .child(
                rect()
                    .width(Size::fill())
                    .a11y_modal(true)
                    .a11y_role(AccessibilityRole::AlertDialog)
                    .child(PopupTitle::new(self.title.clone()))
                    .child(
                        PopupContent::new()
                            // Wrapped, never cut.
                            .child(
                                label()
                                    .text(self.sentence.clone())
                                    .font_size(metrics::BODY)
                                    .width(Size::fill()),
                            )
                            .maybe_child(files)
                            .maybe_child(details),
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
