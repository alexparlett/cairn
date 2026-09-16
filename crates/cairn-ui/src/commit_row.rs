//! One commit's line in the history list, in four columns.
//!
//! The layout is Fork's and Sourcetree's: the graph and the subject share the
//! first column — the graph is where the subject starts, so indenting the text
//! by the lane is what makes a branch legible as a branch — then author,
//! abbreviated commit id, and date. A flat list with a date on every row, not
//! date groups: both clients ship the column and neither ships the grouping,
//! and a group header is a variable-height row, which L7 rules out.

use cairn_model::{CommitSummary, GraphRow};
use freya::prelude::*;

use crate::date_text;
use crate::graph_cell::graph_cell;
use crate::graph_geometry::ROW_HEIGHT;

/// Width of the author column.
pub const AUTHOR_WIDTH: f32 = 150.0;
/// Width of the abbreviated commit id column.
pub const ID_WIDTH: f32 = 72.0;
/// Width of the date column. Sized for `YYYY-MM-DD HH:MM`.
pub const DATE_WIDTH: f32 = 132.0;
/// Gap between the columns.
pub const COLUMN_GAP: f32 = 10.0;
/// Space either side of the row's contents.
pub const ROW_PADDING: f32 = 10.0;

/// Font size of everything in a row. One size, because a row is one line and a
/// second size in it would only be decoration.
const ROW_FONT_SIZE: f32 = 13.0;

/// One row of the history list.
///
/// Presentation only: it reports nothing and holds no handler, so two rows with
/// the same content compare equal and Freya can skip re-rendering an unchanged
/// one. Selecting is the list's business ([`crate::HistoryList`]), because the
/// list is what owns the keyboard and the focus.
#[derive(Debug, PartialEq, Clone)]
pub struct CommitRow {
    commit: CommitSummary,
    graph: GraphRow,
    lanes: usize,
    selected: bool,
    key: DiffKey,
}

impl CommitRow {
    /// A row for `commit`, drawing `graph` in a column `lanes` lanes wide.
    ///
    /// `lanes` is the width of the graph column for the WHOLE list, not for
    /// this row: every row reserves the same width, or the subjects do not line
    /// up into a column.
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
            // `Size::flex` only shares out the leftover space under
            // `Content::Flex`; without it the first column takes everything and
            // the three fixed columns are pushed off the window.
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT))
            .cross_align(Alignment::center())
            .padding(Gaps::new(0., ROW_PADDING, 0., ROW_PADDING))
            .spacing(COLUMN_GAP)
            .maybe(self.selected, |el| el.background(highlight))
            .child(
                // The graph and the subject share one column, so the subject
                // starts where the commit's own lane is and a branch is
                // readable as a branch.
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
                    // `short()` formats into a stack buffer; the copy out of it
                    // is Freya's price, not the id's — `text` takes a
                    // `Cow<'static, str>`, which no borrow can satisfy. Going
                    // through `as_str` keeps it to one 7-byte copy rather than
                    // a trip through a formatter.
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

/// The column headings above the list.
///
/// Its columns are the same constants the rows use, so the headings cannot
/// drift away from what they head.
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
            // Named for what it is. `CommitSummary` carries seconds since the
            // epoch and no offset, so this is UTC and not the reader's clock —
            // see `date_text`.
            .child(heading("Date (UTC)", Size::px(DATE_WIDTH)))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
