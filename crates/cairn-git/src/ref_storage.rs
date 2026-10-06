//! Where a repository keeps its refs, decided at open.
//!
//! git keeps refs as files (`refs/`, `packed-refs`) unless the repository's own
//! configuration says otherwise: `extensions.refStorage`, a repository-format-version-1
//! extension git reads from `$GIT_COMMON_DIR/config` alone (not the system or global file,
//! and not through an include), whose values are `files` and `reftable`. gix 0.87 reads no
//! such setting: it opens a reftable repository as if its refs were files, and then fails
//! on the first ref it reads (`HEAD` names `refs/heads/.invalid`). Reftable is not built
//! here (PRD R1.9), so such a repository is refused as it is opened, before the snapshot
//! or anything Cairn runs reads a ref (gix's open itself reads `HEAD`, which in a reftable
//! repository is a placeholder it reads without failing), with a reason the window draws.

use std::path::Path;

use crate::Error;

/// The one value under which refs are files.
const FILES: &str = "files";

/// Refuses `repository` unless its refs are files, as git decides it: a format-version-0
/// repository that sets `extensions.refStorage` at all is
/// [`Error::RefStorageNeedsFormatVersion1`] (git: "repo version is 0, but v1-only
/// extension found"), and any value but `files` is [`Error::RefStorageUnsupported`]. A
/// configuration file that cannot be read was read by gix a moment ago, as it opened, so
/// failing to read it now is reported as an open failure.
pub(crate) fn refuse_unread_storage(repository: &gix::ThreadSafeRepository) -> Result<(), Error> {
    let common = repository
        .common_dir
        .as_deref()
        .unwrap_or_else(|| repository.git_dir());
    let path = || {
        repository
            .work_dir()
            .unwrap_or_else(|| repository.git_dir())
            .to_owned()
    };
    let format = format(common)?;
    match format.storage {
        None => Ok(()),
        Some(storage) if format.version == 0 => Err(Error::RefStorageNeedsFormatVersion1 {
            path: path(),
            storage,
        }),
        Some(storage) if storage == FILES => Ok(()),
        Some(storage) => Err(Error::RefStorageUnsupported {
            path: path(),
            storage,
        }),
    }
}

/// What the repository's own configuration says of its format.
#[derive(Debug, PartialEq, Eq)]
struct Format {
    /// `core.repositoryFormatVersion`; `0` when unset.
    version: i64,
    /// `extensions.refStorage`, as configured.
    storage: Option<String>,
}

/// The repository format as git reads it: from the common directory's `config`, with no
/// include followed; the last value wins, as for any single-valued key. A version that does
/// not parse is gix's to refuse, and it already opened the repository, so it reads as `0`.
fn format(common_dir: &Path) -> Result<Format, Error> {
    let path = common_dir.join("config");
    // git and gix both open a repository with no `config` at all, as format 0 with no
    // extension.
    if !path.exists() {
        return Ok(Format {
            version: 0,
            storage: None,
        });
    }
    let config = gix::config::File::from_path_no_includes(path.clone(), gix::config::Source::Local)
        .map_err(|source| Error::Open {
            path,
            source: Box::new(source),
        })?;
    Ok(Format {
        version: config
            .integer("core.repositoryFormatVersion")
            .ok()
            .flatten()
            .unwrap_or(0),
        storage: config
            .string("extensions.refStorage")
            .map(|value| value.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str, config: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("cairn-ref-storage-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("config"), config).unwrap();
        path
    }

    fn storage(dir: &Path) -> Option<String> {
        format(dir).unwrap().storage
    }

    /// Caught by: the setting read case-sensitively by its section (git's keys are not),
    /// a missing key taken for anything but files, or the value compared loosely.
    #[test]
    fn the_storage_is_read_from_the_common_directorys_own_file() {
        let absent = scratch("absent", "[core]\n\trepositoryformatversion = 1\n");
        assert_eq!(
            format(&absent).unwrap(),
            Format {
                version: 1,
                storage: None
            }
        );
        let reftable = scratch(
            "reftable",
            "[core]\n\trepositoryformatversion = 1\n[Extensions]\n\tREFSTORAGE = reftable\n",
        );
        assert_eq!(storage(&reftable).as_deref(), Some("reftable"));
        let files = scratch(
            "files",
            "[core]\n\trepositoryformatversion = 1\n[extensions]\n\trefStorage = files\n",
        );
        assert_eq!(storage(&files).as_deref(), Some(FILES));
        let unversioned = scratch("unversioned", "[extensions]\n\trefStorage = files\n");
        assert_eq!(
            format(&unversioned).unwrap(),
            Format {
                version: 0,
                storage: Some(FILES.to_owned())
            }
        );
        for path in [absent, reftable, files, unversioned] {
            std::fs::remove_dir_all(path).unwrap();
        }
    }

    /// git reads the repository format from the file itself, never through an include, so
    /// an included file's setting is not the repository's. Caught by: reading the
    /// configuration with includes followed.
    #[test]
    fn an_included_file_does_not_set_the_storage() {
        let dir = scratch("included", "");
        std::fs::write(dir.join("extra"), "[extensions]\n\trefStorage = reftable\n").unwrap();
        std::fs::write(
            dir.join("config"),
            format!(
                "[core]\n\trepositoryformatversion = 1\n[include]\n\tpath = {}\n",
                dir.join("extra").display()
            ),
        )
        .unwrap();
        assert_eq!(storage(&dir), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
