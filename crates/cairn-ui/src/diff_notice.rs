//! What stands where a file's rows would be (PRD R6.8, R6.9): the states of R1.2 that are
//! not text, and a text diff with no row to draw.
//!
//! Each state says what it is in words that cannot be mistaken for another — a submodule's
//! notice names commits, never "binary" — and quotes git where git has words for it: a mode
//! change is git's own `old mode` and `new mode` lines, a rename with no content change git's
//! `similarity index`, `rename from` and `rename to` lines, a submodule git's `Subproject
//! commit` line with its `-dirty`. A change with no content to draw is titled in the user's
//! words (2026-10-03): "Mode changed", "Renamed without changes", "Renamed, mode changed",
//! and a copy's alike. Fork's words where Fork's are recorded: "Changes are too large to
//! display" and "Load Diff" (Finding 21, Windows), and "Old" and "New" over a binary's two
//! sizes (Finding 22). [`DiffNotice::of`] reads a [`ShownDiff`] by naming every state, so a
//! ninth state does not compile until it is given a notice.

use cairn_model::{ChangeStatus, DiffContent, DiffLimits, FileMode, Oid, ShownDiff, SizeLimit};
use freya::prelude::*;

use crate::diff_palette::{
    ADDED_EMPHASIS, DIFF_FONT_FAMILY, DIFF_FONT_SIZE, DIFF_MUTED, DIFF_TEXT, GROUND,
    REMOVED_EMPHASIS,
};

/// Fork's words for a file past the limits (Finding 21, Windows).
pub const TOO_LARGE_TO_DISPLAY: &str = "Changes are too large to display";
/// Fork's button, which asks for the file past the limits (Finding 21, Windows).
pub const LOAD_DIFF_CAPTION: &str = "Load Diff";
/// Said over a binary file's two sizes.
pub const BINARY_FILE: &str = "Binary file";
/// The label over an LFS pointer's text (R6.8).
pub const LFS_POINTER: &str = "Git LFS pointer";
/// Said over a submodule's two commits.
pub const SUBMODULE: &str = "Submodule";
/// Said over git's header lines for a change with no content to draw that is neither a
/// change of mode nor a rename or a copy — which git prints no such lines for, so no answer
/// the engine gives reaches it today; named rather than left to fall through to another
/// state's words.
pub const NO_CONTENT_CHANGE: &str = "No change to the file's content";
/// Said over git's rename lines for a rename with no content or mode change (the user's
/// words, 2026-10-03).
pub const RENAMED_WITHOUT_CHANGES: &str = "Renamed without changes";
/// Said over git's copy lines for a copy with no content or mode change, in the rename's form.
pub const COPIED_WITHOUT_CHANGES: &str = "Copied without changes";
/// Said over git's mode lines for a change of mode alone (the user's words, 2026-10-03).
pub const MODE_CHANGED: &str = "Mode changed";
/// Said over git's mode and rename lines for a rename with no content change whose mode moved
/// (the user's words, 2026-10-03).
pub const RENAMED_MODE_CHANGED: &str = "Renamed, mode changed";
/// Said over git's mode and copy lines for a copy with no content change whose mode moved, in
/// the rename's form.
pub const COPIED_MODE_CHANGED: &str = "Copied, mode changed";
/// Said for a conflicted path (R3.4), in the user's words (2026-10-03): git's own "Unmerged
/// path", and why no diff is drawn.
pub const CONFLICTED: &str =
    "Unmerged path — conflicts must be resolved before a diff can be shown";
/// Said for a file whose diff has no change to show and hides none: a working-tree path git
/// shows nothing for, or a file whose content did not change.
pub const NO_CHANGES_SHOWN: &str = "No changes to show.";
/// Said for a file whose every change ignoring whitespace hides (R6.7).
pub const ONLY_WHITESPACE_CHANGED: &str =
    "Every change to this file is whitespace, which is being ignored.";
/// Fork's labels over the two sides of a binary (Finding 22).
pub const OLD_SIDE: &str = "Old";
pub const NEW_SIDE: &str = "New";

/// What stands in place of a file's rows, read once from its answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffNotice {
    Binary {
        old: Option<u64>,
        new: Option<u64>,
    },
    TooLarge {
        crossed: SizeLimit,
        loadable: bool,
    },
    LfsPointer {
        old: Option<String>,
        new: Option<String>,
    },
    Submodule {
        old: Option<Oid>,
        new: Option<Oid>,
        dirty: bool,
    },
    /// git's own header lines for a change with no content to draw: a mode change, a rename
    /// or copy with no content change, or both — under `title`.
    NoContentChange {
        title: &'static str,
        lines: Vec<String>,
    },
    Conflicted,
    Unsupported {
        reason: String,
    },
    /// Text, with nothing to draw: every change whitespace being ignored, or none at all.
    NothingShown {
        hides_changes: bool,
    },
}

impl DiffNotice {
    /// What stands in place of `shown`'s rows, or `None` when it has rows to draw.
    pub fn of(shown: &ShownDiff) -> Option<Self> {
        let diff = shown.diff();
        let file = &diff.file;
        let present = |status_absent: bool, value: u64| (!status_absent).then_some(value);
        let added = file.status == ChangeStatus::Added;
        let deleted = file.status == ChangeStatus::Deleted;
        match &diff.content {
            DiffContent::Text { .. } if shown.row_count() > 0 => None,
            DiffContent::Text { .. } => Some(match header_lines(file) {
                lines if !lines.is_empty() && !shown.hides_changes() => Self::NoContentChange {
                    title: no_content_title(file),
                    lines,
                },
                _ => Self::NothingShown {
                    hides_changes: shown.hides_changes(),
                },
            }),
            DiffContent::Binary { old_size, new_size } => Some(Self::Binary {
                old: present(added, *old_size),
                new: present(deleted, *new_size),
            }),
            DiffContent::TooLarge { crossed, loadable } => Some(Self::TooLarge {
                crossed: *crossed,
                loadable: *loadable,
            }),
            DiffContent::LfsPointer { old, new } => Some(Self::LfsPointer {
                old: old.clone(),
                new: new.clone(),
            }),
            DiffContent::Submodule {
                old_target,
                new_target,
                dirty,
            } => Some(Self::Submodule {
                old: *old_target,
                new: *new_target,
                dirty: *dirty,
            }),
            DiffContent::ModeChangeOnly => Some(Self::NoContentChange {
                title: no_content_title(file),
                lines: header_lines(file),
            }),
            DiffContent::Conflicted => Some(Self::Conflicted),
            DiffContent::Unsupported { reason } => Some(Self::Unsupported {
                reason: reason.clone(),
            }),
        }
    }
}

/// How a line of a notice is coloured: a side's own colour, or the diff's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeTone {
    Old,
    New,
    Plain,
}

/// One row of a notice where it must be drawn as rows of one height: under a file opened in
/// place in the Commit tab (R5.3), whose list is one virtualised list of equal rows. The same
/// words [`DiffNoticeView`] draws, from the same [`DiffNotice`], one line each
/// (`every_notice_says_the_same_words_in_place_as_in_the_changes_tab`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeRow {
    /// What the state is.
    Title(String),
    /// Muted words under it: a size, a reason, nothing shown.
    Muted(String),
    /// A side's caption, Fork's "Old" or "New", and what that side holds when it is one line.
    Side {
        caption: &'static str,
        tone: NoticeTone,
        text: Option<String>,
    },
    /// A line git prints, in the diff's typeface.
    Git { text: String, tone: NoticeTone },
    /// Load Diff, for a file past the limits that can be loaded.
    LoadDiff,
}

/// `notice` as rows, in the order [`DiffNoticeView`] draws its words. One match naming every
/// state, so a state added does not compile until it is given rows.
pub fn notice_rows(notice: &DiffNotice) -> Vec<NoticeRow> {
    let title = |text: &str| NoticeRow::Title(text.to_owned());
    let mut rows = Vec::new();
    match notice {
        DiffNotice::Binary { old, new } => {
            rows.push(title(BINARY_FILE));
            for (caption, tone, size) in [
                (OLD_SIDE, NoticeTone::Old, old),
                (NEW_SIDE, NoticeTone::New, new),
            ] {
                if let Some(size) = size {
                    rows.push(NoticeRow::Side {
                        caption,
                        tone,
                        text: Some(size_text(*size)),
                    });
                }
            }
        }
        DiffNotice::TooLarge { crossed, loadable } => {
            rows.push(title(TOO_LARGE_TO_DISPLAY));
            rows.push(NoticeRow::Muted(too_large_reason(*crossed, *loadable)));
            if *loadable {
                rows.push(NoticeRow::LoadDiff);
            }
        }
        DiffNotice::LfsPointer { old, new } => {
            rows.push(title(LFS_POINTER));
            for (caption, tone, pointer) in [
                (OLD_SIDE, NoticeTone::Old, old),
                (NEW_SIDE, NoticeTone::New, new),
            ] {
                if let Some(pointer) = pointer {
                    rows.push(NoticeRow::Side {
                        caption,
                        tone,
                        text: None,
                    });
                    rows.extend(pointer.lines().map(|line| NoticeRow::Git {
                        text: line.to_owned(),
                        tone: NoticeTone::Plain,
                    }));
                }
            }
        }
        DiffNotice::Submodule { old, new, dirty } => {
            rows.push(title(SUBMODULE));
            if let Some(id) = old {
                rows.push(NoticeRow::Git {
                    text: format!("-{}", subproject_line(*id, false)),
                    tone: NoticeTone::Old,
                });
            }
            if let Some(id) = new {
                rows.push(NoticeRow::Git {
                    text: format!("+{}", subproject_line(*id, *dirty)),
                    tone: NoticeTone::New,
                });
            }
        }
        DiffNotice::NoContentChange {
            title: words,
            lines,
        } => {
            rows.push(title(words));
            rows.extend(lines.iter().map(|line| NoticeRow::Git {
                text: line.clone(),
                tone: NoticeTone::Plain,
            }));
        }
        DiffNotice::Conflicted => rows.push(title(CONFLICTED)),
        DiffNotice::Unsupported { reason } => rows.push(title(reason)),
        DiffNotice::NothingShown { hides_changes } => rows.push(NoticeRow::Muted(
            if *hides_changes {
                ONLY_WHITESPACE_CHANGED
            } else {
                NO_CHANGES_SHOWN
            }
            .to_owned(),
        )),
    }
    rows
}

/// git's extended header lines for `file` — `old mode`/`new mode`, then `similarity index`
/// and `rename from`/`rename to` (or `copy from`/`copy to`) — as `git diff` prints them;
/// none for a file that is neither renamed, copied nor changed in mode.
pub fn header_lines(file: &cairn_model::ChangedFile) -> Vec<String> {
    let mut lines = Vec::new();
    if file.mode_changed()
        && let (Some(old), Some(new)) = (file.old_mode, file.new_mode)
    {
        lines.push(format!("old mode {}", mode_digits(old)));
        lines.push(format!("new mode {}", mode_digits(new)));
    }
    let (word, similarity) = match file.status {
        ChangeStatus::Renamed(similarity) => ("rename", similarity),
        ChangeStatus::Copied(similarity) => ("copy", similarity),
        ChangeStatus::Added
        | ChangeStatus::Deleted
        | ChangeStatus::Modified
        | ChangeStatus::TypeChanged => return lines,
    };
    lines.push(format!("similarity index {}%", similarity.percent()));
    lines.push(format!("{word} from {}", file.old_path.display()));
    lines.push(format!("{word} to {}", file.new_path.display()));
    lines
}

/// What a change with no content to draw is called (the user's words, 2026-10-03): a change
/// of mode alone is "Mode changed"; a rename or a copy is renamed or copied "without changes",
/// or ", mode changed" when its mode moved too; anything else has no change to its content.
fn no_content_title(file: &cairn_model::ChangedFile) -> &'static str {
    match (file.status, file.mode_changed()) {
        (ChangeStatus::Renamed(_), false) => RENAMED_WITHOUT_CHANGES,
        (ChangeStatus::Renamed(_), true) => RENAMED_MODE_CHANGED,
        (ChangeStatus::Copied(_), false) => COPIED_WITHOUT_CHANGES,
        (ChangeStatus::Copied(_), true) => COPIED_MODE_CHANGED,
        (
            ChangeStatus::Added
            | ChangeStatus::Deleted
            | ChangeStatus::Modified
            | ChangeStatus::TypeChanged,
            true,
        ) => MODE_CHANGED,
        (
            ChangeStatus::Added
            | ChangeStatus::Deleted
            | ChangeStatus::Modified
            | ChangeStatus::TypeChanged,
            false,
        ) => NO_CONTENT_CHANGE,
    }
}

fn mode_digits(mode: FileMode) -> String {
    mode.octal().to_owned()
}

/// A size as Fork shows one, in kilobytes and in bytes (Finding 22); under a kilobyte, in
/// bytes alone. Fork's sample does not settle whether its KB is 1,000 or 1,024 bytes, so the
/// unit is said honestly: KiB, 1,024 bytes (the user's decision, 2026-10-03).
pub fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{} bytes", grouped(bytes));
    }
    format!(
        "{:.1} KiB ({} bytes)",
        bytes as f64 / 1024.0,
        grouped(bytes)
    )
}

/// `n` with its thousands grouped by commas.
pub(crate) fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A size in mebibytes as a notice says one: whole where it is whole (`64 MiB`), else to a
/// tenth (`72.3 MiB`), as [`size_text`] gives kibibytes — and where a tenth would round a size
/// that is not whole to one that is (`64.0 MiB` for 64 MiB and a byte), to as many more places
/// as it takes to read as what it is, seven at most (`64.000001 MiB`, `63.999999 MiB`), trailing
/// zeros dropped. Counted in whole numbers, so no size is misread by a float.
pub fn mib_text(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    let (whole, part) = (bytes / MIB, bytes % MIB);
    if part == 0 {
        return format!("{whole} MiB");
    }
    // Seven places always tell a part of 1 to MIB - 1 bytes from whole: the rounding moves the
    // fraction by at most 5e-8, and a byte is about 9.5e-7 of a MiB.
    let mut places = 1u32;
    let fraction = loop {
        let scale = 10u64.pow(places);
        let rounded = (part * scale + MIB / 2) / MIB;
        if (rounded != 0 && rounded != scale) || places == 7 {
            break rounded;
        }
        places += 1;
    };
    let digits = format!("{fraction:0width$}", width = places as usize);
    format!("{whole}.{} MiB", digits.trim_end_matches('0'))
}

/// Why a file is too large, in the measurement the limit fired on, and whether it can be
/// loaded anyway. Past the load-anyway ceiling it says the file's size and the ceiling (the
/// user's words, 2026-10-03): "72.3 MiB — larger than the 64 MiB Cairn can load". The
/// ceiling is [`DiffLimits::LOAD_ANYWAY_BYTES`], the one the application asks every file diff
/// with — R2.6's ceilings are fixed — and not the limit the engine names, which on a first ask
/// is the drawing limit (1 MiB) the file crossed first.
pub fn too_large_reason(crossed: SizeLimit, loadable: bool) -> String {
    if let (SizeLimit::Bytes { measured, .. }, false) = (crossed, loadable) {
        return format!(
            "{} — larger than the {} Cairn can load",
            mib_text(measured),
            mib_text(DiffLimits::LOAD_ANYWAY_BYTES)
        );
    }
    let why = match crossed {
        SizeLimit::Bytes { limit, measured } => format!(
            "{} bytes, more than the limit of {} bytes",
            grouped(measured),
            grouped(limit)
        ),
        SizeLimit::Lines { limit, measured } => format!(
            "{} lines, more than the limit of {} lines",
            grouped(u64::from(measured)),
            grouped(u64::from(limit))
        ),
        SizeLimit::LineLength { limit, measured } => format!(
            "a line of {} bytes, longer than the limit of {} bytes",
            grouped(u64::from(measured)),
            grouped(u64::from(limit))
        ),
    };
    if loadable {
        why
    } else {
        format!("{why}; too large to load")
    }
}

/// git's line for a submodule's commit, as `git diff` prints it, with `-dirty` where the
/// working tree's checkout has changes of its own.
pub fn subproject_line(id: Oid, dirty: bool) -> String {
    format!(
        "Subproject commit {}{}",
        id.hex().as_str(),
        if dirty { "-dirty" } else { "" }
    )
}

/// The notice drawn in place of a file's rows. Load Diff, offered only for a file that can be
/// loaded, reports through [`DiffNoticeView::on_load`]; what it asks is the caller's.
pub struct DiffNoticeView {
    notice: DiffNotice,
    on_load: EventHandler<()>,
    key: DiffKey,
}

impl DiffNoticeView {
    pub fn new(notice: DiffNotice) -> Self {
        Self {
            notice,
            on_load: EventHandler::new(|_| {}),
            key: DiffKey::None,
        }
    }

    pub fn on_load(mut self, on_load: impl Into<EventHandler<()>>) -> Self {
        self.on_load = on_load.into();
        self
    }
}

impl PartialEq for DiffNoticeView {
    fn eq(&self, other: &Self) -> bool {
        self.notice == other.notice && self.key == other.key
    }
}

impl std::fmt::Debug for DiffNoticeView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffNoticeView")
            .field("notice", &self.notice)
            .finish_non_exhaustive()
    }
}

impl KeyExt for DiffNoticeView {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

fn title(text: impl Into<String>) -> Label {
    label().text(text.into()).font_size(13.).color(DIFF_TEXT)
}

fn muted(text: impl Into<String>) -> Label {
    label().text(text.into()).font_size(12.).color(DIFF_MUTED)
}

/// A line git prints, in the diff's typeface.
fn git_line(text: impl Into<String>, colour: Color) -> Label {
    label()
        .text(text.into())
        .max_lines(1)
        .font_family(DIFF_FONT_FAMILY)
        .font_size(DIFF_FONT_SIZE)
        .color(colour)
}

/// A side's caption — Fork's "Old" in red, "New" in green — over what that side holds.
fn side(caption: &'static str, colour: Color, lines: Vec<Label>) -> Rect {
    rect()
        .spacing(4.)
        .child(label().text(caption).font_size(12.).color(colour))
        .children(lines.into_iter().map(Element::from))
}

impl Component for DiffNoticeView {
    fn render(&self) -> impl IntoElement {
        let body = rect().center().spacing(8.);
        let body = match &self.notice {
            DiffNotice::Binary { old, new } => {
                body.child(title(BINARY_FILE)).child(
                    rect()
                        .horizontal()
                        .spacing(32.)
                        .maybe_child(old.map(|size| {
                            side(OLD_SIDE, REMOVED_EMPHASIS, vec![muted(size_text(size))])
                        }))
                        .maybe_child(new.map(|size| {
                            side(NEW_SIDE, ADDED_EMPHASIS, vec![muted(size_text(size))])
                        })),
                )
            }
            DiffNotice::TooLarge { crossed, loadable } => {
                let on_load = self.on_load.clone();
                body.child(title(TOO_LARGE_TO_DISPLAY))
                    .child(muted(too_large_reason(*crossed, *loadable)))
                    .maybe_child(loadable.then(|| {
                        Button::new()
                            .compact()
                            .on_press(move |_| on_load.call(()))
                            .child(LOAD_DIFF_CAPTION)
                    }))
            }
            DiffNotice::LfsPointer { old, new } => {
                let pointer = |text: &str| -> Vec<Label> {
                    text.lines()
                        .map(|line| git_line(line.to_owned(), DIFF_TEXT))
                        .collect()
                };
                // One side above the other: a pointer's `oid` line is wider than half a pane.
                body.child(title(LFS_POINTER)).child(
                    rect()
                        .spacing(12.)
                        .maybe_child(
                            old.as_deref()
                                .map(|text| side(OLD_SIDE, REMOVED_EMPHASIS, pointer(text))),
                        )
                        .maybe_child(
                            new.as_deref()
                                .map(|text| side(NEW_SIDE, ADDED_EMPHASIS, pointer(text))),
                        ),
                )
            }
            DiffNotice::Submodule { old, new, dirty } => body.child(title(SUBMODULE)).child(
                rect()
                    .spacing(4.)
                    .maybe_child(old.map(|id| {
                        git_line(format!("-{}", subproject_line(id, false)), REMOVED_EMPHASIS)
                    }))
                    .maybe_child(new.map(|id| {
                        git_line(format!("+{}", subproject_line(id, *dirty)), ADDED_EMPHASIS)
                    })),
            ),
            DiffNotice::NoContentChange {
                title: words,
                lines,
            } => body.child(title(*words)).child(
                rect().spacing(2.).children(
                    lines
                        .iter()
                        .map(|line| Element::from(git_line(line.clone(), DIFF_TEXT))),
                ),
            ),
            DiffNotice::Conflicted => body.child(title(CONFLICTED)),
            DiffNotice::Unsupported { reason } => body.child(title(reason.clone())),
            DiffNotice::NothingShown { hides_changes } => body.child(muted(if *hides_changes {
                ONLY_WHITESPACE_CHANGED
            } else {
                NO_CHANGES_SHOWN
            })),
        };
        rect()
            .expanded()
            .center()
            .padding(12.)
            .background(GROUND)
            .child(body)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_reasons_read_as_numbers_with_their_thousands_grouped() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(2_532_736), "2,532,736");
        assert_eq!(size_text(2_048), "2.0 KiB (2,048 bytes)");
        assert_eq!(size_text(4), "4 bytes");
        assert_eq!(
            too_large_reason(
                SizeLimit::Bytes {
                    limit: 1_048_576,
                    measured: 2_532_736
                },
                true
            ),
            "2,532,736 bytes, more than the limit of 1,048,576 bytes"
        );
        // Past the ceiling, in MiB as the user worded it (2026-10-03); the engine's first ask
        // names the drawing limit, which the sentence does not repeat.
        assert_eq!(mib_text(64 * 1024 * 1024), "64 MiB");
        assert_eq!(mib_text(75_812_045), "72.3 MiB");
        assert_eq!(
            too_large_reason(
                SizeLimit::Bytes {
                    limit: 1_048_576,
                    measured: 75_812_045
                },
                false
            ),
            "72.3 MiB — larger than the 64 MiB Cairn can load"
        );
        assert!(
            too_large_reason(
                SizeLimit::Lines {
                    limit: 50_000,
                    measured: 60_000
                },
                false
            )
            .ends_with("too large to load")
        );
    }

    /// Phase 05's obligation carried into phase 08: a file just past the 64 MiB ceiling never
    /// reads as the ceiling itself. A tenth is kept wherever it says the size honestly; where
    /// it would round to a whole number of MiB — "64.0 MiB" for 64 MiB and one byte — the
    /// size is given to as many places as it takes to read as more (or less) than whole.
    /// Caught by: rounding to a tenth unconditionally ("64.0 MiB — larger than the 64 MiB
    /// Cairn can load"), or dropping the fraction ("64 MiB").
    #[test]
    fn a_size_just_past_the_ceiling_never_reads_as_the_ceiling() {
        const MIB: u64 = 1024 * 1024;
        let ceiling = DiffLimits::LOAD_ANYWAY_BYTES;
        assert_eq!(mib_text(ceiling + 1), "64.000001 MiB");
        assert_eq!(mib_text(ceiling + 100), "64.0001 MiB");
        assert_eq!(mib_text(ceiling + MIB / 20), "64.05 MiB");
        assert_eq!(mib_text(ceiling + MIB / 10), "64.1 MiB");
        assert_eq!(mib_text(64 * MIB - 1), "63.999999 MiB");
        for measured in [
            ceiling + 1,
            ceiling + 7,
            ceiling + 1_000,
            ceiling + MIB / 21,
        ] {
            let said = too_large_reason(
                SizeLimit::Bytes {
                    limit: 1_048_576,
                    measured,
                },
                false,
            );
            let size = said.split(" MiB").next().unwrap_or_default();
            assert!(
                size.parse::<f64>().is_ok_and(|read| read > 64.0),
                "{measured} bytes read as {said:?}"
            );
        }
        // Whole numbers throughout: a byte past a tebibyte is still told from whole.
        assert_eq!(mib_text((1 << 40) | 1), "1048576.000001 MiB");
        assert_eq!(
            mib_text(75_812_045),
            "72.3 MiB",
            "a tenth wherever it is honest"
        );
    }
}
