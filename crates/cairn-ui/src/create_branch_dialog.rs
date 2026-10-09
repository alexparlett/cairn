//! Fork's Create Branch dialog (`docs/prd/staging-and-commit.md` R11.3; the user's decision,
//! 2026-10-09; evidence `docs/research/staging-and-commit/fork-create-branch-evidence.md`),
//! opened by "New Branch…" on any commit row, or at `HEAD` by its chord: under the title Fork's
//! line on folders, then "Create branch at:" the commit — its short id and subject, read only —
//! a "Branch name:" field, "Check out after create", and, while that is ticked and the working
//! tree has changes, "Local changes:" with "Don't change" and "Discard". Its button reads
//! "Create", or "Create and Checkout" while the box is ticked.
//!
//! A name is refused inline, before git runs: the button stays disabled until the engine has
//! answered that git takes the name and no branch has it, and a refusal is said in the buttons'
//! row, left of them, behind Fork's warning triangle — Fork's words for a name taken, git's for
//! one it does not take. Return in the name
//! field presses the button when it is enabled; Escape, Cancel and a press outside cancel.
//!
//! What the dialog holds is the window's: the name, the box, the choice and the engine's answer
//! come in as props, and every change goes out through a handler. "Stash and reapply", Fork's
//! third choice, waits for packet 5b, which builds stashing (the user's decision 2, a stated
//! deviation from Fork).

use cairn_model::Oid;
use freya::prelude::*;

use crate::check_box::check_box;
use crate::ref_glyphs::{GLYPH_SIZE, RefGlyph};
use crate::text_field::text_field;

/// The dialog's title, Fork's.
pub const CREATE_BRANCH_TITLE: &str = "Create Branch";
/// The line under the title, Fork's (VSHOT TrackerWin #1394).
pub const CREATE_BRANCH_SUBTITLE: &str = "Use '/' as a path separator to create folders";
/// The commit row's caption, Fork's.
pub const CREATE_BRANCH_AT: &str = "Create branch at:";
/// The name field's caption, Fork's.
pub const BRANCH_NAME_CAPTION: &str = "Branch name:";
/// The name field's placeholder, Fork's.
pub const BRANCH_NAME_PLACEHOLDER: &str = "Enter branch name";
/// The check box's caption, Fork's.
pub const CHECK_OUT_AFTER_CREATE: &str = "Check out after create";
/// The local changes' caption, Fork's.
pub const LOCAL_CHANGES_LABEL: &str = "Local changes:";
/// Keep the changes and check out over them, Fork's default.
pub const DONT_CHANGE_CAPTION: &str = "Don't change";
/// Throw the changes away, confirmed first.
pub const DISCARD_LOCAL_CAPTION: &str = "Discard";
/// The button while the box is unticked.
pub const CREATE_CAPTION: &str = "Create";
/// The button while the box is ticked.
pub const CREATE_AND_CHECKOUT_CAPTION: &str = "Create and Checkout";
/// The safe answer.
pub const CANCEL_BRANCH_CAPTION: &str = "Cancel";

const DIALOG_WIDTH: f32 = 520.0;
/// The captions' column: Fork right-aligns them against the values.
const CAPTION_WIDTH: f32 = 130.0;
const FONT_SIZE: f32 = 13.0;

/// What a checkout after create does with the working tree's changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalChoice {
    /// "Don't change": carried over, or git refuses.
    Keep,
    /// "Discard": thrown away, confirmed first.
    Discard,
}

pub struct CreateBranchDialog {
    serial: u64,
    at: Oid,
    subject: String,
    name: State<String>,
    ready: bool,
    refusal: Option<String>,
    checkout: bool,
    local: Option<LocalChoice>,
    on_checkout: EventHandler<bool>,
    on_local: EventHandler<LocalChoice>,
    on_create: EventHandler<()>,
    on_cancel: EventHandler<()>,
    key: DiffKey,
}

impl CreateBranchDialog {
    /// The dialog for a branch at `at`, whose subject is `subject`; `serial` names this
    /// opening of it, and `name` is the field's text, the window's.
    pub fn new(serial: u64, at: Oid, subject: impl Into<String>, name: State<String>) -> Self {
        Self {
            serial,
            at,
            subject: subject.into(),
            name,
            ready: false,
            refusal: None,
            checkout: false,
            local: None,
            on_checkout: EventHandler::new(|_: bool| {}),
            on_local: EventHandler::new(|_: LocalChoice| {}),
            on_create: EventHandler::new(|()| {}),
            on_cancel: EventHandler::new(|()| {}),
            key: DiffKey::None,
        }
    }

    /// Whether the name typed is one the engine said can be created: the button is enabled
    /// only then.
    pub fn ready(mut self, ready: bool) -> Self {
        self.ready = ready;
        self
    }

    /// Why the name typed cannot be created, said beside the buttons.
    pub fn refusal(mut self, refusal: Option<String>) -> Self {
        self.refusal = refusal;
        self
    }

    /// "Check out after create", ticked or not.
    pub fn checkout(mut self, checkout: bool) -> Self {
        self.checkout = checkout;
        self
    }

    /// "Local changes:", drawn with `choice` selected — the window passes it only while the
    /// box is ticked and the working tree has changes.
    pub fn local_changes(mut self, choice: Option<LocalChoice>) -> Self {
        self.local = choice;
        self
    }

    pub fn on_checkout(mut self, on_checkout: impl Into<EventHandler<bool>>) -> Self {
        self.on_checkout = on_checkout.into();
        self
    }

    pub fn on_local(mut self, on_local: impl Into<EventHandler<LocalChoice>>) -> Self {
        self.on_local = on_local.into();
        self
    }

    /// The button, or Return in the name field, while the name is ready.
    pub fn on_create(mut self, on_create: impl Into<EventHandler<()>>) -> Self {
        self.on_create = on_create.into();
        self
    }

    /// Cancel, Escape, or a press outside the dialog.
    pub fn on_cancel(mut self, on_cancel: impl Into<EventHandler<()>>) -> Self {
        self.on_cancel = on_cancel.into();
        self
    }
}

// Hand-written: an `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for CreateBranchDialog {
    fn eq(&self, other: &Self) -> bool {
        self.serial == other.serial
            && self.at == other.at
            && self.subject == other.subject
            && self.name == other.name
            && self.ready == other.ready
            && self.refusal == other.refusal
            && self.checkout == other.checkout
            && self.local == other.local
            && self.key == other.key
    }
}

impl std::fmt::Debug for CreateBranchDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateBranchDialog")
            .field("serial", &self.serial)
            .field("at", &self.at)
            .field("ready", &self.ready)
            .field("checkout", &self.checkout)
            .field("local", &self.local)
            .finish_non_exhaustive()
    }
}

impl KeyExt for CreateBranchDialog {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for CreateBranchDialog {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        let ready = self.ready;
        let create = {
            let on_create = self.on_create.clone();
            move || {
                if ready {
                    on_create.call(());
                }
            }
        };
        let caption = |text: &'static str| {
            label()
                .text(text)
                .width(Size::px(CAPTION_WIDTH))
                .text_align(TextAlign::End)
                .max_lines(1)
                .font_size(FONT_SIZE)
                .color(colours.text_secondary)
        };
        let row = |name: &'static str, value: Element| {
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(8.)
                .child(caption(name))
                .child(value)
        };
        let commit = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(6.)
            .child(commit_glyph(colours.text_primary))
            .child(
                label()
                    .text(format!("{} {}", self.at.short().as_str(), self.subject))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .width(Size::px(DIALOG_WIDTH - CAPTION_WIDTH - 60.))
                    .font_size(FONT_SIZE)
                    .color(colours.text_primary),
            );
        let field = text_field(self.name)
            .width(Size::px(DIALOG_WIDTH - CAPTION_WIDTH - 60.))
            .placeholder(BRANCH_NAME_PLACEHOLDER)
            .auto_focus(true)
            .on_submit({
                let create = create.clone();
                move |_: String| create()
            });
        let mut content = PopupContent::new()
            .child(row(CREATE_BRANCH_AT, commit.into()))
            .child(row(BRANCH_NAME_CAPTION, field.into()))
            .child(row(
                "",
                check_box(
                    CHECK_OUT_AFTER_CREATE,
                    self.checkout,
                    true,
                    self.on_checkout.clone(),
                ),
            ));
        if let Some(choice) = self.local {
            let option = |text: &'static str, this: LocalChoice| {
                radio(text, choice == this, {
                    let on_local = self.on_local.clone();
                    move || on_local.call(this)
                })
            };
            content = content.child(row(
                LOCAL_CHANGES_LABEL,
                rect()
                    .spacing(6.)
                    .child(option(DONT_CHANGE_CAPTION, LocalChoice::Keep))
                    .child(option(DISCARD_LOCAL_CAPTION, LocalChoice::Discard))
                    .into(),
            ));
        }
        let primary = if self.checkout {
            CREATE_AND_CHECKOUT_CAPTION
        } else {
            CREATE_CAPTION
        };
        // Fork's refusal sits in the buttons' row, left of them, behind its warning triangle
        // (USHOT TrackerWin #2472, Tracker #1911); the buttons keep PopupButtons' spacing.
        let said = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::flex(1.))
            .cross_align(Alignment::Center)
            .spacing(6.)
            .maybe(self.refusal.is_some(), |said| {
                said.child(
                    rect()
                        .key(RefGlyph::Gone)
                        .width(Size::px(GLYPH_SIZE))
                        .height(Size::px(GLYPH_SIZE))
                        .child(RefGlyph::Gone.draw(colours.warning)),
                )
            })
            .maybe_child(self.refusal.clone().map(|reason| {
                label()
                    .text(reason)
                    .width(Size::flex(1.))
                    .max_lines(2)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(12.)
                    .color(colours.text_secondary)
            }));
        let buttons = rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .padding(8.)
            .spacing(4.)
            .child(said)
            .child(
                Button::new()
                    .on_press({
                        let on_cancel = self.on_cancel.clone();
                        move |_| on_cancel.call(())
                    })
                    .child(CANCEL_BRANCH_CAPTION),
            )
            .child(
                Button::new()
                    .filled()
                    .enabled(ready)
                    .on_press(move |_| create())
                    .child(primary),
            );
        let on_close = self.on_cancel.clone();
        Popup::new()
            .width(Size::px(DIALOG_WIDTH))
            .on_close_request(move |()| on_close.call(()))
            .child(
                rect()
                    .width(Size::fill())
                    .a11y_modal(true)
                    .a11y_role(AccessibilityRole::Dialog)
                    .child(PopupTitle::new(CREATE_BRANCH_TITLE.to_owned()))
                    .child(
                        label()
                            .text(CREATE_BRANCH_SUBTITLE)
                            .width(Size::fill())
                            .padding(Gaps::new(0., 8., 4., 8.))
                            .font_size(12.)
                            .color(colours.text_secondary),
                    )
                    .child(content)
                    .child(buttons),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(DiffKey::U64(self.serial))
    }
}

/// Fork's commit glyph beside the commit: an outlined diamond, painted as a shape.
fn commit_glyph(colour: Color) -> Rect {
    rect()
        .width(Size::px(8.))
        .height(Size::px(8.))
        .rotate(45.)
        .border(Border::new().fill(colour).width(1.5))
}

/// One choice of "Local changes:": a radio button to assistive technology, its state set,
/// pressed by the pointer or, focused, by Space or Return.
fn radio(caption: &'static str, selected: bool, pressed: impl Fn() + 'static) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    let dot = rect()
        .width(Size::px(14.))
        .height(Size::px(14.))
        .corner_radius(7.)
        .main_align(Alignment::Center)
        .cross_align(Alignment::Center)
        .border(
            Border::new()
                .fill(colours.border)
                .width(1.)
                .alignment(BorderAlignment::Inner),
        )
        .maybe_child(selected.then(|| {
            rect()
                .width(Size::px(6.))
                .height(Size::px(6.))
                .corner_radius(3.)
                .background(colours.primary)
        }));
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(6.)
        .a11y_role(AccessibilityRole::RadioButton)
        .a11y_alt(caption)
        .a11y_focusable(true)
        .a11y_builder(move |node| node.set_toggled(Toggled::from(selected)))
        .cursor(CursorIcon::Pointer)
        .on_press(move |e: Event<PressEventData>| {
            e.stop_propagation();
            pressed();
        })
        .child(dot)
        .child(
            label()
                .text(caption)
                .font_size(FONT_SIZE)
                .color(colours.text_primary),
        )
        .into()
}
