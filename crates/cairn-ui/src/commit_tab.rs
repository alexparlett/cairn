//! The Commit tab (PRD R5.3): who wrote the commit and who committed it, each with the full
//! timestamp at its own offset; the full commit id; each parent as a link; the whole
//! message; then the changed files, a rename or a copy showing both names.
//!
//! Laid out as Fork lays it out (decision L9), with one difference forced by the list's
//! size: the whole tab is ONE virtualised list of equal rows — header, message lines, then
//! one row per changed file — because a commit can touch 55,184 paths (R5.5) and a list that
//! builds a row each is the thing this repository's invariants exist to stop. So a message
//! line is never wrapped, as git never re-wraps one; a line wider than the pane scrolls
//! sideways rather than being cut.
//!
//! No avatar and no ref chips (L9), and the tab never asks for anything: a parent link
//! reports the parent and the caller decides what pressing it reaches.

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{ChangeSet, ChangeStatus, ChangedFile, Oid, Signature};
use freya::prelude::*;

use crate::{date_text, message_lines};

/// Every row of the tab is this tall: a fixed size is what keeps the list O(viewport).
pub const DETAIL_ROW_HEIGHT: f32 = 24.0;

const FONT_SIZE: f32 = 13.0;
const SUBJECT_FONT_SIZE: f32 = 15.0;
const CAPTION_FONT_SIZE: f32 = 11.0;
const CAPTION_WIDTH: f32 = 72.0;
const BADGE_WIDTH: f32 = 18.0;
const PADDING: f32 = 12.0;
const GAP: f32 = 10.0;

/// The caption over the author's column.
pub const AUTHOR_CAPTION: &str = "AUTHOR";
/// The caption over the committer's column.
pub const COMMITTER_CAPTION: &str = "COMMITTER";
/// The caption beside the full commit id.
pub const ID_CAPTION: &str = "SHA";
/// The caption beside the parent links.
pub const PARENTS_CAPTION: &str = "PARENTS";

/// What stands where the files would be when a commit changed none.
pub const NO_FILES: &str = "No files changed.";

/// The notice above the files when `diff.renameLimit` stopped git's rename search (R2.2):
/// git's own warning, and the number it asks the limit to be raised to.
pub fn cut_short_notice(needed_limit: usize) -> String {
    format!(
        "Exhaustive rename detection was skipped due to too many files: set diff.renameLimit \
         to at least {needed_limit} to pair every rename."
    )
}

/// The bare letter Fork draws for `status` (user decision 5, 2026-10-03): git's
/// `--name-status` letter without the similarity score git prints after `R` and `C`.
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
    Captions,
    People {
        author: Signature,
        committer: Signature,
    },
    Dates {
        author: String,
        committer: String,
    },
    Id(String),
    Parents(Vec<Oid>),
    Rule,
    Subject(String),
    Body(String),
    CutShort(usize),
    NoFiles,
}

/// The rows above the files, for `changes`. Proportional to the message, never to the
/// files: those are read by index as they scroll into view.
fn header_lines(changes: &ChangeSet) -> Vec<Line> {
    #[cfg(test)]
    tests::HEADER_BUILDS.with(|builds| builds.set(builds.get() + 1));
    let mut lines = Vec::new();
    if let Some(details) = &changes.details {
        lines.push(Line::Captions);
        lines.push(Line::People {
            author: details.author.clone(),
            committer: details.committer.clone(),
        });
        lines.push(Line::Dates {
            author: date_text::git_default(details.author.time),
            committer: date_text::git_default(details.committer.time),
        });
        lines.push(Line::Id(details.id.hex().as_str().to_owned()));
        if !details.parents.is_empty() {
            lines.push(Line::Parents(details.parents.clone()));
        }
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
    }
    lines
}

/// Everything [`header_lines`] reads from a change set but the commit's own fields, which
/// its id names: one commit's header is built once, however often the state it is read
/// from is written (a file's diff arriving, from phase 06) without changing the commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeaderKey {
    commit: Option<Oid>,
    needed_limit: Option<usize>,
    no_files: bool,
}

impl HeaderKey {
    fn of(changes: &ChangeSet) -> Self {
        Self {
            commit: changes.details.as_ref().map(|details| details.id),
            needed_limit: changes.renames.needed_limit,
            no_files: changes.files.is_empty(),
        }
    }
}

/// The header last built, and what it was built from.
type HeaderCache = Rc<RefCell<Option<(HeaderKey, Rc<Vec<Line>>)>>>;

/// The header for `changes`: the one in `cache` while its key still holds.
fn cached_header(cache: &HeaderCache, changes: &ChangeSet) -> Rc<Vec<Line>> {
    let key = HeaderKey::of(changes);
    let mut cached = cache.borrow_mut();
    if let Some((built_for, lines)) = cached.as_ref()
        && *built_for == key
    {
        return lines.clone();
    }
    let lines = Rc::new(header_lines(changes));
    *cached = Some((key, lines.clone()));
    lines
}

/// The Commit tab over one change set. `changes` is a handle, not a copy: the files are read
/// by index as rows are built, so a commit of any size costs one viewport per frame.
pub struct CommitTab {
    changes: Readable<ChangeSet>,
    on_parent: EventHandler<Oid>,
    key: DiffKey,
}

impl CommitTab {
    pub fn new(changes: impl Into<Readable<ChangeSet>>) -> Self {
        Self {
            changes: changes.into(),
            on_parent: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    /// A parent link was pressed. Whether that parent can be reached is the caller's to
    /// decide (R5.3: a loaded parent is selected; an unloaded one is issue #3).
    pub fn on_parent(mut self, on_parent: impl Into<EventHandler<Oid>>) -> Self {
        self.on_parent = on_parent.into();
        self
    }
}

// Hand-written: `EventHandler` never compares equal, and its identity is stable.
impl PartialEq for CommitTab {
    fn eq(&self, other: &Self) -> bool {
        self.changes == other.changes && self.key == other.key
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

/// Data captured inside the builder closure is invisible to `VirtualScrollView`'s diffing,
/// so what decides which rows are drawn is passed here.
#[derive(Clone)]
struct TabData {
    changes: Readable<ChangeSet>,
    lines: Rc<Vec<Line>>,
    files: usize,
    on_parent: EventHandler<Oid>,
}

impl PartialEq for TabData {
    fn eq(&self, other: &Self) -> bool {
        // The header is cached, so an unchanged one is the same allocation: no compare.
        (Rc::ptr_eq(&self.lines, &other.lines) || self.lines == other.lines)
            && self.files == other.files
    }
}

impl Component for CommitTab {
    fn render(&self) -> impl IntoElement {
        let cache: HeaderCache = use_hook(HeaderCache::default);
        // Reading subscribes the tab to the answer it draws.
        let (lines, files, identity) = {
            let changes = self.changes.read();
            let identity = changes.details.as_ref().map(|details| details.id);
            (
                cached_header(&cache, &changes),
                changes.files.len(),
                identity,
            )
        };
        let data = TabData {
            changes: self.changes.clone(),
            lines,
            files,
            on_parent: self.on_parent.clone(),
        };
        let length = data.lines.len() + files;

        VirtualScrollView::new_with_data(data, build_row)
            // Another commit is another list: it opens at its top, not where the last one was.
            .key(identity)
            .length(length)
            .item_size(DETAIL_ROW_HEIGHT)
            .expanded()
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

fn build_row(item: VirtualItem, data: &TabData) -> Element {
    let row = match data.lines.get(item.index) {
        Some(line) => header_row(line, &data.on_parent),
        None => {
            let index = item.index - data.lines.len();
            // Read, not peeked: the list redraws when the answer it shows is replaced.
            match data.changes.read().files.get(index) {
                Some(file) => file_row(file),
                // The count and the files can disagree for one frame; an empty row of the
                // right height stands in.
                None => rect().into(),
            }
        }
    };
    rect()
        .key(item.index)
        .min_width(Size::fill())
        .height(Size::px(item.size))
        .main_align(Alignment::Center)
        .padding(Gaps::new(0., PADDING, 0., PADDING))
        .child(row)
        .into()
}

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
    text(content, CAPTION_FONT_SIZE, colours().text_placeholder)
}

fn header_row(line: &Line, on_parent: &EventHandler<Oid>) -> Element {
    let colours = colours();
    match line {
        Line::Captions => columns(caption(AUTHOR_CAPTION), caption(COMMITTER_CAPTION)),
        Line::People { author, committer } => columns(person(author), person(committer)),
        Line::Dates { author, committer } => columns(
            text(author.clone(), FONT_SIZE, colours.text_secondary),
            text(committer.clone(), FONT_SIZE, colours.text_secondary),
        ),
        Line::Id(id) => labelled(
            ID_CAPTION,
            text(id.clone(), FONT_SIZE, colours.text_primary).into(),
        ),
        Line::Parents(parents) => labelled(
            PARENTS_CAPTION,
            rect()
                .horizontal()
                .spacing(GAP)
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
                                FONT_SIZE,
                                colours.text_highlight,
                            )
                            .text_decoration(TextDecoration::Underline),
                        )
                        .into()
                }))
                .into(),
        ),
        Line::Rule => rect()
            .width(Size::fill())
            .height(Size::px(1.))
            .background(colours.border)
            .into(),
        Line::Subject(subject) => text(subject.clone(), SUBJECT_FONT_SIZE, colours.text_primary)
            .font_weight(FontWeight::BOLD)
            .into(),
        Line::Body(body) => text(body.clone(), FONT_SIZE, colours.text_primary).into(),
        Line::CutShort(needed) => {
            text(cut_short_notice(*needed), FONT_SIZE, colours.warning).into()
        }
        Line::NoFiles => text(NO_FILES, FONT_SIZE, colours.text_placeholder).into(),
    }
}

/// Fork's two columns: the author's on the left, the committer's on the right.
fn columns(left: Label, right: Label) -> Element {
    rect()
        .horizontal()
        .content(Content::Flex)
        .width(Size::fill())
        .spacing(GAP)
        .child(
            rect().width(Size::flex(1.)).child(
                left.width(Size::fill())
                    .text_overflow(TextOverflow::Ellipsis),
            ),
        )
        .child(
            rect().width(Size::flex(1.)).child(
                right
                    .width(Size::fill())
                    .text_overflow(TextOverflow::Ellipsis),
            ),
        )
        .into()
}

/// A name and, in a lighter tone, the address beside it, as git prints `Name <email>`.
fn person(signature: &Signature) -> Label {
    let colours = colours();
    label()
        .max_lines(1)
        .font_size(FONT_SIZE)
        .color(colours.text_primary)
        .text(format!("{} <{}>", signature.name, signature.email))
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

fn file_row(file: &ChangedFile) -> Element {
    let colours = colours();
    let badge = match file.status {
        ChangeStatus::Added => colours.success,
        ChangeStatus::Deleted => colours.error,
        ChangeStatus::Modified | ChangeStatus::TypeChanged => colours.warning,
        ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => colours.info,
    };
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(GAP)
        .child(
            text(status_letter(file.status), FONT_SIZE, badge)
                .width(Size::px(BADGE_WIDTH))
                .font_weight(FontWeight::BOLD),
        )
        .child(text(file_text(file), FONT_SIZE, colours.text_primary))
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
        let lines = header_lines(&set(
            Some(details("subject\n\nbody one\r\nbody two\n", 2)),
            10_000,
            None,
        ));
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

    /// A root commit has no parents row; a cut-short search says so; an empty commit says it
    /// changed nothing.
    #[test]
    fn the_header_says_what_is_missing_and_what_was_cut_short() {
        let root = header_lines(&set(Some(details("x", 0)), 0, Some(2774)));
        assert!(!root.iter().any(|line| matches!(line, Line::Parents(_))));
        assert!(root.contains(&Line::CutShort(2774)));
        assert!(root.contains(&Line::NoFiles));

        let merge = header_lines(&set(Some(details("x", 2)), 1, None));
        assert!(
            merge
                .iter()
                .any(|line| matches!(line, Line::Parents(p) if p.len() == 2))
        );
        assert!(!merge.iter().any(|line| matches!(line, Line::CutShort(_))));
        assert!(!merge.contains(&Line::NoFiles));
    }

    /// R1: a write to the state the tab reads that leaves the commit as it was — what a
    /// file's diff arriving in the window's `DiffState` is — redraws the tab without
    /// building its header again; another commit builds it once. Caught by: building the
    /// header on every render, or caching it past a change of commit.
    #[test]
    fn the_header_is_built_once_per_commit_however_often_the_state_is_written() {
        #[derive(Clone)]
        struct Fixture {
            state: State<(ChangeSet, u32)>,
        }
        let app = || {
            let fixture = use_consume::<Fixture>();
            let changes = fixture
                .state
                .into_readable()
                .map(|(changes, _)| changes, |_| true);
            rect()
                .expanded()
                .child(CommitTab::new(changes))
                .into_element()
        };
        let (mut test, fixture) = TestingRunner::new(
            app,
            (600., 400.).into(),
            |runner| {
                runner.provide_root_context(|| Fixture {
                    state: State::create((set(Some(details("one\n", 1)), 3, None), 0)),
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
    }

    /// T6, user decision 5: each status is its bare letter, as Fork draws it — a rename and
    /// a copy without the similarity git's `--name-status` prints after the letter. Caught
    /// by: a letter swapped between two statuses, or a score appended.
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
