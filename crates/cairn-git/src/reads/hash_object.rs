//! `git hash-object --path=<p> -- <p>`: the object id one working-tree file has in git's
//! form, as the stale check before a discard of lines needs it (`docs/prd/staging-and-commit.md`
//! R3.7, R3.9).
//!
//! The drawn side of an unstaged or untracked diff is git's form of the file — after the
//! clean filter driver its attributes name, the line-ending conversion, `ident` and a
//! working-tree encoding (`super::working_tree`) — and its id is the one git printed on the
//! patch's `index` line. What a discard compares that id with must be computed the same way,
//! by git, from the file as it is now: `hash-object` with `--path` names the path whose
//! attributes apply, and the file argument is read from the working tree. Without `-w` it
//! writes nothing: no object, no index, no ref (`hashing_a_file_writes_nothing`, which holds
//! the git directory byte-identical after it). It reads no index entry for the path and
//! takes no lock; as a read of the working tree it runs the path's clean filter driver, as
//! `git diff-files` does for the diff it is compared with (D1 as amended, `crate::reads`).
//!
//! A symlink is not asked: `hash-object` opens the path and so reads the file a link points
//! to, where git stores a link as its target. The caller hashes a link itself (`crate::ops`'
//! re-check). A path that is not a file is the caller's to refuse before it asks; git's
//! failure to read one is [`Error::GitFailed`].

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt as _;

use cairn_model::{Oid, RepoPath};

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// One id and its newline, with room to spare: git prints nothing else.
const CEILING: usize = 256;

/// The id `path` has in git's form now. `path` is work-tree-relative and names a file, not a
/// link or a directory (module docs). Cancelled through `cancel` as any read is; an answer
/// that is not one object id is [`Error::UnexpectedGitOutput`].
pub(crate) fn hash_object(
    git: &GitBinary,
    repo: &Repository,
    path: &RepoPath,
    cancel: &impl Cancel,
) -> Result<Oid, Error> {
    let arguments = arguments(path);
    let output = git
        .read_invocation()
        .in_repository(repo)
        .args(&arguments)
        .start()?
        .collect(cancel, CEILING, |_| {})?;
    let text = output.stdout_text();
    let hex = text.strip_suffix('\n').unwrap_or(&text);
    Oid::parse(hex).map_err(|_| Error::UnexpectedGitOutput {
        arguments: arguments
            .iter()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        record: text.into_owned(),
    })
}

/// `hash-object --path=<p> -- <p>`: the attributes of `<p>`, and the file `<p>` read from
/// the working tree, where the read runs; `--` first, so a path starting with `-` is a path.
/// Never `-w`, `--stdin` or `--no-filters`.
fn arguments(path: &RepoPath) -> Vec<OsString> {
    let mut named = b"--path=".to_vec();
    named.extend_from_slice(path.as_bytes());
    vec![
        OsString::from("hash-object"),
        OsString::from_vec(named),
        OsString::from("--"),
        OsString::from_vec(path.as_bytes().to_vec()),
    ]
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;
    use crate::CancelSignal;
    use crate::ops::Askpass;

    /// The `git` Cairn would find, so the fixture is built by the binary the read runs.
    fn program() -> PathBuf {
        GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
            .unwrap()
            .path()
            .to_owned()
    }

    fn strings(arguments: &[OsString]) -> Vec<String> {
        arguments
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }

    /// Caught by: `-w` (a write), `--stdin` or `--no-filters` (another form than git's),
    /// the path left off `--path` (no attributes) or put before `--`.
    #[test]
    fn the_read_is_hash_object_of_the_path_in_its_own_attributes() {
        assert_eq!(
            strings(&arguments(&RepoPath::new("-dash name"))),
            ["hash-object", "--path=-dash name", "--", "-dash name"]
        );
    }

    /// A repository built by the machine's own `git`, removed when the test ends.
    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "cairn-hash-object-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            let fixture = Self(root);
            fixture.git(&["init", "-q", "."]);
            fixture
        }

        fn git(&self, args: &[&str]) -> String {
            let output = Command::new(program())
                .current_dir(&self.0)
                .args(args)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("HOME", &self.0)
                .env("GIT_AUTHOR_NAME", "A")
                .env("GIT_AUTHOR_EMAIL", "a@example.com")
                .env("GIT_COMMITTER_NAME", "A")
                .env("GIT_COMMITTER_EMAIL", "a@example.com")
                .output()
                .unwrap();
            assert!(output.status.success(), "git {args:?}: {output:?}");
            String::from_utf8(output.stdout).unwrap()
        }

        fn path(&self) -> &Path {
            &self.0
        }

        /// Every file under the git directory with its bytes, for a before-and-after.
        fn git_dir_snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
            let mut found = Vec::new();
            let mut pending = vec![self.0.join(".git")];
            while let Some(directory) = pending.pop() {
                for entry in std::fs::read_dir(&directory).unwrap().flatten() {
                    let path = entry.path();
                    if entry.file_type().unwrap().is_dir() {
                        pending.push(path);
                    } else {
                        found.push((path.clone(), std::fs::read(&path).unwrap()));
                    }
                }
            }
            found.sort();
            found
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The stopping rule's question, answered: `hash-object --path` reproduces the id git's
    /// own diff gives the working-tree side of a CRLF file under `core.autocrlf` and of a
    /// file under a clean filter — the `index` line of `git diff-files --full-index` — and
    /// not the id of the bytes on disk. And it writes nothing: the git directory is
    /// byte-identical after it, `.git/objects` included.
    #[test]
    fn hashing_a_file_gives_git_diffs_id_and_writes_nothing() {
        let fixture = Fixture::new();
        fixture.git(&["config", "core.autocrlf", "true"]);
        fixture.git(&["config", "core.safecrlf", "false"]);
        fixture.git(&["config", "filter.rot.clean", "tr a-zA-Z n-za-mN-ZA-M"]);
        fixture.git(&["config", "filter.rot.smudge", "tr a-zA-Z n-za-mN-ZA-M"]);
        std::fs::write(fixture.path().join(".gitattributes"), "*.r filter=rot\n").unwrap();
        std::fs::write(fixture.path().join("dos.txt"), "one\r\ntwo\r\n").unwrap();
        std::fs::write(fixture.path().join("x.r"), "hello\nworld\n").unwrap();
        fixture.git(&["add", "-A"]);
        fixture.git(&["commit", "-qm", "base"]);
        std::fs::write(fixture.path().join("dos.txt"), "one\r\ntwo\r\nthree\r\n").unwrap();
        std::fs::write(fixture.path().join("x.r"), "hello\nworld\nagain\n").unwrap();

        let git = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None)).unwrap();
        let repo = Repository::discover(fixture.path()).unwrap();
        for path in ["dos.txt", "x.r"] {
            let diff = fixture.git(&["diff-files", "-p", "--full-index", "--", path]);
            let drawn = diff
                .lines()
                .find_map(|line| line.strip_prefix("index "))
                .and_then(|ids| ids.split_once(".."))
                .map(|(_, new)| new.split(' ').next().unwrap().to_owned())
                .unwrap();
            let raw = fixture.git(&["hash-object", "--no-filters", "--", path]);
            let before = fixture.git_dir_snapshot();
            let hashed =
                hash_object(&git, &repo, &RepoPath::new(path), &CancelSignal::new()).unwrap();
            assert_eq!(fixture.git_dir_snapshot(), before, "{path}: hashing wrote");
            assert_eq!(hashed.to_string(), drawn, "{path}: not git diff's id");
            assert_ne!(
                hashed.to_string(),
                raw.trim(),
                "{path}: the fixture's form is its bytes, so it decides nothing"
            );
        }
    }
}
