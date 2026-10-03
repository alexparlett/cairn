//! What stands where a file's rows would be (PRD R6.8, R6.9): the states of R1.2 that are
//! not text, and a text diff with no row to draw.
//!
//! Each state says what it is in words that cannot be mistaken for another — a submodule's
//! notice names commits, never "binary" — and quotes git where git has words for it: a mode
//! change is git's own `old mode` and `new mode` lines, a rename with no content change git's
//! `similarity index`, `rename from` and `rename to` lines, a submodule git's `Subproject
//! commit` line with its `-dirty`. Fork's words where Fork's are recorded: "Changes are too
//! large to display" and "Load Diff" (Finding 21, Windows), and "Old" and "New" over a
//! binary's two sizes (Finding 22). [`DiffNotice::of`] reads a [`ShownDiff`] by naming every
//! state, so a ninth state does not compile until it is given a notice.

use cairn_model::{ChangeStatus, DiffContent, FileMode, Oid, ShownDiff, SizeLimit};
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
/// Said over git's mode lines, for a change with no content to draw (and over a rename's
/// lines when its mode moved too).
pub const NO_CONTENT_CHANGE: &str = "No change to the file's content";
/// Said over git's rename lines for a rename with no content or mode change (the user's
/// words, 2026-10-03).
pub const RENAMED_WITHOUT_CHANGES: &str = "Renamed without changes";
/// Said over git's copy lines for a copy with no content or mode change, in the rename's form.
pub const COPIED_WITHOUT_CHANGES: &str = "Copied without changes";
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

/// What a change with no content to draw is called: a rename or a copy whose mode did not
/// move either is renamed or copied "without changes"; anything else has no change to its
/// content.
fn no_content_title(file: &cairn_model::ChangedFile) -> &'static str {
    if file.mode_changed() {
        return NO_CONTENT_CHANGE;
    }
    match file.status {
        ChangeStatus::Renamed(_) => RENAMED_WITHOUT_CHANGES,
        ChangeStatus::Copied(_) => COPIED_WITHOUT_CHANGES,
        ChangeStatus::Added
        | ChangeStatus::Deleted
        | ChangeStatus::Modified
        | ChangeStatus::TypeChanged => NO_CONTENT_CHANGE,
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

/// Why a file is too large, in the measurement the limit fired on, and whether it can be
/// loaded anyway.
pub fn too_large_reason(crossed: SizeLimit, loadable: bool) -> String {
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
}
