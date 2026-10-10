//! The diff's staging gesture (staging-and-commit R9, L5; Fork, Finding 23): in Local Changes'
//! diff only, a hovered chunk is outlined and floats its actions — `Stage` and `Discard
//! Changes…` over the unstaged diff, `Unstage` over the staged one — and a drag across lines
//! narrows them, and the stage and discard chords, to the changed lines it covers.
//!
//! **Outside the recycled rows.** The virtualising view builds and drops rows as they scroll;
//! nothing here lives in one. The outline, the selection's tint and the floating actions are
//! one layer laid over the list ([`GestureLayer`]), positioned from the rows' numbers and the
//! scroll — so the rows the Commit and Changes tabs draw are the same rows, and those tabs,
//! which hand their view no [`Gesture`], draw none of this (R9.5). The layer is built from the
//! hover and the selection alone: an outline, a tint and at most three actions, however long
//! the diff or the selection.
//!
//! **Tracked by the list, never a row.** The pointer is heard by the view's root through global
//! listeners, so a drag goes on across rows the virtual list unmounts as it scrolls, and the
//! list scrolls itself at its edges (`EdgeScroll`, R7.6). What a drag selects is the rows
//! between its ends, read from the layout ([`cairn_model::UnifiedLayout::selection_in`]) when it
//! ends — never from the rows built — so the selection is whole however far it reached. A
//! release the window never hears ends it at the next press, or when the window loses focus.
//!
//! **What an action takes.** A hovered chunk's actions take every changed line its outline
//! holds (`hunk_selection`: at any context, every exact change the drawn hunk groups, so the
//! chunk is what the person sees outlined); a selection's take its own lines, counted in the
//! caption (`Stage 2 Lines`). Side by side, a drag stays in the column it began in (R9.3).
//! Hovering is not selecting (Fork, Tracker #103): with nothing selected the chords act on the
//! whole file, and a press without a drag clears a selection.
//!
//! **After an action** the actions hide until the diff is drawn again or the pointer moves, so
//! a second press cannot act twice on rows the first already took (Fork, Tracker #480); then
//! they follow whatever is under the pointer, which the layer reads afresh from the new rows.
//!
//! **Which answer a selection belongs to.** A selection names rows, and the rows of another
//! answer are other lines: the view is handed a number for the answer it draws (`drawn`), each
//! selection carries the number it was made under, and one made under another number is
//! nothing.

use std::ops::Range;

use cairn_model::{FileMode, Selection, ShownDiff, SideColumn};
use freya::prelude::*;

use crate::diff_notice::{DiffNotice, header_lines};
use crate::diff_palette::{CURRENT_CHANGE, DIFF_FONT_FAMILY, DIFF_FONT_SIZE, DIFF_MUTED, GROUND};
use crate::diff_view::{DIFF_ROW_HEIGHT, number_column_width, text_width};
use crate::edge_scroll::EdgeScroll;
use crate::end_room::SCROLLBAR_THICKNESS;
use crate::expansion::{Expansion, Item, Opened};
use crate::local_changes_drag::DRAG_THRESHOLD;
use crate::side_by_side_rows::{Columns, MIDDLE_WIDTH};

/// A hovered chunk's action over the unstaged diff (Fork's).
pub const STAGE_CHUNK_CAPTION: &str = "Stage";
/// A hovered chunk's discard over the unstaged diff, which confirms (Fork's Mac words).
pub const DISCARD_CHUNK_CAPTION: &str = "Discard Changes…";
/// A hovered chunk's action over the staged diff (Fork's).
pub const UNSTAGE_CHUNK_CAPTION: &str = "Unstage";

/// How many layers the gesture's layer is lifted above the view's root: past the depth of
/// the rows the virtualising view builds (about a dozen), below `Layer::Overlay`.
const GESTURE_LAYER: i16 = 64;

/// What an action of the gesture does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GestureVerb {
    Stage,
    Unstage,
    Discard,
}

/// Which of Local Changes' diffs the gesture is over: what its actions are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GestureSide {
    /// A path's unstaged or untracked diff: stage, and discard (R9.1).
    Unstaged,
    /// A path's staged diff: unstage. Staged changes are never discarded (R3.6).
    Staged,
}

impl GestureSide {
    /// The actions floated over this side's diff, in the order drawn.
    pub fn verbs(self) -> &'static [GestureVerb] {
        match self {
            Self::Unstaged => &[GestureVerb::Stage, GestureVerb::Discard],
            Self::Staged => &[GestureVerb::Unstage],
        }
    }
}

/// A chunk's caption for `verb`.
pub fn chunk_caption(verb: GestureVerb) -> &'static str {
    match verb {
        GestureVerb::Stage => STAGE_CHUNK_CAPTION,
        GestureVerb::Unstage => UNSTAGE_CHUNK_CAPTION,
        GestureVerb::Discard => DISCARD_CHUNK_CAPTION,
    }
}

/// A selection's caption for `verb` over `count` lines (R9.2): `Stage 2 Lines`,
/// `Unstage 1 Line`, and `Discard 2 Lines…`, whose ellipsis says it confirms — the
/// confirmation's own button then reads `Discard 2 Lines` (the user's decision, 2026-10-09).
pub fn lines_caption(verb: GestureVerb, count: usize) -> String {
    let lines = if count == 1 { "Line" } else { "Lines" };
    match verb {
        GestureVerb::Stage => format!("Stage {count} {lines}"),
        GestureVerb::Unstage => format!("Unstage {count} {lines}"),
        GestureVerb::Discard => format!("Discard {count} {lines}…"),
    }
}

/// The mode row's caption for `verb` (R9.4).
pub fn mode_caption(verb: GestureVerb) -> &'static str {
    match verb {
        GestureVerb::Stage => "Stage Mode Change",
        GestureVerb::Unstage => "Unstage Mode Change",
        GestureVerb::Discard => "Discard Mode Change…",
    }
}

/// What a person asked of the gesture: `verb` on `selection` of the file at `file` — the one
/// file a single diff draws is file 0; the files drawn together are numbered in their order —
/// under the answer numbered `drawn`. The selection names lines of that answer alone: whoever
/// acts on it refuses it once another answer is drawn (phase 08 QA item 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GestureAct {
    pub file: usize,
    pub verb: GestureVerb,
    pub selection: Selection,
    pub drawn: u64,
    /// Whether the selection is the hovered chunk's own — its floating action pressed — rather
    /// than lines a drag selected or the mode row's mode: a discard's prompt names it as the
    /// chunk (staging-and-commit phase 15).
    pub chunk: bool,
}

/// Where a drag across a diff's lines stands (R9.2), kept by the window and handed to the view
/// with its [`Gesture`]: the window reads the lines selected for the chords, and the view
/// writes it as the pointer moves.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LineDrag {
    /// The answer drawn when it began; under another it is nothing.
    drawn: u64,
    phase: Phase,
}

#[derive(Debug, Clone, PartialEq, Default)]
enum Phase {
    #[default]
    Idle,
    /// Pressed on `anchor`, a row of `file`, not yet moved past the drag threshold.
    Armed {
        file: usize,
        column: Option<SideColumn>,
        anchor: usize,
        at: CursorPoint,
    },
    /// Dragging from `anchor`: the other end is the row under the pointer.
    Dragging {
        file: usize,
        column: Option<SideColumn>,
        anchor: usize,
    },
    /// Released: `rows` of `file` selected, and the changed lines they hold.
    Selected {
        file: usize,
        column: Option<SideColumn>,
        rows: Range<usize>,
        selection: Selection,
    },
}

impl LineDrag {
    /// The changed lines selected under the answer `drawn`, and the file they are of; `None`
    /// with nothing selected, or a selection made under another answer.
    pub fn selected(&self, drawn: u64) -> Option<(usize, &Selection)> {
        match self.phase_under(drawn) {
            Phase::Selected {
                file, selection, ..
            } => Some((*file, selection)),
            Phase::Idle | Phase::Armed { .. } | Phase::Dragging { .. } => None,
        }
    }

    /// Whether a drag is on under the answer `drawn`.
    pub fn is_dragging(&self, drawn: u64) -> bool {
        match self.phase_under(drawn) {
            Phase::Dragging { .. } => true,
            Phase::Idle | Phase::Armed { .. } | Phase::Selected { .. } => false,
        }
    }

    /// Whether a drag is on, under whichever answer it began.
    fn dragging_under_any(&self) -> bool {
        match self.phase {
            Phase::Dragging { .. } => true,
            Phase::Idle | Phase::Armed { .. } | Phase::Selected { .. } => false,
        }
    }

    fn phase_under(&self, drawn: u64) -> &Phase {
        static IDLE: Phase = Phase::Idle;
        if self.drawn == drawn {
            &self.phase
        } else {
            &IDLE
        }
    }
}

/// The gesture a diff view draws: the side it is over, the number of the answer it draws, the
/// drag the window keeps, and where its actions go. A view handed none draws no gesture.
#[derive(Clone)]
pub struct Gesture {
    side: GestureSide,
    drawn: u64,
    lines: State<LineDrag>,
    on_act: EventHandler<GestureAct>,
}

impl Gesture {
    pub fn new(
        side: GestureSide,
        drawn: u64,
        lines: State<LineDrag>,
        on_act: impl Into<EventHandler<GestureAct>>,
    ) -> Self {
        Self {
            side,
            drawn,
            lines,
            on_act: on_act.into(),
        }
    }

    pub fn side(&self) -> GestureSide {
        self.side
    }

    pub(crate) fn lines(&self) -> State<LineDrag> {
        self.lines
    }
}

// Hand-written: `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for Gesture {
    fn eq(&self, other: &Self) -> bool {
        self.side == other.side && self.drawn == other.drawn && self.lines == other.lines
    }
}

impl std::fmt::Debug for Gesture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gesture")
            .field("side", &self.side)
            .field("drawn", &self.drawn)
            .finish_non_exhaustive()
    }
}

/// The rows a view draws, as the gesture reads them: one file's diff, or several files drawn
/// together, each under its own row.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Places<'a> {
    One(&'a ShownDiff),
    Stacked(&'a Expansion),
}

/// The diff `file` draws as rows, if it draws rows: a file opened together whose answer is a
/// notice, or is not here yet, draws none the gesture acts on.
fn drawn_rows(expansion: &Expansion, file: usize) -> Option<&ShownDiff> {
    match expansion.get(file)? {
        Opened::Shown(shown) => DiffNotice::of(shown).is_none().then_some(&**shown),
        Opened::Reading | Opened::Failed(_) => None,
    }
}

impl<'a> Places<'a> {
    /// The file and its own row that list row `row` is, for a row of a diff's lines; `None`
    /// for a file's own row, a notice's, or past the end.
    pub(crate) fn place(self, row: usize, side_by_side: bool) -> Option<(usize, usize)> {
        match self {
            Self::One(shown) => (row < shown.rows(side_by_side)).then_some((0, row)),
            Self::Stacked(expansion) => match expansion.item(row, side_by_side) {
                Item::File(_) => None,
                Item::Under { file, row } => {
                    let shown = drawn_rows(expansion, file)?;
                    let local = row.checked_sub(usize::from(shown.hides_changes()))?;
                    (local < shown.rows(side_by_side)).then_some((file, local))
                }
            },
        }
    }

    /// The diff `file` draws as rows.
    pub(crate) fn shown(self, file: usize) -> Option<&'a ShownDiff> {
        match self {
            Self::One(shown) => (file == 0).then_some(shown),
            Self::Stacked(expansion) => drawn_rows(expansion, file),
        }
    }

    /// The list row of `file`'s first row of lines.
    pub(crate) fn start(self, file: usize, side_by_side: bool) -> Option<usize> {
        match self {
            Self::One(_) => (file == 0).then_some(0),
            Self::Stacked(expansion) => {
                let shown = drawn_rows(expansion, file)?;
                Some(
                    expansion.position(file, side_by_side) + 1 + usize::from(shown.hides_changes()),
                )
            }
        }
    }
}

/// A chunk under the pointer: its file, its hunk, and the list rows it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chunk {
    pub(crate) file: usize,
    pub(crate) hunk: usize,
    pub(crate) rows: Range<usize>,
}

/// The chunk list row `row` falls in.
pub(crate) fn chunk_at(places: Places<'_>, side_by_side: bool, row: usize) -> Option<Chunk> {
    let (file, local) = places.place(row, side_by_side)?;
    let shown = places.shown(file)?;
    let (hunk, rows) = if side_by_side {
        let layout = shown.side_by_side_layout()?;
        let hunk = layout.hunk_at(local)?;
        (hunk, layout.hunk_rows(hunk)?)
    } else {
        let layout = shown.layout()?;
        let hunk = layout.hunk_at(local)?;
        (hunk, layout.hunk_rows(hunk)?)
    };
    let start = places.start(file, side_by_side)?;
    Some(Chunk {
        file,
        hunk,
        rows: start + rows.start..start + rows.end,
    })
}

/// Every changed line a chunk draws: what its actions take.
pub(crate) fn chunk_selection(
    shown: &ShownDiff,
    side_by_side: bool,
    hunk: usize,
) -> Option<Selection> {
    let (text, _) = shown.text()?;
    if side_by_side {
        shown.side_by_side_layout()?.hunk_selection(text, hunk)
    } else {
        shown.layout()?.hunk_selection(text, hunk)
    }
}

/// The changed lines drawn in `rows` of `shown` — of `column` alone, side by side.
pub(crate) fn rows_selection(
    shown: &ShownDiff,
    column: Option<SideColumn>,
    rows: Range<usize>,
) -> Option<Selection> {
    let (text, _) = shown.text()?;
    match column {
        Some(column) => shown
            .side_by_side_layout()?
            .selection_in(text, rows, column),
        None => shown.layout()?.selection_in(text, rows),
    }
}

/// What the gesture reads its rows from: a handle, peeked in a handler and read in the layer.
#[derive(Clone, PartialEq)]
pub(crate) enum Source {
    One(Readable<ShownDiff>),
    Stacked(Readable<Expansion>),
}

impl Source {
    /// `with` given the rows as they are, without subscribing: for a handler.
    fn peeked<R>(&self, with: impl FnOnce(Places<'_>) -> R) -> R {
        match self {
            Self::One(shown) => with(Places::One(&shown.peek())),
            Self::Stacked(expansion) => with(Places::Stacked(&expansion.peek())),
        }
    }

    /// `with` given the rows as they are, subscribing the caller: for the layer.
    fn read<R>(&self, with: impl FnOnce(Places<'_>) -> R) -> R {
        match self {
            Self::One(shown) => with(Places::One(&shown.read())),
            Self::Stacked(expansion) => with(Places::Stacked(&expansion.read())),
        }
    }
}

/// Where the pointer is over the list, and where the list is: the view's own, written by its
/// listeners and read by its layer. Copy, as Freya's handles are.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Pointer {
    /// The pointer, relative to the list's top left: `None` outside it, unless a drag is on.
    at: State<Option<(f32, f32)>>,
    /// The list in window coordinates: left, top, width, height.
    frame: State<(f32, f32, f32, f32)>,
    /// The answer an action was last taken under, and where the pointer was: the actions hide
    /// until either moves.
    acted: State<Option<(u64, (f32, f32))>>,
}

pub(crate) fn use_pointer() -> Pointer {
    Pointer {
        at: use_state(|| None),
        frame: use_state(|| (0.0, 0.0, 0.0, 0.0)),
        acted: use_state(|| None),
    }
}

/// Everything the gesture's listeners and layer need of the view they are in.
#[derive(Clone, PartialEq)]
pub(crate) struct Host {
    pub(crate) gesture: Gesture,
    pub(crate) pointer: Pointer,
    pub(crate) source: Source,
    pub(crate) side_by_side: bool,
    pub(crate) scroll: ScrollController,
    pub(crate) edge: EdgeScroll,
}

/// The list row at `y` pixels below the list's top, scrolled as `scroll` is; `None` above the
/// first.
fn row_at(y: f32, scroll: ScrollController) -> Option<usize> {
    let (_, scrolled) = <(i32, i32)>::from(scroll);
    let content = f64::from(y) - f64::from(scrolled);
    (content >= 0.0).then(|| (content / f64::from(DIFF_ROW_HEIGHT)) as usize)
}

/// The column `x` pixels from the list's left falls in, side by side, for `shown`'s columns in
/// a list `width` wide.
fn column_at(x: f32, width: f32, shown: &ShownDiff, scroll: ScrollController) -> SideColumn {
    let (scrolled_x, _) = <(i32, i32)>::from(scroll);
    let columns = Columns::of(
        width,
        number_column_width(shown),
        text_width(shown),
        scrolled_x as f32,
    );
    if x < columns.column + MIDDLE_WIDTH / 2.0 {
        SideColumn::Old
    } else {
        SideColumn::New
    }
}

impl Host {
    fn relative(&self, at: CursorPoint) -> (f32, f32, bool) {
        let (left, top, width, height) = *self.pointer.frame.peek();
        let (x, y) = (at.x as f32 - left, at.y as f32 - top);
        (x, y, x >= 0.0 && y >= 0.0 && x < width && y < height)
    }

    /// A primary press on the list's rows, away from its scrollbars: a drag may begin on the
    /// changed line under it. Any selection goes.
    fn pressed(&self, at: CursorPoint) {
        let (x, y, inside) = self.relative(at);
        let (_, _, width, height) = *self.pointer.frame.peek();
        let mut lines = self.gesture.lines;
        if !inside || x >= width - SCROLLBAR_THICKNESS || y >= height - SCROLLBAR_THICKNESS {
            return;
        }
        let side_by_side = self.side_by_side;
        let scroll = self.scroll;
        let armed = row_at(y, scroll).and_then(|row| {
            self.source.peeked(|places| {
                let (file, anchor) = places.place(row, side_by_side)?;
                let column = if side_by_side {
                    Some(column_at(x, width, places.shown(file)?, scroll))
                } else {
                    None
                };
                Some(Phase::Armed {
                    file,
                    column,
                    anchor,
                    at,
                })
            })
        });
        lines.set(LineDrag {
            drawn: self.gesture.drawn,
            phase: armed.unwrap_or_default(),
        });
    }

    /// The pointer moved, anywhere: kept while over the list or dragging, and an armed press
    /// moved far enough begins a drag.
    fn moved(&self, at: CursorPoint) {
        let (x, y, inside) = self.relative(at);
        let drawn = self.gesture.drawn;
        let mut lines = self.gesture.lines;
        let began = match lines.peek().phase_under(drawn) {
            Phase::Armed {
                file,
                column,
                anchor,
                at: from,
            } if from.distance_to(at) > DRAG_THRESHOLD => Some(Phase::Dragging {
                file: *file,
                column: *column,
                anchor: *anchor,
            }),
            Phase::Idle | Phase::Armed { .. } | Phase::Dragging { .. } | Phase::Selected { .. } => {
                None
            }
        };
        if let Some(began) = began {
            lines.set(LineDrag {
                drawn,
                phase: began,
            });
            let mut edge = self.edge;
            edge.begin();
        }
        let dragging = lines.peek().is_dragging(drawn);
        let mut pointer = self.pointer.at;
        pointer.set_if_modified((inside || dragging).then_some((x, y)));
    }

    /// The button came up: an armed press without a drag selects nothing, and a drag selects
    /// the changed lines between its ends.
    fn released(&self) {
        let drawn = self.gesture.drawn;
        let mut lines = self.gesture.lines;
        // Read in place: a selection is as long as its lines, and this runs on every release.
        let dragging = match lines.peek().phase_under(drawn) {
            Phase::Armed { .. } => Some(false),
            Phase::Dragging { .. } => Some(true),
            Phase::Idle | Phase::Selected { .. } => None,
        };
        match dragging {
            Some(false) => lines.set(LineDrag::default()),
            Some(true) => self.finish(),
            None => {}
        }
    }

    /// A drag ends where the pointer last was: the rows between its ends, inside the file it
    /// began in, and the changed lines they hold — nothing selected when they hold none.
    pub(crate) fn finish(&self) {
        let drawn = self.gesture.drawn;
        let mut lines = self.gesture.lines;
        // Read in place, the three fields copied: this runs on every press.
        let (file, column, anchor) = match lines.peek().phase_under(drawn) {
            Phase::Dragging {
                file,
                column,
                anchor,
            } => (*file, *column, *anchor),
            Phase::Idle | Phase::Armed { .. } | Phase::Selected { .. } => return,
        };
        let pointer = *self.pointer.at.peek();
        let side_by_side = self.side_by_side;
        let scroll = self.scroll;
        let selected = self.source.peeked(|places| {
            let focus = dragged_to(places, side_by_side, file, pointer, scroll).unwrap_or(anchor);
            let rows = anchor.min(focus)..anchor.max(focus) + 1;
            let selection = rows_selection(places.shown(file)?, column, rows.clone())?;
            (!selection.is_empty()).then_some(Phase::Selected {
                file,
                column,
                rows,
                selection,
            })
        });
        lines.set(LineDrag {
            drawn,
            phase: selected.unwrap_or_default(),
        });
    }

    /// `verb` on what the actions are over: the selection, or the chunk under the pointer.
    fn act(&self, verb: GestureVerb, file: usize, selection: Selection, chunk: bool) {
        let mut lines = self.gesture.lines;
        lines.set(LineDrag::default());
        let mut acted = self.pointer.acted;
        let at = self.pointer.at.peek().unwrap_or((-1.0, -1.0));
        acted.set(Some((self.gesture.drawn, at)));
        self.gesture.on_act.call(GestureAct {
            file,
            verb,
            selection,
            drawn: self.gesture.drawn,
            chunk,
        });
    }

    /// The list was laid out at `area`, in window coordinates: what the pointer is read
    /// against. Called by the view's own size listener, which an element holds one of.
    pub(crate) fn sized(&self, area: Area) {
        let mut frame = self.pointer.frame;
        frame.set_if_modified((area.min_x(), area.min_y(), area.width(), area.height()));
    }

    /// The list's root, listening for the pointer: every move, the release, and a press while
    /// a drag is on — whose release the window never heard. Global listeners, so a drag is
    /// heard across rows unmounted under it.
    pub(crate) fn on_root(&self, root: Rect) -> Rect {
        let (moving, releasing, downing) = (self.clone(), self.clone(), self.clone());
        root.on_global_pointer_move(move |e: Event<PointerEventData>| {
            moving.moved(e.global_location())
        })
        .on_global_pointer_press(move |_: Event<PointerEventData>| releasing.released())
        .on_global_pointer_down(move |_: Event<PointerEventData>| {
            // The list's own press, heard first, armed afresh; a drag still on now lost its
            // release.
            downing.finish();
        })
    }

    /// The rows' container, where a drag begins: only a press on the rows, never on the
    /// actions laid over them.
    pub(crate) fn on_rows(&self, rows: Rect) -> Rect {
        let pressing = self.clone();
        rows.on_pointer_down(move |e: Event<PointerEventData>| {
            if e.is_primary() {
                pressing.pressed(e.global_location());
            }
        })
    }

    /// Escape lets a selection go; `true` when it did.
    pub(crate) fn escaped(&self) -> bool {
        let mut lines = self.gesture.lines;
        let drawn = self.gesture.drawn;
        if lines.peek().selected(drawn).is_some() {
            lines.set(LineDrag::default());
            return true;
        }
        false
    }
}

/// The row of `file` a drag has reached: the row under `pointer`, held inside `file`'s rows.
fn dragged_to(
    places: Places<'_>,
    side_by_side: bool,
    file: usize,
    pointer: Option<(f32, f32)>,
    scroll: ScrollController,
) -> Option<usize> {
    let (_, y) = pointer?;
    let rows = places.shown(file)?.rows(side_by_side);
    let last = rows.checked_sub(1)?;
    let start = places.start(file, side_by_side)?;
    let row = row_at(y, scroll).unwrap_or(0);
    Some(row.saturating_sub(start).min(last))
}

/// Lets a drag go when the window loses focus: its release may be heard by another window
/// (phase 06's QA item 16, as the lists' drag does). `lines` is the drag the view is handed
/// first; the window keeps one for the life of the view.
pub(crate) fn use_drag_ends_on_focus_lost(lines: Option<State<LineDrag>>) {
    use_side_effect(move || {
        let focused = *Platform::get().is_app_focused.read();
        if let Some(mut lines) = lines
            && !focused
            && lines.peek().dragging_under_any()
        {
            lines.set(LineDrag::default());
        }
    });
}

/// The layer laid over a diff's rows: the hovered chunk's outline, the selection's tint, and
/// the floating actions — at most one outline, one tint and three actions, whatever is drawn.
#[derive(Clone, PartialEq)]
pub(crate) struct GestureLayer {
    pub(crate) host: Host,
}

/// A visible stretch of list rows, in pixels from the list's top: clipped to the list.
fn span(rows: &Range<usize>, scrolled: i32, height: f32) -> Option<(f32, f32)> {
    let top = rows.start as f64 * f64::from(DIFF_ROW_HEIGHT) + f64::from(scrolled);
    let bottom = rows.end as f64 * f64::from(DIFF_ROW_HEIGHT) + f64::from(scrolled);
    let (top, bottom) = (top.max(0.0) as f32, (bottom as f32).min(height));
    (bottom > top).then_some((top, bottom))
}

/// What the layer draws, worked out from the rows and the pointer.
#[derive(Debug, Clone, PartialEq)]
struct Drawn {
    outline: Option<(f32, f32)>,
    /// The tint's top and bottom, and its left and width.
    tint: Option<(f32, f32, f32, f32)>,
    actions: Option<Actions>,
}

/// What the floating actions take when one is pressed — read then, never per frame.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Takes {
    /// The selection, of `count` lines.
    Selection { count: usize },
    /// Every changed line of the chunk `hunk`.
    Chunk { hunk: usize },
}

/// Where the floating actions stand, the file they act on and what they take.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Actions {
    top: f32,
    file: usize,
    takes: Takes,
}

impl Component for GestureLayer {
    fn render(&self) -> impl IntoElement {
        let host = &self.host;
        let drawn_as = host.gesture.drawn;
        let (_, _, width, height) = *host.pointer.frame.read();
        let (scrolled_x, scrolled) = <(i32, i32)>::from(host.scroll);
        let pointer = *host.pointer.at.read();
        // Read, never copied: a selection is as long as the lines it holds.
        let lines = host.gesture.lines.read();
        let acted = *host.pointer.acted.read();
        let hidden = acted.is_some_and(|(under, at)| under == drawn_as && Some(at) == pointer);
        let side_by_side = host.side_by_side;
        let scroll = host.scroll;

        let drawn = host.source.read(|places| {
            let column_span = |file: usize, column: Option<SideColumn>| -> (f32, f32) {
                match (column, places.shown(file)) {
                    (Some(column), Some(shown)) => {
                        let columns = Columns::of(
                            width,
                            number_column_width(shown),
                            text_width(shown),
                            scrolled_x as f32,
                        );
                        match column {
                            SideColumn::Old => (0.0, columns.column),
                            SideColumn::New => (columns.column + MIDDLE_WIDTH, columns.column),
                        }
                    }
                    (None, _) | (_, None) => (0.0, width),
                }
            };
            let global = |file: usize, rows: &Range<usize>| -> Option<Range<usize>> {
                let start = places.start(file, side_by_side)?;
                Some(start + rows.start..start + rows.end)
            };
            match lines.phase_under(drawn_as) {
                Phase::Dragging {
                    file,
                    column,
                    anchor,
                } => {
                    let focus =
                        dragged_to(places, side_by_side, *file, pointer, scroll).unwrap_or(*anchor);
                    let rows = (*anchor).min(focus)..(*anchor).max(focus) + 1;
                    let tint = global(*file, &rows)
                        .and_then(|rows| span(&rows, scrolled, height))
                        .map(|(top, bottom)| {
                            let (left, wide) = column_span(*file, *column);
                            (top, bottom, left, wide)
                        });
                    Drawn {
                        outline: None,
                        tint,
                        actions: None,
                    }
                }
                Phase::Selected {
                    file,
                    column,
                    rows,
                    selection,
                } => {
                    let visible =
                        global(*file, rows).and_then(|rows| span(&rows, scrolled, height));
                    let tint = visible.map(|(top, bottom)| {
                        let (left, wide) = column_span(*file, *column);
                        (top, bottom, left, wide)
                    });
                    let actions = visible.filter(|_| !hidden).map(|(top, _)| Actions {
                        top,
                        file: *file,
                        takes: Takes::Selection {
                            count: selection.len(),
                        },
                    });
                    Drawn {
                        outline: None,
                        tint,
                        actions,
                    }
                }
                Phase::Idle | Phase::Armed { .. } => {
                    let chunk = pointer
                        .filter(|_| !hidden)
                        .and_then(|(_, y)| row_at(y, scroll))
                        .and_then(|row| chunk_at(places, side_by_side, row));
                    let visible = chunk
                        .as_ref()
                        .and_then(|chunk| span(&chunk.rows, scrolled, height));
                    Drawn {
                        outline: visible,
                        tint: None,
                        actions: chunk.zip(visible).map(|(chunk, (top, _))| Actions {
                            top,
                            file: chunk.file,
                            takes: Takes::Chunk { hunk: chunk.hunk },
                        }),
                    }
                }
            }
        });

        let accent = CURRENT_CHANGE;
        let mut layer = rect()
            .position(Position::new_absolute().top(0.).left(0.))
            .width(Size::px(width))
            .height(Size::px(height))
            // A node's layer is its parent's plus one, so the rows, built deep inside the
            // virtualising view, stand layers above the view's root: the layer is lifted past
            // them, to be drawn over them and pressed before them, and stays far below the
            // window's overlays — a menu, the confirmation.
            .layer(Layer::Relative(GESTURE_LAYER));
        if let Some((top, bottom, left, wide)) = drawn.tint {
            layer = layer.child(
                rect()
                    .interactive(Interactive::No)
                    .position(Position::new_absolute().top(top).left(left))
                    .width(Size::px(wide))
                    .height(Size::px(bottom - top))
                    .background(accent.with_a(56)),
            );
        }
        if let Some((top, bottom)) = drawn.outline {
            layer = layer.child(
                rect()
                    .interactive(Interactive::No)
                    .position(Position::new_absolute().top(top).left(0.))
                    .width(Size::px((width - SCROLLBAR_THICKNESS).max(0.)))
                    .height(Size::px(bottom - top))
                    .background(accent.with_a(14))
                    .border(Border::new().fill(accent).width(1.)),
            );
        }
        if let Some(Actions { top, file, takes }) = drawn.actions {
            let mut actions = rect().horizontal().spacing(4.).position(
                Position::new_absolute()
                    .top(top + 2.)
                    .right(SCROLLBAR_THICKNESS + 6.),
            );
            for verb in host.gesture.side.verbs() {
                let caption = match takes {
                    Takes::Selection { count } => lines_caption(*verb, count),
                    Takes::Chunk { .. } => chunk_caption(*verb).to_owned(),
                };
                let (acting, verb) = (host.clone(), *verb);
                actions = actions.child(floating_action(caption, move || {
                    // What is taken is read as the action is pressed: a copy of the selection,
                    // or the chunk's lines, once.
                    let taken = match takes {
                        Takes::Selection { .. } => acting
                            .gesture
                            .lines
                            .peek()
                            .selected(acting.gesture.drawn)
                            .map(|(_, selection)| selection.clone()),
                        Takes::Chunk { hunk } => acting.source.peeked(|places| {
                            chunk_selection(places.shown(file)?, acting.side_by_side, hunk)
                        }),
                    };
                    if let Some(taken) = taken {
                        let chunk = match takes {
                            Takes::Selection { .. } => false,
                            Takes::Chunk { .. } => true,
                        };
                        acting.act(verb, file, taken, chunk);
                    }
                }));
            }
            layer = layer.child(actions);
        }
        layer
    }
}

/// One floating action: a small filled button that takes no focus — the diff keeps it, and
/// its chords with it — and whose press never begins a drag.
fn floating_action(caption: String, pressed: impl Fn() + 'static) -> Rect {
    let colours = get_theme_or_default().read().colors().clone();
    rect()
        .padding(Gaps::new(2., 8., 2., 8.))
        .corner_radius(4.)
        .background(CURRENT_CHANGE)
        .a11y_role(AccessibilityRole::Button)
        .on_pointer_down(|e: Event<PointerEventData>| e.stop_propagation())
        .on_press(move |_: Event<PressEventData>| pressed())
        .child(
            label()
                .text(caption)
                .font_size(12.)
                .max_lines(1)
                .color(colours.text_inverse),
        )
}

/// A file's mode change as a row of its own over its diff (R9.4): git's `old mode` and `new
/// mode`, and, while hovered, its own actions — the mode change alone, selected through
/// [`Selection::select_mode`], never a line.
#[derive(Clone)]
pub struct ModeRow {
    old: FileMode,
    new: FileMode,
    side: GestureSide,
    drawn: u64,
    on_act: EventHandler<GestureAct>,
}

impl ModeRow {
    /// The mode row of `file`'s change — in the answer numbered `drawn` — when its mode changed
    /// and the gesture is over it.
    pub fn of(
        file: &cairn_model::ChangedFile,
        side: GestureSide,
        drawn: u64,
        on_act: impl Into<EventHandler<GestureAct>>,
    ) -> Option<Self> {
        let (Some(old), Some(new)) = (file.old_mode, file.new_mode) else {
            return None;
        };
        file.mode_changed().then(|| Self {
            old,
            new,
            side,
            drawn,
            on_act: on_act.into(),
        })
    }
}

impl std::fmt::Debug for ModeRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModeRow")
            .field("old", &self.old)
            .field("new", &self.new)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ModeRow {
    fn eq(&self, other: &Self) -> bool {
        self.old == other.old
            && self.new == other.new
            && self.side == other.side
            && self.drawn == other.drawn
    }
}

impl Component for ModeRow {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        // git's own two lines, as a notice prints them.
        let file = cairn_model::ChangedFile {
            status: cairn_model::ChangeStatus::Modified,
            old_path: cairn_model::RepoPath::from(""),
            new_path: cairn_model::RepoPath::from(""),
            old_mode: Some(self.old),
            new_mode: Some(self.new),
            old_id: None,
            new_id: None,
        };
        let words = header_lines(&file).join("  ");
        let (side, drawn) = (self.side, self.drawn);
        let on_act = self.on_act.clone();
        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(DIFF_ROW_HEIGHT + 4.))
            .cross_align(Alignment::Center)
            .padding(Gaps::new(0., SCROLLBAR_THICKNESS + 6., 0., 8.))
            .background(GROUND)
            .maybe(*hovered.read(), |el| {
                el.border(Border::new().fill(CURRENT_CHANGE).width(1.))
            })
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .child(
                rect().width(Size::flex(1.)).child(
                    label()
                        .text(words)
                        .max_lines(1)
                        .font_family(DIFF_FONT_FAMILY)
                        .font_size(DIFF_FONT_SIZE)
                        .color(DIFF_MUTED),
                ),
            )
            .maybe_child(hovered.read().then(|| {
                let mut actions = rect().horizontal().spacing(4.);
                for verb in side.verbs() {
                    let (on_act, verb) = (on_act.clone(), *verb);
                    actions =
                        actions.child(floating_action(mode_caption(verb).to_owned(), move || {
                            let mut selection = Selection::empty();
                            selection.select_mode();
                            on_act.call(GestureAct {
                                file: 0,
                                verb,
                                selection,
                                drawn,
                                chunk: false,
                            });
                        }));
                }
                actions
            }))
    }
}

#[cfg(test)]
mod tests {
    use cairn_model::{
        ChangeStatus, ChangedFile, ChangedRange, Context, DiffContent, DisplayOverlay, FileDiff,
        LineSpan, RepoPath, TextDiff, split_lines,
    };

    use super::*;

    fn shown(context: u32) -> ShownDiff {
        let old: Vec<u8> = (0..40)
            .flat_map(|n| format!("l{n}\n").into_bytes())
            .collect();
        let new: Vec<u8> = (0..40)
            .flat_map(|n| match n {
                10 | 15 | 30 => format!("L{n}\n").into_bytes(),
                _ => format!("l{n}\n").into_bytes(),
            })
            .collect();
        let at = |n: u32| ChangedRange::new(LineSpan::at(n, 1), LineSpan::at(n, 1));
        ShownDiff::new(
            FileDiff {
                file: ChangedFile {
                    status: ChangeStatus::Modified,
                    old_path: RepoPath::from("f"),
                    new_path: RepoPath::from("f"),
                    old_mode: None,
                    new_mode: None,
                    old_id: None,
                    new_id: None,
                },
                content: DiffContent::Text {
                    text: TextDiff::new(
                        split_lines(&old),
                        split_lines(&new),
                        vec![at(10), at(15), at(30)],
                    ),
                    overlay: DisplayOverlay::none(),
                },
            },
            Context::lines(context),
        )
    }

    /// The chunk under a row is the drawn hunk it falls in, and its actions take every change
    /// it groups: at context three the edits at lines 11 and 16 are one chunk, at context one
    /// two. Caught by: a chunk read as the change under the row.
    #[test]
    fn a_rows_chunk_is_the_hunk_drawn_around_it() {
        let wide = shown(3);
        let places = Places::One(&wide);
        let first = chunk_at(places, false, 0).expect("a chunk at the first row");
        assert_eq!(first.hunk, 0);
        let selection = chunk_selection(&wide, false, first.hunk).expect("exact");
        assert_eq!(selection.len(), 4, "both edits, both sides");
        assert_eq!(
            chunk_at(places, false, first.rows.end).map(|c| c.hunk),
            Some(1)
        );
        assert_eq!(chunk_at(places, false, wide.rows(false)), None);

        let narrow = shown(1);
        let places = Places::One(&narrow);
        let first = chunk_at(places, false, 0).expect("a chunk");
        assert_eq!(
            chunk_selection(&narrow, false, first.hunk).map(|s| s.len()),
            Some(2)
        );
        let side = chunk_at(places, true, 0).expect("side by side");
        assert_eq!(
            chunk_selection(&narrow, true, side.hunk).map(|s| s.len()),
            Some(2)
        );
    }

    /// A selection is nothing under another answer, and is the lines under its own. Caught
    /// by: a selection kept across answers (lines of another diff acted on).
    #[test]
    fn a_selection_belongs_to_the_answer_it_was_made_under() {
        let mut selection = Selection::empty();
        selection.select_added(cairn_model::LineNumber::from_index(3));
        let drag = LineDrag {
            drawn: 7,
            phase: Phase::Selected {
                file: 0,
                column: None,
                rows: 2..4,
                selection: selection.clone(),
            },
        };
        assert_eq!(drag.selected(7), Some((0, &selection)));
        assert_eq!(drag.selected(8), None);
        assert!(!drag.is_dragging(7));
    }

    /// The captions: Fork's for a chunk, a count of lines for a selection (R9.2), the mode's
    /// own (R9.4), and each side's actions (R9.1). Caught by: a discard offered over the
    /// staged diff.
    #[test]
    fn each_side_offers_its_own_actions_in_its_own_words() {
        assert_eq!(
            GestureSide::Unstaged.verbs(),
            &[GestureVerb::Stage, GestureVerb::Discard]
        );
        assert_eq!(GestureSide::Staged.verbs(), &[GestureVerb::Unstage]);
        assert_eq!(lines_caption(GestureVerb::Stage, 2), "Stage 2 Lines");
        assert_eq!(lines_caption(GestureVerb::Unstage, 1), "Unstage 1 Line");
        assert_eq!(lines_caption(GestureVerb::Discard, 2), "Discard 2 Lines…");
        assert_eq!(lines_caption(GestureVerb::Discard, 1), "Discard 1 Line…");
        assert_eq!(chunk_caption(GestureVerb::Discard), "Discard Changes…");
        assert_eq!(mode_caption(GestureVerb::Stage), "Stage Mode Change");
    }

    /// A visible stretch is clipped to the list. Caught by: an outline drawn above the list's
    /// top, over the bar.
    #[test]
    fn a_span_is_clipped_to_the_list() {
        let row = f64::from(DIFF_ROW_HEIGHT) as f32;
        assert_eq!(span(&(0..2), 0, 100.0), Some((0.0, 2.0 * row)));
        assert_eq!(span(&(0..2), -(row as i32), 100.0), Some((0.0, row)));
        assert_eq!(span(&(0..2), -2 * row as i32, 100.0), None);
        assert_eq!(span(&(0..100), 0, 50.0), Some((0.0, 50.0)));
    }
}
