//! Which repository the window opens.

use std::ffi::OsString;
use std::path::PathBuf;

/// `arguments` includes the program name, which is dropped here. Later arguments are ignored.
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

/// Falls back to `.` on failure.
pub fn working_directory() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Program name first, as `std::env::args_os` yields it.
    fn args(list: &[&str]) -> Vec<OsString> {
        std::iter::once(OsString::from("/usr/bin/cairn"))
            .chain(list.iter().map(OsString::from))
            .collect()
    }

    /// Caught by: opening the argument list's first entry, the Cairn binary's own path.
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

    #[test]
    fn later_arguments_are_ignored() {
        assert_eq!(
            chosen(args(&["/srv/one", "/srv/two"]), PathBuf::from("/elsewhere")),
            PathBuf::from("/srv/one")
        );
    }

    /// Caught by: treating an empty argument as a path.
    #[test]
    fn an_empty_argument_falls_back_to_the_working_directory() {
        assert_eq!(
            chosen(args(&[""]), PathBuf::from("/elsewhere")),
            PathBuf::from("/elsewhere")
        );
    }

    /// Kept as written, so an error names the path the reader typed.
    #[test]
    fn a_relative_path_is_passed_through_as_written() {
        assert_eq!(
            chosen(args(&["../sibling"]), PathBuf::from("/elsewhere")),
            PathBuf::from("../sibling")
        );
    }
}
