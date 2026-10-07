//! The refs on a history row, as Fork draws them (refs-and-status R5): one outlined chip per
//! ref between the graph and the subject, tinted with the row's lane colour — a tag indigo —
//! each kind told by its glyph and shape (R5.5): a local branch is a plain chip, the current
//! one with a check mark before its name; a remote-tracking ref and a tag carry their glyph in
//! a cap of their own at the chip's left; a stash's row carries a `stash@{n}` chip with a box.
//!
//! **Order.** The current branch first (git's `HEAD -> main` leads too), then the row's refs in
//! the snapshot's order — local branches, remote-tracking refs, tags, each by name — Fork's
//! local-before-remote; where Fork puts tags is not recorded (`fork-refs-and-status-ui.md`,
//! OPEN 3).
//!
//! **Compact labels** (R5.2, Fork's default): a local branch whose upstream is a
//! remote-tracking ref labelling the same row draws that upstream as the remote glyph in front
//! of its own chip, and the upstream draws no chip of its own; any other remote's ref at that
//! commit keeps its chip (Fork, Tracker #1365). Which ref is a branch's upstream is read from
//! the refs snapshot; that both label this row is read from the row.
//!
//! **Clipped, never counted** (R5.3): chips are laid out left to right and stop being BUILT
//! once the room the column has is spent — the chip that crosses its edge is drawn and cut
//! there, none after it is made — so a row labelled by thousands of refs costs what the
//! column shows. A chip's width is not known until the toolkit lays its text out, so the room
//! is spent by a lower bound of each chip's width ([`min_width`]): the column is always filled
//! to its edge, and at most a column's worth of lower bounds is built past it.

use cairn_model::{HeadState, Label, Lane, RefKind, RefsSnapshot, RowLabels, Upstream};
use freya::prelude::*;

use crate::lane_palette::lane_colour;
use crate::ref_glyphs::{GLYPH_SIZE, RefGlyph};

/// A chip's height, inside a row's.
pub const CHIP_HEIGHT: f32 = 18.0;
/// The space between two chips, and between the last and the subject.
pub const CHIP_GAP: f32 = 4.0;
/// The chip's name text size.
pub const CHIP_FONT_SIZE: f32 = 12.0;
/// The space either side of a chip's name.
const NAME_PADDING: f32 = 5.0;
/// A cap's width: its glyph and the space either side of it.
const CAP_WIDTH: f32 = GLYPH_SIZE + 6.0;
/// The space between the check mark and the current branch's name.
const CHECK_GAP: f32 = 3.0;
/// The least any character of a ref's name is wide at [`CHIP_FONT_SIZE`]: under a sixth of
/// an em, narrower than any letter, digit or punctuation a ref name may hold in a text face
/// (`the_room_a_chip_is_counted_by_never_exceeds_what_it_is_drawn_at`).
const MIN_ADVANCE: f32 = 2.0;
const CORNER_RADIUS: f32 = 3.0;

/// Fork's tag colour: indigo, whatever the lane (`fork-refs-and-status-ui.md`, section 1).
pub const TAG_INDIGO: Color = Color::from_rgb(98, 92, 196);

/// What kind of chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChipKind {
    /// A local branch: `current` when `HEAD` is on it, `tracked` when its upstream labels the
    /// same row and is folded into this chip as the remote glyph (R5.2).
    Branch {
        current: bool,
        tracked: bool,
    },
    /// A remote-tracking ref no local branch on the row tracks.
    Remote,
    Tag,
    /// A stash's row: `stash@{n}`.
    Stash,
}

impl ChipKind {
    /// The glyph in the chip's cap, if it has one.
    pub fn cap(self) -> Option<RefGlyph> {
        match self {
            Self::Branch { tracked: true, .. } | Self::Remote => Some(RefGlyph::Remote),
            Self::Branch { tracked: false, .. } => None,
            Self::Tag => Some(RefGlyph::Tag),
            Self::Stash => Some(RefGlyph::Stash),
        }
    }

    fn is_current(self) -> bool {
        match self {
            Self::Branch { current, .. } => current,
            Self::Remote | Self::Tag | Self::Stash => false,
        }
    }
}

/// One chip: its kind, and the name it draws.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chip {
    pub kind: ChipKind,
    pub text: String,
}

impl Chip {
    /// A stash's chip, `stash@{index}`.
    pub fn stash(index: usize) -> Self {
        Self {
            kind: ChipKind::Stash,
            text: format!("stash@{{{index}}}"),
        }
    }
}

/// A lower bound of the width `chip` is drawn at: what it is drawn at with its name's every
/// character as narrow as [`MIN_ADVANCE`].
pub fn min_width(chip: &Chip) -> f32 {
    let cap = if chip.kind.cap().is_some() {
        CAP_WIDTH
    } else {
        0.
    };
    let check = if chip.kind.is_current() {
        GLYPH_SIZE + CHECK_GAP
    } else {
        0.
    };
    cap + check + 2. * NAME_PADDING + chip.text.chars().count() as f32 * MIN_ADVANCE
}

/// The chips a row with `labels` draws in `room` pixels, in order, compacted (R5.2) and cut
/// at the room's edge (R5.3). `refs` is the snapshot a branch's upstream is read from; with
/// none yet, no chip is compacted. Reads at most the labels it builds a chip for, the current
/// branch's and the upstreams it folds — each found by a search — however many the row has.
pub fn row_chips(labels: RowLabels<'_>, refs: Option<&RefsSnapshot>, room: f32) -> Vec<Chip> {
    #[cfg(test)]
    LAYOUTS.with(|layouts| layouts.set(layouts.get() + 1));
    let mut laid = Laid {
        chips: Vec::new(),
        used: 0.,
        room,
    };
    // Upstreams already folded into a branch's chip: at most one per chip built.
    let mut folded: Vec<&str> = Vec::new();
    let current = refs
        .and_then(|refs| match &refs.head {
            HeadState::Branch(name) => labels.find(name.as_str()),
            HeadState::Detached(_) | HeadState::Unborn(_) => None,
        })
        .filter(|label| label.current);
    if let Some(label) = current
        && !laid.full()
    {
        laid.push(branch_chip(label, labels, refs, &mut folded));
    }
    // Stops at the first label past the room: the rest are never read.
    for label in labels.iter() {
        if laid.full() {
            break;
        }
        if current.is_some_and(|current| current.name == label.name) {
            continue;
        }
        match label.kind {
            RefKind::LocalBranch => {
                let chip = branch_chip(label, labels, refs, &mut folded);
                laid.push(chip);
            }
            RefKind::RemoteTracking => {
                if folded.contains(&label.name) {
                    continue;
                }
                laid.push(Chip {
                    kind: ChipKind::Remote,
                    text: label.short_name().to_owned(),
                });
            }
            RefKind::Tag => laid.push(Chip {
                kind: ChipKind::Tag,
                text: label.short_name().to_owned(),
            }),
        }
    }
    laid.chips
}

#[cfg(test)]
thread_local! {
    /// How many rows' chips were laid out, on this thread: a test counts it across frames.
    pub(crate) static LAYOUTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Chips laid out so far, and the room they used.
struct Laid {
    chips: Vec<Chip>,
    used: f32,
    room: f32,
}

impl Laid {
    fn full(&self) -> bool {
        self.used >= self.room
    }

    fn push(&mut self, chip: Chip) {
        self.used += min_width(&chip) + CHIP_GAP;
        self.chips.push(chip);
    }
}

/// `label`'s chip, a local branch's: tracked when its upstream is a remote-tracking ref that
/// labels the same row, which is then recorded in `folded`.
fn branch_chip<'r>(
    label: Label<'_>,
    labels: RowLabels<'_>,
    refs: Option<&'r RefsSnapshot>,
    folded: &mut Vec<&'r str>,
) -> Chip {
    let upstream = refs
        .and_then(|refs| refs.find(&cairn_model::RefName::new(label.name)))
        .and_then(|branch| branch.upstream.as_ref())
        .and_then(|upstream| match upstream {
            Upstream::Exists { name, .. } => Some(name.as_str()),
            Upstream::Gone { .. } => None,
        })
        .filter(|name| {
            labels
                .find(name)
                .is_some_and(|found| found.kind == RefKind::RemoteTracking)
        });
    if let Some(name) = upstream {
        folded.push(name);
    }
    Chip {
        kind: ChipKind::Branch {
            current: label.current,
            tracked: upstream.is_some(),
        },
        text: label.short_name().to_owned(),
    }
}

/// The colour a chip of `kind` is tinted with on a row in `lane`.
pub fn tint(kind: ChipKind, lane: Lane) -> Color {
    match kind {
        ChipKind::Tag => TAG_INDIGO,
        ChipKind::Branch { .. } | ChipKind::Remote | ChipKind::Stash => lane_colour(lane),
    }
}

/// `chip` drawn: Fork's outlined, tinted chip, its glyph in a cap at its left, the current
/// branch's check mark before its name. Keyed by what it draws, so a glyph is never left
/// painted where another chip now stands (`ref_glyphs`).
pub fn chip_element(chip: &Chip, lane: Lane) -> Element {
    let colour = tint(chip.kind, lane);
    let text = get_theme_or_default().read().colors().text_primary;
    rect()
        .key(chip)
        .horizontal()
        .height(Size::px(CHIP_HEIGHT))
        .cross_align(Alignment::center())
        .corner_radius(CORNER_RADIUS)
        .background(colour.with_a(56))
        .border(Border::new().fill(colour).width(1.))
        .maybe_child(chip.kind.cap().map(|glyph| {
            rect()
                .width(Size::px(CAP_WIDTH))
                .height(Size::fill())
                .center()
                .corner_radius(CORNER_RADIUS)
                .background(colour.with_a(140))
                .child(glyph.draw(text))
        }))
        .child(
            rect()
                .horizontal()
                .height(Size::fill())
                .cross_align(Alignment::center())
                .padding(Gaps::new(0., NAME_PADDING, 0., NAME_PADDING))
                .spacing(CHECK_GAP)
                .maybe_child(chip.kind.is_current().then(|| RefGlyph::Current.draw(text)))
                .child(
                    label()
                        .text(chip.text.clone())
                        .max_lines(1)
                        .font_size(CHIP_FONT_SIZE)
                        .color(text),
                ),
        )
        .into()
}

/// A row of chips, unclipped: its container clips it.
pub fn chips_row(chips: &[Chip], lane: Lane) -> Rect {
    chips.iter().fold(
        rect()
            .horizontal()
            .height(Size::fill())
            .cross_align(Alignment::center())
            .spacing(CHIP_GAP),
        |row, chip| row.child(chip_element(chip, lane)),
    )
}
