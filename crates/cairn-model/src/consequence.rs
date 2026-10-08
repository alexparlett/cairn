//! What a destructive operation will destroy, and the words a person is shown
//! for it.
//!
//! A [`Consequence`] is computed by the engine from the repository as it is,
//! and the prompt and the button's label are rendered from it here, beside the
//! type, so no text that names a count or a path can be typed apart from the
//! value it describes. [`crate::Confirmed`] carries one, with the prompt
//! rendered from it, and every destructive operation re-reads the state it
//! names immediately before it runs, refusing when anything moved
//! (`docs/prd/staging-and-commit.md` R1).
//!
//! Every count and path a prompt renders is read from the value: nothing here
//! looks at a repository, a clock or a setting, so the same `Consequence`
//! always renders the same words.

use std::path::PathBuf;
use std::time::Duration;

use crate::{Oid, RefName, RepoPath};

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
        /// Selected lines the working tree has and the index does not: they
        /// are deleted.
        added: usize,
        /// Selected lines the index has and the working tree does not: they
        /// are put back.
        removed: usize,
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
        /// How long ago it was last modified, when it was read.
        age: Duration,
        /// Its size when it was read; with `age`, what the re-check compares
        /// so a lock removed and made again since is refused.
        bytes: u64,
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
        /// in the working tree.
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
                added,
                removed,
            } => format!(
                "Do you want to discard {} in {}? You can't undo this action.",
                counted(added + removed, "line", "lines"),
                path.display()
            ),
            Self::DiscardFiles { files } => discard_files_prompt(files),
            Self::Amend {
                commit,
                subject,
                published,
            } => {
                let short = commit.short();
                let short = short.as_str();
                let replaces = format!(
                    "Replaces {short} '{subject}'. The old commit stays in Show Lost Commits."
                );
                match published {
                    Publication::Unpublished => replaces,
                    Publication::Upstream(upstream) => format!(
                        "{short} is already on {}. Sharing the amended commit needs a force \
                         push. {replaces}",
                        upstream.shorthand()
                    ),
                    Publication::SomeRemote => format!(
                        "{short} is already on a remote. Sharing the amended commit needs a \
                         force push. {replaces}"
                    ),
                }
            }
            Self::RemoveLock { path, age, bytes } => format!(
                "Remove {}? It was last changed {} ago and holds {}. Another program may still \
                 own it: removing a lock a running git holds can corrupt the repository.",
                path.display(),
                elapsed(*age),
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
                added,
                removed,
            } => format!("Discard {}", counted(added + removed, "Line", "Lines")),
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
                age: _,
                bytes: _,
            } => format!(
                "Remove {}",
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string())
            ),
        }
    }
}

/// Fork's words (L8): "Do you want to discard the changes in 3 files? 2
/// modified (14 lines), 1 untracked file deleted (2.1 KiB). You can't undo
/// this action." One file is named by its path.
fn discard_files_prompt(files: &[DiscardedFile]) -> String {
    let what = match files {
        [only] => only.path.display().into_owned(),
        _ => counted(files.len(), "file", "files"),
    };
    let mut modified = 0usize;
    let mut lines = 0usize;
    let mut binary = 0usize;
    let mut untracked = 0usize;
    let mut untracked_bytes = 0u64;
    for file in files {
        match &file.loss {
            FileLoss::Modified {
                index: _,
                working_tree: _,
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
    let mut parts = Vec::with_capacity(2);
    if modified > 0 {
        let detail = match (modified - binary, binary) {
            (_, 0) => counted(lines, "line", "lines"),
            (0, _) => "binary".to_owned(),
            (_, binary) => format!("{}, {binary} binary", counted(lines, "line", "lines")),
        };
        parts.push(format!("{modified} modified ({detail})"));
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

    fn untracked(path: &str, bytes: u64) -> DiscardedFile {
        DiscardedFile {
            path: RepoPath::from(path),
            loss: FileLoss::Untracked {
                working_tree: oid(3),
                bytes,
            },
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
        let deleted = Consequence::DiscardFiles {
            files: vec![untracked("scratch", 12)],
        };
        assert_eq!(
            deleted.prompt(),
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

    /// Caught by: counting only the added lines (what is deleted) or only the removed ones.
    #[test]
    fn discarded_lines_count_both_sides_of_the_selection() {
        let consequence = Consequence::DiscardLines {
            path: RepoPath::from("src/lib.rs"),
            index: Some(oid(1)),
            working_tree: oid(2),
            added: 1,
            removed: 1,
        };
        assert_eq!(
            consequence.prompt(),
            "Do you want to discard 2 lines in src/lib.rs? You can't undo this action."
        );
        assert_eq!(consequence.action(), "Discard 2 Lines");
        let one = Consequence::DiscardLines {
            path: RepoPath::from("new.txt"),
            index: None,
            working_tree: oid(2),
            added: 1,
            removed: 0,
        };
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
        let short = commit.short();
        let short = short.as_str();
        assert_eq!(
            amend(Publication::Unpublished).prompt(),
            format!(
                "Replaces {short} 'Fix the parser'. The old commit stays in Show Lost Commits."
            )
        );
        assert_eq!(
            amend(Publication::Upstream(RefName::new(
                "refs/remotes/origin/main"
            )))
            .prompt(),
            format!(
                "{short} is already on origin/main. Sharing the amended commit needs a force \
                 push. Replaces {short} 'Fix the parser'. The old commit stays in Show Lost \
                 Commits."
            )
        );
        assert!(
            amend(Publication::SomeRemote)
                .prompt()
                .starts_with(&format!("{short} is already on a remote. "))
        );
        assert_eq!(
            amend(Publication::Unpublished).action(),
            format!("Amend {short}")
        );
    }

    #[test]
    fn removing_a_lock_names_it_its_age_and_the_risk() {
        let consequence = Consequence::RemoveLock {
            path: PathBuf::from("/work/repo/.git/index.lock"),
            age: Duration::from_secs(3 * 60 + 20),
            bytes: 0,
        };
        assert_eq!(
            consequence.prompt(),
            "Remove /work/repo/.git/index.lock? It was last changed 3 minutes ago and holds 0 \
             bytes. Another program may still own it: removing a lock a running git holds can \
             corrupt the repository."
        );
        assert_eq!(consequence.action(), "Remove index.lock");
    }

    #[test]
    fn sizes_and_ages_read_in_their_largest_unit() {
        assert_eq!(size(0), "0 bytes");
        assert_eq!(size(1), "1 byte");
        assert_eq!(size(1023), "1023 bytes");
        assert_eq!(size(1024), "1.0 KiB");
        assert_eq!(size(5 * 1024 * 1024 + 512 * 1024), "5.5 MiB");
        assert_eq!(elapsed(Duration::from_secs(1)), "1 second");
        assert_eq!(elapsed(Duration::from_secs(59)), "59 seconds");
        assert_eq!(elapsed(Duration::from_secs(60)), "1 minute");
        assert_eq!(elapsed(Duration::from_secs(2 * 3600)), "2 hours");
        assert_eq!(elapsed(Duration::from_secs(3 * 86_400 + 5)), "3 days");
    }

    /// Caught by: a prompt that depends on anything but the value.
    #[test]
    fn the_same_consequence_always_renders_the_same_words() {
        let consequence = Consequence::DiscardFiles {
            files: vec![modified("a", Some(2)), untracked("b", 4096)],
        };
        assert_eq!(consequence.prompt(), consequence.clone().prompt());
        assert_eq!(consequence.action(), consequence.clone().action());
    }
}
