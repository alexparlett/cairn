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
//! always renders the same words. A path or a branch's name is text a repository's
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
        /// Whether the selection is the chunk a hovered chunk's own Discard took — every
        /// changed line of the hunk drawn — rather than lines selected by a drag: the prompt
        /// names it "this chunk" (staging-and-commit phase 15). Words alone: no re-check reads it.
        chunk: bool,
        /// The patch the discard applies, emitted from the diff the user confirmed
        /// with this selection: what runs is what was confirmed, never a patch built
        /// again from a later diff, whose lines could align otherwise.
        patch: Patch,
    },
    /// Whole files: each tracked file's unstaged change restored from the
    /// index, each untracked file deleted. Never empty when the engine builds
    /// it.
    DiscardFiles { files: Vec<DiscardedFile> },
    /// `HEAD` replaced by an amended commit (staging-and-commit R6.4): the commit, the remote
    /// ref that has it, and whether git will log the move — what the amend re-checks, and
    /// nothing drawn for display alone, so no display field is ever a freshness condition.
    Amend {
        commit: Oid,
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
    /// A branch created at a commit and checked out with Fork's forced checkout (`git checkout
    /// -q --no-track -f -b <branch> <at> --`; staging-and-commit R11.3, the user's decision of
    /// 2026-10-10): the local changes and any untracked files in the way discarded, as git
    /// decides. Fixed and generic — it predicts nothing of what git deletes (the redesign's rule
    /// 7) — and what the re-check holds: `HEAD`, the commit and the name, nothing else (R1.4).
    CheckoutDiscarding {
        /// The branch created, as typed.
        branch: String,
        /// The commit it is created at, and checked out.
        at: Oid,
        /// `HEAD`'s commit when this was read; `None` on an unborn branch. The re-check holds it.
        head: Option<Oid>,
    },
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
        /// index's mode, then the working tree's. Detail of the file, never the
        /// prompt's sentence (the redesign's D2).
        mode: Option<(FileMode, FileMode)>,
    },
    /// A file added with `git add -N`, whose index entry is the empty blob:
    /// `git restore` writes that entry back, so the discard leaves the file
    /// EMPTY and the entry in place, as the user's own `git restore` does (the
    /// user's decision of 2026-10-08; N3, checked against real git in phase 15:
    /// git empties it and keeps the entry, on every git the gate runs).
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
                chunk,
                patch: _,
            } => format!(
                "{} {UNDONE}",
                lines_question(&quoted(path.as_bytes()), selection.len(), *mode, *chunk)
            ),
            Self::DiscardFiles { files } => discard_files_prompt(files),
            Self::CheckoutDiscarding {
                branch,
                at,
                head: _,
            } => format!(
                "Discard local changes and any untracked files in the way, then check out {} at \
                 {}. {UNDONE}",
                quoted(branch.as_bytes()),
                at.short().as_str()
            ),
            Self::Amend {
                commit,
                published,
                reflog,
            } => amend_prompt(*commit, published, *reflog),
            Self::RemoveLock {
                path,
                modified,
                read_at,
                bytes,
                device: _,
                inode: _,
            } => format!(
                "Remove {}? It was last changed {} ago and holds {}. Another program may still \
                 own it: removing a lock a running git holds can corrupt the index. {UNDONE}",
                quoted(path.as_os_str().as_encoded_bytes()),
                elapsed(read_at.duration_since(*modified).unwrap_or(Duration::ZERO)),
                size(*bytes)
            ),
        }
    }

    /// The commit an amend replaces: what its prompt names and its re-check holds `HEAD` to.
    /// `None` for every other operation.
    pub fn amended(&self) -> Option<Oid> {
        match self {
            Self::Amend {
                commit,
                published: _,
                reflog: _,
            } => Some(*commit),
            Self::DiscardLines { .. }
            | Self::DiscardFiles { .. }
            | Self::RemoveLock { .. }
            | Self::CheckoutDiscarding { .. } => None,
        }
    }

    /// Whether the operation must be confirmed before it runs (staging-and-commit R1.5): every
    /// destructive operation but an amend git logs and no remote has, which Show Lost Commits
    /// recovers and which runs at once, asking nothing. An amend a remote already has, or one
    /// git keeps no reflog for, is confirmed first.
    pub fn needs_confirming(&self) -> bool {
        match self {
            Self::Amend {
                commit: _,
                published,
                reflog,
            } => *published != Publication::Unpublished || *reflog == Reflog::NotWritten,
            Self::DiscardLines { .. }
            | Self::DiscardFiles { .. }
            | Self::RemoveLock { .. }
            | Self::CheckoutDiscarding { .. } => true,
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
                chunk: _,
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
            Self::Amend { .. } => "Amend".to_owned(),
            // Confirmed by the Create Branch dialog's own button, which reads so (B3).
            Self::CheckoutDiscarding { .. } => "Create and Checkout".to_owned(),
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

    /// What the operation is called where its run is listed — the activity popover — in Fork's
    /// imperative form, as its Activity Manager names one ("Fetch origin", "Create branch
    /// 'develop'"; `fork-staging-and-commit.md` §7): "Discard lines of a.rs", "Discard 3
    /// files", "Amend", "Create branch 'topic'", "Remove index.lock".
    pub fn name(&self) -> String {
        match self {
            Self::DiscardLines {
                path,
                index: _,
                working_tree: _,
                on_disk: _,
                executable: _,
                selection: _,
                mode: _,
                chunk: _,
                patch: _,
            } => format!("Discard lines of {}", quoted(path.as_bytes())),
            Self::DiscardFiles { files } => {
                format!("Discard {}", counted(files.len(), "file", "files"))
            }
            Self::Amend { .. } => "Amend".to_owned(),
            Self::CheckoutDiscarding {
                branch,
                at: _,
                head: _,
            } => format!("Create branch '{}'", escaped(branch.as_bytes())),
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

/// R10.6's dialog text (the redesign of 2026-10-10): one fixed sentence for each reason the amend
/// is confirmed, naming the commit — that a remote already has it, and that git keeps no reflog
/// to recover it from — both where both hold. An amend for neither is recoverable and never
/// asks; its prompt, drawn by no dialog, says where the old commit stays.
fn amend_prompt(commit: Oid, published: &Publication, reflog: Reflog) -> String {
    let short = commit.short();
    let short = short.as_str();
    let on = match published {
        Publication::Unpublished => None,
        Publication::Upstream(upstream) => Some(quoted(upstream.shorthand().as_bytes())),
        Publication::SomeRemote => Some("a remote".to_owned()),
    };
    let shared = on.map(|on| {
        format!("{short} is already on {on}. Amending it rewrites history others may have.")
    });
    let unrecoverable = (reflog == Reflog::NotWritten).then(|| {
        format!("{short} can't be recovered after this: this repository keeps no reflog.")
    });
    match (shared, unrecoverable) {
        (Some(shared), Some(unrecoverable)) => format!("{shared} {unrecoverable}"),
        (Some(sentence), None) | (None, Some(sentence)) => sentence,
        (None, None) => format!("Amend {short}? The old commit stays in Show Lost Commits."),
    }
}

/// How every destructive prompt but an amend's ends (staging-and-commit R1.2, the redesign's D2).
const UNDONE: &str = "You can't undo this.";

/// The one sentence frame for whole files (R1.2, R8.4, the redesign's D2): the question naming
/// what is discarded — one file by its path, several by their count — then the worst loss only
/// when it is worse than changes, which is an untracked file deleted, then that it can't be
/// undone: "Discard all changes in 31 files? 2 untracked files will be deleted. You can't undo
/// this." What happens to each file (restored, emptied, its lines, a mode change) is the
/// detail behind "Show files", never the sentence's.
fn discard_files_prompt(files: &[DiscardedFile]) -> String {
    let what = match files {
        [only] => quoted(only.path.as_bytes()),
        _ => counted(files.len(), "file", "files"),
    };
    let untracked = files
        .iter()
        .filter(|file| match file.loss {
            FileLoss::Untracked { .. } => true,
            FileLoss::Modified { .. } | FileLoss::Emptied { .. } => false,
        })
        .count();
    let worst = match untracked {
        0 => String::new(),
        1 => "1 untracked file will be deleted. ".to_owned(),
        n => format!("{n} untracked files will be deleted. "),
    };
    format!("Discard all changes in {what}? {worst}{UNDONE}")
}

/// The question for a discard of lines (R8.4, the redesign's D2), naming no mode in octal —
/// the mode row is on screen: "Discard 2 changed lines in src/main.rs?", the chunk a hovered
/// chunk's Discard took "Discard this chunk (6 lines) in src/main.rs?", a mode change alone
/// "Discard the mode change of run.sh?". Lines and the mode together — which no route of the
/// view asks, the mode row acting on the mode alone — read "Discard 2 changed lines and the
/// mode change in run.sh?".
fn lines_question(
    path: &str,
    lines: usize,
    mode: Option<(FileMode, FileMode)>,
    chunk: bool,
) -> String {
    match (lines, mode, chunk) {
        (0, Some(_), _) => format!("Discard the mode change of {path}?"),
        (lines, Some(_), _) => format!(
            "Discard {} and the mode change in {path}?",
            counted(lines, "changed line", "changed lines")
        ),
        (lines, None, true) => format!(
            "Discard this chunk ({}) in {path}?",
            counted(lines, "line", "lines")
        ),
        (lines, None, false) => format!(
            "Discard {} in {path}?",
            counted(lines, "changed line", "changed lines")
        ),
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

    fn checkout() -> Consequence {
        Consequence::CheckoutDiscarding {
            branch: "topic".to_owned(),
            at: oid(0xab),
            head: Some(oid(7)),
        }
    }

    /// Create Branch's Discard (R11.3, the user's decision of 2026-10-10): the prompt the
    /// dialog's press confirms is fixed and generic — it names the branch and the commit, says
    /// local changes and untracked files in the way go and that it can't be undone, and names
    /// no file, so nothing is predicted of what git deletes; the button is the dialog's own.
    /// Caught by: a file or a count put back in, the branch or commit left out of what is
    /// confirmed, or a second dialog's button.
    #[test]
    fn a_checkout_that_discards_says_so_in_fixed_words() {
        assert_eq!(
            checkout().prompt(),
            "Discard local changes and any untracked files in the way, then check out topic at \
             abababa. You can't undo this."
        );
        assert_eq!(checkout().action(), "Create and Checkout");
        assert_eq!(checkout().amended(), None);
        let odd = Consequence::CheckoutDiscarding {
            branch: "we\nird".to_owned(),
            at: oid(0xab),
            head: None,
        };
        assert!(
            odd.prompt().contains("check out \"we\\nird\" at"),
            "{}",
            odd.prompt()
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
            chunk: false,
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

    /// The redesign's D2 (R1.2, R8.4): a discard of files asks in one sentence — one file named
    /// by its path, several counted — and the button counts the files. Caught by: a kind of
    /// change or a path listed in the sentence, a count taken from anything but the files, or a
    /// prompt ending otherwise than the frame.
    #[test]
    fn a_discard_of_files_asks_in_the_one_sentence_frame() {
        let one = Consequence::DiscardFiles {
            files: vec![modified("src/main.rs", Some(14))],
        };
        assert_eq!(
            one.prompt(),
            "Discard all changes in src/main.rs? You can't undo this."
        );
        assert_eq!(one.action(), "Discard Changes in 1 File");
        let many: Vec<DiscardedFile> = (0..31)
            .map(|n| modified(&format!("f{n}.rs"), Some(n)))
            .collect();
        let several = Consequence::DiscardFiles {
            files: many.clone(),
        };
        assert_eq!(
            several.prompt(),
            "Discard all changes in 31 files? You can't undo this."
        );
        assert_eq!(several.action(), "Discard Changes in 31 Files");
        let mut with_untracked = many[..29].to_vec();
        with_untracked.push(untracked("notes.txt", 2150));
        with_untracked.push(untracked("scratch", 12));
        let with_untracked = Consequence::DiscardFiles {
            files: with_untracked,
        };
        assert_eq!(
            with_untracked.prompt(),
            "Discard all changes in 31 files? 2 untracked files will be deleted. You can't undo \
             this."
        );
        assert_eq!(with_untracked.action(), "Discard Changes in 31 Files");
        // The token's prompt is the dialog's, word for word.
        assert_eq!(
            crate::Confirmed::by_user(with_untracked.clone()).prompt(),
            with_untracked.prompt()
        );
    }

    /// The phase's QA brief: the worst loss in the sentence is the worst one in the list, so a
    /// selection holding an untracked file always says it will be deleted — alone, beside every
    /// other kind, and named by its path when it is the one file — and one holding none never
    /// says it. Restored, emptied, binary and mode changes are changes, behind "Show files".
    /// Caught by: the deletion said only for several files, or only when every file is
    /// untracked, or said where nothing is deleted.
    #[test]
    fn the_worst_loss_is_said_whenever_an_untracked_file_is_deleted() {
        let alone = Consequence::DiscardFiles {
            files: vec![untracked("scratch", 12)],
        };
        assert_eq!(
            alone.prompt(),
            "Discard all changes in scratch? 1 untracked file will be deleted. You can't undo \
             this."
        );
        let emptied = DiscardedFile {
            path: RepoPath::from("new.txt"),
            loss: FileLoss::Emptied {
                index: oid(1),
                working_tree: oid(2),
                executable: false,
                lines: Some(5),
            },
        };
        let moded = DiscardedFile {
            path: RepoPath::from("run.sh"),
            loss: FileLoss::Modified {
                index: oid(1),
                working_tree: Some(oid(2)),
                executable: true,
                lines: Some(0),
                mode: Some((FileMode::Regular, FileMode::Executable)),
            },
        };
        let changes = vec![
            modified("a.rs", Some(3)),
            modified("logo.png", None),
            deleted("gone.rs"),
            emptied,
            moded,
        ];
        let no_untracked = Consequence::DiscardFiles {
            files: changes.clone(),
        };
        assert_eq!(
            no_untracked.prompt(),
            "Discard all changes in 5 files? You can't undo this."
        );
        let mut every_kind = changes;
        every_kind.insert(2, untracked("notes.txt", 2150));
        let every_kind = Consequence::DiscardFiles { files: every_kind };
        assert_eq!(
            every_kind.prompt(),
            "Discard all changes in 6 files? 1 untracked file will be deleted. You can't undo \
             this."
        );
        for prompt in [no_untracked.prompt(), every_kind.prompt()] {
            assert!(!prompt.contains("100644"), "no octal: {prompt}");
            assert!(!prompt.contains("a.rs"), "no path of several: {prompt}");
        }
    }

    /// The redesign's D2 and 4b: lines from the diff are counted, both sides of the selection;
    /// the chunk a hovered chunk's Discard took is named as the chunk with its lines; a mode
    /// change alone is named in words, no octal; and the button counts what it discards.
    /// Caught by: counting one side only, a chunk worded as lines or lines as a chunk, a mode
    /// named in octal, or the button counting otherwise than the prompt.
    #[test]
    fn a_discard_of_lines_names_the_lines_the_chunk_or_the_mode() {
        let consequence = lines("src/lib.rs", selection(3, 1));
        assert_eq!(
            consequence.prompt(),
            "Discard 4 changed lines in src/lib.rs? You can't undo this."
        );
        assert_eq!(consequence.action(), "Discard 4 Lines");
        assert_eq!(
            lines("src/lib.rs", selection(1, 3)).action(),
            "Discard 4 Lines"
        );
        let one = lines("new.txt", selection(0, 1));
        assert_eq!(
            one.prompt(),
            "Discard 1 changed line in new.txt? You can't undo this."
        );
        assert_eq!(one.action(), "Discard 1 Line");

        let chunk = |selection: Selection| Consequence::DiscardLines {
            path: RepoPath::from("src/main.rs"),
            index: Some(oid(1)),
            working_tree: oid(2),
            on_disk: oid(4),
            executable: false,
            selection,
            mode: None,
            chunk: true,
            patch: Patch::empty(),
        };
        assert_eq!(
            chunk(selection(2, 4)).prompt(),
            "Discard this chunk (6 lines) in src/main.rs? You can't undo this."
        );
        assert_eq!(chunk(selection(2, 4)).action(), "Discard 6 Lines");
        assert_eq!(
            chunk(selection(0, 1)).prompt(),
            "Discard this chunk (1 line) in src/main.rs? You can't undo this."
        );

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
                chunk: false,
                patch: Patch::empty(),
            }
        };
        let alone = with_mode(Selection::empty());
        assert_eq!(
            alone.prompt(),
            "Discard the mode change of run.sh? You can't undo this."
        );
        assert_eq!(alone.action(), "Discard Mode Change");
        // No route of the view asks lines and the mode together; the words still hold.
        let both = with_mode(selection(1, 1));
        assert_eq!(
            both.prompt(),
            "Discard 2 changed lines and the mode change in run.sh? You can't undo this."
        );
        assert_eq!(both.action(), "Discard 2 Lines and Mode Change");
        assert_eq!(
            with_mode(selection(0, 1)).action(),
            "Discard 1 Line and Mode Change"
        );
        for prompt in [alone.prompt(), both.prompt()] {
            assert!(!prompt.contains("100"), "no octal: {prompt}");
        }
    }

    /// Phase 14's QA item 5b and R1.2: every destructive prompt but an amend's — whose fixed
    /// sentences R10.6 sets — ends in the frame's "You can't undo this.", never "this action".
    /// Caught by: one operation's prompt left on the old ending.
    #[test]
    fn every_destructive_prompt_ends_in_the_frame() {
        let every = [
            lines("a.rs", selection(1, 0)),
            Consequence::DiscardFiles {
                files: vec![modified("a.rs", Some(1)), untracked("b", 1)],
            },
            checkout(),
            lock("/r/.git/index.lock", Duration::from_secs(20), 0),
        ];
        for consequence in every {
            let prompt = consequence.prompt();
            assert!(prompt.ends_with(" You can't undo this."), "{prompt}");
            assert!(!prompt.contains("this action"), "{prompt}");
        }
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
            chunk: false,
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

    /// R10.6 as the redesign of 2026-10-10 fixed it: one sentence for each reason an amend is
    /// confirmed — a remote has the commit (its upstream named, or "a remote"), git keeps no
    /// reflog — the id named in each, both where both hold; the button "Amend". Caught by: a
    /// subject or a force-push sentence back in the prompt, or a reason left unsaid.
    #[test]
    fn an_amend_says_why_it_is_confirmed_in_fixed_sentences() {
        let commit = oid(0xab);
        let amend = |published, reflog| Consequence::Amend {
            commit,
            published,
            reflog,
        };
        let upstream = || Publication::Upstream(RefName::new("refs/remotes/origin/main"));
        assert_eq!(commit.short().as_str(), "abababa");
        assert_eq!(
            amend(upstream(), Reflog::Written).prompt(),
            "abababa is already on origin/main. Amending it rewrites history others may have."
        );
        assert_eq!(
            amend(Publication::SomeRemote, Reflog::Written).prompt(),
            "abababa is already on a remote. Amending it rewrites history others may have."
        );
        assert_eq!(
            amend(Publication::Unpublished, Reflog::NotWritten).prompt(),
            "abababa can't be recovered after this: this repository keeps no reflog."
        );
        assert_eq!(
            amend(upstream(), Reflog::NotWritten).prompt(),
            "abababa is already on origin/main. Amending it rewrites history others may have. \
             abababa can't be recovered after this: this repository keeps no reflog."
        );
        assert_eq!(
            amend(Publication::Unpublished, Reflog::Written).prompt(),
            "Amend abababa? The old commit stays in Show Lost Commits."
        );
        for published in [
            Publication::Unpublished,
            upstream(),
            Publication::SomeRemote,
        ] {
            for reflog in [Reflog::Written, Reflog::NotWritten] {
                let consequence = amend(published.clone(), reflog);
                assert_eq!(consequence.action(), "Amend");
                assert_eq!(consequence.name(), "Amend");
                assert_eq!(consequence.amended(), Some(commit));
            }
        }
    }

    /// R1.5 as the redesign amended it: an amend git logs and no remote has is recoverable and
    /// asks nothing; one a remote has, or one git keeps no reflog for, is confirmed; and every
    /// other destructive operation is confirmed always. Caught by: a published or unlogged amend
    /// run unconfirmed, or a discard let through without a confirmation.
    #[test]
    fn only_an_amend_git_logs_and_no_remote_has_runs_unconfirmed() {
        let amend = |published, reflog| Consequence::Amend {
            commit: oid(0xab),
            published,
            reflog,
        };
        assert!(!amend(Publication::Unpublished, Reflog::Written).needs_confirming());
        assert!(amend(Publication::Unpublished, Reflog::NotWritten).needs_confirming());
        for published in [
            Publication::Upstream(RefName::new("refs/remotes/origin/main")),
            Publication::SomeRemote,
        ] {
            for reflog in [Reflog::Written, Reflog::NotWritten] {
                assert!(amend(published.clone(), reflog).needs_confirming());
            }
        }
        let discard = lines("a.rs", selection(1, 0));
        assert!(discard.needs_confirming());
        assert_eq!(discard.amended(), None);
        assert!(lock("/r/.git/index.lock", Duration::from_secs(1), 0).needs_confirming());
        assert!(checkout().needs_confirming());
        assert!(
            Consequence::DiscardFiles {
                files: vec![modified("a.rs", Some(1))],
            }
            .needs_confirming()
        );
    }

    /// Phase 11's QA: each operation's name, in Fork's imperative form, as the activity
    /// popover lists it; a path quoted as git quotes it, so a name cannot rewrite the list.
    #[test]
    fn each_operation_is_named_in_forks_imperative_form() {
        assert_eq!(
            lines("src/a.rs", selection(2, 0)).name(),
            "Discard lines of src/a.rs"
        );
        assert_eq!(
            lines("a\nb", selection(1, 0)).name(),
            "Discard lines of \"a\\nb\""
        );
        let one = Consequence::DiscardFiles {
            files: vec![modified("a.rs", Some(1))],
        };
        assert_eq!(one.name(), "Discard 1 file");
        let two = Consequence::DiscardFiles {
            files: vec![modified("a.rs", Some(1)), modified("b.rs", None)],
        };
        assert_eq!(two.name(), "Discard 2 files");
        assert_eq!(checkout().name(), "Create branch 'topic'");
        assert_eq!(
            lock("/r/.git/index.lock", Duration::ZERO, 0).name(),
            "Remove index.lock"
        );
        let amend = Consequence::Amend {
            commit: oid(0xab),
            published: Publication::Unpublished,
            reflog: Reflog::Written,
        };
        assert_eq!(amend.name(), "Amend");
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
             corrupt the index. You can't undo this."
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

    /// A name a repository's author chose — a path, a remote branch — cannot rewrite the prompt
    /// around it: a newline, a line separator and a right-to-left override are escaped as git
    /// escapes a C-quoted path, and the name is quoted, as git quotes it, once anything in it is.
    #[test]
    fn a_path_or_a_remotes_name_cannot_rewrite_the_prompt() {
        let path = "a\nYou can undo this.\u{202e}txt.exe";
        let consequence = Consequence::DiscardFiles {
            files: vec![modified(path, Some(1))],
        };
        assert_eq!(
            consequence.prompt(),
            "Discard all changes in \"a\\nYou can undo \
             this.\\342\\200\\256txt.exe\"? You can't undo this."
        );
        let quote = lines("say \"hi\"\\", selection(1, 0));
        assert_eq!(
            quote.prompt(),
            "Discard 1 changed line in \"say \\\"hi\\\"\\\\\"? You can't undo this."
        );
        let chunk = Consequence::DiscardLines {
            path: RepoPath::from("x\u{2028}y"),
            index: None,
            working_tree: oid(2),
            on_disk: oid(4),
            executable: false,
            selection: selection(0, 2),
            mode: None,
            chunk: true,
            patch: Patch::empty(),
        };
        assert_eq!(
            chunk.prompt(),
            "Discard this chunk (2 lines) in \"x\\342\\200\\250y\"? You can't undo this."
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
        let remote = Consequence::Amend {
            commit: oid(0xab),
            published: Publication::Upstream(RefName::new(
                "refs/remotes/origin/x\u{2028}Amending is safe\u{7}",
            )),
            reflog: Reflog::Written,
        };
        assert_eq!(
            remote.prompt(),
            "abababa is already on \"origin/x\\342\\200\\250Amending is safe\\a\". Amending it \
             rewrites history others may have."
        );
        let plain = Consequence::DiscardFiles {
            files: vec![modified("docs/naïve café.md", Some(2))],
        };
        assert!(
            plain.prompt().contains("in docs/naïve café.md? You can't"),
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
