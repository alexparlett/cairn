//! The Commit tab, as Fork lays it out (`docs/research/ui-review/fork-window-survey-2026-10-16.md`
//! §5, `fork-committer-2026-10-20.md`): the AUTHOR block — the author's 42 px avatar beside the
//! name, the address and the date with its seconds and offset, in the zone the window is handed
//! — and, beside it on the same rows, the COMMITTER block only where the committer differs from
//! the author by name, address or instant ([`committer_differs`]); REFS, SHA and PARENTS; a rule,
//! the whole message, a rule; then Expand All and the changed files, a rename or a copy showing
//! both names.
//!
//! The whole tab is ONE virtualised list of equal rows — header, message lines, then one row
//! per changed file — because a commit can touch 55,184 paths and a list that builds a row each
//! is the thing this repository's invariants exist to stop. Its one pitch is the diff's
//! ([`COMMIT_TAB_ROW_HEIGHT`]), so a file opened in place draws its diff at the diff's row
//! height; the avatar spans three rows, each drawing its band of it. A message line is never
//! wrapped, as git never re-wraps one; a line wider than the pane scrolls sideways rather than
//! being cut.
//!
//! A file's diff opens in place under its row, as Fork opens it: a file pressed reports it,
//! and the caller opens or closes it; what an opened file draws — its rows, unified or side by
//! side as the shared setting says, the notice that stands in their place, or that it is being
//! read or could not be — is read from an [`Expansion`] and drawn as more rows of the same
//! list, at the same height, so a commit with any number of files open still builds one
//! viewport of rows. Above the files, Fork's Expand All (Collapse All while any file is open)
//! and, when Expand All stopped at its line budget, how many files it left collapsed.
//!
//! Above the id, Fork's REFS row: the chips of the refs pointing at the commit, as its row in
//! the history draws them — given to the tab laid out already, and cut at the pane's edge as
//! Fork's Mac pane cuts them — and no row for a commit with none. PARENTS stands for a root
//! commit too, with nothing beside it, as Fork's does.
//!
//! The tab never asks for anything: a parent link reports the parent and the caller decides
//! what pressing it reaches. The commit's id, its parents and the files' paths are drawn in the
//! diff's typeface, IBM Plex Mono. Fork's ⌄ beside each address is not drawn: nothing Cairn
//! builds fills its menu.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{ChangeSet, ChangeStatus, ChangedFile, Lane, Oid, Signature};
use freya::prelude::*;

use crate::push_button::{ButtonSize, PushButton};

use crate::accelerators;
use crate::avatar::{AvatarSize, avatar_of};
use crate::detail::expansion::{Expansion, Item, Opened};
use crate::detail::file_menu::{FileChoice, FileMenuIn, open_file_menu};
use crate::detail::message_lines;
use crate::diff::header::HIDDEN_CHANGES_NOTICE;
use crate::diff::notice::{DiffNotice, NoticeRow, NoticeTone, grouped, notice_rows};
use crate::diff::palette::{
    ADDED_EMPHASIS, DIFF_FONT_FAMILY, DIFF_FONT_SIZE, DIFF_MUTED, DIFF_TEXT, REMOVED_EMPHASIS,
};
use crate::diff::view::{RowGeometry, draw_row};
use crate::history::date_text;
use crate::history::ref_chips::{Chip, chips_row};
use crate::local_time::LocalZone;
use crate::style::{focus, icons, metrics, tones};

/// A row of the Changes tab's file list and of Local Changes' lists.
pub const DETAIL_ROW_HEIGHT: f32 = 24.0;

/// Every row of the Commit tab is this tall — a file opened in place's rows, its diff's among
/// them, at the diff's own row height (`style::metrics::DIFF_ROW`), and so every row above
/// them: a fixed size is what keeps the list O(viewport), and the virtualising view lays rows
/// of one size out without a walk. Fork's own header and file rows are taller (20 and 22 px,
/// uv7b) than its diff's 17; the one pitch is the diff's.
pub const COMMIT_TAB_ROW_HEIGHT: f32 = metrics::DIFF_ROW;

/// Fork's control above the files that opens every file.
pub const EXPAND_ALL_CAPTION: &str = "Expand All";
/// What it turns into while any file is open.
pub const COLLAPSE_ALL_CAPTION: &str = "Collapse All";
/// Said under a file opened in place while its diff is on its way.
pub const READING_DIFF: &str = "Reading the diff…";

/// Said beside Expand All when it stopped with its line budget spent: how many of the commit's
/// files it left collapsed.
pub fn budget_notice(collapsed: usize) -> String {
    format!(
        "Expand All stopped at its line budget: {} file{} left collapsed.",
        grouped(collapsed as u64),
        if collapsed == 1 { "" } else { "s" }
    )
}

const CAPTION_WIDTH: f32 = metrics::COMMIT_TAB_CAPTIONS;
const BADGE_WIDTH: f32 = 18.0;
const PADDING: f32 = metrics::COMMIT_TAB_INSET;
const GAP: f32 = metrics::COMMIT_TAB_GAP;
/// The room between the parent links, and between a file row's parts.
const VALUE_GAP: f32 = 8.0;
/// Where a file's row starts: its chevron, badge and document glyph then end where the values
/// start, `PADDING + CAPTION_WIDTH + GAP` from the pane's edge.
const FILE_INSET: f32 = PADDING + CAPTION_WIDTH + GAP
    - metrics::SIDEBAR_CHEVRON
    - BADGE_WIDTH
    - FILE_GLYPH
    - 3. * VALUE_GAP;
/// A file row's document glyph.
const FILE_GLYPH: f32 = 12.0;

/// The caption over the author's column.
pub const AUTHOR_CAPTION: &str = "AUTHOR";
/// The caption over the committer's column.
pub const COMMITTER_CAPTION: &str = "COMMITTER";
/// The caption beside the chips of the refs pointing at the commit.
pub const REFS_CAPTION: &str = "REFS";
/// The caption beside the full commit id.
pub const ID_CAPTION: &str = "SHA";
/// The caption beside the parent links.
pub const PARENTS_CAPTION: &str = "PARENTS";

/// What stands where the files would be when a commit changed none.
pub const NO_FILES: &str = "No files changed.";

/// The notice above the files when `diff.renameLimit` stopped git's rename search:
/// git's own warning, and the number it asks the limit to be raised to.
pub fn cut_short_notice(needed_limit: usize) -> String {
    format!(
        "Exhaustive rename detection was skipped due to too many files: set diff.renameLimit \
         to at least {needed_limit} to pair every rename."
    )
}

/// The bare letter Fork draws for `status`: git's `--name-status` letter without the
/// similarity score git prints after `R` and `C`.
pub fn status_letter(status: ChangeStatus) -> &'static str {
    match status {
        ChangeStatus::Added => "A",
        ChangeStatus::Deleted => "D",
        ChangeStatus::Modified => "M",
        ChangeStatus::TypeChanged => "T",
        ChangeStatus::Renamed(_) => "R",
        ChangeStatus::Copied(_) => "C",
    }
}

/// A file's row text: its path, or both of its paths for a rename or a copy, as Fork
/// draws them.
pub fn file_text(file: &ChangedFile) -> String {
    match file.status {
        ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => {
            format!("{} → {}", file.old_path.display(), file.new_path.display())
        }
        ChangeStatus::Added
        | ChangeStatus::Deleted
        | ChangeStatus::Modified
        | ChangeStatus::TypeChanged => file.new_path.display().into_owned(),
    }
}

/// One row above the files.
#[derive(Debug, Clone, PartialEq)]
enum Line {
    /// A row of the people's block: the author's, and the committer's beside it where it
    /// differs from the author ([`committer_differs`]).
    People(PeopleRow),
    /// The refs pointing at the commit, drawn as its row draws them, in its lane's colour.
    Refs(Refs),
    Id(String),
    /// The parent links: none for a root commit, whose row still stands, as Fork's does.
    Parents(Vec<Oid>),
    Rule,
    Subject(String),
    Body(String),
    CutShort(usize),
    NoFiles,
    /// Expand All, above the files, and what it stopped at.
    FilesBar,
}

/// Who a block of the people's rows is about: a name, its address and its date.
#[derive(Debug, Clone, PartialEq)]
struct Person {
    caption: &'static str,
    name: String,
    email: String,
    date: String,
}

/// Which row of a person's block: its caption, then the rows its avatar spans — the name and
/// address, the date, and the room under them the avatar still needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockRow {
    Caption,
    Name,
    Date,
    Tail,
}

impl BlockRow {
    const ALL: [BlockRow; 4] = [
        BlockRow::Caption,
        BlockRow::Name,
        BlockRow::Date,
        BlockRow::Tail,
    ];

    /// Which slice of the avatar the row draws, counted from its top: the avatar spans the
    /// rows under the caption.
    fn avatar_slice(self) -> Option<usize> {
        match self {
            Self::Caption => None,
            Self::Name => Some(0),
            Self::Date => Some(1),
            Self::Tail => Some(2),
        }
    }
}

/// One row of the people's block.
#[derive(Debug, Clone, PartialEq)]
struct PeopleRow {
    row: BlockRow,
    author: Rc<Person>,
    committer: Option<Rc<Person>>,
}

/// Whether the Commit tab draws the committer's block: only where the committer differs from
/// the author, as Fork draws it (`docs/research/ui-review/fork-committer-2026-10-20.md`).
pub fn committer_differs(author: &Signature, committer: &Signature) -> bool {
    author.name != committer.name
        || author.email != committer.email
        || author.time.seconds != committer.time.seconds
}

/// The chips of the refs pointing at a commit and the lane its row is in: what the REFS row
/// draws. No chips, no row.
#[derive(Debug, Clone, PartialEq)]
pub struct Refs {
    pub chips: Vec<Chip>,
    pub lane: Lane,
}

impl Default for Refs {
    fn default() -> Self {
        Self {
            chips: Vec::new(),
            lane: Lane::new(0),
        }
    }
}

/// The rows above the files, for `changes`. Proportional to the message and the refs drawn,
/// never to the files: those are read by index as they scroll into view.
fn header_lines(changes: &ChangeSet, refs: &Refs, zone: &LocalZone) -> Vec<Line> {
    #[cfg(test)]
    tests::HEADER_BUILDS.with(|builds| builds.set(builds.get() + 1));
    let mut lines = Vec::new();
    if let Some(details) = &changes.details {
        let person = |caption, signature: &Signature, note: &str| Person {
            caption,
            name: signature.name.clone(),
            email: signature.email.clone(),
            date: format!("{}{note}", date_text::long_date(signature.time, zone)),
        };
        // An unreadable zone is said once, after the author's date.
        let author = Rc::new(person(
            AUTHOR_CAPTION,
            &details.author,
            date_text::utc_fallback_note(zone),
        ));
        let committer = committer_differs(&details.author, &details.committer)
            .then(|| Rc::new(person(COMMITTER_CAPTION, &details.committer, "")));
        for row in BlockRow::ALL {
            lines.push(Line::People(PeopleRow {
                row,
                author: author.clone(),
                committer: committer.clone(),
            }));
        }
        if !refs.chips.is_empty() {
            lines.push(Line::Refs(refs.clone()));
        }
        lines.push(Line::Id(details.id.hex().as_str().to_owned()));
        lines.push(Line::Parents(details.parents.clone()));
        lines.push(Line::Rule);
        // The lines `git log` shows, not the bytes stored: git trims and expands them.
        for (n, text) in message_lines::shown_lines(&details.message)
            .into_iter()
            .enumerate()
        {
            lines.push(if n == 0 {
                Line::Subject(text)
            } else {
                Line::Body(text)
            });
        }
        lines.push(Line::Rule);
    }
    if let Some(needed) = changes.renames.needed_limit {
        lines.push(Line::CutShort(needed));
    }
    if changes.files.is_empty() {
        lines.push(Line::NoFiles);
    } else {
        lines.push(Line::FilesBar);
    }
    lines
}

/// Everything [`header_lines`] reads from a change set but the commit's own fields, which
/// its id names, and the refs drawn: one commit's header is built once, however often the
/// state it is read from is written (a file's diff arriving) without changing the commit or
/// its refs.
#[derive(Debug, Clone, PartialEq)]
struct HeaderKey {
    commit: Option<Oid>,
    needed_limit: Option<usize>,
    no_files: bool,
    refs: Refs,
    zone: LocalZone,
}

impl HeaderKey {
    fn of(changes: &ChangeSet, refs: &Refs, zone: &LocalZone) -> Self {
        Self {
            commit: changes.details.as_ref().map(|details| details.id),
            needed_limit: changes.renames.needed_limit,
            no_files: changes.files.is_empty(),
            refs: refs.clone(),
            zone: zone.clone(),
        }
    }
}

/// The header last built, and what it was built from.
type HeaderCache = Rc<RefCell<Option<(HeaderKey, Rc<Vec<Line>>)>>>;

/// The header for `changes`: the one in `cache` while its key still holds.
fn cached_header(
    cache: &HeaderCache,
    changes: &ChangeSet,
    refs: &Refs,
    zone: &LocalZone,
) -> Rc<Vec<Line>> {
    let key = HeaderKey::of(changes, refs, zone);
    let mut cached = cache.borrow_mut();
    if let Some((built_for, lines)) = cached.as_ref()
        && *built_for == key
    {
        return lines.clone();
    }
    let lines = Rc::new(header_lines(changes, refs, zone));
    *cached = Some((key, lines.clone()));
    lines
}

/// The Commit tab over one change set. `changes` is a handle, not a copy: the files are read
/// by index as rows are built, so a commit of any size costs one viewport per frame; so is
/// `expansion`, what each file opened in place draws.
///
/// The tab takes focus when it is pressed or reached with Tab, and while it has it, ↑ and ↓
/// move the current file — Fork's previous and next file, keys of the focused list rather
/// than chords. A chord pressed here is left for whoever hears it.
pub struct CommitTab {
    changes: Readable<ChangeSet>,
    refs: Refs,
    zone: LocalZone,
    expansion: Readable<Expansion>,
    side_by_side: bool,
    on_parent: EventHandler<Oid>,
    on_file: EventHandler<usize>,
    on_file_pressed: EventHandler<usize>,
    on_expand_all: EventHandler<bool>,
    on_load: EventHandler<usize>,
    on_file_menu: EventHandler<(usize, FileChoice)>,
    key: DiffKey,
}

impl CommitTab {
    pub fn new(changes: impl Into<Readable<ChangeSet>>) -> Self {
        Self {
            changes: changes.into(),
            refs: Refs::default(),
            zone: LocalZone::default(),
            expansion: Readable::from_value(Expansion::new()),
            side_by_side: false,
            on_parent: EventHandler::new(|_| {}),
            on_file: EventHandler::new(|_| {}),
            on_file_pressed: EventHandler::new(|_| {}),
            on_expand_all: EventHandler::new(|_| {}),
            on_load: EventHandler::new(|_| {}),
            on_file_menu: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// The chips of the refs pointing at the commit, laid out as its row in the history lays
    /// them out, and that row's lane: the REFS row.
    pub fn refs(mut self, refs: Refs) -> Self {
        self.refs = refs;
        self
    }

    /// The zone the author's and committer's dates are drawn in, handed down by the window.
    pub fn zone(mut self, zone: LocalZone) -> Self {
        self.zone = zone;
        self
    }

    /// What the files opened in place draw, by their index in the change set.
    pub fn expansion(mut self, expansion: impl Into<Readable<Expansion>>) -> Self {
        self.expansion = expansion.into();
        self
    }

    /// Draw opened files side by side rather than unified: the one setting every diff view
    /// shares (users report Fork's Commit-tab diffs follow it).
    pub fn side_by_side(mut self, side_by_side: bool) -> Self {
        self.side_by_side = side_by_side;
        self
    }

    /// A parent link was pressed. Whether that parent can be reached is the caller's to
    /// decide (a loaded parent is selected; an unloaded one is issue #3).
    pub fn on_parent(mut self, on_parent: impl Into<EventHandler<Oid>>) -> Self {
        self.on_parent = on_parent.into();
        self
    }

    /// A file became the current one — pressed, or reached with ↑ or ↓ — by its index in the
    /// change set's files.
    pub fn on_file(mut self, on_file: impl Into<EventHandler<usize>>) -> Self {
        self.on_file = on_file.into();
        self
    }

    /// A file was pressed — after [`Self::on_file`] reports it current — as distinct from
    /// reached with an arrow: Fork opens a pressed file in place, and closes an open one.
    /// Which it does is the caller's.
    pub fn on_file_pressed(mut self, on_file_pressed: impl Into<EventHandler<usize>>) -> Self {
        self.on_file_pressed = on_file_pressed.into();
        self
    }

    /// Expand All (`true`) or Collapse All (`false`) was pressed.
    pub fn on_expand_all(mut self, on_expand_all: impl Into<EventHandler<bool>>) -> Self {
        self.on_expand_all = on_expand_all.into();
        self
    }

    /// Load Diff was pressed under the file opened in place at this index.
    pub fn on_load(mut self, on_load: impl Into<EventHandler<usize>>) -> Self {
        self.on_load = on_load.into();
        self
    }

    /// A file's menu item was chosen from the menu a right press on its row opens, which first
    /// makes it the current file without opening or closing it, by the file's index in the
    /// change set.
    pub fn on_file_menu(
        mut self,
        on_file_menu: impl Into<EventHandler<(usize, FileChoice)>>,
    ) -> Self {
        self.on_file_menu = on_file_menu.into();
        self
    }
}

// Hand-written: `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for CommitTab {
    fn eq(&self, other: &Self) -> bool {
        self.changes == other.changes
            && self.refs == other.refs
            && self.zone == other.zone
            && self.expansion == other.expansion
            && self.side_by_side == other.side_by_side
            && self.key == other.key
    }
}

impl std::fmt::Debug for CommitTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommitTab").finish_non_exhaustive()
    }
}

impl KeyExt for CommitTab {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// What one commit's list is drawn from, as the tab hands it to the list.
#[derive(Clone)]
struct TabContent {
    changes: Readable<ChangeSet>,
    expansion: Readable<Expansion>,
    lines: Rc<Vec<Line>>,
    files: usize,
    /// The rows the opened files add, read as the tab renders: a file opening or closing
    /// changes the list's length.
    added: usize,
    side_by_side: bool,
    on_parent: EventHandler<Oid>,
    on_file: EventHandler<usize>,
    on_file_pressed: EventHandler<usize>,
    on_expand_all: EventHandler<bool>,
    on_load: EventHandler<usize>,
    on_file_menu: EventHandler<(usize, FileChoice)>,
    /// The tab's focus target, which a pressed row takes.
    tab_id: AccessibilityId,
}

impl TabContent {
    fn length(&self) -> usize {
        self.lines.len() + self.files + self.added
    }
}

impl PartialEq for TabContent {
    fn eq(&self, other: &Self) -> bool {
        // The header is cached, so an unchanged one is the same allocation: no compare.
        (Rc::ptr_eq(&self.lines, &other.lines) || self.lines == other.lines)
            && self.files == other.files
            && self.added == other.added
            && self.side_by_side == other.side_by_side
            && self.tab_id == other.tab_id
    }
}

/// Data captured inside the builder closure is invisible to `VirtualScrollView`'s diffing,
/// so what decides which rows are drawn is passed here.
#[derive(Clone, PartialEq)]
struct TabData {
    content: TabContent,
    /// The current file, by index into the files; drawn highlighted.
    current: Option<usize>,
    cursor: State<Option<usize>>,
    /// The tab's width, which a side-by-side diff's columns are halves of.
    view_width: f32,
    /// Read as a side-by-side row is built, for the sideways scroll its text slides by.
    scroll: ScrollController,
    /// Whether the tab has focus, which the current file's tone says.
    focused: bool,
}

/// One commit's list. Keyed by the commit, so another commit is a list of its own: it opens
/// at its top, with no file current.
#[derive(Clone)]
struct TabBody {
    content: TabContent,
    key: DiffKey,
}

impl PartialEq for TabBody {
    fn eq(&self, other: &Self) -> bool {
        self.content == other.content && self.key == other.key
    }
}

impl KeyExt for TabBody {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for TabBody {
    fn render(&self) -> impl IntoElement {
        let tab_id = self.content.tab_id;
        let focus = use_focus(tab_id);
        let controller = use_scroll_controller(ScrollConfig::default);
        let cursor = use_state(|| None::<usize>);
        let mut width = use_state(|| 0.0f32);
        let data = TabData {
            content: self.content.clone(),
            current: *cursor.read(),
            cursor,
            view_width: *width.read(),
            scroll: controller,
            focused: focus().is_focused(),
        };
        // The rows after the last are room for the horizontal scrollbar (`end_room`).
        let length = crate::end_room::with_end_room(data.content.length(), COMMIT_TAB_ROW_HEIGHT);
        rect()
            .expanded()
            .a11y_id(tab_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::List)
            .on_key_down(keyboard(&data, controller))
            .on_sized(move |e: Event<SizedEventData>| width.set_if_modified(e.area.width()))
            .border(focus::ring_when(focus()))
            .child(
                VirtualScrollView::new_with_data_controlled(data, build_row, controller)
                    .length(length)
                    .item_size(COMMIT_TAB_ROW_HEIGHT)
                    // The arrows move the current file, not the viewport.
                    .scroll_with_arrows(false)
                    .expanded(),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// ↑ and ↓ over the files; every other key, and every chord, is left unhandled — the copy
/// chords too, which Fork's Commit tab menu shows none of.
fn keyboard(
    data: &TabData,
    mut controller: ScrollController,
) -> impl FnMut(Event<KeyboardEventData>) + 'static {
    let (files, header) = (data.content.files, data.content.lines.len());
    let (expansion, side_by_side) = (data.content.expansion.clone(), data.content.side_by_side);
    let mut cursor = data.cursor;
    let on_file = data.content.on_file.clone();
    move |e: Event<KeyboardEventData>| {
        if accelerators::is_chord(&e, &[]) {
            return;
        }
        let Some(last) = files.checked_sub(1) else {
            return;
        };
        // Nothing current yet: either arrow starts at the first file.
        let current = *cursor.peek();
        let next = match e.key {
            Key::Named(NamedKey::ArrowDown) => current.map_or(0, |at| (at + 1).min(last)),
            Key::Named(NamedKey::ArrowUp) => current.map_or(0, |at| at.saturating_sub(1)),
            _ => return,
        };
        e.stop_propagation();
        cursor.set(Some(next));
        on_file.call(next);
        // Past the rows the files opened above it add.
        let row = header + expansion.peek().position(next, side_by_side);
        controller.scroll_to_offset(
            row as f32 * COMMIT_TAB_ROW_HEIGHT,
            COMMIT_TAB_ROW_HEIGHT,
            Direction::Vertical,
        );
    }
}

impl Component for CommitTab {
    fn render(&self) -> impl IntoElement {
        let cache: HeaderCache = use_hook(HeaderCache::default);
        // The tab's, not one commit's: focus stays on the tab as the commit changes.
        let tab_id = use_a11y();
        // Reading subscribes the tab to the answer it draws.
        let (lines, files, identity) = {
            let changes = self.changes.read();
            let identity = changes.details.as_ref().map(|details| details.id);
            (
                cached_header(&cache, &changes, &self.refs, &self.zone),
                changes.files.len(),
                identity,
            )
        };
        let added = self.expansion.read().added_rows(self.side_by_side);
        let content = TabContent {
            changes: self.changes.clone(),
            expansion: self.expansion.clone(),
            lines,
            files,
            added,
            side_by_side: self.side_by_side,
            on_parent: self.on_parent.clone(),
            on_file: self.on_file.clone(),
            on_file_pressed: self.on_file_pressed.clone(),
            on_expand_all: self.on_expand_all.clone(),
            on_load: self.on_load.clone(),
            on_file_menu: self.on_file_menu.clone(),
            tab_id,
        };

        TabBody {
            content,
            key: DiffKey::None,
        }
        // Another commit is another list: it opens at its top, not where the last one was.
        .key(identity)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

fn build_row(item: VirtualItem, data: &TabData) -> Element {
    let row = rect()
        .key(item.index)
        .min_width(Size::fill())
        .height(Size::px(item.size))
        .main_align(Alignment::Center)
        .padding(Gaps::new(0., PADDING, 0., PADDING));
    let content = &data.content;
    if item.index >= content.length() {
        // Room for the horizontal scrollbar after the last row (`end_room`).
        return row.into();
    }
    let Some(at) = item.index.checked_sub(content.lines.len()) else {
        return match content.lines.get(item.index) {
            Some(Line::FilesBar) => row.child(files_bar(content)).into(),
            Some(line) => row.child(header_row(line, &content.on_parent)).into(),
            None => row.into(),
        };
    };
    // Read, not peeked: the list redraws when a file opens, closes or is answered.
    let placed = content.expansion.read().item(at, content.side_by_side);
    match placed {
        Item::File(index) => file_item(row, index, data),
        Item::Under { file, row: under } => under_row(item, file, under, data),
    }
}

/// A changed file's own row: its disclosure, status letter and path; pressed, it becomes the
/// current file and is reported pressed.
fn file_item(row: Rect, index: usize, data: &TabData) -> Element {
    let content = &data.content;
    // Read, not peeked: the list redraws when the answer it shows is replaced.
    let current = data.current == Some(index);
    let chosen = tones::selected(data.focused);
    let Some(drawn) = content
        .changes
        .read()
        .files
        .get(index)
        .map(|file| file_row(file, current.then_some(chosen.text)))
    else {
        // The count and the files can disagree for one frame; an empty row of the right
        // height stands in.
        return row.into();
    };
    let open = content.expansion.read().is_open(index);
    let (tab_id, mut cursor, on_file) = (content.tab_id, data.cursor, content.on_file.clone());
    let mut right_cursor = data.cursor;
    let on_file_pressed = content.on_file_pressed.clone();
    // Turned horizontal, the row's main axis is its width, where the centring `build_row`
    // gives a vertical row would put the file in the middle of the pane: it starts at its
    // padding, and `cross_align` centres it top to bottom.
    row.horizontal()
        .main_align(Alignment::Start)
        .cross_align(Alignment::Center)
        .spacing(VALUE_GAP)
        // The chevron stands in under the captions, so the path starts at the values' column
        // as Fork's does (uv7b).
        .padding(Gaps::new(0., PADDING, 0., FILE_INSET))
        .maybe(current, |el| el.background(chosen.band))
        .on_press(move |_| {
            tab_id.request_focus();
            cursor.set(Some(index));
            on_file.call(index);
            on_file_pressed.call(index);
        })
        // A right press makes the file current, opening or closing nothing, and opens its menu.
        .on_pointer_down({
            let (on_file, on_file_menu) = (content.on_file.clone(), content.on_file_menu.clone());
            move |e: Event<PointerEventData>| {
                if e.button() != Some(MouseButton::Right) {
                    return;
                }
                tab_id.request_focus();
                if !current {
                    right_cursor.set(Some(index));
                    on_file.call(index);
                }
                open_file_menu(FileMenuIn::CommitTab, tab_id, index, on_file_menu.clone());
            }
        })
        .child(if open {
            icons::chevron_down
        } else {
            icons::chevron_right
        }(
            metrics::SIDEBAR_CHEVRON,
            if current {
                chosen.text
            } else {
                tones::DETAIL_LABEL
            },
        ))
        .child(drawn)
        .into()
}

/// Row `under` of what the file opened in place at `file` draws under its own row.
fn under_row(item: VirtualItem, file: usize, under: usize, data: &TabData) -> Element {
    let content = &data.content;
    let plain = |text: Label| -> Element {
        rect()
            .key(item.index)
            .min_width(Size::fill())
            .height(Size::px(item.size))
            .main_align(Alignment::Center)
            .padding(Gaps::new(0., PADDING, 0., PADDING))
            .child(text)
            .into()
    };
    let expansion = content.expansion.read();
    let Some(opened) = expansion.get(file) else {
        // Closed between the count and this row, for one frame.
        return plain(label());
    };
    match opened {
        Opened::Reading => plain(text(READING_DIFF, metrics::BODY, tones::PLACEHOLDER)),
        Opened::Failed(message) => plain(text(message.clone(), metrics::BODY, colours().error)),
        Opened::Shown(shown) => {
            if let Some(notice) = DiffNotice::of(shown) {
                let rows = notice_rows(&notice);
                return match rows.get(under) {
                    Some(NoticeRow::LoadDiff) => {
                        let on_load = content.on_load.clone();
                        rect()
                            .key(item.index)
                            .height(Size::px(item.size))
                            .main_align(Alignment::Center)
                            .padding(Gaps::new(0., PADDING, 0., PADDING))
                            .child(
                                PushButton::caption(
                                    crate::diff::notice::LOAD_DIFF_CAPTION,
                                    ButtonSize::Compact,
                                )
                                .on_press(move || on_load.call(file)),
                            )
                            .into()
                    }
                    Some(row) => plain(notice_line(row)),
                    None => plain(label()),
                };
            }
            let hides = shown.hides_changes();
            if hides && under == 0 {
                return plain(text(HIDDEN_CHANGES_NOTICE, metrics::BODY, DIFF_MUTED));
            }
            let geometry = RowGeometry::of(
                shown,
                content.side_by_side,
                data.view_width,
                None,
                data.scroll,
            );
            draw_row(
                item.index,
                under - usize::from(hides),
                item.size,
                shown,
                &geometry,
            )
        }
    }
}

/// One row of a notice standing in place of an opened file's rows: the words the Changes tab's
/// notice draws, a line each.
pub(crate) fn notice_line(row: &NoticeRow) -> Label {
    let tone = |tone: &NoticeTone| match tone {
        NoticeTone::Old => REMOVED_EMPHASIS,
        NoticeTone::New => ADDED_EMPHASIS,
        NoticeTone::Plain => DIFF_TEXT,
    };
    match row {
        NoticeRow::Title(words) => text(words.clone(), metrics::BODY, DIFF_TEXT),
        NoticeRow::Muted(words) => text(words.clone(), metrics::BODY, DIFF_MUTED),
        NoticeRow::Side {
            caption,
            tone: side,
            text: words,
        } => label()
            .max_lines(1)
            .font_size(metrics::BODY)
            .color(tone(side))
            .text(match words {
                Some(words) => format!("{caption}  {words}"),
                None => (*caption).to_owned(),
            }),
        NoticeRow::Git {
            text: words,
            tone: line,
        } => text(words.clone(), DIFF_FONT_SIZE, tone(line)).font_family(DIFF_FONT_FAMILY),
        NoticeRow::LoadDiff => text(
            crate::diff::notice::LOAD_DIFF_CAPTION,
            metrics::BODY,
            DIFF_TEXT,
        ),
    }
}

/// Expand All, right-aligned above the files as Fork places it — Collapse All while any file
/// is open — and, when Expand All stopped at its line budget, how many files it left
/// collapsed.
fn files_bar(content: &TabContent) -> Element {
    let (any_open, stopped, collapsed) = {
        let expansion = content.expansion.read();
        (
            !expansion.is_empty(),
            expansion.stopped_at_budget(),
            content.files.saturating_sub(expansion.len()),
        )
    };
    let on_expand_all = content.on_expand_all.clone();
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(GAP)
        .child(rect().width(Size::flex(1.)).maybe_child(
            stopped.then(|| text(budget_notice(collapsed), metrics::BODY, tones::DETAIL_LABEL)),
        ))
        .child(ExpandAll {
            any_open,
            on_expand_all,
        })
        .into()
}

/// Fork's Expand All — Collapse All while any file is open — its caption alone, no button drawn
/// round it (uv9); a Tab stop, Space or Enter pressing it, with a ring while focus came by key.
#[derive(Clone)]
struct ExpandAll {
    any_open: bool,
    on_expand_all: EventHandler<bool>,
}

impl PartialEq for ExpandAll {
    fn eq(&self, other: &Self) -> bool {
        self.any_open == other.any_open
    }
}

impl Component for ExpandAll {
    fn render(&self) -> impl IntoElement {
        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let (any_open, on_expand_all) = (self.any_open, self.on_expand_all.clone());
        rect()
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .border(focus::ring_when(focus()))
            .on_press(move |_| on_expand_all.call(!any_open))
            .child(text(
                if any_open {
                    COLLAPSE_ALL_CAPTION
                } else {
                    EXPAND_ALL_CAPTION
                },
                metrics::BODY,
                tones::TEXT,
            ))
    }
}

/// The theme's status tones (error, warning and the badges Fork Dark's table does not name).
fn colours() -> ColorsSheet {
    get_theme_or_default().read().colors().clone()
}

fn text(content: impl Into<String>, size: f32, colour: Color) -> Label {
    label()
        .text(content.into())
        .max_lines(1)
        .font_size(size)
        .color(colour)
}

fn caption(content: &'static str) -> Label {
    text(content, metrics::CAPTION, tones::DETAIL_LABEL)
}

fn header_row(line: &Line, on_parent: &EventHandler<Oid>) -> Element {
    let colours = colours();
    match line {
        Line::People(people) => people_row(people),
        // Cut at the pane's edge, never wrapped and never counted (Fork's Mac pane).
        Line::Refs(refs) => rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(COMMIT_TAB_ROW_HEIGHT))
            .cross_align(Alignment::Center)
            .spacing(GAP)
            .child(
                caption(REFS_CAPTION)
                    .width(Size::px(CAPTION_WIDTH))
                    .text_align(TextAlign::End),
            )
            .child(
                chips_row(&refs.chips, refs.lane)
                    .width(Size::flex(1.))
                    .height(Size::px(COMMIT_TAB_ROW_HEIGHT))
                    .overflow(Overflow::Clip),
            )
            .into(),
        Line::Id(id) => labelled(
            ID_CAPTION,
            text(id.clone(), metrics::BODY, tones::TEXT)
                .font_family(DIFF_FONT_FAMILY)
                .into(),
        ),
        Line::Parents(parents) => labelled(
            PARENTS_CAPTION,
            rect()
                .horizontal()
                .spacing(VALUE_GAP)
                .children(parents.iter().map(|parent| -> Element {
                    let parent = *parent;
                    let on_parent = on_parent.clone();
                    rect()
                        .key(parent)
                        .a11y_role(AccessibilityRole::Link)
                        .cursor(CursorIcon::Pointer)
                        .on_press(move |_| on_parent.call(parent))
                        .child(
                            text(
                                parent.short().as_str().to_owned(),
                                metrics::BODY,
                                tones::LINK,
                            )
                            .font_family(DIFF_FONT_FAMILY),
                        )
                        .into()
                }))
                .into(),
        ),
        Line::Rule => rect()
            .width(Size::fill())
            .height(Size::px(1.))
            .background(tones::RULE)
            .into(),
        Line::Subject(subject) => in_values(
            text(subject.clone(), metrics::DETAIL_SUBJECT, tones::TEXT)
                .font_weight(FontWeight::BOLD)
                .into(),
        ),
        Line::Body(body) => in_values(text(body.clone(), metrics::BODY, tones::TEXT).into()),
        Line::CutShort(needed) => {
            text(cut_short_notice(*needed), metrics::BODY, colours.warning).into()
        }
        Line::NoFiles => text(NO_FILES, metrics::BODY, tones::PLACEHOLDER).into(),
        // Drawn by the list, which holds what it reads (`files_bar`).
        Line::FilesBar => rect().into(),
    }
}

/// One row of the people's block: the author's part, and the committer's beside it where it
/// is drawn.
fn people_row(people: &PeopleRow) -> Element {
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .child(person_part(&people.author, people.row))
        .maybe_child(
            people
                .committer
                .as_ref()
                .map(|committer| person_part(committer, people.row)),
        )
        .into()
}

/// A person's part of one row of the block: under the caption column, the slice of the avatar
/// this row draws; beside it, the caption, the name with its address, or the date.
fn person_part(person: &Person, row: BlockRow) -> Element {
    let value: Element = match row {
        BlockRow::Caption => caption(person.caption).into(),
        BlockRow::Name => paragraph()
            .max_lines(1)
            .font_size(metrics::BODY)
            .text_overflow(TextOverflow::Ellipsis)
            .span(
                Span::new(person.name.clone())
                    .color(tones::TEXT)
                    .font_weight(FontWeight::BOLD),
            )
            .span(Span::new(format!("  {}", person.email)).color(tones::DETAIL_LABEL))
            .into(),
        BlockRow::Date => text(person.date.clone(), metrics::BODY, tones::DETAIL_LABEL).into(),
        BlockRow::Tail => rect().into(),
    };
    rect()
        .horizontal()
        .width(Size::flex(1.))
        .height(Size::px(COMMIT_TAB_ROW_HEIGHT))
        .cross_align(Alignment::Center)
        .spacing(GAP)
        .child(
            rect()
                .width(Size::px(CAPTION_WIDTH))
                .height(Size::px(COMMIT_TAB_ROW_HEIGHT))
                .main_align(Alignment::End)
                .horizontal()
                .maybe_child(
                    row.avatar_slice()
                        .map(|slice| avatar_slice(&person.name, slice)),
                ),
        )
        .child(value)
        .into()
}

/// The `slice`th row-high band of `name`'s avatar, counted from its top: the avatar spans the
/// rows under its caption, each row drawing its band, so it is whole however the list is
/// scrolled and each row stays one pitch tall.
fn avatar_slice(name: &str, slice: usize) -> Element {
    // Each band reaches a pixel into the next row, which draws over it: at a fractional offset
    // the bands' smoothed edges would otherwise show as seams.
    rect()
        .width(Size::px(metrics::DETAIL_AVATAR))
        .height(Size::px(COMMIT_TAB_ROW_HEIGHT + 1.))
        .overflow(Overflow::Clip)
        // A rect's offset moves what it holds, not itself.
        .offset_y(-(slice as f32) * COMMIT_TAB_ROW_HEIGHT)
        .child(avatar_of(name, AvatarSize::Detail))
        .into()
}

/// `value` in the values' column, where Fork starts the message (uv9).
fn in_values(value: Element) -> Element {
    rect()
        .padding(Gaps::new(0., 0., 0., CAPTION_WIDTH + GAP))
        .child(value)
        .into()
}

/// A grey caption right-aligned against its value, as Fork's `SHA` and `PARENTS` rows.
fn labelled(name: &'static str, value: Element) -> Element {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(GAP)
        .child(
            caption(name)
                .width(Size::px(CAPTION_WIDTH))
                .text_align(TextAlign::End),
        )
        .child(value)
        .into()
}

/// A changed file's badge and path, the path bold in `selected`'s tone when the row is the
/// list's chosen one. Fork Dark's badges where it gives one (an added file's green, a modified
/// one's yellow), the theme's status tones for the rest.
pub(crate) fn file_row(file: &ChangedFile, selected: Option<Color>) -> Element {
    let colours = colours();
    let badge = match file.status {
        ChangeStatus::Added => tones::ADDED_BADGE,
        ChangeStatus::Deleted => colours.error,
        ChangeStatus::Modified | ChangeStatus::TypeChanged => tones::MODIFIED_BADGE,
        ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => colours.info,
    };
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(VALUE_GAP)
        .child(
            text(status_letter(file.status), metrics::BODY, badge)
                .width(Size::px(BADGE_WIDTH))
                .font_weight(FontWeight::BOLD),
        )
        .child(icons::file(FILE_GLYPH, tones::DETAIL_LABEL))
        .child(
            text(
                file_text(file),
                metrics::BODY,
                selected.unwrap_or(tones::TEXT),
            )
            .maybe(selected.is_some(), |el| el.font_weight(FontWeight::BOLD))
            .font_family(DIFF_FONT_FAMILY),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use cairn_model::{CommitDetails, FileMode, RenameDetection, RepoPath, Similarity, Timestamp};
    use freya_testing::TestingRunner;

    use super::*;

    thread_local! {
        /// How many headers [`header_lines`] has built on this thread.
        pub(super) static HEADER_BUILDS: Cell<usize> = const { Cell::new(0) };
    }

    fn details(message: &str, parents: usize) -> CommitDetails {
        let signature = |name: &str| Signature {
            name: name.to_owned(),
            email: format!("{name}@example.com"),
            time: Timestamp::new(1_700_000_000, 0),
        };
        CommitDetails {
            id: Oid::from_bytes(&[7; 20]).unwrap(),
            parents: (0..parents)
                .map(|n| Oid::from_bytes(&[n as u8; 20]).unwrap())
                .collect(),
            author: signature("ada"),
            committer: signature("grace"),
            message: message.to_owned(),
        }
    }

    fn set(details: Option<CommitDetails>, files: usize, needed: Option<usize>) -> ChangeSet {
        ChangeSet {
            files: (0..files)
                .map(|n| ChangedFile {
                    status: ChangeStatus::Modified,
                    old_path: RepoPath::from(format!("f{n}").as_str()),
                    new_path: RepoPath::from(format!("f{n}").as_str()),
                    old_mode: Some(FileMode::Regular),
                    new_mode: Some(FileMode::Regular),
                    old_id: None,
                    new_id: None,
                })
                .collect(),
            details,
            renames: RenameDetection {
                needed_limit: needed,
                ..RenameDetection::default()
            },
        }
    }

    /// Every line of the message is a row, the trailing newline git writes is not, and the
    /// header's size does not grow with the files. Caught by: dropping the body, keeping
    /// the empty line after the final newline, or building a row per file here.
    #[test]
    fn the_header_is_the_message_and_never_the_files() {
        let lines = header_lines(
            &set(
                Some(details("subject\n\nbody one\r\nbody two\n", 2)),
                10_000,
                None,
            ),
            &Refs::default(),
            &LocalZone::default(),
        );
        let message: Vec<&Line> = lines
            .iter()
            .filter(|line| matches!(line, Line::Subject(_) | Line::Body(_)))
            .collect();
        assert_eq!(
            message,
            [
                &Line::Subject("subject".to_owned()),
                &Line::Body(String::new()),
                &Line::Body("body one".to_owned()),
                &Line::Body("body two".to_owned()),
            ]
        );
        assert!(lines.len() < 20, "{} header rows", lines.len());
    }

    /// A root commit's parents row stands empty; a cut-short search says so; an empty commit
    /// says it changed nothing.
    #[test]
    fn the_header_says_what_is_missing_and_what_was_cut_short() {
        let root = header_lines(
            &set(Some(details("x", 0)), 0, Some(2774)),
            &Refs::default(),
            &LocalZone::default(),
        );
        // Fork draws PARENTS with nothing beside it for a root commit (cm13).
        assert!(root.contains(&Line::Parents(Vec::new())));
        assert!(root.contains(&Line::CutShort(2774)));
        assert!(root.contains(&Line::NoFiles));

        let merge = header_lines(
            &set(Some(details("x", 2)), 1, None),
            &Refs::default(),
            &LocalZone::default(),
        );
        assert!(
            merge
                .iter()
                .any(|line| matches!(line, Line::Parents(p) if p.len() == 2))
        );
        assert!(!merge.iter().any(|line| matches!(line, Line::CutShort(_))));
        assert!(!merge.contains(&Line::NoFiles));
    }

    fn refs_of(names: &[&str]) -> Refs {
        Refs {
            chips: names
                .iter()
                .map(|name| Chip {
                    kind: crate::history::ref_chips::ChipKind::Tag,
                    text: (*name).to_owned(),
                })
                .collect(),
            lane: Lane::new(2),
        }
    }

    /// The REFS row stands above the id, holding the chips given, and a commit with none has
    /// no row. Caught by: a row drawn for no refs, the row below the id, or the chips lost.
    #[test]
    fn the_refs_row_stands_above_the_id_and_only_when_a_ref_points_at_the_commit() {
        let changes = set(Some(details("x", 1)), 1, None);
        let none = header_lines(&changes, &Refs::default(), &LocalZone::default());
        assert!(!none.iter().any(|line| matches!(line, Line::Refs(_))));

        let two = refs_of(&["v1.0", "v1.1"]);
        let lines = header_lines(&changes, &two, &LocalZone::default());
        let at = |wanted: fn(&Line) -> bool| lines.iter().position(wanted);
        let refs = at(|line| matches!(line, Line::Refs(_)));
        let id = at(|line| matches!(line, Line::Id(_)));
        let people = lines
            .iter()
            .rposition(|line| matches!(line, Line::People(_)));
        assert!(
            people < refs && refs < id && refs.is_some(),
            "REFS at {refs:?}, the people's block ending at {people:?}, the id at {id:?}"
        );
        assert_eq!(lines.get(refs.unwrap_or_default()), Some(&Line::Refs(two)));
    }

    /// A write to the state the tab reads that leaves the commit as it was — what a file's
    /// diff arriving in the window's `DiffState` is — redraws the tab without building its
    /// header again; another commit builds it once. Caught by: building the header on every
    /// render, or caching it past a change of commit.
    #[test]
    fn the_header_is_built_once_per_commit_however_often_the_state_is_written() {
        #[derive(Clone)]
        struct Fixture {
            state: State<(ChangeSet, u32)>,
            refs: State<Refs>,
        }
        let app = || {
            let fixture = use_consume::<Fixture>();
            let changes = fixture
                .state
                .into_readable()
                .map(|(changes, _)| changes, |_| true);
            rect()
                .expanded()
                .child(CommitTab::new(changes).refs(fixture.refs.read().clone()))
                .into_element()
        };
        let (mut test, fixture) = TestingRunner::new(
            app,
            (600., 400.).into(),
            |runner| {
                runner.provide_root_context(|| Fixture {
                    state: State::create((set(Some(details("one\n", 1)), 3, None), 0)),
                    refs: State::create(Refs::default()),
                })
            },
            1.,
        );
        test.sync_and_update();
        let builds = || HEADER_BUILDS.with(Cell::get);
        let first = builds();
        assert!(first >= 1, "the header was never built");

        let mut state = fixture.state;
        for unrelated in 1..=3 {
            state.write().1 = unrelated;
            test.sync_and_update();
        }
        assert_eq!(builds(), first, "an unrelated write rebuilt the header");

        let mut other = details("two\n", 1);
        other.id = Oid::from_bytes(&[8; 20]).unwrap();
        state.write().0 = set(Some(other), 3, None);
        test.sync_and_update();
        assert_eq!(
            builds(),
            first + 1,
            "another commit did not rebuild the header once"
        );

        // The refs pointing at the commit changed (a refresh moved a branch onto it): the
        // REFS row is built again, once.
        let mut refs = fixture.refs;
        refs.set(refs_of(&["main"]));
        test.sync_and_update();
        assert_eq!(
            builds(),
            first + 2,
            "new refs did not rebuild the header once"
        );
        state.write().1 = 9;
        test.sync_and_update();
        assert_eq!(builds(), first + 2, "the same refs rebuilt the header");
    }

    /// Each status is its bare letter, as Fork draws it — a rename and a copy without the
    /// similarity git's `--name-status` prints after the letter. Caught by: a letter swapped
    /// between two statuses, or a score appended.
    #[test]
    fn every_status_is_its_bare_letter() {
        let similar = Similarity::from_percent(87);
        for (status, letter) in [
            (ChangeStatus::Added, "A"),
            (ChangeStatus::Deleted, "D"),
            (ChangeStatus::Modified, "M"),
            (ChangeStatus::TypeChanged, "T"),
            (ChangeStatus::Renamed(similar), "R"),
            (ChangeStatus::Copied(similar), "C"),
        ] {
            assert_eq!(status_letter(status), letter, "{status:?}");
        }
    }

    #[test]
    fn a_rename_and_a_copy_show_both_names_and_nothing_else_does() {
        let mut file = set(None, 1, None).files.remove(0);
        file.old_path = RepoPath::from("old/name.rs");
        file.new_path = RepoPath::from("new/name.rs");
        for status in [
            ChangeStatus::Renamed(Similarity::from_percent(90)),
            ChangeStatus::Copied(Similarity::from_percent(90)),
        ] {
            file.status = status;
            assert_eq!(file_text(&file), "old/name.rs → new/name.rs");
        }
        file.status = ChangeStatus::Modified;
        assert_eq!(file_text(&file), "new/name.rs");
    }
}
