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

use crate::{Oid, RefName, RepoPath, Selection};

/// What one destructive operation will destroy, as the engine read it.
///
/// One variant per destructive operation of R1.5. Discarding files and
/// deleting untracked files share [`Consequence::DiscardFiles`]: Fork confirms
/// a selection of both in one dialog, with one prompt naming each kind
/// (`brainstorm.md` L8), and one prompt is one confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Consequence {
    /// Selected lines of one path's unstaged change, put back to the index's
    /// version (or, for an untracked file, removed from it) in the working
    /// tree.
    DiscardLines {
        path: RepoPath,
        /// The index entry's blob the lines were computed against; `None` for
        /// an untracked file, which has no entry.
        index: Option<Oid>,
        /// The working-tree file as git hashes it, the side the lines are
        /// taken from.
        working_tree: Oid,
        /// The lines selected in the unstaged diff (index to working tree):
        /// its added lines are deleted, its removed lines put back. The patch
        /// the operation applies is built from exactly this selection.
        selection: Selection,
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
        /// The working-tree file as git hashes it; `None` when it is deleted
        /// in the working tree, and discarding brings it back.
        working_tree: Option<Oid>,
        /// Changed lines of the unstaged diff; `None` for a change that has no
        /// lines (binary).
        lines: Option<usize>,
    },
    /// An untracked file, deleted.
    Untracked {
        /// The file as git hashes it.
        working_tree: Oid,
        bytes: u64,
    },
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
                selection,
            } => format!(
                "Do you want to discard {} in {}? You can't undo this action.",
                counted(selection.len(), "line", "lines"),
                quoted(path.as_bytes())
            ),
            Self::DiscardFiles { files } => discard_files_prompt(files),
            Self::Amend {
                commit,
                subject,
                published,
            } => {
                let short = commit.short();
                let short = short.as_str();
                let subject = escaped(subject.as_bytes());
                let replaces = format!(
                    "Replaces {short} '{subject}'. The old commit stays in Show Lost Commits."
                );
                match published {
                    Publication::Unpublished => replaces,
                    Publication::Upstream(upstream) => format!(
                        "{short} is already on {}. Sharing the amended commit needs a force \
                         push. {replaces}",
                        quoted(upstream.shorthand().as_bytes())
                    ),
                    Publication::SomeRemote => format!(
                        "{short} is already on a remote. Sharing the amended commit needs a \
                         force push. {replaces}"
                    ),
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

    /// The label of the button that confirms it.
    pub fn action(&self) -> String {
        match self {
            Self::DiscardLines {
                path: _,
                index: _,
                working_tree: _,
                selection,
            } => format!("Discard {}", counted(selection.len(), "Line", "Lines")),
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
            } => format!("Amend {}", commit.short().as_str()),
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

/// Fork's words (L8): "Do you want to discard the changes in 3 files? 2
/// modified (14 lines), 1 untracked file deleted (2.1 KiB). You can't undo
/// this action." One file is named by its path; a file deleted in the working
/// tree, which the discard brings back, is named as that.
fn discard_files_prompt(files: &[DiscardedFile]) -> String {
    let what = match files {
        [only] => quoted(only.path.as_bytes()),
        _ => counted(files.len(), "file", "files"),
    };
    let mut modified = 0usize;
    let mut lines = 0usize;
    let mut binary = 0usize;
    let mut restored = 0usize;
    let mut untracked = 0usize;
    let mut untracked_bytes = 0u64;
    for file in files {
        match &file.loss {
            FileLoss::Modified {
                index: _,
                working_tree: None,
                lines: _,
            } => restored += 1,
            FileLoss::Modified {
                index: _,
                working_tree: Some(_),
                lines: counted_lines,
            } => {
                modified += 1;
                match counted_lines {
                    Some(n) => lines += n,
                    None => binary += 1,
                }
            }
            FileLoss::Untracked {
                working_tree: _,
                bytes,
            } => {
                untracked += 1;
                untracked_bytes = untracked_bytes.saturating_add(*bytes);
            }
        }
    }
    let mut parts = Vec::with_capacity(3);
    if modified > 0 {
        let detail = match (modified - binary, binary) {
            (_, 0) => counted(lines, "line", "lines"),
            (0, _) => "binary".to_owned(),
            (_, binary) => format!("{}, {binary} binary", counted(lines, "line", "lines")),
        };
        parts.push(format!("{modified} modified ({detail})"));
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
                lines,
            },
        }
    }

    fn deleted(path: &str) -> DiscardedFile {
        DiscardedFile {
            path: RepoPath::from(path),
            loss: FileLoss::Modified {
                index: oid(1),
                working_tree: None,
                lines: Some(9),
            },
        }
    }

    fn untracked(path: &str, bytes: u64) -> DiscardedFile {
        DiscardedFile {
            path: RepoPath::from(path),
            loss: FileLoss::Untracked {
                working_tree: oid(3),
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
            selection,
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
            "Do you want to discard the changes in 3 files? 2 modified (14 lines), 1 untracked \
             file deleted (2.1 KiB). You can't undo this action."
        );
        assert_eq!(consequence.action(), "Discard Changes in 3 Files");
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
            "Do you want to discard the changes in 3 files? 3 untracked files deleted (2.0 \
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
            "Do you want to discard the changes in 2 files? 2 modified (binary). You can't \
             undo this action."
        );
        let mixed = Consequence::DiscardFiles {
            files: vec![modified("logo.png", None), modified("a.rs", Some(3))],
        };
        assert_eq!(
            mixed.prompt(),
            "Do you want to discard the changes in 2 files? 2 modified (3 lines, 1 binary). \
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
            "Do you want to discard the changes in 3 files? 1 modified (2 lines), 2 deleted \
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
            selection: selection(0, 1),
        };
        assert_eq!(
            one.prompt(),
            "Do you want to discard 1 line in new.txt? You can't undo this action."
        );
        assert_eq!(one.action(), "Discard 1 Line");
    }

    /// R10.6's text, and the force push named exactly when a remote has the commit.
    #[test]
    fn an_amend_names_the_commit_it_replaces_and_whether_a_remote_has_it() {
        let commit = oid(0xab);
        let amend = |published| Consequence::Amend {
            commit,
            subject: "Fix the parser".to_owned(),
            published,
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
