//! Whether what the diff thread keeps is still what git would answer: a stamp of every file
//! a kept answer was read from ([`cairn_git::DiffInputs`]), taken before each query, in
//! three tiers by what a change to them costs (PRD R4.5, amended in phase 04's QA).
//!
//! - **The configuration** — every file git or gix reads it from, includes and files that
//!   do not exist yet among them. Moved, the diff thread's repository handle is opened
//!   again (gix read its copy at open) and everything kept goes ([`Moved::Configuration`]).
//! - **What every path reads** — `info/attributes`, `core.attributesFile`, the system
//!   attributes, the working tree's `.gitmodules` — and **what the index holds** of them
//!   ([`cairn_git::StagedInputs`], read again only when the index file or `HEAD` moved,
//!   and compared by value, so a stat-only refresh keeps everything). Moved, the session
//!   and every kept answer go ([`Moved::Answers`]).
//! - **What one path reads** — the working tree's `.gitattributes` in each directory above
//!   it. Each kept answer records the directories it read ([`Dependence`]) and is checked
//!   on every hit; the session records the ones it has read, and goes when one moved.
//!
//! **A stamp taken while its file could still change unseen matches nothing.** A file's
//! times are only as fine as its filesystem keeps them, so a file changed twice within
//! that grain can show one stamp for two contents. Each query takes its stamps against the
//! time it started; a file whose modification or change time is within [`SETTLING`] of
//! that is unsettled, an answer read under an unsettled stamp is answered but not kept,
//! and the next query reads afresh whatever such a stamp covers. The same rule makes a
//! stamp taken after a read safe: whatever changed after the query started is unsettled.

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cairn_git::{DiffInputs, Repository, StagedInputs};
use cairn_model::{Oid, RepoPath};

/// How long after its last change a file's stamp is trusted to show the next change: the
/// coarsest time a filesystem Cairn may meet keeps (FAT's two seconds).
pub(super) const SETTLING: Duration = Duration::from_secs(2);

/// One file as one query found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Stamp {
    /// `None` when there was no file.
    file: Option<FileStamp>,
    settled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    modified_ns: i128,
    changed_ns: i128,
    len: u64,
    inode: u64,
    device: u64,
}

impl Stamp {
    /// `path` now, for a query that started at `started`. A link is followed; a link to
    /// nothing is stamped as itself.
    pub(super) fn of(path: &Path, started: SystemTime) -> Self {
        let found = std::fs::metadata(path).or_else(|_| std::fs::symlink_metadata(path));
        let file = found.ok().map(|meta| FileStamp {
            modified_ns: i128::from(meta.mtime()) * 1_000_000_000 + i128::from(meta.mtime_nsec()),
            changed_ns: i128::from(meta.ctime()) * 1_000_000_000 + i128::from(meta.ctime_nsec()),
            len: meta.size(),
            inode: meta.ino(),
            device: meta.dev(),
        });
        let since = started
            .checked_sub(SETTLING)
            .and_then(|since| since.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_nanos() as i128);
        let settled = file.is_none_or(|file| file.modified_ns.max(file.changed_ns) < since);
        Self { file, settled }
    }

    /// Whether `now` shows the file as this stamp did — never, for a stamp taken while the
    /// file could still change unseen.
    pub(super) fn still(&self, now: &Stamp) -> bool {
        self.settled && self.file == now.file
    }

    pub(super) fn is_settled(&self) -> bool {
        self.settled
    }
}

fn stamps(paths: &[PathBuf], started: SystemTime) -> Vec<Stamp> {
    paths.iter().map(|path| Stamp::of(path, started)).collect()
}

fn all_still(then: &[Stamp], now: &[Stamp]) -> bool {
    then.len() == now.len() && then.iter().zip(now).all(|(then, now)| then.still(now))
}

/// The directories whose `.gitattributes` an answer read, each as it was stamped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Dependence(Vec<(Vec<u8>, Stamp)>);

impl Dependence {
    pub(super) fn is_settled(&self) -> bool {
        self.0.iter().all(|(_, stamp)| stamp.is_settled())
    }

    /// Roughly what keeping it holds, for a kept answer's budget.
    pub(super) fn held_bytes(&self) -> usize {
        self.0
            .iter()
            .map(|(directory, _)| directory.len() + std::mem::size_of::<(Vec<u8>, Stamp)>())
            .sum()
    }
}

/// One query's view of the working tree's `.gitattributes`: each directory stamped once,
/// however many paths, answers and checks ask about it.
pub(super) struct Directories<'i> {
    inputs: &'i DiffInputs,
    started: SystemTime,
    stamped: HashMap<Vec<u8>, Stamp>,
}

impl<'i> Directories<'i> {
    pub(super) fn new(inputs: &'i DiffInputs, started: SystemTime) -> Self {
        Self {
            inputs,
            started,
            stamped: HashMap::new(),
        }
    }

    fn stamp(&mut self, directory: &[u8]) -> Stamp {
        if let Some(stamp) = self.stamped.get(directory) {
            return *stamp;
        }
        let stamp = match self.inputs.attributes_in(directory) {
            Some(file) => Stamp::of(&file, self.started),
            // A bare repository: no working tree, nothing to read.
            None => Stamp {
                file: None,
                settled: true,
            },
        };
        self.stamped.insert(directory.to_vec(), stamp);
        stamp
    }

    /// What reading `paths`' attributes depends on: every directory above each, once.
    pub(super) fn of<'p>(&mut self, paths: impl IntoIterator<Item = &'p RepoPath>) -> Dependence {
        let mut named: HashSet<&[u8]> = HashSet::new();
        let mut directories = Vec::new();
        for path in paths {
            for directory in DiffInputs::directories(path) {
                if named.insert(directory) {
                    directories.push(directory.to_vec());
                }
            }
        }
        Dependence(
            directories
                .into_iter()
                .map(|directory| {
                    let stamp = self.stamp(&directory);
                    (directory, stamp)
                })
                .collect(),
        )
    }

    /// Whether every directory `dependence` read is as it was.
    pub(super) fn still(&mut self, dependence: &Dependence) -> bool {
        dependence
            .0
            .iter()
            .all(|(directory, then)| then.still(&self.stamp(directory)))
    }
}

/// The directories a diff session has read attributes from, as they were stamped: gix's
/// attribute stack keeps the top of the tree for the life of the session, and every
/// directory a path shares with the one before it.
#[derive(Debug, Default)]
pub(super) struct SessionReads(HashMap<Vec<u8>, Stamp>);

impl SessionReads {
    /// Whether a session that read what this holds may answer about `dependence`'s paths:
    /// every directory it read among theirs is as it was.
    pub(super) fn still_for(&self, dependence: &Dependence) -> bool {
        dependence
            .0
            .iter()
            .all(|(directory, now)| self.0.get(directory).is_none_or(|then| then.still(now)))
    }

    pub(super) fn record(&mut self, dependence: &Dependence) {
        for (directory, stamp) in &dependence.0 {
            self.0.insert(directory.clone(), *stamp);
        }
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
}

/// What moved since the query before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Moved {
    Nothing,
    /// What every path reads, or what the index holds of it: the session and every kept
    /// answer go.
    Answers,
    /// The configuration: the handle is opened again, and everything kept goes with it.
    Configuration,
}

/// What [`Freshness::check`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Checked {
    pub(super) moved: Moved,
    /// Every configuration and global stamp settled: an answer read now may be kept.
    pub(super) settled: bool,
}

/// The index's part, and when it was last read.
#[derive(Debug)]
struct Staged {
    index: Stamp,
    head: Option<Oid>,
    /// `None` when it could not be read, which matches nothing.
    holds: Option<StagedInputs>,
}

/// The configuration and global tiers as the thread's handle, session and kept answers
/// were read under them; one per repository handle.
#[derive(Debug)]
pub(super) struct Freshness {
    configuration: Vec<Stamp>,
    global: Option<Vec<Stamp>>,
    staged: Option<Staged>,
}

impl Freshness {
    /// For a handle whose opening began at `opened_at`: its configuration, stamped against
    /// that time, so that a file changed while it was being read is unsettled.
    pub(super) fn opened(inputs: &DiffInputs, opened_at: SystemTime) -> Self {
        Self {
            configuration: stamps(inputs.configuration(), opened_at),
            global: None,
            staged: None,
        }
    }

    /// Before a query that started at `started`: what moved since the last. A
    /// configuration that moved is reported as it is found and nothing else is read; the
    /// handle's replacement starts afresh.
    pub(super) fn check(
        &mut self,
        repo: &Repository,
        inputs: &DiffInputs,
        started: SystemTime,
    ) -> Checked {
        let configuration = stamps(inputs.configuration(), started);
        if !all_still(&self.configuration, &configuration) {
            return Checked {
                moved: Moved::Configuration,
                settled: false,
            };
        }

        let global = stamps(inputs.global(), started);
        let mut moved = self
            .global
            .as_ref()
            .is_some_and(|then| !all_still(then, &global));
        let settled = configuration.iter().chain(&global).all(Stamp::is_settled);
        self.global = Some(global);

        let index = Stamp::of(inputs.index(), started);
        let head = repo.head_id();
        let unchanged = self
            .staged
            .as_ref()
            .is_some_and(|staged| staged.index.still(&index) && staged.head == head);
        if !unchanged {
            let holds = repo.staged_inputs().ok();
            if let Some(staged) = &self.staged {
                moved |= holds.is_none() || staged.holds.is_none() || staged.holds != holds;
            }
            self.staged = Some(Staged { index, head, holds });
        }

        Checked {
            moved: if moved {
                Moved::Answers
            } else {
                Moved::Nothing
            },
            settled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("cairn-freshness-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        if let Err(error) = std::fs::create_dir_all(&path) {
            panic!("making {}: {error}", path.display());
        }
        path
    }

    fn later(by: Duration) -> SystemTime {
        SystemTime::now() + by
    }

    /// The racy guard: a file changed within the settling time of a query's start is
    /// unsettled, and an unsettled stamp matches nothing — not even itself — so whatever
    /// it covers is read afresh; once settled, a stamp matches the same file and nothing
    /// else, a missing file included. Caught by: comparing times alone (two writes within
    /// one tick of a coarse clock look the same), or trusting a stamp taken in that tick.
    #[test]
    fn a_stamp_taken_while_its_file_may_still_change_matches_nothing() {
        let directory = scratch("racy");
        let file = directory.join("config");
        std::fs::write(&file, "one").unwrap();

        let racy = Stamp::of(&file, SystemTime::now());
        assert!(!racy.is_settled());
        assert!(!racy.still(&racy), "an unsettled stamp matched itself");

        // The same file, asked about by a query that starts once it has settled.
        let settled = Stamp::of(&file, later(SETTLING + Duration::from_secs(1)));
        assert!(settled.is_settled());
        assert!(settled.still(&Stamp::of(&file, later(SETTLING * 2))));

        std::fs::write(&file, "two, longer").unwrap();
        assert!(!settled.still(&Stamp::of(&file, later(SETTLING * 2))));

        let missing = Stamp::of(&directory.join("absent"), SystemTime::now());
        assert!(
            missing.is_settled(),
            "a file that is not there cannot change unseen"
        );
        assert!(missing.still(&Stamp::of(&directory.join("absent"), SystemTime::now())));
        std::fs::write(directory.join("absent"), "").unwrap();
        assert!(
            !missing.still(&Stamp::of(&directory.join("absent"), later(SETTLING * 2))),
            "a file created where there was none was not seen"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// A same-length rewrite through a new file (how git and most editors save) that kept
    /// the old modification time is seen, by the change time and the inode a modification
    /// time cannot fake. Caught by: a stamp of modification time and length alone.
    #[test]
    fn a_file_replaced_by_another_of_the_same_length_is_seen() {
        let directory = scratch("replaced");
        let file = directory.join("attributes");
        std::fs::write(&file, "a -diff\n").unwrap();
        let then = Stamp::of(&file, later(SETTLING * 2));
        let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
        let replacement = directory.join("attributes.lock");
        std::fs::write(&replacement, "b -diff\n").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&replacement)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        std::fs::rename(&replacement, &file).unwrap();
        assert!(!then.still(&Stamp::of(&file, later(SETTLING * 2))));
        let _ = std::fs::remove_dir_all(&directory);
    }

    /// A session that read a directory's attributes goes when that directory's moved, and
    /// not for a directory it never read. Caught by: keeping the session (gix's stack keeps
    /// the top of the tree's `.gitattributes` for its whole life).
    #[test]
    fn a_session_is_trusted_only_for_the_directories_it_read_as_they_were() {
        let directory = scratch("session");
        let a = |content: &str| {
            std::fs::write(directory.join(".gitattributes"), content).unwrap();
        };
        a("* -diff\n");
        let then = Stamp::of(&directory.join(".gitattributes"), later(SETTLING * 2));
        let mut reads = SessionReads::default();
        let read = Dependence(vec![(Vec::new(), then)]);
        reads.record(&read);
        assert!(reads.still_for(&read));
        assert!(
            reads.still_for(&Dependence(vec![(b"elsewhere".to_vec(), then)])),
            "a directory the session never read made it go"
        );
        a("* -diff\n*.rs diff\n");
        let now = Stamp::of(&directory.join(".gitattributes"), later(SETTLING * 2));
        assert!(!reads.still_for(&Dependence(vec![(Vec::new(), now)])));
        let _ = std::fs::remove_dir_all(&directory);
    }
}
