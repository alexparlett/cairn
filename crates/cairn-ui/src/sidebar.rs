//! The sidebar, Fork's (`docs/research/ui-review/fork-window-survey-2026-10-16.md` §3, uw27):
//! a header row with the repository's name and a gear whose menu copies its path; Local Changes
//! with its count and All Commits, each with its icon, the one chosen a rounded grey band, which
//! choose what the main region shows; a filter field with its magnifier; and one virtualized list
//! of the refs — Branches, Remotes, Tags, Stashes, each a bold heading with its chevron and no
//! count, branches and remote-tracking refs in folders split at `/`, each row its leaf name and
//! each level indented 12 px. The rows are laid out on a worker (`RefsSnapshot::sidebar_rows`);
//! the list reads each one's name from the snapshot they index as it builds that row alone, so a
//! snapshot of tens of thousands of refs builds one viewport.
//!
//! A branch draws Fork's marks: the current branch a check mark in place of its glyph and its
//! name bold; a branch with no upstream its own quieter glyph; a branch whose upstream is gone
//! Fork's warning on its glyph's bottom right — the current branch's at its right, beside the
//! check mark; any other branch with an upstream its ahead and behind counts at its right
//! (`counts_text`). A remote's folder is its cloud; a stash reads its message after git's
//! `On <branch>: ` (`stash_text`). A line under the filter says what a press is doing —
//! "Finding <ref>…" — or why it found nothing.
//!
//! A right press on a ref selects its commit as a press does and opens its menu (`ref_menu`):
//! New Branch… and Copy Branch Name or Copy Tag Name. The sidebar's keys are its own while it
//! has focus (`keys`): ↑ and ↓ walk the rows of the list, ← and → open, close and climb them as
//! a tree.

use std::sync::Arc;

use cairn_model::{
    AheadBehind, HeadState, Oid, RefKind, RefName, RefsSnapshot, SidebarRow, SidebarSection,
    Upstream, folder_name,
};
use freya::prelude::*;

use crate::accelerators::Action;
use crate::history::list::NEW_BRANCH_CAPTION;
use crate::menu::{Item, context_menu};
use crate::style::{icons, metrics, tones};
use crate::text_field::text_field;

pub(crate) mod keys;

pub use keys::{SidebarStep, chosen_when_walked, walked_to};

/// A row's height in the sidebar's list.
pub const SIDEBAR_ROW_HEIGHT: f32 = metrics::SIDEBAR_ROW;
/// How far each folder level indents what it holds.
pub const SIDEBAR_INDENT: f32 = metrics::SIDEBAR_INDENT;
/// The filter box's placeholder.
pub const SIDEBAR_FILTER_PLACEHOLDER: &str = "Filter";
/// The entry that shows the working tree's changes in the main region.
pub const LOCAL_CHANGES_CAPTION: &str = "Local Changes";
/// The entry that shows the history in the main region.
pub const ALL_COMMITS_CAPTION: &str = "All Commits";
/// What a detached `HEAD`'s row is called, as Fork names it in Branches.
pub const DETACHED_HEAD_CAPTION: &str = "HEAD";
/// The gear's one item (uw27): the repository's path to the clipboard.
pub const COPY_REPOSITORY_PATH_CAPTION: &str = "Copy Repository Path";
/// The name the header's gear goes by, for its tooltip and a test.
pub const REPOSITORY_GEAR_NAME: &str = "Repository settings";
/// A branch's menu item: its name to the clipboard (FS1 §8).
pub const COPY_BRANCH_NAME_CAPTION: &str = "Copy Branch Name";
/// A tag's menu item: its name to the clipboard (FS1 §8).
pub const COPY_TAG_NAME_CAPTION: &str = "Copy Tag Name";
/// Why New Branch… is greyed on a tag that names no commit.
pub const NO_COMMIT_TO_BRANCH_FROM: &str = "It names no commit to start a branch at.";

/// What the main region shows.
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

/// `Local Changes (N)`, the count left out when there is none.
pub fn local_changes_text(count: Option<usize>) -> String {
    match count {
        Some(count) if count > 0 => format!("{LOCAL_CHANGES_CAPTION} ({count})"),
        Some(_) | None => LOCAL_CHANGES_CAPTION.to_owned(),
    }
}

/// A stash's row text: its message after git's `On <branch>: `, as Fork reads it
/// ("WIP on parser"); any other message whole. A branch name holds no `:`, so the prefix ends
/// at the first `: `.
pub fn stash_text(message: &str) -> &str {
    message
        .strip_prefix("On ")
        .and_then(|rest| rest.split_once(": "))
        .filter(|(branch, _)| !branch.is_empty())
        .map_or(message, |(_, after)| after)
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

/// The gear's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GearChoice {
    CopyRepositoryPath,
}

/// A ref's menu choice, on the row it was opened on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefChoice {
    /// New Branch…, at the ref's commit.
    NewBranch,
    /// Copy Branch Name or Copy Tag Name.
    CopyName,
}

pub struct Sidebar {
    shown: Option<SidebarRefs>,
    name: Option<String>,
    counts: Option<BranchCounts>,
    local_changes: Option<usize>,
    main: MainView,
    chosen: Option<SidebarTarget>,
    filter: Writable<String>,
    notice: Option<String>,
    on_row: EventHandler<SidebarRow>,
    on_main: EventHandler<MainView>,
    on_gear: EventHandler<GearChoice>,
    on_ref_menu: EventHandler<(SidebarRow, RefChoice)>,
    key: DiffKey,
}

impl Sidebar {
    /// The sidebar over `shown` — nothing listed until the first rows are laid out — with the
    /// filter's text in `filter`, which the caller asks the rows again for as it changes.
    pub fn new(shown: Option<SidebarRefs>, filter: impl Into<Writable<String>>) -> Self {
        Self {
            shown,
            name: None,
            counts: None,
            local_changes: None,
            main: MainView::AllCommits,
            chosen: None,
            filter: filter.into(),
            notice: None,
            on_row: EventHandler::new(|_| {}),
            on_main: EventHandler::new(|_| {}),
            on_gear: EventHandler::new(|_| {}),
            on_ref_menu: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// The repository's name, drawn bold in the header row; `None` until it is opened.
    pub fn name(mut self, name: Option<String>) -> Self {
        self.name = name;
        self
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

    /// A row pressed, or walked to by ↑ or ↓ or opened by Return: a section or folder to open
    /// or close, a ref or stash to find. A right press on a ref, a stash or a detached `HEAD`
    /// reports it too, before its menu opens.
    pub fn on_row(mut self, on_row: impl Into<EventHandler<SidebarRow>>) -> Self {
        self.on_row = on_row.into();
        self
    }

    /// Local Changes or All Commits pressed, or opened by Return.
    pub fn on_main(mut self, on_main: impl Into<EventHandler<MainView>>) -> Self {
        self.on_main = on_main.into();
        self
    }

    /// The gear's menu chosen.
    pub fn on_gear(mut self, on_gear: impl Into<EventHandler<GearChoice>>) -> Self {
        self.on_gear = on_gear.into();
        self
    }

    /// A ref's menu chosen, with the row it was opened on.
    pub fn on_ref_menu(
        mut self,
        on_ref_menu: impl Into<EventHandler<(SidebarRow, RefChoice)>>,
    ) -> Self {
        self.on_ref_menu = on_ref_menu.into();
        self
    }
}

// Hand-written: `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for Sidebar {
    fn eq(&self, other: &Self) -> bool {
        self.shown == other.shown
            && self.name == other.name
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
            .field("name", &self.name)
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
    walked: Option<usize>,
    walk: State<Option<keys::Walk>>,
    on_row: EventHandler<SidebarRow>,
    on_ref_menu: EventHandler<(SidebarRow, RefChoice)>,
    sidebar_id: AccessibilityId,
    focused: bool,
}

impl PartialEq for ListData {
    fn eq(&self, other: &Self) -> bool {
        self.shown == other.shown
            && same_counts(&self.counts, &other.counts)
            && self.chosen == other.chosen
            && self.walked == other.walked
            && self.walk == other.walk
            && self.sidebar_id == other.sidebar_id
            && self.focused == other.focused
    }
}

impl Component for Sidebar {
    fn render(&self) -> impl IntoElement {
        // The sidebar takes focus when one of its rows is pressed, and its chosen row says so;
        // while it has focus the arrows are its own.
        let sidebar_id = use_a11y();
        let focus = use_focus(sidebar_id);
        let focused = focus().is_focused();
        let controller = use_scroll_controller(ScrollConfig::default);
        // Where a press or a key last left the walk, while it holds (`keys::place_now`).
        let walk = use_state(|| None::<keys::Walk>);
        let walked = keys::place_now(
            walk.read().as_ref(),
            self.chosen.as_ref(),
            self.shown.as_ref(),
        );

        // Local Changes and All Commits: a press switches the main region; the arrows never
        // reach them, as in Fork.
        let entry = |text: String, view: MainView, icon: fn(f32, Color) -> Element| {
            let on_main = self.on_main.clone();
            let chosen = self.main == view;
            rect()
                .width(Size::fill())
                .height(Size::px(SIDEBAR_ROW_HEIGHT))
                .padding(Gaps::new(
                    0.,
                    metrics::SIDEBAR_BAND_INSET,
                    0.,
                    metrics::SIDEBAR_BAND_INSET,
                ))
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .height(Size::fill())
                        .cross_align(Alignment::Center)
                        .padding(Gaps::new(
                            0.,
                            0.,
                            0.,
                            metrics::SIDEBAR_INSET + SIDEBAR_INDENT - metrics::SIDEBAR_BAND_INSET,
                        ))
                        .spacing(metrics::SIDEBAR_GAP)
                        .corner_radius(metrics::SIDEBAR_BAND_RADIUS)
                        .maybe(chosen, |el| el.background(tones::SIDEBAR_CHOSEN))
                        .on_press(move |_| on_main.call(view))
                        .child(
                            rect()
                                .key("icon")
                                .child(icon(metrics::SIDEBAR_ICON, tones::SIDEBAR_ICON)),
                        )
                        .child(
                            label()
                                .text(text)
                                .max_lines(1)
                                .font_size(metrics::BODY)
                                .color(tones::TEXT),
                        ),
                )
        };

        let header = {
            let on_gear = self.on_gear.clone();
            rect()
                .horizontal()
                .content(Content::Flex)
                .width(Size::fill())
                .height(Size::px(SIDEBAR_ROW_HEIGHT))
                .cross_align(Alignment::Center)
                .padding(Gaps::new(
                    0.,
                    metrics::SIDEBAR_INSET,
                    0.,
                    metrics::SIDEBAR_INSET + 4.,
                ))
                .child(
                    label()
                        .text(self.name.clone().unwrap_or_default())
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .width(Size::flex(1.))
                        .font_size(metrics::BODY)
                        .font_weight(FontWeight::BOLD)
                        .color(tones::TEXT),
                )
                .child(
                    TooltipContainer::new(Tooltip::new_text(REPOSITORY_GEAR_NAME)).child(
                        rect()
                            .a11y_role(AccessibilityRole::Button)
                            .a11y_builder(|node| node.set_value(REPOSITORY_GEAR_NAME))
                            .on_press(move |_| {
                                let on_gear = on_gear.clone();
                                ContextMenu::open(context_menu(
                                    vec![vec![Item::new(
                                        COPY_REPOSITORY_PATH_CAPTION,
                                        GearChoice::CopyRepositoryPath,
                                    )]],
                                    move |choice: GearChoice| on_gear.call(choice),
                                ));
                            })
                            .child(icons::gear(metrics::SIDEBAR_ICON, tones::SIDEBAR_ICON)),
                    ),
                )
        };

        let list: Element = match &self.shown {
            Some(shown) => {
                let length = shown.rows.len();
                VirtualScrollView::new_with_data_controlled(
                    ListData {
                        shown: shown.clone(),
                        counts: self.counts.clone(),
                        chosen: self.chosen.clone(),
                        walked,
                        walk,
                        on_row: self.on_row.clone(),
                        on_ref_menu: self.on_ref_menu.clone(),
                        sidebar_id,
                        focused,
                    },
                    build_row,
                    controller,
                )
                .length(length)
                .item_size(SIDEBAR_ROW_HEIGHT)
                // The arrows walk the rows, not the viewport.
                .scroll_with_arrows(false)
                .expanded()
                .into()
            }
            None => rect().expanded().into(),
        };

        let on_key = keys::handler(keys::Keys {
            walk,
            chosen: self.chosen.clone(),
            shown: self.shown.clone(),
            on_row: self.on_row.clone(),
            controller,
        });

        rect()
            .expanded()
            .background(tones::SIDEBAR)
            .a11y_id(sidebar_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::List)
            .on_key_down(on_key)
            .maybe(focus() == Focus::Keyboard, |el| {
                el.border(Border::new().fill(tones::ACCENT).width(1.))
            })
            .padding(Gaps::new(4., 0., 0., 0.))
            .child(header)
            .child(entry(
                local_changes_text(self.local_changes),
                MainView::LocalChanges,
                icons::local_changes,
            ))
            .child(entry(
                ALL_COMMITS_CAPTION.to_owned(),
                MainView::AllCommits,
                icons::all_commits,
            ))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(1.))
                    .margin(Gaps::new(4., 0., 0., 0.))
                    .background(tones::RULE),
            )
            .child(
                rect().width(Size::fill()).padding(6.).child(
                    text_field(self.filter.clone())
                        .placeholder(SIDEBAR_FILTER_PLACEHOLDER)
                        .compact()
                        .background(tones::FILTER_FIELD)
                        .hover_background(tones::FILTER_FIELD)
                        .focus_background(tones::FILTER_FIELD)
                        .leading(icons::magnifier(metrics::SIDEBAR_ICON, tones::SIDEBAR_ICON))
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
                            .font_size(metrics::BODY)
                            .color(tones::DETAIL_LABEL),
                    )
            }))
            .child(list)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// The icon a sidebar row draws before its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarGlyph {
    /// A folder of branches or remote-tracking refs: an open folder.
    Folder,
    /// A remote's own folder, `origin`: its cloud.
    Remote,
    /// A branch with an upstream, and a remote-tracking ref.
    Branch,
    /// A branch with no upstream: Cairn's quieter glyph of its own (a `ui.md` deviation).
    LocalOnly,
    /// The branch `HEAD` is on, and a detached `HEAD`: a check mark.
    Current,
    /// A branch whose upstream is configured but gone: the branch glyph with a warning on it.
    Gone,
    Tag,
    Stash,
}

impl SidebarGlyph {
    /// The icon, at the sidebar's size, in its own tone — or `selected`'s text while its row is
    /// chosen. A gone upstream's is the branch glyph with Fork's warning on its bottom right.
    fn draw(self, selected: Option<Color>) -> Element {
        let tone = |own: Color| selected.unwrap_or(own);
        let size = metrics::SIDEBAR_ICON;
        match self {
            Self::Folder => icons::folder(size, tone(tones::SIDEBAR_ICON)),
            Self::Remote => icons::cloud(size, tone(tones::REMOTE_GLYPH)),
            Self::Branch => icons::branch(size, tone(tones::SIDEBAR_ICON)),
            Self::LocalOnly => icons::local_branch(size, tone(tones::SIDEBAR_DIM_ICON)),
            Self::Current => icons::check(size, tone(tones::SIDEBAR_ICON)),
            Self::Gone => rect()
                .width(Size::px(size))
                .height(Size::px(size))
                .child(icons::branch(size, tone(tones::SIDEBAR_ICON)))
                .child(
                    rect()
                        .position(
                            Position::new_absolute()
                                .top(size - metrics::SIDEBAR_BADGE)
                                .left(size - metrics::SIDEBAR_BADGE),
                        )
                        .child(icons::warning(metrics::SIDEBAR_BADGE, tones::UPSTREAM_GONE)),
                )
                .into(),
            Self::Tag => icons::tag(size, tone(tones::SIDEBAR_ICON)),
            Self::Stash => icons::archive(size, tone(tones::SIDEBAR_ICON)),
        }
    }
}

/// What one row draws, read out of the snapshot for that row alone.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawnRow {
    /// The icon before the name: `None` for a section's heading, which draws its chevron alone.
    pub glyph: Option<SidebarGlyph>,
    /// Whether it is a section or folder, open (`Some(true)`) or closed: drawn with the
    /// chevron that says so.
    pub disclosure: Option<bool>,
    pub text: String,
    /// How many levels in it stands: 0 for a heading and for what stands straight under one.
    pub depth: u16,
    /// A heading, or the current branch: its name drawn bold.
    pub bold: bool,
    /// A branch's counts against its upstream, drawn at the row's right.
    pub counts: Option<String>,
    /// The current branch's upstream is gone: the warning drawn at the row's right, the check
    /// mark keeping its place.
    pub warning: bool,
    pub chosen: bool,
}

/// How far in a row's chevron slot starts: a heading's at the inset, every row under it one
/// level further in per level it stands.
pub fn row_indent(row: &DrawnRow) -> f32 {
    let levels = match row.glyph {
        None => 0.,
        Some(_) => 1. + f32::from(row.depth),
    };
    metrics::SIDEBAR_INSET + levels * SIDEBAR_INDENT
}

/// What `row` of `refs` draws: its icon, its name, how deep it sits, and a branch's marks.
pub fn drawn_row(
    row: SidebarRow,
    refs: &RefsSnapshot,
    counts: Option<&[(RefName, AheadBehind)]>,
    chosen: Option<&SidebarTarget>,
) -> Option<DrawnRow> {
    let target = SidebarTarget::of(row, refs);
    let is_chosen = target.is_some() && target.as_ref() == chosen;
    let plain = |glyph, text: String, depth| DrawnRow {
        glyph: Some(glyph),
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
            glyph: None,
            disclosure: Some(open),
            text: section_caption(section).to_owned(),
            depth: 0,
            bold: true,
            counts: None,
            warning: false,
            chosen: false,
        },
        SidebarRow::Folder { first, depth, open } => {
            // A remote's own folder is its cloud; every folder under it, and every folder of
            // branches, an open folder.
            let remote = depth == 0
                && refs
                    .refs
                    .get(first as usize)
                    .is_some_and(|listed| listed.kind == RefKind::RemoteTracking);
            let glyph = if remote {
                SidebarGlyph::Remote
            } else {
                SidebarGlyph::Folder
            };
            DrawnRow {
                disclosure: Some(open),
                ..plain(glyph, folder_name(refs, first, depth).to_owned(), depth)
            }
        }
        SidebarRow::DetachedHead => DrawnRow {
            bold: true,
            ..plain(SidebarGlyph::Current, DETACHED_HEAD_CAPTION.to_owned(), 0)
        },
        SidebarRow::Stash { index } => {
            let stash = refs.stashes.get(index as usize)?;
            plain(
                SidebarGlyph::Stash,
                stash_text(&stash.message).to_owned(),
                0,
            )
        }
        SidebarRow::Ref { index, depth } => {
            let listed = refs.refs.get(index as usize)?;
            let short = listed.name.shorthand();
            // A symbolic remote-tracking ref — `origin/HEAD` — keeps its remote's name, as
            // Fork lists it.
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
                        (true, _) => SidebarGlyph::Current,
                        (false, Some(Upstream::Gone { .. })) => SidebarGlyph::Gone,
                        (false, Some(Upstream::Exists { .. })) => SidebarGlyph::Branch,
                        (false, None) => SidebarGlyph::LocalOnly,
                    };
                    DrawnRow {
                        bold: current,
                        counts,
                        warning: current && gone,
                        ..plain(glyph, text, depth)
                    }
                }
                RefKind::RemoteTracking => plain(SidebarGlyph::Branch, text, depth),
                RefKind::Tag => plain(SidebarGlyph::Tag, text, depth),
            }
        }
    })
}

/// The menu a right press on `row` opens: a branch's or a remote-tracking ref's New Branch… and
/// Copy Branch Name, a tag's New Branch… and Copy Tag Name (FS1 §8, the items that work); a
/// stash, a detached `HEAD`, a folder and a heading open none. New Branch… is greyed on a ref
/// that names no commit.
pub fn ref_menu(row: SidebarRow, refs: &RefsSnapshot) -> Option<Vec<Vec<Item<RefChoice>>>> {
    let listed = match row {
        SidebarRow::Ref { index, .. } => refs.refs.get(index as usize)?,
        SidebarRow::Section { .. }
        | SidebarRow::Folder { .. }
        | SidebarRow::DetachedHead
        | SidebarRow::Stash { .. } => return None,
    };
    let copy = match listed.kind {
        RefKind::LocalBranch | RefKind::RemoteTracking => COPY_BRANCH_NAME_CAPTION,
        RefKind::Tag => COPY_TAG_NAME_CAPTION,
    };
    let no_commit = listed
        .commit_id()
        .is_none()
        .then(|| NO_COMMIT_TO_BRANCH_FROM.to_owned());
    Some(vec![
        vec![
            Item::new(NEW_BRANCH_CAPTION, RefChoice::NewBranch)
                .chord(Action::NewBranch)
                .greyed(no_commit),
        ],
        vec![Item::new(copy, RefChoice::CopyName)],
    ])
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
    let on_row = data.on_row.clone();
    let right_row = data.on_row.clone();
    let on_ref_menu = data.on_ref_menu.clone();
    let refs = Arc::clone(&data.shown.refs);
    let sidebar_id = data.sidebar_id;
    let mut walk = data.walk;
    // Where a press leaves the walk: on this row, with the entry it chooses chosen — and, for a
    // heading or a folder, which the press opens or closes, kept while the rows are laid out
    // again around it.
    let pressed_walk = if keys::chosen_when_walked(sidebar_row) {
        keys::Walk::at(
            &data.shown,
            item.index,
            SidebarTarget::of(sidebar_row, &data.shown.refs),
        )
    } else {
        keys::Walk::keeping(item.index, data.chosen.clone())
    };
    let right_walk = pressed_walk.clone();
    let mut right_walk_state = data.walk;
    let indent = row_indent(&drawn);
    let chosen = tones::selected(data.focused);
    // The walk's row is the one drawn selected while it holds; else the entry chosen.
    let selected = match data.walked {
        Some(walked) => walked == item.index,
        None => drawn.chosen,
    };
    let text = if selected { chosen.text } else { tones::TEXT };
    let icon_tone = selected.then_some(chosen.text);
    let chevron = drawn.disclosure.map(|open| {
        let tone = icon_tone.unwrap_or(tones::SIDEBAR_DIM_ICON);
        let icon = if open {
            icons::chevron_down(metrics::SIDEBAR_CHEVRON, tone)
        } else {
            icons::chevron_right(metrics::SIDEBAR_CHEVRON, tone)
        };
        rect()
            .key(("chevron", open))
            .width(Size::px(metrics::SIDEBAR_CHEVRON))
            .child(icon)
    });
    // A leaf keeps the chevron's slot empty, so its icon stands where its folders' do.
    let chevron = chevron.unwrap_or_else(|| {
        rect()
            .key(("chevron", "slot"))
            .width(Size::px(metrics::SIDEBAR_CHEVRON))
    });
    // Fork's band: rounded, the row's full height, standing in a few pixels from either edge.
    let band = rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .height(Size::fill())
        .cross_align(Alignment::Center)
        .padding(Gaps::new(
            0.,
            metrics::SIDEBAR_INSET - metrics::SIDEBAR_BAND_INSET,
            0.,
            indent - metrics::SIDEBAR_BAND_INSET,
        ))
        .spacing(metrics::SIDEBAR_GAP)
        .corner_radius(metrics::SIDEBAR_BAND_RADIUS)
        .maybe(selected, |el| el.background(chosen.band))
        .child(chevron)
        .maybe_child(drawn.glyph.map(|glyph| {
            rect()
                .key(("glyph", glyph))
                .width(Size::px(metrics::SIDEBAR_ICON))
                .height(Size::px(metrics::SIDEBAR_ICON))
                .child(glyph.draw(icon_tone))
        }))
        .child(
            label()
                .text(drawn.text)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .width(Size::flex(1.))
                .font_size(metrics::BODY)
                .maybe(drawn.bold || selected, |el| {
                    el.font_weight(FontWeight::BOLD)
                })
                .color(text),
        )
        .maybe_child(drawn.counts.map(|counts| {
            label()
                .text(counts)
                .max_lines(1)
                .font_size(metrics::BODY)
                .color(text)
        }))
        .maybe_child(drawn.warning.then(|| {
            rect()
                .width(Size::px(metrics::SIDEBAR_ICON))
                .height(Size::px(metrics::SIDEBAR_ICON))
                .child(icons::warning(metrics::SIDEBAR_ICON, tones::UPSTREAM_GONE))
        }));
    row.padding(Gaps::new(
        0.,
        metrics::SIDEBAR_BAND_INSET,
        0.,
        metrics::SIDEBAR_BAND_INSET,
    ))
    .on_press(move |_| {
        sidebar_id.request_focus();
        walk.set(Some(pressed_walk.clone()));
        on_row.call(sidebar_row);
    })
    // A right press on a ref selects its commit as a press does, then opens its menu.
    .on_pointer_down(move |e: Event<PointerEventData>| {
        if e.button() != Some(MouseButton::Right) {
            return;
        }
        match sidebar_row {
            SidebarRow::Ref { .. } | SidebarRow::DetachedHead | SidebarRow::Stash { .. } => {
                sidebar_id.request_focus();
                right_walk_state.set(Some(right_walk.clone()));
                right_row.call(sidebar_row);
            }
            SidebarRow::Section { .. } | SidebarRow::Folder { .. } => return,
        }
        let Some(groups) = ref_menu(sidebar_row, &refs) else {
            return;
        };
        let on_ref_menu = on_ref_menu.clone();
        ContextMenu::open_from_down(context_menu(groups, move |choice: RefChoice| {
            on_ref_menu.call((sidebar_row, choice));
        }));
    })
    .child(band)
    .into()
}

/// A branch's distance from its upstream as Fork for Windows draws it: ahead then behind, each
/// count with its arrow after it, a space between the two, a zero left out (`1↑ 1↓`, amd3) —
/// empty when the branch is level with its upstream.
pub fn counts_text(counts: AheadBehind) -> String {
    let ahead = (counts.ahead > 0).then(|| format!("{}↑", counts.ahead));
    let behind = (counts.behind > 0).then(|| format!("{}↓", counts.behind));
    ahead
        .into_iter()
        .chain(behind)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fork for Windows' counts: ahead then behind, each with its arrow after it, a space
    /// between, a zero left out, and nothing at all for a branch level with its upstream.
    /// Caught by: the two swapped, a zero drawn, the space dropped, or an arrow on the wrong
    /// count.
    #[test]
    fn counts_are_ahead_then_behind_and_a_zero_is_left_out() {
        let counted = |ahead, behind| AheadBehind { ahead, behind };
        assert_eq!(counts_text(counted(1, 18)), "1↑ 18↓");
        assert_eq!(counts_text(counted(0, 853)), "853↓");
        assert_eq!(counts_text(counted(2, 0)), "2↑");
        assert_eq!(counts_text(counted(0, 0)), "");
    }

    #[test]
    fn local_changes_says_its_count_and_leaves_out_none() {
        assert_eq!(local_changes_text(Some(3)), "Local Changes (3)");
        assert_eq!(local_changes_text(Some(0)), "Local Changes");
        assert_eq!(local_changes_text(None), "Local Changes");
    }

    /// A stash reads its message after git's `On <branch>: `, a detached one's `(no branch)`
    /// too; a message of any other form is whole. Caught by: the prefix kept, a message cut at
    /// a `: ` of its own, or another form cut.
    #[test]
    fn a_stash_reads_its_message_after_on_branch() {
        assert_eq!(stash_text("On main: WIP on parser"), "WIP on parser");
        assert_eq!(stash_text("On feature/x: a: b"), "a: b");
        assert_eq!(stash_text("On (no branch): wip"), "wip");
        assert_eq!(
            stash_text("WIP on main: 1234567 subject"),
            "WIP on main: 1234567 subject"
        );
        assert_eq!(stash_text("On : odd"), "On : odd");
        assert_eq!(stash_text("Online: notes"), "Online: notes");
    }
}
