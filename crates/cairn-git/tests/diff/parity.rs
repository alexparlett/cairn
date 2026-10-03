//! R2.4 and R2.8, enforced: the content query's unified projection is what the user's own
//! `git diff` prints — every header, the function context after it, and every line — under
//! every algorithm, with and without the indent heuristic, under a diff driver's algorithm,
//! at more than one context, ignoring whitespace, over crafted content chosen because the
//! algorithms disagree on it and over this repository's own history.
//!
//! The reference is porcelain `git diff` between the commit's parent and the commit, for the
//! file's paths, with the rename detection that pairs them — the command a user types — so
//! it reads `diff.algorithm`, the drivers and the indent heuristic exactly as they do. Only
//! `--no-ext-diff` and `--no-textconv` are added, since Cairn runs neither program (R2.3).

use cairn_git::{CancelSignal, ChangesRequest, ContentOptions, DiffSession, Repository};
use cairn_model::{
    ChangeStatus, ChangedFile, Context, DiffContent, FileDiff, Oid, TextDiff, UnifiedRow,
    UnifiedRows,
};

use super::repositories::{self, Repo};
use super::{ok, some};

/// One hunk as `git diff` spells it: the header up to its closing `@@`, the function
/// context after it, and the marked lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub header: String,
    pub function: String,
    pub lines: Vec<String>,
}

const NO_NEWLINE: &str = "\\ No newline at end of file";

/// The detection that pairs `file` the way the changes query found it.
fn detection(file: &ChangedFile) -> &'static [&'static str] {
    match file.status {
        ChangeStatus::Renamed(_) => &["-M"],
        ChangeStatus::Copied(_) => &["-C", "--find-copies-harder"],
        ChangeStatus::Added
        | ChangeStatus::Deleted
        | ChangeStatus::Modified
        | ChangeStatus::TypeChanged => &["--no-renames"],
    }
}

/// What the user's `git diff -U<context>` prints for `file` between `old` and `new`, with
/// `extra` flags (`-w`). A type change is printed by git as a deletion and an addition,
/// which Cairn draws as one change of every line, so for one the two blobs are compared.
pub fn git_view(
    repo: &Repo,
    old: &str,
    new: &str,
    file: &ChangedFile,
    context: u32,
    extra: &[&str],
) -> Vec<Hunk> {
    let unified = format!("-U{context}");
    let mut args = vec![
        "diff",
        unified.as_str(),
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--src-prefix=a/",
        "--dst-prefix=b/",
    ];
    args.extend_from_slice(extra);
    let (old_path, new_path) = (file.old_path.display(), file.new_path.display());
    let blobs;
    if file.status == ChangeStatus::TypeChanged {
        blobs = [
            some(file.old_id, "an old side").hex().as_str().to_owned(),
            some(file.new_id, "a new side").hex().as_str().to_owned(),
        ];
        args.extend([blobs[0].as_str(), blobs[1].as_str()]);
        return hunks_of(&repo.git(&args));
    }
    args.extend_from_slice(detection(file));
    args.extend([old, new, "--", &new_path]);
    if old_path != new_path {
        args.push(&old_path);
    }
    let out = repo.git(&args);
    let wanted = format!("diff --git a/{old_path} b/{new_path}");
    let section: String = out
        .split_inclusive('\n')
        .skip_while(|line| line.trim_end_matches('\n') != wanted)
        .enumerate()
        .take_while(|(index, line)| *index == 0 || !line.starts_with("diff --git "))
        .map(|(_, line)| line)
        .collect();
    hunks_of(&section)
}

fn hunks_of(out: &str) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    for line in out.split_inclusive('\n') {
        let line = line.strip_suffix('\n').unwrap_or(line);
        if let Some(after) = line.strip_prefix("@@ ") {
            let close = some(after.find(" @@"), "a header's closing @@");
            let rest = &after[close + 3..];
            hunks.push(Hunk {
                header: format!("@@ {}", &after[..close + 3]),
                function: rest.strip_prefix(' ').unwrap_or(rest).to_owned(),
                lines: Vec::new(),
            });
            continue;
        }
        let Some(hunk) = hunks.last_mut() else {
            continue; // A header line, above the first hunk.
        };
        if line.starts_with('\\') {
            hunk.lines.push(NO_NEWLINE.to_owned());
        } else if line.starts_with([' ', '+', '-']) {
            hunk.lines.push(line.to_owned());
        }
    }
    hunks
}

/// The same shape, built from the unified projection a view draws at `context`, each header
/// carrying the function context the overlay holds for it. With `ignoring_whitespace`, the
/// projection is over the whitespace-ignoring ranges and a context line is the new side's,
/// which is the side git prints it from.
pub fn cairn_view(diff: &FileDiff, context: Context, ignoring_whitespace: bool) -> Vec<Hunk> {
    let DiffContent::Text { text, overlay } = &diff.content else {
        panic!("{} is not text: {:?}", diff.file.new_path, diff.content);
    };
    let shown;
    let drawn: &TextDiff = if ignoring_whitespace {
        shown = TextDiff::new(
            text.old_lines().to_vec(),
            text.new_lines().to_vec(),
            some(
                overlay.changes_ignoring_whitespace(),
                "the whitespace-ignoring ranges",
            )
            .to_vec(),
        );
        &shown
    } else {
        text
    };
    let rows = UnifiedRows::new(drawn, context);
    let mut hunks: Vec<Hunk> = Vec::new();
    for index in 0..rows.len() {
        let row = some(rows.row(index), "a row below the count");
        let (marker, line) = match row {
            UnifiedRow::Header(header) => {
                hunks.push(Hunk {
                    header: header.to_string(),
                    function: overlay.function_context().of(header).map_or_else(
                        || "<no function context read for this hunk>".to_owned(),
                        |text| String::from_utf8_lossy(text).into_owned(),
                    ),
                    lines: Vec::new(),
                });
                continue;
            }
            UnifiedRow::Context { new, .. } => (' ', some(drawn.new_line(new), "a new line")),
            UnifiedRow::Removed { line, .. } => ('-', line),
            UnifiedRow::Added { line, .. } => ('+', line),
        };
        let hunk = some(hunks.last_mut(), "a row before its header");
        hunk.lines
            .push(format!("{marker}{}", String::from_utf8_lossy(line.bytes())));
        if !line.ends_with_newline() {
            hunk.lines.push(NO_NEWLINE.to_owned());
        }
    }
    hunks
}

/// What a comparison found.
#[derive(Debug, Default)]
pub struct Tally {
    pub files: usize,
    pub hunks: usize,
    /// Hunks whose function context git printed as something.
    pub with_function: usize,
    pub divergent: Vec<String>,
}

impl Tally {
    fn add(&mut self, other: Tally) {
        self.files += other.files;
        self.hunks += other.hunks;
        self.with_function += other.with_function;
        self.divergent.extend(other.divergent);
    }
}

/// Every text file `commit` changed, compared at `context` with `git diff`'s answer, and
/// with `git diff -w`'s when `whitespace` is set.
pub fn compare_commit(
    repo: &Repo,
    session: &mut DiffSession<'_>,
    commit: &str,
    context: u32,
    whitespace: bool,
) -> Tally {
    let id = ok(Oid::parse(commit), "a commit id");
    let request = ChangesRequest::commit(id);
    let set = ok(
        session.changes(super::git(), &request, &CancelSignal::new()),
        "the changes query answers",
    );
    let parent = match repo.try_git(&["rev-parse", &format!("{commit}^1")], &[], None) {
        Ok(parent) => parent.trim().to_owned(),
        Err(_) => repo
            .git(&["hash-object", "-t", "tree", "/dev/null"])
            .trim()
            .to_owned(),
    };
    let options = ContentOptions {
        load_anyway: true,
        ignore_whitespace: whitespace,
        context: Context::lines(context),
        ..ContentOptions::default()
    };
    let mut tally = Tally::default();
    for file in &set.files {
        let diff = ok(
            session.file_diff(super::git(), &request, file, &options, &CancelSignal::new()),
            "a file diff",
        );
        if !matches!(diff.content, DiffContent::Text { .. }) {
            continue;
        }
        let extra: &[&str] = if whitespace { &["-w"] } else { &[] };
        let theirs = git_view(repo, &parent, commit, file, context, extra);
        let ours = cairn_view(&diff, Context::lines(context), whitespace);
        tally.files += 1;
        tally.hunks += theirs.len();
        tally.with_function += theirs.iter().filter(|h| !h.function.is_empty()).count();
        if ours != theirs {
            let first = ours.iter().zip(&theirs).find(|(a, b)| a != b).map_or_else(
                || format!("{} hunks against git's {}", ours.len(), theirs.len()),
                |(a, b)| format!("Cairn {a:?}\n    git {b:?}"),
            );
            tally.divergent.push(format!(
                "{} in {commit} at -U{context}{}:\n    {first}",
                file.new_path,
                if whitespace { " -w" } else { "" }
            ));
        }
    }
    tally
}

fn assert_no_divergence(what: &str, tally: &Tally) {
    assert!(
        tally.divergent.is_empty(),
        "{what}: {} of {} files differ from `git diff`:\n{}",
        tally.divergent.len(),
        tally.files,
        tally.divergent.join("\n")
    );
}

/// The porcelain answers of two configurations differ for at least one file of `head` —
/// which is what makes a fixture able to tell them apart, and a comparison under them a
/// test rather than a formality.
fn discriminates(repo: &Repo, head: &str, one: &[&str], other: &[&str]) -> bool {
    discriminates_under(repo, head, one, other, None)
}

/// [`discriminates`], for the files under `under` alone when it is given.
fn discriminates_under(
    repo: &Repo,
    head: &str,
    one: &[&str],
    other: &[&str],
    under: Option<&str>,
) -> bool {
    let run = |flags: &[&str]| {
        let mut args: Vec<&str> = flags.to_vec();
        args.extend([
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "HEAD^",
            head,
        ]);
        if let Some(under) = under {
            args.extend(["--", under]);
        }
        repo.git(&args)
    };
    run(one) != run(other)
}

/// The configurations every discriminating comparison runs under.
const CONFIGURATIONS: &[&[(&str, &str)]] = &[
    &[],
    &[("diff.algorithm", "myers")],
    &[("diff.algorithm", "minimal")],
    &[("diff.algorithm", "patience")],
    &[("diff.algorithm", "histogram")],
    &[("diff.indentHeuristic", "false")],
    &[
        ("diff.algorithm", "histogram"),
        ("diff.indentHeuristic", "false"),
    ],
    // A driver's algorithm against `diff.algorithm`, and against git's default: the
    // driver's wins for its paths from git 2.40, and is ignored before.
    &[
        ("diff.algorithm", "histogram"),
        ("diff.drv.algorithm", "patience"),
    ],
    &[("diff.drv.algorithm", "minimal")],
];

/// R2.4 on content the algorithms disagree on: under every configuration, at three lines
/// of context, every header, function context and line is `git diff -U3`'s. The fixture is
/// first shown to discriminate — each algorithm's answer differs from myers', patience's
/// from histogram's, the indent heuristic's from its absence, and, on a git that reads
/// one, the driver's algorithm from `diff.algorithm` — so agreeing is not agreeing about
/// nothing. Caught by: dropping `--diff-algorithm`, asking at `-U0`, reading a change from a
/// header, taking the function context from anywhere but git's own header.
#[test]
fn every_discriminating_file_reads_as_git_diff_shows_it_under_every_configuration() {
    let probe = repositories::discriminating(&[]);
    let head = probe.git(&["rev-parse", "HEAD"]).trim().to_owned();
    for (one, other) in [
        (
            &["-c", "diff.algorithm=myers"][..],
            &["-c", "diff.algorithm=minimal"][..],
        ),
        (
            &["-c", "diff.algorithm=myers"],
            &["-c", "diff.algorithm=patience"],
        ),
        (
            &["-c", "diff.algorithm=myers"],
            &["-c", "diff.algorithm=histogram"],
        ),
        (
            &["-c", "diff.algorithm=patience"],
            &["-c", "diff.algorithm=histogram"],
        ),
        (
            &["-c", "diff.indentHeuristic=true"],
            &["-c", "diff.indentHeuristic=false"],
        ),
    ] {
        assert!(
            discriminates(&probe, &head, one, other),
            "the fixture does not tell {one:?} from {other:?}"
        );
    }
    let drivers_read = super::git().version()
        >= cairn_git::ops::GitVersion {
            major: 2,
            minor: 40,
            patch: 0,
        };
    if drivers_read {
        assert!(
            discriminates(
                &probe,
                &head,
                &["-c", "diff.algorithm=histogram"],
                &[
                    "-c",
                    "diff.algorithm=histogram",
                    "-c",
                    "diff.drv.algorithm=patience"
                ],
            ),
            "the fixture does not tell a driver's algorithm from diff.algorithm"
        );
        // The last configuration, a driver's algorithm alone, changes the driver's files.
        assert!(
            discriminates_under(
                &probe,
                &head,
                &[],
                &["-c", "diff.drv.algorithm=minimal"],
                Some("drv"),
            ),
            "the fixture's drv/ files read the same under diff.drv.algorithm=minimal as \
             under git's default, so that configuration decides nothing"
        );
    }

    let mut total = Tally::default();
    for config in CONFIGURATIONS {
        let repo = repositories::discriminating(config);
        let engine = ok(Repository::discover(repo.path()), "the fixture opens");
        let mut session = ok(engine.diff_session(), "a diff session");
        let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
        let tally = compare_commit(&repo, &mut session, &head, 3, false);
        assert_no_divergence(&format!("{config:?}"), &tally);
        assert!(
            tally.files >= 70,
            "{config:?}: only {} files compared",
            tally.files
        );
        total.add(tally);
    }
    assert!(
        total.with_function > 0,
        "no hunk carried function context, so none was compared"
    );
    eprintln!(
        "discriminating fixture: {} files, {} hunks ({} with function context) under {} \
         configurations, none differing from git diff -U3",
        total.files,
        total.hunks,
        total.with_function,
        CONFIGURATIONS.len()
    );
}

/// The function context a hunk carries depends on where it starts, so on the context: at
/// one, three, five and eight lines, with git's default rule (`code/`) and a driver's
/// `xfuncname` capturing part of the line (`drv/`), each header's text is `git diff`'s.
/// Caught by: function context read once at one context and shown at another, or read from
/// the wrong side of the header.
#[test]
fn the_function_context_is_git_diffs_at_every_context() {
    let repo = repositories::discriminating(&[]);
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    let mut session = ok(engine.diff_session(), "a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let mut texts_by_context = Vec::new();
    for context in [1, 3, 5, 8] {
        let tally = compare_commit(&repo, &mut session, &head, context, false);
        assert_no_divergence(&format!("-U{context}"), &tally);
        texts_by_context.push(tally.with_function);
    }
    assert!(
        texts_by_context.iter().all(|count| *count > 0),
        "a context with no function context compared: {texts_by_context:?}"
    );

    // Both rules are reached: the driver's capture (a bare name) and the default (a line).
    let request = ChangesRequest::commit(ok(Oid::parse(&head), "an id"));
    let set = ok(
        session.changes(super::git(), &request, &CancelSignal::new()),
        "the changes query answers",
    );
    let shown = |path: &str| -> Vec<String> {
        let file = some(
            set.files
                .iter()
                .find(|file| file.new_path.display() == path),
            "the file changed",
        );
        let diff = ok(session_file_diff(&engine, &request, file, 3), "a file diff");
        cairn_view(&diff, Context::lines(3), false)
            .into_iter()
            .map(|hunk| hunk.function)
            .collect()
    };
    assert!(
        shown("drv/subs.pl").iter().any(|text| text == "bravo"),
        "the driver's capture group did not reach a header: {:?}",
        shown("drv/subs.pl")
    );
    assert!(
        shown("code/functions.c")
            .iter()
            .any(|text| text.starts_with("int ")),
        "git's default rule did not reach a header: {:?}",
        shown("code/functions.c")
    );
}

fn session_file_diff(
    engine: &Repository,
    request: &ChangesRequest,
    file: &ChangedFile,
    context: u32,
) -> Result<FileDiff, cairn_git::Error> {
    engine.file_diff(
        super::git(),
        request,
        file,
        &ContentOptions {
            context: Context::lines(context),
            ..ContentOptions::default()
        },
        &CancelSignal::new(),
    )
}

/// R2.8: ignoring whitespace is `git diff -w`, header, function context and line, on files
/// with whitespace-only edits, real ones, and both — and the notice that changes are hidden
/// is still decided: said for a file whose only edits are whitespace, and not for one whose
/// edits are all real. Caught by: comparing lines with their whitespace stripped by Cairn's
/// own rule, which places a slider elsewhere than git does.
#[test]
fn ignoring_whitespace_reads_as_git_diff_w_shows_it() {
    let repo = repositories::whitespace();
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    let mut session = ok(engine.diff_session(), "a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let tally = compare_commit(&repo, &mut session, &head, 3, true);
    assert_no_divergence("-w", &tally);
    assert!(tally.files >= 30, "only {} files compared", tally.files);

    let discriminating = repositories::discriminating(&[]);
    let engine_d = ok(
        Repository::discover(discriminating.path()),
        "the fixture opens",
    );
    let mut session_d = ok(engine_d.diff_session(), "a diff session");
    let head_d = discriminating.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let tally = compare_commit(&discriminating, &mut session_d, &head_d, 3, true);
    assert_no_divergence("-w over the discriminating fixture", &tally);

    let request = ChangesRequest::commit(ok(Oid::parse(&head), "an id"));
    let set = ok(
        session.changes(super::git(), &request, &CancelSignal::new()),
        "the changes query answers",
    );
    let hides = |path: &str| -> bool {
        let file = some(
            set.files
                .iter()
                .find(|file| file.new_path.display() == path),
            "the file changed",
        );
        let diff = ok(
            engine.file_diff(
                super::git(),
                &request,
                file,
                &ContentOptions {
                    ignore_whitespace: true,
                    ..ContentOptions::default()
                },
                &CancelSignal::new(),
            ),
            "a file diff",
        );
        let DiffContent::Text { text, overlay } = &diff.content else {
            panic!("{path} is not text");
        };
        overlay.hides_a_change(text)
    };
    assert!(
        hides("only-whitespace.c"),
        "hidden whitespace edits went unannounced"
    );
    assert!(
        !hides("only-real.c"),
        "a file with real edits only claimed to hide some"
    );
}

/// R2.4 over real content nobody crafted: up to the last 150 non-merge commits of this
/// repository, under each algorithm, every text file compared with `git diff -U3`, with no
/// divergence allowed. The content-parity spike found gix differing on 9 of 262 such files
/// under myers. Run in a clone sharing this checkout's objects, so the configuration each
/// algorithm needs is never written to the checkout itself.
#[test]
fn this_repositorys_history_reads_as_git_diff_shows_it_under_every_algorithm() {
    let here = ok(
        Repository::discover(env!("CARGO_MANIFEST_DIR")),
        "this checkout opens",
    );
    let workdir = some(here.workdir(), "a working tree").to_owned();
    let mut summary = Vec::new();
    for algorithm in ["myers", "minimal", "patience", "histogram"] {
        let clone = Repo::shared_clone_of(&workdir, "history-parity");
        clone.config("diff.algorithm", algorithm);
        let engine = ok(Repository::discover(clone.path()), "the clone opens");
        let mut session = ok(engine.diff_session(), "a diff session");
        let commits: Vec<String> = clone
            .git(&["rev-list", "--no-merges", "--max-count=150", "HEAD"])
            .lines()
            .map(str::to_owned)
            .collect();
        assert!(commits.len() >= 10, "only {} commits", commits.len());
        let mut tally = Tally::default();
        for commit in &commits {
            tally.add(compare_commit(&clone, &mut session, commit, 3, false));
        }
        assert_no_divergence(algorithm, &tally);
        assert!(tally.files >= 50, "{algorithm}: only {} files", tally.files);
        summary.push(format!(
            "{algorithm}: {} commits, {} files, {} hunks",
            commits.len(),
            tally.files,
            tally.hunks
        ));
    }
    eprintln!(
        "this repository's history, none differing from git diff -U3: {}",
        summary.join("; ")
    );
}

/// Expand All answers every file exactly as asking about each file alone does — with
/// whitespace ignored and not, over renames and copies, hidden and listed submodules, a
/// driver whose algorithm sends its files to a read of their own, and the crafted edge
/// cases. It decides the answers, not how many processes gave them: asking about every
/// file alone passes it too, which is what
/// `expand_all_runs_one_diff_tree_per_comparison` counts. Caught by: a patch matched to
/// the wrong file, the whole-comparison read asked with other detection than the change
/// set's, or a file the answer does not hold dropped rather than asked about alone.
#[test]
fn expand_all_answers_what_each_file_answers_alone() {
    let fixtures = [
        repositories::crafted(),
        repositories::rewrites(&[("diff.renames", "copies")]),
        repositories::rewrites(&[("diff.renames", "false")]),
        repositories::discriminating(&[
            ("diff.algorithm", "histogram"),
            ("diff.drv.algorithm", "patience"),
        ]),
        repositories::whitespace(),
        repositories::submodules(&[("diff.ignoreSubmodules", "all")]),
        repositories::attributes(),
    ];
    let mut files = 0usize;
    for repo in &fixtures {
        let engine = ok(Repository::discover(repo.path()), "the fixture opens");
        let mut session = ok(engine.diff_session(), "a diff session");
        let commits: Vec<String> = repo
            .git(&["rev-list", "--no-merges", "HEAD"])
            .lines()
            .map(str::to_owned)
            .collect();
        for commit in &commits {
            let request = ChangesRequest::commit(ok(Oid::parse(commit), "an id"));
            let set = ok(
                session.changes(super::git(), &request, &CancelSignal::new()),
                "the changes query answers",
            );
            for ignore_whitespace in [false, true] {
                let options = ContentOptions {
                    ignore_whitespace,
                    ..ContentOptions::default()
                };
                let all = ok(
                    session.file_diffs(
                        super::git(),
                        &request,
                        &set,
                        &options,
                        &CancelSignal::new(),
                    ),
                    "Expand All answers",
                );
                assert_eq!(all.len(), set.files.len());
                for (expanded, file) in all.iter().zip(&set.files) {
                    let alone = ok(
                        session.file_diff(
                            super::git(),
                            &request,
                            file,
                            &options,
                            &CancelSignal::new(),
                        ),
                        "a file diff",
                    );
                    assert_eq!(
                        expanded, &alone,
                        "{} in {commit} (ignoring whitespace: {ignore_whitespace})",
                        file.new_path
                    );
                    files += 1;
                }
            }
        }
    }
    assert!(files >= 300, "only {files} files compared");
}

/// How many `git diff-tree` runs Expand All makes, read from the repository's command log:
/// one for the whole comparison, and a second when whitespace is ignored — never one per
/// file — over a fixture of seventy-odd text files, renames and copies among them, where
/// every file is held by the one answer. Its answers are shown equal to the per-file ones
/// by `expand_all_answers_what_each_file_answers_alone`; this is what that test cannot
/// see. Caught by: every file sent to a read of its own, which answers the same.
#[test]
fn expand_all_runs_one_diff_tree_per_comparison() {
    let repo = repositories::discriminating(&[]);
    let shared = ok(
        cairn_git::SharedRepository::discover(repo.path()),
        "the fixture opens",
    );
    let engine = shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let request = ChangesRequest::commit(repo.rev("HEAD"));
    let set = ok(
        session.changes(super::git(), &request, &CancelSignal::new()),
        "the changes query answers",
    );
    assert!(set.files.len() >= 70, "only {} files", set.files.len());
    let diff_trees = || {
        shared
            .command_log()
            .iter()
            .filter(|record| record.arguments.iter().any(|a| a == "diff-tree"))
            .count()
    };
    for (ignore_whitespace, expected) in [(false, 1), (true, 2)] {
        let before = diff_trees();
        let options = ContentOptions {
            ignore_whitespace,
            load_anyway: true,
            ..ContentOptions::default()
        };
        let all = ok(
            session.file_diffs(super::git(), &request, &set, &options, &CancelSignal::new()),
            "Expand All answers",
        );
        assert_eq!(all.len(), set.files.len());
        assert_eq!(
            diff_trees() - before,
            expected,
            "Expand All (ignoring whitespace: {ignore_whitespace}) ran another number of \
             diff-tree than one per comparison"
        );
    }
}

/// The content query skips git where git has one answer: a file added or deleted, emptied
/// or filled, under a diff driver with an `xfuncname` and without; a rename that kept its
/// blob, which git prints with no hunk at all; and a type change, which git prints as a
/// deletion of every old line and an addition of every new one — each of the last two
/// first shown to be git's own answer. Each still reads as `git diff` shows it, function
/// context — which git prints none of for a hunk starting at the first line — included,
/// and no process starts. Caught by: git asked about any of them, or one of them answered
/// otherwise than git does.
#[test]
fn a_file_git_is_not_asked_about_still_reads_as_git_diff_shows_it() {
    let repo = repositories::discriminating(&[]);
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    let mut session = ok(engine.diff_session(), "a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let parent = repo.git(&["rev-parse", "HEAD^"]).trim().to_owned();
    let request = ChangesRequest::commit(ok(Oid::parse(&head), "an id"));
    let set = ok(
        session.changes(super::git(), &request, &CancelSignal::new()),
        "the changes query answers",
    );
    let shared = ok(
        cairn_git::SharedRepository::discover(repo.path()),
        "the fixture opens",
    );
    let worker = shared.to_worker();
    let mut skipped = 0usize;
    for path in [
        "added.c",
        "drv/added.pl",
        "deleted.c",
        "drv/deleted.pl",
        "emptied.c",
        "drv/emptied.pl",
        "filled.c",
        "drv/filled.pl",
    ] {
        let file = some(
            set.files
                .iter()
                .find(|file| file.new_path.display() == path),
            "the file changed",
        );
        for context in [1, 3] {
            let before = shared.command_log().len();
            let diff = ok(
                worker.file_diff(
                    super::git(),
                    &request,
                    file,
                    &ContentOptions {
                        context: Context::lines(context),
                        ..ContentOptions::default()
                    },
                    &CancelSignal::new(),
                ),
                "a file diff",
            );
            assert_eq!(
                shared.command_log().len(),
                before,
                "{path}: git was asked about a file with one answer"
            );
            assert_eq!(
                cairn_view(&diff, Context::lines(context), false),
                git_view(&repo, &parent, &head, file, context, &[]),
                "{path} at -U{context}"
            );
            skipped += 1;
        }
    }
    assert_eq!(skipped, 16);

    // A rename that kept its blob, and a type change (the crafted history's last commit
    // turns `crlf.txt` into a link).
    let kept = Repo::new("kept-blob");
    kept.write("same.txt", b"one\ntwo\nthree\n");
    kept.commit("seed");
    kept.git(&["mv", "same.txt", "moved.txt"]);
    kept.commit("rename, nothing else");
    let crafted = repositories::crafted();
    for (repo, path) in [(&kept, "moved.txt"), (&crafted, "crlf.txt")] {
        let shared = ok(
            cairn_git::SharedRepository::discover(repo.path()),
            "the fixture opens",
        );
        let worker = shared.to_worker();
        let (head, parent) = (repo.rev("HEAD"), repo.rev("HEAD^"));
        let request = ChangesRequest::commit(head);
        let set = ok(
            worker.changes(super::git(), &request, &CancelSignal::new()),
            "the changes query answers",
        );
        let file = some(
            set.files
                .iter()
                .find(|file| file.new_path.display() == path),
            "the file changed",
        );
        let porcelain = repo.git(&["diff", "-M", "HEAD^", "HEAD"]);
        let sections = porcelain
            .split("diff --git ")
            .filter(|section| !section.is_empty())
            .collect::<Vec<_>>();
        match file.status {
            ChangeStatus::Renamed(_) => {
                assert_eq!(file.old_id, file.new_id, "{path} kept its blob");
                assert!(
                    sections.len() == 1
                        && sections[0].contains("similarity index 100%")
                        && !sections[0].contains("@@"),
                    "git prints a rename that kept its blob otherwise: {porcelain}"
                );
            }
            ChangeStatus::TypeChanged => {
                assert!(
                    sections.len() == 2
                        && sections[0].contains("deleted file mode")
                        && sections[1].contains("new file mode 120000"),
                    "git prints a type change otherwise: {porcelain}"
                );
            }
            other => panic!("{path} is {other:?}, not the change this checks"),
        }
        let before = shared.command_log().len();
        let diff = ok(
            worker.file_diff(
                super::git(),
                &request,
                file,
                &ContentOptions::default(),
                &CancelSignal::new(),
            ),
            "a file diff",
        );
        assert_eq!(
            shared.command_log().len(),
            before,
            "{path}: git was asked about a file with one answer"
        );
        let ours = cairn_view(&diff, Context::lines(3), false);
        let theirs = git_view(repo, &parent.to_string(), &head.to_string(), file, 3, &[]);
        assert_eq!(ours, theirs, "{path}");
        if file.status == ChangeStatus::TypeChanged {
            // git's two sections, read as one: every old line removed, every new one added.
            let lines_of = |section: &str, marker: char| -> Vec<String> {
                // Split at `\n` alone, so a CRLF line keeps its `\r` as git printed it.
                section
                    .split('\n')
                    .skip_while(|line| !line.starts_with("@@"))
                    .filter(|line| line.starts_with(marker) || line.starts_with('\\'))
                    .map(str::to_owned)
                    .collect()
            };
            let mut printed = lines_of(sections[0], '-');
            printed.extend(lines_of(sections[1], '+'));
            assert_eq!(ours.len(), 1, "{path}: one change of every line");
            assert_eq!(ours[0].lines, printed, "{path} against git's two sections");
        } else {
            assert!(
                ours.is_empty(),
                "{path}: a rename that kept its blob has no hunk"
            );
        }
    }
}

/// What the user's `git diff -U3` prints for `file` of `HEAD`, its paths given as literal
/// pathspecs and the whole output read, so a path git quotes in its `diff --git` line (or a
/// glob character in it) is still one file's answer: the output is required to hold exactly
/// one file.
fn literal_git_view(repo: &Repo, file: &ChangedFile, config: &[&str]) -> Vec<Hunk> {
    let (old_path, new_path) = (file.old_path.display(), file.new_path.display());
    let mut args: Vec<&str> = config.to_vec();
    args.extend([
        "--literal-pathspecs",
        "diff",
        "-U3",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
    ]);
    args.extend_from_slice(detection(file));
    args.extend(["HEAD^", "HEAD", "--", &new_path]);
    if old_path != new_path {
        args.push(&old_path);
    }
    let out = repo.git(&args);
    assert_eq!(
        out.lines()
            .filter(|line| line.starts_with("diff --git "))
            .count(),
        1,
        "git diff for {new_path:?} is not one file's: {out}"
    );
    hunks_of(&out)
}

/// `git diff --name-status -z` of `HEAD` as (status letter, old path, new path).
fn name_status(repo: &Repo) -> std::collections::BTreeSet<(char, Vec<u8>, Vec<u8>)> {
    let out = repo.git(&["diff", "--name-status", "-z", "-M", "HEAD^", "HEAD"]);
    let mut fields = out.split('\0').filter(|field| !field.is_empty());
    let mut found = std::collections::BTreeSet::new();
    while let Some(status) = fields.next() {
        let letter = some(status.chars().next(), "a status letter");
        let first = some(fields.next(), "a path").as_bytes().to_vec();
        let second = if matches!(letter, 'R' | 'C') {
            some(fields.next(), "a second path").as_bytes().to_vec()
        } else {
            first.clone()
        };
        found.insert((letter, first, second));
    }
    found
}

fn letter(status: &ChangeStatus) -> char {
    match status {
        ChangeStatus::Added => 'A',
        ChangeStatus::Deleted => 'D',
        ChangeStatus::Modified => 'M',
        ChangeStatus::TypeChanged => 'T',
        ChangeStatus::Renamed(_) => 'R',
        ChangeStatus::Copied(_) => 'C',
    }
}

/// Every file of `HEAD` asked about alone and through Expand All: the two answers equal,
/// and each equal to `git diff` for its paths, under `config` (`-c` pairs, as git's own
/// global options). Returns how many files were compared.
fn alone_and_expanded_read_as_git_diff(repo: &Repo, config: &[&str]) -> usize {
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    let mut session = ok(engine.diff_session(), "a diff session");
    let head = repo.rev("HEAD");
    let request = ChangesRequest::commit(head);
    let set = ok(
        session.changes(super::git(), &request, &CancelSignal::new()),
        "the changes query answers",
    );
    let options = ContentOptions {
        load_anyway: true,
        ..ContentOptions::default()
    };
    let all = ok(
        session.file_diffs(super::git(), &request, &set, &options, &CancelSignal::new()),
        "Expand All answers",
    );
    assert_eq!(all.len(), set.files.len());
    for (expanded, file) in all.iter().zip(&set.files) {
        let alone = ok(
            session.file_diff(super::git(), &request, file, &options, &CancelSignal::new()),
            "a file diff",
        );
        assert_eq!(
            expanded, &alone,
            "{:?}: Expand All against alone",
            file.new_path
        );
        assert_eq!(
            cairn_view(&alone, Context::lines(3), false),
            literal_git_view(repo, file, config),
            "{:?} against git diff",
            file.new_path
        );
    }
    set.files.len()
}

/// C6 on paths git quotes: a space, a double quote, a tab, a newline, a backslash, a
/// non-ASCII letter, glob characters and a leading `-`, each edited, and a rename from one
/// such path to another. The changes query lists exactly `git diff --name-status -z`'s
/// pairs, and every file reads as `git diff` shows it, alone and through Expand All, whose
/// one `diff-tree -p` prints most of them under a quoted `diff --git` line. The fixture is
/// first shown to make git quote. Caught by: a patch matched to its file by the `diff --git`
/// line's paths, a pathspec read as a glob, or a path read up to a space.
#[test]
fn a_path_git_quotes_reads_as_git_diff_shows_it_alone_and_through_expand_all() {
    let repo = repositories::unusual_paths();
    let porcelain = repo.git(&["diff", "-M", "HEAD^", "HEAD"]);
    assert!(
        porcelain
            .lines()
            .filter(|line| line.starts_with("diff --git \""))
            .count()
            >= 4,
        "git quotes too few of the fixture's paths for this to decide anything:\n{porcelain}"
    );

    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    let mut session = ok(engine.diff_session(), "a diff session");
    let set = ok(
        session.changes(
            super::git(),
            &ChangesRequest::commit(repo.rev("HEAD")),
            &CancelSignal::new(),
        ),
        "the changes query answers",
    );
    let listed: std::collections::BTreeSet<(char, Vec<u8>, Vec<u8>)> = set
        .files
        .iter()
        .map(|file| {
            (
                letter(&file.status),
                file.old_path.as_bytes().to_vec(),
                file.new_path.as_bytes().to_vec(),
            )
        })
        .collect();
    assert_eq!(listed, name_status(&repo), "the changes query against git");
    for path in repositories::UNUSUAL_PATHS {
        assert!(
            set.files
                .iter()
                .any(|file| file.new_path.as_bytes() == path.as_bytes()),
            "{path:?} is not listed"
        );
    }
    assert!(
        set.files
            .iter()
            .any(|file| matches!(file.status, ChangeStatus::Renamed(_))),
        "the rename was not found"
    );

    let compared = alone_and_expanded_read_as_git_diff(&repo, &[]);
    assert_eq!(compared, repositories::UNUSUAL_PATHS.len() + 1);
}

/// git chooses a renamed file's diff driver by its OLD path (`run_diff` in `diff.c` looks
/// the driver up by `one->path`), so with `diff.drv.algorithm = minimal`, a file renamed out
/// of `drv/` is diffed with minimal and one renamed into it with myers — shown here on git
/// itself, where the fixture's content tells the two algorithms apart. Each reads as `git
/// diff` shows it, alone and through Expand All. Before git 2.40 no driver names an
/// algorithm, and both read as myers. Caught by: the driver looked up by the new path, in
/// the per-file read or in Expand All.
#[test]
fn a_renamed_files_driver_algorithm_is_its_old_paths() {
    let repo = repositories::renamed_across_a_driver(&[]);
    let diff = |flags: &[&str], new: &str, old: &str| {
        let mut args = vec!["diff", "-M", "--no-ext-diff", "--no-color"];
        args.extend_from_slice(flags);
        args.extend(["HEAD^", "HEAD", "--", new, old]);
        repo.git(&args)
    };
    for (new, old) in [("out/in.pl", "drv/in.pl"), ("drv/out.pl", "out/out.pl")] {
        let myers = diff(&["--diff-algorithm=myers"], new, old);
        let minimal = diff(&["--diff-algorithm=minimal"], new, old);
        assert!(
            myers.contains("rename from"),
            "{old} -> {new} is not a rename"
        );
        assert_ne!(
            myers, minimal,
            "{old} -> {new}: the fixture does not tell myers from minimal"
        );
        let drivers_read = super::git().version()
            >= cairn_git::ops::GitVersion {
                major: 2,
                minor: 40,
                patch: 0,
            };
        let configured = diff(&[], new, old);
        let expected = if drivers_read && old.starts_with("drv/") {
            &minimal
        } else {
            &myers
        };
        assert_eq!(
            &configured, expected,
            "{old} -> {new}: git's own rule is not the old path's driver"
        );
    }
    assert_eq!(alone_and_expanded_read_as_git_diff(&repo, &[]), 2);
}
