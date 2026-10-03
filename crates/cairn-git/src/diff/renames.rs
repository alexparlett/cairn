//! Which rename and copy detection the user's configuration asks for, and whether git's
//! limit cut the search short.
//!
//! `git diff-tree` is plumbing and reads neither key the way `git log` and `git show` do:
//! `diff.renames` not at all, `diff.renameLimit` only as a default for `-l`. So the
//! configuration is read here, in process, through gix's view of the same files git reads,
//! and parsed by git's own rules (`git_config_rename`, `git_parse_maybe_bool`,
//! `git_parse_signed` in git's `diff.c` and `parse.c`); then detection is passed to git
//! spelled out, `-M` or `-C` with `-l<limit>`, so what git searches is exactly what the
//! user's `git show` would.
//!
//! Whether the limit cut the search short is decided from the answer and the limit, never
//! from git's stderr, where the warning is prose in the user's language. git skips its
//! exhaustive stage when `sources x destinations > limit x limit`, counting what the exact
//! (and, for renames, the basename) stage left unpaired (`too_many_rename_candidates` in
//! `diffcore-rename.c`). When it skips, those leftovers are exactly the answer's unpaired
//! paths; when it does not, the answer's unpaired paths are a subset of what it compared.
//! So the inequality over the answer's own counts holds exactly when the search was cut
//! short — with the set of sources counted as the git version in use counts it:
//!
//! - renames, git 2.31 and later: the deletions left unpaired, since sources the exact and
//!   basename stages paired are culled before the check;
//! - renames, git 2.30: every deletion, paired or not, since 2.30 culls nothing;
//! - copies, every version: every deletion and every modified file, paired or not, since a
//!   source may be copied more than once and none is culled.
//!
//! A limit of zero or less is "no limit" from git 2.33, and 32,767 before it; the default is
//! 1,000 from git 2.33 and 400 before it. Every one of these is read from git's source at
//! v2.30.0, v2.31.0, v2.32.0, v2.33.0 and v2.56.0, and the version that links decides the
//! tests (`crates/cairn-git/tests/diff/changes.rs`).

use cairn_model::{ChangeStatus, ChangedFile};

use super::RenameDetection;
use super::git_config::{invalid, last_value, parse_bool, parse_int};
use crate::Error;
use crate::ops::GitVersion;
use crate::reads::Detection;

/// What `diff.renames` asks for, as `git_config_rename` reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Off,
    Renames,
    Copies,
}

/// The two keys as the user's configuration sets them, parsed the way git parses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Configured {
    mode: Mode,
    /// `diff.renameLimit`, or `None` when it is not set and git's default applies.
    limit: Option<i64>,
}

/// The search git is asked for, and how to read its answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Search {
    mode: Mode,
    /// The `-l` passed: what the user configured, or the default of the git in use, and
    /// `0` for any limit git treats as none.
    argument: u32,
    /// The limit git applies with that argument; `None` for none at all.
    applied: Option<u32>,
    version: GitVersion,
}

const UNLIMITED_FROM: GitVersion = GitVersion {
    major: 2,
    minor: 33,
    patch: 0,
};
const CULLED_FROM: GitVersion = GitVersion {
    major: 2,
    minor: 31,
    patch: 0,
};

/// What git's `-l0` means before 2.33.
const OLD_CEILING: u32 = 32_767;

impl Configured {
    /// The two keys from the configuration gix read when the repository was opened.
    pub(super) fn read(repo: &gix::Repository) -> Result<Self, Error> {
        let config = repo.config_snapshot();
        let file = config.plumbing();
        let renames = last_value(file, "diff", None, "renames");
        let limit = last_value(file, "diff", None, "renameLimit");

        let mode = match renames {
            None => Mode::Renames,
            Some(value) => parse_renames(value.as_ref().map(|value| value.as_slice()))
                .ok_or_else(|| invalid("diff.renames", value))?,
        };
        let limit = match limit {
            None => None,
            Some(value) => Some(
                value
                    .as_ref()
                    .and_then(|value| parse_int(value))
                    .ok_or_else(|| invalid("diff.renameLimit", value))?,
            ),
        };
        Ok(Self { mode, limit })
    }

    pub(super) fn search(self, version: GitVersion) -> Search {
        let unlimited_means_none = version >= UNLIMITED_FROM;
        let configured = self
            .limit
            .unwrap_or(if unlimited_means_none { 1000 } else { 400 });
        let argument = u32::try_from(configured).unwrap_or(0);
        let applied = match argument {
            0 if unlimited_means_none => None,
            0 => Some(OLD_CEILING),
            limit => Some(limit),
        };
        Search {
            mode: self.mode,
            argument,
            applied,
            version,
        }
    }
}

impl Search {
    pub(super) fn detection(self) -> Detection {
        match self.mode {
            Mode::Off => Detection::Off,
            Mode::Renames => Detection::Renames {
                limit: self.argument,
            },
            Mode::Copies => Detection::Copies {
                limit: self.argument,
            },
        }
    }

    /// How the search went, read from git's answer to it (module docs).
    pub(super) fn outcome(self, files: &[ChangedFile]) -> RenameDetection {
        if self.mode == Mode::Off {
            return RenameDetection::default();
        }
        RenameDetection {
            enabled: true,
            copies: self.mode == Mode::Copies,
            limit: self.applied,
            needed_limit: self
                .applied
                .and_then(|limit| self.needed_limit(limit, files)),
        }
    }

    /// `Some(n)` when the search was cut short, with `n` the limit that would have let it
    /// run: the larger of the two counts, which is the number git's warning names.
    fn needed_limit(self, limit: u32, files: &[ChangedFile]) -> Option<usize> {
        let destinations = files
            .iter()
            .filter(|file| matches!(file.status, ChangeStatus::Added))
            .count();
        let sources = match self.mode {
            Mode::Off => return None,
            Mode::Renames if self.version >= CULLED_FROM => files
                .iter()
                .filter(|file| matches!(file.status, ChangeStatus::Deleted))
                .count(),
            Mode::Renames => distinct_sources(files, |status| {
                matches!(
                    status,
                    ChangeStatus::Deleted | ChangeStatus::Renamed(_) | ChangeStatus::Copied(_)
                )
            }),
            Mode::Copies => {
                distinct_sources(files, |status| !matches!(status, ChangeStatus::Added))
            }
        };
        let candidates = (sources as u128) * (destinations as u128);
        (candidates > u128::from(limit) * u128::from(limit)).then_some(sources.max(destinations))
    }
}

/// How many different paths, on the old side, the entries whose status `counts` names
/// come from: a source copied twice is one source.
fn distinct_sources(files: &[ChangedFile], counts: impl Fn(ChangeStatus) -> bool) -> usize {
    let mut paths: Vec<&[u8]> = files
        .iter()
        .filter(|file| counts(file.status))
        .map(|file| file.old_path.as_bytes())
        .collect();
    paths.sort_unstable();
    paths.dedup();
    paths.len()
}

/// `git_config_rename`: the bare key and every true value is renames, `copies` and `copy`
/// are copies, every false value is off; `None` for anything git would refuse.
pub(super) fn parse_renames(value: Option<&[u8]>) -> Option<Mode> {
    let Some(value) = value else {
        return Some(Mode::Renames);
    };
    if value.eq_ignore_ascii_case(b"copies") || value.eq_ignore_ascii_case(b"copy") {
        return Some(Mode::Copies);
    }
    Some(if parse_bool(Some(value))? {
        Mode::Renames
    } else {
        Mode::Off
    })
}

#[cfg(test)]
mod tests {
    use cairn_model::{Oid, RepoPath};

    use super::*;

    fn version(minor: u32) -> GitVersion {
        GitVersion {
            major: 2,
            minor,
            patch: 0,
        }
    }

    fn entry(status: ChangeStatus, old: &str, new: &str) -> ChangedFile {
        let id = Oid::parse("07da224c7ec04501dfb451be161fa962effe1dc1").unwrap();
        ChangedFile {
            status,
            old_path: RepoPath::new(old),
            new_path: RepoPath::new(new),
            old_mode: None,
            new_mode: None,
            old_id: Some(id),
            new_id: Some(id),
        }
    }

    /// `git_config_rename` over each spelling. Real git reading the same values is
    /// `the_configuration_is_read_the_way_git_reads_it`, in `tests/diff/changes.rs`.
    #[test]
    fn diff_renames_reads_the_way_git_config_rename_does() {
        assert_eq!(parse_renames(None), Some(Mode::Renames), "the bare key");
        for value in ["true", "YES", "on", "1", "2", "0x10", "-1", " 7", "1k"] {
            assert_eq!(
                parse_renames(Some(value.as_bytes())),
                Some(Mode::Renames),
                "{value}"
            );
        }
        for value in ["false", "No", "OFF", "", "0", "00", "0x0", "0k"] {
            assert_eq!(
                parse_renames(Some(value.as_bytes())),
                Some(Mode::Off),
                "{value}"
            );
        }
        for value in ["copies", "Copy", "COPIES"] {
            assert_eq!(
                parse_renames(Some(value.as_bytes())),
                Some(Mode::Copies),
                "{value}"
            );
        }
        for value in ["copied", "maybe", "1x", "true ", "0x", "08", "99999999999"] {
            assert_eq!(parse_renames(Some(value.as_bytes())), None, "{value}");
        }
    }

    /// Caught by: one default or one meaning of zero for every git, which would pass git
    /// a different search from the one the user's own `git show` makes.
    #[test]
    fn the_limit_git_applies_depends_on_the_git() {
        let renames = |limit| Configured {
            mode: Mode::Renames,
            limit,
        };
        let new = renames(None).search(version(33));
        assert_eq!((new.argument, new.applied), (1000, Some(1000)));
        let old = renames(None).search(version(32));
        assert_eq!((old.argument, old.applied), (400, Some(400)));

        for limit in [Some(0), Some(-3)] {
            let new = renames(limit).search(version(56));
            assert_eq!((new.argument, new.applied), (0, None), "{limit:?}");
            let old = renames(limit).search(version(30));
            assert_eq!((old.argument, old.applied), (0, Some(32_767)), "{limit:?}");
        }
        let set = renames(Some(7)).search(version(30));
        assert_eq!((set.argument, set.applied), (7, Some(7)));
        assert_eq!(set.detection(), Detection::Renames { limit: 7 });
        let copies = Configured {
            mode: Mode::Copies,
            limit: Some(9),
        };
        assert_eq!(
            copies.search(version(56)).detection(),
            Detection::Copies { limit: 9 }
        );
        let off = Configured {
            mode: Mode::Off,
            limit: Some(9),
        };
        assert_eq!(off.search(version(56)).detection(), Detection::Off);
        assert_eq!(
            off.search(version(56)).outcome(&[]),
            RenameDetection::default()
        );
    }

    /// The three counts the module docs derive, at the boundary: a product equal to the
    /// square is a search that ran, one more is one cut short. Caught by: `>=` for `>`,
    /// counting paired deletions on a git that culls them, not counting them on one that
    /// does not, or counting a source copied twice as two.
    #[test]
    fn the_answer_says_it_was_cut_short_exactly_when_git_skipped_its_search() {
        use ChangeStatus::{Added, Copied, Deleted, Modified, Renamed};
        let exact = Renamed(cairn_model::Similarity::from_percent(100));
        let copy = Copied(cairn_model::Similarity::from_percent(100));
        // Two unpaired deletions, two unpaired additions, and three exact renames.
        let files = vec![
            entry(Deleted, "d1", "d1"),
            entry(Deleted, "d2", "d2"),
            entry(Added, "a1", "a1"),
            entry(Added, "a2", "a2"),
            entry(exact, "r1", "s1"),
            entry(exact, "r2", "s2"),
            entry(exact, "r3", "s3"),
            entry(Modified, "m", "m"),
            entry(copy, "m", "c1"),
            entry(copy, "m", "c2"),
        ];
        let search = |mode, limit, minor| {
            Configured {
                mode,
                limit: Some(limit),
            }
            .search(version(minor))
        };

        // Renames on a git that culls: 2 x 2 against the square of the limit.
        let culled = search(Mode::Renames, 2, 31).outcome(&files);
        assert!(!culled.was_cut_short(), "4 is not more than 4: {culled:?}");
        let culled = search(Mode::Renames, 1, 56).outcome(&files);
        assert_eq!(culled.needed_limit, Some(2), "{culled:?}");

        // Renames on 2.30: the paired deletions still count, and so does each distinct
        // source a copy was made from — (2 + 3 + 1) x 2 = 12.
        let whole = search(Mode::Renames, 3, 30).outcome(&files);
        assert_eq!(whole.needed_limit, Some(6), "12 is more than 9: {whole:?}");
        let whole = search(Mode::Renames, 4, 30).outcome(&files);
        assert!(!whole.was_cut_short(), "12 is not more than 16: {whole:?}");

        // Copies: every deletion and every modified file, a source copied twice counted
        // once — (2 + 3 + 1) x 2 = 12, on every git.
        for minor in [30, 56] {
            let copies = search(Mode::Copies, 3, minor).outcome(&files);
            assert_eq!(copies.needed_limit, Some(6), "{copies:?}");
            assert!(copies.copies && copies.enabled);
            let copies = search(Mode::Copies, 4, minor).outcome(&files);
            assert!(!copies.was_cut_short(), "{copies:?}");
        }

        // No limit at all is never cut short, and nothing on one side never is.
        assert!(!search(Mode::Renames, 0, 56).outcome(&files).was_cut_short());
        let one_sided = vec![entry(Deleted, "d", "d")];
        assert!(
            !search(Mode::Renames, 1, 56)
                .outcome(&one_sided)
                .was_cut_short()
        );
    }
}
