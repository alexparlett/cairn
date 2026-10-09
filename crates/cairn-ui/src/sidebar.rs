//! The sidebar, Fork's (refs-and-status R8; `fork-refs-and-status-ui.md`, section 5): Local
//! Changes with its count and All Commits, which choose what the main region shows (R8.7); a
//! filter box (R8.3); and one virtualized list of the refs — Branches, Remotes, Tags, Stashes,
//! each a section that opens and closes, branches and remote-tracking refs in folders split at
//! `/` (R8.1, R8.2, R8.4). The rows are laid out on a worker (`RefsSnapshot::sidebar_rows`); the
//! list reads each one's name from the snapshot they index as it builds that row alone, so a
//! snapshot of tens of thousands of refs builds one viewport.
//!
//! A branch draws Fork's marks: the current branch a check mark in place of its glyph and its
//! name bold; a branch whose upstream is gone the warning triangle Fork draws in place of its
//! glyph — the current branch's at its right, beside the check mark, as Fork marks an active
//! branch with an invalid upstream; any other branch with an upstream its behind and ahead
//! counts, as the title bar prints them (`counts_text`). A line under the filter says what a
//! press is doing — "Finding <ref>…" — or why it found nothing.

use std::sync::Arc;

use cairn_model::{
    AheadBehind, HeadState, Oid, RefKind, RefName, RefsSnapshot, SidebarRow, SidebarSection,
    Upstream, folder_name,
};
use freya::prelude::*;

use crate::diff_palette::DIFF_FONT_FAMILY;
use crate::ref_glyphs::{GLYPH_SIZE, RefGlyph};
use crate::status_box::counts_text;
use crate::text_field::text_field;

/// A row's height in the sidebar's list.
pub const SIDEBAR_ROW_HEIGHT: f32 = 22.0;
/// How far each folder level indents what it holds.
pub const SIDEBAR_INDENT: f32 = 14.0;
/// The filter box's placeholder.
pub const SIDEBAR_FILTER_PLACEHOLDER: &str = "Filter";
/// The entry that shows the working tree's changes in the main region.
pub const LOCAL_CHANGES_CAPTION: &str = "Local Changes";
/// The entry that shows the history in the main region.
pub const ALL_COMMITS_CAPTION: &str = "All Commits";
/// What a detached `HEAD`'s row is called, as Fork names it in Branches.
pub const DETACHED_HEAD_CAPTION: &str = "HEAD";
const FONT_SIZE: f32 = 13.0;

/// What the main region shows (R8.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainView {
    LocalChanges,
    #[default]
    AllCommits,
}

/// A section's caption.
pub fn section_caption(section: SidebarSection) -> &'static str {
    match section {
        SidebarSection::Branches => "Branches",
        SidebarSection::Remotes => "Remotes",
        SidebarSection::Tags => "Tags",
        SidebarSection::Stashes => "Stashes",
    }
}

/// `Local Changes (N)`, the count left out when there is none (R9.2).
pub fn local_changes_text(count: Option<usize>) -> String {
    match count {
        Some(count) if count > 0 => format!("{LOCAL_CHANGES_CAPTION} ({count})"),
        Some(_) | None => LOCAL_CHANGES_CAPTION.to_owned(),
    }
}

/// The entry last pressed, drawn chosen: a ref by its name, a stash by its place in the list
/// and its commit — identities that survive a snapshot laid out again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarTarget {
    Ref(RefName),
    DetachedHead,
    Stash { index: usize, commit: Oid },
}

impl SidebarTarget {
    /// What `row` of `refs` is, if it is an entry a press chooses rather than opens.
    pub fn of(row: SidebarRow, refs: &RefsSnapshot) -> Option<Self> {
        match row {
            SidebarRow::Ref { index, .. } => refs
                .refs
                .get(index as usize)
                .map(|listed| Self::Ref(listed.name.clone())),
            SidebarRow::DetachedHead => Some(Self::DetachedHead),
            SidebarRow::Stash { index } => {
                refs.stashes.get(index as usize).map(|stash| Self::Stash {
                    index: stash.index,
                    commit: stash.commit,
                })
            }
            SidebarRow::Section { .. } | SidebarRow::Folder { .. } => None,
        }
    }
}

/// The rows a worker laid out and the snapshot they index, kept together: a row names its ref
/// by its place in that snapshot and no other.
#[derive(Debug, Clone)]
pub struct SidebarRefs {
    pub refs: Arc<RefsSnapshot>,
    pub rows: Arc<Vec<SidebarRow>>,
}

impl PartialEq for SidebarRefs {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.refs, &other.refs) && Arc::ptr_eq(&self.rows, &other.rows)
    }
}

/// Each local branch's distance from its upstream, in the snapshot's order, by name.
pub type BranchCounts = Arc<Vec<(RefName, AheadBehind)>>;

/// `branch`'s counts among `counts`: a binary search, since they come in the snapshot's order.
fn counts_of(counts: &[(RefName, AheadBehind)], branch: &RefName) -> Option<AheadBehind> {
    counts
        .binary_search_by(|(name, _)| name.as_str().as_bytes().cmp(branch.as_str().as_bytes()))
        .ok()
        .and_then(|at| counts.get(at))
        .map(|(_, counts)| *counts)
}

pub struct Sidebar {
    shown: Option<SidebarRefs>,
    counts: Option<BranchCounts>,
    local_changes: Option<usize>,
    main: MainView,
    chosen: Option<SidebarTarget>,
    filter: Writable<String>,
    notice: Option<String>,
    on_row: EventHandler<SidebarRow>,
    on_main: EventHandler<MainView>,
    key: DiffKey,
}

impl Sidebar {
    /// The sidebar over `shown` — nothing listed until the first rows are laid out — with the
    /// filter's text in `filter`, which the caller asks the rows again for as it changes.
    pub fn new(shown: Option<SidebarRefs>, filter: impl Into<Writable<String>>) -> Self {
        Self {
            shown,
            counts: None,
            local_changes: None,
            main: MainView::AllCommits,
            chosen: None,
            filter: filter.into(),
            notice: None,
            on_row: EventHandler::new(|_| {}),
            on_main: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// Each branch's counts against its upstream, as the last refresh counted them.
    pub fn counts(mut self, counts: Option<BranchCounts>) -> Self {
        self.counts = counts;
        self
    }

    /// How many paths the last status listed; `None` before one was read.
    pub fn local_changes(mut self, count: Option<usize>) -> Self {
        self.local_changes = count;
        self
    }

    pub fn main(mut self, main: MainView) -> Self {
        self.main = main;
        self
    }

    pub fn chosen(mut self, chosen: Option<SidebarTarget>) -> Self {
        self.chosen = chosen;
        self
    }

    /// What a press is doing, or why it found nothing, under the filter.
    pub fn notice(mut self, notice: Option<String>) -> Self {
        self.notice = notice;
        self
    }

    /// A row pressed: a section or folder to open or close, a ref or stash to find.
    pub fn on_row(mut self, on_row: impl Into<EventHandler<SidebarRow>>) -> Self {
        self.on_row = on_row.into();
        self
    }

    pub fn on_main(mut self, on_main: impl Into<EventHandler<MainView>>) -> Self {
        self.on_main = on_main.into();
        self
    }
}

// Hand-written: `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for Sidebar {
    fn eq(&self, other: &Self) -> bool {
        self.shown == other.shown
            && same_counts(&self.counts, &other.counts)
            && self.local_changes == other.local_changes
            && self.main == other.main
            && self.chosen == other.chosen
            && self.filter == other.filter
            && self.notice == other.notice
            && self.key == other.key
    }
}

fn same_counts(one: &Option<BranchCounts>, other: &Option<BranchCounts>) -> bool {
    match (one, other) {
        (Some(one), Some(other)) => Arc::ptr_eq(one, other),
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

impl std::fmt::Debug for Sidebar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sidebar")
            .field("main", &self.main)
            .field("chosen", &self.chosen)
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}

impl KeyExt for Sidebar {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

/// What the list's builder reads, compared so the list is rebuilt only when it changes.
#[derive(Clone)]
struct ListData {
    shown: SidebarRefs,
    counts: Option<BranchCounts>,
    chosen: Option<SidebarTarget>,
    on_row: EventHandler<SidebarRow>,
}

impl PartialEq for ListData {
    fn eq(&self, other: &Self) -> bool {
        self.shown == other.shown
            && same_counts(&self.counts, &other.counts)
            && self.chosen == other.chosen
    }
}

impl Component for Sidebar {
    fn render(&self) -> impl IntoElement {
        let colours = get_theme_or_default().read().colors().clone();
        let entry = |text: String, view: MainView, on_main: EventHandler<MainView>| {
            rect()
                .width(Size::fill())
                .height(Size::px(SIDEBAR_ROW_HEIGHT))
                .main_align(Alignment::Center)
                .padding(Gaps::new(0., 8., 0., 8.))
                .maybe(self.main == view, |el| {
                    el.background(colours.surface_secondary)
                })
                .on_press(move |_| on_main.call(view))
                .child(
                    label()
                        .text(text)
                        .max_lines(1)
                        .font_size(FONT_SIZE)
                        .color(colours.text_primary),
                )
        };
        let list: Element = match &self.shown {
            Some(shown) => {
                let length = shown.rows.len();
                VirtualScrollView::new_with_data(
                    ListData {
                        shown: shown.clone(),
                        counts: self.counts.clone(),
                        chosen: self.chosen.clone(),
                        on_row: self.on_row.clone(),
                    },
                    build_row,
                )
                .length(length)
                .item_size(SIDEBAR_ROW_HEIGHT)
                .expanded()
                .into()
            }
            None => rect().expanded().into(),
        };
        rect()
            .expanded()
            .padding(Gaps::new(6., 0., 0., 0.))
            .child(entry(
                local_changes_text(self.local_changes),
                MainView::LocalChanges,
                self.on_main.clone(),
            ))
            .child(entry(
                ALL_COMMITS_CAPTION.to_owned(),
                MainView::AllCommits,
                self.on_main.clone(),
            ))
            .child(
                rect().width(Size::fill()).padding(6.).child(
                    text_field(self.filter.clone())
                        .placeholder(SIDEBAR_FILTER_PLACEHOLDER)
                        .compact()
                        .width(Size::fill()),
                ),
            )
            .maybe_child(self.notice.clone().map(|notice| {
                rect()
                    .width(Size::fill())
                    .padding(Gaps::new(0., 8., 4., 8.))
                    .child(
                        label()
                            .text(notice)
                            .font_size(12.)
                            .color(colours.text_secondary),
                    )
            }))
            .child(list)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// What one row draws, read out of the snapshot for that row alone.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnRow {
    pub glyph: RefGlyph,
    /// Whether it is a section or folder, open (`Some(true)`) or closed: drawn with the
    /// triangle that says so.
    pub disclosure: Option<bool>,
    pub text: String,
    pub depth: u16,
    /// The current branch: its name drawn bold.
    pub bold: bool,
    /// A branch's counts against its upstream, drawn at the row's right.
    pub counts: Option<String>,
    /// The current branch's upstream is gone: the warning triangle drawn at the row's right,
    /// the check mark keeping its place.
    pub warning: bool,
    pub chosen: bool,
}

/// What `row` of `refs` draws: its glyph, its name, how deep it sits, and a branch's marks.
pub fn drawn_row(
    row: SidebarRow,
    refs: &RefsSnapshot,
    counts: Option<&[(RefName, AheadBehind)]>,
    chosen: Option<&SidebarTarget>,
) -> Option<DrawnRow> {
    let target = SidebarTarget::of(row, refs);
    let is_chosen = target.is_some() && target.as_ref() == chosen;
    let plain = |glyph, text: String, depth| DrawnRow {
        glyph,
        disclosure: None,
        text,
        depth,
        bold: false,
        counts: None,
        warning: false,
        chosen: is_chosen,
    };
    Some(match row {
        SidebarRow::Section { section, open } => DrawnRow {
            disclosure: Some(open),
            ..plain(
                disclosure_glyph(open),
                section_caption(section).to_owned(),
                0,
            )
        },
        SidebarRow::Folder { first, depth, open } => DrawnRow {
            disclosure: Some(open),
            ..plain(
                RefGlyph::Folder,
                folder_name(refs, first, depth).to_owned(),
                depth,
            )
        },
        SidebarRow::DetachedHead => DrawnRow {
            bold: true,
            ..plain(RefGlyph::Current, DETACHED_HEAD_CAPTION.to_owned(), 0)
        },
        SidebarRow::Stash { index } => {
            let stash = refs.stashes.get(index as usize)?;
            plain(RefGlyph::Stash, stash.message.clone(), 0)
        }
        SidebarRow::Ref { index, depth } => {
            let listed = refs.refs.get(index as usize)?;
            let short = listed.name.shorthand();
            // A symbolic remote-tracking ref — `origin/HEAD` — keeps its remote's name, as
            // Fork lists it (the user's decision, 2026-10-07).
            let text = match (listed.kind, &listed.symbolic) {
                (RefKind::Tag, _) | (RefKind::RemoteTracking, Some(_)) => short,
                (RefKind::LocalBranch, _) | (RefKind::RemoteTracking, None) => {
                    short.rsplit('/').next().unwrap_or(short)
                }
            }
            .to_owned();
            match listed.kind {
                RefKind::LocalBranch => {
                    let current =
                        matches!(&refs.head, HeadState::Branch(name) if *name == listed.name);
                    let gone = listed.upstream.as_ref().is_some_and(Upstream::is_gone);
                    let counts = match &listed.upstream {
                        Some(Upstream::Exists { .. }) => counts
                            .and_then(|counts| counts_of(counts, &listed.name))
                            .map(counts_text)
                            .filter(|text| !text.is_empty()),
                        Some(Upstream::Gone { .. }) | None => None,
                    };
                    let glyph = match (current, &listed.upstream) {
                        (true, _) => RefGlyph::Current,
                        (false, Some(Upstream::Gone { .. })) => RefGlyph::Gone,
                        (false, Some(Upstream::Exists { .. })) => RefGlyph::Branch,
                        (false, None) => RefGlyph::LocalOnly,
                    };
                    DrawnRow {
                        bold: current,
                        counts,
                        warning: current && gone,
                        ..plain(glyph, text, depth)
                    }
                }
                RefKind::RemoteTracking => plain(RefGlyph::Remote, text, depth),
                RefKind::Tag => plain(RefGlyph::Tag, text, depth),
            }
        }
    })
}

fn disclosure_glyph(open: bool) -> RefGlyph {
    if open {
        RefGlyph::Opened
    } else {
        RefGlyph::Closed
    }
}

fn build_row(item: VirtualItem, data: &ListData) -> Element {
    let row = rect()
        .key(item.index)
        .width(Size::fill())
        .height(Size::px(item.size));
    let Some(&sidebar_row) = data.shown.rows.get(item.index) else {
        return row.into();
    };
    let Some(drawn) = drawn_row(
        sidebar_row,
        &data.shown.refs,
        data.counts.as_deref().map(Vec::as_slice),
        data.chosen.as_ref(),
    ) else {
        return row.into();
    };
    let colours = get_theme_or_default().read().colors().clone();
    let on_row = data.on_row.clone();
    let indent = 8. + f32::from(drawn.depth) * SIDEBAR_INDENT;
    let section = matches!(sidebar_row, SidebarRow::Section { .. });
    let glyph_colour = match drawn.glyph {
        RefGlyph::Gone => colours.warning,
        _ => colours.text_secondary,
    };
    row.horizontal()
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .padding(Gaps::new(0., 8., 0., indent))
        .spacing(5.)
        .maybe(drawn.chosen, |el| el.background(colours.surface_secondary))
        .on_press(move |_| on_row.call(sidebar_row))
        .maybe_child(
            drawn
                .disclosure
                .filter(|_| !section)
                .map(|open| glyph(disclosure_glyph(open), colours.text_secondary)),
        )
        .child(glyph(drawn.glyph, glyph_colour))
        .child(
            label()
                .text(drawn.text)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .width(Size::flex(1.))
                .font_size(if section { 12. } else { FONT_SIZE })
                .maybe(drawn.bold || section, |el| el.font_weight(FontWeight::BOLD))
                .color(if section {
                    colours.text_secondary
                } else {
                    colours.text_primary
                }),
        )
        .maybe_child(drawn.counts.map(|text| {
            label()
                .text(text)
                .max_lines(1)
                .font_family(DIFF_FONT_FAMILY)
                .font_size(12.)
                .color(colours.text_secondary)
        }))
        .maybe_child(
            drawn
                .warning
                .then(|| glyph(RefGlyph::Gone, colours.warning)),
        )
        .into()
}

/// A glyph keyed by what it draws: a canvas repaints only when its layout changes.
fn glyph(glyph: RefGlyph, colour: Color) -> Element {
    rect()
        .key(glyph)
        .width(Size::px(GLYPH_SIZE))
        .height(Size::px(GLYPH_SIZE))
        .child(glyph.draw(colour))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_changes_says_its_count_and_leaves_out_none() {
        assert_eq!(local_changes_text(Some(3)), "Local Changes (3)");
        assert_eq!(local_changes_text(Some(0)), "Local Changes");
        assert_eq!(local_changes_text(None), "Local Changes");
    }
}
