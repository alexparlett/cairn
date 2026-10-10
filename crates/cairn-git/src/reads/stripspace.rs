//! `git stripspace`, the message on stdin: a merge's, a cherry-pick's or a revert's `MERGE_MSG`
//! cleaned as git's own editor session would clean it before the commit box shows it
//! (`docs/prd/staging-and-commit.md` R6.10, C2, C32 — the fourth porcelain read, accepted by the
//! user on 2026-10-10).
//!
//! **Why git cleans it.** `MERGE_MSG` carries git's commentary for the editor — `# Conflicts:`
//! and the paths under it, and, under `commit.cleanup=scissors`, a scissors line and what follows
//! — each line starting with the comment character, `#` unless the user set another.
//! `git commit -F -` keeps comment lines under its default cleanup, so a box filled with
//! `MERGE_MSG` as git wrote it would commit them where the user's own `git commit` would not.
//!
//! **The editor's cleanup, as git applies it** ([`as_the_editor_leaves`]): `cleanup_message` in
//! git's `sequencer.c` and `get_cleanup_mode` in `builtin/commit.c` (at v2.30.0 and v2.56.0), by
//! `commit.cleanup` (`crate::reads::commit_cleanup`; `git help commit`, `--cleanup`):
//!
//! - **unset, `default` or `strip`** — the edited message's `strip`: comment lines removed with
//!   surplus whitespace, `git stripspace --strip-comments`, git reading the comment character in
//!   the repository it runs in;
//! - **`whitespace`** — surplus whitespace removed, comment lines kept: `git stripspace`;
//! - **`verbatim`** — the message as it is;
//! - **`scissors`** — everything from git's scissors line on cut (`wt_status_locate_end` in
//!   `wt-status.c`: the line `<comment> ------------------------ >8 ------------------------`,
//!   at the start or after a newline), then `whitespace`'s cleaning. The comment string the line
//!   starts with is git's answer, `git stripspace --comment-lines` of one line
//!   ([`comment_string`]), so `core.commentChar` — and from git 2.45 `core.commentString`, its
//!   alias, whichever came last — is read as git reads it.
//!
//! Under **`core.commentChar=auto`** (`crate::reads::comment_char_is_auto`), git's commit picks a
//! comment character that starts no line of the message (`adjust_comment_line_char`) before the
//! editor runs, so no line of `MERGE_MSG` is a comment and no scissors line in it is found: every
//! mode but `verbatim` leaves `whitespace`'s cleaning. The template git adds around the message
//! in an editor session — the help lines and the status, which under `whitespace` and `verbatim`
//! the person removes by hand (`git help config`, `commit.cleanup`) — is not the message's, and
//! the commit box shows none. Each setting is checked against `git commit` with an editor on git
//! 2.30.9, 2.32.7 and the host's (`tests/diff/commit.rs`).
//!
//! **What it runs and writes.** Nothing: `stripspace` reads its stdin and the configuration,
//! takes no lock, reads no index or object, starts no filter, textconv, external diff or hook
//! (`the_stripspace_read_writes_nothing_and_runs_nothing`). As a read it runs with
//! `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1` and carries no askpass token. git 2.56 warns
//! on stderr that `core.commentChar=auto` is deprecated; the answer is stdout's, and the warning
//! is not read.
//!
//! `"stripspace"` is built here and nowhere else in `reads/`, once, its options `--strip-comments`
//! and `--comment-lines` and nothing else (`the_porcelain_reads_are_the_named_queries`).

use crate::ops::GitBinary;
use crate::reads::CommitCleanup;
use crate::{Cancel, Error, Repository};

/// The verb.
const VERB: &str = "stripspace";
/// Comment lines removed with the whitespace.
const STRIP_COMMENTS: &str = "--strip-comments";
/// Every line commented: how the comment string is asked of git.
const COMMENT_LINES: &str = "--comment-lines";

/// What a cleaned message may be: cleaning never lengthens one, and git's own commit
/// reads a message of any size, so this bounds only a runaway answer.
const CEILING: usize = 64 * 1024 * 1024;

/// What git's scissors line holds after its comment string (`cut_line` in git's `wt-status.c`).
const CUT_LINE: &[u8] = b" ------------------------ >8 ------------------------\n";

/// How `git stripspace` is asked to clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cleaning {
    /// No option: surplus whitespace.
    Whitespace,
    /// `--strip-comments`.
    Comments,
    /// `--comment-lines`.
    CommentLines,
}

/// `message` as git's editor session would leave it in `repo` before it is committed (module
/// docs). Cancelled through `cancel` as any read is.
pub(crate) fn as_the_editor_leaves(
    git: &GitBinary,
    repo: &Repository,
    message: &[u8],
    cancel: &impl Cancel,
) -> Result<Vec<u8>, Error> {
    let cleanup = crate::reads::commit_cleanup(git, repo, cancel)?;
    if cleanup == CommitCleanup::Verbatim {
        return Ok(message.to_vec());
    }
    let auto = crate::reads::comment_char_is_auto(git, repo, cancel)?;
    match cleanup {
        CommitCleanup::Default | CommitCleanup::Strip if !auto => {
            stripspace(git, repo, message, Cleaning::Comments, cancel)
        }
        CommitCleanup::Scissors if !auto => {
            let comment = comment_string(git, repo, cancel)?;
            let kept = &message[..scissors_end(message, &comment)];
            stripspace(git, repo, kept, Cleaning::Whitespace, cancel)
        }
        CommitCleanup::Default
        | CommitCleanup::Strip
        | CommitCleanup::Scissors
        | CommitCleanup::Whitespace
        | CommitCleanup::Verbatim => stripspace(git, repo, message, Cleaning::Whitespace, cancel),
    }
}

/// Where git's editor cuts `message` at its scissors line (`wt_status_locate_end`): at the
/// line's start, or the whole message where there is none.
fn scissors_end(message: &[u8], comment: &[u8]) -> usize {
    let mut pattern = b"\n".to_vec();
    pattern.extend_from_slice(comment);
    pattern.extend_from_slice(CUT_LINE);
    if message.starts_with(&pattern[1..]) {
        return 0;
    }
    message
        .windows(pattern.len())
        .position(|window| window == pattern.as_slice())
        .map_or(message.len(), |at| at + 1)
}

/// The comment string git reads in `repo` — `#`, `core.commentChar`, or from git 2.45
/// `core.commentString` — as `git stripspace --comment-lines` writes it before a line.
fn comment_string(
    git: &GitBinary,
    repo: &Repository,
    cancel: &impl Cancel,
) -> Result<Vec<u8>, Error> {
    let commented = stripspace(git, repo, b"x\n", Cleaning::CommentLines, cancel)?;
    match commented.strip_suffix(b" x\n") {
        Some(comment) if !comment.is_empty() => Ok(comment.to_vec()),
        Some(_) | None => Err(Error::UnexpectedGitOutput {
            arguments: format!("{VERB} {COMMENT_LINES}"),
            record: String::from_utf8_lossy(&commented).into_owned(),
        }),
    }
}

/// `message` as `git stripspace` cleans it in `repo`, `how` it is asked. Cancelled through
/// `cancel` as any read is.
fn stripspace(
    git: &GitBinary,
    repo: &Repository,
    message: &[u8],
    how: Cleaning,
    cancel: &impl Cancel,
) -> Result<Vec<u8>, Error> {
    let option = match how {
        Cleaning::Whitespace => None,
        Cleaning::Comments => Some(STRIP_COMMENTS),
        Cleaning::CommentLines => Some(COMMENT_LINES),
    };
    let output = git
        .read_invocation()
        .in_repository(repo)
        .arg(VERB)
        .args(option)
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
            stripspace(
                &gitbin,
                &repo,
                message.as_bytes(),
                Cleaning::Comments,
                &CancelSignal::new(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn comment_of(root: &Path) -> String {
        let gitbin =
            GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None)).unwrap();
        let repo = Repository::discover(root).unwrap();
        String::from_utf8(comment_string(&gitbin, &repo, &CancelSignal::new()).unwrap()).unwrap()
    }

    /// Caught by: another verb, or an option that is not the strip of comments or the comment
    /// of every line.
    #[test]
    fn the_read_is_stripspace_and_its_two_options() {
        assert_eq!(
            (VERB, STRIP_COMMENTS, COMMENT_LINES),
            ("stripspace", "--strip-comments", "--comment-lines")
        );
    }

    /// The comment string is git's answer: `#` by default, `core.commentChar` set, and — on git
    /// 2.45 and later, which read it as an alias — `core.commentString`, the later of the two.
    /// Caught by: Cairn's own reading of the setting, or the alias ignored where git reads it.
    #[test]
    fn the_comment_string_is_gits() {
        let root = repository("comment-string");
        assert_eq!(comment_of(&root), "#");
        git(&root, &["config", "core.commentChar", ";"]);
        assert_eq!(comment_of(&root), ";");
        let gitbin =
            GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None)).unwrap();
        let aliased = crate::ops::GitVersion {
            major: 2,
            minor: 45,
            patch: 0,
        };
        git(&root, &["config", "core.commentString", "//"]);
        let expected = if gitbin.version() >= aliased {
            "//"
        } else {
            ";"
        };
        assert_eq!(comment_of(&root), expected);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// git's `wt_status_locate_end`: the scissors line found at the start or after a newline,
    /// with the comment string asked, and nowhere else. Caught by: a cut at a line that only
    /// looks like one, or under another comment string.
    #[test]
    fn the_scissors_line_is_found_as_git_finds_it() {
        let line = "# ------------------------ >8 ------------------------\n";
        let message = format!("Merge x\n\n{line}# Conflicts:\n");
        assert_eq!(scissors_end(message.as_bytes(), b"#"), "Merge x\n\n".len());
        assert_eq!(scissors_end(line.as_bytes(), b"#"), 0);
        assert_eq!(scissors_end(message.as_bytes(), b";"), message.len());
        let indented = format!("Merge x\n {line}");
        assert_eq!(scissors_end(indented.as_bytes(), b"#"), indented.len());
        let unended = "Merge x\n# ------------------------ >8 ------------------------";
        assert_eq!(scissors_end(unended.as_bytes(), b"#"), unended.len());
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
        assert_eq!(comment_of(&root), "#");
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
