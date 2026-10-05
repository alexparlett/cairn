//! Which diff algorithm the user's `git diff` uses for a file, so the content query can ask
//! `git diff-tree` for the same one.
//!
//! Plumbing never reads `diff.algorithm` — it is porcelain's (`git_diff_ui_config`) — so
//! it is read here and passed as `--diff-algorithm`. From git 2.40 a diff driver may name an
//! algorithm of its own (`diff.<driver>.algorithm`), and that beats `diff.algorithm` for the
//! path whose OLD side's `diff` attribute names the driver (`run_diff` in git's `diff.c`
//! looks the driver up by `one->path`). `diff-tree` applies a driver's algorithm itself, but
//! only when no `--diff-algorithm` is given, so for such a path no flag is passed. Before
//! 2.40 git has no such key and ignores it, and so does this.
//!
//! Whether a driver applies is asked of git (`crate::reads::diff_attributes`), and only when
//! it can matter: the git in use honours driver algorithms and the configuration names one
//! git accepts. The rules were read from git's source at v2.39.5, v2.40.0 and v2.56.0 and
//! reproduced against each, and against 2.30.9 and 2.32.7.

use cairn_model::RepoPath;
use gix::bstr::BString;

use super::git_config::{invalid, last_value};
use crate::ops::{GitBinary, GitVersion};
use crate::reads::{Algorithm, DiffAttribute, diff_attributes};
use crate::{Cancel, Error, Repository};

/// The first git that reads `diff.<driver>.algorithm`.
const DRIVERS_FROM: GitVersion = GitVersion {
    major: 2,
    minor: 40,
    patch: 0,
};

/// Which algorithm git diffs one path with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PathAlgorithm {
    /// `diff.algorithm`, or git's default: passed as `--diff-algorithm`.
    Configured(Algorithm),
    /// The one the path's diff driver names, which beats `diff.algorithm`.
    Driver(Algorithm),
}

impl PathAlgorithm {
    /// The flag a read of this path alone passes: none for a driver's, which `diff-tree`
    /// then applies itself, as the user's `git diff -- <path>` does.
    pub(super) fn flag(self) -> Option<Algorithm> {
        match self {
            Self::Configured(algorithm) => Some(algorithm),
            Self::Driver(_) => None,
        }
    }
}

/// The algorithms the user's configuration asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Algorithms {
    /// `diff.algorithm`, or myers, git's default.
    configured: Algorithm,
    /// Each driver that names an algorithm git accepts, when the git in use reads them.
    drivers: Vec<(BString, Algorithm)>,
}

impl Algorithms {
    /// `diff.algorithm` as porcelain reads it — a value git refuses, or the bare key, is
    /// [`Error::InvalidConfig`], as the user's `git diff` refuses to run on it — and the
    /// drivers' algorithms, which git reads only from 2.40.
    pub(super) fn read(repo: &gix::Repository, version: GitVersion) -> Result<Self, Error> {
        Self::from_config(repo.config_snapshot().plumbing(), version)
    }

    fn from_config(file: &gix::config::File, version: GitVersion) -> Result<Self, Error> {
        let configured = match last_value(file, "diff", None, "algorithm") {
            None => Algorithm::Myers,
            Some(value) => value
                .as_deref()
                .and_then(|value| Algorithm::parse(value))
                .ok_or_else(|| invalid("diff.algorithm", value))?,
        };
        let mut drivers: Vec<(BString, Algorithm)> = Vec::new();
        if version >= DRIVERS_FROM {
            let mut names: Vec<BString> = file
                .sections_by_name("diff")
                .into_iter()
                .flatten()
                .filter_map(|section| {
                    section
                        .header()
                        .subsection_name()
                        .map(|name| name.to_owned())
                })
                .collect();
            names.sort();
            names.dedup();
            for name in names {
                // A value git cannot parse is ignored by git when it diffs (`set_diff_algorithm`
                // fails and nothing changes), so it is no driver algorithm here either. The
                // bare key makes git refuse its whole configuration, which `diff-tree` then
                // does itself.
                if let Some(Some(value)) = last_value(file, "diff", Some(&name), "algorithm")
                    && let Some(algorithm) = Algorithm::parse(&value)
                {
                    drivers.push((name, algorithm));
                }
            }
        }
        Ok(Self {
            configured,
            drivers,
        })
    }

    /// What every file is diffed with unless its driver names an algorithm.
    #[cfg(test)]
    pub(super) fn configured(&self) -> Algorithm {
        self.configured
    }

    /// The algorithm git diffs each file with, by its old path: the driver's where the
    /// path's driver names one, `diff.algorithm` otherwise. Starts a process only when
    /// some driver names one.
    pub(super) fn for_old_paths(
        &self,
        git: &GitBinary,
        repo: &Repository,
        paths: &[&RepoPath],
        cancel: &impl Cancel,
    ) -> Result<Vec<PathAlgorithm>, Error> {
        if self.drivers.is_empty() {
            return Ok(vec![
                PathAlgorithm::Configured(self.configured);
                paths.len()
            ]);
        }
        Ok(diff_attributes(git, repo, paths, cancel)?
            .into_iter()
            .map(|attribute| match attribute {
                DiffAttribute::Driver(name) => self
                    .drivers
                    .iter()
                    .find(|(driver, _)| driver == &name)
                    .map_or(
                        PathAlgorithm::Configured(self.configured),
                        |(_, algorithm)| PathAlgorithm::Driver(*algorithm),
                    ),
                DiffAttribute::Unspecified | DiffAttribute::Set | DiffAttribute::Unset => {
                    PathAlgorithm::Configured(self.configured)
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEFORE: GitVersion = GitVersion {
        major: 2,
        minor: 39,
        patch: 5,
    };

    const FROM: GitVersion = DRIVERS_FROM;

    fn read(config: &str, version: GitVersion) -> Result<Algorithms, Error> {
        Algorithms::from_config(&gix::config::File::try_from(config).unwrap(), version)
    }

    /// `diff.algorithm` as porcelain reads it.
    #[test]
    fn diff_algorithm_is_read_the_way_git_diff_reads_it() {
        assert_eq!(read("", BEFORE).unwrap().configured, Algorithm::Myers);
        assert_eq!(
            read("[diff]\n\talgorithm = Histogram\n", BEFORE)
                .unwrap()
                .configured,
            Algorithm::Histogram
        );
        assert_eq!(
            read(
                "[diff]\n\talgorithm = patience\n[diff]\n\talgorithm = minimal\n",
                BEFORE
            )
            .unwrap()
            .configured,
            Algorithm::Minimal,
            "the last value is the one git uses"
        );
        for refused in ["[diff]\n\talgorithm = fast\n", "[diff]\n\talgorithm\n"] {
            assert!(
                matches!(read(refused, BEFORE), Err(Error::InvalidConfig { .. })),
                "{refused:?} was accepted"
            );
        }
    }

    /// A driver counts only from git 2.40, only with a value git parses, by its exact name,
    /// the last value winning. Caught by: honouring drivers on a git that ignores them, or a
    /// value git ignores when it diffs. Which path a driver applies to is git's answer,
    /// decided against real git in `crates/cairn-git/tests/diff/content.rs`.
    #[test]
    fn a_drivers_algorithm_counts_from_git_2_40_when_git_can_use_it() {
        let config = "[diff \"pat\"]\n\talgorithm = minimal\n[diff \"pat\"]\n\talgorithm = Patience\n\
                      [diff \"bad\"]\n\talgorithm = quick\n[diff \"none\"]\n\txfuncname = x\n\
                      [diff \"Pat\"]\n\talgorithm = histogram\n";
        assert_eq!(
            read(config, FROM).unwrap().drivers,
            vec![
                (BString::from("Pat"), Algorithm::Histogram),
                (BString::from("pat"), Algorithm::Patience),
            ],
            "a subsection keeps its case, and an unparseable value is no algorithm"
        );
        assert!(read(config, BEFORE).unwrap().drivers.is_empty());
        assert_eq!(read(config, FROM).unwrap().configured(), Algorithm::Myers);
    }
}
