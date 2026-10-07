//! Local Changes' left side (refs-and-status R9.1, R9.5): a filter field, then Unstaged above
//! Staged, each headed by its name and each one flat, virtualised list of paths with Fork's
//! badges (`fork-refs-and-status-ui.md` section 6). The diff of the path chosen is the caller's,
//! on the right.
//!
//! **A row.** Its badge — the change's letter (`M`, `A` for an added or untracked path, `D`,
//! `R`, `C`, `T`, `S` for a submodule) or, for a conflicted path, Fork's warning triangle — then
//! its path, a rename's or a copy's source before it. The badge is coloured as the Commit tab's
//! letters are, and no two kinds share a shape, so none is told by colour alone.
//!
//! **What it asks.** Nothing: a row pressed, or reached with ↑ or ↓ in its focused list, is
//! reported by its list and its place there, and the caller decides what that shows. Each list
//! reads its rows by index, through the filter's answer (`ShownFiles`), as they are built; it
//! walks no path. Nothing here stages, unstages or discards (R9.6).

use cairn_model::{ChangeKind, ChangeList, LocalChange, LocalChanges};
use freya::prelude::*;

use crate::accelerators;
use crate::changes_list::{FILTER_PLACEHOLDER, FILTERING, filter_count};
use crate::commit_tab::DETAIL_ROW_HEIGHT;
use crate::diff_palette::DIFF_FONT_FAMILY;
use crate::file_filter::ShownFiles;
use crate::ref_glyphs::RefGlyph;

/// The heading over the upper list.
pub const UNSTAGED_CAPTION: &str = "Unstaged";
/// The heading over the lower list.
pub const STAGED_CAPTION: &str = "Staged";
/// Said in place of both lists when the filter leaves no path.
pub const NO_PATH_MATCHES: &str = "No path matches the filter.";

/// A heading's height.
pub const LIST_HEADER_HEIGHT: f32 = 26.0;

const FONT_SIZE: f32 = 13.0;
const BADGE_WIDTH: f32 = 18.0;

/// The heading over `list`.
pub fn list_caption(list: ChangeList) -> &'static str {
    match list {
        ChangeList::Unstaged => UNSTAGED_CAPTION,
        ChangeList::Staged => STAGED_CAPTION,
    }
}

/// The letter a change's badge carries; `None` for a conflicted path, whose badge is Fork's
/// warning triangle, painted.
pub fn badge_letter(kind: ChangeKind) -> Option<&'static str> {
    match kind {
        ChangeKind::Modified => Some("M"),
        ChangeKind::Added => Some("A"),
        ChangeKind::Deleted => Some("D"),
        ChangeKind::Renamed => Some("R"),
        ChangeKind::Copied => Some("C"),
        ChangeKind::TypeChanged => Some("T"),
        ChangeKind::Submodule => Some("S"),
        ChangeKind::Conflicted => None,
    }
}

/// A row's text: its path, or its source and its path for a rename or a copy, as the Commit
/// tab draws them.
pub fn change_text(change: &LocalChange<'_>) -> String {
    match change.from {
        Some(from) => format!("{} → {}", from.display(), change.path.display()),
        None => change.path.display().into_owned(),
    }
}

/// The two lists, through the filter. `changes`, `unstaged` and `staged` are handles: a row
/// reads its path by index as it is built.
pub struct LocalChangesList {
    changes: Readable<LocalChanges>,
    unstaged: Readable<ShownFiles>,
    staged: Readable<ShownFiles>,
    filter: Writable<String>,
    chosen: Option<(ChangeList, usize)>,
    on_choose: EventHandler<(ChangeList, usize)>,
    key: DiffKey,
}

impl LocalChangesList {
    pub fn new(
        changes: impl Into<Readable<LocalChanges>>,
        unstaged: impl Into<Readable<ShownFiles>>,
        staged: impl Into<Readable<ShownFiles>>,
        filter: impl Into<Writable<String>>,
    ) -> Self {
        Self {
            changes: changes.into(),
            unstaged: unstaged.into(),
            staged: staged.into(),
            filter: filter.into(),
            chosen: None,
            on_choose: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// The path chosen, by its list and its row there: drawn highlighted, and where ↑ and ↓
    /// move from in its list.
    pub fn chosen(mut self, chosen: Option<(ChangeList, usize)>) -> Self {
        self.chosen = chosen;
        self
    }

    /// A row was pressed, or reached with ↑ or ↓, by its list and its row there.
    pub fn on_choose(mut self, on_choose: impl Into<EventHandler<(ChangeList, usize)>>) -> Self {
        self.on_choose = on_choose.into();
        self
    }
}

impl PartialEq for LocalChangesList {
    fn eq(&self, other: &Self) -> bool {
        self.changes == other.changes
            && self.unstaged == other.unstaged
            && self.staged == other.staged
            && self.chosen == other.chosen
            && self.key == other.key
    }
}

impl std::fmt::Debug for LocalChangesList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalChangesList")
            .field("chosen", &self.chosen)
            .finish_non_exhaustive()
    }
}

impl KeyExt for LocalChangesList {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl Component for LocalChangesList {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        // Lengths only: nothing here walks the paths.
        let (unstaged, staged, total, waiting) = {
            let changes = self.changes.read();
            let (unstaged, staged) = (self.unstaged.read(), self.staged.read());
            (
                unstaged.len(changes.len(ChangeList::Unstaged)),
                staged.len(changes.len(ChangeList::Staged)),
                changes.len(ChangeList::Unstaged) + changes.len(ChangeList::Staged),
                *unstaged == ShownFiles::Waiting || *staged == ShownFiles::Waiting,
            )
        };
        let filtering = !self.filter.read().is_empty();
        let count = filtering.then(|| {
            label()
                .text(if waiting {
                    FILTERING.to_owned()
                } else {
                    filter_count(unstaged + staged, total)
                })
                .max_lines(1)
                .font_size(12.)
                .color(colours.text_secondary)
        });
        let section = |list: ChangeList, shown: &Readable<ShownFiles>, rows: usize| ListSection {
            list,
            changes: self.changes.clone(),
            shown: shown.clone(),
            rows,
            chosen: self
                .chosen
                .and_then(|(chosen, row)| (chosen == list).then_some(row)),
            on_choose: self.on_choose.clone(),
        };
        let lists: Element = if filtering && !waiting && unstaged + staged == 0 {
            rect()
                .width(Size::fill())
                .height(Size::flex(1.))
                .center()
                .child(
                    label()
                        .text(NO_PATH_MATCHES)
                        .font_size(FONT_SIZE)
                        .color(colours.text_placeholder),
                )
                .into()
        } else {
            rect()
                .width(Size::fill())
                .height(Size::flex(1.))
                .content(Content::Flex)
                .child(section(ChangeList::Unstaged, &self.unstaged, unstaged))
                .child(section(ChangeList::Staged, &self.staged, staged))
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
            .child(lists)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// One list under its heading: half the side, its rows virtualised.
#[derive(Clone)]
struct ListSection {
    list: ChangeList,
    changes: Readable<LocalChanges>,
    shown: Readable<ShownFiles>,
    rows: usize,
    chosen: Option<usize>,
    on_choose: EventHandler<(ChangeList, usize)>,
}

impl PartialEq for ListSection {
    fn eq(&self, other: &Self) -> bool {
        self.list == other.list
            && self.changes == other.changes
            && self.shown == other.shown
            && self.rows == other.rows
            && self.chosen == other.chosen
    }
}

/// What decides which rows are drawn, passed to the virtualising view.
#[derive(Clone)]
struct ListData {
    list: ChangeList,
    changes: Readable<LocalChanges>,
    shown: Readable<ShownFiles>,
    rows: usize,
    chosen: Option<usize>,
    on_choose: EventHandler<(ChangeList, usize)>,
    list_id: AccessibilityId,
}

impl PartialEq for ListData {
    fn eq(&self, other: &Self) -> bool {
        self.list == other.list
            && self.rows == other.rows
            && self.chosen == other.chosen
            && self.list_id == other.list_id
    }
}

impl Component for ListSection {
    fn render(&self) -> impl IntoElement {
        let list_id = use_a11y();
        let focus = use_focus(list_id);
        let controller = use_scroll_controller(ScrollConfig::default);
        let colours = get_theme_or_default().read().colors().clone();
        let data = ListData {
            list: self.list,
            changes: self.changes.clone(),
            shown: self.shown.clone(),
            rows: self.rows,
            chosen: self.chosen,
            on_choose: self.on_choose.clone(),
            list_id,
        };
        rect()
            .width(Size::fill())
            .height(Size::flex(1.))
            .content(Content::Flex)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::px(LIST_HEADER_HEIGHT))
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new(0., 8., 0., 8.))
                    .background(colours.surface_tertiary)
                    .child(
                        label()
                            .text(list_caption(self.list))
                            .max_lines(1)
                            .font_size(12.)
                            .font_weight(FontWeight::BOLD)
                            .color(colours.text_secondary),
                    ),
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
                    .on_key_down(keyboard(&data, controller))
                    .child(
                        VirtualScrollView::new_with_data_controlled(data, build_row, controller)
                            .length(self.rows)
                            .item_size(DETAIL_ROW_HEIGHT)
                            // The arrows move the path chosen, not the viewport.
                            .scroll_with_arrows(false)
                            .expanded(),
                    ),
            )
    }

    /// The two lists are siblings of one type: each is told apart by its list, so neither ever
    /// takes the other's focus or scroll.
    fn render_key(&self) -> DiffKey {
        DiffKey::U64(match self.list {
            ChangeList::Unstaged => 0,
            ChangeList::Staged => 1,
        })
    }
}

/// ↑ and ↓ over the rows `data`'s list shows; every other key, and every chord, is left
/// unhandled. Nothing chosen in the list yet: either arrow starts at its first row.
fn keyboard(
    data: &ListData,
    mut controller: ScrollController,
) -> impl FnMut(Event<KeyboardEventData>) + 'static {
    let (list, shown, changes, chosen, on_choose) = (
        data.list,
        data.shown.clone(),
        data.changes.clone(),
        data.chosen,
        data.on_choose.clone(),
    );
    move |e: Event<KeyboardEventData>| {
        if accelerators::is_chord(&e) {
            return;
        }
        let shown = shown.peek();
        let rows = shown.len(changes.peek().len(list));
        let Some(last) = rows.checked_sub(1) else {
            return;
        };
        let at = chosen.and_then(|row| shown.row_of(row));
        let drawn = match e.key {
            Key::Named(NamedKey::ArrowDown) => at.map_or(0, |at| (at + 1).min(last)),
            Key::Named(NamedKey::ArrowUp) => at.map_or(0, |at| at.saturating_sub(1)),
            _ => return,
        };
        e.stop_propagation();
        let Some(row) = shown.file_at(drawn) else {
            return;
        };
        on_choose.call((list, row));
        controller.scroll_to_offset(
            drawn as f32 * DETAIL_ROW_HEIGHT,
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
    let Some(at) = data.shown.read().file_at(item.index) else {
        return row.into();
    };
    let Some(drawn) = data
        .changes
        .read()
        .get(data.list, at)
        .map(|change| change_row(&change))
    else {
        // The count and the rows can disagree for one frame.
        return row.into();
    };
    let (list, list_id, on_choose) = (data.list, data.list_id, data.on_choose.clone());
    let highlight = get_theme_or_default().read().colors().surface_secondary;
    row.maybe(data.chosen == Some(at), |el| el.background(highlight))
        .on_press(move |_| {
            list_id.request_focus();
            on_choose.call((list, at));
        })
        .child(drawn)
        .into()
}

/// One path's row: its badge and its text.
fn change_row(change: &LocalChange<'_>) -> Element {
    let colours = get_theme_or_default().read().colors().clone();
    let colour = match change.kind {
        ChangeKind::Added => colours.success,
        ChangeKind::Deleted => colours.error,
        ChangeKind::Modified | ChangeKind::TypeChanged | ChangeKind::Conflicted => colours.warning,
        ChangeKind::Renamed | ChangeKind::Copied | ChangeKind::Submodule => colours.info,
    };
    let badge: Element = match badge_letter(change.kind) {
        Some(letter) => label()
            .text(letter)
            .font_size(FONT_SIZE)
            .font_weight(FontWeight::BOLD)
            .color(colour)
            .into(),
        // A canvas repaints only when its layout changes: keyed by what it draws.
        None => RefGlyph::Gone.draw(colour).key("conflicted").into(),
    };
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(10.)
        .child(
            rect()
                .width(Size::px(BADGE_WIDTH))
                .cross_align(Alignment::Center)
                .child(badge),
        )
        .child(
            label()
                .text(change_text(change))
                .max_lines(1)
                .font_size(FONT_SIZE)
                .color(colours.text_primary)
                .font_family(DIFF_FONT_FAMILY),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R9.1: each kind of change has a badge no other kind has — a letter, or the conflict's
    /// triangle — so none is told by colour alone. Caught by: two kinds given one letter, or a
    /// conflict drawn as a letter another kind carries.
    #[test]
    fn each_kind_of_change_has_a_badge_of_its_own() {
        let kinds = [
            ChangeKind::Modified,
            ChangeKind::Added,
            ChangeKind::Deleted,
            ChangeKind::Renamed,
            ChangeKind::Copied,
            ChangeKind::TypeChanged,
            ChangeKind::Submodule,
            ChangeKind::Conflicted,
        ];
        let badges: Vec<Option<&str>> = kinds.iter().map(|kind| badge_letter(*kind)).collect();
        assert_eq!(
            badges,
            [
                Some("M"),
                Some("A"),
                Some("D"),
                Some("R"),
                Some("C"),
                Some("T"),
                Some("S"),
                None
            ]
        );
        let mut letters: Vec<&str> = badges.iter().flatten().copied().collect();
        letters.sort_unstable();
        letters.dedup();
        assert_eq!(letters.len(), kinds.len() - 1, "two kinds share a letter");
    }
}
