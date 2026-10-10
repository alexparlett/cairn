//! `git config`, in query form: what a git that is about to run will read from the
//! repository's configuration, answered by that git — for a fetch of one remote, and for a
//! commit and an amend.
//!
//! **Why git answers.** A check made before a git runs has to decide on what that git will
//! read, and no second reader is that: gix 0.87 evaluates a linked worktree's `includeIf
//! "gitdir:..."` against the common directory where git evaluates it against the worktree's own
//! git directory (`.git/worktrees/<id>`), reads the system file from `/etc/gitconfig` where git
//! reads its own `$(sysconfdir)`, and decides trust by an owner rule of its own. So git answers
//! — the same binary, environment and `--git-dir`/`--work-tree` as the operation
//! (`in_repository`), a moment before it.
//!
//! - **A fetch** (`crate::ops::refspec_policy`): the refspec check refuses a fetch whose remote
//!   is a mirror, whose refspecs write local branches, or whose pruning would delete local tags
//!   ([`fetch_settings`]).
//! - **A commit and an amend** (staging-and-commit R6.11, `review-code-engine.md` M4):
//!   `i18n.commitEncoding`, which a commit is refused for when it names anything but UTF-8, and
//!   `core.logAllRefUpdates`, which decides whether git logs an amend's move, so whether the
//!   replaced commit can be recovered ([`commit_settings`]).
//!
//! This is the second porcelain verb a read runs, accepted by the user on 2026-10-04 beside
//! `git diff --no-index`, and widened to the commit's settings on 2026-10-10
//! (`crate::reads`, "What a read may run").
//!
//! Every invocation is `git config --includes --null` and then one named query of one key:
//! `--type=bool --get <key>` for a boolean, which prints the LAST value as git parses it — git
//! reads these keys last-one-wins through `git_config_bool`, at v2.30.0 and v2.56.0; `--get
//! <key>`, the last value as written, for a key git does not read as a boolean (a name, or
//! `core.logAllRefUpdates`, whose `always` no boolean parse takes); or `--get-all <key>` for
//! `remote.<name>.fetch`, every value in order, as `remote.c` appends each. Never a setter: no
//! `--add`, `--unset`, `--replace-all`, `--edit` or `--rename-section`, and no `set`/`unset`
//! subcommand (`the_porcelain_reads_are_the_named_queries`). `--type`, `--get`, `--get-all`,
//! `--null` and `--includes` are all in git 2.18 and later (`builtin/config.c` at v2.30.0), and
//! v2.56.0 still takes this form without a word on stderr (reproduced with 2.30.9, 2.32.7 and
//! 2.56.0). A key always starts `remote.`, `fetch.`, `core.` or `i18n.`, so it is never read as
//! an option.
//!
//! What the exit says: 0 with an answer; 1 with none — the key is unset, or not a key at all (a
//! remote name with a newline, which no configuration can hold either); anything else, a value
//! git will not parse as a boolean or a configuration file it cannot read (128), is
//! [`Error::GitFailed`], and the caller refuses on it, as git would then die on the same value.
//! It writes nothing: `git config` in query form takes no lock and reads no index
//! (`the_refspec_checks_reads_write_nothing`, in `crates/cairn-git/tests/fetch.rs`, holds the
//! git directory byte-identical), and it runs no program. As a read it runs with
//! `GIT_OPTIONAL_LOCKS=0` and `GIT_NO_LAZY_FETCH=1` and carries no askpass token, neither of
//! which `git config` has a use for.

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
/// A key git reads as text: its last value, as written.
const LAST_VALUE: [&str; 1] = ["--get"];

/// `core.logAllRefUpdates` as git reads it (`git_default_core_config` in git's `config.c`, at
/// v2.30.0 and v2.56.0): `always` in any case, else a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogRefUpdates {
    /// False: git creates no reflog, but appends to one that exists.
    None,
    /// True: git logs `HEAD`, branches, remote-tracking refs and notes.
    Normal,
    /// `always`: git logs every ref it updates.
    Always,
}

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

/// `i18n.commitEncoding` as written (staging-and-commit R6.1, R6.11): what git names a commit's
/// encoding, refused before a commit runs unless it names UTF-8; `None` when it is unset. A key
/// with no value reads as empty, which no encoding is called — git itself dies on it.
pub(crate) fn commit_encoding(
    git: &GitBinary,
    repo: &Repository,
    cancel: &impl Cancel,
) -> Result<Option<Vec<u8>>, Error> {
    text(git, repo, "i18n.commitEncoding", cancel)
}

/// `core.logAllRefUpdates` as git reads it (R6.4, R6.11): `always`, in any case, or else the
/// value as a boolean; `None` when it is unset, where git logs unless the repository is bare.
///
/// git reads each value in turn, taking `always` before its boolean parse, so the last value
/// decides. `--type=bool --get` cannot ask that: it parses every value of the key, and fails on
/// an `always` in the user's global configuration beneath the `true` `git init` writes in the
/// repository's own. So the last value is asked as written, and parsed here by git's own
/// rules (`crate::diff::git_config::parse_bool`, `git_config_bool`); only an empty one — the bare
/// key, which is true, or `= `, which is false, both printed as nothing by `--get` — is asked
/// of git's boolean parse, which tells them apart, and fails, as a residual, where an earlier
/// value is `always`. A value git refuses is [`Error::InvalidConfig`], as git dies on it.
pub(crate) fn log_all_ref_updates(
    git: &GitBinary,
    repo: &Repository,
    cancel: &impl Cancel,
) -> Result<Option<LogRefUpdates>, Error> {
    const KEY: &str = "core.logAllRefUpdates";
    let Some(value) = text(git, repo, KEY, cancel)? else {
        return Ok(None);
    };
    if value.eq_ignore_ascii_case(b"always") {
        return Ok(Some(LogRefUpdates::Always));
    }
    let logs = if value.is_empty() {
        boolean(git, repo, KEY, cancel)?
    } else {
        Some(
            crate::diff::git_config::parse_bool(Some(&value)).ok_or_else(|| {
                Error::InvalidConfig {
                    key: KEY.to_owned(),
                    value: String::from_utf8_lossy(&value).into_owned(),
                }
            })?,
        )
    };
    Ok(logs.map(|logs| {
        if logs {
            LogRefUpdates::Normal
        } else {
            LogRefUpdates::None
        }
    }))
}

/// The last value of `key` as written, or `None` when it is unset.
fn text(
    git: &GitBinary,
    repo: &Repository,
    key: &str,
    cancel: &impl Cancel,
) -> Result<Option<Vec<u8>>, Error> {
    let records = query(git, repo, &LAST_VALUE, key, cancel)?;
    match records {
        None => Ok(None),
        Some(mut records) if records.len() == 1 => Ok(records.pop()),
        Some(other) => Err(Error::UnexpectedGitOutput {
            arguments: described(&LAST_VALUE, key),
            record: format!("{} records, where a last value is one record", other.len()),
        }),
    }
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
    /// `--type=bool` (git's booleans read by Cairn), `--get` where every value counts, or
    /// `--type=bool` on a key whose `always` it refuses.
    #[test]
    fn the_queries_are_config_reads_in_query_form() {
        assert_eq!(QUERY, ["config", "--includes", "--null"]);
        assert_eq!(BOOLEAN, ["--type=bool", "--get"]);
        assert_eq!(EVERY_VALUE, ["--get-all"]);
        assert_eq!(LAST_VALUE, ["--get"]);
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

    /// The commit's two settings in `at`, each asked of git.
    fn commit_reads(fixture: &Fixture, at: &Path) -> (Option<Vec<u8>>, Option<LogRefUpdates>) {
        let repo = Repository::discover(at).unwrap_or_else(|e| panic!("{e}"));
        let git = fixture.binary();
        let cancel = CancelSignal::new();
        (
            commit_encoding(&git, &repo, &cancel).unwrap_or_else(|e| panic!("{e}")),
            log_all_ref_updates(&git, &repo, &cancel).unwrap_or_else(|e| panic!("{e}")),
        )
    }

    /// R6.11: `core.logAllRefUpdates` as git reads it — unset, true and false in git's
    /// spellings and as numbers, `always` in any case, the bare key (true) and `= ` (false), the
    /// last of several (an `always` beneath a `true`, as a global setting sits beneath the one
    /// `git init` writes) — and `i18n.commitEncoding` as written. Caught by: a boolean parse of
    /// every value (which an `always` fails), `always` read case-sensitively, the first value
    /// taken, or the bare key and an empty value read alike.
    #[test]
    fn the_commits_settings_are_read_as_git_reads_them() {
        let fixture = Fixture::new("commit");
        let main = fixture.main();
        // `git init` writes `logAllRefUpdates = true`; unset, it is git's default.
        fixture.git(&["config", "--unset", "core.logAllRefUpdates"]);
        assert_eq!(commit_reads(&fixture, &main), (None, None));
        for (written, read) in [
            ("true", LogRefUpdates::Normal),
            ("yes", LogRefUpdates::Normal),
            ("2k", LogRefUpdates::Normal),
            ("false", LogRefUpdates::None),
            ("off", LogRefUpdates::None),
            ("0", LogRefUpdates::None),
            ("always", LogRefUpdates::Always),
            ("ALWAYS", LogRefUpdates::Always),
        ] {
            fixture.git(&["config", "core.logAllRefUpdates", written]);
            assert_eq!(commit_reads(&fixture, &main).1, Some(read), "{written}");
        }
        let config = main.join(".git/config");
        let original = std::fs::read_to_string(&config).unwrap_or_else(|e| panic!("{e}"));
        let with = |tail: &str| {
            std::fs::write(&config, format!("{original}{tail}")).unwrap_or_else(|e| panic!("{e}"));
        };
        // `always` above, `true` last.
        with("[core]\n\tlogAllRefUpdates = true\n[i18n]\n\tcommitEncoding = latin1\n");
        assert_eq!(
            commit_reads(&fixture, &main),
            (Some(b"latin1".to_vec()), Some(LogRefUpdates::Normal)),
            "the last value decides"
        );
        fixture.git(&["config", "--unset-all", "core.logAllRefUpdates"]);
        let original = std::fs::read_to_string(&config).unwrap_or_else(|e| panic!("{e}"));
        let with = |tail: &str| {
            std::fs::write(&config, format!("{original}{tail}")).unwrap_or_else(|e| panic!("{e}"));
        };
        with("[core]\n\tlogAllRefUpdates\n");
        assert_eq!(
            commit_reads(&fixture, &main).1,
            Some(LogRefUpdates::Normal),
            "the bare key"
        );
        with("[core]\n\tlogAllRefUpdates =\n");
        assert_eq!(
            commit_reads(&fixture, &main).1,
            Some(LogRefUpdates::None),
            "empty"
        );
        with("[core]\n\tlogAllRefUpdates = never\n");
        let repo = Repository::discover(&main).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            matches!(
                log_all_ref_updates(&fixture.binary(), &repo, &CancelSignal::new()),
                Err(Error::InvalidConfig { .. })
            ),
            "a value git will not read was read"
        );
    }

    /// C32: set only through a linked worktree's `includeIf "gitdir:..."`, both settings are
    /// read in that worktree and not in the main one, as git reads them. Caught by: gix's
    /// reading of the include, which evaluates it against the common directory.
    #[test]
    fn a_linked_worktrees_conditional_include_sets_the_commits_settings() {
        let fixture = Fixture::new("commit-worktree");
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
        let include = fixture.root.join("commit.inc");
        std::fs::write(
            &include,
            "[core]\n\tlogAllRefUpdates = false\n[i18n]\n\tcommitEncoding = Shift_JIS\n",
        )
        .unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&[
            "config",
            &format!("includeIf.gitdir:{}.path", git_dir.display()),
            &include.display().to_string(),
        ]);
        assert_eq!(
            commit_reads(&fixture, &linked),
            (Some(b"Shift_JIS".to_vec()), Some(LogRefUpdates::None))
        );
        assert_eq!(
            commit_reads(&fixture, &fixture.main()),
            (None, Some(LogRefUpdates::Normal))
        );
    }
}
