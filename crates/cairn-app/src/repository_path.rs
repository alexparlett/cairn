//! Which repository the window opens (R5.1).
//!
//! The whole of Cairn's repository selection, deliberately: the first
//! command-line argument, or the process working directory when there is none.
//! **Not a picker, not a manager, not tabs and not a recent list** — R5.3 parks
//! that choice in the design spine's "Still open", and an argument commits to
//! none of them. A second way to choose a repository here means the packet has
//! been left. Resolution is a pure function so a test can decide the rule
//! without launching a window.

use std::ffi::OsString;
use std::path::PathBuf;

/// The repository to open.
///
/// `arguments` is the process arguments **as the process receives them**,
/// program name and all: dropping it is part of the rule, so it happens inside
/// the function the tests decide rather than at the call site they cannot
/// reach. Everything after the first real argument is ignored — one repository
/// at a time (R5.3), so a second path is not a second window.
pub fn chosen(
    arguments: impl IntoIterator<Item = OsString>,
    working_directory: PathBuf,
) -> PathBuf {
    arguments
        .into_iter()
        .nth(1)
        .filter(|argument| !argument.is_empty())
        .map_or(working_directory, PathBuf::from)
}

/// Where the process was started, for the default above. A failure here is not
/// worth a dialog: `.` is what every shell tool falls back to, and discovery
/// from it fails with a message naming a path either way.
pub fn working_directory() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Process arguments as `std::env::args_os` yields them: program name
    /// first. Every case below goes through this, so a `chosen` that forgot to
    /// drop it fails all of them rather than none.
    fn args(list: &[&str]) -> Vec<OsString> {
        std::iter::once(OsString::from("/usr/bin/cairn"))
            .chain(list.iter().map(OsString::from))
            .collect()
    }

    /// The program name is not a path. The mutation it catches: a `chosen` that
    /// took the argument list whole and read its first entry would open the
    /// repository containing the Cairn binary, which from a checkout looks
    /// exactly like it worked.
    #[test]
    fn the_program_name_is_not_the_repository() {
        assert_eq!(
            chosen(args(&[]), PathBuf::from("/home/someone/project")),
            PathBuf::from("/home/someone/project"),
            "the program name was taken for a path"
        );
        assert_eq!(
            chosen(args(&["/srv/repo"]), PathBuf::from("/elsewhere")),
            PathBuf::from("/srv/repo")
        );
    }

    /// R5.1's default. No argument means the directory Cairn was started in,
    /// which is what makes `cairn` inside a checkout do the obvious thing.
    #[test]
    fn no_argument_means_the_working_directory() {
        assert_eq!(
            chosen(args(&[]), PathBuf::from("/home/someone/project")),
            PathBuf::from("/home/someone/project")
        );
    }

    #[test]
    fn the_first_argument_is_the_repository() {
        assert_eq!(
            chosen(args(&["/srv/repo"]), PathBuf::from("/elsewhere")),
            PathBuf::from("/srv/repo")
        );
    }

    /// R5.3: one repository at a time. Inventing a meaning for a second path is
    /// how a repository manager arrives by accident.
    #[test]
    fn later_arguments_are_ignored() {
        assert_eq!(
            chosen(args(&["/srv/one", "/srv/two"]), PathBuf::from("/elsewhere")),
            PathBuf::from("/srv/one")
        );
    }

    /// An empty argument is a shell accident, not a path: treating it as one
    /// opens the working directory's repository while reporting the empty
    /// string, which is worse than either.
    #[test]
    fn an_empty_argument_falls_back_to_the_working_directory() {
        assert_eq!(
            chosen(args(&[""]), PathBuf::from("/elsewhere")),
            PathBuf::from("/elsewhere")
        );
    }

    /// A relative path is kept as written, so the message naming it when
    /// discovery fails (R5.2) is the path the reader typed.
    #[test]
    fn a_relative_path_is_passed_through_as_written() {
        assert_eq!(
            chosen(args(&["../sibling"]), PathBuf::from("/elsewhere")),
            PathBuf::from("../sibling")
        );
    }
}
