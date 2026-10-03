//! What a commit's or a comparison's diff reads that is not in its commits, so that whoever
//! keeps an answer can tell when it would no longer be the one git gives.
//!
//! An answer about two commits is the same answer only while everything else it was read
//! from is the same. That is three kinds of file outside the object database, and what the
//! index and `HEAD` hold:
//!
//! - **The configuration**, every file of it: the system file, the XDG and global files,
//!   `$GIT_DIR/config` and `config.worktree`, and every `include.path` and
//!   `includeIf.<condition>.path` target, whether or not it exists — creating one is a
//!   change — and whether or not its condition holds, since that can change too.
//!   `includeIf "onbranch:..."` depends on `HEAD`, so `HEAD` is one of them where such a
//!   section exists. gix reads its copy when the repository is opened; `git` reads its own
//!   as it runs, from the environment Cairn built, which carries `HOME` and
//!   `XDG_CONFIG_HOME` but none of the `GIT_CONFIG_*` file variables — so the files both
//!   name are listed, each once.
//! - **The attributes and submodules every path reads**: `$GIT_DIR/info/attributes`,
//!   `core.attributesFile` (else the XDG `git/attributes`), the system `gitattributes`, and
//!   the working tree's `.gitmodules`.
//! - **The attributes one path reads**: the working tree's `.gitattributes` in the
//!   directory holding it and in each directory above (`DiffInputs::directories`,
//!   `DiffInputs::attributes_in`). git reads them before the index's (`crate::diff`'s
//!   session reads them the same way), and for a commit's diff `git diff-tree` reads them
//!   both to call a file binary and to score a rename: an unstaged `-diff` turns a
//!   CRLF file's rename into a deletion and an addition.
//! - **The index and `HEAD`**, by what they hold rather than by their files:
//!   [`StagedInputs`], every `.gitattributes` the index holds and its `.gitmodules` (else
//!   `HEAD`'s). The index file changes whenever git refreshes a stat, which changes nothing
//!   a diff reads, so its stamp says only when to look again.
//!
//! Nothing here reads a file's content but the index's and `HEAD`'s; the caller decides how
//! to tell that a file moved (`crates/cairn-app/src/worker/diff_freshness.rs`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cairn_model::{Oid, RepoPath};
use gix::bstr::ByteSlice as _;

use crate::object_id::model_id;
use crate::{Error, Repository};

/// How an environment is read by name, as gix's storage locations ask it.
type Environment<'a> = dyn FnMut(&str) -> Option<OsString> + 'a;

/// The files outside the object database a commit's or a comparison's diff reads, as this
/// handle's configuration names them. Opening the repository again can name others: a
/// changed `core.attributesFile`, a new include.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffInputs {
    configuration: Vec<PathBuf>,
    global: Vec<PathBuf>,
    index: PathBuf,
    workdir: Option<PathBuf>,
}

impl DiffInputs {
    /// Every configuration file gix or git reads, existing or not.
    pub fn configuration(&self) -> &[PathBuf] {
        &self.configuration
    }

    /// The attribute files every path reads, and the working tree's `.gitmodules`,
    /// existing or not.
    pub fn global(&self) -> &[PathBuf] {
        &self.global
    }

    /// The index file. Its stamp says when to read [`StagedInputs`] again; it is not what
    /// a diff depends on.
    pub fn index(&self) -> &Path {
        &self.index
    }

    /// The directories whose `.gitattributes` git reads for `path`, the top of the tree
    /// (empty) first, as repository-relative bytes: every directory above it.
    pub fn directories(path: &RepoPath) -> impl Iterator<Item = &[u8]> {
        let bytes = path.as_bytes();
        std::iter::once(&bytes[..0]).chain(
            bytes
                .iter()
                .enumerate()
                .filter(|(_, byte)| **byte == b'/')
                .map(move |(at, _)| &bytes[..at]),
        )
    }

    /// The working tree's `.gitattributes` in `directory`, one of [`Self::directories`];
    /// `None` in a bare repository, which has no working tree to read.
    pub fn attributes_in(&self, directory: &[u8]) -> Option<PathBuf> {
        let mut file = self.workdir.clone()?;
        if !directory.is_empty() {
            file.push(gix::path::from_bstr(directory.as_bstr()));
        }
        file.push(".gitattributes");
        Some(file)
    }
}

/// What the index holds that a diff of commits reads — every `.gitattributes` entry and the
/// `.gitmodules` entries, each by path, stage and blob — and, where the index holds no
/// `.gitmodules`, the one `HEAD` holds, which is where git reads it next. Compared by
/// value: a refreshed stat leaves it equal, a staged attribute edit does not.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StagedInputs {
    entries: Vec<(Vec<u8>, u32, Oid)>,
    head_gitmodules: Option<Oid>,
}

impl Repository {
    /// [`DiffInputs`] as this handle's configuration names them.
    pub fn diff_inputs(&self) -> DiffInputs {
        let repo = self.inner();
        let mut configuration = Vec::new();
        let add = |list: &mut Vec<PathBuf>, path: PathBuf| {
            if !list.contains(&path) {
                list.push(path);
            }
        };

        // gix's view, from this process's environment, and git's, from the roster its
        // environment inherits (`HOME`, `XDG_CONFIG_HOME`) and nothing else.
        let mut ours = |name: &str| gix::path::env::var(name);
        let mut gits = |name: &str| match name {
            "HOME" | "XDG_CONFIG_HOME" => gix::path::env::var(name),
            _ => None,
        };
        let environments: [&mut Environment<'_>; 2] = [&mut ours, &mut gits];
        for environment in environments {
            for source in [
                gix::config::Source::System,
                gix::config::Source::Git,
                gix::config::Source::User,
            ] {
                if let Some(path) = source.storage_location(&mut *environment) {
                    add(&mut configuration, path);
                }
            }
        }
        add(&mut configuration, repo.common_dir().join("config"));
        add(&mut configuration, repo.git_dir().join("config.worktree"));

        let snapshot = repo.config_snapshot();
        let file = snapshot.plumbing();
        let home = gix::path::env::home_dir();
        let context = gix::config::path::interpolate::Context {
            git_install_dir: None,
            home_dir: home.as_deref(),
            home_for_user: Some(gix::config::path::interpolate::home_for_user),
        };
        let mut on_branch = false;
        for section in file.sections() {
            if let Some(path) = &section.meta().path {
                add(&mut configuration, path.clone());
            }
            let header = section.header();
            let name = header.name();
            let included = if name.eq_ignore_ascii_case(b"include") {
                header.subsection_name().is_none()
            } else if name.eq_ignore_ascii_case(b"includeIf") {
                on_branch |= header
                    .subsection_name()
                    .is_some_and(|condition| condition.starts_with(b"onbranch:"));
                header.subsection_name().is_some()
            } else {
                false
            };
            if !included {
                continue;
            }
            for value in section.values("path") {
                let Ok(target) = gix::config::Path::from(value).interpolate(context) else {
                    continue;
                };
                let target = if target.is_relative() {
                    match section.meta().path.as_deref().and_then(Path::parent) {
                        Some(directory) => directory.join(target),
                        None => continue,
                    }
                } else {
                    target
                };
                add(&mut configuration, target);
            }
        }
        if on_branch {
            add(&mut configuration, repo.git_dir().join("HEAD"));
        }

        let mut global = Vec::new();
        add(
            &mut global,
            repo.common_dir().join("info").join("attributes"),
        );
        match snapshot.trusted_path("core.attributesFile") {
            Ok(Some(path)) => add(&mut global, path),
            Ok(None) | Err(_) => {
                for environment in [
                    &mut ours as &mut dyn FnMut(&str) -> Option<OsString>,
                    &mut gits,
                ] {
                    if let Some(path) = gix::path::env::xdg_config("attributes", environment) {
                        add(&mut global, path);
                    }
                }
            }
        }
        if let Some(path) = gix::attrs::Source::System.storage_location(&mut |_| None) {
            add(&mut global, path);
        }
        if let Some(workdir) = self.workdir() {
            add(&mut global, workdir.join(".gitmodules"));
        }

        DiffInputs {
            configuration,
            global,
            index: repo.index_path(),
            workdir: self.workdir().map(Path::to_owned),
        }
    }

    /// The commit `HEAD` names now; `None` on an unborn branch, or where it cannot be
    /// read. With the index file's stamp, it says when [`StagedInputs`] may have moved:
    /// git reads `HEAD`'s `.gitmodules` where the index holds none.
    pub fn head_id(&self) -> Option<Oid> {
        let id = self.inner().head_id().ok()?;
        model_id(&id).ok()
    }

    /// [`StagedInputs`] as the index holds them now: nothing where there is no index
    /// file, where git reads no in-tree attributes either.
    pub fn staged_inputs(&self) -> Result<StagedInputs, Error> {
        let setup = |source: Box<dyn std::error::Error + Send + Sync>| Error::DiffSetup { source };
        let repo = self.inner();
        let index = repo
            .index_or_empty()
            .map_err(|source| setup(Box::new(source)))?;
        let mut entries = Vec::new();
        for entry in index.entries() {
            let path = entry.path(&index);
            let read = path == ".gitattributes"
                || path.ends_with(b"/.gitattributes")
                || path == ".gitmodules";
            if read {
                entries.push((path.to_vec(), entry.stage_raw(), model_id(&entry.id)?));
            }
        }
        let head_gitmodules = if entries.iter().any(|(path, _, _)| path == b".gitmodules") {
            None
        } else {
            match repo.head_tree() {
                Ok(tree) => tree
                    .find_entry(".gitmodules")
                    .map(|entry| model_id(&entry.object_id()))
                    .transpose()?,
                Err(_) => None,
            }
        };
        Ok(StagedInputs {
            entries,
            head_gitmodules,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caught by: leaving out the top of the tree (whose `.gitattributes` applies to every
    /// path), or the path's own directory.
    #[test]
    fn a_path_reads_the_attributes_of_every_directory_above_it() {
        let directories = |path: &str| -> Vec<String> {
            DiffInputs::directories(&RepoPath::from(path))
                .map(|d| String::from_utf8_lossy(d).into_owned())
                .collect()
        };
        assert_eq!(directories("a.txt"), [""]);
        assert_eq!(directories("a/b/c.txt"), ["", "a", "a/b"]);
    }
}
