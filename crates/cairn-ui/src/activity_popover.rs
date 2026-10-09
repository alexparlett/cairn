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
//!
//! The user's decisions of 2026-10-09 on it: it hangs from the status box, its left edge under
//! the box's and an arrow pointing up at it, kept inside the window's width (A); `Remove
//! index.lock…` stays drawn while it cannot be pressed — a `git` of Cairn's running — and says
//! why beside it (H); the confirmed prompt is cut to four lines with "Show All", which moves it
//! whole into the scrolling list above the lines, to scroll with them, and "Show Less" (K).

use std::rc::Rc;

use cairn_model::Oid;
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
/// Shows a confirmed prompt cut to four lines whole (the user's decision K, 2026-10-09).
pub const SHOW_ALL_CAPTION: &str = "Show All";
/// Cuts a prompt shown whole back to four lines.
pub const SHOW_LESS_CAPTION: &str = "Show Less";

const WIDTH: f32 = 860.;
const HEIGHT: f32 = 460.;
const LIST_WIDTH: f32 = 260.;
/// Kept from the window's edges, and the arrow from the panel's corners.
const MARGIN: f32 = 8.;
/// The arrow pointing at the status box: a square turned on its corner, half of it showing.
const ARROW: f32 = 12.;
/// How far the arrow's tip sits in from the box's left edge.
const ARROW_INSET: f32 = 18.;
/// The confirmed prompt's cut (the user's decision K): four lines.
const PROMPT_LINES: usize = 4;
/// About how many characters a line of the right pane holds, for whether a prompt is longer
/// than its cut: its width over a character's mean advance at 12 px.
const PROMPT_CHARS_PER_LINE: f32 = (WIDTH - LIST_WIDTH - 16.) / 6.5;
/// The prompt's line height, for its first measure before it is laid out.
const PROMPT_LINE_HEIGHT: f32 = 16.;
/// One operation's row on the left.
pub const ENTRY_ROW_HEIGHT: f32 = 38.;
/// One line on the right.
pub const LINE_ROW_HEIGHT: f32 = 16.;
/// Where the popover hangs before the status box has been laid out: under the title bar.
const TOP: f32 = 44.;

/// `Remove index.lock…` as the entry offers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockOffer {
    /// Pressable; `note` is why the last press could not go ahead, if it could not.
    Ready { note: Option<String> },
    /// Not pressable now, and why: a `git` of Cairn's is running in the repository.
    Blocked(String),
}

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
    /// `Remove index.lock…`, where R12.4 offers it.
    pub lock: Option<LockOffer>,
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
    on_remove_lock: EventHandler<usize>,
    on_close: EventHandler<()>,
    /// The status box's laid-out left edge and bottom, in the window: where it hangs from.
    anchor: Option<(f32, f32)>,
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
            anchor: None,
            key: DiffKey::None,
        }
    }

    /// Hangs it from the status box, whose laid-out left edge and bottom are `left` and `bottom`
    /// in the window (the user's decision A, 2026-10-09).
    pub fn anchor(mut self, anchor: Option<(f32, f32)>) -> Self {
        self.anchor = anchor;
        self
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

    /// `Remove index.lock…` pressed, on the entry at this place: what it would cost is read
    /// now, at the press.
    pub fn on_remove_lock(mut self, on_remove: impl Into<EventHandler<usize>>) -> Self {
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
        self.entries == other.entries
            && self.selected == other.selected
            && self.anchor == other.anchor
            && self.key == other.key
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
/// changed in place — and, shown whole, its confirmed prompt as the first row.
#[derive(Clone)]
struct Lines {
    lines: Rc<Vec<ActivityLine>>,
    prompt: Option<(String, State<f32>)>,
}

impl PartialEq for Lines {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.lines, &other.lines)
            && self.prompt.as_ref().map(|(text, _)| text)
                == other.prompt.as_ref().map(|(text, _)| text)
    }
}

/// Whether `prompt` runs past its four-line cut, by its characters at the pane's width.
fn longer_than_its_cut(prompt: &str) -> bool {
    prompt.chars().count() as f32 > PROMPT_CHARS_PER_LINE * PROMPT_LINES as f32
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
    // Shown whole, the prompt is the first row, as tall as it lays out.
    let index = match &lines.prompt {
        Some((prompt, height)) if item.index == 0 => {
            let mut height = *height;
            return rect()
                .key(usize::MAX)
                .width(Size::fill())
                .height(Size::px(item.size))
                .padding(Gaps::new(4., 6., 4., 6.))
                .child(
                    label()
                        .text(prompt.clone())
                        .width(Size::fill())
                        .font_size(12.)
                        .color(colours.text_primary)
                        .on_sized(move |e: Event<SizedEventData>| {
                            height.set_if_modified(e.area.height() + 8.);
                        }),
                )
                .into();
        }
        Some(_) => item.index - 1,
        None => item.index,
    };
    let (text, colour) = match lines.lines.get(index) {
        Some(ActivityLine::Ran(command)) => (command.clone(), colours.text_primary),
        Some(ActivityLine::Output(output)) => (output.clone(), colours.text_secondary),
        None => (String::new(), colours.text_secondary),
    };
    rect()
        .key(item.index)
        .min_width(Size::fill())
        .height(Size::px(item.size))
        .padding(Gaps::new(0., 6., 0., 6.))
        .child(
            label()
                .text(text)
                .max_lines(1)
                .font_family(DIFF_FONT_FAMILY)
                .font_size(12.)
                .color(colour),
        )
        .into()
}

impl Component for ActivityPopover {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        let close = self.on_close.clone();
        let closing = self.on_close.clone();
        // Which entry's prompt is shown whole, by place; another selection cuts it again.
        let mut whole = use_state(|| None::<usize>);
        let prompt_height = use_state(|| 0f32);
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
        let selected = self.selected;
        let right = self.entries.get(self.selected).map(|entry| {
            let on_replaced = self.on_replaced.clone();
            let on_remove = self.on_remove_lock.clone();
            let replaced = entry.replaced;
            let lock = entry.lock.clone();
            let shown_whole = *whole.read() == Some(selected) && entry.prompt.is_some();
            let whole_prompt = entry.prompt.clone().filter(|_| shown_whole);
            let rows = entry.lines.len() + usize::from(whole_prompt.is_some());
            let size = if let Some(prompt) = &whole_prompt {
                let height = prompt_height;
                // A first measure, until the prompt is laid out and measures itself.
                let estimate = (prompt.chars().count() as f32 / PROMPT_CHARS_PER_LINE).ceil()
                    * PROMPT_LINE_HEIGHT
                    + 8.;
                ItemSize::Dynamic(Callback::new(move |index: usize| {
                    if index > 0 {
                        LINE_ROW_HEIGHT
                    } else {
                        match *height.peek() {
                            measured if measured > 0. => measured,
                            _ => estimate,
                        }
                    }
                }))
            } else {
                ItemSize::Fixed(LINE_ROW_HEIGHT)
            };
            let lines = VirtualScrollView::new_with_data(
                Lines {
                    lines: entry.lines.clone(),
                    prompt: whole_prompt.map(|prompt| (prompt, prompt_height)),
                },
                line_row,
            )
            .length(rows)
            .item_size(size)
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
                    let long = longer_than_its_cut(&prompt);
                    rect()
                        .width(Size::fill())
                        .child(
                            rect()
                                .horizontal()
                                .spacing(8.)
                                .cross_align(Alignment::center())
                                .child(
                                    label()
                                        .text(CONFIRMED_CAPTION)
                                        .font_size(11.)
                                        .color(colours.text_secondary),
                                )
                                .maybe_child((long || shown_whole).then(|| {
                                    Button::new()
                                        .compact()
                                        .on_press(move |_| {
                                            let open = *whole.peek() == Some(selected);
                                            whole.set((!open).then_some(selected));
                                        })
                                        .child(if shown_whole {
                                            SHOW_LESS_CAPTION
                                        } else {
                                            SHOW_ALL_CAPTION
                                        })
                                })),
                        )
                        .maybe_child((!shown_whole).then(|| {
                            label()
                                .text(prompt)
                                .max_lines(PROMPT_LINES)
                                .text_overflow(TextOverflow::Ellipsis)
                                .font_size(12.)
                                .color(colours.text_primary)
                        }))
                }))
                .maybe_child((replaced.is_some() || lock.is_some()).then(|| {
                    rect()
                        .horizontal()
                        .spacing(8.)
                        .cross_align(Alignment::center())
                        .maybe_child(replaced.map(|commit| {
                            Button::new()
                                .compact()
                                .on_press(move |_| on_replaced.call(commit))
                                .child(SHOW_REPLACED_CAPTION)
                        }))
                        .maybe_child(lock.clone().map(|offer| {
                            let ready = matches!(offer, LockOffer::Ready { .. });
                            Button::new()
                                .compact()
                                .enabled(ready)
                                .on_press(move |_| {
                                    if ready {
                                        on_remove.call(selected);
                                    }
                                })
                                .child(REMOVE_LOCK_CAPTION)
                        }))
                        .maybe_child(
                            lock.and_then(|offer| match offer {
                                LockOffer::Ready { note } => note,
                                LockOffer::Blocked(why) => Some(why),
                            })
                            .map(|why| {
                                label()
                                    .text(why)
                                    .max_lines(2)
                                    .font_size(12.)
                                    .color(colours.text_secondary)
                            }),
                        )
                }))
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::flex(1.))
                        .border(Border::new().fill(colours.border).width(1.))
                        .child(lines),
                )
        });

        // Hung from the status box (the user's decision A): its left edge under the box's, kept
        // inside the window's width, the arrow's tip over the box wherever the panel had to
        // move to fit.
        let room = Platform::get().root_size.read().width;
        let (box_left, box_bottom) = self.anchor.unwrap_or((MARGIN, TOP - ARROW / 2.));
        let (panel_left, width, arrow_left) = hung(box_left, room);
        let panel_top = box_bottom + ARROW / 2. + 2.;

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
            // The arrow, under the panel, so only its upper half shows.
            .child(
                rect()
                    .position(
                        Position::new_global()
                            .top(panel_top - ARROW / 2.)
                            .left(panel_left + arrow_left),
                    )
                    .width(Size::px(ARROW))
                    .height(Size::px(ARROW))
                    .rotate(45.)
                    .background(colours.background)
                    .border(Border::new().fill(colours.border).width(1.)),
            )
            .child(
                rect()
                    .position(Position::new_global().top(panel_top).left(panel_left))
                    .child(
                        rect()
                            .a11y_role(AccessibilityRole::Dialog)
                            .a11y_modal(true)
                            .width(Size::px(width))
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

/// Where the panel hangs from a status box whose left edge is at `box_left` in a window `room`
/// wide: the panel's left edge and width — under the box's, as wide as it is made, both kept
/// inside the window — and the arrow's place along it, its tip over the box.
pub fn hung(box_left: f32, room: f32) -> (f32, f32, f32) {
    let width = WIDTH.min((room - 2. * MARGIN).max(LIST_WIDTH));
    let left = box_left.min(room - MARGIN - width).max(MARGIN);
    let arrow = (box_left + ARROW_INSET - ARROW / 2. - left).clamp(MARGIN, width - MARGIN - ARROW);
    (left, width, arrow)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The user's decision A: the panel's left edge sits under the box's, and in a window too
    /// narrow for it there, it moves left to fit while the arrow stays over the box; it never
    /// leaves the window. Caught by: a panel centred, or hanging off the window's right edge.
    #[test]
    fn the_panel_hangs_under_the_box_inside_the_window() {
        let (left, width, arrow) = hung(120., 1440.);
        assert_eq!((left, width), (120., WIDTH));
        assert_eq!(left + arrow + ARROW / 2., 120. + ARROW_INSET);
        let (left, width, arrow) = hung(700., 1200.);
        assert!(left + width <= 1200. - MARGIN, "off the right edge");
        assert_eq!(
            left + arrow + ARROW / 2.,
            700. + ARROW_INSET,
            "the arrow left the box"
        );
        let (left, width, _) = hung(10., 500.);
        assert!(left >= MARGIN && left + width <= 500. - MARGIN + 0.01);
    }
}
