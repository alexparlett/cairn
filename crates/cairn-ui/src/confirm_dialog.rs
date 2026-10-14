//! The confirmation a destructive operation waits on (staging-and-commit R7.4, L8).
//!
//! It draws a [`Consequence`] the engine computed — its prompt and its button's label, both
//! rendered from it by its own `prompt` and `action`, so no count or path is typed apart
//! from the value it describes — and, when its button is pressed, builds the
//! [`Confirmed`] token from that same value: this file is a confirmation surface, the one
//! place in the window a token is made (`CONFIRMATION_SURFACES`, root `CLAUDE.md`).
//!
//! **It opens at the press** (the redesign's rule 3, the user's decisions of 2026-10-10 on phase
//! 15): while the engine counts what the operation would destroy it says "Counting…" and its
//! button, greyed, reads the dialog's title ([`DialogAnswer::Counting`]); the count arriving
//! makes it the confirmation ([`DialogAnswer::Ready`]), the button its own caption. A refusal
//! the engine makes while counting makes the same dialog the refusal
//! ([`DialogAnswer::Refused`], rule 4): titled "Couldn't <title>", the engine's one sentence and
//! "Nothing was changed.", Close alone and no Error Details, since no git failed.
//!
//! **Its words are short and never cut** (rule 6): the prompt wraps whole, one line under it says
//! what the selection held that the operation leaves as it is (the redesign's D1), and the
//! detail of a discard of several files — each path and what happens to it — sits behind a
//! "Show files" disclosure over a virtualized list, so a selection of fifty thousand builds one
//! viewport of rows.
//!
//! It is modal to the keyboard and to assistive technology: focus is held inside it
//! (`a11y_modal`) and starts on **Cancel**, a deliberate deviation from Fork, which styles
//! Discard as the default (L8, Fork Tracker #1080) — so a Return pressed by habit cancels.
//! Escape cancels and so does a press outside it, as `Popup` does. The window's chords are
//! made inert while it is open by the window itself, whose key listener runs before any
//! dialog's (`shortcuts::act`). One acknowledgement builds one token: once answered, the
//! dialog ignores every later press until it is closed.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{Confirmed, Consequence};
use freya::prelude::*;

use crate::accelerators::Os;
use crate::answer_button::{AnswerColours, answer, greyed_answer};
use crate::button_order::ordered;
use crate::git_error_dialog::CLOSE_CAPTION;

/// Width of the dialog: room for a prompt naming a long path.
const DIALOG_WIDTH: f32 = 520.;

/// The safe answer, which holds focus as the dialog opens.
pub const CANCEL_CAPTION: &str = "Cancel";

/// What the dialog says while the engine counts (R7.4).
pub const COUNTING_TEXT: &str = "Counting…";

/// The disclosure over the files a discard of several takes (R1.2, the redesign's D2); its
/// caption stays and its glyph turns, ▸ closed and ▾ open (the user's decision of 2026-10-10).
pub const SHOW_FILES_CAPTION: &str = "Show files";

/// The tail a refusal found while running adds, once (R14.2).
pub const NOTHING_CHANGED: &str = "Nothing was changed.";

/// The height of one row of "Show files".
pub const SHOWN_FILE_HEIGHT: f32 = 20.;

/// The height of the list "Show files" opens: a viewport, however many files.
const SHOWN_FILES_HEIGHT: f32 = 200.;

/// Where the confirmation stands.
#[derive(Debug, Clone)]
pub enum DialogAnswer {
    /// The engine is counting what the operation would destroy.
    Counting,
    /// What it would destroy: the confirmation.
    Ready(Rc<Consequence>),
    /// The engine refused while counting, in one sentence: the refusal.
    Refused(Rc<str>),
}

impl DialogAnswer {
    /// Which of the three it is, and for a refusal its words: what a render compares, since
    /// one dialog's consequence never changes once it has arrived.
    fn phase(&self) -> (u8, Option<&str>) {
        match self {
            Self::Counting => (0, None),
            Self::Ready(_) => (1, None),
            Self::Refused(why) => (2, Some(why)),
        }
    }
}

pub struct ConfirmDialog {
    /// Which confirmation this is: the dialog's identity, and its key, so a different
    /// confirmation is a new dialog — its answers, its focus on Cancel and its one token
    /// afresh — however the window got from one to the next.
    serial: u64,
    title: String,
    answer: DialogAnswer,
    /// One line under the prompt: what the selection held that the operation leaves as it is.
    left: Option<String>,
    on_confirm: EventHandler<Confirmed>,
    on_cancel: EventHandler<()>,
    platform: Os,
    key: DiffKey,
}

impl ConfirmDialog {
    /// A dialog titled `title` (the redesign's Title Case, "Discard Changes") asking whether to
    /// accept `consequence`; `serial` names this confirmation, and must differ for every other.
    pub fn new(serial: u64, title: impl Into<String>, consequence: Rc<Consequence>) -> Self {
        Self::answering(serial, title, DialogAnswer::Ready(consequence))
    }

    /// The dialog numbered `serial`, titled `title`, as `answer` stands: counting, ready or
    /// refused. One confirmation keeps its serial from the press to its answer, so the dialog
    /// opened counting is the one that confirms or refuses.
    pub fn answering(serial: u64, title: impl Into<String>, answer: DialogAnswer) -> Self {
        Self {
            serial,
            title: title.into(),
            answer,
            left: None,
            on_confirm: EventHandler::new(|_| {}),
            on_cancel: EventHandler::new(|()| {}),
            platform: Os::current(),
            key: DiffKey::None,
        }
    }

    /// One line said under the prompt — what the selection held that the operation leaves as
    /// it is (the redesign's D1: "1 submodule and 1 conflicted file are left as they are.") —
    /// never part of the prompt the token records, which names only what is destroyed.
    pub fn left(mut self, left: Option<String>) -> Self {
        self.left = left;
        self
    }

    /// The button was pressed: the token, built from the consequence drawn, once.
    pub fn on_confirm(mut self, on_confirm: impl Into<EventHandler<Confirmed>>) -> Self {
        self.on_confirm = on_confirm.into();
        self
    }

    /// The user declined, or closed a refusal: Cancel or Close, Escape, or a press outside
    /// the dialog.
    pub fn on_cancel(mut self, on_cancel: impl Into<EventHandler<()>>) -> Self {
        self.on_cancel = on_cancel.into();
        self
    }

    /// Which platform's button order to draw in (the user's decision E): this build's own
    /// unless a test names another.
    pub fn platform(mut self, platform: Os) -> Self {
        self.platform = platform;
        self
    }
}

// By the serial and the answer's phase alone: one confirmation is one serial, whose title,
// consequence and handlers never change, so a window's render compares two numbers rather than
// every file of a consequence; and a different confirmation is also a different key
// (`render_key`), so the toolkit mounts a new dialog rather than keeping this one's handlers
// under its words.
impl PartialEq for ConfirmDialog {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
            && self.answer.phase() == other.answer.phase()
            && self.left == other.left
            && self.platform == other.platform
            && self.key == other.key
    }
}

impl std::fmt::Debug for ConfirmDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmDialog")
            .field("serial", &self.serial)
            .field("title", &self.title)
            .field("answer", &self.answer.phase())
            .finish_non_exhaustive()
    }
}

impl KeyExt for ConfirmDialog {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// The files "Show files" lists, compared by the consequence they come from.
struct ShownFiles(Rc<Consequence>);

impl PartialEq for ShownFiles {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// One row of "Show files": the path, and what happens to it, read by index as it is built.
/// The row is as wide as its words, never cut (rule 6): a path wider than the dialog scrolls
/// the list sideways, as every other list of paths does. An index past the list's own rows
/// (the end room) builds an empty row.
fn shown_file(item: VirtualItem, files: &ShownFiles) -> Element {
    let (path, what) = files.0.listed_file(item.index).unwrap_or_default();
    rect()
        .key(item.index)
        .min_width(Size::fill())
        .height(Size::px(item.size))
        .horizontal()
        .padding(Gaps::new(0., 6., 0., 6.))
        .spacing(12.)
        .child(label().text(path).max_lines(1).font_size(13.))
        .child(label().text(what).max_lines(1).font_size(13.))
        .into()
}

impl Component for ConfirmDialog {
    fn render(&self) -> impl IntoElement {
        let cancel_id = use_a11y();
        let confirm_id = use_a11y();
        let close_id = use_a11y();
        // Rendered once, when the consequence has arrived: a confirmation's words never change,
        // and a consequence of many files is not rendered again on every frame.
        let words = use_hook(|| Rc::new(RefCell::new(None::<(String, String)>)));
        // Set by the first answer; every later press is ignored, so one acknowledgement is one
        // token however fast the button is pressed again.
        let answered = use_state(|| false);
        let mut showing_files = use_state(|| false);
        let colours = AnswerColours::of_theme();

        let cancel = {
            let on_cancel = self.on_cancel.clone();
            move || {
                let mut answered = answered;
                if !*answered.peek() {
                    answered.set(true);
                    on_cancel.call(());
                }
            }
        };

        let (title, body, buttons, files) = match &self.answer {
            DialogAnswer::Refused(why) => {
                // Rule 4: the refusal, in this dialog, Close alone.
                let buttons = PopupButtons::new().child(answer(
                    self.serial,
                    close_id,
                    CLOSE_CAPTION.to_owned(),
                    true,
                    &colours,
                    cancel.clone(),
                ));
                (
                    format!("Couldn't {}", self.title),
                    format!("{why} {NOTHING_CHANGED}"),
                    buttons,
                    None,
                )
            }
            DialogAnswer::Counting => {
                let buttons = PopupButtons::new().children(ordered(
                    self.platform,
                    vec![greyed_answer(self.title.clone(), &colours)],
                    answer(
                        self.serial,
                        cancel_id,
                        CANCEL_CAPTION.to_owned(),
                        true,
                        &colours,
                        {
                            let cancel = cancel.clone();
                            move || cancel()
                        },
                    ),
                ));
                (self.title.clone(), COUNTING_TEXT.to_owned(), buttons, None)
            }
            DialogAnswer::Ready(consequence) => {
                let (prompt, action) = words
                    .borrow_mut()
                    .get_or_insert_with(|| (consequence.prompt(), consequence.action()))
                    .clone();
                let confirm = {
                    let on_confirm = self.on_confirm.clone();
                    let consequence = consequence.clone();
                    move || {
                        let mut answered = answered;
                        if !*answered.peek() {
                            answered.set(true);
                            on_confirm.call(Confirmed::by_user((*consequence).clone()));
                        }
                    }
                };
                // In the platform's order (the user's decision E); focus starts on Cancel
                // either way, and stays there as the count arrives.
                let buttons = PopupButtons::new().children(ordered(
                    self.platform,
                    vec![answer(
                        self.serial,
                        confirm_id,
                        action,
                        false,
                        &colours,
                        confirm,
                    )],
                    answer(
                        self.serial,
                        cancel_id,
                        CANCEL_CAPTION.to_owned(),
                        true,
                        &colours,
                        {
                            let cancel = cancel.clone();
                            move || cancel()
                        },
                    ),
                ));
                let listed = consequence.listed();
                let files = (listed > 0).then(|| (consequence.clone(), listed));
                (self.title.clone(), prompt, buttons, files)
            }
        };

        let disclosure = files.map(|(consequence, listed)| {
            let open = *showing_files.read();
            let glyph = if open { "▾" } else { "▸" };
            let mut column = rect().width(Size::fill()).spacing(6.).child(
                rect()
                    .a11y_focusable(true)
                    .a11y_role(AccessibilityRole::Button)
                    .a11y_alt(SHOW_FILES_CAPTION)
                    .a11y_builder(move |node| node.set_expanded(open))
                    .horizontal()
                    .spacing(4.)
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        let open = *showing_files.peek();
                        showing_files.set(!open);
                    })
                    .child(label().text(glyph).font_size(13.))
                    .child(label().text(SHOW_FILES_CAPTION).font_size(13.)),
            );
            if open {
                // Room for the horizontal scrollbar after the last row (`end_room`).
                let length = crate::end_room::with_end_room(listed, SHOWN_FILE_HEIGHT);
                column = column.child(
                    VirtualScrollView::new_with_data(ShownFiles(consequence), shown_file)
                        .length(length)
                        .item_size(SHOWN_FILE_HEIGHT)
                        .width(Size::fill())
                        .height(Size::px(
                            SHOWN_FILES_HEIGHT.min(SHOWN_FILE_HEIGHT * length as f32),
                        )),
                );
            }
            column
        });
        let left = match &self.answer {
            DialogAnswer::Ready(_) | DialogAnswer::Counting => self.left.clone(),
            DialogAnswer::Refused(_) => None,
        };

        Popup::new()
            .width(Size::px(DIALOG_WIDTH))
            .on_close_request(move |()| cancel())
            .child(
                rect()
                    .width(Size::fill())
                    .a11y_modal(true)
                    .a11y_role(AccessibilityRole::AlertDialog)
                    .child(PopupTitle::new(title))
                    .child(
                        PopupContent::new()
                            .child(label().text(body).width(Size::fill()).font_size(14.))
                            .maybe_child(
                                left.map(|left| {
                                    label().text(left).width(Size::fill()).font_size(14.)
                                }),
                            )
                            .maybe_child(disclosure),
                    )
                    .child(buttons),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(DiffKey::U64(self.serial))
    }
}
