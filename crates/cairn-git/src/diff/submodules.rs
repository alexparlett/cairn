//! Which changed submodules the user's own `git log` hides: `diff.ignoreSubmodules`, and
//! the per-submodule `submodule.<name>.ignore` that overrides it.
//!
//! `git diff-tree` is plumbing and never reads `diff.ignoreSubmodules`; porcelain does, and
//! with it set to `all` `git log` and `git show` list no change to a submodule (a gitlink,
//! mode `160000`) unless that submodule's own setting says otherwise. The per-submodule
//! setting both read alike — `diff-tree` already hides a submodule whose own setting is
//! `all` — so what differs is only a submodule with no setting of its own under a global
//! `all`. The rules, read from git's `diff.c`, `submodule.c` and `submodule-config.c` at
//! v2.30.0 and v2.56.0 and reproduced against both:
//!
//! - `diff.ignoreSubmodules` is the last value across the configuration, exactly one of
//!   `all`, `dirty`, `untracked` or `none`; the bare key or any other spelling (`ALL`
//!   included) makes `git log` refuse to run. Only `all` hides anything from a list of
//!   changed paths: the other two describe a working tree.
//! - A gitlink is hidden when it was added, deleted or changed as a gitlink on both sides;
//!   a type change between a gitlink and a file is never hidden.
//! - A submodule's own setting is `submodule.<name>.ignore` in the configuration, else the
//!   same key in `.gitmodules`; any value replaces the global one (`none` shows it, a value
//!   git does not know makes it refuse). The name comes from `.gitmodules`, mapping its
//!   path: the last name to claim a path has it.
//! - `.gitmodules` is the working tree's file when one exists, else the index's, else
//!   `HEAD`'s — never the version in the commit being shown — and none at all in a bare
//!   repository, or while the index holds it unmerged.
//! - In `.gitmodules`, an `ignore` value git does not know is warned about and skipped,
//!   leaving the one before it; a suspicious name (empty, or with a `..` component) is
//!   skipped whole.
//!
//! Hiding the gitlinks after git answers would not be what `git log` shows: git hides them
//! before rename detection, so they are never counted against `diff.renameLimit`, and a
//! search that the hidden gitlinks would push past the limit runs in `git log` and pairs
//! renames that a list filtered afterwards would show unpaired. So the query asks git not
//! to queue them: `--ignore-submodules=all` when no submodule has a setting of its own —
//! which is exact, since then a gitlink's own setting can only be unset or `all` — and
//! otherwise `:(exclude)` pathspecs naming the hidden gitlinks of a first answer
//! (`crate::diff::changes`).

use std::collections::{BTreeMap, BTreeSet};

use cairn_model::{ChangeStatus, ChangedFile, FileMode, RepoPath};
use gix::bstr::{BString, ByteSlice};

use super::git_config::{invalid, last_value, parse_bool};
use crate::Error;

/// What the user's configuration hides of a list of changed paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Hiding {
    /// Nothing: `diff.ignoreSubmodules` is unset or not `all`, and `diff-tree` already
    /// hides what a submodule's own setting does.
    Nothing,
    /// Every gitlink: `diff.ignoreSubmodules=all`, and no submodule has a setting of its
    /// own that could show one, or make git refuse.
    EveryGitlink,
    /// Every gitlink but those at these paths, whose submodule has a setting of its own —
    /// which `diff-tree` applies itself, showing it, hiding it, or refusing as `git log`
    /// does.
    GitlinksExcept(BTreeSet<Vec<u8>>),
}

impl Hiding {
    /// The configuration gix read when the repository was opened, and `.gitmodules` where
    /// git reads it from. `has_worktree` is whether git runs with a working tree.
    pub(super) fn read(repo: &gix::Repository, has_worktree: bool) -> Result<Self, Error> {
        let config = repo.config_snapshot();
        let file = config.plumbing();
        match last_value(file, "diff", None, "ignoreSubmodules") {
            None => return Ok(Self::Nothing),
            Some(Some(value)) if value == "all" => {}
            Some(Some(value))
                if [&b"dirty"[..], b"untracked", b"none"].contains(&value.as_bytes()) =>
            {
                return Ok(Self::Nothing);
            }
            Some(value) => return Err(invalid("diff.ignoreSubmodules", value)),
        }
        let modules = if has_worktree {
            Modules::read(repo)
        } else {
            Modules::default()
        };
        let own_setting: BTreeSet<Vec<u8>> = modules
            .path_name
            .iter()
            .filter(|(_, name)| {
                let configured = last_value(file, "submodule", Some(name.as_slice()), "ignore");
                let setting = match configured {
                    Some(value) => Some(value),
                    None => modules.ignore.get(*name).cloned().map(Some),
                };
                // Unset or `all` is hidden either way; anything else is the submodule's own
                // to decide — shown, or refused by git — and `diff-tree` decides it.
                let hidden_anyway = match &setting {
                    None => true,
                    Some(Some(value)) => value == "all",
                    Some(None) => false,
                };
                !hidden_anyway
            })
            .map(|(path, _)| path.clone())
            .collect();
        Ok(if own_setting.is_empty() && !modules.git_may_refuse {
            Self::EveryGitlink
        } else {
            Self::GitlinksExcept(own_setting)
        })
    }

    /// The paths of `file` that are a gitlink this hides, on either side.
    pub(super) fn hidden_paths<'f>(&self, file: &'f ChangedFile) -> Vec<&'f RepoPath> {
        let shown = match self {
            Self::Nothing => return Vec::new(),
            Self::EveryGitlink => None,
            Self::GitlinksExcept(shown) => Some(shown),
        };
        let hidden = |path: &RepoPath| shown.is_none_or(|shown| !shown.contains(path.as_bytes()));
        let gitlink = |mode: Option<FileMode>| mode == Some(FileMode::Submodule);
        let (old, new) = (gitlink(file.old_mode), gitlink(file.new_mode));
        let mut paths = Vec::new();
        match file.status {
            ChangeStatus::TypeChanged => {}
            ChangeStatus::Added => {
                if new && hidden(&file.new_path) {
                    paths.push(&file.new_path);
                }
            }
            ChangeStatus::Deleted => {
                if old && hidden(&file.old_path) {
                    paths.push(&file.old_path);
                }
            }
            ChangeStatus::Modified => {
                if old && new && hidden(&file.new_path) {
                    paths.push(&file.new_path);
                }
            }
            ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => {
                if old && hidden(&file.old_path) {
                    paths.push(&file.old_path);
                }
                if new && hidden(&file.new_path) {
                    paths.push(&file.new_path);
                }
            }
        }
        paths
    }
}

/// The `--ignore-submodules` value the user's `git diff` applies to `path` and plumbing
/// does not: `diff.ignoreSubmodules`, which only porcelain reads, unless the path's
/// submodule has an `ignore` of its own — `submodule.<name>.ignore` in the configuration or
/// in `.gitmodules` — which beats it, and which `diff-files` and `diff-index` apply
/// themselves. Passing the flag would override that setting too, so it is passed only
/// where there is none. A value porcelain refuses is [`Error::InvalidConfig`], as
/// [`Hiding::read`] refuses it; a `.gitmodules` git might refuse to read is left to git.
/// Reproduced with git 2.30.9 and 2.56.0 against `git diff` for a moved, a dirty and an
/// untracked-content submodule under each value, and under each with a setting of its own
/// (`docs/systems/diff.md`, "The working-tree query").
pub(super) fn working_tree_ignore(
    repo: &gix::Repository,
    path: &RepoPath,
) -> Result<Option<&'static str>, Error> {
    let config = repo.config_snapshot();
    let file = config.plumbing();
    let global = match last_value(file, "diff", None, "ignoreSubmodules") {
        None => return Ok(None),
        Some(Some(value)) => match value.as_bytes() {
            b"all" => "all",
            b"dirty" => "dirty",
            b"untracked" => "untracked",
            b"none" => "none",
            _ => return Err(invalid("diff.ignoreSubmodules", Some(value))),
        },
        Some(None) => return Err(invalid("diff.ignoreSubmodules", None)),
    };
    let modules = Modules::read(repo);
    if modules.git_may_refuse {
        return Ok(None);
    }
    let own = modules.path_name.get(path.as_bytes()).is_some_and(|name| {
        last_value(file, "submodule", Some(name.as_slice()), "ignore").is_some()
            || modules.ignore.contains_key(name)
    });
    Ok((!own).then_some(global))
}

/// What `.gitmodules` says, as git's `parse_config` reads it.
#[derive(Debug, Default)]
struct Modules {
    /// Each path a submodule claims, and the name that claimed it last.
    path_name: BTreeMap<Vec<u8>, Vec<u8>>,
    /// Each name's `ignore`, the last value git accepts.
    ignore: BTreeMap<Vec<u8>, BString>,
    /// Whether git might refuse to read the file — it could not be read or parsed, or it
    /// holds a value git's parser dies on. Over-approximated: a file that only might be
    /// refused is answered the way that stays right if it is (`Hiding::GitlinksExcept`).
    git_may_refuse: bool,
}

impl Modules {
    fn read(repo: &gix::Repository) -> Self {
        let bytes = match gitmodules_bytes(repo) {
            Source::None => return Self::default(),
            Source::Unreadable => {
                return Self {
                    git_may_refuse: true,
                    ..Self::default()
                };
            }
            Source::Bytes(bytes) => bytes,
        };
        let options = gix::config::file::init::Options::default();
        let Ok(file) = gix::config::File::from_bytes_no_includes(
            &bytes,
            gix::config::file::Metadata::api(),
            options,
        ) else {
            return Self {
                git_may_refuse: true,
                ..Self::default()
            };
        };
        Self::parse(&file)
    }

    fn parse(file: &gix::config::File) -> Self {
        let mut modules = Self::default();
        let mut name_path: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        let Some(sections) = file.sections_by_name("submodule") else {
            return modules;
        };
        for section in sections {
            let Some(name) = section.header().subsection_name() else {
                continue;
            };
            let name = name.as_bytes().to_vec();
            let body = section.body();
            // A value git's parser dies on, in any section: over-approximated, since a
            // refusal is answered correctly by the cautious path either way. gix reads the
            // bare key (`path` with no `=`), which git refuses, as an empty value, so every
            // empty value counts.
            let bare = |key: &str| body.values(key).iter().any(|value| value.is_empty());
            let refused_update = body.values("update").iter().any(|value| {
                !["none", "checkout", "rebase", "merge"].contains(&value.to_str_lossy().as_ref())
            });
            let refused_bool = |key: &str| {
                body.values(key).iter().any(|value| {
                    parse_bool(Some(value.as_slice())).is_none()
                        && !(key == "fetchRecurseSubmodules" && value == "on-demand")
                })
            };
            if ["path", "ignore", "url", "update", "branch"]
                .into_iter()
                .any(bare)
                || refused_update
                || refused_bool("shallow")
                || refused_bool("fetchRecurseSubmodules")
            {
                modules.git_may_refuse = true;
            }
            if !is_valid_name(&name) {
                continue;
            }
            for path in body.values("path") {
                if path.starts_with(b"-") {
                    continue;
                }
                if let Some(old) = name_path.insert(name.clone(), path.to_vec()) {
                    modules.path_name.remove(&old);
                }
                modules.path_name.insert(path.to_vec(), name.clone());
            }
            for value in body.values("ignore") {
                if [&b"all"[..], b"dirty", b"untracked", b"none"].contains(&value.as_bytes()) {
                    modules.ignore.insert(name.clone(), value);
                }
            }
        }
        modules
    }
}

/// git's `check_submodule_name`: not empty, and no `..` as a path component, with `/` and
/// `\` both separators.
fn is_valid_name(name: &[u8]) -> bool {
    !name.is_empty()
        && !name
            .split(|byte| *byte == b'/' || *byte == b'\\')
            .any(|component| component == b"..")
}

enum Source {
    None,
    Unreadable,
    Bytes(Vec<u8>),
}

/// `.gitmodules` where git's `config_from_gitmodules` reads it: the working tree's file
/// when anything is there, else the index's at stage 0, else `HEAD`'s — and nothing while
/// the index holds it unmerged.
fn gitmodules_bytes(repo: &gix::Repository) -> Source {
    let index = match repo.try_index() {
        Ok(index) => index,
        Err(_) => return Source::Unreadable,
    };
    let path = ".gitmodules".as_bytes().as_bstr();
    if let Some(index) = &index
        && let Some(range) = index.entry_range(path)
        && index.entries()[range]
            .iter()
            .any(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
    {
        return Source::None;
    }
    if let Some(workdir) = repo.workdir() {
        let file = workdir.join(".gitmodules");
        if std::fs::symlink_metadata(&file).is_ok() {
            return match std::fs::read(&file) {
                Ok(bytes) => Source::Bytes(bytes),
                Err(_) => Source::Unreadable,
            };
        }
    }
    let staged = index.as_ref().and_then(|index| {
        index
            .entry_by_path_and_stage(path, gix::index::entry::Stage::Unconflicted)
            .map(|entry| entry.id)
    });
    let id = match staged {
        Some(id) => Some(id),
        None => match repo.head_tree() {
            Ok(tree) => tree
                .find_entry(".gitmodules")
                .map(|entry| entry.object_id()),
            Err(_) => None,
        },
    };
    match id {
        None => Source::None,
        Some(id) => match repo.find_object(id) {
            Ok(object) if object.kind == gix::object::Kind::Blob => {
                Source::Bytes(object.detach().data)
            }
            Ok(_) | Err(_) => Source::Unreadable,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modules(text: &str) -> Modules {
        Modules::parse(&gix::config::File::try_from(text).unwrap())
    }

    /// The mapping as git's `parse_config` builds it: the last name to claim a path has
    /// it, a name's later path releases its earlier one, a path starting with `-` and a
    /// suspicious name are skipped, and an `ignore` git does not know leaves the one before.
    #[test]
    fn gitmodules_reads_the_way_git_parse_config_does() {
        let read = modules(
            "[submodule \"a\"]\n\tpath = s\n\tignore = none\n\tignore = bogus\n\
             [submodule \"b\"]\n\tpath = s\n\
             [submodule \"c\"]\n\tpath = first\n\tpath = second\n\
             [submodule \"d\"]\n\tpath = -dash\n\
             [submodule \"x/../y\"]\n\tpath = evil\n\tignore = none\n",
        );
        assert_eq!(
            read.path_name,
            BTreeMap::from([
                (b"s".to_vec(), b"b".to_vec()),
                (b"second".to_vec(), b"c".to_vec()),
            ])
        );
        assert_eq!(
            read.ignore.get(b"a".as_slice()),
            Some(&BString::from("none"))
        );
        assert_eq!(read.ignore.len(), 1);
        assert!(!read.git_may_refuse);
    }

    /// Each value git's parser dies on marks the file, and an ordinary one does not.
    #[test]
    fn a_gitmodules_git_refuses_is_marked() {
        for text in [
            "[submodule \"a\"]\n\tpath\n",
            "[submodule \"a\"]\n\tpath = s\n\tignore\n",
            "[submodule \"a\"]\n\turl\n",
            "[submodule \"a\"]\n\tupdate = !rm -rf\n",
            "[submodule \"a\"]\n\tupdate = sometimes\n",
            "[submodule \"a\"]\n\tshallow = perhaps\n",
            "[submodule \"a\"]\n\tfetchRecurseSubmodules = perhaps\n",
        ] {
            assert!(modules(text).git_may_refuse, "{text:?}");
        }
        let ordinary = modules(
            "[submodule \"a\"]\n\tpath = s\n\turl = ../a\n\tbranch = main\n\tupdate = rebase\n\
             \tshallow = true\n\tfetchRecurseSubmodules = on-demand\n\tignore = dirty\n",
        );
        assert!(!ordinary.git_may_refuse);
    }

    #[test]
    fn a_suspicious_name_is_what_check_submodule_name_refuses() {
        for name in ["", "..", "../x", "x/..", "a\\..\\b", "a/../b"] {
            assert!(!is_valid_name(name.as_bytes()), "{name:?}");
        }
        for name in ["a", "a/b", "..a", "a..", "a/.../b", ".x"] {
            assert!(is_valid_name(name.as_bytes()), "{name:?}");
        }
    }
}
