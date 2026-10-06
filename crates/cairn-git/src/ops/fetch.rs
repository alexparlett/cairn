//! Fetching from a remote: the first operation to run a `git` verb, and the
//! first that can ask the user for a credential.
//!
//! **Not destructive, and deliberately takes no [`cairn_model::Confirmed`].**
//! With the refspecs git gives a clone (`+refs/heads/*:refs/remotes/<remote>/*`)
//! a fetch adds objects and moves remote-tracking refs; it touches no local
//! branch, no index and no working tree, and every ref it moves is in the
//! reflog wherever the repository keeps one (a bare repository logs nothing
//! by default; the old tip's commits survive until `gc` either way). The one
//! deletion it performs is the user's own: `fetch.prune` (or
//! `remote.<name>.prune`, which overrides it) is honoured exactly as
//! `git fetch` honours it, so a remote-tracking ref the remote has already
//! deleted is removed here too — its commits survive until `gc`, and the
//! ref was never local work. Nothing is passed for prune, so git reads that
//! configuration itself, fresh, on every run (decided on issue #17).
//!
//! What is never done, whatever the configuration: pruning a local tag.
//! Tags have no reflog, so `--no-prune-tags` is always passed (in git since
//! 2.17, within [`super::GitVersion::MINIMUM`]) and `fetch.pruneTags` and
//! `remote.<name>.pruneTags` are ignored — a user who set them gets stale
//! tags left standing rather than removed without a word. That flag only
//! withholds the tag refspec git would add, so a remote whose OWN refspecs
//! write `refs/tags/` is refused while pruning is on, before any process
//! starts (`super::refspec_policy`). The same check refuses a remote whose
//! refspecs would write local branches — a mirror clone, `remote.<name>.mirror`,
//! `+refs/*:refs/*`, any `refs/heads/*` destination — with the setting quoted
//! in [`Error::FetchRefused`]; a fetch that overwrites local branches, with
//! no reflog in a bare repository, is a fetch from a terminal, not from a
//! button. Push, the next operation on this backend, is the one that needs
//! the token whatever the configuration (L8); pruning that says what will
//! go, as a confirmed operation, is issue #17's remainder.
//!
//! What it invalidates: `refs` (remote-tracking refs moved) and `objects`
//! (new objects arrived), declared on the [`Performed`] per the cache contract
//! in the [`super`] module docs, and honoured by the worker in `cairn-app` by
//! reopening its history walk from `HEAD`.
//!
//! Progress is git's own: `--progress` makes it write its meters to stderr
//! even with no terminal, and [`FetchInProgress::finish`] hands each redrawn
//! line to the caller as it arrives. It runs on the runner every invocation
//! runs on (`docs/systems/git-processes.md`): its own process group, a thread
//! per pipe. Cancelling ends that group ([`FetchCancel`]): `SIGTERM`, which
//! git cleans its lock files up on, then `SIGKILL` if it is still there after
//! the grace period. Whatever `*.lock` files are under the git directory
//! afterwards — a `SIGKILL` that landed mid ref-update, a lock from an
//! earlier crash, or one another git holds this instant, which a listing
//! cannot tell apart — travel on the [`Error::GitCancelled`] so the banner
//! can name them and say when acting on them is safe (`super::stranded_locks`);
//! the runner lists them after the reap, because a fetch is a write.

use cairn_model::AskpassToken;

use super::{GitBinary, Invalidated, Performed, WriteAuthority, refspec_policy};
use crate::process::{Invocation, KillHandle, Write};
use crate::{CancelSignal, Error, Repository};

/// Starts `git fetch --progress --no-prune-tags <remote>` in `repo`, with
/// `token` as the operation's askpass authorisation when there is a channel
/// to answer on; without one, a prompt fails closed. Returns as soon as the
/// process is running; [`FetchInProgress::finish`] waits for it. A remote
/// whose configuration would have the fetch write local branches, or delete
/// local tags under pruning, is [`Error::FetchRefused`] and no process
/// starts (module docs).
///
/// `remote` is a configured remote name or a URL, as `git fetch` takes it;
/// it follows `--end-of-options`, so a name beginning with `-` is a remote and
/// never an option.
pub fn fetch(
    git: &GitBinary,
    repo: &Repository,
    remote: &str,
    token: Option<&AskpassToken>,
) -> Result<FetchInProgress, Error> {
    refspec_policy::check(git, repo, remote)?;
    // A write: it moves remote-tracking refs and adds objects, and it is the
    // one invocation that may ask the user for a credential.
    let mut command = git
        .write_invocation(WriteAuthority::new())
        .in_repository(repo)
        .args(ARGUMENTS)
        .arg(remote);
    if let Some(token) = token {
        command = command.authorized_by(token);
    }
    Ok(FetchInProgress {
        invocation: command.start()?,
        remote: remote.to_owned(),
    })
}

/// Everything before the remote. Nothing for prune, so git applies the
/// user's `fetch.prune`; `--no-prune-tags` always, so no tag is ever pruned
/// (module docs).
const ARGUMENTS: [&str; 4] = ["fetch", "--progress", "--no-prune-tags", "--end-of-options"];

/// A fetch that has been started; see [`fetch`].
#[derive(Debug)]
pub struct FetchInProgress {
    /// Holds the repository's git and common directories, where a cancel or a
    /// failure looks for lock files (`in_repository`); paths rather than the
    /// repository, which belongs to the thread that opened it.
    invocation: Invocation<Write>,
    remote: String,
}

impl FetchInProgress {
    /// A handle that cancels this fetch from another thread.
    pub fn canceller(&self) -> FetchCancel {
        FetchCancel(self.invocation.kill_handle())
    }

    /// Streams git's progress to `progress`, one line per redraw, until the
    /// process exits. Success is a [`Performed`] declaring `refs` and
    /// `objects` invalid — the declaration is what a fetch CAN change; whether
    /// a ref actually moved is for the caller to see, which the worker does by
    /// comparing [`crate::Repository::ref_tips`] before and after, on every
    /// outcome, since a failed or killed fetch may have updated some refs
    /// before it stopped. That comparison sees only what the refs snapshot
    /// holds — the local branches, remote-tracking refs and tags, `HEAD`, each
    /// branch's upstream and the stash list — so a fetch that moves only a ref
    /// outside those namespaces (`refs/notes/`, `refs/pull/`, `refs/replace/`)
    /// is reported as having moved none. A cancelled fetch is [`Error::GitCancelled`],
    /// carrying every `*.lock` left under the git directory once git is gone;
    /// anything git refused is [`Error::GitFailed`] carrying its stderr —
    /// which is where "could not read Username ...: terminal prompts
    /// disabled" arrives when no credential could be had — and the lock files
    /// present. Dropped instead of finished, the fetch is ended and reaped on
    /// a thread of the runner's.
    ///
    /// The cancel is [`FetchCancel`]'s alone, so the signal the runner polls
    /// here is one nobody holds; and stdout, where `git fetch` writes nothing
    /// a person or Cairn reads, is drained and dropped.
    pub fn finish(self, progress: impl FnMut(&str)) -> Result<Performed, Error> {
        self.invocation
            .finish(&CancelSignal::new(), |_| {}, progress)
            .map(|_| {
                Performed::new(
                    format!("fetched {}", self.remote),
                    Invalidated::refs().and(Invalidated::objects()),
                )
            })
    }
}

/// Cancels a running fetch by ending its `git` and everything it started
/// (`SIGTERM` to the process group, then `SIGKILL` after the grace period).
/// Cloneable and `Send`, so the thread waiting in [`FetchInProgress::finish`]
/// need not be the one deciding to stop; the waiter still reaps the process,
/// and the cancel never blocks the caller.
#[derive(Debug, Clone)]
pub struct FetchCancel(KillHandle);

impl FetchCancel {
    pub fn cancel(&self) {
        self.0.kill();
    }
}

/// Against a stub `git`, since the runner is crate-private: what the process is
/// actually given. The behaviour the arguments buy is pinned end to end by the
/// pruning tests in `tests/fetch.rs`.
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::process::stub_git::{StubGit, discover_retrying, printed_environment};

    /// The stub's answer to the refspec check's `git config` reads: exit 1, git's "no
    /// such key", so the check passes and the fetch itself is what the stub runs.
    const UNSET_CONFIG: &str =
        "for argument in \"$@\"; do [ \"$argument\" = config ] && exit 1; done\n";

    /// The repository named ahead of the verb (`process/cli.rs`), so the fetch lands
    /// in the repository Cairn opened; then nothing for prune — git reads
    /// `fetch.prune` itself — and `--no-prune-tags` always, before
    /// `--end-of-options` and the remote.
    #[test]
    fn the_arguments_leave_prune_to_git_forbid_pruning_tags_and_end_the_options() {
        let stub = StubGit::with_git(&format!(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
             {UNSET_CONFIG}printf '%s\\n' \"$@\" >&2",
        ));
        let git = discover_retrying(stub.environment()).unwrap();
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let mut seen = Vec::new();
        fetch(&git, &repo, "-origin", None)
            .unwrap()
            .finish(|line| seen.push(line.to_owned()))
            .unwrap();
        let named = |option: &str, path: &std::path::Path| {
            format!("{option}{}", std::fs::canonicalize(path).unwrap().display())
        };
        let canonical = |line: &String| match line.split_once('=') {
            Some((option, path)) if option.starts_with("--") => {
                named(&format!("{option}="), std::path::Path::new(path))
            }
            _ => line.clone(),
        };
        assert_eq!(
            seen.iter().map(canonical).collect::<Vec<_>>(),
            [
                named("--git-dir=", repo.git_dir()),
                named("--work-tree=", repo.workdir().unwrap()),
                "fetch".to_owned(),
                "--progress".to_owned(),
                "--no-prune-tags".to_owned(),
                "--end-of-options".to_owned(),
                "-origin".to_owned(),
            ]
        );
    }

    /// Fetch runs with the write environment the tests in `ops/authority.rs`
    /// spell out — no `GIT_OPTIONAL_LOCKS`, the editor pinned — and with the
    /// token it was given, or none: the stub prints what it was given, on
    /// stderr, which is what a fetch hands on as progress.
    #[test]
    fn a_fetch_runs_with_the_write_environment_and_its_token() {
        let stub = StubGit::with_git(&format!(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
             {UNSET_CONFIG}/usr/bin/env >&2",
        ));
        let environment = stub.environment();
        let path = environment
            .get("PATH")
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap();
        let git = discover_retrying(environment).unwrap();
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let printed = |token: Option<&AskpassToken>| {
            let mut seen = String::new();
            fetch(&git, &repo, "origin", token)
                .unwrap()
                .finish(|line| {
                    seen.push_str(line);
                    seen.push('\n');
                })
                .unwrap();
            printed_environment(&seen)
        };
        let base = [
            ("GIT_ASKPASS", StubGit::HELPER),
            ("GIT_EDITOR", "false"),
            ("GIT_SEQUENCE_EDITOR", "false"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("PATH", path.as_str()),
            ("SSH_ASKPASS", StubGit::HELPER),
            ("SSH_ASKPASS_REQUIRE", "force"),
        ];
        let spelled = |extra: &[(&str, &str)]| {
            base.iter()
                .chain(extra)
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect::<std::collections::BTreeMap<_, _>>()
        };

        assert_eq!(printed(None), spelled(&[]));
        let token = AskpassToken::new("fetch-token");
        assert_eq!(
            printed(Some(&token)),
            spelled(&[("CAIRN_ASKPASS_TOKEN", "fetch-token")])
        );
    }

    /// G17, the engine half's last clause: a fetch run with an askpass token is
    /// logged once, as what it was given and what it said, and its record holds
    /// neither the token nor any value of the environment git ran with. Decisive
    /// because the stub writes the environment it was given to a file beside
    /// itself, which must show the token and every value looked for — so each
    /// was there to leak — while what it says on stderr, which the log keeps,
    /// is a line of progress.
    #[test]
    fn a_fetch_with_a_token_is_logged_once_without_the_token_or_the_environment() {
        let stub = StubGit::with_git_from(|directory| {
            format!(
                "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
                 {UNSET_CONFIG}/usr/bin/env > '{}'; echo 'Receiving objects: 100%, done.' >&2",
                directory.join("environment-seen").display()
            )
        });
        let home = format!("/nonexistent/home-g17-{}", std::process::id());
        let language = "xx_G17.UTF-8";
        let environment = stub.environment_with(|name| match name {
            "HOME" => Some(home.clone().into()),
            "LANG" => Some(language.into()),
            _ => None,
        });
        let path = environment
            .get("PATH")
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap();
        let git = discover_retrying(environment).unwrap();
        let repo = Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let token = AskpassToken::new(format!("g17-token-{}", std::process::id()));

        fetch(&git, &repo, "origin", Some(&token))
            .unwrap()
            .finish(|_| {})
            .unwrap();

        let seen = printed_environment(
            &std::fs::read_to_string(stub.directory().join("environment-seen")).unwrap(),
        );
        let values = [
            ("CAIRN_ASKPASS_TOKEN", token.as_str()),
            ("HOME", home.as_str()),
            ("LANG", language),
            ("PATH", path.as_str()),
            ("GIT_ASKPASS", StubGit::HELPER),
        ];
        for (name, value) in values {
            assert_eq!(
                seen.get(name).map(String::as_str),
                Some(value),
                "git was not given {name}, so its absence below decides nothing"
            );
        }
        let log = repo.processes().log();
        let (fetches, reads): (Vec<_>, Vec<_>) = log
            .iter()
            .partition(|record| record.arguments.first().map(String::as_str) == Some("fetch"));
        assert_eq!(
            fetches.len(),
            1,
            "one fetch, recorded {} times",
            fetches.len()
        );
        assert_eq!(
            reads.len(),
            4,
            "the refspec check's four reads, logged as any read is: {reads:?}"
        );
        assert!(
            reads
                .iter()
                .all(|record| record.arguments.first().map(String::as_str) == Some("config")),
            "something but the check's reads was logged beside the fetch: {reads:?}"
        );
        let record = fetches[0];
        assert_eq!(
            record.arguments,
            [
                "fetch",
                "--progress",
                "--no-prune-tags",
                "--end-of-options",
                "origin"
            ]
        );
        assert_eq!(record.stderr, "Receiving objects: 100%, done.");
        let rendered = format!("{log:?}");
        for (name, value) in values {
            assert!(
                !rendered.contains(value),
                "the log's record carries {name}'s value: {rendered}"
            );
        }
    }
}
