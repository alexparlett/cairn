//! `git stripspace --strip-comments`, the message on stdin: a merge's, a cherry-pick's or a
//! revert's `MERGE_MSG` cleaned as git's own editor cleans it before the commit box shows it
//! (`docs/prd/staging-and-commit.md` R6.10, C2, C32 — the fourth porcelain read, accepted by the
//! user on 2026-10-10).
//!
//! **Why git cleans it.** `MERGE_MSG` carries git's commentary for the editor — `# Conflicts:`
//! and the paths under it, and, under `commit.cleanup=scissors`, a scissors line and what follows
//! — each line starting with `core.commentChar` (or `core.commentString`), which is `#` unless the
//! user set another. `git commit -F -` keeps comment lines under its default cleanup, so a box
//! filled with `MERGE_MSG` as git wrote it would commit them; git's editor strips them. Which
//! lines are comments is git's to say, from the comment character it reads: `stripspace` run in
//! the repository reads `core.commentChar` itself (`builtin/stripspace.c` reads the configuration
//! for `--strip-comments`, at v2.30.0 and v2.56.0), so `;` set there strips `;` lines and keeps
//! a `#123` line, as the editor does. What `--strip-comments` leaves is exactly git's `strip`
//! cleanup: comment lines gone, trailing whitespace and leading and trailing blank lines removed,
//! runs of blank lines squeezed to one, a final newline — the same on git 2.30.9, 2.32.7 and
//! 2.56.0 (`tests/diff/commit.rs`).
//!
//! **Where it is not the editor.** The editor's cleanup is `commit.cleanup`'s, and this is
//! `strip`'s whatever that says, as C32 settles it: under `whitespace` or `verbatim` the editor
//! keeps comment lines this drops; under `scissors` it keeps a `#` line above the scissors line
//! that is not git's (a `#123` in the merge's own message), which this drops; and under
//! `core.commentChar=auto` (deprecated in git 2.56) git's commit picks another comment character
//! when a line of the message starts with `#`, keeping `# Conflicts:`, where `stripspace` reads
//! `auto` as `#` and drops it. Each is pinned in `tests/diff/commit.rs`.
//!
//! **What it runs and writes.** Nothing: `stripspace` reads its stdin and the configuration,
//! takes no lock, reads no index or object, starts no filter, textconv, external diff or hook
//! (`the_stripspace_read_writes_nothing_and_runs_nothing`). As a read it runs with
//! `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1` and carries no askpass token. git 2.56 warns
//! on stderr that `core.commentChar=auto` is deprecated; the answer is stdout's, and the warning
//! is not read.
//!
//! `"stripspace"` is built here and nowhere else in `reads/`, once, with `--strip-comments` its
//! one option (`the_porcelain_reads_are_the_named_queries`).

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// The verb and its one option.
const ARGUMENTS: [&str; 2] = ["stripspace", "--strip-comments"];

/// What a cleaned message may be: cleaning never lengthens one, and git's own commit
/// reads a message of any size, so this bounds only a runaway answer.
const CEILING: usize = 64 * 1024 * 1024;

/// `message` as `git stripspace --strip-comments` leaves it in `repo`, git reading the comment
/// character itself (module docs). Cancelled through `cancel` as any read is.
pub(crate) fn stripspace(
    git: &GitBinary,
    repo: &Repository,
    message: &[u8],
    cancel: &impl Cancel,
) -> Result<Vec<u8>, Error> {
    let output = git
        .read_invocation()
        .in_repository(repo)
        .args(ARGUMENTS)
        .input(message.to_vec())
        .start()?
        .collect(cancel, CEILING, |_| {})?;
    Ok(output.stdout().to_vec())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;
    use crate::CancelSignal;
    use crate::ops::Askpass;

    fn program() -> PathBuf {
        GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
            .unwrap()
            .path()
            .to_owned()
    }

    fn git(directory: &Path, args: &[&str]) {
        let status = Command::new(program())
            .current_dir(directory)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("HOME", directory)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    /// A repository at a fresh path under the temporary directory.
    fn repository(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("cairn-stripspace-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "."]);
        root
    }

    fn cleaned(root: &Path, message: &str) -> String {
        let gitbin =
            GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None)).unwrap();
        let repo = Repository::discover(root).unwrap();
        String::from_utf8(
            stripspace(&gitbin, &repo, message.as_bytes(), &CancelSignal::new()).unwrap(),
        )
        .unwrap()
    }

    /// Caught by: another verb, or an option that is not the strip of comments.
    #[test]
    fn the_read_is_stripspace_of_comments() {
        assert_eq!(ARGUMENTS, ["stripspace", "--strip-comments"]);
    }

    /// git reads the comment character in the repository it runs in: `#` lines go by default,
    /// `;` lines once `core.commentChar` is `;` — a `#123` line then kept — and blank lines are
    /// squeezed as git's `strip` squeezes them. Caught by: the read run outside the repository
    /// (git would strip `#` whatever the setting), or Cairn's own idea of a comment.
    #[test]
    fn the_comment_character_is_the_repositorys() {
        let root = repository("comment-char");
        let message = "Merge x\n\n\n#123 fixes it\n;45 also  \n\n# Conflicts:\n#\ta\n";
        assert_eq!(cleaned(&root, message), "Merge x\n\n;45 also\n");
        git(&root, &["config", "core.commentChar", ";"]);
        assert_eq!(
            cleaned(&root, message),
            "Merge x\n\n#123 fixes it\n\n# Conflicts:\n#\ta\n"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Every file under `directory`, with its bytes, for a before-and-after.
    fn snapshot(directory: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut found = Vec::new();
        let mut pending = vec![directory.to_owned()];
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

    /// C32: `stripspace` runs no program — not a filter, a textconv, an external diff, a hook or
    /// an fsmonitor, each configured to leave a marker — and writes nothing under the git
    /// directory. Caught by: a verb that reads the index or runs what the configuration names.
    #[test]
    fn the_stripspace_read_writes_nothing_and_runs_nothing() {
        let root = repository("runs");
        let markers = root.with_extension("markers");
        let _ = std::fs::remove_dir_all(&markers);
        std::fs::create_dir_all(&markers).unwrap();
        let leaves =
            |name: &str, then: &str| format!("touch '{}'; {then}", markers.join(name).display());
        for (key, value) in [
            ("diff.marked.textconv", leaves("textconv", "cat")),
            ("diff.external", leaves("external", "true")),
            ("filter.marked.smudge", leaves("smudge", "cat")),
            ("filter.marked.clean", leaves("clean", "cat")),
            ("core.fsmonitor", leaves("fsmonitor", "true")),
        ] {
            git(&root, &["config", key, &value]);
        }
        std::fs::write(root.join(".gitattributes"), "* diff=marked filter=marked\n").unwrap();
        for hook in ["pre-commit", "commit-msg", "post-commit"] {
            let path = root.join(".git/hooks").join(hook);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("#!/bin/sh\n{}\n", leaves(hook, "true"))).unwrap();
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let before = snapshot(&root.join(".git"));
        assert_eq!(cleaned(&root, "subject\n\n# a comment\n"), "subject\n");
        assert_eq!(snapshot(&root.join(".git")), before, "the read wrote");
        assert_eq!(
            std::fs::read_dir(&markers).unwrap().count(),
            0,
            "the read ran a program"
        );
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&markers);
    }
}
