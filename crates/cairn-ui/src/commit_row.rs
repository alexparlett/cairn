//! One commit's row in the history list.

use cairn_model::{CommitSummary, GraphRow};
use freya::prelude::*;

use crate::date_text;
use crate::graph_cell::graph_cell;
use crate::graph_geometry::ROW_HEIGHT;

pub const AUTHOR_WIDTH: f32 = 150.0;
pub const ID_WIDTH: f32 = 72.0;
/// Sized for `YYYY-MM-DD HH:MM`.
pub const DATE_WIDTH: f32 = 132.0;
pub const COLUMN_GAP: f32 = 10.0;
pub const ROW_PADDING: f32 = 10.0;

pub const ROW_FONT_SIZE: f32 = 13.0;

/// No handler, so equal content compares equal and Freya skips re-rendering.
#[derive(Debug, PartialEq, Clone)]
pub struct CommitRow {
    commit: CommitSummary,
    graph: GraphRow,
    lanes: usize,
    selected: bool,
    key: DiffKey,
}

impl CommitRow {
    /// `lanes` is the graph column's width for the whole list, not for this row.
    pub fn new(commit: CommitSummary, graph: GraphRow, lanes: usize) -> Self {
        Self {
            commit,
            graph,
            lanes,
            selected: false,
            key: DiffKey::None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl KeyExt for CommitRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl ComponentOwned for CommitRow {
    fn render(self) -> impl IntoElement {
        let colours = get_theme_or_default();
        let primary = colours.read().colors().text_primary;
        let secondary = colours.read().colors().text_secondary;
        let highlight = colours.read().colors().surface_secondary;

        rect()
            .horizontal()
            // `Size::flex` only shares out leftover space under `Content::Flex`.
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT))
            .cross_align(Alignment::center())
            .padding(Gaps::new(0., ROW_PADDING, 0., ROW_PADDING))
            .spacing(COLUMN_GAP)
            .maybe(self.selected, |el| el.background(highlight))
            .child(
                rect()
                    .horizontal()
                    .content(Content::Flex)
                    .width(Size::flex(1.))
                    .height(Size::px(ROW_HEIGHT))
                    .cross_align(Alignment::center())
                    .spacing(COLUMN_GAP)
                    .child(graph_cell(
                        &self.graph,
                        self.commit.parents.len(),
                        self.lanes,
                    ))
                    .child(
                        label()
                            .text(self.commit.summary)
                            .width(Size::flex(1.))
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .font_size(ROW_FONT_SIZE)
                            .color(primary),
                    ),
            )
            .child(
                label()
                    .text(self.commit.author_name)
                    .width(Size::px(AUTHOR_WIDTH))
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis)
                    .font_size(ROW_FONT_SIZE)
                    .color(secondary),
            )
            .child(
                label()
                    // `text` takes a `Cow<'static, str>`, so the abbreviation is copied.
                    .text(self.commit.id.short().as_str().to_string())
                    .width(Size::px(ID_WIDTH))
                    .max_lines(1)
                    .font_size(ROW_FONT_SIZE)
                    .color(secondary),
            )
            .child(
                label()
                    .text(date_text::utc_minutes(self.commit.author_time))
                    .width(Size::px(DATE_WIDTH))
                    .max_lines(1)
                    .font_size(ROW_FONT_SIZE)
                    .color(secondary),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct HistoryHeader {
    key: DiffKey,
}

impl HistoryHeader {
    pub fn new() -> Self {
        Self { key: DiffKey::None }
    }
}

impl Default for HistoryHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyExt for HistoryHeader {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl ComponentOwned for HistoryHeader {
    fn render(self) -> impl IntoElement {
        let colours = get_theme_or_default();
        let text = colours.read().colors().text_placeholder;
        let line = colours.read().colors().border;

        let heading = |caption: &'static str, width: Size| {
            label()
                .text(caption)
                .width(width)
                .max_lines(1)
                .font_size(ROW_FONT_SIZE)
                .color(text)
        };

        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT))
            .cross_align(Alignment::center())
            .padding(Gaps::new(0., ROW_PADDING, 0., ROW_PADDING))
            .spacing(COLUMN_GAP)
            .border(Border::new().fill(line).width(BorderWidth {
                top: 0.,
                right: 0.,
                bottom: 1.,
                left: 0.,
            }))
            .child(heading("Graph and subject", Size::flex(1.)))
            .child(heading("Author", Size::px(AUTHOR_WIDTH)))
            .child(heading("Commit", Size::px(ID_WIDTH)))
            .child(heading("Date (UTC)", Size::px(DATE_WIDTH)))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
