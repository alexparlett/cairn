//! What a destructive operation will destroy, and the words a person is shown
//! for it.
//!
//! A [`Consequence`] is computed by the engine from the repository as it is,
//! and the prompt and the button's label are rendered from it here, beside the
//! type, so no text that names a count or a path can be typed apart from the
//! value it describes. [`crate::Confirmed`] carries one, with the prompt
//! rendered from it, and every destructive operation re-reads the state it
//! names immediately before it runs, refusing when anything moved
//! (`docs/prd/staging-and-commit.md` R1). An operation derives every target —
//! each path, each line — from the `Consequence` it was confirmed with, never
//! from a parameter beside it.
//!
//! Every count and path a prompt renders is read from the value: nothing here
//! looks at a repository, a clock or a setting, so the same `Consequence`
//! always renders the same words. A path or a subject is text a repository's
//! author chose, so a control character, a line separator or a bidirectional
//! override in it is escaped before it is drawn: it cannot rewrite the prompt
//! around it.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::{FileMode, Oid, Patch, RefName, RepoPath, Selection};

/// What one destructive operation will destroy, as the engine read it.
///
/// One variant per destructive operation of R1.5. Discarding files and
/// deleting untracked files share [`Consequence::DiscardFiles`]: Fork confirms
/// a selection of both in one dialog, with one prompt naming each kind
/// (`brainstorm.md` L8), and one prompt is one confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consequence {
    /// Selected lines of one path's unstaged change, and its mode change when
    /// that is selected too, put back to the index's version (or, for an
    /// untracked file, removed from it) in the working tree. Never a selection
    /// of nothing: the engine refuses to build one.
    DiscardLines {
        path: RepoPath,
        /// The index entry's blob the lines were computed against; `None` for
        /// an untracked file, which has no entry, or an intent-to-add one.
        index: Option<Oid>,
        /// The working-tree file in git's form — through its clean filter and
        /// line-ending conversion, as `git hash-object --path` hashes it — the
        /// side the lines are taken from.
        working_tree: Oid,
        /// The working-tree file's bytes as they are on disk, hashed with no
        /// filter (a symlink as its target, as git stores one): what the
        /// re-check compares, so an edit git's form does not show — line
        /// endings alone — still refuses.
        on_disk: Oid,
        /// Whether the working-tree file is executable, as its mode is on disk: with
        /// `on_disk`, what the re-check compares, so a `chmod` after the confirmation
        /// refuses rather than being silently undone.
        executable: bool,
        /// The lines selected in the unstaged diff (index to working tree):
        /// its added lines are deleted, its removed lines put back. The patch
        /// the operation applies is built from exactly this selection.
        selection: Selection,
        /// The mode change discarded, as the diff draws it: the index's mode,
        /// then the working tree's, which the discard puts back to the first.
        /// Present exactly when the selection holds the mode.
        mode: Option<(FileMode, FileMode)>,
        /// The patch the discard applies, emitted from the diff the user confirmed
        /// with this selection: what runs is what was confirmed, never a patch built
        /// again from a later diff, whose lines could align otherwise.
        patch: Patch,
    },
    /// Whole files: each tracked file's unstaged change restored from the
    /// index, each untracked file deleted. Never empty when the engine builds
    /// it.
    DiscardFiles { files: Vec<DiscardedFile> },
    /// `HEAD` replaced by an amended commit.
    Amend {
        commit: Oid,
        /// The replaced commit's subject, as the history draws it.
        subject: String,
        published: Publication,
        /// Whether git will record the move in a reflog, which is where Show Lost
        /// Commits finds the replaced commit.
        reflog: Reflog,
    },
    /// A lock file removed by Cairn itself, since git has no verb for it.
    RemoveLock {
        /// The lock's absolute path, `<gitdir>/index.lock`.
        path: PathBuf,
        /// Its modification time, when it was read.
        modified: SystemTime,
        /// When it was read; the age a prompt names is `read_at - modified`,
        /// so the renderer looks at no clock.
        read_at: SystemTime,
        /// Its size when it was read.
        bytes: u64,
        /// The device and inode it was read on: with `modified` and `bytes`,
        /// what the re-check compares, so a lock removed and made again since
        /// is refused even inside one timestamp tick.
        device: u64,
        inode: u64,
    },
    /// A branch created at a commit and checked out with git's `checkout -f`: every staged
    /// and unstaged change to a tracked file discarded, and every untracked file the commit's
    /// tree holds a file at overwritten (staging-and-commit R11.3; the user's decision 3,
    /// 2026-10-09 — Create Branch's "Discard", the one exception to "a staged change is never
    /// discarded"). Never empty when the engine builds it.
    CheckoutDiscarding {
        /// The branch created, as typed.
        branch: String,
        /// The commit it is created at, and checked out.
        at: Oid,
        /// `HEAD`'s commit when this was read: the changes are counted against it, and the
        /// re-check holds it.
        head: Option<Oid>,
        changes: Vec<LostChange>,
        /// Untracked files the checkout leaves where they are, said so in the prompt.
        kept_untracked: usize,
    },
}

/// One path a [`Consequence::CheckoutDiscarding`] loses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostChange {
    pub path: RepoPath,
    pub loss: ChangeLoss,
}

/// What a checkout that discards loses at one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeLoss {
    /// A tracked file's staged and unstaged changes since `HEAD`.
    Changed {
        kind: ChangedKind,
        /// The index entry's blob as read; `None` where the index holds none.
        index: Option<Oid>,
        /// The file's bytes on disk, hashed with no filter (a symlink as its target); `None`
        /// where it is not there.
        working_tree: Option<Oid>,
        /// Whether it is executable on disk; compared by the re-check.
        executable: bool,
        /// Changed lines of its staged diff and its unstaged diff together; `None` where
        /// either is not text.
        lines: Option<usize>,
    },
    /// An untracked file at a path the checked-out commit holds a file at: git's `checkout
    /// -f` writes the commit's file over it.
    Overwritten {
        /// The file's bytes on disk, hashed with no filter.
        working_tree: Oid,
        executable: bool,
        bytes: u64,
    },
    /// Untracked content at a path the checked-out commit holds a file at — the path itself,
    /// or a directory on the way to it — which git's `checkout -f` deletes whole with
    /// everything under it: the files counted, and their bytes.
    Removed {
        kind: RemovedKind,
        files: usize,
        bytes: u64,
    },
}

/// What untracked content a checkout that discards deletes whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovedKind {
    /// A directory of untracked files.
    Directory,
    /// A repository nested in the working tree, its own `.git` among what is deleted.
    Repository,
}

/// How a tracked path differs from `HEAD`, for the prompt's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangedKind {
    /// In `HEAD` and still there, changed.
    Modified,
    /// Not in `HEAD`: staged as a new file, which the checkout deletes.
    Added,
    /// In `HEAD`, deleted in the index or the working tree.
    Deleted,
}

/// One file a [`Consequence::DiscardFiles`] destroys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscardedFile {
    pub path: RepoPath,
    pub loss: FileLoss,
}

/// What is lost of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileLoss {
    /// A tracked file's unstaged change, the working tree restored from the
    /// index.
    Modified {
        /// The index entry's blob, what the file becomes.
        index: Oid,
        /// The working-tree file's bytes as they are on disk, hashed with no
        /// filter (a symlink as its target); `None` when it is deleted in the
        /// working tree, and discarding brings it back. What the re-check
        /// compares, so any byte changed after the confirmation — a line ending
        /// included — refuses.
        working_tree: Option<Oid>,
        /// Whether the working-tree file is executable, as its mode is on disk (`false`
        /// when it is deleted): compared by the re-check with `working_tree`, so a
        /// `chmod` after the confirmation refuses rather than being silently undone.
        executable: bool,
        /// Changed lines of the unstaged diff; `None` for a change that has no
        /// lines (binary).
        lines: Option<usize>,
        /// The mode change the restore takes back, as the diff draws it: the
        /// index's mode, then the working tree's. Named in the prompt with both
        /// modes beside the lines (the user's decision of 2026-10-08), so a file
        /// whose only change is its mode never reads as "0 lines".
        mode: Option<(FileMode, FileMode)>,
    },
    /// A file added with `git add -N`, whose index entry is the empty blob:
    /// `git restore` writes that entry back, so the discard leaves the file
    /// EMPTY and the entry in place, as the user's own `git restore` does (the
    /// user's decision of 2026-10-08), and the prompt names it as emptied.
    Emptied {
        /// The intent-to-add entry's blob, the empty one, what the file becomes.
        index: Oid,
        /// The working-tree file's bytes as they are on disk, hashed with no
        /// filter (a symlink as its target).
        working_tree: Oid,
        /// Whether it is executable, as its mode is on disk; compared by the re-check.
        executable: bool,
        /// Its lines, every one added; `None` for a file that is not text.
        lines: Option<usize>,
    },
    /// An untracked file, deleted.
    Untracked {
        /// The file's bytes as they are on disk, hashed with no filter (a
        /// symlink as its target).
        working_tree: Oid,
        /// Whether it is executable, as its mode is on disk; compared by the re-check.
        executable: bool,
        bytes: u64,
    },
}

/// Whether git records an amend's move of the branch and `HEAD` in a reflog —
/// and so whether the replaced commit can be found again afterwards (R10.6,
/// decided by the user on 2026-10-08).
///
/// git writes the entry when `core.logAllRefUpdates` is `true` or `always` —
/// `true` by default in a repository with a working tree, `false` in a bare one
/// — or, whatever it is set to, when the ref's log file already exists, since
/// git appends to an existing log (checked with git 2.56: an amend under
/// `false` from the start writes no `.git/logs`, and under `false` set after
/// the logs exist still appends to them). The engine reads which (phase 05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reflog {
    /// git will write the entry: the replaced commit stays findable.
    Written,
    /// git will write none: nothing in the repository points at the replaced
    /// commit afterwards.
    NotWritten,
}

/// Whether a remote already has the commit an amend replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Publication {
    /// No remote-tracking ref reaches it.
    Unpublished,
    /// The current branch's upstream has it.
    Upstream(RefName),
    /// With no upstream that has it, some remote-tracking ref reaches it.
    SomeRemote,
}

impl Consequence {
    /// The prompt a person is shown, and the text a [`crate::Confirmed`]
    /// records: every count and path in it read from this value.
    pub fn prompt(&self) -> String {
        match self {
            Self::DiscardLines {
                path,
                index: _,
                working_tree: _,
                on_disk: _,
                executable: _,
                selection,
                mode,
                patch: _,
            } => format!(
                "Do you want to discard {} in {}? You can't undo this action.",
                discarded_lines(selection.len(), *mode),
                quoted(path.as_bytes())
            ),
            Self::DiscardFiles { files } => discard_files_prompt(files),
            Self::CheckoutDiscarding {
                branch,
                at,
                head: _,
                changes,
                kept_untracked,
            } => checkout_discarding_prompt(branch, *at, changes, *kept_untracked),
            Self::Amend {
                commit,
                subject,
                published,
                reflog,
            } => {
                let replaces = amend_replaces(*commit, subject, *reflog);
                match amend_force_push(*commit, published) {
                    Some(warning) => format!("{warning} {replaces}"),
                    None => replaces,
                }
            }
            Self::RemoveLock {
                path,
                modified,
                read_at,
                bytes,
                device: _,
                inode: _,
            } => format!(
                "Remove {}? It was last changed {} ago and holds {}. Another program may still \
                 own it: removing a lock a running git holds can corrupt the repository.",
                quoted(path.as_os_str().as_encoded_bytes()),
                elapsed(read_at.duration_since(*modified).unwrap_or(Duration::ZERO)),
                size(*bytes)
            ),
        }
    }

    /// An amend's first sentences, when a remote already has the commit it replaces (R10.6):
    /// "<short id> is already on <remote ref>. Sharing the amended commit needs a force push."
    /// — what the dialog asks first. `None` for an amend no remote has, and for every other
    /// operation. [`Consequence::prompt`] is this, then [`Consequence::replaces`], so a
    /// surface that draws the two apart draws exactly the words the token records
    /// (staging-and-commit phase 01's QA item 21).
    pub fn force_push_warning(&self) -> Option<String> {
        match self {
            Self::Amend {
                commit,
                subject: _,
                published,
                reflog: _,
            } => amend_force_push(*commit, published),
            Self::DiscardLines { .. }
            | Self::DiscardFiles { .. }
            | Self::RemoveLock { .. }
            | Self::CheckoutDiscarding { .. } => None,
        }
    }

    /// The commit an amend replaces: what its prompt names and its re-check holds `HEAD` to.
    /// `None` for every other operation.
    pub fn amended(&self) -> Option<Oid> {
        match self {
            Self::Amend {
                commit,
                subject: _,
                published: _,
                reflog: _,
            } => Some(*commit),
            Self::DiscardLines { .. }
            | Self::DiscardFiles { .. }
            | Self::RemoveLock { .. }
            | Self::CheckoutDiscarding { .. } => None,
        }
    }

    /// Whether confirming this needs the dialog first: an amend a remote already has, whose
    /// replacement needs a force push to share (R10.6, L12). Nothing else asks a dialog by
    /// this rule.
    pub fn needs_force_push(&self) -> bool {
        self.force_push_warning().is_some()
    }

    /// An amend's line under its button (R10.6): "Replaces <short id> '<subject>'.", then
    /// whether the old commit can be found again. `None` for every other operation.
    pub fn replaces(&self) -> Option<String> {
        match self {
            Self::Amend {
                commit,
                subject,
                published: _,
                reflog,
            } => Some(amend_replaces(*commit, subject, *reflog)),
            Self::DiscardLines { .. }
            | Self::DiscardFiles { .. }
            | Self::RemoveLock { .. }
            | Self::CheckoutDiscarding { .. } => None,
        }
    }

    /// The label of the button that confirms it.
    pub fn action(&self) -> String {
        match self {
            Self::DiscardLines {
                path: _,
                index: _,
                working_tree: _,
                on_disk: _,
                executable: _,
                selection,
                mode,
                patch: _,
            } => match (selection.len(), mode) {
                (0, Some(_)) => "Discard Mode Change".to_owned(),
                (lines, Some(_)) => {
                    format!(
                        "Discard {} and Mode Change",
                        counted(lines, "Line", "Lines")
                    )
                }
                (lines, None) => format!("Discard {}", counted(lines, "Line", "Lines")),
            },
            Self::DiscardFiles { files } => {
                format!(
                    "Discard Changes in {}",
                    counted(files.len(), "File", "Files")
                )
            }
            Self::Amend {
                commit,
                subject: _,
                published: _,
                reflog: _,
            } => format!("Amend {}", commit.short().as_str()),
            Self::CheckoutDiscarding { .. } => "Discard Changes and Check Out".to_owned(),
            Self::RemoveLock {
                path,
                modified: _,
                read_at: _,
                bytes: _,
                device: _,
                inode: _,
            } => format!(
                "Remove {}",
                quoted(
                    path.file_name()
                        .unwrap_or(path.as_os_str())
                        .as_encoded_bytes()
                )
            ),
        }
    }
}

/// R10.6's line under the amend button: the commit replaced and whether it can be found
/// again afterwards.
fn amend_replaces(commit: Oid, subject: &str, reflog: Reflog) -> String {
    let short = commit.short();
    let subject = escaped(subject.as_bytes());
    let afterwards = match reflog {
        Reflog::Written => "The old commit stays in Show Lost Commits.",
        Reflog::NotWritten => {
            "The old commit can't be recovered afterwards: this repository keeps no reflog."
        }
    };
    format!("Replaces {} '{subject}'. {afterwards}", short.as_str())
}

/// R10.6's dialog text, when a remote already has the commit an amend replaces.
fn amend_force_push(commit: Oid, published: &Publication) -> Option<String> {
    let short = commit.short();
    let short = short.as_str();
    match published {
        Publication::Unpublished => None,
        Publication::Upstream(upstream) => Some(format!(
            "{short} is already on {}. Sharing the amended commit needs a force push.",
            quoted(upstream.shorthand().as_bytes())
        )),
        Publication::SomeRemote => Some(format!(
            "{short} is already on a remote. Sharing the amended commit needs a force push."
        )),
    }
}

/// Fork's words (L8): "Do you want to discard the changes in 3 files (a.rs, b.rs
/// and c.rs)? 2 modified (14 lines), 1 untracked file deleted (2.1 KiB). You can't
/// undo this action." One file is named by its path, several by their first three
/// paths and how many more (the user's decision, 2026-10-09); a file deleted in the working
/// tree, which the discard brings back, is named as that.
fn discard_files_prompt(files: &[DiscardedFile]) -> String {
    let what = match files {
        [only] => quoted(only.path.as_bytes()),
        _ => format!(
            "{} ({})",
            counted(files.len(), "file", "files"),
            named(files)
        ),
    };
    let mut modified = Lines::default();
    let mut modes: Vec<(FileMode, FileMode)> = Vec::new();
    let mut emptied = Lines::default();
    let mut restored = 0usize;
    let mut untracked = 0usize;
    let mut untracked_bytes = 0u64;
    for file in files {
        match &file.loss {
            FileLoss::Modified {
                index: _,
                working_tree: None,
                executable: _,
                lines: _,
                mode: _,
            } => restored += 1,
            FileLoss::Modified {
                index: _,
                working_tree: Some(_),
                executable: _,
                lines,
                mode,
            } => {
                add_lines(&mut modified, *lines);
                modes.extend(*mode);
            }
            FileLoss::Emptied {
                index: _,
                working_tree: _,
                executable: _,
                lines,
            } => add_lines(&mut emptied, *lines),
            FileLoss::Untracked {
                working_tree: _,
                executable: _,
                bytes,
            } => {
                untracked += 1;
                untracked_bytes = untracked_bytes.saturating_add(*bytes);
            }
        }
    }
    let mut parts = Vec::with_capacity(4);
    if modified.files > 0 {
        let mode = match modes.as_slice() {
            [] => None,
            [(from, to)] if modified.files == 1 => Some(format!(
                "the mode change ({} to {})",
                from.octal(),
                to.octal()
            )),
            several => Some(counted(several.len(), "mode change", "mode changes")),
        };
        let detail = match (line_detail(&modified, mode.is_some()), mode) {
            (Some(lines), None) => lines,
            (None, Some(mode)) => mode,
            (Some(lines), Some(mode)) if modified.files == 1 => format!("{lines} and {mode}"),
            (Some(lines), Some(mode)) => format!("{lines}, {mode}"),
            (None, None) => counted(0, "line", "lines"),
        };
        parts.push(format!("{} modified ({detail})", modified.files));
    }
    if emptied.files > 0 {
        parts.push(format!(
            "{} emptied ({})",
            counted(emptied.files, "new file", "new files"),
            line_detail(&emptied, false).unwrap_or_else(|| counted(0, "line", "lines"))
        ));
    }
    if restored > 0 {
        parts.push(format!(
            "{} restored",
            counted(restored, "deleted file", "deleted files")
        ));
    }
    if untracked > 0 {
        parts.push(format!(
            "{} deleted ({})",
            counted(untracked, "untracked file", "untracked files"),
            size(untracked_bytes)
        ));
    }
    format!(
        "Do you want to discard the changes in {what}? {}. You can't undo this action.",
        parts.join(", ")
    )
}

/// Create Branch's discard (R11.3, the user's decision 3), in the discard prompts' words (L8):
/// "Do you want to create branch 'topic' at 1a2b3c4, check it out and discard the changes in
/// 3 files (a.rs, b.rs and c.rs)? 2 modified (14 lines), 1 untracked file overwritten (2.1
/// KiB). Other untracked files are kept. You can't undo this action."
fn checkout_discarding_prompt(
    branch: &str,
    at: Oid,
    changes: &[LostChange],
    kept_untracked: usize,
) -> String {
    let paths: Vec<&RepoPath> = changes.iter().map(|change| &change.path).collect();
    let what = match paths.as_slice() {
        [only] => quoted(only.as_bytes()),
        _ => format!(
            "{} ({})",
            counted(paths.len(), "file", "files"),
            named_paths(&paths)
        ),
    };
    let (mut modified, mut added) = (Lines::default(), Lines::default());
    let mut deleted = 0usize;
    let (mut overwritten, mut overwritten_bytes) = (0usize, 0u64);
    // (directories, files under them, bytes), and the same for nested repositories.
    let (mut directories, mut repositories) = ((0usize, 0usize, 0u64), (0usize, 0usize, 0u64));
    for change in changes {
        match &change.loss {
            ChangeLoss::Changed {
                kind: ChangedKind::Modified,
                index: _,
                working_tree: _,
                executable: _,
                lines,
            } => add_lines(&mut modified, *lines),
            ChangeLoss::Changed {
                kind: ChangedKind::Added,
                index: _,
                working_tree: _,
                executable: _,
                lines,
            } => add_lines(&mut added, *lines),
            ChangeLoss::Changed {
                kind: ChangedKind::Deleted,
                ..
            } => deleted += 1,
            ChangeLoss::Overwritten {
                working_tree: _,
                executable: _,
                bytes,
            } => {
                overwritten += 1;
                overwritten_bytes = overwritten_bytes.saturating_add(*bytes);
            }
            ChangeLoss::Removed { kind, files, bytes } => {
                let tally = match kind {
                    RemovedKind::Directory => &mut directories,
                    RemovedKind::Repository => &mut repositories,
                };
                tally.0 += 1;
                tally.1 += files;
                tally.2 = tally.2.saturating_add(*bytes);
            }
        }
    }
    let lines_of =
        |kind: &Lines| line_detail(kind, false).unwrap_or_else(|| counted(0, "line", "lines"));
    let mut parts = Vec::with_capacity(4);
    if modified.files > 0 {
        parts.push(format!(
            "{} modified ({})",
            modified.files,
            lines_of(&modified)
        ));
    }
    if added.files > 0 {
        parts.push(format!(
            "{} deleted ({})",
            counted(added.files, "new file", "new files"),
            lines_of(&added)
        ));
    }
    if deleted > 0 {
        parts.push(format!(
            "{} restored",
            counted(deleted, "deleted file", "deleted files")
        ));
    }
    if overwritten > 0 {
        parts.push(format!(
            "{} overwritten ({})",
            counted(overwritten, "untracked file", "untracked files"),
            size(overwritten_bytes)
        ));
    }
    // Interim words, awaiting the user's (phase 10's QA, held item A).
    if directories.0 > 0 {
        parts.push(format!(
            "{} in the way removed ({}, {})",
            counted(directories.0, "directory", "directories"),
            counted(directories.1, "untracked file", "untracked files"),
            size(directories.2)
        ));
    }
    if repositories.0 > 0 {
        parts.push(format!(
            "{} removed ({}, {})",
            counted(repositories.0, "nested repository", "nested repositories"),
            counted(repositories.1, "file", "files"),
            size(repositories.2)
        ));
    }
    let lost_untracked = overwritten + directories.0 + repositories.0;
    let kept = match (kept_untracked, lost_untracked) {
        (0, _) => "",
        (_, 0) => " Untracked files are kept.",
        _ => " Other untracked files are kept.",
    };
    format!(
        "Do you want to create branch {} at {}, check it out and discard the changes in {what}? \
         {}.{kept} You can't undo this action.",
        quoted(branch.as_bytes()),
        at.short().as_str(),
        parts.join(", ")
    )
}

/// How many files a discard of several names before it counts the rest.
const NAMED_FILES: usize = 3;

/// The files of a discard of several, as its prompt names them (the user's decision,
/// 2026-10-09): the first [`NAMED_FILES`] by their quoted paths, and how many more after them
/// — "a.rs, b.rs and c.rs", "a.rs, b.rs, c.rs and 2 more".
fn named(files: &[DiscardedFile]) -> String {
    let paths: Vec<&RepoPath> = files.iter().map(|file| &file.path).collect();
    named_paths(&paths)
}

/// [`named`], over the paths themselves.
fn named_paths(paths: &[&RepoPath]) -> String {
    let shown: Vec<String> = paths
        .iter()
        .take(NAMED_FILES)
        .map(|path| quoted(path.as_bytes()))
        .collect();
    let rest = paths.len().saturating_sub(NAMED_FILES);
    match (shown.split_last(), rest) {
        (Some((last, before)), 0) if !before.is_empty() => {
            format!("{} and {last}", before.join(", "))
        }
        (_, 0) => shown.join(", "),
        (_, more) => format!("{} and {more} more", shown.join(", ")),
    }
}

/// The lines of a kind of file in a discard, and how many of those files are not text.
#[derive(Default)]
struct Lines {
    files: usize,
    lines: usize,
    binary: usize,
}

/// One more file of a kind, with its lines (`None` for one that is not text). Free
/// functions rather than an impl, so `impl Consequence` stays this file's one impl block.
fn add_lines(kind: &mut Lines, lines: Option<usize>) {
    kind.files += 1;
    match lines {
        Some(n) => kind.lines += n,
        None => kind.binary += 1,
    }
}

/// A kind's lines and its files without them, in words — "14 lines", "binary", "14 lines,
/// 1 binary" — or nothing where the only change beside them is a mode (`moded`) and there
/// is no line to count: never "0 lines" for a mode change.
fn line_detail(kind: &Lines, moded: bool) -> Option<String> {
    let text = kind.files - kind.binary;
    match (kind.lines, kind.binary) {
        (0, 0) if moded => None,
        (lines, 0) => Some(counted(lines, "line", "lines")),
        (0, _) if moded || text == 0 => Some("binary".to_owned()),
        (lines, binary) => Some(format!(
            "{}, {binary} binary",
            counted(lines, "line", "lines")
        )),
    }
}

/// What a discard of lines takes, in words: the lines, the mode change, or both. The
/// mode change is named with both modes, as the diff draws them (`old mode`, `new mode`).
fn discarded_lines(lines: usize, mode: Option<(FileMode, FileMode)>) -> String {
    match (lines, mode) {
        (0, Some((from, to))) => format!("the mode change ({} to {})", from.octal(), to.octal()),
        (lines, Some((from, to))) => format!(
            "{} and the mode change ({} to {})",
            counted(lines, "line", "lines"),
            from.octal(),
            to.octal()
        ),
        (lines, None) => counted(lines, "line", "lines"),
    }
}

fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A path as git's `quote_c_style` writes it under `core.quotePath=false`:
/// as it is when nothing in it needs quoting, otherwise in double quotes with
/// `"`, `\` and control characters escaped — and, beyond git, the line
/// separators and bidirectional controls a prompt must not let through.
fn quoted(bytes: &[u8]) -> String {
    let inner = escaped(bytes);
    if inner.as_bytes() == bytes && !bytes.contains(&b'"') {
        inner
    } else {
        format!("\"{}\"", inner.replace('"', "\\\""))
    }
}

/// Text with every character that could rewrite the line around it escaped
/// as git escapes it in a C-quoted path: `\n`, `\t` and the other named
/// escapes, an octal escape per byte for the other control characters, the
/// line and paragraph separators, the bidirectional controls and bytes that
/// are not UTF-8; a `\` doubled. Printable text, any script, is kept.
fn escaped(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        for c in chunk.valid().chars() {
            match c {
                '\u{7}' => out.push_str("\\a"),
                '\u{8}' => out.push_str("\\b"),
                '\t' => out.push_str("\\t"),
                '\n' => out.push_str("\\n"),
                '\u{b}' => out.push_str("\\v"),
                '\u{c}' => out.push_str("\\f"),
                '\r' => out.push_str("\\r"),
                '\\' => out.push_str("\\\\"),
                c if c.is_control() || breaks_the_line(c) => {
                    let mut encoded = [0u8; 4];
                    for byte in c.encode_utf8(&mut encoded).bytes() {
                        out.push_str(&format!("\\{byte:03o}"));
                    }
                }
                c => out.push(c),
            }
        }
        for byte in chunk.invalid() {
            out.push_str(&format!("\\{byte:03o}"));
        }
    }
    out
}

/// The characters that reorder or break the text around them without being
/// control characters: the line and paragraph separators, and the
/// bidirectional marks, embeddings, overrides and isolates.
fn breaks_the_line(c: char) -> bool {
    matches!(
        c,
        '\u{2028}'
            | '\u{2029}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{061c}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

/// A size as a person reads it: bytes below a KiB, then one decimal place.
fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return counted(
            usize::try_from(bytes).unwrap_or(usize::MAX),
            "byte",
            "bytes",
        );
    }
    // Precision past a TiB does not matter to the person reading it.
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// An age as a person reads it, in its largest whole unit.
fn elapsed(age: Duration) -> String {
    let seconds = age.as_secs();
    let (n, one, many) = match seconds {
        0..60 => (seconds, "second", "seconds"),
        60..3600 => (seconds / 60, "minute", "minutes"),
        3600..86_400 => (seconds / 3600, "hour", "hours"),
        _ => (seconds / 86_400, "day", "days"),
    };
    counted(usize::try_from(n).unwrap_or(usize::MAX), one, many)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineNumber;

    fn oid(byte: u8) -> Oid {
        Oid::from_bytes(&[byte; 20]).unwrap()
    }

    fn modified(path: &str, lines: Option<usize>) -> DiscardedFile {
        DiscardedFile {
            path: RepoPath::from(path),
            loss: FileLoss::Modified {
                index: oid(1),
                working_tree: Some(oid(2)),
                executable: false,
                lines,
                mode: None,
            },
        }
    }

    fn lost(path: &str, kind: ChangedKind, lines: Option<usize>) -> LostChange {
        LostChange {
            path: RepoPath::from(path),
            loss: ChangeLoss::Changed {
                kind,
                index: Some(oid(1)),
                working_tree: Some(oid(2)),
                executable: false,
                lines,
            },
        }
    }

    fn checkout(changes: Vec<LostChange>, kept_untracked: usize) -> Consequence {
        Consequence::CheckoutDiscarding {
            branch: "topic".to_owned(),
            at: oid(0xab),
            head: Some(oid(7)),
            changes,
            kept_untracked,
        }
    }

    /// Create Branch's discard (R11.3, the user's decision 3): the prompt names the branch, the
    /// commit, each kind of loss counted as the discard prompts count — modified lines, new
    /// files deleted, deleted files restored, untracked files overwritten by size — whether
    /// other untracked files are kept, and that it can't be undone; one file by its path,
    /// several by the first three. Caught by: a staged-only or new file left uncounted, an
    /// overwritten untracked file left unsaid, the kept files claimed when there are none, or
    /// the branch and commit left out of what is confirmed.
    #[test]
    fn a_checkout_that_discards_names_the_branch_and_every_loss() {
        let all = checkout(
            vec![
                lost("a.rs", ChangedKind::Modified, Some(10)),
                lost("b.rs", ChangedKind::Modified, Some(4)),
                lost("new.rs", ChangedKind::Added, Some(3)),
                lost("gone.rs", ChangedKind::Deleted, Some(8)),
                LostChange {
                    path: RepoPath::from("in-the-way.txt"),
                    loss: ChangeLoss::Overwritten {
                        working_tree: oid(9),
                        executable: false,
                        bytes: 2150,
                    },
                },
            ],
            2,
        );
        assert_eq!(
            all.prompt(),
            "Do you want to create branch topic at abababa, check it out and discard the \
             changes in 5 files (a.rs, b.rs, new.rs and 2 more)? 2 modified (14 lines), 1 new \
             file deleted (3 lines), 1 deleted file restored, 1 untracked file overwritten (2.1 \
             KiB). Other untracked files are kept. You can't undo this action."
        );
        assert_eq!(all.action(), "Discard Changes and Check Out");
        let one = checkout(vec![lost("a.rs", ChangedKind::Modified, None)], 0);
        assert_eq!(
            one.prompt(),
            "Do you want to create branch topic at abababa, check it out and discard the \
             changes in a.rs? 1 modified (binary). You can't undo this action."
        );
        let kept = checkout(vec![lost("a.rs", ChangedKind::Modified, Some(1))], 4);
        assert!(
            kept.prompt()
                .contains("(1 line). Untracked files are kept. You can't")
        );
        assert!(!kept.needs_force_push() && kept.replaces().is_none());
        assert_eq!(kept.amended(), None);
    }

    /// Phase 10's QA (items 1-2): a directory of untracked files, or a nested repository, that
    /// the checkout deletes whole is named lost with what is under it, and never counted among
    /// the untracked files kept. The words are interim, awaiting the user (held item A).
    /// Caught by: such a loss left out of the prompt, or "Untracked files are kept." said when
    /// every untracked file is lost.
    #[test]
    fn a_directory_or_a_nested_repository_in_the_way_is_named_lost() {
        let removed = |path: &str, kind: RemovedKind, files: usize, bytes: u64| LostChange {
            path: RepoPath::from(path),
            loss: ChangeLoss::Removed { kind, files, bytes },
        };
        let consequence = checkout(
            vec![
                lost("f", ChangedKind::Modified, Some(1)),
                removed("d", RemovedKind::Directory, 2, 15),
                removed("nested", RemovedKind::Repository, 30, 4096),
            ],
            0,
        );
        assert_eq!(
            consequence.prompt(),
            "Do you want to create branch topic at abababa, check it out and discard the \
             changes in 3 files (f, d and nested)? 1 modified (1 line), 1 directory in the way \
             removed (2 untracked files, 15 bytes), 1 nested repository removed (30 files, 4.0 \
             KiB). You can't undo this action."
        );
        let some_kept = checkout(vec![removed("d", RemovedKind::Directory, 1, 3)], 2);
        assert!(
            some_kept
                .prompt()
                .contains("Other untracked files are kept.")
        );
    }

    fn deleted(path: &str) -> DiscardedFile {
        DiscardedFile {
            path: RepoPath::from(path),
            loss: FileLoss::Modified {
                index: oid(1),
                working_tree: None,
                executable: false,
                lines: Some(9),
                mode: None,
            },
        }
    }

    fn untracked(path: &str, bytes: u64) -> DiscardedFile {
        DiscardedFile {
            path: RepoPath::from(path),
            loss: FileLoss::Untracked {
                working_tree: oid(3),
                executable: false,
                bytes,
            },
        }
    }

    /// A selection of `removed` removed lines and `added` added ones.
    fn selection(removed: u32, added: u32) -> Selection {
        let mut selection = Selection::empty();
        for n in 0..removed {
            selection.select_removed(LineNumber::from_index(n));
        }
        for n in 0..added {
            selection.select_added(LineNumber::from_index(n));
        }
        selection
    }

    fn lines(path: &str, selection: Selection) -> Consequence {
        Consequence::DiscardLines {
            path: RepoPath::from(path),
            index: Some(oid(1)),
            working_tree: oid(2),
            on_disk: oid(4),
            executable: false,
            selection,
            mode: None,
            patch: Patch::empty(),
        }
    }

    fn lock(path: &str, age: Duration, bytes: u64) -> Consequence {
        let read_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        Consequence::RemoveLock {
            path: PathBuf::from(path),
            modified: read_at - age,
            read_at,
            bytes,
            device: 1,
            inode: 2,
        }
    }

    /// L8's example, word for word. Caught by: a count taken from anything but the files.
    #[test]
    fn a_mixed_discard_names_each_kind_with_its_count() {
        let consequence = Consequence::DiscardFiles {
            files: vec![
                modified("a.rs", Some(10)),
                modified("b.rs", Some(4)),
                untracked("notes.txt", 2150),
            ],
        };
        assert_eq!(
            consequence.prompt(),
            "Do you want to discard the changes in 3 files (a.rs, b.rs and notes.txt)? 2 \
             modified (14 lines), 1 untracked \
             file deleted (2.1 KiB). You can't undo this action."
        );
        assert_eq!(consequence.action(), "Discard Changes in 3 Files");
    }

    /// The user's decision (2026-10-09): a discard of several files names them — the first
    /// three by their quoted paths, then how many more — so the dialog says which files go.
    /// Caught by: a count alone, every name listed however many, or a name left unquoted.
    #[test]
    fn a_discard_of_several_files_names_the_first_three_and_counts_the_rest() {
        let two = Consequence::DiscardFiles {
            files: vec![modified("a.rs", Some(1)), modified("b.rs", Some(1))],
        };
        assert_eq!(
            two.prompt(),
            "Do you want to discard the changes in 2 files (a.rs and b.rs)? 2 modified (2 \
             lines). You can't undo this action."
        );
        let five = Consequence::DiscardFiles {
            files: vec![
                modified("a.rs", Some(1)),
                modified("b.rs", Some(1)),
                modified("c.rs", Some(1)),
                modified("d.rs", Some(1)),
                untracked("e.txt", 12),
            ],
        };
        assert_eq!(
            five.prompt(),
            "Do you want to discard the changes in 5 files (a.rs, b.rs, c.rs and 2 more)? 4 \
             modified (4 lines), 1 untracked file deleted (12 bytes). You can't undo this \
             action."
        );
        // A name is quoted as any path in a prompt is: it cannot rewrite the sentence.
        let quoted = Consequence::DiscardFiles {
            files: vec![
                modified("a\nYou can undo this.rs", Some(1)),
                modified("b.rs", Some(1)),
            ],
        };
        assert_eq!(
            quoted.prompt(),
            "Do you want to discard the changes in 2 files (\"a\\nYou can undo this.rs\" and \
             b.rs)? 2 modified (2 lines). You can't undo this action."
        );
        // The token's prompt is the dialog's, word for word.
        assert_eq!(
            crate::Confirmed::by_user(five.clone()).prompt(),
            five.prompt()
        );
    }

    /// Caught by: the untracked files' sizes not summed (the last one's kept), or their
    /// count taken as one.
    #[test]
    fn untracked_files_are_counted_and_their_sizes_summed() {
        let consequence = Consequence::DiscardFiles {
            files: vec![
                untracked("a", 1000),
                untracked("b", 1000),
                untracked("c", 48),
            ],
        };
        assert_eq!(
            consequence.prompt(),
            "Do you want to discard the changes in 3 files (a, b and c)? 3 untracked files \
             deleted (2.0 \
             KiB). You can't undo this action."
        );
    }

    /// Caught by: a prompt that names a count where one path is the whole of it, or a
    /// singular rendered as a plural.
    #[test]
    fn one_file_is_named_by_its_path_and_counted_singly() {
        let consequence = Consequence::DiscardFiles {
            files: vec![modified("src/main.rs", Some(1))],
        };
        assert_eq!(
            consequence.prompt(),
            "Do you want to discard the changes in src/main.rs? 1 modified (1 line). You \
             can't undo this action."
        );
        assert_eq!(consequence.action(), "Discard Changes in 1 File");
        let deleted_one = Consequence::DiscardFiles {
            files: vec![untracked("scratch", 12)],
        };
        assert_eq!(
            deleted_one.prompt(),
            "Do you want to discard the changes in scratch? 1 untracked file deleted (12 \
             bytes). You can't undo this action."
        );
    }

    /// A binary change has no lines, and the prompt says so rather than counting it as none.
    #[test]
    fn a_binary_change_is_named_binary_never_counted_as_no_lines() {
        let all_binary = Consequence::DiscardFiles {
            files: vec![modified("logo.png", None), modified("icon.png", None)],
        };
        assert_eq!(
            all_binary.prompt(),
            "Do you want to discard the changes in 2 files (logo.png and icon.png)? 2 modified \
             (binary). You can't \
             undo this action."
        );
        let mixed = Consequence::DiscardFiles {
            files: vec![modified("logo.png", None), modified("a.rs", Some(3))],
        };
        assert_eq!(
            mixed.prompt(),
            "Do you want to discard the changes in 2 files (logo.png and a.rs)? 2 modified (3 \
             lines, 1 binary). \
             You can't undo this action."
        );
    }

    /// A file deleted in the working tree comes back when it is discarded; the prompt says
    /// that rather than counting it as a modification's lines.
    #[test]
    fn a_file_deleted_in_the_working_tree_is_named_as_restored() {
        let consequence = Consequence::DiscardFiles {
            files: vec![
                deleted("gone.rs"),
                modified("a.rs", Some(2)),
                deleted("also.rs"),
            ],
        };
        assert_eq!(
            consequence.prompt(),
            "Do you want to discard the changes in 3 files (gone.rs, a.rs and also.rs)? 1 \
             modified (2 lines), 2 deleted \
             files restored. You can't undo this action."
        );
        let one = Consequence::DiscardFiles {
            files: vec![deleted("gone.rs")],
        };
        assert_eq!(
            one.prompt(),
            "Do you want to discard the changes in gone.rs? 1 deleted file restored. You \
             can't undo this action."
        );
    }

    /// The count is the selection's, both sides of it. Caught by: counting only the added
    /// lines (what is deleted) or only the removed ones.
    #[test]
    fn discarded_lines_count_both_sides_of_the_selection() {
        let consequence = lines("src/lib.rs", selection(3, 1));
        assert_eq!(
            consequence.prompt(),
            "Do you want to discard 4 lines in src/lib.rs? You can't undo this action."
        );
        assert_eq!(consequence.action(), "Discard 4 Lines");
        let other_way = lines("src/lib.rs", selection(1, 3));
        assert_eq!(other_way.action(), "Discard 4 Lines");
        let one = Consequence::DiscardLines {
            path: RepoPath::from("new.txt"),
            index: None,
            working_tree: oid(2),
            on_disk: oid(4),
            executable: false,
            selection: selection(0, 1),
            mode: None,
            patch: Patch::empty(),
        };
        assert_eq!(
            one.prompt(),
            "Do you want to discard 1 line in new.txt? You can't undo this action."
        );
        assert_eq!(one.action(), "Discard 1 Line");
    }

    /// Phase 01's QA item 19: a mode change selected for discard is named, with both modes,
    /// beside the lines or alone — never "0 lines". Caught by: counting the selection's lines
    /// alone, or rendering the mode the discard restores as the one it removes.
    #[test]
    fn a_discarded_mode_change_is_named_with_its_modes() {
        let with_mode = |lines: Selection| {
            let mut selection = lines;
            selection.select_mode();
            Consequence::DiscardLines {
                path: RepoPath::from("run.sh"),
                index: Some(oid(1)),
                working_tree: oid(2),
                on_disk: oid(4),
                executable: false,
                selection,
                mode: Some((FileMode::Regular, FileMode::Executable)),
                patch: Patch::empty(),
            }
        };
        let both = with_mode(selection(1, 1));
        assert_eq!(
            both.prompt(),
            "Do you want to discard 2 lines and the mode change (100644 to 100755) in run.sh? \
             You can't undo this action."
        );
        assert_eq!(both.action(), "Discard 2 Lines and Mode Change");
        let alone = with_mode(Selection::empty());
        assert_eq!(
            alone.prompt(),
            "Do you want to discard the mode change (100644 to 100755) in run.sh? You can't \
             undo this action."
        );
        assert_eq!(alone.action(), "Discard Mode Change");
        let one = with_mode(selection(0, 1));
        assert_eq!(one.action(), "Discard 1 Line and Mode Change");
    }

    /// Phase 03's QA item 2: the executable bit is part of what a discard re-checks, so two
    /// consequences that differ in it alone are different values — and, until the user
    /// settles how a mode change is worded, render the same words.
    #[test]
    fn the_executable_bit_is_compared_but_not_yet_worded() {
        let with = |executable| Consequence::DiscardFiles {
            files: vec![DiscardedFile {
                path: RepoPath::from("run.sh"),
                loss: FileLoss::Modified {
                    index: oid(1),
                    working_tree: Some(oid(2)),
                    executable,
                    lines: Some(1),
                    mode: None,
                },
            }],
        };
        assert_ne!(with(true), with(false));
        assert_eq!(with(true).prompt(), with(false).prompt());
    }

    /// Phase 03's QA item 5: the patch a discard of lines applies is part of the value the user
    /// confirms, so a consequence with another patch is another consequence; the prompt is
    /// rendered from the selection and does not change with it.
    #[test]
    fn the_confirmed_patch_is_part_of_the_value() {
        let with = |patch: &str| Consequence::DiscardLines {
            path: RepoPath::from("file.txt"),
            index: Some(oid(1)),
            working_tree: oid(2),
            on_disk: oid(4),
            executable: false,
            selection: selection(1, 0),
            mode: None,
            patch: crate::emit_patch(
                &crate::ChangedFile {
                    status: crate::ChangeStatus::Modified,
                    old_path: RepoPath::from(patch),
                    new_path: RepoPath::from(patch),
                    old_mode: Some(FileMode::Regular),
                    new_mode: Some(FileMode::Executable),
                    old_id: Some(oid(1)),
                    new_id: Some(oid(2)),
                },
                &crate::TextDiff::new(Vec::new(), Vec::new(), Vec::new()),
                &{
                    let mut mode = Selection::empty();
                    mode.select_mode();
                    mode
                },
            ),
        };
        assert_ne!(with("a"), with("b"));
        assert_eq!(with("a").prompt(), with("b").prompt());
    }

    /// The user's decision 6 (2026-10-08): a whole file's mode change is named with both
    /// modes beside its lines, and a file whose only change is its mode never reads "0
    /// lines". Full prompts, one file and several.
    #[test]
    fn a_whole_files_mode_change_is_named_never_counted_as_no_lines() {
        let file = |path: &str, lines: usize, mode: Option<(FileMode, FileMode)>| DiscardedFile {
            path: RepoPath::from(path),
            loss: FileLoss::Modified {
                index: oid(1),
                working_tree: Some(oid(2)),
                executable: false,
                lines: Some(lines),
                mode,
            },
        };
        let changed = Some((FileMode::Regular, FileMode::Executable));
        let alone = Consequence::DiscardFiles {
            files: vec![file("run.sh", 0, changed)],
        };
        assert_eq!(
            alone.prompt(),
            "Do you want to discard the changes in run.sh? 1 modified (the mode change \
             (100644 to 100755)). You can't undo this action."
        );
        let beside = Consequence::DiscardFiles {
            files: vec![file("run.sh", 2, changed)],
        };
        assert_eq!(
            beside.prompt(),
            "Do you want to discard the changes in run.sh? 1 modified (2 lines and the mode \
             change (100644 to 100755)). You can't undo this action."
        );
        let several = Consequence::DiscardFiles {
            files: vec![
                file("a.rs", 10, None),
                file("run.sh", 0, changed),
                file("tool", 3, Some((FileMode::Executable, FileMode::Regular))),
            ],
        };
        assert_eq!(
            several.prompt(),
            "Do you want to discard the changes in 3 files (a.rs, run.sh and tool)? 3 modified \
             (13 lines, 2 mode \
             changes). You can't undo this action."
        );
    }

    /// The user's decision 5 (2026-10-08): an intent-to-add file's discard leaves it empty, as
    /// `git restore` does, and the prompt says so — never "modified".
    #[test]
    fn an_intent_to_add_file_is_named_as_emptied() {
        let emptied = |path: &str, lines: Option<usize>| DiscardedFile {
            path: RepoPath::from(path),
            loss: FileLoss::Emptied {
                index: oid(1),
                working_tree: oid(2),
                executable: false,
                lines,
            },
        };
        let one = Consequence::DiscardFiles {
            files: vec![emptied("new.txt", Some(5))],
        };
        assert_eq!(
            one.prompt(),
            "Do you want to discard the changes in new.txt? 1 new file emptied (5 lines). You \
             can't undo this action."
        );
        let two = Consequence::DiscardFiles {
            files: vec![
                emptied("a.txt", Some(5)),
                emptied("b.bin", None),
                modified("c.rs", Some(1)),
            ],
        };
        assert_eq!(
            two.prompt(),
            "Do you want to discard the changes in 3 files (a.txt, b.bin and c.rs)? 1 modified \
             (1 line), 2 new files \
             emptied (5 lines, 1 binary). You can't undo this action."
        );
    }

    /// R10.6's text, and the force push named exactly when a remote has the commit.
    #[test]
    fn an_amend_names_the_commit_it_replaces_and_whether_a_remote_has_it() {
        let commit = oid(0xab);
        let amend = |published| Consequence::Amend {
            commit,
            subject: "Fix the parser".to_owned(),
            published,
            reflog: Reflog::Written,
        };
        assert_eq!(commit.short().as_str(), "abababa");
        assert_eq!(
            amend(Publication::Unpublished).prompt(),
            "Replaces abababa 'Fix the parser'. The old commit stays in Show Lost Commits."
        );
        assert_eq!(
            amend(Publication::Upstream(RefName::new(
                "refs/remotes/origin/main"
            )))
            .prompt(),
            "abababa is already on origin/main. Sharing the amended commit needs a force \
             push. Replaces abababa 'Fix the parser'. The old commit stays in Show Lost \
             Commits."
        );
        assert_eq!(
            amend(Publication::SomeRemote).prompt(),
            "abababa is already on a remote. Sharing the amended commit needs a force push. \
             Replaces abababa 'Fix the parser'. The old commit stays in Show Lost Commits."
        );
        assert_eq!(amend(Publication::Unpublished).action(), "Amend abababa");
    }

    /// R10.6 as the user decided it on 2026-10-08: "stays in Show Lost Commits" only when
    /// git will write the reflog entry that keeps the old commit findable, and otherwise
    /// that it cannot be recovered and why. Caught by: the promise made whatever the
    /// repository keeps.
    #[test]
    fn an_amend_promises_recovery_only_where_a_reflog_is_written() {
        let amend = |published, reflog| Consequence::Amend {
            commit: oid(0xab),
            subject: "Fix the parser".to_owned(),
            published,
            reflog,
        };
        assert_eq!(
            amend(Publication::Unpublished, Reflog::Written).prompt(),
            "Replaces abababa 'Fix the parser'. The old commit stays in Show Lost Commits."
        );
        assert_eq!(
            amend(Publication::Unpublished, Reflog::NotWritten).prompt(),
            "Replaces abababa 'Fix the parser'. The old commit can't be recovered afterwards: \
             this repository keeps no reflog."
        );
        assert_eq!(
            amend(
                Publication::Upstream(RefName::new("refs/remotes/origin/main")),
                Reflog::NotWritten
            )
            .prompt(),
            "abababa is already on origin/main. Sharing the amended commit needs a force \
             push. Replaces abababa 'Fix the parser'. The old commit can't be recovered \
             afterwards: this repository keeps no reflog."
        );
        assert_eq!(
            amend(Publication::Unpublished, Reflog::NotWritten).action(),
            "Amend abababa"
        );
    }

    /// Phase 01's QA item 21: the amend's two texts are rendered apart — the force push the
    /// dialog asks first, the line under the button — and the prompt a token records is
    /// exactly the two joined, so what a surface draws of either is what the token quotes.
    /// The dialog is needed exactly when a remote has the commit, and for nothing else.
    /// Caught by: a part that drifts from the prompt, or a dialog asked for an unpublished
    /// amend or a discard.
    #[test]
    fn an_amends_parts_are_its_prompt_and_only_a_published_one_needs_the_dialog() {
        let amend = |published, reflog| Consequence::Amend {
            commit: oid(0xab),
            subject: "Fix the parser".to_owned(),
            published,
            reflog,
        };
        for reflog in [Reflog::Written, Reflog::NotWritten] {
            let unpublished = amend(Publication::Unpublished, reflog);
            assert_eq!(unpublished.amended(), Some(oid(0xab)));
            assert_eq!(unpublished.force_push_warning(), None);
            assert!(!unpublished.needs_force_push());
            assert_eq!(unpublished.replaces(), Some(unpublished.prompt()));
            for published in [
                Publication::Upstream(RefName::new("refs/remotes/origin/main")),
                Publication::SomeRemote,
            ] {
                let consequence = amend(published, reflog);
                assert!(consequence.needs_force_push());
                let (Some(warning), Some(replaces)) =
                    (consequence.force_push_warning(), consequence.replaces())
                else {
                    panic!("a published amend lost a part: {consequence:?}");
                };
                assert!(warning.ends_with("needs a force push."), "{warning}");
                assert!(replaces.starts_with("Replaces abababa"), "{replaces}");
                assert_eq!(consequence.prompt(), format!("{warning} {replaces}"));
            }
        }
        let discard = lines("a.rs", selection(1, 0));
        assert!(!discard.needs_force_push());
        assert_eq!(discard.replaces(), None);
        assert_eq!(discard.amended(), None);
        assert_eq!(
            lock("/r/.git/index.lock", Duration::from_secs(1), 0).replaces(),
            None
        );
    }

    #[test]
    fn removing_a_lock_names_it_its_age_and_the_risk() {
        let consequence = lock(
            "/work/repo/.git/index.lock",
            Duration::from_secs(3 * 60 + 20),
            0,
        );
        assert_eq!(
            consequence.prompt(),
            "Remove /work/repo/.git/index.lock? It was last changed 3 minutes ago and holds 0 \
             bytes. Another program may still own it: removing a lock a running git holds can \
             corrupt the repository."
        );
        assert_eq!(consequence.action(), "Remove index.lock");
        assert_eq!(lock("/", Duration::ZERO, 0).action(), "Remove /");
    }

    /// A lock modified after it was read (a clock that moved) is no age at all, not a
    /// panic or a wrapped number.
    #[test]
    fn a_lock_from_the_future_is_no_age_at_all() {
        let read_at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let consequence = Consequence::RemoveLock {
            path: PathBuf::from("/r/.git/index.lock"),
            modified: read_at + Duration::from_secs(50),
            read_at,
            bytes: 1,
            device: 0,
            inode: 0,
        };
        assert!(
            consequence
                .prompt()
                .contains("last changed 0 seconds ago and holds 1 byte."),
            "{}",
            consequence.prompt()
        );
    }

    /// A name a repository's author chose cannot rewrite the prompt around it: a newline,
    /// a line separator and a right-to-left override are escaped as git escapes a C-quoted
    /// path, and the path is quoted, as git quotes it, once anything in it is.
    #[test]
    fn a_path_or_a_subject_cannot_rewrite_the_prompt() {
        let path = "a\nYou can undo this action.\u{202e}txt.exe";
        let consequence = Consequence::DiscardFiles {
            files: vec![modified(path, Some(1))],
        };
        assert_eq!(
            consequence.prompt(),
            "Do you want to discard the changes in \"a\\nYou can undo this \
             action.\\342\\200\\256txt.exe\"? 1 modified (1 line). You can't undo this action."
        );
        let quote = lines("say \"hi\"\\", selection(1, 0));
        assert_eq!(
            quote.prompt(),
            "Do you want to discard 1 line in \"say \\\"hi\\\"\\\\\"? You can't undo this \
             action."
        );
        let bytes = Consequence::DiscardFiles {
            files: vec![DiscardedFile {
                path: RepoPath::new(vec![b'a', 0xff, b'\t']),
                loss: FileLoss::Untracked {
                    working_tree: oid(3),
                    executable: false,
                    bytes: 1,
                },
            }],
        };
        assert!(
            bytes.prompt().contains("in \"a\\377\\t\"?"),
            "{}",
            bytes.prompt()
        );
        let subject = Consequence::Amend {
            commit: oid(0xab),
            subject: "Fix\u{2028}Replaces nothing\u{7}".to_owned(),
            published: Publication::Unpublished,
            reflog: Reflog::Written,
        };
        assert_eq!(
            subject.prompt(),
            "Replaces abababa 'Fix\\342\\200\\250Replaces nothing\\a'. The old commit stays \
             in Show Lost Commits."
        );
        let plain = Consequence::DiscardFiles {
            files: vec![modified("docs/naïve café.md", Some(2))],
        };
        assert!(
            plain.prompt().contains("in docs/naïve café.md? 1 modified"),
            "printable text of any script is kept as it is: {}",
            plain.prompt()
        );
    }

    #[test]
    fn sizes_and_ages_read_in_their_largest_unit() {
        assert_eq!(size(0), "0 bytes");
        assert_eq!(size(1), "1 byte");
        assert_eq!(size(1023), "1023 bytes");
        assert_eq!(size(1024), "1.0 KiB");
        assert_eq!(size(5 * 1024 * 1024 + 512 * 1024), "5.5 MiB");
        assert_eq!(size(3 * 1024 * 1024 * 1024), "3.0 GiB");
        assert_eq!(size(2 * 1024 * 1024 * 1024 * 1024), "2.0 TiB");
        assert_eq!(size(4096 * 1024 * 1024 * 1024 * 1024), "4096.0 TiB");
        assert_eq!(elapsed(Duration::from_secs(0)), "0 seconds");
        assert_eq!(elapsed(Duration::from_secs(1)), "1 second");
        assert_eq!(elapsed(Duration::from_secs(59)), "59 seconds");
        assert_eq!(elapsed(Duration::from_secs(60)), "1 minute");
        assert_eq!(elapsed(Duration::from_secs(3600)), "1 hour");
        assert_eq!(elapsed(Duration::from_secs(2 * 3600)), "2 hours");
        assert_eq!(elapsed(Duration::from_secs(86_400)), "1 day");
        assert_eq!(elapsed(Duration::from_secs(3 * 86_400 + 5)), "3 days");
    }
}
