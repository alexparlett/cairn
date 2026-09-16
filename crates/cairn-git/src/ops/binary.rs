//! Finding `git` and checking it is new enough.
//!
//! Done once, at startup, and loudly: a `git` that is missing or too old is
//! reported with the version Cairn needs, never worked around.

use std::fmt;
use std::path::{Path, PathBuf};

use super::{GitCommand, GitEnvironment};
use crate::Error;

/// The program name searched for on `PATH`.
const PROGRAM: &str = "git";

/// A `git` version, as `git --version` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    /// The oldest git Cairn runs with. A support policy, not a technical floor:
    /// raising it is the user's decision.
    pub const MINIMUM: Self = Self {
        major: 2,
        minor: 30,
        patch: 0,
    };

    /// Reads `git version 2.39.3 (Apple Git-146)`, `git version 2.47.0.windows.1`
    /// and `git version 2.30.0-rc1` alike; `None` for anything else.
    pub fn parse(output: &str) -> Option<Self> {
        let rest = output.trim().strip_prefix("git version ")?;
        let token = rest.split_whitespace().next()?;
        let mut parts = token.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts
            .next()
            .map(|part| {
                let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
                digits.parse().unwrap_or(0)
            })
            .unwrap_or(0);
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A `git` that has been found and checked; every invocation starts here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBinary {
    path: PathBuf,
    version: GitVersion,
    environment: GitEnvironment,
}

impl GitBinary {
    /// Locates `git` on this process's `PATH` and checks its version. Call once at startup.
    pub fn discover() -> Result<Self, Error> {
        Self::discover_with(GitEnvironment::new(|name| std::env::var_os(name)))
    }

    /// `environment` is both where `git` is searched for (its `PATH` entry) and
    /// what `git` then runs with, so it is found where its helpers will be.
    pub fn discover_with(environment: GitEnvironment) -> Result<Self, Error> {
        let path = locate(&environment)?;
        let version = probe(&path, &environment)?;
        if version < GitVersion::MINIMUM {
            return Err(Error::GitTooOld {
                path,
                found: version,
                required: GitVersion::MINIMUM,
            });
        }
        Ok(Self {
            path,
            version,
            environment,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn version(&self) -> GitVersion {
        self.version
    }

    pub fn environment(&self) -> &GitEnvironment {
        &self.environment
    }

    /// An invocation of this `git`, ready for its arguments.
    pub fn command(&self) -> GitCommand<'_> {
        GitCommand::new(&self.path, &self.environment)
    }
}

fn locate(environment: &GitEnvironment) -> Result<PathBuf, Error> {
    let mut searched = Vec::new();
    let Some(path) = environment.get("PATH") else {
        return Err(Error::GitNotFound {
            searched,
            required: GitVersion::MINIMUM,
        });
    };
    for directory in std::env::split_paths(path) {
        let candidate = directory.join(PROGRAM);
        if is_executable(&candidate) {
            return Ok(candidate);
        }
        searched.push(directory);
    }
    Err(Error::GitNotFound {
        searched,
        required: GitVersion::MINIMUM,
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn probe(path: &Path, environment: &GitEnvironment) -> Result<GitVersion, Error> {
    let output = GitCommand::new(path, environment).arg("--version").run()?;
    let text = output.stdout_text();
    GitVersion::parse(&text).ok_or_else(|| Error::GitVersionUnreadable {
        path: path.to_owned(),
        output: text.trim().to_owned(),
        required: GitVersion::MINIMUM,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u32, minor: u32, patch: u32) -> GitVersion {
        GitVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn parses_the_forms_git_prints_in_the_field() {
        for (printed, expected) in [
            ("git version 2.55.0\n", version(2, 55, 0)),
            ("git version 2.39.3 (Apple Git-146)", version(2, 39, 3)),
            ("git version 2.47.0.windows.1", version(2, 47, 0)),
            ("git version 2.30.0-rc1", version(2, 30, 0)),
            ("git version 2.30", version(2, 30, 0)),
        ] {
            assert_eq!(GitVersion::parse(printed), Some(expected), "{printed:?}");
        }
    }

    #[test]
    fn refuses_what_is_not_a_version() {
        for printed in [
            "",
            "banana",
            "git version",
            "git version x.y",
            "version 2.30.0",
        ] {
            assert_eq!(GitVersion::parse(printed), None, "{printed:?}");
        }
    }

    #[test]
    fn versions_order_numerically_not_lexically() {
        assert!(version(2, 9, 0) < version(2, 30, 0));
        assert!(version(2, 30, 0) >= GitVersion::MINIMUM);
        assert!(version(2, 29, 99) < GitVersion::MINIMUM);
        assert!(version(3, 0, 0) > GitVersion::MINIMUM);
    }

    #[test]
    fn the_minimum_displays_as_a_user_would_write_it() {
        assert_eq!(GitVersion::MINIMUM.to_string(), "2.30.0");
    }
}
