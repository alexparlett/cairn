//! `git status --porcelain=v2 -z`: the working tree's status, as the user's own `git status`
//! answers it (PRD R3, the packet's L1 and L2).
//!
//! gix's status is not used: it differed from git on 12 of 38 fixtures
//! (`docs/research/refs-and-status/status-agreement-spike.md`) and starts clean filters
//! outside `process/`. So git answers, and everything that decides its answer is the user's:
//! renames and copies as `status.renames`, `status.renameLimit` and `diff.renames` make them
//! (R3.3) — nothing here passes `-M`, `--no-renames` or `--find-renames`; submodules as
//! `submodule.<name>.ignore` and `diff.ignoreSubmodules` report them (R3.6) — nothing passes
//! `--ignore-submodules`; and no `--ignored`, so an ignored file is never listed (R3.5).
//! Porcelain v2 is untranslated and ignores the user's presentation settings by design
//! (`status.relativePaths`, `color.status`, `status.branch` and `status.showStash` change
//! nothing in it: reproduced with 2.30.9, 2.32.7 and 2.56.0), and `-z` prints every path as
//! its bytes, unquoted, each record ended by a NUL — so a path with a newline, a leading space
//! or bytes that are not UTF-8 arrives whole, and nothing here splits on anything but NUL.
//!
//! **Untracked files, one per file unless the user said none (L2).** git's default lists an
//! untracked directory as one `dir/` record; Fork, and Cairn, list its files. But
//! `status.showUntrackedFiles=no` must be honoured, and git itself has to read it: the value
//! is a different language on different gits (git 2.56 takes `false` for `no` and `true` for
//! `normal`, where 2.30.9 refuses both as a bad configuration — reproduced), and gix's reading
//! of a linked worktree's `includeIf` and of the system file is not git's (the reason for
//! `super::fetch_settings`). So the first read passes no `--untracked-files` at all, and git
//! lists untracked paths as the user configured: none under `no`, whereupon that answer is
//! the answer. Only where that answer collapsed a directory — an untracked record ending in
//! `/` — is the status read again with `--untracked-files=all`, and the second answer is the
//! answer, whole; the first is dropped. A directory listed under `all` too is a nested
//! repository, which git lists as its directory in every mode. Two reads of `git status` and
//! nothing else: no `git config` is run, so no porcelain read beyond `status` is added. A
//! change to the setting between the two reads is not seen: the second read lists every
//! untracked file whatever the setting then says, and the next status read sees the change.
//!
//! **What it writes, runs and reads.** A read runs with `GIT_OPTIONAL_LOCKS=0`, which `git
//! status` honours by never writing the index — not the refreshed stat information, not the
//! untracked cache, not the fsmonitor token — and `GIT_NO_LAZY_FETCH=1`. The index is held
//! byte-identical after every read (`a_status_read_leaves_the_index_byte_identical`, and the
//! fixtures of `crates/cairn-git/tests/status.rs`). Residuals, accepted as parity with the
//! user's own `git status` (PRD R3.8): under a split index git advances the mtime of
//! `sharedindex.*`, and under a sparse index the mtime of the loose tree objects it expands
//! (bytes unchanged in both); it runs the repository's `core.fsmonitor` (hook or daemon) and
//! each stat-dirty file's clean filter, as D1 allows a read; and in each submodule it reads,
//! it runs `git status` inside it, with that repository's own hook and filters. Because a
//! read never writes the refreshed stat back, a tree whose every file's stat changed stays
//! as slow to read as the first time (736 ms on rust-lang/rust) until something refreshes the
//! index; nothing here does. Staged rename detection compares blobs of `HEAD` and the index,
//! and in a partial clone a blob only the promisor holds is never fetched by a read from git
//! 2.44 (`GIT_NO_LAZY_FETCH=1`, `crate::reads`): git fails, so the WHOLE read fails —
//! [`Error::GitFailed`], nothing listed, no pack written — where the user's own `git status`
//! would fetch the blob and answer. A git older than 2.44 ignores the variable and fetches,
//! writing a pack: the floor's residual
//! (`in_a_partial_clone_a_status_read_fails_rather_than_fetching`).
//!
//! **Failure.** Classified by exit status and the repository's state, never by stderr. One
//! failure is a state rather than an error (R3.7): a git older than 2.32 cannot read a sparse
//! index (it knows no `sdir` extension, `read-cache.c` before v2.32.0) and exits 128, so a
//! failed read whose index gix finds sparse, on such a git, answers
//! [`WorkingTreeStatus::IndexUnreadable`]. Every other failure is [`Error::GitFailed`]; a
//! record this parser does not know is [`Error::UnexpectedGitOutput`], and nothing of that
//! answer is used. A bare repository has no working tree and runs nothing.

use cairn_model::{
    ChangedEntry, ConflictKind, ConflictedEntry, RepoPath, Similarity, StagedChange, StatusEntry,
    SubmoduleState, UnreadableIndex, UnstagedChange, WorkingTreeStatus,
};

use crate::ops::{GitBinary, GitVersion};
use crate::{Cancel, Error, Repository};

/// The verb and what every status read passes: porcelain v2, NUL-terminated.
const PORCELAIN: [&str; 3] = ["status", "--porcelain=v2", "-z"];

/// Added to the second read, after the first collapsed an untracked directory.
const EVERY_UNTRACKED_FILE: &str = "--untracked-files=all";

/// The first git that reads a sparse index: `read-cache.c` learnt the `sdir` extension in
/// v2.32.0 (it is absent from v2.31.0's).
const READS_A_SPARSE_INDEX: GitVersion = GitVersion {
    major: 2,
    minor: 32,
    patch: 0,
};

/// The working tree's status as `git status` reports it. `cancel` is polled while git runs:
/// a superseded read ends git's process group and answers [`Error::StatusCancelled`], and
/// one superseded before it starts runs nothing.
pub(crate) fn status(
    git: &GitBinary,
    repo: &Repository,
    cancel: &impl Cancel,
) -> Result<WorkingTreeStatus, Error> {
    if repo.workdir().is_none() {
        return Ok(WorkingTreeStatus::NoWorkingTree);
    }
    let first = match read(git, repo, None, cancel)? {
        Read::Listed(entries) => entries,
        Read::Unreadable(why) => return Ok(WorkingTreeStatus::IndexUnreadable(why)),
    };
    if !collapses_a_directory(&first) {
        return Ok(WorkingTreeStatus::Listed(first));
    }
    drop(first);
    match read(git, repo, Some(EVERY_UNTRACKED_FILE), cancel)? {
        Read::Listed(entries) => Ok(WorkingTreeStatus::Listed(entries)),
        Read::Unreadable(why) => Ok(WorkingTreeStatus::IndexUnreadable(why)),
    }
}

/// What one run of `git status` answered.
enum Read {
    Listed(Vec<StatusEntry>),
    Unreadable(UnreadableIndex),
}

/// Whether git listed an untracked directory as one record, which it does for a directory
/// only under its `normal` mode, or for a nested repository in every mode.
fn collapses_a_directory(entries: &[StatusEntry]) -> bool {
    entries.iter().any(|entry| match entry {
        StatusEntry::Untracked(path) => path.as_bytes().ends_with(b"/"),
        StatusEntry::Changed(_) | StatusEntry::Conflicted(_) => false,
    })
}

/// The arguments one read passes after the repository's location.
fn arguments(untracked: Option<&str>) -> Vec<&str> {
    PORCELAIN.iter().copied().chain(untracked).collect()
}

fn read(
    git: &GitBinary,
    repo: &Repository,
    untracked: Option<&str>,
    cancel: &impl Cancel,
) -> Result<Read, Error> {
    if cancel.is_cancelled() {
        return Err(Error::StatusCancelled);
    }
    let arguments = arguments(untracked);
    let mut parser = Parser::default();
    let outcome = git
        .read_invocation()
        .in_repository(repo)
        .args(&arguments)
        .start()?
        .records(cancel, |record| parser.push(record), |_| {});
    match outcome {
        Ok(_) => parser
            .finish()
            .map(Read::Listed)
            .map_err(|record| Error::UnexpectedGitOutput {
                arguments: arguments.join(" "),
                record,
            }),
        Err(Error::GitReadCancelled { .. }) => Err(Error::StatusCancelled),
        Err(failed @ Error::GitFailed { .. }) => {
            if git.version() < READS_A_SPARSE_INDEX && index_is_sparse(repo) {
                Ok(Read::Unreadable(UnreadableIndex::Sparse))
            } else {
                Err(failed)
            }
        }
        Err(other) => Err(other),
    }
}

/// Whether the repository's index is a sparse index, as gix reads it; `false` when it cannot
/// be read at all, which leaves git's own failure to stand.
fn index_is_sparse(repo: &Repository) -> bool {
    repo.inner()
        .open_index()
        .is_ok_and(|index| index.is_sparse())
}

/// Porcelain v2's records, read one NUL-terminated record at a time. The first error is
/// kept and the rest of the answer ignored; [`Parser::finish`] reports it.
#[derive(Debug, Default)]
pub(super) struct Parser {
    entries: Vec<StatusEntry>,
    /// A `2` record read up to its path, waiting for the next record: its source path.
    awaiting_source: Option<Pairing>,
    error: Option<String>,
}

/// A rename or copy record whose source path is the next record.
#[derive(Debug)]
struct Pairing {
    path: RepoPath,
    staged: Code,
    unstaged: Code,
    paired: Paired,
    similarity: Similarity,
    submodule: Option<SubmoduleState>,
}

/// Which side of a `2` record the rename or copy is on, and which it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paired {
    StagedRename,
    StagedCopy,
    UnstagedRename,
    UnstagedCopy,
}

/// One letter of a record's `XY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Code(u8);

impl Parser {
    pub(super) fn push(&mut self, record: &[u8]) {
        if self.error.is_some() {
            return;
        }
        if let Some(pairing) = self.awaiting_source.take() {
            match pairing.finish(record) {
                Ok(entry) => self.entries.push(entry),
                Err(reason) => self.fail(reason, record),
            }
            return;
        }
        match self.record(record) {
            Ok(Some(entry)) => self.entries.push(entry),
            Ok(None) => {}
            Err(reason) => self.fail(reason, record),
        }
    }

    pub(super) fn finish(self) -> Result<Vec<StatusEntry>, String> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if let Some(pairing) = self.awaiting_source {
            return Err(format!(
                "a rename or copy of {} with no source path after it",
                pairing.path
            ));
        }
        Ok(self.entries)
    }

    fn fail(&mut self, reason: String, record: &[u8]) {
        self.error = Some(format!("{reason}: {:?}", String::from_utf8_lossy(record)));
    }

    /// One record that is not a rename's source: an entry, `None` for a header, or why it
    /// is not a record git prints for this query.
    fn record(&mut self, record: &[u8]) -> Result<Option<StatusEntry>, String> {
        match record.split_first() {
            // A header (`# branch.oid ...`, `# stash 2`): only under `--branch` or
            // `--show-stash`, which are not passed; git's documentation tells a parser to
            // skip a header it does not know.
            Some((b'#', _)) => Ok(None),
            Some((b'1', rest)) => ordinary(rest).map(Some),
            Some((b'2', rest)) => {
                self.awaiting_source = Some(paired(rest)?);
                Ok(None)
            }
            Some((b'u', rest)) => unmerged(rest).map(Some),
            Some((b'?', rest)) => untracked(rest).map(Some),
            Some((b'!', _)) => Err("an ignored path, which is never asked for".to_owned()),
            Some(_) | None => Err("not a porcelain v2 record".to_owned()),
        }
    }
}

/// Splits `rest` — a record after its one-letter kind — into `count` space-separated fields,
/// the last being the path, which may hold spaces of its own. The record must start with a
/// space, and every field must be there.
fn fields(rest: &[u8], count: usize) -> Result<Vec<&[u8]>, String> {
    let Some(rest) = rest.strip_prefix(b" ") else {
        return Err("no space after the record's kind".to_owned());
    };
    let fields: Vec<&[u8]> = rest.splitn(count, |byte| *byte == b' ').collect();
    if fields.len() != count {
        return Err(format!(
            "{} fields where {count} were expected",
            fields.len()
        ));
    }
    Ok(fields)
}

/// `1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>`.
fn ordinary(rest: &[u8]) -> Result<StatusEntry, String> {
    let fields = fields(rest, 8)?;
    let (staged, unstaged) = codes(field(&fields, 0)?)?;
    let submodule = submodule(field(&fields, 1)?)?;
    modes(&fields[2..5])?;
    ids(&fields[5..7])?;
    let path = path(field(&fields, 7)?)?;
    Ok(StatusEntry::Changed(ChangedEntry {
        path,
        staged: staged_change(staged)?,
        unstaged: unstaged_change(unstaged)?,
        submodule,
    }))
}

/// `2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path>`, its source the next record.
fn paired(rest: &[u8]) -> Result<Pairing, String> {
    let fields = fields(rest, 9)?;
    let (staged, unstaged) = codes(field(&fields, 0)?)?;
    let submodule = submodule(field(&fields, 1)?)?;
    modes(&fields[2..5])?;
    ids(&fields[5..7])?;
    let (letter, similarity) = score(field(&fields, 7)?)?;
    let path = path(field(&fields, 8)?)?;
    // git keeps one rename per entry (`wt-status.c`: "multiple renames on the same target?
    // how?"), so exactly one side names it, with the score's letter.
    let paired = match (staged.0, unstaged.0) {
        (b'R', b'.' | b'M' | b'D' | b'T') if letter == b'R' => Paired::StagedRename,
        (b'C', b'.' | b'M' | b'D' | b'T') if letter == b'C' => Paired::StagedCopy,
        (b'.', b'R') if letter == b'R' => Paired::UnstagedRename,
        (b'.', b'C') if letter == b'C' => Paired::UnstagedCopy,
        _ => {
            return Err(format!(
                "a rename or copy record whose sides and score disagree ({}{} {})",
                char::from(staged.0),
                char::from(unstaged.0),
                char::from(letter)
            ));
        }
    };
    Ok(Pairing {
        path,
        staged,
        unstaged,
        paired,
        similarity,
        submodule,
    })
}

impl Pairing {
    fn finish(self, source: &[u8]) -> Result<StatusEntry, String> {
        let from = path(source).map_err(|reason| format!("the source path: {reason}"))?;
        let similarity = self.similarity;
        let (staged, unstaged) = match self.paired {
            Paired::StagedRename => (
                Some(StagedChange::Renamed { from, similarity }),
                unstaged_change(self.unstaged)?,
            ),
            Paired::StagedCopy => (
                Some(StagedChange::Copied { from, similarity }),
                unstaged_change(self.unstaged)?,
            ),
            Paired::UnstagedRename => (
                staged_change(self.staged)?,
                Some(UnstagedChange::Renamed { from, similarity }),
            ),
            Paired::UnstagedCopy => (
                staged_change(self.staged)?,
                Some(UnstagedChange::Copied { from, similarity }),
            ),
        };
        Ok(StatusEntry::Changed(ChangedEntry {
            path: self.path,
            staged,
            unstaged,
            submodule: self.submodule,
        }))
    }
}

/// `u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`.
fn unmerged(rest: &[u8]) -> Result<StatusEntry, String> {
    let fields = fields(rest, 10)?;
    let code = field(&fields, 0)?;
    let kind = ConflictKind::from_code(code).ok_or_else(|| {
        format!(
            "{:?} is not one of git's seven unmerged states",
            String::from_utf8_lossy(code)
        )
    })?;
    let submodule = submodule(field(&fields, 1)?)?;
    modes(&fields[2..6])?;
    ids(&fields[6..9])?;
    let path = path(field(&fields, 9)?)?;
    Ok(StatusEntry::Conflicted(ConflictedEntry {
        path,
        kind,
        submodule,
    }))
}

/// `? <path>`.
fn untracked(rest: &[u8]) -> Result<StatusEntry, String> {
    let Some(rest) = rest.strip_prefix(b" ") else {
        return Err("no space after the record's kind".to_owned());
    };
    path(rest).map(StatusEntry::Untracked)
}

fn field<'a>(fields: &[&'a [u8]], at: usize) -> Result<&'a [u8], String> {
    fields
        .get(at)
        .copied()
        .ok_or_else(|| format!("no field {at}"))
}

fn path(bytes: &[u8]) -> Result<RepoPath, String> {
    if bytes.is_empty() {
        Err("an empty path".to_owned())
    } else {
        Ok(RepoPath::new(bytes.to_vec()))
    }
}

/// `XY`: the staged letter and the unstaged one, `.` for no change.
fn codes(field: &[u8]) -> Result<(Code, Code), String> {
    match field {
        [staged, unstaged] => Ok((Code(*staged), Code(*unstaged))),
        _ => Err("a status that is not two letters".to_owned()),
    }
}

/// The staged letter of a `1` record, or of a `2` record's side without the pairing.
fn staged_change(code: Code) -> Result<Option<StagedChange>, String> {
    match code.0 {
        b'.' => Ok(None),
        b'A' => Ok(Some(StagedChange::Added)),
        b'M' => Ok(Some(StagedChange::Modified)),
        b'D' => Ok(Some(StagedChange::Deleted)),
        b'T' => Ok(Some(StagedChange::TypeChanged)),
        other => Err(format!(
            "{:?} is not a staged change git prints here",
            char::from(other)
        )),
    }
}

/// The unstaged letter of a `1` record, or of a `2` record's side without the pairing.
/// `A` is an intent-to-add entry: git's index-to-working-tree diff lists nothing else as
/// added.
fn unstaged_change(code: Code) -> Result<Option<UnstagedChange>, String> {
    match code.0 {
        b'.' => Ok(None),
        b'M' => Ok(Some(UnstagedChange::Modified)),
        b'D' => Ok(Some(UnstagedChange::Deleted)),
        b'T' => Ok(Some(UnstagedChange::TypeChanged)),
        b'A' => Ok(Some(UnstagedChange::IntentToAdd)),
        other => Err(format!(
            "{:?} is not an unstaged change git prints here",
            char::from(other)
        )),
    }
}

/// `<sub>`: `N...` for a path that is no submodule, or `S` and a letter or a `.` for each of
/// new commits, modified content and untracked content.
fn submodule(field: &[u8]) -> Result<Option<SubmoduleState>, String> {
    let flag = |byte: u8, letter: u8| match byte {
        b'.' => Ok(false),
        byte if byte == letter => Ok(true),
        _ => Err(format!(
            "{:?} is not a submodule state",
            String::from_utf8_lossy(field)
        )),
    };
    match field {
        b"N..." => Ok(None),
        [b'S', commits, modified, untracked] => Ok(Some(SubmoduleState {
            new_commits: flag(*commits, b'C')?,
            modified_content: flag(*modified, b'M')?,
            untracked_content: flag(*untracked, b'U')?,
        })),
        _ => Err(format!(
            "{:?} is not a submodule state",
            String::from_utf8_lossy(field)
        )),
    }
}

/// Each mode git prints with `%06o`: six octal digits.
fn modes(fields: &[&[u8]]) -> Result<(), String> {
    for mode in fields {
        if mode.len() != 6 || !mode.iter().all(|byte| (b'0'..=b'7').contains(byte)) {
            return Err(format!("{:?} is not a mode", String::from_utf8_lossy(mode)));
        }
    }
    Ok(())
}

/// Each object id: forty hex digits for SHA-1, sixty-four for SHA-256.
fn ids(fields: &[&[u8]]) -> Result<(), String> {
    for id in fields {
        let hex = id
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte));
        if !(id.len() == 40 || id.len() == 64) || !hex {
            return Err(format!(
                "{:?} is not an object id",
                String::from_utf8_lossy(id)
            ));
        }
    }
    Ok(())
}

/// `<X><score>`: `R` or `C` and a percentage, `%d` without padding.
fn score(field: &[u8]) -> Result<(u8, Similarity), String> {
    let refused = || {
        format!(
            "{:?} is not a rename or copy score",
            String::from_utf8_lossy(field)
        )
    };
    let Some((&letter, digits)) = field.split_first() else {
        return Err(refused());
    };
    if !matches!(letter, b'R' | b'C')
        || digits.is_empty()
        || digits.len() > 3
        || !digits.iter().all(u8::is_ascii_digit)
    {
        return Err(refused());
    }
    let percent = digits
        .iter()
        .fold(0u16, |value, digit| value * 10 + u16::from(digit - b'0'));
    let percent = u8::try_from(percent)
        .ok()
        .filter(|percent| *percent <= 100)
        .ok_or_else(refused)?;
    Ok((letter, Similarity::from_percent(percent)))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::CancelSignal;
    use crate::ops::{Askpass, GitEnvironment};
    use crate::process::stub_git::{StubGit, discover_retrying, printed_environment};

    const ZERO: &str = "0000000000000000000000000000000000000000";
    const BLOB: &str = "78981922613b2afb6025042ff6bd878ac1994e85";

    /// Parses `records` as one answer, each record the bytes between two NULs.
    fn parsed(records: &[&[u8]]) -> Result<Vec<StatusEntry>, String> {
        let mut parser = Parser::default();
        for record in records {
            parser.push(record);
        }
        parser.finish()
    }

    fn changed(
        path: &[u8],
        staged: Option<StagedChange>,
        unstaged: Option<UnstagedChange>,
    ) -> StatusEntry {
        StatusEntry::Changed(ChangedEntry {
            path: RepoPath::new(path.to_vec()),
            staged,
            unstaged,
            submodule: None,
        })
    }

    fn one(code: &str, path: &[u8]) -> Vec<u8> {
        let mut record = format!("1 {code} N... 100644 100644 100644 {BLOB} {BLOB} ").into_bytes();
        record.extend_from_slice(path);
        record
    }

    /// Every letter of a `1` record's two sides reaches its own variant, and `.` is no
    /// change. Caught by: two letters read as one (`T` as `M`), `A` on the unstaged side
    /// read as anything but intent-to-add, or a side's `.` read as a change.
    #[test]
    fn each_letter_of_an_ordinary_record_is_its_own_change() {
        let cases: [(&str, Option<StagedChange>, Option<UnstagedChange>); 9] = [
            (".M", None, Some(UnstagedChange::Modified)),
            (".D", None, Some(UnstagedChange::Deleted)),
            (".T", None, Some(UnstagedChange::TypeChanged)),
            (".A", None, Some(UnstagedChange::IntentToAdd)),
            ("A.", Some(StagedChange::Added), None),
            ("M.", Some(StagedChange::Modified), None),
            ("D.", Some(StagedChange::Deleted), None),
            ("T.", Some(StagedChange::TypeChanged), None),
            (
                "MD",
                Some(StagedChange::Modified),
                Some(UnstagedChange::Deleted),
            ),
        ];
        for (code, staged, unstaged) in cases {
            assert_eq!(
                parsed(&[&one(code, b"a.txt")]),
                Ok(vec![changed(b"a.txt", staged, unstaged)]),
                "{code}"
            );
        }
        for refused in ["R.", ".R", "C.", "U.", "?.", "!.", "..M", "M", ".X", "a."] {
            assert!(
                parsed(&[&one(refused, b"a.txt")]).is_err(),
                "{refused} was read as a change"
            );
        }
    }

    /// A path is everything after the eighth field, spaces and all, and whatever bytes it
    /// holds — a newline, a leading space, bytes that are not UTF-8 — since `-z` quotes
    /// nothing. Caught by: a path split on a space or a newline, trimmed, or decoded.
    #[test]
    fn a_path_is_its_bytes_whatever_they_hold() {
        for path in [
            &b"with space.txt"[..],
            b" leading space",
            b"new\nline",
            b"tab\tname",
            b"bad\xffname",
            b"-dash",
            b"back\\slash \"quoted\"",
            b"dir with space/x",
        ] {
            assert_eq!(
                parsed(&[&one(".M", path)]),
                Ok(vec![changed(path, None, Some(UnstagedChange::Modified))]),
                "{path:?}"
            );
            let mut record = b"? ".to_vec();
            record.extend_from_slice(path);
            assert_eq!(
                parsed(&[&record]),
                Ok(vec![StatusEntry::Untracked(RepoPath::new(path.to_vec()))]),
                "{path:?}"
            );
        }
    }

    /// A `2` record's source is the next record, and the rename or copy belongs to the side
    /// whose letter names it, with the score's percentage. Caught by: the source dropped or
    /// taken for a record of its own, the pair put on the wrong side, a source with a space
    /// cut, or the score lost.
    #[test]
    fn a_rename_or_copy_takes_the_next_record_as_its_source_on_its_own_side() {
        let two = |code: &str, score: &str, path: &[u8]| {
            let mut record =
                format!("2 {code} N... 100644 100644 100644 {BLOB} {BLOB} {score} ").into_bytes();
            record.extend_from_slice(path);
            record
        };
        let similarity = Similarity::from_percent;
        let from = || RepoPath::new(b"old name\nwith newline".to_vec());
        assert_eq!(
            parsed(&[
                &two("RM", "R86", b"new name"),
                b"old name\nwith newline",
                &one(".M", b"after"),
            ]),
            Ok(vec![
                changed(
                    b"new name",
                    Some(StagedChange::Renamed {
                        from: from(),
                        similarity: similarity(86),
                    }),
                    Some(UnstagedChange::Modified),
                ),
                changed(b"after", None, Some(UnstagedChange::Modified)),
            ])
        );
        assert_eq!(
            parsed(&[&two("C.", "C100", b"copy"), b"old name\nwith newline"]),
            Ok(vec![changed(
                b"copy",
                Some(StagedChange::Copied {
                    from: from(),
                    similarity: similarity(100),
                }),
                None,
            )])
        );
        assert_eq!(
            parsed(&[&two(".R", "R0", b"y"), b"old name\nwith newline"]),
            Ok(vec![changed(
                b"y",
                None,
                Some(UnstagedChange::Renamed {
                    from: from(),
                    similarity: similarity(0),
                }),
            )])
        );
        assert_eq!(
            parsed(&[&two(".C", "C75", b"y"), b"old name\nwith newline"]),
            Ok(vec![changed(
                b"y",
                None,
                Some(UnstagedChange::Copied {
                    from: from(),
                    similarity: similarity(75),
                }),
            )])
        );
        for (code, score) in [
            ("RR", "R100"),
            ("R.", "C100"),
            (".C", "R100"),
            ("M.", "R100"),
            ("R.", "R101"),
            ("R.", "R"),
            ("R.", "100"),
            ("R.", "R1000"),
            ("R.", "R-1"),
        ] {
            assert!(
                parsed(&[&two(code, score, b"x"), b"y"]).is_err(),
                "{code} {score} was read"
            );
        }
        assert!(
            parsed(&[&two("R.", "R100", b"x")]).is_err(),
            "a rename with no source after it was read"
        );
        assert!(
            parsed(&[&two("R.", "R100", b"x"), b""]).is_err(),
            "an empty source was read"
        );
    }

    /// Each of git's seven unmerged codes reaches its own kind, never one "conflicted".
    /// Caught by: a code mapped to a neighbour's kind, or an unknown code accepted.
    #[test]
    fn each_unmerged_code_is_its_own_kind() {
        for kind in ConflictKind::ALL {
            let mut record = b"u ".to_vec();
            record.extend_from_slice(kind.code());
            record.extend_from_slice(
                format!(" N... 100644 100644 100644 100644 {BLOB} {BLOB} {BLOB} c d").as_bytes(),
            );
            assert_eq!(
                parsed(&[&record]),
                Ok(vec![StatusEntry::Conflicted(ConflictedEntry {
                    path: RepoPath::from("c d"),
                    kind,
                    submodule: None,
                })])
            );
        }
        let unknown =
            format!("u MM N... 100644 100644 100644 100644 {BLOB} {BLOB} {BLOB} c").into_bytes();
        assert!(parsed(&[&unknown]).is_err());
    }

    /// The submodule field: `N...` is no submodule, and each of `S`'s three letters is its
    /// own flag, on an ordinary, a rename's and an unmerged record alike. Caught by: a flag
    /// read from the wrong position, `S...` read as none, a letter in the wrong place
    /// accepted, or the field dropped from a `2` or `u` record.
    #[test]
    fn each_submodule_letter_is_its_own_flag() {
        let state = |new_commits, modified_content, untracked_content| SubmoduleState {
            new_commits,
            modified_content,
            untracked_content,
        };
        for (field, expected) in [
            ("N...", None),
            ("S...", Some(state(false, false, false))),
            ("SC..", Some(state(true, false, false))),
            ("S.M.", Some(state(false, true, false))),
            ("S..U", Some(state(false, false, true))),
            ("SCMU", Some(state(true, true, true))),
        ] {
            let record =
                format!("1 .M {field} 160000 160000 160000 {BLOB} {BLOB} sub").into_bytes();
            assert_eq!(
                parsed(&[&record]),
                Ok(vec![StatusEntry::Changed(ChangedEntry {
                    path: RepoPath::from("sub"),
                    staged: None,
                    unstaged: Some(UnstagedChange::Modified),
                    submodule: expected,
                })]),
                "{field}"
            );
        }
        for refused in ["N..U", "SM..", "S.U.", "S...C", "S..", "X...", "s..."] {
            let record =
                format!("1 .M {refused} 160000 160000 160000 {BLOB} {BLOB} sub").into_bytes();
            assert!(parsed(&[&record]).is_err(), "{refused} was read");
        }

        // The same field on a rename's record and on an unmerged one reaches its entry.
        let renamed =
            format!("2 R. SC.U 160000 160000 160000 {BLOB} {BLOB} R100 new sub").into_bytes();
        assert_eq!(
            parsed(&[&renamed, b"old sub"]),
            Ok(vec![StatusEntry::Changed(ChangedEntry {
                path: RepoPath::from("new sub"),
                staged: Some(StagedChange::Renamed {
                    from: RepoPath::from("old sub"),
                    similarity: Similarity::from_percent(100),
                }),
                unstaged: None,
                submodule: Some(state(true, false, true)),
            })])
        );
        let unmerged =
            format!("u UU S.M. 160000 160000 160000 160000 {BLOB} {BLOB} {BLOB} conflicted sub")
                .into_bytes();
        assert_eq!(
            parsed(&[&unmerged]),
            Ok(vec![StatusEntry::Conflicted(ConflictedEntry {
                path: RepoPath::from("conflicted sub"),
                kind: ConflictKind::BothModified,
                submodule: Some(state(false, true, false)),
            })])
        );
    }

    /// What is not a record git prints for this query is refused, never guessed at: an
    /// ignored path (never asked for), an unknown kind, a field missing, a bad mode or id,
    /// an empty path. A header is skipped. Caught by: any of them read as an entry.
    #[test]
    fn what_git_does_not_print_here_is_refused_and_a_header_skipped() {
        assert_eq!(
            parsed(&[b"# branch.oid (initial)", b"# stash 2"]),
            Ok(vec![])
        );
        let sha256 = "a".repeat(64);
        assert!(
            parsed(&[format!("1 .M N... 100644 100644 100644 {sha256} {sha256} a").as_bytes()])
                .is_ok(),
            "a SHA-256 id was refused"
        );
        for refused in [
            "! ignored".to_owned(),
            "x what".to_owned(),
            "?".to_owned(),
            "? ".to_owned(),
            "?a".to_owned(),
            String::new(),
            format!("1 .M N... 100644 100644 100644 {BLOB} {BLOB}"),
            format!("1 .M N... 100644 100644 100644 {BLOB} {BLOB} "),
            format!("1 .M N... 100648 100644 100644 {BLOB} {BLOB} a"),
            format!("1 .M N... 10064 100644 100644 {BLOB} {BLOB} a"),
            format!("1 .M N... 100644 100644 100644 {ZERO}0 {BLOB} a"),
            format!("1 .M N... 100644 100644 100644 {} {BLOB} a", "G".repeat(40)),
            format!("1  .M N... 100644 100644 100644 {BLOB} {BLOB} a"),
            format!("1.M N... 100644 100644 100644 {BLOB} {BLOB} a"),
        ] {
            assert!(
                parsed(&[refused.as_bytes()]).is_err(),
                "{refused:?} was read"
            );
        }
        // The first refusal stands; nothing after it is read.
        assert!(parsed(&[b"! ignored", b"? fine"]).is_err());
    }

    /// The reads `git status` runs, spelled out: the first with no `--untracked-files`, so
    /// git reads `status.showUntrackedFiles` itself; the second, only after the first
    /// collapsed a directory, with `--untracked-files=all` and nothing else added. No
    /// `--ignored`, rename or submodule option, ever. A stub `git` prints what it was given
    /// into a file beside it, and answers `? dir/` to the first read.
    #[test]
    fn the_reads_are_status_porcelain_v2_and_all_untracked_only_after_a_collapse() {
        let stub = StubGit::with_git_from(|directory| {
            let log = directory.join("argv").display().to_string();
            format!(
                "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
                 printf '%s\\n' \"$@\" >> '{log}'\n\
                 echo -- >> '{log}'\n\
                 case \"$*\" in\n\
                 *--untracked-files=all*) printf '? dir/file\\0' ;;\n\
                 *) printf '? dir/\\0' ;;\n\
                 esac"
            )
        });
        let git = discover_retrying(stub.environment()).unwrap_or_else(|e| panic!("{e}"));
        let repo =
            Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap_or_else(|e| panic!("{e}"));
        let answer = status(&git, &repo, &CancelSignal::new()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            answer,
            WorkingTreeStatus::Listed(vec![StatusEntry::Untracked(RepoPath::from("dir/file"))]),
            "the second answer is the answer"
        );
        let argv = std::fs::read_to_string(stub.directory().join("argv"))
            .unwrap_or_else(|e| panic!("{e}"));
        let canonical = |path: &Path| {
            std::fs::canonicalize(path)
                .unwrap_or_else(|e| panic!("{e}"))
                .display()
                .to_string()
        };
        let workdir = repo.workdir().unwrap_or_else(|| panic!("a working tree"));
        let location = [
            format!("--git-dir={}", canonical(repo.git_dir())),
            format!("--work-tree={}", canonical(workdir)),
        ];
        let reads: Vec<Vec<String>> = argv
            .split("--\n")
            .filter(|read| !read.is_empty())
            .map(|read| {
                read.lines()
                    .map(|line| match line.split_once('=') {
                        Some((option, path))
                            if option == "--git-dir" || option == "--work-tree" =>
                        {
                            format!("{option}={}", canonical(Path::new(path)))
                        }
                        _ => line.to_owned(),
                    })
                    .collect()
            })
            .collect();
        let expected = |extra: &[&str]| -> Vec<String> {
            location
                .iter()
                .cloned()
                .chain(
                    ["status", "--porcelain=v2", "-z"]
                        .iter()
                        .chain(extra)
                        .map(|word| (*word).to_owned()),
                )
                .collect()
        };
        assert_eq!(reads, [expected(&[]), expected(&["--untracked-files=all"])]);
    }

    /// One read and no second where nothing collapsed: an untracked file, a change, a clean
    /// tree. Caught by: the second read run every time (twice the cost, and git's
    /// `status.showUntrackedFiles=no` overridden by `all`).
    #[test]
    fn a_first_answer_with_no_collapsed_directory_is_the_answer() {
        for answer in [
            "? file\\0",
            "1 .M N... 100644 100644 100644 {B} {B} dir/\\0",
            "",
        ] {
            let answer = answer.replace("{B}", BLOB);
            let stub = StubGit::with_git_from(|directory| {
                let log = directory.join("argv").display().to_string();
                format!(
                    "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
                     echo read >> '{log}'\n\
                     printf '{answer}'"
                )
            });
            let git = discover_retrying(stub.environment()).unwrap_or_else(|e| panic!("{e}"));
            let repo =
                Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap_or_else(|e| panic!("{e}"));
            assert!(matches!(
                status(&git, &repo, &CancelSignal::new()),
                Ok(WorkingTreeStatus::Listed(_))
            ));
            let reads = std::fs::read_to_string(stub.directory().join("argv"))
                .unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(reads, "read\n", "after {answer:?}");
        }
    }

    /// The read runs with a read's environment, spelled out: optional locks off, no lazy
    /// fetch, no askpass token. Caught by: status built as a write, which would refresh and
    /// rewrite the index it reads.
    #[test]
    fn a_status_read_runs_with_the_read_environment() {
        let stub = StubGit::with_git_from(|directory| {
            let log = directory.join("env").display().to_string();
            format!(
                "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
                 /usr/bin/env > '{log}'"
            )
        });
        let environment = stub.environment();
        let path = environment
            .get("PATH")
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| panic!("a PATH"));
        let git = discover_retrying(environment).unwrap_or_else(|e| panic!("{e}"));
        let repo =
            Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap_or_else(|e| panic!("{e}"));
        status(&git, &repo, &CancelSignal::new()).unwrap_or_else(|e| panic!("{e}"));
        let printed = printed_environment(
            &std::fs::read_to_string(stub.directory().join("env"))
                .unwrap_or_else(|e| panic!("{e}")),
        );
        let expected: std::collections::BTreeMap<String, String> = [
            ("GIT_ASKPASS", StubGit::HELPER),
            ("GIT_EDITOR", "false"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_SEQUENCE_EDITOR", "false"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("PATH", path.as_str()),
            ("SSH_ASKPASS", StubGit::HELPER),
            ("SSH_ASKPASS_REQUIRE", "force"),
        ]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
        assert_eq!(printed, expected);
    }

    // ── real git ─────────────────────────────────────────────────────────────

    /// A repository built by the `git` Cairn itself would find, with a home holding no
    /// configuration.
    struct Fixture {
        root: PathBuf,
        program: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "cairn-status-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("home")).unwrap_or_else(|e| panic!("{e}"));
            std::fs::create_dir_all(root.join("repo")).unwrap_or_else(|e| panic!("{e}"));
            let program = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
                .unwrap_or_else(|e| panic!("{e}"))
                .path()
                .to_owned();
            let fixture = Self { root, program };
            fixture.git(&["init", "-q", "."]);
            fixture
        }

        fn repo(&self) -> PathBuf {
            self.root.join("repo")
        }

        fn home(&self) -> OsString {
            self.root.join("home").into_os_string()
        }

        fn git(&self, args: &[&str]) {
            self.git_with(&self.program.clone(), args);
        }

        fn git_with(&self, program: &Path, args: &[&str]) {
            let output = std::process::Command::new(program)
                .current_dir(self.repo())
                .args(args)
                .env("HOME", self.home())
                .env("XDG_CONFIG_HOME", self.home())
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
        }

        fn put(&self, path: &str, content: &str) {
            let path = self.repo().join(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{e}"));
            }
            std::fs::write(path, content).unwrap_or_else(|e| panic!("{e}"));
        }

        fn binary(&self) -> GitBinary {
            let home = self.home();
            GitBinary::discover_with(GitEnvironment::new(
                |name| match name {
                    "PATH" => std::env::var_os("PATH"),
                    "HOME" | "XDG_CONFIG_HOME" => Some(home.clone()),
                    _ => None,
                },
                &Askpass::new("/nonexistent/cairn-askpass", None),
            ))
            .unwrap_or_else(|e| panic!("{e}"))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// An epoch, as the worker keeps one: superseded once the counter moves.
    struct Epoch {
        current: Arc<AtomicU64>,
        started_under: u64,
    }

    impl Cancel for Epoch {
        fn is_cancelled(&self) -> bool {
            self.current.load(Ordering::Acquire) != self.started_under
        }
    }

    fn eventually(deadline: Duration, mut condition: impl FnMut() -> bool) -> bool {
        let started = Instant::now();
        while started.elapsed() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        condition()
    }

    /// C5: a status read superseded while git is still running ends its process group —
    /// git and what it started, here a `core.fsmonitor` hook that would sleep for a minute —
    /// rather than waiting for it: the read answers cancelled within a second, the
    /// repository's registry holds no running invocation the moment it returns, the command
    /// log says it was ended, and the hook's own process is gone. Caught by: a cancel that
    /// waits for git, a cancel that ends git but not its group, or a read left registered.
    #[test]
    fn a_superseded_status_read_ends_its_process_group_and_leaves_nothing_running() {
        let fixture = Fixture::new("superseded");
        fixture.put("a.txt", "a\n");
        fixture.git(&["add", "."]);
        fixture.git(&["commit", "-q", "-m", "one"]);
        let pid_file = fixture.root.join("hook.pid");
        let hook = fixture.root.join("slow-fsmonitor");
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\necho $$ > '{}'\nexec sleep 60\n",
                pid_file.display()
            ),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|e| panic!("{e}"));
        }
        fixture.git(&["config", "core.fsmonitor", &hook.display().to_string()]);

        let repo = Repository::discover(fixture.repo()).unwrap_or_else(|e| panic!("{e}"));
        let git = fixture.binary();
        let epochs = Arc::new(AtomicU64::new(0));
        let query = Epoch {
            current: Arc::clone(&epochs),
            started_under: 0,
        };
        // Superseded once the hook is running, so the cancel lands inside git's read.
        let superseding = {
            let epochs = Arc::clone(&epochs);
            let pid_file = pid_file.clone();
            std::thread::spawn(move || {
                let started = eventually(Duration::from_secs(10), || pid_file.exists());
                std::thread::sleep(Duration::from_millis(50));
                epochs.fetch_add(1, Ordering::Release);
                started
            })
        };
        let started = Instant::now();
        let outcome = status(&git, &repo, &query);
        let elapsed = started.elapsed();
        let running_after = repo.processes().running();
        let hook_ran = superseding
            .join()
            .unwrap_or_else(|_| panic!("the superseding thread panicked"));

        assert!(
            hook_ran,
            "git never ran the fsmonitor hook, so nothing was slow"
        );
        assert!(
            matches!(outcome, Err(Error::StatusCancelled)),
            "expected a cancelled read, got {outcome:?}"
        );
        assert!(elapsed < Duration::from_secs(5), "took {elapsed:?}");
        assert_eq!(running_after, 0, "the registry still holds the read");
        let log = repo.processes().log();
        assert_eq!(log.len(), 1, "{log:?}");
        assert!(log[0].cancelled, "git finished before the cancel: {log:?}");
        let pid = std::fs::read_to_string(&pid_file).unwrap_or_else(|e| panic!("{e}"));
        let process = PathBuf::from(format!("/proc/{}", pid.trim()));
        let ended = eventually(Duration::from_secs(5), || {
            // Gone, or a zombie its new parent has yet to reap: either way, not sleeping on.
            std::fs::read_to_string(process.join("stat"))
                .map(|stat| {
                    stat.rsplit(')')
                        .next()
                        .is_some_and(|rest| rest.trim_start().starts_with('Z'))
                })
                .unwrap_or(true)
        });
        assert!(ended, "the hook git started outlived the cancel");
    }

    /// The first `git` on `PATH` that writes a sparse index (2.32 or later); under
    /// `scripts/git-floor.sh`, the machine's git behind the floor's.
    fn a_git_that_writes_a_sparse_index() -> PathBuf {
        let path = std::env::var_os("PATH").unwrap_or_default();
        for dir in std::env::split_paths(&path) {
            let program = dir.join("git");
            let Ok(output) = std::process::Command::new(&program)
                .arg("--version")
                .output()
            else {
                continue;
            };
            let version = GitVersion::parse(&String::from_utf8_lossy(&output.stdout));
            if version.is_some_and(|version| version >= READS_A_SPARSE_INDEX) {
                return program;
            }
        }
        panic!("no git on PATH writes a sparse index (2.32 or later)")
    }

    /// A failed read answers R3.7's state only where both hold: the index is sparse, and
    /// the git is older than 2.32. A stub `git` that fails every status, reporting the
    /// version it is told to, over a real sparse index and over a plain one. Caught by: the
    /// version left out (a 2.40 that failed for any reason in a sparse checkout would be
    /// said not to read its index), or the index left out (any failure of an old git would
    /// be said to be a sparse index).
    #[test]
    fn a_failure_is_the_unreadable_state_only_on_an_old_git_over_a_sparse_index() {
        let sparse = Fixture::new("sparse-state");
        for dir in ["in", "out"] {
            sparse.put(&format!("{dir}/x.txt"), "x\n");
        }
        sparse.git(&["add", "."]);
        sparse.git(&["commit", "-q", "-m", "one"]);
        let builder = a_git_that_writes_a_sparse_index();
        sparse.git_with(
            &builder,
            &["sparse-checkout", "init", "--cone", "--sparse-index"],
        );
        sparse.git_with(&builder, &["sparse-checkout", "set", "in"]);
        let plain = Fixture::new("plain-state");
        plain.put("x.txt", "x\n");
        plain.git(&["add", "."]);
        plain.git(&["commit", "-q", "-m", "one"]);

        let failing = |version: &str| {
            let stub = StubGit::with_git(&format!(
                "if [ \"$1\" = --version ]; then echo 'git version {version}'; exit 0; fi\n\
                 echo 'fatal: whatever git says' >&2; exit 128"
            ));
            let git = discover_retrying(stub.environment()).unwrap_or_else(|e| panic!("{e}"));
            (stub, git)
        };
        let read = |fixture: &Fixture, git: &GitBinary| {
            let repo = Repository::discover(fixture.repo()).unwrap_or_else(|e| panic!("{e}"));
            status(git, &repo, &CancelSignal::new())
        };
        let (_old_stub, old) = failing("2.31.8");
        let (_new_stub, new) = failing("2.32.0");
        assert_eq!(
            read(&sparse, &old).ok(),
            Some(WorkingTreeStatus::IndexUnreadable(UnreadableIndex::Sparse))
        );
        assert!(
            matches!(read(&sparse, &new), Err(Error::GitFailed { .. })),
            "a git that reads a sparse index failed, and was said not to read it"
        );
        assert!(
            matches!(read(&plain, &old), Err(Error::GitFailed { .. })),
            "an old git's failure over a plain index was said to be a sparse index"
        );
    }

    /// The second read — `--untracked-files=all`, after the first collapsed a directory — is
    /// cancelled as the first is: superseded while it runs, its process group is ended, the
    /// read answers cancelled, nothing is left registered, and the log holds both reads with
    /// the second ended. A stub `git` answers `? dir/` to the first and sleeps through the
    /// second. Caught by: the second read given a cancel that never fires.
    #[test]
    fn a_status_read_superseded_during_its_second_read_ends_it() {
        // The stub's `PATH` is its own directory, so `sleep` is named by where it is.
        let sleep = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|dir| dir.join("sleep"))
            .find(|path| path.is_file())
            .unwrap_or_else(|| panic!("no sleep on PATH"));
        let sleep = sleep.display();
        let stub = StubGit::with_git_from(|directory| {
            let started = directory.join("second-started").display().to_string();
            format!(
                "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
                 case \"$*\" in\n\
                 *--untracked-files=all*) : > '{started}'; exec '{sleep}' 30 ;;\n\
                 *) printf '? dir/\\0' ;;\n\
                 esac"
            )
        });
        let git = discover_retrying(stub.environment()).unwrap_or_else(|e| panic!("{e}"));
        let repo =
            Repository::discover(env!("CARGO_MANIFEST_DIR")).unwrap_or_else(|e| panic!("{e}"));
        let epochs = Arc::new(AtomicU64::new(0));
        let query = Epoch {
            current: Arc::clone(&epochs),
            started_under: 0,
        };
        let superseding = {
            let epochs = Arc::clone(&epochs);
            let started = stub.directory().join("second-started");
            std::thread::spawn(move || {
                let reached = eventually(Duration::from_secs(10), || started.exists());
                epochs.fetch_add(1, Ordering::Release);
                reached
            })
        };
        let begun = Instant::now();
        let outcome = status(&git, &repo, &query);
        let elapsed = begun.elapsed();
        let running_after = repo.processes().running();
        let reached = superseding
            .join()
            .unwrap_or_else(|_| panic!("the superseding thread panicked"));
        assert!(reached, "the second read never started");
        assert!(
            matches!(outcome, Err(Error::StatusCancelled)),
            "expected a cancelled read, got {outcome:?}"
        );
        assert!(elapsed < Duration::from_secs(5), "took {elapsed:?}");
        assert_eq!(running_after, 0, "the registry still holds the second read");
        let log = repo.processes().log();
        assert_eq!(log.len(), 2, "{log:?}");
        assert!(!log[0].cancelled, "the first read was ended: {log:?}");
        assert!(log[1].cancelled, "the second read finished: {log:?}");
    }

    /// A read superseded before it starts runs no `git` at all.
    #[test]
    fn a_status_read_superseded_before_it_starts_runs_nothing() {
        let fixture = Fixture::new("before");
        let repo = Repository::discover(fixture.repo()).unwrap_or_else(|e| panic!("{e}"));
        let cancel = CancelSignal::new();
        cancel.cancel();
        assert!(matches!(
            status(&fixture.binary(), &repo, &cancel),
            Err(Error::StatusCancelled)
        ));
        assert!(repo.processes().log().is_empty(), "a process was started");
    }

    /// The index is byte-identical after a read of a dirty tree whose index is stale —
    /// status re-hashes the stale entry, finds it clean, and as a write would store its stat
    /// — and nothing is left locked. Caught by: the read built so that git may write.
    #[test]
    fn a_status_read_leaves_the_index_byte_identical() {
        let fixture = Fixture::new("index");
        fixture.put("stale", "same\n");
        fixture.put("edited", "a\n");
        fixture.git(&["add", "."]);
        fixture.git(&["commit", "-q", "-m", "one"]);
        // The owner may set a file's times through a descriptor opened only to read it.
        std::fs::File::open(fixture.repo().join("stale"))
            .and_then(|file| {
                file.set_modified(std::time::SystemTime::now() - Duration::from_secs(3600))
            })
            .unwrap_or_else(|e| panic!("{e}"));
        fixture.put("edited", "b\n");
        fixture.put("untracked/deep/file", "c\n");
        let index = fixture.repo().join(".git/index");
        let before = std::fs::read(&index).unwrap_or_else(|e| panic!("{e}"));
        let repo = Repository::discover(fixture.repo()).unwrap_or_else(|e| panic!("{e}"));
        let answer = status(&fixture.binary(), &repo, &CancelSignal::new())
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            answer,
            WorkingTreeStatus::Listed(vec![
                changed(b"edited", None, Some(UnstagedChange::Modified)),
                StatusEntry::Untracked(RepoPath::from("untracked/deep/file")),
            ])
        );
        assert_eq!(
            repo.processes().log().len(),
            2,
            "the collapsed directory was read again"
        );
        assert!(
            std::fs::read(&index).unwrap_or_else(|e| panic!("{e}")) == before,
            "a status read rewrote the index"
        );
        assert!(!fixture.repo().join(".git/index.lock").exists());
        // Decisive only if git, free to, would have written: the same status run outside
        // Cairn, with locks allowed, refreshes the stale entry and rewrites the index.
        fixture.git(&["status", "--porcelain"]);
        assert!(
            std::fs::read(&index).unwrap_or_else(|e| panic!("{e}")) != before,
            "git left the index alone with locks allowed too, so the read decided nothing"
        );
    }
}
