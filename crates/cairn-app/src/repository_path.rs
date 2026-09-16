//! Which repository the window opens (R5.1): the first command-line argument,
//! or the working directory.
//!
//! The whole of Cairn's repository selection — no picker, no manager, no tabs,
//! no recent list, which R5.3 parks in the design spine's "Still open". A second
//! way to choose a repository here means the packet has been left.

use std::ffi::OsString;
use std::path::PathBuf;

/// `arguments` is the process arguments as the process receives them, program
/// name and all: dropping it is part of the rule, so it happens here rather than
/// at a call site no test can reach. Everything after the first real argument is
/// ignored — one repository at a time (R5.3).
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

/// Where the process was started. A failure is not worth a dialog: `.` is what
/// every shell tool falls back to, and discovery from it fails with a message
/// naming a path either way.
pub fn working_directory() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Program name first, as `std::env::args_os` yields it. Every case below
    /// goes through this, so a `chosen` that forgot to drop it fails all of
    /// them.
    fn args(list: &[&str]) -> Vec<OsString> {
        std::iter::once(OsString::from("/usr/bin/cairn"))
            .chain(list.iter().map(OsString::from))
            .collect()
    }

    /// Caught by: reading the argument list's first entry, which opens the
    /// repository containing the Cairn binary — from a checkout, indistinguishable
    /// from working.
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

    /// R5.1's default: the directory Cairn was started in.
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

    /// An empty argument is a shell accident: treating it as a path opens the
    /// working directory while reporting the empty string.
    #[test]
    fn an_empty_argument_falls_back_to_the_working_directory() {
        assert_eq!(
            chosen(args(&[""]), PathBuf::from("/elsewhere")),
            PathBuf::from("/elsewhere")
        );
    }

    /// Kept as written, so R5.2's message names the path the reader typed.
    #[test]
    fn a_relative_path_is_passed_through_as_written() {
        assert_eq!(
            chosen(args(&["../sibling"]), PathBuf::from("/elsewhere")),
            PathBuf::from("../sibling")
        );
    }
}
