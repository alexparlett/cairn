//! Several files' diffs drawn together (staging-and-commit R8.1, the user's decision of
//! 2026-10-09: with several paths selected in Local Changes, the diff draws the selected
//! files' diffs together, as Fork does): one virtualised list of rows of one height — each
//! file's own row, its path, then its diff's rows, unified or side by side as the shared
//! setting says, or the notice standing in their place, or that it is being read or could not
//! be — laid out by the Commit tab's table of files opened in place ([`Expansion`]), so a row
//! is placed by a binary search over the files however many are drawn, and one viewport of
//! rows is built however long they are.
//!
//! Handed a [`Gesture`], it draws the staging gesture over its rows as the single file's diff
//! view does (`staging_gesture`): a chunk, a drag and its actions are each one file's — a drag
//! stays inside the file it began in — and name that file by its place among the files drawn.

use cairn_model::{RepoPath, ShownDiff};
use freya::prelude::*;

use crate::commit_tab::notice_line;
use crate::diff_notice::{DiffNotice, NoticeRow, notice_rows};
use crate::diff_palette::{
    DIFF_FONT_FAMILY, DIFF_FONT_SIZE, DIFF_MUTED, DIFF_TEXT, GROUND, HEADER_BAR,
};
use crate::diff_view::{
    DIFF_ROW_HEIGHT, Limits, RowGeometry, content_width, draw_row, number_column_width,
    scroll_by_key, text_width,
};
use crate::expansion::{Expansion, Item, Opened};
use crate::side_by_side_rows::Columns;
use crate::staging_gesture::{
    Gesture, GestureLayer, Host, Source, use_drag_ends_on_focus_lost, use_pointer,
};

/// Said under a file drawn together while its diff is on its way.
pub const READING_FILE_DIFF: &str = "Reading the diff…";

/// The diffs of several files, each under its own row. `files` holds what each draws, by its
/// place among `paths`; both are handles, read by index as rows are built.
pub struct StackedDiff {
    files: Readable<Expansion>,
    paths: Readable<Vec<RepoPath>>,
    scroll: ScrollController,
    side_by_side: bool,
    gesture: Option<Gesture>,
    key: DiffKey,
}

impl StackedDiff {
    pub fn new(
        files: impl Into<Readable<Expansion>>,
        paths: impl Into<Readable<Vec<RepoPath>>>,
        scroll: ScrollController,
    ) -> Self {
        Self {
            files: files.into(),
            paths: paths.into(),
            scroll,
            side_by_side: false,
            gesture: None,
            key: DiffKey::None,
        }
    }

    pub fn side_by_side(mut self, side_by_side: bool) -> Self {
        self.side_by_side = side_by_side;
        self
    }

    /// The staging gesture over the rows (staging-and-commit R9).
    pub fn gesture(mut self, gesture: Option<Gesture>) -> Self {
        self.gesture = gesture;
        self
    }
}

impl PartialEq for StackedDiff {
    fn eq(&self, other: &Self) -> bool {
        self.files == other.files
            && self.paths == other.paths
            && self.scroll == other.scroll
            && self.side_by_side == other.side_by_side
            && self.gesture == other.gesture
            && self.key == other.key
    }
}

impl std::fmt::Debug for StackedDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StackedDiff")
            .field("side_by_side", &self.side_by_side)
            .finish_non_exhaustive()
    }
}

impl KeyExt for StackedDiff {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// What decides which rows are drawn, passed to the virtualising view.
#[derive(Clone, PartialEq)]
struct StackedData {
    files: Readable<Expansion>,
    paths: Readable<Vec<RepoPath>>,
    side_by_side: bool,
    rows: usize,
    view_width: f32,
    scroll: ScrollController,
}

/// How wide the widest file's rows are, in a view `view_width` wide: one pass over the files
/// drawn, done as the view renders — proportional to the files, never to their lines.
fn widest(expansion: &Expansion, side_by_side: bool, view_width: f32) -> f32 {
    expansion
        .iter()
        .filter_map(|(_, opened)| match opened {
            Opened::Shown(shown) if side_by_side => Some(
                Columns::of(
                    view_width,
                    number_column_width(shown),
                    text_width(shown),
                    0.0,
                )
                .row_width(),
            ),
            Opened::Shown(shown) => Some(content_width(shown)),
            Opened::Reading | Opened::Failed(_) => None,
        })
        .fold(view_width, f32::max)
}

impl Component for StackedDiff {
    fn render(&self) -> impl IntoElement {
        let focus_id = use_a11y();
        let mut viewport = use_state(|| (0.0f32, 0.0f32));
        let view_width = viewport.read().0;
        let pointer = use_pointer();
        let edge = crate::edge_scroll::use_edge_scroll(self.scroll);
        use_drag_ends_on_focus_lost(self.gesture.as_ref().map(Gesture::lines));
        let (rows, width) = {
            let files = self.files.read();
            let count = self.paths.read().len();
            let rows = count + files.added_rows(self.side_by_side);
            let width = widest(&files, self.side_by_side, view_width);
            (rows, width)
        };
        let data = StackedData {
            files: self.files.clone(),
            paths: self.paths.clone(),
            side_by_side: self.side_by_side,
            rows,
            view_width,
            scroll: self.scroll,
        };
        let length = crate::end_room::with_end_room(rows, DIFF_ROW_HEIGHT);
        let scroll = self.scroll;
        let host = self.gesture.clone().map(|gesture| Host {
            gesture,
            pointer,
            source: Source::Stacked(self.files.clone()),
            side_by_side: self.side_by_side,
            scroll,
            edge,
        });
        let list = VirtualScrollView::new_with_data_controlled(data, build_row, scroll)
            .length(length)
            .item_size(DIFF_ROW_HEIGHT)
            .scroll_with_arrows(false)
            .expanded();
        let (sizing, escaping) = (host.clone(), host.clone());
        let root = rect()
            .expanded()
            .background(GROUND)
            .a11y_id(focus_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::List)
            .on_press(move |_| focus_id.request_focus())
            .on_sized(move |e: Event<SizedEventData>| {
                viewport.set_if_modified((e.area.width(), e.area.height()));
                if let Some(host) = &sizing {
                    host.sized(e.area);
                }
            })
            .on_key_down(move |e: Event<KeyboardEventData>| {
                if e.key == Key::Named(NamedKey::Escape)
                    && escaping.as_ref().is_some_and(Host::escaped)
                {
                    e.stop_propagation();
                    return;
                }
                let (shown_width, shown_height) = *viewport.peek();
                let limits = Limits {
                    rows: length,
                    width,
                    shown_width,
                    shown_height,
                };
                if scroll_by_key(&e, scroll, limits) {
                    e.stop_propagation();
                }
            });
        match host {
            Some(host) => host
                .on_root(root)
                .child(edge.on(host.on_rows(rect().expanded())).child(list))
                .child(GestureLayer { host }),
            None => root.child(list),
        }
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

fn build_row(item: VirtualItem, data: &StackedData) -> Element {
    let plain = |child: Label| -> Element {
        rect()
            .key(item.index)
            .width(Size::fill())
            .height(Size::px(item.size))
            .main_align(Alignment::Center)
            .padding(Gaps::new(0., 8., 0., 8.))
            .child(child)
            .into()
    };
    if item.index >= data.rows {
        // Room for the horizontal scrollbar after the last row (`end_room`).
        return rect().key(item.index).height(Size::px(item.size)).into();
    }
    // Read, not peeked: the list redraws when a file's answer arrives.
    let files = data.files.read();
    match files.item(item.index, data.side_by_side) {
        Item::File(file) => {
            let path = data
                .paths
                .read()
                .get(file)
                .map(|path| path.display().into_owned())
                .unwrap_or_default();
            rect()
                .key(item.index)
                .width(Size::fill())
                .height(Size::px(item.size))
                .main_align(Alignment::Center)
                .padding(Gaps::new(0., 8., 0., 8.))
                .background(HEADER_BAR)
                .child(
                    label()
                        .text(path)
                        .max_lines(1)
                        .font_family(DIFF_FONT_FAMILY)
                        .font_size(DIFF_FONT_SIZE)
                        .font_weight(FontWeight::BOLD)
                        .color(DIFF_TEXT),
                )
                .into()
        }
        Item::Under { file, row } => match files.get(file) {
            None => plain(label()),
            Some(Opened::Reading) => plain(muted(READING_FILE_DIFF.to_owned())),
            Some(Opened::Failed(message)) => plain(muted(message.clone())),
            Some(Opened::Shown(shown)) => shown_row(item, row, shown, data, &plain),
        },
    }
}

/// Row `row` of what `shown` draws under its file's row: a line of its notice, or of its diff.
fn shown_row(
    item: VirtualItem,
    row: usize,
    shown: &ShownDiff,
    data: &StackedData,
    plain: &dyn Fn(Label) -> Element,
) -> Element {
    if let Some(notice) = DiffNotice::of(shown) {
        return match notice_rows(&notice).get(row) {
            // Load Diff is the single file's view's: a file drawn together past the limits
            // says so, and is loaded by choosing it alone.
            Some(NoticeRow::LoadDiff) | None => plain(label()),
            Some(line) => plain(notice_line(line)),
        };
    }
    let hides = usize::from(shown.hides_changes());
    if hides == 1 && row == 0 {
        return plain(muted(crate::diff_header::HIDDEN_CHANGES_NOTICE.to_owned()));
    }
    let geometry = RowGeometry::of(shown, data.side_by_side, data.view_width, None, data.scroll);
    draw_row(item.index, row - hides, item.size, shown, &geometry)
}

fn muted(text: String) -> Label {
    label()
        .text(text)
        .max_lines(1)
        .font_size(12.)
        .color(DIFF_MUTED)
}
