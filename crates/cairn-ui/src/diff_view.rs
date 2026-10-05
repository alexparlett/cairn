//! The diff view (PRD R6.1, R6.4, R6.5, R6.9): one file's diff as `git diff` prints it,
//! drawn row by row through the virtualising view — unified (`unified_rows`) or side by side
//! (`side_by_side_rows`), as the shared setting says, both from one prepared answer.
//!
//! **What is built once, and what per row.** [`ShownDiff`] (`cairn-model`) is built once per
//! answer, on the worker that answered it (phase 07): both rows' indexes, proportional to the
//! changes, whether ignoring whitespace hides a change, the gutter's digits and the widest
//! line as drawn (one pass over the drawn bytes, a line past the long-line limit counted to
//! its cut). Toggling side-by-side therefore builds nothing. A frame builds the rows in view and nothing else: per row, a search of the
//! index, one search of the intra-line ranges, and the line's drawn text — at most
//! [`LINE_CUT_BYTES`](crate::LINE_CUT_BYTES) of it — never growing with the file, the line
//! or the scroll offset. Every row has one height, [`DIFF_ROW_HEIGHT`].
//!
//! **Horizontal extent.** Unified: every row is as wide as the view, or as
//! [`content_width`] where that is wider — the widest drawn line's columns at IBM
//! Plex Mono's advance, an upper bound measured once — so the horizontal scrollbar does not
//! change as rows scroll in and out. Side by side: each column is half the view and the
//! extent is the widest line's overflow past its column (`side_by_side_rows`). A line drawn
//! wider than its columns predict (a glyph the font lacks, drawn by a wider fallback) can run
//! past that extent. Long lines scroll sideways, never wrap (wrap is issue #34).

use std::ops::Range;

use cairn_model::{ChangeStops, DisplayOverlay, HunkHeader, ShownDiff};
use freya::prelude::*;

use crate::accelerators;
use crate::diff_line_text::cut_marker;
use crate::diff_palette::GROUND;
use crate::diff_row_parts::{
    ADVANCE, SEPARATOR_WIDTH, TEXT_END_PADDING, TEXT_PADDING, number_width,
};
use crate::side_by_side_rows::Columns;
use crate::{side_by_side_rows, unified_rows};

/// Every row of the diff is this tall: Fork's, measured — a 17 pt pitch for Menlo at 11 pt
/// on the Mac (Finding 24, a vendor screenshot at 2×, May 2026), the hunk header the same.
/// Fork's Windows pitch is not established at a known scale.
pub const DIFF_ROW_HEIGHT: f32 = 17.0;

/// [`DIFF_ROW_HEIGHT`] in whole pixels, for scroll arithmetic: a row's offset is computed in
/// integers, which an `f32` cannot hold exactly past 2^24 pixels — about 987,000 rows.
const ROW_PIXELS: i64 = 17;
const _: () = assert!(ROW_PIXELS as f32 == DIFF_ROW_HEIGHT);

/// git's marker for a line that did not end, drawn as its own row.
pub const NO_NEWLINE_AT_END: &str = "\\ No newline at end of file";

/// Rows a change moved to sits below the top of the view, so the line above it shows.
const LEAD_ROWS: usize = 1;

/// How wide a number column of `shown` is.
pub(crate) fn number_column_width(shown: &ShownDiff) -> f32 {
    number_width(shown.number_digits())
}

/// How wide a line's text of `shown` is at most, the gap after the separator included — the
/// widest drawn line, and the cut marker when a line is cut: what a side-by-side column
/// slides through.
pub fn text_width(shown: &ShownDiff) -> f32 {
    let marker = if shown.has_cut_line() {
        // At most the marker for the most a line can lose: a file is loaded only to the
        // load-anyway ceiling, so no line is cut by more.
        cut_marker(cairn_model::DiffLimits::LOAD_ANYWAY_BYTES as usize)
            .chars()
            .count()
    } else {
        0
    };
    TEXT_PADDING + (shown.widest_columns() + marker) as f32 * ADVANCE + TEXT_END_PADDING
}

/// How wide every unified row of `shown` is at least: the gutters, the separator and the
/// widest drawn line.
pub fn content_width(shown: &ShownDiff) -> f32 {
    2.0 * number_column_width(shown) + SEPARATOR_WIDTH + text_width(shown)
}

/// A hunk header as a row draws it: git's `@@ -a,b +c,d @@`, and after a space the function
/// context git printed for a hunk starting there.
pub(crate) fn header_words(header: HunkHeader, overlay: &DisplayOverlay) -> String {
    let mut words = header.to_string();
    if let Some(function) = overlay.function_context().of(header)
        && !function.is_empty()
    {
        words.push(' ');
        words.push_str(&String::from_utf8_lossy(function));
    }
    words
}

/// Where previous or next change last moved to, and the scroll it left the view at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeCursor {
    pub change: usize,
    pub scrolled_y: i32,
}

/// One step of previous or next change (R6.2) over `stops`, the view scrolled to
/// `scrolled_y`: from `cursor` while the view is still where that step left it, otherwise
/// from the top row in view — so the first press finds the first change at or below the
/// top, even one already in view. The new cursor says which change, and the scroll that
/// puts its first row [`LEAD_ROWS`] below the top; `None` when there is no change that way.
/// Rows and pixels are whole numbers throughout, so the answer is exact however deep.
pub fn step_change(
    stops: &ChangeStops,
    cursor: Option<ChangeCursor>,
    scrolled_y: i32,
    forward: bool,
) -> Option<ChangeCursor> {
    let held =
        cursor.filter(|cursor| cursor.scrolled_y == scrolled_y && cursor.change < stops.len());
    let top_row = usize::try_from((-i64::from(scrolled_y)).max(0) / ROW_PIXELS).unwrap_or(0);
    let change = match (held, forward) {
        (Some(held), true) => Some(held.change + 1).filter(|next| *next < stops.len()),
        (Some(held), false) => held.change.checked_sub(1),
        (None, true) => stops.first_from(top_row),
        (None, false) => stops.last_before(top_row),
    }?;
    let start = stops.start(change)?;
    let top = i64::try_from(start.saturating_sub(LEAD_ROWS))
        .unwrap_or(i64::MAX)
        .saturating_mul(ROW_PIXELS);
    Some(ChangeCursor {
        change,
        scrolled_y: i32::try_from(-top).unwrap_or(i32::MIN),
    })
}

/// One file's diff, unified or side by side. `shown` is a handle, not a copy: rows are read
/// from it as they are built. `scroll` is the application's, so previous and next change can
/// move the view; `current` is the change they last moved to, marked in the gutter.
pub struct DiffView {
    shown: Readable<ShownDiff>,
    scroll: ScrollController,
    current: Option<Range<usize>>,
    side_by_side: bool,
    key: DiffKey,
}

impl DiffView {
    pub fn new(shown: impl Into<Readable<ShownDiff>>, scroll: ScrollController) -> Self {
        Self {
            shown: shown.into(),
            scroll,
            current: None,
            side_by_side: false,
            key: DiffKey::None,
        }
    }

    /// The rows of the change last moved to.
    pub fn current(mut self, current: Option<Range<usize>>) -> Self {
        self.current = current;
        self
    }

    /// Side by side rather than unified (R6.1).
    pub fn side_by_side(mut self, side_by_side: bool) -> Self {
        self.side_by_side = side_by_side;
        self
    }
}

impl PartialEq for DiffView {
    fn eq(&self, other: &Self) -> bool {
        self.shown == other.shown
            && self.scroll == other.scroll
            && self.current == other.current
            && self.side_by_side == other.side_by_side
            && self.key == other.key
    }
}

impl std::fmt::Debug for DiffView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffView")
            .field("current", &self.current)
            .field("side_by_side", &self.side_by_side)
            .finish_non_exhaustive()
    }
}

impl KeyExt for DiffView {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// Where a row's parts go and how it is marked — everything a row is drawn with but the answer
/// it reads: what the diff view hands its rows, and what the Commit tab hands a file opened in
/// place under its row, so both draw one row the same way.
#[derive(Clone)]
pub(crate) struct RowGeometry {
    pub(crate) side_by_side: bool,
    /// The unified rows' least width.
    pub(crate) width: f32,
    pub(crate) number_width: f32,
    /// A line's text at its widest, the gap after the separator included.
    pub(crate) text_width: f32,
    /// The view's own width, which a side-by-side column is half of.
    pub(crate) view_width: f32,
    pub(crate) current: Option<Range<usize>>,
    /// Read as a side-by-side row is built, for the sideways scroll its text slides by.
    pub(crate) scroll: ScrollController,
}

impl RowGeometry {
    /// The geometry of `shown`'s rows in a view `view_width` wide scrolled by `scroll`.
    pub(crate) fn of(
        shown: &ShownDiff,
        side_by_side: bool,
        view_width: f32,
        current: Option<Range<usize>>,
        scroll: ScrollController,
    ) -> Self {
        Self {
            side_by_side,
            width: content_width(shown),
            number_width: number_column_width(shown),
            text_width: text_width(shown),
            view_width,
            current,
            scroll,
        }
    }
}

impl PartialEq for RowGeometry {
    fn eq(&self, other: &Self) -> bool {
        self.side_by_side == other.side_by_side
            && self.width == other.width
            && self.number_width == other.number_width
            && self.text_width == other.text_width
            && self.view_width == other.view_width
            && self.current == other.current
    }
}

/// What decides which rows are drawn, passed to the virtualising view: data captured in its
/// builder is invisible to its diffing.
#[derive(Clone, PartialEq)]
struct RowsData {
    shown: Readable<ShownDiff>,
    rows: usize,
    geometry: RowGeometry,
}

fn build_row(item: VirtualItem, data: &RowsData) -> Element {
    // Read, not peeked: the list redraws when the answer it shows is replaced.
    let shown = data.shown.read();
    draw_row(item.index, item.index, item.size, &shown, &data.geometry)
}

/// Row `row` of `shown`, `size` tall and keyed by `key`, unified or side by side as `geometry`
/// says.
pub(crate) fn draw_row(
    key: usize,
    row: usize,
    size: f32,
    shown: &ShownDiff,
    geometry: &RowGeometry,
) -> Element {
    if geometry.side_by_side {
        side_by_side_rows::build(key, row, size, shown, geometry)
    } else {
        unified_rows::build(key, row, size, shown, geometry)
    }
}

impl Component for DiffView {
    fn render(&self) -> impl IntoElement {
        let focus_id = use_a11y();
        let mut viewport = use_state(|| (0.0f32, 0.0f32));
        let view_width = viewport.read().0;
        // Reading subscribes the view to the answer it draws.
        let data = {
            let shown = self.shown.read();
            RowsData {
                shown: self.shown.clone(),
                rows: shown.rows(self.side_by_side),
                geometry: RowGeometry::of(
                    &shown,
                    self.side_by_side,
                    view_width,
                    self.current.clone(),
                    self.scroll,
                ),
            }
        };
        let rows = data.rows;
        // How far the view scrolls sideways: past the unified rows' width, or through a
        // side-by-side column's overflow.
        let geometry = &data.geometry;
        let width = if geometry.side_by_side {
            Columns::of(view_width, geometry.number_width, geometry.text_width, 0.0).row_width()
        } else {
            geometry.width
        };
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

#[cfg(test)]
mod tests {
    use cairn_model::{
        ChangeStatus, ChangedFile, ChangedRange, Context, DiffContent, DiffLine, FileDiff,
        LineSpan, RepoPath, TextDiff, UnifiedLayout, UnifiedRow, split_lines,
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

        let first = step_change(layout.stops(), None, 0, true).expect("a first change");
        assert_eq!(first.change, 0);
        assert_eq!(
            first.scrolled_y,
            -((row_of(first) - LEAD_ROWS) as f32 * DIFF_ROW_HEIGHT) as i32
        );
        let second =
            step_change(layout.stops(), Some(first), first.scrolled_y, true).expect("a second");
        assert_eq!(second.change, 1);
        let back =
            step_change(layout.stops(), Some(second), second.scrolled_y, false).expect("back");
        assert_eq!(back.change, 0);
        assert_eq!(
            step_change(layout.stops(), Some(back), back.scrolled_y, false),
            None
        );

        let mut cursor = back;
        for _ in 0..9 {
            cursor =
                step_change(layout.stops(), Some(cursor), cursor.scrolled_y, true).expect("next");
        }
        assert_eq!(cursor.change, 9);
        assert_eq!(
            step_change(layout.stops(), Some(cursor), cursor.scrolled_y, true),
            None
        );

        // Scrolled by hand to the row the sixth change starts on: the cursor is stale, and
        // next starts from what is in view.
        let sixth = layout.change_row(5).expect("a sixth change");
        let by_hand = -((sixth as f32) * DIFF_ROW_HEIGHT) as i32;
        assert_eq!(
            step_change(layout.stops(), Some(cursor), by_hand, true).map(|c| c.change),
            Some(5)
        );
        assert_eq!(
            step_change(layout.stops(), Some(cursor), by_hand, false).map(|c| c.change),
            Some(4)
        );
    }

    /// Phase 06 QA (T8): previous and next change stop at what is drawn. With whitespace
    /// ignored a whitespace-only change draws nothing and is never a stop, at a context
    /// and at the entire file; at the entire file every change of its one hunk is a stop.
    /// Each stop is the change's first row, a removed line at the change's own number,
    /// and previous retraces next. Caught by: stepping over the exact ranges under `-w` (a
    /// stop where nothing is drawn), or stopping once per hunk rather than per change.
    #[test]
    fn previous_and_next_change_stop_at_what_is_drawn() {
        let old: Vec<u8> = (0..60)
            .flat_map(|n| format!("l{n}\n").into_bytes())
            .collect();
        let new: Vec<u8> = (0..60)
            .flat_map(|n| match n {
                10 | 50 => format!("L{n}\n").into_bytes(),
                30 => format!(" l{n}\n").into_bytes(),
                _ => format!("l{n}\n").into_bytes(),
            })
            .collect();
        let at = |line: u32| ChangedRange::new(LineSpan::at(line, 1), LineSpan::at(line, 1));
        let shown = |overlay: DisplayOverlay, context: Context| {
            ShownDiff::new(
                FileDiff {
                    file: file(),
                    content: DiffContent::Text {
                        text: TextDiff::new(
                            split_lines(&old),
                            split_lines(&new),
                            vec![at(10), at(30), at(50)],
                        ),
                        overlay,
                    },
                },
                context,
            )
        };
        let ignoring = || DisplayOverlay::new(Some(vec![at(10), at(50)]), Vec::new());
        for (shown, stops) in [
            (shown(ignoring(), Context::lines(1)), &[10, 50][..]),
            (shown(ignoring(), Context::EntireFile), &[10, 50]),
            (
                shown(DisplayOverlay::none(), Context::EntireFile),
                &[10, 30, 50],
            ),
        ] {
            let layout = shown.layout().expect("text");
            let (text, overlay) = shown.text().expect("text");
            let stopped_at = |cursor: ChangeCursor| {
                let row = layout.change_row(cursor.change).expect("a change");
                match layout.row(text, overlay, row) {
                    Some(UnifiedRow::Removed { old, .. }) => old.one_based() - 1,
                    other => panic!("change {} starts on {other:?}", cursor.change),
                }
            };
            let mut forward = Vec::new();
            let mut cursor = step_change(layout.stops(), None, 0, true);
            while let Some(at) = cursor {
                forward.push(stopped_at(at));
                cursor = step_change(layout.stops(), Some(at), at.scrolled_y, true);
            }
            assert_eq!(forward, stops, "{:?}", shown.context());
            let last = layout.change_count().checked_sub(1).expect("a change");
            let mut backward = Vec::new();
            let mut cursor = Some(ChangeCursor {
                change: last,
                scrolled_y: 7,
            });
            while let Some(at) = cursor {
                backward.push(stopped_at(at));
                cursor = step_change(layout.stops(), Some(at), at.scrolled_y, false);
            }
            backward.reverse();
            assert_eq!(backward, stops, "{:?} backwards", shown.context());
        }
    }

    /// Phase 06 QA's obligation, met in phase 07: previous and next change compute rows and
    /// pixels in whole numbers, so a change a million rows down is found from the row really
    /// at the top and is put exactly one row below it. The scroll offsets here are odd past
    /// 2^24 pixels, which an `f32` cannot hold: the old arithmetic took the top row for the
    /// row above it (so previous change skipped the change just above the view) and put the
    /// change a pixel off. Caught by: any step of the arithmetic done in `f32`.
    #[test]
    fn a_change_a_million_rows_down_is_stepped_to_exactly() {
        let lines = 1_000_200u32;
        let old: Vec<DiffLine> = (0..lines).map(|_| DiffLine::terminated("")).collect();
        let mut new = old.clone();
        // Two one-line edits: one near the top, one about a million rows down.
        let (near, far) = (10u32, 999_998u32);
        new[near as usize] = DiffLine::terminated("x");
        new[far as usize] = DiffLine::terminated("y");
        let text = TextDiff::new(
            old,
            new,
            vec![
                ChangedRange::new(LineSpan::at(near, 1), LineSpan::at(near, 1)),
                ChangedRange::new(LineSpan::at(far, 1), LineSpan::at(far, 1)),
            ],
        );
        let layout = UnifiedLayout::exact(&text, Context::EntireFile);
        let start = layout.change_row(1).expect("the far change");
        assert!(
            start.is_multiple_of(2),
            "the fixture needs an even start: {start}"
        );
        // The view's top is the row after the change's first: previous change goes to it.
        let below = i32::try_from((start as i64 + 1) * ROW_PIXELS).expect("fits");
        assert_ne!(
            ((below as f32) / DIFF_ROW_HEIGHT) as usize,
            start + 1,
            "an f32 gets this top row right, so the test decides nothing"
        );
        let back = step_change(layout.stops(), None, -below, false).expect("a change above");
        assert_eq!(
            back.change, 1,
            "previous change skipped the change above the view"
        );
        assert_eq!(
            i64::from(back.scrolled_y),
            -((start as i64 - 1) * ROW_PIXELS),
            "the change is not exactly a row below the top"
        );
        assert!(matches!(
            layout.row(&text, &DisplayOverlay::none(), start),
            Some(UnifiedRow::Removed { .. })
        ));
        // And next change from just above it reaches it, to the pixel.
        let above = back.scrolled_y + 1;
        let next = step_change(layout.stops(), None, above, true).expect("the change below");
        assert_eq!(next, back);
    }

    /// The answer's extent is measured once: every row is the gutters, the separator, the
    /// gap after it and the widest line wide, whichever rows are built.
    #[test]
    fn the_width_is_the_widest_line_whatever_is_in_view() {
        let shown = every_tenth();
        let expected = 2.0 * number_width(3)
            + SEPARATOR_WIDTH
            + TEXT_PADDING
            + 3.0 * ADVANCE
            + TEXT_END_PADDING;
        assert_eq!(content_width(&shown), expected);
    }
}
