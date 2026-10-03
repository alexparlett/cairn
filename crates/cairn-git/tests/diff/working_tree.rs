//! C7: one path's staged, unstaged and untracked diffs are what the user's `git diff
//! --cached`, `git diff` and `git diff --no-index /dev/null` show, on fixtures that make a
//! working tree differ from git's form of it — a `text=auto` file with CRLF endings, a path
//! under a clean filter driver the fixture writes — and every state of R3.4 is answered,
//! with the git directory byte-identical after every query (R3.5).
//!
//! The reference is porcelain, run with the fixture's own configuration and nothing of the
//! machine's (`Repo::run`), and only AFTER Cairn's queries and the snapshot taken around
//! them, since porcelain `git diff` refreshes the index it reads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cairn_git::{
    CancelSignal, ContentOptions, Error, Repository, SharedRepository, WorkingTreeDiff,
};
use cairn_model::{
    ChangeStatus, Context, DiffContent, DiffLimits, DiffLine, FileDiff, FileMode, RepoPath,
    SizeLimit,
};

use super::parity::{Hunk, cairn_view, hunks_of};
use super::repositories::Repo;
use super::{ok, some};

const EVERY: [WorkingTreeDiff; 3] = [
    WorkingTreeDiff::Staged,
    WorkingTreeDiff::Unstaged,
    WorkingTreeDiff::Untracked,
];

fn engine(repo: &Repo) -> Repository {
    ok(Repository::discover(repo.path()), "the fixture opens")
}

fn ask(repo: &Repo, path: &str, which: WorkingTreeDiff) -> Option<FileDiff> {
    ask_with(repo, path, which, &ContentOptions::default())
}

fn ask_with(
    repo: &Repo,
    path: &str,
    which: WorkingTreeDiff,
    options: &ContentOptions,
) -> Option<FileDiff> {
    ok(
        engine(repo).working_tree_diff(
            super::git(),
            &RepoPath::new(path),
            which,
            options,
            &CancelSignal::new(),
        ),
        &format!("the {which:?} diff of {path}"),
    )
}

/// Porcelain's answer, whatever its exit status (`--no-index` exits 1 with an answer).
fn porcelain(repo: &Repo, args: &[&str]) -> String {
    let (status, stdout, stderr) = repo.run(args, &[], None);
    assert!(
        status.code().is_some_and(|code| code <= 1),
        "git {args:?}: {status} {stderr}"
    );
    stdout
}

/// What the user's `git diff` prints for `path`: `--cached`, plain, or `--no-index` against
/// `/dev/null`, at three lines of context, with `extra` flags.
fn git_diff(repo: &Repo, path: &str, which: WorkingTreeDiff, extra: &[&str]) -> String {
    let mut args = vec![
        "diff",
        "-U3",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ];
    args.extend_from_slice(extra);
    match which {
        WorkingTreeDiff::Staged => args.extend(["--cached", "--", path]),
        WorkingTreeDiff::Unstaged => args.extend(["--", path]),
        // A path that is `-` is standard input to `--no-index`; git's own advice is to spell
        // it `./-`, which is what the user types.
        WorkingTreeDiff::Untracked => {
            let spelled = if path == "-" { "./-" } else { path };
            args.extend(["--no-index", "--", "/dev/null", spelled]);
        }
    }
    porcelain(repo, &args)
}

/// Every file under `dir`, by path, with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![dir.to_owned()];
    while let Some(next) = pending.pop() {
        for entry in ok(std::fs::read_dir(&next), "a directory reads") {
            let path = ok(entry, "an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.clone(), ok(std::fs::read(&path), "a file reads"));
            }
        }
    }
    files
}

/// Asks every query in `queries`, with whitespace and without, checking the git directory
/// byte for byte after each: no refreshed index, no object, no ref.
fn answers_writing_nothing(
    repo: &Repo,
    queries: &[(&str, WorkingTreeDiff)],
) -> Vec<Option<FileDiff>> {
    let git_dir = repo.path().join(".git");
    let before = snapshot(&git_dir);
    let mut answers = Vec::new();
    for (path, which) in queries {
        for ignore_whitespace in [false, true] {
            let options = ContentOptions {
                ignore_whitespace,
                ..ContentOptions::default()
            };
            let answer = ask_with(repo, path, *which, &options);
            assert!(
                snapshot(&git_dir) == before,
                "the {which:?} diff of {path} changed the git directory"
            );
            if !ignore_whitespace {
                answers.push(answer);
            }
        }
    }
    answers
}

/// Cairn's answer and git's, hunk for hunk with the function context; `None` is git
/// printing nothing at all.
fn same_as_git(repo: &Repo, path: &str, which: WorkingTreeDiff, answer: &Option<FileDiff>) {
    for whitespace in [false, true] {
        let extra: &[&str] = if whitespace { &["-w"] } else { &[] };
        let theirs = git_diff(repo, path, which, extra);
        let ours = match answer {
            None => {
                assert_eq!(
                    theirs, "",
                    "{which:?} {path}: Cairn says no change, git does not"
                );
                continue;
            }
            Some(diff) => diff,
        };
        if whitespace && ours.text().is_none() {
            continue;
        }
        let answered = if whitespace {
            ok(
                engine(repo).working_tree_diff(
                    super::git(),
                    &RepoPath::new(path),
                    which,
                    &ContentOptions {
                        ignore_whitespace: true,
                        ..ContentOptions::default()
                    },
                    &CancelSignal::new(),
                ),
                "the whitespace-ignoring diff",
            )
        } else {
            answer.clone()
        };
        let diff = some(answered, "an answer under -w too");
        let git_hunks = hunks_of(&theirs);
        let all_whitespace = whitespace && git_hunks.is_empty();
        let cairn: Vec<Hunk> = if all_whitespace {
            let overlay = some(diff.overlay(), "an overlay");
            assert!(
                some(
                    overlay.changes_ignoring_whitespace(),
                    "the whitespace-ignoring ranges"
                )
                .is_empty(),
                "{which:?} {path}: git -w shows nothing, Cairn shows changes"
            );
            Vec::new()
        } else {
            cairn_view(&diff, Context::lines(3), whitespace)
        };
        assert_eq!(
            cairn,
            git_hunks,
            "{which:?} {path}{}: Cairn's hunks are not git diff's",
            if whitespace { " -w" } else { "" }
        );
    }
}

fn lines(diff: &FileDiff) -> (Vec<String>, Vec<String>) {
    let text = some(diff.text(), "a text answer");
    let show = |side: &[DiffLine]| {
        side.iter()
            .map(|line| String::from_utf8_lossy(line.bytes()).into_owned())
            .collect()
    };
    (show(text.old_lines()), show(text.new_lines()))
}

fn script(repo: &Repo, name: &str, body: &str) -> PathBuf {
    let path = repo.path().join(name);
    ok(std::fs::write(&path, body), "writing a script");
    repo.chmod(name, 0o755);
    path
}

/// A repository whose `.gitattributes` and helper scripts are committed outside the paths
/// the tests read, and whose working tree is then changed by `change`.
fn base(name: &str) -> Repo {
    let repo = Repo::new(name);
    repo.write("seed", b"seed\n");
    repo.commit("seed");
    repo
}

/// C7, the case that silently regresses: a `text=auto` file whose working tree has CRLF
/// where the index has LF is no change at all — not every line changed — staged or not;
/// an edit in it is one line, held in git's form (no `\r`); and an untracked CRLF file is
/// added in that form too, as `git diff --no-index` converts it. Caught by: reading the
/// working tree's bytes (every line differs by its `\r`), or `None` for the edited file.
#[test]
fn a_text_auto_file_with_crlf_endings_is_no_change_and_an_edit_is_one_line() {
    let repo = base("crlf");
    repo.write(".gitattributes", b"*.txt text=auto\n");
    repo.write("same.txt", b"one\ntwo\nthree\n");
    repo.write("edited.txt", b"one\ntwo\nthree\n");
    repo.commit("lf in the index");
    repo.write("same.txt", b"one\r\ntwo\r\nthree\r\n");
    repo.write("edited.txt", b"one\r\nTWO\r\nthree\r\n");
    repo.write("new.txt", b"u1\r\nu2\r\n");
    let raw = ok(std::fs::read(repo.path().join("same.txt")), "the file");
    assert!(raw.contains(&b'\r'), "the working tree does not hold CRLF");

    let answers = answers_writing_nothing(
        &repo,
        &[
            ("same.txt", WorkingTreeDiff::Unstaged),
            ("same.txt", WorkingTreeDiff::Staged),
            ("edited.txt", WorkingTreeDiff::Unstaged),
            ("new.txt", WorkingTreeDiff::Untracked),
        ],
    );
    assert_eq!(
        answers[0], None,
        "a CRLF checkout of an LF file read as a change"
    );
    assert_eq!(answers[1], None);
    let edited = some(answers[2].clone(), "the edit");
    assert_eq!(lines(&edited).1, ["one", "TWO", "three"], "not git's form");
    let added = some(answers[3].clone(), "the untracked file");
    assert_eq!(lines(&added).1, ["u1", "u2"], "not git's form");
    assert_eq!(added.file.status, ChangeStatus::Added);

    same_as_git(&repo, "same.txt", WorkingTreeDiff::Unstaged, &answers[0]);
    same_as_git(&repo, "edited.txt", WorkingTreeDiff::Unstaged, &answers[2]);
    same_as_git(&repo, "new.txt", WorkingTreeDiff::Untracked, &answers[3]);
}

/// A fixture-written clean filter that upper-cases, leaves a mark and records its
/// environment.
fn upper_filter(repo: &Repo, driver: &str) -> PathBuf {
    let marks = repo.path().join(format!("{driver}-ran"));
    let body = format!(
        "#!/bin/sh\nenv >> '{}'\necho ran >> '{}'\nexec tr 'a-z' 'A-Z'\n",
        repo.path().join(format!("{driver}-env")).display(),
        marks.display()
    );
    let program = script(repo, &format!("{driver}.sh"), &body);
    repo.config(
        &format!("filter.{driver}.clean"),
        &program.display().to_string(),
    );
    repo.config(&format!("filter.{driver}.smudge"), "cat");
    marks
}

/// C7, the filter: a path under a clean filter driver the fixture writes is diffed in the
/// driver's form, which is distinctive — the working tree holds lower case, git and Cairn
/// see upper case — staged, unstaged and untracked, each as `git diff` shows it. The driver
/// ran (its mark), through git (its environment is a read's: `GIT_OPTIONAL_LOCKS=0`, no
/// terminal prompt, and nothing of the test process's own beyond the roster), and the
/// lines Cairn holds are its output. Caught by: reading the working tree's bytes (lower
/// case), a filter run by gix in Cairn's own environment (the cargo variable leaks in), or
/// the filter not run at all (no mark).
#[test]
fn a_clean_filter_drivers_form_is_what_is_diffed_and_it_runs_under_git() {
    let repo = base("filter");
    let marks = upper_filter(&repo, "upper");
    repo.write(".gitattributes", b"*.up filter=upper\n");
    repo.write("tracked.up", b"hello\nworld\nfoo\n");
    repo.commit("filtered");
    repo.write("tracked.up", b"hello\nworld\nbar\n");
    repo.write("staged.up", b"one\ntwo\n");
    repo.git(&["add", "staged.up"]);
    repo.write("untracked.up", b"lower\ncase\n");
    let _ = std::fs::remove_file(&marks);
    let _ = std::fs::remove_file(repo.path().join("upper-env"));

    let answers = answers_writing_nothing(
        &repo,
        &[
            ("tracked.up", WorkingTreeDiff::Unstaged),
            ("staged.up", WorkingTreeDiff::Staged),
            ("untracked.up", WorkingTreeDiff::Untracked),
        ],
    );
    assert!(marks.exists(), "the clean filter never ran");
    let tracked = some(answers[0].clone(), "the filtered edit");
    assert_eq!(lines(&tracked).0, ["HELLO", "WORLD", "FOO"]);
    assert_eq!(
        lines(&tracked).1,
        ["HELLO", "WORLD", "BAR"],
        "the working tree's own bytes, not the filter's form"
    );
    let untracked = some(answers[2].clone(), "the untracked file");
    assert_eq!(lines(&untracked).1, ["LOWER", "CASE"]);
    let environment = ok(
        std::fs::read_to_string(repo.path().join("upper-env")),
        "the filter's environment",
    );
    for set in [
        "GIT_OPTIONAL_LOCKS=0",
        "GIT_TERMINAL_PROMPT=0",
        "GIT_NO_LAZY_FETCH=1",
    ] {
        assert!(
            environment.lines().any(|line| line == set),
            "the filter did not run under a read's environment ({set} missing)"
        );
    }
    if std::env::var_os("CARGO_MANIFEST_DIR").is_some() {
        assert!(
            !environment.contains("CARGO_MANIFEST_DIR="),
            "the filter saw the test process's own environment, not the one Cairn built"
        );
    }

    // The id Cairn reports for the working-tree side is the object git would store. (Run
    // after the environment is read: it runs the filter in this process's environment.)
    let stored = repo.git(&["hash-object", "--path", "untracked.up", "untracked.up"]);
    assert_eq!(
        untracked.file.new_id.map(|id| id.to_string()),
        Some(stored.trim().to_owned())
    );

    same_as_git(&repo, "tracked.up", WorkingTreeDiff::Unstaged, &answers[0]);
    same_as_git(&repo, "staged.up", WorkingTreeDiff::Staged, &answers[1]);
    same_as_git(
        &repo,
        "untracked.up",
        WorkingTreeDiff::Untracked,
        &answers[2],
    );
}

/// A long-running filter process (`filter.<driver>.process`, gitattributes' "Long Running
/// Filter Process", what `git lfs install` configures): a pkt-line server in POSIX `sh` and
/// `dd`, upper-casing what git sends it and recording each command. It advertises clean and
/// smudge, and records every command git sends after the handshake.
const PROCESS_FILTER: &str = r#"#!/bin/sh
log="$1"; work="$2"
read_pkt() {
  hex=$(dd bs=1 count=4 2>/dev/null)
  [ -z "$hex" ] && exit 0
  if [ "$hex" = "0000" ]; then len=-1; return; fi
  len=$(( 0x$hex - 4 ))
  dd bs=1 count=$len 2>/dev/null > "$work/pkt"
}
write_text() { printf '%04x%s\n' $(( ${#1} + 5 )) "$1"; }
flush() { printf '0000'; }
read_pkt; read_pkt; read_pkt
write_text git-filter-server; write_text version=2; flush
while read_pkt && [ "$len" -ge 0 ]; do :; done
write_text capability=clean; write_text capability=smudge; flush
while :; do
  read_pkt; cat "$work/pkt" >> "$log"
  while read_pkt && [ "$len" -ge 0 ]; do :; done
  : > "$work/in"
  while read_pkt && [ "$len" -ge 0 ]; do cat "$work/pkt" >> "$work/in"; done
  tr 'a-z' 'A-Z' < "$work/in" > "$work/out"
  write_text status=success; flush
  size=$(wc -c < "$work/out")
  if [ "$size" -gt 0 ]; then printf '%04x' $(( size + 4 )); cat "$work/out"; fi
  flush; flush
done
"#;

/// D1 as amended, for the driver git-lfs installs: a `filter.<driver>.process` runs on the
/// reads of the working tree — `diff-files` and `diff --no-index` — and git sends it
/// `command=clean` and nothing else, so its form is what is diffed, as `git diff` shows
/// it; a staged read starts it not at all. Caught by: a working-tree side read without
/// git's conversion (lower case), or a read that makes git smudge.
#[test]
fn a_long_running_filter_process_is_sent_only_clean_and_its_form_is_diffed() {
    let repo = base("process-filter");
    let program = script(&repo, "process.sh", PROCESS_FILTER);
    let work = repo.path().join("process-filter");
    ok(
        std::fs::create_dir_all(&work),
        "the filter's scratch directory",
    );
    let log = repo.path().join("process.log");
    repo.write("tracked.pf", b"hello\nworld\n");
    repo.commit("before the filter");
    repo.config(
        "filter.p.process",
        &format!("{} {} {}", program.display(), log.display(), work.display()),
    );
    repo.write(".gitattributes", b"*.pf filter=p\n");
    repo.write("staged.pf", b"one\ntwo\n");
    repo.git(&["add", "staged.pf"]);
    repo.write("tracked.pf", b"hello\nthere\n");
    repo.write("untracked.pf", b"lower\ncase\n");
    let _ = std::fs::remove_file(&log);

    let staged = ok(
        engine(&repo).working_tree_diff(
            super::git(),
            &RepoPath::new("staged.pf"),
            WorkingTreeDiff::Staged,
            &ContentOptions::default(),
            &CancelSignal::new(),
        ),
        "the staged diff",
    );
    assert!(!log.exists(), "a staged read started the filter process");
    let answers = answers_writing_nothing(
        &repo,
        &[
            ("tracked.pf", WorkingTreeDiff::Unstaged),
            ("untracked.pf", WorkingTreeDiff::Untracked),
        ],
    );
    let sent = ok(std::fs::read_to_string(&log), "the filter's record");
    assert!(
        !sent.is_empty() && sent.lines().all(|line| line == "command=clean"),
        "git sent the filter process {sent:?}"
    );
    let tracked = some(answers[0].clone(), "the filtered edit");
    assert_eq!(lines(&tracked).1, ["HELLO", "THERE"]);
    let untracked = some(answers[1].clone(), "the untracked file");
    assert_eq!(lines(&untracked).1, ["LOWER", "CASE"]);
    same_as_git(&repo, "tracked.pf", WorkingTreeDiff::Unstaged, &answers[0]);
    same_as_git(
        &repo,
        "untracked.pf",
        WorkingTreeDiff::Untracked,
        &answers[1],
    );
    same_as_git(&repo, "staged.pf", WorkingTreeDiff::Staged, &staged);
}

/// The QA brief's failing driver: a required clean filter that exits non-zero, or whose
/// program does not exist, is git's failure with its diagnostic — `fatal`, status 128, and
/// what git said on stderr — from every read that runs it, never an empty diff; one that
/// is not required is what git does with it, the unfiltered content diffed, exactly as
/// `git diff` shows it. Caught by: a failed read answered as no change, or git's
/// diagnostic dropped from the error (the message names the path anyway, through the
/// arguments, so only the stderr decides that).
#[test]
fn a_failing_clean_filter_is_gits_failure_with_its_diagnostic_or_what_git_shows() {
    let repo = base("failing");
    let failing = script(&repo, "fail.sh", "#!/bin/sh\ncat >/dev/null\nexit 3\n");
    repo.write("required.req", b"x\ny\n");
    repo.write("missing.gone", b"x\ny\n");
    repo.write("optional.opt", b"x\ny\n");
    repo.commit("before the filters");
    repo.config("filter.req.clean", &failing.display().to_string());
    repo.config("filter.req.required", "true");
    repo.config("filter.gone.clean", "/nonexistent/cairn-clean-filter");
    repo.config("filter.gone.required", "true");
    repo.config("filter.opt.clean", &failing.display().to_string());
    repo.write(
        ".gitattributes",
        b"*.req filter=req\n*.gone filter=gone\n*.opt filter=opt\n",
    );
    repo.write("required.req", b"x\nz\n");
    repo.write("missing.gone", b"x\nz\n");
    repo.write("optional.opt", b"x\nz\n");
    repo.write("new.req", b"q\n");
    repo.write("new.gone", b"q\n");

    for (path, which) in [
        ("required.req", WorkingTreeDiff::Unstaged),
        ("new.req", WorkingTreeDiff::Untracked),
        ("missing.gone", WorkingTreeDiff::Unstaged),
        ("new.gone", WorkingTreeDiff::Untracked),
    ] {
        let outcome = engine(&repo).working_tree_diff(
            super::git(),
            &RepoPath::new(path),
            which,
            &ContentOptions::default(),
            &CancelSignal::new(),
        );
        match outcome {
            Err(Error::GitFailed { status, stderr, .. }) => {
                assert_eq!(status.code(), Some(128), "{which:?} {path}: git's fatal");
                assert!(
                    !stderr.trim().is_empty(),
                    "{which:?} {path}: git's diagnostic was dropped"
                );
            }
            other => panic!("{which:?} {path}: expected git's failure, got {other:?}"),
        }
    }
    // A file `--no-index` cannot read exits 1 as a difference does, with no answer: an
    // error, never "no change".
    let missing = engine(&repo).working_tree_diff(
        super::git(),
        &RepoPath::new("missing.txt"),
        WorkingTreeDiff::Untracked,
        &ContentOptions::default(),
        &CancelSignal::new(),
    );
    assert!(
        matches!(missing, Err(Error::GitFailed { .. })),
        "a file that is not there answered {missing:?}"
    );
    let optional = ask(&repo, "optional.opt", WorkingTreeDiff::Unstaged);
    assert_eq!(lines(&some(optional.clone(), "the diff")).1, ["x", "z"]);
    same_as_git(&repo, "optional.opt", WorkingTreeDiff::Unstaged, &optional);
}

/// C7's states that are not text, each answered rather than errored and each what `git
/// diff` says: a deleted file, a type change (a file become a symlink), a mode-only change,
/// and the same three staged. Caught by: a deletion answered as no change, a mode change
/// read as content, or a symlink's target not taken from git.
#[test]
fn a_deleted_file_a_type_change_and_a_mode_change_answer_their_state() {
    let repo = base("states");
    repo.write("gone.txt", b"gone\n");
    repo.write("link.txt", b"was a file\n");
    repo.write("mode.sh", b"echo\n");
    repo.commit("three files");
    repo.remove("gone.txt");
    repo.symlink("link.txt", "mode.sh");
    repo.chmod("mode.sh", 0o755);

    let unstaged = answers_writing_nothing(
        &repo,
        &[
            ("gone.txt", WorkingTreeDiff::Unstaged),
            ("link.txt", WorkingTreeDiff::Unstaged),
            ("mode.sh", WorkingTreeDiff::Unstaged),
        ],
    );
    let gone = some(unstaged[0].clone(), "the deletion");
    assert_eq!(gone.file.status, ChangeStatus::Deleted);
    assert_eq!(lines(&gone), (vec!["gone".to_owned()], Vec::new()));
    let link = some(unstaged[1].clone(), "the type change");
    assert_eq!(link.file.status, ChangeStatus::TypeChanged);
    assert_eq!(link.file.new_mode, Some(FileMode::Symlink));
    assert_eq!(
        lines(&link),
        (vec!["was a file".to_owned()], vec!["mode.sh".to_owned()])
    );
    let mode = some(unstaged[2].clone(), "the mode change");
    assert_eq!(mode.content, DiffContent::ModeChangeOnly);
    assert_eq!(mode.file.new_mode, Some(FileMode::Executable));
    assert_eq!(
        mode.file.old_id, mode.file.new_id,
        "the content did not move"
    );

    same_as_git(&repo, "gone.txt", WorkingTreeDiff::Unstaged, &unstaged[0]);
    type_change_is_gits(&repo, "link.txt", WorkingTreeDiff::Unstaged, &link);
    assert!(
        git_diff(&repo, "mode.sh", WorkingTreeDiff::Unstaged, &[])
            .contains("old mode 100644\nnew mode 100755\n"),
        "git does not call it a mode change"
    );

    repo.git(&["add", "--all", "."]);
    let staged = answers_writing_nothing(
        &repo,
        &[
            ("gone.txt", WorkingTreeDiff::Staged),
            ("link.txt", WorkingTreeDiff::Staged),
            ("mode.sh", WorkingTreeDiff::Staged),
        ],
    );
    assert_eq!(
        some(staged[0].clone(), "the staged deletion").file.status,
        ChangeStatus::Deleted
    );
    same_as_git(&repo, "gone.txt", WorkingTreeDiff::Staged, &staged[0]);
    type_change_is_gits(
        &repo,
        "link.txt",
        WorkingTreeDiff::Staged,
        &some(staged[1].clone(), "the staged type change"),
    );
    assert_eq!(
        some(staged[2].clone(), "the staged mode change").content,
        DiffContent::ModeChangeOnly
    );
    assert_eq!(
        unstaged_now(&repo, "mode.sh"),
        None,
        "staged, nothing is left unstaged"
    );
}

fn unstaged_now(repo: &Repo, path: &str) -> Option<FileDiff> {
    ask(repo, path, WorkingTreeDiff::Unstaged)
}

/// git prints a type change as a deletion and an addition; Cairn draws one change of every
/// line, so each side is compared with git's section for it.
fn type_change_is_gits(repo: &Repo, path: &str, which: WorkingTreeDiff, diff: &FileDiff) {
    let out = git_diff(repo, path, which, &[]);
    let sections: Vec<&str> = out.split("diff --git ").filter(|s| !s.is_empty()).collect();
    assert_eq!(
        sections.len(),
        2,
        "git's type change is two sections: {out}"
    );
    let marked = |section: &str, marker: char| -> Vec<String> {
        hunks_of(section)
            .into_iter()
            .flat_map(|hunk| hunk.lines)
            .filter_map(|line| line.strip_prefix(marker).map(str::to_owned))
            .collect()
    };
    let (old, new) = lines(diff);
    assert_eq!(old, marked(sections[0], '-'));
    assert_eq!(new, marked(sections[1], '+'));
}

/// C7, conflicted: a path the index holds unmerged answers conflicted from both the staged
/// and the unstaged query — never a diff against one stage — where git itself shows a
/// combined diff and "Unmerged path". Caught by: reading stage 0 (absent) as a deletion,
/// or stage 2 as the old side.
#[test]
fn a_conflicted_path_answers_conflicted() {
    let repo = base("conflict");
    repo.write("f", b"base\n");
    repo.commit("base");
    repo.git(&["checkout", "--quiet", "-b", "other"]);
    repo.write("f", b"theirs\n");
    repo.commit("theirs");
    repo.git(&["checkout", "--quiet", "main"]);
    repo.write("f", b"ours\n");
    repo.commit("ours");
    let (status, _, _) = repo.run(&["merge", "--quiet", "other"], &[], None);
    assert!(!status.success(), "the merge did not conflict");

    let answers = answers_writing_nothing(
        &repo,
        &[
            ("f", WorkingTreeDiff::Staged),
            ("f", WorkingTreeDiff::Unstaged),
        ],
    );
    for answer in &answers {
        assert_eq!(
            some(answer.clone(), "an answer").content,
            DiffContent::Conflicted
        );
    }
    assert!(git_diff(&repo, "f", WorkingTreeDiff::Unstaged, &[]).starts_with("diff --cc f"));
    assert!(git_diff(&repo, "f", WorkingTreeDiff::Staged, &[]).contains("Unmerged path f"));
}

/// C7, a submodule: its commit on each side and whether its checkout is dirty, read from
/// git — moved, dirty, moved and dirty, staged — and hidden exactly where the user's `git
/// diff` hides it under `diff.ignoreSubmodules`, which plumbing does not read, and under a
/// submodule's own `ignore`, which beats it. Caught by: the `-dirty` mark dropped or read
/// into the id, or `diff.ignoreSubmodules` not passed (plumbing shows what porcelain hides).
#[test]
fn a_submodule_answers_its_commits_and_whether_it_is_dirty_as_git_diff_shows_it() {
    let inner = base("inner");
    inner.write("i", b"i\n");
    let first = inner.commit("i");
    inner.write("j", b"j\n");
    let second = inner.commit("j");
    let outer = base("outer");
    let url = inner.path().display().to_string();
    for name in ["moved", "dirty", "both"] {
        outer.git(&[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "--quiet",
            "add",
            &url,
            name,
        ]);
    }
    outer.commit("three submodules");
    outer.git(&["-C", "moved", "checkout", "--quiet", first.hex().as_str()]);
    outer.write("dirty/i", b"changed\n");
    outer.git(&["-C", "both", "checkout", "--quiet", first.hex().as_str()]);
    outer.write("both/i", b"changed\n");

    let answers = answers_writing_nothing(
        &outer,
        &[
            ("moved", WorkingTreeDiff::Unstaged),
            ("dirty", WorkingTreeDiff::Unstaged),
            ("both", WorkingTreeDiff::Unstaged),
        ],
    );
    let expect = [(first, false), (second, true), (first, true)];
    for ((answer, (target, dirty)), name) in
        answers.iter().zip(expect).zip(["moved", "dirty", "both"])
    {
        let diff = some(
            answer.clone(),
            &format!(
                "a submodule answer for {name}: git says {}",
                git_diff(&outer, name, WorkingTreeDiff::Unstaged, &[])
            ),
        );
        assert_eq!(
            diff.content,
            DiffContent::Submodule {
                old_target: Some(second),
                new_target: Some(target),
                dirty,
            },
            "{}",
            diff.file.new_path
        );
        let shown = git_diff(
            &outer,
            &diff.file.new_path.display(),
            WorkingTreeDiff::Unstaged,
            &[],
        );
        let mark = if dirty { "-dirty" } else { "" };
        assert!(
            shown.contains(&format!(
                "+Subproject commit {}{mark}\n",
                target.hex().as_str()
            )),
            "git shows otherwise: {shown}"
        );
    }

    for setting in ["none", "untracked", "dirty", "all"] {
        outer.config("diff.ignoreSubmodules", setting);
        for name in ["moved", "dirty", "both"] {
            let answer = ask(&outer, name, WorkingTreeDiff::Unstaged);
            let shown = git_diff(&outer, name, WorkingTreeDiff::Unstaged, &[]);
            assert_eq!(
                answer.is_none(),
                shown.is_empty(),
                "{name} under diff.ignoreSubmodules={setting}: Cairn {answer:?}, git {shown:?}"
            );
        }
    }
    // A submodule's own setting beats the global one, in plumbing and porcelain alike.
    outer.config("diff.ignoreSubmodules", "all");
    outer.config("submodule.dirty.ignore", "none");
    assert!(ask(&outer, "dirty", WorkingTreeDiff::Unstaged).is_some());
    assert!(!git_diff(&outer, "dirty", WorkingTreeDiff::Unstaged, &[]).is_empty());
    outer.git(&["config", "--unset", "submodule.dirty.ignore"]);
    outer.git(&["config", "--unset", "diff.ignoreSubmodules"]);

    outer.git(&["add", "moved"]);
    let staged = some(
        ask(&outer, "moved", WorkingTreeDiff::Staged),
        "the staged move",
    );
    assert_eq!(
        staged.content,
        DiffContent::Submodule {
            old_target: Some(second),
            new_target: Some(first),
            dirty: false,
        }
    );
}

/// C7, a sparse index: unsupported, and said so — for a path inside the sparse cone and
/// one outside it — rather than an answer read from an index Cairn does not read. Skipped
/// on a git that cannot write one (before 2.32), and only there: on any other git, a
/// sparse checkout that cannot be set up is a failure, never a skip. The index is written
/// by `sparse-checkout init --cone --sparse-index`, deprecated in git's documentation but
/// the one spelling that writes a sparse index on every git from 2.32: `set --cone
/// --sparse-index` writes none on 2.32.7 (reproduced; `set` takes those options from a
/// later release), where `init` does on 2.32.7 and 2.56.0 alike.
#[test]
fn a_sparse_index_is_unsupported_and_says_so() {
    if super::git().version() < super::since(32) {
        eprintln!(
            "SKIPPED a_sparse_index_is_unsupported_and_says_so: git {} cannot write a sparse \
             index",
            super::git().version()
        );
        return;
    }
    let repo = base("sparse");
    repo.write("in/a", b"a\n");
    repo.write("out/b", b"b\n");
    repo.commit("two directories");
    let (status, _, stderr) = repo.run(
        &["sparse-checkout", "init", "--cone", "--sparse-index"],
        &[],
        None,
    );
    assert!(
        status.success(),
        "git {} could not write a sparse index: {}",
        super::git().version(),
        stderr.trim()
    );
    repo.git(&["sparse-checkout", "set", "in"]);
    // A sparse index carries the `sdir` extension (`Documentation/gitformat-index.txt`).
    let index = ok(std::fs::read(repo.path().join(".git/index")), "the index");
    assert!(
        index.windows(4).any(|window| window == b"sdir"),
        "the index is not a sparse index"
    );
    repo.write("in/a", b"a, changed\n");

    let answers = answers_writing_nothing(
        &repo,
        &[
            ("in/a", WorkingTreeDiff::Unstaged),
            ("in/a", WorkingTreeDiff::Staged),
            ("out/b", WorkingTreeDiff::Unstaged),
        ],
    );
    for answer in answers {
        let diff = some(answer, "an answer");
        let DiffContent::Unsupported { reason } = diff.content else {
            panic!("not unsupported: {diff:?}");
        };
        assert!(reason.contains("sparse"), "{reason}");
    }
}

/// A file whose stat moved and whose content did not is no change — `diff-files` lists
/// it and prints nothing, and the user's `git diff` shows nothing. Caught by: the listed
/// record answered as a change with no lines.
#[test]
fn a_stat_dirty_file_is_no_change() {
    let repo = base("stat");
    repo.write("same.txt", b"same\n");
    repo.commit("a file");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    repo.write("same.txt", b"same\n");
    let raw = repo.git(&["diff-files", "--name-only"]);
    assert!(
        raw.contains("same.txt"),
        "the fixture is not stat-dirty: {raw:?}"
    );
    let answers = answers_writing_nothing(&repo, &[("same.txt", WorkingTreeDiff::Unstaged)]);
    assert_eq!(answers[0], None);
    same_as_git(&repo, "same.txt", WorkingTreeDiff::Unstaged, &answers[0]);
}

/// `git add -N`: the unstaged diff is a new file with every line, as `git diff` shows; the
/// staged diff is nothing, as `git diff --cached` shows — where `diff-index --cached` lists
/// an empty file added. Caught by: asking plumbing for the staged side without the index's
/// intent-to-add flag.
#[test]
fn an_intent_to_add_path_is_new_unstaged_and_nothing_staged() {
    let repo = base("ita");
    repo.write("n.txt", b"i1\ni2\n");
    repo.git(&["add", "-N", "n.txt"]);
    let answers = answers_writing_nothing(
        &repo,
        &[
            ("n.txt", WorkingTreeDiff::Unstaged),
            ("n.txt", WorkingTreeDiff::Staged),
        ],
    );
    let unstaged = some(answers[0].clone(), "the new file");
    assert_eq!(unstaged.file.status, ChangeStatus::Added);
    assert_eq!(lines(&unstaged).1, ["i1", "i2"]);
    assert_eq!(answers[1], None);
    same_as_git(&repo, "n.txt", WorkingTreeDiff::Unstaged, &answers[0]);
    same_as_git(&repo, "n.txt", WorkingTreeDiff::Staged, &answers[1]);
}

/// The untracked shapes `git diff --no-index` has answers for: plain text, no final
/// newline, an empty file, a symlink (its target), a binary file, and paths git would read
/// as options, globs or standard input — a file named `-`, which `--no-index` reads as
/// stdin unless it is spelled `./-`, holds its own lines, as the user's `git diff
/// --no-index -- /dev/null ./-` shows them. Caught by: a symlink followed, a missing
/// newline marker, a path read as a pathspec or an option, or `-` passed as itself (an
/// empty file added, read from Cairn's empty stdin).
#[test]
fn an_untracked_file_is_what_git_diff_no_index_shows() {
    let repo = base("untracked");
    repo.write("plain.txt", b"a\nb\n");
    repo.write("no-eol.txt", b"no newline");
    repo.write("empty.txt", b"");
    repo.symlink("link", "plain.txt");
    repo.write("bin.dat", b"a\0b\n");
    repo.write("-dash *.txt", b"x\n");
    repo.write("-", b"dash\nalone\n");
    repo.write("-x", b"option\n");
    let paths = [
        "plain.txt",
        "no-eol.txt",
        "empty.txt",
        "link",
        "bin.dat",
        "-dash *.txt",
        "-",
        "-x",
    ];
    let queries: Vec<(&str, WorkingTreeDiff)> = paths
        .iter()
        .map(|path| (*path, WorkingTreeDiff::Untracked))
        .collect();
    let answers = answers_writing_nothing(&repo, &queries);
    for (path, answer) in paths.iter().zip(&answers) {
        let diff = some(answer.clone(), "an untracked answer");
        assert_eq!(diff.file.status, ChangeStatus::Added, "{path}");
        match *path {
            "bin.dat" => assert_eq!(
                diff.content,
                DiffContent::Binary {
                    old_size: 0,
                    new_size: 4
                }
            ),
            "link" => {
                assert_eq!(diff.file.new_mode, Some(FileMode::Symlink));
                assert_eq!(lines(&diff).1, ["plain.txt"]);
                same_as_git(&repo, path, WorkingTreeDiff::Untracked, answer);
            }
            _ => same_as_git(&repo, path, WorkingTreeDiff::Untracked, answer),
        }
    }
    assert!(
        git_diff(&repo, "bin.dat", WorkingTreeDiff::Untracked, &[]).contains("Binary files"),
        "git does not call it binary"
    );
    let dash = some(answers[6].clone(), "the file named -");
    assert_eq!(
        lines(&dash).1,
        ["dash", "alone"],
        "not the file's own lines"
    );
    assert_eq!(
        dash.file.new_path,
        RepoPath::new("-"),
        "the path asked about"
    );
    assert_eq!(dash.file.old_path, RepoPath::new("-"));
    assert!(
        porcelain(&repo, &["diff", "--no-index", "--", "/dev/null", "./-"]).contains("+dash\n"),
        "the user's git does not read ./- as the file"
    );
}

/// Ordinary edits, staged and unstaged at once, with whitespace changes among them, under
/// the default algorithm and a configured one: each of the four answers is `git diff`'s,
/// `-w` included. Caught by: the staged and unstaged sides swapped, the algorithm not
/// passed, or the whitespace reading taken from the exact one.
#[test]
fn staged_and_unstaged_edits_read_as_git_diff_shows_them() {
    let repo = base("edits");
    let body: String = (1..=40).map(|n| format!("line {n}\n")).collect();
    repo.write("f.txt", body.as_bytes());
    repo.commit("forty lines");
    let staged = body
        .replace("line 3\n", "line three\n")
        .replace("line 30\n", "  line 30\n");
    repo.write("f.txt", staged.as_bytes());
    repo.git(&["add", "f.txt"]);
    let unstaged = staged
        .replace("line 10\n", "line ten\nline ten and a half\n")
        .replace("line 20\n", "line 20 \n");
    repo.write("f.txt", unstaged.as_bytes());
    for algorithm in [None, Some("histogram"), Some("patience")] {
        if let Some(algorithm) = algorithm {
            repo.config("diff.algorithm", algorithm);
        }
        let answers = answers_writing_nothing(
            &repo,
            &[
                ("f.txt", WorkingTreeDiff::Staged),
                ("f.txt", WorkingTreeDiff::Unstaged),
            ],
        );
        same_as_git(&repo, "f.txt", WorkingTreeDiff::Staged, &answers[0]);
        same_as_git(&repo, "f.txt", WorkingTreeDiff::Unstaged, &answers[1]);
    }
}

/// Line endings at the edges, staged and unstaged: an edit near a last line with no
/// newline (which git prints as context, marked), a change to the final newline alone,
/// each way, and a CRLF file kept as it is (`-text`) edited mid-file, its `\r` part of each
/// line — every answer what `git diff --cached` and `git diff` show. Caught by: a context
/// line rebuilt as terminated whatever git printed (the rebuilt side is not the object git
/// named), a final newline read from the old side, or a `\r` dropped.
#[test]
fn line_endings_at_the_edges_read_as_git_diff_shows_them() {
    let repo = base("edges");
    repo.write(".gitattributes", b"*.crlf -text\n");
    repo.write("noeol.txt", b"one\ntwo\nthree\nfour\nfive");
    repo.write("eol.txt", b"a\nb\nc");
    repo.write("kept.crlf", b"a\r\nb\r\nc\r\nd\r\ne\r\n");
    repo.commit("the edges");
    repo.write("noeol.txt", b"one\ntwo\nTHREE\nfour\nfive");
    repo.write("eol.txt", b"a\nb\nc\n");
    repo.write("kept.crlf", b"a\r\nB\r\nc\r\nd\r\ne\r\n");
    repo.git(&["add", "noeol.txt", "eol.txt", "kept.crlf"]);
    repo.write("noeol.txt", b"one\nTWO\nTHREE\nfour\nfive");
    repo.write("eol.txt", b"a\nb\nc");
    repo.write("kept.crlf", b"a\r\nB\r\nc\r\nD\r\ne\r\n");

    let paths = ["noeol.txt", "eol.txt", "kept.crlf"];
    let mut queries = Vec::new();
    for path in paths {
        queries.push((path, WorkingTreeDiff::Staged));
        queries.push((path, WorkingTreeDiff::Unstaged));
    }
    let answers = answers_writing_nothing(&repo, &queries);
    for ((path, which), answer) in queries.iter().zip(&answers) {
        let diff = some(answer.clone(), &format!("{which:?} {path}"));
        let new = some(diff.text(), "a text answer").new_lines().to_vec();
        match (*path, which) {
            ("noeol.txt" | "eol.txt", WorkingTreeDiff::Unstaged) => assert!(
                new.last().is_some_and(|line| !line.ends_with_newline()),
                "{which:?} {path}: the last line has no newline"
            ),
            ("eol.txt", _) => assert!(new.last().is_some_and(DiffLine::ends_with_newline)),
            ("kept.crlf", WorkingTreeDiff::Staged) => {
                assert_eq!(lines(&diff).1[1], "B\r", "the \\r is part of the line");
            }
            _ => {}
        }
        same_as_git(&repo, path, *which, answer);
    }
}

/// An unborn branch: the staged diff is against the empty tree, as `git diff --cached`
/// compares it; and a path that is a file in the index where `HEAD` had a directory is
/// that one file. Caught by: asking for `HEAD` on an unborn branch (git fails), or a
/// pathspec that also matches the directory's old contents.
#[test]
fn a_staged_file_on_an_unborn_branch_and_beside_a_directory_is_that_file() {
    let unborn = Repo::new("unborn");
    unborn.write("first.txt", b"first\n");
    unborn.git(&["add", "first.txt"]);
    let answer = ask(&unborn, "first.txt", WorkingTreeDiff::Staged);
    assert_eq!(lines(&some(answer.clone(), "the staged file")).1, ["first"]);
    same_as_git(&unborn, "first.txt", WorkingTreeDiff::Staged, &answer);

    let repo = base("dir-file");
    repo.write("d/x", b"x\n");
    repo.commit("a directory");
    repo.git(&["rm", "--quiet", "-r", "d"]);
    repo.write("d", b"now a file\n");
    repo.git(&["add", "d"]);
    let answer = some(ask(&repo, "d", WorkingTreeDiff::Staged), "the file");
    assert_eq!(answer.file.status, ChangeStatus::Added);
    assert_eq!(lines(&answer).1, ["now a file"]);
}

/// R2.6 on the working tree. A file past the ceiling in the index is refused before git
/// diffs it; one past it only in the working tree is refused once git's output passes what
/// both sides could hold; loading anyway reads both. And a clean filter that makes a large
/// file small — as git-lfs's does — is diffed in that small form, as `git diff` shows it,
/// where judging by the file on disk would refuse it. Caught by: the size taken from the
/// working tree's bytes before the filter, or no ceiling on git's output.
#[test]
fn the_size_ceiling_is_judged_on_gits_form_of_the_working_tree() {
    let repo = base("sizes");
    let shrink = script(&repo, "shrink.sh", "#!/bin/sh\nwc -c | tr -d ' '\n");
    repo.config("filter.shrink.clean", &shrink.display().to_string());
    repo.config("filter.shrink.smudge", "cat");
    repo.write(".gitattributes", b"*.big filter=shrink\n");
    let small = ContentOptions {
        limits: cairn_model::DiffLimits {
            max_bytes: 4096,
            ..cairn_model::DiffLimits::default()
        },
        ..ContentOptions::default()
    };
    let big: String = (0..2000).map(|n| format!("{n}\n")).collect();
    repo.write("grown.txt", b"small\n");
    repo.write("pointer.big", b"tiny\n");
    repo.commit("small files");
    repo.write("grown.txt", big.as_bytes());
    repo.write("pointer.big", big.as_bytes());

    let grown = some(
        ask_with(&repo, "grown.txt", WorkingTreeDiff::Unstaged, &small),
        "an answer",
    );
    let DiffContent::TooLarge {
        crossed: SizeLimit::Bytes { limit, measured },
        loadable,
    } = grown.content
    else {
        panic!("not refused: {grown:?}");
    };
    assert_eq!(limit, 4096);
    assert!(measured > 4096 && loadable, "{measured} {loadable}");
    let loaded = ContentOptions {
        load_anyway: true,
        ..small
    };
    let read = some(
        ask_with(&repo, "grown.txt", WorkingTreeDiff::Unstaged, &loaded),
        "loaded",
    );
    assert_eq!(lines(&read).1.len(), 2000);

    // A filter that makes a small file enormous: refused once git's output passes what two
    // sides within the ceiling could print, without the rest of it read — so the
    // measurement is the ceiling's next byte, not the size git would have printed.
    let grow = script(
        &repo,
        "grow.sh",
        "#!/bin/sh\ncat >/dev/null\nyes aaaaaaa | head -c 3000000\n",
    );
    repo.config("filter.grow.clean", &grow.display().to_string());
    repo.write(
        ".gitattributes",
        b"*.big filter=shrink\n*.grow filter=grow\n",
    );
    repo.write("expands.grow", b"small\n");
    let expands = some(
        ask_with(&repo, "expands.grow", WorkingTreeDiff::Untracked, &small),
        "the expanded file",
    );
    assert_eq!(
        expands.content,
        DiffContent::TooLarge {
            crossed: SizeLimit::Bytes {
                limit: 4096,
                measured: 4097
            },
            loadable: true,
        },
        "git's whole output was read rather than cut at the ceiling"
    );

    let pointer = some(
        ask_with(&repo, "pointer.big", WorkingTreeDiff::Unstaged, &small),
        "the filtered file",
    );
    assert_eq!(
        lines(&pointer).1,
        [big.len().to_string()],
        "not the filter's small form"
    );
    same_as_git(
        &repo,
        "pointer.big",
        WorkingTreeDiff::Unstaged,
        &Some(pointer),
    );

    repo.git(&["add", "grown.txt"]);
    repo.write("grown.txt", b"small again\n");
    let staged_big = some(
        ask_with(&repo, "grown.txt", WorkingTreeDiff::Unstaged, &small),
        "an answer",
    );
    assert!(
        matches!(staged_big.content, DiffContent::TooLarge { .. }),
        "the index's large blob was diffed: {staged_big:?}"
    );
}

/// R2.6's ceiling on git's output counts a side Cairn has not measured — the working tree,
/// which only git reads — at the size limit, so an untracked file and a large edit of a
/// small indexed file, each well inside the DEFAULT limits, are read whole and are what
/// `git diff` shows. Caught by: an unmeasured side counted as nothing (git ended once its
/// output passes the room for the headers, and the file answered too large).
#[test]
fn a_working_tree_side_is_counted_at_the_limit_so_a_large_one_within_it_is_read() {
    let repo = base("unmeasured");
    let body: String = (0..20_000).map(|n| format!("line {n:05}\n")).collect();
    assert!(body.len() > 200 * 1024 && (body.len() as u64) < DiffLimits::MAX_BYTES);
    repo.write("small.txt", b"small\n");
    repo.commit("a small file");
    repo.write("small.txt", body.as_bytes());
    repo.write("new.txt", body.as_bytes());
    for (path, which) in [
        ("new.txt", WorkingTreeDiff::Untracked),
        ("small.txt", WorkingTreeDiff::Unstaged),
    ] {
        let answer = ask(&repo, path, which);
        let diff = some(answer.clone(), "an answer");
        assert!(
            diff.text().is_some(),
            "{which:?} {path}: {:?}",
            diff.content
        );
        assert_eq!(lines(&diff).1.len(), 20_000, "{which:?} {path}");
        same_as_git(&repo, path, which, &answer);
    }
}

/// The ceiling on git's output is twice both sides, since git prints every line of a
/// rewrite with a marker before it: a staged rewrite of every line, both sides just under
/// half the size limit and every line short — where the markers weigh most — is read
/// whole and is what `git diff --cached` shows. Caught by: the output allowed only the two
/// sides' bytes (git ended mid-patch, and the file answered too large).
#[test]
fn a_rewrite_of_every_line_within_the_limit_is_read_whole() {
    let repo = base("rewrite");
    let lines_per_side = 40_000;
    let old = "a\n".repeat(lines_per_side);
    let new = "b\n".repeat(lines_per_side);
    let limits = DiffLimits {
        max_bytes: 2 * old.len() as u64 + 2,
        ..DiffLimits::default()
    };
    assert!(old.len() as u64 * 2 < limits.max_bytes && lines_per_side < 50_000);
    repo.write("rewrite.txt", old.as_bytes());
    repo.commit("every line a");
    repo.write("rewrite.txt", new.as_bytes());
    repo.git(&["add", "rewrite.txt"]);
    let options = ContentOptions {
        limits,
        ..ContentOptions::default()
    };
    let answer = ask_with(&repo, "rewrite.txt", WorkingTreeDiff::Staged, &options);
    let diff = some(answer.clone(), "the rewrite");
    assert!(diff.text().is_some(), "{:?}", diff.content);
    let (old_lines, new_lines) = lines(&diff);
    assert_eq!(
        (old_lines.len(), new_lines.len()),
        (lines_per_side, lines_per_side)
    );
    let theirs = hunks_of(&git_diff(
        &repo,
        "rewrite.txt",
        WorkingTreeDiff::Staged,
        &[],
    ));
    assert_eq!(cairn_view(&diff, Context::lines(3), false), theirs);
}

/// The stale-read guard on the working tree: a clean filter whose output changes every
/// time it runs is a file that changed between git's reads of it — git hashes the working
/// tree again for its `index` line — so the lines rebuilt from the patch are not the
/// content git named, and the answer is the error a caller retries, never those lines.
/// Caught by: the rebuilt side not checked against git's id.
#[test]
fn content_that_changes_between_gits_reads_is_the_error_a_caller_retries() {
    let repo = base("unstable");
    let counter = repo.path().join("counter");
    let body = format!(
        "#!/bin/sh\ncat >/dev/null\necho x >> '{}'\nwc -l < '{}'\n",
        counter.display(),
        counter.display()
    );
    let program = script(&repo, "count.sh", &body);
    repo.write("f.cnt", b"anything\n");
    repo.commit("before the filter");
    repo.config("filter.count.clean", &program.display().to_string());
    repo.write(".gitattributes", b"*.cnt filter=count\n");
    repo.write("f.cnt", b"anything, edited\n");
    let outcome = engine(&repo).working_tree_diff(
        super::git(),
        &RepoPath::new("f.cnt"),
        WorkingTreeDiff::Unstaged,
        &ContentOptions::default(),
        &CancelSignal::new(),
    );
    match outcome {
        Err(Error::ContentReadsDisagree { path, .. }) => assert_eq!(path, "f.cnt"),
        other => panic!("expected the stale-read error, got {other:?}"),
    }
}

/// The stale-read guard's third check: the whitespace-ignoring answer is a second git
/// process, and the working tree it read must be the object the first one named. A clean
/// filter whose output is stable within one git process but not across two — the first
/// process's reads (two on every git: the diff, then the hash for its `index` line) see the
/// file as it is, every later read sees a trailing space on its last line — makes the `-w`
/// read's lines agree with the lines held (that line is far from the edit, and under `-w`
/// unchanged, so never printed) while it names another object. The exact answer alone is
/// drawn; asked with `-w`, the answer is the error a caller retries. Caught by: the `-w`
/// read's `index` line not compared with the object already named.
#[test]
fn a_whitespace_ignoring_read_of_other_content_is_the_error_a_caller_retries() {
    let repo = base("unstable-across");
    let counter = repo.path().join("counter");
    let body = format!(
        "#!/bin/sh\necho x >> '{counter}'\nn=$(wc -l < '{counter}')\n\
         if [ $(( (n - 1) / 2 )) -eq 0 ]; then exec cat; else exec sed '$ s/$/ /'; fi\n",
        counter = counter.display()
    );
    let program = script(&repo, "generation.sh", &body);
    let original: String = (1..=20).map(|n| format!("l{n}\n")).collect();
    repo.write("f.gen", original.as_bytes());
    repo.commit("before the filter");
    repo.config("filter.generation.clean", &program.display().to_string());
    repo.write(".gitattributes", b"*.gen filter=generation\n");
    repo.write("f.gen", original.replacen("l1\n", "X\n", 1).as_bytes());

    let exact = some(
        ask(&repo, "f.gen", WorkingTreeDiff::Unstaged),
        "the exact answer",
    );
    assert_eq!(lines(&exact).1.last().map(String::as_str), Some("l20"));
    let read = ok(std::fs::read_to_string(&counter), "the counter");
    assert_eq!(
        read.lines().count(),
        2,
        "one git process read the file twice"
    );

    ok(std::fs::remove_file(&counter), "resetting the counter");
    let outcome = engine(&repo).working_tree_diff(
        super::git(),
        &RepoPath::new("f.gen"),
        WorkingTreeDiff::Unstaged,
        &ContentOptions {
            ignore_whitespace: true,
            ..ContentOptions::default()
        },
        &CancelSignal::new(),
    );
    match outcome {
        Err(Error::ContentReadsDisagree { path, .. }) => assert_eq!(path, "f.gen"),
        other => panic!("expected the stale-read error, got {other:?}"),
    }
}

/// R3.5 against the programs a read must not run: with a textconv that caches, an external
/// diff, a driver's `command` and a smudge filter configured, and `core.fsmonitor` naming a
/// hook, every query on every kind of side leaves the git directory byte-identical and runs
/// none of them — the fsmonitor hook, which every read with a working tree runs as `git
/// diff` does, and the clean filter of the paths read from the working tree are the only
/// programs that ran. Caught by: `--textconv` or `--ext-diff` on a read, or porcelain `git
/// diff` for a tracked path (which refreshes the index).
#[test]
fn a_working_tree_query_writes_nothing_and_runs_only_the_clean_filter_and_fsmonitor() {
    let repo = base("traps");
    let trap_mark = repo.path().join("trap-ran");
    let trap = script(
        &repo,
        "trap.sh",
        &format!("#!/bin/sh\n: > '{}'\ncat \"$1\"\n", trap_mark.display()),
    );
    let smudge_mark = repo.path().join("smudge-ran");
    let smudge = script(
        &repo,
        "smudge.sh",
        &format!("#!/bin/sh\n: > '{}'\ncat\n", smudge_mark.display()),
    );
    let monitored = repo.path().join("fsmonitor-ran");
    let monitor = script(
        &repo,
        "monitor.sh",
        &format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\nexit 1\n",
            monitored.display()
        ),
    );
    repo.write("a.txt", b"one\ntwo\n");
    repo.write("b.txt", b"one\ntwo\n");
    repo.commit("two files");
    let clean_mark = upper_filter(&repo, "mark");
    repo.config("filter.mark.smudge", &smudge.display().to_string());
    repo.config("diff.trap.textconv", &trap.display().to_string());
    repo.config("diff.trap.cachetextconv", "true");
    repo.config("diff.trap.command", &trap.display().to_string());
    repo.config("diff.external", &trap.display().to_string());
    repo.config("core.fsmonitor", &monitor.display().to_string());
    repo.write(".gitattributes", b"*.txt diff=trap filter=mark\n");
    repo.write("a.txt", b"one\ntwo, staged\n");
    repo.git(&["add", "a.txt"]);
    repo.write("a.txt", b"one\ntwo, staged and then not\n");
    repo.write("b.txt", b"one\ntwo, unstaged\n");
    repo.write("c.txt", b"untracked\n");
    let _ = std::fs::remove_file(&clean_mark);
    let _ = std::fs::remove_file(&monitored);

    let shared = ok(SharedRepository::discover(repo.path()), "the fixture opens");
    let before = snapshot(&repo.path().join(".git"));
    let engine = shared.to_worker();
    let mut read = 0;
    for (path, which) in [
        ("a.txt", WorkingTreeDiff::Staged),
        ("a.txt", WorkingTreeDiff::Unstaged),
        ("b.txt", WorkingTreeDiff::Unstaged),
        ("c.txt", WorkingTreeDiff::Untracked),
    ] {
        for ignore_whitespace in [false, true] {
            let answer = ok(
                engine.working_tree_diff(
                    super::git(),
                    &RepoPath::new(path),
                    which,
                    &ContentOptions {
                        ignore_whitespace,
                        ..ContentOptions::default()
                    },
                    &CancelSignal::new(),
                ),
                "an answer",
            );
            read += usize::from(answer.is_some_and(|diff| diff.text().is_some()));
        }
    }
    assert_eq!(read, 8, "a query answered no text");
    assert!(
        snapshot(&repo.path().join(".git")) == before,
        "the git directory changed under a read"
    );
    assert!(
        !repo.path().join(".git/refs/notes").exists(),
        "a textconv cache was written"
    );
    assert!(
        !trap_mark.exists(),
        "a textconv, external diff or driver command ran"
    );
    assert!(!smudge_mark.exists(), "a smudge filter ran");
    assert!(
        clean_mark.exists(),
        "the clean filter never ran on a working-tree read"
    );
    assert!(
        monitored.exists(),
        "the repository's core.fsmonitor did not run"
    );
    let log = shared.command_log();
    for record in &log {
        let verb = record
            .arguments
            .iter()
            .find(|argument| !argument.starts_with('-') && !argument.contains('='))
            .map(String::as_str);
        assert!(
            matches!(
                verb,
                Some("diff-index" | "diff-files" | "diff" | "check-attr")
            ),
            "a read ran {:?}",
            record.arguments
        );
        if verb == Some("diff") {
            assert!(
                record.arguments.iter().any(|a| a == "--no-index"),
                "porcelain git diff ran"
            );
        }
    }

    // The traps can run at all, so their not running decided something.
    let ran = std::process::Command::new(&trap)
        .arg(repo.path().join("a.txt"))
        .stdout(std::process::Stdio::null())
        .status();
    assert!(ran.is_ok_and(|status| status.success()) && trap_mark.exists());
}

/// An untracked file is asked for by a path relative to the top of the working tree, and
/// is a file or a symlink: a path that is empty, absolute or has a `.` or `..` component is
/// refused before anything is read — `--no-index` would read a file outside the working
/// tree — and a named pipe or a directory is `Unsupported`, said before git runs (git 2.56
/// waits on a pipe for a writer, git 2.30.9 and 2.32.7 print a gitlink record for it and
/// fail, and every git diffs `/dev/null` against `<dir>/null` in a directory). Caught by:
/// the path handed to git as given, or git asked about what is not a file.
#[test]
fn an_untracked_path_outside_the_working_tree_or_not_a_file_is_refused_before_git_runs() {
    let repo = base("not-a-file");
    let holder = Repo::new("outside-holder");
    let outside = holder.path().join("outside");
    ok(
        std::fs::write(&outside, b"outside\n"),
        "a file outside the repository",
    );
    repo.write("plain", b"plain\n");
    repo.write("dir/inner", b"inner\n");
    let fifo = std::process::Command::new("mkfifo")
        .arg(repo.path().join("pipe"))
        .status();
    assert!(fifo.is_ok_and(|status| status.success()), "mkfifo");
    let shared = ok(SharedRepository::discover(repo.path()), "the fixture opens");
    let ask_shared = |path: &RepoPath| {
        let cancel = CancelSignal::new();
        let (sent, answered) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let answer = shared.to_worker().working_tree_diff(
                    super::git(),
                    path,
                    WorkingTreeDiff::Untracked,
                    &ContentOptions::default(),
                    &cancel,
                );
                let _ = sent.send(answer);
            });
            match answered.recv_timeout(std::time::Duration::from_secs(30)) {
                Ok(answer) => answer,
                Err(_) => {
                    cancel.cancel();
                    panic!("the untracked read of {path} did not return: git waits on it")
                }
            }
        })
    };

    let mut refused = vec![
        String::new(),
        outside.display().to_string(),
        "./plain".to_owned(),
        "dir/./inner".to_owned(),
        "dir/../plain".to_owned(),
        "..".to_owned(),
        ".".to_owned(),
    ];
    let beside = some(holder.path().file_name(), "a name").to_string_lossy();
    refused.push(format!("../{beside}/outside"));
    for path in &refused {
        match ask_shared(&RepoPath::new(path.as_bytes())) {
            Err(Error::NotAWorkTreePath { path: named }) => assert_eq!(&named, path),
            other => panic!("{path:?} answered {other:?}"),
        }
    }
    for path in ["pipe", "dir"] {
        let answer = ok(ask_shared(&RepoPath::new(path)), "an answer");
        let diff = some(answer, "an answer for what is not a file");
        let DiffContent::Unsupported { reason } = &diff.content else {
            panic!("{path} answered {diff:?}");
        };
        assert!(!reason.is_empty(), "{path}");
        assert_eq!(diff.file.new_path, RepoPath::new(path));
    }
    assert!(
        shared.command_log().is_empty(),
        "a git ran: {:?}",
        shared.command_log()
    );
    // The same paths are asked about as git holds them, and answered.
    let plain = some(
        ok(ask_shared(&RepoPath::new("plain")), "the plain file"),
        "an answer",
    );
    assert_eq!(lines(&plain).1, ["plain"]);
    let inner = some(
        ok(ask_shared(&RepoPath::new("dir/inner")), "the inner file"),
        "an answer",
    );
    assert_eq!(lines(&inner).1, ["inner"]);
}

/// A query superseded before it starts runs nothing, and a bare repository has no working
/// tree to diff — said so, not an error.
#[test]
fn a_superseded_query_runs_nothing_and_a_bare_repository_says_it_has_no_working_tree() {
    let repo = base("cancelled");
    repo.write("seed", b"changed\n");
    let shared = ok(SharedRepository::discover(repo.path()), "the fixture opens");
    let cancelled = CancelSignal::new();
    cancelled.cancel();
    for which in EVERY {
        let outcome = shared.to_worker().working_tree_diff(
            super::git(),
            &RepoPath::new("seed"),
            which,
            &ContentOptions::default(),
            &cancelled,
        );
        assert!(
            matches!(outcome, Err(Error::ContentCancelled)),
            "{outcome:?}"
        );
    }
    assert!(shared.command_log().is_empty(), "a process was started");

    let bare = Repo::new("bare-holder");
    let path = bare.path().join("bare.git");
    let (status, _, stderr) = bare.run(
        &[
            "clone",
            "--quiet",
            "--bare",
            &repo.path().display().to_string(),
            &path.display().to_string(),
        ],
        &[],
        None,
    );
    assert!(status.success(), "{stderr}");
    let engine = ok(Repository::discover(&path), "the bare repository opens");
    for which in EVERY {
        let answer = ok(
            engine.working_tree_diff(
                super::git(),
                &RepoPath::new("seed"),
                which,
                &ContentOptions::default(),
                &CancelSignal::new(),
            ),
            "an answer",
        );
        assert!(
            matches!(
                answer.map(|diff| diff.content),
                Some(DiffContent::Unsupported { .. })
            ),
            "{which:?}"
        );
    }
}

/// The discriminating fixture's commit, put back as a working-tree change — unstaged after
/// `reset --mixed`, staged after `reset --soft` — under configurations its content tells
/// apart: every changed file of both reads as `git diff` and `git diff --cached` show it,
/// `-w` and function context included. Caught by: the algorithm not passed to plumbing, a
/// driver's algorithm overridden, or hunks taken from anywhere but git.
#[test]
fn every_discriminating_file_reads_as_git_diff_shows_it_staged_and_unstaged() {
    let configurations: [&[(&str, &str)]; 3] = [
        &[],
        &[("diff.algorithm", "patience")],
        &[
            ("diff.algorithm", "histogram"),
            ("diff.drv.algorithm", "minimal"),
        ],
    ];
    for config in configurations {
        for (reset, which) in [
            ("--mixed", WorkingTreeDiff::Unstaged),
            ("--soft", WorkingTreeDiff::Staged),
        ] {
            let repo = super::repositories::discriminating(config);
            repo.git(&["reset", "--quiet", reset, "HEAD^"]);
            let listed = match which {
                WorkingTreeDiff::Staged => repo.git(&["diff", "--cached", "--name-only", "-z"]),
                WorkingTreeDiff::Unstaged | WorkingTreeDiff::Untracked => {
                    repo.git(&["diff", "--name-only", "-z"])
                }
            };
            let paths: Vec<&str> = listed.split('\0').filter(|p| !p.is_empty()).collect();
            assert!(
                paths.len() > 60,
                "only {} files changed: {config:?}",
                paths.len()
            );
            let mut text = 0;
            for path in paths {
                let answer = ask(&repo, path, which);
                text += usize::from(answer.as_ref().is_some_and(|diff| diff.text().is_some()));
                same_as_git(&repo, path, which, &answer);
            }
            assert!(
                text > 60,
                "only {text} text files were compared: {config:?}"
            );
        }
    }
}

/// A file past the ceiling in the index whose working tree changed only its stat, or only
/// its mode, is what `git diff` shows — nothing, and a mode change — not "too large": the
/// raw record alone cannot tell a stat-only change from an edit, so git is asked for the
/// patch, under the ceiling. Caught by: answering from the raw record whenever the index
/// side is past the ceiling.
#[test]
fn a_large_file_whose_stat_or_mode_alone_moved_is_what_git_diff_shows() {
    let repo = base("large-unchanged");
    let small = ContentOptions {
        limits: cairn_model::DiffLimits {
            max_bytes: 4096,
            ..cairn_model::DiffLimits::default()
        },
        ..ContentOptions::default()
    };
    let big: String = (0..2000).map(|n| format!("{n}\n")).collect();
    repo.write("touched.txt", big.as_bytes());
    repo.write("moded.txt", big.as_bytes());
    repo.write("edited.txt", big.as_bytes());
    repo.commit("three large files");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    repo.write("touched.txt", big.as_bytes());
    repo.chmod("moded.txt", 0o755);
    repo.write("edited.txt", format!("{big}one more\n").as_bytes());
    let raw = repo.git(&["diff-files", "--name-only"]);
    assert!(
        raw.contains("touched.txt"),
        "the fixture is not stat-dirty: {raw:?}"
    );

    let touched = ask_with(&repo, "touched.txt", WorkingTreeDiff::Unstaged, &small);
    assert_eq!(touched, None);
    let moded = some(
        ask_with(&repo, "moded.txt", WorkingTreeDiff::Unstaged, &small),
        "the mode change",
    );
    assert_eq!(moded.content, DiffContent::ModeChangeOnly);
    let edited = some(
        ask_with(&repo, "edited.txt", WorkingTreeDiff::Unstaged, &small),
        "the edit",
    );
    assert!(
        matches!(edited.content, DiffContent::TooLarge { .. }),
        "{edited:?}"
    );
    same_as_git(&repo, "touched.txt", WorkingTreeDiff::Unstaged, &touched);

    // Staged, the mode change is a record whose two ids are one blob: `git diff --cached`
    // shows the mode lines and no hunk, as a commit's mode-only change answers.
    repo.git(&["add", "moded.txt"]);
    let staged = some(
        ask_with(&repo, "moded.txt", WorkingTreeDiff::Staged, &small),
        "the staged mode change",
    );
    assert_eq!(staged.content, DiffContent::ModeChangeOnly, "{staged:?}");
    assert_eq!(staged.file.new_mode, Some(FileMode::Executable));
    assert_eq!(staged.file.old_id, staged.file.new_id);
    let shown = git_diff(&repo, "moded.txt", WorkingTreeDiff::Staged, &[]);
    assert!(
        shown.contains("old mode 100644\nnew mode 100755\n") && !shown.contains("\n@@ "),
        "git diff --cached does not show a mode change alone: {shown}"
    );
    assert_eq!(
        ask_with(&repo, "moded.txt", WorkingTreeDiff::Unstaged, &small),
        None
    );
}

/// `git diff --no-index` is porcelain, and reads the user's presentation settings that the
/// plumbing reads never do. Every one found to change its output — path prefixes and their
/// quoting, hunk joining and order, a relative path — is set hostile here, with colour
/// forced, a zero default context and an external diff program besides, and the untracked
/// answer is unchanged, still what the user's own `git diff --no-index` shows in content
/// terms, the program not run, and the read seen carrying each default it pins. Caught by:
/// `--no-color` dropped (colour codes read as lines), or a key's `-c` dropped from the read.
#[test]
fn an_untracked_answer_is_the_same_under_hostile_presentation_settings() {
    let repo = base("hostile");
    repo.write("sub/new é.txt", b"a\n\nb\n\tc\nd \n");
    let calm = ask(&repo, "sub/new é.txt", WorkingTreeDiff::Untracked);

    let trap_mark = repo.path().join("external-ran");
    let trap = script(
        &repo,
        "external.sh",
        &format!("#!/bin/sh\n: > '{}'\n", trap_mark.display()),
    );
    repo.write("order", b"sub/new \xc3\xa9.txt\n");
    for (key, value) in [
        ("diff.noprefix", "true"),
        ("diff.mnemonicPrefix", "true"),
        ("diff.srcPrefix", "SRC/"),
        ("diff.dstPrefix", "DST/"),
        ("core.quotePath", "false"),
        ("diff.interHunkContext", "10"),
        ("diff.relative", "true"),
        ("diff.orderFile", "order"),
        ("diff.suppressBlankEmpty", "true"),
        ("diff.context", "0"),
        ("color.ui", "always"),
        ("color.diff", "always"),
        ("diff.colorMoved", "zebra"),
        ("diff.wsErrorHighlight", "all"),
        ("core.abbrev", "4"),
        ("diff.external", &trap.display().to_string()),
    ] {
        repo.config(key, value);
    }
    let shared = ok(SharedRepository::discover(repo.path()), "the fixture opens");
    let hostile = ok(
        shared.to_worker().working_tree_diff(
            super::git(),
            &RepoPath::new("sub/new é.txt"),
            WorkingTreeDiff::Untracked,
            &ContentOptions::default(),
            &CancelSignal::new(),
        ),
        "the untracked answer",
    );
    // Every setting but one is presentation and changes nothing. `diff.interHunkContext`
    // is how the user's `git diff` groups hunks, which the answer carries for the view to
    // group with (phase 06); the lines, ranges and texts it holds are the same.
    let (hostile_diff, calm_diff) = (
        some(hostile.as_ref(), "an answer"),
        some(calm.as_ref(), "an answer"),
    );
    assert_eq!(
        hostile_diff.file, calm_diff.file,
        "a presentation setting changed the file"
    );
    assert_eq!(
        hostile_diff.text(),
        calm_diff.text(),
        "a presentation setting changed the lines"
    );
    let (hostile_overlay, calm_overlay) = (
        some(hostile_diff.overlay(), "an overlay"),
        some(calm_diff.overlay(), "an overlay"),
    );
    assert_eq!(hostile_overlay.highlights(), calm_overlay.highlights());
    assert_eq!(
        hostile_overlay.changes_ignoring_whitespace(),
        calm_overlay.changes_ignoring_whitespace()
    );
    assert_eq!(
        hostile_overlay.function_context().len(),
        calm_overlay.function_context().len()
    );
    assert_eq!(hostile_overlay.function_context().inter_hunk_context(), 10);
    assert_eq!(calm_overlay.function_context().inter_hunk_context(), 0);
    assert!(!trap_mark.exists(), "the external diff program ran");
    let diff = some(hostile, "an answer");
    assert_eq!(lines(&diff).1, ["a", "", "b", "\tc", "d "]);
    // The user's own `git diff --no-index`, with those settings: the same lines, once its
    // forced colour is turned off (colour is presentation too).
    let theirs = git_diff(
        &repo,
        "sub/new é.txt",
        WorkingTreeDiff::Untracked,
        &["--no-color"],
    );
    assert_eq!(
        cairn_view(&diff, Context::lines(3), false),
        hunks_of(&theirs)
    );

    let log = shared.command_log();
    let read = some(
        log.iter()
            .find(|record| record.arguments.iter().any(|a| a == "--no-index")),
        "the --no-index read",
    );
    for setting in [
        "diff.noprefix=false",
        "diff.mnemonicPrefix=false",
        "diff.srcPrefix=a/",
        "diff.dstPrefix=b/",
        "core.quotePath=true",
        "diff.interHunkContext=0",
        "diff.relative=false",
        "diff.orderFile=/dev/null",
        "diff.suppressBlankEmpty=false",
    ] {
        assert!(
            read.arguments.iter().any(|argument| argument == setting),
            "the read does not pin {setting}: {:?}",
            read.arguments
        );
    }
    for flag in [
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "-U3",
        "--no-abbrev",
    ] {
        assert!(
            read.arguments.iter().any(|argument| argument == flag),
            "{flag}"
        );
    }
}
