//! `git diff-tree -p`: which lines of a file changed, and the function context of each hunk,
//! as git computes them.
//!
//! The content query's ranges are git's own (the content-parity decision of 2026-10-03,
//! `docs/research/diff-engine/content-parity-spike.md`): gix reads both versions of a file,
//! and this read asks git which of their lines changed. It runs
//!
//! ```text
//! git [--literal-pathspecs] -c diff.suppressBlankEmpty=false diff-tree -r -z --raw
//!     --no-abbrev -p -U<n> --no-ext-diff --no-textconv --no-color [-a] [-w]
//!     [--diff-algorithm=<algorithm>] <detection> --end-of-options <old> <new> [-- <paths>]
//! ```
//!
//! — query plumbing that writes nothing: never `--textconv` (which, with a patch and
//! `diff.<driver>.cachetextconv`, writes a notes ref) nor `--ext-diff` (which runs the
//! user's program), and `diff-tree` reads no index and refreshes none.
//!
//! **Why each part is there.**
//!
//! - `-U<n>`, with `n` at least one. At context zero git trims the tail the two sides share
//!   before it diffs (`trim_common_tail` in `xdiff-interface.c`), which changes the script
//!   it finds; at any context of one or more the script is the one `git diff -U3` shows.
//!   `n` is the context the view groups at, so the function context in git's headers is the
//!   text git prints for those very hunks.
//! - Each change is read as a maximal run of `-` and `+` lines, never from the `@@`
//!   headers, which group changes at the context and so cannot say where one ends.
//! - `--diff-algorithm`: plumbing never reads `diff.algorithm`, so the caller passes the
//!   user's, or none when the path's diff driver names its own (`crate::diff`).
//!   `diff.indentHeuristic` plumbing does read, so nothing is passed for it.
//! - `-c diff.suppressBlankEmpty=false`: plumbing reads that key, and with it set a blank
//!   context line loses its leading space, which would read as a line of no kind.
//! - `--literal-pathspecs` (a global option, before the verb) for a file's own paths, so a
//!   path holding `*` or `?` names that path alone. Not for the whole-comparison form, whose
//!   only pathspecs are the `:(exclude,literal)` magic the changes query's submodule rule
//!   needs, which literal pathspecs would switch off.
//! - `-a` for one file: the caller has already decided the file is text, by git's rules
//!   (R2.5), so git is not asked to decide it again. The whole-comparison form leaves it off,
//!   so a binary file costs git a line rather than a diff, and a file git and the caller
//!   disagree about is asked about on its own.
//! - `--raw -z` in the same call: the raw records come first, each file's patch after them
//!   in the same order (a type change's patch is two sections, a deletion and an addition),
//!   which is how each patch is matched to the file it is about — never by parsing the
//!   paths out of a `diff --git` line, which git quotes.
//!
//! **What it reads.** The two commits' trees and the blobs of the files it diffs — for one
//! file, its two blobs, which the caller has already read through gix, so in a partial
//! clone a blob the clone lacks fails the caller's own read before this one starts; for a
//! whole comparison, every changed file's, which the caller has also read first, each
//! answering at least its header. So a read here has nothing to fetch that the caller did
//! not already find present (`crate::reads`, on git older than 2.44).
//!
//! Nothing is parsed from stderr, and the patch is read as bytes: a line's content is
//! whatever git printed after its marker, and the `\ No newline at end of file` marker is
//! read by its first byte alone, since its text is not something to depend on.

use std::ffi::OsString;
use std::ops::Range;
use std::os::unix::ffi::OsStringExt as _;

use cairn_model::{
    ChangeStatus, ChangedFile, ChangedRange, DiffLine, LineNumber, LineSpan, Oid, RepoPath,
};

use super::changes::RawRecords;
use super::{Detection, Submodules};
use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// The diff algorithms git knows, spelled as `--diff-algorithm` takes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Algorithm {
    Myers,
    Minimal,
    Patience,
    Histogram,
}

impl Algorithm {
    /// `parse_algorithm_value` in git's `diff.c`: the four names, any case, and `default`
    /// for myers. `None` for anything else, which git refuses.
    pub(crate) fn parse(value: &[u8]) -> Option<Self> {
        let is = |name: &str| value.eq_ignore_ascii_case(name.as_bytes());
        if is("myers") || is("default") {
            Some(Self::Myers)
        } else if is("minimal") {
            Some(Self::Minimal)
        } else if is("patience") {
            Some(Self::Patience)
        } else if is("histogram") {
            Some(Self::Histogram)
        } else {
            None
        }
    }

    /// Always the long spelling: `--minimal` alone combines with a diff driver's algorithm
    /// where `--diff-algorithm=minimal` replaces it.
    fn flag(self) -> &'static str {
        match self {
            Self::Myers => "--diff-algorithm=myers",
            Self::Minimal => "--diff-algorithm=minimal",
            Self::Patience => "--diff-algorithm=patience",
            Self::Histogram => "--diff-algorithm=histogram",
        }
    }
}

/// What a patch read is about.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Scope<'a> {
    /// One changed file: its paths are the pathspec, read literally, and the detection is
    /// what pairs them — `-M` for a rename, `-C --find-copies-harder` for a copy, so that a
    /// copy's source is a candidate whether or not it changed, and none otherwise.
    File(&'a ChangedFile),
    /// Every file of the comparison, with the changes query's detection and submodule rule,
    /// so git pairs and lists exactly what that query listed.
    Comparison {
        detection: Detection,
        submodules: Submodules<'a>,
    },
}

/// One patch read: two trees, the context, the algorithm, and what to read.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PatchQuery<'a> {
    pub(crate) old: &'a Oid,
    pub(crate) new: &'a Oid,
    /// The context the view groups at; raised to one, never zero.
    pub(crate) context: u32,
    /// `None` passes no algorithm, so git applies a diff driver's own.
    pub(crate) algorithm: Option<Algorithm>,
    /// `-w`: the display-only reading, never the exact one.
    pub(crate) ignore_whitespace: bool,
    pub(crate) scope: Scope<'a>,
}

/// One file of git's answer: its raw record, and its patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilePatch {
    pub(crate) file: ChangedFile,
    /// `None` for a type change, whose patch is a deletion and an addition rather than a
    /// diff of the two contents; and, under `-w` on a git older than the one that leaves
    /// such a file out of the raw records too, for a modified file whose every change is
    /// whitespace, which git lists and prints no patch for.
    pub(crate) text: Option<PatchText>,
}

/// One file's patch as git printed it, kept as bytes until it is read against the lines
/// it is about.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct PatchText {
    hunks: Vec<GitHunk>,
    lines: Vec<BodyLine>,
    body: Vec<u8>,
    binary: bool,
    /// What the `index` line names, `<old>..<new>` in full (`--full-index`), when there
    /// is one: how a section is told from a file git listed and printed no patch for.
    index: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitHunk {
    old: LineSpan,
    new: LineSpan,
    /// What git printed after the header's closing `@@` and the space that separates it.
    function: Vec<u8>,
    /// Indices into `lines`.
    lines: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BodyLine {
    kind: LineKind,
    /// Into `body`: the line without its marker and without its `\n`.
    bytes: Range<usize>,
    terminated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    Context,
    Removed,
    Added,
}

/// A patch read against the lines it is about.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Reading {
    pub(crate) changes: Vec<ChangedRange>,
    /// Each hunk's old-side start and git's function context for it.
    pub(crate) function_context: Vec<(LineNumber, Vec<u8>)>,
}

impl PatchText {
    /// git printed `Binary files .. differ` instead of a diff.
    pub(crate) fn is_binary(&self) -> bool {
        self.binary
    }

    /// The changes, as maximal runs of removed and added lines, and each hunk's function
    /// context — after checking every line git printed against the lines the caller holds:
    /// a removed line against `old`, an added line against `new`, a context line against
    /// `new` (git prints context from the new side, which under `-w` may differ from the old
    /// in its whitespace) and, unless whitespace was ignored, against `old` as well. A line
    /// that differs, its newline included, or one past the end of its side, is `Err` with
    /// what differed: git read other content than the caller did, and neither answer is
    /// one to draw.
    pub(crate) fn read_against(
        &self,
        old: &[DiffLine],
        new: &[DiffLine],
        whitespace_ignored: bool,
    ) -> Result<Reading, String> {
        let mut reading = Reading::default();
        let (mut old_end, mut new_end) = (0u32, 0u32);
        for hunk in &self.hunks {
            let (old_start, new_start) = (hunk.old.start().index(), hunk.new.start().index());
            if old_start < old_end
                || new_start < new_end
                || old_start - old_end != new_start - new_end
            {
                return Err(format!(
                    "the hunk at old line {} and new line {} is out of step with the one before",
                    hunk.old.start().one_based(),
                    hunk.new.start().one_based()
                ));
            }
            reading
                .function_context
                .push((hunk.old.start(), hunk.function.clone()));

            let (mut at_old, mut at_new) = (old_start, new_start);
            let mut run: Option<(u32, u32)> = None;
            let close = |run: &mut Option<(u32, u32)>, at: (u32, u32), out: &mut Vec<_>| {
                if let Some((from_old, from_new)) = run.take() {
                    out.push(ChangedRange::new(
                        LineSpan::at(from_old, at.0 - from_old),
                        LineSpan::at(from_new, at.1 - from_new),
                    ));
                }
            };
            for line in self.lines.get(hunk.lines.clone()).unwrap_or_default() {
                let bytes = self.body.get(line.bytes.clone()).unwrap_or_default();
                match line.kind {
                    LineKind::Context => {
                        close(&mut run, (at_old, at_new), &mut reading.changes);
                        same(new, at_new, bytes, line.terminated, "new")?;
                        if !whitespace_ignored {
                            same(old, at_old, bytes, line.terminated, "old")?;
                        }
                        at_old += 1;
                        at_new += 1;
                    }
                    LineKind::Removed => {
                        run.get_or_insert((at_old, at_new));
                        same(old, at_old, bytes, line.terminated, "old")?;
                        at_old += 1;
                    }
                    LineKind::Added => {
                        run.get_or_insert((at_old, at_new));
                        same(new, at_new, bytes, line.terminated, "new")?;
                        at_new += 1;
                    }
                }
            }
            close(&mut run, (at_old, at_new), &mut reading.changes);
            (old_end, new_end) = (at_old, at_new);
        }
        Ok(reading)
    }
}

/// Whether line `index` of `side` is `bytes`, ending in a newline exactly when `terminated`.
fn same(
    side: &[DiffLine],
    index: u32,
    bytes: &[u8],
    terminated: bool,
    which: &str,
) -> Result<(), String> {
    match side.get(index as usize) {
        Some(line) if line.bytes() == bytes && line.ends_with_newline() == terminated => Ok(()),
        Some(_) => Err(format!(
            "line {} of the {which} side is not the line git printed there",
            index + 1
        )),
        None => Err(format!(
            "git printed line {} of the {which} side, which has {} lines",
            index + 1,
            side.len()
        )),
    }
}

/// The patch for what `query` names, file by file in git's order.
///
/// `cancel` is polled by the runner on every tick while `git` runs: a superseded query ends
/// the process and answers [`Error::ContentCancelled`], and one already superseded starts
/// nothing. A failure is classified by `git`'s exit status ([`Error::GitFailed`]); output
/// this parser does not know is [`Error::UnexpectedGitOutput`], and nothing of it is used.
pub(crate) fn patches(
    git: &GitBinary,
    repo: &Repository,
    query: &PatchQuery<'_>,
    cancel: &impl Cancel,
) -> Result<Vec<FilePatch>, Error> {
    if cancel.is_cancelled() {
        return Err(Error::ContentCancelled);
    }
    let arguments = arguments(query);
    let mut parser = Parser::default();
    let outcome = git
        .read_invocation()
        .in_repository(repo)
        .args(&arguments)
        .start()?
        .finish(cancel, |chunk| parser.push(chunk), |_| {});
    match outcome {
        Ok(_) => {
            parser
                .finish(query.ignore_whitespace)
                .map_err(|record| Error::UnexpectedGitOutput {
                    arguments: arguments
                        .iter()
                        .map(|argument| argument.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(" "),
                    record,
                })
        }
        Err(Error::GitReadCancelled { .. }) => Err(Error::ContentCancelled),
        Err(other) => Err(other),
    }
}

fn arguments(query: &PatchQuery<'_>) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = Vec::new();
    if matches!(query.scope, Scope::File(_)) {
        arguments.push("--literal-pathspecs".into());
    }
    arguments.extend(
        [
            "-c",
            "diff.suppressBlankEmpty=false",
            "diff-tree",
            "-r",
            "-z",
            "--raw",
            "--no-abbrev",
            "-p",
            "--full-index",
        ]
        .map(OsString::from),
    );
    arguments.push(format!("-U{}", query.context.max(1)).into());
    arguments.extend(["--no-ext-diff", "--no-textconv", "--no-color"].map(OsString::from));
    if matches!(query.scope, Scope::File(_)) {
        arguments.push("-a".into());
    }
    if query.ignore_whitespace {
        arguments.push("-w".into());
    }
    if let Some(algorithm) = query.algorithm {
        arguments.push(algorithm.flag().into());
    }
    match query.scope {
        Scope::File(file) => match file.status {
            ChangeStatus::Renamed(_) => arguments.push("-M".into()),
            ChangeStatus::Copied(_) => {
                arguments.extend(["-C", "--find-copies-harder"].map(OsString::from));
            }
            ChangeStatus::Added
            | ChangeStatus::Deleted
            | ChangeStatus::Modified
            | ChangeStatus::TypeChanged => arguments.push("--no-renames".into()),
        },
        Scope::Comparison {
            detection,
            submodules,
        } => {
            arguments.extend(detection.arguments().into_iter().map(OsString::from));
            if submodules == Submodules::HideEvery {
                arguments.push("--ignore-submodules=all".into());
            }
        }
    }
    arguments.push("--end-of-options".into());
    arguments.push(query.old.to_string().into());
    arguments.push(query.new.to_string().into());
    match query.scope {
        Scope::File(file) => {
            arguments.push("--".into());
            arguments.push(path(&file.new_path));
            if file.old_path != file.new_path {
                arguments.push(path(&file.old_path));
            }
        }
        Scope::Comparison { submodules, .. } => {
            if let Submodules::Excluding(paths) = submodules
                && !paths.is_empty()
            {
                arguments.push("--".into());
                for excluded in paths {
                    let mut pathspec = b":(exclude,literal)".to_vec();
                    pathspec.extend_from_slice(excluded.as_bytes());
                    arguments.push(OsString::from_vec(pathspec));
                }
            }
        }
    }
    arguments
}

fn path(path: &RepoPath) -> OsString {
    OsString::from_vec(path.as_bytes().to_vec())
}

/// Reads the answer as it arrives: NUL-terminated raw records up to the empty one that
/// separates them from the patches, then the patches line by line.
#[derive(Debug, Default)]
struct Parser {
    in_patches: bool,
    pending: Vec<u8>,
    records: RawRecords,
    sections: Vec<PatchText>,
    /// Lines still owed to the hunk being read: old side, new side.
    owed: (u32, u32),
    /// Whether the last line read was a line of a hunk's body, which is the only line a
    /// `\ No newline at end of file` marker may follow.
    after_body_line: bool,
    malformed: Option<String>,
}

impl Parser {
    fn push(&mut self, mut chunk: &[u8]) {
        while !chunk.is_empty() && self.malformed.is_none() {
            let terminator = if self.in_patches { b'\n' } else { 0 };
            let Some(end) = chunk.iter().position(|byte| *byte == terminator) else {
                self.pending.extend_from_slice(chunk);
                return;
            };
            let (whole, rest) = (&chunk[..end], &chunk[end + 1..]);
            if self.pending.is_empty() {
                self.unit(whole);
            } else {
                let mut joined = std::mem::take(&mut self.pending);
                joined.extend_from_slice(whole);
                self.unit(&joined);
            }
            chunk = rest;
        }
    }

    fn unit(&mut self, unit: &[u8]) {
        if self.in_patches {
            self.line(unit);
        } else if unit.is_empty() {
            self.in_patches = true;
        } else {
            self.records.push(unit);
        }
    }

    fn refuse(&mut self, what: impl Into<String>) {
        if self.malformed.is_none() {
            self.malformed = Some(what.into());
        }
    }

    fn line(&mut self, line: &[u8]) {
        let lossy = || String::from_utf8_lossy(line).into_owned();
        if self.owed != (0, 0) {
            self.body_line(line);
            return;
        }
        if line.first() == Some(&b'\\') {
            if self.after_body_line {
                self.mark_unterminated();
            } else {
                self.refuse(format!("a no-newline marker after no line: {}", lossy()));
            }
            return;
        }
        self.after_body_line = false;
        if line.starts_with(b"diff --git ") {
            self.sections.push(PatchText::default());
            return;
        }
        let Some(section) = self.sections.last_mut() else {
            self.refuse(format!("a line before the first patch: {}", lossy()));
            return;
        };
        if let Some(header) = line.strip_prefix(b"@@ ") {
            let Some((old, new, function)) = hunk_header(header) else {
                self.refuse(format!("a hunk header git does not print: {}", lossy()));
                return;
            };
            if old.is_empty() && new.is_empty() {
                self.refuse(format!("a hunk with no lines: {}", lossy()));
                return;
            }
            let first = section.lines.len();
            section.hunks.push(GitHunk {
                old,
                new,
                function: function.to_vec(),
                lines: first..first,
            });
            self.owed = (old.len(), new.len());
            return;
        }
        if !section.hunks.is_empty() {
            self.refuse(format!("a line after a hunk's last: {}", lossy()));
            return;
        }
        // The extended header: `index`, the modes, the rename and copy lines, `---` and
        // `+++` — none of which this read needs — and the line that stands for a diff git
        // would not make.
        if line.starts_with(b"Binary files ") {
            section.binary = true;
        } else if let Some(index) = line.strip_prefix(b"index ") {
            let ids = index.split(|byte| *byte == b' ').next().unwrap_or_default();
            section.index = Some(ids.to_vec());
        }
    }

    fn body_line(&mut self, line: &[u8]) {
        let kind = match line.first() {
            Some(b' ') if self.owed.0 > 0 && self.owed.1 > 0 => LineKind::Context,
            Some(b'-') if self.owed.0 > 0 => LineKind::Removed,
            Some(b'+') if self.owed.1 > 0 => LineKind::Added,
            Some(b'\\') if self.after_body_line => {
                self.mark_unterminated();
                return;
            }
            _ => {
                self.refuse(format!(
                    "a line that is not one of the hunk's: {}",
                    String::from_utf8_lossy(line)
                ));
                return;
            }
        };
        match kind {
            LineKind::Context => self.owed = (self.owed.0 - 1, self.owed.1 - 1),
            LineKind::Removed => self.owed.0 -= 1,
            LineKind::Added => self.owed.1 -= 1,
        }
        let Some(section) = self.sections.last_mut() else {
            return;
        };
        let start = section.body.len();
        section
            .body
            .extend_from_slice(line.get(1..).unwrap_or_default());
        section.lines.push(BodyLine {
            kind,
            bytes: start..section.body.len(),
            terminated: true,
        });
        if let Some(hunk) = section.hunks.last_mut() {
            hunk.lines.end = section.lines.len();
        }
        self.after_body_line = true;
    }

    fn mark_unterminated(&mut self) {
        let last = self
            .sections
            .last_mut()
            .and_then(|section| section.lines.last_mut());
        match last {
            Some(line) if line.terminated => line.terminated = false,
            _ => self.refuse("a second no-newline marker on one line"),
        }
        // A second marker in a row is refused above; this keeps the next one from being
        // taken for the same line's.
        self.after_body_line = false;
    }

    /// The files, each with its patch. `Err` is the first thing that did not parse.
    ///
    /// Under `-w` (`whitespace_ignored`), git before the version that leaves such a file
    /// out of its raw records as well lists a modified file whose every change is
    /// whitespace and prints no patch for it — reproduced with git 2.30.9, 2.39.5 and 2.40.0,
    /// where 2.56 lists nothing. So there, a modified file whose mode did not change is
    /// matched to the next patch only when that patch's `index` line names its two blobs,
    /// and is otherwise one git printed nothing for. A type change, a rename, a copy, an
    /// addition, a deletion and a mode change always have a patch: each has header lines git
    /// must show.
    fn finish(mut self, whitespace_ignored: bool) -> Result<Vec<FilePatch>, String> {
        if !self.pending.is_empty() {
            let rest = std::mem::take(&mut self.pending);
            if self.in_patches {
                self.line(&rest);
            } else {
                self.refuse(format!(
                    "output cut off in a record: {}",
                    String::from_utf8_lossy(&rest)
                ));
            }
        }
        if let Some(malformed) = self.malformed {
            return Err(malformed);
        }
        if self.owed != (0, 0) {
            return Err("a hunk cut off before its last line".to_owned());
        }
        let files = self
            .records
            .finish("diff-tree")
            .map_err(|error| error.to_string())?;
        if !files.is_empty() && !self.in_patches {
            return Err("raw records with no patches after them".to_owned());
        }
        let mut sections = self.sections.into_iter().peekable();
        let mut answer = Vec::with_capacity(files.len());
        for file in files {
            let may_be_left_out = whitespace_ignored
                && file.status == ChangeStatus::Modified
                && file.old_mode == file.new_mode;
            if may_be_left_out {
                let names = match (file.old_id, file.new_id) {
                    (Some(old), Some(new)) => format!("{old}..{new}").into_bytes(),
                    _ => Vec::new(),
                };
                let printed = sections
                    .peek()
                    .is_some_and(|section| section.index.as_deref() == Some(&names[..]));
                if !printed {
                    answer.push(FilePatch { file, text: None });
                    continue;
                }
            }
            let text = if file.status == ChangeStatus::TypeChanged {
                // A deletion of the old kind, then an addition of the new.
                if sections.next().is_none() || sections.next().is_none() {
                    return Err(format!("no patch for the type change of {}", file.new_path));
                }
                None
            } else {
                match sections.next() {
                    Some(section) => Some(section),
                    None => return Err(format!("no patch for {}", file.new_path)),
                }
            };
            answer.push(FilePatch { file, text });
        }
        if sections.next().is_some() {
            return Err("a patch for no file git listed".to_owned());
        }
        Ok(answer)
    }
}

/// git's output for a patch read, parsed as [`patches`] parses it, for a test elsewhere in
/// the crate that needs a patch without running git.
#[cfg(test)]
pub(crate) fn parse(output: &[u8]) -> Result<Vec<FilePatch>, String> {
    let mut parser = Parser::default();
    parser.push(output);
    parser.finish(false)
}

/// `-<a>[,<b>] +<c>[,<d>] @@[ <function>]`, as model spans: an empty side names the line
/// before it, a count of one may be left out.
fn hunk_header(header: &[u8]) -> Option<(LineSpan, LineSpan, &[u8])> {
    let (old, rest) = side(header, b'-')?;
    let rest = rest.strip_prefix(b" ")?;
    let (new, rest) = side(rest, b'+')?;
    let rest = rest.strip_prefix(b" @@")?;
    let function = match rest.split_first() {
        None => &[][..],
        Some((b' ', function)) => function,
        Some(_) => return None,
    };
    Some((old, new, function))
}

fn side(text: &[u8], marker: u8) -> Option<(LineSpan, &[u8])> {
    let text = text.strip_prefix(&[marker])?;
    let (start, rest) = number(text)?;
    let (count, rest) = match rest.strip_prefix(b",") {
        Some(rest) => number(rest)?,
        None => (1, rest),
    };
    let index = if count == 0 {
        start
    } else {
        start.checked_sub(1)?
    };
    Some((LineSpan::at(index, count), rest))
}

fn number(text: &[u8]) -> Option<(u32, &[u8])> {
    let digits = text.iter().take_while(|byte| byte.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let value = std::str::from_utf8(text.get(..digits)?)
        .ok()?
        .parse()
        .ok()?;
    Some((value, text.get(digits..)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{FileMode, split_lines};

    const OLD: &str = "07da224c7ec04501dfb451be161fa962effe1dc1";
    const NEW: &str = "ebc3711f1349e6204d89c344c178221fb8bf5113";

    fn record(status: &str, paths: &[&str]) -> Vec<u8> {
        let (old_mode, old_id, new_mode, new_id) = match status {
            "A" => ("000000", "0".repeat(40), "100644", NEW.to_owned()),
            "D" => ("100644", OLD.to_owned(), "000000", "0".repeat(40)),
            "T" => ("100644", OLD.to_owned(), "120000", NEW.to_owned()),
            _ => ("100644", OLD.to_owned(), "100644", NEW.to_owned()),
        };
        let mut out = format!(":{old_mode} {new_mode} {old_id} {new_id} {status}\0").into_bytes();
        for path in paths {
            out.extend_from_slice(path.as_bytes());
            out.push(0);
        }
        out
    }

    /// The output as git writes it: records, the empty record, then the patches.
    fn output(records: &[Vec<u8>], patches: &str) -> Vec<u8> {
        let mut out: Vec<u8> = records.concat();
        out.push(0);
        out.extend_from_slice(patches.as_bytes());
        out
    }

    /// Fed a byte at a time and all at once: the answer must not depend on how git's
    /// writes were split.
    fn parse(bytes: &[u8]) -> Result<Vec<FilePatch>, String> {
        let mut whole = Parser::default();
        whole.push(bytes);
        let at_once = whole.finish(false);
        let mut drip = Parser::default();
        for byte in bytes {
            drip.push(std::slice::from_ref(byte));
        }
        assert_eq!(
            drip.finish(false),
            at_once,
            "the answer depended on the chunking"
        );
        at_once
    }

    fn file(old: &str, new: &str, status: ChangeStatus) -> ChangedFile {
        ChangedFile {
            status,
            old_path: RepoPath::new(old),
            new_path: RepoPath::new(new),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: Oid::parse(OLD).ok(),
            new_id: Oid::parse(NEW).ok(),
        }
    }

    fn query<'a>(file: &'a ChangedFile, old: &'a Oid, new: &'a Oid) -> PatchQuery<'a> {
        PatchQuery {
            old,
            new,
            context: 3,
            algorithm: Some(Algorithm::Histogram),
            ignore_whitespace: false,
            scope: Scope::File(file),
        }
    }

    fn strings(arguments: Vec<OsString>) -> Vec<String> {
        arguments
            .into_iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }

    /// The read is `diff-tree` — query plumbing — with a patch, and never with a flag that
    /// runs a program the user configured or writes (`--textconv`, `--ext-diff`) or one
    /// that lets git trim the tail it diffs (`-U0`). Caught by: porcelain in place of the
    /// plumbing, either flag added, the context dropped to zero, the algorithm passed in
    /// its short form or dropped, a pathspec read with magic, or the trees before
    /// `--end-of-options`.
    #[test]
    fn a_file_read_is_diff_tree_with_a_patch_and_never_runs_a_program() {
        let (old, new) = (Oid::parse(OLD).unwrap(), Oid::parse(NEW).unwrap());
        let modified = file(":(glob)*.rs", ":(glob)*.rs", ChangeStatus::Modified);
        let mut asked = query(&modified, &old, &new);
        asked.context = 0;
        let arguments = strings(arguments(&asked));
        let verb = arguments.iter().position(|a| a == "diff-tree").unwrap();
        assert_eq!(
            arguments[..verb],
            ["--literal-pathspecs", "-c", "diff.suppressBlankEmpty=false"],
            "global options before the verb: {arguments:?}"
        );
        for wanted in [
            "-p",
            "--full-index",
            "--raw",
            "-z",
            "-r",
            "--no-abbrev",
            "--no-textconv",
            "--no-ext-diff",
            "--no-color",
            "-a",
            "--diff-algorithm=histogram",
            "--no-renames",
            "-U1",
        ] {
            assert!(
                arguments.iter().any(|a| a == wanted),
                "{wanted} missing: {arguments:?}"
            );
        }
        for refused in [
            "--textconv",
            "--ext-diff",
            "-U0",
            "--histogram",
            "--minimal",
            "-w",
            "diff",
            "show",
            "log",
        ] {
            assert!(
                !arguments.iter().any(|a| a == refused),
                "{refused} present: {arguments:?}"
            );
        }
        let trees = arguments
            .iter()
            .position(|a| a == "--end-of-options")
            .unwrap();
        assert_eq!(
            arguments[trees + 1..],
            [OLD, NEW, "--", ":(glob)*.rs"],
            "the trees, then the path once, read literally"
        );
    }

    /// Detection per status, both paths of a pair, `-w` only when asked, and no algorithm
    /// flag at all when a driver names its own.
    #[test]
    fn a_pair_names_both_paths_and_is_detected_the_way_it_was_found() {
        let (old, new) = (Oid::parse(OLD).unwrap(), Oid::parse(NEW).unwrap());
        let renamed = file(
            "from",
            "to",
            ChangeStatus::Renamed(cairn_model::Similarity::from_percent(90)),
        );
        let copied = file(
            "src",
            "dst",
            ChangeStatus::Copied(cairn_model::Similarity::from_percent(90)),
        );
        let mut asked = query(&renamed, &old, &new);
        asked.ignore_whitespace = true;
        asked.algorithm = None;
        let renamed_arguments = strings(arguments(&asked));
        let a = &renamed_arguments;
        assert!(a.iter().any(|x| x == "-M"), "{a:?}");
        assert!(a.iter().any(|x| x == "-w"), "{a:?}");
        assert!(
            !a.iter().any(|x| x.starts_with("--diff-algorithm")),
            "{a:?}"
        );
        assert!(a.ends_with(&["--".into(), "to".into(), "from".into()]));

        let copied_arguments = strings(arguments(&query(&copied, &old, &new)));
        let a = &copied_arguments;
        assert!(
            a.iter().any(|x| x == "-C") && a.iter().any(|x| x == "--find-copies-harder"),
            "{a:?}"
        );
        assert!(a.ends_with(&["--".into(), "dst".into(), "src".into()]));
    }

    /// The whole-comparison form asks what the changes query asked, and nothing it would
    /// read as a literal pathspec — its exclusions are magic.
    #[test]
    fn a_comparison_read_asks_what_the_changes_query_asked() {
        let (old, new) = (Oid::parse(OLD).unwrap(), Oid::parse(NEW).unwrap());
        let excluded = [RepoPath::new("sub")];
        let asked = PatchQuery {
            old: &old,
            new: &new,
            context: 5,
            algorithm: Some(Algorithm::Myers),
            ignore_whitespace: false,
            scope: Scope::Comparison {
                detection: Detection::Copies { limit: 7 },
                submodules: Submodules::Excluding(&excluded),
            },
        };
        let arguments = strings(arguments(&asked));
        assert_eq!(arguments[0], "-c", "{arguments:?}");
        assert!(!arguments.iter().any(|a| a == "--literal-pathspecs"));
        assert!(!arguments.iter().any(|a| a == "-a"));
        assert!(arguments.iter().any(|a| a == "-U5"));
        assert!(arguments.windows(2).any(|pair| pair == ["-C", "-l7"]));
        assert!(arguments.ends_with(&["--".into(), ":(exclude,literal)sub".into()]));
    }

    #[test]
    fn every_algorithm_is_spelled_and_parsed_as_git_does() {
        for (value, parsed) in [
            ("myers", Some(Algorithm::Myers)),
            ("Default", Some(Algorithm::Myers)),
            ("MINIMAL", Some(Algorithm::Minimal)),
            ("patience", Some(Algorithm::Patience)),
            ("Histogram", Some(Algorithm::Histogram)),
            ("histo", None),
            ("", None),
        ] {
            assert_eq!(Algorithm::parse(value.as_bytes()), parsed, "{value}");
        }
        for algorithm in [
            Algorithm::Myers,
            Algorithm::Minimal,
            Algorithm::Patience,
            Algorithm::Histogram,
        ] {
            let flag = algorithm.flag();
            let name = flag.strip_prefix("--diff-algorithm=").unwrap();
            assert_eq!(Algorithm::parse(name.as_bytes()), Some(algorithm));
        }
    }

    const ONE_HUNK: &str = "diff --git a/f b/f\nindex 07da224..ebc3711 100644\n--- a/f\n+++ b/f\n\
        @@ -2,4 +2,4 @@ fn main() {\n b\n-c\n-d\n+C\n+D\n e\n";

    /// Changes are maximal runs, the function context is what follows the header's `@@`,
    /// and every line is checked. Caught by: reading a change from the header, which spans
    /// its context.
    #[test]
    fn a_change_is_a_run_of_marked_lines_and_the_header_carries_the_function() {
        let patches = parse(&output(&[record("M", &["f"])], ONE_HUNK)).unwrap();
        assert_eq!(patches.len(), 1);
        let text = patches[0].text.as_ref().unwrap();
        let old = split_lines(b"a\nb\nc\nd\ne\nf\n");
        let new = split_lines(b"a\nb\nC\nD\ne\nf\n");
        let reading = text.read_against(&old, &new, false).unwrap();
        assert_eq!(
            reading.changes,
            vec![ChangedRange::new(LineSpan::at(2, 2), LineSpan::at(2, 2))]
        );
        assert_eq!(
            reading.function_context,
            vec![(LineNumber::from_index(1), b"fn main() {".to_vec())]
        );
    }

    /// Two changes in one hunk are two ranges; an insertion's empty side says where it sits.
    #[test]
    fn two_runs_in_one_hunk_are_two_changes() {
        let patch = "diff --git a/f b/f\n@@ -1,5 +1,5 @@\n a\n-b\n+B\n c\n+new\n d\n-e\n";
        let patches = parse(&output(&[record("M", &["f"])], patch)).unwrap();
        let old = split_lines(b"a\nb\nc\nd\ne\n");
        let new = split_lines(b"a\nB\nc\nnew\nd\n");
        let reading = patches[0]
            .text
            .as_ref()
            .unwrap()
            .read_against(&old, &new, false)
            .unwrap();
        assert_eq!(
            reading.changes,
            vec![
                ChangedRange::new(LineSpan::at(1, 1), LineSpan::at(1, 1)),
                ChangedRange::new(LineSpan::at(3, 0), LineSpan::at(3, 1)),
                ChangedRange::new(LineSpan::at(4, 1), LineSpan::at(5, 0)),
            ]
        );
        assert_eq!(
            reading.function_context,
            vec![(LineNumber::from_index(0), Vec::new())],
            "no text after `@@` is git printing none"
        );
    }

    /// The stale-read guard: a line git printed that is not the line held — in its bytes,
    /// in its newline, or past the end — refuses the whole reading. Caught by: checking
    /// only the changed lines, or only the bytes.
    #[test]
    fn a_line_git_printed_that_is_not_the_one_held_refuses_the_reading() {
        let patches = parse(&output(&[record("M", &["f"])], ONE_HUNK)).unwrap();
        let text = patches[0].text.as_ref().unwrap();
        let new = split_lines(b"a\nb\nC\nD\ne\nf\n");
        for (old, why) in [
            (&b"a\nb\nc\nX\ne\nf\n"[..], "a removed line differs"),
            (b"a\nB\nc\nd\ne\nf\n", "a context line differs"),
            (b"a\nb\nc\nd\ne", "a context line lost its newline"),
            (b"a\nb\nc\n", "the side ends early"),
        ] {
            let held = split_lines(old);
            assert!(
                text.read_against(&held, &new, false).is_err(),
                "accepted when {why}"
            );
        }
        let other_new = split_lines(b"a\nb\nC\nd\ne\nf\n");
        assert!(
            text.read_against(&split_lines(b"a\nb\nc\nd\ne\nf\n"), &other_new, false)
                .is_err()
        );
        assert!(
            text.read_against(&split_lines(b"a\nb\nc\nd\ne\nf\n"), &new, false)
                .is_ok(),
            "the passing twin"
        );
    }

    /// Under `-w` a context line is the new side's, which may differ from the old in its
    /// whitespace; it is checked against the new side only.
    #[test]
    fn under_ignored_whitespace_context_is_the_new_sides() {
        let patch = "diff --git a/f b/f\n@@ -1,3 +1,3 @@\n   a\n-b\n+B\n c\n";
        let patches = parse(&output(&[record("M", &["f"])], patch)).unwrap();
        let text = patches[0].text.as_ref().unwrap();
        let old = split_lines(b"a\nb\nc\n");
        let new = split_lines(b"  a\nB\nc\n");
        assert!(text.read_against(&old, &new, true).is_ok());
        assert!(
            text.read_against(&old, &new, false).is_err(),
            "the exact reading checks the old side too"
        );
    }

    /// `\ No newline at end of file` belongs to the line before it, inside a hunk or at its
    /// end, and is read by its first byte. Caught by: counting it as a line, or attaching it
    /// to the wrong one.
    #[test]
    fn a_no_newline_marker_belongs_to_the_line_before_it() {
        let patch = "diff --git a/f b/f\n@@ -1,2 +1,2 @@\n a\n-b\n\\ Kein Zeilenumbruch\n+b\n";
        let patches = parse(&output(&[record("M", &["f"])], patch)).unwrap();
        let reading = patches[0]
            .text
            .as_ref()
            .unwrap()
            .read_against(&split_lines(b"a\nb"), &split_lines(b"a\nb\n"), false)
            .unwrap();
        assert_eq!(
            reading.changes,
            vec![ChangedRange::new(LineSpan::at(1, 1), LineSpan::at(1, 1))]
        );
        let patch = "diff --git a/f b/f\n@@ -1 +1 @@\n-a\n+b\n\\ No newline at end of file\n";
        let patches = parse(&output(&[record("M", &["f"])], patch)).unwrap();
        assert!(
            patches[0]
                .text
                .as_ref()
                .unwrap()
                .read_against(&split_lines(b"a\n"), &split_lines(b"b"), false)
                .is_ok()
        );
    }

    /// Patches follow their records in order; a type change is two sections and carries
    /// none; a file with no hunks (a pure rename, a mode change) and a binary file are
    /// still a section each.
    #[test]
    fn each_patch_is_matched_to_its_record_by_order() {
        let patches = "diff --git a/b b/b\nindex 1..2 100644\nBinary files a/b and b/b differ\n\
            diff --git a/t b/t\ndeleted file mode 100644\n--- a/t\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n\
            diff --git a/t b/t\nnew file mode 120000\n--- /dev/null\n+++ b/t\n@@ -0,0 +1 @@\n+y\n\
            \\ No newline at end of file\n\
            diff --git a/from b/to\nsimilarity index 100%\nrename from from\nrename to to\n\
            diff --git a/f b/f\n@@ -1 +1 @@\n-1\n+2\n";
        let parsed = parse(&output(
            &[
                record("M", &["b"]),
                record("T", &["t"]),
                record("R100", &["from", "to"]),
                record("M", &["f"]),
            ],
            patches,
        ))
        .unwrap();
        assert_eq!(parsed.len(), 4);
        assert!(parsed[0].text.as_ref().unwrap().is_binary());
        assert_eq!(parsed[1].text, None, "a type change carries no diff");
        assert_eq!(parsed[2].file.new_path.as_bytes(), b"to");
        assert!(parsed[2].text.as_ref().unwrap().hunks.is_empty());
        assert_eq!(parsed[3].file.new_path.as_bytes(), b"f");
        assert_eq!(parsed[3].text.as_ref().unwrap().hunks.len(), 1);
    }

    /// Under `-w`, a git that lists a whitespace-only file and prints no patch for it: the
    /// file is matched to no patch, and the next file to its own. Caught by: matching by
    /// order alone, which hands one file's patch to the file before it.
    #[test]
    fn under_ignored_whitespace_a_listed_file_with_no_patch_has_none() {
        let patch = format!(
            "diff --git a/g b/g\nindex {OLD}..{NEW} 100644\n--- a/g\n+++ b/g\n@@ -1 +1 @@\n-x\n+y\n"
        );
        // f changed between two other blobs, so g's `index` line does not name it.
        let f = format!(
            ":100644 100644 {} {} M\0f\0",
            "1".repeat(40),
            "2".repeat(40)
        )
        .into_bytes();
        let bytes = output(&[f, record("M", &["g"])], &patch);
        let mut parser = Parser::default();
        parser.push(&bytes);
        let parsed = parser.finish(true).unwrap();
        assert_eq!(parsed[0].text, None, "f has no patch");
        assert_eq!(parsed[1].file.new_path.as_bytes(), b"g");
        assert_eq!(parsed[1].text.as_ref().unwrap().hunks.len(), 1);
        let mut parser = Parser::default();
        parser.push(&bytes);
        assert!(
            parser.finish(false).is_err(),
            "without -w a listed file must have its patch"
        );
    }

    #[test]
    fn no_change_is_no_output_and_no_files() {
        assert_eq!(parse(b""), Ok(Vec::new()));
    }

    /// Each of these is output git does not print; a guess could draw a wrong diff.
    #[test]
    fn what_git_does_not_print_is_refused() {
        let m = || vec![record("M", &["f"])];
        for (records, patches, why) in [
            (m(), "", "a record with no patch"),
            (
                m(),
                "diff --git a/f b/f\n@@ -1,2 +1,2 @@\n a\n",
                "a hunk cut off",
            ),
            (
                m(),
                "diff --git a/f b/f\n@@ -1 +1 @@\n-a\n+b\nstray\n",
                "a line after the hunk",
            ),
            (
                m(),
                "diff --git a/f b/f\n@@ -1 +1 @@\n-a\n\n",
                "an empty body line",
            ),
            (
                m(),
                "diff --git a/f b/f\n@@ -1 +1\n-a\n+b\n",
                "a header without its @@",
            ),
            (
                m(),
                "diff --git a/f b/f\n@@ -0 +1 @@\n-a\n+b\n",
                "a line 0 with a line in it",
            ),
            (
                m(),
                "diff --git a/f b/f\n@@ -1 +1 @@x\n-a\n+b\n",
                "no space before the function",
            ),
            (
                m(),
                "diff --git a/f b/f\n\\ No newline\n",
                "a marker after no line",
            ),
            (
                m(),
                "diff --git a/f b/f\n@@ -1 +1 @@\n-a\n\\ x\n\\ x\n+b\n",
                "two markers on one line",
            ),
            (m(), "@@ -1 +1 @@\n-a\n+b\n", "a hunk before any patch"),
            (
                m(),
                "diff --git a/f b/f\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/g b/g\n",
                "a patch for no file",
            ),
            (
                vec![record("T", &["t"])],
                "diff --git a/t b/t\n",
                "half a type change",
            ),
        ] {
            assert!(parse(&output(&records, patches)).is_err(), "accepted {why}");
        }
        let mut cut = record("M", &["f"]);
        cut.pop();
        assert!(parse(&cut).is_err(), "accepted a record cut off");
        assert!(
            parse(&record("M", &["f"])).is_err(),
            "accepted records with no patches after them"
        );
    }

    /// Hunks that do not step through the file together — a gap of one length on the old
    /// side and another on the new — would be drawn as a patch that omits content.
    #[test]
    fn hunks_out_of_step_are_refused() {
        let patch = "diff --git a/f b/f\n@@ -1 +1 @@\n-a\n+A\n@@ -5 +6 @@\n-e\n+E\n";
        let patches = parse(&output(&[record("M", &["f"])], patch)).unwrap();
        let old = split_lines(b"a\nb\nc\nd\ne\n");
        let new = split_lines(b"A\nb\nc\nd\nx\nE\n");
        assert!(
            patches[0]
                .text
                .as_ref()
                .unwrap()
                .read_against(&old, &new, false)
                .is_err()
        );
    }
}
