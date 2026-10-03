//! The unified diff view (PRD R6.4, R6.5): one file's diff as `git diff` prints it, drawn
//! row by row through the virtualising view.
//!
//! **A row.** An old and a new line-number gutter, a thin separator, then — tinted for a
//! changed line — a marker column (`-`, `+`, or blank) and the line. A removed line leaves
//! the new gutter blank and an added one the old, so the gutter and the marker say what the
//! tint says (L11). A hunk header is git's `@@ -a,b +c,d @@` and the function context git
//! printed after it, in muted text at the same height with no band and no button (Fork,
//! Finding 13); git's `\ No newline at end of file` is a muted row of its own. Every row has
//! one height, [`DIFF_ROW_HEIGHT`], which is what keeps the list O(viewport).
//!
//! **What is built once, and what per row.** [`ShownDiff`] is built once per answer, when
//! it arrives: the rows' index ([`UnifiedLayout`], proportional to the changes), whether
//! ignoring whitespace hides a change, the gutter's digits and the widest line (one pass
//! over the bytes). A frame then builds the rows in view and nothing else: per row, a search
//! of the index, one search of the intra-line ranges, and the line's own text — work that
//! grows with that line, never with the file or the scroll offset.
//!
//! **Horizontal extent.** Every row is as wide as the view, or as
//! [`ShownDiff::content_width`] where that is wider — the widest line's columns at IBM Plex
//! Mono's advance, an upper bound measured once — so the horizontal scrollbar does not
//! change as rows scroll in and out (`the_horizontal_extent_is_the_widest_lines_wherever_the_
//! view_is`). A line drawn wider than its columns predict (a glyph the font lacks, drawn by
//! a wider fallback) can run past that extent. The gutter scrolls sideways with the text.
//! Long lines scroll sideways, never wrap (wrap is issue #34).

use std::ops::Range;

use cairn_model::{
    ByteRange, Context, DisplayOverlay, FileDiff, LineNumber, TextDiff, UnifiedLayout, UnifiedRow,
};
use freya::prelude::*;

use crate::accelerators;
use crate::diff_line_text::{shown_line, widest_columns};
use crate::diff_palette::{
    ADDED_EMPHASIS, ADDED_TINT, CURRENT_CHANGE, DIFF_FONT_FAMILY, DIFF_FONT_SIZE, DIFF_MUTED,
    DIFF_TEXT, GROUND, GUTTER_SEPARATOR, MONO_ADVANCE_EM, REMOVED_EMPHASIS, REMOVED_TINT,
};

/// Every row of the diff is this tall: Fork's, measured — a 17 pt pitch for Menlo at 11 pt
/// on the Mac (Finding 24, a vendor screenshot at 2×, May 2026), the hunk header the same.
/// Fork's Windows pitch is not established at a known scale.
pub const DIFF_ROW_HEIGHT: f32 = 17.0;

/// git's marker for a line that did not end, drawn as its own row.
pub const NO_NEWLINE_AT_END: &str = "\\ No newline at end of file";

/// Rows a change moved to sits below the top of the view, so the line above it shows.
const LEAD_ROWS: usize = 1;

const ADVANCE: f32 = DIFF_FONT_SIZE * MONO_ADVANCE_EM;
const NUMBER_PADDING: f32 = 6.0;
const SEPARATOR_WIDTH: f32 = 1.0;
const CURRENT_SEPARATOR_WIDTH: f32 = 3.0;
const MARKER_WIDTH: f32 = 2.0 * ADVANCE + 6.0;
const TEXT_END_PADDING: f32 = 24.0;

/// One file's diff as the unified view draws it: built once when the answer arrives, at the
/// context it was asked at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownDiff {
    diff: FileDiff,
    context: Context,
    /// `None` for a file that is not text.
    layout: Option<UnifiedLayout>,
    hides_changes: bool,
    widest_columns: usize,
    number_digits: usize,
}

impl ShownDiff {
    /// Builds what the view needs of `diff`, asked at `context`: proportional to its changes
    /// and one pass over its bytes, never repeated per frame.
    pub fn new(diff: FileDiff, context: Context) -> Self {
        let (layout, hides_changes, widest_columns, number_digits) =
            match (diff.text(), diff.overlay()) {
                (Some(text), Some(overlay)) => (
                    Some(UnifiedLayout::shown(text, overlay, context)),
                    overlay.hides_a_change(text),
                    widest_columns(text.old_lines().iter().chain(text.new_lines())),
                    digits(text.old_lines().len().max(text.new_lines().len())),
                ),
                _ => (None, false, 0, 1),
            };
        Self {
            diff,
            context,
            layout,
            hides_changes,
            widest_columns,
            number_digits,
        }
    }

    pub fn diff(&self) -> &FileDiff {
        &self.diff
    }

    /// The answer itself, for a caller letting go of it.
    pub fn into_diff(self) -> FileDiff {
        self.diff
    }

    pub fn context(&self) -> Context {
        self.context
    }

    /// The rows, for a text diff.
    pub fn layout(&self) -> Option<&UnifiedLayout> {
        self.layout.as_ref()
    }

    pub fn row_count(&self) -> usize {
        self.layout.as_ref().map_or(0, UnifiedLayout::len)
    }

    /// Whether ignoring whitespace hides a change that is really there (R6.7) — false when
    /// whitespace is not ignored, and false when ignoring it hides nothing.
    pub fn hides_changes(&self) -> bool {
        self.hides_changes
    }

    /// How wide every row is at least: the gutters, the marker column and the widest line.
    pub fn content_width(&self) -> f32 {
        gutter_width(self.number_digits)
            + SEPARATOR_WIDTH
            + MARKER_WIDTH
            + self.widest_columns as f32 * ADVANCE
            + TEXT_END_PADDING
    }

    fn number_width(&self) -> f32 {
        number_width(self.number_digits)
    }

    /// The text diff and its overlay, for a file that is text.
    fn text(&self) -> Option<(&TextDiff, &DisplayOverlay)> {
        self.diff.text().zip(self.diff.overlay())
    }
}

fn digits(count: usize) -> usize {
    count.max(1).to_string().len()
}

fn number_width(digits: usize) -> f32 {
    digits as f32 * ADVANCE + 2.0 * NUMBER_PADDING
}

fn gutter_width(digits: usize) -> f32 {
    2.0 * number_width(digits)
}

/// Where previous or next change last moved to, and the scroll it left the view at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeCursor {
    pub change: usize,
    pub scrolled_y: i32,
}

/// One step of previous or next change (R6.2) over `layout`, the view scrolled to
/// `scrolled_y`: from `cursor` while the view is still where that step left it, otherwise
/// from the top row in view — so the first press finds the first change at or below the
/// top, even one already in view. The new cursor says which change, and the scroll that
/// puts its first row [`LEAD_ROWS`] below the top; `None` when there is no change that way.
pub fn step_change(
    layout: &UnifiedLayout,
    cursor: Option<ChangeCursor>,
    scrolled_y: i32,
    forward: bool,
) -> Option<ChangeCursor> {
    let held = cursor
        .filter(|cursor| cursor.scrolled_y == scrolled_y && cursor.change < layout.change_count());
    let top_row = (scrolled_y.saturating_neg().max(0) as f32 / DIFF_ROW_HEIGHT) as usize;
    let change = match (held, forward) {
        (Some(held), true) => Some(held.change + 1).filter(|next| *next < layout.change_count()),
        (Some(held), false) => held.change.checked_sub(1),
        (None, true) => layout.first_change_from(top_row),
        (None, false) => layout.previous_change_before(top_row),
    }?;
    let start = layout.change_row(change)?;
    let top = start.saturating_sub(LEAD_ROWS) as f32 * DIFF_ROW_HEIGHT;
    Some(ChangeCursor {
        change,
        scrolled_y: -(top as i32),
    })
}

/// The unified rows of one file's diff. `shown` is a handle, not a copy: rows are read from
/// it as they are built. `scroll` is the application's, so previous and next change can move
/// the view; `current` is the change they last moved to, marked in the gutter.
pub struct UnifiedDiffView {
    shown: Readable<ShownDiff>,
    scroll: ScrollController,
    current: Option<Range<usize>>,
    key: DiffKey,
}

impl UnifiedDiffView {
    pub fn new(shown: impl Into<Readable<ShownDiff>>, scroll: ScrollController) -> Self {
        Self {
            shown: shown.into(),
            scroll,
            current: None,
            key: DiffKey::None,
        }
    }

    /// The rows of the change last moved to.
    pub fn current(mut self, current: Option<Range<usize>>) -> Self {
        self.current = current;
        self
    }
}

impl PartialEq for UnifiedDiffView {
    fn eq(&self, other: &Self) -> bool {
        self.shown == other.shown
            && self.scroll == other.scroll
            && self.current == other.current
            && self.key == other.key
    }
}

impl std::fmt::Debug for UnifiedDiffView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedDiffView")
            .field("current", &self.current)
            .finish_non_exhaustive()
    }
}

impl KeyExt for UnifiedDiffView {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// What decides which rows are drawn, passed to the virtualising view: data captured in its
/// builder is invisible to its diffing.
#[derive(Clone)]
struct RowsData {
    shown: Readable<ShownDiff>,
    rows: usize,
    width: f32,
    number_width: f32,
    current: Option<Range<usize>>,
}

impl PartialEq for RowsData {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
            && self.width == other.width
            && self.number_width == other.number_width
            && self.current == other.current
    }
}

impl Component for UnifiedDiffView {
    fn render(&self) -> impl IntoElement {
        let focus_id = use_a11y();
        let mut viewport = use_state(|| (0.0f32, 0.0f32));
        // Reading subscribes the view to the answer it draws.
        let data = {
            let shown = self.shown.read();
            RowsData {
                shown: self.shown.clone(),
                rows: shown.row_count(),
                width: shown.content_width(),
                number_width: shown.number_width(),
                current: self.current.clone(),
            }
        };
        let (rows, width) = (data.rows, data.width);
        let scroll = self.scroll;

        rect()
            .expanded()
            .background(GROUND)
            .a11y_id(focus_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::List)
            .on_press(move |_| focus_id.request_focus())
            .on_sized(move |e: Event<SizedEventData>| {
                viewport.set_if_modified((e.area.width(), e.area.height()));
            })
            .on_key_down(move |e: Event<KeyboardEventData>| {
                let (shown_width, shown_height) = *viewport.peek();
                let limits = Limits {
                    rows,
                    width,
                    shown_width,
                    shown_height,
                };
                if scroll_by_key(&e, scroll, limits) {
                    e.stop_propagation();
                }
            })
            .child(
                VirtualScrollView::new_with_data_controlled(data, build_row, scroll)
                    .length(rows)
                    .item_size(DIFF_ROW_HEIGHT)
                    // The view's own key handling is the focused rect's, above.
                    .scroll_with_arrows(false)
                    .expanded(),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// How far the view can scroll.
#[derive(Debug, Clone, Copy)]
struct Limits {
    rows: usize,
    width: f32,
    shown_width: f32,
    shown_height: f32,
}

/// The keys of a focused list: the arrows by a row or a few columns, Page Up and Page Down by
/// a view, Home and End. A chord is left for whoever hears it — previous and next change
/// are the detail pane's.
fn scroll_by_key(
    e: &Event<KeyboardEventData>,
    mut scroll: ScrollController,
    limits: Limits,
) -> bool {
    if accelerators::is_chord(e) {
        return false;
    }
    let (x, y): (i32, i32) = scroll.into();
    let page = (limits.shown_height - DIFF_ROW_HEIGHT).max(DIFF_ROW_HEIGHT);
    let lowest = -(limits.rows as f32 * DIFF_ROW_HEIGHT - limits.shown_height).max(0.0);
    let leftmost = -(limits.width - limits.shown_width).max(0.0);
    let (x, y) = (x as f32, y as f32);
    let (to_x, to_y) = match e.key {
        Key::Named(NamedKey::ArrowUp) => (x, y + DIFF_ROW_HEIGHT),
        Key::Named(NamedKey::ArrowDown) => (x, y - DIFF_ROW_HEIGHT),
        Key::Named(NamedKey::PageUp) => (x, y + page),
        Key::Named(NamedKey::PageDown) => (x, y - page),
        Key::Named(NamedKey::Home) => (x, 0.0),
        Key::Named(NamedKey::End) => (x, lowest),
        Key::Named(NamedKey::ArrowLeft) => (x + 4.0 * ADVANCE, y),
        Key::Named(NamedKey::ArrowRight) => (x - 4.0 * ADVANCE, y),
        _ => return false,
    };
    scroll.scroll_to_x(to_x.clamp(leftmost, 0.0) as i32);
    scroll.scroll_to_y(to_y.clamp(lowest, 0.0) as i32);
    true
}

/// What a line row draws: its two numbers, its marker, its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    Context,
    Removed,
    Added,
}

fn build_row(item: VirtualItem, data: &RowsData) -> Element {
    let row = rect()
        .key(item.index)
        .horizontal()
        .content(Content::Flex)
        // As wide as the view, or as the widest line where that is wider: the tint reaches
        // the edge, and the extent is the answer's, whichever rows are built.
        .width(Size::fill())
        .min_width(Size::px(data.width))
        .height(Size::px(item.size));
    // Read, not peeked: the list redraws when the answer it shows is replaced.
    let shown = data.shown.read();
    let Some((text, overlay)) = shown.text() else {
        return row.into();
    };
    let Some(drawn) = shown
        .layout()
        .and_then(|layout| layout.row(text, overlay, item.index))
    else {
        // The count and the answer can disagree for one frame; an empty row of the right
        // height stands in.
        return row.into();
    };
    let current = data
        .current
        .as_ref()
        .is_some_and(|rows| rows.contains(&item.index));
    let number_width = data.number_width;
    match drawn {
        UnifiedRow::Header(header) => {
            let mut words = header.to_string();
            if let Some(function) = overlay.function_context().of(header)
                && !function.is_empty()
            {
                words.push(' ');
                words.push_str(&String::from_utf8_lossy(function));
            }
            note_row(row, number_width, current, words)
        }
        UnifiedRow::Context { old, new, line } => line_row(
            row,
            number_width,
            current,
            (Some(old), Some(new)),
            LineKind::Context,
            line.bytes(),
            &[],
        ),
        UnifiedRow::Removed { old, line } => line_row(
            row,
            number_width,
            current,
            (Some(old), None),
            LineKind::Removed,
            line.bytes(),
            overlay.on_removed_line(old),
        ),
        UnifiedRow::Added { new, line } => line_row(
            row,
            number_width,
            current,
            (None, Some(new)),
            LineKind::Added,
            line.bytes(),
            overlay.on_added_line(new),
        ),
        UnifiedRow::NoNewlineAtEnd => {
            note_row(row, number_width, current, NO_NEWLINE_AT_END.to_owned())
        }
    }
}

/// The two numbers, right-aligned, on the diff's ground: a side the line is not on is blank.
fn gutter(numbers: (Option<LineNumber>, Option<LineNumber>), number_width: f32) -> Rect {
    let number = |at: Option<LineNumber>| {
        label()
            .width(Size::px(number_width))
            .padding(Gaps::new(0., NUMBER_PADDING, 0., NUMBER_PADDING))
            .text_align(TextAlign::End)
            .max_lines(1)
            .font_family(DIFF_FONT_FAMILY)
            .font_size(DIFF_FONT_SIZE)
            .color(DIFF_MUTED)
            .text(
                at.map(|line| line.one_based().to_string())
                    .unwrap_or_default(),
            )
    };
    rect()
        .horizontal()
        .height(Size::fill())
        .cross_align(Alignment::Center)
        .child(number(numbers.0))
        .child(number(numbers.1))
}

fn separator(current: bool) -> Rect {
    rect()
        .width(Size::px(if current {
            CURRENT_SEPARATOR_WIDTH
        } else {
            SEPARATOR_WIDTH
        }))
        .height(Size::fill())
        .background(if current {
            CURRENT_CHANGE
        } else {
            GUTTER_SEPARATOR
        })
}

/// The row's text, in the diff's typeface, one line, never wrapped.
fn line_text(words: String, colour: Color) -> Paragraph {
    paragraph()
        .span(words)
        .font_family(DIFF_FONT_FAMILY)
        .font_size(DIFF_FONT_SIZE)
        .color(colour)
        .max_lines(1)
        .vertical_align(VerticalAlign::Center)
        .height(Size::fill())
}

fn line_row(
    row: Rect,
    number_width: f32,
    current: bool,
    numbers: (Option<LineNumber>, Option<LineNumber>),
    kind: LineKind,
    bytes: &[u8],
    ranges: &[ByteRange],
) -> Element {
    let (marker, tint, emphasis) = match kind {
        LineKind::Context => (" ", GROUND, GROUND),
        LineKind::Removed => ("-", REMOVED_TINT, REMOVED_EMPHASIS),
        LineKind::Added => ("+", ADDED_TINT, ADDED_EMPHASIS),
    };
    let line = shown_line(bytes, ranges);
    row.child(gutter(numbers, number_width))
        .child(separator(current))
        .child(
            rect()
                .horizontal()
                .width(Size::flex(1.))
                .height(Size::fill())
                .cross_align(Alignment::Center)
                .background(tint)
                .child(
                    label()
                        .width(Size::px(MARKER_WIDTH))
                        .padding(Gaps::new(0., 0., 0., 4.))
                        .max_lines(1)
                        .font_family(DIFF_FONT_FAMILY)
                        .font_size(DIFF_FONT_SIZE)
                        .color(DIFF_TEXT)
                        .text(marker),
                )
                .child(
                    line_text(line.text, DIFF_TEXT)
                        .highlights(Some(line.highlights))
                        .highlight_color(emphasis),
                ),
        )
        .into()
}

/// A hunk header, or git's end-of-file marker: muted text where the line's text goes, no
/// numbers, no tint, no band.
fn note_row(row: Rect, number_width: f32, current: bool, words: String) -> Element {
    row.child(gutter((None, None), number_width))
        .child(separator(current))
        .child(
            rect()
                .horizontal()
                .width(Size::flex(1.))
                .height(Size::fill())
                .cross_align(Alignment::Center)
                .child(rect().width(Size::px(MARKER_WIDTH)))
                .child(line_text(words, DIFF_MUTED)),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use cairn_model::{
        ChangeStatus, ChangedFile, ChangedRange, DiffContent, LineSpan, RepoPath, split_lines,
    };

    use super::*;

    fn file() -> ChangedFile {
        ChangedFile {
            status: ChangeStatus::Modified,
            old_path: RepoPath::from("f.txt"),
            new_path: RepoPath::from("f.txt"),
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        }
    }

    /// Ten changes, one every ten lines, of a hundred lines.
    fn every_tenth() -> ShownDiff {
        let old: Vec<u8> = (0..100)
            .flat_map(|n| format!("l{n}\n").into_bytes())
            .collect();
        let new: Vec<u8> = (0..100)
            .flat_map(|n| {
                if n % 10 == 5 {
                    format!("L{n}\n").into_bytes()
                } else {
                    format!("l{n}\n").into_bytes()
                }
            })
            .collect();
        let changes = (0..10)
            .map(|k| ChangedRange::new(LineSpan::at(k * 10 + 5, 1), LineSpan::at(k * 10 + 5, 1)))
            .collect();
        ShownDiff::new(
            FileDiff {
                file: file(),
                content: DiffContent::Text {
                    text: TextDiff::new(split_lines(&old), split_lines(&new), changes),
                    overlay: DisplayOverlay::none(),
                },
            },
            Context::lines(1),
        )
    }

    /// Next from the top goes to the first change even when it is in view (Fork's reported
    /// bug is skipping it, TrackerWin #2393); then each step moves one change; previous
    /// comes back; past either end nothing moves. A scroll away starts again from the top
    /// row in view. Caught by: stepping from a stale cursor, or skipping the change in view.
    #[test]
    fn previous_and_next_change_step_one_change_from_where_the_view_is() {
        let shown = every_tenth();
        let layout = shown.layout().expect("text");
        assert_eq!(layout.change_count(), 10);
        let row_of = |cursor: ChangeCursor| layout.change_row(cursor.change).expect("a change");

        let first = step_change(layout, None, 0, true).expect("a first change");
        assert_eq!(first.change, 0);
        assert_eq!(
            first.scrolled_y,
            -((row_of(first) - LEAD_ROWS) as f32 * DIFF_ROW_HEIGHT) as i32
        );
        let second = step_change(layout, Some(first), first.scrolled_y, true).expect("a second");
        assert_eq!(second.change, 1);
        let back = step_change(layout, Some(second), second.scrolled_y, false).expect("back");
        assert_eq!(back.change, 0);
        assert_eq!(
            step_change(layout, Some(back), back.scrolled_y, false),
            None
        );

        let mut cursor = back;
        for _ in 0..9 {
            cursor = step_change(layout, Some(cursor), cursor.scrolled_y, true).expect("next");
        }
        assert_eq!(cursor.change, 9);
        assert_eq!(
            step_change(layout, Some(cursor), cursor.scrolled_y, true),
            None
        );

        // Scrolled by hand to the row the sixth change starts on: the cursor is stale, and
        // next starts from what is in view.
        let sixth = layout.change_row(5).expect("a sixth change");
        let by_hand = -((sixth as f32) * DIFF_ROW_HEIGHT) as i32;
        assert_eq!(
            step_change(layout, Some(cursor), by_hand, true).map(|c| c.change),
            Some(5)
        );
        assert_eq!(
            step_change(layout, Some(cursor), by_hand, false).map(|c| c.change),
            Some(4)
        );
    }

    /// The answer's extent is measured once: every row is the gutters, the marker column
    /// and the widest line wide, whichever rows are built.
    #[test]
    fn the_width_is_the_widest_line_whatever_is_in_view() {
        let shown = every_tenth();
        let expected =
            gutter_width(3) + SEPARATOR_WIDTH + MARKER_WIDTH + 3.0 * ADVANCE + TEXT_END_PADDING;
        assert_eq!(shown.content_width(), expected);
    }
}
