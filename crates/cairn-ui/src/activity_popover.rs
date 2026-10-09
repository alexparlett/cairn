//! Fork's Activity popover (staging-and-commit R12.1; `fork-staging-and-commit.md` §7), opened
//! from the title bar's status box: on the left the session's operations, newest first — each
//! its name, its status and when it started, with a cancel for a running one that can be
//! cancelled — and on the right the one selected: its status and how long it took, what it
//! said, the prompt it confirmed when it had one (Cairn's addition, L14), the way back where
//! there is one — an amend's replaced commit, shown in Show Lost Commits — and `Remove
//! index.lock…` where R12.4 offers it, then each `git` it ran as Fork prints it, `$ git ...`,
//! with what it wrote to stderr.
//!
//! Both lists are virtualized — the operations and the selected one's lines, which are as long
//! as git's output makes them — so neither builds more than a viewport of rows (R12.1). Every
//! line is the window's, already stripped of escape sequences and of a URL's userinfo
//! (R12.2): this file draws text it is given and reads nothing else. Escape and a press
//! outside close it.

use std::rc::Rc;

use cairn_model::{Consequence, Oid};
use freya::prelude::*;

use crate::diff_palette::DIFF_FONT_FAMILY;

/// The popover's title, Fork's word for its log.
pub const ACTIVITY_TITLE: &str = "Activity";
/// The way back from an amend: its replaced commit, in Show Lost Commits.
pub const SHOW_REPLACED_CAPTION: &str = "Show Replaced Commit";
/// R12.4's action, confirmed like every destructive operation.
pub const REMOVE_LOCK_CAPTION: &str = "Remove index.lock…";
/// The cancel beside a running operation that can be cancelled (Fork's ×).
pub const CANCEL_OPERATION_CAPTION: &str = "×";
/// What the popover says before any operation has run.
pub const NO_ACTIVITY: &str = "Nothing has run in this window yet.";
/// Said above the prompt an operation confirmed.
pub const CONFIRMED_CAPTION: &str = "Confirmed:";

const WIDTH: f32 = 860.;
const HEIGHT: f32 = 460.;
const LIST_WIDTH: f32 = 260.;
/// One operation's row on the left.
pub const ENTRY_ROW_HEIGHT: f32 = 38.;
/// One line on the right.
pub const LINE_ROW_HEIGHT: f32 = 16.;
/// Where the popover hangs: under the title bar.
const TOP: f32 = 44.;

/// One line of an operation's detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityLine {
    /// A `git` it ran, as Fork prints it: `$ git ...`.
    Ran(String),
    /// A line it wrote — stderr, a hook's output.
    Output(String),
}

/// An operation as the popover draws it, every text the window's.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityEntry {
    pub name: String,
    /// `running`, `succeeded`, …
    pub status: String,
    /// When it started, as the window says it.
    pub started: String,
    /// How long it took, once it has ended.
    pub took: Option<String>,
    /// What its ending said, beyond its status.
    pub message: Option<String>,
    /// The prompt it confirmed, for a destructive operation (R1.6).
    pub prompt: Option<String>,
    /// The commit an amend replaced, which Show Lost Commits draws (R12.1).
    pub replaced: Option<Oid>,
    /// `Remove index.lock…`'s consequence, where R12.4 offers it.
    pub lock: Option<Rc<Consequence>>,
    /// A running operation that can be cancelled: a commit, an amend, a fetch.
    pub cancellable: bool,
    pub lines: Rc<Vec<ActivityLine>>,
}

/// The popover over `entries`, newest first, with `selected` drawn on the right.
pub struct ActivityPopover {
    entries: Rc<Vec<ActivityEntry>>,
    selected: usize,
    on_select: EventHandler<usize>,
    on_cancel: EventHandler<usize>,
    on_replaced: EventHandler<Oid>,
    on_remove_lock: EventHandler<Rc<Consequence>>,
    on_close: EventHandler<()>,
    key: DiffKey,
}

impl ActivityPopover {
    pub fn new(entries: Rc<Vec<ActivityEntry>>, selected: usize) -> Self {
        Self {
            entries,
            selected,
            on_select: EventHandler::new(|_| {}),
            on_cancel: EventHandler::new(|_| {}),
            on_replaced: EventHandler::new(|_| {}),
            on_remove_lock: EventHandler::new(|_| {}),
            on_close: EventHandler::new(|()| {}),
            key: DiffKey::None,
        }
    }

    /// An operation pressed on the left: its index among the entries.
    pub fn on_select(mut self, on_select: impl Into<EventHandler<usize>>) -> Self {
        self.on_select = on_select.into();
        self
    }

    /// A running operation's cancel pressed: its index among the entries.
    pub fn on_cancel(mut self, on_cancel: impl Into<EventHandler<usize>>) -> Self {
        self.on_cancel = on_cancel.into();
        self
    }

    /// The way back pressed: the commit an amend replaced.
    pub fn on_replaced(mut self, on_replaced: impl Into<EventHandler<Oid>>) -> Self {
        self.on_replaced = on_replaced.into();
        self
    }

    /// `Remove index.lock…` pressed: the consequence the confirmation is to draw.
    pub fn on_remove_lock(mut self, on_remove: impl Into<EventHandler<Rc<Consequence>>>) -> Self {
        self.on_remove_lock = on_remove.into();
        self
    }

    /// Escape, or a press outside.
    pub fn on_close(mut self, on_close: impl Into<EventHandler<()>>) -> Self {
        self.on_close = on_close.into();
        self
    }
}

impl PartialEq for ActivityPopover {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries && self.selected == other.selected && self.key == other.key
    }
}

impl std::fmt::Debug for ActivityPopover {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActivityPopover")
            .field("entries", &self.entries.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl KeyExt for ActivityPopover {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// What the list on the left builds its rows from.
#[derive(Clone)]
struct EntryList {
    entries: Rc<Vec<ActivityEntry>>,
    selected: usize,
    on_select: EventHandler<usize>,
    on_cancel: EventHandler<usize>,
}

impl PartialEq for EntryList {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.entries, &other.entries) && self.selected == other.selected
    }
}

/// The selected operation's lines, compared by identity: an entry's lines are replaced, never
/// changed in place.
#[derive(Clone)]
struct Lines(Rc<Vec<ActivityLine>>);

impl PartialEq for Lines {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

fn entry_row(item: VirtualItem, list: &EntryList) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    let index = item.index;
    let Some(entry) = list.entries.get(index) else {
        return rect().key(index).height(Size::px(item.size)).into();
    };
    let selected = index == list.selected;
    let on_select = list.on_select.clone();
    let on_cancel = list.on_cancel.clone();
    rect()
        .key(index)
        .width(Size::fill())
        .height(Size::px(item.size))
        .horizontal()
        .content(Content::Flex)
        .cross_align(Alignment::center())
        .padding(Gaps::new(2., 8., 2., 8.))
        .maybe(selected, |row| row.background(colours.surface_secondary))
        .on_press(move |_| on_select.call(index))
        .child(
            rect()
                .width(Size::flex(1.))
                .child(
                    label()
                        .text(entry.name.clone())
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .font_size(13.)
                        .color(colours.text_primary),
                )
                .child(
                    label()
                        .text(format!("{} · {}", entry.status, entry.started))
                        .max_lines(1)
                        .font_size(11.)
                        .color(colours.text_secondary),
                ),
        )
        .maybe_child(entry.cancellable.then(|| {
            Button::new()
                .compact()
                .on_press(move |_| on_cancel.call(index))
                .child(CANCEL_OPERATION_CAPTION)
        }))
        .into()
}

fn line_row(item: VirtualItem, lines: &Lines) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    let (text, colour) = match lines.0.get(item.index) {
        Some(ActivityLine::Ran(command)) => (command.clone(), colours.text_primary),
        Some(ActivityLine::Output(output)) => (output.clone(), colours.text_secondary),
        None => (String::new(), colours.text_secondary),
    };
    rect()
        .key(item.index)
        .min_width(Size::fill())
        .height(Size::px(item.size))
        .padding(Gaps::new(0., 6., 0., 6.))
        .child(label().text(text).max_lines(1).color(colour))
        .into()
}

impl Component for ActivityPopover {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        let close = self.on_close.clone();
        let closing = self.on_close.clone();
        let list = EntryList {
            entries: self.entries.clone(),
            selected: self.selected,
            on_select: self.on_select.clone(),
            on_cancel: self.on_cancel.clone(),
        };
        let left: Element = if self.entries.is_empty() {
            rect()
                .expanded()
                .center()
                .child(
                    label()
                        .text(NO_ACTIVITY)
                        .font_size(12.)
                        .color(colours.text_secondary),
                )
                .into()
        } else {
            VirtualScrollView::new_with_data(list, entry_row)
                .length(self.entries.len())
                .item_size(ENTRY_ROW_HEIGHT)
                .expanded()
                .into()
        };
        let right = self.entries.get(self.selected).map(|entry| {
            let on_replaced = self.on_replaced.clone();
            let on_remove = self.on_remove_lock.clone();
            let replaced = entry.replaced;
            let lock = entry.lock.clone();
            let lines = VirtualScrollView::new_with_data(Lines(entry.lines.clone()), line_row)
                .length(entry.lines.len())
                .item_size(LINE_ROW_HEIGHT)
                .width(Size::fill())
                .height(Size::flex(1.));
            rect()
                .width(Size::flex(1.))
                .height(Size::fill())
                .content(Content::Flex)
                .spacing(6.)
                .padding(Gaps::new(8., 8., 8., 8.))
                .child(
                    label()
                        .text(entry.name.clone())
                        .max_lines(1)
                        .font_size(14.)
                        .font_weight(FontWeight::BOLD)
                        .color(colours.text_primary),
                )
                .child(
                    label()
                        .text(match &entry.took {
                            Some(took) => {
                                format!(
                                    "{} · started {} · took {took}",
                                    entry.status, entry.started
                                )
                            }
                            None => format!("{} · started {}", entry.status, entry.started),
                        })
                        .max_lines(1)
                        .font_size(12.)
                        .color(colours.text_secondary),
                )
                .maybe_child(entry.message.clone().map(|message| {
                    label()
                        .text(message)
                        .max_lines(3)
                        .text_overflow(TextOverflow::Ellipsis)
                        .font_size(12.)
                        .color(colours.text_primary)
                }))
                .maybe_child(entry.prompt.clone().map(|prompt| {
                    rect()
                        .width(Size::fill())
                        .child(
                            label()
                                .text(CONFIRMED_CAPTION)
                                .font_size(11.)
                                .color(colours.text_secondary),
                        )
                        .child(
                            label()
                                .text(prompt)
                                .max_lines(4)
                                .text_overflow(TextOverflow::Ellipsis)
                                .font_size(12.)
                                .color(colours.text_primary),
                        )
                }))
                .maybe_child((replaced.is_some() || lock.is_some()).then(|| {
                    rect()
                        .horizontal()
                        .spacing(8.)
                        .maybe_child(replaced.map(|commit| {
                            Button::new()
                                .compact()
                                .on_press(move |_| on_replaced.call(commit))
                                .child(SHOW_REPLACED_CAPTION)
                        }))
                        .maybe_child(lock.map(|consequence| {
                            Button::new()
                                .compact()
                                .on_press(move |_| on_remove.call(consequence.clone()))
                                .child(REMOVE_LOCK_CAPTION)
                        }))
                }))
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::flex(1.))
                        .border(Border::new().fill(colours.border).width(1.))
                        .font_family(DIFF_FONT_FAMILY)
                        .font_size(12.)
                        .child(lines),
                )
        });

        rect()
            .layer(Layer::Overlay)
            .position(Position::new_global())
            .child(
                rect()
                    .position(Position::new_global().top(0.).left(0.))
                    .height(Size::window_percent(100.))
                    .width(Size::window_percent(100.))
                    .on_press(move |_| close.call(())),
            )
            .child(
                rect()
                    .position(Position::new_global().top(TOP).left(0.))
                    .width(Size::window_percent(100.))
                    .cross_align(Alignment::center())
                    .child(
                        rect()
                            .a11y_role(AccessibilityRole::Dialog)
                            .width(Size::px(WIDTH))
                            .height(Size::px(HEIGHT))
                            .corner_radius(8.)
                            .background(colours.background)
                            .border(Border::new().fill(colours.border).width(1.))
                            .shadow(Shadow::new().y(4.).blur(5.).color((0, 0, 0, 30)))
                            .on_global_key_down(move |e: Event<KeyboardEventData>| {
                                if e.key == Key::Named(NamedKey::Escape) {
                                    closing.call(());
                                    e.prevent_default();
                                }
                            })
                            .content(Content::Flex)
                            .child(
                                rect().padding(Gaps::new(8., 10., 4., 10.)).child(
                                    label()
                                        .text(ACTIVITY_TITLE)
                                        .font_size(13.)
                                        .font_weight(FontWeight::BOLD)
                                        .color(colours.text_primary),
                                ),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .height(Size::flex(1.))
                                    .content(Content::Flex)
                                    .child(
                                        rect()
                                            .width(Size::px(LIST_WIDTH))
                                            .height(Size::fill())
                                            .child(left),
                                    )
                                    .maybe_child(right),
                            ),
                    ),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
