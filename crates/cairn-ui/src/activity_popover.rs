//! Fork's Activity Manager (staging-and-commit R12.1, C29; `fork-observed-2026-10-10.md` `am1`,
//! `am2`), opened from the title bar's status box: on the left the session's operations, newest
//! first — each its one name, Fork's result line under it ("staged", "Switched to branch
//! 'topic'"), when it started, ⚠ on a failure and a shape of its own on a cancel, with a cancel
//! for a running one that can be cancelled, and no status word — and on the right the one
//! selected: its name; one of four words, running, succeeded, failed or cancelled, with its time
//! and how long it took (F8, which Fork lacks); a failed or cancelled operation's one sentence,
//! never cut; the prompt it confirmed, whole and wrapped, for an operation that ran (F3,
//! Cairn's addition); an amend's way back, "Show in Lost Commits" (F4); then one append-only
//! list of each `$ git …` it ran and what that said, each command's ending in one form.
//!
//! Both lists are virtualized, each row of one fixed height — the operations and the selected
//! one's lines, which are as long as git's output makes them — so neither builds more than a
//! viewport of rows (R12.1). An operation whose output passed its bound starts with "Earlier
//! output not kept.". Every line is the window's, already stripped of escape sequences and of a
//! URL's userinfo (R12.2): this file draws text it is given and reads nothing else. Escape and
//! a press outside close it. It hangs from the status box, its left edge under the box's and an
//! arrow pointing up at it, kept inside the window's width (the user's decision A,
//! 2026-10-09).

use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use cairn_model::Oid;
use freya::prelude::*;

use crate::diff_palette::DIFF_FONT_FAMILY;

/// The popover's title, Fork's word for its log.
pub const ACTIVITY_TITLE: &str = "Activity";
/// The way back from an amend (F4): Show Lost Commits turned on and the replaced commit selected.
pub const SHOW_IN_LOST_COMMITS_CAPTION: &str = "Show in Lost Commits";
/// The cancel beside a running operation that can be cancelled (Fork's ×).
pub const CANCEL_OPERATION_CAPTION: &str = "×";
/// What the popover says before any operation has run.
pub const NO_ACTIVITY: &str = "Nothing has run in this window yet.";
/// Said above the prompt an operation confirmed.
pub const CONFIRMED_CAPTION: &str = "Confirmed:";
/// The first line of an operation whose output passed its bound: its oldest lines went.
pub const EARLIER_NOT_KEPT: &str = "Earlier output not kept.";
/// Beside a failed operation's row, as Fork marks one.
pub const FAILED_GLYPH: &str = "⚠";
/// Beside a cancelled operation's row: a shape of its own, since the window tells kinds apart
/// by shape.
pub const CANCELLED_GLYPH: &str = "⊘";

const WIDTH: f32 = 860.;
const HEIGHT: f32 = 460.;
const LIST_WIDTH: f32 = 260.;
/// Kept from the window's edges, and the arrow from the panel's corners.
const MARGIN: f32 = 8.;
/// The arrow pointing at the status box: a square turned on its corner, half of it showing.
const ARROW: f32 = 12.;
/// How far the arrow's tip sits in from the box's left edge.
const ARROW_INSET: f32 = 18.;
/// One operation's row on the left.
pub const ENTRY_ROW_HEIGHT: f32 = 38.;
/// One line on the right.
pub const LINE_ROW_HEIGHT: f32 = 16.;
/// Where the popover hangs before the status box has been laid out: under the title bar.
const TOP: f32 = 44.;

/// One line of an operation's detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityLine {
    /// A `git` it ran, as Fork prints it: `$ git ...`.
    Ran(String),
    /// A line it wrote — stderr, a hook's output — or how a command ended.
    Output(String),
}

/// How an operation stands, in one of the four words (rule 5, R14.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl ActivityStatus {
    /// Its word, as the right pane's header says it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// What its row draws beside the time: ⚠ for a failure, a shape of its own for a cancel.
    fn glyph(self) -> Option<&'static str> {
        match self {
            Self::Failed => Some(FAILED_GLYPH),
            Self::Cancelled => Some(CANCELLED_GLYPH),
            Self::Running | Self::Succeeded => None,
        }
    }
}

/// An operation as the popover draws it, every text the window's.
#[derive(Debug, Clone)]
pub struct ActivityEntry {
    pub name: String,
    /// Fork's result line under its row, where there is one.
    pub result: Option<String>,
    pub status: ActivityStatus,
    /// When it started, as the window says it.
    pub started: String,
    /// How long it took, once it has ended.
    pub took: Option<String>,
    /// Its one sentence: why it failed or what a cancel left, or what a success did beyond its
    /// name (an amend whose replaced commit was pushed while it ran, a forced checkout).
    pub sentence: Option<String>,
    /// The prompt it confirmed, for a destructive operation that ran (R1.6, F3).
    pub prompt: Option<String>,
    /// The commit an amend replaced, which Show Lost Commits draws (R12.1, F4).
    pub replaced: Option<Oid>,
    /// A running operation that can be cancelled: a commit, an amend, a fetch.
    pub cancellable: bool,
    /// Its `$ git …` lines and what they said, oldest first: a snapshot of what the window
    /// keeps, shared, never copied — empty for an entry not selected.
    pub lines: Arc<VecDeque<ActivityLine>>,
    /// Older lines were let go of to keep it bounded: "Earlier output not kept." heads them.
    pub earlier_not_kept: bool,
}

/// Compared by the snapshot its lines are: a snapshot is replaced, never changed in place.
impl PartialEq for ActivityEntry {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.result == other.result
            && self.status == other.status
            && self.started == other.started
            && self.took == other.took
            && self.sentence == other.sentence
            && self.prompt == other.prompt
            && self.replaced == other.replaced
            && self.cancellable == other.cancellable
            && Arc::ptr_eq(&self.lines, &other.lines)
            && self.earlier_not_kept == other.earlier_not_kept
    }
}

/// The popover over `entries`, newest first, with `selected` drawn on the right.
pub struct ActivityPopover {
    entries: Rc<Vec<ActivityEntry>>,
    selected: usize,
    on_select: EventHandler<usize>,
    on_cancel: EventHandler<usize>,
    on_replaced: EventHandler<Oid>,
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

/// The selected operation's lines, compared by their snapshot — and, first when its oldest
/// lines went, "Earlier output not kept.".
#[derive(Clone)]
struct Lines {
    lines: Arc<VecDeque<ActivityLine>>,
    earlier_not_kept: bool,
}

impl PartialEq for Lines {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.lines, &other.lines) && self.earlier_not_kept == other.earlier_not_kept
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
        .spacing(6.)
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
                .maybe_child(entry.result.clone().map(|result| {
                    label()
                        .text(result)
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .font_size(11.)
                        .color(colours.text_secondary)
                })),
        )
        .child(
            label()
                .text(entry.started.clone())
                .max_lines(1)
                .font_size(11.)
                .color(colours.text_secondary),
        )
        .maybe_child(entry.status.glyph().map(|glyph| {
            label()
                .text(glyph)
                .font_size(12.)
                .color(colours.text_primary)
        }))
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
    let (text, colour) = match item.index.checked_sub(usize::from(lines.earlier_not_kept)) {
        None => (EARLIER_NOT_KEPT.to_owned(), colours.text_secondary),
        Some(index) => match lines.lines.get(index) {
            Some(ActivityLine::Ran(command)) => (command.clone(), colours.text_primary),
            Some(ActivityLine::Output(output)) => (output.clone(), colours.text_secondary),
            None => (String::new(), colours.text_secondary),
        },
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

/// The right pane's header line: the status word with when it started, and how long it took.
fn header_line(entry: &ActivityEntry) -> String {
    match &entry.took {
        Some(took) => format!("{} {} · took {took}", entry.status.word(), entry.started),
        None => format!("{} {}", entry.status.word(), entry.started),
    }
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
            let replaced = entry.replaced;
            let rows = entry.lines.len() + usize::from(entry.earlier_not_kept);
            let lines = VirtualScrollView::new_with_data(
                Lines {
                    lines: entry.lines.clone(),
                    earlier_not_kept: entry.earlier_not_kept,
                },
                line_row,
            )
            .length(rows)
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
                        .text(header_line(entry))
                        .max_lines(1)
                        .font_size(12.)
                        .color(colours.text_secondary),
                )
                // One sentence, never cut (rule 6).
                .maybe_child(entry.sentence.clone().map(|sentence| {
                    label()
                        .text(sentence)
                        .width(Size::fill())
                        .font_size(12.)
                        .color(colours.text_primary)
                }))
                // The prompt it confirmed, whole and wrapped (F3): laid out above the lines, so
                // their rows keep one fixed height.
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
                                .width(Size::fill())
                                .font_size(12.)
                                .color(colours.text_primary),
                        )
                }))
                .maybe_child(replaced.map(|commit| {
                    Button::new()
                        .compact()
                        .on_press(move |_| on_replaced.call(commit))
                        .child(SHOW_IN_LOST_COMMITS_CAPTION)
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
