//! `git check-attr diff`: which diff driver git applies to a path.
//!
//! Asked only when it can matter: when the git in use honours `diff.<driver>.algorithm`
//! (2.40 and later) and the configuration names an algorithm for some driver, the content
//! query has to know whether a path's driver is one of them, because a driver's algorithm
//! beats `diff.algorithm` for the user's `git diff` (`crate::diff`). git's own attribute
//! machinery answers — the same `git_check_attr` call `git diff` makes, reading the working
//! tree's `.gitattributes`, then the index's, `info/attributes` and `core.attributesFile`,
//! in a bare repository whatever git reads there — so the answer is git's by construction
//! rather than a second implementation of its rules.
//!
//! It runs `git check-attr --stdin -z diff`, the paths fed on stdin NUL-terminated, so no
//! path is ever read as an option and none counts against the command line's length. It is
//! query plumbing that writes nothing: it reads the index and never refreshes it, and runs
//! no program (`the_content_query_writes_nothing_and_runs_nothing`, in
//! `crates/cairn-git/tests/diff/content.rs`). The answer is three NUL-terminated fields per
//! path — the path, the attribute, its value — in the order the paths were given.

use std::ffi::OsString;

use cairn_model::RepoPath;

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// The `diff` attribute of one path, as `git check-attr` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DiffAttribute {
    Unspecified,
    /// `diff`: git's own text diff, no driver.
    Set,
    /// `-diff`: binary.
    Unset,
    /// `diff=<driver>`.
    Driver(Vec<u8>),
}

const ARGUMENTS: [&str; 4] = ["check-attr", "--stdin", "-z", "diff"];

/// The `diff` attribute of each of `paths`, in their order. Cancelled through `cancel` as
/// any read is, answering [`Error::ContentCancelled`]; an answer that does not name each
/// path in turn is [`Error::UnexpectedGitOutput`].
pub(crate) fn diff_attributes(
    git: &GitBinary,
    repo: &Repository,
    paths: &[&RepoPath],
    cancel: &impl Cancel,
) -> Result<Vec<DiffAttribute>, Error> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    if cancel.is_cancelled() {
        return Err(Error::ContentCancelled);
    }
    let mut input = Vec::new();
    for path in paths {
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    let mut fields: Vec<Vec<u8>> = Vec::with_capacity(paths.len() * 3);
    let outcome = git
        .read_invocation()
        .in_repository(repo)
        .args(ARGUMENTS.map(OsString::from))
        .input(input)
        .start()?
        .records(cancel, |field| fields.push(field.to_vec()), |_| {});
    match outcome {
        Ok(_) => read(paths, &fields).map_err(|record| Error::UnexpectedGitOutput {
            arguments: ARGUMENTS.join(" "),
            record,
        }),
        Err(Error::GitReadCancelled { .. }) => Err(Error::ContentCancelled),
        Err(other) => Err(other),
    }
}

fn read(paths: &[&RepoPath], fields: &[Vec<u8>]) -> Result<Vec<DiffAttribute>, String> {
    if fields.len() != paths.len() * 3 {
        return Err(format!(
            "{} fields for {} paths, where each path has three",
            fields.len(),
            paths.len()
        ));
    }
    paths
        .iter()
        .zip(fields.chunks(3))
        .map(|(path, triple)| match triple {
            [named, attribute, value]
                if named.as_slice() == path.as_bytes() && attribute.as_slice() == b"diff" =>
            {
                Ok(match value.as_slice() {
                    b"unspecified" => DiffAttribute::Unspecified,
                    b"set" => DiffAttribute::Set,
                    b"unset" => DiffAttribute::Unset,
                    driver => DiffAttribute::Driver(driver.to_vec()),
                })
            }
            _ => Err(format!(
                "an answer for another path or attribute than {path}'s diff"
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(text: &[&str]) -> Vec<Vec<u8>> {
        text.iter().map(|field| field.as_bytes().to_vec()).collect()
    }

    #[test]
    fn each_value_check_attr_prints_is_read_for_its_path() {
        let (a, b, c, d) = (
            RepoPath::new("a"),
            RepoPath::new("b c"),
            RepoPath::new("d"),
            RepoPath::new("e"),
        );
        let answer = read(
            &[&a, &b, &c, &d],
            &fields(&[
                "a",
                "diff",
                "unspecified",
                "b c",
                "diff",
                "set",
                "d",
                "diff",
                "unset",
                "e",
                "diff",
                "rust",
            ]),
        )
        .unwrap();
        assert_eq!(
            answer,
            vec![
                DiffAttribute::Unspecified,
                DiffAttribute::Set,
                DiffAttribute::Unset,
                DiffAttribute::Driver(b"rust".to_vec()),
            ]
        );
    }

    /// The read is query plumbing over one attribute, its paths on stdin rather than on the
    /// command line. Caught by: another verb, another attribute, or paths as arguments.
    #[test]
    fn the_read_is_check_attr_of_diff_with_its_paths_on_stdin() {
        assert_eq!(ARGUMENTS, ["check-attr", "--stdin", "-z", "diff"]);
    }

    /// Caught by: reading an answer for one path as another's, or a short answer as whole.
    #[test]
    fn an_answer_out_of_order_or_short_is_refused() {
        let (a, b) = (RepoPath::new("a"), RepoPath::new("b"));
        assert!(read(&[&a, &b], &fields(&["b", "diff", "x", "a", "diff", "y"])).is_err());
        assert!(read(&[&a, &b], &fields(&["a", "diff", "x"])).is_err());
        assert!(read(&[&a], &fields(&["a", "text", "x"])).is_err());
    }
}
