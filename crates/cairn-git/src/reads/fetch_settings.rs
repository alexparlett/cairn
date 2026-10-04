//! `git config`, in query form: what a fetch of one remote will read from the
//! repository's configuration, answered by the git that will fetch.
//!
//! The fetch's refspec check (`crate::ops::refspec_policy`) refuses a fetch whose
//! remote is a mirror, whose refspecs write local branches, or whose pruning would
//! delete local tags. What it decides on has to be what the fetch's own git will
//! read, and no second reader is that: gix 0.87 evaluates a linked worktree's
//! `includeIf "gitdir:..."` against the common directory where git evaluates it
//! against the worktree's own git directory (`.git/worktrees/<id>`), reads the
//! system file from `/etc/gitconfig` where git reads its own `$(sysconfdir)`, and
//! decides trust by an owner rule of its own. So git answers — the same binary,
//! environment and `--git-dir`/`--work-tree` as the fetch (`in_repository`), a
//! moment before it — and parses its own booleans (`--type=bool`). This is the
//! second porcelain verb a read runs, accepted by the user on 2026-10-04 beside
//! `git diff --no-index` (`crate::reads`, "What a read may run").
//!
//! Every invocation is `git config --includes --null` and then a query: `--type=bool
//! --get <key>` for a boolean, which prints the LAST value as git parses it — git's
//! `remote.c` and `builtin/fetch.c` read these keys last-one-wins through
//! `git_config_bool` too, at v2.30.0 and v2.56.0 — or `--get-all <key>` for
//! `remote.<name>.fetch`, every value in order, as `remote.c` appends each. Never a
//! setter: no `--add`, `--unset`, `--replace-all`, `--edit` or `--rename-section`,
//! and no `set`/`unset` subcommand (`the_porcelain_reads_are_the_two_named_queries`).
//! `--type`, `--get`, `--get-all`, `--null` and `--includes` are all in git 2.18 and
//! later (`builtin/config.c` at v2.30.0), and v2.56.0 still takes this form without a
//! word on stderr (reproduced with 2.30.9, 2.32.7 and 2.56.0). The key always starts
//! `remote.` or `fetch.`, so it is never read as an option.
//!
//! What the exit says: 0 with an answer; 1 with none — the key is unset, or not a
//! key at all (a remote name with a newline, which no configuration can hold either);
//! anything else, a value git will not parse as a boolean or a configuration file it
//! cannot read (128), is [`Error::GitFailed`], and the check refuses the fetch on it,
//! as git would then die on the same value. It writes nothing: `git config` in query
//! form takes no lock and reads no index (`the_refspec_checks_reads_write_nothing`,
//! in `crates/cairn-git/tests/fetch.rs`, holds the git directory byte-identical), and
//! it runs no program. As a read it runs with `GIT_OPTIONAL_LOCKS=0` and
//! `GIT_NO_LAZY_FETCH=1` and carries no askpass token, neither of which `git config`
//! has a use for.

use std::ffi::OsString;

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// What a fetch of one remote reads from the configuration, as git reads it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct FetchSettings {
    /// `remote.<name>.mirror`; `None` when unset.
    pub(crate) mirror: Option<bool>,
    /// `remote.<name>.fetch`, every value in the order git appends them.
    pub(crate) refspecs: Vec<Vec<u8>>,
    /// `remote.<name>.prune`, which overrides `fetch.prune` when set.
    pub(crate) remote_prune: Option<bool>,
    /// `fetch.prune`.
    pub(crate) fetch_prune: Option<bool>,
}

/// The verb and what every query here passes: include directives followed, as git's
/// own reading of the configuration follows them, and NUL-terminated answers.
const QUERY: [&str; 3] = ["config", "--includes", "--null"];
/// A boolean key: its last value, parsed by git.
const BOOLEAN: [&str; 2] = ["--type=bool", "--get"];
/// A multi-valued key: every value, as written.
const EVERY_VALUE: [&str; 1] = ["--get-all"];

/// The settings a fetch of `remote` will read in `repo`, each asked of `git`.
/// Cancelled through `cancel` as any read is, answering [`Error::GitReadCancelled`];
/// an answer that is not what the query prints is [`Error::UnexpectedGitOutput`].
pub(crate) fn fetch_settings(
    git: &GitBinary,
    repo: &Repository,
    remote: &str,
    cancel: &impl Cancel,
) -> Result<FetchSettings, Error> {
    let key = |name: &str| format!("remote.{remote}.{name}");
    Ok(FetchSettings {
        mirror: boolean(git, repo, &key("mirror"), cancel)?,
        refspecs: values(git, repo, &key("fetch"), cancel)?,
        remote_prune: boolean(git, repo, &key("prune"), cancel)?,
        fetch_prune: boolean(git, repo, "fetch.prune", cancel)?,
    })
}

/// The last value of `key` as git parses a boolean, or `None` when it is unset.
fn boolean(
    git: &GitBinary,
    repo: &Repository,
    key: &str,
    cancel: &impl Cancel,
) -> Result<Option<bool>, Error> {
    let records = query(git, repo, &BOOLEAN, key, cancel)?;
    match records.as_deref() {
        None => Ok(None),
        Some([value]) if value.as_slice() == b"true" => Ok(Some(true)),
        Some([value]) if value.as_slice() == b"false" => Ok(Some(false)),
        Some(other) => Err(Error::UnexpectedGitOutput {
            arguments: described(&BOOLEAN, key),
            record: format!(
                "{} records, where a boolean is one record, true or false",
                other.len()
            ),
        }),
    }
}

/// Every value of `key`, in order; empty when it is unset.
fn values(
    git: &GitBinary,
    repo: &Repository,
    key: &str,
    cancel: &impl Cancel,
) -> Result<Vec<Vec<u8>>, Error> {
    Ok(query(git, repo, &EVERY_VALUE, key, cancel)?.unwrap_or_default())
}

/// Runs `git config` with `action` over `key`: the records it printed, or `None` when
/// it exited 1, which is git's "no such key".
fn query(
    git: &GitBinary,
    repo: &Repository,
    action: &[&str],
    key: &str,
    cancel: &impl Cancel,
) -> Result<Option<Vec<Vec<u8>>>, Error> {
    let mut records = Vec::new();
    let outcome = git
        .read_invocation()
        .in_repository(repo)
        .args(QUERY.iter().chain(action).map(OsString::from))
        .arg(key)
        .start()?
        .records(cancel, |record| records.push(record.to_vec()), |_| {});
    match outcome {
        Ok(_) => Ok(Some(records)),
        Err(Error::GitFailed { status, .. }) if status.code() == Some(1) => Ok(None),
        Err(other) => Err(other),
    }
}

fn described(action: &[&str], key: &str) -> String {
    let mut words: Vec<&str> = QUERY.iter().chain(action).copied().collect();
    words.push(key);
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::CancelSignal;
    use crate::ops::{Askpass, GitEnvironment};

    /// Each query is `config --includes --null` and a query action, never a setter.
    /// Caught by: another verb, a dropped `--null` (answers split on newlines), a dropped
    /// `--type=bool` (git's booleans read by Cairn), or `--get` where every value counts.
    #[test]
    fn the_queries_are_config_reads_in_query_form() {
        assert_eq!(QUERY, ["config", "--includes", "--null"]);
        assert_eq!(BOOLEAN, ["--type=bool", "--get"]);
        assert_eq!(EVERY_VALUE, ["--get-all"]);
        assert_eq!(
            described(&BOOLEAN, "fetch.prune"),
            "config --includes --null --type=bool --get fetch.prune"
        );
    }

    struct Fixture {
        root: PathBuf,
        program: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "cairn-fetch-settings-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("home")).unwrap_or_else(|e| panic!("{e}"));
            std::fs::create_dir_all(root.join("main")).unwrap_or_else(|e| panic!("{e}"));
            let program = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
                .unwrap_or_else(|e| panic!("{e}"))
                .path()
                .to_owned();
            let fixture = Self { root, program };
            fixture.git(&["init", "-q", "."]);
            fixture
        }

        fn main(&self) -> PathBuf {
            self.root.join("main")
        }

        fn git(&self, args: &[&str]) -> String {
            let output = std::process::Command::new(&self.program)
                .current_dir(self.main())
                .args(args)
                .env("HOME", self.root.join("home"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "a")
                .env("GIT_AUTHOR_EMAIL", "a@example.com")
                .env("GIT_COMMITTER_NAME", "a")
                .env("GIT_COMMITTER_EMAIL", "a@example.com")
                .output()
                .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }

        fn binary(&self) -> GitBinary {
            let home = self.root.join("home").into_os_string();
            GitBinary::discover_with(GitEnvironment::new(
                |name| match name {
                    "PATH" => std::env::var_os("PATH"),
                    "HOME" => Some(home.clone()),
                    _ => None,
                },
                &Askpass::new("/nonexistent/cairn-askpass", None),
            ))
            .unwrap_or_else(|e| panic!("{e}"))
        }

        fn settings(&self, at: &Path, remote: &str) -> Result<FetchSettings, Error> {
            let repo = Repository::discover(at).unwrap_or_else(|e| panic!("{e}"));
            fetch_settings(&self.binary(), &repo, remote, &CancelSignal::new())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// Booleans as git parses them — any spelling, a key with no `=`, the last of several
    /// values — and every refspec in order, for the remote named and no other. Caught by:
    /// Cairn parsing a boolean, the first value taken, or one refspec lost.
    #[test]
    fn each_setting_is_read_as_git_reads_it() {
        let fixture = Fixture::new("read");
        let config = fixture.main().join(".git/config");
        let mut text = std::fs::read_to_string(&config).unwrap_or_else(|e| panic!("{e}"));
        text.push_str(
            "[remote \"origin\"]\n\tmirror\n\tprune = yes\n\tprune = off\n\
             \tfetch = +refs/heads/*:refs/remotes/origin/*\n\tfetch = +refs/tags/*:refs/tags/*\n\
             [fetch]\n\tprune = 1\n[remote \"Origin\"]\n\tmirror = false\n",
        );
        std::fs::write(&config, text).unwrap_or_else(|e| panic!("{e}"));
        let read = fixture
            .settings(&fixture.main(), "origin")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            read,
            FetchSettings {
                mirror: Some(true),
                refspecs: vec![
                    b"+refs/heads/*:refs/remotes/origin/*".to_vec(),
                    b"+refs/tags/*:refs/tags/*".to_vec(),
                ],
                remote_prune: Some(false),
                fetch_prune: Some(true),
            }
        );
        let other = fixture
            .settings(&fixture.main(), "Origin")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            other.mirror,
            Some(false),
            "a remote's name is case-sensitive"
        );
        assert!(other.refspecs.is_empty());
        let none = fixture
            .settings(&fixture.main(), "no such remote")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((none.mirror, none.remote_prune), (None, None));
        assert!(none.refspecs.is_empty());
        let unkeyable = fixture
            .settings(&fixture.main(), "a\nb")
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(unkeyable.mirror, None, "a name no key can hold is unset");
    }

    /// In a linked worktree git evaluates `includeIf "gitdir:..."` against the worktree's
    /// own git directory, and this read asks git, at the floor as on the newest git.
    /// Caught by: the read made anywhere but in the repository the fetch runs in.
    #[test]
    fn a_linked_worktrees_conditional_include_is_read() {
        let fixture = Fixture::new("worktree");
        fixture.git(&["commit", "-q", "--allow-empty", "-m", "first"]);
        fixture.git(&["worktree", "add", "-q", "-b", "linked", "../linked"]);
        let linked = fixture.root.join("linked");
        let git_dir = std::fs::canonicalize(fixture.git(&[
            "-C",
            "../linked",
            "rev-parse",
            "--absolute-git-dir",
        ]))
        .unwrap_or_else(|e| panic!("{e}"));
        let include = fixture.root.join("mirror.inc");
        std::fs::write(&include, "[remote \"origin\"]\n\tmirror = true\n")
            .unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&[
            "config",
            &format!("includeIf.gitdir:{}.path", git_dir.display()),
            &include.display().to_string(),
        ]);
        let read = |at: &Path| {
            fixture
                .settings(at, "origin")
                .unwrap_or_else(|e| panic!("{e}"))
                .mirror
        };
        assert_eq!(read(&linked), Some(true));
        assert_eq!(read(&fixture.main()), None);
    }

    /// A value git will not read as a boolean, or a configuration file it cannot parse, is
    /// an error and never an unset key; and so is a cancelled read. Caught by: any failure
    /// read as "unset", which would pass the check on a setting it never saw.
    #[test]
    fn a_failed_or_cancelled_read_is_an_error_never_an_unset_key() {
        let fixture = Fixture::new("failed");
        fixture.git(&["config", "fetch.prune", "sometimes"]);
        match fixture.settings(&fixture.main(), "origin") {
            Err(Error::GitFailed { status, .. }) => assert_ne!(status.code(), Some(1)),
            other => panic!("a bad boolean was read as {other:?}"),
        }
        fixture.git(&["config", "--unset", "fetch.prune"]);
        // Opened first: gix refuses to open a repository whose configuration it cannot
        // parse, and the file may break after Cairn opened it.
        let repo = Repository::discover(fixture.main()).unwrap_or_else(|e| panic!("{e}"));
        let config = fixture.main().join(".git/config");
        let mut text = std::fs::read_to_string(&config).unwrap_or_else(|e| panic!("{e}"));
        text.push_str("[remote \"origin\"\n");
        std::fs::write(&config, text).unwrap_or_else(|e| panic!("{e}"));
        match fetch_settings(&fixture.binary(), &repo, "origin", &CancelSignal::new()) {
            Err(Error::GitFailed { status, .. }) => assert_ne!(status.code(), Some(1)),
            other => panic!("an unparseable configuration was read as {other:?}"),
        }

        let fresh = Fixture::new("cancelled");
        let repo = Repository::discover(fresh.main()).unwrap_or_else(|e| panic!("{e}"));
        let cancel = CancelSignal::new();
        cancel.cancel();
        assert!(matches!(
            fetch_settings(&fresh.binary(), &repo, "origin", &cancel),
            Err(Error::GitReadCancelled { .. })
        ));
    }
}
