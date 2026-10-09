//! One commit's row in the history list: its graph, the chips of the refs pointing at it
//! (`ref_chips`), its subject — bold on `HEAD`'s row, as Fork marks it — its author, short id
//! and date.

use cairn_model::{CommitSummary, RowEdges};
use freya::prelude::*;

use crate::date_text;
use crate::graph_cell::graph_cell;
use crate::graph_geometry::{ROW_HEIGHT, graph_width};
use crate::ref_chips::{CHIP_GAP, Chip, chips_row};

pub const AUTHOR_WIDTH: f32 = 150.0;
pub const ID_WIDTH: f32 = 72.0;
/// Sized for `YYYY-MM-DD HH:MM`.
pub const DATE_WIDTH: f32 = 132.0;
pub const COLUMN_GAP: f32 = 10.0;
pub const ROW_PADDING: f32 = 10.0;

pub const ROW_FONT_SIZE: f32 = 13.0;

/// The room a row's chips and subject share in a list `list_width` wide whose graph column
/// is `lanes` wide: what is left of the row past its padding, the graph and the fixed
/// columns. What [`crate::ref_chips::row_chips`] lays chips out in.
pub fn label_room(list_width: f32, lanes: usize) -> f32 {
    list_width
        - 2. * ROW_PADDING
        - (AUTHOR_WIDTH + ID_WIDTH + DATE_WIDTH)
        - 3. * COLUMN_GAP
        - graph_width(lanes)
        - COLUMN_GAP
}

/// No handler, so equal content compares equal and Freya skips re-rendering.
#[derive(Debug, PartialEq, Clone)]
pub struct CommitRow {
    commit: CommitSummary,
    graph: RowEdges,
    lanes: usize,
    selected: bool,
    chips: Vec<Chip>,
    head: bool,
    lost: bool,
    key: DiffKey,
}

impl CommitRow {
    /// `graph` is the row's lane and every line crossing it, derived for the row drawn
    /// ([`cairn_model::row_edges`]); `lanes` is the graph column's width for the whole list,
    /// not for this row.
    pub fn new(commit: CommitSummary, graph: RowEdges, lanes: usize) -> Self {
        Self {
            commit,
            graph,
            lanes,
            selected: false,
            chips: Vec::new(),
            head: false,
            lost: false,
            key: DiffKey::None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// The chips drawn between the graph and the subject, laid out already
    /// ([`crate::ref_chips::row_chips`]): the row only draws them.
    pub fn chips(mut self, chips: Vec<Chip>) -> Self {
        self.chips = chips;
        self
    }

    /// Whether this is `HEAD`'s commit, whose subject is bold (R5.1).
    pub fn head(mut self, head: bool) -> Self {
        self.head = head;
        self
    }

    /// Whether no ref reaches this commit — Show Lost Commits drew it from a reflog — so the
    /// row's texts — subject, author, short id and date — are drawn in the placeholder grey, as
    /// Fork draws a lost commit, its graph and chips in their own colours (staging-and-commit
    /// R11.1; the user's decision, 2026-10-09).
    pub fn lost(mut self, lost: bool) -> Self {
        self.lost = lost;
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
        let dimmed = colours.read().colors().text_placeholder;
        let (primary, secondary) = if self.lost {
            (dimmed, dimmed)
        } else {
            (
                colours.read().colors().text_primary,
                colours.read().colors().text_secondary,
            )
        };
        let highlight = colours.read().colors().surface_secondary;

        let row = rect()
            .horizontal()
            // `Size::flex` only shares out leftover space under `Content::Flex`.
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT))
            .cross_align(Alignment::center())
            .padding(Gaps::new(0., ROW_PADDING, 0., ROW_PADDING))
            .spacing(COLUMN_GAP)
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
                        self.commit.parent_count,
                        self.lanes,
                    ))
                    .child(
                        // Chips then subject, cut at the column's edge: many chips push the
                        // subject out of it, as in Fork (R5.3).
                        rect()
                            .horizontal()
                            .content(Content::Flex)
                            .width(Size::flex(1.))
                            .height(Size::px(ROW_HEIGHT))
                            .cross_align(Alignment::center())
                            .spacing(CHIP_GAP)
                            .overflow(Overflow::Clip)
                            .maybe_child(
                                (!self.chips.is_empty())
                                    .then(|| chips_row(&self.chips, self.graph.lane)),
                            )
                            .child(
                                label()
                                    .text(self.commit.summary)
                                    .width(Size::flex(1.))
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .font_size(ROW_FONT_SIZE)
                                    .maybe(self.head, |label| label.font_weight(FontWeight::BOLD))
                                    .color(primary),
                            ),
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
                    .text(self.commit.id.short())
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
            );
        // The selection's background behind the row.
        rect()
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT))
            .maybe(self.selected, |el| el.background(highlight))
            .child(row)
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
