//! One commit's row in the history list, as Fork draws it (`docs/research/ui-review/
//! fork-window-survey-2026-10-16.md`, §4), with no heading over the list: its graph, the chips
//! of the refs pointing at it (`ref_chips`), its subject, its author after an avatar
//! (`avatar`), its short id and its local date — the whole row bold on `HEAD`'s, and its texts
//! grey on a row `HEAD` cannot reach.

use cairn_model::{CommitSummary, RowEdges};
use freya::prelude::*;

use crate::avatar::avatar;
use crate::history::date_text;
use crate::history::graph_cell::graph_cell;
use crate::history::graph_geometry::{ROW_HEIGHT, graph_width};
use crate::history::ref_chips::{CHIP_GAP, Chip, chips_row};
use crate::local_time::LocalZone;
use crate::style::{metrics, tones};

/// The author column: the avatar and the name after it.
pub const AUTHOR_WIDTH: f32 = 140.0;
/// The 7-character id.
pub const ID_WIDTH: f32 = 64.0;
/// Sized for Fork's `d Mon yyyy HH:mm` at its widest, `28 May 2026 23:59`.
pub const DATE_WIDTH: f32 = 116.0;
pub const COLUMN_GAP: f32 = 10.0;
pub const ROW_PADDING: f32 = 10.0;
/// The room between an avatar and the author's name.
const AVATAR_GAP: f32 = 5.0;

pub const ROW_FONT_SIZE: f32 = metrics::BODY;

/// The room a row's chips and subject share in a list `list_width` wide whose graph column
/// is `lanes` wide: what is left of the row past its padding, the graph and the fixed
/// columns. What [`crate::history::ref_chips::row_chips`] lays chips out in.
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
    focused: bool,
    chips: Vec<Chip>,
    head: bool,
    lost: bool,
    head_reaches: bool,
    zone: LocalZone,
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
            focused: false,
            chips: Vec::new(),
            head: false,
            lost: false,
            head_reaches: true,
            zone: LocalZone::default(),
            key: DiffKey::None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Whether the list the row is in has focus: a selected row is Fork's blue with white
    /// text while it has, and its grey when not (`tones::selected`).
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// The chips drawn between the graph and the subject, laid out already
    /// ([`crate::history::ref_chips::row_chips`]): the row only draws them.
    pub fn chips(mut self, chips: Vec<Chip>) -> Self {
        self.chips = chips;
        self
    }

    /// Whether this is `HEAD`'s commit, whose row is bold — subject, author, id and date — as
    /// Fork marks it.
    pub fn head(mut self, head: bool) -> Self {
        self.head = head;
        self
    }

    /// Whether `HEAD` reaches this commit, as the walk marked it: when it does not, the row's
    /// texts are drawn in Fork's grey for such a row, its graph and chips in their own colours.
    pub fn head_reaches(mut self, head_reaches: bool) -> Self {
        self.head_reaches = head_reaches;
        self
    }

    /// The zone the date is drawn in, handed down by the window.
    pub fn zone(mut self, zone: LocalZone) -> Self {
        self.zone = zone;
        self
    }

    /// Whether no ref reaches this commit — Show Lost Commits drew it from a reflog — so the
    /// row's texts — subject, author, short id and date — are drawn in Fork's grey for a row
    /// `HEAD` cannot reach, its graph and chips in their own colours.
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
        // A selected row's every text is in the selection's tone; any other is the body's, or
        // the unreachable grey for a commit `HEAD` cannot reach or a lost one. A selected row and
        // `HEAD`'s are bold throughout.
        let chosen = tones::selected(self.focused);
        let text = match (self.selected, self.lost || !self.head_reaches) {
            (true, _) => chosen.text,
            (false, true) => tones::UNREACHABLE_TEXT,
            (false, false) => tones::TEXT,
        };
        let emphasised = self.selected || self.head;
        let bold = |label: Label| label.maybe(emphasised, |l| l.font_weight(FontWeight::BOLD));

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
                        // subject out of it, as in Fork.
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
                            .child(bold(
                                label()
                                    .text(self.commit.summary)
                                    .width(Size::flex(1.))
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .font_size(ROW_FONT_SIZE)
                                    .color(text),
                            )),
                    ),
            )
            .child(
                rect()
                    .horizontal()
                    .content(Content::Flex)
                    .width(Size::px(AUTHOR_WIDTH))
                    .height(Size::px(ROW_HEIGHT))
                    .cross_align(Alignment::center())
                    .spacing(AVATAR_GAP)
                    .child(avatar(&self.commit.author_name))
                    .child(bold(
                        label()
                            .text(self.commit.author_name)
                            .width(Size::flex(1.))
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .font_size(ROW_FONT_SIZE)
                            .color(text),
                    )),
            )
            .child(bold(
                label()
                    .text(self.commit.id.short())
                    .width(Size::px(ID_WIDTH))
                    .max_lines(1)
                    .font_size(ROW_FONT_SIZE)
                    .color(text),
            ))
            .child(bold(
                label()
                    .text(date_text::history_date(self.commit.author_time, &self.zone))
                    .width(Size::px(DATE_WIDTH))
                    .max_lines(1)
                    .font_size(ROW_FONT_SIZE)
                    .color(text),
            ));
        // The selection's band behind the row.
        rect()
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT))
            .maybe(self.selected, |el| el.background(chosen.band))
            .child(row)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
