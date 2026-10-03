//! One path's working-tree diff, as git computes it: `git diff-index --cached` for what is
//! staged, `git diff-files` for what is not, and `git diff --no-index` for a file git does
//! not track.
//!
//! Phase 03 of `diff-engine` (PRD R3). A working-tree side is read by git, never by Cairn:
//! git converts it to its own form — the clean filter driver the path's attributes name,
//! line endings, `ident`, a working-tree encoding — exactly as the user's `git diff` does,
//! and the lines Cairn holds for that side are rebuilt from git's own patch
//! ([`super::PatchText::new_side`]), so they are that form by construction. Each read runs
//!
//! ```text
//! git -c diff.suppressBlankEmpty=false <verb> -z --raw --no-abbrev -p --full-index -U<n>
//!     --no-ext-diff --no-textconv --no-color [-w] [--diff-algorithm=<a>]
//!     [--ignore-submodules=<v>] <what> -- <path>
//! ```
//!
//! — `diff --no-index`, a porcelain mode, also given `-c` for each key of
//! [`NO_INDEX_PRESENTATION`], the user's presentation settings that porcelain reads and
//! plumbing does not — with `<verb> <what>` one of `diff-index --cached --no-renames --end-of-options <commit>`
//! and `:(literal)<path> :(exclude,glob)<path, escaped>/**` as the pathspec; `diff-files
//! --no-renames` with that pathspec; or `diff --no-index` with `/dev/null <path>`, the path
//! spelled `./-` when it is `-` (`no_index_operand`), which git reads as stdin. The
//! exclusion keeps a path that is a directory on one side (`d` a file in the index, `d/x`
//! in `HEAD`) to the one record asked about, as `git diff -- d` would not.
//!
//! **What each reads, writes and runs** — reproduced with git 2.30.9, 2.32.7 and 2.56.0
//! against a snapshot of every file under the git directory, `.git/modules` included:
//!
//! - None writes anything: `diff-index` and `diff-files` never refresh the index (porcelain
//!   `git diff` does, `GIT_OPTIONAL_LOCKS` or not — `crate::reads`), `--full-index` makes
//!   git hash the working-tree side for its `index` line without writing the object, and
//!   `diff --no-index` reads no index at all.
//! - `diff-files` and `diff --no-index` run the clean filter driver of the path, through
//!   git, with the read's environment (`crate::process`) and what git adds for a filter;
//!   git runs it once for the diff and once for the hash of its `index` line (a `-w` read
//!   four times on git 2.56.0, twice on 2.30.9 and 2.32.7; a raw-only read, none). The driver is
//!   the user's program, and what it does is its own — git-lfs's clean stores the object in
//!   `.git/lfs/objects`, exactly as it does under the user's `git diff`. `diff-index
//!   --cached` reads only objects and runs none.
//! - `diff-files` and `diff-index` read the index, so they run the repository's
//!   `core.fsmonitor` like every read with a working tree; and for a submodule whose
//!   checkout git must look into, `diff-files` runs `git status` inside it, which may run
//!   that repository's own fsmonitor and clean filters — what the user's `git diff` runs.
//!   `--no-ext-diff` and `--no-textconv` keep every other program off.
//!
//! **Where plumbing and porcelain differ, and what is passed for it.** Plumbing reads
//! neither `diff.algorithm` (passed, as for a commit) nor `diff.ignoreSubmodules`, which
//! porcelain applies to a submodule with no `ignore` of its own (passed by the caller as
//! `--ignore-submodules`, which plumbing honours, from `crate::diff`); and `diff-index
//! --cached` lists an intent-to-add entry as an empty file added where `git diff --cached`
//! lists nothing, which the caller decides from the index before asking. Rename detection
//! is off in plumbing and cannot pair one path anyway; `--no-renames` says so.
//!
//! **Exit status.** `diff-index` and `diff-files` exit 0 with an answer. `diff --no-index`
//! exits 1 when the two sides differ — always, against `/dev/null` — and also 1 when it
//! cannot read the file, printing nothing: so its status 1 is an answer only when a record
//! for the path came with it, and is otherwise git's failure, carried as
//! [`Error::GitFailed`]. A required clean filter that fails is git's `fatal`, 128, on all
//! three; one that is not required makes git fall back to the unfiltered content with an
//! `error:` on stderr and exit 0 — git's own answer, which Cairn shows as git does, without
//! the warning, since stderr is prose it never parses.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt as _;

use cairn_model::{ChangedFile, Oid, RepoPath};

use super::Algorithm;
use super::patches::{Parser, PatchText};
use crate::ops::GitBinary;
use crate::{Cancel, Error, Repository};

/// Which of a path's working-tree diffs git is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side<'a> {
    /// `git diff-index --cached <commit>`: the commit (`HEAD`, or the empty tree on an
    /// unborn branch) against the index.
    Staged { commit: &'a Oid },
    /// `git diff-files`: the index against the working tree.
    Unstaged,
    /// `git diff --no-index /dev/null <path>`: nothing against the working tree.
    Untracked,
}

/// One working-tree read.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WorkingTreeQuery<'a> {
    pub(crate) side: Side<'a>,
    pub(crate) path: &'a RepoPath,
    /// The context the view groups at; raised to one, never zero.
    pub(crate) context: u32,
    /// `None` passes no algorithm, so git applies a diff driver's own.
    pub(crate) algorithm: Option<Algorithm>,
    pub(crate) ignore_whitespace: bool,
    /// `--ignore-submodules=<value>`, where porcelain would apply a value plumbing does not
    /// read. Not passed to `diff --no-index`, which has no submodule to ignore.
    pub(crate) ignore_submodules: Option<&'static str>,
    /// Only the raw record: `-p` left off, so git reads no content.
    pub(crate) raw_only: bool,
    /// The most stdout taken before git is ended and the answer is
    /// [`WorkingTreeAnswer::PastCeiling`].
    pub(crate) ceiling: usize,
}

/// What git answered about the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkingTreeAnswer {
    /// git listed nothing: its diff of the path is empty.
    Unlisted,
    /// git's one record for the path, and the patch sections it printed for it — none for a
    /// file listed with no content change (a stat-only change `diff-files` lists, or, under
    /// `-w` on git before 2.4x, a change of whitespace alone), one for a change, two for a
    /// type change (a deletion, then an addition).
    Listed {
        file: ChangedFile,
        sections: Vec<PatchText>,
    },
    /// git printed more than the ceiling and was ended. The record, which comes before any
    /// patch, when it arrived whole.
    PastCeiling { file: Option<ChangedFile> },
}

/// Asks git for one path's working-tree diff. `cancel` is polled while git runs: a
/// superseded query ends the process and answers [`Error::ContentCancelled`], and one
/// already superseded starts nothing. A failure is [`Error::GitFailed`], with git's own
/// diagnostic — a required clean filter that failed among them, naming the path — and
/// output this parser does not know is [`Error::UnexpectedGitOutput`].
pub(crate) fn working_tree_patch(
    git: &GitBinary,
    repo: &Repository,
    query: &WorkingTreeQuery<'_>,
    cancel: &impl Cancel,
) -> Result<WorkingTreeAnswer, Error> {
    if cancel.is_cancelled() {
        return Err(Error::ContentCancelled);
    }
    let arguments = arguments(query);
    let described = || {
        arguments
            .iter()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut parser = Parser::default();
    // `--no-index` prints the path as it was given, so the record for `-` names `./-`.
    let printed = match query.side {
        Side::Untracked => RepoPath::new(no_index_operand(query.path)),
        Side::Staged { .. } | Side::Unstaged => query.path.clone(),
    };
    let outcome = git
        .read_invocation()
        .in_repository(repo)
        .args(&arguments)
        .start()?
        .finish_within(cancel, query.ceiling, |chunk| parser.push(chunk), |_| {});
    // `diff --no-index` says "the sides differ" with status 1, and "could not read it" with
    // status 1 too; only the first comes with a record (module doc).
    let failure = match outcome {
        Ok(_) => None,
        Err(Error::GitFailed { status, .. })
            if query.side == Side::Untracked && status.code() == Some(1) =>
        {
            Some(outcome)
        }
        Err(Error::GitReadCancelled { .. }) => return Err(Error::ContentCancelled),
        Err(Error::GitOutputTooLarge { .. }) => {
            return Ok(WorkingTreeAnswer::PastCeiling {
                file: parser
                    .records_so_far()
                    .and_then(|files| one_for(files, query.path, &printed).ok().flatten()),
            });
        }
        Err(other) => return Err(other),
    };
    let unexpected = |record: String| Error::UnexpectedGitOutput {
        arguments: described(),
        record,
    };
    let (files, sections) = parser.finish_listing(query.raw_only).map_err(unexpected)?;
    let Some(file) = one_for(files, query.path, &printed).map_err(unexpected)? else {
        if let Some(Err(error)) = failure {
            return Err(error);
        }
        if !sections.is_empty() {
            return Err(unexpected("a patch for no file git listed".to_owned()));
        }
        return Ok(WorkingTreeAnswer::Unlisted);
    };
    if !query.raw_only {
        sections_fit(&file, sections.len()).map_err(unexpected)?;
    }
    Ok(WorkingTreeAnswer::Listed { file, sections })
}

/// The one record git printed for `path` — under `printed`, the spelling git was given,
/// and named `path` again — if it printed one; `Err` for a record about any other path, or
/// a second one.
fn one_for(
    files: Vec<ChangedFile>,
    path: &RepoPath,
    printed: &RepoPath,
) -> Result<Option<ChangedFile>, String> {
    let mut files = files.into_iter();
    let first = files.next();
    if files.next().is_some() {
        return Err("more than one record for one path".to_owned());
    }
    match first {
        Some(mut file) if &file.new_path == printed && &file.old_path == printed => {
            file.new_path = path.clone();
            file.old_path = path.clone();
            Ok(Some(file))
        }
        Some(file) => Err(format!("a record for {}", file.new_path)),
        None => Ok(None),
    }
}

/// The path as `git diff --no-index` is given it: as itself, except `-`, which git reads
/// as standard input and whose own spelling, in git's advice (`diff-no-index.c`), is
/// `./-`. Only `-` is respelled: git passes the operand to the clean filter driver as its
/// `%f`, so a path given as `./<path>` would hand the driver a name the user's own `git
/// diff --no-index -- /dev/null <path>` does not (reproduced with git 2.30.9 and 2.56.0),
/// where `./-` is the spelling the user must type too. git prints the operand verbatim, on
/// the raw record and the patch headers alike (2.30.9, 2.32.7 and 2.56.0), and reads the
/// attributes of `./<path>` as those of `<path>`.
fn no_index_operand(path: &RepoPath) -> Vec<u8> {
    if path.as_bytes() == b"-" {
        b"./-".to_vec()
    } else {
        path.as_bytes().to_vec()
    }
}

/// Whether `count` sections are what git prints for `file`: two for a type change, one for
/// anything else that changed the content or the mode, and none only where nothing but the
/// stat (or, under `-w`, the whitespace) moved.
fn sections_fit(file: &ChangedFile, count: usize) -> Result<(), String> {
    use cairn_model::ChangeStatus;
    let fits = match file.status {
        ChangeStatus::TypeChanged => count == 2,
        // None: `diff-files` lists a file whose stat moved and content did not, and git
        // before 2.4x lists one whose every change `-w` ignores; either prints nothing.
        ChangeStatus::Modified if !file.mode_changed() => count <= 1,
        ChangeStatus::Added
        | ChangeStatus::Deleted
        | ChangeStatus::Modified
        | ChangeStatus::Renamed(_)
        | ChangeStatus::Copied(_) => count == 1,
    };
    if fits {
        Ok(())
    } else {
        Err(format!(
            "{count} patch sections for a {:?} of {}",
            file.status, file.new_path
        ))
    }
}

/// What porcelain `git diff --no-index` reads of the user's configuration and plumbing
/// does not, set back to git's defaults on that invocation alone: each changes only how
/// the answer is printed — the header's path prefixes and quoting, how hunks are joined
/// and ordered, a path made relative — never what git diffs or how. Found by experiment
/// on git 2.30.9, 2.32.7 and 2.56.0: of every diff, colour and core key tried, these (and
/// `diff.srcPrefix`/`diff.dstPrefix`, from 2.45) change its output, and the rest are
/// already decided by a flag (`-U`, `--no-color`, `--no-ext-diff`, `--no-textconv`,
/// `--no-abbrev`, `--full-index`); with them set, the output is byte-identical under every
/// one set hostile at once. Kept, as parity with the user's `git diff --no-index`: the
/// conversion keys (`core.autocrlf`, `core.eol`, `core.safecrlf`, the attributes, the
/// filter drivers), which decide git's form of the file, and the algorithm keys, which
/// cannot move a change that is every line of the file.
const NO_INDEX_PRESENTATION: [&str; 9] = [
    "diff.noprefix=false",
    "diff.mnemonicPrefix=false",
    "diff.srcPrefix=a/",
    "diff.dstPrefix=b/",
    "core.quotePath=true",
    "diff.interHunkContext=0",
    "diff.relative=false",
    "diff.orderFile=/dev/null",
    "diff.suppressBlankEmpty=false",
];

fn arguments(query: &WorkingTreeQuery<'_>) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = Vec::new();
    if query.side == Side::Untracked {
        for setting in NO_INDEX_PRESENTATION {
            arguments.extend(["-c", setting].map(OsString::from));
        }
    } else {
        // Plumbing reads none of `NO_INDEX_PRESENTATION` but this one.
        arguments.extend(["-c", "diff.suppressBlankEmpty=false"].map(OsString::from));
    }
    match query.side {
        Side::Staged { .. } => {
            arguments.extend(["diff-index", "--cached", "--no-renames"].map(OsString::from));
        }
        Side::Unstaged => {
            arguments.extend(["diff-files", "--no-renames"].map(OsString::from));
        }
        Side::Untracked => arguments.extend(["diff", "--no-index"].map(OsString::from)),
    }
    arguments.extend(["-z", "--raw", "--no-abbrev"].map(OsString::from));
    if !query.raw_only {
        arguments.extend(["-p", "--full-index"].map(OsString::from));
        arguments.push(format!("-U{}", query.context.max(1)).into());
    }
    arguments.extend(["--no-ext-diff", "--no-textconv", "--no-color"].map(OsString::from));
    if query.ignore_whitespace {
        arguments.push("-w".into());
    }
    if let Some(algorithm) = query.algorithm {
        arguments.push(algorithm.flag().into());
    }
    if let Some(value) = query.ignore_submodules
        && query.side != Side::Untracked
    {
        arguments.push(format!("--ignore-submodules={value}").into());
    }
    match query.side {
        Side::Staged { commit } => {
            arguments.push("--end-of-options".into());
            arguments.push(commit.to_string().into());
            arguments.push("--".into());
            arguments.extend(pathspec(query.path));
        }
        Side::Unstaged => {
            arguments.push("--".into());
            arguments.extend(pathspec(query.path));
        }
        Side::Untracked => {
            arguments.extend(["--", "/dev/null"].map(OsString::from));
            arguments.push(OsString::from_vec(no_index_operand(query.path)));
        }
    }
    arguments
}

/// The path itself, read literally, and nothing under it: a directory of that name on the
/// other side is a different path. The exclusion is a glob of the path's contents with the
/// path's own glob characters escaped — `:(exclude,literal)<path>/` would also exclude a
/// gitlink at the path, which git matches as a directory (reproduced with git 2.30.9 and
/// 2.56.0).
fn pathspec(path: &RepoPath) -> [OsString; 2] {
    let mut exact = b":(literal)".to_vec();
    exact.extend_from_slice(path.as_bytes());
    let mut under = b":(exclude,glob)".to_vec();
    for byte in path.as_bytes() {
        if matches!(byte, b'\\' | b'*' | b'?' | b'[') {
            under.push(b'\\');
        }
        under.push(*byte);
    }
    under.extend_from_slice(b"/**");
    [OsString::from_vec(exact), OsString::from_vec(under)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{ChangeStatus, FileMode, split_lines};

    const COMMIT: &str = "07da224c7ec04501dfb451be161fa962effe1dc1";

    fn strings(arguments: Vec<OsString>) -> Vec<String> {
        arguments
            .into_iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }

    fn query<'a>(side: Side<'a>, path: &'a RepoPath) -> WorkingTreeQuery<'a> {
        WorkingTreeQuery {
            side,
            path,
            context: 3,
            algorithm: Some(Algorithm::Myers),
            ignore_whitespace: false,
            ignore_submodules: None,
            raw_only: false,
            ceiling: 1 << 20,
        }
    }

    /// Each read is plumbing (or `diff --no-index`, which reads no index), never runs a
    /// textconv or an external diff, prints a patch with full ids, and names exactly the
    /// path asked about. Caught by: porcelain `git diff` for a tracked path (it refreshes
    /// the index), `--textconv` or `--ext-diff`, `-U0`, a pathspec that would also match a
    /// directory's contents or read `*` as a glob, or `-a` (git decides what is binary).
    #[test]
    fn each_read_is_plumbing_that_names_exactly_its_path_and_runs_no_program() {
        let commit = Oid::parse(COMMIT).unwrap();
        let path = RepoPath::new("dir/*.txt");
        let staged = strings(arguments(&query(Side::Staged { commit: &commit }, &path)));
        assert_eq!(
            staged,
            [
                "-c",
                "diff.suppressBlankEmpty=false",
                "diff-index",
                "--cached",
                "--no-renames",
                "-z",
                "--raw",
                "--no-abbrev",
                "-p",
                "--full-index",
                "-U3",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--diff-algorithm=myers",
                "--end-of-options",
                COMMIT,
                "--",
                ":(literal)dir/*.txt",
                r":(exclude,glob)dir/\*.txt/**",
            ]
        );
        let unstaged = strings(arguments(&query(Side::Unstaged, &path)));
        assert_eq!(unstaged[2..4], ["diff-files", "--no-renames"]);
        assert_eq!(
            unstaged[unstaged.len() - 3..],
            ["--", ":(literal)dir/*.txt", r":(exclude,glob)dir/\*.txt/**"]
        );
        let untracked = strings(arguments(&query(Side::Untracked, &path)));
        let settings = NO_INDEX_PRESENTATION.len() * 2;
        assert_eq!(untracked[settings..settings + 2], ["diff", "--no-index"]);
        for setting in NO_INDEX_PRESENTATION {
            assert!(
                untracked
                    .windows(2)
                    .any(|pair| pair[0] == "-c" && pair[1] == setting),
                "{setting} is not set on the --no-index read: {untracked:?}"
            );
        }
        assert_eq!(
            untracked[untracked.len() - 3..],
            ["--", "/dev/null", "dir/*.txt"]
        );
        let dash = RepoPath::new("-");
        let stdin_named = strings(arguments(&query(Side::Untracked, &dash)));
        assert_eq!(
            stdin_named[stdin_named.len() - 3..],
            ["--", "/dev/null", "./-"],
            "`-` alone is standard input to --no-index"
        );
        for read in [&staged, &unstaged, &untracked] {
            for refused in [
                "--textconv",
                "--ext-diff",
                "-a",
                "--text",
                "-U0",
                "--literal-pathspecs",
            ] {
                assert!(!read.iter().any(|a| a == refused), "{refused} in {read:?}");
            }
            assert!(!read.iter().any(|a| a == "diff") || read.iter().any(|a| a == "--no-index"));
        }
    }

    /// `-w`, an algorithm left to a driver, porcelain's submodule rule and a raw-only read
    /// are each spelled as git takes them, and `diff --no-index` gets no submodule rule.
    #[test]
    fn the_optional_flags_are_spelled_as_git_takes_them() {
        let path = RepoPath::new("sub");
        let mut asked = query(Side::Unstaged, &path);
        asked.ignore_whitespace = true;
        asked.algorithm = None;
        asked.ignore_submodules = Some("dirty");
        asked.context = 0;
        let read = strings(arguments(&asked));
        assert!(read.iter().any(|a| a == "-w"));
        assert!(
            read.iter().any(|a| a == "-U1"),
            "context zero is asked at one"
        );
        assert!(read.iter().any(|a| a == "--ignore-submodules=dirty"));
        assert!(!read.iter().any(|a| a.starts_with("--diff-algorithm")));
        asked.side = Side::Untracked;
        let read = strings(arguments(&asked));
        assert!(!read.iter().any(|a| a.starts_with("--ignore-submodules")));
        asked.side = Side::Unstaged;
        asked.raw_only = true;
        let read = strings(arguments(&asked));
        assert!(
            !read.iter().any(|a| a == "-p" || a.starts_with("-U")),
            "{read:?}"
        );
    }

    fn listing(output: &[u8]) -> (Vec<ChangedFile>, Vec<PatchText>) {
        let mut parser = Parser::default();
        parser.push(output);
        parser.finish_listing(false).unwrap()
    }

    const NULL: &str = "0000000000000000000000000000000000000000";
    const OLD: &str = "e044abc2547e25c86f7112a2bb01d67353dad83b";
    const NEW: &str = "10cb2af7ad1c280275c3a6e4253744c740c2845f";

    /// `diff-files`' own answer for an edit: the record names no new id (git does not hash
    /// the working tree for it), the `index` line does — and the new side is rebuilt from
    /// the old side and the patch, git's lines and only git's. Caught by: a new side read
    /// from anywhere but the patch, a line outside a hunk not taken from the old side, or
    /// the null id taken for an object.
    #[test]
    fn the_new_side_is_rebuilt_from_the_old_side_and_gits_patch() {
        let output = format!(
            ":100644 100644 {OLD} {NULL} M\0plain.txt\0\0diff --git a/plain.txt b/plain.txt\n\
             index {OLD}..{NEW} 100644\n--- a/plain.txt\n+++ b/plain.txt\n\
             @@ -2,3 +2,3 @@\n two\n-three\n+THREE\n four\n"
        );
        let (files, sections) = listing(output.as_bytes());
        let plain = RepoPath::new("plain.txt");
        let file = one_for(files, &plain, &plain).unwrap().unwrap();
        assert_eq!(file.new_id.map(|id| id.to_string()), Some(NULL.to_owned()));
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].new_index_id(), Some(Oid::parse(NEW).unwrap()));
        let old = split_lines(b"one\ntwo\nthree\nfour\nfive");
        let (new, reading) = sections[0].new_side(&old).unwrap();
        assert_eq!(new, split_lines(b"one\ntwo\nTHREE\nfour\nfive"));
        assert_eq!(reading.changes.len(), 1);

        let other_old = split_lines(b"one\ntwo\nthree!\nfour\nfive");
        assert!(
            sections[0].new_side(&other_old).is_err(),
            "a removed line that is not the old side's was accepted"
        );
    }

    /// An addition — an intent-to-add path, or an untracked file through `--no-index` — is
    /// all of its new side in the patch, a deletion none of it, and a file with no final
    /// newline keeps git's marker. Caught by: a line before the first hunk invented, or an
    /// unterminated last line read as terminated.
    #[test]
    fn an_addition_and_a_deletion_rebuild_from_their_one_side() {
        let added = format!(
            ":000000 100644 {NULL} {NULL} A\0new.txt\0\0diff --git a/new.txt b/new.txt\n\
             new file mode 100644\nindex {NULL}..{NEW}\n--- /dev/null\n+++ b/new.txt\n\
             @@ -0,0 +1,2 @@\n+LOWER\n+CASE\n\\ No newline at end of file\n"
        );
        let (files, sections) = listing(added.as_bytes());
        assert_eq!(files[0].status, ChangeStatus::Added);
        let (new, reading) = sections[0].new_side(&[]).unwrap();
        assert_eq!(new, split_lines(b"LOWER\nCASE"));
        assert_eq!(reading.changes.len(), 1);

        let deleted = format!(
            ":100644 000000 {OLD} {NULL} D\0gone.txt\0\0diff --git a/gone.txt b/gone.txt\n\
             deleted file mode 100644\nindex {OLD}..{NULL}\n--- a/gone.txt\n+++ /dev/null\n\
             @@ -1 +0,0 @@\n-gone\n"
        );
        let (_, sections) = listing(deleted.as_bytes());
        assert_eq!(
            sections[0].new_index_id(),
            None,
            "the null id names nothing"
        );
        let (new, _) = sections[0].new_side(&split_lines(b"gone\n")).unwrap();
        assert!(new.is_empty());
    }

    /// A submodule's sides are git's `Subproject commit` lines, the new one carrying
    /// `-dirty` when the checkout has changes of its own. Caught by: the mark read as part
    /// of the id (no id parses), or taken from the old side.
    #[test]
    fn a_submodules_commits_and_dirtiness_are_read_from_gits_lines() {
        let a = "ebc73fff2b3c4f0a202cd874e6369e8ba912fcb6";
        let b = "3848932be0a7c21278c1a299d711cc0ebed1f7af";
        let output = format!(
            ":160000 160000 {a} {NULL} M\0sub\0\0diff --git a/sub b/sub\nindex {a}..{b} 160000\n\
             --- a/sub\n+++ b/sub\n@@ -1 +1 @@\n-Subproject commit {a}\n\
             +Subproject commit {b}-dirty\n"
        );
        let (files, sections) = listing(output.as_bytes());
        assert_eq!(files[0].new_mode, Some(FileMode::Submodule));
        assert_eq!(
            sections[0].submodule_targets(),
            (
                Some(Oid::parse(a).unwrap()),
                Some(Oid::parse(b).unwrap()),
                true
            )
        );
        let clean = output.replace("-dirty", "");
        let (_, sections) = listing(clean.as_bytes());
        assert!(!sections[0].submodule_targets().2);
    }

    /// One path, one record: a second record, or one for another path, is refused, and so
    /// is a section count git does not print for the record's kind of change — except no
    /// section for a modification that kept its mode, which is a stat-only change.
    #[test]
    fn one_path_has_at_most_one_record_with_the_sections_its_change_prints() {
        let path = RepoPath::new("d");
        let record = |status: ChangeStatus, name: &str| ChangedFile {
            status,
            old_path: RepoPath::new(name),
            new_path: RepoPath::new(name),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
            old_id: None,
            new_id: None,
        };
        assert!(one_for(vec![record(ChangeStatus::Added, "d/x")], &path, &path).is_err());
        assert!(
            one_for(
                vec![
                    record(ChangeStatus::Added, "d"),
                    record(ChangeStatus::Deleted, "d")
                ],
                &path,
                &path
            )
            .is_err()
        );
        assert_eq!(one_for(Vec::new(), &path, &path), Ok(None));
        // `-` is given to `--no-index` as `./-`, and its record is named `-` again; a
        // record for `-` itself is not the one asked for.
        let dash = RepoPath::new("-");
        let spelled = RepoPath::new(no_index_operand(&dash));
        assert_eq!(spelled, RepoPath::new("./-"));
        let named = one_for(vec![record(ChangeStatus::Added, "./-")], &dash, &spelled);
        assert_eq!(
            named.map(|file| file.map(|file| file.new_path)),
            Ok(Some(dash.clone()))
        );
        assert!(one_for(vec![record(ChangeStatus::Added, "-")], &dash, &spelled).is_err());
        assert_eq!(no_index_operand(&RepoPath::new("-x")), b"-x");
        assert_eq!(no_index_operand(&RepoPath::new("d/-")), b"d/-");

        let modified = record(ChangeStatus::Modified, "d");
        assert!(sections_fit(&modified, 0).is_ok(), "a stat-only change");
        assert!(sections_fit(&modified, 1).is_ok());
        assert!(sections_fit(&modified, 2).is_err());
        let typed = record(ChangeStatus::TypeChanged, "d");
        assert!(sections_fit(&typed, 2).is_ok());
        assert!(sections_fit(&typed, 1).is_err());
        let mut moded = record(ChangeStatus::Modified, "d");
        moded.new_mode = Some(FileMode::Executable);
        assert!(
            sections_fit(&moded, 0).is_err(),
            "a mode change prints its header"
        );
        assert!(sections_fit(&record(ChangeStatus::Added, "d"), 0).is_err());
    }
}
