//! The Changes tab's left side: a filter field over the changed files, and the files it
//! leaves as one virtualised list; and the tab's one-line summary of the commit.
//!
//! Laid out as Fork lays it out: the filter at the top of the list, one row per file with its
//! status letter and its path — both paths for a rename or a copy — and the file chosen
//! highlighted. The list has focus of its own: pressed, or reached with Tab, ↑ and ↓ move to
//! the previous and next file shown, which is chosen, as Fork's list does. The list never
//! asks for anything: it reports a file by its index in the change set and the caller
//! decides what that shows.
//!
//! The summary is Fork's (`docs/research/ui-review/fork-window-survey-2026-10-16.md` §6,
//! `fork-window-survey-2-2026-10-16.md` §1b): the author's avatar and name, the commit's id at
//! nine characters and its date in the secondary tone, and its subject in bold, on one line.

use cairn_model::{ChangeSet, CommitDetails, CommitSummary, Oid};
use freya::prelude::*;

use crate::accelerators::{self, Scope};
use crate::avatar::{AvatarSize, avatar_of};
use crate::detail::commit_tab::{DETAIL_ROW_HEIGHT, file_row};
use crate::detail::file_filter::ShownFiles;
use crate::detail::file_menu::{FileChoice, FileMenuIn, file_menu, open_file_menu};
use crate::diff::header::DIFF_HEADER_HEIGHT;
use crate::diff::palette::DIFF_FONT_FAMILY;
use crate::history::date_text;
use crate::local_time::LocalZone;
use crate::push_button::{ButtonFace, ButtonGlyph, ButtonSize, PushButton};
use crate::style::{focus, icons, metrics, tones};
use crate::text_field::text_field;

/// The filter field's placeholder.
pub const FILTER_PLACEHOLDER: &str = "Filter";
/// Said in place of the list when the filter leaves no file.
pub const NO_FILE_MATCHES: &str = "No file matches the filter.";
/// Said under the filter while its first answer for a change set is on its way.
pub const FILTERING: &str = "Filtering…";

/// Said under an active filter: how many of the change set's `total` files the list shows,
/// so a file hidden by a filter kept from the last commit is not taken for one this commit
/// did not touch.
pub fn filter_count(shown: usize, total: usize) -> String {
    format!(
        "Showing {} of {} file{}",
        crate::diff::notice::grouped(shown as u64),
        crate::diff::notice::grouped(total as u64),
        if total == 1 { "" } else { "s" }
    )
}
/// The summary line's height: Fork's 23 px and the rule under it
/// (`style::metrics::DETAIL_SUMMARY`).
pub const SUMMARY_HEIGHT: f32 = metrics::DETAIL_SUMMARY;
/// How many characters of the commit's id the summary draws: Fork's nine ("f93015d03", uw5).
pub const SUMMARY_ID_LENGTH: usize = 9;

/// The changed files of one change set, through a filter. `changes` and `shown` are handles,
/// not copies: a row reads its file by index as it is built.
pub struct ChangesList {
    changes: Readable<ChangeSet>,
    shown: Readable<ShownFiles>,
    filter: Writable<String>,
    current: Option<usize>,
    on_file: EventHandler<usize>,
    on_file_menu: EventHandler<(usize, FileChoice)>,
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
            on_file_menu: EventHandler::new(|_| {}),
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

    /// A file's menu item was chosen — from the menu a right press opens on it, which first
    /// makes it the current file, or by the item's copy chord with the list focused on the
    /// current file — by the file's index in the change set.
    pub fn on_file_menu(
        mut self,
        on_file_menu: impl Into<EventHandler<(usize, FileChoice)>>,
    ) -> Self {
        self.on_file_menu = on_file_menu.into();
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
    on_file_menu: EventHandler<(usize, FileChoice)>,
    list_id: AccessibilityId,
    /// Whether the list has focus, which the chosen file's tone says.
    focused: bool,
}

impl PartialEq for ListData {
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
            && self.current == other.current
            && self.list_id == other.list_id
            && self.focused == other.focused
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
            on_file_menu: self.on_file_menu.clone(),
            list_id,
            // Read from the toolkit's focus, so the rows are drawn again as it moves.
            focused: focus().is_focused(),
        };
        let filtering = !self.filter.read().is_empty();

        let count = filtering.then(|| {
            label()
                .text(if waiting {
                    FILTERING.to_owned()
                } else {
                    filter_count(rows, total)
                })
                .max_lines(1)
                .font_size(metrics::BODY)
                .color(tones::DETAIL_LABEL)
        });
        let list: Element = if rows == 0 && filtering && !waiting {
            rect()
                .expanded()
                .center()
                .child(
                    label()
                        .text(NO_FILE_MATCHES)
                        .font_size(metrics::BODY)
                        .color(tones::PLACEHOLDER),
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
            // Fork's bar over the files, level with the diff's: a magnifier and the field, on
            // the detail pane's tone with the rule under it (uw5).
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(DIFF_HEADER_HEIGHT))
                    .background(tones::DETAIL)
                    .border(
                        Border::new()
                            .fill(tones::RULE)
                            .width(BorderWidth {
                                top: 0.,
                                right: 0.,
                                bottom: 1.,
                                left: 0.,
                            })
                            .alignment(BorderAlignment::Inner),
                    )
                    .child(
                        text_field(self.filter.clone())
                            .placeholder(FILTER_PLACEHOLDER)
                            .flat()
                            .background(tones::DETAIL)
                            .hover_background(tones::DETAIL)
                            .focus_background(tones::DETAIL)
                            .leading(icons::magnifier(
                                metrics::DIFF_BAR_ICON,
                                tones::DIFF_BAR_ICON,
                            ))
                            .height(Size::px(DIFF_HEADER_HEIGHT - 1.))
                            .theme_layout(InputLayoutThemePartial {
                                inner_margin: Some(Preference::Specific(Gaps::new(0., 6., 0., 6.))),
                                ..Default::default()
                            })
                            .width(Size::fill()),
                    ),
            )
            .maybe_child(count.map(|count| {
                rect()
                    .width(Size::fill())
                    .padding(Gaps::new(2., 8., 2., 8.))
                    .child(count)
            }))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .background(tones::HISTORY)
                    .a11y_id(list_id)
                    .a11y_focusable(true)
                    .a11y_role(AccessibilityRole::List)
                    .border(focus::ring_when(focus()))
                    .on_key_down(keyboard(
                        self.shown.clone(),
                        self.changes.clone(),
                        self.current,
                        (self.on_file.clone(), self.on_file_menu.clone()),
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
        // The rows after the last are room for the horizontal scrollbar (`end_room`); a row
        // past the list's own builds empty.
        let rows = crate::end_room::with_end_room(self.data.rows, DETAIL_ROW_HEIGHT);
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

/// ↑ and ↓ over the files shown, and the copy chords on the current file's menu; every other
/// key, and every chord, is left unhandled. Nothing current yet: either arrow starts at the
/// first file shown.
fn keyboard(
    shown: Readable<ShownFiles>,
    changes: Readable<ChangeSet>,
    current: Option<usize>,
    (on_file, on_file_menu): (EventHandler<usize>, EventHandler<(usize, FileChoice)>),
    mut controller: ScrollController,
) -> impl FnMut(Event<KeyboardEventData>) + 'static {
    move |e: Event<KeyboardEventData>| {
        if let Some(copy) = accelerators::resolve_key(&e, Scope::List) {
            e.stop_propagation();
            if let Some(index) = current
                && let Some(choice) =
                    crate::menu::chosen_by(&file_menu(FileMenuIn::ChangesTab), copy)
            {
                on_file_menu.call((index, choice));
            }
            return;
        }
        if accelerators::is_chord(&e, &[]) {
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
    let current = data.current == Some(index);
    let chosen = tones::selected(data.focused);
    let Some(drawn) = data
        .changes
        .read()
        .files
        .get(index)
        .map(|file| file_row(file, current.then_some(chosen.text)))
    else {
        // The count and the files can disagree for one frame.
        return row.into();
    };
    let (list_id, on_file) = (data.list_id, data.on_file.clone());
    let (choosing, on_file_menu) = (data.on_file.clone(), data.on_file_menu.clone());
    row.maybe(current, |el| el.background(chosen.band))
        .on_press(move |_| {
            list_id.request_focus();
            on_file.call(index);
        })
        // A right press makes the file current, as a press does, and opens its menu.
        .on_pointer_down(move |e: Event<PointerEventData>| {
            if e.button() != Some(MouseButton::Right) {
                return;
            }
            list_id.request_focus();
            if !current {
                choosing.call(index);
            }
            open_file_menu(FileMenuIn::ChangesTab, list_id, index, on_file_menu.clone());
        })
        .child(drawn)
        .into()
}

/// The Changes tab's one line about the commit, as Fork draws it: author, short id, date and
/// subject, each cut with an ellipsis rather than wrapped.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangesSummary {
    details: CommitDetails,
    zone: LocalZone,
}

impl ChangesSummary {
    pub fn new(details: CommitDetails) -> Self {
        Self {
            details,
            zone: LocalZone::default(),
        }
    }

    /// The zone the date is drawn in, handed down by the window.
    pub fn zone(mut self, zone: LocalZone) -> Self {
        self.zone = zone;
        self
    }
}

/// The summary's four parts after its avatar, in order: author, the id's first nine
/// characters, the author's date in `zone` as the history draws it, subject.
pub fn summary_parts(details: &CommitDetails, zone: &LocalZone) -> [String; 4] {
    let hex = details.id.hex();
    [
        details.author.name.clone(),
        hex.as_str()
            .get(..SUMMARY_ID_LENGTH)
            .unwrap_or(hex.as_str())
            .to_owned(),
        date_text::history_date(details.author.time.seconds, zone),
        details.subject().to_owned(),
    ]
}

fn short(id: Oid) -> String {
    id.short().as_str().to_owned()
}

impl Component for ChangesSummary {
    fn render(&self) -> impl IntoElement {
        let [author, id, date, subject] = summary_parts(&self.details, &self.zone);
        let part = |text: String, colour: Color| {
            label()
                .text(text)
                .max_lines(1)
                .font_size(metrics::BODY)
                .color(colour)
        };
        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(SUMMARY_HEIGHT))
            .cross_align(Alignment::Center)
            .padding(Gaps::new(0., 10., 0., 10.))
            .spacing(8.)
            .border(
                Border::new()
                    .fill(tones::RULE)
                    .width(BorderWidth {
                        top: 0.,
                        right: 0.,
                        bottom: 1.,
                        left: 0.,
                    })
                    .alignment(BorderAlignment::Inner),
            )
            .child(avatar_of(&author, AvatarSize::Summary))
            .child(part(author, tones::TEXT))
            .child(part(id, tones::DETAIL_LABEL))
            .child(part(date, tones::DETAIL_LABEL))
            .child(
                part(subject, tones::TEXT)
                    .font_weight(FontWeight::BOLD)
                    .width(Size::flex(1.))
                    .text_overflow(TextOverflow::Ellipsis),
            )
    }
}

/// Said before a comparison's base, the lower of the two rows: the commit the diff runs
/// from, its lines the removed ones.
pub const BASE_CAPTION: &str = "Base";
/// Said before a comparison's tip: the commit the diff runs to, its lines the added ones.
pub const TIP_CAPTION: &str = "Tip";
/// The swap control's name, which assistive technology reads and its tooltip says.
pub const SWAP_LABEL: &str = "Swap base and tip";

/// A commit as a comparison's header names it: its short id, its author and its subject.
pub fn comparison_parts(commit: &CommitSummary) -> [String; 3] {
    [
        short(commit.id),
        commit.author_name.clone(),
        commit.summary.clone(),
    ]
}

/// The Changes tab's header over a comparison of two commits: both commits named, one per
/// line — the base, then the tip — with Fork's swap control at its right.
pub struct ComparisonHeader {
    base: CommitSummary,
    tip: CommitSummary,
    on_swap: EventHandler<()>,
    key: DiffKey,
}

impl ComparisonHeader {
    pub fn new(base: CommitSummary, tip: CommitSummary) -> Self {
        Self {
            base,
            tip,
            on_swap: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// The swap control was pressed: what it reverses is the caller's.
    pub fn on_swap(mut self, on_swap: impl Into<EventHandler<()>>) -> Self {
        self.on_swap = on_swap.into();
        self
    }
}

impl PartialEq for ComparisonHeader {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.tip == other.tip && self.key == other.key
    }
}

impl std::fmt::Debug for ComparisonHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComparisonHeader")
            .field("base", &self.base.id)
            .field("tip", &self.tip.id)
            .finish_non_exhaustive()
    }
}

impl KeyExt for ComparisonHeader {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for ComparisonHeader {
    fn render(&self) -> impl IntoElement {
        let part = |text: String, colour: Color| {
            label()
                .text(text)
                .max_lines(1)
                .font_size(metrics::BODY)
                .color(colour)
        };
        let line = |caption: &'static str, commit: &CommitSummary| -> Element {
            let [id, author, subject] = comparison_parts(commit);
            rect()
                .horizontal()
                .content(Content::Flex)
                .width(Size::fill())
                .height(Size::px(SUMMARY_HEIGHT))
                .cross_align(Alignment::Center)
                .spacing(12.)
                .child(part(caption.to_owned(), tones::PLACEHOLDER).width(Size::px(32.)))
                .child(part(id, tones::DETAIL_LABEL).font_family(DIFF_FONT_FAMILY))
                .child(part(author, tones::TEXT))
                .child(
                    part(subject, tones::TEXT)
                        .width(Size::flex(1.))
                        .text_overflow(TextOverflow::Ellipsis),
                )
                .into()
        };
        let on_swap = self.on_swap.clone();
        rect()
            .horizontal()
            .content(Content::Flex)
            .width(Size::fill())
            .height(Size::px(2. * SUMMARY_HEIGHT))
            .cross_align(Alignment::Center)
            .padding(Gaps::new(0., 12., 0., 12.))
            .spacing(8.)
            .child(
                rect()
                    .width(Size::flex(1.))
                    .child(line(BASE_CAPTION, &self.base))
                    .child(line(TIP_CAPTION, &self.tip)),
            )
            .child(
                TooltipContainer::new(Tooltip::new_text(SWAP_LABEL)).child(
                    PushButton::new(
                        ButtonFace::Glyph {
                            glyph: ButtonGlyph::Swap,
                            name: SWAP_LABEL.to_owned(),
                        },
                        ButtonSize::Compact,
                    )
                    .on_press(move || on_swap.call(())),
                ),
            )
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}
