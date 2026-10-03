//! The Changes tab's left side (PRD R5.4, R5.5): a filter field over the changed files, and
//! the files it leaves as one virtualised list; and the tab's one-line summary of the commit.
//!
//! Laid out as Fork lays it out (Finding 5): the filter at the top of the list, one row per
//! file with its status letter and its path — both paths for a rename or a copy — and the
//! file chosen highlighted. The list has focus of its own: pressed, or reached with Tab, ↑ and
//! ↓ move to the previous and next file shown, which is chosen (Fork's model, user decision
//! 6). The list never asks for anything: it reports a file by its index in the change set and
//! the caller decides what that shows.
//!
//! The summary is Fork's (Finding 2): who wrote the commit, its short id, its date and its
//! subject, on one line — no avatar (L9).

use cairn_model::{ChangeSet, CommitDetails, Oid};
use freya::prelude::*;

use crate::accelerators;
use crate::commit_tab::{DETAIL_ROW_HEIGHT, file_row};
use crate::date_text;
use crate::diff_palette::DIFF_FONT_FAMILY;
use crate::file_filter::ShownFiles;

/// The filter field's placeholder.
pub const FILTER_PLACEHOLDER: &str = "Filter";
/// Said in place of the list when the filter leaves no file.
pub const NO_FILE_MATCHES: &str = "No file matches the filter.";
/// Said under the filter while its first answer for a change set is on its way.
pub const FILTERING: &str = "Filtering…";

/// Said under an active filter (the user's decision, 2026-10-03): how many of the change
/// set's `total` files the list shows, so a file hidden by a filter kept from the last commit
/// is not taken for one this commit did not touch.
pub fn filter_count(shown: usize, total: usize) -> String {
    format!(
        "Showing {} of {} file{}",
        crate::diff_notice::grouped(shown as u64),
        crate::diff_notice::grouped(total as u64),
        if total == 1 { "" } else { "s" }
    )
}
/// The summary line's height: the diff bar's, 30 px, so the tab's two strips line up (the
/// user's decision, 2026-10-03).
pub const SUMMARY_HEIGHT: f32 = crate::diff_header::DIFF_HEADER_HEIGHT;

const FONT_SIZE: f32 = 13.0;

/// The changed files of one change set, through a filter. `changes` and `shown` are handles,
/// not copies: a row reads its file by index as it is built.
pub struct ChangesList {
    changes: Readable<ChangeSet>,
    shown: Readable<ShownFiles>,
    filter: Writable<String>,
    current: Option<usize>,
    on_file: EventHandler<usize>,
    key: DiffKey,
}

impl ChangesList {
    pub fn new(
        changes: impl Into<Readable<ChangeSet>>,
        shown: impl Into<Readable<ShownFiles>>,
        filter: impl Into<Writable<String>>,
    ) -> Self {
        Self {
            changes: changes.into(),
            shown: shown.into(),
            filter: filter.into(),
            current: None,
            on_file: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// The file chosen, by its index in the change set: drawn highlighted, and where ↑ and ↓
    /// move from.
    pub fn current(mut self, current: Option<usize>) -> Self {
        self.current = current;
        self
    }

    /// A file was pressed, or reached with ↑ or ↓, by its index in the change set.
    pub fn on_file(mut self, on_file: impl Into<EventHandler<usize>>) -> Self {
        self.on_file = on_file.into();
        self
    }
}

impl PartialEq for ChangesList {
    fn eq(&self, other: &Self) -> bool {
        self.changes == other.changes
            && self.shown == other.shown
            && self.current == other.current
            && self.key == other.key
    }
}

impl std::fmt::Debug for ChangesList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChangesList")
            .field("current", &self.current)
            .finish_non_exhaustive()
    }
}

impl KeyExt for ChangesList {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// What decides which rows are drawn, passed to the virtualising view.
#[derive(Clone)]
struct ListData {
    changes: Readable<ChangeSet>,
    shown: Readable<ShownFiles>,
    rows: usize,
    current: Option<usize>,
    on_file: EventHandler<usize>,
    list_id: AccessibilityId,
}

impl PartialEq for ListData {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows && self.current == other.current && self.list_id == other.list_id
    }
}

impl Component for ChangesList {
    fn render(&self) -> impl IntoElement {
        let list_id = use_a11y();
        let focus = use_focus(list_id);
        let controller = use_scroll_controller(ScrollConfig::default);
        // Reading subscribes the list to the answers it draws. Both counts are lengths, read
        // as they are: nothing here walks the files.
        let (rows, total, waiting, identity) = {
            let changes = self.changes.read();
            let shown = self.shown.read();
            (
                shown.len(changes.files.len()),
                changes.files.len(),
                *shown == ShownFiles::Waiting,
                changes.details.as_ref().map(|details| details.id),
            )
        };
        let data = ListData {
            changes: self.changes.clone(),
            shown: self.shown.clone(),
            rows,
            current: self.current,
            on_file: self.on_file.clone(),
            list_id,
        };
        let colours = get_theme_or_default().read().colors().clone();
        let filtering = !self.filter.read().is_empty();

        let count = filtering.then(|| {
            label()
                .text(if waiting {
                    FILTERING.to_owned()
                } else {
                    filter_count(rows, total)
                })
                .max_lines(1)
                .font_size(12.)
                .color(colours.text_secondary)
        });
        let list: Element = if rows == 0 && filtering && !waiting {
            rect()
                .expanded()
                .center()
                .child(
                    label()
                        .text(NO_FILE_MATCHES)
                        .font_size(FONT_SIZE)
                        .color(colours.text_placeholder),
                )
                .into()
        } else {
            FileRows {
                data,
                controller,
                key: DiffKey::None,
            }
            // Another commit is another list: it opens at its top.
            .key(identity)
            .into()
        };

        rect()
            .expanded()
            .content(Content::Flex)
            .child(
                rect()
                    .width(Size::fill())
                    .padding(Gaps::new(4., 6., 4., 6.))
                    .child(
                        Input::new(self.filter.clone())
                            .placeholder(FILTER_PLACEHOLDER)
                            .compact()
                            .width(Size::fill()),
                    )
                    .maybe_child(count),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .a11y_id(list_id)
                    .a11y_focusable(true)
                    .a11y_role(AccessibilityRole::List)
                    .maybe(focus() == Focus::Keyboard, |el| {
                        el.border(Border::new().fill(colours.border_focus).width(1.))
                    })
                    .on_key_down(keyboard(
                        self.shown.clone(),
                        self.changes.clone(),
                        self.current,
                        self.on_file.clone(),
                        controller,
                    ))
                    .child(list),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// One change set's rows, keyed by its commit.
#[derive(Clone)]
struct FileRows {
    data: ListData,
    controller: ScrollController,
    key: DiffKey,
}

impl PartialEq for FileRows {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data && self.key == other.key
    }
}

impl KeyExt for FileRows {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for FileRows {
    fn render(&self) -> impl IntoElement {
        let rows = self.data.rows;
        VirtualScrollView::new_with_data_controlled(self.data.clone(), build_row, self.controller)
            .length(rows)
            .item_size(DETAIL_ROW_HEIGHT)
            // The arrows move the current file, not the viewport.
            .scroll_with_arrows(false)
            .expanded()
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// ↑ and ↓ over the files shown; every other key, and every chord, is left unhandled. Nothing
/// current yet: either arrow starts at the first file shown.
fn keyboard(
    shown: Readable<ShownFiles>,
    changes: Readable<ChangeSet>,
    current: Option<usize>,
    on_file: EventHandler<usize>,
    mut controller: ScrollController,
) -> impl FnMut(Event<KeyboardEventData>) + 'static {
    move |e: Event<KeyboardEventData>| {
        if accelerators::is_chord(&e) {
            return;
        }
        let shown = shown.peek();
        let rows = shown.len(changes.peek().files.len());
        let Some(last) = rows.checked_sub(1) else {
            return;
        };
        let at = current.and_then(|index| shown.row_of(index));
        let row = match e.key {
            Key::Named(NamedKey::ArrowDown) => at.map_or(0, |row| (row + 1).min(last)),
            Key::Named(NamedKey::ArrowUp) => at.map_or(0, |row| row.saturating_sub(1)),
            _ => return,
        };
        e.stop_propagation();
        let Some(index) = shown.file_at(row) else {
            return;
        };
        on_file.call(index);
        controller.scroll_to_offset(
            row as f32 * DETAIL_ROW_HEIGHT,
            DETAIL_ROW_HEIGHT,
            Direction::Vertical,
        );
    }
}

fn build_row(item: VirtualItem, data: &ListData) -> Element {
    let row = rect()
        .key(item.index)
        .min_width(Size::fill())
        .height(Size::px(item.size))
        .main_align(Alignment::Center)
        .padding(Gaps::new(0., 8., 0., 8.));
    // Read, not peeked: the list redraws when the answer it shows is replaced.
    let Some(index) = data.shown.read().file_at(item.index) else {
        return row.into();
    };
    let Some(drawn) = data.changes.read().files.get(index).map(file_row) else {
        // The count and the files can disagree for one frame.
        return row.into();
    };
    let (list_id, on_file) = (data.list_id, data.on_file.clone());
    let highlight = get_theme_or_default().read().colors().surface_secondary;
    row.maybe(data.current == Some(index), |el| el.background(highlight))
        .on_press(move |_| {
            list_id.request_focus();
            on_file.call(index);
        })
        .child(drawn)
        .into()
}

/// The Changes tab's one line about the commit: author, short id, date and subject (Fork,
/// Finding 2), each cut with an ellipsis rather than wrapped.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangesSummary {
    details: CommitDetails,
}

impl ChangesSummary {
    pub fn new(details: CommitDetails) -> Self {
        Self { details }
    }
}

/// The summary's four parts, in order: author, short id, date, subject.
pub fn summary_parts(details: &CommitDetails) -> [String; 4] {
    [
        details.author.name.clone(),
        short(details.id),
        date_text::long_date(details.author.time),
        details.subject().to_owned(),
    ]
}

fn short(id: Oid) -> String {
    id.short().as_str().to_owned()
}

impl Component for ChangesSummary {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        let [author, id, date, subject] = summary_parts(&self.details);
        let part = |text: String, colour: Color| {
            label()
                .text(text)
                .max_lines(1)
                .font_size(FONT_SIZE)
                .color(colour)
        };
        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(SUMMARY_HEIGHT))
            .cross_align(Alignment::Center)
            .padding(Gaps::new(0., 12., 0., 12.))
            .spacing(12.)
            .child(part(author, colours.text_primary))
            .child(part(id, colours.text_secondary).font_family(DIFF_FONT_FAMILY))
            .child(part(date, colours.text_secondary))
            .child(
                part(subject, colours.text_primary)
                    .width(Size::flex(1.))
                    .text_overflow(TextOverflow::Ellipsis),
            )
    }
}
