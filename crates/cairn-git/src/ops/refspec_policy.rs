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
//! The check reads the repository afresh rather than through the handle the
//! worker opened at startup, because gix reads configuration once at open and
//! `git` reads it on every run: a refspec added in a terminal since Cairn
//! started must be seen by the same fetch that would act on it. It reads it
//! the way the child will: the child's environment is built, never inherited
//! (`GitEnvironment`), so a `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM` or
//! `GIT_CONFIG_COUNT` in Cairn's own environment never reaches git, and gix
//! is opened with those denied too — otherwise the check could pass a fetch
//! git would then prune. The one file the two can still disagree on is the
//! SYSTEM configuration: gix reads `/etc/gitconfig`, git reads its own
//! `$(sysconfdir)/gitconfig`, the same file for a distribution's git and a
//! different one for a git installed under another prefix; asking git where
//! its file is would spawn a process outside `GitEnvironment`, so this is a
//! stated residual, not a check. What a configured remote name git does not
//! know, or a URL, has no configured refspecs and passes; git then fetches
//! into `FETCH_HEAD` alone, or refuses the name itself.

use std::path::Path;

use gix::bstr::{BStr, ByteSlice};
use gix::open::permissions::{Config, Environment};
use gix::open::{Options, Permissions};
use gix::sec::Permission;

use crate::Error;
use crate::error::RefusedWrite;

/// Refuses a fetch of `remote` in the repository at `git_dir` whose
/// configuration would have it write local branches or, with pruning on,
/// delete local tags; see the module docs.
pub(crate) fn check(git_dir: &Path, remote: &str) -> Result<(), Error> {
    let repo = gix::open_opts(
        git_dir,
        Options::default().permissions(as_the_child_reads()),
    )
    .map_err(|source| Error::Open {
        path: git_dir.to_owned(),
        source: Box::new(source),
    })?;
    let Some(found) = repo.try_find_remote(remote.as_bytes().as_bstr()) else {
        return Ok(());
    };
    let found = found.map_err(|source| Error::RemoteConfig {
        remote: remote.to_owned(),
        source: Box::new(source),
    })?;
    let config = repo.config_snapshot();
    let refused = |setting: String, write: RefusedWrite| Error::FetchRefused {
        remote: remote.to_owned(),
        setting,
        write,
    };
    if config.boolean(format!("remote.{remote}.mirror").as_str()) == Some(true) {
        return Err(refused(
            format!("remote.{remote}.mirror = true"),
            RefusedWrite::Mirror,
        ));
    }
    let pruning = prunes(&config, remote);
    for spec in found.refspecs(gix::remote::Direction::Fetch) {
        let spec = spec.to_ref();
        let Some(destination) = spec.destination() else {
            continue;
        };
        let setting = || format!("remote.{remote}.fetch = {}", spec.to_bstring());
        if writes_under(destination, "refs/heads/") {
            return Err(refused(setting(), RefusedWrite::LocalBranches));
        }
        if let Some(prune_setting) = &pruning
            && writes_under(destination, "refs/tags/")
        {
            return Err(refused(
                format!("{} with {prune_setting} = true", setting()),
                RefusedWrite::LocalTags,
            ));
        }
    }
    Ok(())
}

/// Read configuration as the child git will: from the files, never from
/// Cairn's own `GIT_*` environment, which the child does not inherit.
fn as_the_child_reads() -> Permissions {
    Permissions {
        env: Environment {
            git_prefix: Permission::Deny,
            ..Environment::all()
        },
        config: Config {
            env: false,
            ..Config::all()
        },
        ..Permissions::secure()
    }
}

/// The setting that turns pruning on for a fetch of `remote`, as
/// git-config(1) has it — `remote.<name>.prune` overrides `fetch.prune`, and
/// both default to off — or `None` when nothing does.
fn prunes(config: &gix::config::Snapshot<'_>, remote: &str) -> Option<String> {
    let remote_setting = format!("remote.{remote}.prune");
    match config.boolean(remote_setting.as_str()) {
        Some(true) => Some(remote_setting),
        Some(false) => None,
        None => config
            .boolean("fetch.prune")
            .filter(|on| *on)
            .map(|_| "fetch.prune".to_owned()),
    }
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
