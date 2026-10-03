//! `git diff-tree`: which paths two trees differ in, with git's own rename and copy pairs.
//!
//! The changes query's one process (decision E, `docs/design/engine.md`, "Where git answers
//! a read"). It runs `git diff-tree -r -z --raw --no-abbrev`, query plumbing that writes
//! nothing — not the index, which it never reads, and not a ref or an object, since it is
//! given neither `--textconv` nor `--ext-diff` and `--raw` reads no content but what rename
//! detection compares. Detection is passed explicitly, as `-M` or `-C` with `-l<limit>`, or
//! `--no-renames`: plumbing does not read `diff.renames` at all, and the caller decides
//! what the user's configuration asks for (`crate::diff`). So it is with the submodules
//! `diff.ignoreSubmodules` hides, which plumbing does not read either: the caller asks for
//! none of them (`--ignore-submodules=all`) or names those to leave out
//! (`:(exclude,literal)` pathspecs after `--`), so that git never queues them — before
//! rename detection, as `git log` hides them.
//!
//! It reads commits and trees, and — when detection is on and the exact stage leaves
//! something to compare — the blobs it compares. In a blob-less partial clone those blobs
//! may be the promisor's alone: from git 2.44 the read's `GIT_NO_LAZY_FETCH=1` makes `git`
//! fail rather than fetch them, so the query answers [`Error::GitFailed`] and writes
//! nothing, where the user's own `git log` would fetch and show the pairs; older git may
//! fetch (`crate::reads`). Pinned by
//! `in_a_partial_clone_a_rename_search_fails_rather_than_fetching`.
//!
//! The answer is parsed from the `-z` records as they arrive, never from stderr: the
//! rename-limit warning git prints there is prose in the user's language, and whether the
//! limit cut detection short is decided from the answer instead (`crate::diff`).

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt as _;

use cairn_model::{ChangeStatus, ChangedFile, FileMode, Oid, RepoPath, Similarity};

use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// The rename and copy detection a changes query asks `git` for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Detection {
    /// `--no-renames`: every rename is a deletion and an addition.
    Off,
    /// `-M -l<limit>`, where `0` is git's own "no limit" (an unlimited search from git 2.33,
    /// and 32,767 before it).
    Renames { limit: u32 },
    /// `-C -l<limit>`: renames, and copies from files the same change modified.
    Copies { limit: u32 },
}

/// Which gitlinks a changes query asks `git` not to queue, before rename detection sees
/// them: what the user's `diff.ignoreSubmodules` hides from their own `git log`, which
/// `diff-tree` never reads (`crate::diff`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Submodules<'a> {
    /// What `diff-tree` lists: every gitlink but one whose own `submodule.<name>.ignore`
    /// is `all`.
    AsListed,
    /// `--ignore-submodules=all`: no gitlink at all.
    HideEvery,
    /// An `:(exclude,literal)` pathspec for each of these paths.
    Excluding(&'a [RepoPath]),
}

impl Detection {
    fn arguments(self) -> Vec<String> {
        match self {
            Self::Off => vec!["--no-renames".to_owned()],
            Self::Renames { limit } => vec!["-M".to_owned(), format!("-l{limit}")],
            Self::Copies { limit } => vec!["-C".to_owned(), format!("-l{limit}")],
        }
    }
}

/// What `old` and `new` differ in, as git reports it with `detection`, in git's order.
///
/// `old` and `new` are object names `git` resolves to trees — a commit's hex id, or the
/// empty tree's. `cancel` is polled by the runner on every tick while `git` runs: a
/// superseded query ends the process rather than waiting for it, and answers
/// [`Error::ChangesCancelled`]. A failure is classified by `git`'s exit status alone
/// ([`Error::GitFailed`]), and a record this parser does not know is
/// [`Error::UnexpectedGitOutput`]; on either, nothing of the answer is returned.
pub(crate) fn changes(
    git: &GitBinary,
    repo: &Repository,
    old: &Oid,
    new: &Oid,
    detection: Detection,
    submodules: Submodules<'_>,
    cancel: &impl Cancel,
) -> Result<Vec<ChangedFile>, Error> {
    if cancel.is_cancelled() {
        return Err(Error::ChangesCancelled { changed: 0 });
    }
    let arguments = arguments(old, new, detection, submodules);

    let mut records = RawRecords::default();
    let outcome = git
        .read_invocation()
        .in_repository(repo)
        .args(&arguments)
        .start()?
        .records(cancel, |record| records.push(record), |_| {});
    match outcome {
        Ok(_) => records.finish(
            &arguments
                .iter()
                .map(|argument| argument.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
        ),
        Err(Error::GitReadCancelled { .. }) => Err(Error::ChangesCancelled {
            changed: records.files.len(),
        }),
        Err(other) => Err(other),
    }
}

/// The query's arguments: `diff-tree` — query plumbing, which writes nothing — in its raw
/// form and never with `--textconv` or `--ext-diff`, the detection spelled out, the two
/// trees after `--end-of-options`, and any pathspec after `--`.
fn arguments(
    old: &Oid,
    new: &Oid,
    detection: Detection,
    submodules: Submodules<'_>,
) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["diff-tree", "-r", "-z", "--raw", "--no-abbrev"]
        .into_iter()
        .map(OsString::from)
        .collect();
    arguments.extend(detection.arguments().into_iter().map(OsString::from));
    if submodules == Submodules::HideEvery {
        arguments.push("--ignore-submodules=all".into());
    }
    arguments.push("--end-of-options".into());
    arguments.push(old.to_string().into());
    arguments.push(new.to_string().into());
    if let Submodules::Excluding(paths) = submodules
        && !paths.is_empty()
    {
        arguments.push("--".into());
        for path in paths {
            let mut pathspec = b":(exclude,literal)".to_vec();
            pathspec.extend_from_slice(path.as_bytes());
            arguments.push(OsString::from_vec(pathspec));
        }
    }
    arguments
}

/// The `--raw -z` format, record by record: a metadata record
/// (`:<old mode> <new mode> <old id> <new id> <status>`), then one path, or two for a
/// rename or a copy — the source, then the destination.
#[derive(Debug, Default)]
struct RawRecords {
    files: Vec<ChangedFile>,
    pending: Option<Pending>,
    /// The first record that did not parse; everything after it is ignored.
    malformed: Option<String>,
}

#[derive(Debug)]
struct Pending {
    meta: Meta,
    source: Option<RepoPath>,
}

#[derive(Debug, Clone, Copy)]
struct Meta {
    status: ChangeStatus,
    old_mode: Option<FileMode>,
    new_mode: Option<FileMode>,
    old_id: Option<Oid>,
    new_id: Option<Oid>,
}

impl RawRecords {
    fn push(&mut self, record: &[u8]) {
        if self.malformed.is_some() {
            return;
        }
        match self.pending.take() {
            None => match parse_meta(record) {
                Some(meta) => {
                    self.pending = Some(Pending { meta, source: None });
                }
                None => self.malformed = Some(String::from_utf8_lossy(record).into_owned()),
            },
            Some(Pending { meta, source }) => {
                let path = RepoPath::new(record);
                let paired = matches!(
                    meta.status,
                    ChangeStatus::Renamed(_) | ChangeStatus::Copied(_)
                );
                match (paired, source) {
                    (true, None) => {
                        self.pending = Some(Pending {
                            meta,
                            source: Some(path),
                        });
                    }
                    (true, Some(source)) => self.files.push(file(meta, source, path)),
                    (false, _) => self.files.push(file(meta, path.clone(), path)),
                }
            }
        }
    }

    fn finish(self, arguments: &str) -> Result<Vec<ChangedFile>, Error> {
        let unexpected = |record: String| Error::UnexpectedGitOutput {
            arguments: arguments.to_owned(),
            record,
        };
        if let Some(record) = self.malformed {
            return Err(unexpected(record));
        }
        if let Some(pending) = self.pending {
            return Err(unexpected(format!(
                "a {:?} record with its path missing",
                pending.meta.status
            )));
        }
        Ok(self.files)
    }
}

fn file(meta: Meta, old_path: RepoPath, new_path: RepoPath) -> ChangedFile {
    ChangedFile {
        status: meta.status,
        old_path,
        new_path,
        old_mode: meta.old_mode,
        new_mode: meta.new_mode,
        old_id: meta.old_id,
        new_id: meta.new_id,
    }
}

/// `None` for anything but the five fields of a metadata record, each in the form git
/// prints it for two trees.
fn parse_meta(record: &[u8]) -> Option<Meta> {
    let text = std::str::from_utf8(record.strip_prefix(b":")?).ok()?;
    let mut fields = text.split(' ');
    let (old_mode, new_mode, old_id, new_id, status) = (
        fields.next()?,
        fields.next()?,
        fields.next()?,
        fields.next()?,
        fields.next()?,
    );
    if fields.next().is_some() {
        return None;
    }
    let old_mode = mode(old_mode)?;
    let new_mode = mode(new_mode)?;
    let status = status_of(status)?;
    // The side a path is absent from has mode `000000` and the null id: which side that
    // is follows from the status, and anything else is not a record git prints.
    let (old_present, new_present) = match status {
        ChangeStatus::Added => (false, true),
        ChangeStatus::Deleted => (true, false),
        ChangeStatus::Modified
        | ChangeStatus::TypeChanged
        | ChangeStatus::Renamed(_)
        | ChangeStatus::Copied(_) => (true, true),
    };
    if old_mode.is_some() != old_present || new_mode.is_some() != new_present {
        return None;
    }
    Some(Meta {
        status,
        old_mode,
        new_mode,
        old_id: id(old_id, old_present)?,
        new_id: id(new_id, new_present)?,
    })
}

/// `Some(None)` for the absent side's `000000`; `None` for a mode a file never has.
fn mode(digits: &str) -> Option<Option<FileMode>> {
    if digits == "000000" {
        return Some(None);
    }
    FileMode::from_octal(digits).map(Some)
}

/// `Some(None)` for the absent side, whose id must be all zeros.
fn id(hex: &str, present: bool) -> Option<Option<Oid>> {
    if present {
        Oid::parse(hex).ok().map(Some)
    } else if !hex.is_empty() && hex.bytes().all(|byte| byte == b'0') {
        Some(None)
    } else {
        None
    }
}

/// The status letter, with its score for a rename or a copy. `U` and `X` — an unmerged
/// path, an unknown change — cannot come from comparing two trees, and are refused.
fn status_of(status: &str) -> Option<ChangeStatus> {
    let (letter, digits) = status.split_at_checked(1)?;
    let similarity = || -> Option<Similarity> {
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let percent: u8 = digits.parse().ok()?;
        (percent <= 100).then(|| Similarity::from_percent(percent))
    };
    Some(match letter {
        "A" if digits.is_empty() => ChangeStatus::Added,
        "D" if digits.is_empty() => ChangeStatus::Deleted,
        "M" if digits.is_empty() => ChangeStatus::Modified,
        "T" if digits.is_empty() => ChangeStatus::TypeChanged,
        "R" => ChangeStatus::Renamed(similarity()?),
        "C" => ChangeStatus::Copied(similarity()?),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "07da224c7ec04501dfb451be161fa962effe1dc1";
    const NEW: &str = "ebc3711f1349e6204d89c344c178221fb8bf5113";
    const NULL: &str = "0000000000000000000000000000000000000000";

    fn parse(records: &[&[u8]]) -> Result<Vec<ChangedFile>, Error> {
        let mut raw = RawRecords::default();
        for record in records {
            raw.push(record);
        }
        raw.finish("diff-tree")
    }

    fn meta(old_mode: &str, new_mode: &str, old: &str, new: &str, status: &str) -> Vec<u8> {
        format!(":{old_mode} {new_mode} {old} {new} {status}").into_bytes()
    }

    /// Every status two trees can differ by, each with the paths it carries: one for most,
    /// two for a rename or a copy. Caught by: reading a pair's second path as the next
    /// record's metadata, or taking the source for the destination.
    #[test]
    fn every_kind_of_record_becomes_the_file_git_named() {
        let added = meta("000000", "100644", NULL, NEW, "A");
        let deleted = meta("100755", "000000", OLD, NULL, "D");
        let modified = meta("100644", "100755", OLD, NEW, "M");
        let type_changed = meta("100644", "120000", OLD, NEW, "T");
        let renamed = meta("100644", "100644", OLD, NEW, "R086");
        let copied = meta("160000", "160000", OLD, NEW, "C100");
        let files = parse(&[
            &added,
            b"a\tb",
            &deleted,
            b"d",
            &modified,
            b"m",
            &type_changed,
            b"t",
            &renamed,
            b"from",
            b"to",
            &copied,
            b"src",
            b"dst \n",
        ])
        .unwrap();
        assert_eq!(files.len(), 6);

        assert_eq!(files[0].status, ChangeStatus::Added);
        assert_eq!(files[0].new_path.as_bytes(), b"a\tb");
        assert_eq!((files[0].old_mode, files[0].old_id), (None, None));
        assert_eq!(files[0].new_mode, Some(FileMode::Regular));
        assert_eq!(files[0].new_id, Some(Oid::parse(NEW).unwrap()));

        assert_eq!(files[1].status, ChangeStatus::Deleted);
        assert_eq!(files[1].old_mode, Some(FileMode::Executable));
        assert_eq!((files[1].new_mode, files[1].new_id), (None, None));

        assert_eq!(files[2].status, ChangeStatus::Modified);
        assert_eq!(files[3].status, ChangeStatus::TypeChanged);
        assert_eq!(files[3].new_mode, Some(FileMode::Symlink));

        assert_eq!(
            files[4].status,
            ChangeStatus::Renamed(Similarity::from_percent(86))
        );
        assert_eq!(files[4].old_path.as_bytes(), b"from");
        assert_eq!(files[4].new_path.as_bytes(), b"to");

        assert_eq!(
            files[5].status,
            ChangeStatus::Copied(Similarity::from_percent(100))
        );
        assert_eq!(files[5].old_mode, Some(FileMode::Submodule));
        assert_eq!(files[5].old_path.as_bytes(), b"src");
        assert_eq!(
            files[5].new_path.as_bytes(),
            b"dst \n",
            "bytes kept as they are"
        );
    }

    /// Caught by: an answer that is cut off after a metadata record being taken as whole.
    #[test]
    fn a_record_left_without_its_path_is_refused() {
        let renamed = meta("100644", "100644", OLD, NEW, "R086");
        assert!(matches!(
            parse(&[&renamed, b"from"]),
            Err(Error::UnexpectedGitOutput { .. })
        ));
        assert!(matches!(
            parse(&[&renamed]),
            Err(Error::UnexpectedGitOutput { .. })
        ));
    }

    /// Each of these is a record git does not print for two trees; reading any of them as
    /// something else would put a wrong row in front of the user. Caught by: a parser that
    /// guesses.
    #[test]
    fn what_git_does_not_print_for_two_trees_is_refused() {
        let refused: Vec<Vec<u8>> = vec![
            b"not a record".to_vec(),
            meta("100644", "100644", OLD, NEW, "U"),
            meta("100644", "100644", OLD, NEW, "X"),
            meta("100644", "100644", OLD, NEW, "R"),
            meta("100644", "100644", OLD, NEW, "R101"),
            meta("100644", "100644", OLD, NEW, "Rx9"),
            meta("100644", "100644", OLD, NEW, "M050"),
            meta("040000", "040000", OLD, NEW, "M"),
            meta("000000", "100644", OLD, NEW, "A"),
            meta("100644", "100644", NULL, NEW, "A"),
            meta("100644", "000000", OLD, NEW, "D"),
            meta("100644", "100644", "abc", NEW, "M"),
            format!(":100644 100644 {OLD} {NEW} M extra").into_bytes(),
            format!("100644 100644 {OLD} {NEW} M").into_bytes(),
        ];
        for record in &refused {
            assert!(
                matches!(
                    parse(&[record, b"path"]),
                    Err(Error::UnexpectedGitOutput { .. })
                ),
                "accepted {:?}",
                String::from_utf8_lossy(record)
            );
        }
    }

    #[test]
    fn no_records_is_no_change() {
        assert!(parse(&[]).unwrap().is_empty());
    }

    #[test]
    fn detection_is_spelled_the_way_diff_tree_takes_it() {
        assert_eq!(Detection::Off.arguments(), ["--no-renames"]);
        assert_eq!(
            Detection::Renames { limit: 1000 }.arguments(),
            ["-M", "-l1000"]
        );
        assert_eq!(Detection::Copies { limit: 0 }.arguments(), ["-C", "-l0"]);
    }
}
