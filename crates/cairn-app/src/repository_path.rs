//! Which repository the window opens (R5.1).
//!
//! The whole of Cairn's repository selection, deliberately: the first
//! command-line argument, or the process working directory when there is none.
//! **Not a picker, not a manager, not tabs and not a recent list** — R5.3 parks
//! the choice between those in the design spine's "Still open", and a command
//! line argument is chosen precisely because it commits to none of them. If
//! this file grows a second way to choose a repository, the packet has been
//! left.
//!
//! Resolution is a pure function so the rule can be decided by a test rather
//! than by launching a window: what the argument list means is the part that
//! can be wrong, and reading the working directory is not.

use std::ffi::OsString;
use std::path::PathBuf;

/// The repository to open.
///
/// `arguments` is the process arguments **without** the program name.
/// Everything after the first is ignored: one repository is open at a time
/// (R5.3), so a second path is not a second window, and guessing what it might
/// have meant is how a picker starts.
pub fn chosen(
    arguments: impl IntoIterator<Item = OsString>,
    working_directory: PathBuf,
) -> PathBuf {
    arguments
        .into_iter()
        .next()
        .filter(|argument| !argument.is_empty())
        .map_or(working_directory, PathBuf::from)
}

/// Where the process was started, for the default above.
///
/// A failure here is not worth a dialog: `.` is what every shell tool falls
/// back to, and discovery from it fails with a message naming a path either
/// way.
pub fn working_directory() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
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

    /// R5.3: one repository at a time. A second path is not a second window,
    /// and inventing a meaning for it is how a repository manager arrives by
    /// accident.
    #[test]
    fn later_arguments_are_ignored() {
        assert_eq!(
            chosen(args(&["/srv/one", "/srv/two"]), PathBuf::from("/elsewhere")),
            PathBuf::from("/srv/one")
        );
    }

    /// An empty argument is not a path; it is a shell accident. Treating it as
    /// one would open the working directory's repository while reporting the
    /// empty string, which is worse than either.
    #[test]
    fn an_empty_argument_falls_back_to_the_working_directory() {
        assert_eq!(
            chosen(args(&[""]), PathBuf::from("/elsewhere")),
            PathBuf::from("/elsewhere")
        );
    }

    /// A relative path is kept as the reader wrote it, so the message naming it
    /// when discovery fails (R5.2) is the path they typed rather than one they
    /// have to translate.
    #[test]
    fn a_relative_path_is_passed_through_as_written() {
        assert_eq!(
            chosen(args(&["../sibling"]), PathBuf::from("/elsewhere")),
            PathBuf::from("../sibling")
        );
    }
}
