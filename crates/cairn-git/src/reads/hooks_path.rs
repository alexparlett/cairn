//! `git rev-parse --git-path hooks`: the directory git runs this repository's hooks from
//! (`docs/prd/staging-and-commit.md` R3.9, R6.6).
//!
//! Where hooks live is git's to say: `core.hooksPath` when it is set — read with git's own
//! precedence, includes and `~` expansion, a relative value meaning relative to where the
//! hook runs — and `<common git dir>/hooks` otherwise, a linked worktree included. `rev-parse
//! --git-path` answers exactly that (git 2.30.9 and 2.56.0 alike: an absolute path for the
//! default, the value for a relative `core.hooksPath`, relative to the directory git ran in),
//! so the commit box offers to skip hooks only where git would run one (phase 05). Query
//! plumbing: it reads configuration and writes nothing, takes no lock and runs nothing.
//!
//! A relative answer is relative to the directory the read ran in — the working tree's top,
//! or the git directory of a bare repository (`crate::process`) — and is joined to it here,
//! so the caller holds a path it can look in.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt as _;
use std::path::PathBuf;

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

const ARGUMENTS: [&str; 3] = ["rev-parse", "--git-path", "hooks"];

/// A path and its newline; a hooks path past this is not one git prints for a person.
const CEILING: usize = 64 * 1024;

/// The hooks directory git uses for `repo`, absolute. Cancelled through `cancel` as any read
/// is; an empty answer is [`Error::UnexpectedGitOutput`].
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the commit engine (staging-and-commit phase 05) offers to skip hooks only \
                  where this says one can run; the two reads of R3.9 land together"
    )
)]
pub(crate) fn hooks_path(
    git: &GitBinary,
    repo: &Repository,
    cancel: &impl Cancel,
) -> Result<PathBuf, Error> {
    let output = git
        .read_invocation()
        .in_repository(repo)
        .args(ARGUMENTS)
        .start()?
        .collect(cancel, CEILING, |_| {})?;
    let printed = output.stdout();
    let path = printed.strip_suffix(b"\n").unwrap_or(printed);
    if path.is_empty() {
        return Err(Error::UnexpectedGitOutput {
            arguments: ARGUMENTS.join(" "),
            record: String::new(),
        });
    }
    let ran_in = repo.workdir().unwrap_or(repo.git_dir());
    Ok(ran_in.join(PathBuf::from(OsString::from_vec(path.to_vec()))))
}

#[cfg(test)]
mod tests {
    use std::path::Path;
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

    /// Caught by: another verb, or asking for another path than the hooks directory.
    #[test]
    fn the_read_is_rev_parse_of_the_hooks_path() {
        assert_eq!(ARGUMENTS, ["rev-parse", "--git-path", "hooks"]);
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

    /// The default, a relative `core.hooksPath` and an absolute one, each answered as the
    /// directory git would run a hook from, made absolute against the working tree.
    #[test]
    fn the_hooks_directory_is_gits_own_answer_made_absolute() {
        let root = std::env::temp_dir().join(format!("cairn-hooks-path-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "."]);
        let gitbin =
            GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None)).unwrap();
        let ask = || {
            let repo = Repository::discover(&root).unwrap();
            hooks_path(&gitbin, &repo, &CancelSignal::new()).unwrap()
        };
        let canonical = |path: PathBuf| std::fs::canonicalize(path).unwrap();
        std::fs::create_dir_all(root.join(".git/hooks")).unwrap();
        assert_eq!(canonical(ask()), canonical(root.join(".git/hooks")));
        std::fs::create_dir_all(root.join("my hooks")).unwrap();
        git(&root, &["config", "core.hooksPath", "my hooks"]);
        assert_eq!(canonical(ask()), canonical(root.join("my hooks")));
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        git(
            &root,
            &["config", "core.hooksPath", elsewhere.to_str().unwrap()],
        );
        assert_eq!(canonical(ask()), canonical(elsewhere));
        let _ = std::fs::remove_dir_all(&root);
    }
}
