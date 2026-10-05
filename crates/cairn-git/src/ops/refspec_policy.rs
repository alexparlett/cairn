//! What a fetch may write locally, decided before `git` runs.
//!
//! `git fetch <remote>` writes wherever the remote's configured refspecs
//! point. For the clone git makes, that is `refs/remotes/<remote>/*`, and
//! every ref there is in the reflog and is nobody's local work. Two
//! configurations break that: a mirror (`git clone --mirror`, which sets
//! `remote.<name>.mirror` and `fetch = +refs/*:refs/*`) or any refspec whose
//! destination is under `refs/heads/`, which has a fetch overwrite local
//! branches — git refuses only the one that is checked out, and in a bare
//! repository nothing is checked out and nothing is reflogged by default.
//! Cairn's fetch never does that: a remote configured that way is refused
//! here, before a process exists, with the setting quoted so the user can see
//! what to change (issue #17). A fetch from a terminal is still theirs.
//!
//! The second refusal follows from pruning. `fetch.prune` is honoured — a
//! pruned remote-tracking ref is one the remote already deleted — but a local
//! tag is never pruned: tags have no reflog, so `--no-prune-tags` is always
//! passed, and that flag only withholds the tag refspec git would ADD. A
//! remote whose own refspecs already write `refs/tags/` would have `--prune`
//! delete local tags through them, so with pruning on such a remote is
//! refused too, and without pruning it fetches as before.
//!
//! The check reads the remote as the fetch's own git will, because git answers it
//! (`crate::reads::fetch_settings`, `git config` in query form — the second
//! porcelain read, accepted by the user on 2026-10-04): the same binary, the same
//! built environment, the same `--git-dir` and `--work-tree`, a moment before the
//! fetch, and afresh on every fetch, because `git` reads configuration on every run
//! and a refspec added in a terminal since Cairn started must be seen by the fetch
//! that would act on it. Nothing here reads configuration through gix: gix
//! evaluates a linked worktree's `includeIf "gitdir:..."` against the common
//! directory where git uses the worktree's own git directory, reads the system file
//! from its own path, and decides trust by an owner rule of its own, and each of
//! those once let a fetch through that git then made as a mirror or a pruner of
//! tags. The check FAILS CLOSED: a read that fails — no `git`, a value git will not
//! parse, an answer that is not one — refuses the fetch as [`Error::RemoteConfig`], and
//! so does a refspec that does not parse; a read ended because the repository is
//! closing is the fetch cancelled before it started, [`Error::GitCancelled`], as a
//! close of a running fetch reports it.
//!
//! One definition of a remote is not configuration: git reads `$GIT_DIR/remotes/<name>`
//! and `$GIT_DIR/branches/<name>` (in the common directory) for a remote no
//! configuration gives a URL, and a `branches/` file fetches into
//! `refs/heads/<name>` — a local branch (reproduced with git 2.30.9 and 2.56.0). No
//! query prints what git makes of those files, and which of them git consults
//! depends on its version (from 2.46 an empty `remote.<name>.url` clears the list,
//! and git built with its 3.0 changes reads neither), so a remote with such a file
//! is refused on sight, whatever the configuration also says, with the file quoted
//! ([`RefusedWrite::DefinedByFile`]). What a configured remote name git does not
//! know, or a URL, has no configured refspecs and passes; git then fetches into
//! `FETCH_HEAD` alone, or refuses the name itself.

use std::path::{Path, PathBuf};

use gix::bstr::{BStr, ByteSlice};

use crate::error::RefusedWrite;
use crate::ops::GitBinary;
use crate::reads::{FetchSettings, fetch_settings};
use crate::{CancelSignal, Error, Repository};

/// Refuses a fetch of `remote` in `repo` whose configuration would have it write
/// local branches or, with pruning on, delete local tags, or whose remote is
/// defined by a file git reads in place of configuration; see the module docs.
/// The configuration is read by `git`, the binary the fetch will run.
pub(crate) fn check(git: &GitBinary, repo: &Repository, remote: &str) -> Result<(), Error> {
    let refused = |setting: String, write: RefusedWrite| Error::FetchRefused {
        remote: remote.to_owned(),
        setting,
        write,
    };
    if let Some(file) = definition_file(repo.inner().common_dir(), remote) {
        return Err(refused(
            file.display().to_string(),
            RefusedWrite::DefinedByFile,
        ));
    }
    // Nobody holds this signal: the reads are a few short `git config` runs ahead of a
    // fetch that has no process yet for its own cancel to end.
    let settings = fetch_settings(git, repo, remote, &CancelSignal::new())
        .map_err(|error| read_failed(remote, error))?;
    decide(remote, &settings).map_err(|refusal| match refusal {
        Refusal::Refused { setting, write } => refused(setting, write),
        Refusal::Unparsed(source) => Error::RemoteConfig {
            remote: remote.to_owned(),
            source,
        },
    })
}

/// What a failed read of the remote's settings makes of the fetch: it does not start
/// either way. A read that was ENDED — which, with nobody holding its cancel signal, only
/// the repository closing does (`SharedRepository::end_invocations`, which ends every
/// invocation in its registry and any that starts after) — is the fetch cancelled before
/// it started, [`Error::GitCancelled`] with no lock files, since a read writes none;
/// every other failure is [`Error::RemoteConfig`], carrying it.
fn read_failed(remote: &str, error: Error) -> Error {
    match error {
        Error::GitReadCancelled { arguments } => Error::GitCancelled {
            arguments,
            stranded_locks: Vec::new(),
        },
        other => Error::RemoteConfig {
            remote: remote.to_owned(),
            source: Box::new(other),
        },
    }
}

/// Why [`decide`] refused.
#[derive(Debug)]
enum Refusal {
    Refused {
        setting: String,
        write: RefusedWrite,
    },
    /// A configured refspec git's own parser would refuse too.
    Unparsed(Box<dyn std::error::Error + Send + Sync>),
}

/// The decision over what git read: a mirror, then each refspec in order, its
/// destination under `refs/heads/`, or under `refs/tags/` while pruning is on.
fn decide(remote: &str, settings: &FetchSettings) -> Result<(), Refusal> {
    if settings.mirror == Some(true) {
        return Err(Refusal::Refused {
            setting: format!("remote.{remote}.mirror = true"),
            write: RefusedWrite::Mirror,
        });
    }
    let pruning = prunes(settings, remote);
    for written in &settings.refspecs {
        let spec = gix::refspec::parse(written.as_bstr(), gix::refspec::parse::Operation::Fetch)
            .map_err(|source| Refusal::Unparsed(Box::new(source)))?;
        let Some(destination) = spec.destination() else {
            continue;
        };
        let setting = || format!("remote.{remote}.fetch = {}", written.as_bstr());
        if writes_under(destination, "refs/heads/") {
            return Err(Refusal::Refused {
                setting: setting(),
                write: RefusedWrite::LocalBranches,
            });
        }
        if let Some(prune_setting) = &pruning
            && writes_under(destination, "refs/tags/")
        {
            return Err(Refusal::Refused {
                setting: format!("{} with {prune_setting} = true", setting()),
                write: RefusedWrite::LocalTags,
            });
        }
    }
    Ok(())
}

/// The setting that turns pruning on for a fetch of `remote`, as `builtin/fetch.c`
/// decides it at v2.30.0 and v2.56.0 — `remote.<name>.prune` when set, else
/// `fetch.prune`, else off — or `None` when nothing does. `fetch.pruneTags` and
/// `remote.<name>.pruneTags` are not read: the fetch passes `--no-prune-tags`, which
/// overrides both (`prune_tags` is set from them only while it is still unset), and
/// neither turns on `prune` itself.
fn prunes(settings: &FetchSettings, remote: &str) -> Option<String> {
    match settings.remote_prune {
        Some(true) => Some(format!("remote.{remote}.prune")),
        Some(false) => None,
        None => (settings.fetch_prune == Some(true)).then(|| "fetch.prune".to_owned()),
    }
}

/// The file git would read `remote` from in place of configuration, if there is
/// one: `remotes/<name>` or `branches/<name>` in the common directory, for a name
/// git reads them for (`valid_remote_nick`: not empty, `.` or `..`, and no `/`).
fn definition_file(common_dir: &Path, remote: &str) -> Option<PathBuf> {
    if remote.is_empty() || remote == "." || remote == ".." || remote.contains('/') {
        return None;
    }
    ["remotes", "branches"]
        .iter()
        .map(|dir| common_dir.join(dir).join(remote))
        .find(|file| file.exists())
}

/// Whether a fetch destination can name a ref under `namespace` (`refs/heads/`
/// or `refs/tags/`, with the slash): an exact ref there, a glob whose fixed
/// prefix lies inside it, or a glob wide enough to cover it (`refs/*`). A
/// destination not under `refs/` is read as git's `get_local_ref` reads it:
/// `heads/`, `tags/` and `remotes/` get `refs/` in front, any other name is a
/// branch, and an unqualified glob is one git ignores ("funny ref") and
/// writes nowhere.
fn writes_under(destination: &BStr, namespace: &str) -> bool {
    let glob = destination.contains(&b'*');
    let qualified = if destination.starts_with(b"refs/") {
        destination.to_vec()
    } else if glob {
        return false;
    } else if [b"heads/".as_slice(), b"tags/", b"remotes/"]
        .iter()
        .any(|prefix| destination.starts_with(prefix))
    {
        [b"refs/".as_slice(), destination].concat()
    } else {
        [b"refs/heads/".as_slice(), destination].concat()
    };
    let fixed = qualified
        .split(|&byte| byte == b'*')
        .next()
        .unwrap_or_default();
    fixed.starts_with(namespace.as_bytes()) || (glob && namespace.as_bytes().starts_with(fixed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heads(destination: &str) -> bool {
        writes_under(destination.as_bytes().as_bstr(), "refs/heads/")
    }

    fn tags(destination: &str) -> bool {
        writes_under(destination.as_bytes().as_bstr(), "refs/tags/")
    }

    /// Caught by: matching on the prefix alone (misses `refs/*`), on the glob alone
    /// (misses an exact branch), or forgetting that an unqualified name is a branch.
    /// The unqualified forms are what git 2.55 writes for them, checked by running it.
    #[test]
    fn every_way_a_destination_can_reach_local_branches_is_seen() {
        for destination in [
            "refs/heads/*",
            "refs/heads/main",
            "refs/heads/team/*",
            "refs/*",
            "refs/h*",
            "main",
            "HEAD",
            "heads/main",
        ] {
            assert!(heads(destination), "{destination} was let through");
        }
    }

    /// The remote-tracking destinations git gives a clone, and what git's own reading
    /// of an unqualified name puts elsewhere: `remotes/origin/x` and `tags/x` get
    /// `refs/` in front, and an unqualified glob is ignored and writes nothing at all.
    #[test]
    fn the_destinations_that_are_not_local_branches_are_let_through() {
        for destination in [
            "refs/remotes/origin/*",
            "refs/remotes/origin/main",
            "refs/tags/*",
            "refs/notes/*",
            "refs/remotes/*",
            "refs/headsup/*",
            "remotes/origin/main",
            "remotes/origin/*",
            "tags/v1",
            "team/*",
            "heads/*",
        ] {
            assert!(!heads(destination), "{destination} was refused");
        }
    }

    fn refusal(settings: &FetchSettings) -> Option<(String, RefusedWrite)> {
        match decide("origin", settings) {
            Ok(()) => None,
            Err(Refusal::Refused { setting, write }) => Some((setting, write)),
            Err(Refusal::Unparsed(source)) => panic!("unexpectedly unparsed: {source}"),
        }
    }

    fn with(refspecs: &[&str]) -> FetchSettings {
        FetchSettings {
            refspecs: refspecs
                .iter()
                .map(|spec| spec.as_bytes().to_vec())
                .collect(),
            ..FetchSettings::default()
        }
    }

    /// The decision over what git read: the mirror first, then each refspec in order, its
    /// setting quoted as written. Caught by: the mirror read as anything but `true`, a
    /// refspec after the first unread, or a negative refspec, which writes nothing,
    /// refused.
    #[test]
    fn the_decision_quotes_the_setting_that_refused() {
        assert_eq!(
            refusal(&with(&["+refs/heads/*:refs/remotes/origin/*"])),
            None
        );
        assert_eq!(refusal(&FetchSettings::default()), None);
        let mirror = FetchSettings {
            mirror: Some(true),
            ..with(&["+refs/heads/*:refs/remotes/origin/*"])
        };
        assert_eq!(
            refusal(&mirror),
            Some((
                "remote.origin.mirror = true".to_owned(),
                RefusedWrite::Mirror
            ))
        );
        let not_mirror = FetchSettings {
            mirror: Some(false),
            ..FetchSettings::default()
        };
        assert_eq!(refusal(&not_mirror), None);
        assert_eq!(
            refusal(&with(&[
                "^refs/heads/secret",
                "+refs/heads/*:refs/remotes/origin/*",
                "+refs/*:refs/*"
            ])),
            Some((
                "remote.origin.fetch = +refs/*:refs/*".to_owned(),
                RefusedWrite::LocalBranches
            ))
        );
        assert!(matches!(
            decide("origin", &with(&["refs/heads/*:refs/remotes/origin"])),
            Err(Refusal::Unparsed(_))
        ));
    }

    /// An empty configured refspec (`fetch =`) is git's `HEAD` with no destination
    /// (`parse_refspec`: an empty source is `HEAD`), which fetches into `FETCH_HEAD` and
    /// writes no ref — run on git 2.30.9 and 2.56.0 — so it is neither refused nor
    /// unparsed, alone or beside a refspec that does refuse. gix-refspec 0.45.1 reads it as
    /// git does (`fetch_head_only`); caught by: a parser that rejects it, which would refuse
    /// the fetch as configuration Cairn could not read, or an empty value skipped so that
    /// a refusing refspec after it is never read.
    #[test]
    fn an_empty_refspec_writes_nothing_and_is_not_refused() {
        assert_eq!(refusal(&with(&[""])), None);
        assert_eq!(
            refusal(&with(&["", "+refs/heads/*:refs/remotes/origin/*"])),
            None
        );
        assert_eq!(
            refusal(&with(&["", "+refs/heads/*:refs/heads/*"])),
            Some((
                "remote.origin.fetch = +refs/heads/*:refs/heads/*".to_owned(),
                RefusedWrite::LocalBranches
            ))
        );
    }

    /// A read ended by the repository's close is the fetch cancelled, as a close of a running
    /// fetch is, and any other failure is the configuration unread; neither starts a fetch.
    /// Caught by: a close that lands during the check reported as a failure to read the
    /// configuration (`a_fetch_closed_as_it_starts_is_still_ended`, in `cairn-app`, end to
    /// end), or a failure read as a cancel the user never asked for.
    #[test]
    fn a_read_ended_by_a_close_is_a_cancel_and_any_other_failure_refuses() {
        let ended = read_failed(
            "origin",
            Error::GitReadCancelled {
                arguments: "config --includes --null --type=bool --get remote.origin.mirror"
                    .to_owned(),
            },
        );
        assert!(
            matches!(
                &ended,
                Error::GitCancelled { stranded_locks, .. } if stranded_locks.is_empty()
            ),
            "{ended:?}"
        );
        let unread = read_failed(
            "origin",
            Error::UnexpectedGitOutput {
                arguments: "config".to_owned(),
                record: "x".to_owned(),
            },
        );
        assert!(
            matches!(&unread, Error::RemoteConfig { remote, .. } if remote == "origin"),
            "{unread:?}"
        );
    }

    /// Pruning as `builtin/fetch.c` decides it: `remote.<name>.prune` when set, either way,
    /// else `fetch.prune`, else off; and only with pruning on is a tag refspec refused.
    /// Caught by: `fetch.prune` beating the remote's own setting, or an unset key read as on.
    #[test]
    fn a_tag_refspec_is_refused_only_while_git_would_prune() {
        let tags = |remote_prune, fetch_prune| FetchSettings {
            remote_prune,
            fetch_prune,
            ..with(&["+refs/tags/*:refs/tags/*"])
        };
        let tag_refusal = |setting: &str| {
            Some((
                format!("remote.origin.fetch = +refs/tags/*:refs/tags/* with {setting} = true"),
                RefusedWrite::LocalTags,
            ))
        };
        assert_eq!(refusal(&tags(None, None)), None);
        assert_eq!(refusal(&tags(None, Some(false))), None);
        assert_eq!(refusal(&tags(Some(false), Some(true))), None);
        assert_eq!(refusal(&tags(None, Some(true))), tag_refusal("fetch.prune"));
        assert_eq!(
            refusal(&tags(Some(true), Some(false))),
            tag_refusal("remote.origin.prune")
        );
    }

    /// The files git reads a remote from, for the names git reads them for: a name with a
    /// `/`, an empty one, `.` and `..` never name such a file. Caught by: a lookup that
    /// joins any name onto the directory, escaping it.
    #[test]
    fn a_definition_file_is_looked_for_only_under_a_name_git_reads_one_for() {
        let root =
            std::env::temp_dir().join(format!("cairn-definition-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["remotes", "branches", "remotes/sub"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("branches/upstream"), "x").unwrap();
        std::fs::write(root.join("remotes/other"), "x").unwrap();
        std::fs::write(root.join("remotes/sub/nested"), "x").unwrap();
        assert_eq!(
            definition_file(&root, "upstream"),
            Some(root.join("branches/upstream"))
        );
        assert_eq!(
            definition_file(&root, "other"),
            Some(root.join("remotes/other"))
        );
        for name in ["origin", "sub/nested", "", ".", ".."] {
            assert_eq!(definition_file(&root, name), None, "{name:?}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_tags_are_told_apart_from_branches() {
        assert!(tags("refs/tags/*"));
        assert!(tags("refs/tags/v1"));
        assert!(tags("refs/*"));
        assert!(tags("tags/v1"), "git reads `tags/x` as refs/tags/x");
        assert!(!tags("refs/remotes/origin/*"));
        assert!(!tags("refs/heads/*"));
        assert!(!tags("main"), "an unqualified name is a branch, not a tag");
        assert!(!tags("tags/*"), "an unqualified glob writes nothing");
    }
}
